"""R6 provenance: an index of every run, the SHA-256 of every manifest and every results file, the
cross-check against R5's recorded hashes, and the checks the design depends on.

Usage: r6_provenance.py   (writes r6-run-index.csv, r6-results-sha256.csv, r6-driver-log.csv,
                           r6-xcheck-r5.csv and r6-stale-manifests.csv; prints the checks)

Checks:
  1. the tuning runs play seeds 10000-10099, the held-out runs 20000-20199 and the diagnostic run
     31000-31099: disjoint, as specified (the diagnostic seeds also differ from R5's 30000-30099);
  2. for each seed, the incidents (tier, family, criticality) are the same in every setting;
  3. cross-check: R5's six held-out manifests, replayed with this branch's binary under their own
     run ids into `xcheck-*` directories (only `source_revision` changed), write `results.csv` and
     `incidents.csv` files whose SHA-256 equals the ones recorded in `r5-results-sha256.csv`, for
     every arm.
"""

import hashlib
import json
import re

import pandas as pd

import r6_common as C


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def main():
    status = []
    for line in open(C.RUNS / "_logs" / "r6-runs.log"):
        line = line.strip()
        m = re.match(r"(\S+) exit=(\d+) wall_s=(\d+)$", line)
        if m:
            status.append((m[1], int(m[2]), int(m[3])))
        elif line:
            status.append((line, None, None))
    index, shas = [], []
    dirs = sorted(p for p in C.RUNS.iterdir() if p.is_dir() and (p.name.startswith("r6-") or p.name.startswith("xcheck-r5-")))
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
            "manifest_sha256": sha(d / "manifest.json"), "usage_exit_code": usage.get("exit_code"),
            "wall_s": round(usage["wall_ns"] / 1e9, 1), "cpu_s": round(usage["cpu_ns"] / 1e9, 1),
            "peak_memory_mb": round(usage["peak_memory_bytes"] / 2**20, 1), "oom_kills": usage.get("oom_kills"),
            "internal_external_ratio": usage.get("internal_external_ratio"),
            "toolchain": man["toolchain"], "cpu_model": man.get("cpu_model"),
        })
    ix = pd.DataFrame(index)
    ix.to_csv(C.OUT / "r6-run-index.csv", index=False)
    pd.DataFrame(shas).to_csv(C.OUT / "r6-results-sha256.csv", index=False)
    pd.DataFrame(status, columns=["run_id", "driver_exit", "wall_s"]).to_csv(C.OUT / "r6-driver-log.csv", index=False)
    stale = sorted(p.name for p in C.MANIFESTS.iterdir() if p.name.endswith(".unrun-stale-revision"))
    pd.DataFrame({"manifest_set_aside_unrun": stale}).to_csv(C.OUT / "r6-stale-manifests.csv", index=False)

    own = ix[ix.run_id.str.startswith("r6-")]
    tune = own[own.run_id.str.startswith("r6-tune")]
    held = own[own.run_id.str.startswith("r6-heldout-")]
    diag = own[own.run_id.str.startswith("r6-diag-")]
    assert set(zip(tune.seed_first, tune.seed_last)) == {(10000, 10099)}, "tuning seeds"
    assert set(zip(held.seed_first, held.seed_last)) == {(20000, 20199)}, "held-out seeds"
    assert set(zip(diag.seed_first, diag.seed_last)) == {(31000, 31099)}, "diagnostic seeds"
    print("check 1: tuning 10000-10099, held-out 20000-20199, diagnostic 31000-31099: disjoint, as specified")

    for stage, arm, label in (("tune1", C.NEVER, "tuning"), ("heldout", C.NEVER, "held-out")):
        ref = None
        for b, rho in C.SETTINGS:
            d = C.RUNS / C.run_id(stage, b, rho) / arm
            inc = pd.read_csv(d / "incidents.csv")[["seed", "incident", "tier", "family", "critical"]]
            if ref is None:
                ref = inc
            else:
                assert inc.equals(ref), f"{label} {C.setting_id(b, rho)}: incidents differ"
        print(f"check 2: {label}: the {len(ref)} incidents are identical in all six settings")

    r5 = pd.read_csv(C.OUT / "r5-results-sha256.csv")
    rows = []
    for b, rho in C.SETTINGS:
        rid = f"r5-heldout-{C.setting_id(b, rho)}"
        d = C.RUNS / f"xcheck-{rid}"
        want = r5[r5.dir == rid].set_index("arm")
        arms = sorted(a.name for a in d.iterdir() if a.is_dir())
        ok_r = ok_i = 0
        bad = []
        for a in arms:
            r_ok = sha(d / a / "results.csv") == want.loc[a, "results_sha256"]
            i_ok = sha(d / a / "incidents.csv") == want.loc[a, "incidents_sha256"]
            ok_r += r_ok
            ok_i += i_ok
            if not (r_ok and i_ok):
                bad.append(a)
        rows.append({"run": rid, "arms": len(arms), "arms_in_r5_record": len(want), "results_identical": ok_r,
                     "incidents_identical": ok_i, "differing": ";".join(bad) if bad else "none"})
    x = pd.DataFrame(rows)
    x.to_csv(C.OUT / "r6-xcheck-r5.csv", index=False)
    print(x.to_string(index=False))


if __name__ == "__main__":
    main()
