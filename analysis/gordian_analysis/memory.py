"""Memory measures from the evaluator's files (work item E1): what an arm declared without asking,
what it declared from memory, and what its memory got wrong, pooled over streams with a cluster
bootstrap.

Every number is a ratio of sums over whole streams (never a mean of per-stream ratios) with its
interval from resampling whole streams, the way `criterion.py` and the lab scripts do it; the
resamples are drawn once per number of streams, so two arms of one run are compared on the same
resamples. The columns are the evaluator's (`crates/gordian-stream-eval/RULES.md`, K1 to K10, and
S20 for the correct decisions); nothing here reads a hidden-side file the harness did not write.

What is measured, by name (`MEASURES`):

* `unasked_correct_hard_recurrences`, `_elsewhere`, `_reachable`, `_hard`: the hard incidents
  declared correctly with no escalation about them, over the hard incidents that repeat an earlier
  one (K1), that have an earlier incident of the same family and mode at another site and are no
  recurrence (K2), either, and all hard incidents (the incidents of `results.csv`).
* `collision_share`: of the recalls whose stored answer was right for its own incident, the share
  that were wrong (a collision, or staleness); `collisions_per_stream`; `inherited_per_stream`
  and `inherited_share`: wrong recalls whose stored answer was wrong, per stream and over the
  recalls with a wrong source; `recalls_per_stream`.
* `calls_per_correct`: reasoner calls over the plain and hard incidents correct by their deadline.
* `cost_s`, `reasoner_cost_s`: modelled seconds per stream, `total_cost_ns + noticer_ns` and the
  reasoner's alone.
* `unasked_wrong_*_per_stream`: incidents with an unasked wrong declaration, per stream, by tier,
  and their paired excess over a control arm that played the same streams (`paired_excess`).

Experience curves (`curve`) are the cumulative count of an event against the incidents seen in
stream order, averaged over the streams that reach each step (within a stream), or against the
streams seen (across streams).
"""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np
import pandas as pd

from . import criterion as C
from .load import StreamArm, load_stream_run

TIERS = ("plain", "hard", "decoy")


@dataclass(frozen=True)
class Boot:
    """The bootstrap: whole streams resampled, 90% percentile intervals (W2's constants)."""

    resamples: int = 10_000
    seed: int = 9950
    percentiles: tuple[float, float] = (5.0, 95.0)
    interval: str = "linear"
    chunk: int = 500

    def counts(self, n: int) -> np.ndarray:
        return C.draw_counts(self.seed, n, self.resamples, self.chunk)

    def ci(self, draws: np.ndarray) -> tuple[float, float]:
        return C.interval(draws, list(self.percentiles), self.interval)


def streamtab(arm: StreamArm) -> pd.DataFrame:
    """One row per stream (index `seed`, ascending): `results.csv` and `memory.csv` joined."""
    if arm.memory is None:
        raise ValueError(f"{arm.path}: no memory files")
    res = arm.results.set_index("seed")
    mem = arm.memory.drop(columns=["run_id", "arm_role", "noticer"]).set_index("seed")
    both = res.join(mem, how="inner")
    both["correct_decisions"] = both["correct_plain"] + both["correct_hard"]
    if both["noticer_ns"].isna().any():
        both["cost_ns"] = both["total_cost_ns"].astype("float64")
    else:
        both["cost_ns"] = (both["total_cost_ns"] + both["noticer_ns"]).astype("float64")
    both["right_source_recalls"] = both["recalls_correct_source_right"] + both["recalls_wrong_source_right"]
    both["wrong_source_recalls"] = both["recalls_correct_source_wrong"] + both["recalls_wrong_source_wrong"]
    both["one"] = 1
    return both.sort_index()


# name -> (numerator column or expression, denominator column or expression, scale)
MEASURES: dict[str, tuple[str, str, float]] = {
    "unasked_correct_hard_recurrences": ("hard_recurrences_unasked_correct", "hard_recurrences", 1.0),
    "unasked_correct_hard_elsewhere": ("hard_elsewhere_unasked_correct", "hard_elsewhere", 1.0),
    "unasked_correct_hard_reachable": ("hard_reachable_unasked_correct", "hard_reachable", 1.0),
    "unasked_correct_hard": ("unasked_correct_hard", "incidents_hard", 1.0),
    "collision_share": ("recalls_wrong_source_right", "right_source_recalls", 1.0),
    "collisions_per_stream": ("recalls_wrong_source_right", "one", 1.0),
    "inherited_share": ("recalls_wrong_source_wrong", "wrong_source_recalls", 1.0),
    "inherited_per_stream": ("recalls_wrong_source_wrong", "one", 1.0),
    "recalls_per_stream": ("recalls", "one", 1.0),
    "calls_per_correct": ("reasoner_calls", "correct_decisions", 1.0),
    "calls_per_stream": ("reasoner_calls", "one", 1.0),
    "cost_s": ("cost_ns", "one", 1e-9),
    "reasoner_cost_s": ("reasoner_cost_ns", "one", 1e-9),
    "stale_wrong_hard_per_stream": ("stale_wrong_hard", "one", 1.0),
    "stale_wrong_plain_per_stream": ("stale_wrong_plain", "one", 1.0),
    "stale_wrong_decoy_per_stream": ("stale_wrong_decoy", "one", 1.0),
    "unasked_wrong_plain_per_stream": ("unasked_wrong_plain", "one", 1.0),
    "unasked_wrong_hard_per_stream": ("unasked_wrong_hard", "one", 1.0),
    "unasked_wrong_decoy_per_stream": ("unasked_wrong_decoy", "one", 1.0),
}


