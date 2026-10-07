"""M4, the anchoring-against-precision frontier: the settings, the criterion, the tuning rule, the
readings and the held-out arms, in one place.

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation). The
criterion is the chief's (docs/lab-queue.md, "## M4"), fixed before any M4 code or run, copied here
and not changed. Every reading of something the brief leaves open is written here before the first
tuning run (the criterion's readings and the tuning rule) or at the freeze, before the held-out run
(the held-out arms), so that the manifests and the analysis cannot disagree. The measures are the
evaluator's (rules N1 to N16); the arithmetic is B2's (`b2_stats.Measures`), unchanged, as M2 and
M3 used it.

Run directories live in artifacts/runs/ of this worktree (git-ignored); manifests in
artifacts/runs/_manifests/.
"""

import m3_common as M3

ROOT = M3.ROOT
RUNS = M3.RUNS
MANIFESTS = M3.MANIFESTS
OUT = M3.OUT
BIN = M3.BIN
NS = M3.NS
MS = M3.MS
setting_id = M3.setting_id
load_json = M3.load_json
R6_RUNS = M3.R6_RUNS
TUNING_SEEDS = M3.TUNING_SEEDS  # 10000-10099
HELDOUT_SEEDS = M3.HELDOUT_SEEDS  # 20000-20199
PRIMARY = M3.PRIMARY  # b = 5, rho = 0.7
SEL_POLICY = M3.SEL_POLICY  # the selection oracle with the rung's own context
sel_delay_s = M3.sel_delay_s  # R5's 16 s
arm_name = M3.arm_name  # sel_<name>_privileged
REANCHOR = M3.REANCHOR
B3_ROWS = M3.B3_ROWS  # ramp + split over the re-anchor, and ramp over the re-anchor, as B3 spells them
N_RESAMPLES = M3.N_RESAMPLES  # 10,000 resamples of whole streams
BOOT_SEED = M3.BOOT_SEED


def run_id(stage):
    b, rho = PRIMARY
    return f"m4-{stage}-{setting_id(b, rho)}"


# ---- the criterion, as the chief fixed it (docs/lab-queue.md, "## M4"; not changed here) ---------
#
# "At 500 ms on the 200 held-out streams, against ramp + split over the re-anchor (anchor-correct
# 0.973, leak noticed 0.986, strict precision 0.693, background 6.24), the frozen medium meets all
# four: anchor-correct >= 0.963 with the paired lower bound above -0.03; leak noticed >= 0.976 with
# the paired lower bound above -0.03; strict precision >= 0.693; background notices per stream
# <= 6.24. Conjunctive. ... Cost per stream and leak anchor-correct are reported beside, never in
# the criterion."
COMPARATOR = "ramp_split_over_re2"  # B3's composed row, spelled as B3's held-out manifest spells it
COMPARATOR_B3 = {"hard_anchor_correct_share": 0.973, "leak_noticed_share": 0.986,
                 "strict_precision": 0.693, "notices_on_background_per_stream": 6.24}
CRITERION_TICK_MS = 500
# The six conditions of the four parts. `at_least` / `at_most` apply to the medium's pooled point
# estimate; `lower_bound_above` to the paired lower bound of (medium minus comparator).
CONDITIONS = [
    {"part": "anchoring", "condition": "point", "measure": "hard_anchor_correct_share",
     "at_least": 0.963},
    {"part": "anchoring", "condition": "paired_lower_bound", "measure": "hard_anchor_correct_share",
     "lower_bound_above": -0.03},
    {"part": "leak", "condition": "point", "measure": "leak_noticed_share", "at_least": 0.976},
    {"part": "leak", "condition": "paired_lower_bound", "measure": "leak_noticed_share",
     "lower_bound_above": -0.03},
    {"part": "precision", "condition": "point", "measure": "strict_precision", "at_least": 0.693},
    {"part": "background", "condition": "point", "measure": "notices_on_background_per_stream",
     "at_most": 6.24},
]
BACKGROUND_BOUND = 6.24
PAIRED_MEASURES = ("hard_anchor_correct_share", "leak_noticed_share")
BESIDE = ("leak_anchor_correct_share", "cost_s_per_stream", "calls_per_stream",
          "substrate_s_per_stream")
