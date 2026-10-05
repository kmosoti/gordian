"""R10 salience ceiling: the settings, the arms, the readings and the constants, in one place.

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation).
Everything that determines what R10 ran, and every reading the plan leaves open, is fixed here and
in `r10_manifests.py` before any R10 held-out run, so the manifests and the analysis cannot
disagree. The criterion itself (docs/local-test-plan.md, 5R, R10) is not in this file's gift: its
margins, pairings, delay, settings and bootstrap are copied from the plan and not changed.

Run directories live in artifacts/runs/ (git-ignored); manifests in artifacts/runs/_manifests/.
"""

import os
import pathlib

import r6_common as C6

C5 = C6.C5
C4 = C6.C4
ROOT = C6.ROOT
RUNS = C6.RUNS
MANIFESTS = C6.MANIFESTS
OUT = C6.OUT
BIN = C6.BIN
NS = C6.NS
setting_id = C6.setting_id
load_json = C6.load_json

# R6's committed run outputs are in the main checkout's ignored artifacts directory. Read only.
R6_RUNS = pathlib.Path(os.environ.get("R6_RUNS", "/home/user/gordian/artifacts/runs/r6"))

# ---- seeds ---------------------------------------------------------------------------------

HELDOUT_SEEDS = C6.HELDOUT_SEEDS  # 20000-20199, as R4 to R7
# The diagnostic run's streams: outside the tuning (10000-10099), held-out (20000-20199), R5's
# (30000-30099), R6's (31000-31099) and R8's (30000 upward questions) seeds that R10 reads.
DIAG_SEEDS = (32_000, 100)

# ---- settings (the plan: primary b = 5, rho = 0.7; sensitivity b = 2.5 and b = 8 at rho = 0.7) ---

PRIMARY = (5.0, 0.7)
SETTINGS = [(5.0, 0.7), (2.5, 0.7), (8.0, 0.7)]


def sel_delay_s(b, rho):
    """The delay R5 tuned for the selection oracle at this setting (r5-selected.json)."""
    return C5.load_json(OUT / "r5-selected.json")[setting_id(b, rho)]["selection_delay_s"]


# ---- the pairings (the plan: the rung's own context, and `window` 40 s, N 256) -------------------

# R6's names for the same selection-oracle arms, so that the comparison arm of R10 is byte-for-byte
# R6's arm (checked, modulo the run id, in r10_provenance.py).
SEL_RUNG = "sel_rung_privileged"
SEL_WIN = "sel_win_w40_n256_privileged"
NOTICE_RUNG = "notice_rung_privileged"
NOTICE_WIN = "notice_win_w40_n256_privileged"
ORACLE = C6.ORACLE  # R4's oracle: a reference row
NEVER = C6.NEVER  # the rung alone: what noticing without escalation scores

WINDOW_BUILDER = {"builder": "window", "window_ns": 40 * NS, "max_refs": 256}

PAIRINGS = [
    # (label, comparison arm, treatment arm)
    ("rung's own context", SEL_RUNG, NOTICE_RUNG),
    ("window 40 s, N 256", SEL_WIN, NOTICE_WIN),
]


def policy(kind, delay_s):
    pol = {"policy": kind}
    if delay_s:
        pol["delay_ns"] = delay_s * NS
    return pol


def heldout_arms(delay_s):
    """(arm name, policy, context) of a held-out run: both pairings, R4's oracle, never_escalate."""
    return [
        (SEL_RUNG, policy("oracle_selection", delay_s), None),
        (NOTICE_RUNG, policy("oracle_notice", delay_s), None),
        (SEL_WIN, policy("oracle_selection", delay_s), WINDOW_BUILDER),
        (NOTICE_WIN, policy("oracle_notice", delay_s), WINDOW_BUILDER),
        (ORACLE, "oracle_escalation", None),
        (NEVER, "never_escalate", None),
    ]


def diag_arms(delay_s):
    """The diagnostic run (rung's context, primary setting): the pair, and R4's oracle."""
    return [
        (SEL_RUNG, policy("oracle_selection", delay_s), None),
        (NOTICE_RUNG, policy("oracle_notice", delay_s), None),
        (ORACLE, "oracle_escalation", None),
    ]


# ---- the public threshold sweep -----------------------------------------------------------------

DEFAULT_Z = 3.0  # RungConfig::default().notice_z
# Three values below the default, fixed before any held-out number. The notice score is
# z = (n - mu) / sqrt(mu + 1) over the abnormal count n in an 8 s window, with mu about 0.8 at the
# default rates, so these correspond to needing n >= 4, 3 and 2 abnormal observations (the default
# needs 5): a ladder that moves the integer threshold one step at a time.
SWEEP_Z = (2.0, 1.0, 0.5)
SWEEP_ARM = SEL_RUNG  # `oracle_selection` with the rung's context, as the plan says


def zname(z):
    return f"z{z:g}"


# ---- the criterion's fixed numbers (copied from the plan; not changed) -----------------------------

N_RESAMPLES = 10_000
BOOT_SEED = 9_950
RESULT1_MARGIN, RESULT1_LOWER = 0.03, 0.01  # hard incidents, slow leak excluded
RESULT2_MARGIN, RESULT2_LOWER = 0.20, 0.10  # slow leak alone


def run_id(stage, b, rho, z=None):
    base = f"r10-{stage}"
    if z is not None:
        base += f"-{zname(z)}"
    return f"{base}-{setting_id(b, rho)}"
