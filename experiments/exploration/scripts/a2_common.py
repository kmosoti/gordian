"""A2, anticipation of hidden edges: the smoke run's settings and arms, and the identity replays, in
one place.

Exploration: nothing here tests a hypothesis, and nothing may later be cited as confirmation. A2's
acceptance (docs/lab-queue.md, "## A2", fixed by the chief 2026-10-07) is deliverables, identity,
the PI's prediction committed before the run (`experiments/exploration/a2-anticipation.md`,
"Prediction", and `PREDICTION` below) and the trace file joinable by Lab 3. No claim, no tuning.

The smoke is A1a's conventions (`a1a_common`: seeds 10000-10019 in stream order, b = 5, rho = 0.7,
M3's frozen 100 ms medium under every arm, run seed 15000), under the selection oracle at R5's
16 s with the rung's context (the brief), with three arms:

- `m3`: M3's frozen medium, no layer. The memoryless arm.
- `a2_learn`: the same medium with the anticipation layer at its first values (the adapter's module
  documentation, `crates/gordian-run/src/stream/arms/medium/anticipation.rs`, and `DESIGN.md`,
  "Anticipation of hidden edges (A2)", departures 78 to 83), the attach switch off, the trace on.
  **The learner.**
- `a2_raw`: the learner with `explained` off (every first alarm counts): a labelled control that
  shows what the public graph's explanation does.

Every value is the first value; nothing is chosen here against a measure, and nothing is changed
after the run. The trace files go to `<run dir>/_trace/anticipation-trace-<key>.csv` through
`GORDIAN_ENGRAM_TRACE_DIR` (A1d's variable), keys 1400 and 1401.
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
M3 = A.M3

SMOKE_SEEDS = A.SMOKE_SEEDS
RUN_SEED = A.RUN_SEED
RUN_DIR = RUNS / "a2"
KEPT = pathlib.Path("/home/user/gordian/artifacts/runs")
A1A_SMOKE = KEPT / "a1a" / "a1a-smoke-b5-rho0.7"
A1C_SMOKE = KEPT / "a1c" / "a1c-smoke-b5-rho0.7"
A1D_SMOKE = KEPT / "a1d" / "a1d-smoke-b5-rho0.7"
TRACE_DIR = "_trace"
TRACE_ENV = "GORDIAN_ENGRAM_TRACE_DIR"
DELAY_S = A.sel_delay_s()  # R5's 16 s

BANDS_NS = [400 * MS, 2_000 * MS, 10_000 * MS]

# The first values of the adapter's module documentation (copied, not chosen here).
FIRST_VALUES = {
    "bands_ns": BANDS_NS,
    "gain": 1.0,
    "threshold": 2.0,
    "tau_ns": 150_000 * MS,
    "explained": True,
    "attach": False,
}

KEYS = {"a2_learn": 1400, "a2_raw": 1401}


def with_layer(key, **over):
    """M3's frozen medium with the anticipation layer under trace key `key`, the first values,
    the trace on, and `over`."""
    a = dict(FIRST_VALUES, trace_key=key, trace=True)
    a.update(over)
    return dict(A.frozen(), anticipation=a)


def arms():
    """(form, noticer) of the smoke run."""
    return [
        ("m3", A.frozen()),
        ("a2_learn", with_layer(KEYS["a2_learn"])),
        ("a2_raw", with_layer(KEYS["a2_raw"], explained=False)),
    ]


def arm_name(form):
    return A.arm_name(form)  # sel_<form>_privileged


def run_id(stage):
    b, rho = PRIMARY
    return f"a2-{stage}-{setting_id(b, rho)}"


# The PI's prediction, committed before any A2 run (the report's "Prediction" section says why).
# Per stream means over the 20 streams, with 80% ranges; probabilities are the PI's.
PREDICTION = {
    "a2_learn": {
        "pair_cells_pairs_per_stream": (33, 25, 40),
        "edges_learned_per_stream_any_band": (12, 3, 28),
        "edges_learned_per_stream_by_band": {"0.4s": (2, 0, 8), "2s": (10, 2, 25), "10s": (1, 0, 10)},
        "predictions_per_stream": (60, 8, 350),
        "prediction_share_by_band": {"0.4s": (0.12, 0.03, 0.30), "2s": (0.80, 0.55, 0.95),
                                     "10s": (0.04, 0.0, 0.15)},
        "public_follow_rate_by_band": {"0.4s": (0.10, 0.03, 0.30), "2s": (0.33, 0.18, 0.50),
                                       "10s": (0.85, 0.60, 0.97)},
        "follow_rate_within_0.10_of_all_trials_rate_in_each_band_p": 0.75,
        "all_trials_follow_rate_at_least_mean_chance_in_each_band_p": 0.6,
        "predictions_on_true_hidden_edges_share_below_0.05_p": 0.8,
        "bill_compute_ms_per_stream_added": (2.0, 1.3, 4.0),
        "decision_columns_equal_to_m3_p": 0.9,
    },
    "a2_raw": {
        "edges_learned_0.4s_at_least_twice_the_learners_p": 0.7,
        "follow_rate_0.4s_above_the_learners_p": 0.7,
        "predictions_per_stream": (150, 20, 600),
    },
}
