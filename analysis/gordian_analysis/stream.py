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

from .load import (
    COMPARISON_ROLE,
    STREAM_FAMILIES,
    STREAM_SELECTION_CLASSES,
    STREAM_SELECTION_FIELDS,
    StreamArm,
    StreamRun,
)

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


# ---------------------------------------------------------------------------------------------
# Notices (work items B1 and B2). The measures are the evaluator's (crates/gordian-stream-eval/
# RULES.md, N1 to N16), read from the notice files the harness writes beside results.csv and
# incidents.csv. Every ratio is pooled from counts over streams, as above; a per-stream ratio is
# never taken.
# ---------------------------------------------------------------------------------------------

LEAK_FAMILY = "slow_leak"


def _need_notices(arm: StreamArm) -> None:
    if arm.notices is None or arm.notice_incidents is None:
        raise ValueError(
            f"arm {arm.name!r} has no notice files (a run made before the Noticer seam)"
        )


def notice_per_stream(arm: StreamArm) -> pd.DataFrame:
    """The numerators and denominators of the notice measures, one row per stream (index `seed`,
    ascending), for pooling and for resampling whole streams.

    Columns: for hard incidents outside the slow-leak family (`hard`), the slow leak (`leak`), plain
    incidents and decoys: the incidents (`_n`), those noticed (`_noticed`: N2), those with an
    anchor-correct notice (`_correct`: N5), those with a site-correct notice (`_site`: N14) and those
    with one notice that is both (`_both`: N15); then `notices`, the notices on background, plain
    incidents, hard incidents and decoys (N8), `notices_on_incidents`, `incidents` (every tier),
    `retirements`, and the notices that are site-correct (`notices_site`: N14) and both
    anchor-correct and site-correct (`notices_both`: N15).
    """
    _need_notices(arm)
    ni = arm.notice_incidents
    seeds = arm.notices["seed"].to_numpy()
    out = pd.DataFrame(index=pd.Index(seeds, name="seed"))
    groups = {
        "hard": (ni["tier"] == "hard") & (ni["family"] != LEAK_FAMILY),
        "leak": (ni["tier"] == "hard") & (ni["family"] == LEAK_FAMILY),
        "plain": ni["tier"] == "plain",
        "decoy": ni["tier"] == "decoy",
    }
    for name, mask in groups.items():
        g = ni[mask].groupby("seed")
        out[f"{name}_n"] = g.size().reindex(seeds, fill_value=0).to_numpy()
        out[f"{name}_noticed"] = g["noticed"].sum().reindex(seeds, fill_value=0).to_numpy()
        out[f"{name}_correct"] = g["anchor_correct"].sum().reindex(seeds, fill_value=0).to_numpy()
        out[f"{name}_site"] = g["site_correct"].sum().reindex(seeds, fill_value=0).to_numpy()
        out[f"{name}_both"] = g["anchor_site_correct"].sum().reindex(seeds, fill_value=0).to_numpy()
    n = arm.notices.set_index("seed")
    out["notices_site"] = n["notices_site_correct"].to_numpy()
    out["notices_both"] = n["notices_anchor_site_correct"].to_numpy()
    out["notices"] = n["notices"].to_numpy()
    out["notices_background"] = n["notices_on_background"].to_numpy()
    out["notices_plain"] = n["notices_on_plain"].to_numpy()
    out["notices_hard"] = n["notices_on_hard"].to_numpy()
    out["notices_decoy"] = n["notices_on_decoy"].to_numpy()
    out["notices_on_incidents"] = out["notices"] - out["notices_background"]
    out["incidents"] = ni.groupby("seed").size().reindex(seeds, fill_value=0).to_numpy()
    out["retirements"] = n["retirements"].to_numpy()
    return out.astype("int64")


def notice_points(arm: StreamArm) -> dict:
    """The pooled notice measures of one arm, as plain Python values.

    `hard_noticed_share`, `hard_anchor_correct_share`: hard non-leak incidents noticed, and noticed
    with an anchor within 1 s of their first observation. `leak_noticed_share`,
    `leak_anchor_correct_share`: the same for the slow leak. `*_per_stream` are means over streams;
    `notices_per_incident` is the pooled count of notices anchored on incidents over incidents.
    `hard_site_correct_share` and `hard_anchor_site_correct_share` (and the leak's): hard non-leak
    incidents with a notice about their site (N14), and with one notice that is anchor-correct and
    about their site (N15). `notice_precision` is the pooled share of notices anchored on an
    incident of any tier (N16); `precision_plain`, `precision_hard` and `precision_decoy` the pooled
    share anchored on that tier (they add to it); `strict_precision` the pooled share that are
    anchor-correct and site-correct; `site_correct_notice_share` the pooled share that are
    site-correct. A share is NaN when its denominator is zero.
    """
    t = notice_per_stream(arm)
    return {
        "arm": arm.name,
        "noticer": str(arm.notices["noticer"].iloc[0]),
        "streams": len(t),
        "hard_incidents": int(t["hard_n"].sum()),
        "hard_noticed_share": pooled_ratio(t["hard_noticed"], t["hard_n"]),
        "hard_anchor_correct_share": pooled_ratio(t["hard_correct"], t["hard_n"]),
        "leak_incidents": int(t["leak_n"].sum()),
        "leak_noticed_share": pooled_ratio(t["leak_noticed"], t["leak_n"]),
        "leak_anchor_correct_share": pooled_ratio(t["leak_correct"], t["leak_n"]),
        "plain_noticed_share": pooled_ratio(t["plain_noticed"], t["plain_n"]),
        "hard_site_correct_share": pooled_ratio(t["hard_site"], t["hard_n"]),
        "hard_anchor_site_correct_share": pooled_ratio(t["hard_both"], t["hard_n"]),
        "leak_site_correct_share": pooled_ratio(t["leak_site"], t["leak_n"]),
        "leak_anchor_site_correct_share": pooled_ratio(t["leak_both"], t["leak_n"]),
        "notice_precision": pooled_ratio(t["notices_on_incidents"], t["notices"]),
        "precision_plain": pooled_ratio(t["notices_plain"], t["notices"]),
        "precision_hard": pooled_ratio(t["notices_hard"], t["notices"]),
        "precision_decoy": pooled_ratio(t["notices_decoy"], t["notices"]),
        "strict_precision": pooled_ratio(t["notices_both"], t["notices"]),
        "site_correct_notice_share": pooled_ratio(t["notices_site"], t["notices"]),
        "notices_per_stream": float(t["notices"].mean()),
        "notices_on_background_per_stream": float(t["notices_background"].mean()),
        "notices_on_plain_per_stream": float(t["notices_plain"].mean()),
        "notices_on_hard_per_stream": float(t["notices_hard"].mean()),
        "notices_on_decoy_per_stream": float(t["notices_decoy"].mean()),
        "notices_per_incident": pooled_ratio(t["notices_on_incidents"], t["incidents"]),
        "retirements_per_stream": float(t["retirements"].mean()),
    }


