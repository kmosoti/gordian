"""R4 headroom check: the settings, the baseline grids and the arm names, in one place.

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation).
Everything that determines what R4 ran is defined here and nowhere else, so that the manifests
and the analysis cannot disagree about it.

Run directories live in artifacts/runs/ (git-ignored); manifests in artifacts/runs/_manifests/.
"""

import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[3]
RUNS = ROOT / "artifacts" / "runs"
MANIFESTS = RUNS / "_manifests"
OUT = ROOT / "experiments" / "exploration"
BIN = ROOT / "target" / "release" / "gordian-run"

TUNING_SEEDS = (10_000, 100)  # start, count: 10000-10099
HELDOUT_SEEDS = (20_000, 200)  # 20000-20199

# Reasoner settings. The informativeness b is swept at a low, the default and a high value;
# rho at 0 and at the default 0.7; a, c and the cost are the stream's defaults (-1, 2).
# h(q = 1, d) = (sigma(a + b - c d) - sigma(a - c d)) / (1 - sigma(a - c d)), the informed
# probability, at the mean hard difficulty d = 0.7 (DESIGN.md section 11): 0.48, 0.92, 1.00.
B_VALUES = (2.5, 5.0, 8.0)
RHO_VALUES = (0.0, 0.7)
SETTINGS = [(b, rho) for b in B_VALUES for rho in RHO_VALUES]


def setting_id(b, rho):
    return f"b{b:g}-rho{rho:g}"


# ---- the baselines' parameter grids (tuned on the tuning streams only) -----------------------

ALWAYS_DELAYS_S = [0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 25, 30]
RANDOM_P = [0.03, 0.06, 0.12, 0.25, 0.5, 0.8]
RANDOM_DELAYS_S = [0, 6, 10, 14, 18]
PERIODIC_PERIODS_S = [1, 2, 3, 4, 6, 8, 10, 12, 15, 20, 30, 45]
THRESHOLD_TAU = [3.0, 4.0, 5.0, 6.0, 8.0, 12.0, 20.0]
THRESHOLD_WAIT_S = [2, 6, 12, 20, 40]

ORACLE = "oracle_escalation_privileged"
ABLATION = "ablation_hidden_rules"
NEVER = "never_escalate"
CHANGE = "change_triggered"

NS = 1_000_000_000


def always_name(d):
    return f"always_d{d:02d}"


def random_name(p, d):
    return f"random_p{round(p * 100):03d}_d{d:02d}"


def periodic_name(t):
    return f"periodic_t{t:02d}"


def threshold_name(tau, w):
    return f"thr_tau{tau:g}_w{w:02d}"


def baseline_arms():
    """Every non-privileged, non-ablation configuration of the tuning grid, as
    `(arm name, policy)` in the manifest's spelling."""
    arms = [(NEVER, NEVER), (CHANGE, CHANGE)]
    for d in ALWAYS_DELAYS_S:
        arms.append(
            (
                always_name(d),
                "always_escalate" if d == 0 else {"policy": "always_escalate", "delay_ns": d * NS},
            )
        )
    for p in RANDOM_P:
        for d in RANDOM_DELAYS_S:
            pol = {"policy": "random_escalation", "p": p}
            if d:
                pol["delay_ns"] = d * NS
            arms.append((random_name(p, d), pol))
    for t in PERIODIC_PERIODS_S:
        arms.append((periodic_name(t), {"policy": "periodic_escalation", "period_ns": t * NS}))
    for tau in THRESHOLD_TAU:
        for w in THRESHOLD_WAIT_S:
            arms.append(
                (
                    threshold_name(tau, w),
                    {"policy": "threshold_score", "tau": tau, "wait_ns": w * NS},
                )
            )
    return arms


def reference_arms():
    """The privileged oracle and the labelled ablation."""
    return [(ORACLE, "oracle_escalation"), (ABLATION, "ablation_hidden_rules")]


def policy_for(name):
    """The manifest spelling of a baseline arm by its name."""
    for n, pol in baseline_arms() + reference_arms():
        if n == name:
            return pol
    raise KeyError(name)


def run_id(stage, b, rho):
    return f"r4-{stage}-{setting_id(b, rho)}"


def load_json(path):
    with open(path) as fh:
        return json.load(fh)
