"""Write the R4 manifests.

Usage:
  r4_manifests.py pilot                      one setting (b=5, rho=0.7), 10 tuning streams, every arm
  r4_manifests.py tune                       every setting, the 100 tuning streams, every arm
  r4_manifests.py heldout SELECTED.json      every setting, the 200 held-out streams, the arms
                                             SELECTED.json lists per setting (see r4_select.py)

Exploration (nothing here tests a hypothesis). A manifest records `git rev-parse HEAD` and the
driver refuses a run whose manifest does not match a clean tree, so run this after committing and
immediately before the run. It starts from `gordian-run init-stream` (environment capture), then
sets the arms, the reasoner's (b, rho), the sampling rate and the backstop.
"""

import json
import subprocess
import sys

import r4_common as C

TIMEOUT_S = 6 * 3600


def write(stage, b, rho, seeds, arms, trace_rate, run_seed, experiment):
    rid = C.run_id(stage, b, rho)
    C.MANIFESTS.mkdir(parents=True, exist_ok=True)
    tmp = C.MANIFESTS / (rid + ".base")
    out = C.MANIFESTS / (rid + ".json")
    if out.exists():
        raise SystemExit(f"{out} exists; a recorded manifest is not overwritten")
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
    m["arms"] = [{"arm": n, "policy": p} for n, p in arms]
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
    stage = sys.argv[1]
    if stage == "pilot":
        b, rho = 5.0, 0.7
        write("pilot", b, rho, (C.TUNING_SEEDS[0], 10), C.baseline_arms() + C.reference_arms(), 0.0,
              4000, "exploration-r4-pilot")
    elif stage == "tune":
        for i, (b, rho) in enumerate(C.SETTINGS):
            write("tune", b, rho, C.TUNING_SEEDS, C.baseline_arms() + C.reference_arms(), 0.0,
                  4100 + i, "exploration-r4-tuning")
    elif stage == "heldout":
        selected = C.load_json(sys.argv[2])
        for i, (b, rho) in enumerate(C.SETTINGS):
            names = selected[C.setting_id(b, rho)]
            arms = [(n, C.policy_for(n)) for n in names]
            for ref in (C.NEVER, C.ORACLE, C.ABLATION):
                if ref not in names:
                    arms.append((ref, C.policy_for(ref)))
            write("heldout", b, rho, C.HELDOUT_SEEDS, arms, 0.01, 4200 + i, "exploration-r4-heldout")
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
