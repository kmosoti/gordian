"""L1 arithmetic: window shares, paired window differences, slopes, curves, with the cluster
bootstrap of B1 and B2 (whole streams resampled, the same counts for every arm and measure, so that
differences are paired), in one place so that no script decides anything twice.

Definitions are `l1_common`'s readings R1 to R5.
"""

import numpy as np
import pandas as pd

import l1_common as C
import b2_stats as B
import r6_stats as S  # noqa: F401  (b2_stats puts the exploration scripts on the path)
from gordian_analysis.measures import outcome_slope, sample_efficiency

NONLEAK = "hard non-leak"


def interval(x):
    """The 90% equal-tailed percentile interval of a resample distribution, NaNs dropped."""
    x = np.asarray(x, float)
    x = x[~np.isnan(x)]
    if not len(x):
        return float("nan"), float("nan")
    return float(np.percentile(x, 5)), float(np.percentile(x, 95))


def counts(rng, n, m):
    return rng.multinomial(n, np.full(n, 1.0 / n), size=m).astype(float)


class Window:
    """Per-stream numerators and denominators of the measures over a window of streams (a slice of
    `Measures` columns), for point estimates and paired cluster bootstrap."""

    def __init__(self, measures, lo, hi):
        self.m = measures
        self.lo, self.hi = lo, hi
        self.n = hi - lo

    def col(self, name, arm):
        return self.m.col[name][self.m.row[arm], self.lo:self.hi]

    def ratio(self, num, den, arm, w=None):
        n_, d_ = self.col(num, arm), self.col(den, arm)
        if w is None:
            w = np.ones(self.n)
        d = float(w @ d_)
        return float(w @ n_) / d if d > 0 else float("nan")

    def mean(self, num, arm):
        return float(self.col(num, arm).mean())

    def boot_ratio(self, num, den, arm, resamples=C.N_RESAMPLES, seed=C.BOOT_SEED):
        rng = np.random.default_rng(seed)
        w = counts(rng, self.n, resamples)
        n_, d_ = self.col(num, arm), self.col(den, arm)
        with np.errstate(divide="ignore", invalid="ignore"):
            r = (w @ n_) / (w @ d_)
        return interval(r)

    def paired(self, num, den, a, b, resamples=C.N_RESAMPLES, seed=C.BOOT_SEED):
        """(point, lower, higher) of ratio(a) - ratio(b), paired over streams."""
        rng = np.random.default_rng(seed)
        w = counts(rng, self.n, resamples)
        with np.errstate(divide="ignore", invalid="ignore"):
            ra = (w @ self.col(num, a)) / (w @ self.col(den, a))
            rb = (w @ self.col(num, b)) / (w @ self.col(den, b))
        lo, hi = interval(ra - rb)
        return self.ratio(num, den, a) - self.ratio(num, den, b), lo, hi

    def boot_mean(self, num, arm, resamples=C.N_RESAMPLES, seed=C.BOOT_SEED):
        rng = np.random.default_rng(seed)
        w = counts(rng, self.n, resamples) / self.n
        return interval(w @ self.col(num, arm))


# ---- slopes ---------------------------------------------------------------------------------------


def incident_outcomes(arm, kind):
    """Per-incident outcomes of an arm in the order seen (seed, then incident id): a frame with
    `stream` (the seed) and `y` (anchor-correct for hard non-leak incidents; noticed for the slow
    leak)."""
    ni = arm.notice_incidents
    leak = ni["family"] == "slow_leak"
    if kind == "anchor":
        sel = (ni["tier"] == "hard") & ~leak
        y = ni["anchor_correct"]
    elif kind == "leak":
        sel = (ni["tier"] == "hard") & leak
        y = ni["noticed"]
    else:
        raise ValueError(kind)
    t = ni[sel].assign(y=y[sel].astype(float)).sort_values(["seed", "incident"])
    return t.rename(columns={"seed": "stream"})[["stream", "y"]].reset_index(drop=True)


def slope(arm, kind, seeds, resamples=C.N_RESAMPLES, seed=C.BOOT_SEED):
    """(point, lower, higher, incidents) of the slope of the outcome against the position of the
    incident, in share per 100 incidents seen, over the streams `seeds` (the first K of a run),
    whole streams resampled; W1's module's `outcome_slope`."""
    r = outcome_slope(incident_outcomes(arm, kind), list(seeds), resamples=resamples, seed=seed)
    return r.slope_per_100, r.lower, r.higher, r.incidents


# ---- the slope of W1's curve (clause 2, reading R3) -----------------------------------------------


def curve_slope(measures, arm, which, k, resamples=C.N_RESAMPLES, seed=C.BOOT_SEED):
    """(point, lower, higher) of the least-squares slope of the cumulative efficiency curve against
    the number of streams over the first `k` streams, in efficiency per 100 streams. The curve is
    W1's `sample_efficiency` on the same per-stream counts; the interval resamples whole streams
    with replacement in place (multiplicities in stream order) and recomputes the weighted
    cumulative ratio. Points with no incident seen yet are left out."""
    f = per_stream_frame(measures, arm, which).iloc[:k]
    inc = f["incidents_hard"].to_numpy(float)
    cor = f["correct_hard"].to_numpy(float)
    x = np.arange(1, k + 1, dtype=float)

    def slopes(w):
        cd = np.cumsum(w * inc, axis=1)
        cc = np.cumsum(w * cor, axis=1)
        with np.errstate(divide="ignore", invalid="ignore"):
            eff = np.where(cd > 0, cc / cd, np.nan)
        ok = ~np.isnan(eff)
        n = ok.sum(axis=1)
        sx = (ok * x).sum(axis=1)
        sxx = (ok * x * x).sum(axis=1)
        e = np.where(ok, eff, 0.0)
        sy = e.sum(axis=1)
        sxy = (e * x).sum(axis=1)
        with np.errstate(divide="ignore", invalid="ignore"):
            var = sxx - sx * sx / n
            cov = sxy - sx * sy / n
            return np.where((n > 1) & (var > 0), cov / var, np.nan)

    point = slopes(np.ones((1, k)))[0]
    # W1's own curve gives the same point (checked by the caller's test of `curve`)
    rng = np.random.default_rng(seed)
    lo, hi = interval(slopes(counts(rng, k, resamples)))
    return point * 100, lo * 100, hi * 100


# ---- curves ---------------------------------------------------------------------------------------


def per_stream_frame(measures, arm, which):
    """W1's input for an arm: `seed`, `incidents_hard`, `correct_hard`. `which`: `anchor` (hard
    non-leak incidents seen, anchor-correct), `leak` (leaks seen, noticed), `all_hard` (both)."""
    i = measures.row[arm]
    col = measures.col
    if which == "anchor":
        inc, cor = col["hard_n"][i], col["hard_correct"][i]
    elif which == "leak":
        inc, cor = col["leak_n"][i], col["leak_noticed"][i]
    elif which == "all_hard":
        inc = col["hard_n"][i] + col["leak_n"][i]
        cor = col["hard_correct"][i] + col["leak_noticed"][i]
    else:
        raise ValueError(which)
    return pd.DataFrame({
        "seed": measures.seeds, "incidents_hard": inc.astype(int), "correct_hard": cor.astype(int),
    })


def curve(measures, arm, which):
    return sample_efficiency(per_stream_frame(measures, arm, which), tier="hard").curve
