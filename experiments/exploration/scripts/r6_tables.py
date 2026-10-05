"""Print the R6 report's tables as Markdown, from the committed CSVs, so that no number in a table
is typed by hand.

Usage: r6_tables.py NAME     (see TABLES below)
Exploration (nothing here tests a hypothesis).
"""

import json
import sys

import numpy as np
import pandas as pd

import r6_common as C
from gordian_analysis.frontier import pareto_mask

OUT = C.OUT
PRIMARY = C.setting_id(*C.PRIMARY)


def f(x, nd=3):
    if x is None or (isinstance(x, float) and np.isnan(x)):
        return "-"
    if isinstance(x, float) and np.isinf(x):
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


def pair(v):
    lo, hi = json.loads(v) if isinstance(v, str) else (np.nan, np.nan)
    return lo, hi


def arm_label(a):
    return f"`{a}`" if isinstance(a, str) else "none"


def criterion_table(path, sens_only=False):
    s = pd.read_csv(OUT / path)
    order = {"b5-rho0.7": 0, "b2.5-rho0.7": 1, "b8-rho0.7": 2, "b2.5-rho0": 3, "b5-rho0": 4, "b8-rho0": 5}
    s = s.assign(o=s.setting.map(order)).sort_values("o")
    rows = []
    for _, r in s.iterrows():
        qlo, qhi = pair(r.ceiling_quality_ci)
        rows.append({
            "setting (b, rho)": r.setting + (" (primary)" if r.setting == PRIMARY else ""),
            "ceiling: quality [90%] @ refs/call": f"{iv(r.ceiling_quality, qlo, qhi)} @ {f(r.ceiling_refs_per_call, 2)}",
            "1: best public at refs/call <= ceiling's": (f"{arm_label(r.c1_best_arm)} {f(r.c1_best_quality)} @ {f(r.c1_best_refs_per_call, 1)}"
                                                         if isinstance(r.c1_best_arm, str) else "none: the empty context, quality 0"),
            "1: gap [90%]": iv(r.c1_gap, r.c1_gap_lo, r.c1_gap_hi),
            "1 (gap >= 0.10, lower bound > 0.05)": yes(r.c1_holds),
            "2: fewest refs/call within 0.05 of ceiling": (f"{arm_label(r.c2_arm)} @ {f(r.c2_refs_per_call, 1)}"
                                                           if isinstance(r.c2_arm, str) else "none reaches it"),
            "2: ratio [90%]": (f"{f(r.c2_ratio, 1)} [{f(r.c2_ratio_lo, 1)}, {f(r.c2_ratio_hi, 1)}]"),
            "2: resamples with none within 0.05": f(r.c2_unreachable_share, 3),
            "2 (ratio >= 1.5, lower bound > 1.25)": yes(r.c2_holds),
        })
    return md(pd.DataFrame(rows))


def criterion():
    return criterion_table("r6-criterion.csv")


def criterion_supp():
    return criterion_table("r6-criterion-supplementary.csv")


def split():
    s = pd.read_csv(OUT / "r6-timing-context-split.csv")
    s = s[s.setting.isin([PRIMARY, "b2.5-rho0.7", "b8-rho0.7"])]
    rows = [{"setting": r.setting, "difference in hard-incident quality (slow leak excluded)": r.difference,
             "paired difference [90%]": iv(r.quality_diff, r.lo, r.hi)} for _, r in s.iterrows()]
    return md(pd.DataFrame(rows))


def points(setting=PRIMARY):
    d = pd.read_csv(OUT / "r6-heldout-points.csv")
    return d[d.setting == setting].copy()


def config_label(r):
    if r.kind == "window":
        return f"W {int(r.p_w)} s, N {int(r.p_n)}"
    if r.kind == "cooccur":
        return f"delta {r.p_d_ms / 1000:g} s, N {int(r.p_n)}"
    if r.kind == "neighbourhood":
        return f"k {int(r.p_k)}, N {int(r.p_n)}"
    return "the rung's own"


