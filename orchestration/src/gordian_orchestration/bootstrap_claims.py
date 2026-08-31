"""Bootstrap Atom claims backed by one append-only GitHub ref.

Issue #70 owns this temporary coordination path until native leases exist.  The
claim log (``refs/heads/gordian-claim-log``) is the authority: each event is an
empty Git commit whose message is a strict canonical JSON document containing the
event and the complete active-claim set.  Assignees, Project status, and issue
comments are projections reconciled after the compare-and-swap succeeds.

The GitHub API is deliberately kept at the boundary.  No local clock, comment
timestamp, assignee, or Project field arbitrates a race.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import secrets
import sys
from dataclasses import dataclass, replace
from datetime import UTC, datetime, timedelta
from email.utils import parsedate_to_datetime
from pathlib import Path
from typing import Any

from .derive_status import (
    BLOCKED,
    DEFAULT_CLOSURE_ROOT,
    DEFAULT_CLOSURE_SCHEMA,
    DEFAULT_OWNER,
    DEFAULT_PROJECT_NUMBER,
    DEFAULT_REPOSITORY,
    READY,
    STATUS_FIELD,
    Change,
    DerivedRow,
    apply_change,
    bootstrap_satisfied,
    derive,
    fetch_board,
    fetch_issues,
    load_closure_schema,
    ready_set,
)
from .gh import (
    EX_CONFIG,
    GitHubApiError,
    GitHubApiResponse,
    GitHubConfigurationError,
    preflight,
    run_gh,
    run_gh_json,
    run_gh_json_response,
    run_gh_response,
)

CLAIM_MINUTES = 240
MAX_ACTIVE_CLAIMS = 3
MAX_CAS_RETRIES = 4
CLAIM_LOG_REF = "refs/heads/gordian-claim-log"
CLAIM_LOG_BRANCH = "gordian-claim-log"
CLAIM_EVENT_SCHEMA = "gordian-bootstrap-claim-event-v1"
LEGACY_ADOPTION_ATOM = 70

ACTOR_PATTERN = re.compile(r"^gordian-agent/[a-z0-9-]+/[A-Za-z0-9_-]+$")
CLAIMED_AT_PATTERN = re.compile(r"^claimed_at: (\S+)$")
EXPIRY_PATTERN = re.compile(r"^expires_after_minutes: (\d+)$")
EVENT_ID_PATTERN = re.compile(r"^[0-9a-f]{32}$")
LEASE_ID_PATTERN = re.compile(r"^[0-9a-f]{32}$")
SHA_PATTERN = re.compile(r"^[0-9a-f]{40}$")
# Human accounts use letters, digits, ``-`` and occasionally ``.``/``_``;
# GitHub's built-in ``[bot]`` accounts also appear in projection reads.  The
# canonical lease still carries only a bounded, argv-safe login token.
LOGIN_PATTERN = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.\[\]-]*$")

_EVENT_KEYS = frozenset(
    {"actor", "atom", "event_id", "lease_id", "login", "reason", "type"}
)
_CLAIM_EVENT_KEYS = _EVENT_KEYS | frozenset({"claimed_at"})
_RECLAIM_EVENT_KEYS = _EVENT_KEYS | frozenset({"target_actor", "target_login"})
_ENTRY_KEYS = frozenset({"actor", "atom", "claim_commit", "lease_id", "login"})
_DOCUMENT_KEYS = frozenset({"active_claims", "event", "schema"})
_EVENT_TYPES = frozenset({"claim", "release", "abort", "reclaim"})


@dataclass(frozen=True, slots=True)
class ClaimLease:
    """Canonical lease metadata plus the server-derived claimed-at instant.

    ``claimed_at`` is obtained from the GitHub response ``Date`` captured when a
    claim event is created. It is optional only for compatibility with old claim-log
    events and fixtures; new events never use a client-supplied commit timestamp for
    liveness.
    """

    actor: str
    claimed_at: datetime | None = None
    expires_after_minutes: int = CLAIM_MINUTES
    comment_url: str = ""
    login: str = ""
    lease_id: str = ""
    claim_commit: str = ""
    event_id: str = ""
    atom: int | None = None

    @property
    def expires_at(self) -> datetime:
        if self.claimed_at is None:
            raise RuntimeError("lease has no server-derived claimed_at")
        return self.claimed_at + timedelta(minutes=self.expires_after_minutes)

    def is_live(self, now: datetime) -> bool:
        return self.claimed_at is not None and now < self.expires_at

    def as_json_object(self, now: datetime) -> dict[str, Any]:
        expires_at = self.expires_at if self.claimed_at is not None else None
        return {
            "actor": self.actor,
            "claimed_at": _timestamp(self.claimed_at) if self.claimed_at is not None else None,
            "expires_after_minutes": self.expires_after_minutes,
            "expires_at": _timestamp(expires_at) if expires_at is not None else None,
            "live": self.is_live(now),
            "comment_url": self.comment_url,
            "login": self.login,
            "lease_id": self.lease_id,
            "claim_commit": self.claim_commit,
            "event_id": self.event_id,
        }


@dataclass(frozen=True, slots=True)
class IssueClaim:
    """One issue's projection together with its canonical lease, if any."""

    number: int
    state: str
    assignees: tuple[str, ...]
    lease: ClaimLease | None
    leases: tuple[ClaimLease, ...] = ()
    projection_drift: tuple[str, ...] = ()

    @property
    def claim_leases(self) -> tuple[ClaimLease, ...]:
        """Compatibility view for callers that used comment history."""
        if self.leases:
            return self.leases
        return (self.lease,) if self.lease is not None else ()


@dataclass(frozen=True, slots=True)
class _LegacyClaimComment:
    """One validated legacy projection comment selected for one-time adoption."""

    comment_id: int
    body: str
    lease: ClaimLease


@dataclass(frozen=True, slots=True)
class ClaimLogState:
    """Validated state at one claim-log head."""

    head_sha: str | None
    tree_sha: str | None
    active_claims: tuple[ClaimLease, ...]
    event: dict[str, Any] | None
    server_now: datetime | None
    message: str = ""
    parents: tuple[str, ...] = ()
    # Validated event documents from the current head back to the first ordinary
    # accepted-main root.  Keeping this small projection of the validated history
    # lets lost-response recovery find an event that has since been followed by a
    # competing append without making the projection layer an authority.
    events: tuple[dict[str, Any], ...] = ()

    def for_atom(self, number: int) -> ClaimLease | None:
        return next((lease for lease in self.active_claims if lease.atom == number), None)

    def for_actor(self, actor: str) -> tuple[ClaimLease, ...]:
        return tuple(lease for lease in self.active_claims if lease.actor == actor)


@dataclass(frozen=True, slots=True)
class _RefSnapshot:
    head_sha: str | None
    server_now: datetime


@dataclass(frozen=True, slots=True)
class _CommitSnapshot:
    sha: str
    tree_sha: str
    message: str
    committed_at: datetime
    parents: tuple[str, ...]
    server_now: datetime


@dataclass(frozen=True, slots=True)
class _ValidatedClaimHistory:
    """The complete validated claim-log path and its head state."""

    root: _CommitSnapshot
    commits: tuple[_CommitSnapshot, ...]  # root -> head
    events: tuple[dict[str, Any], ...]  # head -> root, one per commit
    entries: tuple[dict[str, Any], ...]  # active entries at head


@dataclass(frozen=True, slots=True)
class _AppendResult:
    event_id: str
    commit_sha: str
    state: ClaimLogState
    lease: ClaimLease | None
    already_present: bool = False


class _CasConflict(RuntimeError):
    """The GitHub ref did not accept this commit as its next fast-forward."""


