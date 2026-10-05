"""R5 analysis of the held-out runs: the criterion as the plan fixed it, the decomposition, the decoys.

Usage: r5_report.py        (reads the six held-out runs, the sensitivity runs and the tuning tables;
                            writes r5-*.csv under experiments/exploration/)

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation).

The criterion (docs/local-test-plan.md, 5R, R5, "Criterion, fixed by the coordinator before any R5
run (2026-10-06)") is applied exactly as written and none of its numbers is changed here:

  EXP-101 has selection headroom at the primary setting (b = 5, rho = 0.7) if either
  1. `oracle_selection_privileged` exceeds the best non-privileged frontier, now including
     `contradiction_escalation`, at equal or lower cost by at least 0.10 in hard-incident quality
     (slow leak excluded), with the 90% cluster-bootstrap lower bound above 0.05; or
  2. at quality within 0.05 of `oracle_selection_privileged`, the best non-privileged configuration
     costs at least 1.5 times as much, with the 90% interval of the cost ratio excluding 1.25.

Readings the plan leaves open, fixed here and stated in the report:

* "the best non-privileged frontier ... at equal or lower cost" is R4's comparison with the
  selection oracle in place of R4's oracle: `frontier.cluster_bootstrap`, the best held-out quality
  among the configurations the tuning streams put on a family frontier, at cost no greater than the
  selection oracle's, recomputed inside every resample.
* criterion 2: "within 0.05" is read as quality at least the selection oracle's minus 0.05 (a
  configuration above it is not excluded); "the best configuration" is the cheapest of those; its
  cost over the selection oracle's cost is the ratio, recomputed in every resample, infinite when
  no configuration reaches that quality; "excluding 1.25" is read as a 5th-percentile ratio above
  1.25 (the point estimate being at least 1.5).
* the same two numbers are reported at b = 2.5 and b = 8 at rho = 0.7 (only b varies) and at rho = 0.

Quality is the pooled fraction (ratio of sums over streams) of hard incidents outside the slow-leak
family declared correctly by their deadline; cost is the mean total modelled cost per stream;
intervals are 90% equal-tailed percentile intervals from 10,000 resamples of whole streams.
"""

import hashlib
import json

import numpy as np
import pandas as pd

import r5_common as C
from gordian_analysis.drift import drift_report, load_drift
from gordian_analysis.frontier import (
    ClusterData,
    arm_point,
    cluster_bootstrap,
    headroom_verdict,
    pareto_mask,
    points_table,
    stream_table,
)
from gordian_analysis.load import load_stream_run

N_RESAMPLES = 10_000
SEED0 = 9_500
MARGIN, LOWER = 0.10, 0.05
WITHIN, RATIO, RATIO_EXCLUDES = 0.05, 1.5, 1.25
FAMILIES = ("compound", "cascade", "split_brain", "slow_leak")


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def family_counts(arm):
    inc = arm.incidents
    hard = inc[inc["tier"] == "hard"]
    out = {}
    for f in FAMILIES:
        g = hard[hard["family"] == f]
        out[f] = (int(g["correct_by_deadline"].sum()), len(g))
    return out


def criterion2(data, sel, baselines, seed, n=N_RESAMPLES, chunk=500):
    """Cost ratio of the cheapest baseline within WITHIN of the selection oracle's quality."""
    io = data.row(sel)
    ib = np.array([data.row(b) for b in baselines])
    q, c = data.estimates()
    ok = (q[ib] >= q[io] - WITHIN) & ~np.isnan(q[ib])
    if ok.any():
        k = np.flatnonzero(ok)[np.argmin(c[ib][ok])]
        point = float(c[ib][k] / c[io])
        arm = baselines[k]
    else:
        point, arm = float("inf"), None
    rng = np.random.default_rng(seed)
    ratios = []
    done = 0
    nst = data.n_streams
    while done < n:
        m = min(chunk, n - done)
        idx = rng.integers(0, nst, size=(m, nst))
        qs, cs = data.estimates(idx)
        qo, co = qs[io], cs[io]
        okm = (qs[ib] >= qo[None, :] - WITHIN) & ~np.isnan(qs[ib])
        cost = np.where(okm, cs[ib], np.inf).min(axis=0)
        ratios.append(cost / co)
        done += m
    r = np.concatenate(ratios)
    return {
        "point": point,
        "arm": arm,
        "lo": float(np.quantile(r, 0.05, method="lower")),
        "hi": float(np.quantile(r, 0.95, method="higher")),
        "unreachable_share": float(np.mean(~np.isfinite(r))),
        "median": float(np.median(r)),
    }


