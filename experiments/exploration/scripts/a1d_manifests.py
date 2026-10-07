"""Write A1d's manifests (L1's conventions; see `a1d_common`).

Usage:
  a1d_manifests.py smoke      the smoke run: three selectors x five arms (`a1d_common.arms()`)
  a1d_manifests.py a1creplay  A1c's kept smoke manifest (A1a's eight arms and A1c's seven), with
                              `source_revision` replaced by HEAD and the run id
                              `a1d-a1creplay-b5-rho0.7`: the reproduction of A1a's and A1c's arms
  a1d_manifests.py r6         R6's held-out manifest (b = 5, rho = 0.7), `source_revision` replaced
                              by HEAD, run id kept: the byte-identity gate

The manifest records `git rev-parse HEAD` and the driver refuses a run whose manifest does not
match a clean tree, so run this after committing and immediately before the run.
"""

import json
import subprocess
import sys

import a1d_common as C

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
            "--trace-sample-rate", "0", "--experiment", "exploration-a1d-smoke", "--out", str(tmp),
        ],
        check=True,
        cwd=C.ROOT,
    )
    m = json.load(open(tmp))
    tmp.unlink()
    arms = C.arms()
    names = [a[0] for a in arms]
    assert len(set(names)) == len(names), "duplicate arm names"
    keys = [a[4]["engram"]["state_key"] for a in arms if "engram" in a[4]]
    assert len(set(keys)) == len(keys), "every engram arm carries under its own key"
    m["arms"] = [{"arm": name, "policy": policy} for name, _, _, policy, _ in arms]
    m["noticers"] = {name: noticer for name, _, _, _, noticer in arms}
    b, rho = C.PRIMARY
    r = m["stream_params"]["reasoner"]
    r["b"] = b
    r["rho"] = rho
    m["run_seed"] = C.RUN_SEED
    m["isolation"]["timeout_secs"] = TIMEOUT_S
    assert m["seeds"] == list(range(seeds[0], seeds[0] + seeds[1])), "seeds in stream order"
    write(rid, m)


def a1creplay():
    m = json.load(open(C.A1C_SMOKE / "manifest.json"))
    assert m["run_id"] == "a1c-smoke-b5-rho0.7"
    m["run_id"] = C.run_id("a1creplay")
    m["source_revision"] = head()
    write(m["run_id"], m)


def r6():
    rid = f"r6-heldout-{C.setting_id(*C.PRIMARY)}"
    m = json.load(open(C.M3.R6_RUNS / "_manifests" / f"{rid}.json"))
    assert m["run_id"] == rid
    m["source_revision"] = head()
    write(f"a1d-xcheck-{rid}", m)


if __name__ == "__main__":
    stage = sys.argv[1] if len(sys.argv) > 1 else ""
    {"smoke": smoke, "a1creplay": a1creplay, "r6": r6}.get(stage, lambda: sys.exit(__doc__))()
