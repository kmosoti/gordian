"""L1's analysis: the criterion's three clauses as written, the sample-efficiency curves, slopes,
end states, everything reported beside.

Usage: l1_analyze.py [STAGE]   (fresh, default, or dev) reads artifacts/runs/l1-STAGE-b5-rho0.7 and
writes experiments/exploration/l1-*.csv (l1-dev-*.csv for the dev stage):

  table      every arm over all streams and over the last 100: the noticing measures, the bounds,
             90% cluster-bootstrap intervals
  criterion  the three clauses with their intervals and verdicts, and the conjunction
  slopes     the slope of the anchor-correct outcome (and of the leak-noticed one) over the first
             50, 100 and 200 streams, share per 100 incidents seen, with intervals
  paired     beside the clauses: the learned arm minus each control, paired over streams: the end-state
             (last 100) differences of anchor-correct and leak-noticed shares and of the notices on
             background, and the differences of the curve slopes over the first 50, 100 and 200
             streams (a curve that rises while early noise dilutes rises for an arm that does not
             learn too; the difference against a control that does not learn is what the learner adds)
  perstream  every arm's per-stream counts (notices on background, all notices, hard non-leak
             incidents and anchor-correct, leaks and noticed): the raw material of every curve, and
             of the post-hoc background-by-stream reading in the report (not a clause, and chosen
             after the clauses were seen to be unable to show a learner that settles within a stream)
  curves     W1's sample-efficiency curves read at 5, 10, 20, 25, 50, 100, 150 and 200 streams
  curves-full  every arm's whole curve

Readings are `l1_common`'s, fixed before the fresh run; the bootstrap is B1's and B2's.
"""

import sys

import numpy as np
import pandas as pd

import l1_common as C
import l1_stats as L
import b2_stats as B
from gordian_analysis.load import load_stream_run

STAGE = sys.argv[1] if len(sys.argv) > 1 else "fresh"
PREFIX = "l1" if STAGE == "fresh" else f"l1-{STAGE}"
RUN = C.RUNS / C.run_id(STAGE)
READ_AT = (5, 10, 20, 25, 50, 100, 150, 200)

BESIDE = [
    ("hard_anchor_correct_share", "hard_correct", "hard_n"),
    ("hard_noticed_share", "hard_noticed", "hard_n"),
    ("leak_noticed_share", "leak_noticed", "leak_n"),
    ("leak_anchor_correct_share", "leak_correct", "leak_n"),
    ("notice_precision", "notices_on_incidents", "notices"),
    ("strict_precision", "notices_both", "notices"),
    ("notices_per_incident", "notices_on_incidents", "incidents"),
    ("plain_noticed_share", "plain_noticed", "plain_n"),
]
MEANS = [("notices_on_background_per_stream", "notices_background"),
         ("notices_per_stream", "notices")]


