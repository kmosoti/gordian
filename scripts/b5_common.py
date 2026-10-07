"""B5: the rows, the arms, the budgets, the tuning rule and the constants, in one place.

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation).
Everything that determines what B5 ran, and every reading the unit's specification leaves open, is
fixed here and in `b5_manifests.py` before the run it governs (the commit that holds each part is
named in the report), so that the manifests and the analysis cannot disagree. The measures are the
evaluator's (`crates/gordian-stream-eval/RULES.md`), not this file's; the arithmetic is B4's
(`b4_stats.Measures`) with B5's verified-decision columns added in `b5_stats.py`. The seeds, the
setting, the bootstrap, the selection oracle's 16 s delay and B3's rows are B2's to B4's, reused
unchanged (through `b4_common`).

Run directories live in artifacts/runs/ of this worktree (git-ignored); manifests in
artifacts/runs/_manifests/.

READINGS (fixed here, before any B5 run)

R1. **The selector** is `public_budgeted` (`crates/gordian-run/src/stream/arms/public_budgeted.rs`,
    whose module documentation holds its readings, written before any tuning): at most `k` questions
    per segment, asked at the step an anomaly becomes ready (notice + delay), best score first, if the
    score reaches the threshold; five features of the rung's public state; no strict-precision proxy
    exists in any noticer of the tree, so the score has none.
R2. **"Verified decisions (plain and hard)"** are the plain and hard incidents (hard including the
    slow-leak family) that the evaluator finds declared correctly by their deadline (S6, S20: the
    `correct_plain` and `correct_hard` counts of `results.csv`), per stream. "Verified" is the
    evaluator's verification against the hidden record, not the reasoner's confirmation. The plain
    incidents the cheap rung declares correctly without any call count: the figure of `never_escalate`
    is the floor, and the table reports each arm's gain over it beside the figure itself.
R3. **"Hard quality"** is B2's: hard non-leak incidents declared correctly by their deadline over hard
    non-leak incidents; leaks are reported beside. **"Critical misses"** are critical plain and hard
    incidents missed, per stream (R5's).
R4. **"At the same k"**: a paired difference against the comparator row (ramp + split over the
    re-anchor, unbilled, the hand-written B3 row) under the selector with the same `k`, the same
    tuned score (one score per `k` for every row), over the same streams.
R5. **Billing.** The harness bills the medium's counted operations and the dataflow noticer's (billed
    by default); it does not bill the hand-written noticers. So the B3 rows are unbilled and the
    medium and dataflow rows are billed, as in B4 and C1; C1 showed that billing moves the logical
    clock by microseconds and nothing else. The table labels each row's billing. Every budgeted and
    reference arm over a row keeps its row's billing; the dataflow rows (comparator and re-anchor,
    billed) are the billed twins of the comparator and the re-anchor, so that a billed comparison
    exists for every sweep arm; the medium rows have no unbilled twin (the medium's charge is not
    switchable without editing Lab 1's files).
R6. **The delay sweep** is the selection oracle (as the brief says; it never asks about a plain
    incident or a decoy) at delays 8, 12, 16 and 20 s after notice, over the comparator, the
    re-anchor, M2's frozen medium at 100 ms and M3's frozen medium at 500 ms ("M2's and M3's frozen
    media" read as 100 ms from M2 and 500 ms from M3, the two ticks the brief names), and the
    dataflow twins of the first two. The oracle is run without `hold_until_asked`, as B4's table and
    R5's primary arm: an anomaly the rung retires before the delay is never asked about, which is
    the selector the sweep measures.
R7. **The table's rows** are B3's eleven rows (billed: no), C1's dataflow row (the comparator's
    rules, billed), M2's frozen media at 100, 500 and 2000 ms and M3's at 100, 500 and 2000 ms
    (billed). Every row is played under: the selection oracle at 16 s (the labelled ceiling for hard
    quality and cost), never escalate, always escalate at 16 s, and for each `k` the budgeted
    selector at its tuned score and the first-`k` baseline (the same budget with every weight zero).
R8. **Feature AUCs** are computed at the step that makes an anomaly ready (notice + 16 s), over the
    anomalies live then, by the evaluator's class of the notice's anchor (hard non-leak against
    plain; leak against decoy; and hard including leaks against plain beside them), on the tuning
    streams, for each of the five features and, labelled as not in the score, the rung's score and
    peak score. A notice the rung retired before the delay has no features and is counted.
    The tuning rows are also the AUC rows, with the re-anchor and the 500 ms medium beside.
"""

import json
import sys

import b4_common as C4
import b3_manifests as M3
import c1_common as C1

