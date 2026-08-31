"""Adversarial tests for closure evidence and timestamp validation."""

from __future__ import annotations

import hashlib
import json
import unittest
from pathlib import Path

from gordian_orchestration.closure_validation import (
    attempt_problems,
    closure_problems,
    parse_rfc3339,
    schema_problems,
    validate,
)

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
        artifact = b"canonical verifier output\n"
        verifier = {
            "verifier_id": "check",
            "command": "true",
            "exit_code": 0,
            "artifact_path": "artifacts/atoms/42/verifiers/check.log",
            "artifact_sha256": hashlib.sha256(artifact).hexdigest(),
        }
        verifier.update(changes.pop("verifier", {}))
        payload = {"atom_id": "42", "verifiers": [verifier]}
        payload.update(changes)
        return payload, artifact

    def _problems(self, payload, artifact=b"canonical verifier output\n"):
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
