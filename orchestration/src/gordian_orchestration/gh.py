"""Single entry point for GitHub CLI invocations.

Orchestration only. Nothing here interprets issue state, Project state, or dependency
edges as Mission Graph evidence; callers own that reading and Rust owns the semantics.

Authentication is non-interactive by design. Preflight requires the process-local
``GORDIAN_GH_TOKEN`` credential and copies it to ``GH_TOKEN`` for every child ``gh`` process.
It never falls back to a token in the environment or to the ``gh`` credential store. No token
is committed, reported, or printed, and no interactive authentication command is ever invoked.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
from collections.abc import Collection, Sequence
from dataclasses import dataclass
from typing import Any

GH_AUTH_HINT = (
    "Unattended GitHub authentication requires a non-empty GORDIAN_GH_TOKEN; ambient GH_TOKEN "
    "and the gh credential store are not used. It is copied to GH_TOKEN for every gh "
    "subprocess. Preflight requires an authenticated identity, repository write access, and "
    "Project 9 read/write API access. Repair credentials interactively outside an unattended "
    "run; the loop never invokes `gh auth login` or `gh auth refresh`."
)

EX_CONFIG = 78
BOOTSTRAP_CAPABILITIES = (
    "authenticated_identity",
    "repository_write",
    "project_read",
    "project_write",
)


class GitHubConfigurationError(RuntimeError):
    """Non-interactive GitHub configuration is absent or insufficient."""


class GitHubApiError(RuntimeError):
    """A GitHub API response was not in the caller's allowed status set."""

    def __init__(self, status: int, message: str, response: GitHubApiResponse | None = None):
        super().__init__(message)
        self.status = status
        self.response = response


@dataclass(frozen=True, slots=True)
class PreflightReport:
    login: str
    capabilities: tuple[str, ...]
    credential_source: str
    repository: str
    project_owner: str
    project_number: int


@dataclass(frozen=True, slots=True)
class GitHubApiResponse:
    """A structured ``gh api --include`` response.

    Header names are normalized to lowercase because HTTP field names are
    case-insensitive.  The body is deliberately retained as text; callers that
    need JSON must decode it and validate its shape themselves.
    """

    status: int
    headers: dict[str, str]
    body: str

    def json(self) -> Any:
        """Decode the response body as JSON without accepting a fallback shape."""
        return json.loads(self.body)


def _redact_token(message: str, token: str) -> str:
    """Keep authentication errors useful without echoing the configured token."""
    return message.replace(token, "<redacted>") if token else message


def _require_token() -> str:
    """Return the sole supported credential, rejecting all ambient fallbacks."""
    token = os.environ.get("GORDIAN_GH_TOKEN", "")
    if not token.strip():
        raise GitHubConfigurationError(
            "GitHub authentication is unavailable: GORDIAN_GH_TOKEN must be set to a "
            "non-empty token; ambient GH_TOKEN and the gh credential store are not used"
        )
    return token


def _child_environment(token: str) -> dict[str, str]:
    """Build the environment for a gh child with the explicit token override."""
    child_environment = os.environ.copy()
    child_environment["GH_TOKEN"] = token
    return child_environment


def _run_process(arguments: Sequence[str]) -> subprocess.CompletedProcess[str]:
    """Run ``gh`` with the explicit credential and no shell."""
    token = _require_token()
    # Keep the parent process and every later child on the same explicit credential. This
    # overwrites an ambient GH_TOKEN, while the child environment also makes the contract
    # visible at each subprocess boundary rather than relying on inherited process state.
    os.environ["GH_TOKEN"] = token
    try:
        return subprocess.run(
            ["gh", *arguments],
            check=False,
            capture_output=True,
            text=True,
            env=_child_environment(token),
        )
    except OSError as error:
        command = "gh " + " ".join(arguments[:3])
        detail = _redact_token(str(error), token)
        raise RuntimeError(f"{command}: {detail}") from error


_HTTP_STATUS_LINE = re.compile(r"(?m)^HTTP/\d(?:\.\d+)?\s+(\d{3})[^\r\n]*\r?\n")


