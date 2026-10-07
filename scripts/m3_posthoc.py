"""M3's post-hoc readings, written after the held-out run had been played and analysed (nothing
here enters the criterion or was fixed before the run).

Usage: m3_posthoc.py   reads artifacts/runs/m3-heldout-b5-rho0.7; writes
                       experiments/exploration/m3-vs-m2-paired.csv (each frozen M3 medium minus M2's
                       frozen medium at the same tick length, paired as in m3_analyze), and
                       m3-misses.csv (per medium row and tick, the hard non-leak incidents that are
                       not anchor-correct: no notice attributed to the incident (a notice anchored on
                       an observation outside the incident, such as a stray just before it, is
                       attributed elsewhere, so it counts here), or attributed with its first
                       notice's anchor after the incident's first observation)
"""

import pandas as pd

import m3_common as C  # noqa: I001 (sets the path for the two below)
import b2_stats as B
from gordian_analysis.load import load_stream_run

RUN = C.RUNS / C.run_id("heldout")
ARM = C.arm_name
MEASURES = ["hard_anchor_correct_share", "leak_noticed_share", "leak_anchor_correct_share",
            "notices_on_background_per_stream", "strict_precision", "calls_per_stream",
            "cost_s_per_stream"]


def main():
    run = load_stream_run(RUN)
    m = B.Measures(run)
    pts = m.points()
    rows = []
    for tick in C.TICKS_MS:
        a, b = ARM(f"med_t{tick}"), ARM(f"m2_t{tick}")
        for meas in MEASURES:
            point, lo, hi = m.paired_difference(meas, a, b, C.BOOT_SEED, C.N_RESAMPLES)
            rows.append({"tick_ms": tick, "measure": meas, "m3": float(pts[meas][m.row[a]]),
                         "m2": float(pts[meas][m.row[b]]), "m3_minus_m2": point, "lower": lo,
                         "higher": hi})
    pd.DataFrame(rows).to_csv(C.OUT / "m3-vs-m2-paired.csv", index=False)

    miss = []
    key = ["seed", "incident"]
    for arm in [a for a in m.names if a.startswith(("sel_med_", "sel_m2_", "sel_reanchor",
                                                   "sel_ramp"))]:
        ni = run.arms[arm].notice_incidents
        hard = ni[(ni.tier == "hard") & (ni.family != "slow_leak")]
        wrong = hard[~hard.anchor_correct.astype(bool)]
        ev = run.arms[arm].notice_events
        ev = ev[(ev.event == "notice") & ev.incident.notna()].copy()
        ev["incident"] = ev.incident.astype(int)
        first = ev.sort_values("at_ns").groupby(key).first()
        unattributed = int((~wrong.noticed.astype(bool)).sum())
        noticed = wrong[wrong.noticed.astype(bool)].set_index(key)
        off = first.reindex(noticed.index)["anchor_offset_ns"].astype(float)
        # An attributed notice is anchored on one of the incident's observations, so its offset
        # from the incident's first observation is never negative.
        assert off.notna().all() and (off >= 0).all()
        assert unattributed + int((off > 0).sum()) == len(wrong)
        miss.append({"arm": arm, "hard_non_leak": len(hard), "not_anchor_correct": len(wrong),
                     "no_notice_attributed": unattributed,
                     "anchored_late": int((off > 0).sum()),
                     "median_late_s": float(off[off > 0].median() / 1e9) if (off > 0).any()
                     else None})
    miss = pd.DataFrame(miss)
    miss.to_csv(C.OUT / "m3-misses.csv", index=False)
    with pd.option_context("display.width", 200):
        print(pd.DataFrame(rows).to_string(index=False, float_format=lambda x: f"{x:+.4f}"))
        print(miss.to_string(index=False))


if __name__ == "__main__":
    main()
