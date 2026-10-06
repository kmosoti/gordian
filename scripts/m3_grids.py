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
    if name == "tune-b":
        # tune-a: the sub-tick lookback with arrivals at event resolution recovers 5 incidents at
        # 500 ms (0.955 -> 0.980) and 7 at 2 s (0.869 -> 0.905); the cluster merge brings strict
        # precision from ~0.50 to ~0.70 at 100 and 500 ms for about one or two incidents of
        # anchoring; a long emitter refractory period costs anchoring (a stray's notice blocks
        # the incident's). At 2 s every remaining miss is a compound incident whose onset is two
        # kinds (an error rate and a message) at its site, which M2's 2 s graph (three kinds, no
        # confirmation: a latch cannot hold for less than a 2 s tick) cannot see, and most notices
        # that are not strictly correct come from the ramp path, anchored on error-rate readings,
        # repeating. Here: the confirmation read in event time (an alarm at a confirming service
        # within 50, 100 or 200 ms of the burst, the burst first or not; any other service or
        # dependents) beside M2's latch; the cluster merge at 0, 50, 100 ms; the ramp emitter's
        # refractory period M2's or 30 s; at 2 s, two kinds confirmed in event time (and three
        # unconfirmed) beside M2's three kinds, and a slower ramp integrator (16 s, threshold 7:
        # about ten ticks of readings in a row) beside M2's.
        for tick in C.TICKS_MS:
            base = dict(m2_frozen(tick), burst_subtick_ns=window_ns(m2_frozen(tick)),
                        burst_every_event=True, burst_lookback_ns=tick * MS)
            t = f"t{tick}"
            bursts = {}
            if tick == 2000:
                bursts["k3"] = base
                two = dict(base, burst_n=2, burst_window_ns=25 * MS, burst_subtick_ns=25 * MS,
                           burst3_window_ns=30 * MS)
            else:
                bursts["latch"] = base
                two = base
            for conf in ("all", "dependents"):
                for cw in (50, 100, 200):
                    for lead in (True, False):
                        bursts[f"{conf[:3]}{cw}{'L' if lead else ''}"] = dict(
                            two, burst_confirm=conf, confirm_window_ns=cw * MS, confirm_lead=lead)
            merges = (0, 100) if tick == 2000 else (0, 50, 100)
            ramps = {"r": {}}
            if tick == 2000:
                ramps["s"] = {"ramp_tau_ns": 16 * S, "ramp_threshold": 7.0}
            for bname, b in bursts.items():
                for merge in merges:
                    for rname, rover in ramps.items():
                        for rr in (None, 30):
                            arm = dict(b, merge_window_ns=merge * MS, **rover)
                            if rr is not None:
                                arm["ramp_refractory_ns"] = rr * S
                            label = f"{bname}_m{merge}_{rname}{rr or 0}_{t}"
                            arms.append((label, arm))
        return arms, seeds, 13_200, "exploration-m3-tuning"
    raise SystemExit(f"unknown stage {name!r}")