def _parse_included_response(output: str) -> GitHubApiResponse:
    """Parse the final HTTP response emitted by ``gh api --include``.

    GitHub may emit an informational response or a redirect before the final
    response.  Selecting the final status block makes the parser safe for both
    cases and prevents a redirect's headers from being used as the server clock.
    """
    matches = list(_HTTP_STATUS_LINE.finditer(output))
    if not matches:
        raise RuntimeError("gh api did not return an HTTP status line")
    status_line = matches[-1]
    header_start = status_line.end()
    separator = output.find("\r\n\r\n", header_start)
    separator_width = 4
    if separator < 0:
        separator = output.find("\n\n", header_start)
        separator_width = 2
    if separator < 0:
        raise RuntimeError("gh api response has no header/body separator")

    headers: dict[str, str] = {}
    for line in output[header_start:separator].replace("\r\n", "\n").splitlines():
        if not line:
            continue
        name, colon, value = line.partition(":")
        if not colon or not name.strip():
            raise RuntimeError("gh api response contains a malformed header")
        headers[name.strip().lower()] = value.strip()
    status = int(status_line.group(1))
    body = output[separator + separator_width :]
    return GitHubApiResponse(status=status, headers=headers, body=body)


def run_gh_response(
    arguments: Sequence[str], *, allowed_statuses: Collection[int] = ()
) -> GitHubApiResponse:
    """Run a GitHub API command and return status, headers, and body.

    ``allowed_statuses`` is explicit so callers implementing a compare-and-swap
    can handle 409/422 as ordinary outcomes while every unexpected status remains
    an error.  The function only accepts ``gh api`` commands; regular CLI commands
    continue through :func:`run_gh` and retain their stdout contract.
    """
    if not arguments or arguments[0] != "api":
        raise ValueError("run_gh_response requires a `gh api` command")
    api_arguments = list(arguments)
    if "--include" not in api_arguments:
        api_arguments.append("--include")
    completed = _run_process(api_arguments)
    token = os.environ.get("GORDIAN_GH_TOKEN", "")
    output = completed.stdout
    try:
        response = _parse_included_response(output)
    except RuntimeError as error:
        stderr = _redact_token(completed.stderr.strip(), token)
        detail = f": {stderr}" if stderr else ""
        raise RuntimeError(f"GitHub API response could not be parsed{detail}") from error

    allowed = {int(status) for status in allowed_statuses}
    # Callers that care about a response contract provide an explicit set.  Do
    # not silently turn a documented 200 read or 201 create into an accepted 202,
    # 204, or other successful-but-unexpected response; such a response may omit
    # the body or headers the caller is about to interpret.  With no set, retain
    # the generic all-2xx wrapper behavior for callers that only need transport.
    if (allowed and response.status not in allowed) or (
        not allowed and not 200 <= response.status < 300
    ):
        detail = _redact_token(response.body.strip(), token)
        message = f"GitHub API returned HTTP {response.status}"
        if detail:
            message += f": {detail}"
        raise GitHubApiError(response.status, message, response)
    return response


def run_gh_json_response(
    arguments: Sequence[str], *, allowed_statuses: Collection[int] = ()
) -> tuple[Any, GitHubApiResponse]:
    """Run a JSON GitHub API request without discarding its response metadata.

    Callers that make a decision from a GitHub representation sometimes need both the
    decoded payload and transport metadata such as the server ``Date`` header.  Keeping
    this as one boundary prevents those callers from combining a body-only read with a
    local clock (or a second, potentially different request).
    """
    response = run_gh_response(arguments, allowed_statuses=allowed_statuses)
    try:
        payload = json.loads(response.body)
    except json.JSONDecodeError as error:
        token = os.environ.get("GORDIAN_GH_TOKEN", "")
        detail = _redact_token(str(error), token)
        raise RuntimeError(f"GitHub API returned invalid JSON: {detail}") from error
    return payload, response


# The shorter alias is useful to callers whose code already names the operation
# after the API rather than after the CLI wrapper.  Both names intentionally share
# one implementation and one status/header contract.
run_gh_api = run_gh_response


def run_gh(arguments: Sequence[str]) -> str:
    """Run the GitHub CLI without a shell and return stdout.

    The raised error preserves stderr while avoiding command-string interpolation, and
    never echoes the environment, so a token in `GORDIAN_GH_TOKEN` cannot leak into a report.
    """
    token = _require_token()
    completed = _run_process(arguments)
    if completed.returncode != 0:
        stderr = _redact_token(completed.stderr.strip(), token) or "GitHub CLI command failed"
        command = _redact_token("gh " + " ".join(arguments[:3]), token)
        raise RuntimeError(f"{command}: {stderr}")
    return completed.stdout


