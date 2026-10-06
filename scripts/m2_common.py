"""M2, the medium as a noticer: the settings, the arms, the readings and the constants, in one place.

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation). The
criterion is the chief's (docs/lab-queue.md, "## M2"), fixed before any M2 code or run: its
comparator, margins, bound, settings and seeds are copied here and not changed. Every reading of
something the brief leaves open is stated here, before any held-out run, so that the manifests and
the analysis cannot disagree. The measures are the evaluator's (rules N1 to N12 of
`crates/gordian-stream-eval/RULES.md`); the arithmetic is B1's (`b1_stats.py`), reused unchanged.

Run directories live in artifacts/runs/ of this worktree (git-ignored); manifests in
artifacts/runs/_manifests/.
"""

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "experiments" / "exploration" / "scripts"
sys.path.insert(0, str(SCRIPTS))
sys.path.insert(0, str(ROOT / "analysis"))

import b1_common as B1  # noqa: E402
import b2_common  # noqa: E402,F401  (B2's scripts, beside this file)

RUNS = B1.RUNS
MANIFESTS = B1.MANIFESTS
OUT = B1.OUT
BIN = B1.BIN
NS = B1.NS
MS = NS // 1000
setting_id = B1.setting_id
load_json = B1.load_json
R6_RUNS = B1.R6_RUNS

TUNING_SEEDS = B1.TUNING_SEEDS  # 10000-10099
HELDOUT_SEEDS = B1.HELDOUT_SEEDS  # 20000-20199
PRIMARY = B1.PRIMARY  # b = 5, rho = 0.7

# ---- the arm every noticer is evaluated under (B1's, unchanged) -------------------------------

SEL_POLICY = B1.SEL_POLICY  # oracle_selection, the rung's own context


def sel_delay_s():
    return B1.sel_delay_s()  # R5's 16 s


def arm_name(name):
    return B1.arm_name(name)  # sel_<name>_privileged


def run_id(stage):
    b, rho = PRIMARY
    return f"m2-{stage}-{setting_id(b, rho)}"


# ---- the criterion, as the chief fixed it (docs/lab-queue.md, "## M2"; not changed here) ----------

# RE-FIXED BY THE CHIEF, 2026-10-06, before any M2 held-out run (docs/lab-queue.md, "## M2", under
# B2's acceptance clause; Lab 1 was told directly while tuning, after two tuning stages): the
# comparator is B2's `ReanchorNoticer` (anchor-correct 0.952 [0.932, 0.970], leak noticed 0.460,
# 6.82 background notices per stream), no longer `RungNoticer` z = 2 (0.914, 0.460, 9.03). Margins,
# settings, seeds and the tick sweep are unchanged. The first two tuning stages (tune-a, tune-b
# grids) were designed under the old budget; their tables are kept as run.
COMPARATOR = "reanchor"  # rerun in the held-out run beside the rung rows, so the table is one run
COMPARATOR_B2 = {"hard_anchor_correct_share": 0.952, "leak_noticed_share": 0.460,
                 "notices_on_background_per_stream": 6.82}
OLD_COMPARATOR = "rung_z2"  # the comparator first fixed, reported beside
BACKGROUND_BOUND = 6.82  # the medium's notices on background per stream may not exceed this
RESULT1 = {"measure": "hard_anchor_correct_share", "margin": 0.03, "lower_bound_above": 0.01}
RESULT2 = {"measure": "leak_noticed_share", "margin": 0.20, "lower_bound_above": 0.10}
TICKS_MS = (100, 500, 2000)
N_RESAMPLES = B1.N_RESAMPLES  # 10,000 resamples of whole streams
BOOT_SEED = B1.BOOT_SEED

# Readings of what the brief leaves open, fixed before any tuning or held-out run:
#
# 1. "with the paired lower bound above 0.01": the 5th percentile of the paired cluster-bootstrap
#    distribution of (medium minus comparator), 90% equal-tailed, resamples of whole streams with
#    the same stream counts for both arms (`b1_stats.Measures.paired_difference`), must be > 0.01
#    (result 1) or > 0.10 (result 2); and the point difference must be >= 0.03 (or >= 0.20).
# 2. "the medium's notices on background per stream not exceeding 6.82": the point estimate (mean
#    over the 200 held-out streams) of the medium arm, <= 6.82. Its interval is reported beside;
#    the bound is on the point, as B2's table gives the comparator's 6.82 as a point.
# 3. The comparator's numbers are those of the `reanchor` arm of the same held-out run (rerun, as
#    the chief asked, "so the table is one run"); B2's 0.952 / 0.460 / 6.82 are what it must
#    reproduce (it replays B2's arm), and the background bound stays the fixed 6.82 whatever the
#    rerun gives.
# 4. "At the best tick length": a result holds for the experiment if it holds, as written, at at
#    least one tick length; the best tick length is the one at which both hold, or else the one
#    with the larger minimum of the two normalized margins (point difference over margin). The
#    sweep is shown whatever it is.
# 5. Leak measures are over the hard incidents of the slow-leak family; "hard non-leak" over the
#    other hard families, as B1's loader defines them.

