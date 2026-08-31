"""Deterministic, claim-gated normalization of live Atom contracts.

The temporary GitHub registry has two different kinds of state that a repair may
touch: issue bodies and native ``blockedBy`` edges.  This module keeps the repair
small and explicit:

* a committed JSON manifest describes body transforms, additive edge intents, and
  additive labels with exact title/state preconditions;
* a plan records the exact body digests observed while making that manifest;
* a local journal is atomically persisted before the first external effect and after
  every state transition;
* a body write is allowed only when a fresh read still has the planned old digest;
* an edge is only ever added, and is considered complete only after a fresh read
  observes it; and
* a label is only ever added, and closed-duplicate label verification uses a
  single-issue read that is not filtered from the registry collection; and
* recovery rolls the plan forward.  It never guesses whether an uncertain edge
  belongs to this run and never deletes one.

GitHub issue PATCH has no general conditional-write contract for this workflow.  The
old-body digest is therefore an application-level precondition enforced by a fresh
read immediately before the write, followed by a fresh verification read.  The
precondition is deliberately not represented as an HTTP header or server-side CAS.

This is orchestration only.  It does not define Mission Graph semantics, readiness,
satisfaction, or authority.  The caller supplies the already-authorized #70 writer
and the live fetch callback; the command layer performs the normal preflight and
claim check before invoking this module.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import tempfile
from collections.abc import Callable, Mapping, Sequence
from contextlib import suppress
from dataclasses import asdict, dataclass, replace
from pathlib import Path
from typing import Any

from .derive_status import IssueRecord, topological_order
from .gh import run_gh, run_gh_json

NORMALIZATION_FORMAT = "gordian-atom-contract-normalization-v1"
JOURNAL_FORMAT = "gordian-atom-contract-normalization-journal-v1"
DEFAULT_MANIFEST = Path("docs/implementation/atom-contract-normalization.json")
DEFAULT_JOURNAL = Path("artifacts/atoms/normalization-journal.json")
DEFAULT_ATOM = 70
DEFAULT_REPOSITORY = "kmosoti/gordian"
CANONICAL_ATOM_NUMBERS = tuple(range(1, 78))
INTEGRATION_VERIFIER_IDS = (
    "rust-check",
    "kg-audit",
    "formal",
    "python",
    "spec-consistency",
)
CANONICAL_CLOSURE_TEMPLATE = "canonical-v1"
VERIFIER_DECLARATION_BEGIN = "<!-- BEGIN GENERATED: ATOM ACCEPTANCE VERIFIER -->"
VERIFIER_DECLARATION_END = "<!-- END GENERATED: ATOM ACCEPTANCE VERIFIER -->"


def canonical_closure_intent(
    atom_specific_verifier_ids: Sequence[str] = (),
) -> dict[str, Any]:
    """Return the compact declarative intent for the shared Closure contract.

    The manifest records this object once per Atom rather than copying a
    rendered Markdown body.  The renderer below derives any Atom-specific
    verifier IDs from that issue's Verification section at plan time.
    """
    return {
        "kind": "canonical",
        "atom": "issue",
        "runbook_sections": [1, 2, "6.6"],
        "verifier_source": "verification-section",
        "atom_specific_verifier_ids": sorted(set(atom_specific_verifier_ids)),
        "record_path": "artifacts/atoms/<N>/closure.json",
        "verifier_log_path": "artifacts/atoms/<N>/verifiers/<verifier_id>.log",
    }

_DEPENDENCIES_HEADING = re.compile(r"^## Dependencies[ \t]*$", re.MULTILINE)
_NEXT_HEADING = re.compile(r"^## [^\n]+$", re.MULTILINE)
_NONE = frozenset(("", "none", "—", "-"))


class NormalizationError(RuntimeError):
    """The normalization plan or its durable state cannot be advanced safely."""


class NormalizationConflict(NormalizationError):
    """A fresh read did not satisfy an exact body or edge precondition."""


def _native_blockers(issue: IssueRecord) -> tuple[int, ...]:
    """Return the native edge set; the Markdown dependency mirror is ignored."""
    blockers = tuple(issue.blocked_by)
    if any(
        isinstance(blocker, bool) or not isinstance(blocker, int) or blocker < 1
        for blocker in blockers
    ):
        raise NormalizationError(
            f"#{issue.number}: native blockers must be positive integers"
        )
    if len(set(blockers)) != len(blockers):
        raise NormalizationError(f"#{issue.number}: native blocker list contains duplicates")
    if issue.number in blockers:
        raise NormalizationError(f"#{issue.number}: native blocker list contains a self-edge")
    return tuple(sorted(blockers))


@dataclass(frozen=True, slots=True)
class BodyReplacement:
    """One exact, declarative string replacement in an issue body."""

    old: str
    new: str
    count: int = 1

    def as_json_object(self) -> dict[str, Any]:
        return asdict(self)


@dataclass(frozen=True, slots=True)
class NormalizationTransform:
    """The complete deterministic transform for one issue."""

    issue: int
    body: str | None = None
    replacements: tuple[BodyReplacement, ...] = ()
    target_crates: tuple[str, ...] | None = None
    target_crates_set: bool = False
    closure_wording: str | None = None
    closure_wording_set: bool = False
    closure_template: str | None = None
    closure_verifier_ids: tuple[str, ...] = ()
    add_blocked_by: tuple[int, ...] = ()
    add_labels: tuple[str, ...] = ()
    expected_title: str | None = None
    expected_state: str | None = None

    def as_json_object(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "issue": self.issue,
            "body": self.body,
            "replacements": [replacement.as_json_object() for replacement in self.replacements],
            "add_blocked_by": list(self.add_blocked_by),
            "add_labels": list(self.add_labels),
            "expected_title": self.expected_title,
            "expected_state": self.expected_state,
        }
        if self.target_crates_set:
            result["target_crates"] = list(self.target_crates or ())
        else:
            result["target_crates"] = None
        if self.closure_template is not None:
            result["closure_wording"] = canonical_closure_intent(self.closure_verifier_ids)
        elif self.closure_wording_set:
            result["closure_wording"] = self.closure_wording or ""
        else:
            result["closure_wording"] = None
        return result


@dataclass(frozen=True, slots=True)
class NormalizationManifest:
    """A validated, canonical manifest loaded from a committed JSON file."""

    atom: int
    repository: str
    transforms: tuple[NormalizationTransform, ...]
    original_plan_sha256: str
    description: str = ""

    @property
    def format(self) -> str:
        return NORMALIZATION_FORMAT

    def as_json_object(self) -> dict[str, Any]:
        return {
            "format": self.format,
            "atom": self.atom,
            "repository": self.repository,
            "original_plan_sha256": self.original_plan_sha256,
            "description": self.description,
            "transforms": [transform.as_json_object() for transform in self.transforms],
        }

    @property
    def digest(self) -> str:
        return manifest_digest(self.as_json_object())


@dataclass(frozen=True, slots=True)
class BodyOperation:
    issue: int
    old_body: str
    old_digest: str
    proposed_body: str
    proposed_digest: str
    changed: bool

    def as_json_object(self) -> dict[str, Any]:
        # Keep the descriptive aliases alongside the short internal names.  The
        # aliases make it unambiguous that these are SHA-256 preconditions for
        # the exact body bytes, rather than a normalized-content digest.
        return {
            **asdict(self),
            "old_body_sha256": self.old_digest,
            "proposed_body_sha256": self.proposed_digest,
        }


@dataclass(frozen=True, slots=True)
class EdgeOperation:
    issue: int
    blocked_by: int
    was_present: bool
    expected_blockers: tuple[int, ...]

    def as_json_object(self) -> dict[str, Any]:
        return {
            "issue": self.issue,
            "blocked_by": self.blocked_by,
            "intended": True,
            "was_present": self.was_present,
            "expected_blockers": list(self.expected_blockers),
        }


@dataclass(frozen=True, slots=True)
class LabelOperation:
    """One exact, additive issue-label intent with identity preconditions."""

    issue: int
    label: str
    expected_title: str
    expected_state: str
    was_present: bool

    def as_json_object(self) -> dict[str, Any]:
        return {
            "issue": self.issue,
            "label": self.label,
            "intended": True,
            "expected_title": self.expected_title,
            "expected_state": self.expected_state,
            "was_present": self.was_present,
        }


@dataclass(frozen=True, slots=True)
class NormalizationPlan:
    """A deterministic plan bound to one manifest and one initial live read."""

    atom: int
    repository: str
    manifest_digest: str
    original_plan_sha256: str
    bodies: tuple[BodyOperation, ...]
    edges: tuple[EdgeOperation, ...]
    labels: tuple[LabelOperation, ...] = ()

    @property
    def changed_bodies(self) -> tuple[BodyOperation, ...]:
        return tuple(operation for operation in self.bodies if operation.changed)

    @property
    def changed_edges(self) -> tuple[EdgeOperation, ...]:
        return tuple(operation for operation in self.edges if not operation.was_present)

    @property
    def changed_labels(self) -> tuple[LabelOperation, ...]:
        return tuple(operation for operation in self.labels if not operation.was_present)

    def as_json_object(self) -> dict[str, Any]:
        operations: list[dict[str, Any]] = []
        operations.extend(
            {"kind": "body", **operation.as_json_object()}
            for operation in self.bodies
            if operation.changed
        )
        operations.extend(
            {"kind": "edge", **operation.as_json_object()}
            for operation in self.edges
            if not operation.was_present
        )
        operations.extend(
            {"kind": "label", **operation.as_json_object()}
            for operation in self.labels
            if not operation.was_present
        )
        return {
            "record_format": NORMALIZATION_FORMAT,
            "atom": self.atom,
            "repository": self.repository,
            "manifest_sha256": self.manifest_digest,
            "original_plan_sha256": self.original_plan_sha256,
            "body_operations": [operation.as_json_object() for operation in self.bodies],
            "edge_operations": [operation.as_json_object() for operation in self.edges],
            "label_operations": [operation.as_json_object() for operation in self.labels],
            "operations": operations,
            "changed_body_count": len(self.changed_bodies),
            "changed_edge_count": len(self.changed_edges),
            "changed_label_count": len(self.changed_labels),
        }


@dataclass(frozen=True, slots=True)
class NormalizationResult:
    """Stable summary returned after advancing a journal."""

    journal: dict[str, Any]
    snapshot: str | None = None

    def as_json_object(self) -> dict[str, Any]:
        return {
            "journal": self.journal,
            "snapshot": self.snapshot,
        }


def validate_manifest_coverage(
    manifest: NormalizationManifest,
    issue_numbers: Sequence[int],
    *,
    required_numbers: Sequence[int] = CANONICAL_ATOM_NUMBERS,
    allowed_extra_numbers: Sequence[int] = (78, 79),
) -> None:
    """Reject a full-registry normalization manifest that is empty or partial.

    Small fixture plans remain useful to the journal unit tests, so coverage is
    checked explicitly by callers that have a full registry read.  A production
    full read must carry one transform for every canonical Atom, with target and
    Closure intent explicitly present; label-only transforms for closed duplicate
    issues are the only permitted extra transforms.
    """
    observed_issues = {int(number) for number in issue_numbers}
    required = {int(number) for number in required_numbers}
    transform_map = {transform.issue: transform for transform in manifest.transforms}
    if not required.issubset(observed_issues):
        return
    missing = sorted(required - set(transform_map))
    unexpected = sorted(set(transform_map) - required - set(allowed_extra_numbers))
    if missing:
        raise NormalizationError(
            "normalization manifest omits canonical Atoms: "
            + ", ".join(f"#{number}" for number in missing)
        )
    if unexpected:
        raise NormalizationError(
            "normalization manifest names unexpected transforms: "
            + ", ".join(f"#{number}" for number in unexpected)
        )
    incomplete = sorted(
        number
        for number in required
        if not transform_map[number].target_crates_set
        or not transform_map[number].closure_wording_set
        or (
            transform_map[number].closure_template is None
            and not (transform_map[number].closure_wording or "").strip()
        )
    )
    if incomplete:
        raise NormalizationError(
            "normalization manifest lacks explicit target/Closure intent for: "
            + ", ".join(f"#{number}" for number in incomplete)
        )


def canonical_closure_wording(
    atom: int, atom_specific_verifier_ids: Sequence[str] = ()
) -> str:
    """Render the one Closure contract shared by every bootstrap Atom.

    The generic verifier commands live in the runbook and landing protocol.  An
    issue Closure names their five stable verifier identities and their canonical
    log locations, then admits only IDs explicitly declared by that issue's
    ``## Verification`` section.  Keeping this renderer deterministic makes the
    committed manifest auditable without copying command snippets into 77 bodies.
    """
    if isinstance(atom, bool) or int(atom) <= 0:
        raise ValueError("atom must be a positive integer")
    specific = tuple(sorted({str(value).strip() for value in atom_specific_verifier_ids}))
    if any(
        not value
        or value in INTEGRATION_VERIFIER_IDS
        or not re.fullmatch(r"[a-z0-9][a-z0-9._-]*", value)
        for value in specific
    ):
        raise ValueError("atom-specific verifier IDs must be safe and non-generic")
    lines = [
        "Closure is the loop defined by [`docs/implementation/agent-runbook.md`]"
        "(docs/implementation/agent-runbook.md) sections 1, 2, and 6.6.",
        "The coordinator writes `artifacts/atoms/"
        f"{int(atom)}/closure.json` after admission in its own bookkeeping change; the record "
        "must validate against `artifacts/schema/closure-record.schema.json`.",
        "The required verifier set is the five project integration verifiers below, plus only "
        "the Atom-specific verifier IDs declared in this issue's `## Verification` section.",
        "",
        "Required verifier logs:",
    ]
    lines.extend(
        f"- `verifier:{verifier}` — "
        f"`artifacts/atoms/{int(atom)}/verifiers/{verifier}.log`"
        for verifier in INTEGRATION_VERIFIER_IDS
    )
    lines.extend(
        f"- `verifier:{verifier}` — "
        f"`artifacts/atoms/{int(atom)}/verifiers/{verifier}.log`"
        for verifier in specific
    )
    lines.append(
        "No generic integration command is repeated in this section; verifier execution and "
        "Atom-specific commands are defined by the referenced contracts."
    )
    return "\n".join(lines)


def normalize_body(body: str) -> str:
    """Use the repository's canonical LF body representation."""
    if not isinstance(body, str):
        raise TypeError("issue body must be text")
    return body.replace("\r\n", "\n").replace("\r", "\n")


