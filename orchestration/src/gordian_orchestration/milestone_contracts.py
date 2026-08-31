"""Audit and reconcile GitHub milestone Acceptance contracts."""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import asdict, dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from .bootstrap_claims import require_live_claim
from .gh import EX_CONFIG, GitHubConfigurationError, preflight, run_gh, run_gh_json

DEFAULT_REPOSITORY = "kmosoti/gordian"
DEFAULT_PROJECT_OWNER = "kmosoti"
DEFAULT_PROJECT_NUMBER = 9
DEFAULT_ISSUE_INDEX = Path("docs/implementation/issue-index.md")
_ACCEPTANCE_LINE = re.compile(r"^Acceptance: (.*)$")
_LINK = re.compile(r"^\[([^]]+)\]\([^)]+\)$")


@dataclass(frozen=True, slots=True)
class InitiativeContract:
    title: str
    rule: str


@dataclass(frozen=True, slots=True)
class Milestone:
    number: int
    title: str
    description: str


@dataclass(frozen=True, slots=True)
class MilestoneAuditReport:
    expected: tuple[str, ...]
    actual: tuple[str, ...]
    problems: tuple[str, ...]

    @property
    def clean(self) -> bool:
        return not self.problems

    def as_json_object(self) -> dict[str, Any]:
        return {**asdict(self), "clean": self.clean}


@dataclass(frozen=True, slots=True)
class MilestoneUpdate:
    """One deterministic, description-only milestone reconciliation operation."""

    number: int
    title: str
    old_description: str
    new_description: str


def plan_milestone_sync(
    contracts: tuple[InitiativeContract, ...], milestones: tuple[Milestone, ...]
) -> tuple[MilestoneUpdate, ...]:
    """Plan acceptance-line repairs without performing any GitHub mutation."""
    by_title = {milestone.title: milestone for milestone in milestones}
    updates = [
        MilestoneUpdate(
            number=milestone.number,
            title=milestone.title,
            old_description=milestone.description,
            new_description=render_description(milestone.description, contract.rule),
        )
        for contract in contracts
        if (milestone := by_title.get(contract.title)) is not None
        and render_description(milestone.description, contract.rule) != milestone.description
    ]
    return tuple(sorted(updates, key=lambda update: (update.number, update.title)))


def parse_initiative_contracts(text: str) -> tuple[InitiativeContract, ...]:
    """Parse the four-cell Initiative register and its exact Acceptance rule column."""
    contracts: list[InitiativeContract] = []
    for line in text.splitlines():
        if not line.startswith("|"):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) != 4 or cells[0] == "Initiative" or cells[0].startswith("---"):
            continue
        link = _LINK.fullmatch(cells[0])
        if link is None:
            continue
        if not cells[3]:
            raise ValueError(f"Initiative {link.group(1)!r} has an empty Acceptance rule")
        contract = InitiativeContract(title=link.group(1), rule=cells[3])
        if any(existing.title == contract.title for existing in contracts):
            raise ValueError(f"Initiative table repeats {contract.title!r}")
        contracts.append(contract)
    if not contracts:
        raise ValueError("issue index contains no four-cell Initiative register")
    return tuple(contracts)


def parse_milestones(payload: Any) -> tuple[Milestone, ...]:
    if not isinstance(payload, list):
        raise ValueError("GitHub milestones response is not a list")
    milestones: list[Milestone] = []
    for item in payload:
        if not isinstance(item, dict):
            raise ValueError("GitHub milestone response contains a non-object")
        try:
            number = int(item["number"])
            title = str(item["title"])
            description = str(item.get("description") or "")
        except (KeyError, TypeError, ValueError) as error:
            raise ValueError("GitHub milestone response has an unreadable row") from error
        milestones.append(Milestone(number=number, title=title, description=description))
    return tuple(sorted(milestones, key=lambda milestone: (milestone.number, milestone.title)))


def fetch_milestones(repository: str) -> tuple[Milestone, ...]:
    payload = run_gh_json(
        ["api", f"repos/{repository}/milestones?state=all&per_page=100"]
    )
    return parse_milestones(payload)


def _acceptance_lines(description: str) -> list[tuple[int, str]]:
    return [
        (index, match.group(1))
        for index, line in enumerate(description.splitlines())
        if (match := _ACCEPTANCE_LINE.fullmatch(line)) is not None
    ]


def render_description(description: str, rule: str) -> str:
    """Replace one existing contract line or append one, preserving other prose."""
    lines = description.splitlines(keepends=True)
    acceptance = [
        index
        for index, line in enumerate(lines)
        if _ACCEPTANCE_LINE.fullmatch(line.rstrip("\r\n"))
    ]
    rendered = f"Acceptance: {rule}\n"
    if acceptance:
        first = acceptance[0]
        replacement = rendered if lines[first].endswith("\n") else rendered.rstrip("\n")
        return "".join(
            replacement if index == first else line
            for index, line in enumerate(lines)
            if index not in acceptance[1:]
        )
    separator = "" if not description or description.endswith(("\n", "\r")) else "\n"
    return description + separator + rendered


