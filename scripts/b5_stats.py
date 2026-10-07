"""B5 arithmetic: B4's measures (B2's quality, cost and calls, the notice and selection measures) with the
verified-decision columns of R2 of `b5_common.py`, pooled over streams, with 90% cluster-bootstrap
intervals, in one place so that no script decides anything twice.

Exploration (nothing here tests a hypothesis). Definitions are B2's and B4's (`b2_stats.py`,
`b4_stats.py`) with these additions, all pooled as ratios of sums over streams:

* **verified decisions per stream**: the plain and hard incidents (hard including the slow-leak family)
  declared correctly by their deadline, `correct_plain + correct_hard` of `results.csv` (S6, S20), over
  streams; the plain and hard parts are reported beside. The cheap rung's own correct declarations are
  in it: `never_escalate` is the floor of every row, and a selector's value is its difference from it.
* **hard quality**: B2's, hard non-leak incidents declared correctly by their deadline over hard non-leak
  incidents; **leak quality** beside; **critical misses per stream**; **calls and cost per stream**.
* intervals: 90% equal-tailed percentile intervals over resamples of whole streams (5th percentile
  `lower`, 95th `higher`), multinomial stream counts from `numpy.random.default_rng(seed)` in chunks
  of 500, the same counts for every arm and every measure, so that a difference between arms is
  paired (B4's `Measures`, whose class this one extends; the only change is the frame it reads).
"""

import numpy as np
import pandas as pd

import b4_stats as B4

CLASSES = B4.CLASSES

MEASURES = dict(B4.MEASURES)
MEASURES.update({
    "verified_per_stream": ("verified", "one", 1.0),
    "verified_plain_per_stream": ("correct_plain", "one", 1.0),
    "verified_hard_per_stream": ("correct_hard", "one", 1.0),
    "hard_incidents_per_stream": ("incidents_hard", "one", 1.0),
    "plain_incidents_per_stream": ("incidents_plain", "one", 1.0),
})


def per_stream(arm):
    """B4's per-stream frame with the hard incidents' correct count and the verified decisions."""
    t = B4.per_stream(arm).copy()
    r = arm.results.set_index("seed").sort_index()
    assert (t.index.to_numpy() == r.index.to_numpy()).all()
    extra = {}
    for name in ("correct_hard", "incidents_hard", "correct_plain", "incidents_plain"):
        col = r[name].astype("int64")
        if name in t.columns:  # already there (R6's frame has some): checked equal, not added twice
            assert (t[name].to_numpy() == col.to_numpy()).all(), name
        else:
            extra[name] = col
    out = pd.concat([t, pd.DataFrame(extra, index=t.index)], axis=1)
    out["verified"] = out["correct_plain"] + out["correct_hard"]
    assert not out.columns.duplicated().any(), list(out.columns[out.columns.duplicated()])
    return out


class Measures(B4.Measures):
    """B4's `Measures` over B5's per-stream frame and measure set."""

    def __init__(self, run, arms=None):
        self.names = list(arms) if arms is not None else list(run.arms)
        self.row = {n: i for i, n in enumerate(self.names)}
        tabs = [per_stream(run.arms[n]) for n in self.names]
        self.seeds = tabs[0].index.to_numpy()
        for t in tabs:
            assert (t.index.to_numpy() == self.seeds).all()
        self.n = len(self.seeds)
        cols = {c for num, den, _ in MEASURES.values() for c in (num, den)}
        self.col = {c: np.stack([t[c].to_numpy(float) for t in tabs]) for c in cols}
        self.frames = dict(zip(self.names, tabs))

    def _ratio(self, measure, w):
        num, den, scale = MEASURES[measure]
        n_, d_ = w @ self.col[num].T, w @ self.col[den].T
        with np.errstate(divide="ignore", invalid="ignore"):
            return np.where(d_ > 0, n_ / d_, np.nan) * scale  # (resamples, arms)

    def points(self, measures=None):
        w = np.ones((1, self.n))
        return {m: self._ratio(m, w)[0] for m in (measures or MEASURES)}

    def boot(self, seed, resamples=10_000, chunk=500, measures=None):
        measures = list(measures or MEASURES)
        rng = np.random.default_rng(seed)
        draws = {m: [] for m in measures}
        done = 0
        while done < resamples:
            k = min(chunk, resamples - done)
            w = self.counts(rng, k)
            for m in measures:
                draws[m].append(self._ratio(m, w))
            done += k
        out = {}
        for m, parts in draws.items():
            x = np.concatenate(parts)
            lo, hi = np.full(x.shape[1], np.nan), np.full(x.shape[1], np.nan)
            for a in range(x.shape[1]):
                col = x[:, a]
                col = col[~np.isnan(col)]
                if len(col):
                    lo[a], hi[a] = B4.S.interval(col)
            out[m] = (lo, hi)
        return out

    def paired(self, measures, pairs, seed, resamples=10_000, chunk=500):
        rng = np.random.default_rng(seed)
        ms = sorted({m for m, _, _ in pairs})
        point = self.points(ms)
        draws = {p: [] for p in pairs}
        done = 0
        while done < resamples:
            k = min(chunk, resamples - done)
            w = self.counts(rng, k)
            cache = {m: self._ratio(m, w) for m in ms}
            for (m, a, b) in pairs:
                draws[(m, a, b)].append(cache[m][:, self.row[a]] - cache[m][:, self.row[b]])
            done += k
        out = {}
        for (m, a, b), parts in draws.items():
            x = np.concatenate(parts)
            x = x[~np.isnan(x)]
            lo, hi = B4.S.interval(x) if len(x) else (float("nan"), float("nan"))
            out[(m, a, b)] = (float(point[m][self.row[a]] - point[m][self.row[b]]), lo, hi)
        return out


def tuning_counts(arm):
    """One arm's pooled counts on the tuning streams, for the tuning rule: verified decisions (total,
    plain, hard), critical misses, calls, cost, hard quality, over `streams` streams."""
    t = per_stream(arm)
    n = len(t)
    return {
        "streams": n,
        "verified": int(t["verified"].sum()),
        "verified_plain": int(t["correct_plain"].sum()),
        "verified_hard": int(t["correct_hard"].sum()),
        "critical_misses": int(t["cmiss"].sum()),
        "calls": int(t["calls"].sum()),
        "cost_ns": int(t["total_cost_ns"].sum()),
        "quality_num": int(t["quality_num"].sum()),
        "quality_den": int(t["quality_den"].sum()),
    }