def paired_diff(data, a, b, seed, n=N_RESAMPLES, chunk=500):
    """Quality of arm `a` minus quality of arm `b`, cluster-bootstrapped over the same streams."""
    ia, ib = data.row(a), data.row(b)
    q, _ = data.estimates()
    rng = np.random.default_rng(seed)
    out = []
    done = 0
    while done < n:
        m = min(chunk, n - done)
        idx = rng.integers(0, data.n_streams, size=(m, data.n_streams))
        qs, _ = data.estimates(idx)
        out.append(qs[ia] - qs[ib])
        done += m
    d = np.concatenate(out)
    lo, hi = np.quantile(d, [0.05, 0.95])
    return float(q[ia] - q[ib]), float(lo), float(hi)


def mean_diff_interval(x, y, seed, n=N_RESAMPLES):
    """Mean over streams of (x - y), cluster-bootstrapped; x and y are per-stream arrays."""
    d = np.asarray(x, float) - np.asarray(y, float)
    rng = np.random.default_rng(seed)
    idx = rng.integers(0, len(d), size=(n, len(d)))
    means = d[idx].mean(axis=1)
    lo, hi = np.quantile(means, [0.05, 0.95])
    return float(d.mean()), float(lo), float(hi)


def tuned_choice(tuning, sid, held_arms, sel_cost_tune):
    """The comparison arm the tuning streams pick as best at cost <= the selection oracle's tuning
    cost (cheapest of ties), among the arms that were run on the held-out streams."""
    t = tuning[(tuning["setting"] == sid) & (tuning["role"] == "comparison") & tuning["quality"].notna()]
    ok = t[(t["cost_s"] <= sel_cost_tune) & t["arm"].isin(held_arms)]
    if ok.empty:
        return None
    top = ok[ok["quality"] == ok["quality"].max()].sort_values("cost_s")
    return top.iloc[0]["arm"]


