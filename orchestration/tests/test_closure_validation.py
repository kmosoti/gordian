"""Adversarial tests for closure evidence and timestamp validation."""

from __future__ import annotations

import hashlib
import json
import unittest
from pathlib import Path

from gordian_orchestration.closure_validation import (
    SourceBinding,
    attempt_problems,
    closure_problems,
    evidence_binding_problems,
    evidence_header,
    executable_command_problem,
    parse_rfc3339,
    schema_problems,
    validate,
)

EXACT = "7b11ee451248e8ee0d3d0cbb8188e56984ec8464"
LOGICAL = "orunzpqurynwmyvmzyqlqpzwmxwvpzmz"
COMMAND = "cargo test --locked"
BOUND_ARTIFACT = evidence_header(EXACT, COMMAND) + b"canonical verifier output\nexit_code=0\n"

VERIFIER_SCHEMA = {
    "type": "object",
    "properties": {
        "atom_id": {"type": "string"},
        "verifiers": {
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "verifier_id": {"type": "string"},
                    "command": {"type": "string"},
                    "exit_code": {"type": "integer"},
                    "artifact_path": {"type": "string"},
                    "artifact_sha256": {"type": "string"},
                },
            },
        },
    },
}


class ClosureEvidenceTests(unittest.TestCase):
    def _payload(self, **changes):
        artifact = BOUND_ARTIFACT
        verifier = {
            "verifier_id": "check",
            "command": COMMAND,
            "exit_code": 0,
            "artifact_path": "artifacts/atoms/42/verifiers/check.log",
            "artifact_sha256": hashlib.sha256(artifact).hexdigest(),
        }
        verifier.update(changes.pop("verifier", {}))
        payload = {"atom_id": "42", "exact_state_id": EXACT, "verifiers": [verifier]}
        payload.update(changes)
        return payload, artifact

    def _problems(self, payload, artifact=BOUND_ARTIFACT):
        reads = []

        def reader(relative):
            reads.append(relative)
            if relative == "artifacts/atoms/42/verifiers/check.log":
                return artifact
            return None

        problems = closure_problems(
            payload,
            VERIFIER_SCHEMA,
            label="closure.json",
            expected_atom="42",
            record_path="artifacts/atoms/42/closure.json",
            read_artifact=reader,
        )
        return problems, reads

    def test_canonical_zero_exit_log_with_exact_digest_passes(self):
        payload, _ = self._payload()
        self.assertEqual(self._problems(payload), ([], ["artifacts/atoms/42/verifiers/check.log"]))

    def test_empty_command_is_rejected_by_the_shared_cross_field_validator(self):
        payload, _ = self._payload(verifier={"command": ""})
        problems, _ = self._problems(payload)
        self.assertTrue(
            any("command must be a non-empty string" in problem for problem in problems)
        )

    def test_empty_or_unsafe_verifier_ids_never_select_an_artifact(self):
        for verifier_id in ("", "../check", "check/log", "check log"):
            with self.subTest(verifier_id=verifier_id):
                payload, _ = self._payload(verifier={"verifier_id": verifier_id})
                problems, reads = self._problems(payload)
                self.assertTrue(problems)
                self.assertEqual(reads, [])

    def test_nonzero_exit_code_is_not_valid_evidence(self):
        payload, _ = self._payload(verifier={"exit_code": 1})
        problems, reads = self._problems(payload)
        self.assertTrue(any("exit_code must be exactly 0" in problem for problem in problems))
        self.assertEqual(reads, [])

    def test_empty_missing_and_invalid_digests_fail(self):
        for digest in ("", "not-a-digest", "A" * 64):
            with self.subTest(digest=digest):
                payload, _ = self._payload(verifier={"artifact_sha256": digest})
                problems, _ = self._problems(payload)
                self.assertTrue(any("artifact_sha256" in problem for problem in problems))

    def test_arbitrary_record_relative_and_self_paths_fail_without_a_read(self):
        for path in ("", "README.md", "verifiers/check.log", "closure.json"):
            with self.subTest(path=path):
                payload, _ = self._payload(verifier={"artifact_path": path})
                problems, reads = self._problems(payload)
                self.assertTrue(any("artifact_path" in problem for problem in problems))
                self.assertEqual(reads, [])

    def test_duplicate_verifier_ids_fail(self):
        payload, artifact = self._payload()
        duplicate = dict(payload["verifiers"][0])
        payload["verifiers"].append(duplicate)
        problems, _ = self._problems(payload, artifact)
        self.assertTrue(any("duplicate verifier_id 'check'" in problem for problem in problems))

    def test_an_artifact_that_merely_exists_is_not_evidence(self):
        # Atom #70's shape: the digest matches, the bytes name no state and no command.
        unbound = b"verifier:formal OK\n   Compiling proc-macro2 v1.0.107\n"
        payload, _ = self._payload(
            verifier={"artifact_sha256": hashlib.sha256(unbound).hexdigest()}
        )
        problems, _ = self._problems(payload, artifact=unbound)
        self.assertEqual(len(problems), 1)
        self.assertIn(f"artifact line 1 must be subject_exact_state_id={EXACT}", problems[0])

    def test_an_artifact_bound_to_another_state_is_not_evidence(self):
        foreign = evidence_header("f" * 40, COMMAND) + b"canonical verifier output\n"
        payload, _ = self._payload(
            verifier={"artifact_sha256": hashlib.sha256(foreign).hexdigest()}
        )
        problems, _ = self._problems(payload, artifact=foreign)
        self.assertEqual(len(problems), 1)
        self.assertIn("artifact line 1 must be", problems[0])

    def test_an_artifact_bound_to_another_command_is_not_evidence(self):
        foreign = evidence_header(EXACT, COMMAND + "; rm -rf x") + b"canonical verifier output\n"
        payload, _ = self._payload(
            verifier={"artifact_sha256": hashlib.sha256(foreign).hexdigest()}
        )
        problems, _ = self._problems(payload, artifact=foreign)
        self.assertEqual(len(problems), 1)
        self.assertIn(f"artifact line 2 must be command={COMMAND}; found", problems[0])

    def test_header_prefix_is_not_enough(self):
        # The subject line must be exactly the id: a longer id or a missing newline fails.
        for artifact in (
            f"subject_exact_state_id={EXACT}0\ncommand={COMMAND}\n".encode(),
            f"subject_exact_state_id={EXACT}".encode(),
            f"subject_exact_state_id={EXACT}\ncommand={COMMAND}x\n".encode(),
            f"subject_exact_state_id={EXACT}\ncommand={COMMAND}".encode(),
        ):
            with self.subTest(artifact=artifact):
                self.assertTrue(evidence_binding_problems(artifact, EXACT, COMMAND))
        self.assertEqual(evidence_binding_problems(BOUND_ARTIFACT, EXACT, COMMAND), [])

    def test_unbindable_exact_state_cannot_be_evidenced(self):
        self.assertTrue(evidence_binding_problems(BOUND_ARTIFACT, "abc", COMMAND))
        self.assertTrue(evidence_binding_problems(BOUND_ARTIFACT, None, COMMAND))

    def test_one_artifact_cannot_witness_two_verifiers(self):
        payload, artifact = self._payload()
        second = dict(payload["verifiers"][0])
        second["verifier_id"] = "other"
        second["artifact_path"] = "artifacts/atoms/42/verifiers/other.log"
        payload["verifiers"].append(second)
        problems, _ = self._problems(payload, artifact)
        self.assertTrue(
            any("artifact_sha256 duplicates verifiers[0]" in problem for problem in problems),
            problems,
        )

    def test_a_multi_line_command_cannot_be_bound(self):
        for command in (COMMAND + "\nfalse", COMMAND + "\r\nfalse", COMMAND + "\n"):
            with self.subTest(command=command):
                payload, _ = self._payload(verifier={"command": command})
                problems, reads = self._problems(payload)
                self.assertTrue(
                    any("must not contain line breaks" in problem for problem in problems)
                )
                self.assertEqual(reads, [])

    def test_prose_is_not_a_command(self):
        # Atom #1's record: a description of what was verified, bound as if it were the
        # command that verified it.  Nothing a shell could run, so no artifact is read.
        for command in (
            "contract positive, injected-negative, and manifest-write-failure paths",
            "fresh exact-state workspace; pinned Python package setup",
            "true",
            ":",
            "echo verified",
            "FOO=bar",
            "/usr/bin/cargo test",
            "../scripts/check.sh",
            "scripts//check.sh",
            "scripts/./check.sh",
        ):
            with self.subTest(command=command):
                payload, _ = self._payload(verifier={"command": command})
                problems, reads = self._problems(payload)
                self.assertTrue(
                    any("command must start with" in problem or "only assignments" in problem
                        for problem in problems),
                    problems,
                )
                self.assertEqual(reads, [])

    def test_pinned_tools_shell_words_and_repository_scripts_are_commands(self):
        scripts = {"scripts/verify-local.sh": b"#!/usr/bin/env bash\n"}
        for command in (
            "cargo test --locked",
            "PYTHONPATH=orchestration/src python3.14 -m gordian_orchestration.github_project",
            "for s in scripts/check-*.sh; do bash \"$s\"; done",
            "cd formal && lake build",
            "! cargo test",
            "[ -f manifest.json ]",
            "scripts/verify-local.sh all",
            "./scripts/verify-local.sh all",
            "GORDIAN_LOG_ROOT=/tmp/logs A=1 bash scripts/verify-local.sh all",
        ):
            with self.subTest(command=command):
                self.assertIsNone(executable_command_problem(command, scripts.get))
        # Existence is only checked when a reader is given: schema-only callers still
        # reject prose without a repository to look in.
        self.assertIsNone(executable_command_problem("scripts/absent.sh", None))
        self.assertIn(
            "does not exist at the subject state",
            executable_command_problem("scripts/absent.sh", scripts.get),
        )

    def test_a_script_must_exist_at_the_subject_state_not_the_bookkeeping_state(self):
        # The bookkeeping change that carries the record may add a script; the verifier
        # ran at the subject state, where it has to exist.
        script = "scripts/check.sh"
        artifact = evidence_header(EXACT, script) + b"canonical verifier output\nexit_code=0\n"
        verifier = {
            "verifier_id": "check",
            "command": script,
            "exit_code": 0,
            "artifact_path": "artifacts/atoms/42/verifiers/check.log",
            "artifact_sha256": hashlib.sha256(artifact).hexdigest(),
            "subject_exact_state_id": EXACT,
        }
        payload = {
            "atom_id": "42",
            "exact_state_id": EXACT,
            "logical_change_id": LOGICAL,
            "verifiers": [verifier],
        }
        record_bytes = json.dumps(payload).encode()
        bookkeeping = {
            "artifacts/atoms/42/closure.json": record_bytes,
            "artifacts/atoms/42/verifiers/check.log": artifact,
            script: b"#!/usr/bin/env bash\n",
        }
        subject_with, subject_without = {script: b"#!/usr/bin/env bash\n"}, {}

        def problems_with(subject):
            return closure_problems(
                payload,
                VERIFIER_SCHEMA,
                label="closure.json",
                expected_atom="42",
                record_path="artifacts/atoms/42/closure.json",
                read_artifact=bookkeeping.get,
                resolve_source=lambda *_: SourceBinding(
                    EXACT, LOGICAL, bookkeeping.get, True, True, subject.get
                ),
                source_binding_required=True,
            )

        self.assertEqual(problems_with(subject_with), [])
        self.assertEqual(
            problems_with(subject_without),
            [
                "closure.json.verifiers[0]: command starts with 'scripts/check.sh', which "
                "does not exist at the subject state"
            ],
        )

    def test_missing_or_mismatched_artifact_fails(self):
        payload, artifact = self._payload()
        problems, _ = self._problems(payload, artifact=b"different\n")
        self.assertTrue(any("hashes to" in problem for problem in problems))

        payload, _ = self._payload()
        payload["verifiers"][0]["artifact_sha256"] = hashlib.sha256(b"missing").hexdigest()
        problems, _ = self._problems(payload, artifact=None)
        self.assertTrue(any("does not exist" in problem for problem in problems))

    def test_checked_in_schema_requires_nonempty_command(self):
        schema_path = Path(__file__).parents[2] / "artifacts/schema/closure-record.schema.json"
        schema = json.loads(schema_path.read_text(encoding="utf-8"))
        command_schema = schema["properties"]["verifiers"]["items"]["properties"]["command"]
        problems = []
        validate("", command_schema, "command", problems)
        self.assertTrue(any("minLength 1" in problem for problem in problems))

    def test_checked_in_schema_rejects_unstructured_benchmark_metadata(self):
        schema_path = Path(__file__).parents[2] / "artifacts/schema/closure-record.schema.json"
        schema = json.loads(schema_path.read_text(encoding="utf-8"))
        benchmark_schema = schema["properties"]["benchmarks"]["items"]
        problems = []
        validate(
            {
                "experiment_id": "experiment:one",
                "run_id": "run-1",
                "artifact_sha256": "0" * 64,
                "untrusted": True,
            },
            benchmark_schema,
            "benchmark",
            problems,
        )
        self.assertTrue(any("unexpected property 'untrusted'" in problem for problem in problems))


