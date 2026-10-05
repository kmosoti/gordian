"""Quality-to-cost points, frontiers and cluster-bootstrap intervals over streams (work item R4).

The headroom check compares the privileged oracle with the best *frontier* of the
non-privileged baselines, not with one tuned point (`docs/local-test-plan.md`, section 5R, R4,
"Margin"). This module holds the arithmetic of that comparison, so that a script that runs it
has nothing to decide:

* **Quality** of an arm is the pooled fraction of *hard incidents declared correctly by their
  deadline*, **excluding the slow-leak family** (whose gap is salience headroom, not
  escalation-timing headroom, and is reported separately). Pooled means a ratio of sums over
  streams, never a mean of per-stream ratios. Plain-incident accuracy, critical misses, false
  alarms and wrong declarations are computed beside it and never folded into it.
* **Cost** of an arm is the mean total modelled cost per stream (`total_cost_ns`: substrate and
  rule plus the reasoner at the manifest's exchange rate). No exchange rate between quality and
  cost is chosen anywhere here.
* A **frontier** is the set of points that no other point beats on both axes: no other point has
  quality at least as high and cost at least as low with one of the two strictly better.
* **Intervals** resample whole streams (the cluster: one stream holds about 27 incidents and two
  or three hard ones, which share a service graph, a noise process and regime changes) and
  recompute everything from the resampled streams, including which baseline configuration is
  best. All arms of a run played the same streams, so a resample draws one set of streams and
  applies it to every arm.

Nothing here sets a margin. `headroom_verdict` reports where the interval lies against margins
that the caller passes in.

The bias worth knowing: the best baseline is the maximum over many noisy configurations, so its
point estimate is biased upward, and the gap to the oracle is biased downward. That errs against
finding headroom. Choosing the configuration on other streams (`best_at_cost` with a
`select_on` table) removes it; both are offered and the report says which it uses.
"""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np
import pandas as pd

from .load import COMPARISON_ROLE, StreamArm, StreamRun

# The family excluded from `quality`; the others are the escalation-timing families.
EXCLUDED_FAMILY = "slow_leak"
DEFAULT_RESAMPLES = 10_000


# ---- one arm -----------------------------------------------------------------------------------


def stream_table(arm: StreamArm, excluded_family: str = EXCLUDED_FAMILY) -> pd.DataFrame:
    """One row per stream (index `seed`, ascending) with the numerators, denominators and cost
    that every ratio of this module is a ratio of.

    `quality_num` and `quality_den` count hard incidents of every family but `excluded_family`
    (declared correctly by their deadline, and all of them). `leak_num` and `leak_den` count the
    excluded family. The remaining columns are the evaluator's counts, unchanged.
    """
    inc = arm.incidents
    hard = inc[inc["tier"] == "hard"]
    kept = hard[hard["family"] != excluded_family]
    left = hard[hard["family"] == excluded_family]
    per = lambda df, col: df.groupby("seed")[col].sum()  # noqa: E731
    r = arm.results.set_index("seed").sort_index()
    out = pd.DataFrame(index=r.index)
    out["quality_num"] = per(kept.assign(x=kept["correct_by_deadline"].astype(int)), "x")
    out["quality_den"] = kept.groupby("seed").size()
    out["leak_num"] = per(left.assign(x=left["correct_by_deadline"].astype(int)), "x")
    out["leak_den"] = left.groupby("seed").size()
    out = out.fillna(0).astype("int64")
    for c in (
        "incidents_plain",
        "correct_plain",
        "incidents_decoy",
        "decoys_alarmed",
        "decoys_dismissed",
        "decoys_silent",
        "critical_incidents",
        "critical_missed_plain",
        "critical_missed_hard",
        "wrong_declarations",
        "false_alarms",
        "reasoner_calls",
        "reasoner_tokens",
        "reasoner_cost_ns",
        "substrate_ns",
        "total_cost_ns",
        "hard_incidents_escalated",
        "incidents_hard",
    ):
        out[c] = r[c].astype("int64")
    return out


