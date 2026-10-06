"""Write the B1 manifests.

Usage:
  b1_manifests.py xcheck    R6's held-out manifest at b = 5, rho = 0.7, unchanged but for
                            `source_revision`, to replay under its own run id (the byte-identity
                            gate of the Noticer seam)
  b1_manifests.py xcheck-final   the same replay again (`xcheck2-`), with the binary of the final tree
  b1_manifests.py xcheck-r10     R10's held-out manifest at b = 5, rho = 0.7 (62 arms of R6 plus the
                            notice oracle's, six arms), replayed the same way
  b1_manifests.py tune      b = 5, rho = 0.7, the 100 tuning streams (10000-10099): the selection
                            oracle under every noticer configuration of the grid
  b1_manifests.py heldout   b = 5, rho = 0.7, the 200 held-out streams (20000-20199): the selection
                            oracle under every noticer configuration of the grid (the four chosen
                            rows of the table are read from it; the rest is sensitivity)

Exploration (nothing here tests a hypothesis). A manifest records `git rev-parse HEAD` and the driver
refuses a run whose manifest does not match a clean tree, so run this after committing and
immediately before the runs. It starts from `gordian-run init-stream` (environment capture), then
sets the arms, the reasoner's (b, rho), the per-arm noticers and the backstop.
"""

import json
import subprocess
import sys

import b1_common as C

TIMEOUT_S = 3 * 3600


def write(rid, seeds, arms, noticers, trace_rate, run_seed, experiment):
    C.MANIFESTS.mkdir(parents=True, exist_ok=True)
    tmp = C.MANIFESTS / (rid + ".base")
    out = C.MANIFESTS / (rid + ".json")
    if out.exists():
        print(f"{out} exists; kept", file=sys.stderr)
        return
    tmp.unlink(missing_ok=True)
    subprocess.run(
        [
            str(C.BIN), "init-stream", "--run-id", rid, "--arms", "never_escalate=never_escalate",
            "--seed-start", str(seeds[0]), "--seed-count", str(seeds[1]),
            "--trace-sample-rate", str(trace_rate), "--experiment", experiment, "--out", str(tmp),
        ],
        check=True,
        cwd=C.ROOT,
    )
    m = json.load(open(tmp))
    tmp.unlink()
    names = [a[0] for a in arms]
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


def xcheck(source="r6", tag=""):
    """R6's (or R10's) held-out manifest at the primary setting with only `source_revision` replaced
    by HEAD, the run id kept. `tag` names a repeat of the replay (`xcheck2-...`)."""
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=C.ROOT, capture_output=True, text=True,
                          check=True).stdout.strip()
    b, rho = C.PRIMARY
    rid = f"{source}-heldout-{C.setting_id(b, rho)}"
    runs = C.R6_RUNS if source == "r6" else C.R10_RUNS
    m = json.load(open(runs / "_manifests" / f"{rid}.json"))
    assert m["run_id"] == rid
    m["source_revision"] = head
    out = C.MANIFESTS / f"xcheck{tag}-{rid}.json"
    if out.exists():
        raise SystemExit(f"{out} exists")
    C.MANIFESTS.mkdir(parents=True, exist_ok=True)
    with open(out, "w") as fh:
        json.dump(m, fh, indent=2)
        fh.write("\n")
    print(out)


def sweep(stage, seeds, run_seed, experiment):
    d = C.sel_delay_s()
    policy = {"policy": C.SEL_POLICY, "delay_ns": d * C.NS}
    arms, noticers = [], {}
    for name, kind, p in C.grid():
        arm = C.arm_name(name)
        arms.append((arm, policy))
        noticers[arm] = C.spec(kind, **p)
    write(C.run_id(stage), seeds, arms, noticers, 0.0, run_seed, experiment)


def main():
    stage = sys.argv[1] if len(sys.argv) > 1 else ""
    if stage == "xcheck":
        xcheck()
    elif stage == "xcheck-final":
        xcheck("r6", "2")  # the replay again, with the binary of the final tree
    elif stage == "xcheck-r10":
        xcheck("r10", "")  # R10's held-out run (it has the notice oracle R6's does not)
    elif stage == "tune":
        sweep("tune", C.TUNING_SEEDS, 11_100, "exploration-b1-tuning")
    elif stage == "heldout":
        sweep("heldout", C.HELDOUT_SEEDS, 11_200, "exploration-b1-heldout")
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
