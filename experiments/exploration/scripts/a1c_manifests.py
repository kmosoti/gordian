"""Write A1c's smoke manifest (L1's conventions; see `a1c_common`).

Usage: a1c_manifests.py smoke

The manifest records `git rev-parse HEAD` and the driver refuses a run whose manifest does not
match a clean tree, so run this after committing and immediately before the run.
"""

import json
import subprocess
import sys

import a1c_common as C

TIMEOUT_S = 2 * 3600


def main():
    if sys.argv[1:] != ["smoke"]:
        sys.exit(__doc__)
    rid = C.run_id("smoke")
    seeds = C.SMOKE_SEEDS
    C.MANIFESTS.mkdir(parents=True, exist_ok=True)
    tmp = C.MANIFESTS / (rid + ".base")
    out = C.MANIFESTS / (rid + ".json")
    if out.exists():
        raise SystemExit(f"{out} exists; a manifest is never overwritten")
    tmp.unlink(missing_ok=True)
    subprocess.run(
        [
            str(C.BIN), "init-stream", "--run-id", rid, "--arms", "never_escalate=never_escalate",
            "--seed-start", str(seeds[0]), "--seed-count", str(seeds[1]),
            "--trace-sample-rate", "0", "--experiment", "exploration-a1c-smoke", "--out", str(tmp),
        ],
        check=True,
        cwd=C.ROOT,
    )
    m = json.load(open(tmp))
    tmp.unlink()
    policy = {"policy": C.SEL_POLICY, "delay_ns": C.sel_delay_s() * C.NS}
    arms = C.arms()
    names = [C.arm_name(n) for n, _ in arms]
    assert len(set(names)) == len(names), "duplicate arm names"
    keys = [n["engram"]["state_key"] for _, n in arms if "engram" in n]
    assert len(set(keys)) == len(keys), "every engram arm carries under its own key"
    m["arms"] = [{"arm": a, "policy": policy} for a in names]
    m["noticers"] = {C.arm_name(n): noticer for n, noticer in arms}
    b, rho = C.PRIMARY
    r = m["stream_params"]["reasoner"]
    r["b"] = b
    r["rho"] = rho
    m["run_seed"] = C.RUN_SEED
    m["isolation"]["timeout_secs"] = TIMEOUT_S
    assert m["seeds"] == list(range(seeds[0], seeds[0] + seeds[1])), "seeds in stream order"
    with open(out, "w") as fh:
        json.dump(m, fh, indent=2)
        fh.write("\n")
    print(out)


if __name__ == "__main__":
    main()