def pooled(num, den) -> float:
    """`sum(num) / sum(den)`; NaN for a zero denominator (no incident, which is not accuracy 0)."""
    d = float(np.sum(den))
    return float(np.sum(num)) / d if d > 0 else float("nan")


def arm_point(arm: StreamArm) -> dict:
    """The point estimates of one arm, over all of its streams."""
    t = stream_table(arm)
    n = len(t)
    critical_missed = t["critical_missed_plain"].sum() + t["critical_missed_hard"].sum()
    return {
        "arm": arm.name,
        "role": arm.role,
        "streams": n,
        "quality": pooled(t["quality_num"], t["quality_den"]),
        "quality_incidents": int(t["quality_den"].sum()),
        "quality_correct": int(t["quality_num"].sum()),
        "cost_ns": float(t["total_cost_ns"].mean()),
        "reasoner_cost_ns": float(t["reasoner_cost_ns"].mean()),
        "calls": float(t["reasoner_calls"].mean()),
        "tokens": float(t["reasoner_tokens"].mean()),
        "leak_rate": pooled(t["leak_num"], t["leak_den"]),
        "leak_incidents": int(t["leak_den"].sum()),
        "plain_rate": pooled(t["correct_plain"], t["incidents_plain"]),
        "critical_miss_rate": pooled(critical_missed, t["critical_incidents"]),
        "critical_misses": int(critical_missed),
        "wrong_per_stream": float(t["wrong_declarations"].mean()),
        "false_alarms_per_stream": float(t["false_alarms"].mean()),
        "decoys_alarmed": int(t["decoys_alarmed"].sum()),
        "decoys": int(t["incidents_decoy"].sum()),
    }


def arm_params(manifest: dict | None) -> dict[str, dict]:
    """Each arm's policy id and parameters from a run's `manifest.json`, by arm name.

    A policy written as its bare id has no parameters. A parameter that is absent from the
    manifest is absent here (a delay of zero is not written).
    """
    out = {}
    for a in (manifest or {}).get("arms", []):
        pol = a["policy"]
        if isinstance(pol, str):
            out[a["arm"]] = {"policy": pol}
        else:
            out[a["arm"]] = dict(pol)
    return out


def points_table(run: StreamRun) -> pd.DataFrame:
    """One row per arm of a run: the arm's policy and parameters (one column per parameter,
    empty where the arm has none) and `arm_point`."""
    params = arm_params(run.manifest)
    rows = []
    for name, arm in run.arms.items():
        row = arm_point(arm)
        p = params.get(name, {})
        row["policy"] = p.get("policy", "")
        for k, v in p.items():
            if k != "policy":
                row[k] = v
        rows.append(row)
    return pd.DataFrame(rows)


# ---- frontiers ---------------------------------------------------------------------------------


def pareto_mask(quality, cost) -> np.ndarray:
    """True for the points no other point dominates (higher quality, lower cost are better).

    A point is dominated when another has quality at least as high and cost at least as low, and
    is strictly better on one. Points that tie on both are all kept. NaN quality is never on the
    frontier.
    """
    q = np.asarray(quality, dtype=float)
    c = np.asarray(cost, dtype=float)
    keep = np.zeros(len(q), dtype=bool)
    for i in range(len(q)):
        if np.isnan(q[i]) or np.isnan(c[i]):
            continue
        better = (q >= q[i]) & (c <= c[i]) & ((q > q[i]) | (c < c[i]))
        keep[i] = not better.any()
    return keep


