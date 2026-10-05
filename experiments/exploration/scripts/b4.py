"""B4: oracle gap per (class, budget), effective ambiguity, headroom numbers.

Stage B exploration script (development run; nothing here tests a hypothesis).
Working directory for outputs and manifests: $GORDIAN_WORK (default /tmp/gordian-exploration).
The repository root is found from this file's location.

The effective-ambiguity section needs the generator's truth per seed. This script regenerates it
into $GORDIAN_WORK/truth.tsv with `cargo run -p gordian-eval --example truth_table` (the
evaluator crate is the only place allowed to read hidden state; the table is analysis input only,
is never given to a policy and is not committed). Set GORDIAN_TRUTH_TSV to use an existing table
instead of regenerating it. Needs cargo and the pinned toolchain; honour CARGO_BUILD_JOBS.
"""
import json
import subprocess
import sys

import numpy as np
import pandas as pd

import os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import *  # noqa  (ROOT, S_DIR, loaders)

pd.set_option("display.width", 300)
pd.set_option("display.max_columns", 60)
df = load_all()
RES = 4000
rng = np.random.default_rng(20261005)
rows = []
for bud in BUDGETS:
    for cls in CLASSES:
        d = df[(df.budget == bud) & (df["class"] == cls)]
        piv = d.pivot(index="seed", columns="arm", values="success").sort_index()
        seeds = piv.index.values
        n = len(seeds)
        pub = piv[PUBLIC].values.astype(float)  # n x arms
        orc = piv["oracle_evidence_privileged"].values.astype(float)
        imm = piv["oracle_immediate_privileged"].values.astype(float)
        means = pub.mean(0)
        k = int(np.argmax(means))
        best = PUBLIC[k]
        tied = [PUBLIC[j] for j in range(len(PUBLIC)) if means[j] == means[k]]
        gap = orc.mean() - means[k]
        # selection-aware bootstrap: re-pick the best arm inside every resample
        idx = rng.integers(0, n, size=(RES, n))
        arm_means = pub[idx].mean(1)  # RES x arms  (fancy index -> RES x n x arms)
        gaps_sel = orc[idx].mean(1) - arm_means.max(1)
        # fixed-arm paired bootstrap of (oracle - chosen arm)
        diff = orc - pub[:, k]
        gaps_fix = diff[idx].mean(1)
        # split-sample: choose on first half of seeds, evaluate on the second
        h = n // 2
        k1 = int(np.argmax(pub[:h].mean(0)))
        gap_split = orc[h:].mean() - pub[h:, k1].mean()
        rows.append(dict(
            budget=bud, cls=cls, n=n, oracle_evidence=orc.mean(), oracle_immediate=imm.mean(),
            best_arm=best, tied=len(tied), best_success=means[k], gap=gap,
            sel_lo=np.quantile(gaps_sel, 0.05), sel_hi=np.quantile(gaps_sel, 0.95),
            fix_lo=np.quantile(gaps_fix, 0.05), fix_hi=np.quantile(gaps_fix, 0.95),
            split_arm=PUBLIC[k1], gap_split=gap_split,
        ))
res = pd.DataFrame(rows)
res.to_json(f"{S_DIR}/b4.json", orient="records", indent=1)
print(res.drop(columns=["oracle_immediate"]).round(3).to_string())

print("\n== classes with gap lower limit (selection-aware 90%) >= 0.10 or gap point >= 0.10")
print(res[(res.gap >= 0.10)][["budget", "cls", "best_arm", "gap", "sel_lo", "sel_hi"]].round(3).to_string())
print("max gap among classes, by budget:")
print(res.groupby("budget").gap.max())
print("classes with gap < 0.10, by budget:", res[res.gap < 0.10].groupby("budget").size().to_dict())
print("overall (pooled over classes) gap by budget (oracle - best arm pooled):")
for bud in BUDGETS:
    d = df[df.budget == bud]
    pooled = d.groupby("arm").success.mean()
    print(bud, round(1 - pooled[PUBLIC].max(), 4), pooled[PUBLIC].idxmax())

