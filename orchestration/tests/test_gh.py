"""Unit tests for the GitHub CLI wrapper, against a mocked subprocess only."""

from __future__ import annotations

import os
import subprocess
import unittest
from unittest.mock import patch

from gordian_orchestration import gh


def _completed(returncode: int, stdout: str = "", stderr: str = ""):
    return subprocess.CompletedProcess(
        args=["gh"], returncode=returncode, stdout=stdout, stderr=stderr
    )


TOKEN = "test-token"


class RunGhTests(unittest.TestCase):
    def test_arguments_are_passed_without_a_shell(self) -> None:
        with (
            patch.dict(
                "os.environ",
                {
                    "GORDIAN_GH_TOKEN": TOKEN,
                    "GH_TOKEN": "ambient-token",
                    "GH_CONFIG_DIR": "/tmp/ambient-gh-config",
                },
                clear=True,
            ),
            patch("subprocess.run", return_value=_completed(0, "ok")) as runner,
        ):
            self.assertEqual(gh.run_gh(["issue", "list"]), "ok")
            self.assertEqual(os.environ["GH_TOKEN"], TOKEN)
        argv = runner.call_args.args[0]
        self.assertEqual(argv, ["gh", "issue", "list"])
        self.assertNotIn("shell", runner.call_args.kwargs)
        self.assertEqual(runner.call_args.kwargs["env"]["GH_TOKEN"], TOKEN)

    def test_failure_preserves_stderr_and_names_the_command(self) -> None:
        with (
            patch.dict("os.environ", {"GORDIAN_GH_TOKEN": TOKEN}, clear=True),
            patch("subprocess.run", return_value=_completed(1, "", "HTTP 403")),
            self.assertRaises(RuntimeError) as raised,
        ):
            gh.run_gh(["project", "item-add", "9", "--owner", "kmosoti"])
        message = str(raised.exception)
        self.assertIn("gh project item-add 9", message)
        self.assertIn("HTTP 403", message)

    def test_failure_without_stderr_still_reports(self) -> None:
        with (
            patch.dict("os.environ", {"GORDIAN_GH_TOKEN": TOKEN}, clear=True),
            patch("subprocess.run", return_value=_completed(1)),
            self.assertRaises(RuntimeError) as raised,
        ):
            gh.run_gh(["auth", "status"])
        self.assertIn("GitHub CLI command failed", str(raised.exception))

    def test_failure_redacts_explicit_token_from_diagnostic(self) -> None:
        secret = "diagnostic-secret"
        with (
            patch.dict("os.environ", {"GORDIAN_GH_TOKEN": secret}, clear=True),
            patch(
                "subprocess.run",
                return_value=_completed(1, stderr=f"token {secret} was rejected"),
            ),
            self.assertRaises(RuntimeError) as raised,
        ):
            gh.run_gh(["auth", "status"])
        message = str(raised.exception)
        self.assertNotIn(secret, message)
        self.assertIn("token <redacted> was rejected", message)

    def test_json_decoding_failure_is_explicit(self) -> None:
        with (
            patch.dict("os.environ", {"GORDIAN_GH_TOKEN": TOKEN}, clear=True),
            patch("subprocess.run", return_value=_completed(0, "not json")),
            self.assertRaises(RuntimeError) as raised,
        ):
            gh.run_gh_json(["issue", "list"])
        self.assertIn("invalid JSON", str(raised.exception))


