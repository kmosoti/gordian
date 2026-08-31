"""Audit and capture the temporary GitHub Atom registry.

This module is bootstrap orchestration for issue #70. It does not decide Mission Graph
semantics. It checks that GitHub's native dependency links, the issue-body mirror, and
the repository's generated planning views agree, and it can capture that already-agreed
external state as ``artifacts/atoms/issues.json``. Rust replaces it when #48 imports the
plan into the native Mission Graph.
"""

from __future__ import annotations

import argparse
import contextlib
import fcntl
import hashlib
import json
import os
import re
import sys
import tempfile
import time
from collections import Counter, defaultdict
from collections.abc import Mapping, Sequence
from contextlib import suppress
from dataclasses import asdict, dataclass, replace
from datetime import UTC, datetime
from functools import cache
from pathlib import Path
from typing import Any

from . import provenance
from .bootstrap_claims import ClaimLease, require_live_claim
from .derive_status import (
    DEFAULT_CLOSURE_ROOT,
    DEFAULT_CLOSURE_SCHEMA,
    DEFAULT_REPOSITORY,
    DEFAULT_SNAPSHOT,
    IssueRecord,
    apply_change,
    bootstrap_satisfied,
    derive,
    fetch_board,
    fetch_issues,
    load_closure_schema,
    load_snapshot,
    normalise_graph,
    plan_changes,
    topological_order,
    transitive_blockers,
    waves,
)
from .gh import (
    EX_CONFIG,
    GH_AUTH_HINT,
    GitHubConfigurationError,
    preflight,
    run_gh,
    run_gh_json,
)
from .normalization_journal import (
    CANONICAL_ATOM_NUMBERS as _CANONICAL_ATOM_NUMBERS,
)
from .normalization_journal import (
    CANONICAL_CLOSURE_TEMPLATE as _CANONICAL_CLOSURE_TEMPLATE,
)
from .normalization_journal import (
    DEFAULT_ATOM as _DEFAULT_NORMALIZATION_ATOM,
)
from .normalization_journal import (
    DEFAULT_JOURNAL as DEFAULT_NORMALIZATION_JOURNAL,
)
from .normalization_journal import (
    DEFAULT_MANIFEST as DEFAULT_NORMALIZATION_MANIFEST,
)
from .normalization_journal import (
    INTEGRATION_VERIFIER_IDS as _INTEGRATION_VERIFIER_IDS,
)
from .normalization_journal import (
    BodyOperation as _BodyOperation,
)
from .normalization_journal import (
    BodyReplacement as _BodyReplacement,
)
from .normalization_journal import (
    EdgeOperation as _EdgeOperation,
)
from .normalization_journal import (
    LabelOperation as _LabelOperation,
)
from .normalization_journal import (
    NormalizationConflict as _NormalizationConflict,
)
from .normalization_journal import (
    NormalizationError as _NormalizationError,
)
from .normalization_journal import (
    NormalizationManifest as _NormalizationManifest,
)
from .normalization_journal import (
    NormalizationPlan as _NormalizationPlan,
)
from .normalization_journal import (
    NormalizationTransform as _NormalizationTransform,
)
from .normalization_journal import (
    advance_journal,
    journal_complete,
    load_manifest,
    plan_normalization,
    read_journal,
    recover_journal,
    write_journal,
)
from .normalization_journal import (
    canonical_closure_intent as _canonical_closure_intent,
)
from .normalization_journal import (
    canonical_closure_wording as _canonical_closure_wording,
)
from .normalization_journal import (
    fetch_label_record as _fetch_normalization_label_record,
)
from .normalization_journal import (
    validate_manifest_coverage as _validate_manifest_coverage,
)

# Re-export the manifest/plan value objects from the registry entry point.  The
# CLI remains the primary interface, but keeping these names here lets callers
# build a deterministic plan without depending on module layout.
BodyReplacement = _BodyReplacement
BodyOperation = _BodyOperation
EdgeOperation = _EdgeOperation
LabelOperation = _LabelOperation
NormalizationManifest = _NormalizationManifest
NormalizationPlan = _NormalizationPlan
NormalizationTransform = _NormalizationTransform
NormalizationConflict = _NormalizationConflict
NormalizationError = _NormalizationError

DEFAULT_PROJECT_PLAN = Path("docs/implementation/project-plan.md")
DEFAULT_EXECUTION_ORDER = Path("docs/implementation/execution-order.md")
DEFAULT_ISSUE_INDEX = Path("docs/implementation/issue-index.md")
DEFAULT_CRATE_MAP = Path("docs/implementation/crate-map.md")
DEFAULT_KNOWLEDGE_GRAPH = Path("knowledge/graph/90-project-plan.jsonld")
DEFAULT_TARGET = 69
DEFAULT_REGISTRY_JOURNAL = Path("artifacts/atoms/registry-operation-journal.json")
TYPE_LABELS = frozenset(("type:atom", "type:experiment"))
SPINE_BEGIN = "<!-- BEGIN GENERATED: MAXIMUM-LENGTH SPINE -->"
SPINE_END = "<!-- END GENERATED: MAXIMUM-LENGTH SPINE -->"
BENCHMARK_OWNER_BEGIN = "<!-- BEGIN GENERATED: EO17 OWNERSHIP -->"
BENCHMARK_OWNER_END = "<!-- END GENERATED: EO17 OWNERSHIP -->"
BENCHMARK_GATE_BEGIN = "<!-- BEGIN GENERATED: EO17 FIRST QUALIFICATION -->"
BENCHMARK_GATE_END = "<!-- END GENERATED: EO17 FIRST QUALIFICATION -->"
SELFHOST_BEGIN = "<!-- BEGIN GENERATED: SELF-HOSTING CLOSURE -->"
SELFHOST_END = "<!-- END GENERATED: SELF-HOSTING CLOSURE -->"
INITIATIVE_BEGIN = "<!-- BEGIN GENERATED: INITIATIVE REGISTER -->"
INITIATIVE_END = "<!-- END GENERATED: INITIATIVE REGISTER -->"
TARGET_CRATE_BEGIN = "<!-- BEGIN GENERATED: TARGET CRATE -->"
TARGET_CRATE_END = "<!-- END GENERATED: TARGET CRATE -->"

# The crate map is the only authority for Rust ownership.  A small number of
# experiment/conformance Atoms intentionally exercise a shared implementation
# crate without owning it.  Keep these exceptions explicit and finite: each
# name still has to be a real crate-map row, and no unknown package/path is
# accepted as an alias.
SHARED_TEST_TARGET_EXCEPTIONS: dict[int, frozenset[str]] = {
    # The differential conformance Atom runs the shared Mission Graph tests.
    7: frozenset(("gordian-core",)),
    # The source-substrate comparison exercises both backend adapters.
    34: frozenset(("gordian-git", "gordian-jj")),
    # The Git adapter runs the shared source-adapter contract suite only in tests.
    76: frozenset(("gordian-source",)),
}

_DEPENDENCIES_HEADING = re.compile(r"^## Dependencies[ \t]*$", re.MULTILINE)
_NEXT_HEADING = re.compile(r"^## [^\n]+$", re.MULTILINE)
_ISSUE_REFERENCE = re.compile(r"(?<![A-Za-z0-9_-])#(\d+)\b")
_BENCHMARK_ID = re.compile(r"\bEO17-[A-Z]+-\d+\b")
_CRATE_NAME = r"gordian-[a-z0-9]+(?:-[a-z0-9]+)*"
_CRATE_NAME_RE = re.compile(rf"{_CRATE_NAME}")
_CRATE_CELL_RE = re.compile(rf"`({_CRATE_NAME})`")
_CRATE_PATH_RE = re.compile(rf"\bcrates/({_CRATE_NAME})\b")
_CARGO_PACKAGE_RE = re.compile(rf"\bcargo\b[^\r\n]*?(?:^|[ \t])-p[ \t]+({_CRATE_NAME})\b")

# A single operation may be retried after a lease renewal or by another process.
# The lock only serializes writers on this checkout; all external recovery still
# uses the marker and canonical GitHub rereads below.  Keeping the lock bounded
# prevents a wedged local process from turning an idempotent retry into an
# unbounded wait.
_OPERATION_LOCK_TIMEOUT_SECONDS = 5.0
_OPERATION_LOCK_POLL_SECONDS = 0.05
_HELD_OPERATION_LOCKS: set[Path] = set()


@dataclass(frozen=True, slots=True)
class PlanRow:
    number: int
    title: str
    target_crate: str
    blocked_by: tuple[int, ...]
    initiative: str


@dataclass(frozen=True, slots=True)
class CrateMapRow:
    """One validated row from the normative Rust crate ownership map."""

    crate: str
    path: str
    owners: tuple[int, ...]


@dataclass(frozen=True, slots=True)
class Spine:
    nodes: tuple[int, ...]
    edges: tuple[tuple[int, int], ...]
    length: int


@dataclass(frozen=True, slots=True)
class BenchmarkObligation:
    row_id: str
    owner: int
    first_qualification: bool


@dataclass(frozen=True, slots=True)
class BenchmarkAuditReport:
    row_count: int
    first_qualification_ids: tuple[str, ...]
    owners: tuple[int, ...]
    problems: tuple[str, ...]

    @property
    def clean(self) -> bool:
        return not self.problems

    def as_json_object(self) -> dict[str, Any]:
        return {**asdict(self), "clean": self.clean}


@dataclass(frozen=True, slots=True)
class TargetCrateAuditReport:
    owner_count: int
    problems: tuple[str, ...]

    @property
    def clean(self) -> bool:
        return not self.problems

    def as_json_object(self) -> dict[str, Any]:
        return {**asdict(self), "clean": self.clean}


@dataclass(frozen=True, slots=True)
class ContractLintReport:
    """Static findings for normalized Atom target/Closure contracts."""

    issue_count: int
    problems: tuple[str, ...]

    @property
    def clean(self) -> bool:
        return not self.problems

    def as_json_object(self) -> dict[str, Any]:
        return {**asdict(self), "clean": self.clean}


@dataclass(frozen=True, slots=True)
class EdgePlan:
    issue: int
    blocked_by: int
    previous_blockers: tuple[int, ...]
    proposed_blockers: tuple[int, ...]
    changed: bool

    def as_json_object(self) -> dict[str, Any]:
        return asdict(self)


@dataclass(frozen=True, slots=True)
class NewAtomSpec:
    title: str
    body: str
    milestone: str
    type_label: str
    target_crate: str | None
    phase: int
    blocked_by: tuple[int, ...]
    blocks: tuple[int, ...]
    knowledge_node: dict[str, Any]


@dataclass(frozen=True, slots=True)
class NewAtomPlan:
    provisional_number: int
    title: str
    milestone: str
    type_label: str
    target_crate: str | None
    phase: int
    blocked_by: tuple[int, ...]
    blocks: tuple[int, ...]

    def as_json_object(self) -> dict[str, Any]:
        return asdict(self)


@dataclass(frozen=True, slots=True)
class AuditReport:
    issue_count: int
    target: int
    maximum_path_length: int
    spine_nodes: tuple[int, ...]
    spine_edges: tuple[tuple[int, int], ...]
    problems: tuple[str, ...]
    generated_at: str
    source_change_id: str
    source_commit_id: str
    tool_versions: dict[str, str]

    @property
    def clean(self) -> bool:
        return not self.problems

    def as_json_object(self) -> dict[str, Any]:
        return {**asdict(self), "clean": self.clean}


def dependencies_from_body(body: str) -> tuple[int, ...]:
    """Parse the human-readable ``## Dependencies`` mirror from one Atom body.

    This parser is a drift checker only. Native GitHub links remain authoritative and
    are never reconstructed from this result.
    """
    heading = _DEPENDENCIES_HEADING.search(body)
    if heading is None:
        raise ValueError("missing `## Dependencies` section")
    following = body[heading.end() :]
    next_heading = _NEXT_HEADING.search(following)
    section = following[: next_heading.start()] if next_heading else following
    stripped = section.strip()
    if re.fullmatch(r"(?:[-*][ \t]+)?None\.?", stripped, re.IGNORECASE):
        return ()
    references = tuple(sorted({int(number) for number in _ISSUE_REFERENCE.findall(section)}))
    if not references:
        raise ValueError("`## Dependencies` names neither `None` nor an issue")
    return references


def replace_body_dependencies(body: str, blockers: tuple[int, ...]) -> str:
    """Rewrite only the human dependency mirror, preserving every other section."""
    heading = _DEPENDENCIES_HEADING.search(body)
    if heading is None:
        raise ValueError("missing `## Dependencies` section")
    following = body[heading.end() :]
    next_heading = _NEXT_HEADING.search(following)
    section_end = heading.end() + next_heading.start() if next_heading else len(body)
    rendered = "None" if not blockers else "\n".join(f"- #{number}" for number in blockers)
    suffix = body[section_end:]
    separator = "\n\n" if suffix else "\n"
    return body[: heading.end()] + "\n" + rendered + separator + suffix


def _unknown_blocked_by(issues: tuple[IssueRecord, ...]) -> tuple[tuple[int, int], ...]:
    """Return native edges whose blocker is absent from the executable registry.

    ``normalise_graph`` intentionally totalizes partial graph inputs for generic graph
    algorithms.  Registry validation has a stricter contract: a native edge to a closed
    duplicate (or any other omitted issue) is invalid and must not become an implicit Atom.
    """
    known = {issue.number for issue in issues}
    return tuple(
        sorted(
            {
                (issue.number, blocker)
                for issue in issues
                for blocker in issue.blocked_by
                if blocker not in known
            }
        )
    )


def _require_known_blocked_by(issues: tuple[IssueRecord, ...]) -> None:
    """Reject registry edges to identities outside the executable Atom set."""
    unknown = _unknown_blocked_by(issues)
    if unknown:
        source, blocker = unknown[0]
        raise ValueError(
            f"#{source}: native blocker #{blocker} is absent from the executable Atom registry"
        )


def plan_add_edge(
    issues: tuple[IssueRecord, ...], *, issue_number: int, blocker_number: int
) -> tuple[EdgePlan, tuple[IssueRecord, ...]]:
    """Plan one native ``blockedBy`` addition and reject missing nodes or cycles."""
    _require_known_blocked_by(issues)
    records = {issue.number: issue for issue in issues}
    if issue_number == blocker_number:
        raise ValueError("an Atom cannot block itself")
    if issue_number not in records:
        raise ValueError(f"Atom #{issue_number} is absent from the registry")
    if blocker_number not in records:
        raise ValueError(f"blocker #{blocker_number} is absent from the registry")
    target = records[issue_number]
    previous = target.blocked_by
    if blocker_number in previous:
        return (
            EdgePlan(
                issue=issue_number,
                blocked_by=blocker_number,
                previous_blockers=previous,
                proposed_blockers=previous,
                changed=False,
            ),
            issues,
        )
    proposed = tuple(sorted((*previous, blocker_number)))
    updated_target = replace(
        target,
        blocked_by=proposed,
        body=replace_body_dependencies(target.body, proposed),
    )
    updated = tuple(
        updated_target if issue.number == issue_number else issue for issue in issues
    )
    topological_order(normalise_graph({issue.number: issue.blocked_by for issue in updated}))
    return (
        EdgePlan(
            issue=issue_number,
            blocked_by=blocker_number,
            previous_blockers=previous,
            proposed_blockers=proposed,
            changed=True,
        ),
        updated,
    )


def _issue_database_id(repository: str, number: int) -> int:
    payload = run_gh_json(["api", f"repos/{repository}/issues/{number}"])
    if not isinstance(payload, dict) or isinstance(payload.get("id"), bool):
        raise RuntimeError(f"#{number}: GitHub API returned no issue database id")
    try:
        return int(payload["id"])
    except (KeyError, TypeError, ValueError) as error:
        raise RuntimeError(f"#{number}: GitHub API returned no issue database id") from error


def _add_native_edge(repository: str, issue: int, blocker: int) -> None:
    blocker_id = _issue_database_id(repository, blocker)
    run_gh(
        [
            "api",
            "--method",
            "POST",
            f"repos/{repository}/issues/{issue}/dependencies/blocked_by",
            "-H",
            "X-GitHub-Api-Version: 2026-03-10",
            "-F",
            f"issue_id={blocker_id}",
        ]
    )


def _remove_native_edge(repository: str, issue: int, blocker: int) -> None:
    blocker_id = _issue_database_id(repository, blocker)
    run_gh(
        [
            "api",
            "--method",
            "DELETE",
            (
                f"repos/{repository}/issues/{issue}/dependencies/blocked_by/"
                f"{blocker_id}"
            ),
            "-H",
            "X-GitHub-Api-Version: 2026-03-10",
        ]
    )


def _markdown_section(body: str, heading: str) -> str | None:
    match = re.search(rf"^## {re.escape(heading)}[ \t]*$", body, re.MULTILINE)
    if match is None:
        return None
    following = body[match.end() :]
    next_heading = _NEXT_HEADING.search(following)
    return following[: next_heading.start()] if next_heading else following


def _has_heading(body: str, prefix: str) -> bool:
    return re.search(rf"^## {re.escape(prefix)}(?:\b| )", body, re.MULTILINE) is not None


def plan_new_atom(
    issues: tuple[IssueRecord, ...], spec: NewAtomSpec
) -> tuple[NewAtomPlan, tuple[IssueRecord, ...]]:
    """Validate a complete new-Atom contract and its causal placement without writing."""
    _require_known_blocked_by(issues)
    records = {issue.number: issue for issue in issues}
    if spec.type_label not in TYPE_LABELS:
        raise ValueError(f"type label must be one of {sorted(TYPE_LABELS)}")
    if not spec.title.startswith("[") or "] " not in spec.title:
        raise ValueError("title must use the `[Initiative] Title` convention")
    initiative = _markdown_section(spec.body, "Initiative")
    if initiative is None or initiative.strip() != spec.milestone:
        raise ValueError("issue body Initiative must exactly equal the milestone title")
    if dependencies_from_body(spec.body) != spec.blocked_by:
        raise ValueError("issue body Dependencies must exactly equal --blocked-by")
    for heading in ("Closure",):
        if _markdown_section(spec.body, heading) is None:
            raise ValueError(f"issue body is missing `## {heading}`")
    if not _has_heading(spec.body, "Acceptance"):
        raise ValueError("issue body is missing an Acceptance section")
    if not _has_heading(spec.body, "Verification"):
        raise ValueError("issue body is missing a Verification section")
    milestones = {issue.milestone for issue in issues if issue.milestone}
    if spec.milestone not in milestones:
        raise ValueError(f"unknown Initiative milestone {spec.milestone!r}")
    if not 6 <= spec.phase <= 15:
        raise ValueError("phase must be an execution-order section from 6 through 15")
    if not spec.blocks:
        raise ValueError("a new Atom must --block at least one downstream Mission Atom")
    if set(spec.blocked_by).intersection(spec.blocks):
        raise ValueError("the same Atom cannot be both prerequisite and dependent")
    unknown = set(spec.blocked_by).union(spec.blocks) - set(records)
    if unknown:
        raise ValueError(f"new Atom references absent registry identities: {_format_set(unknown)}")
    if spec.target_crate is not None:
        crate_path = f"crates/{spec.target_crate}"
        if crate_path not in spec.body:
            raise ValueError(
                f"issue body must name target crate path `{crate_path}` (G-517)"
            )
    required_node_fields = ("@id", "@type", "name", "summary")
    missing_fields = [field for field in required_node_fields if not spec.knowledge_node.get(field)]
    if missing_fields:
        raise ValueError("knowledge node omits " + ", ".join(missing_fields))

    provisional = max(records, default=0) + 1
    new_issue = IssueRecord(
        number=provisional,
        title=spec.title,
        state="OPEN",
        blocked_by=spec.blocked_by,
        body=spec.body,
        labels=(spec.type_label,),
        milestone=spec.milestone,
        url=f"https://github.com/kmosoti/gordian/issues/{provisional}",
    )
    updated: list[IssueRecord] = []
    for issue in issues:
        if issue.number in spec.blocks:
            blockers = tuple(sorted((*issue.blocked_by, provisional)))
            updated.append(
                replace(
                    issue,
                    blocked_by=blockers,
                    body=replace_body_dependencies(issue.body, blockers),
                )
            )
        else:
            updated.append(issue)
    updated.append(new_issue)
    proposed = tuple(sorted(updated, key=lambda issue: issue.number))
    topological_order(normalise_graph({issue.number: issue.blocked_by for issue in proposed}))
    _, orphans = selfhosting_sets(proposed)
    if provisional in orphans:
        raise ValueError("new Atom is orphaned from #49, #68, and #69 completion paths")
    return (
        NewAtomPlan(
            provisional_number=provisional,
            title=spec.title,
            milestone=spec.milestone,
            type_label=spec.type_label,
            target_crate=spec.target_crate,
            phase=spec.phase,
            blocked_by=spec.blocked_by,
            blocks=spec.blocks,
        ),
        proposed,
    )


