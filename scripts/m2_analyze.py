"""M2's held-out analysis: the criterion as written at each tick length, everything reported beside.

Usage: m2_analyze.py   reads artifacts/runs/m2-heldout-b5-rho0.7; writes
                       experiments/exploration/m2-heldout-table.csv (every arm, every measure, 90%
                       cluster-bootstrap intervals), m2-criterion.csv (the paired differences and
                       the verdict per tick length and result), m2-latency.csv

Readings are `m2_common`'s, fixed before the held-out run. The bootstrap is B1's and B2's: 10,000
resamples of whole streams, multinomial counts from numpy.random.default_rng(BOOT_SEED), the same
counts for every arm and every measure (`b2_stats.Measures`), so differences are paired.
"""

import json

import numpy as np
import pandas as pd

import m2_common as C  # noqa: I001 (sets the path for the two below)
import b2_stats as B
from gordian_analysis.load import load_stream_run

RUN = C.RUNS / C.run_id("heldout")
ARM = C.arm_name

SHOW = [
    "hard_noticed_share", "hard_anchor_correct_share", "hard_anchor_site_correct_share",
    "leak_noticed_share", "leak_anchor_correct_share", "notices_on_background_per_stream",
    "notice_precision", "strict_precision", "notices_per_incident", "plain_noticed_share",
    "quality", "leak_quality", "substrate_s_per_stream", "cost_s_per_stream",
]


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
    table.to_csv(C.OUT / "m2-heldout-table.csv", index=False)

    comp = ARM(C.COMPARATOR)
    crit = []
    for tick in C.TICKS_MS:
        med = ARM(f"med_t{tick}")
        bg = float(pts["notices_on_background_per_stream"][m.row[med]])
        for name, res in (("result1", C.RESULT1), ("result2", C.RESULT2)):
            for ref, label in ((comp, "reanchor (the comparator)"),
                               (ARM(C.OLD_COMPARATOR), "rung z = 2 (first comparator, beside)")):
                point, lo, hi = m.paired_difference(res["measure"], med, ref, C.BOOT_SEED,
                                                    C.N_RESAMPLES)
                holds = (point >= res["margin"] and lo > res["lower_bound_above"]
                         and bg <= C.BACKGROUND_BOUND)
                crit.append({
                    "tick_ms": tick, "result": name, "measure": res["measure"], "versus": label,
                    "medium": float(pts[res["measure"]][m.row[med]]),
                    "reference": float(pts[res["measure"]][m.row[ref]]),
                    "difference": point, "lower": lo, "higher": hi, "margin": res["margin"],
                    "lower_bound_above": res["lower_bound_above"],
                    "medium_background_per_stream": bg, "background_bound": C.BACKGROUND_BOUND,
                    "point_ok": point >= res["margin"], "bound_ok": lo > res["lower_bound_above"],
                    "background_ok": bg <= C.BACKGROUND_BOUND,
                    "holds": holds if ref == comp else None,
                })
    crit = pd.DataFrame(crit)
    crit.to_csv(C.OUT / "m2-criterion.csv", index=False)

    lat = []
    for arm in m.names:
        ni = run.arms[arm].notice_incidents
        for fam, mask in (("hard_non_leak", (ni.tier == "hard") & (ni.family != "slow_leak")),
                          ("slow_leak", (ni.tier == "hard") & (ni.family == "slow_leak"))):
            x = ni[mask & ni.noticed.astype(bool)]["notice_latency_ns"].astype(float) / 1e9
            lat.append({"arm": arm, "group": fam, "noticed": len(x),
                        "median_s": float(np.median(x)) if len(x) else None,
                        "p90_s": float(np.quantile(x, 0.9)) if len(x) else None})
    pd.DataFrame(lat).to_csv(C.OUT / "m2-latency.csv", index=False)

    with pd.option_context("display.width", 220, "display.max_columns", 40):
        cols = ["arm"] + SHOW
        print(table[cols].to_string(index=False, float_format=lambda x: f"{x:.3f}"))
        print(crit.to_string(index=False, float_format=lambda x: f"{x:.3f}"))
        print(pd.DataFrame(lat).to_string(index=False, float_format=lambda x: f"{x:.2f}"))
    print(json.dumps({"comparator_reproduces_b2": {
        k: float(pts[k][m.row[comp]]) for k in
        ("hard_anchor_correct_share", "leak_noticed_share", "notices_on_background_per_stream")}}))


if __name__ == "__main__":
    main()
