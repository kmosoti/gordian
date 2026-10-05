"""R6 arithmetic: one arm's points, the frontier over references per call, and the criterion's
cluster-bootstrap, in one place so that no script decides anything twice.

Exploration (nothing here tests a hypothesis). Definitions, as R4 and R5 fixed them and R6 reuses:

* **quality**: the pooled fraction (ratio of sums over streams) of hard incidents outside the
  slow-leak family declared correctly by their deadline. Plain accuracy, critical misses, false
  alarms are computed beside it and never folded into it.
* **references per call**: the pooled ratio `sum(reasoner_refs) / sum(reasoner_calls)` over
  streams (R6's cost axis, as the plan's criterion fixes it).
* intervals: 90% equal-tailed percentile intervals over resamples of whole streams (clusters).
"""

import numpy as np
import pandas as pd

from gordian_analysis.frontier import pareto_mask, stream_table

FAMILIES = ("compound", "cascade", "split_brain", "slow_leak")


def pooled(num, den):
    d = float(np.sum(den))
    return float(np.sum(num)) / d if d > 0 else float("nan")


def family_counts(arm):
    hard = arm.incidents[arm.incidents["tier"] == "hard"]
    out = {}
    for f in FAMILIES:
        g = hard[hard["family"] == f]
        out[f] = (int(g["correct_by_deadline"].sum()), len(g))
    return out


def per_stream(arm):
    """Per-stream numerators and denominators, aligned on `seed` (ascending)."""
    t = stream_table(arm)
    r = arm.results.set_index("seed").sort_index()
    t["refs"] = r["reasoner_refs"].astype("int64")
    t["calls"] = r["reasoner_calls"].astype("int64")
    t["refused"] = r["escalations_refused"].astype("int64")
    t["tokens"] = r["reasoner_tokens"].astype("int64")
    t["hard_all_num"] = t["quality_num"] + t["leak_num"]
    t["hard_all_den"] = t["quality_den"] + t["leak_den"]
    t["cmiss"] = t["critical_missed_plain"] + t["critical_missed_hard"]
    return t


def point(arm):
    t = per_stream(arm)
    fam = family_counts(arm)
    calls = int(t["calls"].sum())
    refs = int(t["refs"].sum())
    out = {
        "arm": arm.name,
        "role": arm.role,
        "streams": len(t),
        "quality": pooled(t["quality_num"], t["quality_den"]),
        "quality_correct": int(t["quality_num"].sum()),
        "quality_incidents": int(t["quality_den"].sum()),
        "leak_rate": pooled(t["leak_num"], t["leak_den"]),
        "refs_per_call": refs / calls if calls else float("nan"),
        "calls_per_stream": float(t["calls"].mean()),
        "refs_per_stream": float(t["refs"].mean()),
        "tokens_per_stream": float(t["tokens"].mean()),
        "cost_s": float(t["total_cost_ns"].mean()) / 1e9,
        "reasoner_cost_s": float(t["reasoner_cost_ns"].mean()) / 1e9,
        "plain_acc": pooled(t["correct_plain"], t["incidents_plain"]),
        "critical_misses": int(t["cmiss"].sum()),
        "critical_incidents": int(t["critical_incidents"].sum()),
        "false_alarms_per_stream": float(t["false_alarms"].mean()),
        "wrong_per_stream": float(t["wrong_declarations"].mean()),
        "refused": int(t["refused"].sum()),
    }
    for f in FAMILIES:
        out[f"{f}_correct"], out[f"{f}_n"] = fam[f]
        out[f"{f}_rate"] = fam[f][0] / fam[f][1] if fam[f][1] else float("nan")
    return out


