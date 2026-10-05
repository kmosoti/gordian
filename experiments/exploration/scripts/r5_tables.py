"""Print the R5 report's tables as Markdown, from the committed CSVs, so that no number in a table
is typed by hand.

Usage: r5_tables.py NAME      NAME in: criterion criterion_sens decomposition decomposition_diffs
                              families decoys tiers trace trace_calls sensitivity robustness tuning
                              provenance drift runs regression
Exploration (nothing here tests a hypothesis).
"""

import sys

import pandas as pd

import r5_common as C

OUT = C.OUT


def f(x, nd=3):
    if x is None or (isinstance(x, float) and pd.isna(x)):
        return "-"
    if isinstance(x, float) and x == float("inf"):
        return "unreachable"
    return f"{x:.{nd}f}"


def iv(x, lo, hi, nd=3):
    return f"{f(x, nd)} [{f(lo, nd)}, {f(hi, nd)}]"


def md(df):
    cols = list(df.columns)
    out = ["| " + " | ".join(cols) + " |", "|" + "|".join("---" for _ in cols) + "|"]
    for _, r in df.iterrows():
        out.append("| " + " | ".join(str(r[c]) for c in cols) + " |")
    return "\n".join(out)


def yes(x):
    return "holds" if bool(x) else "does not hold"


def criterion(only=None):
    s = pd.read_csv(OUT / "r5-criterion.csv")
    if only is not None:
        s = s[s.setting.isin(only)]
    rows = []
    for _, r in s.iterrows():
        rows.append(
            {
                "setting (b, rho)": r.setting,
                "selection oracle: quality [90%] @ cost s": f"{iv(r.selection_quality, r.selection_quality_lo, r.selection_quality_hi)} @ {f(r.selection_cost_s, 2)}",
                "1: best public at cost <= its cost": f"`{r.c1_best_public_arm}` {f(r.c1_best_public_quality)} @ {f(r.c1_best_public_cost_s, 2)}",
                "1: gap [90%]": iv(r.c1_gap, r.c1_gap_lo, r.c1_gap_hi),
                "1 (gap >= 0.10, lower bound > 0.05)": yes(r.c1_holds),
                "2: cheapest public within 0.05 of its quality": f"`{r.c2_cheapest_within_arm}`" if isinstance(r.c2_cheapest_within_arm, str) else "none",
                "2: cost ratio [90%]": iv(r.c2_ratio, r.c2_ratio_lo, r.c2_ratio_hi, 2),
                "2 (ratio >= 1.5, lower bound > 1.25)": yes(r.c2_holds),
            }
        )
    return md(pd.DataFrame(rows))


def criterion_sens():
    return criterion()


def decomposition():
    d = pd.read_csv(OUT / "r5-decomposition.csv")
    d = d[~d.step.str.startswith("difference")]
    rows = []
    for _, r in d.iterrows():
        rows.append(
            {
                "setting": r.setting,
                "step": r.step,
                "arm": f"`{r.arm}`",
                "hard quality (excl. leak)": f(r.quality_excl_leak),
                "cost s/stream": f(r.cost_s, 2),
                "calls/stream": f(r.calls_per_stream, 1),
            }
        )
    return md(pd.DataFrame(rows))


def decomposition_diffs():
    d = pd.read_csv(OUT / "r5-decomposition.csv")
    d = d[d.step.str.startswith("difference")]
    rows = []
    for _, r in d.iterrows():
        rows.append(
            {
                "setting": r.setting,
                "difference": r.step.replace("difference: ", ""),
                "quality difference [90%]": iv(r.quality_excl_leak, r.quality_lo, r.quality_hi),
            }
        )
    return md(pd.DataFrame(rows))


def families():
    d = pd.read_csv(OUT / "r5-decomposition.csv")
    d = d[~d.step.str.startswith("difference")]
    rows = []
    for _, r in d.iterrows():
        row = {"setting": r.setting, "step": r.step, "arm": f"`{r.arm}`"}
        for fam in ("compound", "cascade", "split_brain", "slow_leak"):
            row[fam] = f"{int(r[fam + '_correct'])}/{int(r[fam + '_n'])} = {f(r[fam + '_rate'], 2)}"
        rows.append(row)
    return md(pd.DataFrame(rows))


def decoys():
    d = pd.read_csv(OUT / "r5-decoys.csv")
    rows = []
    for _, r in d.iterrows():
        rows.append(
            {
                "setting": r.setting,
                "arm (role)": f"`{r.arm}` ({r.label})",
                "cost s": f(r.cost_s, 2),
                "decoys dismissed / alarmed / silent (of %d)" % int(r.decoys): f"{int(r.dismissed)} / {int(r.alarmed)} / {int(r.silent)}",
                "false alarms/stream": f(r.false_alarms_per_stream, 2),
                "minus never's [90%]": iv(r.false_alarms_minus_never, r.fa_minus_never_lo, r.fa_minus_never_hi, 2),
                "wrong decl./stream": f(r.wrong_declarations_per_stream, 2),
                "critical misses (rate)": f"{int(r.critical_misses)} ({f(r.critical_miss_rate, 3)})",
                "plain acc.": f(r.plain_accuracy, 3),
            }
        )
    return md(pd.DataFrame(rows))


