"""R10 provenance: an index of every run, the SHA-256 of every manifest and results file, the
byte-identity check against R6's recorded hashes, and the checks the design depends on.

Usage: r10_provenance.py   (writes r10-run-index.csv, r10-results-sha256.csv, r10-driver-log.csv,
                            r10-regression.csv and r10-vs-r6.csv; prints the checks)

Checks:
  1. regression: R6's held-out manifest at b = 5, rho = 0.7, replayed with this branch's binary
     under its own run id (only `source_revision` changed) into `xcheck-r6-heldout-b5-rho0.7`,
     writes `results.csv` and `incidents.csv` whose SHA-256 equals the ones recorded in
     `r6-results-sha256.csv`, for every arm;
  2. the held-out runs play seeds 20000-20199 and the diagnostic run 32000-32099, disjoint from
     the tuning seeds 10000-10099 and from every seed R4 to R6 used for diagnostics;
  3. for each seed, the incidents (tier, family, criticality) are the same in every setting;
  4. the arms R10 shares with R6 (`sel_rung`, `sel_win_w40_n256`, R4's oracle, `never_escalate`) write
     the same `results.csv` and `incidents.csv` as R6's own files for the same arm at the same
     setting, once the `run_id` column (which names the run) is dropped: the comparison arms of R10
     are R6's arms.
"""

import hashlib
import json
import re

import pandas as pd

import r10_common as C


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def body_sha(path):
    """SHA-256 of a CSV without its first column (`run_id`)."""
    lines = open(path).read().splitlines()
    return hashlib.sha256("\n".join(line.split(",", 1)[1] for line in lines).encode()).hexdigest()


