"""B4 provenance: an index of every manifest and of the run directory that exists, the SHA-256 of every output
file, the regression against R6's recorded hashes, the cross-checks against M2's run and against B3's table,
and what the bill refused.

Usage: b4_provenance.py   (writes b4-run-index.csv, b4-results-sha256.csv, b4-driver-log.csv, b4-vs-earlier.csv
                           and b4-refusals.csv; prints the checks; exit 1 if one fails)

The run directories of the tuning stages and of the byte-identity replay were deleted after their tables were
recorded (the brief allows it); their manifests stay in artifacts/runs/_manifests/ and are indexed, with the
driver's exit status and wall time from the log, so that what ran is listed even where its output is gone. Only
the held-out run's outputs exist to hash.

Checks:
  1. regression: R6's held-out manifest at b = 5, rho = 0.7, replayed with this branch's binary under its own
     run id writes `results.csv` and `incidents.csv` whose SHA-256 equals R6's record for every arm (62 of 62)
     by `b4_gate.py` (`b4-regression.csv`, and `b4-regression-final.csv` for the rerun made at the end);
  2. the held-out run plays seeds 20000-20199, every tuning manifest 10000-10099;
  3. the incidents (tier, family, criticality) are the same in every arm of the held-out run;
  4. this run's selection-oracle arms are earlier runs' arms, byte for byte once the `run_id` column is dropped
     where an earlier run holds them: M2's `sel_rung_z3_privileged`, `sel_rung_z2_privileged`,
     `sel_reanchor_privileged` and `sel_med_t100_privileged` (read from the main checkout's
     `artifacts/runs/m2`, read only); and, for B3's other rows, whose outputs were deleted, the table's
     numbers of `b3-noticers-table.csv` reproduce to the digit (`b4_vs_b3` below);
  5. every non-privileged arm's calls are all about notices (no unattributed call), no segment was step-capped,
     and what the bill refused is listed per arm, never dropped.
"""

import hashlib
import json
import re

import pandas as pd

import b4_common as C
import b4_stats as S
from gordian_analysis.load import load_stream_run

FILES = ["results.csv", "incidents.csv", "notices.csv", "notice_incidents.csv", "notice_events.csv",
         "selection.csv", "selection_notices.csv"]
M2_HELDOUT = C.C1.R10_RUNS.parent / "m2" / "m2-heldout-b5-rho0.7"


def sha(path):
    with open(path, "rb") as fh:
        return hashlib.sha256(fh.read()).hexdigest()


def body_sha(path):
    """SHA-256 of a CSV without its first column (`run_id`)."""
    with open(path) as fh:
        lines = fh.read().splitlines()
    return hashlib.sha256("\n".join(line.split(",", 1)[1] for line in lines).encode()).hexdigest()


