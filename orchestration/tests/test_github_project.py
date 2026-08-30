from __future__ import annotations

import unittest
from unittest.mock import call, patch

from gordian_orchestration.github_project import Config, IssueRef, ProjectItemRef, reconcile


class ReconciliationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.config = Config(
            owner="kmosoti",
            repository="kmosoti/gordian",
            project_number=9,
            dry_run=False,
        )
        self.issue_one = IssueRef(
            number=1,
            title="First",
            url="https://github.com/kmosoti/gordian/issues/1",
        )
        self.issue_two = IssueRef(
            number=2,
            title="Second",
            url="https://github.com/kmosoti/gordian/issues/2",
        )

    @patch("gordian_orchestration.github_project._run_gh", return_value="{}")
    @patch("gordian_orchestration.github_project._project_items")
    @patch("gordian_orchestration.github_project._open_issues")
    def test_dry_run_reports_missing_and_duplicate_items(
        self,
        open_issues,
        project_items,
        run_gh,
    ) -> None:
        open_issues.return_value = [self.issue_one, self.issue_two]
        project_items.return_value = [
            ProjectItemRef(item_id="a", url=self.issue_one.url),
            ProjectItemRef(item_id="b", url=self.issue_one.url),
        ]
        config = Config(
            owner=self.config.owner,
            repository=self.config.repository,
            project_number=self.config.project_number,
            dry_run=True,
        )

        report = reconcile(config)

        self.assertEqual(report.missing_before, (self.issue_two.url,))
        self.assertEqual(report.remaining_after, (self.issue_two.url,))
        self.assertEqual(report.duplicate_urls_before, (self.issue_one.url,))
        self.assertEqual(report.added_urls, ())
        self.assertEqual(report.failed_urls, ())
        self.assertFalse(report.converged)
        run_gh.assert_called_once_with(
            ["project", "view", "9", "--owner", "kmosoti", "--format", "json"]
        )

    @patch("gordian_orchestration.github_project._run_gh", return_value="{}")
    @patch("gordian_orchestration.github_project._project_items")
    @patch("gordian_orchestration.github_project._open_issues")
    def test_apply_adds_missing_issues_and_verifies_convergence(
        self,
        open_issues,
        project_items,
        run_gh,
    ) -> None:
        open_issues.return_value = [self.issue_two, self.issue_one]
        project_items.side_effect = [
            [],
            [
                ProjectItemRef(item_id="a", url=self.issue_one.url),
                ProjectItemRef(item_id="b", url=self.issue_two.url),
            ],
        ]

        report = reconcile(self.config)

        self.assertEqual(report.missing_before, (self.issue_one.url, self.issue_two.url))
        self.assertEqual(report.added_urls, (self.issue_one.url, self.issue_two.url))
        self.assertEqual(report.failed_urls, ())
        self.assertEqual(report.remaining_after, ())
        self.assertTrue(report.converged)
        self.assertEqual(project_items.call_count, 2)
        self.assertEqual(
            run_gh.call_args_list,
            [
                call(["project", "view", "9", "--owner", "kmosoti", "--format", "json"]),
                call(
                    [
                        "project",
                        "item-add",
                        "9",
                        "--owner",
                        "kmosoti",
                        "--url",
                        self.issue_one.url,
                        "--format",
                        "json",
                    ]
                ),
                call(
                    [
                        "project",
                        "item-add",
                        "9",
                        "--owner",
                        "kmosoti",
                        "--url",
                        self.issue_two.url,
                        "--format",
                        "json",
                    ]
                ),
            ],
        )


if __name__ == "__main__":
    unittest.main()
