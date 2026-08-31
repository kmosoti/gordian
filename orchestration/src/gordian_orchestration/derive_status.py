"""Derive Project 9's Wave, Fan In, Fan Out and Status from the native blocked-by graph.

Every value this module computes is a **projection of GitHub's own issue-dependency
edges** onto the temporary Project 9 board. The module owns no Mission Graph semantics:
it does not decide Atom readiness, hard dependency validity, evidence compatibility,
candidate admission, or satisfaction. Those decisions belong to Rust. This module exists
only because the bootstrap Mission has no native dependency store yet, and **it is
deleted when #48 lands**.

Edge source
-----------
Edges are read from the native `blockedBy` connection, either live through
`gh api graphql` or from a strict, coherent `artifacts/atoms/issues.json` registry
snapshot. The connection's `totalCount` is used to prove pagination retrieved every edge.
`issueDependenciesSummary` is never read: its blocking counter has been observed wrong
for #11, #18 and #44, and issue #70 explicitly excludes it as an edge source.
Closed issues carrying GitHub's `duplicate` label are not executable Atom contracts and
are excluded; an open duplicate is a reconciliation error rather than a hidden row.

Derived fields
--------------
Wave
    Longest-path depth in the blocked-by DAG. `Wave = 0` for an Atom with no blockers,
    otherwise `1 + max(Wave of its blockers)`. Longest path, not shortest: an Atom is not
    reachable earlier than its slowest prerequisite.
Fan In
    In-degree: the number of direct blockers.
Fan Out
    Out-degree: the number of open or closed issues this one directly blocks, counted
    over the edge lists rather than over any GitHub counter.
Status
    `Ready` iff the issue is open and every blocker is satisfied; `Blocked` otherwise.
    `In Progress`, `In Review` and `Accepted` are human-owned: this module never writes
    over them and never derives them.

Selection order
---------------
The `ready` subcommand prints its rows in the total selection order that
`docs/implementation/execution-order.md` section 5 states once (G-530): lowest `Wave`,
then highest `Fan Out`, then lowest issue number. The runbook's selection step is "take
the first unclaimed row of that output", so the order printed here is the order an agent
follows. The `derive` report keeps ascending issue order, because it is a board-writer
input rather than a work queue.

Bootstrap satisfaction
----------------------
During bootstrap an Atom counts as satisfied for readiness **iff** its issue is closed
**and** `artifacts/atoms/<N>/closure.json` exists at the accepted `trunk()` commit **and**
the record and every named verifier artifact validate there against the closure-record
schema. This is the bootstrap analogue of Satisfied-as-admitted; closing an issue on its
own is bookkeeping, not evidence. A registry snapshot is inspection-only and can never
produce dispatchable rows because its assignee/claim state may be stale.
"""

from __future__ import annotations

import argparse
import heapq
import json
import subprocess
import sys
from collections.abc import Iterable, Mapping, Sequence
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

from . import provenance
from .closure_validation import (
    BytesReader,
    closure_problems,
    jj_source_resolver,
    load_json,
    local_bytes_reader,
    parse_rfc3339,
)
from .gh import EX_CONFIG, GH_AUTH_HINT, GitHubConfigurationError, graphql, preflight

DEFAULT_OWNER = "kmosoti"
DEFAULT_REPOSITORY = "kmosoti/gordian"
DEFAULT_PROJECT_NUMBER = 9
DEFAULT_SNAPSHOT = Path("artifacts/atoms/issues.json")
DEFAULT_CLOSURE_ROOT = Path("artifacts/atoms")
DEFAULT_CLOSURE_SCHEMA = Path("artifacts/schema/closure-record.schema.json")

STATUS_FIELD = "Status"
WAVE_FIELD = "Wave"
FAN_IN_FIELD = "Fan In"
FAN_OUT_FIELD = "Fan Out"

READY = "Ready"
BLOCKED = "Blocked"
HUMAN_OWNED_STATUSES = ("In Progress", "In Review", "Accepted")
DERIVED_STATUSES = (READY, BLOCKED)


class DependencyCycleError(RuntimeError):
    """The blocked-by edges do not form a DAG, so no wave assignment exists."""


# --------------------------------------------------------------------------------------
# Pure graph math. Nothing below this banner performs I/O.
# --------------------------------------------------------------------------------------


def issue_number(value: Any) -> int:
    """Coerce a GitHub node, `#N` string, or integer into an issue number."""
    if isinstance(value, bool):
        raise TypeError("issue number must not be a boolean")
    if isinstance(value, int):
        return value
    if isinstance(value, str):
        return int(value.strip().lstrip("#"))
    if isinstance(value, Mapping):
        return issue_number(value["number"])
    raise TypeError(f"cannot read an issue number from {value!r}")


def normalise_graph(raw: Mapping[Any, Iterable[Any]]) -> dict[int, tuple[int, ...]]:
    """Return a total, deduplicated, self-edge-free blocked-by graph.

    Every number referenced as a blocker becomes a node, so the graph is closed under its
    own edges even when the caller passed a partial listing.
    """
    graph: dict[int, tuple[int, ...]] = {}
    for key, blockers in raw.items():
        node = issue_number(key)
        edges = {issue_number(blocker) for blocker in blockers}
        edges.discard(node)
        graph[node] = tuple(sorted(edges))
    for blockers in list(graph.values()):
        for blocker in blockers:
            graph.setdefault(blocker, ())
    return dict(sorted(graph.items()))


def dependents(graph: Mapping[int, Sequence[int]]) -> dict[int, tuple[int, ...]]:
    """Invert the graph: for each issue, the issues it directly blocks."""
    inverted: dict[int, list[int]] = {node: [] for node in graph}
    for node, blockers in graph.items():
        for blocker in blockers:
            inverted.setdefault(blocker, []).append(node)
    return {node: tuple(sorted(edges)) for node, edges in sorted(inverted.items())}


def topological_order(graph: Mapping[int, Sequence[int]]) -> tuple[int, ...]:
    """Return every node with blockers before dependents, smallest number first."""
    forward = dependents(graph)
    remaining = {node: len(set(blockers)) for node, blockers in graph.items()}
    frontier = [node for node, degree in remaining.items() if degree == 0]
    heapq.heapify(frontier)
    order: list[int] = []
    while frontier:
        node = heapq.heappop(frontier)
        order.append(node)
        for dependent in forward.get(node, ()):
            remaining[dependent] -= 1
            if remaining[dependent] == 0:
                heapq.heappush(frontier, dependent)
    if len(order) != len(graph):
        residue = tuple(sorted(node for node, degree in remaining.items() if degree > 0))
        raise DependencyCycleError(f"blocked-by cycle among issues {list(residue)}")
    return tuple(order)


def waves(graph: Mapping[int, Sequence[int]]) -> dict[int, int]:
    """Return the longest-path depth of every node."""
    depth: dict[int, int] = {}
    for node in topological_order(graph):
        blockers = graph[node]
        depth[node] = 0 if not blockers else 1 + max(depth[blocker] for blocker in blockers)
    return dict(sorted(depth.items()))


def fan_in(graph: Mapping[int, Sequence[int]]) -> dict[int, int]:
    """Return the in-degree (direct blocker count) of every node."""
    return {node: len(set(blockers)) for node, blockers in sorted(graph.items())}


def fan_out(graph: Mapping[int, Sequence[int]]) -> dict[int, int]:
    """Return the out-degree (directly blocked count) of every node."""
    return {node: len(edges) for node, edges in dependents(graph).items()}


def transitive_blockers(graph: Mapping[int, Sequence[int]], node: int) -> tuple[int, ...]:
    """Return the transitive blocker closure of one node."""
    seen: set[int] = set()
    stack = list(graph.get(node, ()))
    while stack:
        current = stack.pop()
        if current in seen:
            continue
        seen.add(current)
        stack.extend(graph.get(current, ()))
    return tuple(sorted(seen))


# --------------------------------------------------------------------------------------
# Records
# --------------------------------------------------------------------------------------


