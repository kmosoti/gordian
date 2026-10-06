"""Choose the parameters of the ramp noticer, the splitting noticer and the hold delays from the tuning
streams only.

Usage:
  b3_select.py ramp     after `b3-tuneramp-b5-rho0.7`: every configuration's tuning point to
                        experiments/exploration/b3-tuning-ramp.csv, the choice into b3-selected.json
  b3_select.py split    after `b3-tunesplit-b5-rho0.7`: b3-tuning-split.csv, the choice per base
  b3_select.py delay    after `b3-tunedelay-b5-rho0.7`: b3-tuning-delays.csv, one delay per table row

The rules are fixed in `b3_common.py` before the run each governs, and repeated here: the ramp
noticer's choice is the configuration of the highest leak noticed share among those within the background
budget (BACKGROUND_BUDGET notices on background per stream, the whole noticer's), then the highest leak
anchor-correct share, then the fewest notices on background, then the most conservative parameters; the
splitting noticer's is the highest anchor-correct share of hard incidents outside the slow-leak family
within the budget, ties toward the larger gap and the larger burst, per base; a delay is R5's rule: the
highest quality, the cheapest of ties. Nothing is chosen on any held-out stream.
"""

import json
import sys

import pandas as pd

import b2_stats as B
import b3_common as C
import b3_manifests as M
from gordian_analysis.load import load_stream_run


def load_run(stage):
    run = load_stream_run(C.RUNS / C.run_id(stage))
    seeds = [int(s) for s in next(iter(run.arms.values())).results["seed"]]
    assert seeds == list(range(C.TUNING_SEEDS[0], C.TUNING_SEEDS[0] + C.TUNING_SEEDS[1])), "tuning seeds"
    return run


def table(run, configs):
    """`configs`: (stem, kind, params) -> a DataFrame of the tuning points of each arm."""
    m = B.Measures(run)
    pts = m.points()
    rows = []
    for stem, kind, p in configs:
        i = m.row[C.arm_name(stem)]
        row = {"stem": stem, "kind": kind, **{f"p_{k}": v for k, v in p.items()}}
        row.update({k: float(v[i]) for k, v in pts.items()})
        rows.append(row)
    return pd.DataFrame(rows)


def load_sel():
    try:
        return C.load_selected()
    except FileNotFoundError:
        return {}


def save_sel(sel):
    with open(C.OUT / "b3-selected.json", "w") as fh:
        json.dump(sel, fh, indent=2)
        fh.write("\n")


def choose(df, objective, tie_keys, budget=C.BACKGROUND_BUDGET):
    """The best row of `df` by the rule: within the budget (else the fewest notices on background,
    flagged); the highest `objective`; ties by `tie_keys` (a list of (column, ascending)).
    Returns (row, number tied at the best, number feasible, within_budget)."""
    feasible = df[df["notices_on_background_per_stream"] <= budget + 1e-12]
    within = len(feasible) > 0
    if not within:
        feasible = df[df["notices_on_background_per_stream"] == df["notices_on_background_per_stream"].min()]
    best = feasible[objective].max()
    tied = feasible[feasible[objective] == best]
    tied = tied.sort_values([k for k, _ in tie_keys], ascending=[a for _, a in tie_keys], kind="stable")
    return tied.iloc[0], len(tied), len(feasible), within


COLS = ["stem", "leak_noticed_share", "leak_anchor_correct_share", "hard_anchor_correct_share",
        "hard_noticed_share", "notices_on_background_per_stream", "notices_per_incident", "notice_precision"]


