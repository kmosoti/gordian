"""The calibration machinery of work item A7b, at sizes that run in seconds.

The long simulation (2,000 experiments at the planned sizes) is `test_calibration_slow.py`,
marked `slow` and excluded from the default run. What is checked here is that the machinery
builds the null it claims to build, replays exactly, counts what it should, and shows the
documented behaviour (the percentile interval is liberal on skewed costs; the studentized one is
closer to nominal) on a small run with fixed seeds.
"""

import math
from pathlib import Path

import numpy as np
import pytest

from gordian_analysis import calibration as cal
from gordian_analysis.intervals import RATIO_METHODS

DATA = Path(__file__).resolve().parents[2] / "experiments/exploration/data/a7b-paired-costs.csv.gz"


@pytest.fixture(scope="module")
def paired_costs():
    return cal.load_paired_costs(DATA)


# ---- the null is what it says -----------------------------------------------------------------


def test_rescale_sets_the_ratio_of_totals_exactly():
    a = np.array([10.0, 20.0, 30.0, 5.0])
    b = np.array([7.0, 3.0, 40.0, 1.0])
    for s in (0.0, 0.2, 0.3, -0.1):
        b2 = cal.rescale_to_savings(a, b, s)
        assert 1.0 - b2.sum() / a.sum() == pytest.approx(s, abs=1e-12)
        assert b2 / b == pytest.approx(np.full(4, b2[0] / b[0]))  # one constant: pairs untouched
    with pytest.raises(ValueError):
        cal.rescale_to_savings(a, np.zeros(4), 0.2)


def test_empirical_sampler_draws_whole_pairs_from_the_rescaled_population():
    a = np.array([1.0, 2.0, 3.0])
    b = np.array([1.0, 1.0, 4.0])
    pop = cal.empirical_population("tiny", 1, a, b, savings=0.25)
    k = (1 - 0.25) * a.sum() / b.sum()
    sa, sb = pop.sampler(np.random.default_rng(0), 500)
    assert set(zip(sa, np.round(sb / k, 12))) <= {(1.0, 1.0), (2.0, 1.0), (3.0, 4.0)}
    assert len(set(sa)) == 3  # all three episodes are drawn


def test_lognormal_sampler_has_the_stated_true_savings():
    pop = cal.lognormal_population("ln", 2, sigma_a=0.5, sigma_r=0.25, savings=0.2)
    a, b = pop.sampler(np.random.default_rng(1), 2_000_000)
    assert 1.0 - b.mean() / a.mean() == pytest.approx(0.2, abs=0.004)


def test_committed_paired_costs_are_b2s_proxy_pairs(paired_costs):
    # S per pair and level, as tabulated in b2-power.md T3 (4 decimals); 5,500 episodes each.
    expect = {
        ("P1", 60_000): 0.2736, ("P1", 100_000): 0.3166, ("P1", 250_000): 0.3176,
        ("P1", 20_000_000): 0.3176, ("P2", 250_000): 0.1316, ("P2", 20_000_000): 0.4480,
    }  # fmt: skip
    for key, s in expect.items():
        a, b = paired_costs[key]
        assert len(a) == len(b) == 5500
        assert 1.0 - b.sum() / a.sum() == pytest.approx(s, abs=5e-5)


# ---- seeds and bookkeeping --------------------------------------------------------------------


def test_each_experiment_has_its_own_replayable_stream():
    r1 = cal.experiment_rng(10, 0.2, 40, 3).random(5)
    r2 = cal.experiment_rng(10, 0.2, 40, 3).random(5)
    assert (r1 == r2).all()
    for other in [(11, 0.2, 40, 3), (10, 0.25, 40, 3), (10, 0.2, 160, 3), (10, 0.2, 40, 4)]:
        assert not (cal.experiment_rng(*other).random(5) == r1).all()


def test_cell_is_reproducible_and_splits_by_experiment_index(paired_costs):
    a, b = paired_costs[("P1", 100_000)]
    pop = cal.empirical_population("P1@100k", 11, a, b)
    kw = dict(resamples=200)
    whole = cal.simulate_cell(pop, 40, 8, **kw)
    again = cal.simulate_cell(pop, 40, 8, **kw)
    assert whole == again
    left = cal.simulate_cell(pop, 40, 5, first=0, **kw)
    right = cal.simulate_cell(pop, 40, 3, first=5, **kw)
    for m in RATIO_METHODS:
        assert whole.exceed[m] == left.exceed[m] + right.exceed[m]
        assert 0 <= whole.exceed[m] <= 8 and whole.failed[m] == 0


def test_a_failing_method_is_counted_not_dropped(monkeypatch, paired_costs):
    def broken(*args, **kwargs):
        raise ValueError("BCa is undefined: test")

    monkeypatch.setattr(cal, "_bca_interval", broken)
    a, b = paired_costs[("P1", 100_000)]
    pop = cal.empirical_population("P1@100k", 11, a, b)
    r = cal.simulate_cell(pop, 40, 6, resamples=100)
    assert r.failed["bca"] == 6 and r.exceed["bca"] == 0 and r.n_exp == 6
    assert r.failed["percentile"] == r.failed["studentized"] == 0
    assert r.rate("bca") == 0.0  # a failure is never an exceedance


def test_mc_se_and_wilson_by_hand():
    r = cal.CellResult("x", 0.2, 40, 2000, 100, 0.2, {"m": 100}, {"m": 0}, {"m": 0.0}, {"m": 0.0},
                       {"m": 0})  # fmt: skip
    assert r.rate("m") == 0.05
    assert r.mc_se("m") == pytest.approx(math.sqrt(0.05 * 0.95 / 2000))
    # 50/1000: centre 0.051721, half-width 0.013592
    lo, hi = cal.wilson_interval(50, 1000)
    assert (lo, hi) == pytest.approx((0.03813, 0.06531), abs=5e-4)


# ---- a small run shows the documented behaviour ----------------------------------------------


def test_percentile_is_liberal_and_studentized_closer_on_skewed_empirical_costs(paired_costs):
    # P1 at 60,000: the most skewed of B1's pairs. 300 experiments of n = 40 at true S = 0.20
    # (seeded, so the numbers are fixed; the full-size version is the slow test). In the
    # committed full run the rates at this n are 0.117 (percentile), 0.068 (BCa), 0.049
    # (studentized); here only the ordering and a wide gap are asserted.
    a, b = paired_costs[("P1", 60_000)]
    pop = cal.empirical_population("P1@60k", 10, a, b)
    r = cal.simulate_cell(pop, 40, 300, resamples=1000)
    assert r.rate("percentile") > r.rate("studentized") + 0.03
    assert r.rate("percentile") > 0.08
    assert r.rate("studentized") < 0.09
    assert sum(r.failed.values()) == 0


def test_power_rises_with_true_savings(paired_costs):
    a, b = paired_costs[("P1", 100_000)]
    rates = []
    for s in (0.20, 0.25, 0.30):
        pop = cal.empirical_population("P1@100k", 11, a, b, savings=s)
        rates.append(cal.simulate_cell(pop, 160, 200, resamples=300).rate("studentized"))
    assert rates[0] < 0.10 and rates[1] > 0.5 and rates[2] > 0.95


def test_planned_size_cell_runs(paired_costs):
    # a smoke run at the planned sizes (a couple of experiments, few resamples)
    a, b = paired_costs[("P1", 60_000)]
    pop = cal.empirical_population("P1@60k", 10, a, b)
    for n in cal.PLANNED_SIZES:
        r = cal.simulate_cell(pop, n, 2, resamples=100)
        assert r.n == n and r.n_exp == 2
