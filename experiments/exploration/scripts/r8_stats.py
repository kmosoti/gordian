"""R8's arithmetic: accuracy by level, the exponential fit, the cluster bootstrap and the criterion.

Exploration (nothing here tests a hypothesis). The criterion is the plan's
(`docs/local-test-plan.md`, 5R, R8, "Criterion, fixed by the coordinator before any R8 code or call
(2026-10-05)"). It is written here, with the readings below, before the first scored call, and is
not changed after it.

The plan's definitions, unchanged:

* A(m) is the share of correct answers at level m in {0, 50, 100, 200, 400}; the control is q = 0 at
  m = 50 and its share correct estimates p0.
* The fit is A(m) = p0 + (A(0) - p0) * exp(-delta * m / 100) by least squares over the five levels,
  p0 fixed at the control's estimate. delta-hat gets a 90% cluster bootstrap over incidents
  (10,000 resamples, seed 9800), refitting in every resample.
* Precondition (identifiability): on hard incidents A(0) >= 0.35 and its 90% lower bound above the
  control's estimate plus 0.15. If it fails: unidentifiable with this model.
* R6 regime: the 90% upper bound of delta-hat below 0.05. R7 regime: the 90% lower bound above
  0.10. Unresolved: anything else, including an interval that straddles 0.05 to 0.10.

Readings the plan leaves open, fixed here before any scored call:

* **Amplitude.** `A(0)` in the fit is the observed level-0 share of the sample being fitted (the
  plan's formula names A(0), not a fitted amplitude), so delta is the only free parameter and the
  m = 0 level has no residual; the least squares is over the four levels m > 0. A fit with a free
  amplitude is reported beside the criterion (`fit_free_amplitude`), never in it.
* **p0 in the resamples.** Each resample draws incidents with their control calls, so p0 is
  re-estimated in every resample and "refitting" includes it; the variant that holds p0 at the full
  sample's value is reported beside the criterion.
* **The least squares** is solved on a grid of delta in [-0.5, 3.0] with step 0.0005 (no
  constraint that delta is non-negative: a model that improves with distractors gets a negative
  delta); a resample whose amplitude A(0) - p0 is not above 0.02 has no identified delta (the
  curve is flat in delta) and is counted and left out of the interval. A fit at the grid's edge is
  counted.
* **Intervals** are the 5th and 95th percentiles of the resampled statistic (method `lower` /
  `higher`, as R6 and R7), over whole incidents (a question with all its levels and its control).
  "The 90% lower bound of A(0)" is the 5th percentile. The precondition compares it with the
  point estimate of p0 plus 0.15.
* **Complete cases.** The paired analysis uses incidents that have a call at every level and the
  control; calls of any other incident are counted and reported and stay in the call file.
* **An answer that fails to parse, times out or errors is wrong** and stays in the denominator.
"""

import numpy as np

LEVELS = (0, 50, 100, 200, 400)
SEED = 9800
B = 10_000
GRID = np.arange(-0.5, 3.0 + 1e-9, 0.0005)
MIN_AMPLITUDE = 0.02
A0_FLOOR = 0.35
P0_MARGIN = 0.15
R6_BELOW = 0.05
R7_ABOVE = 0.10


def accuracy_matrix(correct):
    """`correct`: array (incidents, 6) of 0/1 over the levels 0, 50, 100, 200, 400 and the control."""
    return np.asarray(correct, dtype=float)


def levels_from(means):
    """`means`: (..., 6) -> A at the five levels and p0."""
    return means[..., :5], means[..., 5]


def fit_delta(a, p0, grid=GRID):
    """Least-squares delta of A(m) = p0 + (A(0) - p0) exp(-delta m / 100) over m in {50, 100, 200,
    400}, for `a` of shape (..., 5) and `p0` of shape (...). Returns (delta, status) with status 0
    for a fit, 1 for a flat curve (amplitude not above `MIN_AMPLITUDE`), 2 for a fit at the grid's
    edge."""
    a = np.asarray(a, dtype=float)
    p0 = np.asarray(p0, dtype=float)
    amp = a[..., 0] - p0
    m = np.asarray(LEVELS[1:], dtype=float)
    shape = amp.shape
    amp_f = amp.reshape(-1)
    a_f = a.reshape(-1, 5)[:, 1:]
    p_f = p0.reshape(-1)
    out = np.full(amp_f.shape, np.nan)
    status = np.zeros(amp_f.shape, dtype=int)
    chunk = 250
    for lo in range(0, len(amp_f), chunk):
        sl = slice(lo, min(lo + chunk, len(amp_f)))
        # curve[i, g, k] = p0_i + amp_i * exp(-grid_g * m_k / 100)
        curve = p_f[sl, None, None] + amp_f[sl, None, None] * np.exp(
            -grid[None, :, None] * m[None, None, :] / 100.0
        )
        sse = ((curve - a_f[sl, None, :]) ** 2).sum(axis=2)
        best = sse.argmin(axis=1)
        out[sl] = grid[best]
        edge = (best == 0) | (best == len(grid) - 1)
        status[sl] = np.where(edge, 2, 0)
    flat = amp_f <= MIN_AMPLITUDE
    out[flat] = np.nan
    status[flat] = 1
    return out.reshape(shape), status.reshape(shape)


