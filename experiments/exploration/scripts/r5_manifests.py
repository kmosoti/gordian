"""Write the R5 manifests.

Usage:
  r5_manifests.py tune                       every setting, the 100 tuning streams: the contradiction
                                             arm's delay x persistence grid and the selection
                                             oracle's delay grid
  r5_manifests.py heldout R5-SELECTED.json   every setting, the 200 held-out streams: R4's
                                             frontier arms, the contradiction arm's tuned frontier,
                                             the selection oracle at its tuned delay, the decoy
                                             oracle, R4's oracle, never_escalate, the ablation
  r5_manifests.py suppl                      the selection oracle at every delay of its grid on the
                                             held-out streams (a labelled sensitivity run; the
                                             criterion uses the tuned delay only)
  r5_manifests.py diag                       b = 5, rho = 0.7, 100 diagnostic streams (seeds
                                             30000-30099) with every ledger kept, for the trace

Exploration (nothing here tests a hypothesis). A manifest records `git rev-parse HEAD` and the
driver refuses a run whose manifest does not match a clean tree, so run this after committing and
immediately before the run. It starts from `gordian-run init-stream` (environment capture), then
sets the arms, the reasoner's (b, rho), the sampling rate and the backstop.
"""

import json
import subprocess
import sys

import r5_common as C

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
    names = [n for n, _ in arms]
    if len(set(names)) != len(names):
        raise SystemExit("duplicate arm names")
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


def heldout_arms(selected, sid):
    """R4's frontier arms, then R5's, then the references, without repeats and in a fixed order."""
    s = selected[sid]
    names = list(s["r4_arms"]) + list(s["contradiction_arms"])
    names.append(C.sel_name(s["selection_delay_s"]))
    names.append(C.DECOY)
    for ref in (C.NEVER, C.ORACLE, C.ABLATION):
        if ref not in names:
            names.append(ref)
    seen = []
    for n in names:
        if n not in seen:
            seen.append(n)
    return [(n, C.policy_for(n)) for n in seen]


def main():
    stage = sys.argv[1]
    if stage == "tune":
        for i, (b, rho) in enumerate(C.SETTINGS):
            write("tune", b, rho, C.TUNING_SEEDS, C.contra_arms() + C.sel_arms(), 0.0, 5100 + i,
                  "exploration-r5-tuning")
    elif stage == "heldout":
        selected = C.load_json(sys.argv[2])
        for i, (b, rho) in enumerate(C.SETTINGS):
            write("heldout", b, rho, C.HELDOUT_SEEDS, heldout_arms(selected, C.setting_id(b, rho)),
                  0.01, 5200 + i, "exploration-r5-heldout")
    elif stage == "suppl":
        for i, (b, rho) in enumerate(C.SETTINGS):
            write("suppl", b, rho, C.HELDOUT_SEEDS, C.sel_arms(), 0.0, 5300 + i,
                  "exploration-r5-sensitivity")
    elif stage == "diag":
        b, rho = C.PRIMARY
        selected = C.load_json(sys.argv[2])
        s = selected[C.setting_id(b, rho)]
        arms = [(C.C4.always_name(6), C.policy_for(C.C4.always_name(6))),
                (C.C4.always_name(14), C.policy_for(C.C4.always_name(14))),
                (C.sel_name(s["selection_delay_s"]), C.policy_for(C.sel_name(s["selection_delay_s"])))]
        arms += [(n, C.policy_for(n)) for n in s["diag_contradiction_arms"]]
        arms.append((C.ORACLE, C.policy_for(C.ORACLE)))
        write("diag", b, rho, C.DIAG_SEEDS, arms, 1.0, 5400, "exploration-r5-trace")
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
