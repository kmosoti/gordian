"""Reconcile Gordian issues into the temporary GitHub Project projection.

This module is deliberately orchestration-only. It invokes the authenticated GitHub
CLI and does not own Mission Graph, Atom, dependency, readiness, or satisfaction
semantics.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from collections import Counter
from dataclasses import asdict, dataclass
from typing import Any


@dataclass(frozen=True, slots=True)
class Config:
    owner: str
    repository: str
    project_number: int
    dry_run: bool


@dataclass(frozen=True, slots=True)
class IssueRef:
    number: int
    title: str
    url: str


@dataclass(frozen=True, slots=True)
class ProjectItemRef:
    item_id: str | None
    url: str


@dataclass(frozen=True, slots=True)
class ReconciliationReport:
    owner: str
    repository: str
    project_number: int
    open_issue_count: int
    current_project_issue_count: int
    missing_urls: tuple[str, ...]
    duplicate_urls: tuple[str, ...]
    added_urls: tuple[str, ...]
    failed_urls: tuple[str, ...]
    dry_run: bool


def _run_gh(arguments: list[str]) -> str:
    """Run GitHub CLI without a shell and return stdout.

    Authentication and the `project` token scope remain the user's local concern.
    The raised error preserves stderr while avoiding command-string interpolation.
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


def _json(arguments: list[str]) -> Any:
    output = _run_gh(arguments)
    try:
        return json.loads(output)
    except json.JSONDecodeError as error:
        raise RuntimeError(f"GitHub CLI returned invalid JSON: {error}") from error


def _open_issues(config: Config) -> list[IssueRef]:
    payload = _json(
        [
            "issue",
            "list",
            "--repo",
            config.repository,
            "--state",
            "open",
            "--limit",
            "1000",
            "--json",
            "number,title,url",
        ]
    )
    if not isinstance(payload, list):
        raise RuntimeError("unexpected `gh issue list` JSON shape")

    issues: list[IssueRef] = []
    for row in payload:
        if not isinstance(row, dict):
            raise RuntimeError("unexpected issue record")
        issues.append(
            IssueRef(
                number=int(row["number"]),
                title=str(row["title"]),
                url=str(row["url"]),
            )
        )
    return sorted(issues, key=lambda issue: issue.number)


def _project_items(config: Config) -> list[ProjectItemRef]:
    payload = _json(
        [
            "project",
            "item-list",
            str(config.project_number),
            "--owner",
            config.owner,
            "--limit",
            "1000",
            "--format",
            "json",
        ]
    )
    if not isinstance(payload, dict):
        raise RuntimeError("unexpected `gh project item-list` JSON shape")
    rows = payload.get("items", [])
    if not isinstance(rows, list):
        raise RuntimeError("project JSON has no item list")

    items: list[ProjectItemRef] = []
    for row in rows:
        if not isinstance(row, dict):
            continue
        content = row.get("content")
        if not isinstance(content, dict):
            continue
        url = content.get("url")
        if not isinstance(url, str) or "/issues/" not in url:
            continue
        item_id = row.get("id")
        items.append(
            ProjectItemRef(
                item_id=item_id if isinstance(item_id, str) else None,
                url=url,
            )
        )
    return items


def reconcile(config: Config) -> ReconciliationReport:
    # This is also a token-scope and project-existence check. The GitHub CLI's
    # documented remediation is `gh auth refresh -s project`.
    _run_gh(
        [
            "project",
            "view",
            str(config.project_number),
            "--owner",
            config.owner,
            "--format",
            "json",
        ]
    )

    issues = _open_issues(config)
    items = _project_items(config)
    project_counts = Counter(item.url for item in items)
    issue_urls = {issue.url for issue in issues}
    missing = sorted(issue_urls - set(project_counts))
    duplicates = sorted(url for url, count in project_counts.items() if count > 1)

    added: list[str] = []
    failed: list[str] = []
    if not config.dry_run:
        for url in missing:
            try:
                _run_gh(
                    [
                        "project",
                        "item-add",
                        str(config.project_number),
                        "--owner",
                        config.owner,
                        "--url",
                        url,
                        "--format",
                        "json",
                    ]
                )
            except RuntimeError as error:
                print(str(error), file=sys.stderr)
                failed.append(url)
            else:
                added.append(url)

    return ReconciliationReport(
        owner=config.owner,
        repository=config.repository,
        project_number=config.project_number,
        open_issue_count=len(issues),
        current_project_issue_count=len(items),
        missing_urls=tuple(missing),
        duplicate_urls=tuple(duplicates),
        added_urls=tuple(added),
        failed_urls=tuple(failed),
        dry_run=config.dry_run,
    )


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Reconcile open Gordian issues into temporary GitHub Project 9."
    )
    parser.add_argument("--owner", default="kmosoti")
    parser.add_argument("--repository", default="kmosoti/gordian")
    parser.add_argument("--project", type=int, default=9, dest="project_number")
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Report missing items without adding them.",
    )
    return parser


def main() -> int:
    arguments = _parser().parse_args()
    config = Config(
        owner=arguments.owner,
        repository=arguments.repository,
        project_number=arguments.project_number,
        dry_run=arguments.dry_run,
    )
    try:
        report = reconcile(config)
    except (OSError, RuntimeError) as error:
        print(str(error), file=sys.stderr)
        print(
            "Verify `gh` authentication and run `gh auth refresh -s project` when needed.",
            file=sys.stderr,
        )
        return 2

    print(json.dumps(asdict(report), indent=2, sort_keys=True))
    return 1 if report.failed_urls else 0


if __name__ == "__main__":
    raise SystemExit(main())
