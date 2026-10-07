"""L2's tuning selection: the rule of `l2_common` ("the tuning rule"), applied to the tuning runs.

Usage: l2_tune.py [--write]

Reads artifacts/runs/l2-tune-bNN-b5-rho0.7 for every batch of the grid (all must be present and
complete), measures each configuration over the tuning window (streams 51-100, seeds 10050-10099)
and over all 100 tuning streams, applies the rule, prints the table and, with --write, writes
experiments/exploration/l2-tuning-points.csv (every configuration, every measure) and
experiments/exploration/l2-selected.json (the chosen configuration, the set it was chosen from).
The rule is `l2_common`'s and was fixed before the tuning run; this script decides nothing the
rule does not.
"""

import json
import sys

import pandas as pd

import b2_stats as B
import l1_stats as L
import l2_common as C
import l2_manifests as MF
from gordian_analysis.load import load_stream_run


def measure(m, arm, lo, hi):
    w = L.Window(m, lo, hi)
    return {
        "ac": w.ratio("hard_correct", "hard_n", arm),
        "ln": w.ratio("leak_noticed", "leak_n", arm),
        "bg": w.mean("notices_background", arm),
        "sp": w.ratio("notices_both", "notices", arm),
        "notices": w.mean("notices", arm),
        "hard_n": float(w.col("hard_n", arm).sum()),
    }


def points():
    rows = []
    for k in range(MF.batches()):
        rid = C.run_id(f"tune-b{k:02d}")
        run = load_stream_run(C.RUNS / rid)
        m = B.Measures(run)
        seeds = [int(s) for s in m.seeds]
        assert seeds == list(range(C.TUNE_SEEDS[0], C.TUNE_SEEDS[0] + C.TUNE_SEEDS[1])), "seeds"
        lo, hi = C.TUNE_WINDOW[0] - 1, C.TUNE_WINDOW[1]
        for (index, size, leak, radius, threshold) in C.grid()[k * MF.BATCH:(k + 1) * MF.BATCH]:
            arm = C.arm_name(C.grid_name(index))
            row = {"index": index, "name": C.grid_name(index), "size": size, "leak": leak,
                   "spectral_radius": radius, "threshold": threshold}
            for key, v in measure(m, arm, lo, hi).items():
                row["window_" + key] = v
            for key, v in measure(m, arm, 0, m.n).items():
                row["all_" + key] = v
            rows.append(row)
    return pd.DataFrame(rows)


def select(df):
    """The rule: (chosen row, which set it came from, the size of that set)."""
    feas = df[(df.window_bg <= C.TUNE_BG) & (df.window_sp >= C.TUNE_SP)]
    which = "margin"
    if feas.empty:
        feas = df[(df.window_bg <= C.BACKGROUND_BOUND) & (df.window_sp >= C.STRICT_BOUND)]
        which = "bounds-as-written"
    if feas.empty:
        worst = pd.concat([df.window_bg / C.BACKGROUND_BOUND, C.STRICT_BOUND / df.window_sp], axis=1).max(axis=1)
        feas = df.assign(_worst=worst)
        feas = feas[feas._worst == feas._worst.min()]
        which = "least-violation"
    order = feas.sort_values(["window_ac", "window_ln", "window_sp", "size", "index"],
                             ascending=[False, False, False, True, True], kind="stable")
    return order.iloc[0], which, len(feas), order


def selftest():
    """The rule on hand-made rows (run with --selftest): the margin set, the bounds-as-written
    set, the least-violation set, and every tie-break, in the order the rule gives."""

    def rows(*specs):
        return pd.DataFrame([
            {"index": i, "size": s, "window_ac": ac, "window_ln": ln, "window_bg": bg, "window_sp": sp}
            for i, (s, ac, ln, bg, sp) in enumerate(specs)])

    # 1. the margin set wins even if a row outside it has a higher AC
    df = rows((32, 0.90, 1.0, 5.0, 0.72), (32, 0.99, 1.0, 6.5, 0.72), (32, 0.95, 1.0, 5.5, 0.71))
    c, which, n, _ = select(df)
    assert (int(c["index"]), which, n) == (2, "margin", 2), (c, which, n)
    # 2. nothing in the margin set: the bounds as written
    df = rows((32, 0.99, 1.0, 6.5, 0.72), (32, 0.95, 1.0, 6.9, 0.80), (32, 0.97, 1.0, 6.8, 0.68))
    c, which, n, _ = select(df)
    assert (int(c["index"]), which, n) == (0, "bounds-as-written", 2), (c, which, n)
    # 3. nothing meets either: the least worst bound ratio
    df = rows((32, 0.99, 1.0, 9.0, 0.50), (32, 0.90, 1.0, 7.5, 0.60), (32, 0.97, 1.0, 20.0, 0.70))
    c, which, n, _ = select(df)
    assert (int(c["index"]), which, n) == (1, "least-violation", 1), (c, which, n)
    # 4. ties on AC go to LN, then SP (background is never a tie-break), then the smaller size,
    #    then the earlier index
    df = rows((32, 0.95, 0.90, 1.0, 0.90), (32, 0.95, 0.95, 5.9, 0.71), (32, 0.95, 0.95, 4.0, 0.80),
              (16, 0.95, 0.95, 4.0, 0.80), (16, 0.95, 0.95, 4.0, 0.80))
    c, which, n, _ = select(df)
    assert int(c["index"]) == 3, c
    print("selftest ok")


def main():
    if "--selftest" in sys.argv[1:]:
        selftest()
        return
    df = points()
    chosen, which, n, order = select(df)
    with pd.option_context("display.width", 250, "display.max_columns", 40, "display.max_rows", 200):
        show = ["index", "size", "leak", "spectral_radius", "threshold", "window_ac", "window_ln",
                "window_bg", "window_sp", "all_bg", "all_sp"]
        print(df[show].to_string(index=False, float_format=lambda x: f"{x:.3f}"))
        print(f"\nfeasible set ({which}): {n} of {len(df)}")
        print("chosen:")
        print(chosen[show].to_string(float_format=lambda x: f"{x:.3f}"))
    if "--write" in sys.argv[1:]:
        df.to_csv(C.OUT / "l2-tuning-points.csv", index=False)
        sel = {
            "rule": "scripts/l2_common.py, the tuning rule",
            "window": list(C.TUNE_WINDOW),
            "margin": {"background": C.TUNE_BG, "strict_precision": C.TUNE_SP},
            "set_used": which,
            "set_size": int(n),
            "grid_size": int(len(df)),
            "chosen": {
                "index": int(chosen["index"]),
                "name": chosen["name"],
                "params": {"size": int(chosen["size"]), "leak": float(chosen["leak"]),
                           "spectral_radius": float(chosen["spectral_radius"]),
                           "threshold": float(chosen["threshold"])},
                "window": {k[len("window_"):]: float(chosen[k]) for k in chosen.index if k.startswith("window_")},
                "all_streams": {k[len("all_"):]: float(chosen[k]) for k in chosen.index if k.startswith("all_")},
            },
            "ties_at_the_top_ac": int((order.window_ac == order.iloc[0].window_ac).sum()),
        }
        with open(C.OUT / "l2-selected.json", "w") as fh:
            json.dump(sel, fh, indent=2)
            fh.write("\n")
        print("wrote l2-tuning-points.csv and l2-selected.json")


if __name__ == "__main__":
    main()
