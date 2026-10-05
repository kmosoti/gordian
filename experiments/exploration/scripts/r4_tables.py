"""Print the R4 report's tables as Markdown, from the committed CSVs, so that no number in the
report is typed by hand.

Usage: r4_tables.py NAME      NAME in: settings reach families tiers decomposition sizing frontier
                              provenance drift runs
Exploration (nothing here tests a hypothesis).
"""

import sys

import pandas as pd

import r4_common as C

OUT = C.OUT


def f(x, nd=3):
    if x is None or (isinstance(x, float) and pd.isna(x)):
        return "-"
    if isinstance(x, float) and x == float("inf"):
        return "never"
    return f"{x:.{nd}f}"


def iv(x, lo, hi, nd=3):
    return f"{f(x, nd)} [{f(lo, nd)}, {f(hi, nd)}]"


def md(df):
    cols = list(df.columns)
    out = ["| " + " | ".join(cols) + " |", "|" + "|".join("---" for _ in cols) + "|"]
    for _, r in df.iterrows():
        out.append("| " + " | ".join(str(r[c]) for c in cols) + " |")
    return "\n".join(out)


def settings():
    s = pd.read_csv(OUT / "r4-settings.csv")
    rows = []
    for _, r in s.iterrows():
        rows.append(
            {
                "setting (b, rho)": r.setting,
                "oracle quality [90%]": iv(r.oracle_quality, r.oracle_quality_lo, r.oracle_quality_hi),
                "oracle cost s/stream [90%]": iv(r.oracle_cost_s, r.oracle_cost_lo, r.oracle_cost_hi),
                "best baseline at cost <= oracle's": f"`{r.best_arm}`",
                "its quality [90%]": iv(r.best_quality, r.best_quality_lo, r.best_quality_hi),
                "its cost s": f(r.best_cost_s),
                "gap [90%]": iv(r.gap, r.gap_lo, r.gap_hi),
                "verdict (margin 0.10, lower bound 0.05)": r.verdict,
            }
        )
    return md(pd.DataFrame(rows))


def robustness():
    s = pd.read_csv(OUT / "r4-settings.csv")
    rows = []
    for _, r in s.iterrows():
        rows.append(
            {
                "setting": r.setting,
                "config chosen on tuning streams": f"`{r.tuned_arm}`",
                "its held-out quality": f(r.tuned_quality),
                "its held-out cost s": f(r.tuned_cost_s),
                "gap to oracle [90%]": iv(r.tuned_gap, r.tuned_gap_lo, r.tuned_gap_hi),
                "oracle cost on tuning streams s": f(r.oracle_tuning_cost_s),
            }
        )
    return md(pd.DataFrame(rows))


def reach():
    s = pd.read_csv(OUT / "r4-settings.csv")
    rows = []
    for _, r in s.iterrows():
        rows.append(
            {
                "setting": r.setting,
                "oracle quality": f(r.oracle_quality),
                "highest-quality baseline (any cost)": f"`{r.highest_quality_arm}`",
                "its quality": f(r.highest_quality),
                "its cost s": f(r.highest_quality_cost_s),
                "its cost / oracle's": f(r.highest_quality_cost_s / r.oracle_cost_s, 1) + "x",
                "baseline reaching oracle quality": (r.reach_arm if isinstance(r.reach_arm, str) else "none"),
                "resamples where one does": f(r.reach_share, 3),
                "ablation quality (cost 0)": f(r.ablation_quality),
            }
        )
    return md(pd.DataFrame(rows))


def families():
    d = pd.read_csv(OUT / "r4-families.csv")
    rows = []
    for _, r in d.iterrows():
        row = {"setting": r.setting, "arm (role)": f"`{r.arm}` ({r.role_label})"}
        for fam in ("compound", "cascade", "split_brain", "slow_leak"):
            row[fam] = f"{int(r[fam + '_correct'])}/{int(r[fam + '_n'])} = {f(r[fam + '_rate'], 2)}"
        rows.append(row)
    return md(pd.DataFrame(rows))


