"""Derive Project 9's Wave, Fan In, Fan Out and Status from the native blocked-by graph.

Every value this module computes is a **projection of GitHub's own issue-dependency
edges** onto the temporary Project 9 board. The module owns no Mission Graph semantics:
it does not decide Atom readiness, hard dependency validity, evidence compatibility,
candidate admission, or satisfaction. Those decisions belong to Rust. This module exists
only because the bootstrap Mission has no native dependency store yet, and **it is
deleted when #48 lands**.

Edge source
-----------
Edges are read from `blockedByIssues` node lists, either live through `gh api graphql`
or from the committed `artifacts/atoms/issues.json` snapshot. The
`issueDependenciesSummary.blocking` counter is **never** read: it is wrong for #11, #18
and #44. The `issueDependenciesSummary.blockedBy` counter is read for one purpose only,
to assert that `blockedByIssues` pagination retrieved every edge.

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
**and** `artifacts/atoms/<N>/closure.json` exists at the commit under evaluation **and**
validates against the closure-record schema. This is the bootstrap analogue of
Satisfied-as-admitted; closing an issue on its own is bookkeeping, not evidence. The
`ready` subcommand exits non-zero when any issue is closed without a validating closure
record, because the readiness it printed would otherwise rest on an unevidenced closure.
"""

from __future__ import annotations

import argparse
import heapq
import json
import sys
from collections.abc import Iterable, Mapping, Sequence
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

from . import provenance
from .gh import GH_AUTH_HINT, graphql

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


def closure_record_valid(path: Path, schema: Mapping[str, Any] | None) -> bool:
    """Check a closure record against the required keys the closure schema declares.

    This is a deliberately shallow structural check: the authoritative validation is
    `scripts/check-closure-records.sh` in CI. Its job here is only to stop a bare
    `git commit` of an empty file from silently unblocking the whole downstream graph.
    """
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return False
    if not isinstance(payload, dict):
        return False
    required = (schema or {}).get("required", [])
    if not isinstance(required, list):
        return False
    return all(
        key in payload and payload[key] not in (None, "", [], {}) for key in required
    )


def load_closure_schema(path: Path) -> Mapping[str, Any] | None:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return payload if isinstance(payload, dict) else None