@dataclass(frozen=True, slots=True)
class IssueRecord:
    number: int
    title: str
    state: str
    blocked_by: tuple[int, ...]
    body: str = ""
    labels: tuple[str, ...] = ()
    milestone: str | None = None
    url: str = ""
    assignees: tuple[str, ...] = ()

    @property
    def is_open(self) -> bool:
        return self.state.upper() == "OPEN"


@dataclass(frozen=True, slots=True)
class BoardItem:
    item_id: str
    number: int
    status: str | None = None
    wave: int | None = None
    fan_in: int | None = None
    fan_out: int | None = None

    def value(self, field_name: str) -> str | int | None:
        return {
            STATUS_FIELD: self.status,
            WAVE_FIELD: self.wave,
            FAN_IN_FIELD: self.fan_in,
            FAN_OUT_FIELD: self.fan_out,
        }[field_name]


@dataclass(frozen=True, slots=True)
class ProjectField:
    field_id: str
    name: str
    data_type: str
    options: tuple[tuple[str, str], ...] = ()

    def option_id(self, option_name: str) -> str:
        for name, identifier in self.options:
            if name == option_name:
                return identifier
        raise RuntimeError(f"Project field {self.name!r} has no option {option_name!r}")


@dataclass(frozen=True, slots=True)
class Board:
    project_id: str
    fields: dict[str, ProjectField]
    items: dict[int, BoardItem]


@dataclass(frozen=True, slots=True)
class DerivedRow:
    number: int
    title: str
    state: str
    wave: int
    fan_in: int
    fan_out: int
    status: str | None
    blocked_by: tuple[int, ...]
    unsatisfied_blockers: tuple[int, ...]

    @property
    def ready(self) -> bool:
        return self.status == READY

    def as_json_object(self) -> dict[str, Any]:
        return {**asdict(self), "ready": self.ready}


@dataclass(frozen=True, slots=True)
class Change:
    number: int
    item_id: str
    field_name: str
    current: str | int | None
    desired: str | int

    def as_json_object(self) -> dict[str, Any]:
        return asdict(self)


# --------------------------------------------------------------------------------------
# Derivation
# --------------------------------------------------------------------------------------


def derived_status(
    record: IssueRecord, satisfied: frozenset[int], current: str | None
) -> str | None:
    """Return the Status this issue should carry, or None to leave the field alone.

    `In Progress`, `In Review` and `Accepted` are set by a human against a claim, a pull
    request, and a closure record respectively. A derivation that overwrote them would
    erase the only signal the board carries that is not already in the graph.
    """
    if current in HUMAN_OWNED_STATUSES:
        return None
    if not record.is_open:
        return None
    if record.assignees:
        return None
    if all(blocker in satisfied for blocker in record.blocked_by):
        return READY
    return BLOCKED


def derive(
    issues: Sequence[IssueRecord],
    *,
    satisfied: Iterable[int] = (),
    board_status: Mapping[int, str | None] | None = None,
) -> tuple[DerivedRow, ...]:
    """Compute Wave, Fan In, Fan Out and Status for every issue."""
    graph = normalise_graph({record.number: record.blocked_by for record in issues})
    depth = waves(graph)
    incoming = fan_in(graph)
    outgoing = fan_out(graph)
    satisfied_set = frozenset(satisfied)
    current = dict(board_status or {})

    rows: list[DerivedRow] = []
    for record in sorted(issues, key=lambda issue: issue.number):
        unsatisfied = tuple(
            blocker for blocker in record.blocked_by if blocker not in satisfied_set
        )
        rows.append(
            DerivedRow(
                number=record.number,
                title=record.title,
                state=record.state,
                wave=depth[record.number],
                fan_in=incoming[record.number],
                fan_out=outgoing.get(record.number, 0),
                status=derived_status(record, satisfied_set, current.get(record.number)),
                blocked_by=record.blocked_by,
                unsatisfied_blockers=unsatisfied,
            )
        )
    return tuple(rows)


SELECTION_KEYS: tuple[str, ...] = ("wave", "-fan_out", "number")
"""The tie-break keys of the total selection order, in order, for the JSON report."""


def selection_key(row: DerivedRow) -> tuple[int, int, int]:
    """The sort key of the total selection order stated in `execution-order.md` section 5.

    Lowest `Wave`, then highest `Fan Out`, then lowest issue number (G-530). The order is
    total because the third key is unique, so two agents reading the same graph select the
    same Atom.
    """
    return (row.wave, -row.fan_out, row.number)


def selection_order(rows: Iterable[DerivedRow]) -> tuple[DerivedRow, ...]:
    """Return `rows` in the selection order, without filtering."""
    return tuple(sorted(rows, key=selection_key))


def ready_set(rows: Iterable[DerivedRow]) -> tuple[DerivedRow, ...]:
    """Return the rows whose derived Status is Ready, in the selection order.

    An Atom a human has already moved to In Progress, In Review or Accepted is not in the
    ready set: it is claimed. That makes this set equal, by construction, to Project 9's
    `Status = Ready` set once `--apply` has run.

    The rows come back sorted by `selection_key`, so the runbook's "take the first
    unclaimed row" is a well-defined instruction against this output rather than against
    ascending issue numbers.
    """
    return selection_order(row for row in rows if row.ready)


# --------------------------------------------------------------------------------------
# Bootstrap satisfaction
# --------------------------------------------------------------------------------------


def closure_record_path(closure_root: Path, number: int) -> Path:
    return closure_root / str(number) / "closure.json"


def closure_record_valid(
    path: Path,
    schema: Mapping[str, Any] | None,
    *,
    resolve_source: Any | None = None,
) -> bool:
    """Inspect a local closure with the same validator used by CI.

    This helper is intentionally not used for readiness: ``bootstrap_satisfied`` reads
    the canonical record and its artifacts from one accepted revision instead.
    """
    try:
        payload = load_json(path)
    except (OSError, ValueError):
        return False
    atom_directory = path.parent
    if atom_directory.parent.name == "atoms" and atom_directory.parent.parent.name == "artifacts":
        repository_root = atom_directory.parent.parent.parent.resolve()
        try:
            record_path = path.resolve().relative_to(repository_root).as_posix()
        except ValueError:
            return False
        expected_atom = atom_directory.name
    else:
        repository_root = path.parent.resolve()
        record_path = path.name
        expected_atom = path.parent.name if path.parent.name.isdigit() else None
    return not closure_problems(
        payload,
        schema,
        label=str(path),
        expected_atom=expected_atom,
        record_path=record_path,
        read_artifact=local_bytes_reader(repository_root),
        resolve_source=resolve_source,
    )


def load_closure_schema(path: Path) -> Mapping[str, Any] | None:
    try:
        payload = load_json(path)
    except (OSError, ValueError):
        return None
    return payload if isinstance(payload, dict) else None


@dataclass(frozen=True, slots=True)
class AcceptedRevision:
    """The immutable source revision used for readiness evidence."""

    commit_id: str
    change_id: str
    repository_root: Path
    reader: BytesReader | None = None
    source_resolver: Any | None = None

    def read(self, relative_path: str) -> bytes | None:
        """Read one repository-relative file from this exact accepted commit."""
        candidate = Path(relative_path)
        if candidate.is_absolute() or ".." in candidate.parts:
            return None
        if self.reader is not None:
            return self.reader(candidate.as_posix())
        try:
            completed = subprocess.run(
                ["jj", "file", "show", "-r", self.commit_id, "--", candidate.as_posix()],
                cwd=self.repository_root,
                check=False,
                capture_output=True,
                timeout=30,
            )
        except (OSError, ValueError, subprocess.SubprocessError):
            return None
        if completed.returncode != 0:
            return None
        return completed.stdout


