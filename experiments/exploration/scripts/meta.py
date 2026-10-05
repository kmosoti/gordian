"""Run metadata: hashes, drift diagnostics, usage, for the B1 md.

Stage B exploration script (development run; nothing here tests a hypothesis).
Working directory for outputs and manifests: $GORDIAN_WORK (default /tmp/gordian-exploration).
The repository root is found from this file's location.
"""
import hashlib
import json
import subprocess
import sys

import os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import *  # noqa  (ROOT, S_DIR, loaders)

VENV = os.environ.get("GORDIAN_ANALYZE", "gordian-analyze")


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


runs = [("pilot", "b1-pilot-c20000000-s1000-1099")]
runs += [(f"B1 {b:,}", RUN_ID[b]) for b in BUDGETS]
runs += [(f"S1 {b:,}", f"b1s-c{b}-s1000-1499") for b in BUDGETS]
runs += [(f"S2 {b:,}", f"b1s2-c{b}-s1000-1499") for b in (15000, 20000, 30000, 40000, 50000)]
runs += [(f"S3 {b:,}", f"b3n50-c{b}-s1000-1499") for b in BUDGETS]

out = []
for label, rid in runs:
    d = f"{RUNS}/{rid}"
    man = json.load(open(f"{d}/manifest.json"))
    use = json.load(open(f"{d}/usage.json"))
    subprocess.run([VENV, "drift", "--run", d, "--json", f"{S_DIR}/drift.json"], check=True,
                   stdout=subprocess.DEVNULL)
    dr = json.load(open(f"{S_DIR}/drift.json"))
    rec = dict(
        label=label, run_id=rid, source_revision=man["source_revision"][:7],
        compute=man["limits"]["compute"], run_seed=man["run_seed"],
        cpu_model=man.get("cpu_model"), cpu_mhz=man.get("cpu_mhz"),
        manifest_sha256=sha(f"{d}/manifest.json"),
        arms={a["arm"]: sha(f"{d}/{a['arm']}/results.csv") for a in man["arms"]},
        drift_cv_ns=dr["ns"]["cv"], drift_ratio_ns=dr["ns"]["ratio_last_first"],
        drift_cv_min=dr["min_ns"]["cv"], drift_ratio_min=dr["min_ns"]["ratio_last_first"],
        drift_blocks=dr["n_blocks"], wall_s=use["wall_ns"] / 1e9, cpu_s=use["cpu_ns"] / 1e9,
        ratio=use["internal_external_ratio"], peak_mb=use["peak_memory_bytes"] / 2**20,
        exit=use["exit_code"], oom=use["oom_kills"], isolation=use["isolation"],
    )
    out.append(rec)
json.dump(out, open(f"{S_DIR}/meta.json", "w"), indent=1)
for r in out:
    print(r["label"], r["run_id"], r["manifest_sha256"][:12], r["cpu_model"], r["cpu_mhz"],
          f"cvns={r['drift_cv_ns']:.3f} l/f={r['drift_ratio_ns']:.3f} cvmin={r['drift_cv_min']:.3f} l/f={r['drift_ratio_min']:.3f}",
          f"wall={r['wall_s']:.1f} cpu={r['cpu_s']:.1f} ratio={r['ratio']:.3f} peak={r['peak_mb']:.1f}MB exit={r['exit']} oom={r['oom']}")