def _timestamp(value: datetime) -> str:
    return value.astimezone(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def _parse_timestamp(value: str) -> datetime:
    parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    if parsed.tzinfo is None:
        raise ValueError("timestamp has no timezone")
    return parsed.astimezone(UTC)


def _parse_server_date(value: str) -> datetime:
    try:
        parsed = parsedate_to_datetime(value)
    except (TypeError, ValueError, IndexError, OverflowError):
        parsed = None
    if parsed is None:
        return _parse_timestamp(value)
    if parsed.tzinfo is None:
        raise ValueError("GitHub Date header has no timezone")
    return parsed.astimezone(UTC)


def _validate_reason(value: Any, *, field: str = "reason") -> str:
    if not isinstance(value, str) or len(value) > 1000:
        raise RuntimeError(f"claim log event {field} is invalid")
    if any(ord(char) < 0x20 and char not in "\t" for char in value):
        raise RuntimeError(f"claim log event {field} contains a control character")
    return value


def _server_now(response: GitHubApiResponse) -> datetime:
    for name, value in response.headers.items():
        if name.lower() == "date" and value.strip():
            try:
                return _parse_server_date(value.strip())
            except ValueError as error:
                raise RuntimeError("GitHub response Date header is malformed") from error
    raise RuntimeError(
        "GitHub response is missing Date; refusing to use a local clock for lease expiry"
    )


def _json_body(response: GitHubApiResponse, *, operation: str) -> Any:
    try:
        value = json.loads(response.body)
    except json.JSONDecodeError as error:
        raise RuntimeError(f"{operation}: GitHub returned invalid JSON") from error
    return value


def _validate_sha(value: Any, *, field: str) -> str:
    if not isinstance(value, str) or SHA_PATTERN.fullmatch(value) is None:
        raise RuntimeError(f"claim log: {field} is not a valid commit SHA")
    return value


def _validate_actor(value: Any, *, field: str = "actor") -> str:
    if not isinstance(value, str) or ACTOR_PATTERN.fullmatch(value) is None:
        raise RuntimeError(f"claim log: {field} is not a valid Gordian actor")
    return value


def _validate_login(value: Any, *, field: str = "login") -> str:
    if not isinstance(value, str) or LOGIN_PATTERN.fullmatch(value) is None:
        raise RuntimeError(f"claim log: {field} is not a valid GitHub login")
    return value


def _validate_id(value: Any, *, field: str) -> str:
    if not isinstance(value, str) or EVENT_ID_PATTERN.fullmatch(value) is None:
        raise RuntimeError(f"claim log: {field} is not a valid random identifier")
    return value


def _validate_atom(value: Any, *, field: str = "atom") -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise RuntimeError(f"claim log: {field} must be a positive integer")
    return value


def _new_id() -> str:
    return secrets.token_hex(16)


def _safe_reason(reason: str) -> str:
    if not isinstance(reason, str):
        raise TypeError("claim event reason must be a string")
    if len(reason) > 1000:
        raise ValueError("claim event reason is too long")
    if any(ord(char) < 0x20 and char not in "\t" for char in reason):
        raise ValueError("claim event reason contains a control character")
    # A caller can accidentally paste either credential spelling into a reason.
    # Redact both the explicit orchestration credential and the child-process
    # override before the value can enter a commit message, comment, or result.
    redacted = reason
    for variable in ("GORDIAN_GH_TOKEN", "GH_TOKEN"):
        token = os.environ.get(variable, "")
        if token:
            redacted = redacted.replace(token, "<redacted>")
    return redacted


def _canonical_json(value: dict[str, Any]) -> str:
    try:
        return json.dumps(
            value,
            ensure_ascii=True,
            allow_nan=False,
            sort_keys=True,
            separators=(",", ":"),
        )
    except (TypeError, ValueError) as error:
        raise RuntimeError("claim event is not canonical JSON") from error


def _entry_object(
    *, atom: int, actor: str, login: str, lease_id: str, claim_commit: str
) -> dict[str, Any]:
    return {
        "actor": actor,
        "atom": atom,
        "claim_commit": claim_commit,
        "lease_id": lease_id,
        "login": login,
    }


def _event_object(
    *,
    event_id: str,
    event_type: str,
    atom: int,
    actor: str,
    login: str,
    lease_id: str,
    reason: str,
    target_actor: str | None = None,
    target_login: str | None = None,
    claimed_at: datetime | None = None,
) -> dict[str, Any]:
    event = {
        "actor": actor,
        "atom": atom,
        "event_id": event_id,
        "lease_id": lease_id,
        "login": login,
        "reason": _safe_reason(reason),
        "type": event_type,
    }
    if event_type == "reclaim":
        if target_actor is None or target_login is None:
            raise ValueError("reclaim events require target actor and login")
        event["target_actor"] = target_actor
        event["target_login"] = target_login
    elif event_type == "claim" and claimed_at is not None:
        event["claimed_at"] = _timestamp(claimed_at)
    elif target_login is not None:
        raise ValueError("target_login is only valid for reclaim")
    elif claimed_at is not None:
        raise ValueError("claimed_at is only valid for claim")
    return event


def _event_document(event: dict[str, Any], entries: list[dict[str, Any]]) -> str:
    document = {
        "active_claims": sorted(
            entries,
            key=lambda entry: (
                int(entry["atom"]),
                str(entry["actor"]),
                str(entry["login"]),
                str(entry["lease_id"]),
            ),
        ),
        "event": event,
        "schema": CLAIM_EVENT_SCHEMA,
    }
    return _canonical_json(document)


def _strict_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    """JSON object hook that refuses duplicate keys instead of silently folding them."""
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise ValueError(f"duplicate JSON key {key!r}")
        value[key] = item
    return value


def _validate_event_document(message: str) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    if not isinstance(message, str):
        raise RuntimeError("claim log commit message is not text")
    try:
        document = json.loads(message, object_pairs_hook=_strict_object)
    except (json.JSONDecodeError, ValueError) as error:
        raise RuntimeError("claim log commit message is not JSON") from error
    if not isinstance(document, dict) or frozenset(document) != _DOCUMENT_KEYS:
        raise RuntimeError("claim log message has an invalid top-level schema")
    if document.get("schema") != CLAIM_EVENT_SCHEMA:
        raise RuntimeError("claim log message has an unsupported schema")
    if _canonical_json(document) != message:
        raise RuntimeError("claim log message is not strict canonical JSON")

    event = document.get("event")
    if not isinstance(event, dict):
        raise RuntimeError("claim log message has an invalid event object")
    event_id = _validate_id(event.get("event_id"), field="event_id")
    event_type = event.get("type")
    if not isinstance(event_type, str) or event_type not in _EVENT_TYPES:
        raise RuntimeError("claim log event type is unsupported")
    if event_type == "reclaim":
        expected_event_keys = _RECLAIM_EVENT_KEYS
    elif event_type == "claim" and "claimed_at" in event:
        expected_event_keys = _CLAIM_EVENT_KEYS
    else:
        expected_event_keys = _EVENT_KEYS
    if frozenset(event) != expected_event_keys:
        raise RuntimeError("claim log event has an invalid schema")
    atom = _validate_atom(event.get("atom"))
    actor = _validate_actor(event.get("actor"))
    login = _validate_login(event.get("login"))
    lease_id = _validate_id(event.get("lease_id"), field="lease_id")
    reason = _validate_reason(event.get("reason"))
    normalized_event = {
        "actor": actor,
        "atom": atom,
        "event_id": event_id,
        "lease_id": lease_id,
        "login": login,
        "reason": reason,
        "type": event_type,
    }
    if event_type == "reclaim":
        normalized_event["target_actor"] = _validate_actor(
            event.get("target_actor"), field="target_actor"
        )
        normalized_event["target_login"] = _validate_login(
            event.get("target_login"), field="target_login"
        )
    elif event_type == "claim" and "claimed_at" in event:
        claimed_at = event.get("claimed_at")
        if not isinstance(claimed_at, str):
            raise RuntimeError("claim event claimed_at is invalid")
        try:
            _parse_timestamp(claimed_at)
        except ValueError as error:
            raise RuntimeError("claim event claimed_at is malformed") from error
        normalized_event["claimed_at"] = _timestamp(_parse_timestamp(claimed_at))

    entries = document.get("active_claims")
    if not isinstance(entries, list) or len(entries) > MAX_ACTIVE_CLAIMS:
        raise RuntimeError("claim log active-claim cap is invalid")
    normalized_entries: list[dict[str, Any]] = []
    atoms: set[int] = set()
    actors: set[str] = set()
    leases: set[str] = set()
    for entry in entries:
        if not isinstance(entry, dict) or frozenset(entry) != _ENTRY_KEYS:
            raise RuntimeError("claim log active entry has an invalid schema")
        entry_atom = _validate_atom(entry.get("atom"))
        entry_actor = _validate_actor(entry.get("actor"))
        entry_login = _validate_login(entry.get("login"))
        entry_lease = _validate_id(entry.get("lease_id"), field="lease_id")
        entry_commit = entry.get("claim_commit")
        if entry_commit != "self":
            entry_commit = _validate_sha(entry_commit, field="claim_commit")
        if entry_atom in atoms:
            raise RuntimeError("claim log contains two active leases for one Atom")
        if entry_actor in actors:
            raise RuntimeError("claim log contains two active leases for one actor")
        if entry_lease in leases:
            raise RuntimeError("claim log contains a duplicate lease id")
        atoms.add(entry_atom)
        actors.add(entry_actor)
        leases.add(entry_lease)
        normalized_entries.append(
            _entry_object(
                atom=entry_atom,
                actor=entry_actor,
                login=entry_login,
                lease_id=entry_lease,
                claim_commit=entry_commit,
            )
        )

    if normalized_entries != sorted(
        normalized_entries,
        key=lambda entry: (
            int(entry["atom"]),
            str(entry["actor"]),
            str(entry["login"]),
            str(entry["lease_id"]),
        ),
    ):
        raise RuntimeError("claim log active state is not in canonical order")

    if event_type == "claim":
        matching = [
            entry
            for entry in normalized_entries
            if entry["atom"] == atom and entry["actor"] == actor and entry["lease_id"] == lease_id
        ]
        if len(matching) != 1 or matching[0]["login"] != login:
            raise RuntimeError("claim event does not match its active state")
    elif any(
        entry["atom"] == atom and entry["actor"] == actor and entry["lease_id"] == lease_id
        for entry in normalized_entries
    ):
        raise RuntimeError("release event leaves its lease active")

    return normalized_event, normalized_entries


def _looks_like_claim_document(message: str) -> bool:
    """Distinguish an ordinary main commit message from a malformed log document."""
    try:
        value = json.loads(message, object_pairs_hook=_strict_object)
    except (json.JSONDecodeError, ValueError):
        return False
    return isinstance(value, dict) and bool(_DOCUMENT_KEYS & frozenset(value))


def parse_claim_comment(body: str, *, url: str = "") -> ClaimLease | None:
    """Parse a projection comment for human tooling, never for arbitration.

    Both the historical three-line form and the current event/ref form are
    understood so old comments remain inspectable.  Neither form is read by the
    claim-log state machine.
    """
    lines = body.strip().splitlines()
    if len(lines) == 3 and ACTOR_PATTERN.fullmatch(lines[0]):
        claimed = CLAIMED_AT_PATTERN.fullmatch(lines[1])
        expiry = EXPIRY_PATTERN.fullmatch(lines[2])
        if claimed is None or expiry is None or int(expiry.group(1)) != CLAIM_MINUTES:
            return None
        try:
            claimed_at = _parse_timestamp(claimed.group(1))
        except ValueError:
            return None
        return ClaimLease(
            actor=lines[0],
            claimed_at=claimed_at,
            expires_after_minutes=CLAIM_MINUTES,
            comment_url=url,
        )
    if len(lines) != 5 or ACTOR_PATTERN.fullmatch(lines[0]) is None:
        return None
    event_match = re.fullmatch(r"claim_event_id: (\S+)", lines[1])
    ref_match = re.fullmatch(r"claim_ref: (\S+)", lines[2])
    lease_match = re.fullmatch(r"lease_id: (\S+)", lines[3])
    expiry = EXPIRY_PATTERN.fullmatch(lines[4])
    if (
        event_match is None
        or ref_match is None
        or ref_match.group(1) != CLAIM_LOG_REF
        or lease_match is None
        or EVENT_ID_PATTERN.fullmatch(event_match.group(1)) is None
        or LEASE_ID_PATTERN.fullmatch(lease_match.group(1)) is None
        or expiry is None
        or int(expiry.group(1)) != CLAIM_MINUTES
    ):
        return None
    return ClaimLease(
        actor=lines[0],
        claimed_at=None,
        expires_after_minutes=CLAIM_MINUTES,
        comment_url=url,
        lease_id=lease_match.group(1),
        event_id=event_match.group(1),
    )


def _legacy_claim_comment_shape(body: str) -> bool:
    """Recognize claim-shaped legacy text, including malformed candidates.

    A migration must not silently skip a damaged or competing claim comment.  This
    predicate is intentionally broader than :func:`parse_claim_comment`; the latter
    is the compatibility parser, while this one identifies text that needs a strict
    migration verdict.
    """
    lines = body.splitlines()
    if len(lines) != 3:
        return False
    return (
        lines[0].startswith("gordian-agent/")
        or lines[1].startswith("claimed_at:")
        or lines[2].startswith("expires_after_minutes:")
    )


def _canonical_claim_comment_shape(body: str) -> bool:
    """Recognize an already-upgraded claim projection as migration evidence."""
    lines = body.splitlines()
    if len(lines) != 5:
        return False
    return any(
        lines[index].startswith(prefix)
        for index, prefix in (
            (0, "gordian-agent/"),
            (1, "claim_event_id:"),
            (2, "claim_ref:"),
            (3, "lease_id:"),
        )
    )


def _read_legacy_claim_comment(
    repository: str,
    *,
    number: int,
    expected_actor: str,
    server_now: datetime | None = None,
) -> tuple[_LegacyClaimComment, datetime]:
    """Read exactly one valid legacy claim comment and its GitHub server clock.

    Only a legacy comment whose first line exactly matches ``expected_actor`` is
    eligible for adoption.  Canonical claim comments remain global migration
    evidence and therefore fail closed regardless of their actor.

    The endpoint response's ``Date`` header is deliberately returned alongside the
    selected comment.  Comment text is historical projection state, but its expiry
    decision is still made against a server clock, never against ``datetime.now``.
    """
    expected_actor = _validate_actor(expected_actor, field="expected_actor")
    payload, response = run_gh_json_response(
        [
            "api",
            _repository_path(repository, f"issues/{number}/comments?per_page=100"),
        ],
        allowed_statuses={200},
    )
    observed_now = _server_now(response)
    if server_now is not None and observed_now != server_now:
        # The caller may supply a previously observed clock only as a consistency
        # check; it can never replace the endpoint's own server timestamp.
        raise RuntimeError("legacy claim comment server time changed during adoption")
    if not isinstance(payload, list):
        raise RuntimeError(f"#{number}: comment response is malformed")

    candidates: list[_LegacyClaimComment] = []
    for raw in payload:
        if not isinstance(raw, dict):
            raise RuntimeError(f"#{number}: comment response is malformed")
        body = raw.get("body")
        if not isinstance(body, str):
            raise RuntimeError(f"#{number}: comment response has a malformed body")
        if _canonical_claim_comment_shape(body):
            raise RuntimeError(f"#{number}: canonical claim comment already exists")
        if not _legacy_claim_comment_shape(body):
            continue
        if body.splitlines()[0] != expected_actor:
            continue
        comment_id = raw.get("id")
        if isinstance(comment_id, bool) or not isinstance(comment_id, int) or comment_id <= 0:
            raise RuntimeError(f"#{number}: legacy claim comment has an invalid id")
        parsed = parse_claim_comment(body, url=str(raw.get("html_url", "")))
        if parsed is None or parsed.claimed_at is None:
            raise RuntimeError(f"#{number}: legacy claim comment timestamp is malformed")
        candidates.append(_LegacyClaimComment(comment_id, body, parsed))

    if len(candidates) != 1:
        if not candidates:
            raise RuntimeError(f"#{number}: no unambiguous legacy claim comment")
        raise RuntimeError(f"#{number}: legacy claim comments are ambiguous")
    selected = candidates[0]
    return selected, observed_now


def _update_claim_comment(
    repository: str, *, comment_id: int, body: str
) -> None:
    """Replace the adopted comment in place, preserving one projection."""
    payload, _ = run_gh_json_response(
        [
            "api",
            "--method",
            "PATCH",
            _repository_path(repository, f"issues/comments/{comment_id}"),
            "-f",
            f"body={body}",
        ],
        allowed_statuses={200},
    )
    if not isinstance(payload, dict):
        raise RuntimeError("updated claim comment response is malformed")
    returned_id = payload.get("id")
    if (
        isinstance(returned_id, bool)
        or not isinstance(returned_id, int)
        or returned_id != comment_id
    ):
        raise RuntimeError("updated claim comment response has the wrong id")


def _claim_comment(actor: str, claimed_at: datetime | ClaimLease, event_id: str = "") -> str:
    """Render a projection comment carrying event/ref identity.

    The datetime overload preserves the old fixture format for callers that only
    render a legacy diagnostic.  Successful claims always pass a ``ClaimLease``
    and therefore emit the event/ref form.
    """
    if isinstance(claimed_at, datetime):
        return (
            f"{actor}\n"
            f"claimed_at: {_timestamp(claimed_at)}\n"
            f"expires_after_minutes: {CLAIM_MINUTES}"
        )
    if claimed_at.actor != actor:
        raise ValueError("claim comment actor does not match lease")
    if EVENT_ID_PATTERN.fullmatch(event_id) is None:
        raise ValueError("claim comment requires a canonical event id")
    if LEASE_ID_PATTERN.fullmatch(claimed_at.lease_id) is None:
        raise ValueError("claim comment requires a canonical lease id")
    return (
        f"{actor}\n"
        f"claim_event_id: {event_id}\n"
        f"claim_ref: {CLAIM_LOG_REF}\n"
        f"lease_id: {claimed_at.lease_id}\n"
        f"expires_after_minutes: {CLAIM_MINUTES}"
    )


def _actor() -> str:
    actor = os.environ.get("GORDIAN_ACTOR", "")
    if ACTOR_PATTERN.fullmatch(actor) is None:
        raise GitHubConfigurationError(
            "GORDIAN_ACTOR must match gordian-agent/<harness>/<run-id>"
        )
    return actor


def _coordinator_recovery_authorized(auth: Any) -> bool:
    """Return whether this process carries the explicit stale-lease capability."""
    capabilities = getattr(auth, "capabilities", ())
    if "coordinator_recovery" in capabilities:
        return True
    return os.environ.get("GORDIAN_COORDINATOR", "").strip().lower() in {
        "1",
        "true",
        "yes",
    }


def _repository_path(repository: str, suffix: str) -> str:
    owner, separator, name = repository.partition("/")
    if not separator or not owner or not name or "/" in name:
        raise ValueError("repository must be OWNER/NAME")
    return f"repos/{owner}/{name}/{suffix}"


def _api(
    arguments: list[str], *, allowed_statuses: set[int] | frozenset[int]
) -> GitHubApiResponse:
    """Single API boundary used by all claim-log operations."""
    return run_gh_response(arguments, allowed_statuses=allowed_statuses)


def _read_ref(repository: str, ref: str = CLAIM_LOG_BRANCH) -> _RefSnapshot:
    if ref != CLAIM_LOG_BRANCH and not re.fullmatch(r"[A-Za-z0-9._/-]+", ref):
        raise ValueError("invalid Git ref name")
    response = _api(
        ["api", _repository_path(repository, f"git/ref/heads/{ref}")],
        allowed_statuses={200, 404},
    )
    server_now = _server_now(response)
    if response.status == 404:
        return _RefSnapshot(head_sha=None, server_now=server_now)
    payload = _json_body(response, operation="read claim ref")
    if not isinstance(payload, dict) or payload.get("ref") != f"refs/heads/{ref}":
        raise RuntimeError("claim log ref response has an invalid ref")
    object_data = payload.get("object")
    if not isinstance(object_data, dict) or object_data.get("type") != "commit":
        raise RuntimeError("claim log ref does not point to a commit")
    head_sha = _validate_sha(object_data.get("sha"), field="ref object")
    return _RefSnapshot(head_sha=head_sha, server_now=server_now)


def _read_commit(repository: str, sha: str) -> _CommitSnapshot:
    sha = _validate_sha(sha, field="commit")
    response = _api(
        ["api", _repository_path(repository, f"git/commits/{sha}")],
        allowed_statuses={200},
    )
    server_now = _server_now(response)
    payload = _json_body(response, operation="read claim commit")
    if not isinstance(payload, dict):
        raise RuntimeError("claim commit response is not an object")
    response_sha = _validate_sha(payload.get("sha"), field="commit response")
    if response_sha != sha:
        raise RuntimeError("claim commit response SHA does not match requested SHA")
    message = payload.get("message")
    tree = payload.get("tree")
    committer = payload.get("committer")
    if not isinstance(message, str) or not isinstance(tree, dict):
        raise RuntimeError("claim commit response has an invalid message or tree")
    tree_sha = _validate_sha(tree.get("sha"), field="commit tree")
    if not isinstance(committer, dict) or not isinstance(committer.get("date"), str):
        raise RuntimeError("claim commit has no server committer date")
    try:
        committed_at = _parse_timestamp(committer["date"])
    except ValueError as error:
        raise RuntimeError("claim commit committer.date is malformed") from error
    parents_data = payload.get("parents")
    if not isinstance(parents_data, list):
        raise RuntimeError("claim commit parents are malformed")
    parents = tuple(
        _validate_sha(parent.get("sha"), field="commit parent")
        for parent in parents_data
        if isinstance(parent, dict)
    )
    if len(parents) != len(parents_data):
        raise RuntimeError("claim commit parent is malformed")
    return _CommitSnapshot(
        sha=sha,
        tree_sha=tree_sha,
        message=message,
        committed_at=committed_at,
        parents=parents,
        server_now=server_now,
    )


def _lease_atom(lease: ClaimLease) -> int:
    if lease.atom is None:
        raise RuntimeError("claim lease has no Atom number")
    return lease.atom


def _raw_entries(state: ClaimLogState, *, parent_sha: str | None) -> list[dict[str, Any]]:
    if parent_sha is None:
        raise RuntimeError("claim log state has no observed parent")
    entries: list[dict[str, Any]] = []
    for lease in state.active_claims:
        claim_commit = lease.claim_commit or parent_sha
        if claim_commit == "self":
            claim_commit = parent_sha
        _validate_sha(claim_commit, field="claim_commit")
        entries.append(
            _entry_object(
                atom=_lease_atom(lease),
                actor=lease.actor,
                login=lease.login,
                lease_id=lease.lease_id,
                claim_commit=claim_commit,
            )
        )
    return entries


def _lease_from_entry(
    entry: dict[str, Any], *, claimed_at: datetime, event_id: str, atom: int
) -> ClaimLease:
    return ClaimLease(
        actor=str(entry["actor"]),
        claimed_at=claimed_at,
        expires_after_minutes=CLAIM_MINUTES,
        login=str(entry["login"]),
        lease_id=str(entry["lease_id"]),
        claim_commit=str(entry["claim_commit"]),
        event_id=event_id,
        atom=atom,
    )


def _entry_identity(entry: dict[str, Any]) -> tuple[int, str, str, str]:
    return (
        int(entry["atom"]),
        str(entry["actor"]),
        str(entry["login"]),
        str(entry["lease_id"]),
    )


def _normalized_parent_entries(
    entries: list[dict[str, Any]], *, parent_sha: str
) -> list[dict[str, Any]]:
    """Resolve the predecessor's ``self`` claim marker for child comparison."""
    return [
        {
            **entry,
            "claim_commit": parent_sha
            if entry["claim_commit"] == "self"
            else entry["claim_commit"],
        }
        for entry in entries
    ]


def _validate_transition(
    *,
    parent_sha: str,
    parent_entries: list[dict[str, Any]],
    event: dict[str, Any],
    entries: list[dict[str, Any]],
) -> None:
    """Require the head document to be exactly one event after its parent.

    The complete active set is duplicated in every event so readers can validate a
    head without trusting comments or a local cache.  ``self`` is the one deliberate
    representation change: a claim commit uses it at creation and the next event
    resolves it to the observed parent SHA.
    """
    predecessor = _normalized_parent_entries(parent_entries, parent_sha=parent_sha)
    predecessor_by_id = {_entry_identity(entry): entry for entry in predecessor}
    current_by_id = {_entry_identity(entry): entry for entry in entries}
    common = set(predecessor_by_id) & set(current_by_id)
    for identity in common:
        if current_by_id[identity] != predecessor_by_id[identity]:
            raise RuntimeError("claim-log event changed an unrelated active lease")

    event_type = str(event["type"])
    if event_type == "claim":
        added = set(current_by_id) - set(predecessor_by_id)
        removed = set(predecessor_by_id) - set(current_by_id)
        if len(added) != 1 or removed:
            raise RuntimeError("claim-log claim does not add exactly one active lease")
        identity = next(iter(added))
        expected_identity = (
            int(event["atom"]),
            str(event["actor"]),
            str(event["login"]),
            str(event["lease_id"]),
        )
        added_entry = current_by_id[identity]
        if identity != expected_identity or added_entry["claim_commit"] != "self":
            raise RuntimeError("claim-log claim added lease does not match its event")
        return

    removed = set(predecessor_by_id) - set(current_by_id)
    added = set(current_by_id) - set(predecessor_by_id)
    if len(removed) != 1 or added:
        raise RuntimeError("claim-log release does not remove exactly one active lease")
    removed_entry = predecessor_by_id[next(iter(removed))]
    if (
        int(removed_entry["atom"]) != int(event["atom"])
        or str(removed_entry["lease_id"]) != str(event["lease_id"])
    ):
        raise RuntimeError("claim-log release removes the wrong lease")
    # A release or abort issued by the owning actor has to carry its authenticated login.
    if (
        str(removed_entry["actor"]) == str(event["actor"])
        and str(removed_entry["login"]) != str(event["login"])
    ):
        raise RuntimeError("claim-log release login does not match its lease")


def _validate_release_authority(
    *,
    parent_sha: str,
    parent_entries: list[dict[str, Any]],
    event: dict[str, Any],
    event_commit: _CommitSnapshot,
    commits: dict[str, _CommitSnapshot],
) -> None:
    """Require release/abort ownership or an explicit stale reclaim.

    Claim timestamps are advisory display metadata.  GitHub commit timestamps are
    caller-controlled and therefore cannot authorize automatic foreign eviction.
    """
    if event["type"] not in {"release", "abort", "reclaim"}:
        return
    predecessor = _normalized_parent_entries(parent_entries, parent_sha=parent_sha)
    # The transition validator has already proved exactly one removal; identify it
    # from the event's atom and lease without trusting the event's actor/login.
    removed = [
        entry
        for entry in predecessor
        if int(entry["atom"]) == int(event["atom"])
        and str(entry["lease_id"]) == str(event["lease_id"])
    ]
    if len(removed) != 1:
        raise RuntimeError("claim-log release removes an unidentifiable lease")
    lease = removed[0]
    if event["type"] == "reclaim":
        if str(event.get("target_actor")) != str(lease["actor"]):
            raise RuntimeError("claim-log reclaim target actor does not match its lease")
        if str(event.get("target_login")) != str(lease["login"]):
            raise RuntimeError("claim-log reclaim target login does not match its lease")
        return
    if str(lease["actor"]) != str(event["actor"]):
        raise RuntimeError(
            f"claim-log {event['type']} belongs to a different actor; "
            "manual/admin recovery is required"
        )
    if str(lease["login"]) != str(event["login"]):
        raise RuntimeError(f"claim-log {event['type']} login does not match its lease")


def _validate_claim_history(
    repository: str, head: _CommitSnapshot
) -> _ValidatedClaimHistory:
    """Validate the complete event path from ``head`` to accepted ``main``.

    The claim ref is an append-only linked list, not a set of independently
    trusted snapshots.  Reading only the head and the active claim commits would
    allow a forged parent or a sibling claim commit to hide a broken transition.
    Every event commit is therefore fetched, checked for a single parent and
    invariant tree, and validated against the complete state in its predecessor.
    """
    common_tree = head.tree_sha
    current = head
    seen: dict[str, _CommitSnapshot] = {}
    event_records: list[tuple[_CommitSnapshot, dict[str, Any], list[dict[str, Any]]]] = []
    event_ids: set[str] = set()

    while True:
        if current.sha in seen:
            raise RuntimeError("claim-log history contains a repeated commit")
        seen[current.sha] = current
        if current.tree_sha != common_tree:
            raise RuntimeError("claim-log event changed the parent tree")
        try:
            event, entries = _validate_event_document(current.message)
        except RuntimeError as error:
            # The first ordinary commit is the only non-event node allowed in the
            # path.  A JSON-looking parent is treated as a malformed event rather
            # than silently becoming an accepted root.
            if _looks_like_claim_document(current.message):
                raise RuntimeError("claim-log parent event document is malformed") from error
            if not event_records:
                raise RuntimeError("claim-log head is not a valid event document") from None
            root = current
            break

        if current.sha == head.sha and len(current.parents) != 1:
            raise RuntimeError("claim-log event commit must have exactly one parent")
        if len(current.parents) != 1:
            raise RuntimeError("claim-log event commit must have exactly one parent")
        event_id = str(event["event_id"])
        if event_id in event_ids:
            raise RuntimeError("claim-log history contains a duplicate event id")
        event_ids.add(event_id)
        event_records.append((current, event, entries))
        parent = _read_commit(repository, current.parents[0])
        if parent.tree_sha != common_tree:
            raise RuntimeError("claim-log event changed the parent tree")
        current = parent

    # Validate transitions in causal order.  A root release/abort cannot invent a
    # predecessor state, and therefore gets the same explicit root diagnostic as
    # the historical reader implementation.
    if event_records[-1][1]["type"] != "claim":
        raise RuntimeError("claim-log root parent is not an accepted main commit")
    _validate_initial_parent(repository, root.sha, tree_sha=common_tree)
    parent_sha = root.sha
    parent_entries: list[dict[str, Any]] = []
    commits_by_sha = {commit.sha: commit for commit, _, _ in event_records}
    for commit, event, entries in reversed(event_records):
        _validate_transition(
            parent_sha=parent_sha,
            parent_entries=parent_entries,
            event=event,
            entries=entries,
        )
        _validate_release_authority(
            parent_sha=parent_sha,
            parent_entries=parent_entries,
            event=event,
            event_commit=commit,
            commits=commits_by_sha,
        )
        parent_sha = commit.sha
        parent_entries = entries

    # An active claim must name a claim event on this exact validated ancestry.
    # Merely fetching a SHA and finding a claim-shaped message is insufficient: a
    # sibling or unrelated commit must never be able to introduce live authority.
    by_sha = {commit.sha: (commit, event, entries) for commit, event, entries in event_records}
    active_entries = tuple(event_records[0][2])
    for entry in active_entries:
        claim_sha = head.sha if entry["claim_commit"] == "self" else str(entry["claim_commit"])
        claim_record = by_sha.get(claim_sha)
        if claim_record is None:
            raise RuntimeError("active claim commit is not an ancestor of the claim-log head")
        claim_commit, claim_event, claim_entries = claim_record
        if claim_event["type"] != "claim":
            raise RuntimeError("active claim points to a non-claim event commit")
        matching_claim = next(
            (
                candidate
                for candidate in claim_entries
                if candidate["atom"] == entry["atom"]
                and candidate["actor"] == entry["actor"]
                and candidate["lease_id"] == entry["lease_id"]
                and candidate["login"] == entry["login"]
            ),
            None,
        )
        if matching_claim is None:
            raise RuntimeError("active claim is absent from its claim event commit")
        if matching_claim["claim_commit"] != "self":
            raise RuntimeError("active claim does not identify its claim event commit")
        if claim_commit.tree_sha != common_tree:
            raise RuntimeError("claim event commit changed the claim-log tree")

    return _ValidatedClaimHistory(
        root=root,
        commits=tuple(commit for commit, _, _ in reversed(event_records)),
        events=tuple(event for _, event, _ in event_records),
        entries=active_entries,
    )


def _read_claim_log(repository: str) -> ClaimLogState:
    """Read and validate the current claim-log head and its complete ancestry."""
    ref = _read_ref(repository)
    if ref.head_sha is None:
        return ClaimLogState(
            head_sha=None,
            tree_sha=None,
            active_claims=(),
            event=None,
            server_now=ref.server_now,
            message="",
            parents=(),
            events=(),
        )
    head = _read_commit(repository, ref.head_sha)
    history = _validate_claim_history(repository, head)
    active: list[ClaimLease] = []
    by_sha = {commit.sha: commit for commit in history.commits}
    event_by_sha = {
        commit.sha: event
        for commit, event in zip(history.commits, reversed(history.events), strict=True)
    }
    head_event = history.events[0]
    for entry in history.entries:
        claim_sha = head.sha if entry["claim_commit"] == "self" else str(entry["claim_commit"])
        claim_commit = by_sha[claim_sha]
        claim_event = event_by_sha[claim_sha]
        active.append(
            _lease_from_entry(
                {**entry, "claim_commit": claim_sha},
                claimed_at=(
                    _parse_timestamp(str(claim_event["claimed_at"]))
                    if "claimed_at" in claim_event
                    else claim_commit.committed_at
                ),
                event_id=str(claim_event["event_id"]),
                atom=int(entry["atom"]),
            )
        )
    return ClaimLogState(
        head_sha=head.sha,
        tree_sha=head.tree_sha,
        active_claims=tuple(active),
        event=head_event,
        server_now=ref.server_now,
        message=head.message,
        parents=head.parents,
        events=history.events,
    )


def _validate_initial_parent(
    repository: str, parent_sha: str, *, tree_sha: str | None = None
) -> None:
    """Require a non-log root parent to be on the accepted ``main`` frontier."""
    main_ref = _read_ref(repository, ref="main")
    if main_ref.head_sha is None:
        raise RuntimeError("claim-log root parent cannot be checked: accepted main is missing")
    if main_ref.head_sha == parent_sha:
        return
    response = _api(
        [
            "api",
            _repository_path(
                repository, f"compare/{parent_sha}...{main_ref.head_sha}"
            ),
        ],
        allowed_statuses={200},
    )
    _server_now(response)
    payload = _json_body(response, operation="check claim-log root parent")
    if not isinstance(payload, dict) or payload.get("status") != "ahead":
        raise RuntimeError("claim-log root parent is not an accepted main ancestor")
    base = payload.get("base_commit")
    merge_base = payload.get("merge_base_commit")
    if not all(isinstance(value, dict) for value in (base, merge_base)):
        raise RuntimeError("claim-log root parent comparison is malformed")
    ahead_by = payload.get("ahead_by")
    behind_by = payload.get("behind_by")
    total_commits = payload.get("total_commits")
    if any(
        isinstance(value, bool) or not isinstance(value, int) or value < 0
        for value in (ahead_by, behind_by, total_commits)
    ) or ahead_by <= 0 or behind_by != 0 or total_commits != ahead_by:
        raise RuntimeError("claim-log root parent comparison has invalid commit counts")
    if (
        _validate_sha(base.get("sha"), field="comparison base") != parent_sha
        or _validate_sha(merge_base.get("sha"), field="comparison merge base") != parent_sha
    ):
        raise RuntimeError("claim-log root parent is not an accepted main ancestor")


def _read_main_tree(repository: str) -> tuple[str, str, datetime]:
    main_ref = _read_ref(repository, ref="main")
    if main_ref.head_sha is None:
        raise RuntimeError("accepted main ref is missing")
    main_commit = _read_commit(repository, main_ref.head_sha)
    return main_ref.head_sha, main_commit.tree_sha, main_ref.server_now


def _create_empty_commit(
    repository: str, *, parent_sha: str, tree_sha: str, message: str
) -> str:
    response = _api(
        [
            "api",
            "--method",
            "POST",
            _repository_path(repository, "git/commits"),
            "-f",
            f"message={message}",
            "-f",
            f"tree={tree_sha}",
            "-f",
            f"parents[]={parent_sha}",
        ],
        allowed_statuses={201},
    )
    payload = _json_body(response, operation="create claim event commit")
    if not isinstance(payload, dict):
        raise RuntimeError("created claim commit response is not an object")
    return _validate_sha(payload.get("sha"), field="created commit")


def _publish_ref(
    repository: str, *, commit_sha: str, observed_head: str | None, force: bool = False
) -> GitHubApiResponse:
    """Publish one event using create-once or fast-forward-only update."""
    if force:
        raise RuntimeError("forced claim-log ref updates are forbidden")
    commit_sha = _validate_sha(commit_sha, field="new claim-log head")
    if observed_head is None:
        response = _api(
            [
                "api",
                "--method",
                "POST",
                _repository_path(repository, "git/refs"),
                "-f",
                f"ref={CLAIM_LOG_REF}",
                "-f",
                f"sha={commit_sha}",
            ],
            allowed_statuses={201, 409, 422},
        )
    else:
        observed_head = _validate_sha(observed_head, field="observed claim-log head")
        response = _api(
            [
                "api",
                "--method",
                "PATCH",
                _repository_path(repository, f"git/refs/heads/{CLAIM_LOG_BRANCH}"),
                "-f",
                f"sha={commit_sha}",
                "-F",
                "force=false",
            ],
            allowed_statuses={200, 409, 422},
        )
    if response.status in (409, 422):
        raise _CasConflict(f"claim-log ref CAS conflict ({response.status})")
    return response


def _state_has_lease(
    state: ClaimLogState, *, atom: int, actor: str, lease_id: str | None = None
) -> ClaimLease | None:
    for lease in state.active_claims:
        if (
            _lease_atom(lease) == atom
            and lease.actor == actor
            and (lease_id is None or lease.lease_id == lease_id)
        ):
            return lease
    return None


def _existing_actor_claim(
    state: ClaimLogState, *, atom: int, actor: str, login: str, event_id: str
) -> _AppendResult | None:
    """Return a live same-actor retry, or fail closed for a stale lease.

    A lease is a lock even when its expiry has passed: accepting a new claim as an
    idempotent retry would make the old projection indistinguishable from the new
    one.  The caller must append an explicit release/abort first.
    """
    existing_actor = next(
        (lease for lease in state.active_claims if lease.actor == actor), None
    )
    if existing_actor is None:
        return None
    if not existing_actor.is_live(_server_clock(state)):
        raise RuntimeError(
            f"#{_lease_atom(existing_actor)}: actor has a stale claim; release it explicitly "
            "before claiming again"
        )
    if _lease_atom(existing_actor) != atom or existing_actor.login != login:
        raise RuntimeError(
            f"#{_lease_atom(existing_actor)}: actor already owns an active claim"
        )
    return _AppendResult(
        event_id=existing_actor.event_id or event_id,
        commit_sha=existing_actor.claim_commit or state.head_sha or "",
        state=state,
        lease=existing_actor,
        already_present=True,
    )


def _state_event_matches(
    state: ClaimLogState,
    *,
    event_type: str,
    atom: int,
    actor: str,
    lease_id: str,
    event_id: str | None = None,
) -> bool:
    return (
        _matching_state_event(
            state,
            event_type=event_type,
            atom=atom,
            actor=actor,
            lease_id=lease_id,
            event_id=event_id,
        )
        is not None
    )


def _matching_state_event(
    state: ClaimLogState,
    *,
    event_type: str,
    atom: int,
    actor: str,
    lease_id: str,
    event_id: str | None = None,
) -> dict[str, Any] | None:
    """Return the requested event from validated ancestry, if present.

    ``ClaimLogState.event`` is only the current head event.  A successful append
    may be followed by another append before its response is read, so callers
    that need event identity must search the validated history rather than use
    the head event as a proxy.
    """
    events = state.events or ((state.event,) if state.event is not None else ())
    return next(
        (
            event
            for event in events
            if event.get("type") == event_type
            and event.get("atom") == atom
            and event.get("actor") == actor
            and event.get("lease_id") == lease_id
            and (event_id is None or event.get("event_id") == event_id)
        ),
        None,
    )


def _server_clock(state: ClaimLogState) -> datetime:
    if state.server_now is None:
        raise RuntimeError(
            "claim-log response has no server Date; refusing local-clock lease decision"
        )
    return state.server_now


def _append_event(
    repository: str,
    *,
    event_type: str,
    atom: int,
    actor: str,
    login: str,
    lease_id: str,
    reason: str,
    target_actor: str | None = None,
    target_login: str | None = None,
    coordinator_authorized: bool = False,
) -> _AppendResult:
    """Append one claim-log transition with bounded deterministic retries.

    Reclaim is deliberately separate from release/abort.  It is available only to
    an explicitly authorized coordinator and only when the target lease is expired
    according to the GitHub response Date observed in the same read.  A live foreign
    lease is never removable.
    """
    if event_type not in _EVENT_TYPES:
        raise ValueError("unsupported claim event type")
    _validate_atom(atom)
    _validate_actor(actor)
    _validate_login(login)
    _validate_id(lease_id, field="lease_id")
    if target_actor is not None:
        target_actor = _validate_actor(target_actor, field="target_actor")
        if event_type in {"release", "abort"} and target_actor != actor:
            raise RuntimeError(
                f"claim-log {event_type} target actor {target_actor} does not match "
                f"event actor {actor}"
            )
    if event_type == "reclaim":
        if not coordinator_authorized:
            raise RuntimeError("claim-log reclaim requires an authorized coordinator")
        if target_actor is None or target_login is None:
            raise ValueError("reclaim requires target actor and login")
        target_login = _validate_login(target_login, field="target_login")
    elif target_login is not None:
        raise ValueError("target_login is only valid for reclaim")
    reason = _safe_reason(reason)
    event_id = _new_id()
    event = _event_object(
        event_id=event_id,
        event_type=event_type,
        atom=atom,
        actor=actor,
        login=login,
        lease_id=lease_id,
        reason=reason,
        target_actor=target_actor,
        target_login=target_login,
    )
    claim_timestamp: datetime | None = None

    for _attempt in range(MAX_CAS_RETRIES):
        state = _read_claim_log(repository)
        server_now = _server_clock(state)
        if event_type == "claim" and claim_timestamp is None:
            claim_timestamp = server_now
            event = _event_object(
                event_id=event_id,
                event_type=event_type,
                atom=atom,
                actor=actor,
                login=login,
                lease_id=lease_id,
                reason=reason,
                claimed_at=claim_timestamp,
            )
        parent_sha = state.head_sha
        tree_sha = state.tree_sha
        if parent_sha is None and tree_sha is not None:
            raise RuntimeError("claim log state has a tree without an observed parent")
        if parent_sha is not None and tree_sha is None:
            raise RuntimeError("claim log state has a parent without its tree")
        if parent_sha is None:
            parent_sha, tree_sha, _ = _read_main_tree(repository)
        if event_type == "claim":
            existing = _existing_actor_claim(
                state,
                atom=atom,
                actor=actor,
                login=login,
                event_id=event_id,
            )
            if existing is not None:
                return existing
            existing_atom = state.for_atom(atom)
            if existing_atom is not None:
                live = existing_atom.is_live(_server_clock(state))
                status = "live" if live else "stale"
                raise RuntimeError(
                    f"#{atom}: another actor holds a {status} claim ({existing_atom.actor})"
                )
            if len(state.active_claims) >= MAX_ACTIVE_CLAIMS:
                raise RuntimeError(
                    f"claim-log active cap {MAX_ACTIVE_CLAIMS} reached; release a claim first"
                )
            entries = _raw_entries(state, parent_sha=parent_sha)
            entries.append(
                _entry_object(
                    atom=atom,
                    actor=actor,
                    login=login,
                    lease_id=lease_id,
                    claim_commit="self",
                )
            )
        else:
            owner_actor = target_actor or actor
            target = _state_has_lease(state, atom=atom, actor=owner_actor, lease_id=lease_id)
            if target is None:
                if _state_event_matches(
                    state,
                    event_type=event_type,
                    atom=atom,
                    actor=actor,
                    lease_id=lease_id,
                    event_id=event_id,
                ):
                    return _AppendResult(
                        event_id=event_id,
                        commit_sha=state.head_sha or "",
                        state=state,
                        lease=None,
                        already_present=True,
                    )
                foreign = state.for_atom(atom)
                if foreign is not None:
                    raise RuntimeError(f"#{atom}: claim changed before release")
                raise RuntimeError(f"#{atom}: requested lease is not active")
            expected_login = target_login if event_type == "reclaim" else login
            if target.login != expected_login:
                raise RuntimeError(f"claim-log {event_type} login does not match its lease")
            if event_type == "reclaim" and target.is_live(_server_clock(state)):
                raise RuntimeError(f"#{atom}: live foreign lease cannot be reclaimed")
            entries = [
                entry
                for entry in _raw_entries(state, parent_sha=parent_sha)
                if not (
                    int(entry["atom"]) == atom
                    and entry["actor"] == owner_actor
                    and entry["lease_id"] == lease_id
                )
            ]

        message = _event_document(event, entries)
        commit_sha = _create_empty_commit(
            repository,
            parent_sha=parent_sha,
            tree_sha=tree_sha,
            message=message,
        )
        try:
            _publish_ref(
                repository,
                commit_sha=commit_sha,
                observed_head=state.head_sha,
            )
        except _CasConflict:
            current = _read_claim_log(repository)
            if event_type == "claim":
                existing = _existing_actor_claim(
                    current,
                    atom=atom,
                    actor=actor,
                    login=login,
                    event_id=event_id,
                )
                if existing is not None:
                    return existing
                cap_reached = len(current.active_claims) >= MAX_ACTIVE_CLAIMS
                if current.for_atom(atom) is not None or cap_reached:
                    raise RuntimeError(f"#{atom}: lost claim-log CAS race") from None
            elif _state_event_matches(
                current,
                event_type=event_type,
                atom=atom,
                actor=actor,
                lease_id=lease_id,
                event_id=event_id,
            ):
                return _AppendResult(
                    event_id=event_id,
                    commit_sha=current.head_sha or "",
                    state=current,
                    lease=None,
                    already_present=True,
                )
            continue
        except Exception as error:
            # A successful GitHub mutation can lose its HTTP response.  A fresh
            # canonical read is the only safe recovery; never infer failure from
            # the exception alone.
            try:
                current = _read_claim_log(repository)
            except Exception:
                raise error from None
            if event_type == "claim":
                existing = _existing_actor_claim(
                    current,
                    atom=atom,
                    actor=actor,
                    login=login,
                    event_id=event_id,
                )
                if existing is not None:
                    return existing
            elif _state_event_matches(
                current,
                event_type=event_type,
                atom=atom,
                actor=actor,
                lease_id=lease_id,
                event_id=event_id,
            ):
                return _AppendResult(
                    event_id=event_id,
                    commit_sha=current.head_sha or "",
                    state=current,
                    lease=None,
                    already_present=True,
                )
            raise error

        current = _read_claim_log(repository)
        if current.head_sha == commit_sha:
            if current.tree_sha != tree_sha:
                raise RuntimeError("claim-log event changed the parent tree")
            if current.parents and current.parents != (parent_sha,):
                raise RuntimeError("claim-log event does not parent the observed head")
        if event_type == "claim":
            lease = _state_has_lease(current, atom=atom, actor=actor, lease_id=lease_id)
            if lease is None:
                raise RuntimeError("claim-log append succeeded but state did not contain claim")
            return _AppendResult(
                event_id=lease.event_id or event_id,
                commit_sha=current.head_sha or commit_sha,
                state=current,
                lease=lease,
            )
        matched_event = _matching_state_event(
            current,
            event_type=event_type,
            atom=atom,
            actor=actor,
            lease_id=lease_id,
            event_id=event_id,
        )
        return _AppendResult(
            # The requested event may be an ancestor of a newer head.  Return
            # its identity, never the identity of the later head event.
            event_id=str(matched_event["event_id"]) if matched_event is not None else event_id,
            commit_sha=current.head_sha or commit_sha,
            state=current,
            lease=None,
        )
    raise RuntimeError(f"claim-log CAS conflicted after {MAX_CAS_RETRIES} attempts")


def _issue_payload(repository: str, number: int) -> dict[str, Any]:
    payload = run_gh_json(
        [
            "issue",
            "view",
            str(number),
            "--repo",
            repository,
            "--json",
            "number,state,assignees",
        ]
    )
    if not isinstance(payload, dict):
        raise RuntimeError(f"#{number}: unexpected issue response")
    response_number = payload.get("number")
    state = payload.get("state")
    assignees = payload.get("assignees")
    if (
        isinstance(response_number, bool)
        or not isinstance(response_number, int)
        or response_number != number
        or not isinstance(state, str)
        or state.upper() not in {"OPEN", "CLOSED"}
        or not isinstance(assignees, list)
    ):
        raise RuntimeError(f"#{number}: malformed issue response")
    logins_list: list[str] = []
    for item in assignees:
        if not isinstance(item, dict) or not isinstance(item.get("login"), str):
            raise RuntimeError(f"#{number}: malformed assignee response")
        logins_list.append(_validate_login(item["login"], field="assignee login"))
    logins = tuple(sorted(logins_list))
    return {
        "number": response_number,
        "state": state,
        "assignees": logins,
    }


def _issue_claim(
    repository: str, number: int, *, state: ClaimLogState | None = None
) -> IssueClaim:
    state = state or _read_claim_log(repository)
    payload = _issue_payload(repository, number)
    lease = state.for_atom(number)
    drift: list[str] = []
    if lease is not None and payload["state"].upper() != "OPEN":
        drift.append("claim-log lease on closed issue")
    if lease is not None and tuple(payload["assignees"]) != (lease.login,):
        drift.append("assignee projection differs from claim log")
    if lease is None and payload["assignees"]:
        drift.append("assignee exists without claim-log lease")
    return IssueClaim(
        number=payload["number"],
        state=payload["state"],
        assignees=payload["assignees"],
        lease=lease,
        leases=state.active_claims,
        projection_drift=tuple(drift),
    )


def require_live_claim(
    *, repository: str, number: int, login: str, now: datetime | None = None
) -> ClaimLease:
    """Require the authenticated actor's live canonical lease.

    ``now`` remains accepted for source compatibility but is intentionally ignored.
    The GitHub response Date is the sole expiry clock.
    """
    actor = _actor()
    state = _read_claim_log(repository)
    server_now = _server_clock(state)
    claim_snapshot = _issue_claim(repository, number, state=state)
    lease = claim_snapshot.lease
    if claim_snapshot.state.upper() != "OPEN":
        raise RuntimeError(f"#{number}: cannot mutate through a claim on a closed issue")
    if lease is None:
        raise RuntimeError(f"#{number}: no canonical claim-log lease")
    if lease.actor != actor:
        raise RuntimeError(f"#{number}: claim belongs to {lease.actor}, not current actor {actor}")
    if lease.login != login:
        raise RuntimeError(
            f"#{number}: claim authenticated login is {lease.login!r}, not {login!r}"
        )
    if not lease.is_live(server_now):
        raise RuntimeError(f"#{number}: bootstrap claim expired at {_timestamp(lease.expires_at)}")
    return lease


def select_unclaimed(
    rows: tuple[DerivedRow, ...], claims: dict[int, IssueClaim]
) -> DerivedRow | None:
    """Return the first row without a canonical active lease."""
    for row in rows:
        claim_snapshot = claims.get(row.number)
        if claim_snapshot is None or claim_snapshot.lease is None:
            return row
    return None


def _ready_rows(
    repository: str, closure_root: Path, closure_schema: Path
) -> tuple[DerivedRow, ...]:
    owner, separator, name = repository.partition("/")
    if not separator:
        raise ValueError("repository must be OWNER/NAME")
    issues = fetch_issues(owner, name)
    # Issue assignees are an eventually-consistent projection of the claim log.
    # Strip them before deriving the Ready set so a stale or human assignment
    # cannot suppress canonical-log selection.  The original issue payload is
    # reread separately at the projection boundary.
    issues = tuple(replace(issue, assignees=()) for issue in issues)
    schema = load_closure_schema(closure_schema)
    satisfied, unevidenced = bootstrap_satisfied(
        issues,
        closure_root=closure_root,
        schema=schema,
    )
    if unevidenced:
        raise RuntimeError(
            "closed without validating closure record: "
            + ", ".join(f"#{number}" for number in unevidenced)
        )
    return ready_set(derive(issues, satisfied=satisfied))


def _set_status(board, number: int, desired: str) -> None:
    item = board.items.get(number)
    if item is None:
        raise RuntimeError(f"#{number}: absent from Project 9")
    if STATUS_FIELD not in board.fields:
        raise RuntimeError("Project 9 has no Status field")
    apply_change(
        board,
        Change(
            number=number,
            item_id=item.item_id,
            field_name=STATUS_FIELD,
            current=item.status,
            desired=desired,
        ),
    )


def _projection_abort(
    repository: str, *, number: int, actor: str, lease: ClaimLease, reason: str
) -> _AppendResult | None:
    state = _read_claim_log(repository)
    current = _state_has_lease(state, atom=number, actor=actor, lease_id=lease.lease_id)
    if current is None:
        return None
    return _append_event(
        repository,
        event_type="abort",
        atom=number,
        actor=actor,
        login=current.login,
        lease_id=current.lease_id,
        reason=reason,
    )


def _reconcile_claim_projection(
    *,
    repository: str,
    number: int,
    login: str,
    board: Any | None,
    previous_status: str | None,
    actor: str,
    assignee_added: bool = False,
    status_attempted: bool = False,
    status_changed: bool = False,
    project_owner: str | None = None,
    project_number: int | None = None,
) -> list[str]:
    """Best-effort cleanup of only mutations introduced by this claim attempt.

    The canonical log is reread before compensation.  If another actor has
    already claimed the Atom, all destructive cleanup stops; an old claim attempt
    must never remove the winner's shared-login projection.
    """
    failures: list[str] = []
    try:
        state = _read_claim_log(repository)
        if state.for_atom(number) is not None:
            _preserve_winner_projection(
                repository=repository,
                number=number,
                board=board,
                project_owner=project_owner,
                project_number=project_number,
            )
            return failures
        issue = _issue_claim(repository, number, state=state)

        def winner_guard() -> bool:
            """Stop stale cleanup and repair a winner's shared-login projection."""
            guarded_state = _read_claim_log(repository)
            if guarded_state.for_atom(number) is None:
                return False
            _preserve_winner_projection(
                repository=repository,
                number=number,
                board=board,
                project_owner=project_owner,
                project_number=project_number,
            )
            return True

        # Guard immediately before and after removal.  A winner can use the
        # same GitHub login and appear while the stale cleanup is in flight;
        # the post-write guard restores that winner's projection.
        if (
            assignee_added
            and issue.state.upper() == "OPEN"
            and login in issue.assignees
            and not winner_guard()
        ):
            removal_error: Exception | None = None
            try:
                run_gh(
                    [
                        "issue",
                        "edit",
                        str(number),
                        "--repo",
                        repository,
                        "--remove-assignee",
                        login,
                    ]
                )
            except Exception as error:
                removal_error = error
            if winner_guard():
                return failures
            if removal_error is not None:
                raise removal_error
        if (
            board is not None
            and issue.state.upper() == "OPEN"
            and previous_status is not None
            and status_attempted
        ):
            compensation_board = board
            if project_owner is not None and project_number is not None:
                compensation_board = fetch_board(project_owner, project_number)
            item = compensation_board.items.get(number)
            if not winner_guard():
                # Restore only when the fresh projection still carries the
                # value this attempt introduced.  Never use the mutation flag
                # to overwrite a concurrent status update; report that drift
                # instead.
                if item is not None and item.status == "In Progress":
                    _set_status(compensation_board, number, previous_status)
                    if winner_guard():
                        return failures
                elif item is not None:
                    failures.append(
                        "projection reconciliation: Project status changed concurrently; "
                        f"preserving {item.status!r}"
                    )
    except Exception as error:
        failures.append(f"projection reconciliation: {error}")
    return failures


def _claim_result(
    *,
    number: int,
    actor: str,
    login: str,
    lease: ClaimLease,
    server_now: datetime,
    already_claimed: bool,
    event_id: str = "",
    raced: bool = False,
) -> dict[str, Any]:
    result: dict[str, Any] = {
        "atom": number,
        "actor": actor,
        "assignee": login,
        "already_claimed": already_claimed,
        "lease": lease.as_json_object(server_now),
    }
    if event_id:
        result["claim_event_id"] = event_id
        result["claim_ref"] = CLAIM_LOG_REF
    if raced:
        result["raced"] = True
    return result


def adopt_legacy_claim(
    *,
    repository: str,
    project_owner: str,
    project_number: int,
    closure_root: Path,
    closure_schema: Path,
    number: int,
    now: datetime | None = None,
) -> dict[str, Any]:
    """Adopt the sole legacy projection, exclusively for Atom 70.

    This is a deliberately explicit bridge for repositories that have the old
    three-line claim comment but no canonical claim-log entry yet.  Every legacy
    predicate is checked before the normal claim CAS is attempted.  The existing
    assignee, Project status, and comment are then reconciled in place; no new
    projection is created.
    """
    if number != LEGACY_ADOPTION_ATOM:
        raise RuntimeError(
            f"legacy claim adoption is only supported for Atom {LEGACY_ADOPTION_ATOM}"
        )
    if project_number != DEFAULT_PROJECT_NUMBER:
        raise RuntimeError("#70: legacy claim adoption requires Project 9")
    auth = preflight(
        repository=repository,
        project_owner=project_owner,
        project_number=project_number,
    )
    actor = _actor()
    login = _validate_login(auth.login, field="authenticated login")
    state = _read_claim_log(repository)
    _server_clock(state)

    canonical = state.for_atom(LEGACY_ADOPTION_ATOM)
    if canonical is not None:
        raise RuntimeError("#70: canonical lease already exists; legacy adoption is one-time")
    if any(lease.actor == actor for lease in state.active_claims):
        raise RuntimeError("#70: actor already owns a canonical lease")
    if len(state.active_claims) >= MAX_ACTIVE_CLAIMS:
        raise RuntimeError(
            f"claim-log active cap {MAX_ACTIVE_CLAIMS} reached; release a claim first"
        )

    issue = _issue_claim(repository, LEGACY_ADOPTION_ATOM, state=state)
    if issue.state.upper() != "OPEN":
        raise RuntimeError("#70: legacy claim requires an OPEN issue")
    if issue.assignees != (login,):
        raise RuntimeError(
            f"#70: legacy claim requires exactly authenticated login {login!r} assigned"
        )

    board = fetch_board(project_owner, project_number)
    item = board.items.get(LEGACY_ADOPTION_ATOM)
    if item is None or item.status != "In Progress":
        raise RuntimeError("#70: legacy claim requires Project 9 Status In Progress")

    legacy_comment, comment_server_now = _read_legacy_claim_comment(
        repository, number=LEGACY_ADOPTION_ATOM, expected_actor=actor
    )
    if legacy_comment.lease.actor != actor:
        raise RuntimeError(
            f"#70: legacy claim comment belongs to {legacy_comment.lease.actor}, not {actor}"
        )
    if not legacy_comment.lease.is_live(comment_server_now):
        raise RuntimeError("#70: legacy claim comment is stale")
    # The claim-log read and comment read each provide server time.  A caller's
    # compatibility ``now`` value is intentionally ignored, but the state read
    # must still have supplied a valid server clock before a CAS is attempted.
    appended = _append_event(
        repository,
        event_type="claim",
        atom=LEGACY_ADOPTION_ATOM,
        actor=actor,
        login=login,
        lease_id=_new_id(),
        reason="adopt legacy claim",
    )
    lease = appended.lease
    if lease is None:
        raise RuntimeError("claim-log adoption append returned no lease")

    # A concurrent same-actor adoption can converge on the existing event.  In
    # either case update the one known comment only; never add a second comment or
    # repeat the assignee/status projections.
    current = _read_claim_log(repository)
    current_lease = current.for_atom(LEGACY_ADOPTION_ATOM)
    if current_lease is None or current_lease.lease_id != lease.lease_id:
        raise RuntimeError("#70: canonical lease changed during legacy adoption")
    current_issue = _issue_claim(repository, LEGACY_ADOPTION_ATOM, state=current)
    if current_issue.state.upper() != "OPEN" or current_issue.assignees != (login,):
        raise RuntimeError("#70: legacy projections changed during adoption")
    _update_claim_comment(
        repository,
        comment_id=legacy_comment.comment_id,
        body=_claim_comment(actor, lease, appended.event_id),
    )
    result = _claim_result(
        number=LEGACY_ADOPTION_ATOM,
        actor=actor,
        login=login,
        lease=current_lease,
        server_now=_server_clock(current),
        already_claimed=appended.already_present,
        event_id=appended.event_id,
        raced=appended.already_present,
    )
    result["adopted_legacy"] = True
    return result


def claim(
    *,
    repository: str,
    project_owner: str,
    project_number: int,
    closure_root: Path,
    closure_schema: Path,
    requested: int | None,
    now: datetime | None = None,
    adopt_legacy: bool = False,
) -> dict[str, Any]:
    """CAS-claim the first Ready Atom, then reconcile its projections."""
    if adopt_legacy:
        if requested != LEGACY_ADOPTION_ATOM:
            raise RuntimeError(
                f"legacy claim adoption requires explicit Atom {LEGACY_ADOPTION_ATOM}"
            )
        return adopt_legacy_claim(
            repository=repository,
            project_owner=project_owner,
            project_number=project_number,
            closure_root=closure_root,
            closure_schema=closure_schema,
            number=LEGACY_ADOPTION_ATOM,
            now=now,
        )
    auth = preflight(
        repository=repository,
        project_owner=project_owner,
        project_number=project_number,
    )
    actor = _actor()
    state = _read_claim_log(repository)
    server_now = _server_clock(state)

    actor_leases = state.for_actor(actor)
    if actor_leases:
        existing = actor_leases[0]
        if not existing.is_live(server_now):
            raise RuntimeError(
                f"#{_lease_atom(existing)}: actor has a stale claim; release it explicitly "
                "before claiming again"
            )
        return _claim_result(
            number=_lease_atom(existing),
            actor=actor,
            login=existing.login,
            lease=existing,
            server_now=server_now,
            already_claimed=True,
            event_id=existing.event_id,
        )

    rows = _ready_rows(repository, closure_root, closure_schema)
    claims = {
        row.number: IssueClaim(
            number=row.number,
            state="OPEN",
            assignees=(),
            lease=state.for_atom(row.number),
            leases=state.active_claims,
        )
        for row in rows
    }
    selected = select_unclaimed(rows, claims)
    if selected is None:
        raise RuntimeError("no unclaimed Ready Atom")
    if requested is not None and requested != selected.number:
        raise RuntimeError(
            f"#{requested} is not the first unclaimed Ready Atom; select #{selected.number}"
        )

    lease_id = _new_id()
    appended = _append_event(
        repository,
        event_type="claim",
        atom=selected.number,
        actor=actor,
        login=auth.login,
        lease_id=lease_id,
        reason="claim",
    )
    lease = appended.lease
    if lease is None:
        raise RuntimeError("claim-log append returned no lease")
    if appended.already_present:
        # A concurrent invocation of this actor converged on the lease that is
        # already authoritative.  It must not duplicate comments or projections.
        return _claim_result(
            number=selected.number,
            actor=actor,
            login=lease.login,
            lease=lease,
            server_now=_server_clock(appended.state),
            already_claimed=True,
            event_id=appended.event_id,
            raced=True,
        )

    board = None
    previous_status: str | None = None
    projections_started = False
    assignee_added = False
    status_attempted = False
    status_changed = False
    try:
        conservative = _issue_claim(repository, selected.number, state=appended.state)
        if conservative.state.upper() != "OPEN":
            _projection_abort(
                repository,
                number=selected.number,
                actor=actor,
                lease=lease,
                reason="issue closed after claim CAS",
            )
            raise RuntimeError(f"#{selected.number}: issue closed after claim CAS; claim aborted")

        board = fetch_board(project_owner, project_number)
        board_item = board.items.get(selected.number)
        if board_item is None:
            raise RuntimeError(f"#{selected.number}: absent from Project {project_number}")
        previous_status = board_item.status or selected.status or READY
        # Mark the boundary before the first external projection.  A lost response
        # can mean the assignee write succeeded, so recovery must reconcile it too.
        projections_started = True
        assignee_added = auth.login not in conservative.assignees
        run_gh(
            [
                "issue",
                "edit",
                str(selected.number),
                "--repo",
                repository,
                "--add-assignee",
                auth.login,
            ]
        )
        status_attempted = previous_status != "In Progress"
        _set_status(board, selected.number, "In Progress")
        if status_attempted:
            status_changed = True
            # Real Project writes do not mutate this read model.  Updating the
            # local copy lets a deterministic compensation test represent the
            # value this attempt actually wrote without another network read.
            board.items[selected.number] = replace(board_item, status="In Progress")
        run_gh(
            [
                "issue",
                "comment",
                str(selected.number),
                "--repo",
                repository,
                "--body",
                _claim_comment(actor, lease, appended.event_id),
            ]
        )

        final_state = _read_claim_log(repository)
        final_issue = _issue_claim(repository, selected.number, state=final_state)
        final_lease = final_state.for_atom(selected.number)
        if final_lease is None or final_lease.lease_id != lease.lease_id:
            raise RuntimeError(f"#{selected.number}: claim changed during projection")
        if final_issue.state.upper() != "OPEN":
            raise RuntimeError(f"#{selected.number}: issue closed during projection")
        return _claim_result(
            number=selected.number,
            actor=actor,
            login=auth.login,
            lease=final_lease,
            server_now=_server_clock(final_state),
            already_claimed=appended.already_present,
            event_id=appended.event_id,
            raced=appended.already_present,
        )
    except Exception as error:
        compensation_failures: list[str] = []
        try:
            _projection_abort(
                repository,
                number=selected.number,
                actor=actor,
                lease=lease,
                reason=str(error),
            )
        except Exception as abort_error:
            compensation_failures.append(f"claim-log abort: {abort_error}")
        if projections_started:
            compensation_failures.extend(
                _reconcile_claim_projection(
                    repository=repository,
                    number=selected.number,
                    login=auth.login,
                    board=board,
                    previous_status=previous_status,
                    actor=actor,
                    assignee_added=assignee_added,
                    status_attempted=status_attempted,
                    status_changed=status_changed,
                    project_owner=project_owner,
                    project_number=project_number,
                )
            )
        if compensation_failures:
            raise RuntimeError(
                f"claim failed ({error}); compensation also failed: "
                + "; ".join(compensation_failures)
            ) from error
        raise


def list_claims(repository: str, *, now: datetime | None = None) -> tuple[dict[str, Any], ...]:
    """Report canonical leases and projection drift, using server Date for liveness."""
    state = _read_claim_log(repository)
    server_now = _server_clock(state)
    output: list[dict[str, Any]] = []
    for lease in state.active_claims:
        snapshot = _issue_claim(repository, _lease_atom(lease), state=state)
        output.append(
            {
                "atom": _lease_atom(lease),
                "assignees": list(snapshot.assignees),
                "lease": lease.as_json_object(server_now),
                "projection_drift": list(snapshot.projection_drift),
            }
        )
    try:
        assigned = run_gh_json(
            [
                "issue",
                "list",
                "--repo",
                repository,
                "--state",
                "open",
                "--assignee",
                "*",
                "--limit",
                "1000",
                "--json",
                "number",
            ]
        )
        assigned_numbers = {
            int(row["number"])
            for row in assigned
            if isinstance(row, dict) and isinstance(row.get("number"), int)
        }
        known = {_lease_atom(lease) for lease in state.active_claims}
        for number in sorted(assigned_numbers - known):
            snapshot = _issue_claim(repository, number, state=state)
            output.append(
                {
                    "atom": number,
                    "assignees": list(snapshot.assignees),
                    "lease": None,
                    "projection_drift": ["assignee exists without claim-log lease"],
                }
            )
    except (OSError, RuntimeError, TypeError, ValueError):
        # Canonical state remains reportable even if an optional drift scan fails.
        pass
    return tuple(sorted(output, key=lambda value: int(value["atom"])))


def _release_comment(
    actor: str, *, event_id: str, reason: str, lease_id: str = ""
) -> str:
    lease_line = f"lease_id: {lease_id}\n" if lease_id else ""
    return (
        f"{actor} released this claim\n"
        f"release_event_id: {event_id}\n"
        f"claim_ref: {CLAIM_LOG_REF}\n"
        f"{lease_line}"
        f"reason: {_safe_reason(reason)}"
    )


def _preserve_winner_projection(
    *,
    repository: str,
    number: int,
    board: Any | None,
    project_owner: str | None = None,
    project_number: int | None = None,
) -> ClaimLease | None:
    """Repair a winner's projection without trusting a stale release snapshot.

    A release and a subsequent claim can share one GitHub login.  Each external
    write is bracketed by canonical-log reads; if the winner disappears before a
    repair, no stale release operation resumes destructive work.
    """
    state = _read_claim_log(repository)
    winner = state.for_atom(number)
    if winner is None:
        return None

    issue = _issue_claim(repository, number, state=state)
    if issue.state.upper() == "OPEN" and winner.login not in issue.assignees:
        verified = False
        for _attempt in range(2):
            state = _read_claim_log(repository)
            winner = state.for_atom(number)
            if winner is None:
                return None
            issue = _issue_claim(repository, number, state=state)
            if issue.state.upper() != "OPEN" or winner.login in issue.assignees:
                verified = True
                break
            run_gh(
                [
                    "issue",
                    "edit",
                    str(number),
                    "--repo",
                    repository,
                    "--add-assignee",
                    winner.login,
                ]
            )
        if not verified:
            raise RuntimeError(f"#{number}: winner assignee projection could not be verified")

    if board is not None:
        state = _read_claim_log(repository)
        winner = state.for_atom(number)
        if winner is None:
            return None
        item = board.items.get(number)
        if item is not None and item.status != "In Progress":
            _set_status(board, number, "In Progress")
            board.items[number] = replace(item, status="In Progress")
            state = _read_claim_log(repository)
            winner = state.for_atom(number)
            if winner is None:
                return None
        if project_owner is not None and project_number is not None:
            verified_board = fetch_board(project_owner, project_number)
            verified_item = verified_board.items.get(number)
            if verified_item is None or verified_item.status != "In Progress":
                raise RuntimeError(
                    f"#{number}: winner Project status projection could not be verified"
                )
    return winner


def release(
    *,
    repository: str,
    project_owner: str,
    project_number: int,
    closure_root: Path,
    closure_schema: Path,
    number: int,
    reason: str,
    now: datetime | None = None,
    coordinator: bool = False,
) -> dict[str, Any]:
    """Append a CAS release, or an authorized stale-lease reclaim."""
    auth = preflight(
        repository=repository,
        project_owner=project_owner,
        project_number=project_number,
    )
    actor = _actor()
    state = _read_claim_log(repository)
    _server_clock(state)
    snapshot = _issue_claim(repository, number, state=state)
    lease = state.for_atom(number)
    if lease is None:
        if not snapshot.assignees:
            return {"atom": number, "released": False, "already_unclaimed": True}
        raise RuntimeError(f"#{number}: assignee exists without a canonical claim-log lease")
    reclaiming = lease.actor != actor
    if reclaiming:
        if not coordinator or not _coordinator_recovery_authorized(auth):
            raise RuntimeError(
                f"#{number}: claim belongs to {lease.actor}, not current actor {actor}"
            )
        if lease.is_live(_server_clock(state)):
            raise RuntimeError(f"#{number}: live foreign lease cannot be reclaimed")
    elif lease.login != auth.login:
        raise RuntimeError(
            f"#{number}: claim authenticated login is {auth.login!r}, "
            f"not {lease.login!r}"
        )

    safe_reason = _safe_reason(reason)
    appended = _append_event(
        repository,
        event_type="reclaim" if reclaiming else "release",
        atom=number,
        actor=actor,
        login=auth.login if reclaiming else lease.login,
        lease_id=lease.lease_id,
        reason=safe_reason,
        target_actor=lease.actor,
        target_login=lease.login if reclaiming else None,
        coordinator_authorized=reclaiming,
    )

    # Closing an issue must not strand a canonical lease.  The release event is
    # authoritative; assignee, Project, and comment projections are not safe or
    # necessary to mutate on a closed issue.
    if snapshot.state.upper() != "OPEN":
        return {
            "atom": number,
            "released": not reclaiming,
            "reclaimed": reclaiming,
            "assignee": lease.login,
            "status": snapshot.state,
            "reason": safe_reason,
            "projection_drift": ["issue closed; release projections skipped"],
            "release_event_id": appended.event_id,
            "claim_ref": CLAIM_LOG_REF,
        }

    desired = BLOCKED
    projection_drift: list[str] = []
    try:
        rows = _ready_rows(repository, closure_root, closure_schema)
        row = next((candidate for candidate in rows if candidate.number == number), None)
        desired = row.status if row is not None else BLOCKED
        if desired not in (READY, BLOCKED):
            desired = BLOCKED
        board = fetch_board(project_owner, project_number)
        board_item = board.items.get(number)
        if board_item is None:
            raise RuntimeError(f"#{number}: absent from Project {project_number}")
        def winner_guard() -> bool:
            guarded_state = _read_claim_log(repository)
            if guarded_state.for_atom(number) is None:
                return False
            projection_drift.append("claim was reacquired during release projection")
            _preserve_winner_projection(
                repository=repository,
                number=number,
                board=board,
                project_owner=project_owner,
                project_number=project_number,
            )
            return True

        if not winner_guard():
            current = _issue_claim(repository, number, state=_read_claim_log(repository))
            if current.state.upper() == "OPEN" and lease.login in current.assignees:
                # If another assignment shares the issue, the claim log cannot
                # prove that this login was introduced by the lease.  Preserve
                # the pre-existing projection rather than deleting human state.
                if tuple(current.assignees) != (lease.login,):
                    projection_drift.append(
                        "pre-existing/shared assignee retained during release"
                    )
                    current = None
                else:
                    # The guard immediately before this mutation is intentional:
                    # a winner can arrive after the previous issue read.
                    if not winner_guard():
                        run_gh(
                            [
                                "issue",
                                "edit",
                                str(number),
                                "--repo",
                                repository,
                                "--remove-assignee",
                                lease.login,
                            ]
                        )
                    if winner_guard():
                        current = None
            if current is not None and not winner_guard():
                # Re-read immediately before and after the status mutation.  A
                # winner gets In Progress, never the release's derived value.
                _set_status(board, number, desired)
                board.items[number] = replace(board_item, status=desired)
                if not winner_guard():
                    run_gh(
                        [
                            "issue",
                            "comment",
                            str(number),
                            "--repo",
                            repository,
                            "--body",
                            _release_comment(
                                actor,
                                event_id=appended.event_id,
                                lease_id=lease.lease_id,
                                reason=safe_reason,
                            ),
                        ]
                    )
                    # Final canonical verification catches a winner that arrived
                    # while the diagnostic comment was being written.
                    winner_guard()
    except Exception as error:
        projection_drift.append(str(error))
        return {
            "atom": number,
            "released": not reclaiming,
            "reclaimed": reclaiming,
            "assignee": lease.login,
            "status": desired,
            "reason": safe_reason,
            "projection_drift": projection_drift,
            "release_event_id": appended.event_id,
            "claim_ref": CLAIM_LOG_REF,
        }
    return {
        "atom": number,
        "released": not reclaiming,
        "reclaimed": reclaiming,
        "assignee": lease.login,
        "status": desired,
        "reason": safe_reason,
        "release_event_id": appended.event_id,
        "claim_ref": CLAIM_LOG_REF,
        **({"projection_drift": projection_drift} if projection_drift else {}),
    }


def reclaim(
    *,
    repository: str,
    project_owner: str,
    project_number: int,
    closure_root: Path,
    closure_schema: Path,
    number: int,
    reason: str,
    now: datetime | None = None,
    authorized_coordinator: bool = False,
) -> dict[str, Any]:
    """Explicitly reclaim one expired foreign lease.

    The caller must opt into the transition and carry the coordinator capability;
    the implementation still re-reads the claim log and uses only GitHub's Date
    header to decide whether the lease is expired.
    """
    if not authorized_coordinator:
        raise RuntimeError("claim-log reclaim requires an authorized coordinator")
    return release(
        repository=repository,
        project_owner=project_owner,
        project_number=project_number,
        closure_root=closure_root,
        closure_schema=closure_schema,
        number=number,
        reason=reason,
        now=now,
        coordinator=True,
    )


def _common(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--repository", default=DEFAULT_REPOSITORY)
    parser.add_argument("--project-owner", default=DEFAULT_OWNER)
    parser.add_argument("--project", type=int, default=DEFAULT_PROJECT_NUMBER)
    parser.add_argument("--closure-root", type=Path, default=DEFAULT_CLOSURE_ROOT)
    parser.add_argument("--closure-schema", type=Path, default=DEFAULT_CLOSURE_SCHEMA)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="gordian-bootstrap",
        description="Non-interactive GitHub claim-log CAS and projection reconciliation.",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)
    preflight_parser = subparsers.add_parser("preflight")
    _common(preflight_parser)
    claim_parser = subparsers.add_parser("claim")
    _common(claim_parser)
    claim_parser.add_argument("atom", type=int, nargs="?")
    claim_parser.add_argument(
        "--adopt-legacy",
        action="store_true",
        help="one-time adoption of Atom 70's validated legacy claim projection",
    )
    claims_parser = subparsers.add_parser("claims")
    _common(claims_parser)
    release_parser = subparsers.add_parser("release")
    _common(release_parser)
    release_parser.add_argument("atom", type=int)
    release_parser.add_argument("--reason", required=True)
    reclaim_parser = subparsers.add_parser("reclaim")
    _common(reclaim_parser)
    reclaim_parser.add_argument("atom", type=int)
    reclaim_parser.add_argument("--reason", required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    arguments = _parser().parse_args(argv)
    try:
        if arguments.command == "preflight":
            result: Any = {
                "login": preflight(
                    repository=arguments.repository,
                    project_owner=arguments.project_owner,
                    project_number=arguments.project,
                ).login
            }
        elif arguments.command == "claims":
            preflight(
                repository=arguments.repository,
                project_owner=arguments.project_owner,
                project_number=arguments.project,
            )
            result = list_claims(arguments.repository)
        elif arguments.command == "claim":
            result = claim(
                repository=arguments.repository,
                project_owner=arguments.project_owner,
                project_number=arguments.project,
                closure_root=arguments.closure_root,
                closure_schema=arguments.closure_schema,
                requested=arguments.atom,
                adopt_legacy=arguments.adopt_legacy,
            )
        elif arguments.command == "release":
            result = release(
                repository=arguments.repository,
                project_owner=arguments.project_owner,
                project_number=arguments.project,
                closure_root=arguments.closure_root,
                closure_schema=arguments.closure_schema,
                number=arguments.atom,
                reason=arguments.reason,
            )
        else:
            result = reclaim(
                repository=arguments.repository,
                project_owner=arguments.project_owner,
                project_number=arguments.project,
                closure_root=arguments.closure_root,
                closure_schema=arguments.closure_schema,
                number=arguments.atom,
                reason=arguments.reason,
                authorized_coordinator=True,
            )
    except GitHubConfigurationError as error:
        print(str(error), file=sys.stderr)
        return EX_CONFIG
    except (OSError, RuntimeError, TypeError, ValueError, GitHubApiError) as error:
        print(str(error), file=sys.stderr)
        return 1
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