class StructuredApiResponseTests(unittest.TestCase):
    def test_api_response_preserves_status_headers_and_body(self) -> None:
        output = (
            "HTTP/2.0 200 OK\r\n"
            "Date: Mon, 31 Aug 2026 05:00:00 GMT\r\n"
            "X-Test: value\r\n"
            "\r\n"
            '{"ref":"refs/heads/gordian-claim-log"}'
        )
        with (
            patch.dict("os.environ", {"GORDIAN_GH_TOKEN": TOKEN}, clear=True),
            patch("subprocess.run", return_value=_completed(0, output)) as runner,
        ):
            response = gh.run_gh_response(
                ["api", "repos/kmosoti/gordian/git/ref/heads/gordian-claim-log"]
            )
        self.assertEqual(response.status, 200)
        self.assertEqual(response.headers["date"], "Mon, 31 Aug 2026 05:00:00 GMT")
        self.assertEqual(response.body, '{"ref":"refs/heads/gordian-claim-log"}')
        self.assertEqual(runner.call_args.args[0][-1], "--include")

    def test_allowed_conflict_status_is_returned_instead_of_raised(self) -> None:
        output = (
            "HTTP/2.0 422 Unprocessable Entity\r\n"
            "Date: Mon, 31 Aug 2026 05:00:00 GMT\r\n"
            "\r\n"
            '{"message":"Reference update failed"}'
        )
        with (
            patch.dict("os.environ", {"GORDIAN_GH_TOKEN": TOKEN}, clear=True),
            patch("subprocess.run", return_value=_completed(1, output)),
        ):
            response = gh.run_gh_response(
                ["api", "--method", "PATCH", "repos/x/y/git/refs/z"],
                allowed_statuses={409, 422},
            )
        self.assertEqual(response.status, 422)

    def test_explicit_status_contract_rejects_unexpected_success(self) -> None:
        output = (
            "HTTP/2.0 204 No Content\r\n"
            "Date: Mon, 31 Aug 2026 05:00:00 GMT\r\n"
            "\r\n"
        )
        with (
            patch.dict("os.environ", {"GORDIAN_GH_TOKEN": TOKEN}, clear=True),
            patch("subprocess.run", return_value=_completed(0, output)),
            self.assertRaises(gh.GitHubApiError) as raised,
        ):
            gh.run_gh_response(
                ["api", "repos/x/y/git/commits"],
                allowed_statuses={201},
            )
        self.assertEqual(raised.exception.status, 204)

    def test_unallowed_status_exposes_status_without_echoing_token(self) -> None:
        output = (
            "HTTP/2.0 403 Forbidden\r\n"
            "Date: Mon, 31 Aug 2026 05:00:00 GMT\r\n"
            "\r\n"
            '{"message":"forbidden"}'
        )
        with (
            patch.dict("os.environ", {"GORDIAN_GH_TOKEN": TOKEN}, clear=True),
            patch("subprocess.run", return_value=_completed(1, output)),
            self.assertRaises(gh.GitHubApiError) as raised,
        ):
            gh.run_gh_response(["api", "repos/x/y"])
        self.assertEqual(raised.exception.status, 403)

    def test_the_auth_hint_names_the_non_interactive_path(self) -> None:
        self.assertIn("GH_TOKEN", gh.GH_AUTH_HINT)
        self.assertIn("Project 9", gh.GH_AUTH_HINT)
        self.assertIn("repository", gh.GH_AUTH_HINT)
        self.assertNotIn("classic", gh.GH_AUTH_HINT)
        self.assertNotIn("workflow", gh.GH_AUTH_HINT)


class GraphqlTests(unittest.TestCase):
    def test_json_response_preserves_server_metadata(self) -> None:
        response = gh.GitHubApiResponse(
            200,
            {"date": "Mon, 31 Aug 2026 05:00:00 GMT"},
            '{"comments": []}',
        )
        with patch.object(gh, "run_gh_response", return_value=response) as runner:
            payload, observed = gh.run_gh_json_response(
                ["api", "repos/kmosoti/gordian/issues/70/comments"],
                allowed_statuses={200},
            )
        self.assertEqual(payload, {"comments": []})
        self.assertIs(observed, response)
        runner.assert_called_once()

    def test_json_response_rejects_invalid_json_without_losing_boundary_context(self) -> None:
        response = gh.GitHubApiResponse(200, {"date": "today"}, "not-json")
        with (
            patch.object(gh, "run_gh_response", return_value=response),
            self.assertRaisesRegex(RuntimeError, "invalid JSON"),
        ):
            gh.run_gh_json_response(["api", "repos/x/y/issues/70/comments"])

    def test_variables_are_typed_by_flag(self) -> None:
        payload = '{"data": {"repository": {"name": "gordian"}}}'
        with (
            patch.dict("os.environ", {"GORDIAN_GH_TOKEN": TOKEN}, clear=True),
            patch("subprocess.run", return_value=_completed(0, payload)) as runner,
        ):
            data = gh.graphql("query{x}", {"owner": "kmosoti", "number": 9})
        self.assertEqual(data, {"repository": {"name": "gordian"}})
        argv = runner.call_args.args[0]
        self.assertIn("-f", argv)
        self.assertEqual(argv[argv.index("owner=kmosoti") - 1], "-f")
        self.assertEqual(argv[argv.index("number=9") - 1], "-F")

    def test_graphql_errors_are_raised_not_returned(self) -> None:
        payload = '{"data": null, "errors": [{"message": "Field does not exist"}]}'
        with (
            patch.dict("os.environ", {"GORDIAN_GH_TOKEN": TOKEN}, clear=True),
            patch("subprocess.run", return_value=_completed(0, payload)),
            self.assertRaises(RuntimeError) as raised,
        ):
            gh.graphql("query{x}")
        self.assertIn("Field does not exist", str(raised.exception))

    def test_missing_data_object_is_an_error(self) -> None:
        with (
            patch.dict("os.environ", {"GORDIAN_GH_TOKEN": TOKEN}, clear=True),
            patch("subprocess.run", return_value=_completed(0, "{}")),
            self.assertRaises(RuntimeError),
        ):
            gh.graphql("query{x}")


