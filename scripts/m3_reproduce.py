"""M3's reproduction check: the rows rerun in M3's held-out run that earlier held-out runs on the
same 200 streams also played (the rung and the re-anchor from M2's run, M2's frozen media, and
B3's two composed rows) must write the same per-arm files, modulo the run id.

Usage: m3_reproduce.py   reads artifacts/runs/m3-heldout-b5-rho0.7 of this worktree and the M2 and
                         B3 held-out runs in the main checkout; writes
                         experiments/exploration/m3-reproduce.csv (one row per arm and file:
                         identical after replacing the run id, yes or no) and
                         m3-results-sha256.csv (the SHA-256 of every arm's files in the held-out
                         run, as written, for the chief's verification of the kept directory)
"""

import hashlib
import sys

import pandas as pd

import m3_common as C

MAIN_RUNS = C.ROOT.parents[2] / "artifacts" / "runs"  # .../gordian/artifacts/runs
M2_RUN = MAIN_RUNS / "m2" / "m2-heldout-b5-rho0.7"
B3_RUN = MAIN_RUNS / "b3" / "b3-heldout-b5-rho0.7"
FILES = ["results.csv", "incidents.csv", "notice_incidents.csv", "notice_events.csv", "notices.csv"]
# (this run's arm, the earlier run, its arm)
PAIRS = [
    ("rung_z3", M2_RUN, "rung_z3"),
    ("rung_z2", M2_RUN, "rung_z2"),
    ("reanchor", M2_RUN, "reanchor"),
    ("m2_t100", M2_RUN, "med_t100"),
    ("m2_t500", M2_RUN, "med_t500"),
    ("m2_t2000", M2_RUN, "med_t2000"),
    ("ramp_split_over_re2", B3_RUN, "ramp_split_over_re2"),
    ("ramp_over_re2", B3_RUN, "ramp_over_re2"),
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
    df.to_csv(C.OUT / "m3-reproduce.csv", index=False)
    sums = []
    for arm in sorted(p.name for p in run.iterdir() if p.is_dir()):
        row = {"dir": run.name, "arm": arm}
        for f in FILES:
            row[f.replace(".csv", "_sha256")] = hashlib.sha256(read(run / arm / f)).hexdigest()
        sums.append(row)
    pd.DataFrame(sums).to_csv(C.OUT / "m3-results-sha256.csv", index=False)
    print(df.to_string(index=False))
    sys.exit(0 if df.identical.dropna().all() else 1)


if __name__ == "__main__":
    main()
