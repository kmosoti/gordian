#!/usr/bin/env python3
"""W2: write the manifests of the perfect-memory ceiling's runs (item 6).

Usage: w2_manifests.py RANGE          (tune | heldout)

One arm per run: the selection oracle at R5's delay (16 s) with the rung's own context, under B2's
re-anchor noticer, at b = 5, rho = 0.7: the arm L1 and M2 call `sel_reanchor_privileged`. The
`heldout` range, 40000-40199, is the range on which L1's fresh run played this arm, so the run is
a replay of that arm (per-stream results must be identical; `w2_ceiling.py --identity` checks it);
`tune` is 10000-10099. The trace sample rate is 1: the per-call context size (the call's modelled
ns) is in the ledger and nowhere else, and the ceiling needs the reasoner's cost per incident.
Nothing is tuned: no parameter of the arm differs from L1's.

A manifest records `git rev-parse HEAD` and the driver refuses a run whose manifest does not match
a clean tree, so run this after committing and immediately before the run:

    scripts/run-driver.sh --manifest artifacts/runs/w2/_manifests/w2-heldout-b5-rho0.7.json \\
        --out artifacts/runs/w2/w2-heldout-b5-rho0.7
"""

import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
BIN = ROOT / "target" / "release" / "gordian-run"
MANIFESTS = ROOT / "artifacts" / "runs" / "w2" / "_manifests"
NS = 1_000_000_000

ARM = "sel_reanchor_privileged"
# B2's comparator in the manifest's spelling (scripts/m2_common.py `REANCHOR`, from b2-selected.json).
REANCHOR = {"noticer": "reanchor", "notice_z": 2.0, "gap_ns": 20_000_000, "min_burst": 2,
            "isolation": "site"}
RANGES = {"tune": (10_000, 100, "exploration-w2-tune"), "heldout": (40_000, 200, "exploration-w2-heldout")}
RUN_SEED = 14_000  # L1's
B, RHO = 5.0, 0.7
TIMEOUT_S = 3600


def delay_ns():
    with open(ROOT / "experiments" / "exploration" / "r5-selected.json") as fh:
        sel = json.load(fh)["b5-rho0.7"]["selection_delay_s"]
    assert sel == 16, sel  # R5's delay at the primary setting, as in every B, M and L table
    return sel * NS


def main():
    which = sys.argv[1] if len(sys.argv) > 1 else ""
    if which not in RANGES:
        raise SystemExit(__doc__)
    start, count, experiment = RANGES[which]
    rid = f"w2-{which}-b5-rho0.7"
    MANIFESTS.mkdir(parents=True, exist_ok=True)
    tmp, out = MANIFESTS / (rid + ".base"), MANIFESTS / (rid + ".json")
    if out.exists():
        raise SystemExit(f"{out} exists; a manifest is never overwritten")
    tmp.unlink(missing_ok=True)
    subprocess.run(
        [str(BIN), "init-stream", "--run-id", rid, "--arms", "never_escalate=never_escalate",
         "--seed-start", str(start), "--seed-count", str(count),
         "--trace-sample-rate", "1", "--experiment", experiment, "--out", str(tmp)],
        check=True, cwd=ROOT)
    m = json.load(open(tmp))
    tmp.unlink()
    m["arms"] = [{"arm": ARM, "policy": {"policy": "oracle_selection", "delay_ns": delay_ns()}}]
    m["noticers"] = {ARM: dict(REANCHOR)}
    r = m["stream_params"]["reasoner"]
    r["b"], r["rho"] = B, RHO
    m["run_seed"] = RUN_SEED
    m["isolation"]["timeout_secs"] = TIMEOUT_S
    assert m["seeds"] == list(range(start, start + count)), "seeds must be in stream order"
    with open(out, "w") as fh:
        json.dump(m, fh, indent=2)
        fh.write("\n")
    print(out)


if __name__ == "__main__":
    main()
