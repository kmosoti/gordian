"""M4's reproduction check: the rows of M4's held-out run that M3's held-out run on the same 200
streams also played (the re-anchor, ramp + split over the re-anchor, M3's frozen 500 ms medium and
M2's frozen 100 ms medium) must write the same per-arm files, modulo the run id; and the SHA-256 of
every arm's files in M4's held-out run, as written, for the chief's verification of the kept
directory.

Usage: m4_reproduce.py   reads artifacts/runs/m4-heldout-b5-rho0.7 of this worktree and M3's
                         held-out run in the main checkout; writes
                         experiments/exploration/m4-reproduce.csv and m4-results-sha256.csv
"""

import hashlib
import sys

import pandas as pd

import m4_common as C

MAIN_RUNS = C.ROOT.parents[2] / "artifacts" / "runs"  # .../gordian/artifacts/runs
M3_RUN = MAIN_RUNS / "m3" / "m3-heldout-b5-rho0.7"
FILES = ["results.csv", "incidents.csv", "notice_incidents.csv", "notice_events.csv", "notices.csv"]
# (this run's arm, the earlier run, its arm)
PAIRS = [
    ("reanchor", M3_RUN, "reanchor"),
    ("ramp_split_over_re2", M3_RUN, "ramp_split_over_re2"),
    ("m3_t500", M3_RUN, "med_t500"),
    ("m2_t100", M3_RUN, "m2_t100"),
]


def read(path):
    with open(path, "rb") as fh:
        return fh.read()


def digest(path, run_id, arm):
    text = read(path).replace(f"{run_id}.{arm}".encode(), b"RUN.ARM")
    return hashlib.sha256(text).hexdigest()


def main():
    run = C.RUNS / C.run_id("heldout")
    rows = []
    for mine, ref_run, theirs in PAIRS:
        a, b = C.arm_name(mine), C.arm_name(theirs)
        for f in FILES:
            pa, pb = run / a / f, ref_run / b / f
            if not pb.exists():
                rows.append({"arm": a, "reference": f"{ref_run.name}/{b}", "file": f,
                             "identical": None})
                continue
            rows.append({
                "arm": a, "reference": f"{ref_run.name}/{b}", "file": f,
                "identical": digest(pa, run.name, a) == digest(pb, ref_run.name, b),
            })
    df = pd.DataFrame(rows)
    df.to_csv(C.OUT / "m4-reproduce.csv", index=False)
    sums = []
    for arm in sorted(p.name for p in run.iterdir() if p.is_dir()):
        row = {"dir": run.name, "arm": arm}
        for f in FILES:
            row[f.replace(".csv", "_sha256")] = hashlib.sha256(read(run / arm / f)).hexdigest()
        sums.append(row)
    pd.DataFrame(sums).to_csv(C.OUT / "m4-results-sha256.csv", index=False)
    print(df.to_string(index=False))
    sys.exit(0 if df.identical.notna().all() and df.identical.all() else 1)


if __name__ == "__main__":
    main()
