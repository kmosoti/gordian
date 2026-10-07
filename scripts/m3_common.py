"""M3, sub-tick support, mutation tests, strict precision: the settings, the criterion, the tuning
rule, the readings and the held-out arms, in one place.

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation). The
criterion is the chief's (docs/lab-queue.md, "## M3"), fixed before any M3 code or run: M2's two
results, the same comparator, margins and background bound, plus strict precision >= 0.67, at
500 ms and 2 s; 100 ms is reported for continuity. It is copied here and not changed. Every
reading of something the brief leaves open is written here before the tuning runs (the tuning
rule) or before the held-out run (the held-out arms), so that the manifests and the analysis
cannot disagree. The measures are the evaluator's (rules N1 to N16); the arithmetic is B2's
(`b2_stats.Measures`), unchanged, as M2 used it.

Run directories live in artifacts/runs/ of this worktree (git-ignored); manifests in
artifacts/runs/_manifests/.
"""

import m2_common as M2

ROOT = M2.ROOT
RUNS = M2.RUNS
MANIFESTS = M2.MANIFESTS
OUT = M2.OUT
BIN = M2.BIN
NS = M2.NS
MS = M2.MS
setting_id = M2.setting_id
load_json = M2.load_json
R6_RUNS = M2.R6_RUNS
TUNING_SEEDS = M2.TUNING_SEEDS  # 10000-10099
HELDOUT_SEEDS = M2.HELDOUT_SEEDS  # 20000-20199
PRIMARY = M2.PRIMARY  # b = 5, rho = 0.7
SEL_POLICY = M2.SEL_POLICY
sel_delay_s = M2.sel_delay_s
arm_name = M2.arm_name
REANCHOR = M2.REANCHOR
N_RESAMPLES = M2.N_RESAMPLES
BOOT_SEED = M2.BOOT_SEED


def run_id(stage):
    b, rho = PRIMARY
    return f"m3-{stage}-{setting_id(b, rho)}"


# ---- the criterion, as the chief fixed it (docs/lab-queue.md, "## M3"; not changed here) ---------

# The comparator is M2's, `ReanchorNoticer` (B2's row: anchor-correct 0.952, leak noticed 0.460,
# 6.82 background notices per stream, strict precision 0.67), "or the B3 row if the chief re-fixes
# it before M3's held-out run". Checked on main immediately before the held-out run; the report
# says what was found.
COMPARATOR = "reanchor"
BACKGROUND_BOUND = 6.82  # notices on background per stream, the medium's point estimate
STRICT_PRECISION_BOUND = 0.67  # the re-anchor's; the medium's point estimate must reach it
RESULT1 = M2.RESULT1  # anchor-correct, margin 0.03, paired lower bound above 0.01
RESULT2 = M2.RESULT2  # leak noticed, margin 0.20, paired lower bound above 0.10
TICKS_MS = (100, 500, 2000)
CRITERION_TICKS_MS = (500, 2000)  # "holds" means result 1 holds at both, under both bounds
CONTINUITY_TICKS_MS = (100,)

# Readings of what the brief leaves open, fixed before any M3 tuning or held-out run:
#
# 1. M2's readings 1 to 3 and 5 (`m2_common`) stand: the paired lower bound is the 5th percentile
#    of the paired cluster bootstrap (10,000 resamples of whole streams, the same counts for
#    every arm); the background bound applies to the point estimate; the comparator's numbers are
#    those of the `reanchor` arm of the same held-out run, and the bounds stay the fixed 6.82 and
#    0.67 whatever it gives.
# 2. "plus strict precision >= 0.67": the medium arm's pooled strict precision (N16, B2's
#    `strict_precision`: anchor- and site-correct notices over all notices, pooled over streams)
#    as a point estimate, >= 0.67. Its interval is reported beside.
# 3. "Holds" for M3: result 1 holds as written (point difference >= 0.03, paired lower bound
#    > 0.01, background <= 6.82, strict precision >= 0.67) at 500 ms AND at 2 s. Result 2 and the
#    100 ms row are reported for continuity under the same arithmetic, with the same bounds shown.
# 4. The medium at each tick length is the configuration this unit's tuning rule selects for that
#    tick length (below); the "sub-tick pruning off" control at a tick length is that same
#    configuration with the sub-tick lookback and the arrivals at event resolution switched off
#    (`burst_subtick_ns = 0`, `burst_every_event = false`), everything else equal.

