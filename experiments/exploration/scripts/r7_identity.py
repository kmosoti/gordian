"""R7 test 1: at delta = 0, R6's held-out manifest at b = 5, rho = 0.7 replays byte for byte.

Usage: r7_identity.py [RUN_DIR_NAME ...]

Compares, for every arm of the replayed run directory, the sha256 of `results.csv` and
`incidents.csv` with the rows of `r6-results-sha256.csv` whose `dir` is `r6-heldout-b5-rho0.7`,
and writes `r7-identity.csv` (one row per arm) next to this script's outputs. Exit status 0 only if
every arm of R6's record is present in the replay and both hashes agree for all of them, and the
replay holds no arm that R6's record lacks. Nothing is excluded or relabelled: an arm that is
missing or that differs is a row with `identical` false.

Exploration (nothing here tests a hypothesis).
"""

import csv
import hashlib
import pathlib
import sys

import r6_common as C

R6_DIR = "r6-heldout-b5-rho0.7"
REPLAY = "r7-gate-r6-heldout-b5-rho0.7"


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest() if path.exists() else ""


def main():
    replay = C.RUNS / (sys.argv[1] if len(sys.argv) > 1 else REPLAY)
    record = {}
    with open(C.OUT / "r6-results-sha256.csv") as fh:
        for row in csv.DictReader(fh):
            if row["dir"] == R6_DIR:
                record[row["arm"]] = row
    arms = sorted(p.name for p in replay.iterdir() if p.is_dir())
    rows = []
    for arm in sorted(set(record) | set(arms)):
        r6 = record.get(arm)
        res = sha(replay / arm / "results.csv")
        inc = sha(replay / arm / "incidents.csv")
        rows.append({
            "arm": arm,
            "in_r6_record": r6 is not None,
            "in_replay": arm in arms,
            "results_identical": r6 is not None and res == r6["results_sha256"],
            "incidents_identical": r6 is not None and inc == r6["incidents_sha256"],
            "results_sha256": res,
            "incidents_sha256": inc,
        })
        rows[-1]["identical"] = (rows[-1]["in_r6_record"] and rows[-1]["in_replay"]
                                 and rows[-1]["results_identical"] and rows[-1]["incidents_identical"])
    out = pathlib.Path(C.OUT / "r7-identity.csv")
    with open(out, "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(rows[0]))
        w.writeheader()
        w.writerows(rows)
    n = len(rows)
    ok = sum(r["identical"] for r in rows)
    print(f"{replay.name}: {ok} of {n} arms identical in results.csv and incidents.csv")
    for r in rows:
        if not r["identical"]:
            print("DIFFERS:", r["arm"], r)
    return 0 if ok == n else 1


if __name__ == "__main__":
    sys.exit(main())
