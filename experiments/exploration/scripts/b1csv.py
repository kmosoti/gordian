"""Write b1-variance.csv (the B1 grid) and b1-supplementary.csv (S1, S2, S3 runs).

Stage B exploration script (development run; nothing here tests a hypothesis).
Working directory for outputs and manifests: $GORDIAN_WORK (default /tmp/gordian-exploration).
The repository root is found from this file's location.
"""
import sys

import pandas as pd

import os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import *  # noqa  (ROOT, S_DIR, loaders)

STOPS = ["terminal", "final_declaration", "budget_exhausted", "horizon", "step_cap"]


def table(frames, extra_cols):
    rows = []
    for (run_id, noise, family, arm, budget), df in frames:
        for cls in CLASSES:
            g = df[df["class"] == cls]
            if len(g) == 0:
                continue
            row = {"run_id": f"{run_id}.{arm}", **extra_cols(family, noise), "arm": arm, "compute_ns": budget,
                   "class": cls, "n": len(g)}
            for c in BOOLS:
                row[f"{c}_mean"] = g[c].mean()
                row[f"{c}_sd"] = g[c].std(ddof=1)
            vc = g.stop_reason.value_counts()
            for s in STOPS:
                row[f"stop_{s}"] = int(vc.get(s, 0))
            for c in ["modelled_cost_ns", "measured_policy_ns"]:
                row[f"{c}_mean"] = g[c].mean()
                row[f"{c}_sd"] = g[c].std(ddof=1)
            row["probes_used_mean"] = g["probes_used"].mean()
            rows.append(row)
    out = pd.DataFrame(rows)
    for c in out.columns:
        if out[c].dtype == float:
            out[c] = out[c].round(6)
    return out


def load_frames(spec):
    for run_id, noise, family, budget, arms in spec:
        for arm in arms:
            yield (run_id, noise, family, arm, budget), load_arm(f"{RUNS}/{run_id}", arm)


b1 = [(RUN_ID[b], 3, "B1", b, ARMS) for b in BUDGETS]
out = table(load_frames(b1), lambda f, n: {})
out.to_csv(f"{ROOT}/experiments/exploration/b1-variance.csv", index=False)
print("b1", len(out))

S1_ARMS = ["heuristic_only", "random_p000", "fixed_heuristic_every8", "fixed_heuristic_every16"]
S2_ARMS = ["heuristic_only", "fixed_heuristic_every2", "fixed_heuristic_every4", "fixed_heuristic_every8",
           "fixed_heuristic_every16", "fixed_estimator_only", "all_components", "random_p025",
           "oracle_evidence_privileged"]
sup = [(f"b1s-c{b}-s1000-1499", 3, "S1", b, S1_ARMS) for b in BUDGETS]
sup += [(f"b1s2-c{b}-s1000-1499", 3, "S2", b, S2_ARMS) for b in (15000, 20000, 30000, 40000, 50000)]
sup += [(f"b3n50-c{b}-s1000-1499", 50, "S3", b, ARMS) for b in BUDGETS]
out = table(load_frames(sup), lambda f, n: {"family": f, "noise_rate": n})
cols = list(out.columns)
cols.remove("family")
cols.remove("noise_rate")
out = out[["family", "noise_rate"] + cols]
out.to_csv(f"{ROOT}/experiments/exploration/b1-supplementary.csv", index=False)
print("supplementary", len(out))
