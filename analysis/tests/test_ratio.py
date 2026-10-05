import numpy as np
import pytest

from gordian_analysis import equivalence as eq
from gordian_analysis.intervals import ratio_of_totals_ci
from gordian_analysis.load import load_pair

from conftest import write_run


def pair(tmp_path, a_vals, b_vals, col="bill_compute"):
    a = write_run(tmp_path / "a", [{"seed": i, col: v} for i, v in enumerate(a_vals, 1)])
    b = write_run(tmp_path / "b", [{"seed": i, col: v} for i, v in enumerate(b_vals, 1)])
    return load_pair(a, b)


def test_point_estimate_by_hand(tmp_path):
    # A = 10, 20, 30 (sum 60); B = 8, 12, 20 (sum 40). S = 1 - 40/60 = 1/3, a ratio of
    # totals. The mean of per-episode ratios would be 1 - (0.8 + 0.6 + 0.667)/3 = 0.311.
    r = ratio_of_totals_ci(pair(tmp_path, [10, 20, 30], [8, 12, 20]), "bill_compute", seed=1,
                           resamples=200)  # fmt: skip
    assert r.savings == pytest.approx(1 / 3)
    assert (r.sum_a, r.sum_b, r.n) == (60.0, 40.0, 3)
    assert r.savings != pytest.approx(1 - np.mean([0.8, 0.6, 20 / 30]))


def test_interval_by_hand_two_episodes(tmp_path):
    # A = [10, 10], B = [0, 10]. Resampled index pairs (each prob 1/4):
    #   (0,0): S = 1 - 0/20 = 1 ; (0,1),(1,0): S = 1 - 10/20 = 0.5 ; (1,1): S = 1 - 20/20 = 0.
    # 25% of mass sits on 0 and on 1, so the 5th and 95th percentiles are exactly 0 and 1.
    r = ratio_of_totals_ci(pair(tmp_path, [10, 10], [0, 10]), "bill_compute", seed=4,
                           resamples=20000, method="percentile")  # fmt: skip
    assert r.savings == pytest.approx(0.5)
    assert r.low == pytest.approx(0.0, abs=1e-12) and r.high == pytest.approx(1.0, abs=1e-12)
    assert r.method == "percentile"


def test_pair_preservation_zero_width(tmp_path):
    # B = 0.7 * A on a very noisy A. Resampling episodes as pairs leaves S = 0.30 in every
    # resample. Resampling the arms independently would give a width of order 0.05 or more.
    rng = np.random.default_rng(8)
    a = rng.uniform(1, 100, size=80)
    r = ratio_of_totals_ci(pair(tmp_path, a, 0.7 * a), "bill_compute", seed=2, resamples=1000)
    assert r.savings == pytest.approx(0.30)
    assert r.high - r.low < 1e-9
    assert r.low == pytest.approx(0.30)


def test_reproducible_and_seed_sensitive(tmp_path):
    rng = np.random.default_rng(3)
    a = rng.uniform(1, 10, size=40)
    p = pair(tmp_path, a, a * rng.uniform(0.5, 1.0, size=40))
    r1 = ratio_of_totals_ci(p, "bill_compute", seed=7, resamples=1000)
    r2 = ratio_of_totals_ci(p, "bill_compute", seed=7, resamples=1000)
    r3 = ratio_of_totals_ci(p, "bill_compute", seed=8, resamples=1000)
    assert r1 == r2
    assert (r1.low, r1.high) != (r3.low, r3.high)
    assert (r1.seed, r3.seed) == (7, 8) and r1.savings == r3.savings


def test_straddling_threshold_decision_is_false(tmp_path):
    # True savings about 0.20 with noisy per-episode ratios: the 90% interval contains 0.20,
    # so S is not shown to exceed it even though the point estimate may be above it.
    rng = np.random.default_rng(5)
    a = rng.uniform(5, 15, size=30)
    b = a * rng.uniform(0.4, 1.2, size=30)
    r = ratio_of_totals_ci(pair(tmp_path, a, b), "bill_compute", seed=1)
    assert r.low < 0.20 < r.high  # construction check
    assert eq.exceeds(r.low, 0.20) is False


def test_exceeds_is_strict_and_uses_lower_limit():
    assert eq.exceeds(0.21, 0.20)
    assert not eq.exceeds(0.20, 0.20)
    assert not eq.exceeds(0.10, 0.20)


def test_decision_true_when_interval_clear_of_threshold(tmp_path):
    a = np.linspace(5, 15, 50)
    r = ratio_of_totals_ci(pair(tmp_path, a, 0.5 * a), "bill_compute", seed=1, resamples=500)
    assert eq.exceeds(r.low, 0.20)


def test_zero_total_in_arm_a_fails_loudly(tmp_path):
    with pytest.raises(ValueError, match="zero"):
        ratio_of_totals_ci(pair(tmp_path, [0, 0, 0], [1, 2, 3]), "bill_compute", seed=1)


def test_resample_with_zero_total_in_arm_a_fails_loudly(tmp_path):
    # sum(A) > 0 overall but a resample can be all zeros: one nonzero among 2 episodes.
    with pytest.raises(ValueError, match="resample"):
        ratio_of_totals_ci(pair(tmp_path, [0, 5], [0, 1]), "bill_compute", seed=1, resamples=200)


def test_negative_values_and_bad_args_rejected(tmp_path):
    p = pair(tmp_path, [1, 2, 3], [1, -2, 3])
    with pytest.raises(ValueError, match="negative"):
        ratio_of_totals_ci(p, "bill_compute", seed=1)
    p2 = pair(tmp_path / "x", [1, 2, 3], [1, 2, 3])
    with pytest.raises(ValueError):
        ratio_of_totals_ci(p2, "bill_compute", seed=1, confidence=1.0)
    with pytest.raises(ValueError):
        ratio_of_totals_ci(p2, "bill_compute", seed=1.5)  # type: ignore[arg-type]


def test_measured_total_ns_metric(tmp_path):
    # measured_total_ns = component + sched + harness; here 6+4+0 = 10 vs 3+3+0 = 6 per episode.
    a = write_run(
        tmp_path / "a",
        [{"seed": s, "measured_component_ns": 6, "measured_sched_ns": 4} for s in (1, 2)],
    )
    b = write_run(
        tmp_path / "b",
        [{"seed": s, "measured_component_ns": 3, "measured_sched_ns": 3} for s in (1, 2)],
    )
    r = ratio_of_totals_ci(load_pair(a, b), "measured_total_ns", seed=1, resamples=100)
    assert r.savings == pytest.approx(1 - 12 / 20)  # 1 - (6+6)/(10+10)
