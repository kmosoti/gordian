#!/usr/bin/env python3
"""W1: print the report's tables as Markdown from the `w1-*.csv` files (no new numbers).

Usage: w1_tables.py [--dir DIR]. DIR defaults to `experiments/exploration/` (this file's parent
directory's parent). Every figure printed is a column of, or a sum or ratio of columns of, a
committed CSV; the ticks CSVs come from `examples/ticks`, the others from `w1_measures.py`.
"""

from __future__ import annotations

import argparse
from pathlib import Path

import pandas as pd

FAMILY_ORDER = ["Compound", "Cascade", "SplitBrain", "SlowLeak"]
MODE_ORDER = ["mimic", "contradict", "none"]


def md(df: pd.DataFrame) -> str:
    cols = list(df.columns)
    lines = ["| " + " | ".join(cols) + " |", "|" + "|".join("---" for _ in cols) + "|"]
    for row in df.itertuples(index=False):
        lines.append("| " + " | ".join(str(v) for v in row) + " |")
    return "\n".join(lines)


def order_key(df: pd.DataFrame) -> pd.DataFrame:
    df = df.copy()
    df["_f"] = df["family"].map(FAMILY_ORDER.index)
    df["_m"] = df["mode"].map(MODE_ORDER.index)
    return df.sort_values(["_f", "_m", "tick_ms"]).drop(columns=["_f", "_m"])


def tick_tables(d: Path) -> None:
    s = pd.read_csv(d / "w1-ticks-summary.csv")
    h = pd.read_csv(d / "w1-ticks-hist.csv")

    print("### Events per tick (200 streams, 600 s each)\n")
    rows = []
    for tick in (100, 500, 2000):
        for name, label in [
            ("tick_all", "all events per tick"),
            ("tick_abnormal", "abnormal events per tick"),
            ("node_tick_all", "all events per node per tick"),
            ("node_tick_abnormal", "abnormal events per node per tick"),
        ]:
            r = s[(s.tick_ms == tick) & (s.measure == name)].iloc[0]
            hh = h[(h.tick_ms == tick) & (h.measure == name)]
            at_least_2 = hh[hh.events >= 2].frequency.sum() / r.cells
            rows.append(
                {
                    "tick": f"{tick} ms",
                    "measure": label,
                    "mean": f"{r['mean']:.3f}",
                    "var/mean": f"{r.dispersion:.2f}",
                    "share empty": f"{r.share_empty:.3f}",
                    "share >= 2": f"{at_least_2:.3f}",
                    "p50": int(r.p50),
                    "p90": int(r.p90),
                    "p99": int(r.p99),
                    "max": int(r["max"]),
                }
            )
    print(md(pd.DataFrame(rows)))

    lags = pd.read_csv(d / "w1-ticks-lags.csv")

    def lag_table(measure: str, unit: str = "ticks") -> pd.DataFrame:
        t = lags[(lags.measure == measure) & (lags.unit == unit)]
        t = order_key(t)
        out = t[
            ["family", "mode", "tick_ms", "n", "missing", "mean", "p50", "p90", "max", "share_zero"]
        ].copy()
        out["mean"] = out["mean"].map(lambda x: f"{x:.2f}")
        out["share_zero"] = out["share_zero"].map(lambda x: f"{x:.3f}")
        out.columns = [
            "family", "mode", "tick ms", "n", "missing", "mean", "p50", "p90", "max", "share in the same tick"
        ]  # fmt: skip
        if unit == "ms":
            out = out.drop(columns=["tick ms", "share in the same tick"])
        return out

    print("\n### First observation to first abnormal observation (ticks)\n")
    print(md(lag_table("first_obs_to_first_abnormal")))
    print("\n### First observation to the partner's first alarm (ticks)\n")
    print(md(lag_table("first_obs_to_partner_alarm")))
    print("\n### Same, in milliseconds (no grid)\n")
    print(md(lag_table("first_obs_to_partner_alarm", "ms")))
    print("\n### First abnormal observation to the partner's first alarm (ticks)\n")
    print(md(lag_table("first_abnormal_to_partner_alarm")))
    print("\n### First observation to first decisive observation (ticks)\n")
    print(md(lag_table("first_obs_to_first_decisive")))
    print("\n### Same, in milliseconds\n")
    print(md(lag_table("first_obs_to_first_decisive", "ms")))

    dec = order_key(pd.read_csv(d / "w1-ticks-decisive.csv"))
    print("\n### Decisive evidence in the same tick as the first observation\n")
    print(md(dec))
    bur = order_key(pd.read_csv(d / "w1-ticks-burst.csv"))
    print("\n### The first-moments burst on the grid\n")
    print(md(bur))


