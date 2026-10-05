"""B3: stress-suite tables. Prints markdown fragments; writes b3.json.

Stage B exploration script (development run; nothing here tests a hypothesis).
Working directory for outputs and manifests: $GORDIAN_WORK (default /tmp/gordian-exploration).
The repository root is found from this file's location.
"""
import json
import sys

import numpy as np
import pandas as pd

import os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import *  # noqa  (ROOT, S_DIR, loaders)

pd.set_option("display.width", 300)
pd.set_option("display.max_columns", 60)
df = load_all()
ST = STRESSORS
REF = "Ambiguous"


def newcombe(p1, n1, p2, n2, z=1.959964):
    """Newcombe (1998) hybrid score interval for p1 - p2 (Wilson intervals)."""
    def wilson(p, n):
        d = 1 + z * z / n
        c = p + z * z / (2 * n)
        h = z * np.sqrt(p * (1 - p) / n + z * z / (4 * n * n))
        return (c - h) / d, (c + h) / d
    l1, u1 = wilson(p1, n1)
    l2, u2 = wilson(p2, n2)
    diff = p1 - p2
    lo = diff - np.sqrt((p1 - l1) ** 2 + (u2 - p2) ** 2)
    hi = diff + np.sqrt((u1 - p1) ** 2 + (p2 - l2) ** 2)
    return diff, lo, hi


rows = []
for bud in BUDGETS:
    for arm in ARMS:
        d = df[(df.budget == bud) & (df.arm == arm)]
        ref = d[d["class"] == REF]
        for cls in ST:
            g = d[d["class"] == cls]
            n = len(g)
            r = dict(budget=bud, arm=arm, stressor=cls, n=n,
                     success=g.success.mean(), crit=g.critical_miss.mean(), wrong=g.wrong.mean(),
                     false_alarm=g.false_alarm.mean(), abstained=g.abstained.mean(),
                     undecided=g.undecided.mean(), cost=g.modelled_cost_ns.mean(),
                     cost_ref=ref.modelled_cost_ns.mean(), comps=g.components_run.mean(),
                     comps_ref=ref.components_run.mean(), probes=g.probes_used.mean(),
                     probes_ref=ref.probes_used.mean(), steps=g.ops_sched.mean(),
                     bill_compute_max=g.bill_compute.max(),
                     decision_s=g.decision_at_ns.mean() / 1e9, decision_s_ref=ref.decision_at_ns.mean() / 1e9,
                     final=(g.stop_reason == "final_declaration").mean(),
                     directives_ignored=g.directives_ignored.mean())
            diff, lo, hi = newcombe(g.success.mean(), n, ref.success.mean(), len(ref))
            r.update(success_ref=ref.success.mean(), d_success=diff, d_lo=lo, d_hi=hi)
            rows.append(r)
res = pd.DataFrame(rows)
res.to_json(f"{S_DIR}/b3.json", orient="records", indent=1)

short = {"NoiseFlood": "NoiseFl", "Duplicates": "Duplic", "FeedbackBait": "FeedBt",
         "QuietUrgent": "QuietU", "StaleMemory": "StaleM", "ComponentTimeout": "CompTO"}


def matrix(col, bud, pct=True, fmt="{:.1f}"):
    t = res[res.budget == bud].pivot(index="arm", columns="stressor", values=col).loc[ARMS, ST]
    t.columns = [short[c] for c in t.columns]
    return (t * 100).round(1) if pct else t.round(2)


if __name__ == "__main__":
    for bud in BUDGETS:
        print("== success %, budget", bud)
        r = matrix("success", bud)
        r.insert(0, "Ambig(ref)", [df[(df.budget == bud) & (df.arm == a) & (df["class"] == REF)].success.mean() * 100 for a in ARMS])
        print(r.round(1).to_string())
        print("-- critical miss %")
        print(matrix("crit", bud).to_string())
        print("-- wrong declarations %")
        print(matrix("wrong", bud).to_string())
        print("-- abstained %")
        print(matrix("abstained", bud).to_string())
    # biting stressors: success differs from Ambiguous with CI excluding 0
    bite = res[(res.d_lo > 0) | (res.d_hi < 0)]
    print("== stressor cells whose success differs from Ambiguous (Newcombe 95% excludes 0):", len(bite), "of", len(res))
    print(bite[["budget", "arm", "stressor", "success", "success_ref", "d_success", "d_lo", "d_hi"]].round(3).to_string())
    print("== max bill_compute vs limit")
    print(df.groupby("budget").bill_compute.max())
    print("== undecided total", df.undecided.sum())
