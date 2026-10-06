"""M3's held-out analysis: the criterion as written at 500 ms and 2 s, 100 ms for continuity,
everything reported beside, every row with strict precision, background and cost.

Usage: m3_analyze.py   reads artifacts/runs/m3-heldout-b5-rho0.7; writes
                       experiments/exploration/m3-heldout-table.csv (every arm, every measure, 90%
                       cluster-bootstrap intervals), m3-criterion.csv (the paired differences and
                       the verdict per tick length and result, for the frozen medium and for its
                       sub-tick-off control), m3-latency.csv, m3-discordance.csv

Readings are `m3_common`'s, fixed before the held-out run. The bootstrap is B1's, B2's and M2's:
10,000 resamples of whole streams, the same counts for every arm and every measure
(`b2_stats.Measures`), so differences are paired.
"""

import json

import numpy as np
import pandas as pd

import m3_common as C  # noqa: I001 (sets the path for the two below)
import b2_stats as B
from gordian_analysis.load import load_stream_run

RUN = C.RUNS / C.run_id("heldout")
ARM = C.arm_name

SHOW = [
    "hard_noticed_share", "hard_anchor_correct_share", "hard_anchor_site_correct_share",
    "leak_noticed_share", "leak_anchor_correct_share", "notices_on_background_per_stream",
    "notice_precision", "strict_precision", "notices_per_incident", "notices_per_stream",
    "plain_noticed_share", "quality", "leak_quality", "calls_per_stream", "substrate_s_per_stream",
    "cost_s_per_stream",
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
    table.to_csv(C.OUT / "m3-heldout-table.csv", index=False)

    comp = ARM(C.COMPARATOR)
    crit = []
    for tick in C.TICKS_MS:
        for variant, label in (("", "frozen M3 medium"), ("_subtick_off", "control: sub-tick off"),
                               ("m2", "M2's frozen medium")):
            med = ARM(f"m2_t{tick}") if variant == "m2" else ARM(f"med_t{tick}{variant}")
            if med not in m.row:
                continue
            j = m.row[med]
            bg = float(pts["notices_on_background_per_stream"][j])
            sp = float(pts["strict_precision"][j])
            for name, res in (("result1", C.RESULT1), ("result2", C.RESULT2)):
                point, lo, hi = m.paired_difference(res["measure"], med, comp, C.BOOT_SEED,
                                                    C.N_RESAMPLES)
                holds = (point >= res["margin"] and lo > res["lower_bound_above"]
                         and bg <= C.BACKGROUND_BOUND and sp >= C.STRICT_PRECISION_BOUND)
                crit.append({
                    "tick_ms": tick, "arm": med, "row": label, "result": name,
                    "in_criterion": tick in C.CRITERION_TICKS_MS and name == "result1"
                    and variant == "",
                    "measure": res["measure"],
                    "medium": float(pts[res["measure"]][j]),
                    "reference": float(pts[res["measure"]][m.row[comp]]),
                    "difference": point, "lower": lo, "higher": hi, "margin": res["margin"],
                    "lower_bound_above": res["lower_bound_above"],
                    "background_per_stream": bg, "background_bound": C.BACKGROUND_BOUND,
                    "strict_precision": sp, "strict_precision_bound": C.STRICT_PRECISION_BOUND,
                    "point_ok": point >= res["margin"], "bound_ok": lo > res["lower_bound_above"],
                    "background_ok": bg <= C.BACKGROUND_BOUND,
                    "strict_precision_ok": sp >= C.STRICT_PRECISION_BOUND,
                    "holds": holds,
                })
    crit = pd.DataFrame(crit)
    crit.to_csv(C.OUT / "m3-criterion.csv", index=False)
    verdict = {}
    for tick in C.CRITERION_TICKS_MS:
        r = crit[(crit.tick_ms == tick) & crit.in_criterion].iloc[0]
        verdict[tick] = bool(r.holds)
    holds = all(verdict.values())

    # The frozen medium against its control, paired, on the measures the control is for.
    ctrl = []
    for tick in C.TICKS_MS:
        a, b = ARM(f"med_t{tick}"), ARM(f"med_t{tick}_subtick_off")
        for meas in ("hard_anchor_correct_share", "strict_precision",
                     "notices_on_background_per_stream", "leak_noticed_share"):
            point, lo, hi = m.paired_difference(meas, a, b, C.BOOT_SEED, C.N_RESAMPLES)
            ctrl.append({"tick_ms": tick, "measure": meas, "frozen_minus_control": point,
                         "lower": lo, "higher": hi})
    pd.DataFrame(ctrl).to_csv(C.OUT / "m3-control.csv", index=False)

    lat = []
    for arm in m.names:
        ni = run.arms[arm].notice_incidents
        for fam, mask in (("hard_non_leak", (ni.tier == "hard") & (ni.family != "slow_leak")),
                          ("slow_leak", (ni.tier == "hard") & (ni.family == "slow_leak"))):
            x = ni[mask & ni.noticed.astype(bool)]["notice_latency_ns"].astype(float) / 1e9
            lat.append({"arm": arm, "group": fam, "noticed": len(x),
                        "median_s": float(np.median(x)) if len(x) else None,
                        "p90_s": float(np.quantile(x, 0.9)) if len(x) else None})
    pd.DataFrame(lat).to_csv(C.OUT / "m3-latency.csv", index=False)

    # Discordance with the comparator on hard non-leak incidents.
    disc = []
    ref = run.arms[comp].notice_incidents
    hard = (ref.tier == "hard") & (ref.family != "slow_leak")
    key = ["seed", "incident"]
    rk = ref[hard].set_index(key).anchor_correct.astype(bool)
    for arm in m.names:
        if arm == comp:
            continue
        ni = run.arms[arm].notice_incidents
        mk = ni[(ni.tier == "hard") & (ni.family != "slow_leak")].set_index(key)
        mk = mk.anchor_correct.astype(bool).reindex(rk.index)
        disc.append({"arm": arm, "medium_right_comparator_wrong": int((mk & ~rk).sum()),
                     "comparator_right_medium_wrong": int((~mk & rk).sum()),
                     "both_wrong": int((~mk & ~rk).sum()), "both_right": int((mk & rk).sum())})
    pd.DataFrame(disc).to_csv(C.OUT / "m3-discordance.csv", index=False)

    with pd.option_context("display.width", 250, "display.max_columns", 40):
        cols = ["arm"] + SHOW
        print(table[cols].to_string(index=False, float_format=lambda x: f"{x:.3f}"))
        print(crit.to_string(index=False, float_format=lambda x: f"{x:.4f}"))
        print(pd.DataFrame(ctrl).to_string(index=False, float_format=lambda x: f"{x:.4f}"))
        print(pd.DataFrame(disc).to_string(index=False))
        print(pd.DataFrame(lat).to_string(index=False, float_format=lambda x: f"{x:.2f}"))
    print(json.dumps({"verdict_result1_by_tick": {str(k): v for k, v in verdict.items()},
                      "m3_holds": holds,
                      "comparator_reproduces_b2": {
                          k: float(pts[k][m.row[comp]]) for k in
                          ("hard_anchor_correct_share", "leak_noticed_share",
                           "notices_on_background_per_stream", "strict_precision")}}))


if __name__ == "__main__":
    main()
