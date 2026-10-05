"""R4 analysis of the held-out runs: the headroom tables, with 90% cluster-bootstrap intervals.

Usage: r4_report.py        (reads the six held-out runs and the tuning table; writes r4-*.csv and
                            r4-tables.json under experiments/exploration/)

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation).

Definitions (docs/local-test-plan.md, 5R, R4, "Margin", fixed by the coordinator before any R4 run):
quality = pooled fraction of hard incidents (slow-leak family excluded) declared correctly by their
deadline; cost = mean total modelled cost per stream; headroom exists on a setting when the oracle's
quality exceeds the best non-privileged frontier's quality at equal or lower cost by at least 0.10
with the lower limit of the 90% interval (streams resampled as clusters) above 0.05. This script
applies those numbers and changes none of them.

Two estimates of "the best non-privileged quality at cost <= the oracle's" are reported:

  frontier   as the margin words it: the best, on the held-out streams, among the configurations that
             the tuning streams put on a family frontier. A maximum over many noisy points, so biased
             upward, which biases the gap against finding headroom. Recomputed inside every resample.
  tuned      the one configuration the *tuning* streams pick as best at cost <= the oracle's tuning
             cost, evaluated on the held-out streams. Unbiased by selection, one noisy point.

The verdict uses `frontier` (the margin's wording); `tuned` is the robustness check.
"""

import json
import hashlib

import numpy as np
import pandas as pd

import r4_common as C
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
SEED0 = 9_000
MARGIN, LOWER = 0.10, 0.05
FAMILIES = ("compound", "cascade", "split_brain", "slow_leak")


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def tuned_choice(tuning, sid, held_arms):
    """The arm the tuning streams pick: best quality among comparison arms with tuning cost at most
    the oracle's tuning cost; the cheapest among ties."""
    t = tuning[tuning["setting"] == sid]
    orc = t[t["role"] == "privileged"].iloc[0]
    ok = t[(t["role"] == "comparison") & (t["cost_s"] <= orc["cost_s"]) & t["quality"].notna()]
    top = ok[ok["quality"] == ok["quality"].max()].sort_values("cost_s")
    for _, r in top.iterrows():
        if r["arm"] in held_arms:
            return r["arm"], float(orc["cost_s"])
    return None, float(orc["cost_s"])


