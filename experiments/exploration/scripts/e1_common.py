"""E1, memory measures and the record rung: the settings, the arms and the seed ranges, in one
place.

Exploration: nothing here tests a hypothesis, and nothing may later be cited as confirmation. E1's
criterion is its deliverables, byte identity, the evaluator's fixtures and mutation checks, the V1
back-test unchanged and the record rung's held-out table with W2's ceiling beside it
(`docs/lab-queue.md`, "## E1"); no claim is made about the record rung.

The record rung's key, gate, policies and tuning rule are written in the module documentation of
`crates/gordian-run/src/stream/arms/noticer_record.rs`, committed before any run (commit 75e7298);
this file copies them and chooses nothing. Conventions are L1's and A1a's: the selection oracle at
R5's delay (16 s) with the rung's own context, b = 5, rho = 0.7, seeds in stream order (so that an
arm that carries a table carries it in that order), the noticer under every arm the later re-anchor
B2 selected.

Run directories live in artifacts/runs/e1/ of this worktree (git-ignored); manifests in
artifacts/runs/_manifests/.
"""

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts"))
sys.path.insert(0, str(ROOT / "analysis"))

import m3_common as M3  # noqa: E402

RUNS = M3.RUNS
MANIFESTS = M3.MANIFESTS
OUT = M3.OUT
BIN = M3.BIN
NS = M3.NS
MS = M3.MS
PRIMARY = M3.PRIMARY  # b = 5, rho = 0.7
setting_id = M3.setting_id
arm_name = M3.arm_name  # sel_<name>_privileged
SEL_POLICY = M3.SEL_POLICY
sel_delay_s = M3.sel_delay_s
REANCHOR = dict(M3.REANCHOR)  # B2's selected later re-anchor, in the manifest's spelling

E1_DIR = RUNS / "e1"
TUNING_SEEDS = (10_000, 100)  # 10000-10099
HELDOUT_SEEDS = (40_000, 200)  # 40000-40199
RUN_SEED = 16_000  # draws the arm order per stream

# Parameters the module documentation fixes (not tuned).
SETTLE_NS = 1_000 * MS
LEVELS = ("kinds", "bands", "timing")
KS = (2, 4, 8)
# W2's tuning constraint (section 10.2): of the recalls whose stored answer was right for its own
# incident, at most this share are wrong, and at most this many such wrong recalls per stream.
BOUND_SHARE = 0.20
BOUND_PER_STREAM = 0.25

FORMS = ("site", "family")
RESETS = (True, False)


def run_id(stage):
    b, rho = PRIMARY
    return f"e1-{stage}-{setting_id(b, rho)}"


def record(form, level, confirm, reset, key, msg_ids=False):
    """The record rung as the manifest spells it: the later re-anchor underneath."""
    n = {
        "noticer": "record",
        "base": dict(REANCHOR),
        "form": form,
        "level": level,
        "confirm": confirm,
        "reset": reset,
        "settle_ns": SETTLE_NS,
        "state_key": key,
    }
    if msg_ids:
        n["msg_ids"] = True
    return n


def confirm_name(c):
    if c == "never":
        return "never"
    if c == "on_contradiction":
        return "contra"
    return f"k{c['every']['k']}"


def state_key(form, reset, confirm, level, stage):
    """A distinct carry key per arm of a run (the process-wide store is keyed by it)."""
    base = {"tune1": 20_000, "tune2": 21_000, "heldout": 22_000, "smoke": 23_000}[stage]
    c = {"never": 0, "on_contradiction": 1}.get(confirm if isinstance(confirm, str) else "", None)
    if c is None:
        c = 1 + confirm["every"]["k"]
    # A mixed radix: the form, the reset setting, the policy and the level cannot collide (the
    # tuning runs were written with a formula that could, in a run holding both a carried arm and an
    # `every` arm; the keys of those runs were distinct, and a key never changes what an arm does).
    return base + 4_000 * (form == "family") + 2_000 * (not reset) + 10 * c + LEVELS.index(level)


def name_of(form, level, confirm, reset):
    return f"rec_{form}_{level}_{confirm_name(confirm)}_{'reset' if reset else 'carry'}"


def control():
    """The memoryless re-anchor, B2's comparator: the arm W2's ceiling is."""
    return ("reanchor", dict(REANCHOR))