def body_digest(body: str) -> str:
    """Return the exact UTF-8 digest of the body bytes that were read or written."""
    if not isinstance(body, str):
        raise TypeError("issue body must be text")
    return hashlib.sha256(body.encode("utf-8")).hexdigest()


def _canonical_json(payload: Any) -> bytes:
    return json.dumps(
        payload,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def manifest_digest(payload: Mapping[str, Any] | NormalizationManifest) -> str:
    """Hash canonical manifest bytes, not formatting or filesystem metadata."""
    if isinstance(payload, NormalizationManifest):
        payload = payload.as_json_object()
    return hashlib.sha256(_canonical_json(payload)).hexdigest()


def original_plan_digest(issues: Sequence[IssueRecord]) -> str:
    """Digest the immutable registry inputs used to derive a normalization plan."""
    payload = [
        {
            "number": issue.number,
            "title": issue.title,
            "state": issue.state,
            "body": issue.body,
            "blocked_by": list(issue.blocked_by),
            "labels": list(issue.labels),
            "milestone": issue.milestone,
        }
        for issue in sorted(issues, key=lambda record: record.number)
    ]
    return hashlib.sha256(_canonical_json(payload)).hexdigest()


def operation_plan_digest(payload: Mapping[str, Any] | NormalizationPlan) -> str:
    """Digest immutable body, edge, and label intents, excluding runtime state."""
    if isinstance(payload, NormalizationPlan):
        payload = payload.as_json_object()
    canonical = {
        key: payload.get(key, [])
        for key in ("body_operations", "edge_operations", "label_operations")
    }
    return hashlib.sha256(_canonical_json(canonical)).hexdigest()


def _positive_issue(value: Any, label: str) -> int:
    if isinstance(value, bool):
        raise ValueError(f"{label} must be a positive integer")
    try:
        number = int(value)
    except (TypeError, ValueError) as error:
        raise ValueError(f"{label} must be a positive integer") from error
    if number <= 0:
        raise ValueError(f"{label} must be a positive integer")
    return number


def _string(value: Any, label: str) -> str:
    if not isinstance(value, str):
        raise ValueError(f"{label} must be a string")
    return normalize_body(value)


def _raw_text(value: Any, label: str) -> str:
    if not isinstance(value, str):
        raise ValueError(f"{label} must be a string")
    return value


def _plan_boolean(value: Any, label: str) -> bool:
    if not isinstance(value, bool):
        raise NormalizationError(f"{label} must be boolean")
    return value


def _string_list(value: Any, label: str) -> tuple[str, ...]:
    if value is None:
        return ()
    if isinstance(value, str):
        values: Sequence[Any] = (value,)
    elif isinstance(value, (list, tuple)):
        values = value
    else:
        raise ValueError(f"{label} must be a string or array of strings")
    normalized: set[str] = set()
    for item in values:
        text = _string(item, label)
        if text.strip().lower() in _NONE:
            continue
        path = text.strip()
        if path.startswith("crates/"):
            path = path.removeprefix("crates/")
        if not re.fullmatch(r"gordian-[a-z0-9]+(?:-[a-z0-9]+)*", path):
            raise ValueError(f"{label} contains invalid crate {text!r}")
        normalized.add(path)
    return tuple(sorted(normalized))


def _label_list(value: Any, label: str) -> tuple[str, ...]:
    if value is None:
        return ()
    if isinstance(value, str):
        values: Sequence[Any] = (value,)
    elif isinstance(value, (list, tuple)):
        values = value
    else:
        raise ValueError(f"{label} must be a string or array of strings")
    normalized: set[str] = set()
    for item in values:
        if not isinstance(item, str):
            raise ValueError(f"{label} must contain only strings")
        text = item.strip()
        if not text or any(ord(char) < 0x20 for char in text):
            raise ValueError(f"{label} contains an invalid label name")
        normalized.add(text)
    return tuple(sorted(normalized))


def _replacements(value: Any) -> tuple[BodyReplacement, ...]:
    if value is None:
        return ()
    rows: list[tuple[str, str, int]] = []
    if isinstance(value, Mapping):
        rows.extend(
            (key, new, 1)
            for key, new in sorted(
                value.items(), key=lambda item: (str(item[0]), str(item[1]))
            )
        )
    elif isinstance(value, list):
        for index, item in enumerate(value):
            if not isinstance(item, Mapping):
                raise ValueError(f"replacement {index} must be an object")
            unknown = set(item) - {"old", "new", "count"}
            if unknown:
                raise ValueError(
                    f"replacement {index} has unknown keys: {', '.join(sorted(unknown))}"
                )
            if "old" not in item or "new" not in item:
                raise ValueError(f"replacement {index} requires old and new")
            count = item.get("count", 1)
            if isinstance(count, bool) or not isinstance(count, int) or count <= 0:
                raise ValueError(f"replacement {index} count must be a positive integer")
            rows.append((item["old"], item["new"], count))
    else:
        raise ValueError("replacements must be an object or array")

    replacements = tuple(
        BodyReplacement(
            old=_string(old, "replacement old"),
            new=_string(new, "replacement new"),
            count=count,
        )
        for old, new, count in rows
    )
    if any(not replacement.old for replacement in replacements):
        raise ValueError("replacement old text must not be empty")
    return replacements


def _transform(raw: Any, index: int) -> NormalizationTransform:
    if not isinstance(raw, Mapping):
        raise ValueError(f"transform {index} must be an object")
    allowed = {
        "issue",
        "number",
        "body",
        "body_text",
        "replacements",
        "replace",
        "target_crates",
        "target_crate",
        "closure_wording",
        "closure",
        "add_blocked_by",
        "blocked_by_additions",
        "add_labels",
        "label_additions",
        "expected_title",
        "expected_state",
    }
    unknown = set(raw) - allowed
    if unknown:
        raise ValueError(
            f"transform {index} has unknown keys: {', '.join(sorted(unknown))}"
        )
    issue_value = raw.get("issue", raw.get("number"))
    if issue_value is None:
        raise ValueError(f"transform {index} requires issue")
    issue = _positive_issue(issue_value, f"transform {index} issue")

    body_value = raw.get("body", raw.get("body_text"))
    body = None if body_value is None else _string(body_value, f"transform {index} body")
    replacements = _replacements(raw.get("replacements", raw.get("replace")))

    target_key = "target_crates" if "target_crates" in raw else "target_crate"
    target_set = target_key in raw
    targets = (
        _string_list(raw[target_key], f"transform {index} {target_key}")
        if target_set
        else None
    )

    closure_key = "closure_wording" if "closure_wording" in raw else "closure"
    closure_set = closure_key in raw
    closure_template = None
    closure: str | None = None
    closure_verifier_ids: tuple[str, ...] = ()
    if closure_set:
        closure_value = raw[closure_key]
        if isinstance(closure_value, Mapping):
            raw_ids = closure_value.get("atom_specific_verifier_ids")
            if not isinstance(raw_ids, list) or any(
                not isinstance(value, str) for value in raw_ids
            ):
                raise ValueError(
                    f"transform {index} {closure_key} requires an explicit "
                    "atom_specific_verifier_ids array"
                )
            closure_verifier_ids = tuple(sorted(set(raw_ids)))
            expected_intent = canonical_closure_intent(closure_verifier_ids)
            if dict(closure_value) != expected_intent:
                raise ValueError(
                    f"transform {index} {closure_key} must be the canonical closure intent"
                )
            if any(
                not re.fullmatch(r"[a-z0-9][a-z0-9._-]*", value)
                or value in INTEGRATION_VERIFIER_IDS
                for value in closure_verifier_ids
            ):
                raise ValueError(
                    f"transform {index} {closure_key} contains an unsafe or generic verifier ID"
                )
            closure_template = CANONICAL_CLOSURE_TEMPLATE
        else:
            closure = _string(closure_value, f"transform {index} {closure_key}")

    labels_key = "add_labels" if "add_labels" in raw else "label_additions"
    add_labels = _label_list(raw.get(labels_key, ()), f"transform {index} {labels_key}")
    expected_title = None
    if "expected_title" in raw:
        expected_title = _raw_text(raw["expected_title"], f"transform {index} expected_title")
        if not expected_title:
            raise ValueError(f"transform {index} expected_title must not be empty")
    expected_state = None
    if "expected_state" in raw:
        expected_state = _raw_text(raw["expected_state"], f"transform {index} expected_state")
        if expected_state not in {"OPEN", "CLOSED"}:
            raise ValueError(f"transform {index} expected_state must be OPEN or CLOSED")
    if add_labels and (expected_title is None or expected_state is None):
        raise ValueError(
            f"transform {index} additive labels require exact expected_title and expected_state"
        )
    if not add_labels and (expected_title is not None or expected_state is not None):
        raise ValueError(
            f"transform {index} expected_title/expected_state require additive labels"
        )

    additions_value = raw.get("add_blocked_by", raw.get("blocked_by_additions", ()))
    if additions_value is None:
        additions_value = ()
    if not isinstance(additions_value, (list, tuple)):
        raise ValueError(f"transform {index} add_blocked_by must be an array")
    additions = tuple(
        sorted({_positive_issue(value, f"transform {index} blocker") for value in additions_value})
    )
    return NormalizationTransform(
        issue=issue,
        body=body,
        replacements=replacements,
        target_crates=targets,
        target_crates_set=target_set,
        closure_wording=closure,
        closure_wording_set=closure_set,
        closure_template=closure_template,
        closure_verifier_ids=closure_verifier_ids,
        add_blocked_by=additions,
        add_labels=add_labels,
        expected_title=expected_title,
        expected_state=expected_state,
    )


def parse_manifest(
    payload: Any,
    *,
    atom: int = DEFAULT_ATOM,
    repository: str | None = None,
) -> NormalizationManifest:
    """Parse and canonicalize a normalization manifest.

    ``transforms`` is the preferred key.  ``operations`` is accepted as a
    compatibility spelling for a declarative transform list, but the parsed form
    and digest always use ``transforms``.  No shell snippets or executable fields
    are accepted.
    """
    if isinstance(payload, (str, bytes, bytearray)):
        try:
            payload = json.loads(payload)
        except (TypeError, json.JSONDecodeError) as error:
            raise ValueError("normalization manifest is not valid JSON") from error
    if not isinstance(payload, Mapping):
        raise ValueError("normalization manifest must be a JSON object")
    unknown = set(payload) - {
        "format",
        "record_format",
        "atom",
        "repository",
        "original_plan_sha256",
        "description",
        "transforms",
        "operations",
    }
    if unknown:
        raise ValueError(
            "normalization manifest has unknown keys: " + ", ".join(sorted(unknown))
        )
    required_format = payload.get("format", payload.get("record_format"))
    if required_format != NORMALIZATION_FORMAT:
        raise ValueError(f"normalization manifest format must be {NORMALIZATION_FORMAT!r}")
    declared_atom = _positive_issue(payload.get("atom", atom), "manifest atom")
    if declared_atom != atom:
        raise ValueError(f"normalization manifest is for Atom #{declared_atom}, expected #{atom}")
    declared_repository = payload.get("repository", repository or DEFAULT_REPOSITORY)
    if not isinstance(declared_repository, str) or "/" not in declared_repository:
        raise ValueError("normalization manifest repository must be OWNER/NAME")
    if repository is not None and declared_repository != repository:
        raise ValueError(
            "normalization manifest repository "
            f"{declared_repository!r} does not match {repository!r}"
        )
    original_digest = payload.get("original_plan_sha256")
    if (
        not isinstance(original_digest, str)
        or re.fullmatch(r"[0-9a-f]{64}", original_digest) is None
    ):
        raise ValueError("normalization manifest requires a valid original_plan_sha256")
    transforms_value = payload.get("transforms", payload.get("operations", []))
    if not isinstance(transforms_value, list):
        raise ValueError("normalization manifest transforms must be an array")
    if not transforms_value:
        raise ValueError("normalization manifest transforms must not be empty")
    transforms = tuple(_transform(row, index) for index, row in enumerate(transforms_value))
    identities = [transform.issue for transform in transforms]
    if len(set(identities)) != len(identities):
        repeated = sorted(number for number in set(identities) if identities.count(number) > 1)
        raise ValueError(
            "normalization manifest repeats "
            + ", ".join(f"#{number}" for number in repeated)
        )
    description = payload.get("description", "")
    if not isinstance(description, str):
        raise ValueError("normalization manifest description must be a string")
    return NormalizationManifest(
        atom=declared_atom,
        repository=declared_repository,
        original_plan_sha256=original_digest,
        transforms=tuple(sorted(transforms, key=lambda transform: transform.issue)),
        description=normalize_body(description),
    )


def load_manifest(
    path: Path,
    *,
    atom: int = DEFAULT_ATOM,
    repository: str | None = None,
) -> NormalizationManifest:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except OSError as error:
        raise NormalizationError(f"cannot read normalization manifest {path}: {error}") from error
    except json.JSONDecodeError as error:
        raise NormalizationError(
            f"normalization manifest {path} is not valid JSON: {error}"
        ) from error
    try:
        return parse_manifest(payload, atom=atom, repository=repository)
    except ValueError as error:
        raise NormalizationError(f"invalid normalization manifest {path}: {error}") from error


def _dependencies(body: str) -> tuple[int, ...]:
    match = _DEPENDENCIES_HEADING.search(body)
    if match is None:
        raise ValueError("missing `## Dependencies` section")
    following = body[match.end() :]
    next_heading = _NEXT_HEADING.search(following)
    section = following[: next_heading.start()] if next_heading else following
    stripped = section.strip()
    if stripped.lower() in _NONE or re.fullmatch(
        r"(?:[-*][ \t]+)?None\.?", stripped, re.IGNORECASE
    ):
        return ()
    references = tuple(
        sorted(
            {
                int(number)
                for number in re.findall(
                    r"(?<![A-Za-z0-9_-])#(\d+)\b", section
                )
            }
        )
    )
    if not references:
        raise ValueError("`## Dependencies` names neither `None` nor an issue")
    return references


def _replace_dependencies(body: str, blockers: Sequence[int]) -> str:
    match = _DEPENDENCIES_HEADING.search(body)
    if match is None:
        raise ValueError("missing `## Dependencies` section")
    following = body[match.end() :]
    next_heading = _NEXT_HEADING.search(following)
    section_end = match.end() + next_heading.start() if next_heading else len(body)
    rendered = (
        "None"
        if not blockers
        else "\n".join(f"- #{number}" for number in sorted(set(blockers)))
    )
    suffix = body[section_end:]
    separator = "\n\n" if suffix else "\n"
    return body[: match.end()] + "\n" + rendered + separator + suffix


def _section(body: str, heading: str) -> tuple[int, int, int] | None:
    match = re.search(rf"^## {re.escape(heading)}[ \t]*$", body, re.MULTILINE)
    if match is None:
        return None
    following = body[match.end() :]
    next_heading = _NEXT_HEADING.search(following)
    section_end = match.end() + next_heading.start() if next_heading else len(body)
    return match.start(), match.end(), section_end


def _sections(body: str, heading: str) -> tuple[tuple[int, int, int], ...]:
    """Return every occurrence of one level-two section in source order.

    ``_section`` intentionally retains its historical first-section behaviour for
    callers that edit ordinary sections.  Closure normalization is different: a
    malformed issue may carry two Closure sections, and the declarative repair has
    to collapse those duplicates deterministically.
    """
    locations: list[tuple[int, int, int]] = []
    for match in re.finditer(rf"^## {re.escape(heading)}[ \t]*$", body, re.MULTILINE):
        following = body[match.end() :]
        next_heading = _NEXT_HEADING.search(following)
        section_end = match.end() + next_heading.start() if next_heading else len(body)
        locations.append((match.start(), match.end(), section_end))
    return tuple(locations)


def _replace_section(body: str, heading: str, wording: str) -> str:
    locations = _sections(body, heading)
    location = locations[0] if locations else None
    content = wording.rstrip("\n")
    if location is None:
        separator = "\n\n" if body and not body.endswith("\n") else "\n"
        return body + separator + f"## {heading}\n\n{content}\n"
    _, heading_end, section_end = location
    suffix = body[section_end:]
    if suffix and not suffix.startswith("\n"):
        suffix = "\n" + suffix
    updated = body[:heading_end] + "\n" + content + "\n" + suffix

    # A duplicate Closure is an old contract shape, not issue-specific prose.
    # Remove every later Closure section after replacing the first one.  The
    # section span ends immediately before the next level-two heading, so notes
    # such as #70's Retirement/Verification sections remain untouched.
    if heading == "Closure":
        while len(_sections(updated, heading)) > 1:
            duplicate = _sections(updated, heading)[1]
            start, _, end = duplicate
            updated = updated[:start] + updated[end:]
    return updated


def _target_block(targets: Sequence[str]) -> str:
    paths = tuple(f"crates/{crate}" for crate in sorted(set(targets)))
    if not paths:
        return ""
    noun = "target" if len(paths) == 1 else "targets"
    quoted = ", ".join(f"`{path}`" for path in paths)
    if len(paths) > 1:
        quoted = quoted.rsplit(", ", 1)
        quoted = " and ".join(quoted)
    return f"Rust {noun}: {quoted}."


def _replace_target_section(body: str, targets: Sequence[str]) -> str:
    if not targets:
        location = _section(body, "Target crate")
        if location is None:
            return body
        start, _, end = location
        section = body[start:end]
        if "<!-- BEGIN GENERATED: TARGET CRATE -->" not in section:
            return body
        # Only remove generated target text.  Human rationale survives, and an
        # empty generated-only section is removed as a whole.
        generated = re.compile(
            r"\n?<!-- BEGIN GENERATED: TARGET CRATE -->.*?<!-- END GENERATED: TARGET CRATE -->\n?",
            re.DOTALL,
        )
        residual = generated.sub("\n", section)
        if residual.strip() == "## Target crate":
            return body[:start] + body[end:]
        return body[:start] + residual + body[end:]
    block = (
        "<!-- BEGIN GENERATED: TARGET CRATE -->\n"
        + _target_block(targets)
        + "\n<!-- END GENERATED: TARGET CRATE -->"
    )
    location = _section(body, "Target crate")
    if location is None:
        return _replace_section(body, "Target crate", block)
    start, heading_end, end = location
    section = body[heading_end:end]
    marker = re.compile(
        r"<!-- BEGIN GENERATED: TARGET CRATE -->.*?<!-- END GENERATED: TARGET CRATE -->",
        re.DOTALL,
    )
    if marker.search(section):
        section = marker.sub(block, section, count=1)
    else:
        section = "\n\n" + block + "\n" + section.lstrip("\n")
    return body[:heading_end] + section + body[end:]


_VERIFIER_DECLARATION_RE = re.compile(
    r"(?:verifier_id|verifier)[ \t]*[:=][ \t]*`?([a-z0-9][a-z0-9._-]*)`?",
    re.IGNORECASE,
)


def _verification_verifier_ids(body: str) -> tuple[str, ...]:
    """Read only explicit Atom-specific verifier IDs from ``## Verification``."""
    locations = _sections(body, "Verification")
    if not locations:
        return ()
    if len(locations) != 1:
        raise ValueError("expected exactly one `## Verification` section")
    location = locations[0]
    _, heading_end, section_end = location
    section = body[heading_end:section_end]
    return tuple(_VERIFIER_DECLARATION_RE.findall(section))


def _ensure_acceptance_verifier(body: str, atom: int, verifier_ids: Sequence[str]) -> str:
    """Inject the one stable Atom acceptance declaration without replacing prose."""
    expected = (f"atom-{atom}-acceptance",)
    declared_intent = tuple(sorted(set(verifier_ids)))
    if declared_intent != expected:
        raise ValueError(
            f"#{atom}: canonical Closure intent must declare exactly {expected[0]!r}"
        )

    block = (
        f"{VERIFIER_DECLARATION_BEGIN}\n"
        f"verifier_id: `{expected[0]}`\n"
        f"{VERIFIER_DECLARATION_END}"
    )
    marker = re.compile(
        rf"{re.escape(VERIFIER_DECLARATION_BEGIN)}.*?"
        rf"{re.escape(VERIFIER_DECLARATION_END)}",
        re.DOTALL,
    )
    locations = _sections(body, "Verification")
    if len(locations) > 1:
        raise ValueError(f"#{atom}: expected at most one `## Verification` section")
    if not locations:
        return _replace_section(body, "Verification", block)

    _, heading_end, section_end = locations[0]
    section = body[heading_end:section_end]
    existing = marker.search(section)
    if existing is not None:
        # Preserve all human prose and the first generated block's location,
        # while collapsing stale duplicate generated declarations to one.
        section = (
            section[: existing.start()]
            + block
            + marker.sub("", section[existing.end() :])
        )
    else:
        section = "\n\n" + block + "\n" + section.lstrip("\n")
    return body[:heading_end] + section + body[section_end:]


def _apply_transform(
    issue: IssueRecord,
    transform: NormalizationTransform,
    blockers: Sequence[int],
) -> str:
    body = normalize_body(transform.body if transform.body is not None else issue.body)
    for replacement in transform.replacements:
        occurrences = body.count(replacement.old)
        if occurrences == 0 and replacement.new in body:
            if body.count(replacement.new) != replacement.count:
                raise ValueError(
                    f"#{issue.number}: replacement post-state expected "
                    f"{replacement.count} occurrence(s) of {replacement.new!r}, "
                    f"found {body.count(replacement.new)}"
                )
            continue
        if occurrences != replacement.count:
            raise ValueError(
                f"#{issue.number}: replacement expected {replacement.count} occurrence(s) of "
                f"{replacement.old!r}, found {occurrences}"
            )
        body = body.replace(replacement.old, replacement.new, replacement.count)
        if body.count(replacement.new) != replacement.count:
            raise ValueError(
                f"#{issue.number}: replacement post-state expected "
                f"{replacement.count} occurrence(s) of {replacement.new!r}, "
                f"found {body.count(replacement.new)}"
            )
    if transform.target_crates_set:
        body = _replace_target_section(body, transform.target_crates or ())
    if transform.closure_wording_set:
        if transform.closure_template == CANONICAL_CLOSURE_TEMPLATE:
            body = _ensure_acceptance_verifier(
                body, issue.number, transform.closure_verifier_ids
            )
            declared_ids = _verification_verifier_ids(body)
            if tuple(sorted(set(declared_ids))) != transform.closure_verifier_ids:
                raise ValueError(
                    f"#{issue.number}: Verification verifier IDs do not match "
                    "the canonical closure intent"
                )
            wording = canonical_closure_wording(issue.number, declared_ids)
        else:
            wording = transform.closure_wording or ""
        body = _replace_section(body, "Closure", wording)
    proposed_blockers = tuple(sorted(set(blockers)))
    if (
        _DEPENDENCIES_HEADING.search(body) is not None
        and proposed_blockers != _dependencies(body)
    ):
        body = _replace_dependencies(body, proposed_blockers)
    return normalize_body(body)


def plan_normalization(
    issues: Sequence[IssueRecord],
    manifest: NormalizationManifest,
    *,
    repository: str | None = None,
    atom: int = DEFAULT_ATOM,
    fetch_label_record: Callable[[str, int], IssueRecord] | None = None,
) -> NormalizationPlan:
    """Build a stable plan from one live/snapshot registry read."""
    if manifest.atom != atom:
        raise NormalizationError(f"manifest is for Atom #{manifest.atom}, expected #{atom}")
    selected_repository = repository or manifest.repository
    if manifest.repository != selected_repository:
        raise NormalizationError(
            f"manifest repository {manifest.repository!r} does not match {selected_repository!r}"
        )
    issue_rows = tuple(issues)
    records = {issue.number: issue for issue in issue_rows}
    if len(records) != len(issue_rows):
        raise NormalizationError("live registry repeats an issue number")
    validate_manifest_coverage(manifest, tuple(records))
    # `blocked_by` is populated from GitHub's native `blockedBy` connection by
    # the caller.  The body is only a repair target and must never add edges to
    # this plan when its human-readable mirror is stale.
    additions: dict[int, set[int]] = {
        number: set(_native_blockers(issue)) for number, issue in records.items()
    }
    transforms = {transform.issue: transform for transform in manifest.transforms}
    label_records: dict[int, IssueRecord] = {}
    for transform in manifest.transforms:
        if transform.issue not in records:
            label_only = (
                bool(transform.add_labels)
                and transform.body is None
                and not transform.replacements
                and not transform.target_crates_set
                and not transform.closure_wording_set
                and not transform.add_blocked_by
            )
            if not label_only or fetch_label_record is None:
                raise NormalizationError(
                    f"normalization transform names absent Atom #{transform.issue}"
                )
            label_issue = fetch_label_record(selected_repository, transform.issue)
            if label_issue.number != transform.issue:
                raise NormalizationError(
                    f"normalization label read returned the wrong issue for #{transform.issue}"
                )
            label_records[transform.issue] = label_issue
            if (
                transform.expected_title is not None
                and transform.expected_state is not None
                and (
                    label_issue.title != transform.expected_title
                    or label_issue.state != transform.expected_state
                )
            ):
                raise NormalizationError(
                    f"#{transform.issue}: manifest title/state precondition does not match "
                    "the label read"
                )
        for blocker in transform.add_blocked_by:
            if blocker not in records:
                raise NormalizationError(
                    f"#{transform.issue}: additive blocker #{blocker} is absent "
                    "from the live registry"
                )
            if blocker == transform.issue:
                raise NormalizationError(f"#{transform.issue}: an Atom cannot block itself")
            additions[transform.issue].add(blocker)
    try:
        topological_order(
            {number: tuple(sorted(blockers)) for number, blockers in additions.items()}
        )
    except (RuntimeError, ValueError) as error:
        raise NormalizationError(f"normalization edge additions create a cycle: {error}") from error

    bodies: list[BodyOperation] = []
    for issue in sorted(issue_rows, key=lambda record: record.number):
        transform = transforms.get(issue.number)
        proposed = (
            _apply_transform(issue, transform, additions[issue.number])
            if transform is not None
            else issue.body
        )
        old = issue.body
        bodies.append(
            BodyOperation(
                issue=issue.number,
                old_body=old,
                old_digest=body_digest(old),
                proposed_body=proposed,
                proposed_digest=body_digest(proposed),
                changed=old != proposed,
            )
        )

    edges: list[EdgeOperation] = []
    for transform in manifest.transforms:
        for blocker in transform.add_blocked_by:
            edges.append(
                EdgeOperation(
                    issue=transform.issue,
                    blocked_by=blocker,
                    was_present=blocker in records[transform.issue].blocked_by,
                    expected_blockers=tuple(sorted(additions[transform.issue])),
                )
            )
    labels: list[LabelOperation] = []
    for transform in manifest.transforms:
        if not transform.add_labels:
            continue
        if transform.expected_title is None or transform.expected_state is None:
            raise NormalizationError(
                f"#{transform.issue}: additive labels require exact title and state preconditions"
            )
        issue = records.get(transform.issue) or label_records[transform.issue]
        if issue.title != transform.expected_title or issue.state != transform.expected_state:
            raise NormalizationError(
                f"#{transform.issue}: manifest title/state precondition does not match "
                "the plan read"
            )
        labels.extend(
            LabelOperation(
                issue=transform.issue,
                label=label,
                expected_title=transform.expected_title,
                expected_state=transform.expected_state,
                was_present=label in issue.labels,
            )
            for label in transform.add_labels
        )
    plan = NormalizationPlan(
        atom=atom,
        repository=selected_repository,
        manifest_digest=manifest.digest,
        original_plan_sha256="",
        bodies=tuple(bodies),
        edges=tuple(sorted(edges, key=lambda operation: (operation.issue, operation.blocked_by))),
        labels=tuple(sorted(labels, key=lambda operation: (operation.issue, operation.label))),
    )
    return replace(plan, original_plan_sha256=operation_plan_digest(plan))


def plan_from_json(payload: Mapping[str, Any]) -> NormalizationPlan:
    """Decode the plan embedded in a journal without trusting its presentation order."""
    if payload.get("record_format") != NORMALIZATION_FORMAT:
        raise NormalizationError("normalization plan has an unexpected record format")
    atom = _positive_issue(payload.get("atom"), "plan atom")
    repository = payload.get("repository")
    manifest_sha256 = payload.get("manifest_sha256")
    original_digest = payload.get("original_plan_sha256")
    if (
        not isinstance(repository, str)
        or not isinstance(manifest_sha256, str)
        or not isinstance(original_digest, str)
    ):
        raise NormalizationError("normalization plan has invalid identity fields")
    raw_bodies = payload.get("body_operations")
    raw_edges = payload.get("edge_operations")
    raw_labels = payload.get("label_operations", [])
    if (
        not isinstance(raw_bodies, list)
        or not isinstance(raw_edges, list)
        or not isinstance(raw_labels, list)
    ):
        raise NormalizationError("normalization plan omits operation lists")
    bodies: list[BodyOperation] = []
    for raw in raw_bodies:
        if not isinstance(raw, Mapping):
            raise NormalizationError("normalization plan body operation is not an object")
        old_body = _raw_text(raw.get("old_body"), "body operation old_body")
        proposed_body = _raw_text(raw.get("proposed_body"), "body operation proposed_body")
        old_digest_value = raw.get("old_body_sha256", raw.get("old_digest", ""))
        old_legacy_digest = raw.get("old_digest")
        proposed_digest_value = raw.get(
            "proposed_body_sha256", raw.get("proposed_digest", "")
        )
        proposed_legacy_digest = raw.get("proposed_digest")
        if not isinstance(old_digest_value, str) or not isinstance(proposed_digest_value, str):
            raise NormalizationError("normalization plan body digests must be strings")
        if old_legacy_digest is not None and old_legacy_digest != old_digest_value:
            raise NormalizationError("normalization plan has conflicting old body digests")
        if (
            proposed_legacy_digest is not None
            and proposed_legacy_digest != proposed_digest_value
        ):
            raise NormalizationError("normalization plan has conflicting proposed body digests")
        changed = raw.get("changed")
        if not isinstance(changed, bool):
            raise NormalizationError("normalization plan body changed flag must be boolean")
        operation = BodyOperation(
            issue=_positive_issue(raw.get("issue"), "body operation issue"),
            old_body=old_body,
            old_digest=old_digest_value,
            proposed_body=proposed_body,
            proposed_digest=proposed_digest_value,
            changed=changed,
        )
        if operation.changed != (operation.old_body != operation.proposed_body):
            raise NormalizationError(
                f"#{operation.issue}: body operation changed flag does not match its bytes"
            )
        if (
            operation.old_digest != body_digest(operation.old_body)
            or operation.proposed_digest != body_digest(operation.proposed_body)
        ):
            raise NormalizationError(
                f"#{operation.issue}: body operation digest does not match its bytes"
            )
        bodies.append(operation)
    edges: list[EdgeOperation] = []
    for raw in raw_edges:
        if not isinstance(raw, Mapping):
            raise NormalizationError("normalization plan edge operation is not an object")
        expected = raw.get("expected_blockers", [])
        if not isinstance(expected, list):
            raise NormalizationError("edge operation expected_blockers must be an array")
        edges.append(
            EdgeOperation(
                issue=_positive_issue(raw.get("issue"), "edge operation issue"),
                blocked_by=_positive_issue(raw.get("blocked_by"), "edge operation blocker"),
                was_present=_plan_boolean(raw.get("was_present"), "edge operation was_present"),
                expected_blockers=tuple(
                    sorted(
                        {
                            _positive_issue(value, "expected blocker")
                            for value in expected
                        }
                    )
                ),
            )
        )
    labels: list[LabelOperation] = []
    for raw in raw_labels:
        if not isinstance(raw, Mapping):
            raise NormalizationError("normalization plan label operation is not an object")
        expected_title = _raw_text(
            raw.get("expected_title"), "label operation expected_title"
        )
        if not expected_title:
            raise NormalizationError("label operation expected_title must not be empty")
        expected_state = _raw_text(
            raw.get("expected_state"), "label operation expected_state"
        )
        if expected_state not in {"OPEN", "CLOSED"}:
            raise NormalizationError("label operation expected_state must be OPEN or CLOSED")
        label = _raw_text(raw.get("label"), "label operation label").strip()
        if not label or any(ord(char) < 0x20 for char in label):
            raise NormalizationError("label operation label is invalid")
        was_present = raw.get("was_present")
        if not isinstance(was_present, bool):
            raise NormalizationError("label operation was_present flag must be boolean")
        labels.append(
            LabelOperation(
                issue=_positive_issue(raw.get("issue"), "label operation issue"),
                label=label,
                expected_title=expected_title,
                expected_state=expected_state,
                was_present=was_present,
            )
        )
    plan = NormalizationPlan(
        atom=atom,
        repository=repository,
        manifest_digest=manifest_sha256,
        original_plan_sha256=original_digest,
        bodies=tuple(sorted(bodies, key=lambda operation: operation.issue)),
        edges=tuple(sorted(edges, key=lambda operation: (operation.issue, operation.blocked_by))),
        labels=tuple(sorted(labels, key=lambda operation: (operation.issue, operation.label))),
    )
    if operation_plan_digest(plan) != original_digest:
        raise NormalizationError(
            "normalization plan operation digest does not match original_plan_sha256"
        )
    return plan


def _atomic_write_json(payload: Mapping[str, Any], path: Path) -> str:
    encoded = json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary: Path | None = None
    try:
        descriptor, temporary_name = tempfile.mkstemp(dir=path.parent, prefix=f".{path.name}.")
        temporary = Path(temporary_name)
        with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
            stream.write(encoded)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
        temporary = None
    finally:
        if temporary is not None:
            with suppress(FileNotFoundError):
                temporary.unlink()
    return encoded


def write_journal(payload: Mapping[str, Any], path: Path) -> str:
    """Atomically replace a local journal and return its canonical pretty JSON."""
    return _atomic_write_json(payload, path)


def read_journal(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except OSError as error:
        raise NormalizationError(f"cannot read normalization journal {path}: {error}") from error
    except json.JSONDecodeError as error:
        raise NormalizationError(
            f"normalization journal {path} is not valid JSON: {error}"
        ) from error
    if not isinstance(payload, dict) or payload.get("record_format") != JOURNAL_FORMAT:
        raise NormalizationError(f"normalization journal {path} has an unexpected format")
    if not isinstance(payload.get("plan"), dict) or not isinstance(payload.get("operations"), list):
        raise NormalizationError("normalization journal omits its plan or operations")
    return payload


def initial_journal(plan: NormalizationPlan, *, writer: str, lease: Any = None) -> dict[str, Any]:
    if not isinstance(writer, str) or not writer.strip():
        raise NormalizationError("an authorized #70 writer identity is required")
    authorized_lease = None if lease is None else _lease_metadata(lease)
    if authorized_lease is not None and authorized_lease["actor"] != writer:
        raise NormalizationError("journal writer does not match the authorized lease actor")
    operations: list[dict[str, Any]] = []
    for operation in plan.bodies:
        if not operation.changed:
            continue
        operations.append(
            {
                "kind": "body",
                "issue": operation.issue,
                "old_body": operation.old_body,
                "old_digest": operation.old_digest,
                "old_body_sha256": operation.old_digest,
                "proposed_body": operation.proposed_body,
                "proposed_digest": operation.proposed_digest,
                "proposed_body_sha256": operation.proposed_digest,
                "changed": operation.changed,
                "state": "pending",
                "attempts": 0,
            }
        )
    for operation in plan.edges:
        if operation.was_present:
            continue
        operations.append(
            {
                "kind": "edge",
                "issue": operation.issue,
                "blocked_by": operation.blocked_by,
                "intended": True,
                "expected_blockers": list(operation.expected_blockers),
                "state": "pending",
                "attempts": 0,
            }
        )
    for operation in plan.labels:
        if operation.was_present:
            continue
        operations.append(
            {
                "kind": "label",
                "issue": operation.issue,
                "label": operation.label,
                "intended": True,
                "expected_title": operation.expected_title,
                "expected_state": operation.expected_state,
                "was_present": operation.was_present,
                "state": "pending",
                "attempts": 0,
            }
        )
    journal = {
        "record_format": JOURNAL_FORMAT,
        "journal_version": 1,
        "atom": plan.atom,
        "repository": plan.repository,
        "manifest_sha256": plan.manifest_digest,
        "plan_sha256": manifest_plan_digest(plan.as_json_object()),
        "authorized_writer": writer,
        "status": "prepared",
        "plan": plan.as_json_object(),
        "operations": operations,
        "conflicts": [],
        "events": [],
    }
    if authorized_lease is not None:
        journal["authorized_lease"] = authorized_lease
    return journal


def _append_event(journal: dict[str, Any], event: Mapping[str, Any]) -> None:
    events = journal.setdefault("events", [])
    if not isinstance(events, list):
        raise NormalizationError("normalization journal events are malformed")
    events.append({"sequence": len(events) + 1, **dict(event)})


def _append_conflict(journal: dict[str, Any], conflict: Mapping[str, Any]) -> None:
    conflicts = journal.setdefault("conflicts", [])
    if not isinstance(conflicts, list):
        raise NormalizationError("normalization journal conflicts are malformed")
    identity = tuple(sorted((str(key), repr(value)) for key, value in conflict.items()))
    if not any(
        tuple(sorted((str(key), repr(value)) for key, value in existing.items())) == identity
        for existing in conflicts
        if isinstance(existing, dict)
    ):
        conflicts.append(dict(conflict))


def _find_record(fetch_records: Callable[[], Sequence[IssueRecord]], issue: int) -> IssueRecord:
    records = tuple(fetch_records())
    matches = [record for record in records if record.number == issue]
    if len(matches) != 1:
        raise NormalizationError(f"fresh live read returned {len(matches)} records for #{issue}")
    return matches[0]


def _records_by_number(
    fetch_records: Callable[[], Sequence[IssueRecord]],
) -> dict[int, IssueRecord]:
    records = tuple(fetch_records())
    result = {record.number: record for record in records}
    if len(result) != len(records):
        raise NormalizationError("fresh live read repeats an issue number")
    return result


def _patch_issue_body(repository: str, issue: int, body: str) -> None:
    """Write one body through the shared non-interactive GitHub CLI wrapper."""
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


def _add_edge(repository: str, issue: int, blocker: int) -> None:
    from .atom_registry import _add_native_edge

    _add_native_edge(repository, issue, blocker)


def _add_label(repository: str, issue: int, label: str) -> None:
    """Add one label through the shared non-interactive GitHub CLI wrapper."""
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


def _fetch_label_record(repository: str, issue: int) -> IssueRecord:
    """Read one issue's identity and labels, including closed duplicates."""
    payload = run_gh_json(
        [
            "issue",
            "view",
            str(issue),
            "--repo",
            repository,
            "--json",
            "number,title,state,labels",
        ]
    )
    if not isinstance(payload, Mapping):
        raise NormalizationError(f"#{issue}: single-issue label read returned an invalid object")
    number = payload.get("number")
    if isinstance(number, bool) or not isinstance(number, int) or number != issue:
        raise NormalizationError(f"#{issue}: single-issue label read returned the wrong issue")
    title = payload.get("title")
    state = payload.get("state")
    labels = payload.get("labels")
    if not isinstance(title, str) or not title:
        raise NormalizationError(f"#{issue}: single-issue label read has no title")
    if state not in {"OPEN", "CLOSED"}:
        raise NormalizationError(f"#{issue}: single-issue label read has an invalid state")
    if not isinstance(labels, list):
        raise NormalizationError(f"#{issue}: single-issue label read has invalid labels")
    names: list[str] = []
    for item in labels:
        if not isinstance(item, Mapping) or not isinstance(item.get("name"), str):
            raise NormalizationError(f"#{issue}: single-issue label read has an invalid label")
        names.append(item["name"])
    if len(set(names)) != len(names):
        raise NormalizationError(f"#{issue}: single-issue label read repeats a label")
    return IssueRecord(
        number=number,
        title=title,
        state=state,
        blocked_by=(),
        labels=tuple(sorted(names)),
    )


def _lease_value(lease: Any, name: str) -> Any:
    if isinstance(lease, Mapping):
        return lease.get(name)
    return getattr(lease, name, None)


def _lease_metadata(lease: Any) -> dict[str, str]:
    """Extract the immutable claim identity without importing the claim module."""
    if lease is None:
        raise NormalizationError("a lease identity is required for this normalization journal")
    fields = {
        name: _lease_value(lease, name)
        for name in ("actor", "login", "lease_id", "claim_commit")
    }
    if any(not isinstance(value, str) or not value.strip() for value in fields.values()):
        raise NormalizationError(
            "lease identity requires non-empty actor, login, lease_id, and claim_commit"
        )
    return {name: str(value) for name, value in fields.items()}


def _check_writer(journal: Mapping[str, Any], writer: str, lease: Any = None) -> None:
    expected = journal.get("authorized_writer")
    if not isinstance(writer, str) or not writer.strip():
        raise NormalizationError("an authorized #70 writer identity is required")
    if expected != writer:
        raise NormalizationError(
            f"normalization journal is owned by {expected!r}, not authorized writer {writer!r}"
        )
    expected_lease = journal.get("authorized_lease")
    if expected_lease is None:
        return
    if not isinstance(expected_lease, Mapping):
        raise NormalizationError("normalization journal has malformed lease identity")
    actual_lease = _lease_metadata(lease)
    if dict(expected_lease) != actual_lease:
        raise NormalizationError("normalization journal lease identity differs from the live claim")


def _operation_key(operation: Mapping[str, Any]) -> tuple[str, int, int]:
    kind = operation.get("kind")
    issue = _positive_issue(operation.get("issue"), "journal operation issue")
    blocker = (
        _positive_issue(operation.get("blocked_by"), "journal operation blocker")
        if kind == "edge"
        else 0
    )
    if kind not in {"body", "edge", "label"}:
        raise NormalizationError(f"journal operation has unknown kind {kind!r}")
    return str(kind), issue, blocker


def _validate_journal_plan(journal: Mapping[str, Any], plan: NormalizationPlan) -> None:
    if journal.get("manifest_sha256") != plan.manifest_digest:
        raise NormalizationError("normalization journal and plan use different manifest digests")
    if journal.get("repository") != plan.repository or journal.get("atom") != plan.atom:
        raise NormalizationError("normalization journal identity differs from the requested plan")
    plan_digest = journal.get("plan_sha256")
    if plan_digest is not None and plan_digest != manifest_plan_digest(plan.as_json_object()):
        raise NormalizationError(
            "normalization journal plan digest differs from the requested plan"
        )
    embedded = journal.get("plan")
    if not isinstance(embedded, Mapping):
        raise NormalizationError("normalization journal plan differs from the requested plan")
    try:
        embedded_plan = plan_from_json(embedded)
    except (NormalizationError, TypeError, ValueError) as error:
        raise NormalizationError("normalization journal plan is malformed") from error
    if manifest_plan_digest(embedded_plan.as_json_object()) != manifest_plan_digest(
        plan.as_json_object()
    ):
        raise NormalizationError("normalization journal plan differs from the requested plan")


def _operation_identity(operation: Mapping[str, Any]) -> dict[str, Any]:
    kind = operation.get("kind")
    if kind == "body":
        keys = (
            "kind", "issue", "old_body", "old_digest", "old_body_sha256",
            "proposed_body", "proposed_digest", "proposed_body_sha256", "changed",
        )
    elif kind == "edge":
        keys = ("kind", "issue", "blocked_by", "intended", "was_present", "expected_blockers")
    elif kind == "label":
        keys = (
            "kind", "issue", "label", "intended", "expected_title", "expected_state",
            "was_present",
        )
    else:
        raise NormalizationError(f"journal operation has unknown kind {kind!r}")
    return {key: operation.get(key) for key in keys}


def _validate_journal_operations(
    journal: Mapping[str, Any], plan: NormalizationPlan
) -> None:
    operations = journal.get("operations")
    if not isinstance(operations, list):
        raise NormalizationError("normalization journal operations are malformed")
    expected = plan.as_json_object()["operations"]
    if not isinstance(expected, list) or len(operations) != len(expected):
        raise NormalizationError(
            "normalization journal operation set differs from the committed plan"
        )
    for index, (actual, canonical) in enumerate(zip(operations, expected, strict=True)):
        if not isinstance(actual, Mapping) or _operation_identity(actual) != _operation_identity(
            canonical
        ):
            raise NormalizationError(
                f"normalization journal operation {index} differs from the committed plan"
            )


def manifest_plan_digest(payload: Mapping[str, Any]) -> str:
    """Digest a plan while excluding only runtime operation state (not plan bytes)."""
    return hashlib.sha256(_canonical_json(payload)).hexdigest()


def _persist(journal: dict[str, Any], path: Path) -> None:
    write_journal(journal, path)


def _record_conflict(
    journal: dict[str, Any],
    operation: dict[str, Any],
    *,
    reason: str,
    observed: str | None = None,
) -> None:
    operation["state"] = "conflict"
    operation["conflict_reason"] = reason
    if observed is not None:
        operation["observed_digest"] = observed
    _append_conflict(
        journal,
        {
            "kind": operation.get("kind"),
            "issue": operation.get("issue"),
            **(
                {"blocked_by": operation.get("blocked_by")}
                if operation.get("kind") == "edge"
                else {}
            ),
            "reason": reason,
            **({"observed_digest": observed} if observed is not None else {}),
        },
    )


def _advance_body(
    journal: dict[str, Any],
    operation: dict[str, Any],
    *,
    fetch_records: Callable[[], Sequence[IssueRecord]],
    patch_body: Callable[[str, int, str], None],
    path: Path,
) -> None:
    issue = _positive_issue(operation.get("issue"), "body operation issue")
    current = _find_record(fetch_records, issue)
    current_digest = body_digest(current.body)
    proposed_digest = str(operation["proposed_digest"])
    old_digest = str(operation["old_digest"])
    if current_digest == proposed_digest:
        operation["state"] = "verified"
        operation["verified_digest"] = current_digest
        _append_event(journal, {"kind": "body", "issue": issue, "event": "already-proposed"})
        _persist(journal, path)
        return
    if current_digest != old_digest:
        _record_conflict(
            journal,
            operation,
            reason="stale-before-write",
            observed=current_digest,
        )
        _append_event(journal, {"kind": "body", "issue": issue, "event": "conflict"})
        _persist(journal, path)
        raise NormalizationConflict(f"#{issue}: old body digest is stale before write")

    operation["attempts"] = int(operation.get("attempts", 0)) + 1
    operation["state"] = "in_flight"
    operation["effect_started"] = True
    _append_event(journal, {"kind": "body", "issue": issue, "event": "write-intended"})
    _persist(journal, path)
    try:
        patch_body(str(journal["repository"]), issue, str(operation["proposed_body"]))
    except Exception as error:
        operation["last_error"] = str(error)
        _append_event(journal, {"kind": "body", "issue": issue, "event": "write-unknown"})
        _persist(journal, path)
        raise

    verified = _find_record(fetch_records, issue)
    verified_digest = body_digest(verified.body)
    if verified_digest != proposed_digest:
        _record_conflict(
            journal,
            operation,
            reason="post-write-verification-mismatch",
            observed=verified_digest,
        )
        _append_event(journal, {"kind": "body", "issue": issue, "event": "verify-conflict"})
        _persist(journal, path)
        raise NormalizationConflict(f"#{issue}: body did not verify after write")
    operation["state"] = "verified"
    operation["verified_digest"] = verified_digest
    _append_event(journal, {"kind": "body", "issue": issue, "event": "verified"})
    _persist(journal, path)


def _advance_edge(
    journal: dict[str, Any],
    operation: dict[str, Any],
    *,
    fetch_records: Callable[[], Sequence[IssueRecord]],
    add_edge: Callable[[str, int, int], None],
    path: Path,
) -> None:
    issue = _positive_issue(operation.get("issue"), "edge operation issue")
    blocker = _positive_issue(operation.get("blocked_by"), "edge operation blocker")
    records = _records_by_number(fetch_records)
    current = records.get(issue)
    if current is None:
        _record_conflict(journal, operation, reason="issue-disappeared")
        _persist(journal, path)
        raise NormalizationConflict(f"#{issue}: issue disappeared during edge reconciliation")
    if blocker in current.blocked_by:
        operation["state"] = "verified"
        operation["verified"] = True
        operation["observed_blockers"] = list(current.blocked_by)
        for conflict in journal.get("conflicts", ()):
            if (
                isinstance(conflict, dict)
                and conflict.get("kind") == "edge"
                and conflict.get("issue") == issue
                and conflict.get("blocked_by") == blocker
            ):
                conflict["resolved"] = True
        _append_event(
            journal,
            {
                "kind": "edge",
                "issue": issue,
                "blocked_by": blocker,
                "event": "already-present",
            },
        )
        _persist(journal, path)
        return

    operation["attempts"] = int(operation.get("attempts", 0)) + 1
    operation["state"] = "in_flight"
    operation["effect_started"] = True
    _append_event(
        journal,
        {
            "kind": "edge",
            "issue": issue,
            "blocked_by": blocker,
            "event": "add-intended",
        },
    )
    _persist(journal, path)
    try:
        add_edge(str(journal["repository"]), issue, blocker)
    except Exception as error:
        # The API call may have succeeded before a transport failure.  Keep the
        # intent and classify it as ambiguous; recovery performs a fresh read and
        # may safely retry the additive operation, but never removes it.
        operation["state"] = "ambiguous"
        operation["last_error"] = str(error)
        _append_conflict(
            journal,
            {
                "kind": "edge",
                "issue": issue,
                "blocked_by": blocker,
                "reason": "edge-write-outcome-ambiguous",
            },
        )
        _append_event(
            journal,
            {
                "kind": "edge",
                "issue": issue,
                "blocked_by": blocker,
                "event": "add-unknown",
            },
        )
        _persist(journal, path)
        raise

    after = _find_record(fetch_records, issue)
    if blocker not in after.blocked_by:
        operation["state"] = "ambiguous"
        _append_conflict(
            journal,
            {
                "kind": "edge",
                "issue": issue,
                "blocked_by": blocker,
                "reason": "edge-write-not-observed",
            },
        )
        _append_event(
            journal,
            {
                "kind": "edge",
                "issue": issue,
                "blocked_by": blocker,
                "event": "verify-missing",
            },
        )
        _persist(journal, path)
        raise NormalizationConflict(f"#{issue}: additive edge #{blocker} did not verify")
    operation["state"] = "verified"
    operation["verified"] = True
    operation["observed_blockers"] = list(after.blocked_by)
    for conflict in journal.get("conflicts", ()):
        if (
            isinstance(conflict, dict)
            and conflict.get("kind") == "edge"
            and conflict.get("issue") == issue
            and conflict.get("blocked_by") == blocker
        ):
            conflict["resolved"] = True
    _append_event(
        journal,
        {
            "kind": "edge",
            "issue": issue,
            "blocked_by": blocker,
            "event": "verified",
        }
    )
    _persist(journal, path)


def _record_label_conflict(
    journal: dict[str, Any],
    operation: dict[str, Any],
    *,
    reason: str,
    observed: IssueRecord,
) -> None:
    operation["state"] = "conflict"
    operation["conflict_reason"] = reason
    operation["observed_title"] = observed.title
    operation["observed_state"] = observed.state
    _append_conflict(
        journal,
        {
            "kind": "label",
            "issue": operation.get("issue"),
            "label": operation.get("label"),
            "reason": reason,
            "expected_title": operation.get("expected_title"),
            "expected_state": operation.get("expected_state"),
            "observed_title": observed.title,
            "observed_state": observed.state,
        },
    )


def _resolve_label_conflicts(journal: dict[str, Any], operation: Mapping[str, Any]) -> None:
    for conflict in journal.get("conflicts", ()):
        if (
            isinstance(conflict, dict)
            and conflict.get("kind") == "label"
            and conflict.get("issue") == operation.get("issue")
            and conflict.get("label") == operation.get("label")
        ):
            conflict["resolved"] = True


def _advance_label(
    journal: dict[str, Any],
    operation: dict[str, Any],
    *,
    fetch_records: Callable[[], Sequence[IssueRecord]],
    add_label: Callable[[str, int, str], None],
    fetch_label_record: Callable[[str, int], IssueRecord] | None,
    path: Path,
) -> None:
    issue = _positive_issue(operation.get("issue"), "label operation issue")
    label = _raw_text(operation.get("label"), "label operation label").strip()
    expected_title = _raw_text(
        operation.get("expected_title"), "label operation expected_title"
    )
    expected_state = _raw_text(
        operation.get("expected_state"), "label operation expected_state"
    )
    current = (
        fetch_label_record(str(journal["repository"]), issue)
        if fetch_label_record is not None
        else _find_record(fetch_records, issue)
    )
    if current.title != expected_title or current.state != expected_state:
        _record_label_conflict(
            journal,
            operation,
            reason="stale-label-title-state",
            observed=current,
        )
        _append_event(journal, {"kind": "label", "issue": issue, "event": "conflict"})
        _persist(journal, path)
        raise NormalizationConflict(f"#{issue}: label title/state is stale before write")
    if label in current.labels:
        operation["state"] = "verified"
        operation["verified"] = True
        operation["observed_labels"] = list(current.labels)
        _resolve_label_conflicts(journal, operation)
        _append_event(journal, {"kind": "label", "issue": issue, "event": "already-present"})
        _persist(journal, path)
        return

    operation["attempts"] = int(operation.get("attempts", 0)) + 1
    operation["state"] = "in_flight"
    operation["effect_started"] = True
    _append_event(journal, {"kind": "label", "issue": issue, "event": "add-intended"})
    _persist(journal, path)
    try:
        add_label(str(journal["repository"]), issue, label)
    except Exception as error:
        operation["state"] = "ambiguous"
        operation["last_error"] = str(error)
        _append_conflict(
            journal,
            {
                "kind": "label",
                "issue": issue,
                "label": label,
                "reason": "label-write-outcome-ambiguous",
            },
        )
        _append_event(journal, {"kind": "label", "issue": issue, "event": "add-unknown"})
        _persist(journal, path)
        raise

    after = (
        fetch_label_record(str(journal["repository"]), issue)
        if fetch_label_record is not None
        else _find_record(fetch_records, issue)
    )
    if after.title != expected_title or after.state != expected_state:
        _record_label_conflict(
            journal,
            operation,
            reason="label-post-write-identity-mismatch",
            observed=after,
        )
        _append_event(journal, {"kind": "label", "issue": issue, "event": "verify-conflict"})
        _persist(journal, path)
        raise NormalizationConflict(f"#{issue}: label identity changed after write")
    if label not in after.labels:
        operation["state"] = "ambiguous"
        _append_conflict(
            journal,
            {
                "kind": "label",
                "issue": issue,
                "label": label,
                "reason": "label-write-not-observed",
            },
        )
        _append_event(journal, {"kind": "label", "issue": issue, "event": "verify-missing"})
        _persist(journal, path)
        raise NormalizationConflict(f"#{issue}: additive label {label!r} did not verify")
    operation["state"] = "verified"
    operation["verified"] = True
    operation["observed_labels"] = list(after.labels)
    _resolve_label_conflicts(journal, operation)
    _append_event(journal, {"kind": "label", "issue": issue, "event": "verified"})
    _persist(journal, path)


def _terminal(operation: Mapping[str, Any]) -> bool:
    return operation.get("state") in {"verified", "compensated", "already-original"}


def _journal_has_conflict(journal: Mapping[str, Any]) -> bool:
    return any(
        isinstance(operation, Mapping) and operation.get("state") in {"conflict", "ambiguous"}
        for operation in journal.get("operations", ())
    )


def advance_journal(
    plan: NormalizationPlan,
    journal_path: Path,
    *,
    writer: str,
    lease: Any = None,
    fetch_records: Callable[[], Sequence[IssueRecord]],
    patch_body: Callable[[str, int, str], None] | None = None,
    add_edge: Callable[[str, int, int], None] | None = None,
    add_label: Callable[[str, int, str], None] | None = None,
    fetch_label_record: Callable[[str, int], IssueRecord] | None = None,
    manifest: NormalizationManifest | None = None,
) -> dict[str, Any]:
    """Persist a journal before effects, then advance all safe operations.

    A new journal is written before the first fetch immediately preceding an
    external write.  Existing journals are resumed by reading their durable states;
    an in-flight operation is reclassified from the fresh live state rather than
    being blindly repeated.
    """
    patch_body = patch_body or _patch_issue_body
    add_edge = add_edge or _add_edge
    add_label = add_label or _add_label
    if journal_path.exists():
        journal = read_journal(journal_path)
        _check_writer(journal, writer, lease)
        if (
            journal.get("status") in {"completed", "effects-verified", "compensated"}
            and not plan.changed_bodies
            and not plan.changed_edges
            and not plan.changed_labels
        ):
            # A second command invocation reads the already-normalized live
            # bodies, so its newly computed plan has no old-state bytes to match
            # against the completed journal.  The manifest identity still binds
            # this idempotent no-op to the original operation.
            if journal.get("manifest_sha256") != plan.manifest_digest:
                raise NormalizationError(
                    "normalization journal and plan use different manifest digests"
                )
            embedded_plan = plan_from_json(journal["plan"])
            if manifest is not None and (
                manifest.original_plan_sha256 != embedded_plan.original_plan_sha256
                or manifest.digest != embedded_plan.manifest_digest
            ):
                raise NormalizationError(
                    "completed normalization journal does not match the committed manifest"
                )
            if (
                embedded_plan.manifest_digest != plan.manifest_digest
                or embedded_plan.repository != plan.repository
                or embedded_plan.atom != plan.atom
            ):
                raise NormalizationError(
                    "completed normalization journal does not match the live plan identity"
                )
            _validate_journal_operations(journal, embedded_plan)
            return journal
        if manifest is not None and manifest.original_plan_sha256 != plan.original_plan_sha256:
            raise NormalizationError("normalization plan does not match the committed manifest")
        _validate_journal_plan(journal, plan)
        _validate_journal_operations(journal, plan)
    else:
        if manifest is not None and manifest.original_plan_sha256 != plan.original_plan_sha256:
            raise NormalizationError("normalization plan does not match the committed manifest")
        journal = initial_journal(plan, writer=writer, lease=lease)
        # This write is the durable intent record and happens before any effect.
        _persist(journal, journal_path)

    operations = journal.get("operations")
    if not isinstance(operations, list):
        raise NormalizationError("normalization journal operations are malformed")
    operations.sort(key=_operation_key)
    journal["status"] = "running"
    _persist(journal, journal_path)
    for operation in operations:
        if not isinstance(operation, dict):
            raise NormalizationError("normalization journal operation is malformed")
        if _terminal(operation):
            continue
        if operation.get("kind") == "body":
            _advance_body(
                journal,
                operation,
                fetch_records=fetch_records,
                patch_body=patch_body,
                path=journal_path,
            )
        elif operation.get("kind") == "edge":
            _advance_edge(
                journal,
                operation,
                fetch_records=fetch_records,
                add_edge=add_edge,
                path=journal_path,
            )
        elif operation.get("kind") == "label":
            _advance_label(
                journal,
                operation,
                fetch_records=fetch_records,
                add_label=add_label,
                fetch_label_record=fetch_label_record,
                path=journal_path,
            )
        else:
            raise NormalizationError(
                f"journal operation has unknown kind {operation.get('kind')!r}"
            )

    if _journal_has_conflict(journal):
        journal["status"] = "conflict"
        _persist(journal, journal_path)
        raise NormalizationConflict("normalization journal contains unresolved conflicts")
    journal["status"] = "effects-verified"
    _append_event(journal, {"event": "effects-verified"})
    _persist(journal, journal_path)
    return journal


def recover_journal(
    journal_path: Path,
    *,
    writer: str,
    lease: Any = None,
    fetch_records: Callable[[], Sequence[IssueRecord]],
    patch_body: Callable[[str, int, str], None] | None = None,
    add_edge: Callable[[str, int, int], None] | None = None,
    add_label: Callable[[str, int, str], None] | None = None,
    fetch_label_record: Callable[[str, int], IssueRecord] | None = None,
    compensate: bool = False,
    manifest: NormalizationManifest | None = None,
) -> dict[str, Any]:
    """Resume an interrupted journal, optionally using guarded body compensation."""
    journal = read_journal(journal_path)
    _check_writer(journal, writer, lease)
    plan_payload = journal.get("plan")
    if not isinstance(plan_payload, Mapping):
        raise NormalizationError("normalization journal has no embedded plan")
    plan = plan_from_json(plan_payload)
    if manifest is None:
        raise NormalizationError("recovery requires the committed normalization manifest")
    if (
        manifest.digest != plan.manifest_digest
        or manifest.original_plan_sha256 != plan.original_plan_sha256
    ):
        raise NormalizationError(
            "recovery plan does not match the committed normalization manifest"
        )
    _validate_journal_plan(journal, plan)
    _validate_journal_operations(journal, plan)
    if compensate:
        compensate_bodies(
            journal_path,
            writer=writer,
            lease=lease,
            fetch_records=fetch_records,
            patch_body=patch_body,
        )
        journal = read_journal(journal_path)
    return advance_journal(
        plan,
        journal_path,
        writer=writer,
        lease=lease,
        fetch_records=fetch_records,
        patch_body=patch_body,
        add_edge=add_edge,
        add_label=add_label,
        fetch_label_record=fetch_label_record,
    )


def compensate_bodies(
    journal_path: Path,
    *,
    writer: str,
    lease: Any = None,
    fetch_records: Callable[[], Sequence[IssueRecord]],
    patch_body: Callable[[str, int, str], None] | None = None,
) -> dict[str, Any]:
    """Guarded best-effort body restoration; native edges are never removed."""
    patch_body = patch_body or _patch_issue_body
    journal = read_journal(journal_path)
    _check_writer(journal, writer, lease)
    operations = journal.get("operations")
    if not isinstance(operations, list):
        raise NormalizationError("normalization journal operations are malformed")
    for operation in sorted(
        (item for item in operations if isinstance(item, dict)),
        key=_operation_key,
    ):
        if operation.get("kind") != "body":
            continue
        if not operation.get("effect_started") and operation.get("state") not in {
            "verified",
            "compensated",
        }:
            # A pending or stale-before-write body was never proven to be ours;
            # even a matching desired digest is not authority to restore it.
            continue
        issue = _positive_issue(operation.get("issue"), "body operation issue")
        current = _find_record(fetch_records, issue)
        current_digest = body_digest(current.body)
        proposed_digest = str(operation.get("proposed_digest"))
        old_digest = str(operation.get("old_digest"))
        if current_digest == old_digest:
            operation["state"] = "already-original"
            operation["compensation"] = "already-original"
            _append_event(
                journal,
                {"kind": "body", "issue": issue, "event": "compensation-not-needed"},
            )
            _persist(journal, journal_path)
            continue
        if current_digest != proposed_digest:
            _record_conflict(
                journal,
                operation,
                reason="compensation-current-body-is-not-our-proposal",
                observed=current_digest,
            )
            _append_event(
                journal,
                {"kind": "body", "issue": issue, "event": "compensation-conflict"},
            )
            _persist(journal, journal_path)
            continue
        operation["state"] = "compensation-in-flight"
        _append_event(journal, {"kind": "body", "issue": issue, "event": "compensation-intended"})
        _persist(journal, journal_path)
        try:
            patch_body(str(journal["repository"]), issue, str(operation["old_body"]))
        except Exception as error:
            operation["last_compensation_error"] = str(error)
            _append_event(
                journal,
                {"kind": "body", "issue": issue, "event": "compensation-unknown"},
            )
            _persist(journal, journal_path)
            raise
        verified = _find_record(fetch_records, issue)
        verified_digest = body_digest(verified.body)
        if verified_digest != old_digest:
            _record_conflict(
                journal,
                operation,
                reason="compensation-verification-mismatch",
                observed=verified_digest,
            )
            _append_event(
                journal,
                {
                    "kind": "body",
                    "issue": issue,
                    "event": "compensation-verify-conflict",
                },
            )
            _persist(journal, journal_path)
            continue
        operation["state"] = "compensated"
        operation["compensation"] = "restored-old-body"
        operation["verified_digest"] = verified_digest
        _append_event(journal, {"kind": "body", "issue": issue, "event": "compensated"})
        _persist(journal, journal_path)
    journal["status"] = "compensated" if not _journal_has_conflict(journal) else "conflict"
    _persist(journal, journal_path)
    return journal


def journal_complete(journal: Mapping[str, Any]) -> bool:
    operations = journal.get("operations")
    return isinstance(operations, list) and not _journal_has_conflict(journal) and all(
        isinstance(operation, Mapping) and _terminal(operation) for operation in operations
    )


# Friendly aliases used by callers that name the command after the operation.
parse_normalization_manifest = parse_manifest
load_normalization_manifest = load_manifest
plan_atom_normalization = plan_normalization
normalize_plan = plan_normalization
apply_normalization = advance_journal
apply_normalization_journal = advance_journal
recover_normalization = recover_journal
recover_normalization_journal = recover_journal
compensate_normalization = compensate_bodies
compensate_normalization_journal = compensate_bodies
fetch_label_record = _fetch_label_record
sha256_body = body_digest
write_normalization_journal = write_journal
load_normalization_journal = read_journal
