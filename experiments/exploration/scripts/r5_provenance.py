"""R5 provenance: an index of every run, the SHA-256 of every manifest and every results file, the
regression check against R4's recorded hashes, and two checks that the design depends on.

Usage: r5_provenance.py        (writes r5-run-index.csv, r5-results-sha256.csv, r5-driver-log.csv and
                                r5-regression.csv; prints the checks)

Checks:
  1. the tuning runs play seeds 10000-10099, the held-out runs 20000-20199 and the diagnostic run
     30000-30099: disjoint, as specified;
  2. for each seed, the incidents (tier, family, criticality) are the same in every setting, so the
     settings differ in the reasoner and not in the world;
  3. regression: R4's six held-out manifests, replayed with the binary of this work under their own
     run ids into other directories (`r5-regress-*`), write `results.csv` and `incidents.csv` files
     whose SHA-256 equals the ones recorded in `r4-results-sha256.csv`, for every arm.
"""

import hashlib
import json
import re

import pandas as pd

import r5_common as C


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def main():
    # The driver log, in the order written. A line that is not a run's exit status is a note the
    # resuming worker added (the container restart); it is kept as a row, never dropped.
    status = []
    for line in open(C.RUNS / "_logs" / "r5-runs.log"):
        line = line.strip()
        m = re.match(r"(\S+) exit=(\d+) wall_s=(\d+)$", line)
        if m:
            status.append((m[1], m[2], int(m[3])))
        elif line:
            status.append((line.lstrip("# "), "note", None))
    # A run directory the restart interrupted holds no results and no usage.json; it is kept on
    # disk (never deleted) and listed here, not read.
    interrupted = sorted(
        p.name for p in C.RUNS.iterdir() if p.is_dir() and p.name.endswith(".interrupted-by-restart")
    )
    for name in interrupted:
        status.append((name, "interrupted", None))
    index, shas = [], []
    for d in sorted(
        p for p in C.RUNS.iterdir() if p.is_dir() and p.name.startswith("r5-") and p.name not in interrupted
    ):
        man = json.load(open(d / "manifest.json"))
        usage = json.load(open(d / "usage.json"))
        arms = sorted(a.name for a in d.iterdir() if a.is_dir())
        for a in arms:
            shas.append(
                {
                    "dir": d.name, "run_id": man["run_id"], "arm": a,
                    "results_sha256": sha(d / a / "results.csv"),
                    "incidents_sha256": sha(d / a / "incidents.csv"),
                }
            )
        index.append(
            {
                "run_id": d.name, "manifest_run_id": man["run_id"], "experiment": man["experiment"],
                "source_revision": man["source_revision"], "arms": len(arms),
                "streams": len(man["seeds"]), "seed_first": man["seeds"][0], "seed_last": man["seeds"][-1],
                "b": man["stream_params"]["reasoner"]["b"], "rho": man["stream_params"]["reasoner"]["rho"],
                "manifest_sha256": sha(d / "manifest.json"),
                "usage_exit_code": usage.get("exit_code"), "wall_s": round(usage["wall_ns"] / 1e9, 1),
                "cpu_s": round(usage["cpu_ns"] / 1e9, 1),
                "peak_memory_mb": round(usage["peak_memory_bytes"] / 2**20, 1),
                "oom_kills": usage.get("oom_kills"),
                "internal_external_ratio": usage.get("internal_external_ratio"),
                "toolchain": man["toolchain"], "cpu_model": man.get("cpu_model"),
            }
        )
    ix = pd.DataFrame(index)
    ix.to_csv(C.OUT / "r5-run-index.csv", index=False)
    pd.DataFrame(shas).to_csv(C.OUT / "r5-results-sha256.csv", index=False)
    pd.DataFrame(status, columns=["run_id", "driver_exit", "wall_s"]).to_csv(C.OUT / "r5-driver-log.csv", index=False)

    # check 1
    tune = ix[ix.run_id.str.startswith("r5-tune-")]
    held = ix[ix.run_id.str.startswith("r5-heldout-") | ix.run_id.str.startswith("r5-suppl-")]
    diag = ix[ix.run_id.str.startswith("r5-diag-")]
    assert set(zip(tune.seed_first, tune.seed_last)) == {(10000, 10099)}, "tuning seeds"
    assert set(zip(held.seed_first, held.seed_last)) == {(20000, 20199)}, "held-out seeds"
    assert set(zip(diag.seed_first, diag.seed_last)) == {(30000, 30099)}, "diagnostic seeds"
    print("check 1: tuning 10000-10099, held-out 20000-20199, diagnostic 30000-30099: disjoint, as specified")

    # check 2
    for stage, arm, label in (("tune", C.sel_name(0), "tuning"), ("heldout", C.NEVER, "held-out")):
        ref = None
        for b, rho in C.SETTINGS:
            d = C.RUNS / C.run_id(stage, b, rho) / arm
            inc = pd.read_csv(d / "incidents.csv")[["seed", "incident", "tier", "family", "critical"]]
            if ref is None:
                ref = inc
            else:
                assert inc.equals(ref), f"{label} {C.setting_id(b, rho)}: incidents differ from the first setting"
        print(f"check 2: {label}: the {len(ref)} incidents (tier, family, criticality) are identical in all six settings")
        if stage == "heldout":
            print("held-out incidents by tier:", ref.groupby("tier").size().to_dict())
            print(ref[ref.tier == "hard"].groupby("family").size().to_dict())

    # check 3: regression against R4's recorded hashes
    r4 = pd.read_csv(C.OUT / "r4-results-sha256.csv")
    rows = []
    for b, rho in C.SETTINGS:
        rid = f"r4-heldout-{C.setting_id(b, rho)}"
        d = C.RUNS / f"r5-regress-{rid}"
        if not d.exists():
            continue
        want = r4[r4.run_id == rid].set_index("arm")
        arms = sorted(a.name for a in d.iterdir() if a.is_dir())
        same_r = same_i = 0
        bad = []
        for a in arms:
            r_ok = sha(d / a / "results.csv") == want.loc[a, "results_sha256"]
            i_ok = sha(d / a / "incidents.csv") == want.loc[a, "incidents_sha256"]
            same_r += r_ok
            same_i += i_ok
            if not (r_ok and i_ok):
                bad.append(a)
        rows.append(
            {
                "R4 run replayed": rid, "arms": len(arms), "arms in R4's record": len(want),
                "results.csv identical": same_r, "incidents.csv identical": same_i,
                "differing arms": ";".join(bad) if bad else "none",
            }
        )
    if rows:
        reg = pd.DataFrame(rows)
        reg.to_csv(C.OUT / "r5-regression.csv", index=False)
        print(reg.to_string(index=False))


if __name__ == "__main__":
    main()
