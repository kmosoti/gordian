"""Choose the follow-up rule's parameters and each row's selector parameters from the tuning streams only.

Usage:
  b4_select.py follow   after `b4-tunefollow-b5-rho0.7`: every configuration's tuning point to
                        experiments/exploration/b4-tuning-follow.csv, the choice (and the sensitivity
                        choices at the other tolerances) into b4-selected.json
  b4_select.py select   after `b4-tuneselect-b5-rho0.7`: every configuration's tuning point to
                        b4-tuning-select.csv, one choice per (row, selector) into b4-selected.json

The rules are fixed in `b4_common.py` before the run each governs, and repeated here. Stage F: among the
follow-up configurations within the background budget, with at most `tolerance` notices anchored on a slow
leak retired by the rule and hard non-leak quality no more than 0.005 under the arm without the rule, the
most decoy notices retired by the rule before escalation; ties by the fewest plain and hard notices
retired by the rule, then the larger readings, min_gain, max_fall and horizon. Stage S: per row and
selector, among the grid's configurations whose quality is at least 0.9 of the grid's best, the highest
quality per modelled second per stream; ties the fewer calls, then the more selective parameter. Nothing
is chosen on any held-out stream.
"""

import json
import sys

import pandas as pd

import b4_common as C
import b4_stats as S
from gordian_analysis.load import load_stream_run


def load_run(stage):
    run = load_stream_run(C.RUNS / C.run_id(stage))
    seeds = [int(s) for s in next(iter(run.arms.values())).results["seed"]]
    assert seeds == list(range(C.TUNING_SEEDS[0], C.TUNING_SEEDS[0] + C.TUNING_SEEDS[1])), "tuning seeds"
    return run


def load_sel():
    try:
        return C.load_selected()
    except FileNotFoundError:
        return {}


def save_sel(sel):
    with open(C.OUT / "b4-selected.json", "w") as fh:
        json.dump(sel, fh, indent=2)
        fh.write("\n")


def counts(arm):
    """One arm's tuning counts, pooled over the streams."""
    t = S.per_stream(arm)
    n = len(t)
    return {
        "streams": n,
        "quality": float(t["quality_num"].sum() / t["quality_den"].sum()),
        "quality_correct": int(t["quality_num"].sum()),
        "quality_incidents": int(t["quality_den"].sum()),
        "background_per_stream": float(t["notices_background"].mean()),
        "calls_per_stream": float(t["calls"].mean()),
        "cost_s_per_stream": float(t["total_cost_ns"].mean()) / 1e9,
        "leak_retired": int(t["s_followup_retired_leak"].sum()),
        "leak_lost": int(t["s_leak_lost"].sum()),
        "decoy_retired": int(t["s_followup_retired_decoy"].sum()),
        "decoy_retired_before": int(t["s_followup_before_escalation_decoy"].sum()),
        "plain_retired": int(t["s_followup_retired_plain"].sum()),
        "hard_retired": int(t["s_followup_retired_hard"].sum()),
        "background_retired": int(t["s_followup_retired_background"].sum()),
        "decoy_notices": int(t["s_notices_decoy"].sum()),
        "calls_decoy": int(t["s_calls_decoy"].sum()),
        "calls_plain": int(t["s_calls_plain"].sum()),
    }


def choose_follow(df, tolerance):
    """The follow-up rule's choice from a tuning table `df` (one row per configuration, with the columns
    `follow_stage` writes) at `tolerance` slow-leak notices the rule may retire: the row, how many were
    feasible and how many tied at the best. The rule is the module docstring's."""
    ok = df[(df["background_per_stream"] <= C.BACKGROUND_BUDGET + 1e-12)
            & (df["leak_retired"] <= tolerance)
            & (df["quality_loss"] <= C.QUALITY_LOSS_TOLERANCE + 1e-12)]
    feasible = len(ok)
    if feasible == 0:
        ok = df[df["leak_retired"] == df["leak_retired"].min()]
    top = ok[ok["decoy_retired_before"] == ok["decoy_retired_before"].max()]
    top = top.sort_values(["collateral", "p_readings", "p_min_gain", "fall_rank", "p_horizon_s"],
                          ascending=[True, False, False, False, False], kind="stable")
    return top.iloc[0], feasible, len(top)


def choose_selector(gd):
    """One (row, selector) choice from its grid `gd` (columns `value`, `quality`, `cost_s`, `calls`):
    among the configurations whose quality is at least 0.9 of the grid's best, the highest quality per
    modelled second; ties the fewer calls, then the larger value (the more selective). Returns the row
    (with `efficiency`) and the number tied."""
    gd = gd.copy()
    gd["efficiency"] = gd["quality"] / gd["cost_s"]
    ok = gd[gd["quality"] >= C.QUALITY_FLOOR * gd["quality"].max() - 1e-12]
    best = ok[ok["efficiency"] == ok["efficiency"].max()]
    best = best.sort_values(["calls", "value"], ascending=[True, False], kind="stable")
    return best.iloc[0], len(best)


