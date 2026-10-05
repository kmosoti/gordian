"""Write the R7 manifests.

Usage:
  r7_manifests.py tune                  the six runs (primary setting at every delta, then b = 2.5
                                        and b = 8 at delta 0.2), the 100 tuning streams: the selection
                                        oracle at R6's delay with every builder configuration of R6's
                                        grids, the context-only ceiling at that delay, R4's oracle
  r7_manifests.py heldout R7-SELECTED.json
                                        the same six runs, the 200 held-out streams: the carried
                                        configurations of every builder (`r7_select.py`) with the
                                        selection oracle, the context-only ceiling, R4's oracle
  r7_manifests.py gate                  R6's held-out manifest at b = 5, rho = 0.7 with only
                                        `source_revision` replaced (test 1, delta = 0)

Exploration (nothing here tests a hypothesis). A manifest records `git rev-parse HEAD` and the driver
refuses a run whose manifest does not match a clean tree, so run this after committing and
immediately before the runs. It follows `r6_manifests.write` (start from `gordian-run init-stream`
for the environment capture, then set the arms and the reasoner's `(b, rho)`), with the distractor
penalty `stream_params.reasoner.distractor_penalty` added. There are no `always_escalate` arms.
"""

import json
import subprocess
import sys

import r6_manifests as M6
import r7_common as C

TIMEOUT_S = 6 * 3600
R6_GATE_SOURCE = "/home/user/gordian/artifacts/runs/r6/_manifests/r6-heldout-b5-rho0.7.json"


def write(stage, b, rho, delta, seeds, arms, run_seed, experiment):
    rid = C.run_id(stage, b, rho, delta)
    C.MANIFESTS.mkdir(parents=True, exist_ok=True)
    tmp = C.MANIFESTS / (rid + ".base")
    out = C.MANIFESTS / (rid + ".json")
    if out.exists():
        # Written, and perhaps run, at an earlier revision: the manifest of a run that was made is
        # never touched; one the driver refused is set aside by hand first (`*.unrun-*`).
        print(f"{out} exists; kept", file=sys.stderr)
        return
    tmp.unlink(missing_ok=True)
    subprocess.run(
        [
            str(C.BIN), "init-stream", "--run-id", rid, "--arms", "never_escalate=never_escalate",
            "--seed-start", str(seeds[0]), "--seed-count", str(seeds[1]),
            "--trace-sample-rate", "0.0", "--experiment", experiment, "--out", str(tmp),
        ],
        check=True,
        cwd=C.ROOT,
    )
    m = json.load(open(tmp))
    tmp.unlink()
    names = [a[0] for a in arms]
    if len(set(names)) != len(names):
        raise SystemExit("duplicate arm names")
    m["arms"] = [M6.arm_entry(*a) for a in arms]
    r = m["stream_params"]["reasoner"]
    r["b"] = b
    r["rho"] = rho
    r["distractor_penalty"] = delta
    m["run_seed"] = run_seed
    m["isolation"]["timeout_secs"] = TIMEOUT_S
    with open(out, "w") as fh:
        json.dump(m, fh, indent=2)
        fh.write("\n")
    print(out)


def tune():
    for k, (b, rho, delta) in enumerate(C.RUNS_SPEC):
        d = C.sel_delay_s(b, rho)
        arms = [C.C6.sel_arm(key, d) for key, _, _ in C.C6.all_builders()]
        arms.append(C.C6.ctxonly_arm(d))
        arms.append(C.C6.reference_arms()[0])
        write("tune", b, rho, delta, C.TUNING_SEEDS, arms, C.TUNE_RUN_SEED0 + k, "exploration-r7-tuning")


def heldout(selected):
    for k, (b, rho, delta) in enumerate(C.RUNS_SPEC):
        s = selected[C.run_id("tune", b, rho, delta)]
        d = s["selection_delay_s"]
        arms = [C.C6.sel_arm(key, d) for key in s["carried"]]
        arms.append(C.C6.ctxonly_arm(d))
        arms.append(C.C6.reference_arms()[0])
        write("heldout", b, rho, delta, C.HELDOUT_SEEDS, arms, C.HELDOUT_RUN_SEED0 + k, "exploration-r7-heldout")


def gate():
    """R6's held-out manifest for b = 5, rho = 0.7 with `source_revision` replaced by HEAD."""
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=C.ROOT, capture_output=True, text=True,
                          check=True).stdout.strip()
    m = json.load(open(R6_GATE_SOURCE))
    m["source_revision"] = head
    out = C.MANIFESTS / "r7-gate-r6-heldout-b5-rho0.7.json"
    if out.exists():
        raise SystemExit(f"{out} exists")
    with open(out, "w") as fh:
        json.dump(m, fh, indent=2)
        fh.write("\n")
    print(out)


def main():
    stage = sys.argv[1]
    if stage == "tune":
        tune()
    elif stage == "heldout":
        heldout(C.load_json(sys.argv[2]))
    elif stage == "gate":
        gate()
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
