"""M4's leak-loss diagnosis, on the tuning streams only: the leaks one arm notices and another does
not, and what the second arm noticed at the leak's site around that time.

Usage: m4_diagnose.py STAGE ARM_MISSING ARM_NOTICING [WINDOW_S]
       (arm names as in the grid, without the sel_/_privileged spelling)

For each slow-leak incident that ARM_NOTICING notices and ARM_MISSING does not: ARM_NOTICING's first
notice attributed to it (site, anchor time, notice time), and every notice and retirement
ARM_MISSING made at that site from WINDOW_S seconds (default 120) before that notice to 30 s after,
with the incident each was attributed to (tier and family) or "background". Evaluator output on
tuning streams, read as M2 and M3 read it to design; nothing here enters a held-out choice.
"""

import sys

import pandas as pd

import m4_common as C  # noqa: I001 (sets the path for the one below)
from gordian_analysis.load import load_stream_run


def main():
    stage, missing, noticing = sys.argv[1:4]
    window = float(sys.argv[4]) if len(sys.argv) > 4 else 120.0
    run = load_stream_run(C.RUNS / C.run_id(stage))
    a, b = run.arms[C.arm_name(missing)], run.arms[C.arm_name(noticing)]
    ni_a, ni_b = a.notice_incidents, b.notice_incidents
    key = ["seed", "incident"]
    leak_a = ni_a[(ni_a.tier == "hard") & (ni_a.family == "slow_leak")].set_index(key)
    leak_b = ni_b[(ni_b.tier == "hard") & (ni_b.family == "slow_leak")].set_index(key)
    lost = leak_a.index[~leak_a.noticed.astype(bool) & leak_b.noticed.astype(bool)
                        .reindex(leak_a.index).fillna(False)]
    gained = leak_a.index[leak_a.noticed.astype(bool) & ~leak_b.noticed.astype(bool)
                          .reindex(leak_a.index).fillna(True)]
    print(f"{missing} misses {len(lost)} leaks that {noticing} notices; "
          f"notices {len(gained)} that it misses; leaks {len(leak_a)}")
    ev_a, ev_b = a.notice_events, b.notice_events
    kinds = ni_a.set_index(key)[["tier", "family"]]
    for seed, inc in lost:
        first = ev_b[(ev_b.seed == seed) & (ev_b.incident == inc) & (ev_b.event == "notice")]
        first = first.sort_values("at_ns").iloc[0]
        t = first.at_ns
        print(f"\nseed {seed} leak {inc}: first observation "
              f"{leak_a.loc[(seed, inc)].first_observation_at_ns / 1e9:.1f} s; {noticing} notices "
              f"it at site {first.site}, anchor {first.anchor_at_ns / 1e9:.2f} s, at {t / 1e9:.1f} s")
        near = ev_a[(ev_a.seed == seed) & (ev_a.site == first.site)
                    & (ev_a.at_ns >= t - window * 1e9) & (ev_a.at_ns <= t + 30e9)]
        for _, e in near.sort_values("at_ns").iterrows():
            if pd.isna(e.incident):
                what = "background"
            else:
                k = kinds.loc[(seed, int(e.incident))]
                what = f"incident {int(e.incident)} ({k.tier} {k.family if isinstance(k.family, str) else ''})"
            print(f"   {missing}: {e.event:7s} anomaly {e.anomaly} at {e.at_ns / 1e9:7.1f} s, "
                  f"anchor {e.anchor_at_ns / 1e9:7.2f} s -> {what}")


if __name__ == "__main__":
    main()