def _vec(tab: pd.DataFrame, col: str) -> np.ndarray:
    return tab[col].to_numpy(dtype="float64")


def pooled(tab: pd.DataFrame, measure: str, boot: Boot) -> dict:
    """`measure` over the streams of `tab`: its point value, its interval and its pooled counts."""
    num_col, den_col, scale = MEASURES[measure]
    num, den = _vec(tab, num_col), _vec(tab, den_col)
    w = boot.counts(len(tab))
    point = C.ratio_point(num, den, scale)
    lo, hi = boot.ci(C.ratio_draws(num, den, scale, w))
    return {"measure": measure, "point": point, "lower": lo, "upper": hi,
            "num": float(num.sum()), "den": float(den.sum()), "streams": len(tab)}


def paired(a: pd.DataFrame, b: pd.DataFrame, measure: str, boot: Boot) -> dict:
    """`a` minus `b` on one measure, over the same streams and the same resamples."""
    if not np.array_equal(a.index.to_numpy(), b.index.to_numpy()):
        raise ValueError("the arms did not play the same streams")
    na, da, scale = _vec(a, MEASURES[measure][0]), _vec(a, MEASURES[measure][1]), MEASURES[measure][2]
    nb, db = _vec(b, MEASURES[measure][0]), _vec(b, MEASURES[measure][1])
    w = boot.counts(len(a))
    point = C.ratio_point(na, da, scale) - C.ratio_point(nb, db, scale)
    draws = C.ratio_draws(na, da, scale, w) - C.ratio_draws(nb, db, scale, w)
    lo, hi = boot.ci(draws)
    return {"measure": measure, "point": point, "lower": lo, "upper": hi, "streams": len(a)}


def paired_excess(arm: pd.DataFrame, control: pd.DataFrame, boot: Boot) -> pd.DataFrame:
    """The paired excess of unasked wrong declarations over the control, per tier and in all, in
    incidents per stream, from `memory.csv`'s counts of the two arms (`RULES.md`, "Derived
    measures"): the control's own errors cancel."""
    rows = []
    for tier in (*TIERS, "all"):
        cols = [f"unasked_wrong_{t}" for t in (TIERS if tier == "all" else (tier,))]
        x = _vec(arm.assign(_s=arm[cols].sum(axis=1)), "_s")
        y = _vec(control.assign(_s=control[cols].sum(axis=1)), "_s")
        w = boot.counts(len(arm))
        n = float(len(arm))
        draws = (w @ x - w @ y) / n
        lo, hi = boot.ci(draws)
        rows.append({"tier": tier, "arm": x.sum() / n, "control": y.sum() / n,
                     "excess": (x.sum() - y.sum()) / n, "lower": lo, "upper": hi})
    return pd.DataFrame(rows)


def load_arms(run_dir) -> dict[str, StreamArm]:
    """Every arm of a stream run directory, with its memory files."""
    return load_stream_run(run_dir).arms


# ---------------------------------------------------------------------------------------------
# Experience curves
# ---------------------------------------------------------------------------------------------


def curve_across_streams(tab: pd.DataFrame, event: str, seen: str = "incidents_hard") -> pd.DataFrame:
    """Cumulative `event` count against the cumulative `seen` count, over streams in seed order:
    the curve of the share of the incidents seen that had the event, stream by stream."""
    cum_event = tab[event].astype("float64").cumsum()
    cum_seen = tab[seen].astype("float64").cumsum()
    return pd.DataFrame({"stream": np.arange(1, len(tab) + 1), "cum_event": cum_event.to_numpy(),
                         "cum_seen": cum_seen.to_numpy(),
                         "share": (cum_event / cum_seen.where(cum_seen > 0)).to_numpy()}, index=tab.index)


def slope_across_streams(tab: pd.DataFrame, boot: Boot, event: str = "unasked_correct_hard",
                         seen: str = "incidents_hard", per: float = 100.0) -> dict:
    """The least-squares slope, against the stream number, of the cumulative share of the incidents
    seen that had the event (`criterion.curve_slopes`: points before the first denominator are left
    out), per `per` streams, with the interval from resampling whole streams in place."""
    num, den = _vec(tab, event), _vec(tab, seen)
    n = len(tab)
    point = C.curve_slopes(num, den, np.ones((1, n)))[0] * per
    draws = C.curve_slopes(num, den, boot.counts(n)) * per
    lo, hi = boot.ci(draws)
    return {"point": float(point), "lower": lo, "upper": hi, "streams": n}


def curve_within_streams(incidents: pd.DataFrame, flag: str, tier: str = "hard",
                         min_streams: int = 20) -> pd.DataFrame:
    """For the n-th incident of a tier in a stream (n counted among that tier's incidents in id
    order), the mean cumulative count of `flag` over the streams that reach n, and how many do. Rows
    with fewer than `min_streams` streams are left out, as W2's curves are."""
    rows = incidents[incidents["tier"] == tier].sort_values(["seed", "incident"]).copy()
    rows["n"] = rows.groupby("seed").cumcount() + 1
    rows["cum"] = rows.groupby("seed")[flag].cumsum()
    g = rows.groupby("n")["cum"].agg(["mean", "count"]).rename(columns={"mean": "mean_cumulative", "count": "streams"})
    return g[g["streams"] >= min_streams].reset_index()