# ---- the tuning rule (fixed before the first M3 tuning run) ---------------------------------------
#
# Per tick length, on the 100 tuning streams (10000-10099) at b = 5, rho = 0.7, under the selection
# oracle at 16 s and the rung's context, with the comparator (`reanchor`) rerun in every stage on
# the same streams: among the medium configurations of the M3 tuning stages listed in
# `m3_select.STAGES`, keep those with
#
#     background notices per stream <= TUNE_BACKGROUND (5.6, M2's tuning budget, the same margin
#     below 6.82) and strict precision >= TUNE_STRICT (0.70, a margin of 0.03 above the bound, so
#     that a held-out set a little less precise than the tuning set does not cross it),
#
# and choose the one that maximizes
#
#     min( (AC - AC_ref) / 0.03 , (LN - LN_ref) / 0.20 )
#
# (M2's score: AC the hard non-leak anchor-correct share, LN the leak noticed share, _ref the
# comparator's). Ties go to the fewer background notices, then the higher strict precision, then
# the earlier stage, then the arm name. If no configuration at a tick length meets both tuning
# bounds, choose, among those within the background budget, the one that maximizes
# min(z1, z2, (SP - TUNE_STRICT) / 0.03), and the selection says so. Structural choices (sub-tick
# lookback on or off, arrivals at event resolution, the cluster merge, the emitter's refractory
# period) are made by the same rule, so an element is in the frozen graph only if it earned its
# place on the tuning streams.
TUNE_BACKGROUND = 5.6
TUNE_STRICT = 0.70


def selected():
    return load_json(OUT / "m3-selected.json")


def m2_selected():
    return load_json(OUT / "m2-selected.json")


def subtick_off(noticer):
    """The labelled control: the same medium with the sub-tick support switched off."""
    return dict(noticer, burst_subtick_ns=0, burst_every_event=False)


# ---- the held-out run (written at the freeze, before it runs) ---------------------------------------
#
# One run, the 200 held-out streams (20000-20199), b = 5, rho = 0.7, the selection oracle at 16 s
# with the rung's context, as B1's, B2's and M2's tables:
# - the comparator rows, rerun: the rung at z = 3 and z = 2 and the re-anchor;
# - the frozen M3 medium at each tick length (m3-selected.json);
# - at each tick length, the control: the same medium with the sub-tick pruning off;
# - at each tick length, M2's frozen medium (m2-selected.json), for continuity with M2's rows;
# - B3's two composed rows (added on the chief's instruction before the held-out run, reported
#   beside and never in the criterion): the ramp and the split over the re-anchor, and the ramp
#   over the re-anchor, spelled as B3's held-out manifest spells them (B3_ROWS);
# - at each tick length, labelled sensitivity rows (nothing is chosen from them): the frozen M3
#   medium with the cluster merge off (if it is on), with arrivals at event resolution off (if
#   on), and with the sub-tick lookback off but arrivals at event resolution kept (if both on).


def heldout(stage):
    """(arms, seeds, run_seed, experiment) of the held-out run; arms are (name, noticer)."""
    if stage != "heldout":
        raise SystemExit(f"unknown held-out stage {stage!r}")
    sel = selected()["ticks"]
    m2 = m2_selected()["ticks"]
    arms = [("rung_z3", {"noticer": "rung", "notice_z": 3.0}),
            ("rung_z2", {"noticer": "rung", "notice_z": 2.0}),
            ("reanchor", dict(REANCHOR))]
    arms += [(name, dict(n)) for name, n in B3_ROWS.items()]
    for tick in TICKS_MS:
        frozen = sel[str(tick)]["noticer"]
        arms.append((f"med_t{tick}", frozen))
        arms.append((f"med_t{tick}_subtick_off", subtick_off(frozen)))
        arms.append((f"m2_t{tick}", m2[str(tick)]["noticer"]))
        if frozen.get("merge_window_ns", 0):
            arms.append((f"med_t{tick}_merge_off", dict(frozen, merge_window_ns=0)))
        if frozen.get("burst_every_event", False):
            arms.append((f"med_t{tick}_every_off", dict(frozen, burst_every_event=False)))
            if frozen.get("burst_subtick_ns", 0):
                arms.append((f"med_t{tick}_cut_off", dict(frozen, burst_subtick_ns=0)))
    return arms, HELDOUT_SEEDS, 13_900, "exploration-m3-heldout"


# B3's composed noticers, copied from B3's held-out manifest
# (/home/user/gordian/artifacts/runs/b3/_manifests/b3-heldout-b5-rho0.7.json, arms
# sel_ramp_split_over_re2_privileged and sel_ramp_over_re2_privileged).
B3_RAMP = {"gap_ns": 1_600_000_000, "max_step": 10, "max_drop": 4, "min_readings": 5,
           "min_rise": 15}
B3_ROWS = {
    "ramp_split_over_re2": {"noticer": "composed", "base": dict(REANCHOR), "ramp": dict(B3_RAMP),
                            "split": {"gap_ns": 3_000_000_000, "min_burst": 3}},
    "ramp_over_re2": {"noticer": "composed", "base": dict(REANCHOR), "ramp": dict(B3_RAMP)},
}