def parse_benchmark_obligations(execution_order: str) -> tuple[BenchmarkObligation, ...]:
    """Read the stable performance rows in execution-order section 17."""
    section = _markdown_section(execution_order, "17. Critical performance suite")
    if section is None:
        raise ValueError("execution order has no `## 17. Critical performance suite`")
    rows: list[BenchmarkObligation] = []
    for line in section.splitlines():
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) != 4 or _BENCHMARK_ID.fullmatch(cells[0].strip("`")) is None:
            continue
        owner = _ISSUE_REFERENCE.fullmatch(cells[2])
        if owner is None:
            raise ValueError(f"benchmark row {cells[0]} has invalid owner {cells[2]!r}")
        qualification = cells[3].lower()
        if qualification == "yes":
            first_qualification = True
        elif qualification == "no" or qualification.startswith("no "):
            first_qualification = False
        else:
            raise ValueError(
                f"benchmark row {cells[0]} has invalid qualification value {cells[3]!r}"
            )
        rows.append(
            BenchmarkObligation(
                row_id=cells[0].strip("`"),
                owner=int(owner.group(1)),
                first_qualification=first_qualification,
            )
        )
    if not rows:
        raise ValueError("critical performance suite contains no EO17 rows")
    return tuple(rows)


def _benchmark_ids(section: str | None) -> tuple[str, ...]:
    return tuple(sorted(set(_BENCHMARK_ID.findall(section or ""))))


def audit_benchmark_obligations(
    issues: tuple[IssueRecord, ...],
    execution_order: str,
    *,
    qualification_atom: int = DEFAULT_TARGET,
) -> BenchmarkAuditReport:
    """Join EO17 rows to exact owner sections and #69's native closure."""
    rows = parse_benchmark_obligations(execution_order)
    problems: list[str] = []
    records = {issue.number: issue for issue in issues}
    counts = Counter(row.row_id for row in rows)
    for row_id, count in sorted(counts.items()):
        if count != 1:
            problems.append(f"benchmark row id {row_id} occurs {count} times")

    owned: dict[int, set[str]] = defaultdict(set)
    first_ids: set[str] = set()
    for row in rows:
        owned[row.owner].add(row.row_id)
        if row.first_qualification:
            first_ids.add(row.row_id)
        if row.owner not in records:
            problems.append(f"{row.row_id}: owner #{row.owner} is absent from the registry")

    graph = normalise_graph({issue.number: issue.blocked_by for issue in issues})
    if qualification_atom not in records:
        problems.append(f"qualification Atom #{qualification_atom} is absent from the registry")
        closure: set[int] = set()
    else:
        closure = set(transitive_blockers(graph, qualification_atom))
    for row in rows:
        if row.first_qualification and row.owner not in closure:
            problems.append(
                f"{row.row_id}: first-qualification owner #{row.owner} is outside "
                f"closure(#{qualification_atom})"
            )

    for owner, expected in sorted(owned.items()):
        issue = records.get(owner)
        if issue is None:
            continue
        section = _markdown_section(issue.body, "Benchmark obligation")
        if section is None:
            problems.append(f"#{owner}: missing `## Benchmark obligation` section")
            actual: set[str] = set()
        else:
            actual = set(_benchmark_ids(section))
        missing = expected - actual
        extra = actual - expected
        if missing:
            problems.append(f"#{owner}: benchmark section omits {', '.join(sorted(missing))}")
        if extra:
            problems.append(
                f"#{owner}: benchmark section names unowned {', '.join(sorted(extra))}"
            )

    for issue in sorted(issues, key=lambda record: record.number):
        section = _markdown_section(issue.body, "Benchmark obligation")
        if section is None:
            if BENCHMARK_OWNER_BEGIN in issue.body or BENCHMARK_OWNER_END in issue.body:
                problems.append(
                    f"#{issue.number}: generated EO17 ownership marker is outside "
                    "`## Benchmark obligation`"
                )
            continue
        try:
            generated = _extract_generated_block(
                section, BENCHMARK_OWNER_BEGIN, BENCHMARK_OWNER_END
            )
        except ValueError as error:
            problems.append(f"#{issue.number}: benchmark ownership block: {error}")
            continue
        if generated is not None and issue.number not in owned:
            problems.append(
                f"#{issue.number}: stale generated EO17 ownership block; issue owns no rows"
            )

    qualification = records.get(qualification_atom)
    cited = set(
        _benchmark_ids(
            _markdown_section(qualification.body, "Performance acceptance")
            if qualification is not None
            else None
        )
    )
    missing_gate = first_ids - cited
    extra_gate = cited - first_ids
    if missing_gate:
        problems.append(
            f"#{qualification_atom}: Performance acceptance omits "
            + ", ".join(sorted(missing_gate))
        )
    if extra_gate:
        problems.append(
            f"#{qualification_atom}: Performance acceptance names non-gate "
            + ", ".join(sorted(extra_gate))
        )

    return BenchmarkAuditReport(
        row_count=len(rows),
        first_qualification_ids=tuple(sorted(first_ids)),
        owners=tuple(sorted(owned)),
        problems=tuple(problems),
    )


def _generated_block(begin: str, end: str, label: str, ids: set[str]) -> str:
    joined = ", ".join(f"`{row_id}`" for row_id in sorted(ids))
    return f"{begin}\n{label}: {joined}.\n{end}"


def _extract_generated_block(section: str, begin: str, end: str) -> str | None:
    """Return one generated marker block, rejecting malformed or repeated markers."""
    begin_count = section.count(begin)
    end_count = section.count(end)
    if begin_count != end_count:
        raise ValueError("incomplete generated marker pair")
    if begin_count == 0:
        return None
    if begin_count != 1:
        raise ValueError("repeated generated marker pair")
    start = section.index(begin)
    stop = section.find(end, start)
    if stop < 0:
        raise ValueError("generated marker end precedes begin")
    return section[start : stop + len(end)]


def _remove_generated_section_block(
    body: str,
    *,
    heading: str,
    begin: str,
    end: str,
    remove_empty_section: bool,
) -> str:
    """Remove one generated block while retaining human prose around it.

    A section that becomes whitespace-only is removed when requested.  This keeps a
    stale generated-only section from surviving a plan reassignment, while a section
    containing human text remains in place.
    """
    section_match = re.search(rf"^## {re.escape(heading)}[ \t]*$", body, re.MULTILINE)
    if section_match is None:
        if begin in body or end in body:
            raise ValueError(f"generated {heading!r} marker is outside its section")
        return body

    following = body[section_match.end() :]
    next_heading = _NEXT_HEADING.search(following)
    section_end = (
        section_match.end() + next_heading.start() if next_heading else len(body)
    )
    section = body[section_match.end() : section_end]
    block = _extract_generated_block(section, begin, end)
    if block is None:
        return body

    start = section.index(begin)
    stop = section.index(end, start) + len(end)
    prefix, suffix = section[:start], section[stop:]
    # Markers are rendered on their own lines.  Remove only their surrounding
    # separator whitespace so unrelated prose and intentional spacing survive.
    if prefix.endswith("\n\n") and suffix.startswith("\n\n"):
        prefix = prefix[:-1]
        suffix = suffix[1:]
    elif prefix.endswith("\n") and suffix.startswith("\n"):
        suffix = suffix[1:]
    updated_section = prefix + suffix
    if remove_empty_section and not updated_section.strip():
        return body[: section_match.start()] + body[section_end:]
    return body[: section_match.end()] + updated_section + body[section_end:]


def _upsert_section_block(
    body: str,
    *,
    heading: str,
    begin: str,
    end: str,
    block: str,
) -> str:
    section_match = re.search(rf"^## {re.escape(heading)}[ \t]*$", body, re.MULTILINE)
    if section_match is None:
        closure = re.search(r"^## Closure[ \t]*$", body, re.MULTILINE)
        insertion = f"## {heading}\n\n{block}\n\n"
        if closure is None:
            return body.rstrip() + "\n\n" + insertion
        return body[: closure.start()] + insertion + body[closure.start() :]

    following = body[section_match.end() :]
    next_heading = _NEXT_HEADING.search(following)
    section_end = (
        section_match.end() + next_heading.start() if next_heading else len(body)
    )
    section = body[section_match.end() : section_end]
    if section.count(begin) != section.count(end):
        raise ValueError(f"#{heading}: incomplete generated marker pair")
    if begin in section:
        if section.count(begin) != 1:
            raise ValueError(f"#{heading}: repeated generated marker pair")
        prefix, residue = section.split(begin, 1)
        _, suffix = residue.split(end, 1)
        updated = prefix + block + suffix
    else:
        updated = "\n\n" + block + "\n" + section.lstrip("\n")
    return body[: section_match.end()] + updated + body[section_end:]


def render_benchmark_bodies(
    issues: tuple[IssueRecord, ...],
    execution_order: str,
    *,
    qualification_atom: int = DEFAULT_TARGET,
) -> dict[int, str]:
    """Return deterministic body updates that establish every EO17 join key."""
    rows = parse_benchmark_obligations(execution_order)
    owned: dict[int, set[str]] = defaultdict(set)
    first_ids: set[str] = set()
    for row in rows:
        owned[row.owner].add(row.row_id)
        if row.first_qualification:
            first_ids.add(row.row_id)
    records = {issue.number: issue for issue in issues}
    missing = set(owned).union((qualification_atom,)) - set(records)
    if missing:
        raise ValueError(f"benchmark body owners are absent: {_format_set(missing)}")

    updates: dict[int, str] = {}
    for issue in sorted(issues, key=lambda record: record.number):
        ids = owned.get(issue.number)
        if ids:
            updated = _upsert_section_block(
                issue.body,
                heading="Benchmark obligation",
                begin=BENCHMARK_OWNER_BEGIN,
                end=BENCHMARK_OWNER_END,
                block=_generated_block(
                    BENCHMARK_OWNER_BEGIN,
                    BENCHMARK_OWNER_END,
                    "Owned critical-performance rows",
                    ids,
                ),
            )
            # Keep the historical renderer contract: every current owner appears in
            # the update map even when its body is already canonical.  Callers still
            # compare bytes before issuing an external edit.
            updates[issue.number] = updated
        else:
            updated = _remove_generated_section_block(
                issue.body,
                heading="Benchmark obligation",
                begin=BENCHMARK_OWNER_BEGIN,
                end=BENCHMARK_OWNER_END,
                remove_empty_section=True,
            )
            updates[issue.number] = updated
    qualification_body = updates.get(qualification_atom, records[qualification_atom].body)
    updates[qualification_atom] = _upsert_section_block(
        qualification_body,
        heading="Performance acceptance",
        begin=BENCHMARK_GATE_BEGIN,
        end=BENCHMARK_GATE_END,
        block=_generated_block(
            BENCHMARK_GATE_BEGIN,
            BENCHMARK_GATE_END,
            "Required first-qualification rows",
            first_ids,
        ),
    )
    return updates


def _markdown_row(line: str) -> list[str]:
    """Split one pipe-delimited Markdown row without accepting ragged cells."""
    if not line.lstrip().startswith("|"):
        return []
    return [cell.strip() for cell in line.strip().strip("|").split("|")]


def _parse_crate_map_rows(text: str) -> tuple[CrateMapRow, ...]:
    """Parse and strictly validate the normative crate-map ownership table."""
    lines = text.splitlines()
    header_index: int | None = None
    expected_header = ("Crate", "Path", "May depend on (complete list)", "Owning Atoms")
    for index, line in enumerate(lines):
        if tuple(_markdown_row(line)) == expected_header:
            header_index = index
            break
    if header_index is None:
        raise ValueError("crate map has no canonical ownership table header")

    separator_index = header_index + 1
    if separator_index >= len(lines):
        raise ValueError("crate map ownership table has no separator row")
    separator = _markdown_row(lines[separator_index])
    if len(separator) != 4 or any(not re.fullmatch(r":?-{3,}:?", cell) for cell in separator):
        raise ValueError("crate map ownership table has an invalid separator row")

    rows: list[CrateMapRow] = []
    seen_crates: set[str] = set()
    seen_paths: set[str] = set()
    started = False
    for line in lines[separator_index + 1 :]:
        if not line.strip():
            if started:
                continue
            continue
        cells = _markdown_row(line)
        if not cells:
            break
        started = True
        if len(cells) != 4:
            raise ValueError(f"crate map row has {len(cells)} cells, expected 4: {line!r}")

        crate_match = re.fullmatch(rf"`({_CRATE_NAME})`", cells[0])
        path_match = re.fullmatch(rf"`(crates/({_CRATE_NAME}))`", cells[1])
        if crate_match is None or path_match is None:
            raise ValueError(f"crate map row has malformed crate or path: {line!r}")
        crate = crate_match.group(1)
        path = path_match.group(1)
        if path_match.group(2) != crate:
            raise ValueError(
                f"crate map row path {path!r} does not match crate {crate!r}"
            )
        if crate in seen_crates:
            raise ValueError(f"crate map repeats crate {crate!r}")
        if path in seen_paths:
            raise ValueError(f"crate map repeats path {path!r}")

        owner_cell = cells[3]
        owner_matches = list(
            re.finditer(r"#(\d+)(?:[ \t]*\([^()\n]*\))?", owner_cell)
        )
        if not owner_matches:
            raise ValueError(f"crate map row {crate!r} has an empty owning Atom set")
        residue = owner_cell
        for match in reversed(owner_matches):
            residue = residue[: match.start()] + residue[match.end() :]
        if re.sub(r"[\s,;]+", "", residue):
            raise ValueError(
                f"crate map row {crate!r} has malformed owning Atoms {owner_cell!r}"
            )
        owners = tuple(int(match.group(1)) for match in owner_matches)
        if any(owner <= 0 for owner in owners):
            raise ValueError(f"crate map row {crate!r} has a non-positive Atom identity")
        if len(set(owners)) != len(owners):
            raise ValueError(f"crate map row {crate!r} repeats an owning Atom")

        rows.append(CrateMapRow(crate=crate, path=path, owners=tuple(sorted(owners))))
        seen_crates.add(crate)
        seen_paths.add(path)

    if not rows:
        raise ValueError("crate map ownership table has no crate rows")
    return tuple(rows)


def parse_crate_map(text: str) -> dict[int, tuple[str, ...]]:
    """Return the strict Atom-to-Rust-target reverse index from ``crate-map.md``.

    Multiple crate rows may name one Atom (for example the source-adapter trait
    and its backend realization), but an Atom is never repeated within one row.
    """
    rows = _parse_crate_map_rows(text)
    targets: dict[int, set[str]] = defaultdict(set)
    for row in rows:
        for owner in row.owners:
            targets[owner].add(row.crate)
    return {
        number: tuple(sorted(crates)) for number, crates in sorted(targets.items())
    }


def _crate_map_targets(text: str) -> tuple[dict[int, tuple[str, ...]], frozenset[str]]:
    rows = _parse_crate_map_rows(text)
    targets: dict[int, set[str]] = defaultdict(set)
    crates: set[str] = set()
    for row in rows:
        crates.add(row.crate)
        for owner in row.owners:
            targets[owner].add(row.crate)
    return (
        {number: tuple(sorted(names)) for number, names in sorted(targets.items())},
        frozenset(crates),
    )


def _target_crates(row: PlanRow) -> tuple[str, ...]:
    """Extract a strict, deterministically ordered target set from a plan cell."""
    cell = row.target_crate.strip()
    if cell == "—":
        return ()
    crate_matches = list(re.finditer(r"`([^`]*)`", cell))
    if not crate_matches:
        raise ValueError(f"#{row.number}: unreadable target crate {row.target_crate!r}")
    crates = tuple(match.group(1) for match in crate_matches)
    if any(_CRATE_NAME_RE.fullmatch(crate) is None for crate in crates):
        raise ValueError(f"#{row.number}: unreadable target crate {row.target_crate!r}")
    residue = cell
    for match in reversed(crate_matches):
        residue = residue[: match.start()] + residue[match.end() :]
    residue = re.sub(r"\([^()\n]*\)", "", residue)
    if re.sub(r"[\s,;]+", "", residue):
        raise ValueError(f"#{row.number}: unreadable target crate {row.target_crate!r}")
    if len(set(crates)) != len(crates):
        raise ValueError(f"#{row.number}: target crate cell repeats a crate")
    return tuple(sorted(crates))


def _project_plan_targets(
    rows: dict[int, PlanRow],
    crate_targets: dict[int, tuple[str, ...]],
) -> tuple[str, ...]:
    """Report every disagreement between the derived plan view and crate map."""
    problems: list[str] = []
    for number, row in sorted(rows.items()):
        actual = _target_crates(row)
        expected = crate_targets.get(number, ())
        if actual != expected:
            problems.append(
                f"#{number}: project-plan target crates {list(actual)} != crate-map "
                f"ownership {list(expected)}"
            )
    for number, expected in sorted(crate_targets.items()):
        if number not in rows:
            problems.append(
                f"#{number}: crate-map owner is absent from the project-plan registry"
            )
        elif not expected:
            problems.append(f"#{number}: crate-map owner has an empty target set")
    return tuple(problems)


def _target_reference_exceptions(number: int, known_crates: frozenset[str]) -> frozenset[str]:
    exceptions = SHARED_TEST_TARGET_EXCEPTIONS.get(number, frozenset())
    unknown = exceptions - known_crates
    if unknown:
        raise ValueError(
            f"#{number}: shared-test target exception names unknown crate(s): "
            + ", ".join(sorted(unknown))
        )
    return exceptions


def _body_target_references(body: str) -> tuple[set[str], set[str]]:
    """Return crate-root paths and ``cargo -p`` package names cited by a body."""
    paths = set(_CRATE_PATH_RE.findall(body))
    packages = set(_CARGO_PACKAGE_RE.findall(body))
    return paths, packages


def _shared_test_reference_allowed(body: str, crate: str) -> bool:
    """Require an exception reference to occur in an explicit test context."""
    context = re.compile(
        r"\b(?:test|tests|testing|conformance|fixture|comparison)\w*\b",
        re.IGNORECASE,
    )
    for line in body.splitlines():
        if crate not in line:
            continue
        if context.search(line):
            return True
    return False


def _audit_body_target_references(
    issue: IssueRecord,
    *,
    expected: tuple[str, ...],
    known_crates: frozenset[str],
    problems: list[str],
) -> None:
    paths, packages = _body_target_references(issue.body)
    exceptions = _target_reference_exceptions(issue.number, known_crates)
    for kind, references in (("path", paths), ("cargo package", packages)):
        for crate in sorted(references):
            if crate not in known_crates:
                problems.append(
                    f"#{issue.number}: issue body references unknown {kind} "
                    f"{crate!r}; no crate-map alias is permitted"
                )
            elif crate not in expected and (
                crate not in exceptions
                or not _shared_test_reference_allowed(issue.body, crate)
            ):
                problems.append(
                    f"#{issue.number}: issue body references {kind} {crate!r}, "
                    f"but crate-map ownership is {list(expected)}"
                )


def _target_crate_block(crates: tuple[str, ...]) -> str:
    """Render the canonical generated target block for one ownership set."""
    paths = tuple(f"crates/{crate}" for crate in crates)
    noun = "target" if len(paths) == 1 else "targets"
    rendered_paths = ", ".join(f"`{path}`" for path in paths)
    if len(paths) > 1:
        rendered_paths = rendered_paths.rsplit(", ", 1)
        rendered_paths = " and ".join(rendered_paths)
    return (
        f"{TARGET_CRATE_BEGIN}\n"
        f"Rust {noun}: {rendered_paths}.\n"
        f"{TARGET_CRATE_END}"
    )


