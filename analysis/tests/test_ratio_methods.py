"""The three interval methods for S = 1 - sum(B)/sum(A) (work item A7b).

Each is checked against something that does not share its code: scipy's BCa, a plain-loop
bootstrap-t, hand-computed levels, and the invariants that must hold (shared resamples, B
proportional to A giving a point interval, a zero total in B).
"""

import json
import math

import numpy as np
import pytest
from scipy.stats import bootstrap, norm

from gordian_analysis import intervals as iv
from gordian_analysis.cli import main
from gordian_analysis.intervals import (
    DEFAULT_RATIO_METHOD,
    RATIO_METHODS,
    ratio_intervals,
    ratio_of_totals_ci,
)
from gordian_analysis.load import load_pair

from conftest import write_run


def pair(tmp_path, a_vals, b_vals, col="bill_compute"):
    a = write_run(tmp_path / "a", [{"seed": i, col: v} for i, v in enumerate(a_vals, 1)])
    b = write_run(tmp_path / "b", [{"seed": i, col: v} for i, v in enumerate(b_vals, 1)])
    return load_pair(a, b)


def skewed_sample(n=60, seed=3, sigma_a=0.8, sigma_r=0.4, ratio=0.8):
    rng = np.random.default_rng(seed)
    a = rng.lognormal(0.0, sigma_a, n)
    return a, a * ratio * rng.lognormal(0.0, sigma_r, n)


def s_of(a, b, axis=-1):
    return 1.0 - b.sum(axis=axis) / a.sum(axis=axis)


# ---- percentile: unchanged by the new code ---------------------------------------------------


def test_percentile_is_the_quantile_of_the_resampled_s():
    a, b = skewed_sample()
    got = ratio_intervals(a, b, np.random.default_rng(5), resamples=3000, methods=("percentile",))
    rng = np.random.default_rng(5)
    s_star = np.concatenate([s_of(a[i], b[i]) for i in iv._resample_indices(rng, len(a), 3000)])
    want = np.quantile(s_star, [0.05, 0.95])
    assert got["percentile"] == pytest.approx(tuple(want), abs=1e-15)


def test_methods_read_the_same_resamples():
    # Same seed, same indices: asking for one method or all three gives the same limits.
    a, b = skewed_sample()
    every = ratio_intervals(a, b, np.random.default_rng(11), resamples=2000)
    for m in RATIO_METHODS:
        alone = ratio_intervals(a, b, np.random.default_rng(11), resamples=2000, methods=(m,))
        assert alone[m] == every[m]


# ---- BCa --------------------------------------------------------------------------------------


def test_bca_levels_by_hand():
    # No bias and no acceleration: BCa is the percentile interval.
    lo, hi = iv._bca_levels(0.0, 0.0, 0.05)
    assert (lo, hi) == pytest.approx((0.05, 0.95))
    # z0 = 0.1, a = 0.05. z_lo = 0.1 - 1.644854 = -1.544854; denominator 1 - 0.05 * z_lo = 1.077243;
    # level = Phi(0.1 + z_lo / 1.077243) = Phi(-1.334125). Likewise for the upper level.
    z0, acc = 0.1, 0.05
    z_lo, z_hi = z0 + norm.ppf(0.05), z0 + norm.ppf(0.95)
    want_lo = norm.cdf(z0 + z_lo / (1 - acc * z_lo))
    want_hi = norm.cdf(z0 + z_hi / (1 - acc * z_hi))
    assert want_lo == pytest.approx(0.0910, abs=2e-4)  # the hand arithmetic above
    assert iv._bca_levels(z0, acc, 0.05) == pytest.approx((want_lo, want_hi))


def test_bca_matches_scipy():
    a, b = skewed_sample()
    ref = bootstrap(
        (a, b), s_of, paired=True, vectorized=True, n_resamples=200_000, confidence_level=0.90,
        method="BCa", random_state=np.random.default_rng(1),
    ).confidence_interval  # fmt: skip
    got = ratio_intervals(a, b, np.random.default_rng(5), resamples=200_000, methods=("bca",))["bca"]
    assert got == pytest.approx((ref.low, ref.high), abs=0.003)
    # and it is not the percentile interval on this skewed sample
    pct = ratio_intervals(a, b, np.random.default_rng(5), resamples=20_000, methods=("percentile",))
    assert abs(got[0] - pct["percentile"][0]) > 0.003


def test_bca_undefined_cases_raise():
    a, b = np.array([10.0, 10.0]), np.array([0.0, 10.0])
    # two episodes, 3 distinct values of S*: S-hat = 0.5 is hit by half the resamples, so z0 = 0
    # and the interval exists...
    assert ratio_intervals(a, b, np.random.default_rng(1), resamples=2000, methods=("bca",))
    # ... but with one nonzero A, a resample (or a leave-one-out) can have no arm-A cost at all.
    with pytest.raises(ValueError, match="zero total"):
        ratio_intervals(np.array([0.0, 5.0]), np.array([0.0, 1.0]), np.random.default_rng(1),
                        resamples=2000, methods=("bca",))  # fmt: skip


