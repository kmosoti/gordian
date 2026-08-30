"""Reconcile Gordian issues into the temporary GitHub Project projection.

This module is deliberately orchestration-only. It invokes the authenticated GitHub
CLI and does not own Mission Graph, Atom, dependency, readiness, or satisfaction
semantics.

Derived board fields are not written here. `gordian-derive-status` owns Wave, Fan In,
Fan Out and Status; this command owns membership only.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

from . import provenance
from .gh import GH_AUTH_HINT, run_gh


@dataclass(frozen=True, slots=True)
class Config:
    owner: str
    repository: str
    project_number: int
    dry_run: bool
    limit: int = 1000


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
    project_issue_count_before: int
    missing_before: tuple[str, ...]
    duplicate_urls_before: tuple[str, ...]
    added_urls: tuple[str, ...]
    failed_urls: tuple[str, ...]
    remaining_after: tuple[str, ...]
    dry_run: bool
    generated_at: str
    source_change_id: str
    source_commit_id: str
    tool_versions: dict[str, str]

    @property
    def converged(self) -> bool:
        return not self.remaining_after and not self.failed_urls

    def as_json_object(self) -> dict[str, Any]:
        """Emit the report with its source and environment identity.

        A reconciliation report that cannot say which working copy and which tool
        versions produced it is not evidence, only an assertion.
        """
        return {**asdict(self), "converged": self.converged}


def _run_gh(arguments: list[str]) -> str:
    """Run GitHub CLI without a shell and return stdout.

    An unattended agent authenticates by exporting `GH_TOKEN`; see `gh.GH_AUTH_HINT`.
    """
    return run_gh(arguments)


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
            str(config.limit),
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
            str(config.limit),
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


def _missing(issue_urls: set[str], items: list[ProjectItemRef]) -> tuple[str, ...]:
    return tuple(sorted(issue_urls - {item.url for item in items}))


def reconcile(
    config: Config, *, stamp: provenance.Provenance | None = None
) -> ReconciliationReport:
    # This is also a token-scope and project-existence check. The non-interactive
    # remediation is a classic personal access token with the `repo` and `project`
    # scopes in `GH_TOKEN`; `gh auth refresh -s project` is the interactive one.
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
    before_items = _project_items(config)
    before_counts = Counter(item.url for item in before_items)
    issue_urls = {issue.url for issue in issues}
    missing_before = _missing(issue_urls, before_items)
    duplicates = tuple(sorted(url for url, count in before_counts.items() if count > 1))

    added: list[str] = []
    failed: list[str] = []
    if not config.dry_run:
        for url in missing_before:
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
        remaining_after = _missing(issue_urls, _project_items(config))
    else:
        remaining_after = missing_before

    stamp = stamp or provenance.collect()
    return ReconciliationReport(
        owner=config.owner,
        repository=config.repository,
        project_number=config.project_number,
        open_issue_count=len(issues),
        project_issue_count_before=len(before_items),
        missing_before=missing_before,
        duplicate_urls_before=duplicates,
        added_urls=tuple(added),
        failed_urls=tuple(failed),
        remaining_after=remaining_after,
        dry_run=config.dry_run,
        generated_at=stamp.generated_at,
        source_change_id=stamp.source_change_id,
        source_commit_id=stamp.source_commit_id,
        tool_versions=stamp.tool_versions,
    )


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Reconcile open Gordian issues into temporary GitHub Project 9."
    )
    parser.add_argument("--owner", default="kmosoti")
    parser.add_argument("--repository", default="kmosoti/gordian")
    parser.add_argument("--project", type=int, default=9, dest="project_number")
    parser.add_argument("--limit", type=int, default=1000)
    parser.add_argument("--report", type=Path)
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
        limit=arguments.limit,
    )
    try:
        report = reconcile(config)
    except (OSError, RuntimeError) as error:
        print(str(error), file=sys.stderr)
        print(GH_AUTH_HINT, file=sys.stderr)
        return 2

    encoded = json.dumps(report.as_json_object(), indent=2, sort_keys=True)
    print(encoded)
    if arguments.report:
        arguments.report.parent.mkdir(parents=True, exist_ok=True)
        arguments.report.write_text(encoded + "\n", encoding="utf-8")

    if report.dry_run:
        return 0
    return 0 if report.converged else 1


if __name__ == "__main__":
    raise SystemExit(main())
