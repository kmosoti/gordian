"""Pooled ratios and the per-arm summary of a stream run (work item R3b).

The unit of replication is the stream (about 27 incidents, two or three of them hard). Every
ratio here is a ratio of counts **pooled over streams**: the sum of the numerators over the sum of
the denominators. None is a mean of per-stream ratios, which would weight a stream with one hard
incident like a stream with five and is undefined for a stream with none. The definitions are the
evaluator's (`crates/gordian-stream-eval/RULES.md`, "Derived ratios"):

- escalation precision is call-level: `needed / (needed + unneeded + background)`;
- escalation recall is incident-level: `hard_incidents_escalated / incidents_hard`.

The two are at different levels on purpose, so they are read with the counts beside them.

A pooled ratio over streams is a point estimate. It carries no interval: an interval must
resample whole streams (the cluster), and that is the headroom check's (R4), not this module's.
"""

from __future__ import annotations

import numpy as np
import pandas as pd

from .load import COMPARISON_ROLE, STREAM_FAMILIES, StreamArm, StreamRun

NS_PER_S = 1_000_000_000


def pooled_ratio(numerator, denominator) -> float:
    """`sum(numerator) / sum(denominator)` over streams; NaN when the denominator sums to zero.

    NaN, not zero: an arm that never escalated has no escalation precision, which is not the
    same as a precision of zero.
    """
    num = float(np.sum(np.asarray(numerator, dtype=float)))
    den = float(np.sum(np.asarray(denominator, dtype=float)))
    return num / den if den > 0 else float("nan")


def escalation_precision(results: pd.DataFrame) -> float:
    """Pooled call-level precision: the share of reasoner calls about a hard incident."""
    calls = results[["escalations_needed", "escalations_unneeded", "escalations_background"]]
    return pooled_ratio(results["escalations_needed"], calls.sum(axis=1))


def escalation_recall(results: pd.DataFrame) -> float:
    """Pooled incident-level recall: the share of hard incidents escalated at least once."""
    return pooled_ratio(results["hard_incidents_escalated"], results["incidents_hard"])


def correct_rate(results: pd.DataFrame, tier: str) -> float:
    """Pooled share of `plain` or `hard` incidents with a correct declaration by their deadline."""
    if tier not in ("plain", "hard"):
        raise ValueError("only plain and hard incidents have a deadline")
    return pooled_ratio(results[f"correct_{tier}"], results[f"incidents_{tier}"])


def critical_miss_rate(results: pd.DataFrame) -> float:
    """Pooled share of critical incidents missed. Critical incidents are plain or hard."""
    missed = results["critical_missed_plain"] + results["critical_missed_hard"]
    return pooled_ratio(missed, results["critical_incidents"])


def correct_per_cost(results: pd.DataFrame, per_ns: float = NS_PER_S) -> float:
    """Pooled correct plain and hard incidents per `per_ns` of total modelled cost (default one
    modelled second): `sum(correct) / sum(total_cost_ns) * per_ns`. NaN for zero cost."""
    correct = results["correct_plain"] + results["correct_hard"]
    return pooled_ratio(correct, results["total_cost_ns"]) * per_ns


def arm_totals(results: pd.DataFrame) -> dict[str, int]:
    """The sum of every count column over streams, as Python ints, with the number of streams."""
    skip = {"seed", "run_id", "arm_role", "stop_reason"}
    totals = {
        c: int(results[c].sum())
        for c in results.columns
        if c not in skip and pd.api.types.is_integer_dtype(results[c])
    }
    totals["streams"] = len(results)
    return totals


def family_table(incidents: pd.DataFrame) -> pd.DataFrame:
    """Per hard-fault family, pooled over streams: incidents, correct by deadline, missed,
    critical misses, incidents escalated at least once, and the correct share.

    One row per family of `STREAM_FAMILIES` in that order, so an arm that met no incident of a
    family still has its row (n = 0, share NaN). Hard incidents only: the family is a hard
    incident's.
    """
    hard = incidents[incidents["tier"] == "hard"]
    rows = []
    for family in STREAM_FAMILIES:
        f = hard[hard["family"] == family]
        rows.append(
            {
                "family": family,
                "incidents": len(f),
                "correct": int(f["correct_by_deadline"].sum()),
                "missed": int(f["missed"].sum()),
                "critical_missed": int(f["critical_miss"].sum()),
                "escalated": int((f["escalations"] > 0).sum()),
                "correct_rate": pooled_ratio(f["correct_by_deadline"], np.ones(len(f))),
            }
        )
    return pd.DataFrame(rows)


