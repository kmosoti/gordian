"""M4's held-out analysis: the criterion as written (500 ms, six conditions against ramp + split over
the re-anchor), every row with its measures and intervals, each ablation against the frozen medium,
every medium row against the two public rows, cost per stream and leak anchor-correct beside.

Usage: m4_analyze.py   reads artifacts/runs/m4-heldout-b5-rho0.7; writes, in
                       experiments/exploration/:
                       m4-heldout-table.csv  every arm, every measure, 90% cluster-bootstrap
                                             intervals
                       m4-criterion.csv      the six conditions for the frozen medium (in the
                                             criterion) and, labelled, for every other medium row
                       m4-paired.csv         every medium row minus each public row, paired
                       m4-ablations.csv      the frozen medium minus each of its ablations, paired
                       m4-leak-misses.csv    per medium row, the leak incidents it did not notice
                       m4-latency.csv        latency of the first notice, hard non-leak and leak

Readings are `m4_common`'s, fixed before the first tuning run. The bootstrap is B1's, B2's, M2's
and M3's: 10,000 resamples of whole streams, the same counts for every arm and every measure
(`b2_stats.Measures`), so differences are paired.
"""

import json

import numpy as np
import pandas as pd

import m4_common as C  # noqa: I001 (sets the path for the two below)
import b2_stats as B
from gordian_analysis.load import load_stream_run

RUN = C.RUNS / C.run_id("heldout")
ARM = C.arm_name

SHOW = [
    "hard_noticed_share", "hard_anchor_correct_share", "hard_anchor_site_correct_share",
    "leak_noticed_share", "leak_anchor_correct_share", "notices_on_background_per_stream",
    "notice_precision", "strict_precision", "notices_per_incident", "notices_per_stream",
    "notices_on_decoy_per_stream", "plain_noticed_share", "quality", "leak_quality",
    "calls_per_stream", "substrate_s_per_stream", "cost_s_per_stream",
]
PAIRED = ["hard_anchor_correct_share", "leak_noticed_share", "leak_anchor_correct_share",
          "notices_on_background_per_stream", "strict_precision", "calls_per_stream",
          "cost_s_per_stream"]
PUBLIC = ["ramp_split_over_re2", "reanchor"]


def conditions(m, pts, med, comp):
    """The six conditions of the criterion for arm `med` against `comp`."""
    out = []
    j = m.row[med]
    for c in C.CONDITIONS:
        k = c["measure"]
        row = {"arm": med, "part": c["part"], "condition": c["condition"], "measure": k,
               "medium": float(pts[k][j]), "comparator": float(pts[k][m.row[comp]])}
        if c["condition"] == "paired_lower_bound":
            point, lo, hi = m.paired_difference(k, med, comp, C.BOOT_SEED, C.N_RESAMPLES)
            row.update(difference=point, lower=lo, higher=hi,
                       threshold=c["lower_bound_above"], met=bool(lo > c["lower_bound_above"]))
        elif "at_least" in c:
            row.update(threshold=c["at_least"], met=bool(pts[k][j] >= c["at_least"]))
        else:
            row.update(threshold=c["at_most"], met=bool(pts[k][j] <= c["at_most"]))
        out.append(row)
    return out


