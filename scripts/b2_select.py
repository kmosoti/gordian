"""Choose the parameters of `reanchor` and the hold delays from the tuning streams only.

Usage:
  b2_select.py stage1      after `b2-tune1-b5-rho0.7`: every configuration's tuning point to
                           experiments/exploration/b2-tuning-stage1.csv, the choice into b2-selected.json
  b2_select.py stage2      after `b2-tune2-b5-rho0.7`: b2-tuning-stage2.csv, the choice added
  b2_select.py stage3      after `b2-tunedelay-b5-rho0.7`: b2-tuning-delays.csv, one delay per table
                           noticer added

The rules are fixed in `b2_common.py` before the run each governs, and repeated here: stage 1 and 2
choose the configuration of the highest anchor-correct share of hard incidents outside the slow-leak
family among those within the background budget (BACKGROUND_BUDGET notices on background per stream),
ties toward the configuration that moves fewer anchors (`any` before `site`, larger gap, larger burst;
larger threshold); stage 3 is R5's rule for a delay: the highest quality, the cheapest of ties.
Nothing is chosen on any held-out stream.
"""

import json
import sys

import pandas as pd

import b2_common as C
import b2_stats as B
from gordian_analysis.load import load_stream_run


def load_run(stage):
    run = load_stream_run(C.RUNS / C.run_id(stage))
    seeds = [int(s) for s in next(iter(run.arms.values())).results["seed"]]
    assert seeds == list(range(C.TUNING_SEEDS[0], C.TUNING_SEEDS[0] + C.TUNING_SEEDS[1])), "tuning seeds"
    return run


def table(run, configs):
    m = B.Measures(run)
    pts = m.points()
    rows = []
    for name, kind, p in configs:
        arm = C.arm_name(name)
        i = m.row[arm]
        row = {"noticer": name, "kind": kind, **{f"p_{k}": v for k, v in p.items()}}
        row.update({k: float(v[i]) for k, v in pts.items()})
        rows.append(row)
    return pd.DataFrame(rows)


def load_sel():
    try:
        return C.load_selected()
    except FileNotFoundError:
        return {}


def save_sel(sel):
    with open(C.OUT / "b2-selected.json", "w") as fh:
        json.dump(sel, fh, indent=2)
        fh.write("\n")


def choose(df, tie_keys):
    """The best of `df` by the rule: within the budget; highest anchor-correct; ties by `tie_keys`
    (a list of (column, ascending))."""
    feasible = df[df["notices_on_background_per_stream"] <= C.BACKGROUND_BUDGET + 1e-12]
    if len(feasible) == 0:
        raise SystemExit("no configuration within the background budget")
    best = feasible[C.TUNE_OBJECTIVE].max()
    tied = feasible[feasible[C.TUNE_OBJECTIVE] == best]
    tied = tied.sort_values([k for k, _ in tie_keys], ascending=[a for _, a in tie_keys])
    return tied.iloc[0], len(tied), len(feasible)


def stage1():
    run = load_run("tune1")
    df = table(run, [c for c in C.stage1_grid()] + [("rung_z3", "rung", {}), ("rung_z2", "rung", {"z": 2.0})])
    df.to_csv(C.OUT / "b2-tuning-stage1.csv", index=False)
    grid = df[df.kind == "reanchor"].copy()
    grid["any_first"] = (grid["p_isolation"] != "any").astype(int)  # `any` (0) sorts before `site` (1)
    pick, ties, feasible = choose(grid, [("any_first", True), ("p_gap_ms", False), ("p_burst", False)])
    rung = df[df.noticer == "rung_z3"].iloc[0]
    sel = load_sel()
    sel["objective"] = C.TUNE_OBJECTIVE
    sel["budget_notices_on_background_per_stream"] = C.BACKGROUND_BUDGET
    sel["tuning_seeds"] = [C.TUNING_SEEDS[0], C.TUNING_SEEDS[0] + C.TUNING_SEEDS[1] - 1]
    sel["stage1"] = {
        "chosen": {
            "name": pick["noticer"], "isolation": pick["p_isolation"], "gap_ms": int(pick["p_gap_ms"]),
            "burst": int(pick["p_burst"]), "z": C.C1.DEFAULT_Z,
            "hard_anchor_correct_share_tuning": float(pick["hard_anchor_correct_share"]),
            "hard_anchor_site_correct_share_tuning": float(pick["hard_anchor_site_correct_share"]),
            "notices_on_background_per_stream_tuning": float(pick["notices_on_background_per_stream"]),
        },
        "rung_z3_tuning": {
            "hard_anchor_correct_share": float(rung["hard_anchor_correct_share"]),
            "hard_anchor_site_correct_share": float(rung["hard_anchor_site_correct_share"]),
            "notices_on_background_per_stream": float(rung["notices_on_background_per_stream"]),
        },
        "feasible": feasible, "tied_at_the_best": ties,
        "rule": "highest anchor-correct share within the background budget; ties: any before site, "
                "larger gap, larger burst",
    }
    save_sel(sel)
    cols = ["noticer", "hard_noticed_share", "hard_anchor_correct_share", "hard_site_correct_share",
            "hard_anchor_site_correct_share", "notices_on_background_per_stream", "notice_precision"]
    print(df[cols].to_string(index=False, float_format=lambda x: f"{x:.3f}"))
    print(json.dumps(sel["stage1"], indent=2))


