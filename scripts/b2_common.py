"""B2: the settings, the arms, the tuning rules and the constants, in one place.

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation).
Everything that determines what B2 ran, and every reading the unit's specification leaves open, is
fixed here and in `b2_manifests.py` before the run it governs, so that the manifests and the
analysis cannot disagree. The measures are the evaluator's (`crates/gordian-stream-eval/RULES.md`,
rules N1 to N16), not this file's. The seeds, the setting, the bootstrap and the selection
oracle's delay grid are R4 to R10's and B1's, reused unchanged (through `b1_common`).

Run directories live in artifacts/runs/ (git-ignored); manifests in artifacts/runs/_manifests/.
"""

import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
# B1's scripts (read-only reuse: this file imports them, it does not edit them).
sys.path.insert(0, str(ROOT / "experiments" / "exploration" / "scripts"))

import b1_common as C1  # noqa: E402  (also puts this worktree's `analysis` on the path)

RUNS = C1.RUNS
MANIFESTS = C1.MANIFESTS
OUT = C1.OUT
BIN = C1.BIN
NS = C1.NS
BOOT_SEED = C1.BOOT_SEED
N_RESAMPLES = C1.N_RESAMPLES
setting_id = C1.setting_id
TUNING_SEEDS = C1.TUNING_SEEDS  # 10000-10099
HELDOUT_SEEDS = C1.HELDOUT_SEEDS  # 20000-20199
PRIMARY = C1.PRIMARY  # b = 5, rho = 0.7
FAMILIES = C1.FAMILIES

# ---- the numbers the queue fixes (docs/lab-queue.md, B2 and M2) ------------------------------------

BACKGROUND_BUDGET = 9.03  # notices on background per stream: the M2 comparator's (RungNoticer z = 2)
TARGET_ANCHOR_CORRECT = 0.944  # the M2 comparator's 0.914 plus its 0.03 margin
COMPARATOR = "rung_z2"

# ---- the selection oracle: the two quality readings ----------------------------------------------

SEL_POLICY = "oracle_selection"
FIXED_DELAY_S = C1.sel_delay_s()  # R5's 16 s for the primary setting: reading 1, as B1
DELAYS_S = list(C1.C10.C6.C5.C4.ALWAYS_DELAYS_S)  # the grid R5 tuned the delay on: 0 ... 30 s

# ---- the noticers ------------------------------------------------------------------------------------

# B1's noticers, with the parameters B1 chose on the tuning streams (b1-selected.json): not re-tuned.
with open(OUT / "b1-selected.json") as _fh:
    B1_SELECTED = json.load(_fh)
CHANGE_Q = B1_SELECTED["change_triggered"]["value"]  # 128 s
EARLIEST_L = B1_SELECTED["earliest_anchor"]["value"]  # 0.25 s

# The four rows of B1's table, and the new row. Names are the arm-name stems.
TABLE = ["rung_z3", "rung_z2", C1.change_name(CHANGE_Q), C1.earliest_name(EARLIEST_L), "reanchor"]

REANCHOR_ISOLATION = ["site", "any"]
REANCHOR_GAP_MS = [50, 100, 150, 200, 300, 400]
# AMENDMENT 1, made after stage 1's first run and before any held-out run: the anchor-correct share
# fell monotonically with the gap and the best configuration was at the smallest gap of the grid
# (50 ms), so the grid is extended downward (run `b2-tune1b`) to see where the shortfall stops. The
# extension is run on the tuning streams only; the rule below chooses among both runs' configurations.
REANCHOR_GAP_MS_EXTENSION = [10, 20, 30]
REANCHOR_MIN_BURST = [2, 3]
REANCHOR_Z = [3.0, 2.0, 1.5, 1.0]  # stage 2: the threshold, at the stage 1 choice of the rest


def reanchor_name(isolation, gap_ms, burst, z=None):
    base = f"reanchor_{isolation}_g{gap_ms}_b{burst}"
    return base if z is None or z == C1.DEFAULT_Z else f"{base}_z{z:g}"


def spec(kind, **p):
    """The manifest spelling of a noticer (B1's, and `reanchor`)."""
    if kind == "reanchor":
        out = {"noticer": "reanchor"}
        if p.get("z") is not None and p["z"] != C1.DEFAULT_Z:
            out["notice_z"] = p["z"]
        out.update({"gap_ns": round(p["gap_ms"] * 1_000_000), "min_burst": p["burst"],
                    "isolation": p["isolation"]})
        return out
    return C1.spec(kind, **p)