def _jj_repository_root(invocation_path: Path) -> Path:
    """Resolve the repository root once from the command's invocation directory."""
    invocation = invocation_path.resolve()
    try:
        completed = subprocess.run(
            ["jj", "root", "--ignore-working-copy"],
            cwd=invocation,
            check=False,
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise RuntimeError(f"cannot resolve Jujutsu repository root: {error}") from error
    output = completed.stdout
    if isinstance(output, bytes):
        try:
            output = output.decode()
        except UnicodeDecodeError as error:
            raise RuntimeError("cannot resolve Jujutsu repository root: invalid output") from error
    root_text = output.strip() if isinstance(output, str) else ""
    if completed.returncode != 0 or not root_text:
        detail = completed.stderr.strip() or "no repository root returned"
        raise RuntimeError(f"cannot resolve Jujutsu repository root: {detail}")
    root = Path(root_text)
    if not root.is_absolute():
        root = invocation / root
    return root.resolve()


def _jj_revision(repository_root: Path, template: str) -> str:
    try:
        completed = subprocess.run(
            [
                "jj",
                "log",
                "-r",
                "trunk()",
                "-n",
                "1",
                "--no-graph",
                "--ignore-working-copy",
                "-T",
                template,
            ],
            cwd=repository_root,
            check=False,
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise RuntimeError(f"cannot resolve accepted trunk() revision: {error}") from error
    value = completed.stdout.strip()
    if completed.returncode != 0 or not value:
        detail = completed.stderr.strip() or "no revision returned"
        raise RuntimeError(f"cannot resolve accepted trunk() {template}: {detail}")
    return value


def accepted_revision(invocation_path: Path | None = None) -> AcceptedRevision:
    """Resolve the repository root and ``trunk()`` once, ignoring the working copy."""
    invocation = (invocation_path or Path.cwd()).resolve()
    root = _jj_repository_root(invocation)
    identities = _jj_revision(root, 'commit_id ++ "\\n" ++ change_id').splitlines()
    if len(identities) != 2 or not all(identities):
        raise RuntimeError("cannot resolve accepted trunk() commit and change identities")
    return AcceptedRevision(
        commit_id=identities[0],
        change_id=identities[1],
        repository_root=root,
    )


def _relative_repository_path(path: Path, repository_root: Path) -> str | None:
    """Return a safe repository-relative path, or ``None`` outside the repository."""
    root = repository_root.resolve()
    candidate = (path if path.is_absolute() else root / path).resolve()
    try:
        relative = candidate.relative_to(root)
    except ValueError:
        return None
    return relative.as_posix()


def _accepted_schema(
    path: Path, revision: AcceptedRevision, schema: Mapping[str, Any] | None
) -> Mapping[str, Any] | None:
    """Load the canonical schema from the accepted revision.

    ``schema_path`` is an input convenience for callers, not an alternate source of
    accepted state.  A path outside the repository's canonical schema location is
    rejected, and the production reader never falls back to a mutable local schema.
    """
    relative = _relative_repository_path(path, revision.repository_root)
    canonical = DEFAULT_CLOSURE_SCHEMA.as_posix()
    if relative != canonical:
        return None
    raw = revision.read(canonical)
    if raw is None:
        # A reader is injectable for deterministic tests and adapter-owned sources.  The
        # production revision has no reader and therefore fails closed when its canonical
        # schema is absent; a test double may provide an explicit schema fallback.
        return schema if revision.reader is not None else None
    try:
        payload = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        return None
    return payload if isinstance(payload, dict) else None


def bootstrap_satisfied(
    issues: Sequence[IssueRecord],
    *,
    closure_root: Path | None,
    schema: Mapping[str, Any] | None = None,
    accepted: AcceptedRevision | None = None,
    schema_path: Path | None = None,
) -> tuple[frozenset[int], tuple[int, ...]]:
    """Return `(satisfied issue numbers, closed issues lacking a valid closure record)`.

    With ``closure_root=None`` the caller has opted into GitHub's own weaker rule, in which
    closure alone unblocks a dependent. That mode is for inspecting the raw graph, never
    for picking the next Atom. Otherwise this function resolves ``trunk()`` and reads the
    closure and verifier artifacts from that exact accepted commit. It never trusts a
    closure copied into the mutable workspace.
    """
    closed = [record.number for record in issues if not record.is_open]
    if closure_root is None:
        return frozenset(closed), ()
    revision = accepted or accepted_revision()
    schema = _accepted_schema(schema_path or DEFAULT_CLOSURE_SCHEMA, revision, schema)
    if schema is None:
        return frozenset(), tuple(sorted(closed))

    satisfied: set[int] = set()
    unevidenced: list[int] = []
    for number in closed:
        path = closure_record_path(closure_root, number)
        relative = _relative_repository_path(path, revision.repository_root)
        expected_path = closure_record_path(DEFAULT_CLOSURE_ROOT, number).as_posix()
        if relative != expected_path:
            # A caller-controlled closure root must not turn an arbitrary record location
            # into satisfaction evidence.  The canonical path is part of the Atom contract.
            unevidenced.append(number)
            continue
        raw = revision.read(relative) if relative is not None else None
        if raw is None:
            unevidenced.append(number)
            continue
        try:
            payload = json.loads(raw.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError):
            unevidenced.append(number)
            continue
        resolver = revision.source_resolver or jj_source_resolver(
            revision.repository_root, revision.commit_id
        )
        problems = closure_problems(
            payload,
            schema,
            label=relative or str(path),
            expected_atom=str(number),
            record_path=relative,
            read_artifact=revision.read,
            resolve_source=resolver,
            source_binding_required=True,
        )
        if not problems:
            satisfied.add(number)
        else:
            unevidenced.append(number)
    return frozenset(satisfied), tuple(sorted(unevidenced))


# --------------------------------------------------------------------------------------
# Reading the graph
# --------------------------------------------------------------------------------------

_ISSUES_QUERY = """
query($owner:String!,$name:String!,$cursor:String){
  repository(owner:$owner,name:$name){
    issues(first:50, after:$cursor, states:[OPEN,CLOSED],
           orderBy:{field:CREATED_AT,direction:ASC}){
      totalCount
      pageInfo{hasNextPage endCursor}
      nodes{
        number
        title
        state
        body
        url
        milestone{title}
        labels(first:100){
          totalCount
          pageInfo{hasNextPage endCursor}
          nodes{name}
        }
        assignees(first:100){
          totalCount
          pageInfo{hasNextPage endCursor}
          nodes{login}
        }
        blockedBy(first:100){
          totalCount
          pageInfo{hasNextPage endCursor}
          nodes{number}
        }
      }
    }
  }
}
"""

_BLOCKED_BY_PAGE_QUERY = """
query($owner:String!,$name:String!,$number:Int!,$cursor:String){
  repository(owner:$owner,name:$name){
    issue(number:$number){
      blockedBy(first:100, after:$cursor){
        totalCount
        pageInfo{hasNextPage endCursor}
        nodes{number}
      }
    }
  }
}
"""

_LABELS_PAGE_QUERY = """
query($owner:String!,$name:String!,$number:Int!,$cursor:String){
  repository(owner:$owner,name:$name){
    issue(number:$number){
      labels(first:100, after:$cursor){
        totalCount
        pageInfo{hasNextPage endCursor}
        nodes{name}
      }
    }
  }
}
"""

_ASSIGNEES_PAGE_QUERY = """
query($owner:String!,$name:String!,$number:Int!,$cursor:String){
  repository(owner:$owner,name:$name){
    issue(number:$number){
      assignees(first:100, after:$cursor){
        totalCount
        pageInfo{hasNextPage endCursor}
        nodes{login}
      }
    }
  }
}
"""


def _connection_total(connection: Mapping[str, Any], context: str) -> int:
    total = connection.get("totalCount")
    if isinstance(total, bool) or not isinstance(total, int) or total < 0:
        raise RuntimeError(f"{context}: missing or invalid totalCount")
    return total


def _next_cursor(
    page: Mapping[str, Any], current: str, seen: set[str], context: str
) -> str:
    has_next = page.get("hasNextPage")
    if type(has_next) is not bool:
        raise RuntimeError(f"{context}: pagination pageInfo hasNextPage is invalid")
    if not has_next:
        return ""
    candidate = page.get("endCursor")
    if not isinstance(candidate, str) or not candidate or candidate == current or candidate in seen:
        raise RuntimeError(f"{context}: pagination cursor did not advance")
    seen.add(candidate)
    return candidate


def _connection_nodes(connection: Mapping[str, Any], context: str) -> list[Mapping[str, Any]]:
    nodes = connection.get("nodes")
    if not isinstance(nodes, list) or any(not isinstance(node, Mapping) for node in nodes):
        raise RuntimeError(f"{context}: connection nodes are missing or invalid")
    return nodes


def _page_info(connection: Mapping[str, Any], context: str) -> Mapping[str, Any]:
    page = connection.get("pageInfo")
    if not isinstance(page, Mapping):
        raise RuntimeError(f"{context}: connection pageInfo is missing or invalid")
    return page


def _blocked_by_pages(
    owner: str, name: str, number: int, cursor: str, expected_total: int
) -> list[int]:
    """Continue native `blockedBy` pagination for one issue."""
    numbers: list[int] = []
    seen_cursors = {cursor}
    while cursor:
        data = graphql(
            _BLOCKED_BY_PAGE_QUERY,
            {"owner": owner, "name": name, "number": number, "cursor": cursor},
        )
        connection = data["repository"]["issue"]["blockedBy"]
        if _connection_total(connection, f"#{number} blockedBy") != expected_total:
            raise RuntimeError(f"#{number}: blockedBy totalCount changed during pagination")
        numbers.extend(
            issue_number(node)
            for node in _connection_nodes(connection, f"#{number} blockedBy")
        )
        page = _page_info(connection, f"#{number} blockedBy")
        cursor = _next_cursor(page, cursor, seen_cursors, f"#{number} blockedBy")
    return numbers


def _label_pages(
    owner: str, name: str, number: int, cursor: str, expected_total: int
) -> list[str]:
    names: list[str] = []
    seen_cursors = {cursor}
    while cursor:
        data = graphql(
            _LABELS_PAGE_QUERY,
            {"owner": owner, "name": name, "number": number, "cursor": cursor},
        )
        connection = data["repository"]["issue"]["labels"]
        if _connection_total(connection, f"#{number} labels") != expected_total:
            raise RuntimeError(f"#{number}: labels totalCount changed during pagination")
        names.extend(
            str(node["name"])
            for node in _connection_nodes(connection, f"#{number} labels")
        )
        cursor = _next_cursor(
            _page_info(connection, f"#{number} labels"), cursor, seen_cursors, f"#{number} labels"
        )
    return names


def _assignee_pages(
    owner: str, name: str, number: int, cursor: str, expected_total: int
) -> list[str]:
    logins: list[str] = []
    seen_cursors = {cursor}
    while cursor:
        data = graphql(
            _ASSIGNEES_PAGE_QUERY,
            {"owner": owner, "name": name, "number": number, "cursor": cursor},
        )
        connection = data["repository"]["issue"]["assignees"]
        if _connection_total(connection, f"#{number} assignees") != expected_total:
            raise RuntimeError(f"#{number}: assignee totalCount changed during pagination")
        logins.extend(
            str(node["login"])
            for node in _connection_nodes(connection, f"#{number} assignees")
        )
        cursor = _next_cursor(
            _page_info(connection, f"#{number} assignees"),
            cursor,
            seen_cursors,
            f"#{number} assignees",
        )
    return logins


def validate_registry_graph(issues: Sequence[IssueRecord]) -> None:
    """Reject an incomplete or cyclic executable native registry."""
    numbers = [issue.number for issue in issues]
    if any(isinstance(number, bool) or number < 1 for number in numbers):
        raise RuntimeError("registry issue numbers must be positive integers")
    if len(numbers) != len(set(numbers)):
        raise RuntimeError("registry repeats an issue number")
    known = set(numbers)
    raw = {issue.number: issue.blocked_by for issue in issues}
    unknown = sorted(
        (issue.number, blocker)
        for issue in issues
        for blocker in issue.blocked_by
        if blocker not in known
    )
    if unknown:
        source, blocker = unknown[0]
        raise RuntimeError(
            f"#{source}: native blocker #{blocker} is absent from the executable Atom registry"
        )
    for issue in issues:
        if issue.number in issue.blocked_by:
            raise RuntimeError(f"#{issue.number}: native blockedBy contains a self-edge")
    try:
        topological_order(raw)
    except DependencyCycleError as error:
        raise RuntimeError(str(error)) from error


def fetch_issues(owner: str, name: str) -> tuple[IssueRecord, ...]:
    """Read every issue and its complete native `blockedBy` connection through `gh`."""
    records: list[IssueRecord] = []
    cursor = ""
    seen_cursors = {cursor}
    expected_total: int | None = None
    retrieved_nodes = 0
    seen_numbers: set[int] = set()
    while True:
        variables: dict[str, str | int] = {"owner": owner, "name": name}
        if cursor:
            variables["cursor"] = cursor
        data = graphql(_ISSUES_QUERY, variables)
        connection = data["repository"]["issues"]
        page_total = _connection_total(connection, "repository issues")
        if expected_total is None:
            expected_total = page_total
        elif page_total != expected_total:
            raise RuntimeError("repository issues totalCount changed during pagination")
        nodes = _connection_nodes(connection, "repository issues")
        retrieved_nodes += len(nodes)
        for node in nodes:
            number = issue_number(node)
            if number in seen_numbers:
                raise RuntimeError(f"repository issues pagination repeated issue #{number}")
            seen_numbers.add(number)
            labels = node["labels"]
            declared_labels = _connection_total(labels, f"#{number} labels")
            label_nodes = _connection_nodes(labels, f"#{number} labels")
            label_names_list = [str(label["name"]) for label in label_nodes]
            label_names = set(label_names_list)
            label_page = _page_info(labels, f"#{number} labels")
            first_cursor = _next_cursor(label_page, "", {""}, f"#{number} labels")
            if first_cursor:
                label_names_list.extend(
                    _label_pages(owner, name, number, first_cursor, declared_labels)
                )
                label_names = set(label_names_list)
            if (
                declared_labels != len(label_names_list)
                or len(label_names) != len(label_names_list)
            ):
                raise RuntimeError(
                    f"#{number}: retrieved {len(label_names_list)} labels but GitHub reports "
                    f"{declared_labels}; label pagination is incomplete"
                )
            if "duplicate" in label_names:
                if str(node["state"]).upper() != "CLOSED":
                    raise RuntimeError(f"#{number}: duplicate-labeled issue is still open")
                continue
            blocked = node["blockedBy"]
            numbers = [
                issue_number(edge)
                for edge in _connection_nodes(blocked, f"#{number} blockedBy")
            ]
            declared = _connection_total(blocked, f"#{number} blockedBy")
            page = _page_info(blocked, f"#{number} blockedBy")
            first_cursor = _next_cursor(page, "", {""}, f"#{number} blockedBy")
            if first_cursor:
                numbers.extend(
                    _blocked_by_pages(owner, name, number, first_cursor, declared)
                )
            if declared != len(numbers) or len(set(numbers)) != len(numbers):
                raise RuntimeError(
                    f"#{number}: retrieved {len(numbers)} blocked-by edges but GitHub "
                    f"reports {declared}; pagination is incomplete"
                )
            assignees = node["assignees"]
            declared_assignees = _connection_total(assignees, f"#{number} assignees")
            assignee_logins = [
                str(user["login"])
                for user in _connection_nodes(assignees, f"#{number} assignees")
            ]
            assignee_page = _page_info(assignees, f"#{number} assignees")
            first_cursor = _next_cursor(assignee_page, "", {""}, f"#{number} assignees")
            if first_cursor:
                assignee_logins.extend(
                    _assignee_pages(owner, name, number, first_cursor, declared_assignees)
                )
            if declared_assignees != len(assignee_logins) or len(set(assignee_logins)) != len(
                assignee_logins
            ):
                raise RuntimeError(
                    f"#{number}: retrieved {len(assignee_logins)} assignees but GitHub reports "
                    f"{declared_assignees}; pagination is incomplete"
                )
            records.append(
                IssueRecord(
                    number=number,
                    title=str(node["title"]),
                    state=str(node["state"]),
                    blocked_by=tuple(sorted(set(numbers))),
                    body=str(node.get("body") or ""),
                    labels=tuple(sorted(label_names)),
                    milestone=(
                        str(node["milestone"]["title"])
                        if isinstance(node.get("milestone"), Mapping)
                        else None
                    ),
                    url=str(node.get("url") or ""),
                    assignees=tuple(sorted(assignee_logins)),
                )
            )
        page = _page_info(connection, "repository issues")
        next_cursor = _next_cursor(page, cursor, seen_cursors, "repository issues")
        if not next_cursor:
            break
        cursor = next_cursor
    if expected_total != retrieved_nodes:
        raise RuntimeError(
            f"repository issues pagination retrieved {retrieved_nodes} nodes but GitHub "
            f"reports {expected_total}"
        )
    result = tuple(sorted(records, key=lambda record: record.number))
    validate_registry_graph(result)
    return result


SNAPSHOT_RECORD_FORMAT = "gordian-atom-registry-v1"
LEGACY_SNAPSHOT_FORMAT = "gordian.atoms.v1"
SNAPSHOT_REQUIRED_FIELDS = frozenset(
    {
        "record_format",
        "generated_at",
        "source_change_id",
        "source_commit_id",
        "tool_versions",
        "source",
        "repository",
        "coherent",
        "issues",
    }
)
SNAPSHOT_ISSUE_FIELDS = frozenset(
    {
        "number",
        "title",
        "state",
        "body",
        "url",
        "milestone",
        "labels",
        "blockedBy",
        "assignees",
    }
)
LEGACY_SNAPSHOT_FIELDS = frozenset(
    {
        "snapshot_format",
        "generated_by",
        "generated_at",
        "source",
        "source_commit",
        "atom_count",
        "edge_count",
        "issues",
    }
)
LEGACY_SNAPSHOT_ISSUE_FIELDS = frozenset(
    {"number", "title", "state", "milestone", "labels", "blocked_by", "body"}
)


def _snapshot_text(value: Any, field: str, path: Path) -> str:
    if not isinstance(value, str) or not value:
        raise RuntimeError(f"{path}: snapshot {field} must be a non-empty string")
    return value


def _snapshot_string_list(value: Any, field: str, path: Path) -> tuple[str, ...]:
    if not isinstance(value, list) or any(not isinstance(item, str) for item in value):
        raise RuntimeError(f"{path}: issue {field} must be a list of strings")
    if len(set(value)) != len(value):
        raise RuntimeError(f"{path}: issue {field} contains duplicates")
    return tuple(sorted(value))


def _snapshot_blockers(value: Any, number: int, path: Path) -> tuple[int, ...]:
    if not isinstance(value, list):
        raise RuntimeError(f"{path}: issue blockedBy must be a list of integer issue numbers")
    if any(isinstance(item, bool) or not isinstance(item, int) or item < 1 for item in value):
        raise RuntimeError(f"{path}: issue blockedBy must be a list of positive integers")
    if number in value:
        raise RuntimeError(f"{path}: issue #{number} cannot block itself")
    if len(set(value)) != len(value):
        raise RuntimeError(f"{path}: issue blockedBy contains duplicates")
    return tuple(sorted(value))


def _migrate_legacy_snapshot(
    payload: Any, path: Path, expected_repository: str
) -> dict[str, Any] | None:
    """Convert the accepted ``gordian.atoms.v1`` capture to the sole read schema.

    G-502 accepted the original shell producer before the richer registry reader
    existed.  The old document is an input compatibility format only: it is
    validated completely, converted deterministically, and never exposed as a
    second graph authority.  A subsequent producer run writes the canonical
    ``gordian-atom-registry-v1`` envelope.
    """
    if not isinstance(payload, dict) or payload.get("snapshot_format") != LEGACY_SNAPSHOT_FORMAT:
        return None
    unknown = set(payload) - LEGACY_SNAPSHOT_FIELDS
    missing = LEGACY_SNAPSHOT_FIELDS - set(payload)
    if missing:
        raise RuntimeError(f"{path}: legacy snapshot is missing required fields {sorted(missing)}")
    if unknown:
        raise RuntimeError(f"{path}: legacy snapshot has unexpected fields {sorted(unknown)}")
    generated_at = payload["generated_at"]
    source = payload["source"]
    source_commit = payload["source_commit"]
    generated_by = payload["generated_by"]
    if parse_rfc3339(generated_at) is None:
        raise RuntimeError(f"{path}: legacy snapshot generated_at must be an RFC 3339 timestamp")
    for value, field in (
        (source, "source"),
        (source_commit, "source_commit"),
        (generated_by, "generated_by"),
    ):
        if not isinstance(value, str) or not value:
            raise RuntimeError(f"{path}: legacy snapshot {field} must be a non-empty string")
    rows = payload["issues"]
    if not isinstance(rows, list):
        raise RuntimeError(f"{path}: legacy snapshot issues must be a list")
    canonical_rows: list[dict[str, Any]] = []
    seen: set[int] = set()
    edge_count = 0
    for index, row in enumerate(rows):
        label = f"{path}: legacy issues[{index}]"
        if not isinstance(row, dict):
            raise RuntimeError(f"{label}: issue record must be an object")
        row_unknown = set(row) - LEGACY_SNAPSHOT_ISSUE_FIELDS
        row_missing = LEGACY_SNAPSHOT_ISSUE_FIELDS - set(row)
        if row_missing:
            raise RuntimeError(f"{label}: missing required fields {sorted(row_missing)}")
        if row_unknown:
            raise RuntimeError(f"{label}: unexpected fields {sorted(row_unknown)}")
        number = row["number"]
        if isinstance(number, bool) or not isinstance(number, int) or number < 1:
            raise RuntimeError(f"{label}: number must be a positive integer")
        if number in seen:
            raise RuntimeError(f"{path}: legacy snapshot repeats issue number {number}")
        seen.add(number)
        blockers = row["blocked_by"]
        if (
            not isinstance(blockers, list)
            or any(
                isinstance(item, bool) or not isinstance(item, int) or item < 1
                for item in blockers
            )
            or number in blockers
            or len(set(blockers)) != len(blockers)
        ):
            raise RuntimeError(f"{label}: blocked_by must be unique positive issue numbers")
        if not isinstance(row["title"], str) or not row["title"]:
            raise RuntimeError(f"{label}: title must be a non-empty string")
        if row["state"] not in ("OPEN", "CLOSED"):
            raise RuntimeError(f"{label}: state must be OPEN or CLOSED")
        if not isinstance(row["body"], str):
            raise RuntimeError(f"{label}: body must be a string")
        if row["milestone"] is not None and not isinstance(row["milestone"], str):
            raise RuntimeError(f"{label}: milestone must be a string or null")
        if not isinstance(row["labels"], list) or any(
            not isinstance(item, str) for item in row["labels"]
        ):
            raise RuntimeError(f"{label}: labels must be a list of strings")
        if len(set(row["labels"])) != len(row["labels"]):
            raise RuntimeError(f"{label}: labels contain duplicates")
        edge_count += len(blockers)
        canonical_rows.append(
            {
                "number": number,
                "title": row["title"],
                "state": row["state"],
                "body": row["body"],
                "url": f"https://github.com/{expected_repository}/issues/{number}",
                "milestone": row["milestone"],
                "labels": sorted(row["labels"]),
                "blockedBy": sorted(blockers),
                "assignees": [],
            }
        )
    if payload["atom_count"] != len(rows) or payload["edge_count"] != edge_count:
        raise RuntimeError(f"{path}: legacy snapshot counts do not match its issue rows")
    return {
        "record_format": SNAPSHOT_RECORD_FORMAT,
        "generated_at": generated_at,
        "source_change_id": f"legacy-snapshot-{source_commit[:12]}",
        "source_commit_id": source_commit,
        "tool_versions": {"snapshot-producer": generated_by},
        "source": source,
        "repository": expected_repository,
        "coherent": True,
        "issues": canonical_rows,
    }


def _validate_snapshot_envelope(
    payload: Any, path: Path, expected_repository: str
) -> list[dict[str, Any]]:
    migrated = _migrate_legacy_snapshot(payload, path, expected_repository)
    if migrated is not None:
        payload = migrated
    if not isinstance(payload, dict):
        raise RuntimeError(f"{path}: expected a strict registry snapshot envelope object")
    unknown = set(payload) - SNAPSHOT_REQUIRED_FIELDS
    missing = SNAPSHOT_REQUIRED_FIELDS - set(payload)
    if missing:
        raise RuntimeError(f"{path}: snapshot is missing required fields {sorted(missing)}")
    if unknown:
        raise RuntimeError(f"{path}: snapshot has unexpected fields {sorted(unknown)}")
    if payload["record_format"] != SNAPSHOT_RECORD_FORMAT:
        raise RuntimeError(
            f"{path}: snapshot record_format must be {SNAPSHOT_RECORD_FORMAT!r}"
        )
    if payload["coherent"] is not True:
        raise RuntimeError(f"{path}: snapshot coherent must be true")
    if payload["repository"] != expected_repository:
        raise RuntimeError(
            f"{path}: snapshot repository {payload['repository']!r} does not match "
            f"expected {expected_repository!r}"
        )
    generated_at = payload["generated_at"]
    if parse_rfc3339(generated_at) is None:
        raise RuntimeError(f"{path}: snapshot generated_at must be an RFC 3339 timestamp")
    for field in ("source_change_id", "source_commit_id", "source"):
        _snapshot_text(payload[field], field, path)
    if payload["source_change_id"] == "unknown" or payload["source_commit_id"] == "unknown":
        raise RuntimeError(f"{path}: snapshot source change and commit identities are required")
    tool_versions = payload["tool_versions"]
    if not isinstance(tool_versions, dict) or any(
        not isinstance(key, str) or not key or not isinstance(value, str) or not value
        for key, value in tool_versions.items()
    ):
        raise RuntimeError(f"{path}: snapshot tool_versions must be a string-to-string object")
    rows = payload["issues"]
    if not isinstance(rows, list):
        raise RuntimeError(f"{path}: snapshot issues must be a list")
    return rows


def load_snapshot(
    path: Path,
    *,
    expected_repository: str = DEFAULT_REPOSITORY,
    require_strict: bool = False,
) -> tuple[IssueRecord, ...]:
    """Read a complete, coherent registry snapshot.

    ``gordian.atoms.v1`` is accepted as a read-only compatibility input and is
    deterministically mapped to the current in-memory row shape.  Callers that can
    authorize a mutation pass ``require_strict=True`` so the legacy input is rejected
    before any effect.  No read path rewrites the input or emits a replacement file.

    A bare list, a partial/arbitrary object, a non-coherent capture, or a snapshot
    without explicit source identity is rejected.  The issue rows are intentionally
    strict because assignee state is part of the claim lock and cannot be reconstructed
    from a stale offline graph.
    """
    try:
        payload = load_json(path)
    except (OSError, ValueError) as error:
        raise RuntimeError(f"{path}: unreadable or malformed JSON: {error}") from error
    if (
        require_strict
        and isinstance(payload, dict)
        and payload.get("snapshot_format") == LEGACY_SNAPSHOT_FORMAT
    ):
        raise RuntimeError(
            f"{path}: legacy snapshot is read-only; mutation requires the strict "
            f"{SNAPSHOT_RECORD_FORMAT!r} envelope"
        )
    rows = _validate_snapshot_envelope(payload, path, expected_repository)
    records: list[IssueRecord] = []
    numbers: set[int] = set()
    duplicate_numbers: set[int] = set()
    for index, row in enumerate(rows):
        label = f"{path}: issues[{index}]"
        if not isinstance(row, dict):
            raise RuntimeError(f"{label}: issue record must be an object")
        unknown = set(row) - SNAPSHOT_ISSUE_FIELDS
        missing = SNAPSHOT_ISSUE_FIELDS - set(row)
        if missing:
            raise RuntimeError(f"{label}: missing required fields {sorted(missing)}")
        if unknown:
            raise RuntimeError(f"{label}: unexpected fields {sorted(unknown)}")
        number = row["number"]
        if isinstance(number, bool) or not isinstance(number, int) or number < 1:
            raise RuntimeError(f"{label}: number must be a positive integer")
        if number in numbers:
            duplicate_numbers.add(number)
        numbers.add(number)
        title = _snapshot_text(row["title"], "title", path)
        state = row["state"]
        if state not in ("OPEN", "CLOSED"):
            raise RuntimeError(f"{label}: state must be OPEN or CLOSED")
        body = row["body"]
        if not isinstance(body, str):
            raise RuntimeError(f"{label}: issue body must be a string")
        url = row["url"]
        if not isinstance(url, str):
            raise RuntimeError(f"{label}: issue url must be a string")
        milestone = row["milestone"]
        if milestone is not None and not isinstance(milestone, str):
            raise RuntimeError(f"{label}: issue milestone must be a string or null")
        labels = _snapshot_string_list(row["labels"], "labels", path)
        blocked_by = _snapshot_blockers(row["blockedBy"], number, path)
        assignees = _snapshot_string_list(row["assignees"], "assignees", path)
        if "duplicate" in labels and state != "CLOSED":
            raise RuntimeError(f"{label}: duplicate-labeled issue must be CLOSED")
        if "duplicate" in labels:
            continue
        records.append(
            IssueRecord(
                number=number,
                title=title,
                state=state,
                blocked_by=blocked_by,
                body=body,
                labels=labels,
                milestone=milestone,
                url=url,
                assignees=assignees,
            )
        )
    if duplicate_numbers:
        raise RuntimeError(
            f"{path}: snapshot repeats issue numbers {sorted(duplicate_numbers)}"
        )
    record_numbers = {record.number for record in records}
    unknown_blockers = sorted(
        {
            blocker
            for record in records
            for blocker in record.blocked_by
            if blocker not in record_numbers
        }
    )
    if unknown_blockers:
        raise RuntimeError(
            f"{path}: blockedBy references issue numbers absent from the executable registry "
            f"{unknown_blockers}"
        )
    result = tuple(sorted(records, key=lambda record: record.number))
    validate_registry_graph(result)
    return result


# --------------------------------------------------------------------------------------
# Reading and writing the board
# --------------------------------------------------------------------------------------

_PROJECT_QUERY_TEMPLATE = """
query($owner:String!,$number:Int!,$cursor:String){
  %s(login:$owner){
    projectV2(number:$number){
      id
      fields(first:50){
        nodes{
          ... on ProjectV2FieldCommon{id name dataType}
          ... on ProjectV2SingleSelectField{id name dataType options{id name}}
        }
      }
      items(first:100, after:$cursor){
        totalCount
        pageInfo{hasNextPage endCursor}
        nodes{
          id
          content{... on Issue{number}}
          fieldValues(first:50){
            nodes{
              ... on ProjectV2ItemFieldNumberValue{
                number field{... on ProjectV2FieldCommon{name}}
              }
              ... on ProjectV2ItemFieldSingleSelectValue{
                name field{... on ProjectV2FieldCommon{name}}
              }
            }
          }
        }
      }
    }
  }
}
"""

_SET_NUMBER_MUTATION = """
mutation($project:ID!,$item:ID!,$field:ID!,$value:Float!){
  updateProjectV2ItemFieldValue(input:{
    projectId:$project,itemId:$item,fieldId:$field,value:{number:$value}
  }){projectV2Item{id}}
}
"""

_SET_SELECT_MUTATION = """
mutation($project:ID!,$item:ID!,$field:ID!,$option:String!){
  updateProjectV2ItemFieldValue(input:{
    projectId:$project,itemId:$item,fieldId:$field,value:{singleSelectOptionId:$option}
  }){projectV2Item{id}}
}
"""


def _parse_field_values(nodes: Iterable[Mapping[str, Any]]) -> dict[str, str | int]:
    values: dict[str, str | int] = {}
    for node in nodes:
        field = node.get("field") or {}
        name = field.get("name")
        if not isinstance(name, str):
            continue
        if "name" in node and isinstance(node["name"], str):
            values[name] = node["name"]
        elif "number" in node and isinstance(node["number"], int | float):
            values[name] = int(node["number"])
    return values


def fetch_board(owner: str, project_number: int, *, owner_type: str = "user") -> Board:
    """Read Project 9's field metadata and every item's current derived values."""
    query = _PROJECT_QUERY_TEMPLATE % owner_type
    fields: dict[str, ProjectField] = {}
    items: dict[int, BoardItem] = {}
    project_id = ""
    cursor = ""
    seen_cursors = {cursor}
    expected_total: int | None = None
    retrieved_nodes = 0
    while True:
        variables: dict[str, str | int] = {"owner": owner, "number": project_number}
        if cursor:
            variables["cursor"] = cursor
        data = graphql(query, variables)
        project = (data.get(owner_type) or {}).get("projectV2")
        if not project:
            raise RuntimeError(f"{owner_type} {owner!r} has no project number {project_number}")
        page_project_id = str(project["id"])
        if project_id and page_project_id != project_id:
            raise RuntimeError(
                "Project identity changed while reading pages: "
                f"{project_id!r} -> {page_project_id!r}"
            )
        project_id = page_project_id
        for node in project["fields"]["nodes"]:
            if not node:
                continue
            field_name = str(node["name"])
            options = tuple(
                (str(option["name"]), str(option["id"])) for option in node.get("options", ())
            )
            field = ProjectField(
                field_id=str(node["id"]),
                name=field_name,
                data_type=str(node.get("dataType", "")),
                options=options,
            )
            existing_field = fields.get(field_name)
            if existing_field is not None and existing_field != field:
                raise RuntimeError(
                    f"Project contains conflicting field definition {field_name!r}"
                )
            fields[field_name] = field
        item_connection = project["items"]
        page_total = _connection_total(item_connection, "Project items")
        if expected_total is None:
            expected_total = page_total
        elif page_total != expected_total:
            raise RuntimeError("Project item totalCount changed during pagination")
        retrieved_nodes += len(item_connection["nodes"])
        for node in item_connection["nodes"]:
            content = node.get("content") or {}
            if "number" not in content:
                continue
            number = issue_number(content)
            if number in items:
                raise RuntimeError(
                    f"Project contains duplicate item for issue #{number}"
                )
            values = _parse_field_values(node["fieldValues"]["nodes"])
            items[number] = BoardItem(
                item_id=str(node["id"]),
                number=number,
                status=values.get(STATUS_FIELD) if isinstance(
                    values.get(STATUS_FIELD), str
                ) else None,
                wave=_as_int(values.get(WAVE_FIELD)),
                fan_in=_as_int(values.get(FAN_IN_FIELD)),
                fan_out=_as_int(values.get(FAN_OUT_FIELD)),
            )
        page = item_connection["pageInfo"]
        next_cursor = _next_cursor(page, cursor, seen_cursors, "Project items")
        if not next_cursor:
            break
        cursor = next_cursor
    if expected_total != retrieved_nodes:
        raise RuntimeError(
            f"Project item pagination retrieved {retrieved_nodes} nodes but GitHub reports "
            f"{expected_total}"
        )
    return Board(project_id=project_id, fields=fields, items=items)


def _as_int(value: Any) -> int | None:
    if isinstance(value, bool) or not isinstance(value, int | float):
        return None
    return int(value)


def plan_changes(
    rows: Sequence[DerivedRow],
    board: Board,
) -> tuple[tuple[Change, ...], tuple[int, ...]]:
    """Return `(changes, issues absent from the board)`.

    A change is emitted only where the board's stored value differs from the derived one,
    which is what makes a second run report zero changes.
    """
    required_fields = (WAVE_FIELD, FAN_IN_FIELD, FAN_OUT_FIELD, STATUS_FIELD)
    missing_fields = tuple(field for field in required_fields if field not in board.fields)
    if missing_fields:
        raise RuntimeError(
            "Project is missing required derived field definitions: "
            + ", ".join(repr(field) for field in missing_fields)
        )
    changes: list[Change] = []
    absent: list[int] = []
    for row in rows:
        item = board.items.get(row.number)
        if item is None:
            absent.append(row.number)
            continue
        desired: list[tuple[str, str | int]] = [
            (WAVE_FIELD, row.wave),
            (FAN_IN_FIELD, row.fan_in),
            (FAN_OUT_FIELD, row.fan_out),
        ]
        if row.status is not None:
            desired.append((STATUS_FIELD, row.status))
        for field_name, value in desired:
            current = item.value(field_name)
            if current != value:
                changes.append(
                    Change(
                        number=row.number,
                        item_id=item.item_id,
                        field_name=field_name,
                        current=current,
                        desired=value,
                    )
                )
    return tuple(changes), tuple(sorted(absent))


def apply_change(board: Board, change: Change) -> None:
    """Write one derived value to Project 9."""
    field = board.fields[change.field_name]
    if field.data_type == "SINGLE_SELECT" or field.options:
        graphql(
            _SET_SELECT_MUTATION,
            {
                "project": board.project_id,
                "item": change.item_id,
                "field": field.field_id,
                "option": field.option_id(str(change.desired)),
            },
        )
        return
    graphql(
        _SET_NUMBER_MUTATION,
        {
            "project": board.project_id,
            "item": change.item_id,
            "field": field.field_id,
            "value": int(change.desired),
        },
    )


# --------------------------------------------------------------------------------------
# CLI
# --------------------------------------------------------------------------------------


def _load_issues(arguments: argparse.Namespace) -> tuple[IssueRecord, ...]:
    if arguments.snapshot is not None:
        return load_snapshot(
            arguments.snapshot,
            expected_repository=arguments.repository,
        )
    owner, _, name = arguments.repository.partition("/")
    return fetch_issues(owner, name)


def _satisfaction(
    arguments: argparse.Namespace,
    issues: Sequence[IssueRecord],
    *,
    accepted: AcceptedRevision | None = None,
) -> tuple[frozenset[int], tuple[int, ...]]:
    if arguments.satisfaction == "closed":
        return bootstrap_satisfied(issues, closure_root=None)
    revision = accepted or accepted_revision()
    return bootstrap_satisfied(
        issues,
        closure_root=arguments.closure_root,
        accepted=revision,
        schema_path=arguments.closure_schema,
    )


def _report(rows: Sequence[DerivedRow], extra: dict[str, Any]) -> dict[str, Any]:
    stamp = provenance.collect()
    return {
        **stamp.as_json_object(),
        "source": "github native blockedBy connections",
        "issue_count": len(rows),
        "atoms": [row.as_json_object() for row in rows],
        **extra,
    }


def _command_derive(arguments: argparse.Namespace) -> int:
    if arguments.snapshot is not None:
        raise ValueError(
            "derive does not support --snapshot; use ready --inspection for offline inspection"
        )
    if arguments.apply:
        if arguments.satisfaction == "closed":
            raise ValueError("--apply requires --satisfaction closure-record")
        preflight(
            repository=arguments.repository,
            project_owner=arguments.owner,
            project_number=arguments.project_number,
        )
    accepted = accepted_revision()
    issues = _load_issues(arguments)
    satisfied, unevidenced = _satisfaction(arguments, issues, accepted=accepted)
    board: Board | None = None
    board_status: dict[int, str | None] = {}
    if arguments.apply or arguments.compare_board:
        board = fetch_board(
            arguments.owner, arguments.project_number, owner_type=arguments.owner_type
        )
        board_status = {number: item.status for number, item in board.items.items()}

    rows = derive(issues, satisfied=satisfied, board_status=board_status)
    extra: dict[str, Any] = {
        "accepted_commit_id": accepted.commit_id,
        "accepted_change_id": accepted.change_id,
        "accepted_source_commit_id": accepted.commit_id,
        "accepted_source_change_id": accepted.change_id,
        "closed_without_closure_record": list(unevidenced),
        "applied": 0,
        "changes": [],
        "absent_from_board": [],
    }
    if board is not None:
        changes, absent = plan_changes(rows, board)
        extra["changes"] = [change.as_json_object() for change in changes]
        extra["absent_from_board"] = list(absent)
        if arguments.apply:
            for change in changes:
                apply_change(board, change)
            extra["applied"] = len(changes)

    encoded = json.dumps(_report(rows, extra), indent=2, sort_keys=True)
    print(encoded)
    if arguments.report:
        arguments.report.parent.mkdir(parents=True, exist_ok=True)
        arguments.report.write_text(encoded + "\n", encoding="utf-8")

    if extra["absent_from_board"]:
        # An Atom the board does not carry has no derived fields at all, so silence here
        # would read as "everything is projected" when part of the graph is invisible.
        print(
            "absent from Project "
            f"{arguments.project_number}: "
            + ", ".join(f"#{number}" for number in extra["absent_from_board"])
            + "; run gordian-project-sync",
            file=sys.stderr,
        )
        return 1
    return 0


def _command_ready(arguments: argparse.Namespace) -> int:
    accepted = accepted_revision()
    issues = _load_issues(arguments)
    satisfied, unevidenced = _satisfaction(arguments, issues, accepted=accepted)
    rows = derive(issues, satisfied=satisfied)
    ready = ready_set(rows)
    open_rows = selection_order(row for row in rows if row.state.upper() == "OPEN")
    inspection = bool(arguments.inspection)
    if arguments.snapshot is not None and not inspection:
        diagnostic = (
            "offline registry snapshots are inspection-only; refusing to emit dispatchable "
            f"readiness rows (accepted commit {accepted.commit_id})"
        )
        if arguments.json:
            print(
                json.dumps(
                    _report(
                        (),
                        {
                            "accepted_commit_id": accepted.commit_id,
                            "accepted_change_id": accepted.change_id,
                            "accepted_source_commit_id": accepted.commit_id,
                            "accepted_source_change_id": accepted.change_id,
                            "snapshot_issue_count": len(issues),
                            "closed_without_closure_record": list(unevidenced),
                            "selection_keys": list(SELECTION_KEYS),
                            "dispatchable": False,
                            "inspection_only": False,
                            "diagnostic": diagnostic,
                        },
                    ),
                    indent=2,
                    sort_keys=True,
                )
            )
        print(diagnostic, file=sys.stderr)
        return 2
    if arguments.satisfaction == "closed" and not inspection:
        diagnostic = (
            "the weaker --satisfaction closed rule is inspection-only; refusing to emit "
            "dispatchable readiness rows"
        )
        if arguments.json:
            print(
                json.dumps(
                    _report(
                        (),
                        {
                            "accepted_commit_id": accepted.commit_id,
                            "accepted_change_id": accepted.change_id,
                            "accepted_source_commit_id": accepted.commit_id,
                            "accepted_source_change_id": accepted.change_id,
                            "closed_without_closure_record": list(unevidenced),
                            "selection_keys": list(SELECTION_KEYS),
                            "dispatchable": False,
                            "inspection_only": False,
                            "diagnostic": diagnostic,
                        },
                    ),
                    indent=2,
                    sort_keys=True,
                )
            )
        print(diagnostic, file=sys.stderr)
        return 2
    listed = open_rows if arguments.all else ready

    if arguments.json:
        print(
            json.dumps(
                _report(
                    listed,
                    {
                        "accepted_commit_id": accepted.commit_id,
                        "accepted_change_id": accepted.change_id,
                        "accepted_source_commit_id": accepted.commit_id,
                        "accepted_source_change_id": accepted.change_id,
                        "closed_without_closure_record": list(unevidenced),
                        "selection_keys": list(SELECTION_KEYS),
                        "dispatchable": not inspection,
                        "inspection_only": inspection,
                    },
                ),
                indent=2,
                sort_keys=True,
            )
        )
    else:
        if inspection:
            print(
                "inspection only: these rows are not dispatchable from this command",
                file=sys.stderr,
            )
        for row in listed:
            status = row.status or "-"
            print(
                f"#{row.number:<4} wave={row.wave:<3} fan_in={row.fan_in:<3} "
                f"fan_out={row.fan_out:<3} {status:<8} {row.title}"
            )
        print(f"{len(ready)} ready of {len(open_rows)} open")

    if unevidenced:
        print(
            f"closed without a validating closure record at accepted commit {accepted.commit_id}: "
            + ", ".join(f"#{number}" for number in unevidenced),
            file=sys.stderr,
        )
        return 1
    if inspection:
        return 0
    return 0


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="gordian-derive-status",
        description=(
            "Project GitHub's native blocked-by graph onto Project 9's derived Wave, "
            "Fan In, Fan Out and Status fields. Not Mission Graph satisfaction."
        ),
    )
    parser.add_argument("--owner", default=DEFAULT_OWNER)
    parser.add_argument("--owner-type", choices=("user", "organization"), default="user")
    parser.add_argument("--repository", default=DEFAULT_REPOSITORY)
    parser.add_argument(
        "--project", type=int, default=DEFAULT_PROJECT_NUMBER, dest="project_number"
    )
    parser.add_argument(
        "--snapshot",
        type=Path,
        default=None,
        help=f"Read edges from a committed snapshot (e.g. {DEFAULT_SNAPSHOT}) instead of gh.",
    )
    parser.add_argument(
        "--satisfaction",
        choices=("closure-record", "closed"),
        default="closure-record",
        help="Bootstrap satisfaction rule; `closed` is GitHub's weaker rule, for inspection.",
    )
    parser.add_argument("--closure-root", type=Path, default=DEFAULT_CLOSURE_ROOT)
    parser.add_argument("--closure-schema", type=Path, default=DEFAULT_CLOSURE_SCHEMA)

    subparsers = parser.add_subparsers(dest="command")

    derive_parser = subparsers.add_parser(
        "derive", help="Compute the derived fields and optionally write them to Project 9."
    )
    derive_parser.add_argument(
        "--apply", action="store_true", help="Write the derived values to Project 9."
    )
    derive_parser.add_argument(
        "--compare-board",
        action="store_true",
        help="Read Project 9 and report the pending changes without writing them.",
    )
    derive_parser.add_argument(
        "--snapshot",
        type=Path,
        dest="snapshot",
        default=argparse.SUPPRESS,
        help=argparse.SUPPRESS,
    )
    derive_parser.add_argument("--report", type=Path)
    derive_parser.set_defaults(handler=_command_derive)

    ready_parser = subparsers.add_parser(
        "ready",
        help=(
            "Print the ready set in the execution-order.md section 5 selection order "
            "(lowest Wave, then highest Fan Out, then lowest issue number); exit "
            "non-zero on unevidenced closure."
        ),
    )
    ready_parser.add_argument("--json", action="store_true")
    ready_parser.add_argument(
        "--snapshot",
        type=Path,
        dest="snapshot",
        default=argparse.SUPPRESS,
        help=argparse.SUPPRESS,
    )
    ready_parser.add_argument(
        "--all",
        action="store_true",
        dest="all",
        help=(
            "List every open Atom with its wave and derived status, not only the ready "
            "set, in the same selection order."
        ),
    )
    ready_parser.add_argument(
        "--inspection",
        action="store_true",
        help=(
            "Explicitly mark this invocation as non-dispatching inspection; required for "
            "the weaker satisfaction rule and useful with an offline snapshot."
        ),
    )
    ready_parser.set_defaults(handler=_command_ready)

    parser.set_defaults(
        handler=_command_derive,
        apply=False,
        compare_board=False,
        report=None,
        json=False,
        all=False,
        inspection=False,
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    arguments = _parser().parse_args(argv)
    try:
        return int(arguments.handler(arguments))
    except DependencyCycleError as error:
        print(str(error), file=sys.stderr)
        return 2
    except GitHubConfigurationError as error:
        print(str(error), file=sys.stderr)
        print(GH_AUTH_HINT, file=sys.stderr)
        return EX_CONFIG
    except (OSError, RuntimeError, KeyError, ValueError) as error:
        print(str(error), file=sys.stderr)
        print(GH_AUTH_HINT, file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
