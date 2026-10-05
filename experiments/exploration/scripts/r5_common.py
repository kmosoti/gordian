"""R5 decomposed headroom: the settings, the new arms' grids and names, in one place.

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation).
Everything that determines what R5 ran is defined here and in `r4_common.py`, whose seeds,
settings and baseline grids R5 reuses unchanged, so the manifests and the analysis cannot disagree.

Run directories live in artifacts/runs/ (git-ignored); manifests in artifacts/runs/_manifests/.
"""

import r4_common as C4

ROOT = C4.ROOT
RUNS = C4.RUNS
MANIFESTS = C4.MANIFESTS
OUT = C4.OUT
BIN = C4.BIN
NS = C4.NS

TUNING_SEEDS = C4.TUNING_SEEDS  # 10000-10099
HELDOUT_SEEDS = C4.HELDOUT_SEEDS  # 20000-20199
DIAG_SEEDS = (30_000, 100)  # the trace diagnostic: outside both sets above

SETTINGS = C4.SETTINGS
PRIMARY = (5.0, 0.7)
setting_id = C4.setting_id

# ---- the new arms' grids (tuned on the tuning streams only) ----------------------------------

# The delay grid is `always_escalate`'s, as the plan says for the selection oracle; the
# contradiction arm uses it too, crossed with a persistence grid.
CONTRA_DELAYS_S = list(C4.ALWAYS_DELAYS_S)
CONTRA_PERSIST_S = [0, 1, 2, 4, 8]
SEL_DELAYS_S = list(C4.ALWAYS_DELAYS_S)

CONTRA_POLICY = "contradiction_escalation"
SEL_POLICY = "oracle_selection"
DECOY_POLICY = "oracle_decoy"
DECOY = "oracle_decoy_privileged"

ORACLE = C4.ORACLE  # R4's oracle, which bundles selection, timing and context
ABLATION = C4.ABLATION
NEVER = C4.NEVER


def contra_name(d, p):
    return f"contra_d{d:02d}_p{p:02d}"


def sel_name(d):
    return f"oracle_selection_d{d:02d}_privileged"


def contra_policy(d, p):
    """The manifest spelling of the contradiction arm; zeros are not written."""
    if d == 0 and p == 0:
        return CONTRA_POLICY
    pol = {"policy": CONTRA_POLICY}
    if d:
        pol["delay_ns"] = d * NS
    if p:
        pol["persist_ns"] = p * NS
    return pol


def sel_policy(d):
    if d == 0:
        return SEL_POLICY
    return {"policy": SEL_POLICY, "delay_ns": d * NS}


def contra_arms():
    return [(contra_name(d, p), contra_policy(d, p)) for d in CONTRA_DELAYS_S for p in CONTRA_PERSIST_S]


def sel_arms():
    return [(sel_name(d), sel_policy(d)) for d in SEL_DELAYS_S]


def policy_for(name):
    """The manifest spelling of any arm R5 runs, by name (R4's arms through `r4_common`)."""
    if name == DECOY:
        return DECOY_POLICY
    for n, pol in contra_arms() + sel_arms():
        if n == name:
            return pol
    return C4.policy_for(name)


def run_id(stage, b, rho):
    return f"r5-{stage}-{setting_id(b, rho)}"


load_json = C4.load_json
