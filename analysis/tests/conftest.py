import csv
from pathlib import Path

import pytest

from gordian_analysis.load import REQUIRED_COLUMNS

FIXTURES = Path(__file__).parent / "fixtures"

DEFAULTS = {
    "run_id": "r",
    "seed": 1,
    "class": "c",
    "success": 1,
    "critical_miss": 0,
    "false_alarm": 0,
    "abstained": 0,
    "probes_used": 1,
    "decision_at_ns": 100,
    "bill_compute": 0,
    "bill_memory": 0,
    "bill_time": 0,
    "bill_probes": 0,
    "bill_comm": 0,
    "bill_storage": 0,
    "components_run": 1,
    "components_skipped": 0,
}


def write_run(path: Path, rows: list[dict], run_id: str = "r", extra_cols: tuple[str, ...] = ()):
    """Write a results.csv under `path` from partial row dicts (defaults fill the rest)."""
    path.mkdir(parents=True, exist_ok=True)
    cols = list(REQUIRED_COLUMNS) + list(extra_cols)
    with open(path / "results.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=cols, lineterminator="\n")
        w.writeheader()
        for r in rows:
            w.writerow({**DEFAULTS, "run_id": run_id, **r})
    return path


@pytest.fixture
def fixtures_dir() -> Path:
    return FIXTURES
