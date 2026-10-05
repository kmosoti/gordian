"""Choose, per reasoner setting, the baseline configurations to run on the held-out streams.

Usage: r4_select.py        (reads the six tuning runs, writes the two files below)

  experiments/exploration/r4-tuning-points.csv   every configuration's point on the tuning streams
  experiments/exploration/r4-selected.json       {setting id: [arm names]}, the held-out arm set

Exploration (nothing here tests a hypothesis). The rule is fixed here, before any held-out run: for
each arm family (always, random, periodic, change-triggered, threshold, never) keep the
configurations on that family's frontier of tuning quality against tuning cost, where quality is
the pooled fraction of hard incidents (slow-leak family excluded) declared correctly by their
deadline and cost is mean total modelled cost per stream. The held-out arm set is the union of the
families' frontiers; `never_escalate`, the privileged oracle and the ablation are added by the
manifest writer. Nothing is chosen on any held-out stream, and no exchange rate between quality
and cost is used.
"""

import json

import pandas as pd

import r4_common as C
from gordian_analysis.frontier import pareto_mask, points_table
from gordian_analysis.load import load_stream_run


def main():
    rows = []
    selected = {}
    for b, rho in C.SETTINGS:
        sid = C.setting_id(b, rho)
        run = load_stream_run(C.RUNS / C.run_id("tune", b, rho))
        pts = points_table(run)
        pts.insert(0, "setting", sid)
        pts.insert(1, "b", b)
        pts.insert(2, "rho", rho)
        pts["frontier_in_family"] = False
        base = pts[pts["role"] == "comparison"]
        for policy, g in base.groupby("policy"):
            m = pareto_mask(g["quality"], g["cost_ns"])
            pts.loc[g.index[m], "frontier_in_family"] = True
        selected[sid] = list(pts.loc[pts["frontier_in_family"], "arm"])
        rows.append(pts)
    allpts = pd.concat(rows, ignore_index=True)
    for src, dst in (("delay_ns", "delay_s"), ("period_ns", "period_s"), ("wait_ns", "wait_s")):
        if src in allpts:
            allpts[dst] = allpts[src] / C.NS
    allpts["cost_s"] = allpts["cost_ns"] / C.NS
    keep = [
        "setting", "b", "rho", "arm", "role", "policy", "p", "delay_s", "period_s", "tau", "wait_s",
        "quality", "quality_correct", "quality_incidents", "cost_s", "calls", "tokens",
        "leak_rate", "leak_incidents", "plain_rate", "critical_misses", "wrong_per_stream",
        "false_alarms_per_stream", "frontier_in_family", "streams",
    ]
    allpts = allpts[[c for c in keep if c in allpts]]
    out = C.OUT / "r4-tuning-points.csv"
    allpts.to_csv(out, index=False, float_format="%.6g")
    (C.OUT / "r4-selected.json").write_text(json.dumps(selected, indent=2) + "\n")
    for sid, names in selected.items():
        print(sid, len(names), "configurations")


if __name__ == "__main__":
    main()
