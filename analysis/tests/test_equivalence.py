import math

import numpy as np
import pytest
from scipy import stats

from gordian_analysis import equivalence as eq
from gordian_analysis.intervals import paired_bootstrap_ci


# Hand-computed case: d = [0.1, 0.2, 0.3], n = 3, df = 2.
#   mean = 0.2; deviations -0.1, 0, 0.1; sum of squares = 0.02; var = 0.02/2 = 0.01; sd = 0.1
#   se = 0.1/sqrt(3) = 0.0577350
D3 = np.array([0.1, 0.2, 0.3])


def t2_cdf(t):
    """Closed-form CDF of Student t with 2 df: 1/2 + t / (2 sqrt(2 + t^2))."""
    return 0.5 + t / (2.0 * math.sqrt(2.0 + t * t))


def test_t_statistic_by_hand():
    # t = 0.2 / 0.057735 = 2 * sqrt(3) = 3.4641
    t, df = eq.paired_t_statistic(D3)
    assert df == 2
    assert t == pytest.approx(2 * math.sqrt(3))


def test_t_statistic_matches_scipy_ttest_rel():
    rng = np.random.default_rng(0)
    a = rng.normal(size=40)
    b = a + rng.normal(0.3, 1.0, size=40)
    t, df = eq.paired_t_statistic(b - a)
    ref = stats.ttest_rel(b, a)
    assert t == pytest.approx(ref.statistic, rel=1e-12)
    assert df == len(a) - 1
    # two-sided p from our t equals scipy's
    assert 2 * stats.t.sf(abs(t), df) == pytest.approx(ref.pvalue, rel=1e-10)


def test_tost_by_hand_margin_half():
    # margin 0.5. t_lower = (0.2 + 0.5)/0.057735 = 7*sqrt(3) = 12.1244
    #             t_upper = (0.2 - 0.5)/0.057735 = -3*sqrt(3) = -5.1962
    # p_lower = 1 - F2(12.1244) ~ 0.003367 ; p_upper = F2(-5.1962) ~ 0.017549 (closed form)
    r = eq.tost_paired(D3, 0.5)
    assert r.t_lower == pytest.approx(7 * math.sqrt(3))
    assert r.t_upper == pytest.approx(-3 * math.sqrt(3))
    assert r.p_lower == pytest.approx(1 - t2_cdf(7 * math.sqrt(3)), rel=1e-9)
    assert r.p_upper == pytest.approx(t2_cdf(-3 * math.sqrt(3)), rel=1e-9)
    # 12.1244 / (2 * 12.2066) = 0.496633 -> p_lower = 0.003367
    assert r.p_lower == pytest.approx(0.003367, abs=1e-6)
    assert r.p_upper == pytest.approx(0.017549, abs=1e-6)
    assert r.p_tost == r.p_upper
    assert r.rejects  # 0.0175 < 0.05


def test_tost_matches_scipy_one_sample_tests():
    rng = np.random.default_rng(1)
    d = rng.normal(0.02, 0.3, size=60)
    m = 0.1
    r = eq.tost_paired(d, m)
    lo = stats.ttest_1samp(d + m, 0.0, alternative="greater")
    hi = stats.ttest_1samp(d - m, 0.0, alternative="less")
    assert r.p_lower == pytest.approx(lo.pvalue, rel=1e-10)
    assert r.p_upper == pytest.approx(hi.pvalue, rel=1e-10)
    assert r.p_tost == max(lo.pvalue, hi.pvalue)


def test_t_interval_by_hand():
    # 90% interval: t_{0.95, 2} = 2.919986 ; half width = 2.919986 * 0.057735 = 0.168585
    lo, hi = eq.t_interval(D3, 0.90)
    assert lo == pytest.approx(0.2 - 0.168585, abs=1e-5)
    assert hi == pytest.approx(0.2 + 0.168585, abs=1e-5)


def test_tost_decision_equals_interval_inside_margin():
    # Rejecting both one-sided tests at alpha <=> the (1-2alpha) t interval is inside +/-margin.
    rng = np.random.default_rng(2)
    for _ in range(300):
        n = int(rng.integers(5, 80))
        d = rng.normal(rng.uniform(-0.2, 0.2), rng.uniform(0.02, 0.5), size=n)
        m = float(rng.uniform(0.05, 0.4))
        lo, hi = eq.t_interval(d, 0.90)
        assert eq.tost_paired(d, m, 0.05).rejects == (-m < lo and hi < m)


def test_zero_variance_does_not_crash():
    r = eq.tost_paired(np.zeros(5), 0.1)
    assert r.p_tost == 0.0 and r.rejects
    r = eq.tost_paired(np.full(5, 0.1), 0.1)  # mean sits exactly on the margin
    assert not r.rejects


def test_input_validation():
    with pytest.raises(ValueError):
        eq.tost_paired(np.array([1.0]), 0.1)
    with pytest.raises(ValueError):
        eq.tost_paired(np.array([1.0, np.nan]), 0.1)
    with pytest.raises(ValueError):
        eq.classify(0.0, 1.0, 0.0)
    with pytest.raises(ValueError):
        eq.classify(1.0, 0.0, 0.1)