class TimestampTests(unittest.TestCase):
    def test_rfc3339_rejects_impossible_calendar_values_and_missing_timezone(self):
        self.assertIsNone(parse_rfc3339("2026-02-30T00:00:00Z"))
        self.assertIsNone(parse_rfc3339("2026-08-31T00:00:00"))
        self.assertIsNotNone(parse_rfc3339("2026-08-31T00:00:00+05:30"))

    def test_attempt_finished_time_is_compared_by_instant_not_string_order(self):
        payload = {
            "attempt_id": "20260831T210000Z-run",
            "started_at": "2026-08-31T23:00:00+02:00",
            "finished_at": "2026-08-31T21:30:00Z",
            "outcome": "verifier_failed",
        }
        self.assertEqual(
            attempt_problems(payload, {"type": "object"}, label="attempt.json"), []
        )

    def test_attempt_with_impossible_timestamp_fails_schema_validation(self):
        schema = {
            "type": "object",
            "properties": {"started_at": {"type": "string", "format": "date-time"}},
        }
        problems = schema_problems(
            {"started_at": "2026-02-30T00:00:00Z"}, schema, "attempt.json"
        )
        self.assertTrue(any("RFC 3339 date-time" in problem for problem in problems))


if __name__ == "__main__":
    unittest.main()
