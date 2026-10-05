"""Print the R10 report's tables as Markdown, from the committed CSVs, so that no number in a table
is typed by hand.

Usage: r10_tables.py NAME     (see TABLES below)
Exploration (nothing here tests a hypothesis).
"""

import sys

import numpy as np
import pandas as pd

import r10_common as C

OUT = C.OUT
PRIMARY = C.setting_id(*C.PRIMARY)
SETTING_ORDER = [C.setting_id(b, r) for b, r in C.SETTINGS]


def f(x, nd=3):
    if x is None or (isinstance(x, float) and np.isnan(x)):
        return "-"
    return f"{x:.{nd}f}"


def sgn(x, nd=3):
    return "-" if x is None or (isinstance(x, float) and np.isnan(x)) else f"{x:+.{nd}f}"


def iv(x, lo, hi, nd=3):
    return f"{sgn(x, nd)} [{sgn(lo, nd)}, {sgn(hi, nd)}]"


def md(df):
    cols = list(df.columns)
    out = ["| " + " | ".join(cols) + " |", "|" + "|".join("---" for _ in cols) + "|"]
    for _, r in df.iterrows():
        out.append("| " + " | ".join(str(r[c]) for c in cols) + " |")
    return "\n".join(out)


def holds(x):
    return "holds" if bool(x) else "does not hold"


def criterion_primary():
    c = pd.read_csv(OUT / "r10-criterion.csv")
    c = c[(c.primary) & (c.pairing == C.PAIRINGS[0][0])]
    rows = []
    for _, r in c.sort_values("result").iterrows():
        rows.append({
            "result": f"{int(r.result)}: {r.what}",
            "notice": f"{int(r.treatment_correct)}/{int(r.incidents)} = {f(r.treatment_rate)}",
            "selection": f"{int(r.comparison_correct)}/{int(r.incidents)} = {f(r.comparison_rate)}",
            "difference [90% paired cluster bootstrap]": iv(r.difference, r.lo, r.hi),
            "needs": f"point >= {r.margin:g}, lower bound > {r.lower_threshold:g}",
            "verdict": holds(r.holds),
        })
    return md(pd.DataFrame(rows))


def criterion_all():
    c = pd.read_csv(OUT / "r10-criterion.csv")
    rows = []
    for pairing in [p[0] for p in C.PAIRINGS]:
        for sid in SETTING_ORDER:
            for result in (1, 2):
                g = c[(c.pairing == pairing) & (c.setting == sid) & (c.result == result)]
                if not len(g):
                    continue
                r = g.iloc[0]
                rows.append({
                    "pairing": pairing,
                    "setting (b, rho)": sid + (" (primary)" if sid == PRIMARY and pairing == C.PAIRINGS[0][0] else ""),
                    "result": f"{int(r.result)}: " + ("hard, leak excluded" if r.result == 1 else "slow leak"),
                    "notice": f"{f(r.treatment_rate)} ({int(r.treatment_correct)}/{int(r.incidents)})",
                    "selection": f"{f(r.comparison_rate)} ({int(r.comparison_correct)}/{int(r.incidents)})",
                    "difference [90%]": iv(r.difference, r.lo, r.hi),
                    "verdict": holds(r.holds),
                })
    return md(pd.DataFrame(rows))


def points(setting):
    p = pd.read_csv(OUT / "r10-points.csv")
    p = p[p.setting == setting]
    order = [C.SEL_RUNG, C.NOTICE_RUNG, C.SEL_WIN, C.NOTICE_WIN, C.ORACLE, C.NEVER]
    rows = []
    for a in order:
        r = p[p.arm == a].iloc[0]
        rows.append({
            "arm": f"`{a}`",
            "hard quality (excl. leak)": f(r.quality),
            "slow-leak quality": f(r.leak_quality),
            "plain accuracy": f(r.plain_acc),
            "critical misses (hard + plain)": f"{int(r.critical_misses)} ({int(r.critical_misses_hard)} + {int(r.critical_misses_plain)})",
            "calls / stream": f(r.calls_per_stream, 2),
            "refs / call": f(r.refs_per_call, 1),
            "cost s / stream": f(r.cost_s, 2),
            "false alarms / stream": f(r.false_alarms_per_stream, 2),
            "wrong declarations / stream": f(r.wrong_per_stream, 2),
            "calls refused": int(r.refused),
        })
    return md(pd.DataFrame(rows))


def points_primary():
    return points(PRIMARY)


def points_b25():
    return points("b2.5-rho0.7")


def points_b8():
    return points("b8-rho0.7")


def decomposition():
    d = pd.read_csv(OUT / "r10-decomposition.csv")
    rows = []
    for _, r in d.iterrows():
        rows.append({
            "incidents": r.incidents, "group": r.group.replace("_", " "),
            "n": f"{int(r.n_incidents)} of {int(r.of_total)}",
            "selection correct": int(r.selection_correct), "notice correct": int(r.notice_correct),
            "gain (incidents)": int(r.gain_incidents),
            "gain in quality [90%]": iv(r.gain_in_quality, r.lo, r.hi),
            "R4 oracle correct": int(r.oracle_correct),
        })
    return md(pd.DataFrame(rows))