def main():
    status, waits = [], {}
    log = C.RUNS / "_logs" / "r10-runs.log"
    for line in open(log):
        line = line.strip()
        m = re.match(r"(\S+) exit=(\d+) wall_s=(\d+)(?: waits=(\d+))?$", line)
        w = re.match(r"(\S+) waiting-for-build$", line)
        if m:
            status.append((m[1], int(m[2]), int(m[3]), int(m[4] or 0)))
        elif w:
            waits[w[1]] = waits.get(w[1], 0) + 1
        elif line:
            status.append((line, None, None, None))
    pd.DataFrame(status, columns=["run_id", "driver_exit", "wall_s", "waits"]).to_csv(C.OUT / "r10-driver-log.csv", index=False)

    index, shas = [], []
    dirs = sorted(p for p in C.RUNS.iterdir() if p.is_dir() and (p.name.startswith("r10-") or p.name.startswith("xcheck-r6-"))
                  and (p / "manifest.json").exists() and p.name != "r10-notices")
    for d in dirs:
        man = json.load(open(d / "manifest.json"))
        usage = json.load(open(d / "usage.json"))
        arms = sorted(a.name for a in d.iterdir() if a.is_dir())
        for a in arms:
            shas.append({"dir": d.name, "run_id": man["run_id"], "arm": a,
                         "results_sha256": sha(d / a / "results.csv"), "incidents_sha256": sha(d / a / "incidents.csv")})
        index.append({
            "run_id": d.name, "manifest_run_id": man["run_id"], "experiment": man["experiment"],
            "source_revision": man["source_revision"], "arms": len(arms), "streams": len(man["seeds"]),
            "seed_first": man["seeds"][0], "seed_last": man["seeds"][-1],
            "b": man["stream_params"]["reasoner"]["b"], "rho": man["stream_params"]["reasoner"]["rho"],
            "notice_z": man["rung"]["notice_z"], "trace_sample_rate": man["trace_sample_rate"],
            "manifest_sha256": sha(d / "manifest.json"), "usage_exit_code": usage.get("exit_code"),
            "wall_s": round(usage["wall_ns"] / 1e9, 1), "cpu_s": round(usage["cpu_ns"] / 1e9, 1),
            "peak_memory_mb": round(usage["peak_memory_bytes"] / 2**20, 1), "oom_kills": usage.get("oom_kills"),
            "internal_external_ratio": usage.get("internal_external_ratio"),
            "ratio_within_tolerance": usage.get("ratio_within_tolerance"),
            "toolchain": man["toolchain"], "cpu_model": man.get("cpu_model"),
        })
    ix = pd.DataFrame(index)
    ix.to_csv(C.OUT / "r10-run-index.csv", index=False)
    pd.DataFrame(shas).to_csv(C.OUT / "r10-results-sha256.csv", index=False)

    # 1. regression against R6's recorded hashes
    rid = f"r6-heldout-{C.setting_id(*C.PRIMARY)}"
    r6 = pd.read_csv(C.OUT / "r6-results-sha256.csv")
    want = r6[r6.dir == rid].set_index("arm")
    d = C.RUNS / f"xcheck-{rid}"
    rows = []
    for a in sorted(x.name for x in d.iterdir() if x.is_dir()):
        r_ok = sha(d / a / "results.csv") == want.loc[a, "results_sha256"]
        i_ok = sha(d / a / "incidents.csv") == want.loc[a, "incidents_sha256"]
        rows.append({"run": rid, "arm": a, "results_identical": r_ok, "incidents_identical": i_ok})
    reg = pd.DataFrame(rows)
    assert set(reg.arm) == set(want.index), "arms of the replay differ from R6's record"
    reg.to_csv(C.OUT / "r10-regression.csv", index=False)
    print(f"check 1: {len(reg)} arms replayed, results identical {int(reg.results_identical.sum())}, "
          f"incidents identical {int(reg.incidents_identical.sum())}, arms in R6's record {len(want)}")
    assert reg.results_identical.all() and reg.incidents_identical.all()

    # 2. seeds
    held = ix[ix.run_id.str.startswith("r10-heldout-") | ix.run_id.str.startswith("r10-sweep-")]
    diag = ix[ix.run_id.str.startswith("r10-diag-")]
    assert set(zip(held.seed_first, held.seed_last)) == {(20000, 20199)}, "held-out seeds"
    assert set(zip(diag.seed_first, diag.seed_last)) == {(32000, 32099)}, "diagnostic seeds"
    print("check 2: held-out and sweep 20000-20199, diagnostic 32000-32099: disjoint from tuning 10000-10099")

    # 3. incidents identical in every setting
    ref = None
    for b, rho in C.SETTINGS:
        inc = pd.read_csv(C.RUNS / C.run_id("heldout", b, rho) / C.NEVER / "incidents.csv")[["seed", "incident", "tier", "family", "critical"]]
        if ref is None:
            ref = inc
        else:
            assert inc.equals(ref), f"{C.setting_id(b, rho)}: incidents differ"
    print(f"check 3: the {len(ref)} held-out incidents are identical in all three settings")

    # 4. the shared arms are R6's arms
    rows = []
    for b, rho in C.SETTINGS:
        mine = C.RUNS / C.run_id("heldout", b, rho)
        theirs = C.R6_RUNS / f"r6-heldout-{C.setting_id(b, rho)}"
        for a in (C.SEL_RUNG, C.SEL_WIN, C.ORACLE, C.NEVER):
            if not (theirs / a / "results.csv").exists():
                # R6 carried other builder configurations at this setting; nothing to compare
                rows.append({"setting": C.setting_id(b, rho), "arm": a, "results_identical_modulo_run_id": None,
                             "incidents_identical_modulo_run_id": None})
                continue
            r_ok = body_sha(mine / a / "results.csv") == body_sha(theirs / a / "results.csv")
            i_ok = body_sha(mine / a / "incidents.csv") == body_sha(theirs / a / "incidents.csv")
            rows.append({"setting": C.setting_id(b, rho), "arm": a, "results_identical_modulo_run_id": r_ok,
                         "incidents_identical_modulo_run_id": i_ok})
    vs = pd.DataFrame(rows)
    vs.to_csv(C.OUT / "r10-vs-r6.csv", index=False)
    print(vs.to_string(index=False))
    cmp_ = vs.dropna(subset=["results_identical_modulo_run_id"])
    assert len(cmp_) >= 7 and cmp_.results_identical_modulo_run_id.all() and cmp_.incidents_identical_modulo_run_id.all()
    print("waits for another worker's build:", waits or "none")


if __name__ == "__main__":
    main()
