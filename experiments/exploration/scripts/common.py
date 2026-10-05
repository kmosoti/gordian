"""Shared loader: reads every arm directory of the Stage B runs.

Stage B exploration script (development run; nothing here tests a hypothesis).
Working directory for outputs and manifests: $GORDIAN_WORK (default /tmp/gordian-exploration).
The repository root is found from this file's location.
"""
import os
import pathlib

import pandas as pd

ROOT = str(pathlib.Path(__file__).resolve().parents[3])
S_DIR = os.environ.get("GORDIAN_WORK", "/tmp/gordian-exploration")
os.makedirs(S_DIR, exist_ok=True)
RUNS = f"{ROOT}/artifacts/runs"
BUDGETS = [20_000_000, 250_000, 100_000, 60_000]
RUN_ID = {b: f"b1-c{b}-s1000-1499" for b in BUDGETS}
ARMS = [
    "heuristic_only", "all_components", "random_p025", "random_p050",
    "fixed_verifier_only", "fixed_estimator_only", "fixed_heuristic_every2",
    "fixed_heuristic_every4", "oracle_evidence_privileged", "oracle_immediate_privileged",
]
PUBLIC = [a for a in ARMS if "privileged" not in a]
CLASSES = [
    "Ambiguous", "DelayedConfigChange", "NoiseFlood", "JointlyDecisive", "NoFault",
    "CriticalFault", "QuietUrgent", "Duplicates", "FeedbackBait", "StaleMemory",
    "ComponentTimeout",
]
STRESSORS = ["NoiseFlood", "Duplicates", "FeedbackBait", "QuietUrgent", "StaleMemory",
             "ComponentTimeout"]
BOOLS = ["success", "critical_miss", "false_alarm", "abstained", "undecided"]


def load_arm(run_dir, arm):
    d = f"{run_dir}/{arm}"
    r = pd.read_csv(f"{d}/results.csv", dtype={"stop_reason": str})
    m = pd.read_csv(f"{d}/measured.csv")
    assert len(r) == len(m)
    df = r.merge(m, on=["seed", "class"], suffixes=("", "_m"), validate="one_to_one")
    assert len(df) == len(r)
    for c in BOOLS:
        df[c] = df[c].astype(str).str.lower().map({"true": 1, "false": 0}).astype(int)
    df["modelled_cost_ns"] = df["modelled_component_ns"] + df["modelled_sched_ns"]
    df["measured_policy_ns"] = df["measured_component_ns"] + df["measured_sched_ns"]
    df["measured_total_ns"] = df["measured_policy_ns"] + df["measured_harness_ns"]
    df["wrong"] = ((df["success"] == 0) & (df["abstained"] == 0) & (df["undecided"] == 0)).astype(int)
    return df


def load_level(run_id):
    run_dir = f"{RUNS}/{run_id}"
    frames = []
    for arm in ARMS:
        df = load_arm(run_dir, arm)
        df["arm"] = arm
        frames.append(df)
    return pd.concat(frames, ignore_index=True)


def load_all():
    frames = []
    for b in BUDGETS:
        df = load_level(RUN_ID[b])
        df["budget"] = b
        frames.append(df)
    out = pd.concat(frames, ignore_index=True)
    out["is_nofault"] = out["class"] == "NoFault"
    return out


def pivot_pairs(df, a, b, budget):
    """Paired rows for arms a and b at one budget, aligned on (seed, class)."""
    x = df[(df.budget == budget) & (df.arm == a)].set_index(["seed", "class"]).sort_index()
    y = df[(df.budget == budget) & (df.arm == b)].set_index(["seed", "class"]).sort_index()
    assert x.index.equals(y.index)
    return x, y
