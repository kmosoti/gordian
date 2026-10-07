"""C1's analysis: the reproduction check, the table, the paired differences, the in-run cost columns.

Usage: c1_analyze.py [STAGE] [RUN_DIR]   (STAGE heldout, default, or tune; RUN_DIR defaults to
artifacts/runs/c1-STAGE-b5-rho0.7)

Writes experiments/exploration/c1-*.csv (c1-tune-*.csv for the tuning stage):

  reproduction  per pair of arms and per file: rows, rows identical, rows that differ (R5 of `c1_common`)
  differences   every differing row of every pair, listed (empty if the record is reproduced)
  table         every arm's measures with 90% cluster-bootstrap intervals (B2's arithmetic)
  paired        A minus B over the same resamples for every measure: dataflow minus the B3 row, the
                unbilled control minus the B3 row, the medium minus the dataflow noticer, the medium
                minus the B3 row
  clock-effect  the billed dataflow arm's incidents.csv against the unbilled control's: the columns
                that differ and by how much (what the bill's clock does)
  cost-run      per stream, per arm, from the run's own files: the bill's compute (`bill_compute`,
                which carries a noticer's charge), the arm's measured bookkeeping time
                (`measured_sched_ns`: the noticer's wall time is inside it), and the difference of the
                dataflow arm from its unbilled control (its charge, in run)

No claim is made about which noticer is better; the rows are numbers and the differences are paired.
"""

import sys

import numpy as np
import pandas as pd

import b2_stats as B
import c1_common as C
from gordian_analysis.load import load_stream_run

STAGE = sys.argv[1] if len(sys.argv) > 1 else "heldout"
RUN = (C.RUNS / sys.argv[2]) if len(sys.argv) > 2 else C.RUNS / C.run_id(STAGE)
PREFIX = "c1" if STAGE == "heldout" else f"c1-{STAGE}"

# The measures the report names, in the order of the table.
MAIN = ["hard_anchor_correct_share", "leak_noticed_share", "leak_anchor_correct_share",
        "notices_on_background_per_stream", "strict_precision"]
BESIDE = ["hard_noticed_share", "notice_precision", "notices_per_incident", "notices_per_stream",
          "quality", "leak_quality", "cost_s_per_stream", "calls_per_stream"]
VOLATILE_RESULTS = {"run_id", "bill_compute", "measured_component_ns", "measured_sched_ns",
                    "measured_harness_ns", "arm_position"}

PAIRS = [("dataflow_vs_b3", C.DATAFLOW, C.B3_ROW), ("dataflow_unbilled_vs_b3", C.DATAFLOW_FREE, C.B3_ROW)]


def drop(frame, cols):
    return frame.drop(columns=[c for c in cols if c in frame.columns]).reset_index(drop=True)


def listing(stage, pair, file, a, b, key):
    """Rows of `a` not in `b` and of `b` not in `a`, per the columns both have, as a frame."""
    ra = {tuple(r) for r in a.astype(str).itertuples(index=False, name=None)}
    rb = {tuple(r) for r in b.astype(str).itertuples(index=False, name=None)}
    out = []
    for side, rows, other in (("only_dataflow_side", ra - rb, "a"), ("only_b3_side", rb - ra, "b")):
        for r in sorted(rows):
            out.append({"stage": stage, "pair": pair, "file": file, "side": side,
                        **{c: v for c, v in zip(a.columns, r)}})
    return pd.DataFrame(out)


def reproduction(run):
    rows, diffs = [], []
    for pair, left, right in PAIRS:
        la, lb = run.arms[C.arm_name(left)], run.arms[C.arm_name(right)]
        files = {
            "notice_events": (la.notice_events, lb.notice_events, ["run_id", "noticer"]),
            "notice_incidents": (la.notice_incidents, lb.notice_incidents, ["run_id", "noticer"]),
            "notices": (la.notices, lb.notices, ["run_id", "noticer"]),
            "incidents": (la.incidents, lb.incidents, ["run_id"]),
            "results": (la.results, lb.results, list(VOLATILE_RESULTS)),
        }
        for file, (fa, fb, cols) in files.items():
            a, b = drop(fa, cols), drop(fb, cols)
            same = a.shape == b.shape and bool(a.astype(str).equals(b.astype(str)))
            differing = 0
            if not same:
                d = listing(STAGE, pair, file, a, b, None)
                differing = len(d)
                diffs.append(d)
            rows.append({"stage": STAGE, "pair": pair, "file": file, "rows_dataflow_side": len(a),
                         "rows_b3_side": len(b), "identical": same, "rows_in_one_only": differing})
    rep = pd.DataFrame(rows)
    rep.to_csv(C.OUT / f"{PREFIX}-reproduction.csv", index=False)
    d = pd.concat(diffs, ignore_index=True) if diffs else pd.DataFrame(
        columns=["stage", "pair", "file", "side"])
    d.to_csv(C.OUT / f"{PREFIX}-differences.csv", index=False)
    print(rep.to_string(index=False))
    print(f"{len(d)} differing rows listed")
    return rep, d


