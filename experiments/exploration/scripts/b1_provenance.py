"""B1 provenance: an index of every run, the SHA-256 of every manifest and output file, the
regression against R6's recorded hashes, the cross-checks against R10's runs, and what the bill
refused.

Usage: b1_provenance.py   (writes b1-run-index.csv, b1-results-sha256.csv, b1-driver-log.csv,
                           b1-regression.csv, b1-vs-r10.csv and b1-refusals.csv; prints the checks)

Checks:
  1. regression: R6's held-out manifest at b = 5, rho = 0.7, replayed with this branch's binary under
     its own run id (only `source_revision` changed) writes `results.csv` and `incidents.csv` whose
     SHA-256 equals R6's record, for every arm (62 of 62);
  2. the tuning run plays seeds 10000-10099 and the held-out run 20000-20199: disjoint;
  3. the incidents (tier, family, criticality) are the same in every arm of a run;
  4. the held-out arm `sel_rung_z3_privileged` is R10's `sel_rung_privileged` at b = 5 and the arm
     `sel_rung_z2_privileged` is R10's z = 2 sweep arm: same `results.csv` and `incidents.csv` once
     the `run_id` column is dropped, so that quality rows are comparable with R10's;
  5. the control `sel_earliest_l0_privileged` (a lookback of zero) is `sel_rung_z3_privileged`;
  6. no run was step-capped, and what the bill refused (escalations, components, rule calls) is
     listed per arm, never dropped.
"""

import hashlib
import json
import re

import pandas as pd

import b1_common as C

FILES = ["results.csv", "incidents.csv", "notices.csv", "notice_incidents.csv", "notice_events.csv"]


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def body_sha(path):
    """SHA-256 of a CSV without its first column (`run_id`)."""
    lines = open(path).read().splitlines()
    return hashlib.sha256("\n".join(line.split(",", 1)[1] for line in lines).encode()).hexdigest()


