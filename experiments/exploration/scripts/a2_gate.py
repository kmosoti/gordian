"""A2's identity checks, from the run files only.

Usage:
  a2_gate.py r6      R6's held-out manifest replayed with this branch's binary
                     (artifacts/runs/a2/a2-xcheck-r6-heldout-b5-rho0.7): `results.csv` and
                     `incidents.csv` of all 62 arms against the SHA-256 recorded in
                     experiments/exploration/r6-results-sha256.csv; writes
                     experiments/exploration/a2-regression.csv; exit 1 unless 62 of 62 match
  a2_gate.py a1c     A1c's smoke manifest replayed (artifacts/runs/a2/a2-a1creplay-b5-rho0.7): each of
                     its 15 arms against A1c's kept run, and A1a's eight against A1a's kept run,
                     `incidents.csv` and `results.csv` identical row for row once the `run_id`
                     column is removed
  a2_gate.py a1d     A1d's smoke manifest replayed (artifacts/runs/a2/a2-a1dreplay-b5-rho0.7): each of
                     its 15 arms against A1d's kept run, the same two files, and its 12 engram trace
                     files byte for byte
  a2_gate.py all     the three; writes experiments/exploration/a2-reproduction.csv (a1c and a1d)
                     and a2-regression.csv (r6); exit 1 unless every one matches
"""

import csv
import hashlib
import sys

import a2_common as C

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


def arms_of(d):
    return sorted(p.name for p in d.iterdir() if p.is_dir() and not p.name.startswith("_"))


def r6():
    d = C.RUN_DIR / f"a2-xcheck-{R6}"
    with open(C.OUT / "r6-results-sha256.csv") as fh:
        want = {r["arm"]: r for r in csv.DictReader(fh) if r["dir"] == R6}
    rows = []
    for a in arms_of(d):
        w = want.get(a, {})
        rows.append({
            "run": R6, "arm": a,
            "results_identical": sha(d / a / "results.csv") == w.get("results_sha256"),
            "incidents_identical": sha(d / a / "incidents.csv") == w.get("incidents_sha256"),
        })
    with open(C.OUT / "a2-regression.csv", "w", newline="") as fh:
        out = csv.DictWriter(fh, ["run", "arm", "results_identical", "incidents_identical"])
        out.writeheader()
        out.writerows(rows)
    res = sum(r["results_identical"] for r in rows)
    inc = sum(r["incidents_identical"] for r in rows)
    print(f"{R6}: {len(rows)} arms replayed; results identical {res}, incidents identical {inc}; "
          f"arms in the record {len(want)}")
    return {r["arm"] for r in rows} == set(want) and res == inc == len(rows)


def replayed(stage, kept_runs):
    """Rows comparing the replay `stage` with each kept run in `kept_runs` (label, dir)."""
    d = C.RUN_DIR / C.run_id(stage)
    rows = []
    for label, kept in kept_runs:
        for a in arms_of(kept):
            rows.append({
                "replay": stage, "kept_run": label, "item": a,
                "identical": stripped_sha(kept / a / "incidents.csv")
                == stripped_sha(d / a / "incidents.csv")
                and stripped_sha(kept / a / "results.csv") == stripped_sha(d / a / "results.csv"),
            })
        trace = kept / C.TRACE_DIR
        if trace.is_dir():
            for f in sorted(p.name for p in trace.iterdir()):
                mine = d / C.TRACE_DIR / f
                rows.append({
                    "replay": stage, "kept_run": label, "item": f"{C.TRACE_DIR}/{f}",
                    "identical": mine.exists() and sha(mine) == sha(trace / f),
                })
    return rows


def main():
    stage = sys.argv[1] if len(sys.argv) > 1 else ""
    if stage not in ("r6", "a1c", "a1d", "all"):
        sys.exit(__doc__)
    ok = True
    if stage in ("r6", "all"):
        ok &= r6()
    rows = []
    if stage in ("a1c", "all"):
        rows += replayed("a1creplay", [("a1c", C.A1C_SMOKE), ("a1a", C.A1A_SMOKE)])
    if stage in ("a1d", "all"):
        rows += replayed("a1dreplay", [("a1d", C.A1D_SMOKE)])
    if rows:
        if stage == "all":
            with open(C.OUT / "a2-reproduction.csv", "w", newline="") as fh:
                out = csv.DictWriter(fh, ["replay", "kept_run", "item", "identical"])
                out.writeheader()
                out.writerows(rows)
        for label in ("a1c", "a1a", "a1d"):
            sub = [r for r in rows if r["kept_run"] == label]
            if sub:
                n = sum(r["identical"] for r in sub)
                print(f"{label}: {n} of {len(sub)} items reproduced")
        ok &= all(r["identical"] for r in rows)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
