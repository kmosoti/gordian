"""B3's supplementary comparison with M2's held-out run: the medium at a 100 ms tick (M2's frozen graph) beside
B3's public rows, on the same 200 held-out streams (seeds 20000-20199, b = 5, rho = 0.7), paired over the same
resamples of streams, with the identity checks that make the two runs one table.

Usage: b3_vs_m2.py [M2_RUN_DIR]   (default: the main checkout's artifacts/runs/m2/m2-heldout-b5-rho0.7, read only;
                                   writes experiments/exploration/b3-vs-m2.csv and b3-vs-m2-identity.csv)

Exploration (nothing here tests a hypothesis; M2's criterion and verdict are M2's and are not read again). The
comparison is the labelled supplementary one the queue names for B3 (docs/lab-queue.md, B3 acceptance), run
after M2's held-out runs, not a re-fixing of M2's comparator. It claims nothing about which noticer is better:
the rows are numbers, and the differences are paired.

Identity checks first: M2's run carries the rung at z = 3 and z = 2 and the re-anchor, rerun (M2 says they
reproduce B2). The same arms in B3's run must have byte-identical notice records and results columns that are
deterministic (everything but the run id and the wall-clock columns), or the two runs are not one table and the
script stops.

Cost is not like for like: the harness bills the components, the shared rule and the reasoner, and the medium's
adapter additionally bills the medium's own operations at the calibrated prices; the public noticers' own work
is not billed anywhere (HARNESS.md). The comparison is made on the measures and the cost is shown for what it is.
"""

import pathlib
import sys

import numpy as np
import pandas as pd

import b2_stats as B
import b3_common as C
from gordian_analysis.load import load_stream_run

M2_RUN = pathlib.Path("/home/user/gordian/artifacts/runs/m2/m2-heldout-b5-rho0.7")
MEDIUM = "sel_med_t100_privileged"
MEASURES = [
    "hard_noticed_share", "hard_anchor_correct_share", "leak_noticed_share", "leak_anchor_correct_share",
    "notices_on_background_per_stream", "notice_precision", "strict_precision", "notices_per_incident",
    "quality", "leak_quality", "cost_s_per_stream", "calls_per_stream", "notices_per_stream",
]
ROWS = ["rung_z3", "rung_z2", "reanchor", "ramp_over_re2", "split_over_re2", "ramp_split_over_re2"]
SAME = {"rung_z3": "sel_rung_z3_privileged", "rung_z2": "sel_rung_z2_privileged", "reanchor": "sel_reanchor_privileged"}
VOLATILE = {"run_id", "wall_ns", "cpu_ns", "measured_ns", "ns_measured", "substrate_ns"}


class Merged:
    """The two runs' arms under one name each, so that `B.Measures` pairs them over the same streams."""

    def __init__(self, mine, theirs):
        self.arms = {"medium_t100": theirs.arms[MEDIUM]}
        for stem in ROWS:
            self.arms[stem] = mine.arms[C.arm_name(stem)]


def frames_equal(a, b):
    cols = [c for c in a.columns if c not in VOLATILE and not c.endswith("_ns_wall")]
    if list(a.columns) != list(b.columns):
        return False
    return bool(a[cols].reset_index(drop=True).equals(b[cols].reset_index(drop=True)))


def identity(mine, theirs):
    rows = []
    for stem, theirs_arm in SAME.items():
        a, b = mine.arms[C.arm_name(stem)], theirs.arms[theirs_arm]
        rows.append({"row": stem, "notice_incidents_identical": frames_equal(a.notice_incidents, b.notice_incidents),
                     "notice_events_identical": frames_equal(a.notice_events, b.notice_events),
                     "incidents_identical": frames_equal(a.incidents, b.incidents)})
    return pd.DataFrame(rows)


def main():
    m2_dir = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else M2_RUN
    mine = load_stream_run(C.RUNS / C.run_id("heldout"))
    theirs = load_stream_run(m2_dir)
    seeds_a = [int(s) for s in mine.arms[C.arm_name("reanchor")].results["seed"]]
    seeds_b = [int(s) for s in theirs.arms[MEDIUM].results["seed"]]
    assert seeds_a == seeds_b, "the two runs are not on the same streams"
    ident = identity(mine, theirs)
    ident.to_csv(C.OUT / "b3-vs-m2-identity.csv", index=False)
    print(ident.to_string(index=False))
    if not ident.drop(columns="row").to_numpy().all():
        sys.exit("the rerun comparator arms of the two runs are not identical: not one table")
    merged = Merged(mine, theirs)
    m = B.Measures(merged)
    pts = m.points()
    ci = m.boot(C.BOOT_SEED, C.N_RESAMPLES)
    rows = []
    for arm in merged.arms:
        i = m.row[arm]
        row = {"arm": arm}
        for key in B.MEASURES:
            row[key] = float(pts[key][i])
            row[f"{key}_lo"] = float(ci[key][0][i])
            row[f"{key}_hi"] = float(ci[key][1][i])
        rows.append(row)
    out = pd.DataFrame(rows)
    paired = []
    for stem in ROWS:
        for measure in MEASURES:
            d, lo, hi = m.paired_difference(measure, "medium_t100", stem, C.BOOT_SEED, C.N_RESAMPLES)
            paired.append({"medium_minus": stem, "measure": measure, "difference": d, "lower": lo, "higher": hi})
    pd.DataFrame(paired).to_csv(C.OUT / "b3-vs-m2-paired.csv", index=False)
    out.to_csv(C.OUT / "b3-vs-m2.csv", index=False)
    show = ["arm", *[k for meas in MEASURES for k in (meas,)]]
    print(out[show].to_string(index=False, float_format=lambda x: f"{x:.3f}"))
    print(pd.DataFrame(paired).to_string(index=False, float_format=lambda x: f"{x:+.3f}"))
    np.seterr(all="ignore")


if __name__ == "__main__":
    main()