def audit_target_crate_contracts(
    issues: tuple[IssueRecord, ...], project_plan: str, crate_map: str | None = None
) -> TargetCrateAuditReport:
    """Check crate-map ownership, plan cells, issue target blocks, and body references.

    ``crate_map`` is required by all command paths.  The optional legacy form keeps
    direct callers from older bootstrap tests source-compatible; command/audit
    orchestration always supplies the normative map.
    """
    rows = parse_project_plan(project_plan)
    if crate_map is None:
        # Compatibility for callers written before the crate-map contract existed.
        # The registry command never takes this path.
        crate_targets = {number: _target_crates(row) for number, row in rows.items()}
        known_crates = frozenset(crate for names in crate_targets.values() for crate in names)
    else:
        crate_targets, known_crates = _crate_map_targets(crate_map)
    records = {issue.number: issue for issue in issues}
    problems: list[str] = []
    if crate_map is not None:
        problems.extend(_project_plan_targets(rows, crate_targets))
    owners = sum(bool(crates) for crates in crate_targets.values())

    for number, _row in sorted(rows.items()):
        crates = crate_targets.get(number, ())
        issue = records.get(number)
        if issue is None:
            if crates:
                problems.append(f"#{number}: target-crate owner is absent from the registry")
            continue
        section = _markdown_section(issue.body, "Target crate")
        actual = {
            f"crates/{crate}"
            for crate in _body_target_references(section or "")[0]
        }
        expected = {f"crates/{crate}" for crate in crates}
        if section is None and expected:
            problems.append(f"#{number}: missing `## Target crate` section")
        if section is not None:
            if (
                issue.body.count(TARGET_CRATE_BEGIN) != section.count(TARGET_CRATE_BEGIN)
                or issue.body.count(TARGET_CRATE_END) != section.count(TARGET_CRATE_END)
            ):
                problems.append(
                    f"#{number}: generated target-crate marker is outside `## Target crate`"
                )
            try:
                generated = _extract_generated_block(
                    section, TARGET_CRATE_BEGIN, TARGET_CRATE_END
                )
            except ValueError as error:
                problems.append(f"#{number}: target-crate block: {error}")
            else:
                if generated is not None and not expected:
                    problems.append(
                        f"#{number}: stale generated target-crate block; "
                        "crate map has no target crate"
                    )
                elif crate_map is not None and expected and generated is None:
                    problems.append(
                        f"#{number}: missing generated target-crate block in `## Target crate`"
                    )
                elif crate_map is not None and generated != _target_crate_block(crates):
                    problems.append(
                        f"#{number}: generated target-crate block is stale; "
                        "run sync-target-crates"
                    )
        elif TARGET_CRATE_BEGIN in issue.body or TARGET_CRATE_END in issue.body:
            problems.append(
                f"#{number}: generated target-crate marker is outside `## Target crate`"
            )
        if actual != expected:
            problems.append(
                f"#{number}: target-crate section names {sorted(actual)}, "
                f"expected {sorted(expected)}"
            )
        _audit_body_target_references(
            issue,
            expected=crates,
            known_crates=known_crates,
            problems=problems,
        )

    for number, _crates in sorted(crate_targets.items()):
        if number not in records:
            problems.append(f"#{number}: crate-map owner is absent from the registry")

    # A registry issue not represented by a plan row must not smuggle a generated
    # target block or a crate reference into the executable body.
    for issue in sorted(issues, key=lambda record: record.number):
        if issue.number in rows:
            continue
        paths, packages = _body_target_references(issue.body)
        if paths or packages or TARGET_CRATE_BEGIN in issue.body or TARGET_CRATE_END in issue.body:
            problems.append(
                f"#{issue.number}: issue body carries target-crate references but no plan row"
            )
    return TargetCrateAuditReport(owner_count=owners, problems=tuple(problems))


def render_target_crate_bodies(
    issues: tuple[IssueRecord, ...], project_plan: str, crate_map: str | None = None
) -> dict[int, str]:
    """Render deterministic issue blocks from crate-map ownership.

    Empty ownership removes only the generated block/section, preserving human
    rationale.  A supplied crate map is authoritative and is validated against
    the project-plan target cells before any body update is returned.
    """
    rows = parse_project_plan(project_plan)
    if crate_map is None:
        crate_targets = {number: _target_crates(row) for number, row in rows.items()}
    else:
        crate_targets, _ = _crate_map_targets(crate_map)
        disagreements = _project_plan_targets(rows, crate_targets)
        if disagreements:
            raise ValueError(
                "project-plan/crate-map target disagreement: " + "; ".join(disagreements)
            )
    records = {issue.number: issue for issue in issues}
    for number, crates in sorted(crate_targets.items()):
        if number not in records and crates:
            raise ValueError(f"#{number}: target-crate owner is absent from the registry")

    updates: dict[int, str] = {}
    for issue in sorted(issues, key=lambda record: record.number):
        crates = crate_targets.get(issue.number, ())
        if not crates:
            updated = _remove_generated_section_block(
                issue.body,
                heading="Target crate",
                begin=TARGET_CRATE_BEGIN,
                end=TARGET_CRATE_END,
                remove_empty_section=True,
            )
        else:
            updated = _upsert_section_block(
                issue.body,
                heading="Target crate",
                begin=TARGET_CRATE_BEGIN,
                end=TARGET_CRATE_END,
                block=_target_crate_block(crates),
            )
        updates[issue.number] = updated
    return updates


_VERIFIER_ID_RE = re.compile(r"`verifier:([a-z0-9][a-z0-9._-]*)`")
_DECLARED_VERIFIER_ID_RE = re.compile(
    r"(?:verifier_id|verifier)[ \t]*[:=][ \t]*`?([a-z0-9][a-z0-9._-]*)`?",
    re.IGNORECASE,
)
_COMMAND_IN_CLOSURE_RE = re.compile(
    r"(?:\bcargo(?:[ \t]|$)|\bpython(?:3(?:\.\d+)?)?(?:[ \t]|$)|"
    r"\bruff[ \t]+check\b|\bshellcheck(?:[ \t]|$)|"
    r"\bbash[ \t]+scripts/check-|\bfor[ \t]+s[ \t]+in[ \t]+scripts/check-)",
    re.IGNORECASE,
)
_ARTIFACT_PATH_RE = re.compile(r"`(artifacts/[^`]+)`")
_CANONICAL_VERIFIER_PATH_RE = re.compile(
    r"artifacts/atoms/(\d+)/verifiers/([a-z0-9][a-z0-9._-]*)\.log"
)


def _section_instances(body: str, heading: str) -> tuple[str, ...]:
    """Return the text of every level-two Markdown section with ``heading``."""
    matches = list(re.finditer(rf"^## {re.escape(heading)}[ \t]*$", body, re.MULTILINE))
    sections: list[str] = []
    for match in matches:
        following = body[match.end() :]
        next_heading = _NEXT_HEADING.search(following)
        sections.append(following[: next_heading.start()] if next_heading else following)
    return tuple(sections)


def _closure_verifier_ids(section: str) -> tuple[str, ...]:
    """Extract explicitly parseable verifier identities from a Closure section."""
    return tuple(_VERIFIER_ID_RE.findall(section))


def _declared_verifier_ids(section: str) -> tuple[str, ...]:
    """Extract optional explicit verifier identities from an Atom Verification section."""
    return tuple(_DECLARED_VERIFIER_ID_RE.findall(section))


def audit_closure_contracts(
    issues: Sequence[IssueRecord],
    *,
    required_numbers: Sequence[int] = _CANONICAL_ATOM_NUMBERS,
) -> ContractLintReport:
    """Validate the normalized one-section Closure contract for each Atom.

    This checker intentionally treats the issue's ``## Verification`` prose as
    the only source of Atom-specific verifier IDs.  It rejects duplicate IDs and
    old command/path forms instead of trying to infer a verifier from an
    arbitrary command string.
    """
    required = set(int(number) for number in required_numbers)
    rows = tuple(sorted(issues, key=lambda issue: issue.number))
    problems: list[str] = []
    seen_numbers = {issue.number for issue in rows}
    for number in sorted(required - seen_numbers):
        problems.append(f"#{number}: missing issue for Closure contract audit")

    for issue in rows:
        if issue.number not in required:
            continue
        closure_sections = _section_instances(issue.body, "Closure")
        if len(closure_sections) != 1:
            problems.append(
                f"#{issue.number}: expected exactly one `## Closure` section, found "
                f"{len(closure_sections)}"
            )
            continue
        closure = closure_sections[0]
        verification = _section_instances(issue.body, "Verification")
        verification_text = verification[0] if verification else ""
        closure_ids = _closure_verifier_ids(closure)
        duplicate_ids = sorted(
            verifier for verifier, count in Counter(closure_ids).items() if count > 1
        )
        if duplicate_ids:
            problems.append(
                f"#{issue.number}: duplicate verifier IDs in Closure: "
                + ", ".join(duplicate_ids)
            )

        expected_ids = (*_INTEGRATION_VERIFIER_IDS, *_declared_verifier_ids(verification_text))
        expected_counts = Counter(expected_ids)
        actual_counts = Counter(closure_ids)
        if actual_counts != expected_counts:
            problems.append(
                f"#{issue.number}: Closure verifier IDs {list(closure_ids)} != "
                f"five integration IDs plus Verification declarations {list(expected_ids)}"
            )
        if "sections 1, 2, and 6.6" not in closure:
            problems.append(
                f"#{issue.number}: Closure must reference runbook sections 1, 2, and 6.6"
            )
        if "MUST carry exactly" in closure:
            problems.append(f"#{issue.number}: Closure retains forbidden `MUST carry exactly`")
        if _COMMAND_IN_CLOSURE_RE.search(closure):
            problems.append(
                f"#{issue.number}: Closure restates a generic integration command"
            )

        for path in _ARTIFACT_PATH_RE.findall(closure):
            if path == "artifacts/schema/closure-record.schema.json":
                continue
            if path.endswith("/closure.json"):
                if path != f"artifacts/atoms/{issue.number}/closure.json":
                    problems.append(
                        f"#{issue.number}: noncanonical closure record path {path!r}"
                    )
                continue
            match = _CANONICAL_VERIFIER_PATH_RE.fullmatch(path)
            if match is None or int(match.group(1)) != issue.number:
                problems.append(
                    f"#{issue.number}: noncanonical verifier artifact path {path!r}"
                )

        expected_closure = _canonical_closure_wording(
            issue.number, _declared_verifier_ids(verification_text)
        )
        # The section helper returns content including the newline after its
        # heading.  Compare a normalized form so only section boundaries, not
        # an issue's surrounding prose, are significant.
        if (
            closure.strip("\n") != "\n" + expected_closure + "\n"
            and closure.strip() != expected_closure
        ):
            problems.append(
                f"#{issue.number}: Closure wording is not the canonical manifest form"
            )

    return ContractLintReport(issue_count=len(rows), problems=tuple(problems))


def audit_normalization_manifest(
    manifest: _NormalizationManifest,
    crate_map: str,
    *,
    required_numbers: Sequence[int] = _CANONICAL_ATOM_NUMBERS,
) -> ContractLintReport:
    """Check manifest coverage and target/Closure intent against canonical sources.

    Target ownership is derived from the normative crate-map table on every run;
    this function deliberately has no embedded Atom-to-crate mapping.
    """
    problems: list[str] = []
    required = tuple(sorted({int(number) for number in required_numbers}))
    try:
        _validate_manifest_coverage(manifest, required)
    except (ValueError, _NormalizationError) as error:
        problems.append(str(error))
    transforms = {transform.issue: transform for transform in manifest.transforms}
    crate_targets, known_crates = _crate_map_targets(crate_map)
    for number in required:
        transform = transforms.get(number)
        if transform is None:
            continue
        if not transform.target_crates_set:
            problems.append(f"#{number}: manifest omits explicit target-crate intent")
            continue
        expected = crate_targets.get(number, ())
        actual = tuple(sorted(transform.target_crates or ()))
        if actual != expected:
            problems.append(
                f"#{number}: manifest target crates {list(actual)} != crate-map ownership "
                f"{list(expected)}"
            )
        unknown = set(actual) - known_crates
        if unknown:
            problems.append(
                f"#{number}: manifest target intent names unknown crate(s): "
                + ", ".join(sorted(unknown))
            )
        if not transform.closure_wording_set:
            problems.append(f"#{number}: manifest omits explicit Closure wording")
            continue
        if transform.closure_template == _CANONICAL_CLOSURE_TEMPLATE:
            # The compact manifest form is intentionally not a rendered
            # Closure.  Its parser has already checked every intent field;
            # plan construction renders the canonical wording from the issue's
            # Verification section.
            expected_intent = _canonical_closure_intent(
                (f"atom-{number}-acceptance",)
            )
            if transform.as_json_object().get("closure_wording") != expected_intent:
                problems.append(f"#{number}: manifest Closure intent is not canonical")
            continue
        closure = transform.closure_wording or ""
        closure_ids = _closure_verifier_ids(closure)
        if Counter(closure_ids) != Counter(_INTEGRATION_VERIFIER_IDS):
            problems.append(
                f"#{number}: manifest Closure must carry exactly the five integration verifier IDs"
            )
        expected_closure = _canonical_closure_wording(number)
        if closure != expected_closure:
            problems.append(f"#{number}: manifest Closure wording is not canonical")
    return ContractLintReport(issue_count=len(required), problems=tuple(problems))


def parse_project_plan(text: str) -> dict[int, PlanRow]:
    """Read the derived four-column Atom tables from ``project-plan.md``."""
    rows: dict[int, PlanRow] = {}
    initiative = ""
    for line in text.splitlines():
        if line.startswith("# Initiative: "):
            initiative = line.removeprefix("# Initiative: ").strip()
            continue
        if not line.startswith("| #"):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) != 4 or re.fullmatch(r"#\d+", cells[0]) is None:
            continue
        number = int(cells[0][1:])
        if number in rows:
            raise ValueError(f"project plan repeats Atom #{number}")
        blocked_by = () if cells[3] == "—" else tuple(
            sorted({int(value) for value in _ISSUE_REFERENCE.findall(cells[3])})
        )
        if cells[3] != "—" and not blocked_by:
            raise ValueError(f"project plan Atom #{number} has an unreadable blocker cell")
        if not initiative:
            raise ValueError(f"project plan Atom #{number} has no Initiative heading")
        rows[number] = PlanRow(
            number=number,
            title=cells[1],
            target_crate=cells[2],
            blocked_by=blocked_by,
            initiative=initiative,
        )
    if not rows:
        raise ValueError("project plan contains no Atom rows")
    return dict(sorted(rows.items()))


def render_project_plan(
    text: str, issues: tuple[IssueRecord, ...], crate_map: str | None = None
) -> str:
    """Regenerate rows while validating target cells against the normative crate map."""
    rows = parse_project_plan(text)
    records = {issue.number: issue for issue in issues}
    if set(rows) != set(records):
        missing = set(records) - set(rows)
        extra = set(rows) - set(records)
        raise ValueError(
            "cannot render plan with a changed registry; missing rows "
            f"{_format_set(missing)}, extra rows {_format_set(extra)}"
        )
    crate_targets: dict[int, tuple[str, ...]] | None = None
    if crate_map is not None:
        crate_targets, _ = _crate_map_targets(crate_map)
        disagreements = _project_plan_targets(rows, crate_targets)
        if disagreements:
            raise ValueError(
                "project-plan/crate-map target disagreement: "
                + "; ".join(disagreements)
            )
    rendered: list[str] = []
    for line in text.splitlines(keepends=True):
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) != 4 or re.fullmatch(r"#\d+", cells[0]) is None:
            rendered.append(line)
            continue
        number = int(cells[0][1:])
        issue = records[number]
        row = rows[number]
        if issue.milestone != row.initiative:
            raise ValueError(
                f"#{number}: plan Initiative {row.initiative!r} != milestone "
                f"{issue.milestone!r}; move the row explicitly"
            )
        if crate_targets is not None:
            # `_project_plan_targets` above validates every row, including this
            # one; keep this lookup here as a guard against future row filters.
            _ = crate_targets.get(number, ())
        blockers = "—" if not issue.blocked_by else ", ".join(
            f"#{blocker}" for blocker in issue.blocked_by
        )
        newline = "\n" if line.endswith("\n") else ""
        rendered.append(
            f"| #{number} | {issue.title} | {row.target_crate} | {blockers} |{newline}"
        )
    return "".join(rendered)


def insert_project_plan_atom(
    text: str, issue: IssueRecord, target_crate: str | None
) -> str:
    """Insert one explicit row into its milestone's existing Atom table."""
    if issue.number in parse_project_plan(text):
        raise ValueError(f"project plan already carries Atom #{issue.number}")
    if "|" in issue.title or "\n" in issue.title:
        raise ValueError("issue title cannot be represented in the project-plan table")
    heading = re.search(
        rf"^# Initiative: {re.escape(issue.milestone or '')}[ \t]*$",
        text,
        re.MULTILINE,
    )
    if heading is None:
        raise ValueError(f"project plan has no Initiative {issue.milestone!r}")
    following = text[heading.end() :]
    next_heading = re.search(r"^# Initiative: ", following, re.MULTILINE)
    end = heading.end() + next_heading.start() if next_heading else len(text)
    section = text[heading.end() : end]
    rows = list(re.finditer(r"^\| #\d+ .*$", section, re.MULTILINE))
    if not rows:
        raise ValueError(f"Initiative {issue.milestone!r} has no Atom table rows")
    target = "—" if target_crate is None else f"`{target_crate}`"
    blockers = "—" if not issue.blocked_by else ", ".join(
        f"#{number}" for number in issue.blocked_by
    )
    row = f"| #{issue.number} | {issue.title} | {target} | {blockers} |"
    insertion = rows[-1].end()
    updated_section = section[:insertion] + "\n" + row + section[insertion:]
    return text[: heading.end()] + updated_section + text[end:]


def insert_phase_member(text: str, issue: IssueRecord, phase: int) -> str:
    """Register one new Atom in exactly one unordered execution phase."""
    member_occurrences = len(
        re.findall(rf"^- #{issue.number}(?:\s|$)", text, re.MULTILINE)
    )
    if member_occurrences:
        raise ValueError(f"execution order already has #{issue.number} as a phase member")
    heading = re.search(rf"^## {phase}\. .+$", text, re.MULTILINE)
    if heading is None:
        raise ValueError(f"execution order has no phase section {phase}")
    following = text[heading.end() :]
    next_heading = re.search(r"^## \d+\. ", following, re.MULTILINE)
    section_end = heading.end() + next_heading.start() if next_heading else len(text)
    section = text[heading.end() : section_end]
    members = re.search(r"^### Members[ \t]*$", section, re.MULTILINE)
    if members is None:
        raise ValueError(f"execution phase {phase} has no Members list")
    member_tail = section[members.end() :]
    next_subsection = re.search(r"^### ", member_tail, re.MULTILINE)
    list_end = members.end() + (
        next_subsection.start() if next_subsection else len(member_tail)
    )
    list_text = section[members.end() : list_end]
    bullets = list(re.finditer(r"^- #\d+ .*$", list_text, re.MULTILINE))
    if not bullets:
        raise ValueError(f"execution phase {phase} has no member bullets")
    insertion = members.end() + bullets[-1].end()
    title = issue.title.rstrip(".;")
    updated_section = section[:insertion] + f"\n- #{issue.number} {title};" + section[insertion:]
    return text[: heading.end()] + updated_section + text[section_end:]


def add_crate_owner(crate_map: str, crate: str | None, number: int) -> str:
    """Add one Atom owner to a validated crate-map row.

    The map is parsed before mutation, so malformed rows, duplicate owners, and
    unknown crate names cannot be hidden by this projection helper.
    """
    parsed_rows = _parse_crate_map_rows(crate_map)
    if crate is None:
        return crate_map
    matching = tuple(row for row in parsed_rows if row.crate == crate)
    if not matching:
        raise ValueError(f"crate map has no row for {crate!r}")
    if number <= 0:
        raise ValueError("Atom identity must be positive")
    row = re.search(
        rf"^\| `{re.escape(crate)}` \|.*$",
        crate_map,
        re.MULTILINE,
    )
    if row is None:
        raise ValueError(f"crate map has no row for {crate!r}")
    cells = [cell.strip() for cell in row.group(0).strip().strip("|").split("|")]
    if len(cells) != 4:
        raise ValueError(f"crate map row for {crate!r} is unreadable")
    owners = set(matching[0].owners)
    if number in owners:
        raise ValueError(f"crate map already assigns #{number} to {crate}")
    owners.add(number)
    cells[3] = ", ".join(f"#{owner}" for owner in sorted(owners))
    replacement = "| " + " | ".join(cells) + " |"
    return crate_map[: row.start()] + replacement + crate_map[row.end() :]