def ramp():
    run = load_run("tuneramp")
    configs = [("rung_z3", "base", {})] + [(C.name_of("r3", ramp=p), "ramp", p) for p in C.ramp_grid()]
    df = table(run, configs)
    df.to_csv(C.OUT / "b3-tuning-ramp.csv", index=False)
    grid = df[df.kind == "ramp"].copy()
    pick, tied, feasible, within = choose(grid, C.TUNE_RAMP_OBJECTIVE, C.ramp_tie_keys())
    rung = df[df.stem == "rung_z3"].iloc[0]
    sel = load_sel()
    sel["budget_notices_on_background_per_stream"] = C.BACKGROUND_BUDGET
    sel["tuning_seeds"] = [C.TUNING_SEEDS[0], C.TUNING_SEEDS[0] + C.TUNING_SEEDS[1] - 1]
    params = {k[2:]: int(pick[k]) for k in pick.index if k.startswith("p_")}
    sel["ramp"] = {
        "chosen": {
            "name": pick["stem"], "params": params,
            "leak_noticed_share_tuning": float(pick["leak_noticed_share"]),
            "leak_anchor_correct_share_tuning": float(pick["leak_anchor_correct_share"]),
            "notices_on_background_per_stream_tuning": float(pick["notices_on_background_per_stream"]),
            "hard_anchor_correct_share_tuning": float(pick["hard_anchor_correct_share"]),
        },
        "rung_z3_tuning": {k: float(rung[k]) for k in ("leak_noticed_share", "leak_anchor_correct_share",
                                                       "hard_anchor_correct_share",
                                                       "notices_on_background_per_stream")},
        "feasible": int(feasible), "of": len(grid), "tied_at_the_best": int(tied), "within_budget": bool(within),
        "rule": "highest leak noticed share within the background budget; then highest leak anchor-correct, "
                "fewest notices on background, larger min_readings, larger min_rise, smaller max_step, smaller "
                "max_drop, smaller gap (b3_common.py)",
    }
    save_sel(sel)
    top = grid.sort_values(["leak_noticed_share", "leak_anchor_correct_share"], ascending=False).head(12)
    print(df[df.kind == "base"][COLS].to_string(index=False, float_format=lambda x: f"{x:.3f}"))
    print(top[COLS].to_string(index=False, float_format=lambda x: f"{x:.3f}"))
    print(json.dumps(sel["ramp"], indent=2))


def split():
    run = load_run("tunesplit")
    sel = load_sel()
    frames = []
    for base, control in (("r3", "rung_z3"), ("re2", "reanchor")):
        configs = [(control, "base", {})] + [(C.name_of(base, split=p), "split", p) for p in C.split_grid()]
        df = table(run, configs)
        df.insert(0, "base", base)
        frames.append(df)
        grid = df[df.kind == "split"].copy()
        pick, tied, feasible, within = choose(grid, C.TUNE_SPLIT_OBJECTIVE, C.split_tie_keys())
        ctl = df[df.kind == "base"].iloc[0]
        params = {k[2:]: int(pick[k]) for k in pick.index if k.startswith("p_")}
        sel[f"split_{base}"] = {
            "chosen": {
                "name": pick["stem"], "params": params,
                "hard_anchor_correct_share_tuning": float(pick["hard_anchor_correct_share"]),
                "hard_noticed_share_tuning": float(pick["hard_noticed_share"]),
                "notices_on_background_per_stream_tuning": float(pick["notices_on_background_per_stream"]),
            },
            "base_tuning": {k: float(ctl[k]) for k in ("hard_anchor_correct_share", "hard_noticed_share",
                                                       "notices_on_background_per_stream")},
            "feasible": int(feasible), "of": len(grid), "tied_at_the_best": int(tied),
            "within_budget": bool(within),
            "rule": "highest anchor-correct share of hard non-leak incidents within the background budget; "
                    "ties: larger gap, larger burst (b3_common.py)",
        }
        print(base)
        print(df[COLS].to_string(index=False, float_format=lambda x: f"{x:.3f}"))
        print(json.dumps(sel[f"split_{base}"], indent=2))
    pd.concat(frames).to_csv(C.OUT / "b3-tuning-split.csv", index=False)
    save_sel(sel)


def delay():
    sel = load_sel()
    run = load_stream_run(C.RUNS / C.run_id("tunedelay"))
    m = B.Measures(run)
    pts = m.points()
    rows = []
    sel["delay"] = {}
    for stem, _ in M.table_rows(sel):
        mine = []
        for d in C.DELAYS_S:
            i = m.row[C.delay_arm_name(stem, d)]
            row = {"stem": stem, "delay_s": d}
            row.update({k: float(v[i]) for k, v in pts.items()})
            rows.append(row)
            mine.append(row)
        g = pd.DataFrame(mine)
        top = g[g["quality"] == g["quality"].max()].sort_values(["cost_s_per_stream", "delay_s"])
        pick = top.iloc[0]
        sel["delay"][stem] = {
            "delay_s": int(pick["delay_s"]), "quality_tuning": float(pick["quality"]),
            "cost_s_per_stream_tuning": float(pick["cost_s_per_stream"]),
            "tied_at_the_best": int(len(top)),
            "rule": "R5: the delay of the highest tuning quality, the cheapest of ties",
        }
    df = pd.DataFrame(rows)
    df.to_csv(C.OUT / "b3-tuning-delays.csv", index=False)
    save_sel(sel)
    print(json.dumps(sel["delay"], indent=2))


def main():
    stage = sys.argv[1] if len(sys.argv) > 1 else ""
    {"ramp": ramp, "split": split, "delay": delay}.get(stage, lambda: sys.exit(__doc__))()


if __name__ == "__main__":
    main()