def main():
    run = load_stream_run(RUN)
    seeds = [int(s) for s in next(iter(run.arms.values())).results["seed"]]
    assert seeds == list(range(C.HELDOUT_SEEDS[0], C.HELDOUT_SEEDS[0] + C.HELDOUT_SEEDS[1]))
    m = B.Measures(run)
    pts = m.points()
    boot = m.boot(C.BOOT_SEED, C.N_RESAMPLES)
    rows = []
    for arm in m.names:
        i = m.row[arm]
        row = {"arm": arm}
        for k in SHOW:
            row[k] = float(pts[k][i])
            row[k + "_lo"] = float(boot[k][0][i])
            row[k + "_hi"] = float(boot[k][1][i])
        rows.append(row)
    table = pd.DataFrame(rows)
    table.to_csv(C.OUT / "m4-heldout-table.csv", index=False)

    comp = ARM(C.COMPARATOR)
    frozen = ARM(f"med_t{C.CRITERION_TICK_MS}")
    mediums = [a for a in m.names if a.startswith(("sel_med_", "sel_m3_", "sel_m2_"))]
    crit = []
    for med in mediums:
        for r in conditions(m, pts, med, comp):
            r["in_criterion"] = med == frozen
            crit.append(r)
    crit = pd.DataFrame(crit)
    crit.to_csv(C.OUT / "m4-criterion.csv", index=False)
    holds = {med: bool(crit[crit.arm == med].met.all()) for med in mediums}

    paired = []
    for med in mediums:
        for ref in PUBLIC:
            for k in PAIRED:
                point, lo, hi = m.paired_difference(k, med, ARM(ref), C.BOOT_SEED, C.N_RESAMPLES)
                paired.append({"arm": med, "reference": ARM(ref), "measure": k,
                               "medium": float(pts[k][m.row[med]]),
                               "reference_value": float(pts[k][m.row[ARM(ref)]]),
                               "difference": point, "lower": lo, "higher": hi})
    paired = pd.DataFrame(paired)
    paired.to_csv(C.OUT / "m4-paired.csv", index=False)

    abl = []
    for a in m.names:
        if a.startswith(f"sel_med_t{C.CRITERION_TICK_MS}_"):
            for k in PAIRED:
                point, lo, hi = m.paired_difference(k, frozen, a, C.BOOT_SEED, C.N_RESAMPLES)
                abl.append({"frozen": frozen, "ablation": a, "measure": k,
                            "frozen_value": float(pts[k][m.row[frozen]]),
                            "ablation_value": float(pts[k][m.row[a]]),
                            "frozen_minus_ablation": point, "lower": lo, "higher": hi})
    abl = pd.DataFrame(abl)
    abl.to_csv(C.OUT / "m4-ablations.csv", index=False)

    miss = []
    for arm in [comp] + mediums:
        ni = run.arms[arm].notice_incidents
        leak = ni[(ni.tier == "hard") & (ni.family == "slow_leak")]
        for _, r in leak[~leak.noticed.astype(bool)].iterrows():
            miss.append({"arm": arm, "seed": int(r.seed), "incident": int(r.incident)})
    miss = pd.DataFrame(miss, columns=["arm", "seed", "incident"])
    miss.to_csv(C.OUT / "m4-leak-misses.csv", index=False)

    lat = []
    for arm in m.names:
        ni = run.arms[arm].notice_incidents
        for fam, mask in (("hard_non_leak", (ni.tier == "hard") & (ni.family != "slow_leak")),
                          ("slow_leak", (ni.tier == "hard") & (ni.family == "slow_leak"))):
            x = ni[mask & ni.noticed.astype(bool)]["notice_latency_ns"].astype(float) / 1e9
            lat.append({"arm": arm, "group": fam, "noticed": len(x),
                        "median_s": float(np.median(x)) if len(x) else None,
                        "p90_s": float(np.quantile(x, 0.9)) if len(x) else None})
    pd.DataFrame(lat).to_csv(C.OUT / "m4-latency.csv", index=False)

    with pd.option_context("display.width", 250, "display.max_columns", 40,
                           "display.max_rows", 500):
        cols = ["arm", "hard_anchor_correct_share", "leak_noticed_share",
                "leak_anchor_correct_share", "notices_on_background_per_stream",
                "strict_precision", "calls_per_stream", "cost_s_per_stream"]
        print(table[cols].to_string(index=False, float_format=lambda x: f"{x:.4f}"))
        print(crit[crit.in_criterion].to_string(index=False, float_format=lambda x: f"{x:.4f}"))
        print(abl.to_string(index=False, float_format=lambda x: f"{x:+.4f}"))
        print(paired.to_string(index=False, float_format=lambda x: f"{x:+.4f}"))
        print(miss.groupby("arm").size().to_string())
    print(json.dumps({"m4_holds": holds[frozen], "all_six_by_row": holds,
                      "comparator_reproduces_b3": {
                          k: float(pts[k][m.row[comp]]) for k in C.COMPARATOR_B3}}, indent=1))


if __name__ == "__main__":
    main()