# ---- the tuning rule (fixed before the first tuning run) ------------------------------------------
#
# Per tick length, on the 100 tuning streams (10000-10099) at b = 5, rho = 0.7, under the selection
# oracle at 16 s and the rung's context: among the configurations of the tuning grids, the one that
# maximizes
#
#     min( (AC - AC_ref) / 0.03 , (LN - LN_ref) / 0.20 )
#
# where AC is the hard non-leak anchor-correct share, LN the leak noticed share, and _ref the
# comparator (`reanchor`, after the re-fix; rung z = 2 before it) on the same tuning streams, among the configurations whose notices on
# background per stream are at most TUNE_BACKGROUND (a margin below the criterion's 9.03, so that a
# held-out stream set noisier than the tuning set does not cross the bound). Ties go to the fewer
# background notices. Structural choices (which coincidence form; whether the rhythms, a phase gate
# or an oscillator are in the graph) are made by the same rule, so an element is in the frozen graph
# only if it earned its place on the tuning streams.
TUNE_BACKGROUND = 7.5  # under the first comparator's 9.03 (tune-a, tune-b)
# After the re-fix the same margin below the new bound, about 17%: 6.82 * 7.5 / 9.03.
TUNE_BACKGROUND = 5.6

# ---- the held-out run (written at the freeze, before it runs) ------------------------------------
#
# One run, the 200 held-out streams (20000-20199), b = 5, rho = 0.7, the selection oracle at 16 s
# with the rung's context, as B1's and B2's tables:
# - the comparator rows: the rung at z = 3 and z = 2 and the re-anchor (B2's row);
# - the frozen medium at each tick length (m2-selected.json);
# - at each tick length, the coincidence ablation (the sliding form in place of the ordered one)
#   and the control of the synthesis decision (the abnormal-only sense adapter);
# - at 100 ms, sensitivity rows (labelled; nothing is chosen from them): the burst path's anchor
#   lookback at 100, 200, 500 and 1,000 ms (the frozen value is 0), for "anchor correctness as a
#   function of the emitter lookback"; and the binned form on the 10 s rhythm, the one oscillome
#   element that the tuning grids tried (it lost there).
# The rhythms, phase gates and oscillators are not in any frozen graph (none earned a place on the
# tuning streams; phase gates and oscillators had no candidate role in this world), so "rhythms
# off", "phase gates off" and "oscillators off" are the frozen arms themselves: they are reported,
# not rerun under another name.
LOOKBACK_SENSITIVITY_MS = (100, 200, 500, 1000)
BINNED_WINDOW_MS = 100


def selected():
    return load_json(OUT / "m2-selected.json")


def heldout(stage):
    """(arms, seeds, run_seed, experiment) of the held-out run; arms are (name, noticer)."""
    if stage != "heldout":
        raise SystemExit(f"unknown held-out stage {stage!r}")
    sel = selected()["ticks"]
    arms = [("rung_z3", {"noticer": "rung", "notice_z": 3.0}),
            ("rung_z2", {"noticer": "rung", "notice_z": 2.0}),
            ("reanchor", dict(REANCHOR))]
    for tick in TICKS_MS:
        frozen = sel[str(tick)]["noticer"]
        arms.append((f"med_t{tick}", frozen))
        arms.append((f"med_t{tick}_sliding", dict(frozen, coincidence="sliding")))
        arms.append((f"med_t{tick}_abnormal_only", dict(frozen, abnormal_only=True)))
    frozen = sel["100"]["noticer"]
    for lb in LOOKBACK_SENSITIVITY_MS:
        arms.append((f"med_t100_lb{lb}", dict(frozen, burst_lookback_ns=lb * MS)))
    arms.append(("med_t100_binned", dict(frozen, coincidence="binned", rhythms=True,
                                         burst_window_ns=BINNED_WINDOW_MS * MS,
                                         burst3_window_ns=BINNED_WINDOW_MS * MS)))
    return arms, HELDOUT_SEEDS, 12_900, "exploration-m2-heldout"


# B2's comparator, in the manifest's spelling (b2-selected.json, stage 2).
REANCHOR = {"noticer": "reanchor", "notice_z": 2.0, "gap_ns": 20_000_000, "min_burst": 2,
            "isolation": "site"}
