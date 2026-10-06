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

COMPARATOR = "rung_z2"  # RungNoticer at z = 2, rerun in the held-out run so the table is one run
COMPARATOR_B1 = {"hard_anchor_correct_share": 0.914, "leak_noticed_share": 0.460,
                 "notices_on_background_per_stream": 9.03}
BACKGROUND_BOUND = 9.03  # the medium's notices on background per stream may not exceed this
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
# 2. "the medium's notices on background per stream not exceeding 9.03": the point estimate (mean
#    over the 200 held-out streams) of the medium arm, <= 9.03. Its interval is reported beside;
#    the bound is on the point, as B1's table gives the comparator's 9.03 as a point.
# 3. The comparator's numbers are those of the rung z = 2 arm of the same held-out run (rerun, as
#    the brief says, "so the table is one run"); B1's 0.914 / 0.460 / 9.03 are what it must
#    reproduce (it replays B1's arm), and the background bound stays the fixed 9.03 whatever the
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
# comparator (rung z = 2) on the same tuning streams, among the configurations whose notices on
# background per stream are at most TUNE_BACKGROUND (a margin below the criterion's 9.03, so that a
# held-out stream set noisier than the tuning set does not cross the bound). Ties go to the fewer
# background notices. Structural choices (which coincidence form; whether the rhythms, a phase gate
# or an oscillator are in the graph) are made by the same rule, so an element is in the frozen graph
# only if it earned its place on the tuning streams.
TUNE_BACKGROUND = 7.5
