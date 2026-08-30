"""Single entry point for GitHub CLI invocations.

Orchestration only. Nothing here interprets issue state, Project state, or dependency
edges as Mission Graph evidence; callers own that reading and Rust owns the semantics.

Authentication is non-interactive by design. An unattended agent supplies a classic
personal access token carrying the `repo` and `project` scopes through the `GH_TOKEN`
environment variable; the token is never committed and never written to a report.
Fine-grained tokens do not carry the classic `project` scope that `gh project item-add`
and the Project v2 field mutations require.
"""

from __future__ import annotations

import json
import subprocess
from collections.abc import Sequence
from typing import Any

GH_AUTH_HINT = (
    "Verify `gh` authentication: export GH_TOKEN with a classic personal access token "
    "carrying the `repo` and `project` scopes (never commit it), or, in an interactive "
    "session only, run `gh auth refresh -s project`."
)


def run_gh(arguments: Sequence[str]) -> str:
    """Run the GitHub CLI without a shell and return stdout.

    The raised error preserves stderr while avoiding command-string interpolation, and
    never echoes the environment, so a token in `GH_TOKEN` cannot leak into a report.
    """
    completed = subprocess.run(
        ["gh", *arguments],
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        stderr = completed.stderr.strip() or "GitHub CLI command failed"
        command = "gh " + " ".join(arguments[:3])
        raise RuntimeError(f"{command}: {stderr}")
    return completed.stdout


def run_gh_json(arguments: Sequence[str]) -> Any:
    """Run the GitHub CLI and decode its stdout as JSON."""
    output = run_gh(arguments)
    try:
        return json.loads(output)
    except json.JSONDecodeError as error:
        raise RuntimeError(f"GitHub CLI returned invalid JSON: {error}") from error


def graphql(query: str, variables: dict[str, str | int] | None = None) -> Any:
    """Execute one GraphQL document and return its `data` object."""
    arguments: list[str] = ["api", "graphql", "-f", f"query={query}"]
    for name, value in (variables or {}).items():
        flag = "-F" if isinstance(value, int) else "-f"
        arguments.extend([flag, f"{name}={value}"])
    payload = run_gh_json(arguments)
    if not isinstance(payload, dict):
        raise RuntimeError("unexpected `gh api graphql` response shape")
    errors = payload.get("errors")
    if errors:
        raise RuntimeError(f"GitHub GraphQL error: {json.dumps(errors)}")
    data = payload.get("data")
    if not isinstance(data, dict):
        raise RuntimeError("`gh api graphql` returned no data object")
    return data