def fit_free_amplitude(a, p0, grid=GRID):
    """Beside the criterion: least squares over all five levels with the amplitude free as well
    (delta on the grid, the amplitude solved in closed form for each delta). Returns
    (delta, amplitude) for one sample `a` of shape (5,)."""
    a = np.asarray(a, dtype=float)
    m = np.asarray(LEVELS, dtype=float)
    e = np.exp(-grid[:, None] * m[None, :] / 100.0)  # (grid, 5)
    y = a - p0
    amp = (e * y).sum(axis=1) / (e * e).sum(axis=1)
    sse = ((amp[:, None] * e - y) ** 2).sum(axis=1)
    i = int(sse.argmin())
    return float(grid[i]), float(amp[i] + 0.0)


def bootstrap_indices(n, b=B, seed=SEED):
    return np.random.default_rng(seed).integers(0, n, size=(b, n))


def percentile_interval(x, level=0.90):
    x = np.asarray(x, dtype=float)
    x = x[~np.isnan(x)]
    if len(x) == 0:
        return (float("nan"), float("nan"))
    lo = np.quantile(x, (1 - level) / 2, method="lower")
    hi = np.quantile(x, 1 - (1 - level) / 2, method="higher")
    return (float(lo), float(hi))


def analyse(correct, b=B, seed=SEED, grid=GRID):
    """The whole estimate for one question set. `correct`: (incidents, 6) of 0/1: the five levels
    then the control. Returns a dict with the point estimates, the intervals and the counts."""
    c = accuracy_matrix(correct)
    n = c.shape[0]
    means = c.mean(axis=0)
    a_hat, p0_hat = levels_from(means)
    d_hat, d_status = fit_delta(a_hat, p0_hat, grid)
    idx = bootstrap_indices(n, b, seed)
    boot = c[idx].mean(axis=1)  # (b, 6)
    a_b, p0_b = levels_from(boot)
    d_b, st_b = fit_delta(a_b, p0_b, grid)
    d_fixed, st_fixed = fit_delta(a_b, np.full(b, p0_hat), grid)
    a0_lo, a0_hi = percentile_interval(a_b[:, 0])
    out = {
        "n_incidents": int(n),
        "A": [float(x) for x in a_hat],
        "A_interval": [list(percentile_interval(a_b[:, k])) for k in range(5)],
        "p0": float(p0_hat),
        "p0_interval": list(percentile_interval(p0_b)),
        "delta": float(d_hat),
        "delta_status": int(d_status),
        "delta_interval": list(percentile_interval(d_b)),
        "delta_unidentified_resamples": int((st_b == 1).sum()),
        "delta_edge_resamples": int((st_b == 2).sum()),
        "delta_interval_p0_fixed": list(percentile_interval(d_fixed)),
        "delta_unidentified_resamples_p0_fixed": int((st_fixed == 1).sum()),
        "A0_lower": a0_lo,
        "A0_minus_p0_interval": list(percentile_interval(a_b[:, 0] - p0_b)),
        "bootstrap": {"resamples": int(b), "seed": int(seed)},
    }
    fd, fa = fit_free_amplitude(a_hat, p0_hat, grid)
    out["fit_free_amplitude"] = {"delta": fd, "amplitude": fa}
    out["precondition"] = precondition(a_hat[0], a0_lo, p0_hat)
    out["outcome"] = outcome(out["precondition"]["holds"], *out["delta_interval"])
    return out


def precondition(a0, a0_lower, p0):
    return {
        "A0": float(a0),
        "A0_lower": float(a0_lower),
        "p0": float(p0),
        "A0_at_least_floor": bool(a0 >= A0_FLOOR),
        "lower_above_p0_plus_margin": bool(a0_lower > p0 + P0_MARGIN),
        "holds": bool(a0 >= A0_FLOOR and a0_lower > p0 + P0_MARGIN),
    }


def outcome(precondition_holds, lo, hi):
    """The plan's outcomes. A model that fails the precondition is unidentifiable (and the plan
    needs both models to fail before it says "with these models")."""
    if not precondition_holds:
        return "unidentifiable (this model)"
    if np.isnan(lo) or np.isnan(hi):
        return "unresolved (no interval)"
    if hi < R6_BELOW:
        return "R6 regime"
    if lo > R7_ABOVE:
        return "R7 regime"
    return "unresolved"
