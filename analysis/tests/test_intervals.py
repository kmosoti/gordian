import numpy as np
import pytest

from gordian_analysis.intervals import paired_bootstrap_ci, percentile_interval

D = np.array([0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0.0])


def test_percentile_interval_by_hand():
    # stats = 1..100. 90% interval: tails 0.05 and 0.95, linear interpolation at
    # position q*(n-1): 0.05*99 = 4.95 -> 1 + 4.95 = 5.95 ; 0.95*99 = 94.05 -> 1 + 94.05 = 95.05
    lo, hi = percentile_interval(np.arange(1, 101, dtype=float), 0.90)
    assert lo == pytest.approx(5.95)
    assert hi == pytest.approx(95.05)


def test_constant_differences_give_degenerate_interval():
    # mean of any resample of [0.25]*8 is 0.25
    r = paired_bootstrap_ci(np.full(8, 0.25), seed=3, n_resamples=200)
    assert r.low == pytest.approx(0.25) and r.high == pytest.approx(0.25)
    assert r.mean == pytest.approx(0.25)


def test_two_point_support_by_hand():
    # d = [0, 1]: a resample of size 2 has mean 0, 0.5, or 1 with probs 1/4, 1/2, 1/4.
    # So the 5th percentile is 0 and the 95th is 1 for a large number of resamples,
    # and every resample mean lies in {0, 0.5, 1}.
    r = paired_bootstrap_ci(np.array([0.0, 1.0]), seed=11, n_resamples=20000)
    assert r.low == pytest.approx(0.0, abs=1e-9)
    assert r.high == pytest.approx(1.0, abs=1e-9)
    assert r.mean == 0.5


def test_reproducible_for_fixed_seed():
    a = paired_bootstrap_ci(D, seed=42, n_resamples=2000)
    b = paired_bootstrap_ci(D, seed=42, n_resamples=2000)
    assert a == b


def test_different_seed_changes_interval_and_is_recorded():
    rng = np.random.default_rng(0)
    d = rng.normal(0.1, 1.0, size=50)
    a = paired_bootstrap_ci(d, seed=1, n_resamples=1000)
    b = paired_bootstrap_ci(d, seed=2, n_resamples=1000)
    assert (a.low, a.high) != (b.low, b.high)
    assert (a.seed, b.seed) == (1, 2)
    assert a.mean == b.mean


def test_seed_is_required():
    with pytest.raises(TypeError):
        paired_bootstrap_ci(D)  # type: ignore[call-arg]


def test_defaults():
    r = paired_bootstrap_ci(D, seed=0)
    assert r.n_resamples == 10_000
    assert r.confidence == 0.90
    assert r.n == len(D)


def test_resamples_episodes_as_pairs():
    # B = A + 0.1 exactly, with A very noisy. Resampling episodes as pairs leaves every
    # difference at 0.1, so the interval has zero width. Resampling the arms independently
    # would give a width of order sd(A)/sqrt(n) ~ 0.3.
    rng = np.random.default_rng(9)
    a = rng.normal(0, 3.0, size=100)
    b = a + 0.1
    r = paired_bootstrap_ci(b - a, seed=5, n_resamples=1000)
    assert r.high - r.low < 1e-9
    assert r.low == pytest.approx(0.1)


def test_interval_agrees_with_t_interval_for_large_normal_sample():
    from gordian_analysis.equivalence import t_interval

    rng = np.random.default_rng(10)
    d = rng.normal(0.2, 1.0, size=2000)
    r = paired_bootstrap_ci(d, seed=1)
    lo, hi = t_interval(d, 0.90)
    assert r.low == pytest.approx(lo, abs=0.01)
    assert r.high == pytest.approx(hi, abs=0.01)


def test_empirical_coverage_is_near_nominal_for_moderate_n():
    # 300 simulated experiments of n = 60 from a skewed distribution with true mean 0.
    rng = np.random.default_rng(12)
    hits = 0
    reps = 300
    for i in range(reps):
        d = rng.exponential(1.0, size=60) - 1.0
        r = paired_bootstrap_ci(d, seed=i, n_resamples=500)
        hits += r.low <= 0.0 <= r.high
    assert 0.84 <= hits / reps <= 0.96


@pytest.mark.parametrize(
    "kwargs",
    [
        {"confidence": 1.0},
        {"confidence": 0.0},
        {"n_resamples": 0},
        {"seed": 1.5},
    ],
)
def test_validation(kwargs):
    args = {"seed": 1, **kwargs}
    with pytest.raises(ValueError):
        paired_bootstrap_ci(D, **args)


def test_rejects_short_or_nonfinite_input():
    with pytest.raises(ValueError):
        paired_bootstrap_ci(np.array([1.0]), seed=1)
    with pytest.raises(ValueError):
        paired_bootstrap_ci(np.array([1.0, np.inf]), seed=1)
