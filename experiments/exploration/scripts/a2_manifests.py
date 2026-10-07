"""Write A2's manifests (L1's conventions; see `a2_common`).

Usage:
  a2_manifests.py smoke      the smoke run: the memoryless arm, the learner and its control
                             (`a2_common.arms()`), under the selection oracle at 16 s
  a2_manifests.py r6         R6's held-out manifest (b = 5, rho = 0.7), `source_revision` replaced
                             by HEAD, run id kept: the byte-identity gate
  a2_manifests.py a1creplay  A1c's kept smoke manifest (A1a's eight arms and A1c's seven),
                             `source_revision` replaced by HEAD, run id `a2-a1creplay-b5-rho0.7`
  a2_manifests.py a1dreplay  A1d's kept smoke manifest (15 arms, 12 trace files),
                             `source_revision` replaced by HEAD, run id `a2-a1dreplay-b5-rho0.7`

The manifest records `git rev-parse HEAD` and the driver refuses a run whose manifest does not
match a clean tree, so run this after committing and immediately before the run.
"""

import json
import subprocess
import sys

import a2_common as C

TIMEOUT_S = 2 * 3600


def head():
    return subprocess.run(["git", "rev-parse", "HEAD"], cwd=C.ROOT, capture_output=True, text=True,
                          check=True).stdout.strip()


def write(name, m):
    C.MANIFESTS.mkdir(parents=True, exist_ok=True)
    out = C.MANIFESTS / (name + ".json")
    if out.exists():
        raise SystemExit(f"{out} exists; a manifest is never overwritten")
    with open(out, "w") as fh:
        json.dump(m, fh, indent=2)
        fh.write("\n")
    print(out)


def smoke():
    rid = C.run_id("smoke")
    seeds = C.SMOKE_SEEDS
    C.MANIFESTS.mkdir(parents=True, exist_ok=True)
    tmp = C.MANIFESTS / (rid + ".base")
    tmp.unlink(missing_ok=True)
    subprocess.run(
        [
            str(C.BIN), "init-stream", "--run-id", rid, "--arms", "never_escalate=never_escalate",
            "--seed-start", str(seeds[0]), "--seed-count", str(seeds[1]),
            "--trace-sample-rate", "0", "--experiment", "exploration-a2-smoke", "--out", str(tmp),
        ],
        check=True,
        cwd=C.ROOT,
    )
    m = json.load(open(tmp))
    tmp.unlink()
    policy = {"policy": C.A.SEL_POLICY, "delay_ns": C.DELAY_S * C.NS}
    arms = C.arms()
    names = [C.arm_name(f) for f, _ in arms]
    assert len(set(names)) == len(names), "duplicate arm names"
    keys = [n["anticipation"]["trace_key"] for _, n in arms if "anticipation" in n]
    assert len(set(keys)) == len(keys), "every layer writes its own trace file"
    m["arms"] = [{"arm": a, "policy": policy} for a in names]
    m["noticers"] = {C.arm_name(f): noticer for f, noticer in arms}
    b, rho = C.PRIMARY
    r = m["stream_params"]["reasoner"]
    r["b"] = b
    r["rho"] = rho
    m["run_seed"] = C.RUN_SEED
    m["isolation"]["timeout_secs"] = TIMEOUT_S
    assert m["seeds"] == list(range(seeds[0], seeds[0] + seeds[1])), "seeds in stream order"
    write(rid, m)


def replay(kept, stage):
    m = json.load(open(kept / "manifest.json"))
    assert m["run_id"] == kept.name
    m["run_id"] = C.run_id(stage)
    m["source_revision"] = head()
    write(m["run_id"], m)


def r6():
    rid = f"r6-heldout-{C.setting_id(*C.PRIMARY)}"
    m = json.load(open(C.M3.R6_RUNS / "_manifests" / f"{rid}.json"))
    assert m["run_id"] == rid
    m["source_revision"] = head()
    write(f"a2-xcheck-{rid}", m)


if __name__ == "__main__":
    stage = sys.argv[1] if len(sys.argv) > 1 else ""
    {
        "smoke": smoke,
        "r6": r6,
        "a1creplay": lambda: replay(C.A1C_SMOKE, "a1creplay"),
        "a1dreplay": lambda: replay(C.A1D_SMOKE, "a1dreplay"),
    }.get(stage, lambda: sys.exit(__doc__))()
