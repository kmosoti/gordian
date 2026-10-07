"""L2's analysis: the criterion's three clauses as amended and written, the fourth reading, the
sample-efficiency curves, background and strict precision against streams seen, end states,
everything reported beside.

Usage: l2_analyze.py [STAGE]   (fresh, default) reads artifacts/runs/l2-STAGE-b5-rho0.7 and writes
experiments/exploration/l2-*.csv:

  table       every arm over all streams, the last 100 and the first 100: the noticing measures, the
              bounds, the quality and cost columns, 90% cluster-bootstrap intervals
  criterion   the clauses with their intervals and verdicts, the conjunction, and the fourth reading
  paired      the ESN minus every other arm, paired over streams: the end-state differences
              (last 100) and the streams 21-40 window
  perstream   every arm's per-stream counts: the raw material of every curve
  curves      W1's sample-efficiency curves read at 5, 10, 20, 25, 50, 100, 150 and 200 streams
  curves-full every arm's whole curve
  slopes      L1's slopes of the anchor-correct curve over the first 50, 100 and 200 streams, for
              continuity (not clauses here)
  blocks      background notices per stream, strict precision, anchor-correct and leak noticed
              against streams seen, in blocks of 20 (`gordian_analysis.measures.block_curve`)
  incidents   per arm, per tier and family: incidents, noticed, anchor-correct, latency

Readings are `l2_common`'s, fixed before the tuning run and the fresh run; the bootstrap is B1's
and B2's.
"""

import sys

import numpy as np
import pandas as pd

import b2_stats as B
import l1_stats as L
import l2_common as C
from gordian_analysis.load import load_stream_run
from gordian_analysis.measures import block_curve, paired_window_difference

STAGE = sys.argv[1] if len(sys.argv) > 1 else "fresh"
PREFIX = "l2" if STAGE == "fresh" else f"l2-{STAGE}"
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
    ("hard_quality", "quality_num", "quality_den"),
]
MEANS = [("notices_on_background_per_stream", "notices_background"),
         ("notices_on_plain_per_stream", "notices_plain"),
         ("notices_on_decoy_per_stream", "notices_decoy"),
         ("notices_per_stream", "notices"),
         ("calls_per_stream", "calls"),
         ("substrate_ns_per_stream", "substrate_ns"),
         ("total_cost_ns_per_stream", "total_cost_ns")]


def short(arm):
    return arm[len("sel_"):-len("_privileged")]


def stream_frame(m, arm, seeds):
    """One arm's per-stream counts as the frame `gordian_analysis.measures` reads."""
    i = m.row[arm]
    cols = ("notices_background", "notices", "notices_both", "hard_n", "hard_correct", "leak_n",
            "leak_noticed", "notices_plain", "notices_decoy", "calls")
    return pd.DataFrame({"seed": seeds, **{c: m.col[c][i].astype(int) for c in cols}})