def summarize_arm(arm: StreamArm) -> dict:
    """One arm's totals, pooled ratios and cost, as plain Python values."""
    r = arm.results
    totals = arm_totals(r)
    n = len(r)
    return {
        "arm": arm.name,
        "role": arm.role,
        "streams": n,
        "totals": totals,
        "correct_rate_plain": correct_rate(r, "plain"),
        "correct_rate_hard": correct_rate(r, "hard"),
        "critical_miss_rate": critical_miss_rate(r),
        "escalation_precision": escalation_precision(r),
        "escalation_recall": escalation_recall(r),
        "correct_per_modelled_second": correct_per_cost(r),
        "cost_per_stream_ns": {
            "substrate": totals["substrate_ns"] / n,
            "reasoner": totals["reasoner_cost_ns"] / n,
            "total": totals["total_cost_ns"] / n,
        },
        "reasoner_per_stream": {
            "calls": totals["reasoner_calls"] / n,
            "refs": totals["reasoner_refs"] / n,
            "tokens": totals["reasoner_tokens"] / n,
        },
        "families": family_table(arm.incidents).to_dict(orient="records"),
    }


def summarize_run(run: StreamRun) -> dict:
    """Every arm of a run, `comparison` arms first, then the references, each in name order."""
    arms = sorted(run.arms.values(), key=lambda a: (a.role != COMPARISON_ROLE, a.role, a.name))
    return {
        "path": str(run.path),
        "streams": len(next(iter(run.arms.values())).results),
        "arms": [summarize_arm(a) for a in arms],
    }


def _f(x: float, spec: str = ".3f") -> str:
    return "-" if x is None or (isinstance(x, float) and np.isnan(x)) else format(x, spec)


def format_summary(summary: dict) -> str:
    """The per-arm table as text. Counts are pooled over streams; ratios are ratios of pooled
    counts; costs are per stream, in modelled seconds."""
    header = (
        f"{'arm':<30} {'role':<10} {'plain ok':>13} {'hard ok':>11} {'crit miss':>10} "
        f"{'wrong':>6} {'false al.':>9} {'calls':>6} {'esc prec':>8} {'esc rec':>8} "
        f"{'tokens':>8} {'cost/stream':>11}"
    )
    lines = [
        f"Stream run: {summary['path']}  ({summary['streams']} streams; pooled over streams)",
        header,
    ]
    for a in summary["arms"]:
        t = a["totals"]
        plain = f"{t['correct_plain']}/{t['incidents_plain']} {_f(a['correct_rate_plain'], '.2f')}"
        hard = f"{t['correct_hard']}/{t['incidents_hard']} {_f(a['correct_rate_hard'], '.2f')}"
        cmiss = t["critical_missed_plain"] + t["critical_missed_hard"]
        lines.append(
            f"{a['arm']:<30} {a['role']:<10} {plain:>13} {hard:>11} {cmiss:>10} "
            f"{t['wrong_declarations']:>6} {t['false_alarms']:>9} {t['reasoner_calls']:>6} "
            f"{_f(a['escalation_precision']):>8} {_f(a['escalation_recall']):>8} "
            f"{t['reasoner_tokens']:>8} {a['cost_per_stream_ns']['total'] / NS_PER_S:>10.3f}s"
        )
    lines += [
        "",
        "plain ok / hard ok: correct by deadline / incidents of the tier, pooled; the rate follows.",
        "crit miss: critical plain and hard incidents missed. wrong: wrong declarations about "
        "plain and hard incidents. false al.: false alarms (decoys and background).",
        "esc prec: needed / all escalations (call-level). esc rec: hard incidents escalated / hard "
        "incidents (incident-level). '-' where the denominator is zero.",
        "cost/stream: total modelled cost (substrate and rule plus the reasoner at the manifest's "
        "exchange rate), modelled seconds per stream.",
        "Hard incidents are rare (about two or three per stream): read the rates with the "
        "counts, and do not read a gap between arms off a point estimate.",
    ]
    return "\n".join(lines)
