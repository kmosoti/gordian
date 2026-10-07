"""A1d, the engram under a non-privileged selector: the smoke run's settings and arms, in one place.

Exploration: nothing here tests a hypothesis, and nothing may later be cited as confirmation.
A1d's acceptance (docs/lab-queue.md, "## A1d", fixed by the chief 2026-10-07) is deliverables,
identity, the test of item 2 and the smoke table on decision columns; the PI's prediction is written
before the run (`experiments/exploration/a1d-negative-experience.md`, "Prediction"), and whatever
the table shows is reported. No claim, no tuning.

The smoke is A1a's and A1c's conventions (`a1a_common`: seeds 10000-10019 in stream order, b = 5,
rho = 0.7, M3's frozen 100 ms medium under every arm, run seed 15000) with three selectors, each
asking at R5's 16 s with the rung's context:

- `thr`: B4's public threshold rule at B4's merged choice for the medium row, `t` = 1 s
  (`b4-selected.json`, `select.medium_t100.thr_t_s`; read from the file, never retuned). **The
  primary public selector of this report** (fixed here, before the run): its reading of "the rung
  cannot resolve it" is the rung's own conclusion and the public checker, the instrument EXP-101's
  draft names first, and it is the row B4's table led with.
- `chg`: B4's public change rule at B4's merged choice, `k` = 12 (`select.medium_t100.chg_k`), a
  second public selector, reported beside.
- `sel`: the selection oracle (`oracle_selection`), the labelled ceiling (hard-incident quality
  only; review log, B4).

Under each selector, five arms (the brief: "the memoryless arm (M3's medium noticer) and the engram
forms (family, two-site, site-keyed)", plus one control):

- `m3`: M3's frozen medium, no engram layer. **The memoryless arm every engram arm is paired
  against, under the same selector.**
- `eng_off`: the layer with bind off and the A1d gate on: no recall ever, but the rung keeps the
  checker's verdicts (the layer's and the checks' cost; monitoring alone; a labelled control).
- `eng_family`: family-keyed, generalising, never confirming (A1a's main form), the A1d gate.
- `eng_2site`: the same with A1c's two-site key.
- `eng_site`: site-keyed, reset per stream (A1a's `eng_site`), the A1d gate.

Every engram parameter is A1a's first value (`a1a_common.FIRST_VALUES`); the A1d arms add only
`gate: stale` (the rule of item 2) and `trace: true` (the counters), and the late-feature switch at
its default for a gated layer (off), as A1c's main gated forms. Nothing is chosen here against a
measure, and nothing is changed after the run.
"""

import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import a1a_common as A  # noqa: E402
import a1c_common as C1c  # noqa: E402

ROOT = A.ROOT
RUNS = A.RUNS
MANIFESTS = A.MANIFESTS
OUT = A.OUT
BIN = A.BIN
NS = A.NS
MS = A.MS
PRIMARY = A.PRIMARY
setting_id = A.setting_id
M3 = A.M3

SMOKE_SEEDS = A.SMOKE_SEEDS
RUN_SEED = A.RUN_SEED
RUN_DIR = RUNS / "a1d"
A1A_SMOKE = C1c.A1A_SMOKE
A1C_SMOKE = pathlib.Path("/home/user/gordian/artifacts/runs/a1c/a1c-smoke-b5-rho0.7")
TRACE_DIR = "_trace"  # inside a run directory: the arms' trace files (engram-trace-<key>.csv)
TRACE_ENV = "GORDIAN_ENGRAM_TRACE_DIR"

DELAY_S = A.sel_delay_s()  # R5's 16 s
B4_STEM = "medium_t100"


def b4_constants():
    """B4's merged choices for the medium row: (threshold t in s, change k)."""
    with open(OUT / "b4-selected.json") as fh:
        sel = json.load(fh)["select"][B4_STEM]
    return sel["thr_t_s"], sel["chg_k"]


THR_T_S, CHG_K = b4_constants()
assert (THR_T_S, CHG_K) == (1, 12), "B4's merged constants for the medium row"


def selectors():
    """(prefix, policy) of the three selectors, the primary first."""
    thr = {"policy": "public_threshold", "delay_ns": DELAY_S * NS}
    if THR_T_S:
        thr["persist_ns"] = round(THR_T_S * NS)
    return [
        ("thr", thr),
        ("chg", {"policy": "public_change", "delay_ns": DELAY_S * NS, "k": CHG_K}),
        ("sel", {"policy": A.SEL_POLICY, "delay_ns": DELAY_S * NS}),
    ]


A1D = {"gate": "stale", "trace": True}
FORMS = ["m3", "eng_off", "eng_family", "eng_2site", "eng_site"]
ENGRAM_FORMS = ["eng_family", "eng_2site", "eng_site"]


def noticer(form, key):
    w = A.with_engram
    return {
        "m3": lambda: A.frozen(),
        "eng_off": lambda: w(key, bind=False, **A1D),
        "eng_family": lambda: w(key, **A1D),
        "eng_2site": lambda: w(key, two_site=True, **A1D),
        "eng_site": lambda: w(key, site="site", **A1D),
    }[form]()


def arm_name(prefix, form):
    if prefix == "sel":
        return A.arm_name(form)  # sel_<form>_privileged, as A1a's and A1c's oracle arms
    if prefix == "thr":
        return f"thr_t{THR_T_S:g}_{form}"
    return f"chg_k{CHG_K}_{form}"


def arms():
    """(arm name, selector prefix, form, policy, noticer) of the smoke run, each engram arm under
    its own state key (1300 + its index)."""
    out = []
    key = 1300
    for prefix, policy in selectors():
        for form in FORMS:
            out.append((arm_name(prefix, form), prefix, form, policy, noticer(form, key)))
            key += 1
    return out


def run_id(stage):
    b, rho = PRIMARY
    return f"a1d-{stage}-{setting_id(b, rho)}"
