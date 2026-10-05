"""R4 provenance: an index of every run, the SHA-256 of every manifest and every results file, and
two checks that the design depends on.

Usage: r4_provenance.py        (writes r4-run-index.csv and r4-results-sha256.csv, prints the checks)

Checks:
  1. the tuning seeds and the held-out seeds are disjoint, and every held-out run played exactly
     seeds 20000-20199 and every tuning run seeds 10000-10099;
  2. for each seed, the incidents (tier, family, criticality) are the same in every setting, so the
     settings differ in the reasoner and not in the world (the reasoner's (b, rho) enter after the
     stream is generated).
"""

import csv
import hashlib
import json
import re

import pandas as pd

import r4_common as C


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def main():
    wall = {}
    status = []
    for line in open(C.RUNS / "_logs" / "r4-runs.log"):
        m = re.match(r"(\S+) exit=(\d+) wall_s=(\d+)", line)
        status.append((m[1], int(m[2]), int(m[3])))
        if m[2] == "0":
            wall[m[1]] = int(m[3])
    index, shas = [], []
    for d in sorted(p for p in C.RUNS.iterdir() if p.is_dir() and p.name.startswith("r4-")):
        man = json.load(open(d / "manifest.json"))
        usage = json.load(open(d / "usage.json"))
        arms = sorted(a.name for a in d.iterdir() if a.is_dir())
        for a in arms:
            shas.append(
                {
                    "run_id": d.name, "arm": a,
                    "results_sha256": sha(d / a / "results.csv"),
                    "incidents_sha256": sha(d / a / "incidents.csv"),
                }
            )
        index.append(
            {
                "run_id": d.name, "experiment": man["experiment"],
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
    pd.DataFrame(index).to_csv(C.OUT / "r4-run-index.csv", index=False)
    pd.DataFrame(shas).to_csv(C.OUT / "r4-results-sha256.csv", index=False)
    pd.DataFrame(status, columns=["run_id", "driver_exit", "wall_s"]).to_csv(
        C.OUT / "r4-driver-log.csv", index=False
    )

    # check 1
    ix = pd.DataFrame(index)
    tune = ix[ix.run_id.str.startswith("r4-tune-")]
    held = ix[ix.run_id.str.startswith("r4-heldout-")]
    assert set(zip(tune.seed_first, tune.seed_last)) == {(10000, 10099)}, "tuning seeds"
    assert set(zip(held.seed_first, held.seed_last)) == {(20000, 20199)}, "held-out seeds"
    assert (tune.streams == 100).all() and (held.streams == 200).all()
    print("check 1: tuning runs play seeds 10000-10099 and held-out runs 20000-20199: disjoint, as specified")

    # check 2
    for stage, label in (("tune", "tuning"), ("heldout", "held-out")):
        ref = None
        names = []
        for b, rho in C.SETTINGS:
            d = C.RUNS / C.run_id(stage, b, rho) / C.ORACLE
            inc = pd.read_csv(d / "incidents.csv")[["seed", "incident", "tier", "family", "critical"]]
            names.append(C.setting_id(b, rho))
            if ref is None:
                ref = inc
            else:
                assert inc.equals(ref), f"{label} {C.setting_id(b, rho)}: incidents differ from the first setting"
        print(f"check 2: {label}: the {len(ref)} incidents (tier, family, criticality) are identical in all six settings")
    tiers = ref.groupby("tier").size().to_dict()
    print("held-out incidents by tier:", tiers)
    print(ref[ref.tier == "hard"].groupby("family").size().to_dict())


if __name__ == "__main__":
    main()
