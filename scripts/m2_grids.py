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
}

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
    raise SystemExit(f"unknown stage {name!r}")
