"""Print the R7 report's tables as Markdown, from the committed CSVs, so that no number in a table is
typed by hand.

Usage: r7_tables.py NAME     (see TABLES below)
Exploration (nothing here tests a hypothesis).
"""

import json
import sys

import numpy as np
import pandas as pd

import r7_common as C

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


def yn(x):
    return "yes" if bool(x) else "no"


def cfg(key):
    """A builder configuration in words."""
    if key == "rung":
        return "rung"
    kind, p = C.BUILDERS[key]
    if kind == "window":
        return f"window W {p['w']} s, N {p['n']}"
    if kind == "cooccur":
        d = p["d_ms"] / 1000
        return f"cooccur delta {d:g} s, N {p['n']}"
    return f"neighbourhood k {p['k']}, N {p['n']}"


def arm_cfg(arm):
    key = C.C6.kind_of_arm(arm)
    return cfg(key) if key else arm


def criterion():
    c = pd.read_csv(OUT / "r7-criterion.csv")
    c = c[c["setting"] == PRIMARY].sort_values("delta")
    rows = []
    for _, r in c.iterrows():
        rows.append({
            "delta": f"{r.delta:g}" + (" (R6's files)" if r.delta == 0 else ""),
            "selected builder (highest tuning quality)": f"{cfg(r.selected)}; tuning {f(r.selected_tuning_quality)}",
            "ceiling quality @ refs/call": f"{f(r.ceiling_quality)} @ {f(r.ceiling_refs_per_call, 1)}",
            "selected quality @ refs/call": f"{f(r.selected_quality)} @ {f(r.selected_refs_per_call, 1)}",
            "G [90%]": iv(r.G, r.lo, r.hi),
            "upper bound < 0.10": yn(r["upper_below_0.10"]),
            "G >= 0.10 and lower bound > 0.05": yn(r.fragile_condition),
        })
    return md(pd.DataFrame(rows))


def sensitivity():
    c = pd.read_csv(OUT / "r7-criterion.csv")
    c = c[(c["delta"] == C.SENS_DELTA)].sort_values("b")
    rows = []
    for _, r in c.iterrows():
        rows.append({
            "setting (rho 0.7), delta 0.2": r.setting + (" (primary)" if r.setting == PRIMARY else ""),
            "selected builder": f"{cfg(r.selected)}; tuning {f(r.selected_tuning_quality)}",
            "ceiling quality @ refs/call": f"{f(r.ceiling_quality)} @ {f(r.ceiling_refs_per_call, 1)}",
            "selected quality @ refs/call": f"{f(r.selected_quality)} @ {f(r.selected_refs_per_call, 1)}",
            "G [90%]": iv(r.G, r.lo, r.hi),
            "G >= 0.10 and lower bound > 0.05": yn(r.fragile_condition),
        })
    return md(pd.DataFrame(rows))


def beside():
    b = pd.read_csv(OUT / "r7-beside.csv")
    rows = []
    for _, r in b.iterrows():
        rows.append({
            "run": f"{r.setting}, delta {r.delta:g}",
            "tuning-selected: G [90%]": iv(r.G_selected, r.G_selected_lo, r.G_selected_hi),
            "held-out-best builder (quality @ refs/call)": f"{arm_cfg(r.heldout_best_arm)}: {f(r.heldout_best_quality)} @ {f(r.heldout_best_refs_per_call, 1)}",
            "held-out-best gap [90%]": iv(r.heldout_best_gap, r.heldout_best_gap_lo, r.heldout_best_gap_hi),
            "fewest refs/call within 0.05 of ceiling": (f"{arm_cfg(r.within005_arm)} @ {f(r.within005_refs_per_call, 1)}"
                                                        if isinstance(r.within005_arm, str) else "none reaches it"),
            "ratio to ceiling's refs/call [90%]": (f"{f(r.within005_ratio, 1)} [{f(r.within005_ratio_lo, 1)}, {f(r.within005_ratio_hi, 1)}]"),
            "resamples with none within 0.05": f(r.within005_unreachable_share),
        })
    return md(pd.DataFrame(rows))