def frontier_keys(points, dedupe=True):
    """Row indices of `points` (columns `quality`, `refs_per_call`) on the frontier of quality
    (higher better) against references per call (lower better). Exact ties on both axes are one
    point: the first row (grid order) is kept, because configurations that tie exactly built the
    same contexts."""
    q = points["quality"].to_numpy(float)
    c = points["refs_per_call"].to_numpy(float)
    mask = pareto_mask(q, c)
    keep = []
    seen = set()
    for i in np.flatnonzero(mask):
        key = (round(q[i], 12), round(c[i], 9))
        if dedupe and key in seen:
            continue
        seen.add(key)
        keep.append(i)
    return keep


class Streams:
    """Per-arm, per-stream numerators and denominators of a run, for cluster-bootstrapping."""

    def __init__(self, run):
        self.names = list(run.arms)
        self.row = {n: i for i, n in enumerate(self.names)}
        tabs = [per_stream(run.arms[n]) for n in self.names]
        self.seeds = tabs[0].index.to_numpy()
        for t in tabs:
            assert (t.index.to_numpy() == self.seeds).all()
        stack = lambda col: np.stack([t[col].to_numpy(float) for t in tabs])  # noqa: E731
        self.qn, self.qd = stack("quality_num"), stack("quality_den")
        self.refs, self.calls = stack("refs"), stack("calls")
        self.cost = stack("total_cost_ns")
        self.n = len(self.seeds)

    def counts(self, rng, m):
        return rng.multinomial(self.n, np.full(self.n, 1.0 / self.n), size=m).astype(float)

    def sums(self, w=None):
        """Pooled quality and references per call per arm; `w` is a (resamples, streams) matrix of
        stream counts, or None for the sample itself."""
        if w is None:
            w = np.ones((1, self.n))
        qn, qd = w @ self.qn.T, w @ self.qd.T
        refs, calls = w @ self.refs.T, w @ self.calls.T
        with np.errstate(divide="ignore", invalid="ignore"):
            q = np.where(qd > 0, qn / qd, np.nan)
            rpc = np.where(calls > 0, refs / calls, 0.0)  # no call: no reference (never_escalate)
        return q, rpc  # (resamples, arms)


def interval(x, lo=0.05, hi=0.95):
    x = np.asarray(x, float)
    return float(np.quantile(x, lo, method="lower")), float(np.quantile(x, hi, method="higher"))