@pytest.mark.parametrize(
    "lo, hi, margin, expected",
    [
        (-0.005, 0.005, 0.01, eq.EQUIVALENT),  # inside (-0.01, 0.01)
        (0.002, 0.009, 0.01, eq.EQUIVALENT),  # inside the margin AND above zero: equivalent wins
        (0.02, 0.05, 0.01, eq.BENEFICIAL),  # above zero, outside margin
        (-0.05, -0.02, 0.01, eq.HARMFUL),  # below zero, outside margin
        (-0.03, 0.04, 0.01, eq.UNRESOLVED),  # straddles zero, wider than margin
        (-0.01, 0.005, 0.01, eq.UNRESOLVED),  # ci_low == -margin is not strictly inside
        (-0.005, 0.01, 0.01, eq.UNRESOLVED),  # ci_high == margin is not strictly inside
        (0.0, 0.05, 0.01, eq.UNRESOLVED),  # ci_low == 0 is not > 0
        (-0.05, 0.0, 0.01, eq.UNRESOLVED),  # ci_high == 0 is not < 0
    ],
)
def test_classify_table(lo, hi, margin, expected):
    assert eq.classify(lo, hi, margin) == expected


def test_noninferior():
    assert eq.noninferior(-0.009, 0.01)
    assert not eq.noninferior(-0.01, 0.01)  # strict
    assert not eq.noninferior(-0.02, 0.01)
    assert eq.noninferior(0.5, 0.01)


def _category_from_data(d, margin):
    lo, hi = eq.t_interval(d, 0.90)
    return eq.classify(lo, hi, margin)


def test_each_category_reachable_from_synthetic_data():
    rng = np.random.default_rng(3)
    base = rng.normal(0, 1, size=400)
    base = (base - base.mean()) / base.std(ddof=1)  # mean 0, sd 1 exactly
    margin = 0.1
    # equivalent: mean 0, sd 0.2, n = 400 -> half width ~ 1.649*0.2/20 = 0.0165 << margin
    assert _category_from_data(0.2 * base, margin) == eq.EQUIVALENT
    # beneficial: mean +1
    assert _category_from_data(1.0 + 0.2 * base, margin) == eq.BENEFICIAL
    # harmful: mean -1
    assert _category_from_data(-1.0 + 0.2 * base, margin) == eq.HARMFUL
    # unresolved: mean 0 but sd 1, n = 20 -> interval ~ +/-0.37, wider than the margin
    assert _category_from_data(base[:20] - base[:20].mean(), margin) == eq.UNRESOLVED


def test_each_category_reachable_through_bootstrap_interval():
    rng = np.random.default_rng(4)
    base = rng.normal(0, 1, size=400)
    base = (base - base.mean()) / base.std(ddof=1)
    margin = 0.1

    def cat(d):
        b = paired_bootstrap_ci(d, seed=7)
        return eq.classify(b.low, b.high, margin)

    assert cat(0.2 * base) == eq.EQUIVALENT
    assert cat(1.0 + 0.2 * base) == eq.BENEFICIAL
    assert cat(-1.0 + 0.2 * base) == eq.HARMFUL
    assert cat(base[:20] - base[:20].mean()) == eq.UNRESOLVED


def test_wide_underpowered_interval_is_never_equivalent():
    # Zero-mean noisy data with few episodes: interval is far wider than the margin, so the
    # category must be unresolved, however close to zero the point estimate is.
    rng = np.random.default_rng(5)
    for _ in range(200):
        n = int(rng.integers(3, 15))
        d = rng.normal(0, 1.0, size=n)
        d = d - d.mean()  # point estimate exactly 0
        lo, hi = eq.t_interval(d, 0.90)
        assert hi - lo > 0.2  # wider than 2 * margin
        assert eq.classify(lo, hi, 0.1) == eq.UNRESOLVED
        b = paired_bootstrap_ci(d, seed=1, n_resamples=500)
        if b.high - b.low >= 0.2:
            assert eq.classify(b.low, b.high, 0.1) == eq.UNRESOLVED


def test_equivalent_implies_interval_narrower_than_twice_margin():
    rng = np.random.default_rng(6)
    for _ in range(500):
        lo = float(rng.uniform(-1, 1))
        hi = lo + float(rng.uniform(0, 2))
        m = float(rng.uniform(0.01, 1))
        if eq.classify(lo, hi, m) == eq.EQUIVALENT:
            assert hi - lo < 2 * m


# ---- preregistered-sample-size gate ------------------------------------------------------


@pytest.mark.parametrize("raw", eq.CATEGORIES)
def test_gate_below_planned_n_forces_unresolved(raw):
    assert eq.gated_category(raw, n=99, planned_n=100) == eq.UNRESOLVED


@pytest.mark.parametrize("raw", eq.CATEGORIES)
def test_gate_at_or_above_planned_n_passes_raw_through(raw):
    assert eq.gated_category(raw, n=100, planned_n=100) == raw
    assert eq.gated_category(raw, n=500, planned_n=100) == raw


@pytest.mark.parametrize("raw", eq.CATEGORIES)
def test_gate_absent_passes_raw_through(raw):
    assert eq.gated_category(raw, n=3, planned_n=None) == raw


def test_classify_stays_pure_of_sample_size():
    import inspect

    assert list(inspect.signature(eq.classify).parameters) == ["ci_low", "ci_high", "margin"]
    assert eq.classify(-0.005, 0.005, 0.01) == eq.EQUIVALENT