def frontier(points: pd.DataFrame, by: str | None = "policy") -> pd.DataFrame:
    """The non-dominated rows of `points` (columns `quality` and `cost_ns`), within each value of
    column `by` (one frontier per arm family) or over all rows when `by` is None. Returns the
    rows with a `frontier` column naming the scope, ordered by cost."""
    if by is None:
        scoped = [("all", points)]
    else:
        scoped = list(points.groupby(by, sort=True))
    parts = []
    for key, g in scoped:
        m = pareto_mask(g["quality"], g["cost_ns"])
        parts.append(g[m].assign(frontier=key))
    if not parts:
        return points.iloc[0:0].assign(frontier="")
    return pd.concat(parts).sort_values(["frontier", "cost_ns", "quality"])


def best_at_cost(points: pd.DataFrame, cost_limit: float) -> pd.Series | None:
    """The row of highest quality among those with cost at most `cost_limit`; the cheapest of
    equal ones. None when no row is affordable."""
    ok = points[(points["cost_ns"] <= cost_limit) & points["quality"].notna()]
    if ok.empty:
        return None
    top = ok[ok["quality"] == ok["quality"].max()]
    return top.sort_values("cost_ns").iloc[0]


def cheapest_reaching(points: pd.DataFrame, quality: float) -> pd.Series | None:
    """The cheapest row whose quality is at least `quality`, or None if none reaches it."""
    ok = points[points["quality"] >= quality]
    if ok.empty:
        return None
    return ok.sort_values(["cost_ns", "quality"], ascending=[True, False]).iloc[0]


# ---- cluster bootstrap -------------------------------------------------------------------------


@dataclass
class ClusterData:
    """Per-stream numerators, denominators and costs of several arms over the same streams.

    Arrays are `(arms, streams)`; `names` orders the rows; `seeds` orders the columns.
    """

    names: list[str]
    seeds: np.ndarray
    quality_num: np.ndarray
    quality_den: np.ndarray
    cost: np.ndarray

    @classmethod
    def from_arms(cls, arms: dict[str, StreamArm]) -> "ClusterData":
        """Align `arms` on `seed`; all must have played the same streams."""
        names = list(arms)
        tables = {n: stream_table(a) for n, a in arms.items()}
        seeds = tables[names[0]].index.to_numpy()
        for n, t in tables.items():
            if not np.array_equal(t.index.to_numpy(), seeds):
                raise ValueError(f"arm {n!r} played other streams than {names[0]!r}")
        return cls(
            names=names,
            seeds=seeds,
            quality_num=np.stack([tables[n]["quality_num"].to_numpy() for n in names]),
            quality_den=np.stack([tables[n]["quality_den"].to_numpy() for n in names]),
            cost=np.stack([tables[n]["total_cost_ns"].to_numpy(dtype=float) for n in names]),
        )

    @property
    def n_streams(self) -> int:
        return len(self.seeds)

    def row(self, name: str) -> int:
        return self.names.index(name)

    def estimates(self, idx: np.ndarray | None = None) -> tuple[np.ndarray, np.ndarray]:
        """Quality and mean cost of every arm over the streams `idx` (all when None; a `(B, n)`
        matrix of stream indices gives `(arms, B)` arrays)."""
        if idx is None:
            num = self.quality_num.sum(axis=1).astype(float)
            den = self.quality_den.sum(axis=1).astype(float)
            cost = self.cost.mean(axis=1)
            return _ratio(num, den), cost
        num = self.quality_num[:, idx].sum(axis=2).astype(float)
        den = self.quality_den[:, idx].sum(axis=2).astype(float)
        cost = self.cost[:, idx].mean(axis=2)
        return _ratio(num, den), cost


def _ratio(num: np.ndarray, den: np.ndarray) -> np.ndarray:
    with np.errstate(invalid="ignore", divide="ignore"):
        return np.where(den > 0, num / np.where(den > 0, den, 1), np.nan)


