"""Unit tests for the GitHub CLI wrapper, against a mocked subprocess only."""

from __future__ import annotations

import subprocess
import unittest
from unittest.mock import patch

from gordian_orchestration import gh


def _completed(returncode: int, stdout: str = "", stderr: str = ""):
    return subprocess.CompletedProcess(
        args=["gh"], returncode=returncode, stdout=stdout, stderr=stderr
    )


class RunGhTests(unittest.TestCase):
    def test_arguments_are_passed_without_a_shell(self) -> None:
        with patch("subprocess.run", return_value=_completed(0, "ok")) as runner:
            self.assertEqual(gh.run_gh(["issue", "list"]), "ok")
        argv = runner.call_args.args[0]
        self.assertEqual(argv, ["gh", "issue", "list"])
        self.assertNotIn("shell", runner.call_args.kwargs)

    def test_failure_preserves_stderr_and_names_the_command(self) -> None:
        with (
            patch("subprocess.run", return_value=_completed(1, "", "HTTP 403")),
            self.assertRaises(RuntimeError) as raised,
        ):
            gh.run_gh(["project", "item-add", "9", "--owner", "kmosoti"])
        message = str(raised.exception)
        self.assertIn("gh project item-add 9", message)
        self.assertIn("HTTP 403", message)

    def test_failure_without_stderr_still_reports(self) -> None:
        with (
            patch("subprocess.run", return_value=_completed(1)),
            self.assertRaises(RuntimeError) as raised,
        ):
            gh.run_gh(["auth", "status"])
        self.assertIn("GitHub CLI command failed", str(raised.exception))

    def test_json_decoding_failure_is_explicit(self) -> None:
        with (
            patch("subprocess.run", return_value=_completed(0, "not json")),
            self.assertRaises(RuntimeError) as raised,
        ):
            gh.run_gh_json(["issue", "list"])
        self.assertIn("invalid JSON", str(raised.exception))

    def test_the_auth_hint_names_the_non_interactive_path(self) -> None:
        self.assertIn("GH_TOKEN", gh.GH_AUTH_HINT)
        self.assertIn("project", gh.GH_AUTH_HINT)
        self.assertIn("repo", gh.GH_AUTH_HINT)


class GraphqlTests(unittest.TestCase):
    def test_variables_are_typed_by_flag(self) -> None:
        payload = '{"data": {"repository": {"name": "gordian"}}}'
        with patch("subprocess.run", return_value=_completed(0, payload)) as runner:
            data = gh.graphql("query{x}", {"owner": "kmosoti", "number": 9})
        self.assertEqual(data, {"repository": {"name": "gordian"}})
        argv = runner.call_args.args[0]
        self.assertIn("-f", argv)
        self.assertEqual(argv[argv.index("owner=kmosoti") - 1], "-f")
        self.assertEqual(argv[argv.index("number=9") - 1], "-F")

    def test_graphql_errors_are_raised_not_returned(self) -> None:
        payload = '{"data": null, "errors": [{"message": "Field does not exist"}]}'
        with (
            patch("subprocess.run", return_value=_completed(0, payload)),
            self.assertRaises(RuntimeError) as raised,
        ):
            gh.graphql("query{x}")
        self.assertIn("Field does not exist", str(raised.exception))

    def test_missing_data_object_is_an_error(self) -> None:
        with (
            patch("subprocess.run", return_value=_completed(0, "{}")),
            self.assertRaises(RuntimeError),
        ):
            gh.graphql("query{x}")


if __name__ == "__main__":
    unittest.main()
