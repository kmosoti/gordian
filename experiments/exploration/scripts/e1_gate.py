"""E1's byte-identity gate: R6's held-out manifest at b = 5, rho = 0.7, replayed with this branch's
binary under its own run id (only `source_revision` changed), writes `incidents.csv` whose SHA-256
equals R6's recorded one, and `results.csv` whose existing columns equal R6's recorded bytes, for
all 62 arms.

`results.csv` gained two columns at the end in work item E1 (`recall_declarations`, `noticer_ns`),
so its SHA-256 over the whole file changes by construction. What is compared is: the file with its
last two columns removed (each line cut after `total_cost_ns`) hashes to R6's recorded
`results_sha256`; and the two new columns are zero in every row of every arm (no R6 arm has a
memory, and none charges a noticer). Both are recorded in the regression file. `memory.csv`,
`memory_incidents.csv` and `recalls.csv` are new files with no R6 record; they are checked only for
being empty of recalls.

Usage (TAG empty for the first replay, e.g. "2" for a later one):
  e1_gate.py manifest [TAG]  write the manifest (R6's, `source_revision` replaced by HEAD, run id kept)
  e1_gate.py check [TAG]     compare artifacts/runs/e1/e1-xcheck{TAG}-r6-heldout-b5-rho0.7 with
                             experiments/exploration/r6-results-sha256.csv; write
                             experiments/exploration/e1-regression{-TAG}.csv; exit 1 unless 62 of 62 match
"""

import csv
import hashlib
import json
import subprocess
import sys

import e1_common as C

RID = f"r6-heldout-{C.setting_id(*C.PRIMARY)}"
TAG = sys.argv[2] if len(sys.argv) > 2 else ""
NAME = f"e1-xcheck{TAG}-{RID}"
DIR = C.E1_DIR / NAME


def sha_bytes(b):
    return hashlib.sha256(b).hexdigest()


def sha(path):
    return sha_bytes(open(path, "rb").read())


def stripped(path):
    """The file without its last two columns, one newline after each line, and the two columns."""
    lines = open(path, "rb").read().decode().split("\n")
    assert lines[-1] == "", "the file ends with a newline"
    kept, appended = [], []
    for line in lines[:-1]:
        cells = line.split(",")
        kept.append(",".join(cells[:-2]))
        appended.append(cells[-2:])
    return ("\n".join(kept) + "\n").encode(), appended


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
        body, appended = stripped(DIR / a / "results.csv")
        header, data = appended[0], appended[1:]
        new_zero = header == ["recall_declarations", "noticer_ns"] and all(r == ["0", "0"] for r in data)
        memory_empty = len(open(DIR / a / "recalls.csv").read().splitlines()) == 1
        rows.append({
            "run": RID, "arm": a,
            "results_existing_columns_identical": sha_bytes(body) == w.get("results_sha256"),
            "results_whole_file_sha256": sha(DIR / a / "results.csv"),
            "new_columns_all_zero": new_zero,
            "incidents_identical": sha(DIR / a / "incidents.csv") == w.get("incidents_sha256"),
            "recalls_csv_has_no_rows": memory_empty,
        })
    name = f"e1-regression{'-' + TAG if TAG else ''}.csv"
    with open(C.OUT / name, "w", newline="") as fh:
        out = csv.DictWriter(fh, list(rows[0]))
        out.writeheader()
        out.writerows(rows)
    res = sum(r["results_existing_columns_identical"] for r in rows)
    inc = sum(r["incidents_identical"] for r in rows)
    zero = sum(r["new_columns_all_zero"] for r in rows)
    empty = sum(r["recalls_csv_has_no_rows"] for r in rows)
    print(f"{RID}: {len(rows)} arms replayed; results (existing columns) identical {res}, incidents "
          f"identical {inc}, new columns all zero {zero}, recalls.csv empty {empty}; arms in the record {len(want)}")
    ok = set(arms) == set(want) and res == len(rows) and inc == len(rows) and zero == len(rows) \
        and empty == len(rows)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    {"manifest": manifest, "check": check}.get(
        sys.argv[1] if len(sys.argv) > 1 else "", lambda: sys.exit(__doc__))()
