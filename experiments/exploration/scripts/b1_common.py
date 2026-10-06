"""B1 noticers: the settings, the arms, the tuning rule and the constants, in one place.

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation).
Everything that determines what B1 ran, and every reading the unit's specification leaves open, is
fixed here and in `b1_manifests.py` before any held-out run, so that the manifests and the analysis
cannot disagree. The measures themselves are the evaluator's (`crates/gordian-stream-eval/
RULES.md`, rules N1 to N12), not this file's.

Run directories live in artifacts/runs/ (git-ignored); manifests in artifacts/runs/_manifests/.
Seeds, settings, the selection oracle's delay and the bootstrap are R4 to R10's, reused unchanged.
"""

import os
import pathlib

import r10_common as C10

C6 = C10.C6
C5 = C10.C5
ROOT = C10.ROOT
RUNS = C10.RUNS
MANIFESTS = C10.MANIFESTS
OUT = C10.OUT
BIN = C10.BIN
NS = C10.NS
setting_id = C10.setting_id
load_json = C10.load_json

# R6's and R10's committed run outputs are in the main checkout's ignored artifacts directory.
# Read only: B1 never writes there.
R6_RUNS = pathlib.Path(os.environ.get("R6_RUNS", "/home/user/gordian/artifacts/runs/r6"))
R10_RUNS = pathlib.Path(os.environ.get("R10_RUNS", "/home/user/gordian/artifacts/runs/r10"))

# ---- seeds ---------------------------------------------------------------------------------

# "R6's tuning streams" are the seeds every baseline of R4 to R6 was tuned on, 10000-10099. The
# queue's parenthesis, "31000-31099 region, as the R6 scripts do", names R6's diagnostic seeds
# (`r6_common.DIAG_SEEDS`), which R6 used for a ledger trace and not for tuning; the report states
# which was read and why. Both are disjoint from the held-out seeds.
TUNING_SEEDS = C6.TUNING_SEEDS  # 10000-10099
HELDOUT_SEEDS = C6.HELDOUT_SEEDS  # 20000-20199
PRIMARY = C10.PRIMARY  # b = 5, rho = 0.7

# ---- the arm every noticer is evaluated under -----------------------------------------------------

# The selection oracle with the rung's own context, at the delay R5 tuned for the primary setting
# (r5-selected.json: 16 s), as R10's `sel_rung_privileged` arm: so that a quality row is comparable
# with R10's.
SEL_POLICY = "oracle_selection"


def sel_delay_s():
    return C10.sel_delay_s(*PRIMARY)


# ---- the noticers and their parameters -------------------------------------------------------------

DEFAULT_Z = C10.DEFAULT_Z  # 3.0, RungConfig::default().notice_z
Z2 = 2.0  # R10's first sweep value, the plan's "z = 2"

# Grids, geometric ladders fixed before any B1 run. `quiet_ns` of ChangeTriggered: the unit's
# description gives no value, and the background rate (about 0.1 abnormal observations a second per
# node) says what the ladder must span: from a quiet period shorter than most background gaps to
# one that leaves a node quiet only if it has been silent for about as long as the stream's first
# regime (the stream is 600 s). `lookback_ns` of EarliestAnchor: from nothing (the rung exactly) to
# eight seconds, the score window.
CHANGE_Q_S = [0.5, 1, 2, 4, 8, 16, 32, 64, 128]
EARLIEST_L_S = [0, 0.25, 0.5, 1, 2, 4, 8]

NAMES = {
    "rung": "rung_z3",
    "rung_z2": "rung_z2",
}


def change_name(q):
    return f"change_q{q:g}"


def earliest_name(l):
    return f"earliest_l{l:g}"


def spec(kind, **p):
    """The manifest spelling of a noticer."""
    if kind == "rung":
        out = {"noticer": "rung"}
        if p.get("z") is not None:
            out["notice_z"] = p["z"]
        return out
    if kind == "change_triggered":
        return {"noticer": "change_triggered", "quiet_ns": round(p["q"] * NS)}
    if kind == "earliest_anchor":
        return {"noticer": "earliest_anchor", "lookback_ns": round(p["l"] * NS)}
    raise KeyError(kind)


def grid():
    """Every noticer configuration of the tuning grid: (arm name, kind, parameters)."""
    out = [("rung_z3", "rung", {}), ("rung_z2", "rung", {"z": Z2})]
    out += [(change_name(q), "change_triggered", {"q": q}) for q in CHANGE_Q_S]
    out += [(earliest_name(l), "earliest_anchor", {"l": l}) for l in EARLIEST_L_S]
    return out


GRID = {name: (kind, p) for name, kind, p in grid()}


def arm_name(name):
    """The arm's name in a manifest: the selection oracle under the noticer `name`."""
    return f"sel_{name}_privileged"


# ---- the tuning rule (fixed before any held-out run) ---------------------------------------------

# Each parameterised noticer's parameter is chosen on the tuning streams (10000-10099) at the
# primary setting, under the selection oracle and the rung's context, as the one that maximises
#
#     the anchor-correct share of hard incidents outside the slow-leak family (pooled over streams)
#
# among the parameters whose notices on background per stream do not exceed those of the default
# `RungNoticer` (`rung_z3`) on the same streams. A tie is broken toward the parameter nearer the
# rung's own behaviour (larger q for ChangeTriggered, smaller l for EarliestAnchor). If no parameter
# of the grid satisfies the constraint, the one with the fewest background notices is chosen and
# the report says so. The constraint is the one M2's criterion will impose (`no more notices on
# background per stream than that noticer`), applied here to the baselines so that each is the best
# it can be at the background budget the rung spends; the quality row is then a consequence, not a
# target. The held-out grid is also run and reported as sensitivity, and no parameter is chosen from
# it.
TUNE_OBJECTIVE = "anchor_correct_hard_no_leak"
TUNE_BUDGET_REFERENCE = "rung_z3"

# ---- the criterion's fixed numbers (R10's; the bootstrap and the interval are the plan's) ------------

N_RESAMPLES = C10.N_RESAMPLES
BOOT_SEED = C10.BOOT_SEED
FAMILIES = ("compound", "cascade", "split_brain", "slow_leak")


def run_id(stage):
    b, rho = PRIMARY
    return f"b1-{stage}-{setting_id(b, rho)}"