def main():
    run = load_stream_run(RUN)
    m = B.Measures(run)
    n = m.n
    seeds = [int(s) for s in m.seeds]
    first = C.FRESH_SEEDS[0] if STAGE == "fresh" else C.TUNE_SEEDS[0]
    assert seeds == list(range(first, first + n)), "seeds out of stream order"
    arms = list(m.names)
    A = C.arm_name
    end_lo = n - C.END_STREAMS if n >= C.END_STREAMS else 0
    wins = {"all": L.Window(m, 0, n), "last100": L.Window(m, end_lo, n),
            "first100": L.Window(m, 0, min(100, n))}
    esn, off, learned, frozen, reanchor, ramp = (
        A(x) for x in ("esn", "esn_off", "learned", "frozen", "reanchor", C.RAMP_SPLIT))
    frames = {a: stream_frame(m, a, seeds) for a in arms}

    # ---- the table
    rows = []
    for arm in arms:
        for wname, w in wins.items():
            row = {"arm": short(arm), "window": wname, "streams": w.n}
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

    # ---- the clauses, as amended and written
    last = wins["last100"]
    num, den = "hard_correct", "hard_n"
    ac = {k: last.ratio(num, den, a) for k, a in
          (("esn", esn), ("off", off), ("learned", learned), ("frozen", frozen), ("reanchor", reanchor))}
    d1, lo1, hi1 = last.paired(num, den, esn, frozen)
    c1a_point = ac["esn"] >= ac["frozen"] + C.CLAUSE1_MARGIN
    c1a_bound = lo1 > C.CLAUSE1_LOWER_ABOVE
    bg = last.mean("notices_background", esn)
    bg_lo, bg_hi = last.boot_mean("notices_background", esn)
    sp = last.ratio("notices_both", "notices", esn)
    sp_lo, sp_hi = last.boot_ratio("notices_both", "notices", esn)
    c1b = bg <= C.BACKGROUND_BOUND
    c1c = sp >= C.STRICT_BOUND
    c1 = bool(c1a_point and c1a_bound and c1b and c1c)

    w2 = paired_window_difference(frames[esn], frames[off], "notices_background",
                                  first=C.CLAUSE2_WINDOW[0], last=C.CLAUSE2_WINDOW[1],
                                  resamples=C.N_RESAMPLES, seed=C.BOOT_SEED)
    c2_point = w2.point <= C.CLAUSE2_GAP
    c2_lower = w2.lower < C.CLAUSE2_LOWER_BELOW
    c2 = bool(c2_point and c2_lower)
    c2_strict = bool(c2_point and w2.higher < C.CLAUSE2_LOWER_BELOW)
    # the same difference read through L1's Window (a cross-check of the new function)
    lw = L.Window(m, C.CLAUSE2_WINDOW[0] - 1, C.CLAUSE2_WINDOW[1])
    d2x, lo2x, hi2x = lw.paired("notices_background", "one", esn, off)
    assert abs(d2x - w2.point) < 1e-9, (d2x, w2.point)
    assert abs(lo2x - w2.lower) < 1e-9 and abs(hi2x - w2.higher) < 1e-9, ((lo2x, hi2x), (w2.lower, w2.higher))

    d3, lo3, hi3 = last.paired(num, den, frozen, off)
    c3 = d3 >= C.CLAUSE3_GAP
    conj = bool(c1 and c2 and c3)

    crit = pd.DataFrame([
        {"clause": "1a", "statement": "ESN anchor-correct (last 100) >= frozen - 0.01 and paired lower bound > -0.03",
         "esn": ac["esn"], "reference": ac["frozen"], "difference": d1, "lower": lo1, "higher": hi1,
         "point_ok": c1a_point, "bound_ok": c1a_bound, "holds": bool(c1a_point and c1a_bound)},
        {"clause": "1b", "statement": "ESN background notices per stream (last 100) <= 6.82",
         "esn": bg, "reference": C.BACKGROUND_BOUND, "difference": bg - C.BACKGROUND_BOUND,
         "lower": bg_lo, "higher": bg_hi, "point_ok": c1b, "bound_ok": None, "holds": bool(c1b)},
        {"clause": "1c", "statement": "ESN strict precision (last 100) >= 0.67",
         "esn": sp, "reference": C.STRICT_BOUND, "difference": sp - C.STRICT_BOUND,
         "lower": sp_lo, "higher": sp_hi, "point_ok": c1c, "bound_ok": None, "holds": bool(c1c)},
        {"clause": 1, "statement": "clause 1: 1a and 1b and 1c", "esn": None, "reference": None,
         "difference": None, "lower": None, "higher": None, "point_ok": None, "bound_ok": None,
         "holds": c1},
        {"clause": 2, "statement": "ESN background per stream over streams 21-40 minus learning-off <= -10, paired lower bound < -5",
         "esn": w2.a, "reference": w2.b, "difference": w2.point, "lower": w2.lower, "higher": w2.higher,
         "point_ok": bool(c2_point), "bound_ok": bool(c2_lower), "holds": c2},
        {"clause": "2s", "statement": "beside, not the clause: the stricter reading, point <= -10 and paired upper bound < -5",
         "esn": w2.a, "reference": w2.b, "difference": w2.point, "lower": w2.lower, "higher": w2.higher,
         "point_ok": bool(c2_point), "bound_ok": bool(w2.higher < C.CLAUSE2_LOWER_BELOW), "holds": c2_strict},
        {"clause": 3, "statement": "learning-off anchor-correct (last 100) below frozen by >= 0.02",
         "esn": ac["off"], "reference": ac["frozen"], "difference": d3, "lower": lo3, "higher": hi3,
         "point_ok": bool(c3), "bound_ok": None, "holds": bool(c3)},
    ])
    d4, lo4, hi4 = last.paired(num, den, esn, learned)
    fourth = {"clause": "4", "statement": "reported separately: ESN anchor-correct (last 100) minus L1's learned medium, paired",
              "esn": ac["esn"], "reference": ac["learned"], "difference": d4, "lower": lo4, "higher": hi4,
              "point_ok": None, "bound_ok": None, "holds": None}
    crit.loc[len(crit)] = fourth
    crit["conjunction"] = conj
    crit.to_csv(C.OUT / f"{PREFIX}-criterion.csv", index=False)

    # ---- paired differences of the ESN against every other arm (beside the clauses)
    pr = []
    for other in ("esn_off", "learned", "frozen", "reanchor", C.RAMP_SPLIT):
        b = A(other)
        for wname, w in (("last100", last), ("all", wins["all"]), ("streams21_40", lw)):
            for name, nm, dn in (("hard_anchor_correct_share", "hard_correct", "hard_n"),
                                 ("hard_noticed_share", "hard_noticed", "hard_n"),
                                 ("leak_noticed_share", "leak_noticed", "leak_n"),
                                 ("leak_anchor_correct_share", "leak_correct", "leak_n"),
                                 ("strict_precision", "notices_both", "notices"),
                                 ("notices_on_background_per_stream", "notices_background", "one"),
                                 ("notices_per_stream", "notices", "one"),
                                 ("hard_quality", "quality_num", "quality_den"),
                                 ("calls_per_stream", "calls", "one"),
                                 ("substrate_ns_per_stream", "substrate_ns", "one")):
                d, lo, hi = w.paired(nm, dn, esn, b)
                pr.append({"esn_minus": other, "measure": name, "window": wname, "difference": d,
                           "lower": lo, "higher": hi})
    pd.DataFrame(pr).to_csv(C.OUT / f"{PREFIX}-paired.csv", index=False)

    # ---- per-stream counts
    ps = []
    for arm in arms:
        i = m.row[arm]
        for j, sd in enumerate(seeds):
            ps.append({"arm": short(arm), "stream": j + 1, "seed": sd,
                       **{c: int(m.col[c][i][j]) for c in ("notices_background", "notices", "notices_both",
                                                           "notices_plain", "notices_decoy", "hard_n",
                                                           "hard_correct", "leak_n", "leak_noticed",
                                                           "leak_correct", "calls")},
                       "substrate_ns": int(m.col["substrate_ns"][i][j]),
                       "total_cost_ns": int(m.col["total_cost_ns"][i][j])})
    pd.DataFrame(ps).to_csv(C.OUT / f"{PREFIX}-perstream.csv", index=False)

    # ---- curves and slopes
    cr, full, sl = [], [], []
    for arm in arms:
        for which in ("anchor", "leak", "all_hard"):
            c = L.curve(m, arm, which)
            for k in READ_AT:
                if k <= n:
                    r = c.iloc[k - 1]
                    cr.append({"arm": short(arm), "measure": which, "streams": k,
                               "cum_incidents": int(r.cum_incidents), "cum_correct": int(r.cum_correct),
                               "efficiency": float(r.efficiency)})
            f = c[["stream", "cum_incidents", "cum_correct", "efficiency"]].copy()
            f.insert(0, "measure", which)
            f.insert(0, "arm", short(arm))
            full.append(f)
        for k in C.SLOPE_WINDOWS:
            if k <= n:
                for kind in ("anchor", "leak"):
                    p, lo, hi = L.curve_slope(m, arm, kind, k)
                    sl.append({"arm": short(arm), "streams": k, "outcome": kind,
                               "reading": "R3 curve, per 100 streams", "slope": p, "lower": lo, "higher": hi})
    pd.DataFrame(cr).to_csv(C.OUT / f"{PREFIX}-curves.csv", index=False)
    pd.concat(full).to_csv(C.OUT / f"{PREFIX}-curves-full.csv", index=False)
    pd.DataFrame(sl).to_csv(C.OUT / f"{PREFIX}-slopes.csv", index=False)

    # ---- measures against streams seen, in blocks of 20
    bl = []
    for arm in arms:
        f = frames[arm]
        for name, nm, dn in (("notices_on_background_per_stream", "notices_background", None),
                             ("strict_precision", "notices_both", "notices"),
                             ("notices_per_stream", "notices", None),
                             ("hard_anchor_correct_share", "hard_correct", "hard_n"),
                             ("leak_noticed_share", "leak_noticed", "leak_n")):
            b = block_curve(f, nm, dn, block=C.BLOCK)
            b.insert(0, "measure", name)
            b.insert(0, "arm", short(arm))
            bl.append(b)
    pd.concat(bl).to_csv(C.OUT / f"{PREFIX}-blocks.csv", index=False)

    # ---- what each arm notices, by tier and family
    inc = []
    for arm in arms:
        ni = run.arms[arm].notice_incidents
        for (tier, family), g in ni.groupby(["tier", "family"], dropna=False):
            lat = g.loc[g["noticed"].astype(bool), "notice_latency_ns"].astype(float) / 1e9
            inc.append({"arm": short(arm), "tier": tier, "family": family if isinstance(family, str) else "",
                        "incidents": len(g), "noticed": int(g["noticed"].sum()),
                        "anchor_correct": int(g["anchor_correct"].sum()),
                        "site_correct": int(g["site_correct"].sum()),
                        "median_latency_s": float(lat.median()) if len(lat) else float("nan"),
                        "mean_notices": float(g["notices"].mean())})
    pd.DataFrame(inc).to_csv(C.OUT / f"{PREFIX}-incidents.csv", index=False)

    with pd.option_context("display.width", 250, "display.max_columns", 40):
        show = ["arm", "window", "hard_anchor_correct_share", "leak_noticed_share",
                "notices_on_background_per_stream", "strict_precision", "notices_per_incident",
                "substrate_ns_per_stream"]
        print(table[table.window != "first100"][show].to_string(index=False, float_format=lambda x: f"{x:.3f}"))
        print(crit.to_string(index=False, float_format=lambda x: f"{x:.4f}"))
        print("conjunction:", conj)
        pp = pd.DataFrame(pr)
        print(pp[pp.window == "last100"].to_string(index=False, float_format=lambda x: f"{x:.4f}"))


if __name__ == "__main__":
    main()
