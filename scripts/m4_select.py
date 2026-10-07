"""Choose the frozen M4 medium per tick length from the tuning runs only (seeds 10000-10099).

Usage: m4_select.py   writes experiments/exploration/m4-selected.json and m4-tuning-points.csv

The rule is `m4_common`'s, fixed before the first M4 tuning run: per tick length, among the medium
configurations of the stages in STAGES, (a) the criterion's own conjunction (all six conditions,
evaluated on the tuning streams against the comparator of the same stage) first; (b) ties to the
higher anchor-correct share, then the higher leak noticed share, then the higher strict precision
(background is a bound only), then the earlier stage and the arm name; (c) if none meets all six,
among those within the background bound, the largest graded conjunction
g = min(AC - 0.963, LB_AC + 0.03, LN - 0.976, LB_LN + 0.03, SP - 0.693) / 0.03, ties as in (b).
"""

import json

import pandas as pd

import m4_common as C
import m4_grids as G

STAGES = ["tune-a"]
PUBLIC = ("reanchor", "ramp_split_over_re2", "ramp_over_re2", "rung_z2", "rung_z3")


def noticer_of(stage, arm):
    arms, *_ = G.stage(stage)
    return dict(arms)[arm]


def conditions(r):
    """{condition: met} and the graded slacks of one configuration's tuning row."""
    met = {
        "anchoring_point": r.anchor_ok >= 0.963,
        "anchoring_lower_bound": r.ac_lo > -0.03,
        "leak_point": r.leak_noticed >= 0.976,
        "leak_lower_bound": r.ln_lo > -0.03,
        "precision_point": r.strict_prec >= 0.693,
        "background_point": r.background <= C.BACKGROUND_BOUND,
    }
    slack = [r.anchor_ok - 0.963, r.ac_lo + 0.03, r.leak_noticed - 0.976, r.ln_lo + 0.03,
             r.strict_prec - 0.693]
    return met, min(slack) / C.GRADE_UNIT


def points():
    rows = []
    for stage in STAGES:
        df = pd.read_csv(C.OUT / f"m4-tuning-{stage}.csv")
        ref = df[df.arm == C.COMPARATOR].iloc[0]
        for _, r in df.iterrows():
            if r.arm in PUBLIC:
                continue
            n = noticer_of(stage, r.arm)
            met, graded = conditions(r)
            row = {"stage": stage, "arm": r.arm, "tick_ms": n["tick_ns"] // C.MS}
            row.update({k: r[k] for k in r.index if k != "arm"})
            row.update({"ref_anchor_ok": ref.anchor_ok, "ref_leak_noticed": ref.leak_noticed,
                        "ref_strict_prec": ref.strict_prec, "ref_background": ref.background})
            row.update(met)
            row["conjunction"] = all(met.values())
            row["graded"] = graded
            rows.append(row)
    return pd.DataFrame(rows)


def main():
    pts = points()
    pts.to_csv(C.OUT / "m4-tuning-points.csv", index=False)
    out = {"rule": "the criterion's conjunction (six conditions on the tuning streams against the "
                   "comparator of the same stage); ties: higher anchor-correct, higher leak "
                   "noticed, higher strict precision, earlier stage, arm name; fallback: within "
                   "the background bound, the largest graded conjunction, ties as above",
           "stages": STAGES, "ticks": {}}
    for tick in C.TICKS_MS:
        g = pts[pts.tick_ms == tick].copy()
        g["stage_order"] = g.stage.map(STAGES.index)
        passing = g[g.conjunction]
        fallback = passing.empty
        cand = g[g.background_point].copy() if fallback else passing.copy()
        keys = (["graded"] if fallback else []) + ["anchor_ok", "leak_noticed", "strict_prec"]
        cand = cand.sort_values(keys + ["stage_order", "arm"],
                                ascending=[False] * len(keys) + [True, True])
        best = cand.iloc[0]
        tied = cand
        for k in keys:
            tied = tied[tied[k].round(12) == round(best[k], 12)]
        out["ticks"][str(tick)] = {
            "stage": best.stage, "arm": best.arm,
            "noticer": noticer_of(best.stage, best.arm),
            "tuning": {k: float(best[k]) for k in
                       ("anchor_ok", "leak_noticed", "strict_prec", "background", "ac_diff",
                        "ac_lo", "ln_diff", "ln_lo", "leak_anchor_ok", "per_incident", "cost_s",
                        "graded")},
            "comparator_tuning": {k: float(best[k]) for k in
                                  ("ref_anchor_ok", "ref_leak_noticed", "ref_strict_prec",
                                   "ref_background")},
            "configurations": int(len(g)),
            "meeting_the_conjunction": int(g.conjunction.sum()),
            "fallback": bool(fallback),
            "tied": [f"{a.stage}/{a.arm}" for _, a in tied.iterrows()],
        }
    with open(C.OUT / "m4-selected.json", "w") as fh:
        json.dump(out, fh, indent=2)
        fh.write("\n")
    print(json.dumps({t: {k: v for k, v in d.items() if k != "noticer"}
                      for t, d in out["ticks"].items()}, indent=2))


if __name__ == "__main__":
    main()
