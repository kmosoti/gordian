"""B2: power for the proxy pairs. Writes b2.json into $GORDIAN_WORK.

Stage B exploration script (development run; nothing here tests a hypothesis).
Working directory for outputs and manifests: $GORDIAN_WORK (default /tmp/gordian-exploration).
The repository root is found from this file's location.
"""
import json
import subprocess
import sys
from multiprocessing import Pool

import numpy as np
import pandas as pd

import os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import *  # noqa  (ROOT, S_DIR, loaders)

CLI = os.environ.get("GORDIAN_ANALYZE", "gordian-analyze")
MARGIN = 0.01
THRESH = 0.20
Z = {0.8: 0.8416212335729143, 0.9: 1.2815515655446004}
Z_A = 1.6448536269514722


def cli_power(sd, power, true_diff=0.0, margin=MARGIN):
    if not sd > 0:
        return None
    if margin + true_diff <= 0:
        return "inferior"
    r = subprocess.run(
        [CLI, "power", "--sd", repr(float(sd)), "--margin", repr(margin), "--alpha", "0.05",
         "--power", str(power), "--true-diff", repr(float(true_diff)), "--json", f"{S_DIR}/pw.json"],
        check=True, capture_output=True)
    return json.load(open(f"{S_DIR}/pw.json"))["n"]


# ---------- cost ratio ----------
def lower_limit(a, b, rng, boots):
    n = len(a)
    idx = rng.integers(0, n, size=(boots, n))
    s = 1.0 - b[idx].sum(1) / a[idx].sum(1)
    return np.quantile(s, 0.05)  # equal-tailed 90% interval: lower limit


def sim_power(args):
    a, b, n, reps, boots, seed, thresh = args
    rng = np.random.default_rng(seed)
    N = len(a)
    hit = 0
    for _ in range(reps):
        i = rng.integers(0, N, size=n)
        if lower_limit(a[i], b[i], rng, boots) > thresh:
            hit += 1
    return hit / reps


def delta_n(a, b, s_true, power, thresh=THRESH):
    """Delta-method n for P(lower limit > thresh) = power when true S = s_true, from the
    per-episode paired structure: Var(S_hat) ~ var_i(B_i - R A_i) / (n * Abar^2)."""
    if s_true <= thresh:
        return None
    R = b.sum() / a.sum()
    sd1 = np.std(b - R * a, ddof=1) / a.mean()
    # the interval is S_hat -/+ z * se; reject when S_hat - z_a * se > thresh
    z = Z_A + Z[power]
    return float((z * sd1 / (s_true - thresh)) ** 2)


def rescale(a, b, s_target):
    R = b.sum() / a.sum()
    return a, b * (1.0 - s_target) / R


if __name__ == "__main__":
    df = load_all()
    PAIRS = [("heuristic_only", "fixed_heuristic_every2"), ("all_components", "random_p050")]
    out = {"success": [], "success_planning": [], "cost": []}

    # ----- success: non-inferiority -----
    for a_name, b_name in PAIRS:
        for bud in BUDGETS:
            x, y = pivot_pairs(df, a_name, b_name, bud)
            for variant in ("all_classes", "excluding_NoFault"):
                keep = np.ones(len(x), bool) if variant == "all_classes" else (
                    x.index.get_level_values(1) != "NoFault")
                d = (y.success.values - x.success.values)[keep]
                sd = float(d.std(ddof=1))
                md = float(d.mean())
                row = dict(pair=f"{a_name} vs {b_name}", budget=bud, variant=variant, n_pairs=int(len(d)),
                           mean_d=md, sd_d=sd, discordant=float((d != 0).mean()),
                           n_up=int((d > 0).sum()), n_down=int((d < 0).sum()))
                for pw in (0.8, 0.9):
                    row[f"n_td0_p{int(pw*100)}"] = cli_power(sd, pw, 0.0)
                    row[f"n_tdobs_p{int(pw*100)}"] = cli_power(sd, pw, md)
                out["success"].append(row)

    # ----- success: generic planning table (symmetric discordance q, true diff 0) -----
    for q in (0.001, 0.0025, 0.005, 0.01, 0.02, 0.05, 0.10, 0.20):
        sd = float(np.sqrt(q))
        row = dict(q=q, sd=sd, n_p80=cli_power(sd, 0.8), n_p90=cli_power(sd, 0.9))
        # a selective arm that truly loses half the margin: mean d = -0.005, with d in {-1, 0, +1}
        # and a discordant fraction q, so var(d) = q - 0.005^2
        md = -0.005
        sd2 = float(np.sqrt(q - md * md)) if q > md * md else None
        row["n_p80_loses_half_margin"] = cli_power(sd2, 0.8, md) if sd2 else None
        row["n_p90_loses_half_margin"] = cli_power(sd2, 0.9, md) if sd2 else None
        out["success_planning"].append(row)

    # ----- cost: relative savings -----
    jobs = []
    meta = []
    for a_name, b_name in PAIRS:
        for bud in BUDGETS:
            x, y = pivot_pairs(df, a_name, b_name, bud)
            A = x.modelled_cost_ns.values.astype(float)
            B = y.modelled_cost_ns.values.astype(float)
            s_emp = 1 - B.sum() / A.sum()
            R_ = B.sum() / A.sum()
            out.setdefault("cost_pairs", []).append(dict(
                pair=f"{a_name} vs {b_name}", budget=bud, n_pairs=int(len(A)), mean_a=float(A.mean()),
                mean_b=float(B.mean()), mean_diff=float((B - A).mean()), sd_diff=float((B - A).std(ddof=1)),
                s=float(s_emp), sigma1=float(np.std(B - R_ * A, ddof=1) / A.mean())))
            scen = [("empirical", s_emp)]
            for s_t in (0.22, 0.25, 0.30):
                scen.append((f"rescaled to S={s_t}", s_t))
            scen.append(("rescaled to S=0.20 (size check)", 0.20))
            for label, s_t in scen:
                a2, b2 = (A, B) if label == "empirical" else rescale(A, B, s_t)
                row = dict(pair=f"{a_name} vs {b_name}", budget=bud, scenario=label, s_true=float(s_t),
                           delta_n_p80=delta_n(a2, b2, s_t, 0.8), delta_n_p90=delta_n(a2, b2, s_t, 0.9))
                meta.append(row)
                for n in (10, 20, 40, 80, 160, 320, 640, 1280):
                    jobs.append((a2, b2, n, 400, 400, 1000 * len(meta) + n, THRESH))
    with Pool(4) as pool:
        res = pool.map(sim_power, jobs, chunksize=1)
    k = 0
    for row in meta:
        sims = {}
        for n in (10, 20, 40, 80, 160, 320, 640, 1280):
            sims[n] = res[k]
            k += 1
        row["sim_power_by_n"] = sims
        out["cost"].append(row)
    json.dump(out, open(f"{S_DIR}/b2.json", "w"), indent=1)
    print("done")