def audit_milestones(
    contracts: tuple[InitiativeContract, ...], milestones: tuple[Milestone, ...]
) -> MilestoneAuditReport:
    expected = tuple(contract.title for contract in contracts)
    actual = tuple(milestone.title for milestone in milestones)
    by_title = {milestone.title: milestone for milestone in milestones}
    problems: list[str] = []
    duplicate_titles = sorted(
        title for title, count in _counts(actual).items() if count > 1
    )
    problems.extend(f"duplicate GitHub milestone {title!r}" for title in duplicate_titles)
    for contract in contracts:
        milestone = by_title.get(contract.title)
        if milestone is None:
            problems.append(f"missing GitHub milestone {contract.title!r}")
            continue
        lines = _acceptance_lines(milestone.description)
        if len(lines) != 1:
            problems.append(
                f"milestone {contract.title!r}: expected exactly one Acceptance line, "
                f"found {len(lines)}"
            )
        elif lines[0][1] != contract.rule:
            problems.append(f"milestone {contract.title!r}: Acceptance rule differs")
    expected_set = set(expected)
    for milestone in milestones:
        if milestone.title not in expected_set:
            problems.append(f"unexpected GitHub milestone {milestone.title!r}")
    return MilestoneAuditReport(expected=expected, actual=actual, problems=tuple(problems))


def _counts(values: tuple[str, ...]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for value in values:
        counts[value] = counts.get(value, 0) + 1
    return counts


def _contracts(path: Path) -> tuple[InitiativeContract, ...]:
    return parse_initiative_contracts(path.read_text(encoding="utf-8"))


def _command_check(arguments: argparse.Namespace) -> int:
    report = audit_milestones(
        _contracts(arguments.issue_index), fetch_milestones(arguments.repository)
    )
    if arguments.json:
        print(json.dumps(report.as_json_object(), indent=2, sort_keys=True))
    elif report.clean:
        print(f"clean: {len(report.expected)} Initiative milestone contracts")
    else:
        for problem in report.problems:
            print(f"FAIL: {problem}")
    return 0 if report.clean else 1


def _command_sync(arguments: argparse.Namespace) -> int:
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
    contracts = _contracts(arguments.issue_index)
    milestones = fetch_milestones(arguments.repository)
    planned_updates = plan_milestone_sync(contracts, milestones)
    payload = {
        "apply": arguments.apply,
        "changed_milestones": [
            {"number": update.number, "title": update.title} for update in planned_updates
        ],
        "changed_count": len(planned_updates),
    }
    if not arguments.apply:
        print(json.dumps(payload, indent=2, sort_keys=True))
        return 0
    originals = {update.number: update.old_description for update in planned_updates}
    applied: list[int] = []
    try:
        for update in planned_updates:
            run_gh(
                [
                    "api",
                    "--method",
                    "PATCH",
                    f"repos/{arguments.repository}/milestones/{update.number}",
                    "-f",
                    f"description={update.new_description}",
                ]
            )
            applied.append(update.number)
        fresh = fetch_milestones(arguments.repository)
        report = audit_milestones(contracts, fresh)
        if not report.clean:
            raise RuntimeError("post-mutation milestone drift: " + "; ".join(report.problems))
    except (OSError, RuntimeError, TypeError, ValueError) as error:
        failures: list[str] = []
        for number in reversed(applied):
            try:
                run_gh(
                    [
                        "api",
                        "--method",
                        "PATCH",
                        f"repos/{arguments.repository}/milestones/{number}",
                        "-f",
                        f"description={originals[number]}",
                    ]
                )
            except RuntimeError as restore_error:
                failures.append(f"milestone #{number}: {restore_error}")
        if failures:
            raise RuntimeError(
                f"milestone sync failed ({error}); compensation also failed: "
                + "; ".join(failures)
            ) from error
        raise RuntimeError(f"milestone sync failed and was compensated: {error}") from error
    print(json.dumps(payload, indent=2, sort_keys=True))
    return 0


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="gordian-milestone-contracts")
    parser.add_argument("--repository", default=DEFAULT_REPOSITORY)
    parser.add_argument("--project-owner", default=DEFAULT_PROJECT_OWNER)
    parser.add_argument(
        "--project", type=int, default=DEFAULT_PROJECT_NUMBER, dest="project_number"
    )
    parser.add_argument("--issue-index", type=Path, default=DEFAULT_ISSUE_INDEX)
    subparsers = parser.add_subparsers(dest="command", required=True)
    check = subparsers.add_parser("check")
    check.add_argument("--json", action="store_true")
    check.set_defaults(handler=_command_check)
    sync = subparsers.add_parser("sync")
    sync.add_argument("--apply", action="store_true")
    sync.set_defaults(handler=_command_sync)
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
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
