"""B1's byte-identity gate: a recorded run's manifest, replayed with this branch's binary under its
own run id (only `source_revision` changed), writes `results.csv` and `incidents.csv` whose SHA-256
equals the ones recorded for it, for every arm.

Usage: b1_gate.py [RUN_DIR [RUN_ID [HASH_CSV [OUT_CSV]]]]
  default: artifacts/runs/xcheck-r6-heldout-b5-rho0.7, run r6-heldout-b5-rho0.7, r6-results-sha256.csv,
  b1-regression.csv. The R10 replay is
  `b1_gate.py xcheck-r10-heldout-b5-rho0.7 r10-heldout-b5-rho0.7 r10-results-sha256.csv
  b1-regression-r10.csv`; the repeat of the R6 replay with the final tree's binary is
  `b1_gate.py xcheck2-r6-heldout-b5-rho0.7 r6-heldout-b5-rho0.7 r6-results-sha256.csv
  b1-regression-final.csv`. Exit 1 if any arm differs or the arms of the replay are not the arms
  of the record.
"""

import hashlib
import sys

import pandas as pd

import b1_common as C


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def main():
    default_rid = f"r6-heldout-{C.setting_id(*C.PRIMARY)}"
    args = sys.argv[1:]
    run_dir = args[0] if len(args) > 0 else f"xcheck-{default_rid}"
    rid = args[1] if len(args) > 1 else default_rid
    hashes = args[2] if len(args) > 2 else "r6-results-sha256.csv"
    out = args[3] if len(args) > 3 else "b1-regression.csv"
    d = C.RUNS / run_dir
    want = pd.read_csv(C.OUT / hashes)
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
    reg.to_csv(C.OUT / out, index=False)
    print(f"{rid}: {len(reg)} arms replayed; results identical {int(reg.results_identical.sum())}, "
          f"incidents identical {int(reg.incidents_identical.sum())}; arms in the record {len(want)}")
    ok = (set(arms) == set(want.index) and reg.results_identical.all() and reg.incidents_identical.all())
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
