from __future__ import annotations

import io
import json
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import patch

from gordian_orchestration.gh import GitHubConfigurationError
from gordian_orchestration.milestone_contracts import (
    InitiativeContract,
    Milestone,
    MilestoneUpdate,
    audit_milestones,
    main,
    parse_initiative_contracts,
    plan_milestone_sync,
    render_description,
)

TABLE = """| Initiative | Atoms | Purpose | Acceptance rule |
| --- | --- | --- | --- |
| [Alpha](https://example.test/1) | #1 | work | alpha rule |
| [Beta](https://example.test/2) | #2 | work | beta rule |
"""


class MilestoneContractTests(unittest.TestCase):
    def test_parse_four_cell_table(self) -> None:
        self.assertEqual(
            parse_initiative_contracts(TABLE),
            (
                InitiativeContract("Alpha", "alpha rule"),
                InitiativeContract("Beta", "beta rule"),
            ),
        )

    def test_render_is_idempotent_and_preserves_prose(self) -> None:
        original = "Intro\n\nAcceptance: old\nTail\n"
        expected = "Intro\n\nAcceptance: new\nTail\n"
        self.assertEqual(render_description(original, "new"), expected)
        self.assertEqual(render_description(expected, "new"), expected)
        self.assertEqual(render_description("Intro", "new"), "Intro\nAcceptance: new\n")

    def test_audit_reports_missing_extra_and_drift(self) -> None:
        milestones = (
            Milestone(1, "Alpha", "Acceptance: wrong\n"),
            Milestone(3, "Gamma", "Acceptance: extra\n"),
        )
        report = audit_milestones(parse_initiative_contracts(TABLE), milestones)
        self.assertFalse(report.clean)
        self.assertEqual(len(report.problems), 3)

    def test_sync_plan_is_sorted_and_contains_only_changed_descriptions(self) -> None:
        milestones = (
            Milestone(2, "Beta", "Acceptance: old\n"),
            Milestone(1, "Alpha", "Acceptance: alpha rule\n"),
        )
        updates = plan_milestone_sync(parse_initiative_contracts(TABLE), milestones)
        self.assertEqual(
            updates,
            (
                MilestoneUpdate(2, "Beta", "Acceptance: old\n", "Acceptance: beta rule\n"),
            ),
        )

    def test_sync_dry_run_does_not_write(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            index = Path(directory) / "index.md"
            index.write_text(TABLE, encoding="utf-8")
            milestones = [
                {"number": 2, "title": "Beta", "description": "Acceptance: old\n"},
                {"number": 1, "title": "Alpha", "description": "Acceptance: alpha rule\n"},
            ]
            output = io.StringIO()
            with patch(
                "gordian_orchestration.milestone_contracts.fetch_milestones",
                return_value=tuple(Milestone(**row) for row in milestones),
            ), patch(
                "gordian_orchestration.milestone_contracts.run_gh"
            ) as gh, redirect_stdout(output):
                self.assertEqual(main(["--issue-index", str(index), "sync"]), 0)
            self.assertEqual(gh.call_count, 0)
            payload = json.loads(output.getvalue())
            self.assertEqual(payload["changed_milestones"], [{"number": 2, "title": "Beta"}])

    def test_check_configuration_error_is_exit_78(self) -> None:
        with patch(
            "gordian_orchestration.milestone_contracts.fetch_milestones",
            side_effect=GitHubConfigurationError("missing token"),
        ), tempfile.TemporaryDirectory() as directory:
            index = Path(directory) / "index.md"
            index.write_text(TABLE, encoding="utf-8")
            self.assertEqual(main(["--issue-index", str(index), "check"]), 78)


if __name__ == "__main__":
    unittest.main()
