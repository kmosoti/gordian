"""Environment and source identity stamped onto every orchestration report.

Methodology requires that any artifact presented as evidence carry its exact source and
environment identity. A reconciliation or derivation report without them is unreproducible
and, worse, indistinguishable from a stale one.

`source_change_id` and `source_commit_id` are the adapter-neutral logical change identity
and exact state identity of the working copy the report was produced from. They keep those
key names because the bootstrap consumers read them by name; the concepts are
`logical_change_id` and `exact_state_id` as defined in `docs/spec/data-model.md`.

Every probe degrades to the literal string `unknown` rather than failing or omitting a key,
so a report is always self-describing about what could not be identified.
"""

from __future__ import annotations

import subprocess
from dataclasses import asdict, dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

UNKNOWN = "unknown"

_CHANGE_ID_TEMPLATE = 'change_id.short(12) ++ " " ++ commit_id.short(12)'


@dataclass(frozen=True, slots=True)
class Provenance:
    generated_at: str
    source_change_id: str
    source_commit_id: str
    tool_versions: dict[str, str]

    def as_json_object(self) -> dict[str, Any]:
        return asdict(self)


def _capture(argv: list[str], *, cwd: Path | None = None) -> str:
    """Return the first stdout line of a local probe, or `unknown`."""
    try:
        completed = subprocess.run(
            argv,
            cwd=cwd,
            check=False,
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (OSError, subprocess.SubprocessError):
        return UNKNOWN
    if completed.returncode != 0:
        return UNKNOWN
    lines = [line.strip() for line in completed.stdout.splitlines() if line.strip()]
    if not lines:
        return UNKNOWN
    return lines[0]


def generated_at() -> str:
    """Return the report time as an RFC 3339 UTC timestamp."""
    return datetime.now(UTC).isoformat(timespec="seconds").replace("+00:00", "Z")


def source_identity(cwd: Path | None = None) -> tuple[str, str]:
    """Return `(source_change_id, source_commit_id)` for the working copy."""
    line = _capture(
        ["jj", "log", "-r", "@", "--no-graph", "--ignore-working-copy", "-T", _CHANGE_ID_TEMPLATE],
        cwd=cwd,
    )
    parts = line.split()
    if len(parts) == 2:
        return parts[0], parts[1]
    commit = _capture(["git", "rev-parse", "HEAD"], cwd=cwd)
    return UNKNOWN, commit


def tool_versions(cwd: Path | None = None) -> dict[str, str]:
    """Return the identity of every external tool an orchestration report depends on."""
    return {
        "gh": _capture(["gh", "--version"]),
        "jj": _capture(["jj", "--version"], cwd=cwd),
    }


def collect(cwd: Path | None = None) -> Provenance:
    """Collect the full provenance stamp in one call."""
    change_id, commit_id = source_identity(cwd)
    return Provenance(
        generated_at=generated_at(),
        source_change_id=change_id or UNKNOWN,
        source_commit_id=commit_id or UNKNOWN,
        tool_versions=tool_versions(cwd),
    )
