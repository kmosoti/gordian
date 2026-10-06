"""Choose the parameters of the two parameterised noticers from the tuning streams only.

Usage: b1_select.py   after the tuning run (`b1-tune-b5-rho0.7`): writes
                      experiments/exploration/b1-tuning-points.csv and b1-selected.json

The rule is fixed in `b1_common.py` (`TUNE_OBJECTIVE`, `TUNE_BUDGET_REFERENCE`) before any run, and
repeated here: for each parameterised noticer, among the grid's parameters whose notices on
background per stream (on the 100 tuning streams, seeds 10000-10099, b = 5, rho = 0.7) do not
exceed the default `RungNoticer`'s, the one with the highest anchor-correct share of hard incidents
outside the slow-leak family; a tie goes to the parameter nearer the rung's own behaviour (the larger
quiet period, the smaller lookback); if none satisfies the constraint, the one with the fewest
background notices. Nothing is chosen on any held-out stream.
"""

import json

import pandas as pd

import b1_common as C
import b1_stats as B
from gordian_analysis.load import load_stream_run


def main():
    run = load_stream_run(C.RUNS / C.run_id("tune"))
    seeds = [int(s) for s in next(iter(run.arms.values())).results["seed"]]
    assert seeds == list(range(C.TUNING_SEEDS[0], C.TUNING_SEEDS[0] + C.TUNING_SEEDS[1])), "tuning seeds"
    m = B.Measures(run)
    pts = m.points()
    rows = []
    for name, kind, p in C.grid():
        arm = C.arm_name(name)
        i = m.row[arm]
        row = {"noticer": name, "kind": kind, **{f"p_{k}": v for k, v in p.items()}}
        row.update({k: float(v[i]) for k, v in pts.items()})
        rows.append(row)
    df = pd.DataFrame(rows)
    df.to_csv(C.OUT / "b1-tuning-points.csv", index=False)
    ref = df[df.noticer == C.TUNE_BUDGET_REFERENCE].iloc[0]
    budget = float(ref["notices_on_background_per_stream"])
    out = {
        "objective": C.TUNE_OBJECTIVE,
        "budget_reference": C.TUNE_BUDGET_REFERENCE,
        "budget_notices_on_background_per_stream": budget,
        "tuning_seeds": [C.TUNING_SEEDS[0], C.TUNING_SEEDS[0] + C.TUNING_SEEDS[1] - 1],
    }
    for kind, key, larger_is_closer in (("change_triggered", "p_q", True), ("earliest_anchor", "p_l", False)):
        g = df[df.kind == kind].copy()
        if kind in C.TUNE_EXCLUDED:  # a control, not a candidate (see b1_common.py, AMENDMENT)
            g = g[g[key] != C.TUNE_EXCLUDED[kind]]
        feasible = g[g["notices_on_background_per_stream"] <= budget + 1e-12]
        if len(feasible):
            best = feasible["hard_anchor_correct_share"].max()
            tied = feasible[feasible["hard_anchor_correct_share"] == best]
            pick = tied.sort_values(key, ascending=not larger_is_closer).iloc[0]
            how = "highest anchor-correct share within the background budget"
            if len(tied) > 1:
                how += f" (tie of {len(tied)} resolved toward the rung)"
        else:
            pick = g.sort_values("notices_on_background_per_stream").iloc[0]
            how = "no parameter within the background budget; the fewest background notices"
        out[kind] = {
            "arm": C.arm_name(pick["noticer"]),
            "noticer": pick["noticer"],
            "parameter": key[2:],
            "value": float(pick[key]),
            "hard_anchor_correct_share_tuning": float(pick["hard_anchor_correct_share"]),
            "notices_on_background_per_stream_tuning": float(pick["notices_on_background_per_stream"]),
            "feasible": [str(x) for x in feasible["noticer"]],
            "rule": how,
        }
    with open(C.OUT / "b1-selected.json", "w") as fh:
        json.dump(out, fh, indent=2)
        fh.write("\n")
    print(df.to_string(index=False, float_format=lambda x: f"{x:.3f}"))
    print(json.dumps(out, indent=2))


if __name__ == "__main__":
    main()
