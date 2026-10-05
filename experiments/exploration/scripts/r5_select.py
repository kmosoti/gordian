"""Choose, per reasoner setting, what the new arms run on the held-out streams.

Usage: r5_select.py        (reads the six R5 tuning runs and R4's selection; writes the files below)

  experiments/exploration/r5-tuning-points.csv   every new configuration's point on the tuning streams
  experiments/exploration/r5-selected.json       {setting id: what the held-out run holds}

Exploration (nothing here tests a hypothesis). The rules are fixed here, before any held-out run:

* `contradiction_escalation`: the configurations on the arm's own frontier of tuning quality
  against tuning cost (the rule R4 applied to every family; quality is the pooled fraction of
  hard incidents, slow-leak family excluded, declared correctly by their deadline, and cost is the
  mean total modelled cost per stream). No exchange rate between quality and cost is used.
* `oracle_selection_privileged`: ONE delay per setting, the one of highest tuning quality, the
  cheapest of ties. (A frontier of the oracle is not needed: its cost hardly moves with the
  delay, and the criterion compares at its cost.) The held-out run holds that delay only; the
  sensitivity run of the other delays is separate and labelled.
* R4's frontier arms per setting are `r4-selected.json`, unchanged.
* For the trace diagnostic (primary setting only), two contradiction configurations: the
  highest-quality one on the frontier, and the highest-quality one among those costing no more
  than the selection oracle at its tuned delay on the tuning streams. Chosen on tuning streams.

Nothing is chosen on any held-out stream.
"""

import json

import pandas as pd

import r5_common as C
from gordian_analysis.frontier import pareto_mask, points_table
from gordian_analysis.load import load_stream_run


def best_quality(df):
    """Highest quality, the cheapest of ties."""
    top = df[df["quality"] == df["quality"].max()].sort_values(["cost_ns", "arm"])
    return top.iloc[0]


def main():
    r4 = C.load_json(C.OUT / "r4-selected.json")
    rows = []
    selected = {}
    for b, rho in C.SETTINGS:
        sid = C.setting_id(b, rho)
        run = load_stream_run(C.RUNS / C.run_id("tune", b, rho))
        pts = points_table(run)
        pts.insert(0, "setting", sid)
        pts.insert(1, "b", b)
        pts.insert(2, "rho", rho)
        con = pts[pts["policy"] == C.CONTRA_POLICY].copy()
        mask = pareto_mask(con["quality"], con["cost_ns"])
        pts["frontier_in_family"] = False
        pts.loc[con.index[mask], "frontier_in_family"] = True
        sel = pts[pts["policy"] == C.SEL_POLICY]
        chosen = best_quality(sel)
        pts["chosen"] = False
        pts.loc[pts["arm"] == chosen["arm"], "chosen"] = True
        frontier = con[mask]
        top = best_quality(frontier)
        affordable = frontier[frontier["cost_ns"] <= chosen["cost_ns"]]
        top_aff = best_quality(affordable) if len(affordable) else top
        sel_delay = int(round(chosen["delay_ns"] / C.NS)) if "delay_ns" in chosen and pd.notna(chosen["delay_ns"]) else 0
        selected[sid] = {
            "r4_arms": r4[sid],
            "contradiction_arms": list(frontier["arm"]),
            "selection_delay_s": sel_delay,
            "selection_arm": chosen["arm"],
            "selection_tuning_quality": float(chosen["quality"]),
            "selection_tuning_cost_s": float(chosen["cost_ns"]) / C.NS,
            "diag_contradiction_arms": list(dict.fromkeys([top["arm"], top_aff["arm"]])),
        }
        rows.append(pts)
        print(f"{sid}: contradiction frontier {len(frontier)} of {len(con)}; "
              f"best quality {top['arm']} {top['quality']:.3f} @ {top['cost_ns'] / C.NS:.2f}s; "
              f"selection oracle delay {sel_delay}s quality {chosen['quality']:.3f} @ {chosen['cost_ns'] / C.NS:.2f}s")
    allpts = pd.concat(rows, ignore_index=True)
    for src, dst in (("delay_ns", "delay_s"), ("persist_ns", "persist_s")):
        if src in allpts:
            allpts[dst] = allpts[src] / C.NS
    allpts["cost_s"] = allpts["cost_ns"] / C.NS
    keep = [
        "setting", "b", "rho", "arm", "role", "policy", "delay_s", "persist_s", "quality",
        "quality_correct", "quality_incidents", "cost_s", "calls", "tokens", "leak_rate",
        "leak_incidents", "plain_rate", "critical_misses", "wrong_per_stream",
        "false_alarms_per_stream", "frontier_in_family", "chosen", "streams",
    ]
    allpts = allpts[[c for c in keep if c in allpts]]
    allpts.to_csv(C.OUT / "r5-tuning-points.csv", index=False, float_format="%.6g")
    (C.OUT / "r5-selected.json").write_text(json.dumps(selected, indent=2) + "\n")


if __name__ == "__main__":
    main()