# ---------- effective ambiguity ----------
TRUTH = os.environ.get("GORDIAN_TRUTH_TSV") or f"{S_DIR}/truth.tsv"
if not os.environ.get("GORDIAN_TRUTH_TSV"):
    seed_lo, seed_hi = int(df.seed.min()), int(df.seed.max())
    with open(TRUTH, "w") as fh:
        subprocess.run(
            ["cargo", "run", "--locked", "--quiet", "-p", "gordian-eval", "--example", "truth_table", "--",
             "--seed-start", str(seed_lo), "--seed-count", str(seed_hi - seed_lo + 1),
             "--classes", ",".join(CLASSES)],
            check=True, cwd=ROOT, stdout=fh)
t = pd.read_csv(TRUTH, sep="\t", keep_default_na=False)
t["kinds_in_set"] = t["kinds_in_set"].replace("", "-")
t["critical"] = t["critical"].astype(str).str.lower() == "true"
print("\n== truth distribution per class (seeds 1000-1499)")
amb = []
for cls in CLASSES:
    g = t[t["class"] == cls]
    vc = g.truth_kind.value_counts(normalize=True)
    sets = g.groupby("kinds_in_set").size()
    # entropy of truth within each public kind-set group
    H = 0.0
    acc_prior = 0.0
    acc_uniform = 0.0
    for key, gg in g.groupby(["kinds_in_set", "contains_nofault"]):
        w = len(gg) / len(g)
        p = gg.truth_kind.value_counts(normalize=True).values
        H += w * -(p * np.log2(p)).sum()
        acc_prior += w * p.max()
        nk = 0 if key[0] == "-" else len(key[0].split("+"))
        nh = nk + (1 if str(key[1]).lower() == "true" else 0)
        acc_uniform += w * (1 / nh if nh else 0)
    amb.append(dict(cls=cls, truth=dict((k, round(v, 3)) for k, v in vc.items()),
                    mean_nhyp=g.n_hyp.mean(), distinct_sets=len(sets),
                    kinds_in_set=";".join(f"{k or '-'}:{v}" for k, v in sets.items()),
                    nofault_in_set=float((g.contains_nofault.astype(str).str.lower() == "true").mean()),
                    H_bits=H, eff_kinds=2 ** H, acc_prior=acc_prior, acc_uniform_in_set=acc_uniform,
                    critical=g.critical.mean()))
amb = pd.DataFrame(amb)
print(amb.round(3).to_string())
amb.to_json(f"{S_DIR}/b4-amb.json", orient="records", indent=1)

# ---------- headroom numbers ----------
print("\n== heuristic family at 20M: calls vs cost (marginal cost per call)")
h = []
for arm, run in [("heuristic_only", "b1-c20000000-s1000-1499"), ("fixed_heuristic_every2", "b1-c20000000-s1000-1499"),
                 ("fixed_heuristic_every4", "b1-c20000000-s1000-1499"), ("fixed_heuristic_every8", "b1s-c20000000-s1000-1499"),
                 ("fixed_heuristic_every16", "b1s-c20000000-s1000-1499"), ("random_p000", "b1s-c20000000-s1000-1499")]:
    a = load_arm(f"{RUNS}/{run}", arm)
    h.append(dict(arm=arm, calls=a.components_run.mean(), steps=a.ops_sched.mean(), cost=a.modelled_cost_ns.mean(),
                  comp=a.modelled_component_ns.mean(), sched=a.modelled_sched_ns.mean(),
                  success=a.success.mean(), succ_faulted=a[a["class"] != "NoFault"].success.mean(),
                  crit=a.critical_miss.mean(), decision_s=a.decision_at_ns.mean() / 1e9))
h = pd.DataFrame(h)
print(h.round(3).to_string())
sub = h[h.arm != "random_p000"]
slope, icpt = np.polyfit(sub.calls, sub.cost, 1)
print("cost ~ calls: slope %.1f ns/call, intercept %.1f" % (slope, icpt))
json.dump(h.to_dict(orient="records"), open(f"{S_DIR}/b4-heur.json", "w"), indent=1)
