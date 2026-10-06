"""M2's byte-identity check: R6's held-out manifest at b = 5, rho = 0.7, replayed with this branch's
binary under its own run id (only `source_revision` changed), writes `results.csv` and
`incidents.csv` whose SHA-256 equals R6's recorded ones, for all 62 arms.

Usage:
  m2_gate.py manifest   write the manifest (R6's, `source_revision` replaced by HEAD, run id kept)
                        to artifacts/runs/_manifests/m2-xcheck-r6-heldout-b5-rho0.7.json
  m2_gate.py check      compare artifacts/runs/m2-xcheck-r6-heldout-b5-rho0.7 with
                        experiments/exploration/r6-results-sha256.csv; write
                        experiments/exploration/m2-regression.csv; exit 1 unless 62 of 62 match
"""

import hashlib
import json
import subprocess
import sys

import pandas as pd

import m2_common as C

RID = f"r6-heldout-{C.setting_id(*C.PRIMARY)}"
NAME = f"m2-xcheck-{RID}"


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def manifest():
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=C.ROOT, capture_output=True, text=True,
                          check=True).stdout.strip()
    m = json.load(open(C.R6_RUNS / "_manifests" / f"{RID}.json"))
    assert m["run_id"] == RID
    m["source_revision"] = head
    out = C.MANIFESTS / f"{NAME}.json"
    if out.exists():
        raise SystemExit(f"{out} exists")
    C.MANIFESTS.mkdir(parents=True, exist_ok=True)
    with open(out, "w") as fh:
        json.dump(m, fh, indent=2)
        fh.write("\n")
    print(out)


def check():
    d = C.RUNS / NAME
    want = pd.read_csv(C.OUT / "r6-results-sha256.csv")
    want = want[want.dir == RID].set_index("arm")
    arms = sorted(p.name for p in d.iterdir() if p.is_dir())
    rows = []
    for a in arms:
        rows.append({
            "run": RID, "arm": a,
            "results_identical": sha(d / a / "results.csv") == want.loc[a, "results_sha256"],
            "incidents_identical": sha(d / a / "incidents.csv") == want.loc[a, "incidents_sha256"],
        })
    reg = pd.DataFrame(rows)
    reg.to_csv(C.OUT / "m2-regression.csv", index=False)
    print(f"{RID}: {len(reg)} arms replayed; results identical {int(reg.results_identical.sum())}, "
          f"incidents identical {int(reg.incidents_identical.sum())}; arms in the record {len(want)}")
    ok = (set(arms) == set(want.index) and reg.results_identical.all()
          and reg.incidents_identical.all())
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    {"manifest": manifest, "check": check}.get(
        sys.argv[1] if len(sys.argv) > 1 else "", lambda: sys.exit(__doc__))()