def main():
    ok = True
    status, waits = [], {}
    with open(C.RUNS / "_logs" / "b4-runs.log") as fh:
        for line in fh:
            line = line.strip()
            m = re.match(r"(\S+) exit=(\d+) wall_s=(\d+)(?: waits=(\d+))?$", line)
            w = re.match(r"(?:\S+ )?(\S+) waiting-for-other-process$", line)
            if m:
                status.append((m[1], int(m[2]), int(m[3]), int(m[4] or 0)))
            elif w:
                waits[w[1]] = waits.get(w[1], 0) + 1
    pd.DataFrame(status, columns=["run_id", "driver_exit", "wall_s", "waits"]).to_csv(
        C.OUT / "b4-driver-log.csv", index=False)

    index = []
    for path in sorted(C.MANIFESTS.glob("*.json")):
        with open(path) as fh:
            man = json.load(fh)
        d = C.RUNS / man["run_id"]
        usage = None
        if (d / "usage.json").exists():
            with open(d / "usage.json") as fh:
                usage = json.load(fh)
        index.append({
            "manifest": path.name, "run_id": man["run_id"], "experiment": man["experiment"],
            "source_revision": man["source_revision"], "arms": len(man["arms"]), "streams": len(man["seeds"]),
            "seed_first": man["seeds"][0], "seed_last": man["seeds"][-1],
            "b": man["stream_params"]["reasoner"]["b"], "rho": man["stream_params"]["reasoner"]["rho"],
            "noticers": len(man.get("noticers", {})), "manifest_sha256": sha(path),
            "output_dir_present": d.exists(),
            "usage_exit_code": usage.get("exit_code") if usage else None,
            "wall_s": round(usage["wall_ns"] / 1e9, 1) if usage else None,
            "cpu_s": round(usage["cpu_ns"] / 1e9, 1) if usage else None,
            "peak_memory_mb": round(usage["peak_memory_bytes"] / 2**20, 1) if usage else None,
            "oom_kills": usage.get("oom_kills") if usage else None,
            "internal_external_ratio": usage.get("internal_external_ratio") if usage else None,
            "toolchain": man["toolchain"], "cpu_model": man.get("cpu_model"),
        })
    ix = pd.DataFrame(index)
    ix.to_csv(C.OUT / "b4-run-index.csv", index=False)

    held = C.RUNS / C.run_id("heldout")
    shas, refusals = [], []
    for a in sorted(x.name for x in held.iterdir() if x.is_dir()):
        row = {"run_id": held.name, "arm": a}
        for f in FILES:
            p = held / a / f
            row[f.replace(".csv", "") + "_sha256"] = sha(p) if p.exists() else ""
        shas.append(row)
        res = pd.read_csv(held / a / "results.csv")
        refusals.append({
            "arm": a, "streams": len(res), "step_capped": int((res.stop_reason != "horizon").sum()),
            "escalations_refused": int(res.escalations_refused.sum()), "probes_refused": int(res.probes_refused.sum()),
            "components_skipped": int(res.components_skipped.sum()), "rule_skipped": int(res.rule_skipped.sum()),
            "calls_unanswered": int(res.calls_unanswered.sum()), "bill_compute_max": int(res.bill_compute.max()),
        })
    pd.DataFrame(shas).to_csv(C.OUT / "b4-results-sha256.csv", index=False)
    rf = pd.DataFrame(refusals)
    rf.to_csv(C.OUT / "b4-refusals.csv", index=False)

    # 1. regression against R6's recorded hashes
    for name in ("b4-regression.csv", "b4-regression-final.csv"):
        path = C.OUT / name
        if not path.exists():
            print(f"check 1 ({name}): not made")
            continue
        reg = pd.read_csv(path)
        print(f"check 1 ({name}): {len(reg)} arms, results identical {int(reg.results_identical.sum())}, "
              f"incidents identical {int(reg.incidents_identical.sum())}")
        ok &= len(reg) == 62 and bool(reg.results_identical.all()) and bool(reg.incidents_identical.all())

    # 2. seeds
    tune = ix[ix.run_id.str.startswith("b4-tune")]
    held_row = ix[ix.run_id == held.name]
    c2 = set(zip(tune.seed_first, tune.seed_last)) == {(10000, 10099)} and \
        set(zip(held_row.seed_first, held_row.seed_last)) == {(20000, 20199)}
    print("check 2: tuning manifests 10000-10099, held-out 20000-20199:", "ok" if c2 else "FAILED",
          f"({len(tune)} tuning manifests)")
    ok &= c2

    # 3. incidents identical across arms
    base = None
    for a in sorted(x.name for x in held.iterdir() if x.is_dir()):
        inc = pd.read_csv(held / a / "incidents.csv")[["seed", "incident", "tier", "family", "critical"]]
        if base is None:
            base = inc
        elif not inc.equals(base):
            ok = False
            print(f"check 3 FAILED: {a}: incidents differ")
    print("check 3: the incidents are the same in every arm of the held-out run")

    # 4. the oracle rows are earlier runs' arms
    pairs = [
        (C.oracle_arm("rung_z3"), M2_HELDOUT / "sel_rung_z3_privileged", "M2 held-out sel_rung_z3_privileged"),
        (C.oracle_arm("rung_z2"), M2_HELDOUT / "sel_rung_z2_privileged", "M2 held-out sel_rung_z2_privileged"),
        (C.oracle_arm("reanchor"), M2_HELDOUT / "sel_reanchor_privileged", "M2 held-out sel_reanchor_privileged"),
        (C.oracle_arm(C.MEDIUM_STEM), M2_HELDOUT / "sel_med_t100_privileged", "M2 held-out sel_med_t100_privileged"),
    ]
    rows = []
    for a, other, what in pairs:
        r_ok = body_sha(held / a / "results.csv") == body_sha(other / "results.csv")
        i_ok = body_sha(held / a / "incidents.csv") == body_sha(other / "incidents.csv")
        n_ok = body_sha(held / a / "notice_events.csv") == body_sha(other / "notice_events.csv") \
            if (other / "notice_events.csv").exists() else None
        rows.append({"arm": a, "compared_with": what, "results_identical_modulo_run_id": r_ok,
                     "incidents_identical_modulo_run_id": i_ok, "notice_events_identical_modulo_run_id": n_ok})
        ok &= r_ok and i_ok and n_ok is not False
    vs = pd.DataFrame(rows)
    vs.to_csv(C.OUT / "b4-vs-earlier.csv", index=False)
    print(vs.to_string(index=False))

    # 4b. B3's rows: the table's numbers reproduce (the outputs of B3's run were deleted with its worktree).
    b3 = pd.read_csv(C.OUT / "b3-noticers-table.csv").set_index("config")
    run = load_stream_run(held)
    m = S.Measures(run, [C.oracle_arm(s) for s in b3.index])
    pts = m.points()
    cols = ["hard_noticed_share", "hard_anchor_correct_share", "leak_noticed_share", "leak_anchor_correct_share",
            "notices_on_background_per_stream", "notice_precision", "strict_precision", "notices_per_incident",
            "quality", "cost_s_per_stream"]
    diff = []
    for stem in b3.index:
        i = m.row[C.oracle_arm(stem)]
        for c in cols:
            diff.append({"config": stem, "measure": c, "b3": float(b3.loc[stem, c]),
                         "b4": float(pts[c][i]), "abs_diff": abs(float(b3.loc[stem, c]) - float(pts[c][i]))})
    d = pd.DataFrame(diff)
    d.to_csv(C.OUT / "b4-vs-b3.csv", index=False)
    worst = float(d["abs_diff"].max())
    print(f"check 4b: B3's table rows (oracle arms) reproduced; the largest absolute difference over "
          f"{len(d)} values is {worst:.2e}")
    ok &= worst < 1e-9

    # 5. what the bill refused, the step cap, and the calls about no notice
    print("check 5: step-capped segments", int(rf.step_capped.sum()), "; refusals by arm in b4-refusals.csv")
    ok &= int(rf.step_capped.sum()) == 0
    unattributed = {}
    for a in run.arms:
        if run.arms[a].role == "comparison":
            n = int(run.arms[a].selection["escalations_unattributed"].sum())
            if n:
                unattributed[a] = n
    print("calls about no notice in comparison arms:", unattributed or "none")
    ok &= not unattributed
    print(rf[(rf.escalations_refused + rf.components_skipped + rf.rule_skipped + rf.probes_refused) > 0]
          .to_string(index=False))
    print("waits for another process (from the log):", waits or "none")
    print("all required checks pass" if ok else "A CHECK FAILED")
    raise SystemExit(0 if ok else 1)


if __name__ == "__main__":
    main()