def tiers():
    d = pd.read_csv(OUT / "r5-tiers.csv")
    rows = []
    for _, r in d.iterrows():
        rows.append(
            {
                "setting": r.setting,
                "arm (role)": f"`{r.arm}` ({r.label})",
                "cost s": f(r.cost_s, 2),
                "calls/stream": f(r.calls_per_stream, 1),
                "hard excl. leak": f(r.quality_excl_leak),
                "plain acc.": f(r.plain_accuracy),
                "crit. misses (rate)": f"{int(r.critical_misses)} ({f(r.critical_miss_rate, 2)})",
                "false alarms/stream": f(r.false_alarms_per_stream, 2),
                "wrong decl./stream": f(r.wrong_declarations_per_stream, 2),
                "hard incidents escalated": f(r.hard_escalated_share, 2),
            }
        )
    return md(pd.DataFrame(rows))


def trace():
    d = pd.read_csv(OUT / "r5-trace.csv")
    rows = []
    for _, r in d.iterrows():
        rows.append(
            {
                "arm": f"`{r.arm}`", "family": r.family,
                "calls about hard": int(r.calls_about_hard),
                "before any evidence": int(r.before_evidence),
                "complete": int(r.complete),
                "lacking": int(r.lacking),
                "missing observations": int(r.missing_obs),
                "service not attached": int(r.obs_service_not_attached),
                "outside window": int(r.obs_outside_window),
                "context cap": int(r.obs_context_cap),
                "other": int(r.obs_other),
            }
        )
    return md(pd.DataFrame(rows))


def sensitivity():
    d = pd.read_csv(OUT / "r5-selection-delay-sensitivity.csv")
    rows = []
    for _, r in d.iterrows():
        rows.append(
            {
                "setting": r.setting,
                "delay s": f(r.delay_s, 0) + (" (chosen on tuning)" if r.chosen_on_tuning_streams else ""),
                "quality": f(r.quality),
                "cost s": f(r.cost_s, 2),
                "compound": f(r.compound_rate, 2), "cascade": f(r.cascade_rate, 2),
                "split brain": f(r.split_brain_rate, 2), "slow leak": f(r.slow_leak_rate, 2),
            }
        )
    return md(pd.DataFrame(rows))


def robustness():
    s = pd.read_csv(OUT / "r5-criterion.csv")
    rows = []
    for _, r in s.iterrows():
        rows.append(
            {
                "setting": r.setting,
                "config chosen on tuning streams (best at tuning cost <= selection oracle's)": f"`{r.tuned_arm}`",
                "its held-out quality @ cost s": f"{f(r.tuned_quality)} @ {f(r.tuned_cost_s, 2)}",
                "gap to selection oracle [90%]": iv(r.tuned_gap, r.tuned_gap_lo, r.tuned_gap_hi),
            }
        )
    return md(pd.DataFrame(rows))


def tuning():
    d = pd.read_csv(OUT / "r5-tuning-points.csv")
    rows = []
    for sid, g in d.groupby("setting", sort=False):
        con = g[g.policy == C.CONTRA_POLICY]
        sel = g[g.policy == C.SEL_POLICY]
        fr = con[con.frontier_in_family].sort_values("cost_s")
        top = fr.sort_values(["quality", "cost_s"], ascending=[False, True]).iloc[0]
        ch = sel[sel.chosen].iloc[0]
        rows.append(
            {
                "setting": sid,
                "contradiction: frontier size of 65": len(fr),
                "contradiction: best tuning quality (config) @ cost s": f"{f(top.quality)} (`{top.arm}`) @ {f(top.cost_s, 2)}",
                "selection oracle: chosen delay s": f(ch.delay_s, 0),
                "its tuning quality @ cost s": f"{f(ch.quality)} @ {f(ch.cost_s, 2)}",
            }
        )
    return md(pd.DataFrame(rows))


def provenance():
    p = pd.read_csv(OUT / "r5-run-index.csv")
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
    p = pd.read_csv(OUT / "r5-provenance.csv")
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
    d = pd.read_csv(OUT / "r5-driver-log.csv")
    d["run_id"] = d.run_id.map(lambda x: f"`{x}`")
    return md(d)


def regression():
    d = pd.read_csv(OUT / "r5-regression.csv")
    return md(d)


if __name__ == "__main__":
    name = sys.argv[1]
    fn = {
        "criterion": criterion, "criterion_sens": criterion_sens, "decomposition": decomposition,
        "decomposition_diffs": decomposition_diffs, "families": families, "decoys": decoys,
        "tiers": tiers, "trace": trace, "sensitivity": sensitivity, "robustness": robustness,
        "tuning": tuning, "provenance": provenance, "drift": drift, "runs": runs,
        "regression": regression,
    }[name]
    print(fn())
