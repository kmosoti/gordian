"""M3's tuning grids, on the tuning streams (10000-10099) only. Each stage is one run.

Exploration of the design space, recorded as run. Nothing here is chosen from a held-out stream.
Every stage reruns the comparator rows (the rung at z = 2 and 3, the re-anchor) on the same
streams. Configurations start from M2's frozen medium at each tick length (`m2-selected.json`).
"""

import m3_common as C

MS = C.MS
S = C.NS


def m2_frozen(tick):
    return dict(C.m2_selected()["ticks"][str(tick)]["noticer"])


def rung(z):
    return {"noticer": "rung", "notice_z": z}


def window_ns(base):
    """The burst's own window (the coincidence the sub-tick lookback sits on)."""
    return base["burst_window_ns"]


def stage(name):
    """(arms, seeds, run_seed, experiment) of tuning stage `name`; arms are (name, noticer)."""
    seeds = C.TUNING_SEEDS
    arms = [("rung_z2", rung(2.0)), ("rung_z3", rung(3.0)), ("reanchor", dict(C.REANCHOR))]
    if name == "tune-a":
        # First look, at each tick length, from M2's frozen medium: the sub-tick lookback (the
        # burst's window, or 50 ms) with and without arrivals at event resolution; a tick lookback
        # of one tick so the cut can reach across an edge; then on that, the cluster merge (50, 100,
        # 200 ms) and a longer emitter refractory period (1, 10, 30 s), the two levers the tuning
        # streams' notice files point at for strict precision (M2's frozen media on them: most
        # notices that are not strictly correct are a second notice of one incident at another
        # service within ~50 ms of its start, at 100 ms and 500 ms, and repeated notices of a
        # continuing incident at its site, at 2 s).
        for tick in C.TICKS_MS:
            base = m2_frozen(tick)
            w = window_ns(base)
            t = f"t{tick}"
            arms.append((f"m2_{t}", base))
            for sub_name, sub in (("w", w), ("50", 50 * MS)):
                for every in (False, True):
                    arms.append((f"sub{sub_name}_e{int(every)}_{t}",
                                 dict(base, burst_subtick_ns=sub, burst_every_event=every)))
            cut = dict(base, burst_subtick_ns=w, burst_every_event=True,
                       burst_lookback_ns=tick * MS)
            arms.append((f"subw_e1_lb1_{t}", cut))
            for refr in (10, 30):
                arms.append((f"subw_e1_lb1_r{refr}_{t}", dict(cut, refractory_ns=refr * S)))
            for merge in (50, 100, 200):
                for refr in (1, 10, 30):
                    arms.append((f"subw_e1_lb1_m{merge}_r{refr}_{t}",
                                 dict(cut, merge_window_ns=merge * MS, refractory_ns=refr * S)))
        return arms, seeds, 13_100, "exploration-m3-tuning"
    raise SystemExit(f"unknown stage {name!r}")