def run_gh_json(arguments: Sequence[str]) -> Any:
    """Run the GitHub CLI and decode its stdout as JSON."""
    output = run_gh(arguments)
    try:
        return json.loads(output)
    except json.JSONDecodeError as error:
        token = os.environ.get("GORDIAN_GH_TOKEN", "")
        detail = _redact_token(str(error), token)
        raise RuntimeError(f"GitHub CLI returned invalid JSON: {detail}") from error


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
        token = os.environ.get("GORDIAN_GH_TOKEN", "")
        detail = _redact_token(json.dumps(errors), token)
        raise RuntimeError(f"GitHub GraphQL error: {detail}")
    data = payload.get("data")
    if not isinstance(data, dict):
        raise RuntimeError("`gh api graphql` returned no data object")
    return data


_PROJECT_CAPABILITY_QUERY = """
query($owner:String!,$number:Int!){
  user(login:$owner){
    projectV2(number:$number){
      id
      viewerCanUpdate
    }
  }
}
"""


def preflight(
    *,
    repository: str = "kmosoti/gordian",
    project_owner: str = "kmosoti",
    project_number: int = 9,
) -> PreflightReport:
    """Fail closed unless the configured GitHub credential has bootstrap capabilities.

    ``GORDIAN_GH_TOKEN`` is the only accepted credential. It is installed into this process
    environment and explicitly passed to every ``gh`` subprocess. No interactive login or
    scope-widening command is ever invoked.
    """
    token = _require_token()
    os.environ["GH_TOKEN"] = token

    try:
        run_gh(["auth", "status"])
    except (OSError, RuntimeError) as error:
        raise GitHubConfigurationError(
            "GitHub authentication is unavailable: "
            + _redact_token(str(error), token)
        ) from error

    try:
        user = run_gh_json(["api", "user"])
    except (OSError, RuntimeError) as error:
        raise GitHubConfigurationError(
            "GitHub credential cannot identify the authenticated user: "
            + _redact_token(str(error), token)
        ) from error
    if not isinstance(user, dict) or not isinstance(user.get("login"), str) or not user["login"]:
        raise GitHubConfigurationError(
            "GitHub credential cannot identify the authenticated user: API response "
            "did not contain a login"
        )

    try:
        repository_payload = run_gh_json(["api", f"repos/{repository}"])
    except (OSError, RuntimeError) as error:
        raise GitHubConfigurationError(
            f"GitHub credential cannot read repository {repository}: "
            + _redact_token(str(error), token)
        ) from error
    permissions = (
        repository_payload.get("permissions")
        if isinstance(repository_payload, dict)
        else None
    )
    if not isinstance(permissions, dict) or permissions.get("push") is not True:
        raise GitHubConfigurationError(
            f"GitHub credential lacks repository write permission for {repository}"
        )

    try:
        project_data = graphql(
            _PROJECT_CAPABILITY_QUERY,
            {"owner": project_owner, "number": project_number},
        )
    except (OSError, RuntimeError) as error:
        raise GitHubConfigurationError(
            f"GitHub credential cannot access Project {project_owner}/{project_number} "
            "through the GraphQL API: "
            + _redact_token(str(error), token)
        ) from error

    owner_data = project_data.get("user") if isinstance(project_data, dict) else None
    project = owner_data.get("projectV2") if isinstance(owner_data, dict) else None
    if not isinstance(project, dict) or not isinstance(project.get("id"), str):
        raise GitHubConfigurationError(
            f"GitHub credential cannot read Project {project_owner}/{project_number}"
        )
    if project.get("viewerCanUpdate") is not True:
        raise GitHubConfigurationError(
            f"GitHub credential can read Project {project_owner}/{project_number} "
            "but lacks Project write permission"
        )

    return PreflightReport(
        login=user["login"],
        capabilities=BOOTSTRAP_CAPABILITIES,
        credential_source="GORDIAN_GH_TOKEN",
        repository=repository,
        project_owner=project_owner,
        project_number=project_number,
    )
