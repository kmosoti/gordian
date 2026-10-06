"""Write the B2 manifests.

Usage:
  b2_manifests.py tune1      b = 5, rho = 0.7, the 100 tuning streams: the selection oracle (R5's 16 s,
                             the rung's retirement) under the rung's noticer at z = 3 and z = 2 (the
                             controls) and under `reanchor` at every (isolation, gap, burst) of stage 1
  b2_manifests.py tune1b     the same streams: `reanchor` at the gaps below the first grid's (amendment 1
                             of `b2_common.py`), at the rung's default threshold
  b2_manifests.py tune2      the same streams: `reanchor` at the stage 1 choice, at every threshold of
                             stage 2 (needs b2-selected.json with stage 1)
  b2_manifests.py tunedelay  the same streams: for each of the five table noticers, the selection oracle
                             with `hold_until_asked` at every delay of R5's grid (needs stages 1 and 2
                             in b2-selected.json)
  b2_manifests.py heldout    the 200 held-out streams (20000-20199): every B1 configuration (the
                             whole grid and the two extra thresholds), the `reanchor` grids (both) and
                             threshold ladder, all at R5's fixed delay with the rung's retirement; and
                             for the five table noticers, the selection oracle with `hold_until_asked`
                             at the delay stage 3 chose (needs b2-selected.json complete)
  b2_manifests.py xcheck-final  R6's held-out manifest again, `source_revision` only replaced, under
                             the prefix `xcheck2-` (the byte-identity gate with the final binary)

Exploration (nothing here tests a hypothesis). A manifest records `git rev-parse HEAD` and the driver
refuses a run whose manifest does not match a clean tree, so run this after committing and
immediately before the runs. It starts from `gordian-run init-stream` (environment capture), then
sets the arms, the reasoner's (b, rho), the per-arm noticers and the backstop.
"""

import json
import subprocess
import sys

import b2_common as C

TIMEOUT_S = 3 * 3600


def write(rid, seeds, arms, noticers, run_seed, experiment):
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
            "--trace-sample-rate", "0.0", "--experiment", experiment, "--out", str(tmp),
        ],
        check=True,
        cwd=C.ROOT,
    )
    with open(tmp) as fh:
        m = json.load(fh)
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


def fixed_arms(configs):
    """The selection oracle at R5's fixed delay under each (name, kind, parameters)."""
    arms, noticers = [], {}
    for name, kind, p in configs:
        arm = C.arm_name(name)
        arms.append((arm, C.fixed_policy()))
        noticers[arm] = C.spec(kind, **p)
    return arms, noticers


def controls():
    return [("rung_z3", "rung", {}), ("rung_z2", "rung", {"z": 2.0})]


def stage1_configs():
    return controls() + C.stage1_grid()


def chosen_reanchor(sel):
    c = sel["stage2"]["chosen"]
    return c["name"], "reanchor", {"isolation": c["isolation"], "gap_ms": c["gap_ms"],
                                   "burst": c["burst"], "z": c["z"]}


def stage2_configs(sel):
    s1 = sel["stage1"]["chosen"]
    return controls() + [
        (C.reanchor_name(s1["isolation"], s1["gap_ms"], s1["burst"], z), "reanchor",
         {"isolation": s1["isolation"], "gap_ms": s1["gap_ms"], "burst": s1["burst"], "z": z})
        for z in C.REANCHOR_Z
    ]


def table_configs(sel):
    """The five noticers of the table, as (name, kind, parameters)."""
    name, kind, p = chosen_reanchor(sel)
    return [
        ("rung_z3", "rung", {}),
        ("rung_z2", "rung", {"z": 2.0}),
        (C.C1.change_name(C.CHANGE_Q), "change_triggered", {"q": C.CHANGE_Q}),
        (C.C1.earliest_name(C.EARLIEST_L), "earliest_anchor", {"l": C.EARLIEST_L}),
        ("reanchor", kind, p),
    ]


def tunedelay(sel):
    arms, noticers = [], {}
    for name, kind, p in table_configs(sel):
        for d in C.DELAYS_S:
            arm = C.delay_arm_name(name, d)
            arms.append((arm, C.hold_policy(d)))
            noticers[arm] = C.spec(kind, **p)
    write(C.run_id("tunedelay"), C.TUNING_SEEDS, arms, noticers, 11_500, "exploration-b2-tunedelay")


def heldout(sel):
    configs = C.b1_grid() + C.stage1_grid() + C.stage1b_grid()
    s1 = sel["stage1"]["chosen"]
    seen = {n for n, _, _ in configs}
    for name, kind, p in stage2_configs(sel):
        if name not in seen:
            configs.append((name, kind, p))
            seen.add(name)
    arms, noticers = fixed_arms(configs)
    # The table's `reanchor` row is the stage 2 choice, under the name `reanchor` too, so that the
    # tables read it by one name wherever the grid also holds it under its parameters' name.
    name, kind, p = chosen_reanchor(sel)
    arm = C.arm_name("reanchor")
    arms.append((arm, C.fixed_policy()))
    noticers[arm] = C.spec(kind, **p)
    for name, kind, p in table_configs(sel):
        d = sel["stage3"][name]["delay_s"]
        arm = C.hold_arm_name(name)
        arms.append((arm, C.hold_policy(d)))
        noticers[arm] = C.spec(kind, **p)
    del s1
    write(C.run_id("heldout"), C.HELDOUT_SEEDS, arms, noticers, 11_700, "exploration-b2-heldout")


def xcheck(tag):
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=C.ROOT, capture_output=True, text=True,
                          check=True).stdout.strip()
    b, rho = C.PRIMARY
    rid = f"r6-heldout-{C.setting_id(b, rho)}"
    with open(C.C1.R6_RUNS / "_manifests" / f"{rid}.json") as fh:
        m = json.load(fh)
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


def main():
    stage = sys.argv[1] if len(sys.argv) > 1 else ""
    if stage == "tune1":
        arms, noticers = fixed_arms(stage1_configs())
        write(C.run_id("tune1"), C.TUNING_SEEDS, arms, noticers, 11_300, "exploration-b2-tune1")
    elif stage == "tune1b":
        arms, noticers = fixed_arms(controls() + C.stage1b_grid())
        write(C.run_id("tune1b"), C.TUNING_SEEDS, arms, noticers, 11_350, "exploration-b2-tune1b")
    elif stage == "tune2":
        sel = C.load_selected()
        arms, noticers = fixed_arms(stage2_configs(sel))
        write(C.run_id("tune2"), C.TUNING_SEEDS, arms, noticers, 11_400, "exploration-b2-tune2")
    elif stage == "tunedelay":
        tunedelay(C.load_selected())
    elif stage == "heldout":
        heldout(C.load_selected())
    elif stage == "xcheck-final":
        xcheck("2")
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
