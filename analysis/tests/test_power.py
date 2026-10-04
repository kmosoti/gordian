import numpy as np
import pytest
from scipy import stats

from gordian_analysis.power import equivalence_n, noninferiority_n


def test_noninferiority_textbook_case():
    # z_{0.95} = 1.64485, z_{0.80} = 0.84162 ; sum = 2.48647
    # n = (2.48647 * 1 / 0.5)^2 = 4.97294^2 = 24.730 -> 25
    r = noninferiority_n(sd=1, margin=0.5, alpha=0.05, power=0.8)
    assert r.n == 25
    assert r.n_unrounded == pytest.approx(24.7302, abs=1e-3)
    assert (r.sd, r.margin, r.alpha, r.power, r.true_diff) == (1, 0.5, 0.05, 0.8, 0.0)
    assert r.design == "noninferiority"


def test_equivalence_textbook_case():
    # z_{0.95} = 1.64485, z_{1 - 0.2/2} = z_{0.90} = 1.28155 ; sum = 2.92640
    # n = (2.92640 / 0.5)^2 = 5.85280^2 = 34.255 -> 35
    r = equivalence_n(sd=1, margin=0.5, alpha=0.05, power=0.8)
    assert r.n == 35
    assert r.n_unrounded == pytest.approx(34.2554, abs=1e-3)


def test_noninferiority_with_true_benefit_needs_fewer():
    # margin + true_diff = 0.75 -> n = (2.48647/0.75)^2 = 10.99 -> 11
    r = noninferiority_n(sd=1, margin=0.5, true_diff=0.25)
    assert r.n == 11
    assert r.n < noninferiority_n(sd=1, margin=0.5).n


def test_noninferiority_with_true_harm_needs_more():
    # margin + true_diff = 0.25 -> n = (2.48647/0.25)^2 = 98.9 -> 99
    assert noninferiority_n(sd=1, margin=0.5, true_diff=-0.25).n == 99


def test_exact_integer_boundary_is_not_bumped():
    # Choose sd so that the unrounded n is an exact integer 100: sd/margin = 10/z.
    z = stats.norm.ppf(0.95) + stats.norm.ppf(0.8)
    r = noninferiority_n(sd=10 / z * 0.5, margin=0.5)
    assert r.n == 100


def test_sd_scaling_by_hand():
    # n scales with sd^2: doubling sd quadruples the unrounded n.
    a = noninferiority_n(sd=1, margin=0.5).n_unrounded
    b = noninferiority_n(sd=2, margin=0.5).n_unrounded
    assert b == pytest.approx(4 * a)


@pytest.mark.parametrize("fn", [noninferiority_n, equivalence_n])
def test_monotone_in_sd_and_margin(fn):
    sds = [0.2, 0.5, 1.0, 2.0, 4.0]
    ns = [fn(sd=s, margin=0.3).n for s in sds]
    assert ns == sorted(ns) and len(set(ns)) == len(ns)
    margins = [0.05, 0.1, 0.2, 0.5, 1.0]
    ns = [fn(sd=1.0, margin=m).n for m in margins]
    assert ns == sorted(ns, reverse=True) and len(set(ns)) == len(ns)


@pytest.mark.parametrize("fn", [noninferiority_n, equivalence_n])
def test_monotone_in_power_and_alpha(fn):
    ns = [fn(sd=1, margin=0.3, power=p).n for p in (0.7, 0.8, 0.9, 0.95)]
    assert ns == sorted(ns) and len(set(ns)) == 4
    ns = [fn(sd=1, margin=0.3, alpha=a).n for a in (0.1, 0.05, 0.025, 0.01)]
    assert ns == sorted(ns) and len(set(ns)) == 4


def test_equivalence_needs_more_than_noninferiority():
    assert equivalence_n(sd=1, margin=0.3).n > noninferiority_n(sd=1, margin=0.3).n


def test_equivalence_true_diff_increases_n_and_must_be_inside_margin():
    base = equivalence_n(sd=1, margin=0.5).n
    assert equivalence_n(sd=1, margin=0.5, true_diff=0.1).n > base
    assert equivalence_n(sd=1, margin=0.5, true_diff=-0.1).n == equivalence_n(
        sd=1, margin=0.5, true_diff=0.1
    ).n
    with pytest.raises(ValueError):
        equivalence_n(sd=1, margin=0.5, true_diff=0.5)


def test_validation():
    with pytest.raises(ValueError):
        noninferiority_n(sd=0, margin=0.5)
    with pytest.raises(ValueError):
        noninferiority_n(sd=1, margin=-0.5)
    with pytest.raises(ValueError):
        noninferiority_n(sd=1, margin=0.5, power=1.0)
    with pytest.raises(ValueError):
        noninferiority_n(sd=1, margin=0.5, alpha=0.6)
    with pytest.raises(ValueError):
        noninferiority_n(sd=1, margin=0.5, true_diff=-0.5)


def test_normal_approximation_understates_n_for_small_samples():
    # At n = 25 the t test has less than 80% power; the report exposes it.
    r = noninferiority_n(sd=1, margin=0.5)
    assert r.achieved_power_t < 0.8
    assert r.achieved_power_t == pytest.approx(0.783, abs=0.005)


def test_achieved_power_matches_simulation():
    # Monte Carlo check of the noncentral-t power for n = 25, sd = 1, margin = 0.5, true 0.
    rng = np.random.default_rng(0)
    n, reps = 25, 20000
    d = rng.normal(0.0, 1.0, size=(reps, n))
    t = (d.mean(axis=1) + 0.5) / (d.std(axis=1, ddof=1) / np.sqrt(n))
    sim = float(np.mean(t > stats.t.ppf(0.95, n - 1)))
    assert sim == pytest.approx(noninferiority_n(sd=1, margin=0.5).achieved_power_t, abs=0.012)


def test_equivalence_power_lower_bound_is_below_simulated_power():
    rng = np.random.default_rng(1)
    r = equivalence_n(sd=1, margin=0.5)
    n, reps = r.n, 20000
    d = rng.normal(0.0, 1.0, size=(reps, n))
    se = d.std(axis=1, ddof=1) / np.sqrt(n)
    c = stats.t.ppf(0.95, n - 1)
    sim = float(np.mean(((d.mean(axis=1) + 0.5) / se > c) & ((d.mean(axis=1) - 0.5) / se < -c)))
    assert r.achieved_power_t <= sim + 0.012
