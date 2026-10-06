"""Choose the frozen M3 medium per tick length from the tuning runs only (seeds 10000-10099).

Usage: m3_select.py   writes experiments/exploration/m3-selected.json and m3-tuning-points.csv

The rule is `m3_common`'s, fixed before the first M3 tuning run: per tick length, among the medium
configurations of the stages in STAGES with background notices per stream <= 5.6 and strict
precision >= 0.70, the one that maximizes min((AC - AC_ref) / 0.03, (LN - LN_ref) / 0.20); ties to
the fewer background notices, the higher strict precision, the earlier stage, the arm name. If none
meets both tuning bounds at a tick length: among those within the background budget, the maximum of
min(z1, z2, (SP - 0.70) / 0.03), said in the selection. AC_ref and LN_ref are the re-anchor's on the
same streams (every stage runs it).
"""

import json

import pandas as pd

import m3_common as C
import m3_grids as G

STAGES = ["tune-a", "tune-b", "tune-c"]


def noticer_of(stage, arm):
    arms, *_ = G.stage(stage)
    return dict(arms)[arm]


def points():
    rows = []
    for stage in STAGES:
        df = pd.read_csv(C.OUT / f"m3-tuning-{stage}.csv")
        ref = df[df.arm == "reanchor"].iloc[0]
        for _, r in df.iterrows():
            if r.arm in ("reanchor", "rung_z2", "rung_z3"):
                continue
            n = noticer_of(stage, r.arm)
            rows.append({
                "stage": stage, "arm": r.arm, "tick_ms": n["tick_ns"] // C.MS,
                "anchor_ok": r.anchor_ok, "leak_noticed": r.leak_noticed,
                "background": r.background, "strict_prec": r.strict_prec,
                "per_incident": r.per_incident,
                "ref_anchor_ok": ref.anchor_ok, "ref_leak_noticed": ref.leak_noticed,
                "ref_strict_prec": ref.strict_prec,
                "z1": (r.anchor_ok - ref.anchor_ok) / C.RESULT1["margin"],
                "z2": (r.leak_noticed - ref.leak_noticed) / C.RESULT2["margin"],
                "z3": (r.strict_prec - C.TUNE_STRICT) / 0.03,
            })
    pts = pd.DataFrame(rows)
    pts["score"] = pts[["z1", "z2"]].min(axis=1)
    pts["fallback_score"] = pts[["z1", "z2", "z3"]].min(axis=1)
    pts["bg_ok"] = pts.background <= C.TUNE_BACKGROUND + 1e-9
    pts["sp_ok"] = pts.strict_prec >= C.TUNE_STRICT - 1e-9
    pts["feasible"] = pts.bg_ok & pts.sp_ok
    return pts


def main():
    pts = points()
    pts.to_csv(C.OUT / "m3-tuning-points.csv", index=False)
    out = {"rule": "max min(z1, z2) with background <= 5.6 and strict precision >= 0.70; ties: "
                   "fewer background notices, higher strict precision, earlier stage, arm name; "
                   "fallback: max min(z1, z2, z3) within the background budget",
           "tune_background": C.TUNE_BACKGROUND, "tune_strict": C.TUNE_STRICT, "stages": STAGES,
           "ticks": {}}
    for tick in C.TICKS_MS:
        g = pts[pts.tick_ms == tick].copy()
        g["stage_order"] = g.stage.map(STAGES.index)
        feas = g[g.feasible]
        fallback = feas.empty
        if fallback:
            feas = g[g.bg_ok].copy()
            key = "fallback_score"
        else:
            key = "score"
        feas = feas.sort_values([key, "background", "strict_prec", "stage_order", "arm"],
                                ascending=[False, True, False, True, True])
        best = feas.iloc[0]
        tied = feas[(feas[key].round(6) == round(best[key], 6))
                    & (feas.background.round(2) == round(best.background, 2))
                    & (feas.strict_prec.round(3) == round(best.strict_prec, 3))]
        out["ticks"][str(tick)] = {
            "stage": best.stage, "arm": best.arm,
            "noticer": noticer_of(best.stage, best.arm),
            "tuning": {k: float(best[k]) for k in
                       ("anchor_ok", "leak_noticed", "background", "strict_prec", "per_incident",
                        "z1", "z2", "z3", "score", "fallback_score")},
            "feasible_configurations": int(g.feasible.sum()),
            "fallback": bool(fallback),
            "tied": [f"{a.stage}/{a.arm}" for _, a in tied.iterrows()],
        }
    with open(C.OUT / "m3-selected.json", "w") as fh:
        json.dump(out, fh, indent=2)
        fh.write("\n")
    print(json.dumps({t: {k: v for k, v in d.items() if k != "noticer"}
                      for t, d in out["ticks"].items()}, indent=2))


if __name__ == "__main__":
    main()
