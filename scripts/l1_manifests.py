"""Write L1 manifests.

Usage: l1_manifests.py STAGE     (dev | fresh; see `l1_common.stage`)

Every manifest is the selection oracle at R5's delay (16 s) with the rung's own context, under the
noticer each arm names, at b = 5, rho = 0.7, as B1's, B2's and M2's tables. The seeds are in
stream order (ascending): the learned arms carry what they learn from one segment to the next in
that order. A manifest records `git rev-parse HEAD` and the driver refuses a run whose manifest
does not match a clean tree, so run this after committing and immediately before the run.
"""

import json
import subprocess
import sys

import l1_common as C

TIMEOUT_S = 4 * 3600


def write(rid, seeds, arms, noticers, run_seed, experiment):
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
            "--trace-sample-rate", "0", "--experiment", experiment, "--out", str(tmp),
        ],
        check=True,
        cwd=C.ROOT,
    )
    m = json.load(open(tmp))
    tmp.unlink()
    names = [a for a, _ in arms]
    if len(set(names)) != len(names):
        raise SystemExit("duplicate arm names")
    m["arms"] = [{"arm": name, "policy": policy} for name, policy in arms]
    m["noticers"] = noticers
    b, rho = C.PRIMARY
    r = m["stream_params"]["reasoner"]
    r["b"] = b
    r["rho"] = rho
    m["run_seed"] = run_seed
    m["isolation"]["timeout_secs"] = TIMEOUT_S
    assert m["seeds"] == list(range(seeds[0], seeds[0] + seeds[1])), "seeds must be in stream order"
    with open(out, "w") as fh:
        json.dump(m, fh, indent=2)
        fh.write("\n")
    print(out)


def main():
    stage = sys.argv[1] if len(sys.argv) > 1 else ""
    arms, seeds, run_seed, experiment = C.stage(stage)
    policy = {"policy": C.M.SEL_POLICY, "delay_ns": C.M.sel_delay_s() * C.NS}
    noticers = {}
    rows = []
    for name, noticer in arms:
        arm = C.arm_name(name)
        rows.append((arm, policy))
        noticers[arm] = noticer
    write(C.run_id(stage), seeds, rows, noticers, run_seed, experiment)


if __name__ == "__main__":
    main()
