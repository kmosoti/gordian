"""Write one multi-arm exploration manifest: mkmanifest.py RUN_ID COMPUTE SEED_START SEED_COUNT RUN_SEED TIMEOUT OUT [ARMSET [NOISE_RATE]]

Stage B exploration script (development run; nothing here tests a hypothesis).
Working directory for outputs and manifests: $GORDIAN_WORK (default /tmp/gordian-exploration).
The repository root is found from this file's location.
"""
import json
import subprocess
import sys

run_id, compute, seed_start, seed_count, run_seed, timeout, out = sys.argv[1:8]
armset = sys.argv[8] if len(sys.argv) > 8 else "b1"
import pathlib
root = str(pathlib.Path(__file__).resolve().parents[3])
tmp = out + ".base"
subprocess.run(
    [root + "/target/release/gordian-run", "init", "--run-id", run_id, "--policy", "heuristic_only",
     "--seed-start", seed_start, "--seed-count", seed_count, "--out", tmp,
     "--experiment", "exploration-b1", "--trace-sample-rate", "0.005"],
    check=True, cwd=root)
m = json.load(open(tmp))
m.pop("arm"), m.pop("policy")


def fp(components, every):
    return {"policy": "fixed_pipeline", "components": components, "every": every}


m["arms"] = [
    {"arm": "heuristic_only", "policy": "heuristic_only"},
    {"arm": "all_components", "policy": "all_components"},
    {"arm": "random_p025", "policy": {"policy": "random_matched", "p": 0.25}},
    {"arm": "random_p050", "policy": {"policy": "random_matched", "p": 0.5}},
    {"arm": "fixed_verifier_only", "policy": fp(["verifier"], 1)},
    {"arm": "fixed_estimator_only", "policy": fp(["estimator"], 1)},
    {"arm": "fixed_heuristic_every2", "policy": fp(["heuristic"], 2)},
    {"arm": "fixed_heuristic_every4", "policy": fp(["heuristic"], 4)},
    {"arm": "oracle_evidence_privileged", "policy": "oracle_evidence"},
    {"arm": "oracle_immediate_privileged", "policy": "oracle_immediate"},
]
if armset == "s1":
    # Supplementary: the cost floor (select nothing), longer periods, and a control copy of
    # heuristic_only to confirm results.csv does not depend on run seed or on the other arms.
    m["arms"] = [
        {"arm": "heuristic_only", "policy": "heuristic_only"},
        {"arm": "random_p000", "policy": {"policy": "random_matched", "p": 0.0}},
        {"arm": "fixed_heuristic_every8", "policy": fp(["heuristic"], 8)},
        {"arm": "fixed_heuristic_every16", "policy": fp(["heuristic"], 16)},
    ]
    m["experiment"] = "exploration-b1-supplementary"
if armset == "s2":
    # Supplementary budget sweep below B1's lowest level, around the cost of the cheap pipelines.
    m["arms"] = [
        {"arm": "heuristic_only", "policy": "heuristic_only"},
        {"arm": "fixed_heuristic_every2", "policy": fp(["heuristic"], 2)},
        {"arm": "fixed_heuristic_every4", "policy": fp(["heuristic"], 4)},
        {"arm": "fixed_heuristic_every8", "policy": fp(["heuristic"], 8)},
        {"arm": "fixed_heuristic_every16", "policy": fp(["heuristic"], 16)},
        {"arm": "fixed_estimator_only", "policy": fp(["estimator"], 1)},
        {"arm": "all_components", "policy": "all_components"},
        {"arm": "random_p025", "policy": {"policy": "random_matched", "p": 0.25}},
        {"arm": "oracle_evidence_privileged", "policy": "oracle_evidence"},
    ]
    m["experiment"] = "exploration-b1-supplementary"
if len(sys.argv) > 9:
    # Supplementary dose probe: the generator's noise rate (a manifest parameter, no code change).
    m["episode_params"]["noise_rate"] = int(sys.argv[9])
    m["experiment"] = "exploration-b3-supplementary"
m["run_seed"] = int(run_seed)
m["limits"]["compute"] = int(compute)
m["isolation"]["timeout_secs"] = int(timeout)
json.dump(m, open(out, "w"), indent=2)
open(out, "a").write("\n")
