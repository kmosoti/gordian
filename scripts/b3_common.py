"""B3: the arms, the grids, the tuning rules and the constants, in one place.

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation).
Everything that determines what B3 ran, and every reading the unit's specification leaves open, is
fixed here and in `b3_manifests.py` before the run it governs (the commit that holds this file is
named in the report), so that the manifests and the analysis cannot disagree. The measures are the
evaluator's (`crates/gordian-stream-eval/RULES.md`, rules N1 to N16), not this file's; B3 adds none.
The seeds, the setting, the bootstrap, the selection oracle's delay grid and B2's table rows are
B2's, reused unchanged (through `b2_common`).

Run directories live in artifacts/runs/ (git-ignored); manifests in artifacts/runs/_manifests/.
"""

import json
import sys

import b2_common as C2

C1 = C2.C1
ROOT = C2.ROOT
RUNS = C2.RUNS
MANIFESTS = C2.MANIFESTS
OUT = C2.OUT
BIN = C2.BIN
NS = C2.NS
BOOT_SEED = C2.BOOT_SEED
N_RESAMPLES = C2.N_RESAMPLES
setting_id = C2.setting_id
TUNING_SEEDS = C2.TUNING_SEEDS  # 10000-10099
HELDOUT_SEEDS = C2.HELDOUT_SEEDS  # 20000-20199
PRIMARY = C2.PRIMARY  # b = 5, rho = 0.7
FAMILIES = C2.FAMILIES
DELAYS_S = C2.DELAYS_S
FIXED_DELAY_S = C2.FIXED_DELAY_S

# ---- the numbers the queue fixes (docs/lab-queue.md, B3) -------------------------------------------

BACKGROUND_BUDGET = 6.82  # notices on background per stream: the re-anchor's held-out value (M2's bound)
LEAK_NOTICED_BAR = 0.660  # "If a public row reaches leak noticed >= 0.660 within the budget"
COMPARATOR = "reanchor"  # the table's paired differences are against B2's re-anchor row

# ---- the noticers ------------------------------------------------------------------------------------

with open(OUT / "b2-selected.json") as _fh:
    B2_SELECTED = json.load(_fh)
_S2 = B2_SELECTED["stage2"]["chosen"]  # the re-anchor row: site, 20 ms, burst >= 2, z = 2
REANCHOR_Z = _S2["z"]
REANCHOR_GAP_MS = _S2["gap_ms"]
REANCHOR_BURST = _S2["burst"]
REANCHOR_ISOLATION = _S2["isolation"]

# A base is a name and its manifest spelling (the `base` object of a composed noticer).
BASES = {
    "r3": ("rung_z3", {"noticer": "rung"}),
    "re2": ("reanchor", {"noticer": "reanchor", "notice_z": REANCHOR_Z,
                         "gap_ns": REANCHOR_GAP_MS * 1_000_000, "min_burst": REANCHOR_BURST,
                         "isolation": REANCHOR_ISOLATION}),
    # the same re-anchor at the rung's default threshold: B2's stage 1 choice, 2.30 notices on
    # background per stream held-out, the base that leaves room in the budget for a composition
    "re3": ("reanchor_z3", {"noticer": "reanchor", "gap_ns": REANCHOR_GAP_MS * 1_000_000,
                            "min_burst": REANCHOR_BURST, "isolation": REANCHOR_ISOLATION}),
}

# ---- the ramp noticer's grid (stage R) --------------------------------------------------------------
#
# Fixed before the stage R run. Each parameter's values bracket what the public readings of the
# tuning streams' leaks show (a step of 0 to 9 a reading, readings 0.6 to 1.5 s apart) and a looser
# value on each side; the grid is the whole product (216 configurations). The ramp is composed over
# the rung's own noticer at its default threshold (the unit says it "composes with the rung's
# noticer"); the ramp component is independent of the base (it opens anomalies of its own), so its
# parameters are evaluated over the other bases unchanged, not re-tuned.
RAMP_GAP_MS = [1600, 2000, 3000]
RAMP_MAX_STEP = [10, 14]
RAMP_MAX_DROP = [0, 2, 4]
RAMP_MIN_READINGS = [3, 4, 5, 6]
RAMP_MIN_RISE = [10, 15, 20]


def ramp_params(gap_ms, max_step, max_drop, min_readings, min_rise):
    return {"gap_ms": gap_ms, "max_step": max_step, "max_drop": max_drop,
            "min_readings": min_readings, "min_rise": min_rise}


def ramp_stem(p):
    return (f"ramp_g{p['gap_ms']}_s{p['max_step']}_d{p['max_drop']}_n{p['min_readings']}"
            f"_r{p['min_rise']}")


def ramp_grid():
    return [ramp_params(g, s, d, n, r) for g in RAMP_GAP_MS for s in RAMP_MAX_STEP
            for d in RAMP_MAX_DROP for n in RAMP_MIN_READINGS for r in RAMP_MIN_RISE]


