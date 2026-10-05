"""The A7b acceptance simulation at full size. Excluded from the default run (marker `slow`).

    cd analysis && .venv/bin/python -W error -m pytest -m slow -q          # all four, about 1 h
    cd analysis && .venv/bin/python -W error -m pytest -m slow -q -k 60k   # one population

Acceptance (docs/local-test-plan.md, A7b): with true S exactly at the 0.20 threshold, the
chosen default's false-exceedance rate at EXP-001's planned n is at most 0.06 over at least
2,000 simulated experiments, using B1's empirical cost distribution. The cells here use the
shipped defaults (10,000 resamples, 90% interval) and the seeds recorded in `calibration.py`.
The tables for every method, every size, the lognormal comparison and power are produced by
`experiments/exploration/scripts/a7b_calibrate.py` and recorded in
`experiments/exploration/a7b-ratio-calibration.md`.
"""

from pathlib import Path

import pytest

from gordian_analysis import calibration as cal
from gordian_analysis.intervals import DEFAULT_RATIO_METHOD

DATA = Path(__file__).resolve().parents[2] / "experiments/exploration/data/a7b-paired-costs.csv.gz"
BAR = 0.06
N_EXPERIMENTS = 2000
POPULATIONS = {  # name: (population id, (pair, compute level)), as in a7b_calibrate.py
    "P1@60k": (10, ("P1", 60_000)),
    "P1@100k": (11, ("P1", 100_000)),
    "P1@20M": (12, ("P1", 20_000_000)),
    "P2@250k": (13, ("P2", 250_000)),
}


@pytest.mark.slow
@pytest.mark.parametrize("name", list(POPULATIONS))
def test_default_interval_false_exceedance_at_planned_n(name):
    pop_id, key = POPULATIONS[name]
    a, b = cal.load_paired_costs(DATA)[key]
    pop = cal.empirical_population(name, pop_id, a, b, savings=cal.THRESHOLD)
    for n in cal.PLANNED_SIZES:
        r = cal.simulate_cell(pop, n, N_EXPERIMENTS, methods=(DEFAULT_RATIO_METHOD,))
        assert r.failed[DEFAULT_RATIO_METHOD] == 0
        rate = r.rate(DEFAULT_RATIO_METHOD)
        assert rate <= BAR, (
            f"{DEFAULT_RATIO_METHOD} on {name} at n={n}: false-exceedance {rate:.4f} "
            f"(MC se {r.mc_se(DEFAULT_RATIO_METHOD):.4f}) exceeds {BAR}"
        )
