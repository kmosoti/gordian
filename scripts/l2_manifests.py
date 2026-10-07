"""Write L2 manifests.

Usage: l2_manifests.py STAGE   (tune-bNN | fresh; see below)

Every manifest is the selection oracle at R5's delay (16 s) with the rung's own context, under the
noticer each arm names, at b = 5, rho = 0.7, as B1's, B2's, M2's and L1's tables. The seeds are in
stream order (ascending): the learning arms carry what they learn from one segment to the next in
that order. A manifest records `git rev-parse HEAD` and the driver refuses a run whose manifest
does not match a clean tree, so run this after committing and immediately before the run.

Stages:
  tune-bNN  batch NN (from 0) of the tuning grid, `BATCH` configurations per manifest, on seeds
            10000-10099
  fresh     the six arms of the main run on seeds 40000-40199, the ESN at the tuned configuration
            recorded in experiments/exploration/l2-selected.json
"""

import json
import subprocess
import sys

import l2_common as C

TIMEOUT_S = 4 * 3600
BATCH = 12


def stage(name):
    """(arms, seeds, run_seed, experiment) of a stage."""
    if name.startswith("tune-b"):
        k = int(name[len("tune-b"):])
        cfgs = C.grid()[k * BATCH:(k + 1) * BATCH]
        if not cfgs:
            raise SystemExit(f"no configurations in batch {k}")
        return [C.grid_arm(*c) for c in cfgs], C.TUNE_SEEDS, C.RUN_SEED, f"exploration-l2-{name}"
    if name == "fresh":
        with open(C.OUT / "l2-selected.json") as fh:
            sel = json.load(fh)
        return C.fresh_arms(sel["chosen"]["params"]), C.FRESH_SEEDS, C.RUN_SEED, "exploration-l2-fresh"
    raise SystemExit(f"unknown stage {name!r}")


def batches():
    return (len(C.grid()) + BATCH - 1) // BATCH


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
    name = sys.argv[1] if len(sys.argv) > 1 else ""
    arms, seeds, run_seed, experiment = stage(name)
    policy = {"policy": C.M.SEL_POLICY, "delay_ns": C.M.sel_delay_s() * C.NS}
    noticers = {}
    rows = []
    for arm_label, noticer in arms:
        arm = C.arm_name(arm_label)
        rows.append((arm, policy))
        noticers[arm] = noticer
    write(C.run_id(name), seeds, rows, noticers, run_seed, experiment)


if __name__ == "__main__":
    main()