def main():
    run = load_stream_run(RUN)
    m = B.Measures(run)
    n = m.n
    seeds = [int(s) for s in m.seeds]
    first = C.FRESH_SEEDS[0] if STAGE == "fresh" else C.DEV_SEEDS[0]
    assert seeds == list(range(first, first + n)), "seeds out of stream order"
    arms = list(m.names)
    A = C.arm_name
    end_lo = n - C.END_STREAMS if n >= C.END_STREAMS else 0
    wins = {"all": L.Window(m, 0, n), "last100": L.Window(m, end_lo, n),
            "first100": L.Window(m, 0, min(100, n))}

    rows = []
    for arm in arms:
        short = arm[len("sel_"):-len("_privileged")]
        for wname, w in wins.items():
            row = {"arm": short, "window": wname, "streams": w.n}
            for name, num, den in BESIDE:
                row[name] = w.ratio(num, den, arm)
                lo, hi = w.boot_ratio(num, den, arm)
                row[name + "_lo"], row[name + "_hi"] = lo, hi
            for name, num in MEANS:
                row[name] = w.mean(num, arm)
                lo, hi = w.boot_mean(num, arm)
                row[name + "_lo"], row[name + "_hi"] = lo, hi
            rows.append(row)
    table = pd.DataFrame(rows)
    table.to_csv(C.OUT / f"{PREFIX}-table.csv", index=False)

    # ---- the three clauses
    learned, off, frozen, reanchor = (A(x) for x in C.CRITERION_ARMS)
    last = wins["last100"]
    num, den = "hard_correct", "hard_n"
    ac = {k: last.ratio(num, den, a) for k, a in
          (("learned", learned), ("off", off), ("frozen", frozen), ("reanchor", reanchor))}
    d1, lo1, hi1 = last.paired(num, den, learned, frozen)
    c1_point = ac["learned"] >= ac["frozen"] + C.CLAUSE1_MARGIN
    c1_bound = lo1 > C.CLAUSE1_LOWER_ABOVE
    s100 = L.curve_slope(m, learned, "anchor", C.CLAUSE_SLOPE_WINDOW)  # reading R3
    s100b = L.slope(run.arms[learned], "anchor", seeds[:C.CLAUSE_SLOPE_WINDOW])  # reading R3b
    c2 = s100[0] > 0 and s100[1] > 0
    c2b = s100b[0] > 0 and s100b[1] > 0
    d3, lo3, hi3 = last.paired(num, den, frozen, off)
    c3 = d3 >= C.CLAUSE3_GAP
    crit = pd.DataFrame([
        {"clause": 1, "statement": "learned anchor-correct (last 100) >= frozen - 0.01 and paired lower bound > -0.03",
         "learned": ac["learned"], "reference": ac["frozen"], "difference": d1, "lower": lo1,
         "higher": hi1, "point_ok": c1_point, "bound_ok": c1_bound, "holds": c1_point and c1_bound},
        {"clause": 2, "statement": "learned slope of W1's anchor-correct curve over the first 100 streams positive, lower bound > 0 (efficiency per 100 streams; reading R3)",
         "learned": s100[0], "reference": 0.0, "difference": s100[0], "lower": s100[1],
         "higher": s100[2], "point_ok": s100[0] > 0, "bound_ok": s100[1] > 0, "holds": c2},
        {"clause": 3, "statement": "learning-off anchor-correct (last 100) below frozen by >= 0.02",
         "learned": ac["off"], "reference": ac["frozen"], "difference": d3, "lower": lo3,
         "higher": hi3, "point_ok": c3, "bound_ok": None, "holds": c3},
    ])
    crit.loc[len(crit)] = {
        "clause": "2b", "statement": "beside, not the clause: learned incident-level slope over the first 100 streams (share per 100 incidents seen; reading R3b)",
        "learned": s100b[0], "reference": 0.0, "difference": s100b[0], "lower": s100b[1],
        "higher": s100b[2], "point_ok": s100b[0] > 0, "bound_ok": s100b[1] > 0, "holds": c2b}
    conj = bool(c1_point and c1_bound and c2 and c3)
    crit["conjunction"] = conj
    crit.to_csv(C.OUT / f"{PREFIX}-criterion.csv", index=False)

    # ---- slopes
    sl = []
    for arm in arms:
        short = arm[len("sel_"):-len("_privileged")]
        for k in C.SLOPE_WINDOWS:
            if k > n:
                continue
            for kind in ("anchor", "leak"):
                p, lo, hi, cnt = L.slope(run.arms[arm], kind, seeds[:k])
                sl.append({"arm": short, "streams": k, "outcome": kind, "reading": "R3b incident-level, per 100 incidents seen",
                           "slope": p, "lower": lo, "higher": hi, "incidents": cnt})
                p, lo, hi = L.curve_slope(m, arm, kind, k)
                sl.append({"arm": short, "streams": k, "outcome": kind, "reading": "R3 curve, per 100 streams",
                           "slope": p, "lower": lo, "higher": hi, "incidents": cnt})
    pd.DataFrame(sl).to_csv(C.OUT / f"{PREFIX}-slopes.csv", index=False)

    # ---- paired differences of the learned arm against every other arm (beside the clauses)
    pr = []
    others = [x for x in ("learned_off", "frozen", "reanchor", C.RAMP_SPLIT)]
    for other in others:
        b = A(other)
        for wname, w in (("last100", last), ("all", wins["all"])):
            for name, num, den in (("hard_anchor_correct_share", "hard_correct", "hard_n"),
                                   ("leak_noticed_share", "leak_noticed", "leak_n"),
                                   ("leak_anchor_correct_share", "leak_correct", "leak_n"),
                                   ("strict_precision", "notices_both", "notices"),
                                   ("notices_on_background_per_stream", "notices_background", "one")):
                d, lo, hi = w.paired(num, den, learned, b)
                pr.append({"learned_minus": other, "measure": name, "window": wname, "difference": d,
                           "lower": lo, "higher": hi})
        for kind in ("anchor", "leak"):
            for k in C.SLOPE_WINDOWS:
                if k <= n:
                    d, lo, hi = L.curve_slope_diff(m, learned, b, kind, k)
                    pr.append({"learned_minus": other, "measure": f"curve_slope_{kind}_per_100_streams",
                               "window": f"first{k}", "difference": d, "lower": lo, "higher": hi})
    pd.DataFrame(pr).to_csv(C.OUT / f"{PREFIX}-paired.csv", index=False)

    # ---- per-stream counts
    ps = []
    for arm in arms:
        short = arm[len("sel_"):-len("_privileged")]
        i = m.row[arm]
        for j, sd in enumerate(seeds):
            ps.append({"arm": short, "stream": j + 1, "seed": sd,
                       **{c: int(m.col[c][i][j]) for c in ("notices_background", "notices", "hard_n",
                                                           "hard_correct", "leak_n", "leak_noticed")}})
    pd.DataFrame(ps).to_csv(C.OUT / f"{PREFIX}-perstream.csv", index=False)

    # ---- curves
    cr, full = [], []
    for arm in arms:
        short = arm[len("sel_"):-len("_privileged")]
        for which in ("anchor", "leak", "all_hard"):
            c = L.curve(m, arm, which)
            for k in READ_AT:
                if k <= n:
                    r = c.iloc[k - 1]
                    cr.append({"arm": short, "measure": which, "streams": k,
                               "cum_incidents": int(r.cum_incidents), "cum_correct": int(r.cum_correct),
                               "efficiency": float(r.efficiency)})
            f = c[["stream", "cum_incidents", "cum_correct", "efficiency"]].copy()
            f.insert(0, "measure", which)
            f.insert(0, "arm", short)
            full.append(f)
    pd.DataFrame(cr).to_csv(C.OUT / f"{PREFIX}-curves.csv", index=False)
    pd.concat(full).to_csv(C.OUT / f"{PREFIX}-curves-full.csv", index=False)

    with pd.option_context("display.width", 250, "display.max_columns", 40):
        show = ["arm", "window", "hard_anchor_correct_share", "leak_noticed_share",
                "notices_on_background_per_stream", "strict_precision", "notices_per_incident"]
        print(table[table.window != "first100"][show].to_string(index=False, float_format=lambda x: f"{x:.3f}"))
        print(crit.to_string(index=False, float_format=lambda x: f"{x:.4f}"))
        print("conjunction:", conj)
        print(pd.DataFrame(pr).to_string(index=False, float_format=lambda x: f"{x:.4f}"))
        sls = pd.DataFrame(sl)
        print(sls[sls.outcome == "anchor"].to_string(index=False, float_format=lambda x: f"{x:.3f}"))
        # the point of the weighted curve slope is W1's curve's own slope
        c = L.curve(m, learned, "anchor").iloc[:C.CLAUSE_SLOPE_WINDOW]
        c = c.dropna(subset=["efficiency"])
        own = np.polyfit(c["stream"], c["efficiency"], 1)[0] * 100
        assert abs(own - s100[0]) < 1e-9, (own, s100[0])


if __name__ == "__main__":
    main()
