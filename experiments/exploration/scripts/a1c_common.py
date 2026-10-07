"""A1c, the recall gate and the two-site key: the smoke run's settings and arms, in one place.

Exploration: nothing here tests a hypothesis, and nothing may later be cited as confirmation.
A1c's acceptance (docs/lab-queue.md, "## A1c", fixed by the chief 2026-10-07) is deliverables,
identity, the displacement test and the smoke table; "a gated family form whose plain unasked-wrong
count exceeds the control's (53) by more than 5 in 20 streams is reported as such; a count within 5
is the expected outcome. No claim either way, and no tuning."

The smoke is A1a's manifest (`a1a_common`: seeds 10000-10019 in stream order, b = 5, rho = 0.7,
the selection oracle at 16 s with the rung's context, M3's frozen 100 ms medium under every arm,
run seed 15000) with A1a's eight arm roles **exactly as A1a wrote them** (`a1a_common.arms()`,
imported, not copied) plus the gated forms. Every engram parameter is A1a's first value
(`a1a_common.FIRST_VALUES`); the gated forms add only the A1c switches, at the values fixed in the
adapter's documentation before any run (`gate`, `late_feature`, `two_site`). Nothing is chosen
here against a measure, and nothing is changed after the run.

The gated forms (the brief: "gated family generalising; gated family with the two-site key; gated
site-keyed"), each with the late-feature switch at its default (off when gated) and on (the brief:
"report both ways"), and one control:

- `eng_off_gated`: the layer with bind off and the gate on: no recall ever, but the rung keeps the
  checker's verdicts (the gate's cost, and a check that monitoring alone changes no declaration).
- `eng_family_gated`: the main gated form (family-keyed, generalising, never confirming, late off).
  **Its plain unasked-wrong count is the first number of the report.**
- `eng_family_gated_2site`: the same with the two-site key.
- `eng_site_gated`: site-keyed (reset per stream, as A1a's `eng_site`), gated.
- `eng_family_gated_late`, `eng_family_gated_2site_late`, `eng_site_gated_late`: the same three
  with A1a's late-feature requirement on.

The PI's prediction, written before any A1c run (the review log's R5 entry: "the public checker
contradicts 97% of plain anomalies at some point"): the gate removes part of A1a's excess but not
enough; the gated family form's plain unasked-wrong count will exceed 58 (the control's 53 plus 5),
probability about 0.6. Hard unasked-correct for the gated family form at most A1a's 3 (the gate may
also close on the slow leaks, which the checker can explain with one known kind). The two-site form
recalls less on plain incidents than the one-site gated form, and its hard recalls are too few in
20 streams to say anything.
"""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import a1a_common as A  # noqa: E402

ROOT = A.ROOT
RUNS = A.RUNS
MANIFESTS = A.MANIFESTS
OUT = A.OUT
BIN = A.BIN
NS = A.NS
MS = A.MS
PRIMARY = A.PRIMARY
setting_id = A.setting_id
arm_name = A.arm_name
SEL_POLICY = A.SEL_POLICY
sel_delay_s = A.sel_delay_s
M3 = A.M3

SMOKE_SEEDS = A.SMOKE_SEEDS
RUN_SEED = A.RUN_SEED
SMOKE_DIR = RUNS / "a1c"
A1A_SMOKE = pathlib.Path("/home/user/gordian/artifacts/runs/a1a/a1a-smoke-b5-rho0.7")

GATE = {"gate": "contradicted"}


def arms():
    """A1a's eight arms, unchanged, then the gated forms."""
    w = A.with_engram
    return A.arms() + [
        ("eng_off_gated", w(1200, bind=False, **GATE)),
        ("eng_family_gated", w(1201, **GATE)),
        ("eng_family_gated_2site", w(1202, two_site=True, **GATE)),
        ("eng_site_gated", w(1203, site="site", **GATE)),
        ("eng_family_gated_late", w(1204, late_feature=True, **GATE)),
        ("eng_family_gated_2site_late", w(1205, two_site=True, late_feature=True, **GATE)),
        ("eng_site_gated_late", w(1206, site="site", late_feature=True, **GATE)),
    ]


A1A_ARMS = [n for n, _ in A.arms()]


def run_id(stage):
    b, rho = PRIMARY
    return f"a1c-{stage}-{setting_id(b, rho)}"
