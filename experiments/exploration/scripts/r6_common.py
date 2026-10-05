"""R6 context-construction headroom: the settings, the builder grids, the arm names, in one place.

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation).
Everything that determines what R6 ran is defined here and in `r4_common.py` / `r5_common.py`,
whose seeds, settings and delay grid R6 reuses unchanged, so the manifests and the analysis cannot
disagree. The grids below were fixed, as generic geometric ladders, before any R6 run (git log).

Run directories live in artifacts/runs/ (git-ignored); manifests in artifacts/runs/_manifests/.
"""

import os
import pathlib

import r5_common as C5

C4 = C5.C4
ROOT = C5.ROOT
RUNS = C5.RUNS
MANIFESTS = C5.MANIFESTS
OUT = C5.OUT
BIN = C5.BIN
NS = C5.NS

TUNING_SEEDS = C5.TUNING_SEEDS  # 10000-10099
HELDOUT_SEEDS = C5.HELDOUT_SEEDS  # 20000-20199
DIAG_SEEDS = (31_000, 100)  # the trace diagnostic: outside the tuning, held-out and R5 trace seeds
SETTINGS = C5.SETTINGS
PRIMARY = C5.PRIMARY
setting_id = C5.setting_id
load_json = C5.load_json

# ---- the builder grids (swept on the tuning streams only) -------------------------------------

WINDOW_W_S = [5, 10, 20, 40, 80]  # retention is 120 s
WINDOW_N = [8, 16, 32, 64, 128, 256, 512]
COOCCUR_DELTA_MS = [250, 500, 1000, 2000, 4000, 8000, 16000]
NEIGH_HOPS = [0, 1, 2, 3, 4, 6]
# The plan sweeps W x N, delta and k. The pilot (10 tuning streams, `r6-pilot-b5-rho0.7`) showed the
# rung's cap of 128 truncating the larger cooccur and neighbourhood contexts, which would confound
# their parameters with a cap nobody chose, so their cap is swept too (a superset of the plan's
# sweep, more generous to the public builders), and window's N goes to 512.
CAPS = [64, 128, 256]

# R5's committed run outputs are in the main checkout's ignored artifacts directory, not in this
# worktree's. Read only: R6 never writes there.
R5_RUNS = pathlib.Path(os.environ.get("R5_RUNS", "/home/user/gordian/artifacts/runs/r5"))

ALWAYS_DELAYS_S = list(C4.ALWAYS_DELAYS_S)

ORACLE = C4.ORACLE
ABLATION = C4.ABLATION
NEVER = C4.NEVER

KINDS = ("rung", "window", "cooccur", "neighbourhood")


def builder_key(kind, **p):
    """The short name of one builder configuration, used inside arm names."""
    if kind == "rung":
        return "rung"
    if kind == "window":
        return f"win_w{p['w']:02d}_n{p['n']:03d}"
    if kind == "cooccur":
        return f"coc_d{p['d_ms']:05d}_n{p['n']:03d}"
    if kind == "neighbourhood":
        return f"nbh_k{p['k']}_n{p['n']:03d}"
    raise KeyError(kind)


def builder_spec(kind, **p):
    """The manifest spelling of a builder, or None for the default (never written)."""
    if kind == "rung":
        return None
    if kind == "window":
        return {"builder": "window", "window_ns": p["w"] * NS, "max_refs": p["n"]}
    if kind == "cooccur":
        return {"builder": "cooccur", "delta_ns": p["d_ms"] * 1_000_000, "max_refs": p["n"]}
    if kind == "neighbourhood":
        return {"builder": "neighbourhood", "hops": p["k"], "max_refs": p["n"]}
    raise KeyError(kind)


def all_builders():
    """Every builder configuration of the sweep: (key, kind, params)."""
    out = [(builder_key("rung"), "rung", {})]
    for w in WINDOW_W_S:
        for n in WINDOW_N:
            out.append((builder_key("window", w=w, n=n), "window", {"w": w, "n": n}))
    for d in COOCCUR_DELTA_MS:
        for n in CAPS:
            out.append((builder_key("cooccur", d_ms=d, n=n), "cooccur", {"d_ms": d, "n": n}))
    for k in NEIGH_HOPS:
        for n in CAPS:
            out.append((builder_key("neighbourhood", k=k, n=n), "neighbourhood", {"k": k, "n": n}))
    return out


BUILDERS = {key: (kind, p) for key, kind, p in all_builders()}


def sel_arm(key, delay_s):
    """(arm name, policy, context) of the selection oracle at `delay_s` with builder `key`."""
    kind, p = BUILDERS[key]
    pol = {"policy": "oracle_selection"}
    if delay_s:
        pol["delay_ns"] = delay_s * NS
    return (f"sel_{key}_privileged", pol, builder_spec(kind, **p))


def always_arm(key, delay_s):
    kind, p = BUILDERS[key]
    pol = "always_escalate" if delay_s == 0 else {"policy": "always_escalate", "delay_ns": delay_s * NS}
    return (f"alw_{key}_d{delay_s:02d}", pol, builder_spec(kind, **p))


def ctxonly_arm(delay_s):
    pol = {"policy": "oracle_selection_context"}
    if delay_s:
        pol["delay_ns"] = delay_s * NS
    return (f"oracle_selection_context_d{delay_s:02d}_privileged", pol, None)


def reference_arms():
    """R4's oracle, the labelled ablation, never_escalate: no builder (they use the rung's)."""
    return [
        (ORACLE, "oracle_escalation", None),
        (ABLATION, "ablation_hidden_rules", None),
        (NEVER, "never_escalate", None),
    ]


def kind_of_arm(name):
    """The builder key an `sel_*` or `alw_*` arm name carries, or None."""
    for prefix in ("sel_", "alw_"):
        if name.startswith(prefix):
            rest = name[len(prefix):]
            for key in BUILDERS:
                if rest == key + "_privileged" or rest.startswith(key + "_d"):
                    return key
    return None


def run_id(stage, b, rho):
    return f"r6-{stage}-{setting_id(b, rho)}"
