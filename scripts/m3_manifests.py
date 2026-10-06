"""Write M3 manifests.

Usage: m3_manifests.py STAGE

Stages are defined in `m3_grids.py` (tuning grids, on the tuning streams 10000-10099) and in
`m3_common.heldout()` (the held-out run, the 200 streams 20000-20199). Every manifest is the
selection oracle at R5's delay (16 s) with the rung's own context, under the noticer each arm names,
at b = 5, rho = 0.7, as B1's table. A manifest records `git rev-parse HEAD` and the driver refuses a
run whose manifest does not match a clean tree, so run this after committing and immediately before
the run. It starts from `gordian-run init-stream` (environment capture), then sets the arms, the
reasoner's (b, rho), the per-arm noticers and the backstop.
"""

import json
import subprocess
import sys

import m3_common as C

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
    with open(out, "w") as fh:
        json.dump(m, fh, indent=2)
        fh.write("\n")
    print(out)


def main():
    stage = sys.argv[1] if len(sys.argv) > 1 else ""
    if stage.startswith("heldout"):
        arms, seeds, run_seed, experiment = C.heldout(stage)
    else:
        import m3_grids as G
        arms, seeds, run_seed, experiment = G.stage(stage)
    policy = {"policy": C.SEL_POLICY, "delay_ns": C.sel_delay_s() * C.NS}
    noticers = {}
    rows = []
    for name, noticer in arms:
        arm = C.arm_name(name)
        rows.append((arm, policy))
        noticers[arm] = noticer
    write(C.run_id(stage), seeds, rows, noticers, run_seed, experiment)


if __name__ == "__main__":
    main()
