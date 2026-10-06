"""M2's tuning grids, on the tuning streams (10000-10099) only. Each stage is one run.

Exploration of the design space, recorded as run. Nothing here is chosen from a held-out stream.
"""

import m2_common as C

MS = C.MS
S = C.NS

# The graph's starting point (`MediumParams::default()` in graph.rs), as manifest fields.
BASE = {
    "tick_ns": 100 * MS,
    "abnormal_only": False,
    "onset_tau_ns": 300 * MS,
    "onset_threshold": 3.0,
    "dependent_weight": 1.0,
    "direct_dependents": False,
    "lookback_ns": 200 * MS,
    "refractory_ns": 6 * S,
    "coincidence": "off",
    "coincidence_window_ns": 300 * MS,
    "ramp": True,
    "ramp_jump": 8.0,
    "ramp_penalty": 3.0,
    "ramp_tau_ns": 3 * S,
    "ramp_threshold": 2.0,
    "ramp_lookback_ns": 3 * S,
    "hold_ns": 6 * S,
    "rhythms": False,
    "onset": True,
    "burst": False,
    "burst_n": 2,
    "burst_window_ns": 25 * MS,
    "burst_lookback_ns": 0,
    "propagation": False,
    "burst_confirm": "none",
    "confirm_hold_ns": 300 * MS,
    "burst3_window_ns": 0,
    "confirm_delay_ticks": 2,
}

# The ramp path as tune-b left it (leak noticed 1.000 at 0.12 background notices per stream).
RAMP = {"ramp": True, "ramp_jump": 10.0, "ramp_penalty": 3.0, "ramp_tau_ns": 4 * S,
        "ramp_threshold": 3.0, "ramp_lookback_ns": 10 * S}

# An onset threshold no burst reaches: the onset path is there and never fires (ramp-only arms).
NEVER = 1.0e6


def medium(**over):
    p = dict(BASE)
    for k, v in over.items():
        if k not in p:
            raise KeyError(k)
        p[k] = v
    return {"noticer": "medium", **p}


def rung(z):
    return {"noticer": "rung", "notice_z": z}


def tick_tag(ns):
    return f"t{ns // MS}"