def main():
    selected = C.load_json(C.OUT / "r5-selected.json")
    t4 = pd.read_csv(C.OUT / "r4-tuning-points.csv")
    t5 = pd.read_csv(C.OUT / "r5-tuning-points.csv")
    tuning = pd.concat([t4, t5], ignore_index=True)
    crit_rows, decomp_rows, decoy_rows, point_rows, tier_rows, meta_rows, suppl_rows = [], [], [], [], [], [], []
    for i, (b, rho) in enumerate(C.SETTINGS):
        sid = C.setting_id(b, rho)
        sel_info = selected[sid]
        sel = C.sel_name(sel_info["selection_delay_s"])
        rid = C.run_id("heldout", b, rho)
        run = load_stream_run(C.RUNS / rid)
        pts = points_table(run)
        pts["cost_s"] = pts["cost_ns"] / C.NS
        comp = pts[pts["role"] == "comparison"].copy()
        names = list(comp["arm"])
        data = ClusterData.from_arms(run.arms)

        # ---- criterion 1 ----
        est = cluster_bootstrap(data, sel, names, seed=SEED0 + i, n_resamples=N_RESAMPLES)
        v1 = headroom_verdict(est, margin=MARGIN, lower_bound=LOWER)
        # ---- criterion 2 ----
        c2 = criterion2(data, sel, names, SEED0 + 50 + i)
        v2 = bool(c2["point"] >= RATIO and c2["lo"] > RATIO_EXCLUDES)
        # ---- robustness: configurations chosen on the tuning streams ----
        sel_tune_cost = sel_info["selection_tuning_cost_s"]
        tuned1 = tuned_choice(tuning, sid, set(names), sel_tune_cost)
        t1 = None
        if tuned1:
            d, lo, hi = paired_diff(data, sel, tuned1, SEED0 + 100 + i)
            row = pts[pts["arm"] == tuned1].iloc[0]
            t1 = (tuned1, float(row["quality"]), float(row["cost_s"]), d, lo, hi)

        sel_pt = pts[pts["arm"] == sel].iloc[0]
        r4o = pts[pts["arm"] == C.ORACLE].iloc[0]
        abl = pts[pts["arm"] == C.ABLATION].iloc[0]
        nev = pts[pts["arm"] == C.NEVER].iloc[0]
        top_any = comp.sort_values(["quality", "cost_ns"], ascending=[False, True]).iloc[0]
        best_eq = pts[pts["arm"] == est.best_arm].iloc[0] if est.best_arm else None
        contra = comp[comp["policy"] == C.CONTRA_POLICY]
        top_contra = contra.sort_values(["quality", "cost_ns"], ascending=[False, True]).iloc[0] if len(contra) else None
        crit_rows.append(
            {
                "setting": sid, "b": b, "rho": rho, "streams": est.n_streams,
                "hard_incidents_excl_leak": int(sel_pt["quality_incidents"]),
                "selection_arm": sel, "selection_delay_s": sel_info["selection_delay_s"],
                "selection_quality": est.oracle_quality, "selection_quality_lo": est.oracle_quality_low,
                "selection_quality_hi": est.oracle_quality_high,
                "selection_cost_s": est.oracle_cost_ns / C.NS,
                "selection_cost_lo": est.oracle_cost_low / C.NS, "selection_cost_hi": est.oracle_cost_high / C.NS,
                "c1_best_public_arm": est.best_arm, "c1_best_public_quality": est.best_quality,
                "c1_best_public_quality_lo": est.best_quality_low, "c1_best_public_quality_hi": est.best_quality_high,
                "c1_best_public_cost_s": est.best_cost_ns / C.NS,
                "c1_gap": est.gap, "c1_gap_lo": est.gap_low, "c1_gap_hi": est.gap_high,
                "c1_holds": v1 == "headroom",
                "c1_no_affordable_share": est.no_affordable_share,
                "c2_cheapest_within_arm": c2["arm"], "c2_ratio": c2["point"], "c2_ratio_lo": c2["lo"],
                "c2_ratio_hi": c2["hi"], "c2_unreachable_share": c2["unreachable_share"],
                "c2_holds": v2,
                "tuned_arm": t1[0] if t1 else None, "tuned_quality": t1[1] if t1 else None,
                "tuned_cost_s": t1[2] if t1 else None, "tuned_gap": t1[3] if t1 else None,
                "tuned_gap_lo": t1[4] if t1 else None, "tuned_gap_hi": t1[5] if t1 else None,
                "bootstrap_seed": est.seed, "resamples": est.n_resamples,
            }
        )

        # ---- decomposition ----
        steps = [
            ("best public, highest quality at any cost", top_any["arm"]),
        ]
        if est.best_arm:
            steps.append(("best public at cost <= selection oracle's", est.best_arm))
        if top_contra is not None:
            steps.append(("best contradiction_escalation, any cost", top_contra["arm"]))
        steps += [
            ("selection oracle (tuned delay)", sel),
            ("R4 oracle (selection + timing + context)", C.ORACLE),
            ("ablation: hidden rules in the cheap rung", C.ABLATION),
            ("never_escalate", C.NEVER),
        ]
        d_sel_r4 = paired_diff(data, C.ORACLE, sel, SEED0 + 200 + i)
        d_sel_top = paired_diff(data, sel, top_any["arm"], SEED0 + 300 + i)
        for label, name in steps:
            arm = run.arms[name]
            p = arm_point(arm)
            fr = family_counts(arm)
            row = {
                "setting": sid, "step": label, "arm": name,
                "quality_excl_leak": p["quality"], "cost_s": p["cost_ns"] / C.NS, "calls_per_stream": p["calls"],
                "hard_incl_leak": (sum(v[0] for v in fr.values()) / sum(v[1] for v in fr.values())),
            }
            for f in FAMILIES:
                row[f"{f}_correct"], row[f"{f}_n"] = fr[f]
                row[f"{f}_rate"] = fr[f][0] / fr[f][1] if fr[f][1] else float("nan")
            decomp_rows.append(row)
        decomp_rows.append(
            {
                "setting": sid, "step": "difference: selection oracle minus best public (any cost)",
                "arm": f"{sel} - {top_any['arm']}", "quality_excl_leak": d_sel_top[0],
                "quality_lo": d_sel_top[1], "quality_hi": d_sel_top[2],
            }
        )
        decomp_rows.append(
            {
                "setting": sid, "step": "difference: selection oracle minus best public at its cost (criterion 1)",
                "arm": f"{sel} - {est.best_arm}", "quality_excl_leak": est.gap,
                "quality_lo": est.gap_low, "quality_hi": est.gap_high,
            }
        )
        decomp_rows.append(
            {
                "setting": sid, "step": "difference: R4 oracle minus selection oracle (timing and context)",
                "arm": f"{C.ORACLE} - {sel}", "quality_excl_leak": d_sel_r4[0],
                "quality_lo": d_sel_r4[1], "quality_hi": d_sel_r4[2],
            }
        )

        # ---- decoys ----
        tab = {n: stream_table(run.arms[n]) for n in run.arms}
        best_dismiss = comp.assign(
            dismissed=[int(tab[n]["decoys_dismissed"].sum()) for n in comp["arm"]]
        ).sort_values(["dismissed", "cost_ns"], ascending=[False, True]).iloc[0]["arm"]
        shown = [("decoy oracle", C.DECOY), ("never_escalate", C.NEVER),
                 ("best public, highest quality", top_any["arm"]),
                 ("public arm with most dismissals", best_dismiss),
                 ("selection oracle", sel), ("R4 oracle", C.ORACLE)]
        never_t = tab[C.NEVER]
        for label, name in shown:
            t = tab[name]
            p = arm_point(run.arms[name])
            fa = mean_diff_interval(t["false_alarms"], never_t["false_alarms"], SEED0 + 400 + i)
            crit_missed = int(t["critical_missed_plain"].sum() + t["critical_missed_hard"].sum())
            decoy_rows.append(
                {
                    "setting": sid, "label": label, "arm": name, "cost_s": p["cost_ns"] / C.NS,
                    "decoys": int(t["incidents_decoy"].sum()),
                    "dismissed": int(t["decoys_dismissed"].sum()), "alarmed": int(t["decoys_alarmed"].sum()),
                    "silent": int(t["decoys_silent"].sum()),
                    "false_alarms_per_stream": float(t["false_alarms"].mean()),
                    "false_alarms_minus_never": fa[0], "fa_minus_never_lo": fa[1], "fa_minus_never_hi": fa[2],
                    "wrong_declarations_per_stream": float(t["wrong_declarations"].mean()),
                    "critical_misses": crit_missed,
                    "critical_incidents": int(t["critical_incidents"].sum()),
                    "critical_miss_rate": crit_missed / int(t["critical_incidents"].sum()),
                    "plain_accuracy": p["plain_rate"],
                    "calls_per_stream": p["calls"],
                }
            )

        # ---- every configuration's held-out point, and the tiers of the arms that matter ----
        on_frontier = dict(zip(comp["arm"], pareto_mask(comp["quality"], comp["cost_ns"])))
        for _, r in pts.iterrows():
            point_rows.append(
                {
                    "setting": sid, "b": b, "rho": rho, "arm": r["arm"], "role": r["role"], "policy": r["policy"],
                    "p": r.get("p"), "delay_s": (r.get("delay_ns") or 0) / C.NS if "delay_ns" in r and pd.notna(r.get("delay_ns")) else 0.0,
                    "period_s": r["period_ns"] / C.NS if "period_ns" in r and pd.notna(r.get("period_ns")) else None,
                    "persist_s": r["persist_ns"] / C.NS if "persist_ns" in r and pd.notna(r.get("persist_ns")) else None,
                    "tau": r.get("tau"), "wait_s": r["wait_ns"] / C.NS if "wait_ns" in r and pd.notna(r.get("wait_ns")) else None,
                    "quality": r["quality"], "quality_correct": r["quality_correct"],
                    "quality_incidents": r["quality_incidents"], "cost_s": r["cost_s"], "calls": r["calls"],
                    "plain_rate": r["plain_rate"], "leak_rate": r["leak_rate"],
                    "critical_misses": r["critical_misses"], "wrong_per_stream": r["wrong_per_stream"],
                    "false_alarms_per_stream": r["false_alarms_per_stream"],
                    "on_heldout_frontier": bool(on_frontier.get(r["arm"], False)),
                }
            )
        for label, name in [("selection oracle", sel), ("R4 oracle", C.ORACLE), ("decoy oracle", C.DECOY),
                            ("never", C.NEVER), ("best public, highest quality", top_any["arm"]),
                            ("best contradiction", top_contra["arm"] if top_contra is not None else C.NEVER)]:
            t = tab[name]
            p = arm_point(run.arms[name])
            tier_rows.append(
                {
                    "setting": sid, "label": label, "arm": name, "cost_s": p["cost_ns"] / C.NS,
                    "calls_per_stream": p["calls"], "quality_excl_leak": p["quality"],
                    "plain_accuracy": p["plain_rate"], "critical_misses": p["critical_misses"],
                    "critical_miss_rate": p["critical_miss_rate"],
                    "false_alarms_per_stream": p["false_alarms_per_stream"],
                    "wrong_declarations_per_stream": p["wrong_per_stream"],
                    "hard_escalated_share": float(t["hard_incidents_escalated"].sum() / t["incidents_hard"].sum()),
                }
            )

        # ---- provenance ----
        usage = run.usage or {}
        man = C.RUNS / rid / "manifest.json"
        drift = drift_report(load_drift(C.RUNS / rid / "drift.csv"))
        meta_rows.append(
            {
                "setting": sid, "run_id": rid, "manifest_sha256": sha(man), "arms": len(run.arms),
                "streams": est.n_streams, "cpu_model": run.manifest.get("cpu_model"),
                "cpu_mhz": run.manifest.get("cpu_mhz"), "source_revision": run.manifest["source_revision"],
                "wall_ns": usage.get("wall_ns"), "cpu_ns": usage.get("cpu_ns"),
                "peak_memory_bytes": usage.get("peak_memory_bytes"), "oom_kills": usage.get("oom_kills"),
                "exit_code": usage.get("exit_code"),
                "internal_external_ratio": usage.get("internal_external_ratio"),
                "drift_blocks": drift.n_blocks, "drift_cv_ns": drift.ns.cv,
                "drift_last_over_first_ns": drift.ns.ratio_last_first,
                "drift_cv_min_ns": drift.min_ns.cv, "drift_last_over_first_min_ns": drift.min_ns.ratio_last_first,
            }
        )

        # ---- sensitivity: the selection oracle at every delay of its grid (a separate run) ----
        sid_run = C.RUNS / C.run_id("suppl", b, rho)
        if sid_run.exists():
            srun = load_stream_run(sid_run)
            sp = points_table(srun)
            for _, r in sp.iterrows():
                fr = family_counts(srun.arms[r["arm"]])
                suppl_rows.append(
                    {
                        "setting": sid, "arm": r["arm"],
                        "delay_s": (r.get("delay_ns") or 0) / C.NS if pd.notna(r.get("delay_ns")) else 0.0,
                        "quality": r["quality"], "cost_s": r["cost_ns"] / C.NS, "calls": r["calls"],
                        "compound_rate": fr["compound"][0] / fr["compound"][1],
                        "cascade_rate": fr["cascade"][0] / fr["cascade"][1],
                        "split_brain_rate": fr["split_brain"][0] / fr["split_brain"][1],
                        "slow_leak_rate": fr["slow_leak"][0] / fr["slow_leak"][1],
                        "chosen_on_tuning_streams": r["arm"] == sel,
                    }
                )

    out = C.OUT
    pd.DataFrame(crit_rows).to_csv(out / "r5-criterion.csv", index=False, float_format="%.6g")
    pd.DataFrame(decomp_rows).to_csv(out / "r5-decomposition.csv", index=False, float_format="%.6g")
    pd.DataFrame(decoy_rows).to_csv(out / "r5-decoys.csv", index=False, float_format="%.6g")
    pd.DataFrame(point_rows).to_csv(out / "r5-heldout-points.csv", index=False, float_format="%.6g")
    pd.DataFrame(tier_rows).to_csv(out / "r5-tiers.csv", index=False, float_format="%.6g")
    pd.DataFrame(meta_rows).to_csv(out / "r5-provenance.csv", index=False)
    if suppl_rows:
        pd.DataFrame(suppl_rows).to_csv(out / "r5-selection-delay-sensitivity.csv", index=False, float_format="%.6g")
    for name in ("criterion", "decomposition", "decoys", "heldout-points", "tiers", "provenance"):
        print(out / f"r5-{name}.csv")


if __name__ == "__main__":
    main()