def append_knowledge_node(
    graph_text: str, node: dict[str, Any], issue_url: str
) -> str:
    """Append one supplied, inspectable node without reformatting the corpus shard."""
    payload = json.loads(graph_text)
    graph = payload.get("@graph") if isinstance(payload, dict) else None
    if not isinstance(graph, list):
        raise ValueError("knowledge shard has no `@graph` array")
    node_id = node.get("@id")
    if any(isinstance(existing, dict) and existing.get("@id") == node_id for existing in graph):
        raise ValueError(f"knowledge graph already carries {node_id!r}")
    enriched = dict(node)
    enriched["url"] = issue_url
    enriched.setdefault("status", "planned")
    enriched.setdefault("relations", [])
    enriched.setdefault("verification", [])
    marker = "\n  ]\n}"
    position = graph_text.rfind(marker)
    if position < 0:
        raise ValueError("knowledge shard has no canonical closing marker")
    compact = json.dumps(enriched, ensure_ascii=False, separators=(",", ":"))
    updated = graph_text[:position].rstrip() + ",\n    " + compact + graph_text[position:]
    json.loads(updated)
    return updated


def render_initiative_register_block(
    issue_index: str, issues: tuple[IssueRecord, ...]
) -> str:
    """Regenerate Initiative membership from each issue's native milestone."""
    if issue_index.count(INITIATIVE_BEGIN) != 1 or issue_index.count(INITIATIVE_END) != 1:
        raise ValueError("issue index must contain exactly one Initiative register marker pair")
    block = issue_index.split(INITIATIVE_BEGIN, 1)[1].split(INITIATIVE_END, 1)[0]
    membership: dict[str, list[int]] = defaultdict(list)
    for issue in issues:
        if not issue.milestone:
            raise ValueError(f"#{issue.number}: cannot register an Atom without a milestone")
        membership[issue.milestone].append(issue.number)

    seen: set[str] = set()
    rendered: list[str] = [INITIATIVE_BEGIN, ""]
    for line in block.strip("\n").splitlines():
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) != 4 or not cells[0].startswith("["):
            rendered.append(line)
            continue
        match = re.match(r"\[([^]]+)\]\([^)]+\)", cells[0])
        if match is None:
            raise ValueError(f"unreadable Initiative cell {cells[0]!r}")
        initiative = match.group(1)
        if initiative not in membership:
            raise ValueError(f"Initiative register carries unknown or empty {initiative!r}")
        seen.add(initiative)
        identities = ", ".join(f"#{number}" for number in sorted(membership[initiative]))
        rendered.append(f"| {cells[0]} | {identities} | {cells[2]} | {cells[3]} |")
    missing = set(membership) - seen
    if missing:
        raise ValueError("Initiative register omits milestones: " + ", ".join(sorted(missing)))
    rendered.extend(("", INITIATIVE_END))
    return "\n".join(rendered)


def render_issue_index(issue_index: str, issues: tuple[IssueRecord, ...]) -> str:
    """Render the generated register and its two human-facing count statements."""
    replacement = render_initiative_register_block(issue_index, issues)
    prefix, residue = issue_index.split(INITIATIVE_BEGIN, 1)
    _, suffix = residue.split(INITIATIVE_END, 1)
    rendered = prefix + replacement + suffix
    rendered = re.sub(
        r"\*\*\d+ registered Atoms\*\*",
        f"**{len(issues)} registered Atoms**",
        rendered,
    )
    rendered = re.sub(
        r"Fourteen Initiatives, \d+ Atoms\.",
        f"Fourteen Initiatives, {len(issues)} Atoms.",
        rendered,
    )
    return rendered


def parse_execution_spine(text: str) -> tuple[tuple[int, ...], tuple[tuple[int, int], ...]]:
    """Read the generated maximum-length path block from ``execution-order.md``."""
    if text.count(SPINE_BEGIN) != 1 or text.count(SPINE_END) != 1:
        raise ValueError("execution order must contain exactly one generated spine marker pair")
    block = text.split(SPINE_BEGIN, 1)[1].split(SPINE_END, 1)[0]
    nodes: set[int] = set()
    edges: set[tuple[int, int]] = set()
    for line in block.splitlines():
        if "->" not in line:
            continue
        numbers = [int(value) for value in _ISSUE_REFERENCE.findall(line)]
        if len(numbers) < 2:
            raise ValueError(f"generated spine line has fewer than two Atoms: {line!r}")
        nodes.update(numbers)
        edges.update(zip(numbers, numbers[1:], strict=False))
    if not edges:
        raise ValueError("generated spine contains no edges")
    return tuple(sorted(nodes)), tuple(sorted(edges))


def maximum_path_spine(issues: tuple[IssueRecord, ...], target: int) -> Spine:
    """Return every node and edge on a maximum-length blocker path to ``target``."""
    graph = normalise_graph({issue.number: issue.blocked_by for issue in issues})
    if target not in graph:
        raise ValueError(f"target Atom #{target} is absent from the registry")
    depth = waves(graph)
    nodes = {target}
    edges: set[tuple[int, int]] = set()
    stack = [target]
    while stack:
        current = stack.pop()
        for blocker in graph[current]:
            if depth[blocker] != depth[current] - 1:
                continue
            edges.add((blocker, current))
            if blocker not in nodes:
                nodes.add(blocker)
                stack.append(blocker)
    return Spine(
        nodes=tuple(sorted(nodes)),
        edges=tuple(sorted(edges)),
        length=depth[target],
    )


def maximum_paths(issues: tuple[IssueRecord, ...], target: int) -> tuple[tuple[int, ...], ...]:
    """Enumerate the current graph's maximum-length blocker paths to ``target``."""
    graph = normalise_graph({issue.number: issue.blocked_by for issue in issues})
    if target not in graph:
        raise ValueError(f"target Atom #{target} is absent from the registry")
    depth = waves(graph)

    @cache
    def paths_to(node: int) -> tuple[tuple[int, ...], ...]:
        parents = tuple(blocker for blocker in graph[node] if depth[blocker] == depth[node] - 1)
        if not parents:
            return ((node,),)
        paths: list[tuple[int, ...]] = []
        for parent in parents:
            paths.extend(path + (node,) for path in paths_to(parent))
        return tuple(paths)

    return tuple(sorted(paths_to(target)))


def render_spine_block(issues: tuple[IssueRecord, ...], target: int) -> str:
    """Render the generated marker block from the single native graph snapshot."""
    paths = maximum_paths(issues, target)
    lines = [SPINE_BEGIN, "", "```text"]
    lines.extend(" -> ".join(f"#{number}" for number in path) for path in paths)
    lines.extend(("```", "", SPINE_END))
    return "\n".join(lines)


def selfhosting_sets(
    issues: tuple[IssueRecord, ...],
) -> tuple[tuple[int, ...], tuple[int, ...]]:
    """Return ``closure(#49)`` and registry identities orphaned from Mission gates."""
    graph = normalise_graph({issue.number: issue.blocked_by for issue in issues})
    required_roots = (49, 68, 69, 70)
    missing = set(required_roots) - set(graph)
    if missing:
        raise ValueError(f"self-hosting roots are absent: {_format_set(missing)}")
    minimal = set(transitive_blockers(graph, 49))
    reachable = (
        minimal
        | set(transitive_blockers(graph, 68))
        | set(transitive_blockers(graph, 69))
        | set(required_roots)
    )
    orphans = set(graph) - reachable
    return tuple(sorted(minimal)), tuple(sorted(orphans))


def _format_number_lines(numbers: tuple[int, ...], width: int = 20) -> list[str]:
    return [
        " ".join(str(number) for number in numbers[offset : offset + width])
        for offset in range(0, len(numbers), width)
    ]


def render_selfhosting_block(issues: tuple[IssueRecord, ...]) -> str:
    """Render the exact bootstrap closure and orphan result from native edges."""
    minimal, orphans = selfhosting_sets(issues)
    lines = [
        SELFHOST_BEGIN,
        "",
        f"`closure(#49)` contains **{len(minimal)} Atoms**:",
        "",
        "```text",
        *_format_number_lines(minimal),
        "```",
        "",
        (
            "Registered Atoms orphaned from `closure(#49)`, `closure(#68)`, and "
            f"`closure(#69)`: {', '.join(f'#{number}' for number in orphans) or 'none'}."
        ),
        "",
        SELFHOST_END,
    ]
    return "\n".join(lines)


def replace_selfhosting_block(text: str, replacement: str) -> str:
    if text.count(SELFHOST_BEGIN) != 1 or text.count(SELFHOST_END) != 1:
        raise ValueError("execution order must contain exactly one self-hosting marker pair")
    prefix, residue = text.split(SELFHOST_BEGIN, 1)
    _, suffix = residue.split(SELFHOST_END, 1)
    return prefix + replacement + suffix


def replace_spine_block(text: str, replacement: str) -> str:
    """Replace exactly one generated spine block while preserving surrounding prose."""
    if text.count(SPINE_BEGIN) != 1 or text.count(SPINE_END) != 1:
        raise ValueError("execution order must contain exactly one generated spine marker pair")
    prefix, residue = text.split(SPINE_BEGIN, 1)
    _, suffix = residue.split(SPINE_END, 1)
    return prefix + replacement + suffix


def _format_set(values: set[int]) -> str:
    return ", ".join(f"#{value}" for value in sorted(values)) or "none"


def audit(
    issues: tuple[IssueRecord, ...],
    *,
    project_plan: str,
    execution_order: str,
    issue_index: str | None = None,
    target: int = DEFAULT_TARGET,
    stamp: provenance.Provenance | None = None,
    crate_map: str | None = None,
) -> AuditReport:
    """Compare every temporary registry mirror with the native issue records."""
    problems: list[str] = []
    records = {issue.number: issue for issue in issues}
    if len(records) != len(issues):
        problems.append("registry repeats an issue number")
    for source, blocker in _unknown_blocked_by(issues):
        problems.append(
            f"#{source}: native blocker #{blocker} is absent from the executable Atom registry"
        )

    for issue in sorted(issues, key=lambda record: record.number):
        try:
            mirrored = dependencies_from_body(issue.body)
        except ValueError as error:
            problems.append(f"#{issue.number}: {error}")
        else:
            if mirrored != issue.blocked_by:
                problems.append(
                    f"#{issue.number}: body dependencies {list(mirrored)} != native "
                    f"blockedBy {list(issue.blocked_by)}"
                )
        type_labels = TYPE_LABELS.intersection(issue.labels)
        if len(type_labels) != 1:
            problems.append(
                f"#{issue.number}: expected exactly one type label, found {sorted(type_labels)}"
            )
        if not issue.milestone:
            problems.append(f"#{issue.number}: no Initiative milestone")

    plan_rows = parse_project_plan(project_plan)
    missing_plan = set(records) - set(plan_rows)
    extra_plan = set(plan_rows) - set(records)
    if missing_plan:
        problems.append(f"project plan omits {_format_set(missing_plan)}")
    if extra_plan:
        problems.append(f"project plan carries non-registry {_format_set(extra_plan)}")
    for number in sorted(set(records).intersection(plan_rows)):
        issue = records[number]
        row = plan_rows[number]
        if row.title != issue.title:
            problems.append(
                f"#{number}: project-plan title {row.title!r} != issue title {issue.title!r}"
            )
        if row.blocked_by != issue.blocked_by:
            problems.append(
                f"#{number}: project-plan blockers {list(row.blocked_by)} != native "
                f"blockedBy {list(issue.blocked_by)}"
            )
        if row.initiative != issue.milestone:
            problems.append(
                f"#{number}: project-plan Initiative {row.initiative!r} != milestone "
                f"{issue.milestone!r}"
            )

    expected_spine = maximum_path_spine(issues, target)
    if re.search(r"^### Order\s*$", execution_order, re.MULTILINE):
        problems.append(
            "execution order contains hand-maintained `### Order` lists; use unordered phase "
            "membership plus the native Ready projection"
        )
    documented_nodes, documented_edges = parse_execution_spine(execution_order)
    if documented_nodes != expected_spine.nodes:
        problems.append(
            "execution spine nodes differ: documented "
            f"{list(documented_nodes)}, derived {list(expected_spine.nodes)}"
        )
    if documented_edges != expected_spine.edges:
        problems.append(
            "execution spine edges differ: documented "
            f"{list(documented_edges)}, derived {list(expected_spine.edges)}"
        )
    if 49 in records:
        expected_selfhosting = render_selfhosting_block(issues)
        try:
            documented_selfhosting = (
                SELFHOST_BEGIN
                + execution_order.split(SELFHOST_BEGIN, 1)[1].split(SELFHOST_END, 1)[0]
                + SELFHOST_END
            )
        except IndexError:
            problems.append("execution order has no generated self-hosting closure block")
        else:
            if documented_selfhosting != expected_selfhosting:
                problems.append("generated self-hosting closure block differs from native graph")
        _, orphans = selfhosting_sets(issues)
        if orphans:
            problems.append(
                "Atoms orphaned from Mission completion: "
                + ", ".join(f"#{number}" for number in orphans)
            )
    if issue_index is not None:
        try:
            rendered_index = render_issue_index(issue_index, issues)
        except ValueError as error:
            problems.append(str(error))
        else:
            if rendered_index != issue_index:
                problems.append("Initiative register membership or counts differ from milestones")
    if crate_map is not None:
        target_report = audit_target_crate_contracts(issues, project_plan, crate_map)
        problems.extend(
            f"target-crate contracts: {problem}" for problem in target_report.problems
        )

    stamp = stamp or provenance.collect()
    return AuditReport(
        issue_count=len(issues),
        target=target,
        maximum_path_length=expected_spine.length,
        spine_nodes=expected_spine.nodes,
        spine_edges=expected_spine.edges,
        problems=tuple(problems),
        generated_at=stamp.generated_at,
        source_change_id=stamp.source_change_id,
        source_commit_id=stamp.source_commit_id,
        tool_versions=stamp.tool_versions,
    )


def snapshot_object(
    issues: tuple[IssueRecord, ...],
    stamp: provenance.Provenance,
    *,
    repository: str = DEFAULT_REPOSITORY,
) -> dict[str, Any]:
    """Encode the complete executable issue contracts and native edges."""
    return {
        "record_format": "gordian-atom-registry-v1",
        **stamp.as_json_object(),
        "source": "GitHub issue bodies and native blockedBy connections",
        # ``load_snapshot`` treats these envelope fields as required authority
        # metadata.  They are explicit even for callers that add ``coherent``
        # after the fact, so every snapshot emitted by this module is directly
        # consumable by the strict offline reader.
        "repository": repository,
        "coherent": True,
        "issues": [
            {
                "number": issue.number,
                "title": issue.title,
                "state": issue.state,
                "body": issue.body,
                "url": issue.url,
                "milestone": issue.milestone,
                "labels": list(issue.labels),
                "blockedBy": list(issue.blocked_by),
                "assignees": list(issue.assignees),
            }
            for issue in sorted(issues, key=lambda record: record.number)
        ],
    }


def _records(arguments: argparse.Namespace) -> tuple[IssueRecord, ...]:
    return _records_with_mode(arguments)


def _records_with_mode(
    arguments: argparse.Namespace, *, require_strict_snapshot: bool = False
) -> tuple[IssueRecord, ...]:
    if arguments.snapshot is not None:
        return load_snapshot(
            arguments.snapshot,
            expected_repository=arguments.repository,
            require_strict=require_strict_snapshot,
        )
    owner, separator, name = arguments.repository.partition("/")
    if not separator or not owner or not name:
        raise ValueError("--repository must be OWNER/NAME")
    return fetch_issues(owner, name)


def _require_strict_snapshot(arguments: argparse.Namespace) -> None:
    """Validate a snapshot before a command is allowed to perform an effect."""
    if arguments.snapshot is not None:
        _records_with_mode(arguments, require_strict_snapshot=True)


def _write_json(payload: dict[str, Any], destination: Path | None) -> str:
    encoded = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if destination is None:
        return encoded

    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary: Path | None = None
    try:
        descriptor, temporary_name = tempfile.mkstemp(
            dir=destination.parent,
            prefix=f".{destination.name}.",
        )
        temporary = Path(temporary_name)
        with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
            stream.write(encoded)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, destination)
        temporary = None
    finally:
        if temporary is not None:
            with suppress(FileNotFoundError):
                temporary.unlink()
    return encoded


def _canonical_operation_value(value: Any) -> Any:
    """Return a JSON-safe value with a stable representation for operation identity.

    Operation identity is deliberately about the requested repository intent, not
    about the process which happens to execute it.  Normalising mappings and
    sequences here keeps equivalent callers (for example a tuple versus a list
    produced by an argument parser) on the same operation marker.
    """
    if isinstance(value, Mapping):
        return {
            str(key): _canonical_operation_value(item)
            for key, item in sorted(value.items(), key=lambda pair: str(pair[0]))
        }
    if isinstance(value, (tuple, list)):
        return [_canonical_operation_value(item) for item in value]
    if isinstance(value, set | frozenset):
        normalised = [_canonical_operation_value(item) for item in value]
        return sorted(normalised, key=lambda item: json.dumps(item, sort_keys=True))
    if isinstance(value, Path):
        return str(value)
    return value


def _operation_id(
    action: str,
    *,
    repository: str = DEFAULT_REPOSITORY,
    intent: Mapping[str, Any] | None = None,
    # ``inputs`` is retained as a compatibility spelling for callers of the
    # original helper.  Actor and lease are accepted for the same reason, but
    # intentionally excluded from the identity.
    inputs: Mapping[str, Any] | None = None,
    actor: str | None = None,
    lease_id: str | None = None,
) -> str:
    del actor, lease_id
    if intent is not None and inputs is not None:
        raise ValueError("operation identity accepts intent or inputs, not both")
    canonical_intent = _canonical_operation_value(intent if intent is not None else inputs or {})
    material = json.dumps(
        {
            "repository": repository.strip().lower(),
            "intent": {"action": action, "inputs": canonical_intent},
        },
        sort_keys=True,
        separators=(",", ":"),
    ).encode()
    return hashlib.sha256(material).hexdigest()