def stage1_grid(gaps=None):
    """Every (isolation, gap, burst) at the rung's default threshold: (name, kind, parameters)."""
    return [(reanchor_name(i, g, b), "reanchor", {"isolation": i, "gap_ms": g, "burst": b})
            for i in REANCHOR_ISOLATION for g in (gaps or REANCHOR_GAP_MS) for b in REANCHOR_MIN_BURST]


def stage1b_grid():
    """The extension of amendment 1: the same, at the gaps below the first grid's."""
    return stage1_grid(REANCHOR_GAP_MS_EXTENSION)


def b1_grid():
    """B1's whole grid and the two extra thresholds of its sensitivity table (20 configurations)."""
    return C1.grid() + C1.extra_grid()


def arm_name(name):
    """The selection oracle at R5's fixed delay, with the rung's retirement: reading 1 (B1's arms)."""
    return C1.arm_name(name)


def hold_arm_name(name):
    """The selection oracle at the noticer's tuned delay, with `hold_until_asked`: reading 2."""
    return f"selhold_{name}_privileged"


def delay_arm_name(name, delay_s):
    return f"selhold_{name}_d{delay_s:02d}_privileged"


def fixed_policy():
    return {"policy": SEL_POLICY, "delay_ns": FIXED_DELAY_S * NS}


def hold_policy(delay_s):
    pol = {"policy": SEL_POLICY, "hold_until_asked": True}
    if delay_s:
        pol["delay_ns"] = delay_s * NS
    return pol


# ---- the tuning rules (fixed here, before the run each governs) --------------------------------------

# Stage 1 (run `b2-tune1`): the 24 configurations of `stage1_grid` at the rung's default threshold,
# on the 100 tuning streams at the primary setting, with the selection oracle at the fixed delay and
# the rung's retirement, as B1's tuning. CHOSEN: the configuration of the highest anchor-correct share
# of hard incidents outside the slow-leak family (pooled over streams) among those whose notices on
# background per stream do not exceed BACKGROUND_BUDGET (9.03; at the default threshold none comes
# near it). A tie is broken toward the configuration that moves fewer anchors: the larger gap, then the
# larger burst, then the literal reading of the brief's "no other abnormal observation at its site"
# (`site`) before `any`. Beside it, never in the rule: the anchor-and-site-correct share, so that a
# configuration that buys anchor-correctness with a wrong site is visible.
#
# AMENDMENT 2, made after stage 1's first run and before any held-out run: the tie rule as first
# written put `any` before `site` (on the reasoning that `any` moves fewer anchors) and ranked the
# reading ahead of the gap. The first run tied the two readings at the top (185 of 199 incidents each),
# with `site`, the brief's own wording, the one with fewer notices on background; a tie rule that
# prefers my own variant to the brief's literal reading is backwards, and the gap is the parameter that
# says how many anchors move. The rule is now: larger gap, larger burst, then `site` before `any`.
#
# Stage 2 (run `b2-tune2`): the stage 1 choice at each threshold of REANCHOR_Z, under the same rule
# and budget; a tie goes to the larger threshold. The result is the `reanchor` row of the table.
#
# Stage 3 (run `b2-tunedelay`): for each of the five table noticers, the selection oracle with
# `hold_until_asked` at each delay of DELAYS_S, on the tuning streams; CHOSEN, R5's rule: the delay of
# the highest quality (hard incidents outside the slow-leak family declared correctly by their
# deadline), the cheapest (mean modelled cost per stream) of ties.
#
# Nothing is chosen on any held-out stream.

TUNE_OBJECTIVE = "hard_anchor_correct_share"
TIE_ORDER_NOTE = "larger gap, larger burst, site before any; larger threshold"


def tie_keys_stage1():
    """The stage 1 tie rule as (column, ascending) pairs over `with_tie_columns`."""
    return [("p_gap_ms", False), ("p_burst", False), ("site_first", True)]


def with_tie_columns(df):
    """`df` (a tuning table) with the column the stage 1 tie rule sorts on: 0 for `site`, 1 for `any`."""
    df = df.copy()
    df["site_first"] = (df["p_isolation"] != "site").astype(int)
    return df


def run_id(stage):
    b, rho = PRIMARY
    return f"b2-{stage}-{setting_id(b, rho)}"


def load_selected():
    with open(OUT / "b2-selected.json") as fh:
        return json.load(fh)
