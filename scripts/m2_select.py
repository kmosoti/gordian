"""Choose the frozen medium per tick length from the tuning runs only (seeds 10000-10099).

Usage: m2_select.py   writes experiments/exploration/m2-selected.json and m2-tuning-points.csv

The rule is `m2_common`'s (fixed before the first tuning run; its budget re-set at the same margin
when the chief re-fixed the comparator): per tick length, among the medium configurations of the
tuning stages, the one that maximizes min((AC - AC_ref) / 0.03, (LN - LN_ref) / 0.20) among those
with notices on background per stream <= TUNE_BACKGROUND (5.6), ties to the fewer background notices.
AC_ref and LN_ref are the re-anchor comparator's on the same streams (every stage from tune-b on
runs it, with the same numbers).

Readings, stated here because the rule as first written did not settle them (added after the
tuning tables were seen and before any held-out run):
- Stages: tune-b to tune-f. tune-a ran before the fix that stopped a live anomaly holding an anchor
  from suppressing a notice, so its medium rows are not the code being frozen; it ran no re-anchor.
- Residual ties (equal anchor-correct, leak noticed and background to two decimals, which happens
  when two configurations differ only in a window no tuning stream exercised) go to the earlier
  stage and then the arm name in sorted order; the tied set is recorded.
- At a tick length where no configuration has a positive score, the rule still picks its maximum,
  and the selection says so.
"""

import json

import pandas as pd

import m2_common as C
import m2_grids as G

STAGES = ["tune-b", "tune-c", "tune-d", "tune-e", "tune-f"]


def noticer_of(stage, arm):
    arms, *_ = G.stage(stage)
    return dict(arms)[arm]


def main():
    rows = []
    for stage in STAGES:
        df = pd.read_csv(C.OUT / f"m2-tuning-{stage}.csv")
        ref = df[df.arm == "reanchor"].iloc[0]
        for _, r in df.iterrows():
            if r.arm in ("reanchor", "rung_z2", "rung_z3"):
                continue
            n = noticer_of(stage, r.arm)
            rows.append({
                "stage": stage, "arm": r.arm, "tick_ms": n["tick_ns"] // C.MS,
                "anchor_ok": r.anchor_ok, "leak_noticed": r.leak_noticed,
                "background": r.background, "ref_anchor_ok": ref.anchor_ok,
                "ref_leak_noticed": ref.leak_noticed,
                "z1": (r.anchor_ok - ref.anchor_ok) / C.RESULT1["margin"],
                "z2": (r.leak_noticed - ref.leak_noticed) / C.RESULT2["margin"],
            })
    pts = pd.DataFrame(rows)
    pts["score"] = pts[["z1", "z2"]].min(axis=1)
    pts["feasible"] = pts.background <= C.TUNE_BACKGROUND + 1e-9
    pts.to_csv(C.OUT / "m2-tuning-points.csv", index=False)
    out = {"rule": "max min(z1, z2) within the tuning background budget; ties: fewer background "
                   "notices, then the earlier stage, then the arm name",
           "tune_background": C.TUNE_BACKGROUND, "stages": STAGES, "ticks": {}}
    for tick in C.TICKS_MS:
        g = pts[(pts.tick_ms == tick) & pts.feasible].copy()
        g["stage_order"] = g.stage.map(STAGES.index)
        g = g.sort_values(["score", "background", "stage_order", "arm"],
                          ascending=[False, True, True, True])
        best = g.iloc[0]
        tied = g[(g.score.round(6) == round(best.score, 6))
                 & (g.background.round(2) == round(best.background, 2))]
        out["ticks"][str(tick)] = {
            "stage": best.stage, "arm": best.arm,
            "noticer": noticer_of(best.stage, best.arm),
            "tuning": {k: float(best[k]) for k in
                       ("anchor_ok", "leak_noticed", "background", "z1", "z2", "score")},
            "feasible_configurations": int(len(g)),
            "tied": [f"{a.stage}/{a.arm}" for _, a in tied.iterrows()],
            "positive_score": bool(best.score > 0),
        }
    with open(C.OUT / "m2-selected.json", "w") as fh:
        json.dump(out, fh, indent=2)
        fh.write("\n")
    print(json.dumps({t: {k: v for k, v in d.items() if k != "noticer"}
                      for t, d in out["ticks"].items()}, indent=2))


if __name__ == "__main__":
    main()