def row_of(r, label=None):
    return {
        "arm / configuration": label or f"`{r.arm}`",
        "hard quality (excl. leak)": f(r.quality),
        "refs per call": f(r.refs_per_call, 1),
        "calls / stream": f(r.calls_per_stream, 1),
        "tokens / stream": f"{r.tokens_per_stream:.0f}",
        "cost s / stream": f(r.cost_s, 2),
        "critical misses": int(r.critical_misses),
        "plain accuracy": f(r.plain_acc),
        "calls refused (budget)": int(r.refused),
    }


def builders(setting=PRIMARY, pairing="selection oracle"):
    """Per builder, the held-out points of the frontier configurations (selection held at the
    selection oracle), with the references per call, tokens, cost, critical misses, plain accuracy."""
    p = points(setting)
    p = p[p.pairing == pairing]
    rows = []
    for kind in C.KINDS:
        g = p[p.kind == kind].sort_values("refs_per_call")
        for _, r in g.iterrows():
            row = row_of(r, f"{kind}: {config_label(r)}")
            rows.append(row)
    return md(pd.DataFrame(rows))


def ladder():
    """Quality a public builder reaches within a budget of references per call (selection oracle,
    held-out, primary setting): the best of the carried configurations of each builder with
    refs per call no greater than the budget; the context-only and R4 ceilings beside it."""
    p = points()
    sel = p[(p.pairing == "selection oracle")]
    co = p[p.pairing == "context-only ceiling (supplementary)"].iloc[0]
    r4 = p[p.pairing == "R4 oracle (ceiling)"].iloc[0]
    rows = []
    for budget in (8, 16, 32, 64, 128, 192, 256, 512):
        row = {"refs per call at most": budget}
        for kind in ("rung", "window", "cooccur", "neighbourhood"):
            g = sel[(sel.kind == kind) & (sel.refs_per_call <= budget)]
            row[kind] = (f"{f(g.quality.max())} (`{g.sort_values(['quality', 'refs_per_call'], ascending=[False, True]).iloc[0].arm.replace('sel_', '').replace('_privileged', '')}`)"
                         if len(g) else "-")
        rows.append(row)
    rows.append({"refs per call at most": f"ceilings: context-only {f(co.quality)} @ {f(co.refs_per_call, 1)}, R4 oracle {f(r4.quality)} @ {f(r4.refs_per_call, 1)}",
                 "rung": "", "window": "", "cooccur": "", "neighbourhood": ""})
    return md(pd.DataFrame(rows))


def refs():
    """The reference arms beside them."""
    p = points()
    keep = [("R4 oracle (ceiling)", None), ("context-only ceiling (supplementary)", None),
            ("ablation (hidden rules)", None), ("never_escalate", None)]
    rows = []
    for role, _ in keep:
        g = p[p.pairing == role]
        for _, r in g.iterrows():
            rows.append(row_of(r, f"{role}: `{r.arm}`"))
    return md(pd.DataFrame(rows))


FAMS = ("compound", "cascade", "split_brain", "slow_leak")


def fam_cell(r, f_):
    n = int(r[f"{f_}_n"])
    return f"{int(r[f'{f_}_correct'])}/{n} = {r[f'{f_}_rate']:.2f}"


def families(setting=PRIMARY):
    """Per hard-fault family for: the ceilings, the rung, and for each builder the highest-quality
    and the cheapest-within-0.05-of-it frontier configuration (selection oracle) and the best
    `always_escalate` pairing; slow leak last and separate."""
    p = points(setting)
    sel = p[p.pairing == "selection oracle"]
    alw = p[p.pairing == "always_escalate"]
    rows = []

    def add(label, r):
        rows.append({"arm": label, "refs per call": f(r.refs_per_call, 1),
                     **{f_.replace("_", " "): fam_cell(r, f_) for f_ in FAMS}})

    for role in ("R4 oracle (ceiling)", "context-only ceiling (supplementary)", "ablation (hidden rules)"):
        for _, r in p[p.pairing == role].iterrows():
            add(f"{role}: `{r.arm}`", r)
    for kind in C.KINDS:
        g = sel[sel.kind == kind]
        top = g.sort_values(["quality", "refs_per_call"], ascending=[False, True]).iloc[0]
        near = g[g.quality >= top.quality - 0.05].sort_values("refs_per_call").iloc[0]
        for tag, r in (("highest quality", top), ("fewest refs within 0.05 of it", near)):
            if kind == "rung" and tag != "highest quality":
                continue
            add(f"selection oracle + {kind} ({config_label(r)}; {tag})", r)
        ga = alw[alw.kind == kind]
        if len(ga):
            r = ga.sort_values(["quality", "refs_per_call"], ascending=[False, True]).iloc[0]
            add(f"`always_escalate` + {kind} ({config_label(r)}; best quality)", r)
    for _, r in p[p.pairing == "never_escalate"].iterrows():
        add("`never_escalate`", r)
    return md(pd.DataFrame(rows))


