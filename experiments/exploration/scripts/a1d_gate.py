"""A1d's identity checks, from the run files only.

Usage:
  a1d_gate.py r6      R6's held-out manifest replayed with this branch's binary
                      (artifacts/runs/a1d/a1d-xcheck-r6-heldout-b5-rho0.7): `results.csv` and
                      `incidents.csv` of all 62 arms against the SHA-256 recorded in
                      experiments/exploration/r6-results-sha256.csv; writes
                      experiments/exploration/a1d-regression.csv; exit 1 unless 62 of 62 match
  a1d_gate.py a1c     A1c's smoke manifest replayed (artifacts/runs/a1d/a1d-a1creplay-b5-rho0.7):
                      each of its 15 arms against A1c's kept run, and A1a's eight against A1a's
                      kept run, `incidents.csv` and `results.csv` identical row for row once the
                      `run_id` column is removed; writes experiments/exploration/a1d-reproduction.csv;
                      exit 1 unless every one matches
"""

import csv
import hashlib
import sys

import a1d_common as C

R6 = f"r6-heldout-{C.setting_id(*C.PRIMARY)}"


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def stripped_sha(path):
    """SHA-256 of a CSV with its first column (`run_id`) removed."""
    h = hashlib.sha256()
    with open(path, newline="") as fh:
        for r in csv.reader(fh):
            h.update(",".join(r[1:]).encode() + b"\n")
    return h.hexdigest()


def r6():
    d = C.RUN_DIR / f"a1d-xcheck-{R6}"
    with open(C.OUT / "r6-results-sha256.csv") as fh:
        want = {r["arm"]: r for r in csv.DictReader(fh) if r["dir"] == R6}
    arms = sorted(p.name for p in d.iterdir() if p.is_dir() and not p.name.startswith("_"))
    rows = []
    for a in arms:
        w = want.get(a, {})
        rows.append({
            "run": R6, "arm": a,
            "results_identical": sha(d / a / "results.csv") == w.get("results_sha256"),
            "incidents_identical": sha(d / a / "incidents.csv") == w.get("incidents_sha256"),
        })
    with open(C.OUT / "a1d-regression.csv", "w", newline="") as fh:
        out = csv.DictWriter(fh, ["run", "arm", "results_identical", "incidents_identical"])
        out.writeheader()
        out.writerows(rows)
    res = sum(r["results_identical"] for r in rows)
    inc = sum(r["incidents_identical"] for r in rows)
    print(f"{R6}: {len(rows)} arms replayed; results identical {res}, incidents identical {inc}; "
          f"arms in the record {len(want)}")
    sys.exit(0 if set(arms) == set(want) and res == inc == len(rows) else 1)


def a1c():
    d = C.RUN_DIR / C.run_id("a1creplay")
    rows = []
    for kept, label in ((C.A1C_SMOKE, "a1c"), (C.A1A_SMOKE, "a1a")):
        for a in sorted(p.name for p in kept.iterdir() if p.is_dir() and not p.name.startswith("_")):
            rows.append({
                "kept_run": label, "arm": a,
                "incidents_identical": stripped_sha(kept / a / "incidents.csv")
                == stripped_sha(d / a / "incidents.csv"),
                "results_identical": stripped_sha(kept / a / "results.csv")
                == stripped_sha(d / a / "results.csv"),
            })
    with open(C.OUT / "a1d-reproduction.csv", "w", newline="") as fh:
        out = csv.DictWriter(fh, ["kept_run", "arm", "incidents_identical", "results_identical"])
        out.writeheader()
        out.writerows(rows)
    for label in ("a1c", "a1a"):
        sub = [r for r in rows if r["kept_run"] == label]
        ok = sum(r["incidents_identical"] and r["results_identical"] for r in sub)
        print(f"{label}: {ok} of {len(sub)} arms reproduced")
    sys.exit(0 if all(r["incidents_identical"] and r["results_identical"] for r in rows) else 1)


if __name__ == "__main__":
    {"r6": r6, "a1c": a1c}.get(sys.argv[1] if len(sys.argv) > 1 else "",
                              lambda: sys.exit(__doc__))()