# ---- studentized ------------------------------------------------------------------------------


def test_studentized_matches_a_plain_loop_bootstrap_t():
    a, b = skewed_sample(n=40)
    n, nb = len(a), 4000
    got = ratio_intervals(a, b, np.random.default_rng(9), resamples=nb,
                          methods=("studentized",))["studentized"]  # fmt: skip
    # The reference sees the same index matrix and computes each resample's t one at a time.
    rng = np.random.default_rng(9)
    idx = np.concatenate(list(iv._resample_indices(rng, n, nb)))
    r_hat = b.sum() / a.sum()
    theta_hat = math.log(r_hat)
    se_hat = np.std(b - r_hat * a, ddof=1) / math.sqrt(n) / b.mean()
    t = []
    for i in idx:
        aa, bb = a[i], b[i]
        r = bb.sum() / aa.sum()
        se = np.std(bb - r * aa, ddof=1) / math.sqrt(n) / bb.mean()
        t.append((math.log(r) - theta_hat) / se)
    t = np.sort(t)
    # order statistics ceil((B+1) p) - 1 (zero-based): 201 -> index 200, 3800.95 -> 3801 -> 3800
    k_lo, k_hi = math.ceil((nb + 1) * 0.05) - 1, math.ceil((nb + 1) * 0.95) - 1
    want_low = 1.0 - math.exp(theta_hat - se_hat * t[k_lo])
    want_high = 1.0 - math.exp(theta_hat - se_hat * t[k_hi])
    assert got == pytest.approx((want_low, want_high), abs=1e-10)


def test_studentized_se_is_the_bootstrap_sd_of_the_log_ratio():
    # The delta-method standard error is what the t statistic is divided by; it should agree
    # with the bootstrap's own spread of log(sum B*/sum A*) at moderate n.
    a, b = skewed_sample(n=300, sigma_a=0.5, sigma_r=0.25)
    rng = np.random.default_rng(2)
    s_star, se_star = iv._ratio_resamples(a, b, rng, 4000, with_se=True, label="x")
    theta_star = np.log1p(-s_star)
    r_hat = b.sum() / a.sum()
    se_hat = np.std(b - r_hat * a, ddof=1) / math.sqrt(len(a)) / b.mean()
    assert se_hat == pytest.approx(theta_star.std(ddof=1), rel=0.08)
    assert np.median(se_star) == pytest.approx(se_hat, rel=0.08)


def test_studentized_widens_where_percentile_is_too_narrow():
    # A right-skewed cost sample: the studentized interval has a lower S limit below the
    # percentile one (it is the more cautious about exceeding a threshold).
    a, b = skewed_sample(n=30, seed=4, sigma_a=1.0, sigma_r=0.5)
    out = ratio_intervals(a, b, np.random.default_rng(1), resamples=20_000)
    assert out["studentized"][0] < out["percentile"][0]


def test_studentized_infinite_t_is_kept_not_dropped():
    # n = 3: a resample that draws one episode three times has zero standard error and t = +-inf.
    # Those resamples are 11% of the total, over the 5% tail, so the lower limit is -inf.
    a, b = np.array([100.0, 100.0, 100.0]), np.array([50.0, 50.0, 90.0])
    lo, hi = ratio_intervals(a, b, np.random.default_rng(1), resamples=2000,
                             methods=("studentized",))["studentized"]  # fmt: skip
    assert lo == -math.inf and hi > 0.0


def test_zero_total_in_b_studentized_raises_others_do_not():
    a, b = np.array([10.0, 20.0, 5.0, 15.0]), np.zeros(4)
    assert ratio_intervals(a, b, np.random.default_rng(1), resamples=500,
                           methods=("percentile",))["percentile"] == (1.0, 1.0)  # fmt: skip
    with pytest.raises(ValueError, match="zero total in arm B"):
        ratio_intervals(a, b, np.random.default_rng(1), resamples=500, methods=("studentized",))


@pytest.mark.parametrize("method", RATIO_METHODS)
def test_b_proportional_to_a_is_a_point_interval(method):
    rng = np.random.default_rng(8)
    a = rng.uniform(1, 100, size=80)
    lo, hi = ratio_intervals(a, 0.7 * a, np.random.default_rng(2), resamples=1000,
                             methods=(method,))[method]  # fmt: skip
    assert lo == pytest.approx(0.30, abs=1e-9) and hi == pytest.approx(0.30, abs=1e-9)


# ---- the public function and its failure modes ------------------------------------------------


