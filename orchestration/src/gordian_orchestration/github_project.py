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
from .gh import EX_CONFIG, GH_AUTH_HINT, GitHubConfigurationError, graphql, preflight, run_gh


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
    is_archived: bool = False


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
    archived_urls_before: tuple[str, ...] = ()
    unarchived_urls: tuple[str, ...] = ()
    remaining_archived_after: tuple[str, ...] = ()

    @property
    def converged(self) -> bool:
        return (
            not self.remaining_after
            and not self.remaining_archived_after
            and not self.failed_urls
            and not self.duplicate_urls_before
        )

    def as_json_object(self) -> dict[str, Any]:
        """Emit the report with its source and environment identity.

        A reconciliation report that cannot say which working copy and which tool
        versions produced it is not evidence, only an assertion.
        """
        return {**asdict(self), "converged": self.converged}


def _run_gh(arguments: list[str]) -> str:
    """Run GitHub CLI without a shell and return stdout.

    An unattended agent authenticates through the fail-closed preflight; see
    `gh.GH_AUTH_HINT`.
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


_PROJECT_ITEMS_QUERY = """
query($owner:String!,$number:Int!,$cursor:String){
  user(login:$owner){
    projectV2(number:$number){
      items(first:100, after:$cursor, archivedStates:[ARCHIVED,NOT_ARCHIVED]){
        totalCount
        pageInfo{hasNextPage endCursor}
        nodes{
          id
          isArchived
          content{... on Issue{url}}
        }
      }
    }
  }
}
"""


def _project_items(config: Config) -> list[ProjectItemRef]:
    """Read all issue items, including archived items, in one paginated query."""
    items: list[ProjectItemRef] = []
    cursor = ""
    seen_cursors: set[str] = set()
    declared_total: int | None = None
    retrieved_nodes = 0
    while True:
        variables: dict[str, str | int] = {
            "owner": config.owner,
            "number": config.project_number,
        }
        if cursor:
            variables["cursor"] = cursor
        data = graphql(_PROJECT_ITEMS_QUERY, variables)
        user = data.get("user")
        project = user.get("projectV2") if isinstance(user, dict) else None
        connection = project.get("items") if isinstance(project, dict) else None
        if not isinstance(connection, dict):
            raise RuntimeError("GitHub GraphQL project item connection is missing")

        total = connection.get("totalCount")
        if isinstance(total, bool) or not isinstance(total, int) or total < 0:
            raise RuntimeError("GitHub GraphQL project item totalCount is invalid")
        if declared_total is None:
            declared_total = total
        elif declared_total != total:
            raise RuntimeError("GitHub GraphQL project item totalCount changed during read")

        nodes = connection.get("nodes")
        if not isinstance(nodes, list):
            raise RuntimeError("GitHub GraphQL project item nodes are missing")
        retrieved_nodes += len(nodes)
        for row in nodes:
            if not isinstance(row, dict):
                continue
            content = row.get("content")
            if not isinstance(content, dict):
                continue
            url = content.get("url")
            if not isinstance(url, str) or "/issues/" not in url:
                continue
            is_archived = row.get("isArchived")
            if not isinstance(is_archived, bool):
                raise RuntimeError(
                    f"project item {row.get('id', '<unknown>')} has invalid archive state"
                )
            item_id = row.get("id")
            items.append(
                ProjectItemRef(
                    item_id=item_id if isinstance(item_id, str) else None,
                    url=url,
                    is_archived=is_archived,
                )
            )

        page = connection.get("pageInfo")
        if not isinstance(page, dict):
            raise RuntimeError("GitHub GraphQL project item pageInfo is missing")
        has_next = page.get("hasNextPage")
        if type(has_next) is not bool:
            raise RuntimeError("GitHub GraphQL project item pageInfo is invalid")
        if not has_next:
            break
        next_cursor = page.get("endCursor")
        if not isinstance(next_cursor, str) or not next_cursor:
            raise RuntimeError("GitHub GraphQL project item page has no continuation cursor")
        if next_cursor in seen_cursors or next_cursor == cursor:
            raise RuntimeError("GitHub GraphQL project item pagination repeated a cursor")
        seen_cursors.add(next_cursor)
        cursor = next_cursor

    if declared_total is None or retrieved_nodes != declared_total:
        raise RuntimeError(
            "GitHub GraphQL project item pagination incomplete: "
            f"retrieved {retrieved_nodes}, expected {declared_total}"
        )
    return sorted(items, key=lambda item: (item.url, item.item_id or "", item.is_archived))


def _missing(issue_urls: set[str], items: list[ProjectItemRef]) -> tuple[str, ...]:
    return tuple(
        sorted(issue_urls - {item.url for item in items if not item.is_archived})
    )


def _archived(issue_urls: set[str], items: list[ProjectItemRef]) -> tuple[str, ...]:
    active_urls = {item.url for item in items if not item.is_archived}
    return tuple(
        sorted(
            issue_urls
            & {item.url for item in items if item.is_archived}
            - active_urls
        )
    )


def _archived_item(items: list[ProjectItemRef], url: str) -> ProjectItemRef | None:
    candidates = sorted(
        (item for item in items if item.url == url and item.is_archived),
        key=lambda item: item.item_id or "",
    )
    return candidates[0] if candidates else None


def reconcile(
    config: Config, *, stamp: provenance.Provenance | None = None
) -> ReconciliationReport:
    issues = _open_issues(config)
    before_items = _project_items(config)
    before_counts = Counter(item.url for item in before_items)
    issue_urls = {issue.url for issue in issues}
    missing_before = _missing(issue_urls, before_items)
    archived_before = _archived(issue_urls, before_items)
    duplicates = tuple(sorted(url for url in issue_urls if before_counts[url] > 1))

    added: list[str] = []
    unarchived: list[str] = []
    failed: list[str] = []
    if not config.dry_run:
        for url in archived_before:
            item = _archived_item(before_items, url)
            if item is None or item.item_id is None:
                print(f"{url}: archived project item has no item id", file=sys.stderr)
                failed.append(url)
                continue
            try:
                _run_gh(
                    [
                        "project",
                        "item-archive",
                        str(config.project_number),
                        "--owner",
                        config.owner,
                        "--id",
                        item.item_id,
                        "--undo",
                    ]
                )
            except RuntimeError as error:
                print(str(error), file=sys.stderr)
                failed.append(url)
            else:
                unarchived.append(url)
        for url in tuple(sorted(set(missing_before) - set(archived_before))):
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
        after_items = _project_items(config)
        remaining_after = _missing(issue_urls, after_items)
        remaining_archived_after = _archived(issue_urls, after_items)
    else:
        remaining_after = missing_before
        remaining_archived_after = archived_before

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
        archived_urls_before=archived_before,
        unarchived_urls=tuple(unarchived),
        remaining_archived_after=remaining_archived_after,
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
    parser.add_argument(
        "--check",
        action="store_true",
        help="Read only; exit nonzero on missing, duplicate, or failed membership.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    raw = list(sys.argv[1:] if argv is None else argv)
    if raw[:1] == ["reconcile"]:
        raw.pop(0)
    arguments = _parser().parse_args(raw)
    if arguments.check and arguments.dry_run:
        print("--check already implies --dry-run", file=sys.stderr)
        return 2
    config = Config(
        owner=arguments.owner,
        repository=arguments.repository,
        project_number=arguments.project_number,
        dry_run=arguments.dry_run or arguments.check,
        limit=arguments.limit,
    )
    try:
        preflight(
            repository=arguments.repository,
            project_owner=arguments.owner,
            project_number=arguments.project_number,
        )
        report = reconcile(config)
    except GitHubConfigurationError as error:
        print(str(error), file=sys.stderr)
        return EX_CONFIG
    except (OSError, RuntimeError) as error:
        print(str(error), file=sys.stderr)
        print(GH_AUTH_HINT, file=sys.stderr)
        return 2

    encoded = json.dumps(report.as_json_object(), indent=2, sort_keys=True)
    print(encoded)
    if arguments.report:
        arguments.report.parent.mkdir(parents=True, exist_ok=True)
        arguments.report.write_text(encoded + "\n", encoding="utf-8")

    if arguments.check:
        return 0 if report.converged else 1
    if report.dry_run:
        return 0
    return 0 if report.converged else 1


if __name__ == "__main__":
    raise SystemExit(main())