def criterion(streams, ceiling, publics, seed, resamples=10_000, chunk=500,
              margin=0.10, lower=0.05, within=0.05, ratio=1.5, ratio_excludes=1.25):
    """R6's two clauses, as the plan writes them, with the readings stated in the report.

    `ceiling` is the privileged decisive-evidence arm's name; `publics` the names of the public
    context builders' arms (selection held at the selection oracle). The zero-evidence comparator
    `never_escalate` (quality 0, no references) is always among the candidates for clause 1 so that
    a comparator exists; for clause 2 it counts only if its quality is within `within` of the
    ceiling's (it never is).
    """
    ic = streams.row[ceiling]
    ip = np.array([streams.row[p] for p in publics])

    def one(q, rpc):
        qc, rc = q[:, ic], rpc[:, ic]
        qp, rp = q[:, ip], rpc[:, ip]
        # clause 1: best public quality among builders with references per call <= the ceiling's
        ok = (rp <= rc[:, None] + 1e-12) & ~np.isnan(qp)
        best = np.where(ok, qp, -np.inf).max(axis=1)
        best = np.where(np.isfinite(best), best, 0.0)  # nothing affordable: the empty context
        gap = qc - best
        # clause 2: fewest references per call among builders within `within` of the ceiling
        near = (qp >= (qc - within)[:, None]) & ~np.isnan(qp)
        fewest = np.where(near, rp, np.inf).min(axis=1)
        return qc, rc, best, gap, fewest / rc

    q, rpc = streams.sums()
    qc, rc, best, gap, rat = (a[0] for a in one(q, rpc))
    ok = (rpc[0, ip] <= rpc[0, ic] + 1e-12) & ~np.isnan(q[0, ip])
    k1 = np.flatnonzero(ok)[np.argmax(q[0, ip][ok])] if ok.any() else None
    near = (q[0, ip] >= qc - within) & ~np.isnan(q[0, ip])
    k2 = np.flatnonzero(near)[np.argmin(rpc[0, ip][near])] if near.any() else None
    rng = np.random.default_rng(seed)
    gaps, rats, bests, qcs, rcs = [], [], [], [], []
    done = 0
    while done < resamples:
        m = min(chunk, resamples - done)
        w = streams.counts(rng, m)
        qs, rs = streams.sums(w)
        _, rcm, bm, gm, ratm = one(qs, rs)
        qcs.append(qs[:, ic])
        rcs.append(rcm)
        bests.append(bm)
        gaps.append(gm)
        rats.append(ratm)
        done += m
    gaps, rats = np.concatenate(gaps), np.concatenate(rats)
    glo, ghi = interval(gaps)
    rlo, rhi = interval(rats[np.isfinite(rats)]) if np.isfinite(rats).any() else (float("inf"),) * 2
    # the 5th percentile of the ratio with unreachable resamples counted as infinite
    rlo_all = float(np.quantile(rats, 0.05, method="lower"))
    rhi_all = float(np.quantile(rats, 0.95, method="higher"))
    return {
        "ceiling_quality": float(qc), "ceiling_quality_ci": interval(np.concatenate(qcs)),
        "ceiling_refs_per_call": float(rc), "ceiling_refs_ci": interval(np.concatenate(rcs)),
        "c1_best_arm": publics[k1] if k1 is not None else None,
        "c1_best_quality": float(q[0, ip][k1]) if k1 is not None else 0.0,
        "c1_best_refs_per_call": float(rpc[0, ip][k1]) if k1 is not None else 0.0,
        "c1_gap": float(gap), "c1_gap_lo": glo, "c1_gap_hi": ghi,
        "c1_holds": bool(gap >= margin and glo > lower),
        "c2_arm": publics[k2] if k2 is not None else None,
        "c2_refs_per_call": float(rpc[0, ip][k2]) if k2 is not None else float("inf"),
        "c2_ratio": float(rat), "c2_ratio_lo": rlo_all, "c2_ratio_hi": rhi_all,
        "c2_unreachable_share": float(np.mean(~np.isfinite(rats))),
        "c2_holds": bool(rat >= ratio and rlo_all > ratio_excludes),
        "resamples": resamples, "seed": seed,
    }


def paired(streams, a, b, seed, resamples=10_000, chunk=500):
    """Quality of arm `a` minus quality of arm `b`, cluster-bootstrapped over the same streams."""
    ia, ib = streams.row[a], streams.row[b]
    q, _ = streams.sums()
    rng = np.random.default_rng(seed)
    out = []
    done = 0
    while done < resamples:
        m = min(chunk, resamples - done)
        qs, _ = streams.sums(streams.counts(rng, m))
        out.append(qs[:, ia] - qs[:, ib])
        done += m
    lo, hi = interval(np.concatenate(out))
    return float(q[0, ia] - q[0, ib]), lo, hi


def mean_diff(x, y, seed, resamples=10_000):
    d = np.asarray(x, float) - np.asarray(y, float)
    rng = np.random.default_rng(seed)
    idx = rng.integers(0, len(d), size=(resamples, len(d)))
    means = d[idx].mean(axis=1)
    lo, hi = interval(means)
    return float(d.mean()), lo, hi


def table(df):
    """A DataFrame as Markdown."""
    cols = list(df.columns)
    out = ["| " + " | ".join(cols) + " |", "|" + "|".join("---" for _ in cols) + "|"]
    for _, r in df.iterrows():
        out.append("| " + " | ".join(str(r[c]) for c in cols) + " |")
    return "\n".join(out)


def to_frame(rows):
    return pd.DataFrame(rows)