def test_default_method_is_one_of_the_methods_and_is_recorded(tmp_path):
    assert DEFAULT_RATIO_METHOD in RATIO_METHODS
    a, b = skewed_sample(n=40)
    p = pair(tmp_path, a, b)
    r = ratio_of_totals_ci(p, "bill_compute", seed=1, resamples=300)
    assert r.method == DEFAULT_RATIO_METHOD
    for m in RATIO_METHODS:
        assert ratio_of_totals_ci(p, "bill_compute", seed=1, resamples=300, method=m).method == m


def test_unknown_method_and_bad_arguments_are_refused(tmp_path):
    p = pair(tmp_path, [1, 2, 3], [1, 2, 2])
    with pytest.raises(ValueError, match="method"):
        ratio_of_totals_ci(p, "bill_compute", seed=1, method="basic")
    for m in RATIO_METHODS:
        with pytest.raises(ValueError):
            ratio_of_totals_ci(p, "bill_compute", seed=1, confidence=1.0, method=m)
        with pytest.raises(ValueError, match="negative"):
            ratio_of_totals_ci(pair(tmp_path / m, [1, 2, 3], [1, -2, 3]), "bill_compute", seed=1,
                               method=m)  # fmt: skip
        with pytest.raises(ValueError, match="zero"):
            ratio_of_totals_ci(pair(tmp_path / m / "z", [0, 0, 0], [1, 2, 3]), "bill_compute",
                               seed=1, method=m)  # fmt: skip


@pytest.mark.parametrize("method", RATIO_METHODS)
def test_reproducible_and_seed_sensitive(tmp_path, method):
    a, b = skewed_sample(n=40)
    p = pair(tmp_path, a, b)
    r1 = ratio_of_totals_ci(p, "bill_compute", seed=7, resamples=1000, method=method)
    r2 = ratio_of_totals_ci(p, "bill_compute", seed=7, resamples=1000, method=method)
    r3 = ratio_of_totals_ci(p, "bill_compute", seed=8, resamples=1000, method=method)
    assert r1 == r2 and (r1.low, r1.high) != (r3.low, r3.high) and r1.savings == r3.savings


def test_all_methods_bracket_the_estimate_on_a_moderate_sample(tmp_path):
    a, b = skewed_sample(n=200, sigma_a=0.5, sigma_r=0.25)
    p = pair(tmp_path, a, b)
    for m in RATIO_METHODS:
        r = ratio_of_totals_ci(p, "bill_compute", seed=3, resamples=2000, method=m)
        assert r.low < r.savings < r.high


# ---- the command line -------------------------------------------------------------------------


def cli_run(capsys, *argv):
    rc = main(["compare", *argv])
    out = capsys.readouterr()
    return rc, out.out, out.err


def test_cli_interval_method_selects_and_reports_the_method(capsys, tmp_path):
    a, b = skewed_sample(n=60)
    ra = write_run(tmp_path / "a", [{"seed": i + 1, "modelled_component_ns": int(v * 1000)}
                                    for i, v in enumerate(a)])  # fmt: skip
    rb = write_run(tmp_path / "b", [{"seed": i + 1, "modelled_component_ns": int(v * 1000)}
                                    for i, v in enumerate(b)])  # fmt: skip
    base = ["--a", str(ra), "--b", str(rb), "--relative-savings", "--threshold", "0.2",
            "--seed", "1", "--resamples", "2000"]  # fmt: skip
    limits = {}
    for m, label in [("percentile", "percentile"), ("bca", "BCa"), ("studentized", "studentized")]:
        j = tmp_path / f"{m}.json"
        rc, out, _ = cli_run(capsys, *base, "--interval-method", m, "--json", str(j))
        assert rc == 0 and f"interval ({label}" in out
        boot = json.loads(j.read_text())["bootstrap"]
        assert boot["method"] == m
        limits[m] = (boot["low"], boot["high"])
    assert len(set(limits.values())) == 3  # three different intervals from the same resamples
    # omitted: the default method
    j = tmp_path / "d.json"
    rc, out, _ = cli_run(capsys, *base, "--json", str(j))
    assert rc == 0 and json.loads(j.read_text())["bootstrap"]["method"] == DEFAULT_RATIO_METHOD


def test_cli_refuses_a_bad_or_misplaced_interval_method(capsys, fixtures_dir):
    base = ["--a", str(fixtures_dir / "run_a"), "--b", str(fixtures_dir / "run_b"), "--seed", "1"]
    for extra in (
        ["--relative-savings", "--threshold", "0.2", "--interval-method", "basic"],  # not a choice
        ["--metric", "success", "--margin", "0.01", "--higher-is-better",
         "--interval-method", "bca"],  # outside --relative-savings
    ):  # fmt: skip
        with pytest.raises(SystemExit) as e:
            main(["compare", *base, *extra])
        assert e.value.code == 2
    err = capsys.readouterr().err
    assert "invalid choice" in err and "applies only with --relative-savings" in err
