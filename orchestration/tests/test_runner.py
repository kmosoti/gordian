"""Unit tests for the process runner.

`run` is the only place orchestration shells out for experiment work, so its failure
behaviour is load-bearing: a silently swallowed non-zero exit would turn a failed trial
into a missing one, which is exactly the cherry-picking the methodology forbids.
"""

from __future__ import annotations

import subprocess
import sys
import unittest
from pathlib import Path

from gordian_orchestration.runner import CommandResult, run


class RunnerTests(unittest.TestCase):
    def test_success_captures_streams_and_argv(self) -> None:
        result = run(
            [sys.executable, "-c", "import sys; print('out'); print('err', file=sys.stderr)"]
        )
        self.assertIsInstance(result, CommandResult)
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout.strip(), "out")
        self.assertEqual(result.stderr.strip(), "err")
        self.assertEqual(result.argv[0], sys.executable)

    def test_non_zero_raises_when_checked(self) -> None:
        with self.assertRaises(subprocess.CalledProcessError) as raised:
            run([sys.executable, "-c", "import sys; sys.stderr.write('boom'); sys.exit(3)"])
        self.assertEqual(raised.exception.returncode, 3)
        self.assertEqual(raised.exception.stderr.strip(), "boom")

    def test_non_zero_is_returned_when_not_checked(self) -> None:
        result = run(
            [sys.executable, "-c", "import sys; sys.exit(3)"],
            check=False,
        )
        self.assertEqual(result.returncode, 3)

    def test_cwd_and_env_are_honoured(self) -> None:
        script = "import os; print(os.getcwd()); print(os.environ.get('GORDIAN_TRIAL', ''))"
        result = run(
            [sys.executable, "-c", script],
            cwd=Path.cwd(),
            env={"GORDIAN_TRIAL": "seed-7", "PATH": ""},
        )
        self.assertEqual(result.stdout.splitlines()[1], "seed-7")

    def test_missing_executable_surfaces_as_oserror(self) -> None:
        with self.assertRaises(OSError):
            run(["gordian-no-such-binary-b7f1"])


if __name__ == "__main__":
    unittest.main()