def ramp_json(p):
    return {"gap_ns": p["gap_ms"] * 1_000_000, "max_step": p["max_step"], "max_drop": p["max_drop"],
            "min_readings": p["min_readings"], "min_rise": p["min_rise"]}


# ---- the splitting noticer's grid (stage S) ---------------------------------------------------------

SPLIT_GAP_MS = [1000, 2000, 3000, 5000, 8000]
SPLIT_MIN_BURST = [2, 3, 4]


def split_params(gap_ms, min_burst):
    return {"gap_ms": gap_ms, "min_burst": min_burst}


def split_stem(p):
    return f"g{p['gap_ms']}_b{p['min_burst']}"


def split_grid():
    return [split_params(g, b) for g in SPLIT_GAP_MS for b in SPLIT_MIN_BURST]


def split_json(p):
    return {"gap_ns": p["gap_ms"] * 1_000_000, "min_burst": p["min_burst"]}


# ---- the composed noticers ---------------------------------------------------------------------------


def composed(base, ramp=None, split=None):
    """The manifest spelling of a composed noticer over the base named `base` ('r3', 're2', 're3')."""
    out = {"noticer": "composed", "base": dict(BASES[base][1])}
    if ramp is not None:
        out["ramp"] = ramp_json(ramp)
    if split is not None:
        out["split"] = split_json(split)
    return out


def name_of(base, ramp=None, split=None):
    """The arm-name stem of a composed noticer."""
    parts = []
    if ramp is not None:
        parts.append(ramp_stem(ramp))
    if split is not None:
        parts.append("split_" + split_stem(split))
    return "_".join(parts) + f"_over_{base}"


def noticer_json(config):
    """The manifest spelling of a configuration: ('base', name) for a base alone or a composed one."""
    kind = config["kind"]
    if kind == "base":
        return dict(BASES[config["base"]][1])
    return composed(config["base"], config.get("ramp"), config.get("split"))


# ---- the selection oracle: the two quality readings ----------------------------------------------

SEL_POLICY = C2.SEL_POLICY
fixed_policy = C2.fixed_policy
hold_policy = C2.hold_policy
arm_name = C2.arm_name
hold_arm_name = C2.hold_arm_name
delay_arm_name = C2.delay_arm_name

# ---- the tuning rules (fixed here, before the run each governs) --------------------------------------
#
# Every stage plays its arms on the 100 tuning streams (10000-10099) at b = 5, rho = 0.7 with the
# selection oracle at R5's fixed delay and the rung's retirement, as B1's and B2's tuning. Nothing is
# chosen on any held-out stream. The background budget is the unit's: BACKGROUND_BUDGET notices on
# background per stream, counted on the whole noticer (the base's notices and the added ones), on the
# tuning streams.
#
# Stage R (run `b3-tuneramp`): the 216 ramp configurations over the rung at its default threshold,
# beside the rung alone. CHOSEN, the unit's objective for the ramp noticer: among those within the
# budget, the highest *leak noticed share* (pooled over the tuning streams' slow leaks); then, beside
# it, the highest *leak anchor-correct share*; then the fewest notices on background per stream; then
# the most conservative parameters: the larger `min_readings`, the larger `min_rise`, the smaller
# `max_step`, the smaller `max_drop`, the smaller gap, in that order. The later keys exist because the
# first may tie on 44 leaks: they are named here so that a tie is not broken after the fact.
#
# Stage S (run `b3-tunesplit`): the 15 split configurations over the rung's noticer at its default
# threshold (S1) and over the later re-anchor row of B2's table (S2), each beside its base alone.
# CHOSEN per base, the unit's objective for the split noticer: among those within the budget, the
# highest *anchor-correct share of hard incidents outside the slow-leak family*; ties: the larger gap,
# then the larger `min_burst`. If no configuration is within the budget the one with the fewest notices
# on background is chosen and the report says so.
#
# Stage D (run `b3-tunedelay`): for every row of the table, the selection oracle with
# `hold_until_asked` at each delay of DELAYS_S; CHOSEN, R5's rule as B2 applied it: the delay of the
# highest quality, the cheapest of ties.
#
# AMENDMENTS (none yet; each would be recorded here with the date of the run it follows, and the run
# before it kept).

TUNE_RAMP_OBJECTIVE = "leak_noticed_share"
TUNE_SPLIT_OBJECTIVE = "hard_anchor_correct_share"


def ramp_tie_keys():
    """The rule's keys after the objective, as (column, ascending): leak anchor-correct is maximised,
    background notices minimised, then the conservative parameters."""
    return [("leak_anchor_correct_share", False), ("notices_on_background_per_stream", True),
            ("p_min_readings", False), ("p_min_rise", False), ("p_max_step", True),
            ("p_max_drop", True), ("p_gap_ms", True)]


def split_tie_keys():
    return [("p_gap_ms", False), ("p_min_burst", False)]


def run_id(stage):
    b, rho = PRIMARY
    return f"b3-{stage}-{setting_id(b, rho)}"


def load_selected():
    with open(OUT / "b3-selected.json") as fh:
        return json.load(fh)


def eprint(*a):
    print(*a, file=sys.stderr)
