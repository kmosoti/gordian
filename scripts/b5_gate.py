"""B5's byte-identity check: R6's held-out manifest at b = 5, rho = 0.7, replayed with this branch's
binary under its own run id (only `source_revision` changed), writes `results.csv` and
`incidents.csv` whose SHA-256 equals R6's recorded ones, for all 62 arms.

Usage (TAG empty for the first replay, "2" for the replay with the final tree's binary):
  b5_gate.py manifest [TAG]   write the manifest (R6's, `source_revision` replaced by HEAD, run id
                        kept) to artifacts/runs/_manifests/b5-xcheck-r6-heldout-b5-rho0.7.json
                        (b5-xcheck2-... for TAG 2)
  b5_gate.py check [TAG]      compare artifacts/runs/b5-xcheck-r6-heldout-b5-rho0.7 with
                        experiments/exploration/r6-results-sha256.csv; write
                        experiments/exploration/b5-regression.csv (b5-regression-final.csv for a
                        tag); exit 1 unless 62 of 62 match

The run goes in its own directory (`--out`), so the manifest's run id stays R6's.
"""

import hashlib
import json
import subprocess
import sys

import pandas as pd

import b5_common as C

RID = f"r6-heldout-{C.setting_id(*C.PRIMARY)}"
TAG = sys.argv[2] if len(sys.argv) > 2 else ""
NAME = f"b5-xcheck{TAG}-{RID}"


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def manifest():
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=C.ROOT, capture_output=True, text=True,
                          check=True).stdout.strip()
    m = json.load(open(C.M.R6_RUNS / "_manifests" / f"{RID}.json"))
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
    reg.to_csv(C.OUT / f"b5-regression{('-final' if TAG else '')}.csv", index=False)
    print(f"{RID}: {len(reg)} arms replayed; results identical {int(reg.results_identical.sum())}, "
          f"incidents identical {int(reg.incidents_identical.sum())}; arms in the record {len(want)}")
    ok = (set(arms) == set(want.index) and reg.results_identical.all()
          and reg.incidents_identical.all())
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    {"manifest": manifest, "check": check}.get(
        sys.argv[1] if len(sys.argv) > 1 else "", lambda: sys.exit(__doc__))()
