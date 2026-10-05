"""R7 provenance: an index of every run, the SHA-256 of every manifest and every results file, the
driver log, and the checks the design depends on.

Usage: r7_provenance.py   (writes r7-run-index.csv, r7-results-sha256.csv, r7-driver-log.csv and
                           r7-ceiling-vs-r6.csv; prints the checks)

Checks:
  1. the tuning runs play seeds 10000-10099 and the held-out runs 20000-20199, R6's seeds, and
     nothing else is run on either set;
  2. the incidents (tier, family, criticality) are the same in every tuning run and in every
     held-out run (the world does not depend on b, rho or delta), and the held-out incidents equal
     R6's (read from R6's own never_escalate arm);
  3. the context-only ceiling and R4's oracle hold only decisive evidence, so the penalty cannot touch
     them: at each held-out run their `results.csv` and `incidents.csv` equal R6's at the same
     setting (except the `run_id` column) when delta changes nothing about them. The result is
     recorded either way (r7-ceiling-vs-r6.csv), not assumed;
  4. every run's manifest records the penalty it was asked to run (`distractor_penalty`).
"""

import hashlib
import json
import os
import pathlib
import re

import pandas as pd

import r7_common as C

R6_RUNS = pathlib.Path(os.environ.get("R6_RUNS", "/home/user/gordian/artifacts/runs/r6"))


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def main():
    status = []
    for line in open(C.RUNS / "_logs" / "r7-runs.log"):
        line = line.strip()
        m = re.match(r"(\S+) exit=(\d+) wall_s=(\d+)$", line)
        if m:
            status.append((m[1], int(m[2]), int(m[3])))
        elif line:
            status.append((line, None, None))
    index, shas = [], []
    dirs = sorted(p for p in C.RUNS.iterdir() if p.is_dir() and p.name.startswith("r7-"))
    for d in dirs:
        man = json.load(open(d / "manifest.json"))
        usage = json.load(open(d / "usage.json"))
        arms = sorted(a.name for a in d.iterdir() if a.is_dir())
        for a in arms:
            shas.append({"dir": d.name, "run_id": man["run_id"], "arm": a,
                         "results_sha256": sha(d / a / "results.csv"), "incidents_sha256": sha(d / a / "incidents.csv")})
        r = man["stream_params"]["reasoner"]
        index.append({
            "run_id": d.name, "manifest_run_id": man["run_id"], "experiment": man["experiment"],
            "source_revision": man["source_revision"], "arms": len(arms), "streams": len(man["seeds"]),
            "seed_first": man["seeds"][0], "seed_last": man["seeds"][-1],
            "b": r["b"], "rho": r["rho"], "distractor_penalty": r.get("distractor_penalty", 0.0),
            "run_seed": man["run_seed"], "manifest_sha256": sha(d / "manifest.json"),
            "usage_exit_code": usage.get("exit_code"), "wall_s": round(usage["wall_ns"] / 1e9, 1),
            "cpu_s": round(usage["cpu_ns"] / 1e9, 1), "peak_memory_mb": round(usage["peak_memory_bytes"] / 2**20, 1),
            "oom_kills": usage.get("oom_kills"), "internal_external_ratio": usage.get("internal_external_ratio"),
            "toolchain": man["toolchain"], "cpu_model": man.get("cpu_model"),
        })
    ix = pd.DataFrame(index)
    ix.to_csv(C.OUT / "r7-run-index.csv", index=False)
    pd.DataFrame(shas).to_csv(C.OUT / "r7-results-sha256.csv", index=False)
    pd.DataFrame(status, columns=["run_id", "driver_exit", "wall_s"]).to_csv(C.OUT / "r7-driver-log.csv", index=False)

    grid = ix[ix.run_id.str.startswith(("r7-tune-", "r7-heldout-"))]
    tune = grid[grid.run_id.str.startswith("r7-tune-")]
    held = grid[grid.run_id.str.startswith("r7-heldout-")]
    assert set(zip(tune.seed_first, tune.seed_last)) == {(10000, 10099)}, "tuning seeds"
    assert set(zip(held.seed_first, held.seed_last)) == {(20000, 20199)}, "held-out seeds"
    print("check 1: tuning 10000-10099, held-out 20000-20199: R6's seeds, as specified")

    expect = {C.run_id(stage, b, rho, delta): delta for stage in ("tune", "heldout")
              for (b, rho, delta) in C.RUNS_SPEC}
    for _, r in grid.iterrows():
        assert r.distractor_penalty == expect[r.run_id], (r.run_id, r.distractor_penalty)
    assert set(grid.run_id) == set(expect), set(expect) ^ set(grid.run_id)
    print(f"check 4: {len(grid)} runs, each manifest records the penalty it was asked to run")

    ref_inc = None
    for stage, ids in (("tuning", tune.run_id), ("held-out", held.run_id)):
        ref = None
        for rid in ids:
            arm = next(a for a in sorted((C.RUNS / rid).iterdir()) if a.is_dir() and a.name.startswith("sel_"))
            inc = pd.read_csv(arm / "incidents.csv")[["seed", "incident", "tier", "family", "critical"]]
            ref = inc if ref is None else ref
            assert inc.equals(ref), f"{stage} {rid}: incidents differ"
        print(f"check 2: {stage}: the {len(ref)} incidents are identical in all {len(list(ids))} runs")
        if stage == "held-out":
            ref_inc = ref
    r6 = pd.read_csv(R6_RUNS / "r6-heldout-b5-rho0.7" / C.C6.NEVER / "incidents.csv")[
        ["seed", "incident", "tier", "family", "critical"]]
    assert r6.equals(ref_inc), "held-out incidents differ from R6's"
    print("check 2: the held-out incidents equal R6's")

    rows = []
    for (b, rho, delta) in C.RUNS_SPEC:
        sid = C.setting_id(b, rho)
        d = C.sel_delay_s(b, rho)
        hid = C.run_id("heldout", b, rho, delta)
        for name in (C.ctxonly_name(d), C.ORACLE):
            row = {"run_id": hid, "setting": sid, "delta": delta, "arm": name}
            for f in ("results", "incidents"):
                mine = pd.read_csv(C.RUNS / hid / name / f"{f}.csv").drop(columns=["run_id"])
                theirs = pd.read_csv(R6_RUNS / f"r6-heldout-{sid}" / name / f"{f}.csv").drop(columns=["run_id"])
                row[f"{f}_equal_to_r6_but_run_id"] = bool(mine.equals(theirs))
            rows.append(row)
    x = pd.DataFrame(rows)
    x.to_csv(C.OUT / "r7-ceiling-vs-r6.csv", index=False)
    print("check 3:")
    print(x.to_string(index=False))


if __name__ == "__main__":
    main()