@dataclass(frozen=True)
class HeadroomEstimate:
    """The oracle against the best non-privileged configurations, point and resampled.

    All figures are for one reasoner setting. `gap` is oracle quality minus the best baseline
    quality among configurations whose cost is at most the oracle's, *in the same resample*.
    `best_arm` is the configuration that attains it at the point estimate. `reach_cost_ns` is the
    cost of the cheapest configuration whose quality is at least the oracle's (inf if none
    does); `reach_*` interval figures are over the resamples where one did, and `reach_share` is
    the share of resamples in which one did. `best_arm_share` is, over the resamples, how often
    each configuration was the best affordable one.
    """

    oracle: str
    baselines: tuple[str, ...]
    confidence: float
    n_streams: int
    n_resamples: int
    seed: int
    oracle_quality: float
    oracle_cost_ns: float
    best_arm: str | None
    best_quality: float
    best_cost_ns: float
    gap: float
    gap_low: float
    gap_high: float
    oracle_quality_low: float
    oracle_quality_high: float
    best_quality_low: float
    best_quality_high: float
    oracle_cost_low: float
    oracle_cost_high: float
    reach_arm: str | None
    reach_cost_ns: float
    reach_cost_low: float
    reach_cost_high: float
    reach_share: float
    best_arm_share: dict
    no_affordable_share: float


def cluster_bootstrap(
    data: ClusterData,
    oracle: str,
    baselines: list[str],
    *,
    seed: int,
    confidence: float = 0.90,
    n_resamples: int = DEFAULT_RESAMPLES,
    chunk: int = 500,
) -> HeadroomEstimate:
    """Pooled cluster bootstrap of the oracle against the best of `baselines` at cost at most
    the oracle's.

    Each resample draws `n_streams` streams with replacement (the cluster), recomputes every
    arm's pooled quality and mean cost from those streams, and takes the best baseline among
    those affordable at the oracle's resampled cost. Intervals are equal-tailed percentile
    intervals. `seed` is required and recorded. A resample in which no baseline is affordable at
    the oracle's resampled cost has no gap and is left out of the interval's quantiles; its share
    is returned as `no_affordable_share` (a cheap arm such as `never_escalate` among the baselines
    makes it zero in practice).
    """
    if not 0.0 < confidence < 1.0:
        raise ValueError("confidence must be in (0, 1)")
    if not baselines:
        raise ValueError("no baselines")
    io = data.row(oracle)
    ib = np.array([data.row(b) for b in baselines])
    n = data.n_streams
    rng = np.random.default_rng(seed)

    q_all, c_all = data.estimates()
    q_o, c_o = q_all[io], c_all[io]
    q_b, c_b = q_all[ib], c_all[ib]
    afford = c_b <= c_o
    best_at = _argbest(q_b, c_b, afford)
    reach_at = _argreach(q_b, c_b, q_o)
    best_arm = None if best_at < 0 else baselines[best_at]
    reach_arm = None if reach_at < 0 else baselines[reach_at]

    gaps, qos, qbs, cos, reaches = [], [], [], [], []
    shares: dict[str, int] = {}
    done = 0
    while done < n_resamples:
        m = min(chunk, n_resamples - done)
        idx = rng.integers(0, n, size=(m, n))
        q, c = data.estimates(idx)
        qo, co = q[io], c[io]  # (m,)
        qb, cb = q[ib], c[ib]  # (baselines, m)
        ok = (cb <= co[None, :]) & ~np.isnan(qb)
        masked = np.where(ok, qb, -np.inf)
        best = masked.max(axis=0)
        has = np.isfinite(best)
        arg = masked.argmax(axis=0)
        for j in np.flatnonzero(has):
            shares[baselines[arg[j]]] = shares.get(baselines[arg[j]], 0) + 1
        gaps.append(np.where(has, qo - best, np.nan))
        qos.append(qo)
        qbs.append(np.where(has, best, np.nan))
        cos.append(co)
        reach_ok = qb >= qo[None, :]
        reaches.append(np.where(reach_ok, cb, np.inf).min(axis=0))
        done += m
    gaps = np.concatenate(gaps)
    qos = np.concatenate(qos)
    qbs = np.concatenate(qbs)
    cos = np.concatenate(cos)
    reaches = np.concatenate(reaches)
    tail = (1.0 - confidence) / 2.0
    qt = [tail, 1.0 - tail]

    def interval(x):
        x = x[np.isfinite(x)]
        if len(x) == 0:
            return float("nan"), float("nan")
        lo, hi = np.quantile(x, qt)
        return float(lo), float(hi)

    finite_reach = reaches[np.isfinite(reaches)]
    return HeadroomEstimate(
        oracle=oracle,
        baselines=tuple(baselines),
        confidence=confidence,
        n_streams=n,
        n_resamples=n_resamples,
        seed=seed,
        oracle_quality=float(q_o),
        oracle_cost_ns=float(c_o),
        best_arm=best_arm,
        best_quality=float(q_b[best_at]) if best_at >= 0 else float("nan"),
        best_cost_ns=float(c_b[best_at]) if best_at >= 0 else float("nan"),
        gap=float(q_o - q_b[best_at]) if best_at >= 0 else float("nan"),
        gap_low=interval(gaps)[0],
        gap_high=interval(gaps)[1],
        oracle_quality_low=interval(qos)[0],
        oracle_quality_high=interval(qos)[1],
        best_quality_low=interval(qbs)[0],
        best_quality_high=interval(qbs)[1],
        oracle_cost_low=interval(cos)[0],
        oracle_cost_high=interval(cos)[1],
        reach_arm=reach_arm,
        reach_cost_ns=float(c_b[reach_at]) if reach_at >= 0 else float("inf"),
        reach_cost_low=interval(finite_reach)[0],
        reach_cost_high=interval(finite_reach)[1],
        reach_share=float(len(finite_reach) / len(reaches)),
        best_arm_share={
            k: v / len(reaches) for k, v in sorted(shares.items(), key=lambda kv: -kv[1])
        },
        no_affordable_share=float(np.mean(~np.isfinite(gaps))),
    )