def tiers():
    d = pd.read_csv(OUT / "r4-tiers.csv")
    rows = []
    for _, r in d.iterrows():
        rows.append(
            {
                "setting": r.setting,
                "arm (role)": f"`{r.arm}` ({r.role_label})",
                "cost s": f(r.cost_s, 2),
                "calls/stream": f(r.calls_per_stream, 1),
                "hard excl. leak": f(r.quality_hard_excl_leak, 3),
                "hard incl. leak": f(r.hard_incl_leak, 3),
                "plain acc.": f(r.plain_accuracy, 3),
                "crit. misses (rate)": f"{int(r.critical_misses_total)} ({f(r.critical_miss_rate, 2)})",
                "decoys alarmed / dismissed / silent (of %d)" % int(r.decoys): f"{int(r.decoys_alarmed)} / {int(r.decoys_dismissed)} / {int(r.decoys_silent)}",
                "false alarms/stream": f(r.false_alarms_per_stream, 2),
                "wrong decl./stream": f(r.wrong_declarations_per_stream, 2),
            }
        )
    return md(pd.DataFrame(rows))


def decomposition():
    d = pd.read_csv(OUT / "r4-decomposition.csv")
    rows = []
    for sid, g in d.groupby("setting", sort=False):
        top = g.sort_values(["quality", "hard_only_cost_s_est"], ascending=[False, True]).iloc[0]
        rows.append(
            {
                "setting": sid,
                "oracle quality / cost s": f"{f(top.oracle_quality)} / {f(top.oracle_cost_s)}",
                "best always arm (delay)": f"`{top.arm}`",
                "its quality": f(top.quality),
                "hard incidents it escalated / informed / got right": f"{f(top.hard_escalated_share, 2)} / {f(top.hard_informed_share, 2)} / {f(top.hard_correct_share, 2)}",
                "its own cost s (all calls)": f(top.arm_cost_s, 2),
                "same quality with hard-only calls: est. cost s [0.5x, 2x tokens]": f"{f(top.hard_only_cost_s_est, 2)} [{f(top.hard_only_cost_s_lo, 2)}, {f(top.hard_only_cost_s_hi, 2)}]",
            }
        )
    return md(pd.DataFrame(rows))


def sizing():
    d = pd.read_csv(OUT / "r4-sizing.csv")
    d = d.assign(
        a=d.a.map(lambda x: f"`{x}`"), b=d.b.map(lambda x: f"`{x}`"),
        diff=d["diff"].map(lambda x: f(x)), se_at_200_streams=d.se_at_200_streams.map(lambda x: f(x, 4)),
    )
    return md(d)


def frontier():
    d = pd.read_csv(OUT / "r4-heldout-points.csv")
    out = []
    for sid, g in d.groupby("setting", sort=False):
        g = g[g.on_heldout_frontier].sort_values("cost_s")
        pts = "; ".join(f"`{r.arm}` ({f(r.quality, 2)} @ {f(r.cost_s, 2)} s)" for _, r in g.iterrows())
        out.append(f"- **{sid}** ({len(g)} points): {pts}")
    return "\n".join(out)


def provenance():
    p = pd.read_csv(OUT / "r4-run-index.csv")
    rows = []
    for _, r in p.iterrows():
        rows.append(
            {
                "run id": f"`{r.run_id}`",
                "revision": f"`{r.source_revision[:7]}`",
                "arms x streams": f"{r.arms} x {r.streams}",
                "manifest.json sha256": f"`{r.manifest_sha256}`",
                "wall s": f(r.wall_s, 0),
            }
        )
    return md(pd.DataFrame(rows))


def drift():
    p = pd.read_csv(OUT / "r4-provenance.csv")
    rows = []
    for _, r in p.iterrows():
        rows.append(
            {
                "run": f"`{r.run_id}`",
                "wall s": f(r.wall_ns / 1e9, 0),
                "cpu s": f(r.cpu_ns / 1e9, 0),
                "peak mem MB": f(r.peak_memory_bytes / 2**20, 0),
                "OOM kills": int(r.oom_kills),
                "internal/external": f(r.internal_external_ratio, 3),
                "drift blocks": int(r.drift_blocks),
                "CV (block ns)": f(r.drift_cv_ns, 3),
                "last/first": f(r.drift_last_over_first_ns, 2),
                "CV (min ns)": f(r.drift_cv_min_ns, 3),
                "last/first (min)": f(r.drift_last_over_first_min_ns, 2),
            }
        )
    return md(pd.DataFrame(rows))


def runs():
    d = pd.read_csv(OUT / "r4-driver-log.csv")
    d["run_id"] = d.run_id.map(lambda x: f"`{x}`")
    return md(d)


if __name__ == "__main__":
    name = sys.argv[1]
    fn = {
        "settings": settings, "robustness": robustness, "reach": reach, "families": families,
        "tiers": tiers, "decomposition": decomposition, "sizing": sizing, "frontier": frontier,
        "provenance": provenance, "drift": drift, "runs": runs,
    }[name]
    print(fn())
