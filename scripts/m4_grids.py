"""M4's tuning grids, on the tuning streams (10000-10099) only. Each stage is one run.

Exploration of the design space, recorded as run. Nothing here is chosen from a held-out stream.
Every stage reruns the public rows on the same streams: the re-anchor, and B3's ramp + split over
the re-anchor (the criterion's comparator) and ramp over the re-anchor, spelled as B3's held-out
manifest spells them. Configurations start from M3's frozen medium at each tick length
(`m3-selected.json`). Device switches: `merge` (the cluster merge), `confirm_in_event_time` (off:
M2's latch, with the frozen medium's `confirm_hold_ns` and `confirm_delay_ticks`), `ramp_inhibit`
with `ramp_inhibit_form` (`any`, M3's; `not_ramp_noticed`, M4's).
"""

import m4_common as C

MS = C.MS
S = C.NS
RAMP_REFRACTORY_ALT_S = 30  # M3's grids' value; the frozen 500 ms medium has it, 100 ms and 2 s not
MERGE_ON_100MS_NS = 10 * MS  # M3's 100 ms medium has no merge; "on" there is M3's smallest window
INHIBITS = {"0": (False, "any"), "a": (True, "any"), "n": (True, "not_ramp_noticed")}


def m3_frozen(tick):
    return dict(C.m3_selected()["ticks"][str(tick)]["noticer"])


def public():
    return [("reanchor", dict(C.REANCHOR))] + [(n, dict(r)) for n, r in C.B3_ROWS.items()]


def with_refractory(noticer, seconds):
    """`noticer` with the ramp emitter's refractory period `seconds` (None: the emitter's
    `refractory_ns`, the key absent)."""
    out = {k: v for k, v in noticer.items() if k != "ramp_refractory_ns"}
    if seconds is not None:
        out["ramp_refractory_ns"] = seconds * S
    return out


def devices(base, merge_on, confirm_event, inhibit):
    """`base` with the three device switches set (the inhibit as a key of INHIBITS)."""
    out = dict(base)
    if merge_on:
        out["merge"] = True
        if not out.get("merge_window_ns", 0):
            out["merge_window_ns"] = MERGE_ON_100MS_NS
    else:
        out["merge"] = False
    out["confirm_in_event_time"] = bool(confirm_event)
    on, form = INHIBITS[inhibit]
    out["ramp_inhibit"] = on
    out["ramp_inhibit_form"] = form
    return out


def stage(name):
    """(arms, seeds, run_seed, experiment) of tuning stage `name`; arms are (name, noticer)."""
    seeds = C.TUNING_SEEDS
    arms = public()
    if name == "tune-a":
        # The device frontier at every tick length: M3's frozen medium with each device switched
        # on and off (the cluster merge; the confirmation in event time against M2's latch; the
        # ramp inhibit off, in M3's form and in M4's), crossed with the ramp emitter's refractory
        # period (the frozen medium's and the other of M3's two values: 30 s at 500 ms is the
        # frozen one, at 100 ms and 2 s the frozen one is the emitter's own 1 s), the lever M3's
        # tuning tables show trading leak noticing for precision beside the inhibit. 24
        # configurations per tick length.
        for tick in C.TICKS_MS:
            frozen = m3_frozen(tick)
            rr_frozen = frozen.get("ramp_refractory_ns")
            rr_frozen = None if rr_frozen is None else rr_frozen // S
            rr_alt = None if rr_frozen is not None else RAMP_REFRACTORY_ALT_S
            for merge_on in (True, False):
                for confirm_event in (True, False):
                    for inh in INHIBITS:
                        for rr in (rr_frozen, rr_alt):
                            arm = with_refractory(
                                devices(frozen, merge_on, confirm_event, inh), rr)
                            label = (f"m{int(merge_on)}_c{'e' if confirm_event else 'l'}"
                                     f"_i{inh}_r{rr or 0}_t{tick}")
                            arms.append((label, arm))
        return arms, seeds, 14_100, "exploration-m4-tuning"
    if name == "tune-b":
        # tune-a (m4-tuning-tune-a.csv; m4-leak-diagnosis-tune-a.txt): at 500 ms M4's inhibit form
        # recovers the leak (44 of 44 at the emitter's 1 s ramp refractory, against 41 for M3's
        # form): the three leaks M3's form loses are each preceded by a ramp notice anchored just
        # outside the leak (attributed to background), whose anomaly the leak's own alarms keep
        # open, so M3's form silences the ramp's second, correct notice. But at 500 ms the form
        # buys almost no precision over no inhibit (0.722 against 0.721; M3's form 0.789): M3's
        # form earned its precision by silencing the ramp after its own notice. The ramp
        # refractory of 30 s buys about 0.06 of precision and loses one leak (seed 10022), by
        # blocking the same corrective second notice 12 s after a mis-anchored one. At 100 ms and
        # 2 s every form with the inhibit loses 7 leaks to anomalies open for minutes at a service
        # whose alarms keep a 15 s hold open (those frozen media's hold; 500 ms has 6 s).
        # Here, at 500 ms: the existing grids (the confirmation in event time at 50 and 100 ms from
        # the dependents and 50 ms from all services; the cluster merge at 0, 10, 20, 30, 50 ms)
        # crossed with the inhibit off / M3's / M4's and the ramp refractory at 1, 5, 10, 15, 20
        # and 30 s (M3's grids had 1 and 30), hold 6 s. At 100 ms and 2 s, for continuity, at the
        # frozen confirmation and merge: hold 6 or 15 s, the inhibit off / M3's / M4's, and the
        # same ramp refractories.
        refractories = (None, 5, 10, 15, 20, 30)
        frozen = m3_frozen(500)
        confirms = {"dep50L": ("dependents", 50), "dep100L": ("dependents", 100),
                    "all50L": ("all", 50)}
        for cname, (conf, cw) in confirms.items():
            for merge in (0, 10, 20, 30, 50):
                for inh in INHIBITS:
                    for rr in refractories:
                        b = dict(frozen, burst_confirm=conf, confirm_window_ns=cw * MS,
                                 confirm_lead=True, merge_window_ns=merge * MS, hold_ns=6 * S)
                        arm = with_refractory(devices(b, merge > 0, True, inh), rr)
                        arms.append((f"{cname}_m{merge}_i{inh}_r{rr or 1}_t500", arm))
        for tick in (100, 2000):
            frozen = m3_frozen(tick)
            for hold in (6, 15):
                for inh in INHIBITS:
                    for rr in refractories:
                        b = dict(frozen, hold_ns=hold * S)
                        on = bool(b.get("merge_window_ns", 0))
                        arm = with_refractory(devices(b, on, True, inh), rr)
                        arms.append((f"h{hold}_i{inh}_r{rr or 1}_t{tick}", arm))
        return arms, seeds, 14_200, "exploration-m4-tuning"
    raise SystemExit(f"unknown stage {name!r}")
