"""R9's arithmetic: accuracy by level, the exponential fit, the cluster bootstrap, the reader
precondition, the misfit, and the values the rerun uses.

Exploration (nothing here tests a hypothesis). The criterion is the plan's
(`docs/local-test-plan.md`, 5R, R9, "Criterion, fixed by the coordinator before any R9 code or run
(2026-10-06)"). These readings are written here, in the commit that freezes the reader and before
any number is computed on seeds 30000 and above, and are not changed after it.

The plan's definitions, unchanged:

* A(m) is the share of correct answers at level m in {0, 50, 100, 200, 400}; the control is q = 0 at
  m = 50 and its share correct estimates p0.
* delta* is the least-squares fit of A(m) = p0 + (A(0) - p0) exp(-delta m / 100) with p0 the control's
  estimate; its 90% interval is a cluster bootstrap over streams (10,000 resamples, seed 9900),
  refitting in every resample. The misfit is the largest absolute residual over the five levels.
* **Reader precondition:** on hard evaluation questions A(0) >= 0.80 with its 90% lower bound above
  0.70. If it fails the outcome is "reader too weak", delta* is reported as labelled, and the rerun
  still happens but decides nothing.

Readings the plan leaves open (the fit is R8's `r8_stats`, reused unchanged with the seed 9900):

* **Clusters.** One question per stream, so a cluster is a question with all its levels and its
  control; the resample draws questions with replacement.
* **Amplitude and p0 in the fit, the grid, the intervals, complete cases:** exactly R8's readings
  (`r8_stats` docstring): the fit's A(0) is the observed level-0 share, p0 is re-estimated in every
  resample, delta is searched on a grid of [-0.5, 3.0] with step 0.0005 with no sign constraint, an
  interval is the 5th and 95th percentile of the resampled delta, resamples whose amplitude A(0) - p0
  is not above 0.02 have no delta and are counted.
* **A(0)'s lower bound** is the 5th percentile of the resampled A(0).
* **The misfit** is computed from the point estimates: the fitted curve at the five levels
  (p0, the observed A(0) and delta*), the residual `A(m) - fit(m)` at each level (zero at m = 0 by
  construction, since the fit takes the observed A(0)), and the largest absolute value. The misfit
  of the free-amplitude fit is reported beside it.
* **A delta that is not a number** (no resample identified one) is reported as such and no rerun value
  is derived.

The values the rerun uses (R7's comparison at delta*_lo, delta*, delta*_hi):

* delta*_lo and delta*_hi are the 5th and 95th percentiles; delta* is the point estimate.
* **The reasoner's penalty is clamped at 0** (`ReasonerSpec::normalized`): a value below 0 is run as
  0. At 0 the comparison is R6's own held-out result, which R7's pipeline already put through its
  procedure (`r7_criterion.delta0_reference`); it is used as the delta = 0 row and not re-run (a value
  within 0.005 of 0 counts as 0, as one within 0.005 of a grid value counts as that value), and the
  report says so. Several of the three values can then coincide at 0.
* **Reuse.** A value within 0.005 of a value of R7's grid (0.05, 0.1, 0.2, 0.4) is run as that grid
  value and R7's tuning and held-out runs are reused and named. The rest are run at the value rounded
  to 4 decimals. Any two of the three values within 0.005 of each other are one run.
* Everything else in the rerun is R7's: manifest writer, streams, selection rule, statistics.

Outcome of the whole of R9 (the plan's three): R6 regime if, at delta*_hi, the 90% upper bound of G
is below 0.10; R7 regime if, at delta*_lo, G >= 0.10 with its 90% lower bound above 0.05; unresolved
otherwise. With a failed precondition the outcome is "reader too weak".
"""

import numpy as np

import r8_stats as S8

LEVELS = S8.LEVELS
SEED = 9900
B = 10_000
A0_FLOOR = 0.80
A0_LOWER_FLOOR = 0.70
R6_G_UPPER = 0.10
R7_G_AT_LEAST = 0.10
R7_G_LOWER = 0.05
R7_GRID = (0.05, 0.1, 0.2, 0.4)
REUSE_TOL = 0.005


def reader_precondition(a0, a0_lower):
    return {
        "A0": float(a0),
        "A0_lower": float(a0_lower),
        "A0_at_least_0.80": bool(a0 >= A0_FLOOR),
        "lower_above_0.70": bool(a0_lower > A0_LOWER_FLOOR),
        "holds": bool(a0 >= A0_FLOOR and a0_lower > A0_LOWER_FLOOR),
    }


def fitted_curve(a0, p0, delta, levels=LEVELS):
    return [float(p0 + (a0 - p0) * np.exp(-delta * m / 100.0)) for m in levels]


def misfit(a, p0, delta):
    """Residuals `A(m) - fit(m)` at the five levels and the largest absolute one."""
    fit = fitted_curve(a[0], p0, delta)
    res = [float(x - f) for x, f in zip(a, fit)]
    return res, float(max(abs(r) for r in res))


