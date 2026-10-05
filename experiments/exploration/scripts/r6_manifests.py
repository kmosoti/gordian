"""Write the R6 manifests.

Usage:
  r6_manifests.py tune1                  every setting, the 100 tuning streams: the selection oracle
                                         (at its R5-tuned delay, r5-selected.json) with every builder
                                         configuration of the sweep, the context-only ceiling at that
                                         delay, R4's oracle, never_escalate
  r6_manifests.py pilot                  b = 5, rho = 0.7, 10 tuning streams, the tune1 arms (to size the work)
  r6_manifests.py tune2 R6-FRONTIER.json every setting, the 100 tuning streams: `always_escalate`
                                         over its delay grid, for every builder configuration on a
                                         tuning frontier (and the rung)
  r6_manifests.py heldout R6-SELECTED.json
                                         every setting, the 200 held-out streams: the frontier
                                         configurations of every builder with the selection oracle and
                                         with `always_escalate` at its tuned delay, the context-only
                                         ceiling, R4's oracle, never_escalate, the ablation
  r6_manifests.py diag R6-SELECTED.json  b = 5, rho = 0.7, 100 diagnostic streams (seeds 31000-31099),
                                         every ledger kept, for the re-trace
  r6_manifests.py xcheck                 R5's six held-out manifests, unchanged but for
                                         `source_revision`, to replay under their own run ids

Exploration (nothing here tests a hypothesis). A manifest records `git rev-parse HEAD` and the driver
refuses a run whose manifest does not match a clean tree, so run this after committing and
immediately before the run. It starts from `gordian-run init-stream` (environment capture), then
sets the arms (each with its context builder), the reasoner's (b, rho), the sampling rate and the
backstop.
"""

import json
import subprocess
import sys

import r6_common as C

TIMEOUT_S = 6 * 3600
R5_RUNS = C.ROOT / "artifacts" / "runs" / "r5"


def arm_entry(name, policy, context):
    e = {"arm": name, "policy": policy}
    if context is not None:
        e["context"] = context
    return e


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
    names = [a[0] for a in arms]
    if len(set(names)) != len(names):
        raise SystemExit("duplicate arm names")
    m["arms"] = [arm_entry(*a) for a in arms]
    r = m["stream_params"]["reasoner"]
    r["b"] = b
    r["rho"] = rho
    m["run_seed"] = run_seed
    m["isolation"]["timeout_secs"] = TIMEOUT_S
    with open(out, "w") as fh:
        json.dump(m, fh, indent=2)
        fh.write("\n")
    print(out)


def r5_delay(sid):
    return C.load_json(C.OUT / "r5-selected.json")[sid]["selection_delay_s"]


def tune1():
    for i, (b, rho) in enumerate(C.SETTINGS):
        d = r5_delay(C.setting_id(b, rho))
        arms = [C.sel_arm(key, d) for key, _, _ in C.all_builders()]
        arms.append(C.ctxonly_arm(d))
        arms += [a for a in C.reference_arms() if a[0] in (C.ORACLE, C.NEVER)]
        write("tune1", b, rho, C.TUNING_SEEDS, arms, 0.0, 6100 + i, "exploration-r6-tuning")


def pilot():
    b, rho = C.PRIMARY
    d = r5_delay(C.setting_id(b, rho))
    arms = [C.sel_arm(key, d) for key, _, _ in C.all_builders()]
    arms.append(C.ctxonly_arm(d))
    arms += [a for a in C.reference_arms() if a[0] in (C.ORACLE, C.NEVER)]
    # The pilot ran at 1d0107f with the first grid (cooccur and neighbourhood without a cap axis,
    # keys `coc_d02000` and `nbh_k2`); its recorded manifest is the record, this line is not it.
    arms += [C.always_arm(key, 14) for key in ("rung", "win_w20_n064", "coc_d02000_n128", "nbh_k2_n128")]
    write("pilot", b, rho, (C.TUNING_SEEDS[0], 10), arms, 0.0, 6000, "exploration-r6-pilot")


def tune2(frontier):
    for i, (b, rho) in enumerate(C.SETTINGS):
        sid = C.setting_id(b, rho)
        keys = frontier[sid]["configs"]  # every builder configuration to give `always` delays to
        arms = [C.always_arm(key, d) for key in keys for d in C.ALWAYS_DELAYS_S]
        write("tune2", b, rho, C.TUNING_SEEDS, arms, 0.0, 6200 + i, "exploration-r6-tuning-always")


def heldout_arms(selected, sid):
    s = selected[sid]
    arms = []
    for key in s["sel_configs"]:
        arms.append(C.sel_arm(key, s["selection_delay_s"]))
    for key, d in s["always_configs"].items():
        arms.append(C.always_arm(key, d))
    arms.append(C.ctxonly_arm(s["selection_delay_s"]))
    arms += C.reference_arms()
    return arms


def heldout(selected):
    for i, (b, rho) in enumerate(C.SETTINGS):
        write("heldout", b, rho, C.HELDOUT_SEEDS, heldout_arms(selected, C.setting_id(b, rho)), 0.0,
              6300 + i, "exploration-r6-heldout")


def diag(selected):
    b, rho = C.PRIMARY
    s = selected[C.setting_id(b, rho)]
    d = s["selection_delay_s"]
    arms = [C.sel_arm(key, d) for key in s["diag_sel_configs"]]
    arms += [C.always_arm(key, dd) for key, dd in s["diag_always_configs"].items()]
    arms.append(C.ctxonly_arm(d))
    arms.append(C.reference_arms()[0])
    write("diag", b, rho, C.DIAG_SEEDS, arms, 1.0, 6400, "exploration-r6-trace")


def xcheck():
    """R5's held-out manifests with only `source_revision` replaced by HEAD."""
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=C.ROOT, capture_output=True, text=True,
                          check=True).stdout.strip()
    C.MANIFESTS.mkdir(parents=True, exist_ok=True)
    for b, rho in C.SETTINGS:
        rid = f"r5-heldout-{C.setting_id(b, rho)}"
        m = json.load(open(R5_RUNS / rid / "manifest.json"))
        m["source_revision"] = head
        out = C.MANIFESTS / f"xcheck-{rid}.json"
        if out.exists():
            raise SystemExit(f"{out} exists")
        with open(out, "w") as fh:
            json.dump(m, fh, indent=2)
            fh.write("\n")
        print(out)


def main():
    stage = sys.argv[1]
    if stage == "pilot":
        pilot()
    elif stage == "tune1":
        tune1()
    elif stage == "tune2":
        tune2(C.load_json(sys.argv[2]))
    elif stage == "heldout":
        heldout(C.load_json(sys.argv[2]))
    elif stage == "diag":
        diag(C.load_json(sys.argv[2]))
    elif stage == "xcheck":
        xcheck()
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
