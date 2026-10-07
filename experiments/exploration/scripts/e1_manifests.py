"""Write E1's manifests (A1a's `a1a_manifests.py`, with E1's arms): the tuning runs and the held-out
run of the record rung.

Usage: e1_manifests.py STAGE     (tune1 | tune2 | heldout)

  tune1    seeds 10000-10099 in stream order: stage 1 of the tuning rule (the module documentation of
           `noticer_record.rs`): for each form and each reset setting, the three levels under the
           policy `never`; and the memoryless re-anchor.
  tune2    the same seeds: stage 2: at the level stage 1 kept for each form and reset setting
           (`e1-selected.json`, written by `e1_select.py stage1`), `every` at k of 2, 4 and 8 and
           `on_contradiction`; and the memoryless re-anchor.
  heldout  seeds 40000-40199 in stream order: every form with and without the reset under each of
           the three policies at the tuned values (`e1-selected.json`, after `e1_select.py stage2`),
           beside the memoryless re-anchor, and the labelled sensitivity rows (the other levels of
           every `never` cell, and the site-keyed key with the free-form message ids).

A manifest records `git rev-parse HEAD` and the driver refuses a run whose manifest does not match a
clean tree, so run this after committing and immediately before the run.
"""

import json
import subprocess
import sys

import e1_common as C

TIMEOUT_S = 6 * 3600
NEVER = "never"
CONTRA = "on_contradiction"


def every(k):
    return {"every": {"k": k}}


def stage_arms(stage):
    """(arms, seeds, experiment) of a stage; arms are (name, noticer)."""
    arms = [C.control()]
    if stage == "tune1":
        for form in C.FORMS:
            for reset in C.RESETS:
                for level in C.LEVELS:
                    key = C.state_key(form, reset, NEVER, level, "tune1")
                    arms.append((C.name_of(form, level, NEVER, reset),
                                 C.record(form, level, NEVER, reset, key)))
        return arms, C.TUNING_SEEDS, "exploration-e1-tune1"
    sel = json.load(open(C.OUT / "e1-selected.json"))
    if stage == "tune2":
        for form in C.FORMS:
            for reset in C.RESETS:
                level = sel["stage1"][form][str(reset).lower()]["level"]
                for confirm in (every(2), every(4), every(8), CONTRA):
                    key = C.state_key(form, reset, confirm, level, "tune2")
                    arms.append((C.name_of(form, level, confirm, reset),
                                 C.record(form, level, confirm, reset, key)))
        return arms, C.TUNING_SEEDS, "exploration-e1-tune2"
    if stage == "heldout":
        cells = sel["heldout"]
        for form in C.FORMS:
            for reset in C.RESETS:
                for confirm_key in ("never", "every", "on_contradiction"):
                    cell = cells[form][str(reset).lower()][confirm_key]
                    confirm = NEVER if confirm_key == "never" else (
                        CONTRA if confirm_key == "on_contradiction" else every(cell["k"]))
                    level = cell["level"]
                    key = C.state_key(form, reset, confirm, level, "heldout")
                    arms.append((C.name_of(form, level, confirm, reset),
                                 C.record(form, level, confirm, reset, key)))
        # Labelled sensitivity rows: nothing is chosen from them.
        for form in C.FORMS:
            for reset in C.RESETS:
                kept = cells[form][str(reset).lower()]["never"]["level"]
                for level in C.LEVELS:
                    if level != kept:
                        key = C.state_key(form, reset, NEVER, level, "heldout") + 500
                        arms.append((C.name_of(form, level, NEVER, reset) + "_sens",
                                     C.record(form, level, NEVER, reset, key)))
        level = cells["site"]["true"]["never"]["level"]
        arms.append((C.name_of("site", level, NEVER, True) + "_msgids",
                     C.record("site", level, NEVER, True, C.state_key("site", True, NEVER, level, "heldout") + 700,
                              msg_ids=True)))
        return arms, C.HELDOUT_SEEDS, "exploration-e1-heldout"
    sys.exit(__doc__)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    stage = sys.argv[1]
    arms, seeds, experiment = stage_arms(stage)
    rid = C.run_id(stage)
    C.MANIFESTS.mkdir(parents=True, exist_ok=True)
    tmp = C.MANIFESTS / (rid + ".base")
    out = C.MANIFESTS / (rid + ".json")
    if out.exists():
        raise SystemExit(f"{out} exists; a manifest is never overwritten")
    tmp.unlink(missing_ok=True)
    subprocess.run(
        [str(C.BIN), "init-stream", "--run-id", rid, "--arms", "never_escalate=never_escalate",
         "--seed-start", str(seeds[0]), "--seed-count", str(seeds[1]),
         "--trace-sample-rate", "0", "--experiment", experiment, "--out", str(tmp)],
        check=True, cwd=C.ROOT,
    )
    m = json.load(open(tmp))
    tmp.unlink()
    policy = {"policy": C.SEL_POLICY, "delay_ns": C.sel_delay_s() * C.NS}
    names = [C.arm_name(n) for n, _ in arms]
    assert len(set(names)) == len(names), "duplicate arm names"
    keys = [n["state_key"] for _, n in arms if "state_key" in n]
    assert len(set(keys)) == len(keys), "every record arm carries under its own key"
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