class PreflightTests(unittest.TestCase):
    def test_stored_gh_credential_is_rejected_without_explicit_token(self) -> None:
        with (
            patch.dict(
                "os.environ",
                {"GH_TOKEN": "stored", "GH_CONFIG_DIR": "/tmp/ambient-gh-config"},
                clear=True,
            ),
            patch("gordian_orchestration.gh.run_gh") as runner,
            self.assertRaisesRegex(
                gh.GitHubConfigurationError,
                "GORDIAN_GH_TOKEN must be set to a non-empty token",
            ),
        ):
            gh.preflight()
        runner.assert_not_called()

    def test_every_gh_subprocess_gets_explicit_token_override(self) -> None:
        secret = "override-secret"
        responses = [
            _completed(0, "authenticated"),
            _completed(0, '{"login":"agent"}'),
            _completed(0, '{"permissions":{"push":true}}'),
            _completed(
                0,
                '{"data":{"user":{"projectV2":'
                '{"id":"PVT_kw","viewerCanUpdate":true}}}}',
            ),
        ]

        with (
            patch.dict(
                "os.environ",
                {
                    "GORDIAN_GH_TOKEN": secret,
                    "GH_TOKEN": "ambient-token",
                    "GH_CONFIG_DIR": "/tmp/ambient-gh-config",
                },
                clear=True,
            ),
            patch("subprocess.run", side_effect=responses) as runner,
        ):
            report = gh.preflight()

        self.assertEqual(report.login, "agent")
        self.assertEqual(report.credential_source, "GORDIAN_GH_TOKEN")
        self.assertEqual(runner.call_count, 4)
        for call in runner.call_args_list:
            self.assertEqual(call.kwargs["env"]["GH_TOKEN"], secret)
        self.assertNotIn(secret, repr(report))

    def test_missing_or_whitespace_token_fails_before_any_gh_call(self) -> None:
        for value in ("", "   ", "\t"):
            with self.subTest(value=repr(value)):
                with (
                    patch.dict(
                        "os.environ",
                        {"GORDIAN_GH_TOKEN": value, "GH_TOKEN": "ambient-token"},
                        clear=True,
                    ),
                    patch("subprocess.run") as runner,
                    self.assertRaisesRegex(
                        gh.GitHubConfigurationError,
                        "GORDIAN_GH_TOKEN must be set to a non-empty token",
                    ),
                ):
                    gh.preflight()
                runner.assert_not_called()

    def test_explicit_token_replaces_ambient_gh_token_without_printing_it(self) -> None:
        secret = "override-secret"
        observed_token = None
        with (
            patch.dict(
                "os.environ",
                {"GH_TOKEN": "stored", "GORDIAN_GH_TOKEN": secret},
                clear=True,
            ),
            patch(
                "gordian_orchestration.gh.run_gh",
                side_effect=[RuntimeError(f"credential {secret} rejected")],
            ),
        ):
            with self.assertRaisesRegex(gh.GitHubConfigurationError, "redacted") as raised:
                gh.preflight()
            observed_token = os.environ["GH_TOKEN"]
        self.assertEqual(observed_token, secret)
        self.assertNotIn(secret, str(raised.exception))

    def test_explicit_token_is_reported_without_secret(self) -> None:
        secret = "override-secret"
        responses = ["authenticated", '{"login":"agent"}', '{"permissions":{"push":true}}']
        with (
            patch.dict("os.environ", {"GORDIAN_GH_TOKEN": secret}, clear=True),
            patch("gordian_orchestration.gh.run_gh", side_effect=responses),
            patch(
                "gordian_orchestration.gh.graphql",
                return_value={
                    "user": {
                        "projectV2": {"id": "PVT_kw", "viewerCanUpdate": True}
                    }
                },
            ),
        ):
            report = gh.preflight()
        self.assertEqual(report.credential_source, "GORDIAN_GH_TOKEN")
        self.assertNotIn(secret, repr(report))

    def test_repository_write_refusal_is_a_configuration_error(self) -> None:
        responses = ["authenticated", '{"login":"agent"}', '{"permissions":{"push":false}}']
        with (
            patch.dict("os.environ", {"GORDIAN_GH_TOKEN": "stored"}, clear=True),
            patch("gordian_orchestration.gh.run_gh", side_effect=responses),
            patch("gordian_orchestration.gh.graphql") as project_probe,
            self.assertRaisesRegex(gh.GitHubConfigurationError, "repository write permission"),
        ):
            gh.preflight()
        project_probe.assert_not_called()

    def test_project_write_refusal_is_a_configuration_error(self) -> None:
        responses = ["authenticated", '{"login":"agent"}', '{"permissions":{"push":true}}']
        with (
            patch.dict("os.environ", {"GORDIAN_GH_TOKEN": "stored"}, clear=True),
            patch("gordian_orchestration.gh.run_gh", side_effect=responses),
            patch(
                "gordian_orchestration.gh.graphql",
                return_value={
                    "user": {
                        "projectV2": {"id": "PVT_kw", "viewerCanUpdate": False}
                    }
                },
            ),
            self.assertRaisesRegex(gh.GitHubConfigurationError, "Project write permission"),
        ):
            gh.preflight()

    def test_project_graphql_refusal_is_a_configuration_error(self) -> None:
        responses = ["authenticated", '{"login":"agent"}', '{"permissions":{"push":true}}']
        with (
            patch.dict("os.environ", {"GORDIAN_GH_TOKEN": "stored"}, clear=True),
            patch("gordian_orchestration.gh.run_gh", side_effect=responses),
            patch(
                "gordian_orchestration.gh.graphql",
                side_effect=RuntimeError("HTTP 403: Resource not accessible"),
            ),
            self.assertRaisesRegex(gh.GitHubConfigurationError, "Project .*GraphQL API"),
        ):
            gh.preflight()

    def test_no_configured_credential_fails_at_authentication_probe(self) -> None:
        with (
            patch.dict("os.environ", {"GH_TOKEN": "ambient-token"}, clear=True),
            patch("gordian_orchestration.gh.run_gh") as runner,
            self.assertRaisesRegex(
                gh.GitHubConfigurationError,
                "GORDIAN_GH_TOKEN must be set to a non-empty token",
            ),
        ):
            gh.preflight()
        runner.assert_not_called()

    def test_probe_failure_is_reclassified_as_configuration(self) -> None:
        with (
            patch.dict("os.environ", {"GORDIAN_GH_TOKEN": "secret"}, clear=True),
            patch("gordian_orchestration.gh.run_gh", side_effect=RuntimeError("HTTP 403")),
            self.assertRaisesRegex(gh.GitHubConfigurationError, "HTTP 403"),
        ):
            gh.preflight()

    def test_preflight_never_invokes_interactive_authentication(self) -> None:
        responses = ["authenticated", '{"login":"agent"}', '{"permissions":{"push":true}}']
        with (
            patch.dict("os.environ", {"GORDIAN_GH_TOKEN": "stored"}, clear=True),
            patch("gordian_orchestration.gh.run_gh", side_effect=responses) as runner,
            patch(
                "gordian_orchestration.gh.graphql",
                return_value={
                    "user": {
                        "projectV2": {"id": "PVT_kw", "viewerCanUpdate": True}
                    }
                },
            ),
        ):
            gh.preflight()
        arguments = [call.args[0] for call in runner.call_args_list]
        self.assertFalse(
            any(
                command[:2] in (["auth", "login"], ["auth", "refresh"])
                for command in arguments
            )
        )


if __name__ == "__main__":
    unittest.main()
