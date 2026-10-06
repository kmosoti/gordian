"""B4: the arms, the grids, the tuning rules and the constants, in one place.

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation).
Everything that determines what B4 ran, and every reading the unit's specification leaves open, is
fixed here and in `b4_manifests.py` before the run it governs (the commit that holds this file is
named in the report), so that the manifests and the analysis cannot disagree. The measures are the
evaluator's (`crates/gordian-stream-eval/RULES.md`: N1 to N16 for notices, E1 to E8 for selection),
not this file's. The seeds, the setting, the bootstrap, the selection oracle's delay and B3's rows are
B3's and B2's, reused unchanged (through `b3_common`).

Run directories live in artifacts/runs/ (git-ignored); manifests in artifacts/runs/_manifests/.
"""

import json
import sys

import b3_common as C3
import b3_manifests as M3

C2 = C3.C2
C1 = C3.C1
ROOT = C3.ROOT
RUNS = C3.RUNS
MANIFESTS = C3.MANIFESTS
OUT = C3.OUT
BIN = C3.BIN
NS = C3.NS
BOOT_SEED = C3.BOOT_SEED
N_RESAMPLES = C3.N_RESAMPLES
setting_id = C3.setting_id
TUNING_SEEDS = C3.TUNING_SEEDS  # 10000-10099
HELDOUT_SEEDS = C3.HELDOUT_SEEDS  # 20000-20199
PRIMARY = C3.PRIMARY  # b = 5, rho = 0.7

# ---- the numbers the queue and R5 fix --------------------------------------------------------------

DELAY_S = C3.FIXED_DELAY_S  # R5's 16 s: "with the rung's context and R5's delay" (queue, B4 item 1)
BACKGROUND_BUDGET = C3.BACKGROUND_BUDGET  # 6.82 notices on background per stream ("the same budget")
COMPARATOR = "ramp_split_over_re2"  # "paired differences against ramp + split over the re-anchor"

# ---- the rows ---------------------------------------------------------------------------------------
#
# "Every B3 row and M2's medium at 100 ms": B3's eleven rows (`b3_manifests.table_rows`), the medium at a
# 100 ms tick with M2's frozen graph (`m2-selected.json`, read, never changed), and the follow-up rule
# (item 3) on each of the five rows that have a ramp, as a row of its own beside the row without it.

MEDIUM_STEM = "medium_t100"
FOLLOW_BASES = ["ramp_over_r3", "ramp_split_over_r3", "ramp_over_re2", "ramp_split_over_re2",
                "ramp_split_over_re3"]


def follow_stem(stem):
    return f"{stem}_follow"


def medium_json():
    with open(OUT / "m2-selected.json") as fh:
        return json.load(fh)["ticks"]["100"]["noticer"]


def with_follow(noticer, follow):
    """The manifest spelling of `noticer` (a composed noticer with a ramp) with `follow` in its ramp."""
    out = json.loads(json.dumps(noticer))
    out["ramp"]["follow"] = follow
    return out


def rows(sel3, follow):
    """The table's rows as (stem, noticer json), in table order: B3's, the medium, and (if `follow` is
    not None) the follow-up variants of the ramp rows."""
    base = M3.table_rows(sel3)
    out = list(base)
    out.append((MEDIUM_STEM, medium_json()))
    if follow is not None:
        by = dict(base)
        out += [(follow_stem(s), with_follow(by[s], follow)) for s in FOLLOW_BASES]
    return out


# ---- the selectors ----------------------------------------------------------------------------------
#
# Every selector asks at R5's delay with the rung's context; what differs is which anomalies. Tuning
# grids are fixed here, before the run that uses them.

THR_PERSIST_S = [0, 1, 2, 4, 8, 16]  # `public_threshold`'s t (contradictory or silent for t seconds)
CHG_K = [1, 2, 3, 5, 8, 12, 20]  # `public_change`'s k (growth in attached observations)


def never_policy():
    return "never_escalate"


def always_policy():
    return {"policy": "always_escalate", "delay_ns": DELAY_S * NS}


def thr_policy(t_s):
    pol = {"policy": "public_threshold", "delay_ns": DELAY_S * NS}
    if t_s:
        pol["persist_ns"] = round(t_s * NS)
    return pol


def chg_policy(k):
    return {"policy": "public_change", "delay_ns": DELAY_S * NS, "k": k}


def oracle_arm(stem):
    return C3.arm_name(stem)  # sel_<stem>_privileged, the selection oracle at 16 s: the ceiling


def never_arm(stem):
    return f"never_{stem}"


def always_arm(stem):
    return f"alw16_{stem}"


def thr_arm(stem, t_s):
    return f"thr_t{t_s:g}_{stem}"


