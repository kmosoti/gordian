import csv
from pathlib import Path

import pytest

from gordian_analysis.load import MEASURED_COLUMNS, RESULTS_COLUMNS

FIXTURES = Path(__file__).parent / "fixtures"

DEFAULTS = {
    "run_id": "r",
    "seed": 1,
    "class": "c",
    "success": 1,
    "critical_miss": 0,
    "false_alarm": 0,
    "abstained": 0,
    "undecided": 0,
    "probes_used": 1,
    "corrections": 0,
    "decision_at_ns": 100,
    "bill_compute": 0,
    "bill_memory": 0,
    "bill_time": 0,
    "bill_probes": 0,
    "bill_comm": 0,
    "bill_storage": 0,
    "components_run": 1,
    "components_skipped": 0,
    "directives_ignored": 0,
    "stop_reason": "terminal",
}

MEASURED_DEFAULTS = {
    "run_id": "r",
    "seed": 1,
    "class": "c",
    "measured_component_ns": 0,
    "measured_sched_ns": 0,
    "measured_harness_ns": 0,
}


def write_run(
    path: Path,
    rows: list[dict],
    run_id: str = "r",
    extra_cols: tuple[str, ...] = (),
    measured: bool = True,
):
    """Write results.csv and measured.csv under `path` from partial row dicts.

    Each row dict may mix keys of both files; defaults fill the rest. An undecided row must
    also set `"decision_at_ns": ""`, as the harness does. `measured=False` skips measured.csv.
    """
    path.mkdir(parents=True, exist_ok=True)
    cols = list(RESULTS_COLUMNS) + list(extra_cols)
    with open(path / "results.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=cols, lineterminator="\n", extrasaction="ignore")
        w.writeheader()
        for r in rows:
            w.writerow({**DEFAULTS, "run_id": run_id, **r})
    if measured:
        with open(path / "measured.csv", "w", newline="") as fh:
            w = csv.DictWriter(fh, fieldnames=MEASURED_COLUMNS, lineterminator="\n",
                               extrasaction="ignore")  # fmt: skip
            w.writeheader()
            for r in rows:
                w.writerow({**MEASURED_DEFAULTS, "run_id": run_id, **r})
    return path


def write_positioned_run(path: Path, rows: list[dict], run_id: str = "r") -> Path:
    """`write_run`, then rewrite measured.csv with an `arm_position` column (an interleaved arm).

    Each row dict gives `arm_position`; the cost is `measured_component_ns` (the other two
    measured columns default to 0, so `measured_total_ns` equals it).
    """
    write_run(path, rows, run_id=run_id)
    cols = list(MEASURED_COLUMNS) + ["arm_position"]
    with open(path / "measured.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=cols, lineterminator="\n", extrasaction="ignore")
        w.writeheader()
        for r in rows:
            w.writerow({**MEASURED_DEFAULTS, "run_id": run_id, **r})
    return path


def write_drift(
    path: Path, ns: list[int], min_ns: list[int] | None = None, run_id: str = "r"
) -> Path:
    """Write `drift.csv` into `path` with one block per entry of `ns`."""
    path.mkdir(parents=True, exist_ok=True)
    min_ns = min_ns or [max(1, n // 2000) for n in ns]
    lines = ["run_id,block,units_done,reps,ns,min_ns"]
    for i, (n, m) in enumerate(zip(ns, min_ns, strict=True)):
        lines.append(f"{run_id},{i},{50 * i},2000,{n},{m}")
    (path / "drift.csv").write_text("\n".join(lines) + "\n")
    return path


@pytest.fixture
def fixtures_dir() -> Path:
    return FIXTURES