def best_by_kind():
    b = pd.read_csv(OUT / "r7-beside.csv")
    rows = []
    for _, r in b.iterrows():
        row = {"run": f"{r.setting}, delta {r.delta:g}",
               "context-only ceiling": f"{f(r.ceiling_quality)} @ {f(r.ceiling_refs_per_call, 1)}"}
        for k in C.KINDS:
            row[k] = f"{f(r[f'best_{k}_quality'])} @ {f(r[f'best_{k}_refs_per_call'], 0)} ({arm_cfg(r[f'best_{k}_arm'])})"
        row["R4 oracle (reference)"] = f(r.r4_oracle_quality)
        rows.append(row)
    return md(pd.DataFrame(rows))


def points(run):
    p = pd.read_csv(OUT / "r7-heldout-points.csv")
    p = p[p["run_id"] == run]
    order = {"context-only ceiling": 0, "selected builder": 1, "carried builder": 2, "R4 oracle (reference)": 3}
    p = p.assign(o=p["role"].map(order)).sort_values(["o", "kind", "refs_per_call"])
    rows = []
    for _, r in p.iterrows():
        label = {"context-only ceiling": "context-only ceiling", "R4 oracle (reference)": "R4 oracle (reference)"}.get(
            r.role, cfg(r.builder) + (" (selected)" if r.role == "selected builder" else ""))
        rows.append({
            "arm": label, "hard quality (excl. leak)": f(r.quality),
            "refs per call": f(r.refs_per_call, 1), "cost s / stream": f(r.cost_s, 2),
            "critical misses": int(r.critical_misses), "plain accuracy": f(r.plain_acc),
            "calls refused": int(r.refused),
        })
    return md(pd.DataFrame(rows))


def points_tables():
    out = []
    for (b, rho, delta) in C.RUNS_SPEC:
        run = C.run_id("heldout", b, rho, delta)
        out.append(f"**{C.setting_id(b, rho)}, delta {delta:g}** (`{run}`)\n\n{points(run)}")
    return "\n\n".join(out)


def optimum():
    o = pd.read_csv(OUT / "r7-optimum.csv")
    o = o[o["setting"] == PRIMARY]
    rows = []
    for kind in C.KINDS:
        for _, r in o[o["kind"] == kind].sort_values("delta").iterrows():
            rows.append({
                "builder": kind, "delta": f"{r.delta:g}",
                "best tuning config": f"{cfg(r.best_config)}: {f(r.best_tuning_quality)} @ {f(r.best_refs_per_call, 1)}",
                "fewest refs within 0.05 of that quality": f"{cfg(r.knee_config)}: {f(r.knee_tuning_quality)} @ {f(r.knee_refs_per_call, 1)}",
                "frontier configurations (of configurations)": f"{int(r.frontier_configurations)} of {int(r.configurations)}",
                "frontier refs/call range": f"{f(r.frontier_refs_min, 1)} to {f(r.frontier_refs_max, 1)}",
            })
    return md(pd.DataFrame(rows))


def optimum_sens():
    o = pd.read_csv(OUT / "r7-optimum.csv")
    o = o[(o["delta"] == C.SENS_DELTA) & (o["setting"] != PRIMARY)]
    rows = []
    for _, r in o.sort_values(["setting", "kind"]).iterrows():
        rows.append({
            "setting, delta 0.2": r.setting, "builder": r.kind,
            "best tuning config": f"{cfg(r.best_config)}: {f(r.best_tuning_quality)} @ {f(r.best_refs_per_call, 1)}",
            "fewest refs within 0.05 of that quality": f"{cfg(r.knee_config)}: {f(r.knee_tuning_quality)} @ {f(r.knee_refs_per_call, 1)}",
        })
    return md(pd.DataFrame(rows))


def frontier_detail():
    """The frontier of every builder at every delta of the primary setting, as quality @ references."""
    o = pd.read_csv(OUT / "r7-optimum.csv")
    o = o[o["setting"] == PRIMARY]
    out = []
    for kind in C.KINDS:
        for _, r in o[o["kind"] == kind].sort_values("delta").iterrows():
            pts = []
            for item in str(r.frontier).split(";"):
                key, rest = item.split(":")
                pts.append(f"{rest} (`{key}`)")
            out.append(f"- `{kind}`, delta {r.delta:g}: " + "; ".join(pts))
    return "\n".join(out)


