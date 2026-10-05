"""What R7 reports beside the criterion, never folded into it (docs/local-test-plan.md, 5R, R7).

Usage: r7_beside.py     (after the six held-out runs and `r7_criterion.py`; writes
                         r7-heldout-points.csv, r7-beside.csv, r7-optimum.csv, r7-reprice.csv)

Per run (delta, setting):

* every arm's point (`r6_stats.point`): hard-incident quality (slow leak excluded), critical misses,
  plain accuracy, references per call, cost per stream, calls refused, family rates;
* the fewest references per call within 0.05 of the ceiling and its ratio to the ceiling's
  (`r6_stats.criterion` clause 2, with the context-only ceiling), over the carried selection-oracle
  arms, with the 5th/95th percentile ratio over 10,000 resamples (unreachable resamples count as
  infinite);
* the held-out-best builder (highest held-out quality among the carried selection-oracle arms, fewest
  references on ties) and its gap to the ceiling with a paired interval, beside the tuning-selected
  builder's G of `r7-criterion.csv`, and the best carried arm of each builder kind;
* each builder's tuning frontier per delta (the six tuning runs, and delta = 0 from R6's committed
  `r6-tuning-selection.csv` at the primary setting), and where the optimum sits: the configuration of
  highest tuning quality per kind and the fewest references within 0.05 of that quality;
* the per-reference price re-priced analytically at 5, 20 and 80 tokens per reference for the
  selection-oracle rows only (the carried builders and the context-only ceiling, not R4's oracle),
  valid for a row only if no call was refused in any stream and the re-priced tokens stay inside the
  manifest's per-stream reasoner token limit in every stream. A row that fails either check is kept
  and flagged; its re-priced cost is not used.

Re-pricing changes the cost of a call, not what it asks or what the reasoner answers: references per
call, the answers and the quality are those of the run. It rests on the arms and the harness not
consulting the price when they decide, which the validity checks make sufficient: if no call was
refused and the re-priced total stays below the limit in every stream, no affordability gate that
depends on the remaining token budget can bind at the new price. The re-pricing is checked at 20
tokens per reference, where it must reproduce the recorded reasoner cost exactly.

Exploration (nothing here tests a hypothesis).
"""

import numpy as np
import pandas as pd

import r6_stats as S
import r7_common as C
from gordian_analysis.load import load_stream_run

SEED0 = 9_800


def classify(name, selected_key):
    if name == C.ORACLE:
        return "R4 oracle (reference)", None, "reference"
    if name.startswith("oracle_selection_context"):
        return "context-only ceiling", None, "ceiling"
    key = C.C6.kind_of_arm(name)
    return ("selected builder" if key == selected_key else "carried builder"), key, C.kind_of_key(key)


def reprice_table(run, arms, ceiling, seed):
    """Re-priced rows for `arms` (selection-oracle rows and the ceiling) at each price."""
    rows = []
    per = {n: S.per_stream(run.arms[n]) for n in arms}
    cost_ns = {}
    for n, t in per.items():
        base = (t["total_cost_ns"] - t["reasoner_cost_ns"]).to_numpy(float)
        calls = t["calls"].to_numpy(float)
        refs = t["refs"].to_numpy(float)
        refused = int(t["refused"].sum())
        for tok in C.REPRICE_TOKENS:
            tokens = C.BASE_TOKENS * calls + tok * refs
            cost = base + tokens * C.NS_PER_TOKEN
            cost_ns[(n, tok)] = cost
            rec = {
                "arm": n, "tokens_per_ref": tok, "calls_refused": refused,
                "tokens_per_stream": float(tokens.mean()), "max_tokens_in_a_stream": float(tokens.max()),
                "token_limit": C.TOKEN_LIMIT,
                "within_limit_every_stream": bool(tokens.max() <= C.TOKEN_LIMIT),
                "cost_s_per_stream": float(cost.mean()) / 1e9,
                "reasoner_cost_s_per_stream": float((tokens * C.NS_PER_TOKEN).mean()) / 1e9,
            }
            rec["valid"] = bool(refused == 0 and rec["within_limit_every_stream"])
            if tok == 20:
                recorded = t["total_cost_ns"].to_numpy(float)
                rec["check_reproduces_recorded_cost_max_abs_ns"] = float(np.abs(cost - recorded).max())
            rows.append(rec)
    # ratios to the ceiling, cluster-bootstrapped (whole streams), at each price
    n_streams = len(next(iter(per.values())))
    rng = np.random.default_rng(seed)
    done, boots = 0, []
    while done < C.N_RESAMPLES:
        m = min(500, C.N_RESAMPLES - done)
        boots.append(rng.multinomial(n_streams, np.full(n_streams, 1.0 / n_streams), size=m).astype(float))
        done += m
    w = np.concatenate(boots)
    for rec in rows:
        n, tok = rec["arm"], rec["tokens_per_ref"]
        if n == ceiling:
            rec["cost_ratio_to_ceiling"] = 1.0
            rec["cost_ratio_lo"] = rec["cost_ratio_hi"] = 1.0
            continue
        a, c = cost_ns[(n, tok)], cost_ns[(ceiling, tok)]
        rec["cost_ratio_to_ceiling"] = float(a.mean() / c.mean())
        ratio = (w @ a) / (w @ c)
        rec["cost_ratio_lo"], rec["cost_ratio_hi"] = S.interval(ratio)
    return rows