def stage2():
    sel = load_sel()
    s1 = sel["stage1"]["chosen"]
    run = load_run("tune2")
    configs = [("rung_z3", "rung", {}), ("rung_z2", "rung", {"z": 2.0})]
    for z in C.REANCHOR_Z:
        configs.append((C.reanchor_name(s1["isolation"], s1["gap_ms"], s1["burst"], z), "reanchor",
                        {"isolation": s1["isolation"], "gap_ms": s1["gap_ms"], "burst": s1["burst"], "z": z}))
    df = table(run, configs)
    df.to_csv(C.OUT / "b2-tuning-stage2.csv", index=False)
    ladder = df[df.kind == "reanchor"].copy()
    pick, ties, feasible = choose(ladder, [("p_z", False)])
    sel["stage2"] = {
        "chosen": {
            "name": "reanchor", "isolation": s1["isolation"], "gap_ms": s1["gap_ms"], "burst": s1["burst"],
            "z": float(pick["p_z"]),
            "hard_anchor_correct_share_tuning": float(pick["hard_anchor_correct_share"]),
            "hard_anchor_site_correct_share_tuning": float(pick["hard_anchor_site_correct_share"]),
            "notices_on_background_per_stream_tuning": float(pick["notices_on_background_per_stream"]),
        },
        "feasible": feasible, "tied_at_the_best": ties,
        "rule": "the stage 1 choice at the threshold of the highest anchor-correct share within the "
                "background budget; ties: the larger threshold",
    }
    save_sel(sel)
    cols = ["noticer", "hard_noticed_share", "hard_anchor_correct_share", "hard_site_correct_share",
            "hard_anchor_site_correct_share", "notices_on_background_per_stream", "notice_precision"]
    print(df[cols].to_string(index=False, float_format=lambda x: f"{x:.3f}"))
    print(json.dumps(sel["stage2"], indent=2))


def stage3():
    sel = load_sel()
    run = load_stream_run(C.RUNS / C.run_id("tunedelay"))
    m = B.Measures(run)
    pts = m.points()
    rows = []
    sel["stage3"] = {}
    for name in [n for n in C.TABLE]:
        # the arm names carry the noticer's stem; `reanchor` is the stage 2 choice
        mine = []
        for d in C.DELAYS_S:
            arm = C.delay_arm_name(name_for_arm(name, sel), d)
            i = m.row[arm]
            row = {"noticer": name, "delay_s": d}
            row.update({k: float(v[i]) for k, v in pts.items()})
            rows.append(row)
            mine.append(row)
        g = pd.DataFrame(mine)
        top = g[g["quality"] == g["quality"].max()].sort_values(["cost_s_per_stream", "delay_s"])
        pick = top.iloc[0]
        sel["stage3"][name] = {
            "delay_s": int(pick["delay_s"]), "quality_tuning": float(pick["quality"]),
            "cost_s_per_stream_tuning": float(pick["cost_s_per_stream"]),
            "tied_at_the_best": int(len(top)),
            "rule": "R5: the delay of the highest tuning quality, the cheapest of ties",
        }
    df = pd.DataFrame(rows)
    df.to_csv(C.OUT / "b2-tuning-delays.csv", index=False)
    save_sel(sel)
    print(df[["noticer", "delay_s", "quality", "cost_s_per_stream", "calls_per_stream"]]
          .to_string(index=False, float_format=lambda x: f"{x:.3f}"))
    print(json.dumps(sel["stage3"], indent=2))


def name_for_arm(name, sel):
    """The arm-name stem of a table noticer in the tunedelay run (`reanchor` is the table's name)."""
    return name


def main():
    stage = sys.argv[1] if len(sys.argv) > 1 else ""
    {"stage1": stage1, "stage2": stage2, "stage3": stage3}.get(stage, lambda: sys.exit(__doc__))()


if __name__ == "__main__":
    main()
