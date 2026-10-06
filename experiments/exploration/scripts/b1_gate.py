"""B1's byte-identity gate: R6's held-out manifest at b = 5, rho = 0.7, replayed with this branch's
binary under its own run id (only `source_revision` changed), writes `results.csv` and
`incidents.csv` whose SHA-256 equals the ones recorded in `r6-results-sha256.csv`, for every arm.

Usage: b1_gate.py [RUN_DIR]   (default artifacts/runs/xcheck-r6-heldout-b5-rho0.7; writes
                              experiments/exploration/b1-regression.csv; exit 1 if any arm differs
                              or the arms of the replay are not the arms of R6's record)
"""

import hashlib
import sys

import pandas as pd

import b1_common as C


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def main():
    rid = f"r6-heldout-{C.setting_id(*C.PRIMARY)}"
    d = C.RUNS / (sys.argv[1] if len(sys.argv) > 1 else f"xcheck-{rid}")
    want = pd.read_csv(C.OUT / "r6-results-sha256.csv")
    want = want[want.dir == rid].set_index("arm")
    arms = sorted(p.name for p in d.iterdir() if p.is_dir())
    rows = []
    for a in arms:
        rows.append({
            "run": rid, "arm": a,
            "results_identical": sha(d / a / "results.csv") == want.loc[a, "results_sha256"],
            "incidents_identical": sha(d / a / "incidents.csv") == want.loc[a, "incidents_sha256"],
        })
    reg = pd.DataFrame(rows)
    reg.to_csv(C.OUT / "b1-regression.csv", index=False)
    print(f"{len(reg)} arms replayed; results identical {int(reg.results_identical.sum())}, "
          f"incidents identical {int(reg.incidents_identical.sum())}; arms in R6's record {len(want)}")
    ok = (set(arms) == set(want.index) and reg.results_identical.all() and reg.incidents_identical.all())
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