def notice_latency(arm: StreamArm) -> pd.DataFrame:
    """Notice latency (N4), seconds, among the incidents noticed: count, median and 90th
    percentile, for hard non-leak incidents, the slow leak and plain incidents. A latency exists
    only for an incident that was noticed, so it says how late the noticed ones were and nothing of
    the others (their number is `incidents - noticed`)."""
    _need_notices(arm)
    ni = arm.notice_incidents
    groups = {
        "hard": (ni["tier"] == "hard") & (ni["family"] != LEAK_FAMILY),
        "slow_leak": (ni["tier"] == "hard") & (ni["family"] == LEAK_FAMILY),
        "plain": ni["tier"] == "plain",
    }
    rows = []
    for name, mask in groups.items():
        g = ni[mask]
        lat = g["notice_latency_ns"].dropna().astype("int64").to_numpy() / NS_PER_S
        rows.append(
            {
                "group": name,
                "incidents": len(g),
                "noticed": int(g["noticed"].sum()),
                "median_s": float(np.median(lat)) if len(lat) else float("nan"),
                "p90_s": float(np.quantile(lat, 0.9)) if len(lat) else float("nan"),
            }
        )
    return pd.DataFrame(rows)


# ---------------------------------------------------------------------------------------------
# Selection (work item B4). The measures are the evaluator's (crates/gordian-stream-eval/RULES.md,
# E1 to E8), read from the selection files the harness writes beside the notice files. Every ratio
# is pooled from counts over streams, as above.
# ---------------------------------------------------------------------------------------------


def _need_selection(arm: StreamArm) -> None:
    if arm.selection is None or arm.selection_notices is None:
        raise ValueError(
            f"arm {arm.name!r} has no selection files (a run made before work item B4)"
        )


def selection_per_stream(arm: StreamArm) -> pd.DataFrame:
    """The numerators and denominators of the selection measures, one row per stream (index `seed`,
    ascending), for pooling and for resampling whole streams.

    Per class `c` of `STREAM_SELECTION_CLASSES` (E2): `<field>_<c>` for each of
    `STREAM_SELECTION_FIELDS` (the calls, tokens and modelled nanoseconds; the notices, those
    escalated, retired before escalation, retired by a follow-up rule, and retired by one before
    escalation), copied from `selection.csv`; `escalations_unattributed` (E1); and the
    incident-level counts of the follow-up rule, from `selection_notices.csv`: `leak_followup_hit`
    (slow-leak incidents with at least one notice retired by the rule before escalation),
    `leak_lost` (slow-leak incidents with such a notice and no notice escalated at all) and
    `decoy_followup_hit` (decoy incidents with at least one notice retired by the rule before
    escalation).
    """
    _need_selection(arm)
    sel = arm.selection.set_index("seed").sort_index()
    cols = {
        f"{field}_{c}": sel[f"{field}_{c}"]
        for field in STREAM_SELECTION_FIELDS
        for c in STREAM_SELECTION_CLASSES
    }
    cols["escalations_unattributed"] = sel["escalations_unattributed"]
    sn = arm.selection_notices
    for name, cls in (("leak", "leak"), ("decoy", "decoy")):
        mine = sn[sn["class"] == cls].copy()
        mine["hit"] = (mine["retire_cause"] == "followup") & mine["retired_before_escalation"]
        mine["asked"] = mine["escalations"] > 0
        g = mine.groupby(["seed", "incident"])
        per = pd.DataFrame({"hit": g["hit"].any(), "asked": g["asked"].any()})
        hit = per[per["hit"]].groupby("seed").size()
        cols[f"{name}_followup_hit"] = hit.reindex(sel.index, fill_value=0).astype("int64")
        if name == "leak":
            lost = per[per["hit"] & ~per["asked"]].groupby("seed").size()
            cols["leak_lost"] = lost.reindex(sel.index, fill_value=0).astype("int64")
    return pd.DataFrame(cols, index=sel.index).astype("int64")