def bootstrap_satisfied(
    issues: Sequence[IssueRecord],
    *,
    closure_root: Path | None,
    schema: Mapping[str, Any] | None = None,
) -> tuple[frozenset[int], tuple[int, ...]]:
    """Return `(satisfied issue numbers, closed issues lacking a valid closure record)`.

    With `closure_root=None` the caller has opted into GitHub's own weaker rule, in which
    closure alone unblocks a dependent. That mode is for inspecting the raw graph, never
    for picking the next Atom.
    """
    closed = [record.number for record in issues if not record.is_open]
    if closure_root is None:
        return frozenset(closed), ()
    satisfied: set[int] = set()
    unevidenced: list[int] = []
    for number in closed:
        if closure_record_valid(closure_record_path(closure_root, number), schema):
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
           orderBy:{field:NUMBER,direction:ASC}){
      pageInfo{hasNextPage endCursor}
      nodes{
        number
        title
        state
        issueDependenciesSummary{blockedBy}
        blockedByIssues(first:100){
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
      blockedByIssues(first:100, after:$cursor){
        pageInfo{hasNextPage endCursor}
        nodes{number}
      }
    }
  }
}
"""


def _blocked_by_rest(owner: str, name: str, number: int, cursor: str) -> list[int]:
    """Continue `blockedByIssues` pagination for one issue."""
    numbers: list[int] = []
    while cursor:
        data = graphql(
            _BLOCKED_BY_PAGE_QUERY,
            {"owner": owner, "name": name, "number": number, "cursor": cursor},
        )
        connection = data["repository"]["issue"]["blockedByIssues"]
        numbers.extend(issue_number(node) for node in connection["nodes"])
        page = connection["pageInfo"]
        cursor = page["endCursor"] if page["hasNextPage"] else ""
    return numbers


def fetch_issues(owner: str, name: str) -> tuple[IssueRecord, ...]:
    """Read every issue and its complete `blockedByIssues` node list through `gh`."""
    records: list[IssueRecord] = []
    cursor = ""
    while True:
        variables: dict[str, str | int] = {"owner": owner, "name": name}
        if cursor:
            variables["cursor"] = cursor
        data = graphql(_ISSUES_QUERY, variables)
        connection = data["repository"]["issues"]
        for node in connection["nodes"]:
            number = issue_number(node)
            blocked = node["blockedByIssues"]
            numbers = [issue_number(edge) for edge in blocked["nodes"]]
            page = blocked["pageInfo"]
            if page["hasNextPage"]:
                numbers.extend(_blocked_by_rest(owner, name, number, page["endCursor"]))
            summary = node.get("issueDependenciesSummary") or {}
            declared = summary.get("blockedBy")
            if isinstance(declared, int) and declared != len(set(numbers)):
                raise RuntimeError(
                    f"#{number}: retrieved {len(set(numbers))} blocked-by edges but GitHub "
                    f"reports {declared}; pagination is incomplete"
                )
            records.append(
                IssueRecord(
                    number=number,
                    title=str(node["title"]),
                    state=str(node["state"]),
                    blocked_by=tuple(sorted(set(numbers))),
                )
            )
        page = connection["pageInfo"]
        if not page["hasNextPage"]:
            break
        cursor = page["endCursor"]
    return tuple(sorted(records, key=lambda record: record.number))


def load_snapshot(path: Path) -> tuple[IssueRecord, ...]:
    """Read issues and their blocked-by node lists from the committed snapshot."""
    payload = json.loads(path.read_text(encoding="utf-8"))
    rows = payload["issues"] if isinstance(payload, dict) else payload
    if not isinstance(rows, list):
        raise RuntimeError(f"{path}: expected a list of issue records")
    records: list[IssueRecord] = []
    for row in rows:
        if not isinstance(row, dict):
            raise RuntimeError(f"{path}: expected a list of issue records")
        blocked = row.get("blockedBy", row.get("blocked_by", ()))
        records.append(
            IssueRecord(
                number=issue_number(row),
                title=str(row.get("title", "")),
                state=str(row.get("state", "OPEN")),
                blocked_by=tuple(sorted({issue_number(edge) for edge in blocked})),
            )
        )
    return tuple(sorted(records, key=lambda record: record.number))


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
    while True:
        variables: dict[str, str | int] = {"owner": owner, "number": project_number}
        if cursor:
            variables["cursor"] = cursor
        data = graphql(query, variables)
        project = (data.get(owner_type) or {}).get("projectV2")
        if not project:
            raise RuntimeError(f"{owner_type} {owner!r} has no project number {project_number}")
        project_id = str(project["id"])
        for node in project["fields"]["nodes"]:
            if not node:
                continue
            options = tuple(
                (str(option["name"]), str(option["id"])) for option in node.get("options", ())
            )
            fields[str(node["name"])] = ProjectField(
                field_id=str(node["id"]),
                name=str(node["name"]),
                data_type=str(node.get("dataType", "")),
                options=options,
            )
        for node in project["items"]["nodes"]:
            content = node.get("content") or {}
            if "number" not in content:
                continue
            number = issue_number(content)
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
        page = project["items"]["pageInfo"]
        if not page["hasNextPage"]:
            break
        cursor = page["endCursor"]
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
            if field_name not in board.fields:
                continue
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
        return load_snapshot(arguments.snapshot)
    owner, _, name = arguments.repository.partition("/")
    return fetch_issues(owner, name)


def _satisfaction(
    arguments: argparse.Namespace, issues: Sequence[IssueRecord]
) -> tuple[frozenset[int], tuple[int, ...]]:
    if arguments.satisfaction == "closed":
        return bootstrap_satisfied(issues, closure_root=None)
    schema = load_closure_schema(arguments.closure_schema)
    return bootstrap_satisfied(issues, closure_root=arguments.closure_root, schema=schema)


def _report(rows: Sequence[DerivedRow], extra: dict[str, Any]) -> dict[str, Any]:
    stamp = provenance.collect()
    return {
        **stamp.as_json_object(),
        "source": "github native blockedBy node lists",
        "issue_count": len(rows),
        "atoms": [row.as_json_object() for row in rows],
        **extra,
    }


def _command_derive(arguments: argparse.Namespace) -> int:
    issues = _load_issues(arguments)
    satisfied, unevidenced = _satisfaction(arguments, issues)
    board: Board | None = None
    board_status: dict[int, str | None] = {}
    if arguments.apply or arguments.compare_board:
        board = fetch_board(
            arguments.owner, arguments.project_number, owner_type=arguments.owner_type
        )
        board_status = {number: item.status for number, item in board.items.items()}

    rows = derive(issues, satisfied=satisfied, board_status=board_status)
    extra: dict[str, Any] = {
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
    issues = _load_issues(arguments)
    satisfied, unevidenced = _satisfaction(arguments, issues)
    rows = derive(issues, satisfied=satisfied)
    ready = ready_set(rows)
    open_rows = selection_order(row for row in rows if row.state.upper() == "OPEN")
    listed = open_rows if arguments.all else ready

    if arguments.json:
        print(
            json.dumps(
                _report(
                    listed,
                    {
                        "closed_without_closure_record": list(unevidenced),
                        "selection_keys": list(SELECTION_KEYS),
                    },
                ),
                indent=2,
                sort_keys=True,
            )
        )
    else:
        for row in listed:
            status = row.status or "-"
            print(
                f"#{row.number:<4} wave={row.wave:<3} fan_in={row.fan_in:<3} "
                f"fan_out={row.fan_out:<3} {status:<8} {row.title}"
            )
        print(f"{len(ready)} ready of {len(open_rows)} open")

    if unevidenced:
        print(
            "closed without a validating closure record: "
            + ", ".join(f"#{number}" for number in unevidenced),
            file=sys.stderr,
        )
        return 1
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
        "--all",
        action="store_true",
        dest="all",
        help=(
            "List every open Atom with its wave and derived status, not only the ready "
            "set, in the same selection order."
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
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    arguments = _parser().parse_args(argv)
    try:
        return int(arguments.handler(arguments))
    except DependencyCycleError as error:
        print(str(error), file=sys.stderr)
        return 2
    except (OSError, RuntimeError, KeyError, ValueError) as error:
        print(str(error), file=sys.stderr)
        print(GH_AUTH_HINT, file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
