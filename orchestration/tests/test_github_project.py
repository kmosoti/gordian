from __future__ import annotations

import json
import unittest
from unittest.mock import call, patch

from gordian_orchestration.gh import GitHubConfigurationError
from gordian_orchestration.github_project import (
    _PROJECT_ITEMS_QUERY,
    Config,
    IssueRef,
    ProjectItemRef,
    ReconciliationReport,
    _project_items,
    main,
    reconcile,
)
from gordian_orchestration.provenance import Provenance

STAMP = Provenance(
    generated_at="2026-08-30T00:00:00Z",
    source_change_id="qxpvzzmnopqr",
    source_commit_id="4f2a1b0c9d8e",
    tool_versions={"gh": "gh version 2.63.2", "jj": "jj 0.34.0"},
)


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

        report = reconcile(config, stamp=STAMP)

        self.assertEqual(report.missing_before, (self.issue_two.url,))
        self.assertEqual(report.remaining_after, (self.issue_two.url,))
        self.assertEqual(report.duplicate_urls_before, (self.issue_one.url,))
        self.assertEqual(report.added_urls, ())
        self.assertEqual(report.failed_urls, ())
        self.assertFalse(report.converged)
        run_gh.assert_not_called()

    @patch("gordian_orchestration.github_project._run_gh", return_value="{}")
    @patch("gordian_orchestration.github_project._project_items")
    @patch("gordian_orchestration.github_project._open_issues")
    def test_dry_run_reports_archive_only_url_separately(
        self,
        open_issues,
        project_items,
        run_gh,
    ) -> None:
        open_issues.return_value = [self.issue_one]
        project_items.return_value = [
            ProjectItemRef(item_id="archived", url=self.issue_one.url, is_archived=True)
        ]

        report = reconcile(
            Config(
                owner=self.config.owner,
                repository=self.config.repository,
                project_number=self.config.project_number,
                dry_run=True,
            ),
            stamp=STAMP,
        )

        self.assertEqual(report.missing_before, (self.issue_one.url,))
        self.assertEqual(report.archived_urls_before, (self.issue_one.url,))
        self.assertEqual(report.remaining_after, (self.issue_one.url,))
        self.assertEqual(report.remaining_archived_after, (self.issue_one.url,))
        self.assertEqual(report.unarchived_urls, ())
        run_gh.assert_not_called()

    @patch("gordian_orchestration.github_project._run_gh", return_value="{}")
    @patch("gordian_orchestration.github_project._project_items")
    @patch("gordian_orchestration.github_project._open_issues")
    def test_apply_unarchives_archive_only_item_and_verifies(
        self,
        open_issues,
        project_items,
        run_gh,
    ) -> None:
        open_issues.return_value = [self.issue_one]
        project_items.side_effect = [
            [ProjectItemRef(item_id="archived", url=self.issue_one.url, is_archived=True)],
            [ProjectItemRef(item_id="archived", url=self.issue_one.url)],
        ]

        report = reconcile(self.config, stamp=STAMP)

        self.assertEqual(report.missing_before, (self.issue_one.url,))
        self.assertEqual(report.archived_urls_before, (self.issue_one.url,))
        self.assertEqual(report.unarchived_urls, (self.issue_one.url,))
        self.assertEqual(report.remaining_after, ())
        self.assertEqual(report.remaining_archived_after, ())
        self.assertTrue(report.converged)
        self.assertEqual(
            run_gh.call_args_list,
            [
                call(
                    [
                        "project",
                        "item-archive",
                        "9",
                        "--owner",
                        "kmosoti",
                        "--id",
                        "archived",
                        "--undo",
                    ]
                ),
            ],
        )

    @patch("gordian_orchestration.github_project._run_gh", return_value="{}")
    @patch("gordian_orchestration.github_project._project_items")
    @patch("gordian_orchestration.github_project._open_issues")
    def test_apply_reports_archived_item_without_id_as_failure(
        self,
        open_issues,
        project_items,
        run_gh,
    ) -> None:
        open_issues.return_value = [self.issue_one]
        project_items.side_effect = [
            [ProjectItemRef(item_id=None, url=self.issue_one.url, is_archived=True)],
            [ProjectItemRef(item_id=None, url=self.issue_one.url, is_archived=True)],
        ]

        report = reconcile(self.config, stamp=STAMP)

        self.assertEqual(report.failed_urls, (self.issue_one.url,))
        self.assertEqual(report.unarchived_urls, ())
        self.assertEqual(report.remaining_archived_after, (self.issue_one.url,))
        self.assertFalse(report.converged)
        run_gh.assert_not_called()


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

        report = reconcile(self.config, stamp=STAMP)

        self.assertEqual(report.missing_before, (self.issue_one.url, self.issue_two.url))
        self.assertEqual(report.added_urls, (self.issue_one.url, self.issue_two.url))
        self.assertEqual(report.failed_urls, ())
        self.assertEqual(report.remaining_after, ())
        self.assertTrue(report.converged)
        self.assertEqual(project_items.call_count, 2)
        self.assertEqual(
            run_gh.call_args_list,
            [
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

    @patch("gordian_orchestration.github_project._run_gh", return_value="{}")
    @patch("gordian_orchestration.github_project._project_items", return_value=[])
    @patch("gordian_orchestration.github_project._open_issues", return_value=[])
    def test_report_carries_source_and_environment_identity(
        self,
        open_issues,
        project_items,
        run_gh,
    ) -> None:
        config = Config(
            owner=self.config.owner,
            repository=self.config.repository,
            project_number=self.config.project_number,
            dry_run=True,
        )

        payload = reconcile(config, stamp=STAMP).as_json_object()

        for key in ("generated_at", "source_change_id", "source_commit_id", "tool_versions"):
            self.assertIn(key, payload)
            self.assertTrue(payload[key], f"{key} is empty")
        self.assertEqual(payload["generated_at"], "2026-08-30T00:00:00Z")
        self.assertTrue(payload["tool_versions"]["gh"])
        self.assertTrue(json.dumps(payload, sort_keys=True))

    @patch("gordian_orchestration.github_project.provenance.collect", return_value=STAMP)
    @patch("gordian_orchestration.github_project._run_gh", return_value="{}")
    @patch("gordian_orchestration.github_project._project_items", return_value=[])
    @patch("gordian_orchestration.github_project._open_issues", return_value=[])
    def test_provenance_is_collected_when_no_stamp_is_supplied(
        self,
        open_issues,
        project_items,
        run_gh,
        collect,
    ) -> None:
        config = Config(
            owner=self.config.owner,
            repository=self.config.repository,
            project_number=self.config.project_number,
            dry_run=True,
        )

        report = reconcile(config)

        collect.assert_called_once_with()
        self.assertEqual(report.source_commit_id, STAMP.source_commit_id)

    @patch("gordian_orchestration.github_project._run_gh", return_value="{}")
    @patch("gordian_orchestration.github_project._project_items")
    @patch("gordian_orchestration.github_project._open_issues")
    def test_duplicate_only_is_not_converged(
        self,
        open_issues,
        project_items,
        run_gh,
    ) -> None:
        open_issues.return_value = [self.issue_one]
        project_items.return_value = [
            ProjectItemRef(item_id="a", url=self.issue_one.url),
            ProjectItemRef(item_id="b", url=self.issue_one.url),
            ProjectItemRef(item_id="external", url="https://example.com/issues/42"),
        ]
        report = reconcile(
            Config(
                owner="kmosoti",
                repository="kmosoti/gordian",
                project_number=9,
                dry_run=True,
            ),
            stamp=STAMP,
        )
        self.assertEqual(report.remaining_after, ())
        self.assertEqual(report.duplicate_urls_before, (self.issue_one.url,))
        self.assertFalse(report.converged)

    @patch("gordian_orchestration.github_project._run_gh", return_value="{}")
    @patch("gordian_orchestration.github_project._project_items")
    @patch("gordian_orchestration.github_project._open_issues")
    def test_unrelated_project_duplicates_do_not_block_convergence(
        self,
        open_issues,
        project_items,
        run_gh,
    ) -> None:
        open_issues.return_value = [self.issue_one]
        project_items.return_value = [
            ProjectItemRef(item_id="issue", url=self.issue_one.url),
            ProjectItemRef(item_id="external-a", url="https://example.com/issues/42"),
            ProjectItemRef(item_id="external-b", url="https://example.com/issues/42"),
        ]

        report = reconcile(
            Config(
                owner="kmosoti",
                repository="kmosoti/gordian",
                project_number=9,
                dry_run=True,
            ),
            stamp=STAMP,
        )

        self.assertEqual(report.project_issue_count_before, 3)
        self.assertEqual(report.duplicate_urls_before, ())
        self.assertTrue(report.converged)
        run_gh.assert_not_called()


class ProjectItemReadTests(unittest.TestCase):
    def setUp(self) -> None:
        self.config = Config(
            owner="kmosoti", repository="kmosoti/gordian", project_number=9, dry_run=True
        )

    @patch("gordian_orchestration.github_project.graphql")
    def test_project_items_reads_all_pages_and_archive_state(self, graphql_call) -> None:
        graphql_call.side_effect = [
            {
                "user": {
                    "projectV2": {
                        "items": {
                            "totalCount": 3,
                            "pageInfo": {"hasNextPage": True, "endCursor": "cursor-1"},
                            "nodes": [
                                {
                                    "id": "active",
                                    "isArchived": False,
                                    "content": {"url": "https://github.com/kmosoti/gordian/issues/1"},
                                },
                                {"id": "draft", "isArchived": False, "content": None},
                            ],
                        }
                    }
                }
            },
            {
                "user": {
                    "projectV2": {
                        "items": {
                            "totalCount": 3,
                            "pageInfo": {"hasNextPage": False, "endCursor": None},
                            "nodes": [
                                {
                                    "id": "archived",
                                    "isArchived": True,
                                    "content": {"url": "https://github.com/kmosoti/gordian/issues/2"},
                                }
                            ],
                        }
                    }
                }
            },
        ]

        items = _project_items(self.config)

        self.assertEqual(
            items,
            [
                ProjectItemRef(
                    item_id="active",
                    url="https://github.com/kmosoti/gordian/issues/1",
                    is_archived=False,
                ),
                ProjectItemRef(
                    item_id="archived",
                    url="https://github.com/kmosoti/gordian/issues/2",
                    is_archived=True,
                ),
            ],
        )
        self.assertIn("archivedStates:[ARCHIVED,NOT_ARCHIVED]", _PROJECT_ITEMS_QUERY)
        self.assertEqual(
            graphql_call.call_args_list,
            [
                call(_PROJECT_ITEMS_QUERY, {"owner": "kmosoti", "number": 9}),
                call(
                    _PROJECT_ITEMS_QUERY,
                    {"owner": "kmosoti", "number": 9, "cursor": "cursor-1"},
                ),
            ],
        )

    @patch("gordian_orchestration.github_project.graphql")
    def test_project_items_rejects_incomplete_total_count(self, graphql_call) -> None:
        graphql_call.return_value = {
            "user": {
                "projectV2": {
                    "items": {
                        "totalCount": 2,
                        "pageInfo": {"hasNextPage": False, "endCursor": None},
                        "nodes": [],
                    }
                }
            }
        }

        with self.assertRaisesRegex(RuntimeError, "pagination incomplete"):
            _project_items(self.config)

    @patch("gordian_orchestration.github_project.graphql")
    def test_project_items_rejects_boolean_total_count(self, graphql_call) -> None:
        graphql_call.return_value = {
            "user": {
                "projectV2": {
                    "items": {
                        "totalCount": True,
                        "pageInfo": {"hasNextPage": False, "endCursor": None},
                        "nodes": [],
                    }
                }
            }
        }

        with self.assertRaisesRegex(RuntimeError, "totalCount is invalid"):
            _project_items(self.config)


class CliTests(unittest.TestCase):
    def _report(self, *, remaining=(), duplicates=()):
        return ReconciliationReport(
            owner="kmosoti",
            repository="kmosoti/gordian",
            project_number=9,
            open_issue_count=1,
            project_issue_count_before=1,
            missing_before=tuple(remaining),
            duplicate_urls_before=tuple(duplicates),
            added_urls=(),
            failed_urls=(),
            remaining_after=tuple(remaining),
            dry_run=True,
            generated_at=STAMP.generated_at,
            source_change_id=STAMP.source_change_id,
            source_commit_id=STAMP.source_commit_id,
            tool_versions=STAMP.tool_versions,
        )

    def test_reconcile_check_exact_closure_form_succeeds_only_on_convergence(self) -> None:
        with (
            patch("gordian_orchestration.github_project.preflight"),
            patch(
                "gordian_orchestration.github_project.reconcile", return_value=self._report()
            ),
        ):
            self.assertEqual(main(["reconcile", "--check"]), 0)
        with (
            patch("gordian_orchestration.github_project.preflight"),
            patch(
                "gordian_orchestration.github_project.reconcile",
                return_value=self._report(remaining=("https://example.invalid/issues/1",)),
            ),
        ):
            self.assertEqual(main(["reconcile", "--check"]), 1)

    def test_configuration_failure_returns_78(self) -> None:
        with patch(
            "gordian_orchestration.github_project.preflight",
            side_effect=GitHubConfigurationError("missing project scope"),
        ):
            self.assertEqual(main(["reconcile", "--check"]), 78)


if __name__ == "__main__":
    unittest.main()