def analyse(correct, b=B, seed=SEED):
    """R8's estimate for one question set with R9's seed, precondition and misfit. `correct` is
    (questions, 6): the five levels then the control, 0 or 1."""
    out = S8.analyse(correct, b=b, seed=seed)
    out["precondition"] = reader_precondition(out["A"][0], out["A0_lower"])
    out.pop("outcome", None)
    d = out["delta"]
    if np.isnan(d):
        out["residuals"], out["misfit"] = None, float("nan")
    else:
        out["residuals"], out["misfit"] = misfit(out["A"], out["p0"], d)
    fd = out["fit_free_amplitude"]
    p0 = out["p0"]
    out["fit_free_amplitude"]["misfit"] = float(
        max(abs(x - (p0 + fd["amplitude"] * np.exp(-fd["delta"] * m / 100.0))) for x, m in zip(out["A"], LEVELS))
    )
    return out


def rerun_values(lo, point, hi):
    """The values R7's comparison is rerun at, from delta*'s interval, by the readings above. Returns
    a list of dicts in the order lo, point, hi with the raw value, the clamped one, and what the run is:
    `{"which", "raw", "run_delta", "source"}` where source is "R6 files (delta 0)", "R7 run reused"
    or "new run"; values that fall together are one run (`same_as`)."""
    rows = []
    for which, raw in (("lo", lo), ("point", point), ("hi", hi)):
        d = max(0.0, float(raw))
        grid = [g for g in R7_GRID if abs(d - g) <= REUSE_TOL]
        if d <= REUSE_TOL:
            rows.append({"which": which, "raw": float(raw), "run_delta": 0.0, "source": "R6 files (delta 0)"})
        elif grid:
            rows.append({"which": which, "raw": float(raw), "run_delta": grid[0], "source": "R7 run reused"})
        else:
            rows.append({"which": which, "raw": float(raw), "run_delta": round(d, 4), "source": "new run"})
    # values that name the same run are one run: the same grid value or delta 0, or two new values
    # within 0.005 of each other
    seen = []
    for r in rows:
        same = next((f for f in seen if f["source"] == r["source"]
                     and (r["source"] != "new run" or abs(f["run_delta"] - r["run_delta"]) <= REUSE_TOL)
                     and (r["source"] == "new run" or f["run_delta"] == r["run_delta"])), None)
        if same is None:
            seen.append(r)
        else:
            r["same_as"] = same["which"]
            r["run_delta"] = same["run_delta"]
    return rows


def outcome(precondition_holds, g_lo_row, g_hi_row):
    """The plan's regimes from G at delta*_lo (`g_lo_row`) and at delta*_hi (`g_hi_row`), dicts with
    `G`, `lo` and `hi` (the 90% interval)."""
    if not precondition_holds:
        return "reader too weak"
    r6 = bool(g_hi_row["hi"] < R6_G_UPPER)
    r7 = bool(g_lo_row["G"] >= R7_G_AT_LEAST and g_lo_row["lo"] > R7_G_LOWER)
    if r6 and r7:
        return "both conditions hold (impossible unless delta*_lo >= delta*_hi): recheck"
    if r6:
        return "R6 regime"
    if r7:
        return "R7 regime"
    return "unresolved"


def selftest():
    rng = np.random.default_rng(1)
    # a known delta is recovered with a reader that has clear amplitude
    for d in (0.0, 0.05, 0.2):
        n = 400
        levels = [0.95 * np.exp(-d * m / 100) + 0.02 * (1 - np.exp(-d * m / 100)) for m in LEVELS]
        cols = [rng.random(n) < p for p in levels] + [rng.random(n) < 0.02]
        c = np.array(cols).T.astype(int)
        r = analyse(c, b=1000)
        assert r["delta_interval"][0] - 0.02 <= d <= r["delta_interval"][1] + 0.02, (d, r["delta_interval"])
        assert r["precondition"]["holds"]
    # the precondition fails for a weak reader
    c = (rng.random((300, 6)) < 0.5).astype(int)
    assert not analyse(c, b=500)["precondition"]["holds"]
    # the rerun's values: clamping, reuse, one run for values that fall together
    rv = rerun_values(-0.02, 0.01, 0.03)
    assert [r["run_delta"] for r in rv] == [0.0, 0.01, 0.03] and rv[0]["source"] == "R6 files (delta 0)"
    rv = rerun_values(0.046, 0.052, 0.07)
    assert [r["source"] for r in rv] == ["R7 run reused", "R7 run reused", "new run"]
    assert rv[0]["run_delta"] == 0.05 and rv[1]["run_delta"] == 0.05
    rv = rerun_values(-0.05, -0.01, 0.002)
    assert all(r["source"] == "R6 files (delta 0)" for r in rv)
    # the three regimes and the failed precondition are each reachable
    lo, hi = {"G": 0.05, "lo": 0.02, "hi": 0.09}, {"G": 0.06, "lo": 0.03, "hi": 0.09}
    assert outcome(True, lo, hi) == "R6 regime"
    assert outcome(True, {"G": 0.15, "lo": 0.1, "hi": 0.2}, {"G": 0.3, "lo": 0.2, "hi": 0.4}) == "R7 regime"
    assert outcome(True, lo, {"G": 0.12, "lo": 0.08, "hi": 0.16}) == "unresolved"
    assert outcome(False, lo, hi) == "reader too weak"
    print("r9_stats selftest: ok")


if __name__ == "__main__":
    selftest()
