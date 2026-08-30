#!/usr/bin/env python3
"""Compatibility entrypoint for the thin GitHub Project orchestrator."""

from __future__ import annotations

import sys
from pathlib import Path

REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPOSITORY_ROOT / "orchestration" / "src"))

from gordian_orchestration.github_project import main  # noqa: E402


if __name__ == "__main__":
    raise SystemExit(main())