def always(setting=PRIMARY):
    p = points(setting)
    p = p[p.pairing == "always_escalate"]
    rows = []
    for kind in C.KINDS:
        g = p[p.kind == kind].sort_values(["quality", "refs_per_call"], ascending=[False, True]).head(3)
        for _, r in g.iterrows():
            d = int(r.arm.rsplit("_d", 1)[1])
            rows.append(row_of(r, f"{kind}: {config_label(r)}, delay {d} s"))
    return md(pd.DataFrame(rows))


def settings_best():
    """For every setting, the best public builder's selection-oracle point beside the ceilings."""
    d = pd.read_csv(OUT / "r6-heldout-points.csv")
    rows = []
    order = ["b5-rho0.7", "b2.5-rho0.7", "b8-rho0.7", "b2.5-rho0", "b5-rho0", "b8-rho0"]
    for sid in order:
        p = d[d.setting == sid]
        sel = p[p.pairing == "selection oracle"]
        r4 = p[p.pairing == "R4 oracle (ceiling)"].iloc[0]
        co = p[p.pairing == "context-only ceiling (supplementary)"].iloc[0]
        rung = sel[sel.kind == "rung"].iloc[0]
        row = {"setting": sid, "rung (R5 reference)": f(rung.quality),
               **{k: "" for k in ("window", "cooccur", "neighbourhood")}}
        for kind in ("window", "cooccur", "neighbourhood"):
            g = sel[sel.kind == kind]
            top = g.sort_values(["quality", "refs_per_call"], ascending=[False, True]).iloc[0]
            row[kind] = f"{f(top.quality)} @ {f(top.refs_per_call, 0)}"
        row["context-only ceiling"] = f"{f(co.quality)} @ {f(co.refs_per_call, 1)}"
        row["R4 oracle"] = f"{f(r4.quality)} @ {f(r4.refs_per_call, 1)}"
        rows.append(row)
    return md(pd.DataFrame(rows))


def heldout_frontier():
    """How the tuned frontiers fared on the held-out streams: per builder, the held-out frontier
    (quality against refs per call) among the configurations carried."""
    p = points()
    sel = p[p.pairing == "selection oracle"]
    rows = []
    for kind in C.KINDS:
        g = sel[sel.kind == kind]
        mask = pareto_mask(g.quality, g.refs_per_call)
        rows.append({"builder": kind, "configurations carried to the held-out run": len(g),
                     "still on the held-out frontier": int(mask.sum())})
    return md(pd.DataFrame(rows))


def trace():
    t = pd.read_csv(OUT / "r6-trace.csv")
    t = t[t.family == "ALL"]
    rows = []
    order = {"rung": 0, "window": 1, "cooccur": 2, "neighbourhood": 3}
    t = t.assign(o=t.kind.map(order)).sort_values(["o", "mean_context_refs"])
    for _, r in t.iterrows():
        rows.append({
            "arm": f"`{r.arm}`", "mean refs": f(r.mean_context_refs, 0),
            "calls about hard": int(r.calls_about_hard), "complete": int(r.complete), "lacking": int(r.lacking),
            "arrived decisive obs": int(r.arrived_obs), "in context": int(r.in_context),
            "service not attached: in / of": f"{int(r.service_not_attached_in)} / {int(r.service_not_attached)}",
            "outside window: in / of": f"{int(r.outside_window_in)} / {int(r.outside_window)}",
            "attached: in / of": f"{int(r.attached_in)} / {int(r.attached)}",
            "still missing: cap": int(r.missing_context_cap), "still missing: not admitted": int(r.missing_not_admitted),
        })
    return md(pd.DataFrame(rows))


