"""C1, the incremental-dataflow noticer: the arms, the runs and the readings, in one place.

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation). The
criterion is the chief's (docs/lab-queue.md, "## C1"), fixed before any C1 code or run: reproduction
(the dataflow noticer's notice record equals the B3 row's, or every difference is explained) and
cost (its own cost per stream beside the medium's, at calibrated prices); no claim about which noticer
is better. Everything a reading of the brief leaves open is stated here, before the held-out run.
The measures are the evaluator's (rules N1 to N16 of `crates/gordian-stream-eval/RULES.md`); the
arithmetic is B1's and B2's (`b2_stats.Measures`), reused unchanged.

Run directories live in artifacts/runs/ of this worktree (git-ignored); manifests in
artifacts/runs/_manifests/.

Readings, fixed here:

R1. **The B3 row** is B3's arm `ramp_split_over_re2` (selection oracle `sel_ramp_split_over_re2_privileged`
    in B3's held-out run): the ramp (`b3-selected.json`, "ramp", "chosen") and the split over the
    re-anchor ("split_re2", "chosen") over B2's re-anchor (z = 2, 20 ms, burst >= 2, isolation site),
    spelled by `b3_common.composed`, exactly as L1's fifth control row spells it.
R2. **The medium** is M2's frozen graph at a 100 ms tick (`m2-selected.json`, ticks, "100"), spelled
    by `l1_common.medium_frozen`.
R3. **The dataflow noticer** is the same base, ramp and split under `"noticer": "dataflow"`
    (`DataflowSpec`), billed; a labelled control is the same with `"billed": false` (it separates what
    the bill's clock does from what the rules do, if the two records differ).
R4. **Every arm** is the selection oracle at R5's 16 s with the rung's own context and retirement
    (B1's, B2's, B3's, M2's), b = 5, rho = 0.7, so the comparator rows are B3's and M2's own.
R5. **"The record equals"**: the arm's `notice_events.csv` rows (every column but the run id and the
    noticer's id) are identical row for row, and so are `notice_incidents.csv` and `incidents.csv`;
    `results.csv` is compared on every column but the run id, the wall-clock columns and the bill
    (which carries the noticer's charge). A difference is listed row by row.
R6. **Intervals** are B2's: 90% percentile cluster bootstrap over whole streams, 10,000 resamples,
    seed `BOOT_SEED`, the same resamples for every arm and measure, so differences are paired.
"""

import json

import b3_common as B3
import l1_common as L

ROOT = L.ROOT
RUNS = L.RUNS
MANIFESTS = L.MANIFESTS
OUT = L.OUT
BIN = L.BIN
NS = L.NS
MS = L.MS
PRIMARY = L.PRIMARY
BOOT_SEED = L.BOOT_SEED
N_RESAMPLES = L.N_RESAMPLES
setting_id = L.setting_id
arm_name = L.arm_name  # sel_<name>_privileged
M = L.M

TUNING_SEEDS = (10_000, 100)
HELDOUT_SEEDS = (20_000, 200)
RUN_SEED = 15_000  # draws the arm order per stream
RESERVED_BG_BUDGET = 6.82  # B3's bound; the dataflow row is held to nothing new here

# the arms (name -> noticer json). `row_pieces` are B3's chosen ramp and split, as B3 recorded them.
B3_ROW = "ramp_split_over_re2"
DATAFLOW = "dataflow"
DATAFLOW_FREE = "dataflow_unbilled"
MEDIUM = "med_t100"


def b3_row():
    return L.ramp_split_over_re2()


def dataflow(billed=True):
    row = b3_row()
    out = {"noticer": "dataflow", "base": row["base"], "ramp": row["ramp"], "split": row["split"]}
    if not billed:
        out["billed"] = False
    return out


def medium():
    return L.medium_frozen()


def arms(stage):
    """(name, noticer) of every arm of a stage: 'tune' (the reproduction on the tuning streams) or
    'heldout'."""
    if stage == "tune":
        return [(DATAFLOW, dataflow()), (DATAFLOW_FREE, dataflow(False)), (B3_ROW, b3_row())]
    if stage == "heldout":
        return [(DATAFLOW, dataflow()), (DATAFLOW_FREE, dataflow(False)), (B3_ROW, b3_row()),
                (MEDIUM, medium())]
    raise SystemExit(f"unknown stage {stage!r}")


def seeds(stage):
    return TUNING_SEEDS if stage == "tune" else HELDOUT_SEEDS


def run_id(stage):
    b, rho = PRIMARY
    return f"c1-{stage}-{setting_id(b, rho)}"


def experiment(stage):
    return f"exploration-c1-{stage}"


def check_row():
    """The two spellings must agree on what they hold; a manifest is only as good as this."""
    d, h = dataflow(), b3_row()
    assert d["base"] == h["base"] and d["ramp"] == h["ramp"] and d["split"] == h["split"]
    with open(OUT / "b3-selected.json") as fh:
        sel = json.load(fh)
    assert B3.composed("re2", ramp=sel["ramp"]["chosen"]["params"],
                       split=sel["split_re2"]["chosen"]["params"]) == h
    return True
