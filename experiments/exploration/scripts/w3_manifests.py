#!/usr/bin/env python3
"""W3: write the manifests of the perfect-memory ceiling's runs on world C (item 1, W2's item 6).

Usage: w3_manifests.py RANGE          (ctune | cheldout)

The arm and every setting are W2's (`w2_manifests.py`): the selection oracle at 16 s with the rung's
own context, under B2's re-anchor noticer, at b = 5, rho = 0.7, run seed 14000, trace sample rate 1.
The one difference is the stream parameters: the tier mix is plain 600, hard 300 per mille (world C:
the hard share raised threefold, decoys kept at 100) instead of 800 / 100. Nothing else in
`stream_params` is touched; this script asserts that the manifest it writes differs from the
defaults in the mix only (b = 5 and rho = 0.7 are the defaults).

    scripts/run-driver.sh --manifest artifacts/runs/w3/_manifests/w3-cheldout-b5-rho0.7.json \\
        --out artifacts/runs/w3/w3-cheldout-b5-rho0.7
"""

import json
import pathlib
import subprocess
import sys

import w2_manifests as W2

ROOT = W2.ROOT
MANIFESTS = ROOT / "artifacts" / "runs" / "w3" / "_manifests"
RANGES = {"ctune": (60_000, 100, "exploration-w3-ctune"), "cheldout": (70_000, 200, "exploration-w3-cheldout")}
MIX = {"plain_permille": 600, "hard_permille": 300}


def main():
    which = sys.argv[1] if len(sys.argv) > 1 else ""
    if which not in RANGES:
        raise SystemExit(__doc__)
    start, count, experiment = RANGES[which]
    rid = f"w3-{which}-b5-rho0.7"
    MANIFESTS.mkdir(parents=True, exist_ok=True)
    tmp, out = MANIFESTS / (rid + ".base"), MANIFESTS / (rid + ".json")
    if out.exists():
        raise SystemExit(f"{out} exists; a manifest is never overwritten")
    tmp.unlink(missing_ok=True)
    subprocess.run(
        [str(W2.BIN), "init-stream", "--run-id", rid, "--arms", "never_escalate=never_escalate",
         "--seed-start", str(start), "--seed-count", str(count),
         "--trace-sample-rate", "1", "--experiment", experiment, "--out", str(tmp)],
        check=True, cwd=ROOT)
    m = json.load(open(tmp))
    tmp.unlink()
    default_params = json.loads(json.dumps(m["stream_params"]))
    m["arms"] = [{"arm": W2.ARM, "policy": {"policy": "oracle_selection", "delay_ns": W2.delay_ns()}}]
    m["noticers"] = {W2.ARM: dict(W2.REANCHOR)}
    r = m["stream_params"]["reasoner"]
    r["b"], r["rho"] = W2.B, W2.RHO
    m["stream_params"]["mix"] = dict(MIX)
    m["run_seed"] = W2.RUN_SEED
    m["isolation"]["timeout_secs"] = W2.TIMEOUT_S
    assert m["seeds"] == list(range(start, start + count)), "seeds must be in stream order"
    # The only differences from the defaults in stream_params: the mix, b, rho.
    sp = m["stream_params"]
    diff = sorted(k for k in sp if sp[k] != default_params[k])
    assert diff == ["mix"], diff  # b = 5 and rho = 0.7 are the defaults
    assert sp["reasoner"] == default_params["reasoner"]
    with open(out, "w") as fh:
        json.dump(m, fh, indent=2)
        fh.write("\n")
    print(out)


if __name__ == "__main__":
    main()