def clock_effect(run):
    """What the bill's clock did: the billed dataflow arm's `incidents.csv` against the unbilled
    control's, per column that differs (the noticer's charge advances the logical clock by
    microseconds, which moves the instants of declarations)."""
    a = run.arms[C.arm_name(C.DATAFLOW)].incidents.reset_index(drop=True)
    b = run.arms[C.arm_name(C.DATAFLOW_FREE)].incidents.reset_index(drop=True)
    rows = []
    for col in a.columns:
        if col == "run_id":
            continue
        diff = (a[col].astype(str) != b[col].astype(str))
        if not diff.any():
            continue
        row = {"stage": STAGE, "column": col, "rows_differing": int(diff.sum()), "rows": len(a)}
        try:
            d = (pd.to_numeric(a[col]) - pd.to_numeric(b[col]))[diff]
            row.update({"min_difference": float(d.min()), "max_difference": float(d.max()),
                        "mean_difference": float(d.mean())})
        except (ValueError, TypeError):
            pass
        rows.append(row)
    out = pd.DataFrame(rows, columns=["stage", "column", "rows_differing", "rows", "min_difference",
                                      "max_difference", "mean_difference"])
    out.to_csv(C.OUT / f"{PREFIX}-clock-effect.csv", index=False)
    print(out.to_string(index=False))
    return out


def cost_run(run):
    rows = []
    for name, arm in run.arms.items():
        measured = pd.read_csv(arm.path / "measured.csv").set_index("seed")
        res = arm.results.set_index("seed")
        for seed in res.index:
            rows.append({"arm": name[len("sel_"):-len("_privileged")], "seed": int(seed),
                         "bill_compute_ns": int(res.loc[seed, "bill_compute"]),
                         "measured_component_ns": int(measured.loc[seed, "measured_component_ns"]),
                         "measured_sched_ns": int(measured.loc[seed, "measured_sched_ns"]),
                         "measured_harness_ns": int(measured.loc[seed, "measured_harness_ns"]),
                         "steps": int(res.loc[seed, "steps"])})
    t = pd.DataFrame(rows)
    t.to_csv(C.OUT / f"{PREFIX}-cost-run.csv", index=False)
    wide = t.pivot(index="seed", columns="arm", values="bill_compute_ns")
    if C.DATAFLOW in wide and C.DATAFLOW_FREE in wide:
        own = (wide[C.DATAFLOW] - wide[C.DATAFLOW_FREE])
        print(f"dataflow arm's own charge in run, per stream: mean {own.mean() / 1e6:.3f} ms, "
              f"min {own.min() / 1e6:.3f}, max {own.max() / 1e6:.3f}")
    return t


def main():
    run = load_stream_run(RUN)
    seeds = [int(s) for s in next(iter(run.arms.values())).results["seed"]]
    first, count = C.seeds(STAGE)
    assert seeds == list(range(first, first + count)), "not the streams the stage names"
    reproduction(run)
    clock_effect(run)
    cost_run(run)
    if STAGE != "heldout":
        return
    arms = [C.arm_name(n) for n in (C.DATAFLOW, C.DATAFLOW_FREE, C.B3_ROW, C.MEDIUM)]
    m = B.Measures(run, arms)
    pts = m.points()
    ci = m.boot(C.BOOT_SEED, C.N_RESAMPLES)
    rows = []
    for arm in arms:
        i = m.row[arm]
        row = {"arm": arm[len("sel_"):-len("_privileged")], "streams": m.n}
        for key in B.MEASURES:
            row[key] = float(pts[key][i])
            row[key + "_lo"], row[key + "_hi"] = float(ci[key][0][i]), float(ci[key][1][i])
        rows.append(row)
    table = pd.DataFrame(rows)
    table.to_csv(C.OUT / f"{PREFIX}-table.csv", index=False)
    show = ["arm"] + MAIN
    print(table[show].to_string(index=False, float_format=lambda x: f"{x:.3f}"))
    comparisons = [(C.DATAFLOW, C.B3_ROW), (C.DATAFLOW_FREE, C.B3_ROW), (C.DATAFLOW_FREE, C.DATAFLOW),
                   (C.MEDIUM, C.DATAFLOW), (C.MEDIUM, C.B3_ROW)]
    paired = []
    for a, b in comparisons:
        for measure in B.MEASURES:
            d, lo, hi = m.paired_difference(measure, C.arm_name(a), C.arm_name(b), C.BOOT_SEED, C.N_RESAMPLES)
            paired.append({"a": a, "minus_b": b, "measure": measure, "difference": d, "lower": lo,
                           "higher": hi})
    p = pd.DataFrame(paired)
    p.to_csv(C.OUT / f"{PREFIX}-paired.csv", index=False)
    print(p[p.measure.isin(MAIN)].to_string(index=False, float_format=lambda x: f"{x:+.3f}"))
    np.seterr(all="ignore")


if __name__ == "__main__":
    main()
