"""R7 reasoner-law sensitivity: the runs, the arm names and the selection of what is carried, in one
place.

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation).
R7 repeats R6's comparison on a reasoner whose informed probability falls with the number of
references in the context that are not decisive evidence (`distractor_penalty`, `delta`, per 100
references). Everything that determines what R7 ran is defined here and in `r6_common.py`, whose
seeds, settings, builder grids, arm names and selection-oracle delay R7 reuses unchanged, so the
manifests and the analysis cannot disagree. The rules below were written before any R7 run
(git log).

Run directories live in artifacts/runs/ (git-ignored); manifests in artifacts/runs/_manifests/.
"""

import r6_common as C6

C5 = C6.C5
C4 = C6.C4
ROOT = C6.ROOT
RUNS = C6.RUNS
MANIFESTS = C6.MANIFESTS
OUT = C6.OUT
BIN = C6.BIN
NS = C6.NS
TUNING_SEEDS = C6.TUNING_SEEDS  # 10000-10099, R6's
HELDOUT_SEEDS = C6.HELDOUT_SEEDS  # 20000-20199, R6's
KINDS = C6.KINDS
BUILDERS = C6.BUILDERS
load_json = C6.load_json
setting_id = C6.setting_id

PRIMARY = C6.PRIMARY  # (b, rho) = (5, 0.7)
DELTAS = (0.05, 0.1, 0.2, 0.4)  # per 100 non-decisive references: the plan's grid
SENS_DELTA = 0.2
# The runs, in a fixed order: the primary setting at every delta, then b = 2.5 and b = 8 at
# rho = 0.7 and delta 0.2. Each is run once on the tuning streams and once on the held-out streams.
RUNS_SPEC = [(PRIMARY[0], PRIMARY[1], d) for d in DELTAS] + [
    (2.5, 0.7, SENS_DELTA),
    (8.0, 0.7, SENS_DELTA),
]

# The criterion's bootstrap: 10,000 resamples of whole streams, 90% equal-tailed. The stream seeds
# are R6's. The bootstrap seeds start at R6's own base (r6_report.SEED0) and add the index of the
# run in RUNS_SPEC.
N_RESAMPLES = 10_000
BOOT_SEED0 = 9_600
# Run seeds (they order the interleaved arms and nothing else).
TUNE_RUN_SEED0 = 7_100
HELDOUT_RUN_SEED0 = 7_300

# The criterion's definitions (docs/local-test-plan.md, 5R, R7), as numbers.
ROBUST_UPPER = 0.10  # robust: at every delta the upper bound of G is below this
FRAGILE_G = 0.10  # fragile: G at least this ...
FRAGILE_LOWER = 0.05  # ... with its lower bound above this
STRONG_DELTA = 0.4  # "fragile only under a strong penalty": the fragile condition at this delta only
LOW_DELTAS = (0.05, 0.1, 0.2)  # "at some delta <= 0.2"
WITHIN = 0.05  # "within 0.05 of the ceiling"
CARRY_NEXT = 3  # "with the three next-best on R6's frontier"

# The per-reference price, re-priced analytically at these tokens per reference.
REPRICE_TOKENS = (5, 20, 80)
BASE_TOKENS = 400  # the world's declared base per call (unchanged by re-pricing)
NS_PER_TOKEN = 250_000  # the manifest's exchange rate (unchanged by re-pricing)
TOKEN_LIMIT = 80_000  # the manifest's `limits.reasoner_tokens` per stream


def dlabel(delta):
    return f"{delta:g}"


def run_id(stage, b, rho, delta):
    return f"r7-{stage}-{setting_id(b, rho)}-d{dlabel(delta)}"


def run_index(b, rho, delta):
    return RUNS_SPEC.index((b, rho, delta))


def sel_delay_s(b, rho):
    """The selection oracle's delay: R6's (r6-selected.json), not re-tuned."""
    return load_json(OUT / "r6-selected.json")[setting_id(b, rho)]["selection_delay_s"]


def r6_frontier_configs(b, rho):
    """The configurations on R6's tuning frontier at delta = 0 for this setting (the frontier of
    each builder kind, plus the rung), `r6-frontier.json`."""
    return list(load_json(OUT / "r6-frontier.json")[setting_id(b, rho)]["configs"])


GRID_ORDER = {key: i for i, (key, _, _) in enumerate(C6.all_builders())}


def kind_of_key(key):
    return BUILDERS[key][0]


def sel_arm_name(key, delay_s):
    return C6.sel_arm(key, delay_s)[0]


def ctxonly_name(delay_s):
    return C6.ctxonly_arm(delay_s)[0]


ORACLE = C6.ORACLE  # R4's oracle: a reference row only