def latency():
    d = pd.read_csv(OUT / "r10-notice-latency.csv")
    d = d[d.source == "diag"]
    rows = []
    for _, r in d.iterrows():
        rows.append({
            "family": r.family, "mode": r["mode"], "hard incidents": int(r.hard_incidents),
            "noticed": int(r.noticed), "never noticed": int(r.never_noticed),
            "latency p25 / median / p75 / max (s)": f"{f(r.latency_p25_s, 1)} / {f(r.latency_median_s, 1)} / {f(r.latency_p75_s, 1)} / {f(r.latency_max_s, 1)}",
            "median from onset (s)": f(r.latency_median_from_onset_s, 1),
            "anchored at the first observation": int(r.anchored_at_first_obs),
        })
    return md(pd.DataFrame(rows))


def latency_heldout():
    d = pd.read_csv(OUT / "r10-notice-latency.csv")
    d = d[d.source == "heldout"]
    rows = []
    for _, r in d.iterrows():
        rows.append({
            "family": r.family, "mode": r["mode"], "hard incidents": int(r.hard_incidents),
            "noticed": int(r.noticed), "never noticed": int(r.never_noticed),
            "latency p25 / median / p75 / max (s)": f"{f(r.latency_p25_s, 1)} / {f(r.latency_median_s, 1)} / {f(r.latency_p75_s, 1)} / {f(r.latency_max_s, 1)}",
            "median from onset (s)": f(r.latency_median_from_onset_s, 1),
        })
    return md(pd.DataFrame(rows))


def never_summary():
    d = pd.read_csv(OUT / "r10-never-noticed-summary.csv")
    p = d.pivot_table(index=["family", "mode"], columns="source", values="never_noticed", aggfunc="sum", fill_value=0).reset_index()
    p.columns.name = None
    return md(p)


def sweep():
    s = pd.read_csv(OUT / "r10-sweep.csv")
    rows = []
    for _, r in s.iterrows():
        rows.append({
            "notice z": f"{r.notice_z:g}" + (" (default)" if r.notice_z == C.DEFAULT_Z else ""),
            "hard incidents noticed (all / non-leak / leak)": f"{f(r.hard_noticed_share)} ({int(r.hard_noticed)}/{int(r.hard_incidents)}) / {f(r.nonleak_noticed_share)} ({int(r.nonleak_noticed)}/{int(r.nonleak_incidents)}) / {f(r.leak_noticed_share)} ({int(r.leak_noticed)}/{int(r.leak_incidents)})",
            "anomalies / stream": f(r.anomalies_per_stream, 2),
            "false notices / stream (background + plain)": f(r.false_notices_per_stream, 2),
            "on decoys / stream": f(r.on_decoy_per_stream, 2),
        })
    a = md(pd.DataFrame(rows))
    rows = []
    for _, r in s.iterrows():
        row = {
            "notice z": f"{r.notice_z:g}" + (" (default)" if r.notice_z == C.DEFAULT_Z else ""),
            "hard quality (excl. leak)": f(r.quality),
            "slow-leak quality": f(r.leak_quality),
            "plain accuracy": f(r.plain_acc),
            "critical misses": int(r.critical_misses),
            "calls / stream": f(r.calls_per_stream, 2),
            "refs / call": f(r.refs_per_call, 1),
            "cost s / stream": f(r.cost_s, 2),
            "false alarms / stream": f(r.false_alarms_per_stream, 2),
            "calls refused": int(r.refused),
        }
        if "quality_minus_default" in s.columns and not np.isnan(r.get("quality_minus_default", np.nan)):
            row["quality minus default [90%]"] = iv(r.quality_minus_default, r.quality_minus_default_lo, r.quality_minus_default_hi)
            row["leak minus default [90%]"] = iv(r.leak_minus_default, r.leak_minus_default_lo, r.leak_minus_default_hi)
        else:
            row["quality minus default [90%]"] = "-"
            row["leak minus default [90%]"] = "-"
        rows.append(row)
    return a + "\n\n" + md(pd.DataFrame(rows))


def regression():
    r = pd.read_csv(OUT / "r10-regression.csv")
    return (f"{len(r)} arms replayed; `results.csv` identical for {int(r.results_identical.sum())}, "
            f"`incidents.csv` identical for {int(r.incidents_identical.sum())}.")


def runs():
    ix = pd.read_csv(OUT / "r10-run-index.csv")
    log = pd.read_csv(OUT / "r10-driver-log.csv")
    log = log.drop_duplicates("run_id", keep="last").set_index("run_id")
    rows = []
    for _, r in ix.iterrows():
        key = r.run_id
        status = log.loc[key] if key in log.index else None
        rows.append({
            "run": f"`{r.run_id}`", "arms": int(r.arms), "streams": int(r.streams),
            "seeds": f"{int(r.seed_first)}-{int(r.seed_last)}", "b, rho": f"{r.b:g}, {r.rho:g}", "notice z": f"{r.notice_z:g}",
            "ledgers kept": f"{r.trace_sample_rate:g}", "driver exit": "-" if status is None else int(status.driver_exit),
            "wall s": f(r.wall_s, 0), "peak MB": f(r.peak_memory_mb, 0), "oom": int(r.oom_kills),
        })
    return md(pd.DataFrame(rows))


TABLES = {
    "criterion_primary": criterion_primary, "criterion_all": criterion_all,
    "points_primary": points_primary, "points_b25": points_b25, "points_b8": points_b8,
    "decomposition": decomposition, "latency": latency, "latency_heldout": latency_heldout,
    "never_summary": never_summary, "sweep": sweep, "regression": regression, "runs": runs,
}

if __name__ == "__main__":
    print(TABLES[sys.argv[1]]())
