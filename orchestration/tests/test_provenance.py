"""Unit tests for the provenance stamp carried by every orchestration report."""

from __future__ import annotations

import unittest
from unittest.mock import patch

from gordian_orchestration import provenance


class ProvenanceTests(unittest.TestCase):
    def test_generated_at_is_rfc_3339_utc(self) -> None:
        stamp = provenance.generated_at()
        self.assertTrue(stamp.endswith("Z"), stamp)
        self.assertEqual(stamp[4], "-")
        self.assertEqual(stamp[10], "T")

    def test_source_identity_reads_the_working_copy(self) -> None:
        with patch(
            "gordian_orchestration.provenance._capture",
            return_value="qxpvzz 4f2a1b0c9d8e",
        ):
            change_id, commit_id = provenance.source_identity()
        self.assertEqual(change_id, "qxpvzz")
        self.assertEqual(commit_id, "4f2a1b0c9d8e")

    def test_every_key_stays_non_empty_when_no_tool_is_present(self) -> None:
        with patch("gordian_orchestration.provenance._capture", return_value="unknown"):
            stamp = provenance.collect()
        payload = stamp.as_json_object()
        for key in ("generated_at", "source_change_id", "source_commit_id", "tool_versions"):
            self.assertIn(key, payload)
            self.assertTrue(payload[key], f"{key} is empty")
        self.assertEqual(payload["source_change_id"], "unknown")
        self.assertEqual(payload["tool_versions"]["gh"], "unknown")

    def test_capture_degrades_rather_than_raising(self) -> None:
        with patch("subprocess.run", side_effect=FileNotFoundError):
            self.assertEqual(provenance._capture(["gordian-absent-tool"]), "unknown")

    def test_capture_ignores_a_failing_probe(self) -> None:
        import subprocess

        completed = subprocess.CompletedProcess(args=[], returncode=1, stdout="x", stderr="")
        with patch("subprocess.run", return_value=completed):
            self.assertEqual(provenance._capture(["jj", "--version"]), "unknown")


if __name__ == "__main__":
    unittest.main()