def paired_gap_interval(data, oracle, arm, seed, n=N_RESAMPLES):
    """Oracle quality minus `arm`'s quality, cluster-bootstrapped; cost is not re-matched."""
    io, ia = data.row(oracle), data.row(arm)
    rng = np.random.default_rng(seed)
    gaps = []
    for _ in range(n // 500):
        idx = rng.integers(0, data.n_streams, size=(500, data.n_streams))
        q, _ = data.estimates(idx)
        gaps.append(q[io] - q[ia])
    g = np.concatenate(gaps)
    lo, hi = np.quantile(g, [0.05, 0.95])
    return float(lo), float(hi)


def family_rates(arm):
    inc = arm.incidents
    hard = inc[inc["tier"] == "hard"]
    out = {}
    for f in FAMILIES:
        g = hard[hard["family"] == f]
        out[f] = (int(g["correct_by_deadline"].sum()), len(g))
    return out


def main():
    tuning = pd.read_csv(C.OUT / "r4-tuning-points.csv")
    setting_rows, frontier_rows, family_rows, tier_rows, meta_rows = [], [], [], [], []
    detail = {}
    for i, (b, rho) in enumerate(C.SETTINGS):
        sid = C.setting_id(b, rho)
        rid = C.run_id("heldout", b, rho)
        run = load_stream_run(C.RUNS / rid)
        pts = points_table(run)
        pts["cost_s"] = pts["cost_ns"] / C.NS
        base = pts[pts["role"] == "comparison"].copy()
        base["on_heldout_frontier"] = pareto_mask(base["quality"], base["cost_ns"])
        data = ClusterData.from_arms(run.arms)
        names = list(base["arm"])
        est = cluster_bootstrap(data, C.ORACLE, names, seed=SEED0 + i, n_resamples=N_RESAMPLES)
        verdict = headroom_verdict(est, margin=MARGIN, lower_bound=LOWER)

        # the configuration the tuning streams choose
        tuned_arm, orc_tune_cost = tuned_choice(tuning, sid, set(names))
        tuned = pts[pts["arm"] == tuned_arm].iloc[0] if tuned_arm else None
        tlo = thi = float("nan")
        if tuned_arm:
            tlo, thi = paired_gap_interval(data, C.ORACLE, tuned_arm, SEED0 + 100 + i)

        orc = pts[pts["arm"] == C.ORACLE].iloc[0]
        abl = pts[pts["arm"] == C.ABLATION].iloc[0]
        nev = pts[pts["arm"] == C.NEVER].iloc[0]
        best = pts[pts["arm"] == est.best_arm].iloc[0] if est.best_arm else None
        reach = pts[pts["arm"] == est.reach_arm].iloc[0] if est.reach_arm else None
        top_any = base.sort_values(["quality", "cost_ns"], ascending=[False, True]).iloc[0]
        s = lambda ns: ns / C.NS  # noqa: E731
        setting_rows.append(
            {
                "setting": sid, "b": b, "rho": rho, "streams": est.n_streams,
                "hard_incidents_excl_leak": int(orc["quality_incidents"]),
                "oracle_quality": est.oracle_quality,
                "oracle_quality_lo": est.oracle_quality_low, "oracle_quality_hi": est.oracle_quality_high,
                "oracle_cost_s": s(est.oracle_cost_ns),
                "oracle_cost_lo": s(est.oracle_cost_low), "oracle_cost_hi": s(est.oracle_cost_high),
                "best_arm": est.best_arm, "best_quality": est.best_quality,
                "best_quality_lo": est.best_quality_low, "best_quality_hi": est.best_quality_high,
                "best_cost_s": s(est.best_cost_ns),
                "gap": est.gap, "gap_lo": est.gap_low, "gap_hi": est.gap_high,
                "verdict": verdict,
                "no_affordable_share": est.no_affordable_share,
                "reach_arm": est.reach_arm,
                "reach_cost_s": s(est.reach_cost_ns) if np.isfinite(est.reach_cost_ns) else float("inf"),
                "reach_cost_lo": s(est.reach_cost_low), "reach_cost_hi": s(est.reach_cost_high),
                "reach_share": est.reach_share,
                "reach_cost_ratio": (est.reach_cost_ns / est.oracle_cost_ns) if np.isfinite(est.reach_cost_ns) else float("inf"),
                "highest_quality_arm": top_any["arm"], "highest_quality": top_any["quality"],
                "highest_quality_cost_s": top_any["cost_s"],
                "tuned_arm": tuned_arm,
                "tuned_quality": None if tuned is None else tuned["quality"],
                "tuned_cost_s": None if tuned is None else tuned["cost_s"],
                "tuned_gap": None if tuned is None else est.oracle_quality - tuned["quality"],
                "tuned_gap_lo": tlo, "tuned_gap_hi": thi,
                "oracle_tuning_cost_s": orc_tune_cost,
                "ablation_quality": abl["quality"], "ablation_cost_s": abl["cost_s"],
                "never_quality": nev["quality"],
                "oracle_leak_rate": orc["leak_rate"], "oracle_leak_incidents": int(orc["leak_incidents"]),
                "ablation_leak_rate": abl["leak_rate"],
                "best_arm_share_top": json.dumps(dict(list(est.best_arm_share.items())[:4])),
                "bootstrap_seed": est.seed, "resamples": est.n_resamples,
            }
        )
        # every configuration's held-out point
        for _, r in pts.iterrows():
            frontier_rows.append(
                {
                    "setting": sid, "b": b, "rho": rho, "arm": r["arm"], "role": r["role"],
                    "policy": r["policy"], "p": r.get("p"), "delay_s": (r.get("delay_ns") or 0) / C.NS if "delay_ns" in r else None,
                    "period_s": r.get("period_ns", np.nan) / C.NS if "period_ns" in r else None,
                    "tau": r.get("tau"), "wait_s": r.get("wait_ns", np.nan) / C.NS if "wait_ns" in r else None,
                    "quality": r["quality"], "quality_correct": r["quality_correct"],
                    "quality_incidents": r["quality_incidents"], "cost_s": r["cost_s"],
                    "calls": r["calls"], "plain_rate": r["plain_rate"],
                    "leak_rate": r["leak_rate"], "critical_misses": r["critical_misses"],
                    "wrong_per_stream": r["wrong_per_stream"],
                    "false_alarms_per_stream": r["false_alarms_per_stream"],
                    "on_heldout_frontier": bool(base.loc[base["arm"] == r["arm"], "on_heldout_frontier"].iloc[0])
                    if r["role"] == "comparison" else False,
                }
            )
        # per family and per tier, for the arms that matter
        shown = [("oracle", C.ORACLE), ("ablation", C.ABLATION), ("never", C.NEVER)]
        if est.best_arm:
            shown.append(("best_at_oracle_cost", est.best_arm))
        if tuned_arm:
            shown.append(("tuned_at_oracle_cost", tuned_arm))
        shown.append(("highest_quality_baseline", top_any["arm"]))
        seen = set()
        for label, name in shown:
            arm = run.arms[name]
            fr = family_rates(arm)
            t = stream_table(arm)
            row = {"setting": sid, "role_label": label, "arm": name}
            for f in FAMILIES:
                row[f"{f}_correct"], row[f"{f}_n"] = fr[f]
                row[f"{f}_rate"] = fr[f][0] / fr[f][1] if fr[f][1] else float("nan")
            family_rows.append(row)
            p = arm_point(arm)
            hard_all = int(t["quality_num"].sum() + t["leak_num"].sum()), int(t["quality_den"].sum() + t["leak_den"].sum())
            tier_rows.append(
                {
                    "setting": sid, "role_label": label, "arm": name,
                    "cost_s": p["cost_ns"] / C.NS, "calls_per_stream": p["calls"],
                    "quality_hard_excl_leak": p["quality"],
                    "hard_incl_leak": hard_all[0] / hard_all[1],
                    "plain_accuracy": p["plain_rate"],
                    "critical_misses_total": p["critical_misses"],
                    "critical_miss_rate": p["critical_miss_rate"],
                    "decoys_alarmed": p["decoys_alarmed"],
                    "decoys_dismissed": int(t["decoys_dismissed"].sum()),
                    "decoys_silent": int(t["decoys_silent"].sum()),
                    "decoys": p["decoys"],
                    "false_alarms_per_stream": p["false_alarms_per_stream"],
                    "wrong_declarations_per_stream": p["wrong_per_stream"],
                    "hard_escalated_share": float(t["hard_incidents_escalated"].sum() / t["incidents_hard"].sum()),
                }
            )
        # provenance and instrument diagnostics
        usage = run.usage or {}
        man = C.RUNS / rid / "manifest.json"
        drift = drift_report(load_drift(C.RUNS / rid / "drift.csv"))
        meta_rows.append(
            {
                "setting": sid, "run_id": rid, "manifest_sha256": sha(man),
                "arms": len(run.arms), "streams": est.n_streams,
                "cpu_model": run.manifest.get("cpu_model"), "cpu_mhz": run.manifest.get("cpu_mhz"),
                "source_revision": run.manifest["source_revision"],
                "wall_ns": usage.get("wall_ns"), "cpu_ns": usage.get("cpu_ns"),
                "peak_memory_bytes": usage.get("peak_memory_bytes"), "oom_kills": usage.get("oom_kills"),
                "exit_code": usage.get("exit_code"),
                "internal_external_ratio": usage.get("internal_external_ratio"),
                "drift_blocks": drift.n_blocks, "drift_cv_ns": drift.ns.cv,
                "drift_last_over_first_ns": drift.ns.ratio_last_first,
                "drift_cv_min_ns": drift.min_ns.cv, "drift_last_over_first_min_ns": drift.min_ns.ratio_last_first,
                "results_sha256": json.dumps({a: sha(C.RUNS / rid / a / "results.csv") for a in run.arms}),
            }
        )
    out = C.OUT
    pd.DataFrame(setting_rows).to_csv(out / "r4-settings.csv", index=False, float_format="%.6g")
    pd.DataFrame(frontier_rows).to_csv(out / "r4-heldout-points.csv", index=False, float_format="%.6g")
    pd.DataFrame(family_rows).to_csv(out / "r4-families.csv", index=False, float_format="%.6g")
    pd.DataFrame(tier_rows).to_csv(out / "r4-tiers.csv", index=False, float_format="%.6g")
    pd.DataFrame(meta_rows).to_csv(out / "r4-provenance.csv", index=False)
    for name in ("settings", "heldout-points", "families", "tiers", "provenance"):
        print(out / f"r4-{name}.csv")


if __name__ == "__main__":
    main()
