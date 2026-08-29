#!/usr/bin/env python3
"""Idempotently reconcile Gordian repository issues into a GitHub Project.

This is temporary bootstrap tooling. GitHub Project fields are not canonical
Gordian state, and issue closure is not evidence that an Atom is satisfied.
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
from collections import Counter
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, Iterable, Sequence

ISSUE_URL = re.compile(r"^https://github\.com/[^/]+/[^/]+/issues/\d+$")


class CommandError(RuntimeError):
    """Raised when an external command fails."""


@dataclass(frozen=True, slots=True)
class Issue:
    number: int
    title: str
    url: str


@dataclass(frozen=True, slots=True)
class ReconciliationReport:
    owner: str
    repository: str
    project_number: int
    repository_issue_count: int
    project_issue_url_count: int
    missing_before: tuple[str, ...]
    duplicate_urls_before: tuple[str, ...]
    added: tuple[str, ...]
    missing_after: tuple[str, ...]

    @property
    def converged(self) -> bool:
        return not self.missing_after


def run(
    args: Sequence[str],
    *,
    expect_json: bool = False,
) -> Any:
    completed = subprocess.run(
        args,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if completed.returncode != 0:
        command = " ".join(args)
        stderr = completed.stderr.strip() or "<no stderr>"
        raise CommandError(f"command failed ({completed.returncode}): {command}\n{stderr}")
    if not expect_json:
        return completed.stdout
    try:
        return json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise CommandError(
            f"command returned invalid JSON: {' '.join(args)}\n{completed.stdout[:500]}"
        ) from error


def collect_issue_urls(value: Any) -> Iterable[str]:
    """Yield issue URLs from GitHub CLI JSON without coupling to one output shape."""
    if isinstance(value, str):
        if ISSUE_URL.fullmatch(value):
            yield value
        return
    if isinstance(value, dict):
        for nested in value.values():
            yield from collect_issue_urls(nested)
        return
    if isinstance(value, list):
        for nested in value:
            yield from collect_issue_urls(nested)


def repository_issues(repository: str, limit: int) -> list[Issue]:
    payload = run(
        [
            "gh",
            "issue",
            "list",
            "--repo",
            repository,
            "--state",
            "all",
            "--limit",
            str(limit),
            "--json",
            "number,title,url",
        ],
        expect_json=True,
    )
    if not isinstance(payload, list):
        raise CommandError("gh issue list returned a non-list JSON value")

    issues: list[Issue] = []
    for row in payload:
        if not isinstance(row, dict):
            raise CommandError("gh issue list returned a malformed item")
        issues.append(
            Issue(
                number=int(row["number"]),
                title=str(row["title"]),
                url=str(row["url"]),
            )
        )
    issues.sort(key=lambda issue: issue.number)
    return issues


def project_issue_urls(owner: str, project_number: int, limit: int) -> Counter[str]:
    payload = run(
        [
            "gh",
            "project",
            "item-list",
            str(project_number),
            "--owner",
            owner,
            "--limit",
            str(limit),
            "--format",
            "json",
        ],
        expect_json=True,
    )
    return Counter(collect_issue_urls(payload))


def reconcile(
    *,
    owner: str,
    repository: str,
    project_number: int,
    limit: int,
    dry_run: bool,
) -> ReconciliationReport:
    issues = repository_issues(repository, limit)
    desired_urls = {issue.url for issue in issues}
    before = project_issue_urls(owner, project_number, limit)

    missing_before = tuple(sorted(desired_urls - before.keys()))
    duplicate_urls = tuple(sorted(url for url, count in before.items() if count > 1))

    added: list[str] = []
    if not dry_run:
        for url in missing_before:
            run(
                [
                    "gh",
                    "project",
                    "item-add",
                    str(project_number),
                    "--owner",
                    owner,
                    "--url",
                    url,
                    "--format",
                    "json",
                ]
            )
            added.append(url)
        after = project_issue_urls(owner, project_number, limit)
        missing_after = tuple(sorted(desired_urls - after.keys()))
    else:
        missing_after = missing_before

    return ReconciliationReport(
        owner=owner,
        repository=repository,
        project_number=project_number,
        repository_issue_count=len(issues),
        project_issue_url_count=sum(before.values()),
        missing_before=missing_before,
        duplicate_urls_before=duplicate_urls,
        added=tuple(added),
        missing_after=missing_after,
    )


def parse_args(argv: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--owner", default="kmosoti", help="GitHub Project owner")
    parser.add_argument(
        "--repository",
        default="kmosoti/gordian",
        help="Repository in OWNER/NAME form",
    )
    parser.add_argument("--project", type=int, default=9, help="GitHub Project number")
    parser.add_argument(
        "--limit",
        type=int,
        default=1000,
        help="Maximum issues/project items to inspect",
    )
    parser.add_argument("--dry-run", action="store_true", help="Report without mutation")
    parser.add_argument("--report", type=Path, help="Optional JSON report destination")
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    if shutil.which("gh") is None:
        print("error: GitHub CLI `gh` is not installed or not on PATH", file=sys.stderr)
        return 2

    try:
        run(["gh", "auth", "status"])
        report = reconcile(
            owner=args.owner,
            repository=args.repository,
            project_number=args.project,
            limit=args.limit,
            dry_run=args.dry_run,
        )
    except CommandError as error:
        print(f"error: {error}", file=sys.stderr)
        print(
            "hint: GitHub Projects mutation requires the `project` token scope; "
            "run `gh auth refresh -s project` when authentication is otherwise valid.",
            file=sys.stderr,
        )
        return 1

    encoded = json.dumps(
        {**asdict(report), "converged": report.converged},
        indent=2,
        sort_keys=True,
    )
    print(encoded)
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(encoded + "\n", encoding="utf-8")

    if args.dry_run:
        return 0
    return 0 if report.converged else 1


if __name__ == "__main__":
    raise SystemExit(main())