def chg_arm(stem, k):
    return f"chg_k{k}_{stem}"


# ---- the follow-up rule's grid (stage F) ----------------------------------------------------------------
#
# FollowSpec (`noticer_follow.rs`): wait for `readings` readings after the completing one (or
# `horizon_ns`), keep the anomaly if the latest is at least `min_gain` above the completing reading's
# value, withdraw it at once on a reversal of more than `max_fall` below the peak. The grid is the whole
# product, fixed before stage F. `max_fall` of None is "never withdraw on a reversal" (u32::MAX).

FOLLOW_READINGS = [1, 2, 3, 4, 6, 8]
FOLLOW_HORIZON_S = [4, 6]
FOLLOW_MIN_GAIN = [0, 3, 6, 10]
FOLLOW_MAX_FALL = [2, 4, 8, None]
U32_MAX = 4_294_967_295


def follow_params(readings, horizon_s, min_gain, max_fall):
    return {"readings": readings, "horizon_s": horizon_s, "min_gain": min_gain, "max_fall": max_fall}


def follow_json(p):
    return {"readings": p["readings"], "horizon_ns": round(p["horizon_s"] * NS), "min_gain": p["min_gain"],
            "max_fall": U32_MAX if p["max_fall"] is None else p["max_fall"]}


def follow_stem_name(p):
    f = "x" if p["max_fall"] is None else p["max_fall"]
    return f"r{p['readings']}_h{p['horizon_s']}_g{p['min_gain']}_f{f}"


def follow_grid():
    return [follow_params(r, h, g, f) for r in FOLLOW_READINGS for h in FOLLOW_HORIZON_S
            for g in FOLLOW_MIN_GAIN for f in FOLLOW_MAX_FALL]


FOLLOW_TOLERANCES = [1, 2, 4]  # sensitivity: leaks the rule may retire on the tuning streams (the rule: 0)

# ---- the tuning rules (fixed here, before the run each governs) --------------------------------------
#
# Every stage plays its arms on the 100 tuning streams (10000-10099) at b = 5, rho = 0.7, with the rung's
# retirement and context, as B1's to B3's tuning. Nothing is chosen on any held-out stream.
#
# Stage F (run `b4-tunefollow`): the follow-up grid (192 configurations) over the noticer of the
# comparator row (ramp + split over the re-anchor), each under `always_escalate` at 16 s (the arm that asks
# about every anomaly still live at the delay, so that a retired anomaly is a call spared and a retired
# leak a leak not asked about), beside the same arm without the rule (`fol_none`). CHOSEN, the unit's
# "tuned under the same budget":
#   feasible: the notices on background per stream of the arm (the whole noticer's) do not exceed
#   BACKGROUND_BUDGET; the notices anchored on a slow leak that the rule retired number at most the
#   tolerance (0 for the rule; FOLLOW_TOLERANCES for the sensitivity configurations); and hard non-leak
#   quality is at least the arm without the rule's minus 0.005.
#   objective: the most notices anchored on decoys that the rule retired before escalation;
#   ties: the fewest notices anchored on plain or hard incidents that the rule retired (collateral),
#   then the larger `readings`, the larger `min_gain`, the larger `max_fall` (None largest), the larger
#   horizon, in that order.
#   If no feasible configuration retires a decoy notice, the rule is reported as finding nothing on the
#   tuning streams, and the follow-up rows are run with the feasible configuration of the ties above.
#
# Stage S (run `b4-tuneselect`): for each row of the table, each selector's grid under its own arm (the
# threshold's THR_PERSIST_S, the change rule's CHG_K), beside `never_escalate`, `always_escalate` at 16 s
# and the selection oracle at 16 s. CHOSEN per (row, selector), the unit's "hard-incident quality per
# unit cost":
#   among the grid's configurations whose hard non-leak quality is at least 0.9 times the best quality in
#   that grid (so that "ask about almost nothing" cannot win by being cheap), the highest quality per
#   modelled second of cost per stream (total cost: substrate, the rule and the reasoner; the medium's own
#   operations included); ties: the fewer calls per stream, then the more selective parameter (the larger
#   t, the larger k).
#
# AMENDMENTS (none yet; each would be recorded here with the date of the run it follows, and the run
# before it kept).

QUALITY_FLOOR = 0.9
QUALITY_LOSS_TOLERANCE = 0.005


def run_id(stage):
    b, rho = PRIMARY
    return f"b4-{stage}-{setting_id(b, rho)}"


def load_selected():
    with open(OUT / "b4-selected.json") as fh:
        return json.load(fh)


def eprint(*a):
    print(*a, file=sys.stderr)
