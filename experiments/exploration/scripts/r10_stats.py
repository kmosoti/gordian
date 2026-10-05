"""R10 arithmetic: per-arm points, and the paired cluster bootstrap of a difference of pooled rates,
in one place so that no script decides anything twice.

Exploration (nothing here tests a hypothesis). Definitions, as R4 to R7 fixed them and R10 reuses:

* **hard-incident quality** (`quality`): the pooled fraction (ratio of sums over streams) of hard
  incidents outside the slow-leak family declared correctly by their deadline.
* **slow-leak quality** (`leak`): the same ratio over the slow-leak incidents alone.
* plain accuracy, critical misses, false alarms are computed beside them and never folded in.
* intervals: 90% equal-tailed percentile intervals (5th percentile `lower`, 95th `higher`, as
  `r6_stats.interval`) over resamples of whole streams (clusters). The resamples are multinomial
  stream counts drawn from `numpy.random.default_rng(seed)` in chunks of 500, as `r6_stats` draws
  them; a difference of two arms applies the same counts to both (paired), and every call with the
  same stream count and seed uses the same draws.
"""

import numpy as np

import r6_stats as S


class Streams(S.Streams):
    """`r6_stats.Streams` plus the slow-leak numerators and denominators and the plain ones."""

    def __init__(self, run):
        super().__init__(run)
        tabs = [S.per_stream(run.arms[n]) for n in self.names]
        stack = lambda col: np.stack([t[col].to_numpy(float) for t in tabs])  # noqa: E731
        self.ln, self.ld = stack("leak_num"), stack("leak_den")
        self.pn, self.pd_ = stack("correct_plain"), stack("incidents_plain")
        self.cm = stack("cmiss")
        self.table = {n: t for n, t in zip(self.names, tabs)}

    def ratio_diff(self, num, den, a, b, w):
        """Pooled `num/den` of arm `a` minus that of arm `b`, for each row of the count matrix `w`
        (streams resampled), or for the sample itself when `w` is None."""
        if w is None:
            w = np.ones((1, self.n))
        ia, ib = self.row[a], self.row[b]
        n_, d_ = w @ num.T, w @ den.T
        with np.errstate(divide="ignore", invalid="ignore"):
            r = np.where(d_ > 0, n_ / d_, np.nan)
        return r[:, ia] - r[:, ib]

    def paired(self, kind, a, b, seed, resamples=10_000, chunk=500):
        """`(point, lo, hi, nan_share)` of the quality (`kind` 'quality') or slow-leak quality
        (`kind` 'leak') of `a` minus that of `b`, cluster-bootstrapped over the streams."""
        num, den = (self.qn, self.qd) if kind == "quality" else (self.ln, self.ld)
        point = float(self.ratio_diff(num, den, a, b, None)[0])
        rng = np.random.default_rng(seed)
        out, done = [], 0
        while done < resamples:
            m = min(chunk, resamples - done)
            out.append(self.ratio_diff(num, den, a, b, self.counts(rng, m)))
            done += m
        x = np.concatenate(out)
        nan_share = float(np.mean(np.isnan(x)))
        x = x[~np.isnan(x)]
        lo, hi = S.interval(x)
        return point, lo, hi, nan_share


def point(arm):
    """`r6_stats.point` plus the per-stream means R10 reports."""
    p = S.point(arm)
    t = S.per_stream(arm)
    p["leak_correct"] = int(t["leak_num"].sum())
    p["leak_incidents"] = int(t["leak_den"].sum())
    p["leak_quality"] = S.pooled(t["leak_num"], t["leak_den"])
    p["critical_misses_hard"] = int(t["critical_missed_hard"].sum())
    p["critical_misses_plain"] = int(t["critical_missed_plain"].sum())
    p["decoys_alarmed_per_stream"] = float(t["decoys_alarmed"].mean())
    p["hard_incidents_escalated_per_stream"] = float(t["hard_incidents_escalated"].mean())
    return p
