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
