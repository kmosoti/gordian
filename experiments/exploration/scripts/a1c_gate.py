"""A1c's byte-identity check (A1a's `a1a_gate.py` with A1c's directories): R6's held-out manifest at b = 5,
rho = 0.7, replayed with this branch's binary under its own run id (only `source_revision`
changed), writes `results.csv` and `incidents.csv` whose SHA-256 equals R6's recorded ones, for all
62 arms.

Usage (TAG empty for the first replay, e.g. "2" for a later one, which gets its own directory and
its own regression file a1c-regression-TAG.csv):
  a1c_gate.py manifest [TAG]  write the manifest (R6's, `source_revision` replaced by HEAD, run id kept)
                         to artifacts/runs/_manifests/a1c-xcheck-r6-heldout-b5-rho0.7.json
  a1c_gate.py check [TAG] compare artifacts/runs/a1c/a1c-xcheck-r6-heldout-b5-rho0.7 with
                         experiments/exploration/r6-results-sha256.csv; write
                         experiments/exploration/a1c-regression.csv; exit 1 unless 62 of 62 match
"""

import csv
import hashlib
import json
import subprocess
import sys

import a1c_common as C

RID = f"r6-heldout-{C.setting_id(*C.PRIMARY)}"
TAG = sys.argv[2] if len(sys.argv) > 2 else ""
NAME = f"a1c-xcheck{TAG}-{RID}"
DIR = C.SMOKE_DIR / NAME


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def manifest():
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=C.ROOT, capture_output=True, text=True,
                          check=True).stdout.strip()
    m = json.load(open(C.M3.R6_RUNS / "_manifests" / f"{RID}.json"))
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
    with open(C.OUT / "r6-results-sha256.csv") as fh:
        want = {r["arm"]: r for r in csv.DictReader(fh) if r["dir"] == RID}
    arms = sorted(p.name for p in DIR.iterdir() if p.is_dir())
    rows = []
    for a in arms:
        w = want.get(a, {})
        rows.append({
            "run": RID, "arm": a,
            "results_identical": sha(DIR / a / "results.csv") == w.get("results_sha256"),
            "incidents_identical": sha(DIR / a / "incidents.csv") == w.get("incidents_sha256"),
        })
    with open(C.OUT / f"a1c-regression{'-' + TAG if TAG else ''}.csv", "w", newline="") as fh:
        out = csv.DictWriter(fh, ["run", "arm", "results_identical", "incidents_identical"])
        out.writeheader()
        out.writerows(rows)
    res = sum(r["results_identical"] for r in rows)
    inc = sum(r["incidents_identical"] for r in rows)
    print(f"{RID}: {len(rows)} arms replayed; results identical {res}, incidents identical {inc}; "
          f"arms in the record {len(want)}")
    ok = set(arms) == set(want) and res == len(rows) and inc == len(rows)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    {"manifest": manifest, "check": check}.get(
        sys.argv[1] if len(sys.argv) > 1 else "", lambda: sys.exit(__doc__))()