C3 = C4.C3
C2 = C4.C2
M = C1.M  # M2's common module: R6_RUNS, the selections
ROOT = C4.ROOT
RUNS = C4.RUNS
MANIFESTS = C4.MANIFESTS
OUT = C4.OUT
BIN = C4.BIN
NS = C4.NS
MS = 1_000_000
BOOT_SEED = C4.BOOT_SEED
N_RESAMPLES = C4.N_RESAMPLES
setting_id = C4.setting_id
TUNING_SEEDS = C4.TUNING_SEEDS  # (10000, 100)
HELDOUT_SEEDS = C4.HELDOUT_SEEDS  # (20000, 200)
PRIMARY = C4.PRIMARY  # b = 5, rho = 0.7

# ---- the numbers the queue fixes (docs/lab-queue.md, B5) -----------------------------------------------

DELAY_S = C4.DELAY_S  # R5's 16 s
KS = [2, 4, 8, 16]
SWEEP_DELAYS_S = [8, 12, 16, 20]
COMPARATOR = C4.COMPARATOR  # "ramp_split_over_re2"

# ---- the rows ---------------------------------------------------------------------------------------------
#
# Stems are arm-name stems. The comparator and the other B3 rows are hand-written, unbilled noticers;
# `df_*` are C1's dataflow noticer (billed), `m2_*` and `m3_*` the frozen media of M2 and M3 (billed).

TICKS_MS = (100, 500, 2000)


def _selected(name):
    with open(OUT / f"{name}-selected.json") as fh:
        return json.load(fh)


def medium_json(unit, tick_ms):
    """The frozen medium of `unit` ('m2' or 'm3') at `tick_ms`, as the manifest spells it."""
    return dict(_selected(unit)["ticks"][str(tick_ms)]["noticer"])


def dataflow_json(stem):
    """C1's dataflow noticer, billed: the comparator's rules, or the re-anchor alone."""
    row = C1.b3_row()
    if stem == "df_" + COMPARATOR:
        return {"noticer": "dataflow", "base": row["base"], "ramp": row["ramp"], "split": row["split"]}
    if stem == "df_reanchor":
        return {"noticer": "dataflow", "base": row["base"]}
    raise KeyError(stem)


def b3_rows():
    """B3's eleven rows as (stem, noticer json), in table order."""
    return M3.table_rows(C3.load_selected())


def table_rows():
    """Every row of the held-out table as (stem, noticer json, billed): B3's eleven, the dataflow row,
    M2's and M3's media."""
    rows = [(s, n, False) for s, n in b3_rows()]
    rows.append(("df_" + COMPARATOR, dataflow_json("df_" + COMPARATOR), True))
    for unit in ("m2", "m3"):
        for t in TICKS_MS:
            rows.append((f"{unit}_t{t}", medium_json(unit, t), True))
    return rows


def sweep_rows():
    """The noticers of the delay sweep as (stem, noticer json, billed)."""
    by = {s: (n, b) for s, n, b in table_rows()}
    out = [(s, *by[s]) for s in (COMPARATOR, "reanchor", "df_" + COMPARATOR, "m2_t100", "m3_t500")]
    out.append(("df_reanchor", dataflow_json("df_reanchor"), True))
    return out


TUNING_ROWS = [COMPARATOR, "m2_t100"]  # the status quo's best public noticer and M2's medium at 100 ms
AUC_ROWS = [COMPARATOR, "m2_t100", "reanchor", "m3_t500"]


def row_json(stem):
    for s, n, _ in table_rows() + sweep_rows():
        if s == stem:
            return n
    raise KeyError(stem)


# ---- the arms ---------------------------------------------------------------------------------------------


def never_policy():
    return "never_escalate"


def always_policy():
    return {"policy": "always_escalate", "delay_ns": DELAY_S * NS}


def oracle_policy(delay_s=DELAY_S):
    return {"policy": "oracle_selection", "delay_ns": delay_s * NS}


def score_json(score):
    """A score as the manifest writes it: the threshold and five weights, every field."""
    keys = ("threshold", "contradiction", "silence", "evidence", "services", "age")
    return {k: float(score.get(k, 0.0)) for k in keys}


def budgeted_policy(k, score, delay_s=DELAY_S):
    return {"policy": "public_budgeted", "delay_ns": delay_s * NS, "k": k, "score": score_json(score)}


FLAT = {}  # every weight zero and the threshold zero: the first k anomalies to become ready


def never_arm(stem):
    return f"never_{stem}"


def always_arm(stem):
    return f"alw16_{stem}"


def oracle_arm(stem, delay_s=DELAY_S):
    return f"sel_{stem}_privileged" if delay_s == DELAY_S else f"seld{delay_s:02d}_{stem}_privileged"


def fk_arm(stem, k):
    return f"fk{k}_{stem}"


def bud_arm(stem, k):
    return f"bud{k}_{stem}"


def run_id(stage):
    b, rho = PRIMARY
    return f"b5-{stage}-{setting_id(b, rho)}"


def eprint(*a):
    print(*a, file=sys.stderr)