def follow_stage():
    run = load_run("tunefollow")
    control = counts(run.arms["fol_none"])
    rows = []
    for p in C.follow_grid():
        row = {**{f"p_{k}": ("x" if v is None else v) for k, v in p.items()},
               **counts(run.arms[f"fol_{C.follow_stem_name(p)}"])}
        row["stem"] = C.follow_stem_name(p)
        rows.append(row)
    df = pd.DataFrame(rows)
    df["quality_loss"] = control["quality"] - df["quality"]
    df["collateral"] = df["plain_retired"] + df["hard_retired"]
    df["fall_rank"] = df["p_max_fall"].map(lambda v: 10**9 if v == "x" else v)
    df.to_csv(C.OUT / "b4-tuning-follow.csv", index=False)
    sel = load_sel()
    chosen = {}
    for tol in [0, *C.FOLLOW_TOLERANCES]:
        pick, feasible, tied = choose_follow(df, tol)
        p = C.follow_params(int(pick["p_readings"]), int(pick["p_horizon_s"]), int(pick["p_min_gain"]),
                            None if pick["p_max_fall"] == "x" else int(pick["p_max_fall"]))
        chosen[tol] = {
            "params": p, "json": C.follow_json(p), "feasible": int(feasible), "tied_at_the_best": int(tied),
            **{k: (float(pick[k]) if isinstance(pick[k], float) else int(pick[k])) for k in
               ("quality", "background_per_stream", "leak_retired", "decoy_retired_before", "collateral",
                "plain_retired", "hard_retired", "decoy_notices", "calls_decoy", "calls_plain")},
        }
    sel["follow"] = {
        "chosen": chosen[0],
        "tolerance": {str(t): chosen[t] for t in C.FOLLOW_TOLERANCES},
        "control": control,
        "rule": "most decoy notices retired by the rule before escalation, within the background budget, "
                "leaks retired <= tolerance (0), hard quality loss <= 0.005; ties: fewest plain and hard "
                "notices retired, larger readings, min_gain, max_fall, horizon (b4_common.py)",
    }
    save_sel(sel)
    print("control", json.dumps(control))
    print(df.sort_values(["decoy_retired_before", "leak_retired"], ascending=[False, True]).head(12)
          [["stem", "decoy_retired_before", "leak_retired", "collateral", "quality_loss",
            "background_per_stream"]].to_string(index=False))
    print(json.dumps(sel["follow"]["chosen"], indent=2))


def select_stage():
    sel = load_sel()
    run = load_run("tuneselect")
    m = S.Measures(run)
    pts = m.points()
    rows = []
    sel["select"] = {}
    stems = [s for s, _ in C.rows(C.C3.load_selected(), sel["follow"]["chosen"]["json"])]
    for stem in stems:
        chosen = {}
        for fam, arms in (("thr", [(t, C.thr_arm(stem, t)) for t in C.THR_PERSIST_S]),
                          ("chg", [(k, C.chg_arm(stem, k)) for k in C.CHG_K])):
            g = []
            for value, arm in arms:
                i = m.row[arm]
                r = {"stem": stem, "selector": fam, "value": value,
                     "quality": float(pts["quality"][i]), "cost_s": float(pts["cost_s_per_stream"][i]),
                     "calls": float(pts["calls_per_stream"][i]),
                     "background": float(pts["notices_on_background_per_stream"][i])}
                g.append(r)
                rows.append(r)
            gd = pd.DataFrame(g)
            pick, tied = choose_selector(gd)
            chosen[fam] = {"value": pick["value"].item(), "quality": float(pick["quality"]),
                           "cost_s": float(pick["cost_s"]), "calls": float(pick["calls"]),
                           "efficiency": float(pick["efficiency"]), "tied": int(tied),
                           "grid_best_quality": float(gd["quality"].max())}
        for ref, arm in (("oracle", C.oracle_arm(stem)), ("never", C.never_arm(stem)),
                         ("always", C.always_arm(stem))):
            i = m.row[arm]
            rows.append({"stem": stem, "selector": ref, "value": "", "quality": float(pts["quality"][i]),
                         "cost_s": float(pts["cost_s_per_stream"][i]),
                         "calls": float(pts["calls_per_stream"][i]),
                         "background": float(pts["notices_on_background_per_stream"][i])})
        sel["select"][stem] = {"thr_t_s": chosen["thr"]["value"], "chg_k": chosen["chg"]["value"],
                               "thr": chosen["thr"], "chg": chosen["chg"]}
    pd.DataFrame(rows).to_csv(C.OUT / "b4-tuning-select.csv", index=False)
    save_sel(sel)
    for stem in stems:
        s = sel["select"][stem]
        print(f"{stem:34s} thr t={s['thr_t_s']:>4}  q={s['thr']['quality']:.3f} cost={s['thr']['cost_s']:.3f}"
              f" | chg k={s['chg_k']:>3} q={s['chg']['quality']:.3f} cost={s['chg']['cost_s']:.3f}")


def main():
    stage = sys.argv[1] if len(sys.argv) > 1 else ""
    {"follow": follow_stage, "select": select_stage}.get(stage, lambda: sys.exit(__doc__))()


if __name__ == "__main__":
    main()
