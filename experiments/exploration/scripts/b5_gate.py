"""B5's byte-identity gate: R6's held-out manifest at b = 5, rho = 0.7, replayed with this branch's
binary under its own run id (only `source_revision` changed), writes `incidents.csv` whose SHA-256
equals R6's recorded one, and `results.csv` whose existing columns equal R6's recorded bytes, for all
62 arms.

What is compared, exactly (E1's reading, `e1_gate.py`, which this follows; the stopped unit's gate
compared whole files, before E1): `results.csv` gained two columns at its end in work item E1
(`recall_declarations`, `noticer_ns`), so its SHA-256 over the whole file changes by construction.
The file with its last two columns removed (each line cut after `total_cost_ns`, one newline after
each line) must hash to R6's recorded `results_sha256`; and the two new columns must be zero in every
row of every arm (no R6 arm has a memory, and none charges a noticer). `incidents.csv` is compared
whole. Both new-column checks and the whole-file hashes are written to the regression file. The files
that did not exist in R6 (`recalls.csv`, `selection*.csv`, ...) have no record; `recalls.csv` is only
checked for being empty of recalls.

Usage:
  b5_gate.py manifest TAG   write the manifest (R6's, `source_revision` replaced by HEAD, run id kept)
                            to artifacts/runs/_manifests/b5-xcheck-TAG-r6-heldout-b5-rho0.7.json
  b5_gate.py check TAG      compare artifacts/runs/b5-xcheck-TAG-r6-heldout-b5-rho0.7 (the run played
                            with OUT_PREFIX=b5-xcheck-TAG-) with experiments/exploration/r6-results-sha256.csv;
                            write experiments/exploration/b5-regression-TAG.csv; exit 1 unless 62 of 62 match

The run goes in its own directory (`--out`), so the manifest's run id stays R6's. The stopped unit's
record (`b5-regression.csv`, the pre-E1 reading on its own tree) is kept unchanged.
"""

import pathlib
import sys

# B5's scripts live here (the unit's territory, E1's convention); the earlier units' modules they build
# on (B2 to B4, C1, M2) live in scripts/, and the analysis package is this checkout's, not an installed one.
_ROOT = pathlib.Path(__file__).resolve().parents[3]
sys.path.insert(0, str(_ROOT / "scripts"))
sys.path.insert(0, str(_ROOT / "analysis"))

import csv  # noqa: E402
import hashlib  # noqa: E402
import json  # noqa: E402
import subprocess  # noqa: E402

import b5_common as C  # noqa: E402

RID = f"r6-heldout-{C.setting_id(*C.PRIMARY)}"
TAG = sys.argv[2] if len(sys.argv) > 2 else ""
NAME = f"b5-xcheck-{TAG}-{RID}"
NEW_COLUMNS = ["recall_declarations", "noticer_ns"]


def sha_bytes(b):
    return hashlib.sha256(b).hexdigest()


def sha(path):
    return sha_bytes(open(path, "rb").read())


def stripped(path):
    """The file without its last two columns (one newline after each line), and the two columns."""
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
    with open(C.OUT / "r6-results-sha256.csv") as fh:
        want = {r["arm"]: r for r in csv.DictReader(fh) if r["dir"] == RID}
    arms = sorted(p.name for p in d.iterdir() if p.is_dir())
    rows = []
    for a in arms:
        w = want.get(a, {})
        body, appended = stripped(d / a / "results.csv")
        header, data = appended[0], appended[1:]
        new_zero = header == NEW_COLUMNS and all(r == ["0", "0"] for r in data)
        recalls = d / a / "recalls.csv"
        recalls_empty = (not recalls.exists()) or len(recalls.read_text().splitlines()) <= 1
        rows.append({
            "run": RID, "arm": a,
            "results_existing_columns_identical": sha_bytes(body) == w.get("results_sha256"),
            "results_whole_file_sha256": sha(d / a / "results.csv"),
            "new_columns_all_zero": new_zero,
            "incidents_identical": sha(d / a / "incidents.csv") == w.get("incidents_sha256"),
            "recalls_csv_has_no_rows": recalls_empty,
        })
    out = C.OUT / f"b5-regression-{TAG}.csv"
    with open(out, "w", newline="") as fh:
        wr = csv.DictWriter(fh, list(rows[0]))
        wr.writeheader()
        wr.writerows(rows)
    res = sum(r["results_existing_columns_identical"] for r in rows)
    inc = sum(r["incidents_identical"] for r in rows)
    zero = sum(r["new_columns_all_zero"] for r in rows)
    empty = sum(r["recalls_csv_has_no_rows"] for r in rows)
    print(f"{RID}: {len(rows)} arms replayed; results (existing columns) identical {res}, incidents "
          f"identical {inc}, new columns all zero {zero}, recalls.csv empty {empty}; arms in the record "
          f"{len(want)}")
    ok = (set(arms) == set(want) and res == len(rows) and inc == len(rows) and zero == len(rows)
          and empty == len(rows))
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    {"manifest": manifest, "check": check}.get(
        sys.argv[1] if len(sys.argv) > 1 else "", lambda: sys.exit(__doc__))()