def reprice():
    p = pd.read_csv(OUT / "r7-reprice.csv")
    p = p[p["role"].isin(["selected builder", "context-only ceiling"])]
    rows = []
    for (rid, delta), g in p.groupby(["run_id", "delta"], sort=False):
        ceil = g[g["role"] == "context-only ceiling"].set_index("tokens_per_ref")
        sel = g[g["role"] == "selected builder"].set_index("tokens_per_ref")
        for tok in C.REPRICE_TOKENS:
            s, c = sel.loc[tok], ceil.loc[tok]
            rows.append({
                "run": f"{s.setting}, delta {delta:g}", "tokens per ref": tok,
                "selected builder": cfg(C.C6.kind_of_arm(s.arm)),
                "selected: cost s / stream": f(s.cost_s_per_stream, 2),
                "ceiling: cost s / stream": f(c.cost_s_per_stream, 2),
                "cost ratio [90%]": iv(s.cost_ratio_to_ceiling, s.cost_ratio_lo, s.cost_ratio_hi, 1),
                "max tokens in a stream (limit 80,000)": f"{s.max_tokens_in_a_stream:,.0f}",
                "calls refused": int(s.calls_refused), "valid": yn(s.valid),
            })
    return md(pd.DataFrame(rows))


def reprice_validity():
    p = pd.read_csv(OUT / "r7-reprice.csv")
    rows = []
    for tok in C.REPRICE_TOKENS:
        g = p[p["tokens_per_ref"] == tok]
        rows.append({
            "tokens per ref": tok, "rows (selection-oracle arms and ceilings)": len(g),
            "rows with no call refused": int((g["calls_refused"] == 0).sum()),
            "rows within the token limit in every stream": int(g["within_limit_every_stream"].sum()),
            "valid rows": int(g["valid"].sum()),
            "invalid rows": ", ".join(sorted({f"{r.setting} d{r.delta:g} {arm_cfg(r.arm)}" for r in g[~g["valid"]].itertuples()})) or "none",
        })
    chk = p[p["tokens_per_ref"] == 20]["check_reproduces_recorded_cost_max_abs_ns"].max()
    return md(pd.DataFrame(rows)) + f"\n\nRe-pricing at 20 tokens per reference reproduces every recorded total cost: largest absolute difference {chk:g} ns per stream."


def runs():
    ix = pd.read_csv(OUT / "r7-run-index.csv")
    rows = []
    for _, r in ix.iterrows():
        rows.append({"run id": f"`{r.run_id}`", "revision": f"`{r.source_revision[:7]}`",
                     "arms x streams": f"{r.arms} x {r.streams}", "delta": f"{r.distractor_penalty:g}",
                     "exit": int(r.usage_exit_code), "wall s": f(r.wall_s, 0),
                     "peak mem MB": f(r.peak_memory_mb, 0), "OOM kills": int(r.oom_kills),
                     "internal/external": f(r.internal_external_ratio),
                     "manifest.json sha256": f"`{r.manifest_sha256}`"})
    return md(pd.DataFrame(rows))


def ceiling_vs_r6():
    x = pd.read_csv(OUT / "r7-ceiling-vs-r6.csv")
    x = x.assign(arm=x["arm"].map(lambda a: f"`{a}`"))
    return md(x)


def identity():
    x = pd.read_csv(OUT / "r7-identity.csv")
    return (f"{int(x['identical'].sum())} of {len(x)} arms identical in `results.csv` and `incidents.csv` "
            f"(arms in R6's record: {int(x['in_r6_record'].sum())}; in the replay: {int(x['in_replay'].sum())}).")


def selection():
    s = json.load(open(OUT / "r7-selected.json"))
    rows = []
    for rid, v in s.items():
        tops = v["top_of_kind"]
        rows.append({
            "tuning run": f"`{rid}`", "delta": f"{v['delta']:g}",
            "top configuration per builder": "; ".join(f"{k}: {cfg(tops[k])}" for k in C.KINDS),
            "configurations carried": len(v["carried"]),
            "selected": f"{cfg(v['selected'])} ({f(v['selected_tuning_quality'])})",
        })
    return md(pd.DataFrame(rows))


TABLES = {
    "criterion": criterion, "sensitivity": sensitivity, "beside": beside, "best_by_kind": best_by_kind,
    "points": points_tables, "optimum": optimum, "optimum_sens": optimum_sens, "frontier_detail": frontier_detail,
    "reprice": reprice, "reprice_validity": reprice_validity, "runs": runs, "ceiling_vs_r6": ceiling_vs_r6,
    "identity": identity, "selection": selection,
}

if __name__ == "__main__":
    print(TABLES[sys.argv[1]]())
