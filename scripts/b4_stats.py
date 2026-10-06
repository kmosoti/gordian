"""B4 arithmetic: B2's measures (quality, cost, calls, the notice measures) and the selection measures
of the evaluator's E1 to E8, pooled over streams, with 90% cluster-bootstrap intervals, in one place so
that no script decides anything twice.

Exploration (nothing here tests a hypothesis). Definitions are B2's (`b2_stats.py`) with these additions,
all pooled as ratios of sums over streams and never as means of per-stream ratios:

* **plain accuracy**: plain incidents declared correctly by their deadline over plain incidents;
  **critical misses per stream** and **critical miss share** (critical incidents missed over critical
  incidents), as R5 reports them beside quality, never folded into it.
* **escalations on decoys, plain incidents, hard incidents, leaks, background per stream** (E2): the
  accepted calls by the class of their focus; **their reasoner cost share** (tokens of the class over
  tokens of all calls, E6) and the class's modelled seconds per stream.
* **notices on decoys per decoy**: the pooled count of notices anchored on decoys over the pooled count of
  decoys (N8, E7); **decoy notices escalated / retired before escalation / retired by the follow-up rule**
  as shares of the notices anchored on decoys, and the same for plain incidents and leaks (E3 to E5, E7).
* **leaks lost to the follow-up rule**: slow-leak incidents with a notice retired by the rule before
  escalation and no notice asked about, over slow-leak incidents; **decoys the rule retired a notice of**,
  over decoys.
* intervals: 90% equal-tailed percentile intervals over resamples of whole streams (5th percentile
  `lower`, 95th `higher`), multinomial stream counts from `numpy.random.default_rng(seed)` in chunks
  of 500, the same counts for every arm and every measure, so that a difference between arms is
  paired.
"""

import numpy as np
import pandas as pd

import b2_common  # noqa: F401  (puts the exploration scripts and this worktree's analysis on the path)
import b2_stats
import r6_stats as S
from gordian_analysis.stream import notice_per_stream, selection_per_stream

CLASSES = ("background", "plain", "hard", "leak", "decoy")

# measure name -> (numerator column, denominator column, scale)
MEASURES = dict(b2_stats.MEASURES)
MEASURES.update({
    "plain_accuracy": ("correct_plain", "incidents_plain", 1.0),
    "critical_misses_per_stream": ("cmiss", "one", 1.0),
    "critical_miss_share": ("cmiss", "critical_incidents", 1.0),
    "decoy_notices_per_decoy": ("s_notices_decoy", "decoy_n", 1.0),
    "leak_lost_share": ("s_leak_lost", "leak_n", 1.0),
    "decoy_followup_hit_share": ("s_decoy_followup_hit", "decoy_n", 1.0),
    "unattributed_per_stream": ("s_escalations_unattributed", "one", 1.0),
    "esc_calls_per_stream": ("esc_calls", "one", 1.0),
    "esc_tokens_per_stream": ("esc_tokens", "one", 1.0),
})
for _c in CLASSES:
    MEASURES[f"esc_{_c}_per_stream"] = (f"s_calls_{_c}", "one", 1.0)
    MEASURES[f"esc_{_c}_cost_share"] = (f"s_tokens_{_c}", "esc_tokens", 1.0)
    MEASURES[f"esc_{_c}_s_per_stream"] = (f"s_modelled_ns_{_c}", "one", 1e-9)
    MEASURES[f"notices_{_c}_escalated_share"] = (f"s_escalated_{_c}", f"s_notices_{_c}", 1.0)
    MEASURES[f"notices_{_c}_retired_before_share"] = (f"s_retired_before_escalation_{_c}", f"s_notices_{_c}", 1.0)
    MEASURES[f"notices_{_c}_followup_share"] = (f"s_followup_retired_{_c}", f"s_notices_{_c}", 1.0)
    MEASURES[f"notices_{_c}_followup_before_share"] = (f"s_followup_before_escalation_{_c}", f"s_notices_{_c}", 1.0)
    MEASURES[f"sel_notices_{_c}_per_stream"] = (f"s_notices_{_c}", "one", 1.0)
    MEASURES[f"followup_retired_{_c}_per_stream"] = (f"s_followup_retired_{_c}", "one", 1.0)
    MEASURES[f"followup_before_{_c}_per_stream"] = (f"s_followup_before_escalation_{_c}", "one", 1.0)


def per_stream(arm):
    """One arm's per-stream numerators and denominators, B2's and the selection's, on `seed`."""
    t = S.per_stream(arm).join(notice_per_stream(arm), how="inner").join(
        selection_per_stream(arm).add_prefix("s_"), how="inner")
    t = t.copy()  # consolidate the joined frame before the few columns added below
    results = arm.results.set_index("seed")
    extra = {
        "one": 1,
        "substrate_ns": results["substrate_ns"],
        "critical_incidents": results["critical_incidents"].astype("int64"),
        "esc_calls": sum(t[f"s_calls_{c}"] for c in CLASSES),
        "esc_tokens": sum(t[f"s_tokens_{c}"] for c in CLASSES),
    }
    return pd.concat([t, pd.DataFrame(extra, index=t.index)], axis=1)


class Measures:
    """Per-arm, per-stream numerators and denominators of a run, for cluster-bootstrapping."""

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

    def paired(self, measures, pairs, seed, resamples=10_000, chunk=500):
        """`{(measure, a, b): (point, lower, higher)}` of `measure` of arm `a` minus that of arm `b`,
        for each (measure, a, b) in `pairs`, all over the same resamples of streams."""
        rng = np.random.default_rng(seed)
        point = self.points(sorted({m for m, _, _ in pairs}))
        draws = {p: [] for p in pairs}
        done = 0
        while done < resamples:
            k = min(chunk, resamples - done)
            w = self.counts(rng, k)
            cache = {m: self._ratio(m, w) for m in sorted({m for m, _, _ in pairs})}
            for (m, a, b) in pairs:
                draws[(m, a, b)].append(cache[m][:, self.row[a]] - cache[m][:, self.row[b]])
            done += k
        out = {}
        for (m, a, b), parts in draws.items():
            x = np.concatenate(parts)
            x = x[~np.isnan(x)]
            lo, hi = S.interval(x) if len(x) else (float("nan"), float("nan"))
            out[(m, a, b)] = (float(point[m][self.row[a]] - point[m][self.row[b]]), lo, hi)
        return out