TICKS_MS = (100, 500, 2000)

# Readings of what the brief leaves open, fixed before any M4 tuning or held-out run:
#
# 1. "the paired lower bound": the 5th percentile of the paired cluster bootstrap of (medium minus
#    comparator), 90% equal-tailed, 10,000 resamples of whole streams with the same stream counts
#    for every arm (B2's `Measures`, seed BOOT_SEED), as M2 and M3 read it; "above -0.03" is
#    strict (> -0.03). "anchor-correct" is the hard non-leak anchor-correct share
#    (`hard_anchor_correct_share`), "leak noticed" the slow-leak noticed share
#    (`leak_noticed_share`), "strict precision" B2's pooled strict precision (N16),
#    "background notices per stream" `notices_on_background_per_stream`.
# 2. The thresholds (0.963, 0.976, 0.693, 6.24) are fixed numbers and compare with the medium's
#    unrounded pooled point estimates, inclusively (>=, <=). They do not move with the comparator
#    rerun.
# 3. The comparator in the paired bounds is the `ramp_split_over_re2` arm rerun in the same
#    held-out run (B3's spelling, `B3_ROWS`), so the differences are paired on the same streams. It
#    should reproduce B3's 0.973 / 0.986 / 0.693 / 6.24; whatever it gives is reported, and the
#    thresholds stay as fixed.
# 4. "The frozen medium" is the configuration the tuning rule below selects at 500 ms. Conjunctive:
#    M4 holds if and only if all six conditions hold for it.
# 5. 100 ms and 2 s: the same rule selects a medium at each, reported on the tuning streams and as
#    labelled continuity rows on the held-out streams, never in the criterion.

# ---- the tuning rule (fixed before the first M4 tuning run) ---------------------------------------
#
# Per tick length, on the 100 tuning streams (10000-10099) at b = 5, rho = 0.7, under the selection
# oracle at 16 s and the rung's context, with the comparator (`ramp_split_over_re2`) rerun in every
# stage on the same streams. Candidates: every medium configuration at that tick length in the M4
# tuning stages (`m4_select.STAGES`). For each, the criterion's six conditions are evaluated on the
# tuning streams exactly as reading 1 to 3 evaluate them on the held-out streams (the same
# thresholds, the paired lower bounds against the comparator of the same stage, B2's bootstrap
# with BOOT_SEED). Then:
#
#   (a) The objective is the criterion's own conjunction: a configuration that meets all six
#       conditions beats one that does not.
#   (b) Ties (among configurations that meet all six) break on the criterion's measures in the
#       order: the higher anchor-correct share, then the higher leak noticed share, then the higher
#       strict precision. Background is a bound only (condition 6), never a tie-break. A tie that
#       remains after the three measures goes to the earlier stage, then the arm name (not a
#       preference, only a unique answer; the selection lists the tied configurations).
#   (c) If no configuration at a tick length meets all six: among those within the background
#       bound (<= 6.24), the one with the largest graded conjunction
#           g = min( (AC - 0.963), (LB_AC + 0.03), (LN - 0.976), (LB_LN + 0.03), (SP - 0.693) ) / 0.03
#       (the smallest slack of the five graded conditions, in units of the criterion's 0.03),
#       ties as in (b); the selection says it fell back.
#
# The device switches (the cluster merge, the confirmation in event time, the ramp inhibit and its
# form) are chosen by the same rule as every other parameter, so a device is in the frozen graph
# only if it earned its place on the tuning streams under the criterion's own conjunction. No
# margin is added to any threshold (M3's rule had margins on its bounds; the brief says the
# objective is the criterion's own conjunction). The paired lower bounds on 100 streams are wider
# than on 200, which makes (a) stricter on the tuning streams than on the held-out ones; that is
# stated, not corrected.
GRADE_UNIT = 0.03


def selected():
    return load_json(OUT / "m4-selected.json")


def m3_selected():
    return load_json(OUT / "m3-selected.json")


def m2_selected():
    return load_json(OUT / "m2-selected.json")