def main():
    sel = C.load_json(C.OUT / "r7-selected.json")
    crit = pd.read_csv(C.OUT / "r7-criterion.csv").set_index("run_id")
    points, beside, reprice = [], [], []
    for k, (b, rho, delta) in enumerate(C.RUNS_SPEC):
        tid = C.run_id("tune", b, rho, delta)
        hid = C.run_id("heldout", b, rho, delta)
        s = sel[tid]
        d = s["selection_delay_s"]
        run = load_stream_run(C.RUNS / hid)
        streams = S.Streams(run)
        pts = {n: S.point(a) for n, a in run.arms.items()}
        ceiling = C.ctxonly_name(d)
        pubs = [n for n in run.arms if n.startswith("sel_")]
        for n, p in pts.items():
            role, key, kind = classify(n, s["selected"])
            row = dict(p)
            row.update({"run_id": hid, "setting": C.setting_id(b, rho), "b": b, "rho": rho, "delta": delta,
                        "role": role, "builder": key, "kind": kind})
            if key:
                for kk, v in C.BUILDERS[key][1].items():
                    row[f"p_{kk}"] = v
            points.append(row)
        # clause-2-style: fewest references per call within 0.05 of the ceiling
        c = S.criterion(streams, ceiling, pubs, C.BOOT_SEED0 + 200 + k, C.N_RESAMPLES)
        # the held-out-best builder and its gap
        best = max(pubs, key=lambda n: (round(pts[n]["quality"], 12), -pts[n]["refs_per_call"]))
        g_best, lo_b, hi_b = S.paired(streams, ceiling, best, SEED0 + k, C.N_RESAMPLES)
        sel_arm = C.sel_arm_name(s["selected"], d)
        row = {
            "run_id": hid, "setting": C.setting_id(b, rho), "delta": delta, "streams": streams.n,
            "ceiling_arm": ceiling, "ceiling_quality": pts[ceiling]["quality"],
            "ceiling_refs_per_call": pts[ceiling]["refs_per_call"],
            "selected_arm": sel_arm, "selected_quality": pts[sel_arm]["quality"],
            "selected_refs_per_call": pts[sel_arm]["refs_per_call"],
            "G_selected": float(crit.loc[hid, "G"]), "G_selected_lo": float(crit.loc[hid, "lo"]),
            "G_selected_hi": float(crit.loc[hid, "hi"]),
            "heldout_best_arm": best, "heldout_best_quality": pts[best]["quality"],
            "heldout_best_refs_per_call": pts[best]["refs_per_call"],
            "heldout_best_gap": g_best, "heldout_best_gap_lo": lo_b, "heldout_best_gap_hi": hi_b,
            "within005_arm": c["c2_arm"], "within005_refs_per_call": c["c2_refs_per_call"],
            "within005_ratio": c["c2_ratio"], "within005_ratio_lo": c["c2_ratio_lo"],
            "within005_ratio_hi": c["c2_ratio_hi"], "within005_unreachable_share": c["c2_unreachable_share"],
            "r4_oracle_quality": pts[C.ORACLE]["quality"],
        }
        for kind in C.KINDS:
            ks = [n for n in pubs if C.kind_of_key(C.C6.kind_of_arm(n)) == kind]
            kb = max(ks, key=lambda n: (round(pts[n]["quality"], 12), -pts[n]["refs_per_call"]))
            row[f"best_{kind}_arm"] = kb
            row[f"best_{kind}_quality"] = pts[kb]["quality"]
            row[f"best_{kind}_refs_per_call"] = pts[kb]["refs_per_call"]
        beside.append(row)
        rp = reprice_table(run, pubs + [ceiling], ceiling, SEED0 + 50 + k)
        for r in rp:
            r.update({"run_id": hid, "setting": C.setting_id(b, rho), "delta": delta,
                      "role": "context-only ceiling" if r["arm"] == ceiling else
                      ("selected builder" if r["arm"] == sel_arm else "carried builder")})
        reprice += rp
        print(C.setting_id(b, rho), f"delta {delta:g}", "ceiling", round(pts[ceiling]["quality"], 3),
              "selected", s["selected"], round(pts[sel_arm]["quality"], 3),
              "held-out best", best, round(pts[best]["quality"], 3), "gap", round(g_best, 3))
    for name, rows in (("heldout-points", points), ("beside", beside), ("reprice", reprice)):
        pd.DataFrame(rows).to_csv(C.OUT / f"r7-{name}.csv", index=False, float_format="%.6g")
    optimum()