def stage(name):
    """(arms, seeds, run_seed, experiment) of tuning stage `name`; arms are (name, noticer)."""
    seeds = C.TUNING_SEEDS
    arms = [("rung_z2", rung(2.0)), ("rung_z3", rung(3.0))]
    if name != "tune-a":
        arms.append(("reanchor", dict(C.REANCHOR)))
    if name == "tune-a":
        # First look at 100 ms: the onset path alone (threshold x time constant x dependents),
        # two propagation forms, and the ramp path alone.
        for th in (2.0, 3.0, 4.0):
            for tau in (150, 300, 600):
                for w in (0.0, 1.0):
                    arms.append((f"on_th{th:g}_tau{tau}_w{w:g}", medium(
                        ramp=False, onset_threshold=th, onset_tau_ns=tau * MS, dependent_weight=w)))
        for form in ("ordered", "sliding"):
            arms.append((f"on_th4_tau300_w1_{form}", medium(
                ramp=False, onset_threshold=4.0, coincidence=form)))
        for jump in (6.0, 10.0):
            for tau in (2, 4):
                for th in (2.0, 3.0):
                    arms.append((f"ramp_j{jump:g}_tau{tau}_th{th:g}", medium(
                        onset_threshold=NEVER, ramp_jump=jump, ramp_tau_ns=tau * S,
                        ramp_threshold=th)))
        return arms, seeds, 12_100, "exploration-m2-tuning"
    if name == "tune-b":
        # After tune-a: a notice is no longer suppressed when a live anomaly merely holds its
        # anchor (the rung's own mis-anchoring had come back through that rule). The onset path
        # at 100 ms around threshold 2 to 3, dependents (none, direct at 1/2, all at 1/2), and the
        # emitter's refractory period; the ramp path's lookback.
        for th in (2.0, 2.5, 3.0):
            for tau in (150, 300):
                for dep, w, direct in (("w0", 0.0, False), ("d05", 0.5, True), ("a05", 0.5, False)):
                    for refr in (1, 3, 6):
                        arms.append((f"on_th{th:g}_tau{tau}_{dep}_r{refr}", medium(
                            ramp=False, onset_threshold=th, onset_tau_ns=tau * MS,
                            dependent_weight=w, direct_dependents=direct, refractory_ns=refr * S)))
        for jump in (6.0, 10.0):
            for lb in (3, 6, 10):
                arms.append((f"ramp_j{jump:g}_tau4_th3_lb{lb}", medium(
                    onset_threshold=NEVER, ramp_jump=jump, ramp_tau_ns=4 * S, ramp_threshold=3.0,
                    ramp_lookback_ns=lb * S)))
        return arms, seeds, 12_200, "exploration-m2-tuning"
    if name == "tune-c":
        # After the re-fix (comparator: reanchor, bound 6.82): tune-b showed the medium's onset
        # misses and the re-anchor's misses disjoint on the tuning streams, the medium's being
        # strays shortly before a burst, and threshold 2 too noisy (24 background notices). The
        # burst cells: abnormal observations of distinct kinds at one service within a window read
        # from the offsets (ordered coincidence), alone or beside the onset integrator at
        # threshold 3, at every tick length; the sliding form beside it.
        for tick in (100, 500, 2000):
            for win in (10, 20, 30, 50):
                for onset in ("off", "th3"):
                    over = dict(RAMP, tick_ns=tick * MS, burst=True, coincidence="ordered",
                                burst_window_ns=win * MS, refractory_ns=1 * S,
                                dependent_weight=0.0, onset_tau_ns=150 * MS, onset_threshold=3.0,
                                onset=(onset == "th3"))
                    arms.append((f"b2_w{win}_{onset}_t{tick}", medium(**over)))
            over = dict(RAMP, tick_ns=tick * MS, burst=True, coincidence="ordered", burst_n=3,
                        burst_window_ns=30 * MS, refractory_ns=1 * S, onset=False)
            arms.append((f"b3_w30_off_t{tick}", medium(**over)))
            over = dict(RAMP, tick_ns=tick * MS, burst=True, coincidence="sliding",
                        burst_window_ns=25 * MS, refractory_ns=1 * S, onset=False)
            arms.append((f"b2_sliding_off_t{tick}", medium(**over)))
        return arms, seeds, 12_300, "exploration-m2-tuning"
    if name == "tune-d":
        # tune-c: two kinds within 20 ms at one service notices 0.995 of the hard non-leak
        # incidents anchor-correct at 100 ms, but at 14 background notices per stream; on the
        # tuning streams' public observations, what separates an onset from a background pair is
        # other abnormal activity around it. So: the two-kind burst gated by a confirming alarm
        # (at a dependent, or anywhere else) held for a while, and three kinds without one.
        for tick in (100, 500, 2000):
            holds = (200, 300, 500) if tick == 100 else (2 * tick, 3 * tick)
            for win in ((20, 30) if tick == 100 else (20,)):
                for confirm in ("dependents", "all"):
                    for hold in holds:
                        for b3 in ((0, 20, 30) if tick == 100 else (0, 20)):
                            over = dict(RAMP, tick_ns=tick * MS, onset=False, burst=True,
                                        coincidence="ordered", burst_window_ns=win * MS,
                                        burst_confirm=confirm, confirm_hold_ns=hold * MS,
                                        burst3_window_ns=b3 * MS, refractory_ns=1 * S)
                            arms.append((f"b2_w{win}_{confirm[:3]}_h{hold}_b3w{b3}_t{tick}",
                                         medium(**over)))
        return arms, seeds, 12_400, "exploration-m2-tuning"
    if name == "tune-e":
        # tune-d: at 100 ms the two-kind burst confirmed by any other service's alarm within a
        # 200 ms hold, with three kinds unconfirmed, is 0.980 anchor-correct at 5.58 background
        # notices; confirmed by dependents only, 0.975 at 2.1. The rest are two-kind pairs (an
        # error rate and a catalogue message) that look like the background's commonest pair.
        # Here: narrower and wider windows and the refractory period at 100 ms; the binned form
        # (the 10 s rhythm's bins) in place of the ordered one; at 500 ms and 2 s a confirmation
        # read one tick late and a density-only ramp (no penalty for jumps), since a 2 s tick
        # sums two readings of a ramp.
        conf = dict(RAMP, onset=False, burst=True, coincidence="ordered", burst_confirm="all",
                    confirm_hold_ns=200 * MS, burst3_window_ns=20 * MS, refractory_ns=1 * S)
        for win in (15, 20, 25):
            for b3 in (15, 20, 25):
                for refr in (0.5, 1, 3):
                    arms.append((f"all_w{win}_b3w{b3}_r{refr:g}_t100", medium(
                        **dict(conf, burst_window_ns=win * MS, burst3_window_ns=b3 * MS,
                               refractory_ns=int(refr * S)))))
        for confirm in ("dependents", "all"):
            arms.append((f"binned_{confirm[:3]}_t100", medium(**dict(
                conf, coincidence="binned", rhythms=True, burst_confirm=confirm,
                burst_window_ns=100 * MS, burst3_window_ns=100 * MS))))
        for tick in (500, 2000):
            for confirm in ("dependents", "all"):
                for delay in (1, 2):
                    for b3 in (20, 30):
                        arms.append((f"{confirm[:3]}_d{delay}_b3w{b3}_t{tick}", medium(**dict(
                            conf, tick_ns=tick * MS, burst_confirm=confirm,
                            confirm_delay_ticks=delay, confirm_hold_ns=delay * tick * MS,
                            burst3_window_ns=b3 * MS))))
            for pen in (0.0, 3.0):
                for th in (2.0, 3.0, 4.0):
                    arms.append((f"ramp_p{pen:g}_th{th:g}_t{tick}", medium(
                        onset=False, tick_ns=tick * MS, ramp=True, ramp_jump=10.0,
                        ramp_penalty=pen, ramp_tau_ns=4 * S, ramp_threshold=th,
                        ramp_lookback_ns=10 * S, refractory_ns=1 * S)))
        return arms, seeds, 12_500, "exploration-m2-tuning"
    raise SystemExit(f"unknown stage {name!r}")