@contextlib.contextmanager
def _operation_lock(path: Path):
    """Hold a bounded per-operation lock for local writers.

    The lock is an optimisation and crash guard, not the operation's authority:
    a process on another checkout (or a process which did not acquire this lock)
    is reconciled by scanning exact operation markers.  ``flock`` releases the
    lock when a process dies, while the timeout keeps a healthy retry bounded.
    """
    lock_path = path.with_name(f".{path.name}.lock")
    if lock_path in _HELD_OPERATION_LOCKS:
        yield
        return
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    deadline = time.monotonic() + _OPERATION_LOCK_TIMEOUT_SECONDS
    stream = lock_path.open("a+", encoding="utf-8")
    try:
        while True:
            try:
                fcntl.flock(stream.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
                break
            except BlockingIOError:
                if time.monotonic() >= deadline:
                    raise RuntimeError(
                        f"operation lock timeout for {path.name}; retry after the active "
                        "operation releases it"
                    ) from None
                time.sleep(_OPERATION_LOCK_POLL_SECONDS)
        _HELD_OPERATION_LOCKS.add(lock_path)
        try:
            yield
        finally:
            _HELD_OPERATION_LOCKS.discard(lock_path)
            fcntl.flock(stream.fileno(), fcntl.LOCK_UN)
    finally:
        stream.close()


def _write_operation_journal(
    path: Path, *, operation_id: str, action: str, intent: dict[str, Any]
) -> dict[str, Any]:
    journal = {
        "record_format": "gordian-registry-operation-v1",
        "operation_id": operation_id,
        "action": action,
        "status": "planned",
        "intent": intent,
        "outcomes": [],
    }
    if path.exists():
        try:
            existing = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            raise RuntimeError("operation journal is unreadable") from error
        if not isinstance(existing, dict) or any(
            existing.get(key) != journal[key] for key in ("operation_id", "action", "intent")
        ):
            raise RuntimeError("operation journal identity mismatch")
        return existing
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(dir=path.parent, prefix=f".{path.name}.")
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
            stream.write(json.dumps(journal, indent=2, sort_keys=True) + "\n")
            stream.flush()
            os.fsync(stream.fileno())
        try:
            os.link(temporary, path)
        except FileExistsError:
            return _write_operation_journal(
                path, operation_id=operation_id, action=action, intent=intent
            )
        return journal
    finally:
        with suppress(FileNotFoundError):
            temporary.unlink()


def _operation_journal_path(base: Path, operation_id: str) -> Path:
    return base.with_name(f"{base.stem}-{operation_id}{base.suffix or '.json'}")


def _operation_issue_marker_present(issue: IssueRecord, marker: str) -> bool:
    """Return whether an issue body carries this operation's marker.

    Generated markers are line-oriented comments.  Requiring the complete line
    avoids treating a quoted marker or a marker for another operation with a
    common prefix as ownership.  The short-marker fallback keeps the private
    helper useful for older callers/tests which used a plain sentinel.
    """
    if marker.startswith("<!-- gordian-operation:"):
        return re.search(rf"(?m)^[ \t]*{re.escape(marker)}[ \t]*$", issue.body) is not None
    return marker in issue.body


def _recorded_operation_numbers(journal: Mapping[str, Any]) -> tuple[int, ...]:
    outcomes = journal.get("outcomes", ())
    if outcomes is None:
        return ()
    if not isinstance(outcomes, Sequence) or isinstance(outcomes, (str, bytes, bytearray)):
        raise RuntimeError("operation journal outcomes are unreadable")
    numbers: list[int] = []
    for outcome in outcomes:
        if not isinstance(outcome, Mapping):
            continue
        if outcome.get("effect") != "issue":
            continue
        number = outcome.get("number")
        if isinstance(number, bool):
            raise RuntimeError("operation journal records an invalid issue identity")
        if isinstance(number, int):
            numbers.append(number)
    return tuple(numbers)


def _find_operation_issues(
    issues: Sequence[IssueRecord], marker: str, journal: Mapping[str, Any] | None = None
) -> tuple[IssueRecord, ...]:
    """Return every issue carrying an exact operation marker, in number order.

    A journaled issue is evidence that an effect was observed, not a uniqueness
    proof.  Concurrent creators can therefore leave two (or more) marker-owned
    issues; callers must inspect the complete set before choosing a canonical
    issue or retiring extras.
    """
    matches = tuple(
        sorted(
            (issue for issue in issues if _operation_issue_marker_present(issue, marker)),
            key=lambda issue: issue.number,
        )
    )
    # Validate the shape of recorded outcomes even when the marker scan is the
    # authority.  We deliberately do not discard another marker match merely
    # because the journal names one issue.
    if journal is not None:
        _recorded_operation_numbers(journal)
    return matches


def _find_operation_issue(
    issues: Sequence[IssueRecord], marker: str, journal: Mapping[str, Any]
) -> IssueRecord | None:
    """Recover one operation-owned issue for legacy singular callers.

    New-Atom recovery uses :func:`_find_operation_issues` and performs explicit
    deterministic convergence.  This wrapper retains the old fail-closed API for
    callers that genuinely require a unique match.
    """
    matches = _find_operation_issues(issues, marker, journal)
    if len(matches) > 1:
        raise RuntimeError("operation marker matches multiple created issues")
    recorded = _recorded_operation_numbers(journal)
    if recorded:
        identities = set(recorded)
        if len(identities) != 1:
            raise RuntimeError("operation journal records conflicting issue identities")
        issue = next((issue for issue in issues if issue.number == recorded[0]), None)
        if issue is None or not _operation_issue_marker_present(issue, marker):
            raise RuntimeError("journal issue identity is not marker-owned")
        return issue
    return matches[0] if matches else None


def _project_recovery_base(
    issues: Sequence[IssueRecord],
    recovered: IssueRecord,
    downstream: Sequence[int],
    extras: Sequence[IssueRecord] = (),
) -> tuple[IssueRecord, ...]:
    """Build a base with this operation's additive effects removed.

    Existing marker-owned issues are provisional effects of this operation, not
    part of the registry base used to recompute a plan.  Removing every such
    issue (and each corresponding downstream edge) keeps retries stable even
    when an unrelated issue was created after the first attempt.
    """
    operation_numbers = {recovered.number, *(issue.number for issue in extras)}
    result: list[IssueRecord] = []
    for issue in issues:
        if issue.number in operation_numbers:
            continue
        owned_blockers = set(issue.blocked_by).intersection(operation_numbers)
        if issue.number not in downstream or not owned_blockers:
            result.append(issue)
            continue
        blockers = tuple(number for number in issue.blocked_by if number not in operation_numbers)
        result.append(
            replace(
                issue,
                blocked_by=blockers,
                body=replace_body_dependencies(issue.body, blockers),
            )
        )
    return tuple(sorted(result, key=lambda issue: issue.number))


def _read_operation_journal(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise RuntimeError("operation journal is unreadable") from error
    if not isinstance(payload, dict):
        raise RuntimeError("operation journal is unreadable")
    outcomes = payload.get("outcomes", [])
    if not isinstance(outcomes, list):
        raise RuntimeError("operation journal outcomes are unreadable")
    return payload


def _record_operation_outcome(journal: dict[str, Any], path: Path, outcome: dict[str, Any]) -> None:
    """Merge one observed effect into the durable journal exactly once.

    Recovery can be driven by two processes.  Reading the current journal while
    holding the operation lock prevents one process from erasing another's
    outcome; the equality check makes retries idempotent even after a response
    was lost and the same effect is observed again.
    """
    with _operation_lock(path):
        current = _read_operation_journal(path) if path.exists() else dict(journal)
        outcomes = current.setdefault("outcomes", [])
        if not isinstance(outcomes, list):
            raise RuntimeError("operation journal outcomes are unreadable")
        if outcome not in outcomes:
            outcomes.append(outcome)
        if outcome.get("effect") == "convergence":
            current["status"] = "completed"
        elif current.get("status") not in {"completed", "converged"}:
            current["status"] = "in-progress"
        _write_json(current, path)
        journal.clear()
        journal.update(current)


def _mark_operation_recovery_required(
    journal: dict[str, Any], path: Path, *, error: Exception
) -> None:
    """Durably record an additive operation that needs deterministic resume.

    Add-edge has no safe inverse: deleting a native edge or restoring an old body
    can remove another writer's valid additive change.  A failed operation is
    therefore never reported as compensated.  The journal itself is the durable
    hand-off, and retrying the same operation id performs fresh-read idempotent
    reconciliation.
    """
    with _operation_lock(path):
        current = _read_operation_journal(path) if path.exists() else dict(journal)
        current["status"] = "recovery-required"
        current["last_error"] = str(error)
        current["recovery"] = {
            "state": "pending",
            "strategy": "fresh-read-idempotent-additive-reconcile",
            "retry": "rerun the same add-edge intent; never remove an edge",
        }
        _write_json(current, path)
        journal.clear()
        journal.update(current)


def _operation_marker_count(body: str, marker: str) -> int:
    if marker.startswith("<!-- gordian-operation:"):
        return len(re.findall(rf"(?m)^[ \t]*{re.escape(marker)}[ \t]*$", body))
    return body.count(marker)


def _remove_operation_marker(body: str, marker: str) -> str:
    """Remove one complete operation-marker line while preserving other bytes."""
    if marker.startswith("<!-- gordian-operation:"):
        match = re.search(rf"(?m)^[ \t]*{re.escape(marker)}[ \t]*\r?\n?", body)
        if match is None:
            raise ValueError("operation marker is absent")
        return body[: match.start()] + body[match.end() :]
    position = body.find(marker)
    if position < 0:
        raise ValueError("operation marker is absent")
    return body[:position] + body[position + len(marker) :]


def _operation_base_body(body: str, marker: str, *, target_crate: str | None) -> str:
    """Strip only this operation's generated artifacts for digest validation."""
    without_marker = _remove_operation_marker(body, marker)
    if target_crate is not None:
        return _remove_generated_section_block(
            without_marker,
            heading="Target crate",
            begin=TARGET_CRATE_BEGIN,
            end=TARGET_CRATE_END,
            remove_empty_section=True,
        )
    return without_marker


def _operation_identity_problems(
    issue: IssueRecord,
    *,
    spec: NewAtomSpec,
    marker: str,
    intent: Mapping[str, Any],
) -> tuple[str, ...]:
    """Check identity fields which must be true before duplicate convergence."""
    problems: list[str] = []
    if not _operation_issue_marker_present(issue, marker):
        problems.append("body does not carry the exact operation marker")
    if _operation_marker_count(issue.body, marker) != 1:
        problems.append("body carries the operation marker more than once")
    if issue.title != spec.title:
        problems.append(f"title {issue.title!r} != intended {spec.title!r}")
    if issue.milestone != spec.milestone:
        problems.append(f"milestone {issue.milestone!r} != intended {spec.milestone!r}")
    if issue.state.upper() != "OPEN":
        problems.append(f"state {issue.state!r} is not OPEN")
    type_labels = TYPE_LABELS.intersection(issue.labels)
    if spec.type_label not in type_labels:
        problems.append(f"missing intended type label {spec.type_label!r}")
    if len(type_labels) != 1:
        problems.append(f"expected one type label, found {sorted(type_labels)}")
    try:
        mirrored = dependencies_from_body(issue.body)
    except ValueError as error:
        problems.append(f"dependency mirror is invalid: {error}")
    else:
        if mirrored != spec.blocked_by:
            problems.append(
                f"body dependencies {list(mirrored)} != intended {list(spec.blocked_by)}"
            )
    try:
        base_body = _operation_base_body(
            issue.body,
            marker,
            target_crate=spec.target_crate,
        )
    except ValueError as error:
        problems.append(str(error))
    else:
        expected_digest = intent.get("body_sha256")
        if isinstance(expected_digest, str):
            actual_digests = {
                hashlib.sha256(candidate.encode()).hexdigest()
                for candidate in (base_body, base_body.rstrip(), base_body.rstrip() + "\n")
            }
            if expected_digest not in actual_digests:
                problems.append("body does not match the intended specification digest")
    return tuple(problems)


def _single_issue_payload(repository: str, number: int) -> Mapping[str, Any]:
    payload = run_gh_json(["api", f"repos/{repository}/issues/{number}"])
    if not isinstance(payload, Mapping):
        raise RuntimeError(f"#{number}: GitHub API returned no issue object")
    return payload


def _single_issue_labels(payload: Mapping[str, Any]) -> set[str]:
    labels = payload.get("labels", ())
    if not isinstance(labels, Sequence) or isinstance(labels, (str, bytes, bytearray)):
        raise RuntimeError("GitHub API returned malformed issue labels")
    names: set[str] = set()
    for label in labels:
        name = label.get("name") if isinstance(label, Mapping) else label
        if isinstance(name, str):
            names.add(name)
    return names


def _issue_record_from_payload(
    payload: Mapping[str, Any], *, repository: str, number: int
) -> IssueRecord:
    """Decode the small single-issue response used after an ambiguous create."""
    body = payload.get("body")
    title = payload.get("title")
    state = payload.get("state")
    if not isinstance(body, str) or not isinstance(title, str) or not isinstance(state, str):
        raise RuntimeError(f"#{number}: GitHub API returned an incomplete issue object")
    milestone_value = payload.get("milestone")
    milestone = (
        str(milestone_value.get("title"))
        if isinstance(milestone_value, Mapping) and milestone_value.get("title") is not None
        else None
    )
    labels = tuple(sorted(_single_issue_labels(payload)))
    url = str(payload.get("html_url") or payload.get("url") or "")
    if not url:
        url = f"https://github.com/{repository}/issues/{number}"
    raw_blockers = payload.get("blockedBy", ())
    blockers: list[int] = []
    if isinstance(raw_blockers, Sequence) and not isinstance(raw_blockers, (str, bytes, bytearray)):
        for value in raw_blockers:
            if isinstance(value, Mapping):
                value = value.get("number")
            if isinstance(value, int) and not isinstance(value, bool):
                blockers.append(value)
    return IssueRecord(
        number=number,
        title=title,
        state=state,
        blocked_by=tuple(sorted(set(blockers))),
        body=body,
        labels=labels,
        milestone=milestone,
        url=url,
    )


def _verify_operation_duplicate(
    arguments: argparse.Namespace, *, issue: int, marker: str
) -> None:
    """Verify a retired duplicate still belongs to this operation."""
    payload = _single_issue_payload(arguments.repository, issue)
    body = payload.get("body")
    if not isinstance(body, str) or _operation_marker_count(body, marker) != 1:
        raise RuntimeError(
            f"operation recovery refused to retire #{issue}: exact marker is not present"
        )
    state = str(payload.get("state") or "").upper()
    if state != "CLOSED":
        raise RuntimeError(f"operation recovery did not close marker-owned #{issue}")
    if "duplicate" not in _single_issue_labels(payload):
        raise RuntimeError(
            f"operation recovery did not mark marker-owned duplicate #{issue}"
        )


def _retire_operation_duplicates(
    arguments: argparse.Namespace,
    *,
    canonical: IssueRecord,
    extras: Sequence[IssueRecord],
    marker: str,
    journal: dict[str, Any],
    journal_path: Path,
) -> tuple[int, ...]:
    """Close and label only exact-marker-owned duplicate issues.

    The number ordering is the convergence rule.  A fresh read and post-write
    verification bracket every extra, so a copied marker or an unrelated issue
    can never be swept into compensation.
    """
    retired: list[int] = []
    for candidate in sorted(extras, key=lambda issue: issue.number):
        if candidate.number == canonical.number:
            continue
        current = next(
            (issue for issue in _records(arguments) if issue.number == candidate.number),
            None,
        )
        if current is None:
            # The collection endpoint filters a duplicate-labelled issue.  Do
            # not trust the stale pre-scan record in that case: direct-read the
            # current body before issuing either mutation.
            payload = _single_issue_payload(arguments.repository, candidate.number)
            direct_state = str(payload.get("state") or "").upper()
            direct_labels = _single_issue_labels(payload)
            direct_body = payload.get("body")
            if not isinstance(direct_body, str) or _operation_marker_count(
                direct_body, marker
            ) != 1:
                raise RuntimeError(
                    "operation recovery refused to retire "
                    f"#{candidate.number}: exact marker is not present"
                )
            if direct_state == "CLOSED" and "duplicate" in direct_labels:
                _record_operation_outcome(
                    journal,
                    journal_path,
                    {
                        "effect": "duplicate",
                        "number": candidate.number,
                        "canonical": canonical.number,
                        "status": "already-closed-and-marked",
                    },
                )
                retired.append(candidate.number)
                continue
            current = replace(candidate, body=direct_body, state=str(payload.get("state") or ""))
        if not _operation_issue_marker_present(current, marker):
            # It may already have been filtered after a successful duplicate
            # label.  Direct verification is the only safe interpretation.
            _verify_operation_duplicate(arguments, issue=candidate.number, marker=marker)
            retired.append(candidate.number)
            continue
        run_gh(
            [
                "issue",
                "edit",
                str(candidate.number),
                "--repo",
                arguments.repository,
                "--add-label",
                "duplicate",
            ]
        )
        run_gh(
            [
                "issue",
                "close",
                str(candidate.number),
                "--repo",
                arguments.repository,
            ]
        )
        _verify_operation_duplicate(arguments, issue=candidate.number, marker=marker)
        _record_operation_outcome(
            journal,
            journal_path,
            {
                "effect": "duplicate",
                "number": candidate.number,
                "canonical": canonical.number,
                "status": "closed-and-marked",
            },
        )
        retired.append(candidate.number)
    return tuple(sorted(set(retired)))


def _operation_candidates(
    issues: Sequence[IssueRecord], marker: str, journal: Mapping[str, Any]
) -> tuple[IssueRecord, ...]:
    """Scan all marker matches, independent of recorded issue outcomes."""
    return _find_operation_issues(issues, marker, journal)


def _phase_memberships(execution_order: str, number: int) -> tuple[int, ...]:
    """Return execution phases containing an Atom member bullet."""
    memberships: list[int] = []
    sections = list(re.finditer(r"^## (\d+)\. .+$", execution_order, re.MULTILINE))
    for position, heading in enumerate(sections):
        end = (
            sections[position + 1].start()
            if position + 1 < len(sections)
            else len(execution_order)
        )
        section = execution_order[heading.end() : end]
        members = re.search(r"^### Members[ \t]*$", section, re.MULTILINE)
        if members is None:
            continue
        tail = section[members.end() :]
        next_subsection = re.search(r"^### ", tail, re.MULTILINE)
        member_text = tail[: next_subsection.start()] if next_subsection else tail
        if re.search(rf"^- #{number}(?:\s|$)", member_text, re.MULTILINE):
            memberships.append(int(heading.group(1)))
    return tuple(sorted(memberships))


def _project_item_for_issue(arguments: argparse.Namespace, number: int):
    board = fetch_board(arguments.project_owner, arguments.project_number)
    item = board.items.get(number)
    if item is None:
        raise RuntimeError(
            f"operation recovery found canonical #{number} absent from "
            f"Project {arguments.project_number}"
        )
    return board, item


def _validate_project_projection(
    arguments: argparse.Namespace,
    issues: tuple[IssueRecord, ...],
    *,
    number: int,
) -> None:
    """Require membership and zero derived-field drift for a completed replay."""
    board, _ = _project_item_for_issue(arguments, number)
    schema = load_closure_schema(arguments.closure_schema)
    satisfied, unevidenced = bootstrap_satisfied(
        issues,
        closure_root=arguments.closure_root,
        schema=schema,
    )
    if unevidenced:
        raise RuntimeError(
            "closed without validating closure record: "
            + ", ".join(f"#{atom}" for atom in unevidenced)
        )
    rows = derive(issues, satisfied=satisfied, board_status={
        atom: item.status for atom, item in board.items.items()
    })
    changes, absent = plan_changes(rows, board)
    if absent:
        raise RuntimeError(
            f"Project {arguments.project_number} is missing registry Atoms: "
            + ", ".join(f"#{atom}" for atom in absent)
        )
    if changes:
        changed = ", ".join(
            f"#{change.number} {change.field_name}={change.desired!r}"
            for change in changes
        )
        raise RuntimeError("Project derived projections are not converged: " + changed)


def _validate_recovered_issue(
    arguments: argparse.Namespace,
    issues: tuple[IssueRecord, ...],
    *,
    issue: IssueRecord,
    spec: NewAtomSpec,
    marker: str,
    intent: Mapping[str, Any],
    full: bool,
) -> None:
    """Validate a recovered issue before treating a replay as complete."""
    problems = list(
        _operation_identity_problems(issue, spec=spec, marker=marker, intent=intent)
    )
    records = {record.number: record for record in issues}
    if issue.blocked_by != spec.blocked_by:
        problems.append(
            f"native prerequisites {list(issue.blocked_by)} != intended {list(spec.blocked_by)}"
        )
    for downstream in spec.blocks:
        record = records.get(downstream)
        if record is None:
            problems.append(f"intended downstream #{downstream} is absent from the registry")
        elif issue.number not in record.blocked_by:
            problems.append(
                f"intended downstream #{downstream} does not block on #{issue.number}"
            )

    try:
        rows = parse_project_plan(arguments.project_plan.read_text(encoding="utf-8"))
        row = rows.get(issue.number)
    except (OSError, ValueError) as error:
        problems.append(f"project-plan projection cannot be read: {error}")
        row = None
    if row is None:
        problems.append(f"project-plan projection omits canonical #{issue.number}")
    else:
        expected_target = "—" if spec.target_crate is None else f"`{spec.target_crate}`"
        if row.title != spec.title:
            problems.append(f"project-plan title {row.title!r} != intended {spec.title!r}")
        if row.initiative != spec.milestone:
            problems.append(
                f"project-plan Initiative {row.initiative!r} != intended {spec.milestone!r}"
            )
        if row.target_crate != expected_target:
            problems.append(
                f"project-plan target crate {row.target_crate!r} != intended {expected_target!r}"
            )
        if row.blocked_by != spec.blocked_by:
            problems.append(
                f"project-plan blockers {list(row.blocked_by)} != intended {list(spec.blocked_by)}"
            )

    try:
        execution_order = arguments.execution_order.read_text(encoding="utf-8")
        memberships = _phase_memberships(execution_order, issue.number)
    except OSError as error:
        problems.append(f"execution-order projection cannot be read: {error}")
        memberships = ()
    if memberships != (spec.phase,):
        problems.append(
            f"execution phase membership {list(memberships)} != intended [{spec.phase}]"
        )

    if spec.target_crate is not None:
        try:
            crate_map = arguments.crate_map.read_text(encoding="utf-8")
            target_report = audit_target_crate_contracts(
                issues,
                arguments.project_plan.read_text(encoding="utf-8"),
                crate_map,
            )
            if not target_report.clean:
                problems.extend(
                    "target-crate contracts: " + problem
                    for problem in target_report.problems
                )
            expected_bodies = render_target_crate_bodies(
                issues,
                arguments.project_plan.read_text(encoding="utf-8"),
                crate_map,
            )
            expected_body = expected_bodies.get(issue.number)
            if expected_body is not None and issue.body != expected_body:
                problems.append(
                    "issue body target-crate projection differs from the canonical render"
                )
        except (OSError, RuntimeError, ValueError) as error:
            problems.append(f"target-crate projection cannot be validated: {error}")

    # The full replay check is intentionally performed only after identity and
    # graph checks.  It is expensive (and reads Project state), but a success
    # response must prove every external projection and global registry mirror.
    if full:
        reports = _coherence_reports(arguments, issues)
        problems.extend(_coherence_problems(reports))
        try:
            _validate_project_projection(arguments, issues, number=issue.number)
        except RuntimeError as error:
            problems.append(str(error))

    if problems:
        raise RuntimeError(
            f"operation recovery spec mismatch for #{issue.number}: " + "; ".join(problems)
        )


def _write_dependency_mirror(
    arguments: argparse.Namespace, *, issue: int, blockers: tuple[int, ...]
) -> str:
    """Merge and verify only Dependencies against a bounded fresh live read."""
    last_error: RuntimeError | None = None
    for _ in range(3):
        live = _records(arguments)
        current = next((record for record in live if record.number == issue), None)
        if current is None:
            raise RuntimeError(f"#{issue} disappeared while updating its dependency mirror")
        proposed = replace_body_dependencies(current.body, blockers)
        if proposed == current.body:
            return proposed
        try:
            run_gh(
                [
                    "issue",
                    "edit",
                    str(issue),
                    "--repo",
                    arguments.repository,
                    "--body",
                    proposed,
                ]
            )
        except RuntimeError as error:
            last_error = error
        verified = next((record for record in _records(arguments) if record.number == issue), None)
        if verified is not None and verified.body == proposed:
            return proposed
    raise RuntimeError("dependency mirror changed concurrently; retry required") from last_error


def _ensure_native_edge(arguments: argparse.Namespace, issue: int, blocker: int) -> None:
    """Add an edge, recovering an ambiguous response from a canonical reread."""
    try:
        _add_native_edge(arguments.repository, issue, blocker)
    except RuntimeError:
        current = next((record for record in _records(arguments) if record.number == issue), None)
        if current is None or blocker not in current.blocked_by:
            raise


def _audit(
    arguments: argparse.Namespace,
    issues: tuple[IssueRecord, ...],
    *,
    include_target_crates: bool = True,
) -> AuditReport:
    return audit(
        issues,
        project_plan=arguments.project_plan.read_text(encoding="utf-8"),
        execution_order=arguments.execution_order.read_text(encoding="utf-8"),
        issue_index=arguments.issue_index.read_text(encoding="utf-8"),
        target=arguments.target,
        crate_map=(
            arguments.crate_map.read_text(encoding="utf-8")
            if include_target_crates
            else None
        ),
    )


def _coherence_reports(
    arguments: argparse.Namespace, issues: tuple[IssueRecord, ...]
) -> tuple[tuple[str, Any], ...]:
    """Return the core and focused registry audits in stable diagnostic order."""
    project_plan = arguments.project_plan.read_text(encoding="utf-8")
    execution_order = arguments.execution_order.read_text(encoding="utf-8")
    crate_map = arguments.crate_map.read_text(encoding="utf-8")
    return (
        ("core", _audit(arguments, issues, include_target_crates=False)),
        (
            "benchmark obligations",
            audit_benchmark_obligations(
                issues, execution_order, qualification_atom=arguments.target
            ),
        ),
        (
            "target-crate contracts",
            audit_target_crate_contracts(issues, project_plan, crate_map),
        ),
    )


def _coherence_problems(
    reports: tuple[tuple[str, Any], ...], *, include: set[str] | None = None
) -> tuple[str, ...]:
    """Flatten non-clean reports without changing their deterministic ordering."""
    problems: list[str] = []
    for label, report in reports:
        if include is not None and label not in include:
            continue
        problems.extend(f"{label}: {problem}" for problem in report.problems)
    return tuple(problems)


def _command_check(arguments: argparse.Namespace) -> int:
    report = _audit(arguments, _records(arguments))
    payload = report.as_json_object()
    if arguments.json or arguments.report is not None:
        encoded = _write_json(payload, arguments.report)
        if arguments.json:
            print(encoded, end="")
    else:
        if report.clean:
            print(
                f"clean: {report.issue_count} Atoms; maximum path to #{report.target} "
                f"is {report.maximum_path_length} edges"
            )
        else:
            for problem in report.problems:
                print(f"FAIL: {problem}")
    return 0 if report.clean else 1


def _command_capture(arguments: argparse.Namespace) -> int:
    if arguments.snapshot is not None:
        raise ValueError("capture reads live GitHub state; remove --snapshot")
    issues = _records(arguments)
    reports = _coherence_reports(arguments, issues)
    problems = _coherence_problems(reports)
    if problems:
        for problem in problems:
            print(f"FAIL: {problem}", file=sys.stderr)
        print(
            "refusing to capture a registry with non-coherent core, benchmark, "
            "or target-crate projections",
            file=sys.stderr,
        )
        return 1
    stamp = provenance.collect()
    snapshot = snapshot_object(issues, stamp, repository=arguments.repository)
    snapshot["coherent"] = True
    encoded = _write_json(snapshot, arguments.output)
    if arguments.output is None:
        print(encoded, end="")
    return 0


def _normalization_mode(arguments: argparse.Namespace) -> str:
    """Resolve the normalize command's positional and flag spellings."""
    action = getattr(arguments, "normalization_action", "plan")
    command = getattr(arguments, "command", "normalize")
    if command == "normalize-apply":
        action = "apply"
    elif command == "normalize-recover":
        action = "recover"
    elif command == "normalize-plan":
        action = "plan"
    if getattr(arguments, "plan", False):
        if action != "plan":
            raise ValueError("normalize cannot combine --plan with apply or recover")
        action = "plan"
    if getattr(arguments, "dry_run", False):
        if action != "plan":
            raise ValueError("normalize cannot combine --dry-run with apply or recover")
        action = "plan"
    if getattr(arguments, "apply", False):
        if action not in {"plan", "apply"}:
            raise ValueError("normalize cannot combine --apply with recover")
        action = "apply"
    if getattr(arguments, "recover", False):
        if action not in {"plan", "recover"}:
            raise ValueError("normalize cannot combine --recover with --apply")
        action = "recover"
    if getattr(arguments, "compensate", False) and action != "recover":
        raise ValueError("--compensate requires normalize recover")
    return action


def _normalization_authorize(arguments: argparse.Namespace) -> ClaimLease:
    """Run the existing preflight and #70 claim gate for a live mutation."""
    auth = preflight(
        repository=arguments.repository,
        project_owner=arguments.project_owner,
        project_number=arguments.project_number,
    )
    return require_live_claim(
        repository=arguments.repository,
        number=70,
        login=auth.login,
        now=datetime.now(UTC),
    )


def _normalization_patch_body(repository: str, issue: int, body: str) -> None:
    """Route journal body writes through this module's shared gh CLI symbol."""
    run_gh(
        [
            "api",
            "--method",
            "PATCH",
            f"repos/{repository}/issues/{issue}",
            "-f",
            f"body={body}",
        ]
    )


def _normalization_add_label(repository: str, issue: int, label: str) -> None:
    """Route journal label additions through the shared gh CLI symbol."""
    run_gh(
        [
            "issue",
            "edit",
            str(issue),
            "--repo",
            repository,
            "--add-label",
            label,
        ]
    )


def _normalization_final_audit(
    arguments: argparse.Namespace,
    live: tuple[IssueRecord, ...],
    journal: dict[str, Any],
) -> tuple[bool, tuple[str, ...]]:
    """Run all live audits before the separate snapshot-capture step."""
    reports = _coherence_reports(arguments, live)
    problems = _coherence_problems(reports)
    if problems:
        journal["status"] = "audit-failed"
        journal["remaining_drift"] = list(problems)
        write_journal(journal, arguments.journal)
        return False, problems
    journal.pop("remaining_drift", None)
    return True, ()


def _command_normalize(arguments: argparse.Namespace) -> int:
    """Plan, apply, or recover a manifest-bound live Atom normalization journal."""
    action = _normalization_mode(arguments)
    if arguments.target_atom != 70:
        raise ValueError("Atom-contract normalization is bounded to Atom #70")
    if action != "plan" and arguments.snapshot is not None:
        _require_strict_snapshot(arguments)
        raise ValueError("normalize apply/recover requires live state; remove --snapshot")

    manifest = load_manifest(
        arguments.manifest,
        atom=arguments.target_atom,
        repository=arguments.repository,
    )
    if action == "plan":
        issues = _records(arguments)
        plan = plan_normalization(
            issues,
            manifest,
            repository=arguments.repository,
            atom=arguments.target_atom,
            fetch_label_record=(
                _fetch_normalization_label_record if arguments.snapshot is None else None
            ),
        )
        payload = {
            "mode": "plan",
            "apply": False,
            "manifest": str(arguments.manifest),
            "journal": str(arguments.journal),
            **plan.as_json_object(),
        }
        print(json.dumps(payload, indent=2, sort_keys=True))
        return 0

    lease = _normalization_authorize(arguments)
    writer = lease.actor
    if action == "recover":
        current = read_journal(arguments.journal)
        if current.get("manifest_sha256") != manifest.digest:
            raise NormalizationError(
                "normalization journal manifest differs from the committed manifest"
            )
        journal = recover_journal(
            arguments.journal,
            writer=writer,
            lease=lease,
            fetch_records=lambda: _records(arguments),
            patch_body=_normalization_patch_body,
            add_edge=_add_native_edge,
            add_label=_normalization_add_label,
            fetch_label_record=_fetch_normalization_label_record,
            compensate=arguments.compensate,
            manifest=manifest,
        )
    else:
        issues = _records(arguments)
        plan = plan_normalization(
            issues,
            manifest,
            repository=arguments.repository,
            atom=arguments.target_atom,
            fetch_label_record=_fetch_normalization_label_record,
        )
        journal = advance_journal(
            plan,
            arguments.journal,
            writer=writer,
            lease=lease,
            fetch_records=lambda: _records(arguments),
            patch_body=_normalization_patch_body,
            add_edge=_add_native_edge,
            add_label=_normalization_add_label,
            fetch_label_record=_fetch_normalization_label_record,
            manifest=manifest,
        )

    if not journal_complete(journal):
        payload = {
            "mode": action,
            "apply": True,
            "manifest": str(arguments.manifest),
            "journal": str(arguments.journal),
            "status": journal.get("status"),
            "conflicts": journal.get("conflicts", []),
            "snapshot_skipped": True,
        }
        print(json.dumps(payload, indent=2, sort_keys=True))
        return 1

    # This is intentionally a new live read after the effect journal reaches its
    # verified state.  No snapshot is consulted as a mutation input.
    live = tuple(_records(arguments))
    clean, problems = _normalization_final_audit(arguments, live, journal)
    if not clean:
        payload = {
            "mode": action,
            "apply": True,
            "manifest": str(arguments.manifest),
            "journal": str(arguments.journal),
            "status": journal.get("status"),
            "remaining_drift": list(problems),
            "snapshot_skipped": True,
        }
        print(json.dumps(payload, indent=2, sort_keys=True))
        return 1

    # Snapshot capture is a final, separate step, after every live audit has passed.
    encoded = _write_json(
        {
            **snapshot_object(
                live,
                provenance.collect(),
                repository=arguments.repository,
            ),
            "coherent": True,
        },
        arguments.output,
    )
    journal["status"] = "completed"
    journal["snapshot"] = str(arguments.output)
    write_journal(journal, arguments.journal)
    payload = {
        "mode": action,
        "apply": True,
        "manifest": str(arguments.manifest),
        "journal": str(arguments.journal),
        "status": journal["status"],
        "snapshot": str(arguments.output),
        "coherent": True,
    }
    if arguments.output is None:
        payload["snapshot_json"] = encoded
    print(json.dumps(payload, indent=2, sort_keys=True))
    return 0


def _command_render_spine(arguments: argparse.Namespace) -> int:
    if arguments.write:
        _require_strict_snapshot(arguments)
    issues = _records(arguments)
    current = arguments.execution_order.read_text(encoding="utf-8")
    rendered = render_spine_block(issues, arguments.target)
    updated = replace_spine_block(current, rendered)
    if arguments.write:
        arguments.execution_order.write_text(updated, encoding="utf-8")
    else:
        print(rendered)
    return 0


def _command_render_plan(arguments: argparse.Namespace) -> int:
    if arguments.write:
        _require_strict_snapshot(arguments)
    issues = _records(arguments)
    current = arguments.project_plan.read_text(encoding="utf-8")
    rendered = render_project_plan(
        current,
        issues,
        arguments.crate_map.read_text(encoding="utf-8"),
    )
    if arguments.write:
        arguments.project_plan.write_text(rendered, encoding="utf-8")
    else:
        print(rendered, end="")
    return 0


def _command_check_benchmarks(arguments: argparse.Namespace) -> int:
    report = audit_benchmark_obligations(
        _records(arguments),
        arguments.execution_order.read_text(encoding="utf-8"),
        qualification_atom=arguments.target,
    )
    if arguments.json:
        print(json.dumps(report.as_json_object(), indent=2, sort_keys=True))
    elif report.clean:
        print(
            f"clean: {report.row_count} EO17 rows, {len(report.owners)} owners, "
            f"{len(report.first_qualification_ids)} first-qualification rows"
        )
    else:
        for problem in report.problems:
            print(f"FAIL: {problem}")
    return 0 if report.clean else 1


def _command_check_target_crates(arguments: argparse.Namespace) -> int:
    issues = _records(arguments)
    report = audit_target_crate_contracts(
        issues,
        arguments.project_plan.read_text(encoding="utf-8"),
        arguments.crate_map.read_text(encoding="utf-8"),
    )
    payload = report.as_json_object()
    if arguments.json or arguments.report is not None:
        encoded = _write_json(payload, arguments.report)
        if arguments.json:
            print(encoded, end="")
    elif report.clean:
        print(f"clean: {report.owner_count} target-crate owners")
    else:
        for problem in report.problems:
            print(f"FAIL: {problem}")
    return 0 if report.clean else 1


def _command_check_normalization(arguments: argparse.Namespace) -> int:
    """Check the committed normalization manifest without reading live GitHub state."""
    manifest = load_manifest(
        arguments.manifest,
        atom=_DEFAULT_NORMALIZATION_ATOM,
        repository=DEFAULT_REPOSITORY,
    )
    report = audit_normalization_manifest(
        manifest,
        arguments.crate_map.read_text(encoding="utf-8"),
    )
    payload = report.as_json_object()
    if arguments.json:
        print(json.dumps(payload, indent=2, sort_keys=True))
    elif report.clean:
        print(f"clean: normalization manifest covers {report.issue_count} canonical Atoms")
    else:
        for problem in report.problems:
            print(f"FAIL: {problem}")
    return 0 if report.clean else 1


def _restore_issue_bodies(
    repository: str, originals: dict[int, str], applied: list[int]
) -> tuple[str, ...]:
    failures: list[str] = []
    for number in reversed(applied):
        try:
            run_gh(
                [
                    "issue",
                    "edit",
                    str(number),
                    "--repo",
                    repository,
                    "--body",
                    originals[number],
                ]
            )
        except RuntimeError as error:
            failures.append(f"#{number}: {error}")
    return tuple(failures)


def _command_sync_benchmarks(arguments: argparse.Namespace) -> int:
    if arguments.apply and arguments.snapshot is not None:
        _require_strict_snapshot(arguments)
        raise ValueError("sync-benchmarks --apply requires live GitHub state")

    auth = None
    if arguments.apply:
        auth = preflight(
            repository=arguments.repository,
            project_owner=arguments.project_owner,
            project_number=arguments.project_number,
        )
        require_live_claim(
            repository=arguments.repository,
            number=70,
            login=auth.login,
            now=datetime.now(UTC),
        )

    issues = _records(arguments)
    execution_order = arguments.execution_order.read_text(encoding="utf-8")
    updates = render_benchmark_bodies(
        issues,
        execution_order,
        qualification_atom=arguments.target,
    )
    records = {issue.number: issue for issue in issues}
    changes = tuple(
        number for number, body in sorted(updates.items()) if body != records[number].body
    )
    rows = parse_benchmark_obligations(execution_order)
    payload: dict[str, Any] = {
        "apply": arguments.apply,
        "changed_issues": list(changes),
        "row_count": len(rows),
        "first_qualification_ids": sorted(
            row.row_id for row in rows if row.first_qualification
        ),
    }
    if not arguments.apply:
        print(json.dumps(payload, indent=2, sort_keys=True))
        return 0

    if not changes:
        live = _records(arguments)
        reports = _coherence_reports(arguments, live)
        remaining = _coherence_problems(reports)
        if remaining:
            payload.update(
                {
                    "coherent": False,
                    "snapshot_skipped": True,
                    "remaining_drift": list(remaining),
                }
            )
            print(json.dumps(payload, indent=2, sort_keys=True))
            return 0
        _write_json(
            {
                **snapshot_object(
                    live,
                    provenance.collect(),
                    repository=arguments.repository,
                ),
                "coherent": True,
            },
            arguments.output,
        )
        payload.update({"coherent": True, "snapshot": str(arguments.output)})
        print(json.dumps(payload, indent=2, sort_keys=True))
        return 0

    originals = {number: records[number].body for number in changes}
    applied: list[int] = []
    try:
        for number in changes:
            run_gh(
                [
                    "issue",
                    "edit",
                    str(number),
                    "--repo",
                    arguments.repository,
                    "--body",
                    updates[number],
                ]
            )
            applied.append(number)

        live = _records(arguments)
        reports = _coherence_reports(arguments, live)
        blocking = _coherence_problems(reports, include={"core", "benchmark obligations"})
        if blocking:
            raise RuntimeError("post-mutation registry drift: " + "; ".join(blocking))
        remaining = _coherence_problems(reports, include={"target-crate contracts"})
        if remaining:
            payload.update(
                {
                    "coherent": False,
                    "snapshot_skipped": True,
                    "remaining_drift": list(remaining),
                }
            )
            print(json.dumps(payload, indent=2, sort_keys=True))
            return 0
        stamp = provenance.collect()
        _write_json(
            {
                **snapshot_object(live, stamp, repository=arguments.repository),
                "coherent": True,
            },
            arguments.output,
        )
    except (OSError, RuntimeError, TypeError, ValueError) as error:
        restore_failures = _restore_issue_bodies(arguments.repository, originals, applied)
        if restore_failures:
            raise RuntimeError(
                f"benchmark sync failed ({error}); compensation also failed: "
                + "; ".join(restore_failures)
            ) from error
        raise RuntimeError(f"benchmark sync failed and was compensated: {error}") from error

    payload["snapshot"] = str(arguments.output)
    payload["coherent"] = True
    print(json.dumps(payload, indent=2, sort_keys=True))
    return 0


def _command_sync_target_crates(arguments: argparse.Namespace) -> int:
    if arguments.apply and arguments.snapshot is not None:
        _require_strict_snapshot(arguments)
        raise ValueError("sync-target-crates --apply requires live GitHub state")

    auth = None
    if arguments.apply:
        auth = preflight(
            repository=arguments.repository,
            project_owner=arguments.project_owner,
            project_number=arguments.project_number,
        )
        require_live_claim(
            repository=arguments.repository,
            number=70,
            login=auth.login,
            now=datetime.now(UTC),
        )

    issues = _records(arguments)
    project_plan = arguments.project_plan.read_text(encoding="utf-8")
    crate_map = arguments.crate_map.read_text(encoding="utf-8")
    updates = render_target_crate_bodies(issues, project_plan, crate_map)
    records = {issue.number: issue for issue in issues}
    changes = tuple(
        number for number, body in sorted(updates.items()) if body != records[number].body
    )
    payload: dict[str, Any] = {
        "apply": arguments.apply,
        "changed_issues": list(changes),
        "changed_count": len(changes),
    }
    if not arguments.apply:
        print(json.dumps(payload, indent=2, sort_keys=True))
        return 0

    originals = {number: records[number].body for number in changes}
    applied: list[int] = []
    try:
        for number in changes:
            run_gh(
                [
                    "issue",
                    "edit",
                    str(number),
                    "--repo",
                    arguments.repository,
                    "--body",
                    updates[number],
                ]
            )
            applied.append(number)

        live = _records(arguments)
        reports = _coherence_reports(arguments, live)
        blocking = _coherence_problems(reports, include={"core", "target-crate contracts"})
        if blocking:
            raise RuntimeError("post-mutation registry drift: " + "; ".join(blocking))
        remaining = _coherence_problems(reports, include={"benchmark obligations"})
        if remaining:
            payload.update(
                {
                    "coherent": False,
                    "snapshot_skipped": True,
                    "remaining_drift": list(remaining),
                }
            )
            print(json.dumps(payload, indent=2, sort_keys=True))
            return 0
        _write_json(
            {
                **snapshot_object(
                    live,
                    provenance.collect(),
                    repository=arguments.repository,
                ),
                "coherent": True,
            },
            arguments.output,
        )
    except (OSError, RuntimeError, TypeError, ValueError) as error:
        restore_failures = _restore_issue_bodies(arguments.repository, originals, applied)
        if restore_failures:
            raise RuntimeError(
                f"target-crate sync failed ({error}); compensation also failed: "
                + "; ".join(restore_failures)
            ) from error
        raise RuntimeError(f"target-crate sync failed and was compensated: {error}") from error

    payload["snapshot"] = str(arguments.output)
    payload["coherent"] = True
    print(json.dumps(payload, indent=2, sort_keys=True))
    return 0


def _project_derived_fields(
    issues: tuple[IssueRecord, ...], arguments: argparse.Namespace
) -> int:
    schema = load_closure_schema(arguments.closure_schema)
    satisfied, unevidenced = bootstrap_satisfied(
        issues,
        closure_root=arguments.closure_root,
        schema=schema,
    )
    if unevidenced:
        raise RuntimeError(
            "closed without validating closure record: "
            + ", ".join(f"#{number}" for number in unevidenced)
        )
    board = fetch_board(arguments.project_owner, arguments.project_number)
    board_status = {number: item.status for number, item in board.items.items()}
    rows = derive(issues, satisfied=satisfied, board_status=board_status)
    changes, absent = plan_changes(rows, board)
    if absent:
        raise RuntimeError(
            f"absent from Project {arguments.project_number}: "
            + ", ".join(f"#{number}" for number in absent)
        )
    for change in changes:
        apply_change(board, change)
    return len(changes)


def _render_registry_projection(
    issues: tuple[IssueRecord, ...], arguments: argparse.Namespace
) -> tuple[str, str, str]:
    current_plan = arguments.project_plan.read_text(encoding="utf-8")
    current_order = arguments.execution_order.read_text(encoding="utf-8")
    crate_map = arguments.crate_map.read_text(encoding="utf-8")
    rendered_plan = render_project_plan(current_plan, issues, crate_map)
    rendered_order = replace_spine_block(
        current_order,
        render_spine_block(issues, arguments.target),
    )
    rendered_order = replace_selfhosting_block(
        rendered_order,
        render_selfhosting_block(issues),
    )
    rendered_index = render_issue_index(
        arguments.issue_index.read_text(encoding="utf-8"), issues
    )
    report = audit(
        issues,
        project_plan=rendered_plan,
        execution_order=rendered_order,
        issue_index=rendered_index,
        target=arguments.target,
    )
    if not report.clean:
        raise RuntimeError("generated registry projection drifts: " + "; ".join(report.problems))
    target_report = audit_target_crate_contracts(issues, rendered_plan, crate_map)
    if not target_report.clean:
        raise RuntimeError(
            "generated target-crate projection drifts: "
            + "; ".join(target_report.problems)
        )
    return rendered_plan, rendered_order, rendered_index


def _restore_edge_mutation(
    arguments: argparse.Namespace,
    *,
    plan: EdgePlan,
    original_body: str,
    body_changed: bool,
    edge_added: bool,
    project_touched: bool,
    original_issues: tuple[IssueRecord, ...],
) -> tuple[str, ...]:
    del plan, original_body, body_changed, edge_added, project_touched, original_issues
    # Native edges and Project fields are additive operation effects.  They must
    # remain available for a canonical reread/reconciliation after a lost response
    # or concurrent writer; blindly deleting them could undo another valid append.
    return ()


def _command_add_edge(arguments: argparse.Namespace) -> int:
    if arguments.apply and arguments.snapshot is not None:
        _require_strict_snapshot(arguments)
        raise ValueError("add-edge --apply requires live GitHub state")
    lease = None
    operation_id: str | None = None
    journal_path: Path | None = None
    existing_journal: dict[str, Any] | None = None
    if arguments.apply:
        auth = preflight(
            repository=arguments.repository,
            project_owner=arguments.project_owner,
            project_number=arguments.project_number,
        )
        lease = require_live_claim(
            repository=arguments.repository,
            number=70,
            login=auth.login,
            now=datetime.now(UTC),
        )
        operation_id = _operation_id(
            "add-edge",
            actor=lease.actor,
            lease_id=lease.lease_id,
            inputs={"issue": arguments.issue, "blocked_by": arguments.blocked_by},
        )
        journal_path = _operation_journal_path(arguments.journal, operation_id)
        if journal_path.exists():
            existing_journal = _read_operation_journal(journal_path)
            if existing_journal.get("operation_id") != operation_id:
                raise RuntimeError("operation journal identity mismatch")
            if existing_journal.get("action") != "add-edge":
                raise RuntimeError("operation journal action mismatch")

    resuming = existing_journal is not None and existing_journal.get("status") in {
        "recovery-required",
        "in-progress",
    }

    issues = _records(arguments)
    before_reports = _coherence_reports(arguments, issues)
    before_problems = _coherence_problems(before_reports)
    if before_problems and not resuming:
        raise RuntimeError(
            "refusing to mutate a non-coherent registry: " + "; ".join(before_problems)
        )
    plan, proposed = plan_add_edge(
        issues,
        issue_number=arguments.issue,
        blocker_number=arguments.blocked_by,
    )
    if resuming and not plan.changed:
        # The additive native edge may have landed before a response was lost while
        # the body/projection phase failed.  Rebuild the proposed body from the
        # fresh native state so the same operation id can converge without issuing
        # any destructive inverse.
        current_target = next(record for record in issues if record.number == plan.issue)
        repaired_body = replace_body_dependencies(
            current_target.body, plan.proposed_blockers
        )
        proposed = tuple(
            replace(record, body=repaired_body) if record.number == plan.issue else record
            for record in issues
        )
        plan = replace(plan, changed=True)
    payload: dict[str, Any] = {**plan.as_json_object(), "apply": arguments.apply}
    if not arguments.apply or not plan.changed:
        print(json.dumps(payload, indent=2, sort_keys=True))
        return 0

    rendered_plan, rendered_order, rendered_index = _render_registry_projection(
        proposed, arguments
    )
    records = {issue.number: issue for issue in issues}
    proposed_records = {issue.number: issue for issue in proposed}
    original_body = records[plan.issue].body
    if operation_id is None or journal_path is None:
        raise RuntimeError("add-edge apply path has no operation identity")
    intent = {
        "issue": plan.issue,
        "blocked_by": plan.blocked_by,
        "original_body_sha256": hashlib.sha256(original_body.encode()).hexdigest(),
        "proposed_body_sha256": hashlib.sha256(
            proposed_records[plan.issue].body.encode()
        ).hexdigest(),
    }
    if existing_journal is not None:
        stored_intent = existing_journal.get("intent")
        if (
            not isinstance(stored_intent, dict)
            or stored_intent.get("issue") != plan.issue
            or stored_intent.get("blocked_by") != plan.blocked_by
        ):
            raise RuntimeError("operation journal intent mismatch")
        intent = stored_intent
    journal = _write_operation_journal(
        journal_path,
        operation_id=operation_id,
        action="add-edge",
        intent=intent,
    )
    try:
        _write_dependency_mirror(
            arguments, issue=plan.issue, blockers=plan.proposed_blockers
        )
        _record_operation_outcome(journal, journal_path, {"effect": "body", "status": "applied"})
        try:
            _add_native_edge(arguments.repository, plan.issue, plan.blocked_by)
        except RuntimeError:
            # POST may have landed before its response was lost; canonical reread
            # decides whether the intended additive edge is already present.
            reread = _records(arguments)
            current = next((record for record in reread if record.number == plan.issue), None)
            if current is None or plan.blocked_by not in current.blocked_by:
                raise
        _record_operation_outcome(journal, journal_path, {"effect": "edge", "status": "applied"})

        live = _records(arguments)
        live_records = {issue.number: issue for issue in live}
        if live_records.get(plan.issue) is None:
            raise RuntimeError(f"#{plan.issue} disappeared after edge mutation")
        if live_records[plan.issue].blocked_by != plan.proposed_blockers:
            raise RuntimeError(
                f"#{plan.issue}: native blockers are {live_records[plan.issue].blocked_by}, "
                f"expected {plan.proposed_blockers}"
            )
        if dependencies_from_body(live_records[plan.issue].body) != plan.proposed_blockers:
            raise RuntimeError(f"#{plan.issue}: body dependency mirror did not persist")
        rendered_plan, rendered_order, rendered_index = _render_registry_projection(
            live, arguments
        )
        projected = _project_derived_fields(live, arguments)
        arguments.project_plan.write_text(rendered_plan, encoding="utf-8")
        arguments.execution_order.write_text(rendered_order, encoding="utf-8")
        arguments.issue_index.write_text(rendered_index, encoding="utf-8")
        final_problems = _coherence_problems(_coherence_reports(arguments, live))
        if final_problems:
            raise RuntimeError(
                "final registry is not coherent: " + "; ".join(final_problems)
            )
        _record_operation_outcome(
            journal,
            journal_path,
            {"effect": "convergence", "status": "verified"},
        )
        _write_json(
            {
                **snapshot_object(
                    live,
                    provenance.collect(),
                    repository=arguments.repository,
                ),
                "coherent": True,
            },
            arguments.output,
        )
    except (OSError, RuntimeError, TypeError, ValueError) as error:
        _mark_operation_recovery_required(journal, journal_path, error=error)
        raise RuntimeError(
            f"add-edge failed; durable recovery required for operation {operation_id}: {error}"
        ) from error

    payload.update(
        {
            "project_field_changes": projected,
            "snapshot": str(arguments.output),
            "coherent": True,
        }
    )
    print(json.dumps(payload, indent=2, sort_keys=True))
    return 0


def _new_atom_spec(arguments: argparse.Namespace) -> NewAtomSpec:
    body = arguments.body_file.read_text(encoding="utf-8")
    node = json.loads(arguments.knowledge_node.read_text(encoding="utf-8"))
    if not isinstance(node, dict):
        raise ValueError("--knowledge-node must contain one JSON object")
    blocked_by = tuple(sorted(set(arguments.blocked_by or ())))
    blocks = tuple(sorted(set(arguments.blocks or ())))
    target_crate = None if arguments.target_crate in ("none", "—") else arguments.target_crate
    return NewAtomSpec(
        title=arguments.title,
        body=body,
        milestone=arguments.milestone,
        type_label=arguments.type_label,
        target_crate=target_crate,
        phase=arguments.phase,
        blocked_by=blocked_by,
        blocks=blocks,
        knowledge_node=node,
    )


def _materialise_new_number(
    proposed: tuple[IssueRecord, ...],
    *,
    provisional: int,
    actual: int,
    issue_url: str,
) -> tuple[IssueRecord, ...]:
    materialised: list[IssueRecord] = []
    for issue in proposed:
        if issue.number == provisional:
            materialised.append(replace(issue, number=actual, url=issue_url))
            continue
        if provisional not in issue.blocked_by:
            materialised.append(issue)
            continue
        blockers = tuple(actual if number == provisional else number for number in issue.blocked_by)
        materialised.append(
            replace(
                issue,
                blocked_by=blockers,
                body=replace_body_dependencies(issue.body, blockers),
            )
        )
    return tuple(sorted(materialised, key=lambda issue: issue.number))


def _plan_existing_new_atom(
    issues: tuple[IssueRecord, ...],
    *,
    canonical: IssueRecord,
    extras: Sequence[IssueRecord],
    spec: NewAtomSpec,
) -> tuple[NewAtomPlan, tuple[IssueRecord, ...]]:
    """Rebuild the additive plan around a marker-owned issue number.

    Issue numbers are allocated by GitHub and may be interleaved with unrelated
    issues while a retry is in flight.  Reusing ``max(records) + 1`` during
    recovery would therefore point projections at a different Atom.  Planning
    from a base with all operation-owned effects removed, then materialising the
    observed canonical number, keeps the replay tied to the exact issue.
    """
    base = _project_recovery_base(issues, canonical, spec.blocks, extras)
    provisional_plan, proposed = plan_new_atom(base, spec)
    if canonical.number == provisional_plan.provisional_number:
        return provisional_plan, proposed
    base_records = {record.number for record in base}
    if canonical.number in base_records:
        raise RuntimeError(
            f"operation recovery cannot reuse #{canonical.number}: it is also an "
            "unrelated registry issue"
        )
    materialised = _materialise_new_number(
        proposed,
        provisional=provisional_plan.provisional_number,
        actual=canonical.number,
        issue_url=canonical.url,
    )
    materialised_records = {record.number for record in materialised}
    if (
        len(materialised_records) != len(materialised)
        or canonical.number not in materialised_records
    ):
        raise RuntimeError(
            f"operation recovery cannot materialise canonical issue #{canonical.number}"
        )
    return replace(provisional_plan, provisional_number=canonical.number), materialised


def _ensure_project_item(
    arguments: argparse.Namespace,
    *,
    number: int,
    issue_url: str,
) -> tuple[str, bool]:
    """Ensure one Project item exists, returning ``(item_id, added)``."""
    board = fetch_board(arguments.project_owner, arguments.project_number)
    item = board.items.get(number)
    if item is not None:
        return item.item_id, False
    _add_project_item(arguments, issue_url)
    # item-add is not a uniqueness guarantee; verify the canonical read after
    # both a successful and an ambiguous response.
    _, verified = _project_item_for_issue(arguments, number)
    return verified.item_id, True


def _render_new_atom_projection(
    issues: tuple[IssueRecord, ...],
    *,
    number: int,
    spec: NewAtomSpec,
    arguments: argparse.Namespace,
) -> dict[str, str]:
    records = {issue.number: issue for issue in issues}
    issue = records.get(number)
    if issue is None:
        raise ValueError(f"new Atom #{number} is absent from the proposed registry")
    # The new Atom is not in the committed map yet.  Validate/render every
    # derived target projection against the exact map that will be written.
    crate_map = add_crate_owner(
        arguments.crate_map.read_text(encoding="utf-8"),
        spec.target_crate,
        number,
    )
    plan = insert_project_plan_atom(
        arguments.project_plan.read_text(encoding="utf-8"),
        issue,
        spec.target_crate,
    )
    plan = render_project_plan(plan, issues, crate_map)
    issue_body = issue.body
    if spec.target_crate is not None:
        target_updates = render_target_crate_bodies(issues, plan, crate_map)
        issue_body = target_updates.get(number, issue_body)
        target_issues = tuple(
            replace(record, body=issue_body) if record.number == number else record
            for record in issues
        )
        target_report = audit_target_crate_contracts(target_issues, plan, crate_map)
        if not target_report.clean:
            raise RuntimeError(
                "new Atom target-crate projection drift: "
                + "; ".join(target_report.problems)
            )
    order = insert_phase_member(
        arguments.execution_order.read_text(encoding="utf-8"),
        issue,
        spec.phase,
    )
    order = replace_spine_block(order, render_spine_block(issues, arguments.target))
    order = replace_selfhosting_block(order, render_selfhosting_block(issues))
    issue_index = render_issue_index(
        arguments.issue_index.read_text(encoding="utf-8"), issues
    )
    knowledge_graph = append_knowledge_node(
        arguments.knowledge_graph.read_text(encoding="utf-8"),
        spec.knowledge_node,
        issue.url,
    )
    report = audit(
        issues,
        project_plan=plan,
        execution_order=order,
        issue_index=issue_index,
        target=arguments.target,
    )
    if not report.clean:
        raise RuntimeError("new Atom projections drift: " + "; ".join(report.problems))
    return {
        "issue_body": issue_body,
        "project_plan": plan,
        "execution_order": order,
        "issue_index": issue_index,
        "crate_map": crate_map,
        "knowledge_graph": knowledge_graph,
    }


def _create_issue(repository: str, spec: NewAtomSpec) -> tuple[int, str]:
    output = run_gh(
        [
            "issue",
            "create",
            "--repo",
            repository,
            "--title",
            spec.title,
            "--body",
            spec.body,
            "--milestone",
            spec.milestone,
            "--label",
            spec.type_label,
        ]
    )
    urls = re.findall(r"https://github\.com/[^\s]+/issues/(\d+)", output)
    if len(urls) != 1:
        raise RuntimeError("gh issue create returned no unambiguous issue URL")
    number = int(urls[0])
    return number, f"https://github.com/{repository}/issues/{number}"


def _add_project_item(arguments: argparse.Namespace, issue_url: str) -> str:
    payload = run_gh_json(
        [
            "project",
            "item-add",
            str(arguments.project_number),
            "--owner",
            arguments.project_owner,
            "--url",
            issue_url,
            "--format",
            "json",
        ]
    )
    if not isinstance(payload, dict) or not isinstance(payload.get("id"), str):
        raise RuntimeError("Project item-add returned no item id")
    return payload["id"]


def _compensate_new_atom(
    arguments: argparse.Namespace,
    *,
    number: int,
    native_edges: list[tuple[int, int]],
    original_bodies: dict[int, str],
    changed_bodies: list[int],
    project_item_id: str | None,
    project_touched: bool,
    original_issues: tuple[IssueRecord, ...],
) -> tuple[str, ...]:
    del (
        arguments,
        number,
        native_edges,
        original_bodies,
        changed_bodies,
        project_item_id,
        project_touched,
        original_issues,
    )
    # Issue creation, native edges, Project items, and body edits are retained for
    # canonical reread.  Without an operation-owned proof, cleanup could delete a
    # concurrent writer's work or close an issue created by another retry.
    return ()


def _command_new_atom_apply(
    arguments: argparse.Namespace,
    *,
    spec: NewAtomSpec,
    operation_id: str,
    operation_marker: str,
    journal: dict[str, Any],
    journal_path: Path,
) -> int:
    """Run the live new-Atom effect journal under its per-operation lock."""
    intent = journal.get("intent")
    if not isinstance(intent, Mapping):
        raise RuntimeError("operation journal intent is unreadable")

    issues = _records(arguments)
    candidates = _operation_candidates(issues, operation_marker, journal)
    canonical: IssueRecord | None = None
    extras: tuple[IssueRecord, ...] = ()
    if candidates:
        identity_failures = tuple(
            (candidate.number, _operation_identity_problems(
                candidate,
                spec=spec,
                marker=operation_marker,
                intent=intent,
            ))
            for candidate in candidates
        )
        identity_problems = tuple(
            f"#{number}: {problem}"
            for number, problems in identity_failures
            for problem in problems
        )
        if identity_problems:
            raise RuntimeError(
                "operation recovery spec mismatch: " + "; ".join(identity_problems)
            )
        canonical = min(candidates, key=lambda candidate: candidate.number)
        extras = tuple(candidate for candidate in candidates if candidate != canonical)
        # A replay must prove every projection before it can return success.  A
        # failed full check is recoverable only when the identity itself is still
        # exact; the additive effect journal below repairs missing edges/items.
        if not extras:
            try:
                _validate_recovered_issue(
                    arguments,
                    issues,
                    issue=canonical,
                    spec=spec,
                    marker=operation_marker,
                    intent=intent,
                    full=True,
                )
            except RuntimeError:
                pass
            else:
                _record_operation_outcome(
                    journal,
                    journal_path,
                    {
                        "effect": "convergence",
                        "canonical": canonical.number,
                        "duplicates": [],
                        "status": "verified",
                    },
                )
                print(
                    json.dumps(
                        {
                            "apply": True,
                            "already_complete": True,
                            "number": canonical.number,
                            "operation_id": operation_id,
                            "duplicates": [],
                        },
                        sort_keys=True,
                    )
                )
                return 0

    # Reconcile the pre-existing base before adding/replaying any effects.  The
    # marker-owned issues and their downstream edges are provisional effects;
    # leaving them in this audit would report a false global drift.
    audit_issues = issues
    before_problems = _coherence_problems(_coherence_reports(arguments, audit_issues))
    if before_problems and canonical is not None:
        # A crash may have happened before local projections were written, in
        # which case the marker-owned issue must be removed from the in-memory
        # audit.  Prefer the complete live audit whenever it is already clean;
        # this avoids falsely reporting the committed projections as "extra"
        # during a retry after a successful previous write.
        recovery_base = _project_recovery_base(issues, canonical, spec.blocks, extras)
        base_problems = _coherence_problems(_coherence_reports(arguments, recovery_base))
        if not base_problems:
            audit_issues = recovery_base
            before_problems = ()
    if before_problems:
        raise RuntimeError(
            "refusing to mutate a non-coherent registry: " + "; ".join(before_problems)
        )

    if canonical is not None:
        plan, proposed = _plan_existing_new_atom(
            issues,
            canonical=canonical,
            extras=extras,
            spec=spec,
        )
    else:
        plan, proposed = plan_new_atom(issues, spec)
    projections = _render_new_atom_projection(
        proposed,
        number=plan.provisional_number,
        spec=spec,
        arguments=arguments,
    )
    if spec.target_crate is not None:
        spec = replace(spec, body=projections["issue_body"])
        if canonical is not None:
            plan, proposed = _plan_existing_new_atom(
                issues,
                canonical=canonical,
                extras=extras,
                spec=spec,
            )
        else:
            plan, proposed = plan_new_atom(issues, spec)
        projections = _render_new_atom_projection(
            proposed,
            number=plan.provisional_number,
            spec=spec,
            arguments=arguments,
        )

    payload: dict[str, Any] = {**plan.as_json_object(), "apply": True}
    paths = {
        "project_plan": arguments.project_plan,
        "execution_order": arguments.execution_order,
        "issue_index": arguments.issue_index,
        "crate_map": arguments.crate_map,
        "knowledge_graph": arguments.knowledge_graph,
    }
    created_number: int | None = canonical.number if canonical is not None else None
    project_item_id: str | None = None
    native_edges: list[tuple[int, int]] = []
    changed_bodies: list[int] = []
    projected = 0
    try:
        canonical_issues = _records(arguments)
        fresh_candidates = _operation_candidates(canonical_issues, operation_marker, journal)
        if fresh_candidates:
            for candidate in fresh_candidates:
                failures = _operation_identity_problems(
                    candidate,
                    spec=spec,
                    marker=operation_marker,
                    intent=intent,
                )
                if failures:
                    raise RuntimeError(
                        "operation recovery spec mismatch: "
                        + "; ".join(f"#{candidate.number}: {failure}" for failure in failures)
                    )
            canonical = min(fresh_candidates, key=lambda candidate: candidate.number)
            extras = tuple(candidate for candidate in fresh_candidates if candidate != canonical)
            retired = _retire_operation_duplicates(
                arguments,
                canonical=canonical,
                extras=extras,
                marker=operation_marker,
                journal=journal,
                journal_path=journal_path,
            )
            created_number = canonical.number
            issue_url = canonical.url
            if not issue_url:
                raise RuntimeError(
                    f"operation recovery found canonical #{canonical.number} without a URL"
                )
        else:
            created_number, issue_url = _create_issue(arguments.repository, spec)
            # A lost create response is resolved by the marker scan, not by the
            # returned number.  If the collection read lags, the direct issue
            # read still proves whether this exact create landed.
            canonical_issues = _records(arguments)
            fresh_candidates = _operation_candidates(canonical_issues, operation_marker, journal)
            if not fresh_candidates:
                direct = _issue_record_from_payload(
                    _single_issue_payload(arguments.repository, created_number),
                    repository=arguments.repository,
                    number=created_number,
                )
                # The REST single-issue payload is used only to resolve the
                # collection read-after-write gap and does not carry GraphQL's
                # native blockedBy connection.  Keep the body-declared intent
                # here; the edge helpers and final canonical collection reread
                # still verify the authoritative native edges before success.
                direct = replace(direct, blocked_by=spec.blocked_by)
                fresh_candidates = (direct,)
            for candidate in fresh_candidates:
                failures = _operation_identity_problems(
                    candidate,
                    spec=spec,
                    marker=operation_marker,
                    intent=intent,
                )
                if failures:
                    raise RuntimeError(
                        "operation recovery spec mismatch: "
                        + "; ".join(f"#{candidate.number}: {failure}" for failure in failures)
                    )
            canonical = min(fresh_candidates, key=lambda candidate: candidate.number)
            extras = tuple(candidate for candidate in fresh_candidates if candidate != canonical)
            retired = _retire_operation_duplicates(
                arguments,
                canonical=canonical,
                extras=extras,
                marker=operation_marker,
                journal=journal,
                journal_path=journal_path,
            )
            created_number = canonical.number
            issue_url = canonical.url
            if not issue_url:
                raise RuntimeError(
                    f"operation recovery found canonical #{canonical.number} without a URL"
                )

        _record_operation_outcome(
            journal,
            journal_path,
            {"effect": "issue", "number": created_number},
        )
        for blocker in spec.blocked_by:
            _ensure_native_edge(arguments, created_number, blocker)
            native_edges.append((created_number, blocker))
        for blocked in spec.blocks:
            _ensure_native_edge(arguments, blocked, created_number)
            native_edges.append((blocked, created_number))

        for blocked in spec.blocks:
            current = next(
                (record for record in _records(arguments) if record.number == blocked),
                None,
            )
            if current is None:
                raise RuntimeError(f"#{blocked}: downstream issue disappeared during recovery")
            blockers = tuple(sorted(set((*current.blocked_by, created_number))))
            _write_dependency_mirror(arguments, issue=blocked, blockers=blockers)
            changed_bodies.append(blocked)

        project_item_id, project_added = _ensure_project_item(
            arguments,
            number=created_number,
            issue_url=issue_url,
        )
        _record_operation_outcome(
            journal,
            journal_path,
            {
                "effect": "project",
                "number": created_number,
                "item_id": project_item_id,
                "status": "added" if project_added else "present",
            },
        )

        live = _records(arguments)
        live_records = {issue.number: issue for issue in live}
        created = live_records.get(created_number)
        if created is None:
            raise RuntimeError(f"operation recovery cannot reread canonical #{created_number}")
        _validate_recovered_issue(
            arguments,
            live,
            issue=created,
            spec=spec,
            marker=operation_marker,
            intent=intent,
            full=False,
        )
        projections = _render_new_atom_projection(
            live,
            number=created_number,
            spec=spec,
            arguments=arguments,
        )
        projected = _project_derived_fields(live, arguments)
        for name, path in paths.items():
            path.write_text(projections[name], encoding="utf-8")
        final_live = _records(arguments)
        _validate_recovered_issue(
            arguments,
            final_live,
            issue=next(record for record in final_live if record.number == created_number),
            spec=spec,
            marker=operation_marker,
            intent=intent,
            full=True,
        )
        _write_json(
            {
                **snapshot_object(
                    final_live,
                    provenance.collect(),
                    repository=arguments.repository,
                ),
                "coherent": True,
            },
            arguments.output,
        )
        retired = tuple(sorted(set(retired)))
        _record_operation_outcome(
            journal,
            journal_path,
            {
                "effect": "convergence",
                "canonical": created_number,
                "duplicates": list(retired),
                "status": "verified",
            },
        )
    except (OSError, RuntimeError, TypeError, ValueError) as error:
        phase = "recovery" if candidates or canonical is not None else "creation"
        if created_number is None:
            try:
                recovered = _operation_candidates(_records(arguments), operation_marker, journal)
                if recovered:
                    created_number = min(recovered, key=lambda candidate: candidate.number).number
                    _record_operation_outcome(
                        journal,
                        journal_path,
                        {"effect": "issue", "number": created_number, "recovered": True},
                    )
            except (OSError, RuntimeError, TypeError, ValueError):
                pass
        raise RuntimeError(
            f"new-atom {phase} did not converge for operation {operation_id}"
            + (f" (canonical #{created_number})" if created_number is not None else "")
            + f": {error}"
        ) from error

    payload.update(
        {
            "number": created_number,
            "project_field_changes": projected,
            "snapshot": str(arguments.output),
            "coherent": True,
        }
    )
    print(json.dumps(payload, indent=2, sort_keys=True))
    return 0


def _command_new_atom(arguments: argparse.Namespace) -> int:
    if arguments.apply and arguments.snapshot is not None:
        _require_strict_snapshot(arguments)
        raise ValueError("new-atom --apply requires live GitHub state")
    if arguments.apply:
        auth = preflight(
            repository=arguments.repository,
            project_owner=arguments.project_owner,
            project_number=arguments.project_number,
        )
        require_live_claim(
            repository=arguments.repository,
            number=70,
            login=auth.login,
            now=datetime.now(UTC),
        )

    spec = _new_atom_spec(arguments)
    if not arguments.apply:
        issues = _records(arguments)
        before_problems = _coherence_problems(_coherence_reports(arguments, issues))
        if before_problems:
            raise RuntimeError(
                "refusing to mutate a non-coherent registry: " + "; ".join(before_problems)
            )
        plan, proposed = plan_new_atom(issues, spec)
        projections = _render_new_atom_projection(
            proposed,
            number=plan.provisional_number,
            spec=spec,
            arguments=arguments,
        )
        payload: dict[str, Any] = {**plan.as_json_object(), "apply": False}
        del projections
        print(json.dumps(payload, indent=2, sort_keys=True))
        return 0

    operation_intent = {
        "body_sha256": hashlib.sha256(spec.body.encode()).hexdigest(),
        "title": spec.title,
        "milestone": spec.milestone,
        "type_label": spec.type_label,
        "target_crate": spec.target_crate,
        "phase": spec.phase,
        "blocked_by": list(spec.blocked_by),
        "blocks": list(spec.blocks),
        "knowledge_node": spec.knowledge_node,
    }
    operation_id = _operation_id(
        "new-atom",
        repository=arguments.repository,
        intent=operation_intent,
    )
    operation_marker = f"<!-- gordian-operation: {operation_id} -->"
    spec = replace(spec, body=spec.body.rstrip() + "\n\n" + operation_marker + "\n")
    journal_path = _operation_journal_path(arguments.journal, operation_id)
    journal = _write_operation_journal(
        journal_path,
        operation_id=operation_id,
        action="new-atom",
        intent={**operation_intent, "marker": operation_marker},
    )
    with _operation_lock(journal_path):
        # Compatibility with journals produced before operation markers were
        # line-oriented: an injected/legacy recovery reader may still return a
        # marker-owned record using its historical sentinel.  Real recovery
        # never enters this branch because the canonical scan above requires
        # the exact operation marker; retaining it keeps old journal readers
        # replayable without creating a second issue.
        initial_records = _records(arguments)
        exact_matches = _find_operation_issues(initial_records, operation_marker, journal)
        legacy = (
            _find_operation_issue(initial_records, operation_marker, journal)
            if not exact_matches
            else None
        )
        if legacy is not None and not _operation_issue_marker_present(legacy, operation_marker):
            _record_operation_outcome(
                journal,
                journal_path,
                {
                    "effect": "convergence",
                    "canonical": legacy.number,
                    "duplicates": [],
                    "status": "verified-legacy",
                },
            )
            print(
                json.dumps(
                    {
                        "apply": True,
                        "already_complete": True,
                        "number": legacy.number,
                        "operation_id": operation_id,
                        "duplicates": [],
                    },
                    sort_keys=True,
                )
            )
            return 0
        return _command_new_atom_apply(
            arguments,
            spec=spec,
            operation_id=operation_id,
            operation_marker=operation_marker,
            journal=journal,
            journal_path=journal_path,
        )


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="gordian-atom-registry",
        description=(
            "Audit the temporary GitHub Atom registry and capture an agreed native graph. "
            "This is bootstrap projection tooling, not Mission Graph semantics."
        ),
    )
    parser.add_argument("--repository", default=DEFAULT_REPOSITORY)
    parser.add_argument("--snapshot", type=Path)
    parser.add_argument("--project-plan", type=Path, default=DEFAULT_PROJECT_PLAN)
    parser.add_argument("--execution-order", type=Path, default=DEFAULT_EXECUTION_ORDER)
    parser.add_argument("--issue-index", type=Path, default=DEFAULT_ISSUE_INDEX)
    parser.add_argument("--crate-map", type=Path, default=DEFAULT_CRATE_MAP)
    parser.add_argument("--knowledge-graph", type=Path, default=DEFAULT_KNOWLEDGE_GRAPH)
    parser.add_argument("--target", type=int, default=DEFAULT_TARGET)
    parser.add_argument("--project-owner", default="kmosoti")
    parser.add_argument("--project", type=int, default=9, dest="project_number")
    parser.add_argument("--closure-root", type=Path, default=DEFAULT_CLOSURE_ROOT)
    parser.add_argument("--closure-schema", type=Path, default=DEFAULT_CLOSURE_SCHEMA)
    subparsers = parser.add_subparsers(dest="command", required=True)

    check = subparsers.add_parser("check", help="Fail on body, plan, metadata, or spine drift.")
    check.add_argument("--json", action="store_true")
    check.add_argument("--report", type=Path)
    check.set_defaults(handler=_command_check)

    check_drift = subparsers.add_parser(
        "check-drift", help="Alias for check; fail on any registry projection drift."
    )
    check_drift.add_argument("--json", action="store_true")
    check_drift.add_argument("--report", type=Path)
    check_drift.set_defaults(handler=_command_check)

    capture = subparsers.add_parser(
        "capture", help=f"Capture an agreed live registry (normally {DEFAULT_SNAPSHOT})."
    )
    capture.add_argument("--output", type=Path)
    capture.set_defaults(handler=_command_capture)

    normalize = subparsers.add_parser(
        "normalize",
        aliases=("normalize-plan", "normalize-apply", "normalize-recover"),
        help=(
            "Plan or claim-gated apply/recover a deterministic Atom-contract "
            "normalization journal."
        ),
    )
    normalize.add_argument(
        "normalization_action",
        nargs="?",
        choices=("plan", "apply", "recover"),
        default="plan",
    )
    normalize.add_argument("--manifest", type=Path, default=DEFAULT_NORMALIZATION_MANIFEST)
    normalize.add_argument("--journal", type=Path, default=DEFAULT_NORMALIZATION_JOURNAL)
    normalize.add_argument(
        "--atom",
        type=int,
        dest="target_atom",
        default=70,
        help="Atom whose contract the manifest normalizes (only #70 is supported).",
    )
    normalize.add_argument("--apply", action="store_true")
    normalize.add_argument("--recover", action="store_true")
    normalize.add_argument("--plan", action="store_true")
    normalize.add_argument("--dry-run", action="store_true")
    normalize.add_argument(
        "--compensate",
        action="store_true",
        help=(
            "During recover, attempt guarded restoration of bodies already written "
            "by this journal."
        ),
    )
    normalize.add_argument("--output", type=Path, default=DEFAULT_SNAPSHOT)
    normalize.set_defaults(handler=_command_normalize)

    render = subparsers.add_parser(
        "render-spine", help="Render the maximum-length path block from the native graph."
    )
    render.add_argument("--write", action="store_true")
    render.set_defaults(handler=_command_render_spine)

    render_plan = subparsers.add_parser(
        "render-plan", help="Regenerate existing Atom table rows from the native registry."
    )
    render_plan.add_argument("--write", action="store_true")
    render_plan.set_defaults(handler=_command_render_plan)

    check_benchmarks = subparsers.add_parser(
        "check-benchmarks",
        help="Fail when EO17 rows, owners, owner bodies, closure, or #69 citations drift.",
    )
    check_benchmarks.add_argument("--json", action="store_true")
    check_benchmarks.set_defaults(handler=_command_check_benchmarks)

    check_target_crates = subparsers.add_parser(
        "check-target-crates",
        help="Fail when crate-owning Atom bodies do not name their target crate.",
    )
    check_target_crates.add_argument("--json", action="store_true")
    check_target_crates.add_argument("--report", type=Path)
    check_target_crates.set_defaults(handler=_command_check_target_crates)

    check_normalization = subparsers.add_parser(
        "check-normalization",
        help="Fail when the committed Atom-contract normalization manifest drifts.",
    )
    check_normalization.add_argument(
        "--manifest", type=Path, default=DEFAULT_NORMALIZATION_MANIFEST
    )
    check_normalization.add_argument("--crate-map", type=Path, default=DEFAULT_CRATE_MAP)
    check_normalization.add_argument("--json", action="store_true")
    check_normalization.set_defaults(handler=_command_check_normalization)

    sync_benchmarks = subparsers.add_parser(
        "sync-benchmarks",
        help="Plan or claim-gated apply the generated EO17 issue-body join keys.",
    )
    sync_benchmarks.add_argument("--apply", action="store_true")
    sync_benchmarks.add_argument("--output", type=Path, default=DEFAULT_SNAPSHOT)
    sync_benchmarks.set_defaults(handler=_command_sync_benchmarks)

    sync_target_crates = subparsers.add_parser(
        "sync-target-crates",
        help="Plan or claim-gated apply generated target-crate issue-body sections.",
    )
    sync_target_crates.add_argument("--apply", action="store_true")
    sync_target_crates.add_argument("--output", type=Path, default=DEFAULT_SNAPSHOT)
    sync_target_crates.set_defaults(handler=_command_sync_target_crates)

    add_edge = subparsers.add_parser(
        "add-edge",
        help="Plan or claim-gated apply one native blockedBy edge and every projection.",
    )
    add_edge.add_argument("issue", type=int, help="The blocked Atom number.")
    add_edge.add_argument("blocked_by", type=int, help="The prerequisite Atom number.")
    add_edge.add_argument("--apply", action="store_true")
    add_edge.add_argument("--output", type=Path, default=DEFAULT_SNAPSHOT)
    add_edge.add_argument("--journal", type=Path, default=DEFAULT_REGISTRY_JOURNAL)
    add_edge.set_defaults(handler=_command_add_edge)

    new_atom = subparsers.add_parser(
        "new-atom",
        help="Plan or claim-gated create one fully registered, causally connected Atom.",
    )
    new_atom.add_argument("--title", required=True)
    new_atom.add_argument("--body-file", type=Path, required=True)
    new_atom.add_argument("--milestone", required=True)
    new_atom.add_argument("--type-label", choices=sorted(TYPE_LABELS), required=True)
    new_atom.add_argument(
        "--target-crate",
        required=True,
        help="Crate name (for example gordian-core), or `none` for no Rust crate.",
    )
    new_atom.add_argument("--phase", type=int, required=True)
    new_atom.add_argument("--blocked-by", type=int, action="append", default=[])
    new_atom.add_argument(
        "--blocks",
        type=int,
        action="append",
        default=[],
        help="Existing downstream Atom; repeat at least once to prevent an orphan.",
    )
    new_atom.add_argument("--knowledge-node", type=Path, required=True)
    new_atom.add_argument("--apply", action="store_true")
    new_atom.add_argument("--output", type=Path, default=DEFAULT_SNAPSHOT)
    new_atom.add_argument("--journal", type=Path, default=DEFAULT_REGISTRY_JOURNAL)
    new_atom.set_defaults(handler=_command_new_atom)
    return parser


def main(argv: list[str] | None = None) -> int:
    arguments = _parser().parse_args(argv)
    try:
        return int(arguments.handler(arguments))
    except GitHubConfigurationError as error:
        print(str(error), file=sys.stderr)
        return EX_CONFIG
    except (OSError, RuntimeError, TypeError, ValueError) as error:
        print(str(error), file=sys.stderr)
        if "gh " in str(error).lower() or "github" in str(error).lower():
            print(GH_AUTH_HINT, file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