def optimum():
    """Each builder's tuning frontier per delta and where its optimum sits."""
    t = pd.read_csv(C.OUT / "r7-tuning-points.csv")
    t = t[t["kind"].isin(C.KINDS)].copy()
    r6 = pd.read_csv(C.OUT / "r6-tuning-selection.csv")
    r6 = r6[(r6["kind"].isin(C.KINDS)) & (r6["setting"] == C.setting_id(*C.PRIMARY))].copy()
    r6["delta"] = 0.0
    r6["run_id"] = "r6-tune1-" + r6["setting"]
    # R6's file holds every grid configuration (frontier flag as R6 computed it at delta 0).
    cols = ["run_id", "setting", "delta", "builder", "kind", "quality", "refs_per_call", "frontier",
            "critical_misses", "plain_acc", "cost_s"]
    allp = pd.concat([r6[cols], t[cols]], ignore_index=True)
    rows = []
    for (rid, sid, delta), g in allp.groupby(["run_id", "setting", "delta"], sort=False):
        for kind in C.KINDS:
            gk = g[g["kind"] == kind].sort_values(["quality", "refs_per_call"], ascending=[False, True])
            top = gk.iloc[0]
            near = gk[gk["quality"] >= top["quality"] - C.WITHIN].sort_values("refs_per_call")
            knee = near.iloc[0]
            fr = gk[gk["frontier"]].sort_values("refs_per_call")
            rows.append({
                "run_id": rid, "setting": sid, "delta": delta, "kind": kind,
                "configurations": len(gk), "frontier_configurations": len(fr),
                "best_config": top["builder"], "best_tuning_quality": top["quality"],
                "best_refs_per_call": top["refs_per_call"],
                "knee_config": knee["builder"], "knee_tuning_quality": knee["quality"],
                "knee_refs_per_call": knee["refs_per_call"],
                "frontier_refs_min": fr["refs_per_call"].min(), "frontier_refs_max": fr["refs_per_call"].max(),
                "frontier": ";".join(f"{r.builder}:{r.quality:.3f}@{r.refs_per_call:.1f}" for r in fr.itertuples()),
            })
    pd.DataFrame(rows).to_csv(C.OUT / "r7-optimum.csv", index=False, float_format="%.6g")


if __name__ == "__main__":
    main()
