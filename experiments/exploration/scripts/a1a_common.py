"""A1a, the engram (the build phase): the smoke run's settings and arms, in one place.

Exploration: nothing here tests a hypothesis, and nothing may later be cited as confirmation. A1a
has no criterion beyond its deliverables (docs/lab-queue.md, "## A1"); its smoke run shows that
recalls occur and declare, measured only by the existing per-incident columns of `incidents.csv`
(`correct_declarations` with `escalations` = 0), and nothing is tuned against any measure. The
engram parameters are the first values fixed in the adapter's module documentation
(`crates/gordian-run/src/stream/arms/medium/engram.rs`) before any run; they are copied here, not
chosen here.

Conventions are L1's (`scripts/l1_common.py`, `scripts/l1_manifests.py`): the selection oracle at
R5's delay (16 s) with the rung's own context, b = 5, rho = 0.7, seeds in stream order, so that the
engram arms carry what they bind from one segment to the next in that order. The noticer under
every arm is M3's frozen 100 ms medium (`m3-selected.json`; sub-tick pruning on, as the review log
decided after M3), with or without the engram layer.

Run directories live in artifacts/runs/ of this worktree (git-ignored), the smoke run under
artifacts/runs/a1a/; manifests in artifacts/runs/_manifests/.
"""

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts"))

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

SMOKE_SEEDS = (10_000, 20)  # 10000-10019, M2's tuning streams; the brief's smoke seeds
RUN_SEED = 15_000  # draws the arm order per stream
SMOKE_DIR = RUNS / "a1a"


def frozen():
    """M3's frozen 100 ms medium, as the manifest spells it (with the `noticer` tag)."""
    n = dict(M3.selected()["ticks"]["100"]["noticer"])
    assert n["noticer"] == "medium" and n["tick_ns"] == 100 * MS
    return n


# The first values of the adapter's module documentation, "Parameters", copied (not chosen here).
FIRST_VALUES = {
    "bind": True,
    "carry": True,
    "site": "family",
    "generalise": False,
    "confirm": "never",
    "key_span_ns": 2_000 * MS,
    "min_features": 2,
    "gain": 1.0,
    "threshold": 0.5,
    "max_strength": 4.0,
    "penalty": 1.0,
    "decay": 0.95,
    "decay_period_ns": 100_000 * MS,
    "refractory_ns": 6_000 * MS,
}


def with_engram(key, **over):
    """M3's frozen medium with an engram layer under `state_key` `key`, the first values, and
    `over` (the switches each arm names)."""
    e = dict(FIRST_VALUES, state_key=key)
    e.update(over)
    return dict(frozen(), engram=e)


def arms():
    """(name, noticer) of the smoke run. Every switch of the brief, each once, at the first
    values: the site switch (family, site), generalisation (off, on), the three confirmation
    policies, and two controls (no layer; the layer with bind off)."""
    return [
        ("m3", frozen()),
        ("eng_off", with_engram(1100, bind=False)),
        ("eng_family", with_engram(1101)),
        ("eng_site", with_engram(1102, site="site")),
        ("eng_family_gen", with_engram(1103, generalise=True)),
        ("eng_site_gen", with_engram(1104, site="site", generalise=True)),
        ("eng_family_k4", with_engram(1105, confirm={"every": {"k": 4}})),
        ("eng_family_contra", with_engram(1106, confirm="on_contradiction")),
    ]


def run_id(stage):
    b, rho = PRIMARY
    return f"a1a-{stage}-{setting_id(b, rho)}"