def trace_families():
    t = pd.read_csv(OUT / "r6-trace.csv")
    t = t[t.family != "ALL"]
    keep = ["sel_rung_privileged", "sel_coc_d16000_n256_privileged", "sel_nbh_k4_n256_privileged",
            "sel_win_w40_n256_privileged", "sel_win_w80_n512_privileged"]
    t = t[t.arm.isin(keep)]
    rows = []
    for arm in keep:
        for fam in FAMS:
            r = t[(t.arm == arm) & (t.family == fam)]
            if r.empty:
                continue
            r = r.iloc[0]
            rows.append({"arm": f"`{arm}`", "family": fam.replace("_", " "), "calls": int(r.calls_about_hard),
                         "complete": int(r.complete), "arrived obs": int(r.arrived_obs), "in context": int(r.in_context),
                         "service not attached: in / of": f"{int(r.service_not_attached_in)} / {int(r.service_not_attached)}",
                         "outside window: in / of": f"{int(r.outside_window_in)} / {int(r.outside_window)}",
                         "missing: cap / not admitted": f"{int(r.missing_context_cap)} / {int(r.missing_not_admitted)}"})
    return md(pd.DataFrame(rows))


def tuning():
    sel = json.load(open(OUT / "r6-selected.json"))
    rows = []
    for sid, s in sel.items():
        fr = json.load(open(OUT / "r6-frontier.json"))[sid]
        rows.append({"setting": sid, "selection oracle delay s (R5's)": s["selection_delay_s"],
                     "frontier configurations: rung / window / cooccur / neighbourhood":
                         " / ".join(str(fr["frontier_sizes"][k]) for k in C.KINDS),
                     "carried (rung always carried)": len(s["sel_configs"]),
                     "`always_escalate` delay of `rung`, s": s["always_configs"]["rung"],
                     "best window / cooccur / neighbourhood (tuning)": " / ".join(
                         f"`{s['best'][k]['best']}`" for k in ("window", "cooccur", "neighbourhood"))})
    return md(pd.DataFrame(rows))


def runs():
    ix = pd.read_csv(OUT / "r6-run-index.csv")
    rows = [{"run id": f"`{r.run_id}`", "revision": f"`{r.source_revision[:7]}`", "arms x streams": f"{r.arms} x {r.streams}",
             "manifest.json sha256": f"`{r.manifest_sha256}`", "wall s": int(round(r.wall_s))} for _, r in ix.iterrows()]
    return md(pd.DataFrame(rows))


def drift():
    p = pd.read_csv(OUT / "r6-provenance.csv")
    rows = [{"run": f"`{r.run_id}`", "wall s": round(r.wall_ns / 1e9), "cpu s": round(r.cpu_ns / 1e9),
             "peak mem MB": round(r.peak_memory_bytes / 2**20), "OOM kills": int(r.oom_kills),
             "internal/external": f(r.internal_external_ratio), "drift blocks": int(r.drift_blocks),
             "CV (block ns)": f(r.drift_cv_ns), "last/first": f(r.drift_last_over_first_ns, 2),
             "CV (min ns)": f(r.drift_cv_min_ns), "last/first (min)": f(r.drift_last_over_first_min_ns, 2)}
            for _, r in p.iterrows()]
    return md(pd.DataFrame(rows))


def xcheck():
    x = pd.read_csv(OUT / "r6-xcheck-r5.csv")
    return md(x.rename(columns={"run": "R5 run replayed", "arms": "arms", "arms_in_r5_record": "arms in R5's record",
                                "results_identical": "results.csv identical",
                                "incidents_identical": "incidents.csv identical",
                                "differing": "differing arms"}))


def xcheck_in_run():
    x = pd.read_csv(OUT / "r6-r5-crosscheck-in-run.csv")
    return md(x)


TABLES = {n: g for n, g in globals().items() if callable(g) and n in (
    "criterion", "criterion_supp", "split", "ladder", "builders", "refs", "families", "always", "settings_best",
    "heldout_frontier", "trace", "trace_families", "tuning", "runs", "drift", "xcheck", "xcheck_in_run")}

if __name__ == "__main__":
    print(TABLES[sys.argv[1]]())