def _argbest(q: np.ndarray, c: np.ndarray, afford: np.ndarray) -> int:
    """Index of the affordable arm of highest quality (cheapest among ties), or -1."""
    ok = afford & ~np.isnan(q)
    if not ok.any():
        return -1
    top = np.where(ok, q, -np.inf).max()
    cand = np.flatnonzero(ok & (q == top))
    return int(cand[np.argmin(c[cand])])


def _argreach(q: np.ndarray, c: np.ndarray, target: float) -> int:
    """Index of the cheapest arm with quality at least `target`, or -1."""
    ok = (q >= target) & ~np.isnan(q)
    if not ok.any():
        return -1
    cand = np.flatnonzero(ok)
    return int(cand[np.argmin(c[cand])])


# ---- verdict -----------------------------------------------------------------------------------

HEADROOM = "headroom"
NO_HEADROOM = "no_headroom"


def headroom_verdict(
    est: HeadroomEstimate, *, margin: float = 0.10, lower_bound: float = 0.05
) -> str:
    """`headroom` if the point gap is at least `margin` and the interval's lower limit exceeds
    `lower_bound`; `no_headroom` otherwise. The margin and the bound are the experiment's, passed
    in: the defaults are the ones the coordinator fixed for R4 and are not this module's to
    change. The condition that headroom counts only where the reasoner is very informative and
    uncorrelated is about settings, not about one estimate, and is the caller's."""
    if not np.isfinite(est.gap) or not np.isfinite(est.gap_low):
        return NO_HEADROOM
    return HEADROOM if est.gap >= margin and est.gap_low > lower_bound else NO_HEADROOM


def comparison_arms(run: StreamRun) -> list[str]:
    """The names of a run's non-privileged, non-ablation arms."""
    return [n for n, a in run.arms.items() if a.role == COMPARISON_ROLE]
