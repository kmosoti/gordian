"""B2 arithmetic: the notice measures (B1's and the site check and precision of N13 to N16), and
the selection oracle's quality and cost per arm, pooled over streams, with 90% cluster-bootstrap
intervals, in one place so that no script decides anything twice.

Exploration (nothing here tests a hypothesis). Definitions are B1's (`b1_stats.py`) with these
additions, all pooled as ratios of sums over streams and never as means of per-stream ratios:

* **site-correct share**: hard incidents outside the slow-leak family with at least one notice
  about their site (N14); **anchor-and-site-correct share**: with one notice that is both
  anchor-correct and about their site (N15); and the same two for the slow leak.
* **notice precision**: notices anchored on an incident of any tier over all notices (N16);
  **precision in a tier**: notices anchored on that tier over all notices (the three add to the
  precision); **strict precision**: notices that are anchor-correct and site-correct over all
  notices; **site-correct notice share**: notices that are site-correct over all notices.
* quality, cost and the notice measures of an arm are read from that arm's own files. The table's
  noticing columns are read from the selection oracle at the fixed delay with the rung's retirement
  (B1's arms); `quality_held` is read from the arm with `hold_until_asked`, and the notice
  measures of that arm are the record of that run (the hold can change what is noticed later at a
  site, `oracle.rs`), reported, never mixed into the table's noticing columns.
* intervals: 90% equal-tailed percentile intervals over resamples of whole streams (5th percentile
  `lower`, 95th `higher`), multinomial stream counts from `numpy.random.default_rng(seed)` in chunks
  of 500, the same counts for every arm and every measure, so that a difference between arms is
  paired.
"""

import numpy as np

import b2_common  # noqa: F401  (puts the exploration scripts and this worktree's analysis on the path)
import r6_stats as S
from gordian_analysis.stream import notice_per_stream

# measure name -> (numerator column, denominator column, scale)
MEASURES = {
    "hard_noticed_share": ("hard_noticed", "hard_n", 1.0),
    "hard_anchor_correct_share": ("hard_correct", "hard_n", 1.0),
    "hard_site_correct_share": ("hard_site", "hard_n", 1.0),
    "hard_anchor_site_correct_share": ("hard_both", "hard_n", 1.0),
    "leak_noticed_share": ("leak_noticed", "leak_n", 1.0),
    "leak_anchor_correct_share": ("leak_correct", "leak_n", 1.0),
    "leak_site_correct_share": ("leak_site", "leak_n", 1.0),
    "leak_anchor_site_correct_share": ("leak_both", "leak_n", 1.0),
    "plain_noticed_share": ("plain_noticed", "plain_n", 1.0),
    "notices_on_background_per_stream": ("notices_background", "one", 1.0),
    "notices_on_plain_per_stream": ("notices_plain", "one", 1.0),
    "notices_on_hard_per_stream": ("notices_hard", "one", 1.0),
    "notices_on_decoy_per_stream": ("notices_decoy", "one", 1.0),
    "notices_per_stream": ("notices", "one", 1.0),
    "notices_per_incident": ("notices_on_incidents", "incidents", 1.0),
    "notice_precision": ("notices_on_incidents", "notices", 1.0),
    "precision_plain": ("notices_plain", "notices", 1.0),
    "precision_hard": ("notices_hard", "notices", 1.0),
    "precision_decoy": ("notices_decoy", "notices", 1.0),
    "strict_precision": ("notices_both", "notices", 1.0),
    "site_correct_notice_share": ("notices_site", "notices", 1.0),
    "quality": ("quality_num", "quality_den", 1.0),
    "leak_quality": ("leak_num", "leak_den", 1.0),
    "cost_s_per_stream": ("total_cost_ns", "one", 1e-9),
    "substrate_s_per_stream": ("substrate_ns", "one", 1e-9),
    "calls_per_stream": ("calls", "one", 1.0),
}


class Measures:
    """Per-arm, per-stream numerators and denominators of a run, for cluster-bootstrapping."""

    def __init__(self, run, arms=None):
        self.names = list(arms) if arms is not None else list(run.arms)
        self.row = {n: i for i, n in enumerate(self.names)}
        tabs = []
        for n in self.names:
            arm = run.arms[n]
            t = S.per_stream(arm).join(notice_per_stream(arm), how="inner")
            t["one"] = 1
            t["substrate_ns"] = arm.results.set_index("seed")["substrate_ns"]
            tabs.append(t)
        self.seeds = tabs[0].index.to_numpy()
        for t in tabs:
            assert (t.index.to_numpy() == self.seeds).all()
        self.n = len(self.seeds)
        cols = {c for num, den, _ in MEASURES.values() for c in (num, den)}
        self.col = {c: np.stack([t[c].to_numpy(float) for t in tabs]) for c in cols}

    def counts(self, rng, m):
        return rng.multinomial(self.n, np.full(self.n, 1.0 / self.n), size=m).astype(float)

    def _ratio(self, measure, w):
        num, den, scale = MEASURES[measure]
        n_, d_ = w @ self.col[num].T, w @ self.col[den].T
        with np.errstate(divide="ignore", invalid="ignore"):
            return np.where(d_ > 0, n_ / d_, np.nan) * scale  # (resamples, arms)

    def points(self, measures=None):
        """{measure: array over arms} for the sample itself."""
        w = np.ones((1, self.n))
        return {m: self._ratio(m, w)[0] for m in (measures or MEASURES)}

    def boot(self, seed, resamples=10_000, chunk=500, measures=None):
        """{measure: (lower, higher) arrays over arms}, 90% intervals, NaN resamples dropped."""
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
                    lo[a], hi[a] = S.interval(col)
            out[m] = (lo, hi)
        return out

    def paired_difference(self, measure, a, b, seed, resamples=10_000, chunk=500):
        """`(point, lower, higher)` of `measure` of arm `a` minus that of arm `b` over the same
        resamples of streams."""
        ia, ib = self.row[a], self.row[b]
        point = self.points([measure])[measure]
        rng = np.random.default_rng(seed)
        out, done = [], 0
        while done < resamples:
            k = min(chunk, resamples - done)
            r = self._ratio(measure, self.counts(rng, k))
            out.append(r[:, ia] - r[:, ib])
            done += k
        x = np.concatenate(out)
        x = x[~np.isnan(x)]
        lo, hi = S.interval(x)
        return float(point[ia] - point[ib]), lo, hi