def main():
    status, waits = [], {}
    log = C.RUNS / "_logs" / "b1-runs.log"
    for line in open(log):
        line = line.strip()
        m = re.match(r"(\S+) exit=(\d+) wall_s=(\d+)(?: waits=(\d+))?$", line)
        w = re.match(r"(\S+) waiting-for-other-process$", line)
        if m:
            status.append((m[1], int(m[2]), int(m[3]), int(m[4] or 0)))
        elif w:
            waits[w[1]] = waits.get(w[1], 0) + 1
    pd.DataFrame(status, columns=["run_id", "driver_exit", "wall_s", "waits"]).to_csv(C.OUT / "b1-driver-log.csv", index=False)

    index, shas, refusals = [], [], []
    dirs = sorted(p for p in C.RUNS.iterdir() if p.is_dir() and (p.name.startswith("b1-") or p.name.startswith("xcheck-r6-"))
                  and (p / "manifest.json").exists())
    for d in dirs:
        man = json.load(open(d / "manifest.json"))
        usage = json.load(open(d / "usage.json"))
        arms = sorted(a.name for a in d.iterdir() if a.is_dir())
        for a in arms:
            row = {"dir": d.name, "run_id": man["run_id"], "arm": a}
            for f in FILES:
                p = d / a / f
                row[f.replace(".csv", "") + "_sha256"] = sha(p) if p.exists() else ""
            shas.append(row)
            res = pd.read_csv(d / a / "results.csv")
            refusals.append({
                "dir": d.name, "arm": a, "streams": len(res),
                "step_capped": int((res.stop_reason != "horizon").sum()),
                "escalations_refused": int(res.escalations_refused.sum()),
                "probes_refused": int(res.probes_refused.sum()),
                "components_skipped": int(res.components_skipped.sum()),
                "rule_skipped": int(res.rule_skipped.sum()),
                "calls_unanswered": int(res.calls_unanswered.sum()),
                "bill_compute_max": int(res.bill_compute.max()),
            })
        index.append({
            "run_id": d.name, "manifest_run_id": man["run_id"], "experiment": man["experiment"],
            "source_revision": man["source_revision"], "arms": len(arms), "streams": len(man["seeds"]),
            "seed_first": man["seeds"][0], "seed_last": man["seeds"][-1],
            "b": man["stream_params"]["reasoner"]["b"], "rho": man["stream_params"]["reasoner"]["rho"],
            "notice_z": man["rung"]["notice_z"], "noticers": len(man.get("noticers", {})),
            "manifest_sha256": sha(d / "manifest.json"), "usage_exit_code": usage.get("exit_code"),
            "wall_s": round(usage["wall_ns"] / 1e9, 1), "cpu_s": round(usage["cpu_ns"] / 1e9, 1),
            "peak_memory_mb": round(usage["peak_memory_bytes"] / 2**20, 1), "oom_kills": usage.get("oom_kills"),
            "internal_external_ratio": usage.get("internal_external_ratio"),
            "toolchain": man["toolchain"], "cpu_model": man.get("cpu_model"),
        })
    ix = pd.DataFrame(index)
    ix.to_csv(C.OUT / "b1-run-index.csv", index=False)
    pd.DataFrame(shas).to_csv(C.OUT / "b1-results-sha256.csv", index=False)
    rf = pd.DataFrame(refusals)
    rf.to_csv(C.OUT / "b1-refusals.csv", index=False)

    # 1. regression against R6's recorded hashes
    rid = f"r6-heldout-{C.setting_id(*C.PRIMARY)}"
    r6 = pd.read_csv(C.OUT / "r6-results-sha256.csv")
    want = r6[r6.dir == rid].set_index("arm")
    d = C.RUNS / f"xcheck-{rid}"
    rows = []
    for a in sorted(x.name for x in d.iterdir() if x.is_dir()):
        rows.append({"run": rid, "arm": a,
                     "results_identical": sha(d / a / "results.csv") == want.loc[a, "results_sha256"],
                     "incidents_identical": sha(d / a / "incidents.csv") == want.loc[a, "incidents_sha256"]})
    reg = pd.DataFrame(rows)
    assert set(reg.arm) == set(want.index), "arms of the replay differ from R6's record"
    reg.to_csv(C.OUT / "b1-regression.csv", index=False)
    print(f"check 1: {len(reg)} arms replayed, results identical {int(reg.results_identical.sum())}, "
          f"incidents identical {int(reg.incidents_identical.sum())}, arms in R6's record {len(want)}")
    assert reg.results_identical.all() and reg.incidents_identical.all()

    # 2. seeds
    tune = ix[ix.run_id == C.run_id("tune")]
    held = ix[ix.run_id == C.run_id("heldout")]
    assert set(zip(tune.seed_first, tune.seed_last)) == {(10000, 10099)}, "tuning seeds"
    assert set(zip(held.seed_first, held.seed_last)) == {(20000, 20199)}, "held-out seeds"
    print("check 2: tuning 10000-10099, held-out 20000-20199: disjoint")

    # 3. incidents identical across arms of a run
    for name in (C.run_id("tune"), C.run_id("heldout")):
        base = None
        for a in sorted(x.name for x in (C.RUNS / name).iterdir() if x.is_dir()):
            inc = pd.read_csv(C.RUNS / name / a / "incidents.csv")[["seed", "incident", "tier", "family", "critical"]]
            if base is None:
                base = inc
            else:
                assert inc.equals(base), f"{name}/{a}: incidents differ"
    print("check 3: the incidents are the same in every arm of the tuning and the held-out run")

    # 4 and 5. the shared arms are R10's, the control is the rung
    mine = C.RUNS / C.run_id("heldout")
    pairs = [
        ("sel_rung_z3_privileged", C.R10_RUNS / "r10-heldout-b5-rho0.7" / "sel_rung_privileged", "R10 held-out sel_rung_privileged"),
        ("sel_rung_z2_privileged", C.R10_RUNS / "r10-sweep-z2-b5-rho0.7" / "sel_rung_privileged", "R10 sweep z = 2 sel_rung_privileged"),
        ("sel_earliest_l0_privileged", mine / "sel_rung_z3_privileged", "this run's sel_rung_z3_privileged (a lookback of 0 is the rung)"),
    ]
    rows = []
    for a, other, what in pairs:
        r_ok = body_sha(mine / a / "results.csv") == body_sha(other / "results.csv")
        i_ok = body_sha(mine / a / "incidents.csv") == body_sha(other / "incidents.csv")
        rows.append({"arm": a, "compared_with": what, "results_identical_modulo_run_id": r_ok,
                     "incidents_identical_modulo_run_id": i_ok})
    vs = pd.DataFrame(rows)
    vs.to_csv(C.OUT / "b1-vs-r10.csv", index=False)
    print(vs.to_string(index=False))
    assert vs.results_identical_modulo_run_id.all() and vs.incidents_identical_modulo_run_id.all()

    # 6. what the bill refused, and the step cap
    print("check 6: step-capped segments", int(rf.step_capped.sum()), "; refusals by arm in b1-refusals.csv")
    print(rf[(rf.escalations_refused + rf.components_skipped + rf.rule_skipped + rf.probes_refused) > 0].to_string(index=False))
    print("waits for another process:", waits or "none")


if __name__ == "__main__":
    main()