def measure_tables(d: Path) -> None:
    se = pd.read_csv(d / "w1-sample-efficiency.csv")
    ep = pd.read_csv(d / "w1-energy-proxy.csv")
    for run in ("r10", "r9"):
        print(f"\n### Sample efficiency, {run} held-out (hard incidents; 90% stream bootstrap)\n")
        t = se[(se.run == run) & (se.tier == "hard")]
        out = pd.DataFrame(
            {
                "arm": t.arm,
                "role": t.role,
                "seen": t.incidents_seen,
                "correct": t.correct,
                "efficiency": t.efficiency.map("{:.3f}".format),
                "90% interval": [f"[{a:.3f}, {b:.3f}]" for a, b in zip(t.ci90_lo, t.ci90_hi)],
                "first half": t.first_half.map("{:.3f}".format),
                "second half": t.second_half.map("{:.3f}".format),
                "after 50": t.after_50.map("{:.3f}".format),
                "after 100": t.after_100.map("{:.3f}".format),
            }
        )
        print(md(out))
        print(f"\n### Energy proxy, {run} held-out (cost per correct decision)\n")
        e = ep[ep.run == run]
        out = pd.DataFrame(
            {
                "arm": e.arm,
                "correct (plain+hard)": e.correct_all,
                "ns total": e.ns_total,
                "ns reasoner": e.ns_reasoner,
                "ns per correct (all)": e.ns_per_correct_all.map("{:.0f}".format),
                "ns per correct hard": e.ns_per_correct_hard.map("{:.0f}".format),
                "J per correct (all), PLACEHOLDER": e.joules_per_correct_all_PLACEHOLDER.map(
                    lambda x: f"{x:.4g}"
                ),
            }
        )
        print(md(out))


def rank_tables(d: Path) -> None:
    """Rank every arm of each run by cost and by the two aim proxies (1 is best)."""
    se = pd.read_csv(d / "w1-sample-efficiency.csv")
    se = se[se.tier == "hard"].set_index(["run", "arm"])
    ep = pd.read_csv(d / "w1-energy-proxy.csv").set_index(["run", "arm"])
    for run in ("r10", "r9"):
        e = ep.loc[run].copy()
        e["efficiency"] = se.loc[run].efficiency
        e["cost_per_stream_s"] = e.ns_total / e.streams / 1e9
        e["reasoner_share"] = e.ns_reasoner / e.ns_total
        r = pd.DataFrame(index=e.index)
        r["cost/stream (s)"] = e.cost_per_stream_s.map("{:.2f}".format)
        r["rank by cost"] = e.cost_per_stream_s.rank(method="min").astype(int)
        r["efficiency"] = e.efficiency.map("{:.3f}".format)
        r["rank by efficiency"] = (-e.efficiency).rank(method="min").astype(int)
        r["rank by ns/correct (all)"] = e.ns_per_correct_all.rank(method="min").astype(int)
        r["rank by ns/correct hard"] = e.ns_per_correct_hard.rank(method="min", na_option="bottom").astype(int)
        r["rank by J/correct (PLACEHOLDER)"] = (
            e.joules_per_correct_all_PLACEHOLDER.astype(float).rank(method="min").astype(int)
        )
        r["reasoner share of cost"] = e.reasoner_share.map("{:.4f}".format)
        print(f"\n### Ranks, {run} (1 is best; ties share the lower rank)\n")
        print(md(r.reset_index()))
        rho = e[
            ["cost_per_stream_s", "efficiency", "ns_per_correct_all", "ns_per_correct_hard"]
        ].corr(method="spearman")
        print("\nSpearman rank correlations (n = %d arms; an arm with no correct hard decision has no ns/correct hard and is ranked last by it):\n" % len(e))
        print(md(rho.round(2).reset_index().rename(columns={"index": ""})))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", type=Path, default=Path(__file__).resolve().parent.parent)
    ap.add_argument("--only", choices=["ticks", "measures", "ranks"])
    a = ap.parse_args()
    if a.only in (None, "ticks"):
        tick_tables(a.dir)
    if a.only in (None, "measures"):
        measure_tables(a.dir)
    if a.only in (None, "ranks"):
        rank_tables(a.dir)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
