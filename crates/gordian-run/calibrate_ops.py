#!/usr/bin/env python3
"""Fit the weights of the counted operations and report whether the counters are valid.

Input: one or more JSON-lines files written by `examples/calibrate_ops.rs` (see its header for
how to run it on an idle core through `scripts/cgroup-run.sh`). Several files are several runs
of the same program; they must hold the same data in the same order, and the time used for a
datum is the *minimum* over the runs, because interference only ever adds time.

For each target (the four components and the shared rule) the weights are a non-negative least
squares fit of the minimum time per call on the operation counts, with no intercept: a per-call
overhead is the explicit `calls` unit. Rows are weighted by 1/time (relative error), so that the
fixed costs of small windows count as much as the slopes of large ones. Only the `fit` set is
fitted. The report then gives, for the `fit` set (in sample), the `heldout` set (new episodes at
window sizes the fit never saw) and the `real` set (states recorded from episodes played by the
harness):

- R^2 of the weighted counts against the minimum time per call, the acceptance figure (at least
  0.9 for every target and every set);
- the median and 90th percentile of |predicted / measured - 1|;
- the worst class (largest |mean relative residual|) and the worst per-class R^2;
- the mean relative residual by window size and by number of probe results, which is the
  residual's shape.

It prints the weights as picoseconds per unit, ready for the constants in code, and exits 1 if
any target fails the R^2 bar on any set. Usage:

    calibrate_ops.py RUN.jsonl [RUN2.jsonl ...] [--json OUT]
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import defaultdict

import numpy as np
from scipy.optimize import lsq_linear

R2_BAR = 0.9
TARGETS = ["heuristic", "estimator", "memory", "verifier", "rule"]


def load(paths):
    runs = []
    for path in paths:
        with open(path) as fh:
            runs.append([json.loads(line) for line in fh if line.strip()])
    first = runs[0]
    for other in runs[1:]:
        if len(other) != len(first):
            sys.exit("the runs hold different numbers of lines")
        for a, b in zip(first, other):
            if (a["target"], a["counts"], a["set"], a["class"]) != (
                b["target"], b["counts"], b["set"], b["class"]
            ):
                sys.exit("the runs do not hold the same data in the same order")
    rows = []
    for i, row in enumerate(first):
        row = dict(row)
        row["min_ns"] = min(run[i]["min_ns"] for run in runs)
        rows.append(row)
    return rows, runs


def design(rows, units):
    x = np.array([[r["counts"].get(u, 0) for u in units] for r in rows], dtype=float)
    t = np.array([r["min_ns"] for r in rows], dtype=float)
    return x, t


def fit(rows, units):
    x, t = design(rows, units)
    # Bounded-variable least squares with a lower bound of zero: non-negative least squares.
    # (scipy's `nnls` aborted with a double free on a rank-deficient design in this environment.)
    return lsq_linear(x / t[:, None], np.ones(len(t)), bounds=(0, np.inf), method="bvls").x


def r2(pred, t):
    ss_res = float(np.sum((t - pred) ** 2))
    ss_tot = float(np.sum((t - t.mean()) ** 2))
    return 1.0 - ss_res / ss_tot if ss_tot > 0 else float("nan")


def summarize(rows, units, w, label):
    """R^2, relative-error percentiles, per-class and per-size residuals for one set."""
    if not rows:
        return None
    x, t = design(rows, units)
    pred = x @ w
    rel = pred / t - 1.0
    out = {
        "set": label,
        "n": len(rows),
        "r2": r2(pred, t),
        "median_abs_rel": float(np.median(np.abs(rel))),
        "p90_abs_rel": float(np.percentile(np.abs(rel), 90)),
        "mean_rel": float(rel.mean()),
    }
    by_class = defaultdict(list)
    for r, e, p, tt in zip(rows, rel, pred, t):
        by_class[r["class"]].append((e, p, tt))
    classes = {}
    for cls, items in sorted(by_class.items()):
        es = np.array([i[0] for i in items])
        ps = np.array([i[1] for i in items])
        ts = np.array([i[2] for i in items])
        classes[cls] = {"n": len(items), "mean_rel": float(es.mean()), "r2": r2(ps, ts)}
    out["classes"] = classes
    worst = max(classes.items(), key=lambda kv: abs(kv[1]["mean_rel"]))
    out["worst_class"] = {"class": worst[0], **worst[1]}
    out["worst_class_r2"] = min(
        (v["r2"] for v in classes.values() if v["n"] >= 8 and np.isfinite(v["r2"])),
        default=float("nan"),
    )
    sizes = defaultdict(list)
    for r, e in zip(rows, rel):
        edges = [0, 24, 48, 96, 192, 384, 768, 1 << 30]
        for lo, hi in zip(edges, edges[1:]):
            if lo < r["n"] <= hi:
                sizes[f"n in ({lo}, {hi if hi < 1 << 30 else 'inf'}]"] += [e]
    out["by_size"] = {k: (len(v), float(np.mean(v))) for k, v in sizes.items()}
    probes = defaultdict(list)
    for r, e in zip(rows, rel):
        if r["source"] == "probed":
            probes[r["probes"]].append(e)
    out["by_probes"] = {k: (len(v), float(np.mean(v))) for k, v in sorted(probes.items())}
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("runs", nargs="+")
    ap.add_argument("--json", help="write the fits and summaries here")
    args = ap.parse_args()
    rows, runs = load(args.runs)
    ok = True
    report = {}
    for target in TARGETS:
        mine = [r for r in rows if r["target"] == target]
        if not mine:
            continue
        units = list(mine[0]["counts"].keys())
        # The rule has no fixed windows; its fit set is the first episodes recorded.
        fit_label = "real_fit" if target == "rule" else "fit"
        fit_rows = [r for r in mine if r["set"] == fit_label]
        w = fit(fit_rows, units)
        print(f"\n=== {target}  ({len(units)} units, {len(fit_rows)} fit rows)")
        print("unit".ljust(18), "weight ns/op".rjust(14), "weight ps/op".rjust(14), "share of fit time".rjust(18))
        x, t = design(fit_rows, units)
        contrib = (x * w).sum(axis=0) / t.sum()
        for u, wi, c in zip(units, w, contrib):
            print(u.ljust(18), f"{wi:14.4f}", f"{round(wi * 1000):14d}", f"{100 * c:17.1f}%")
        per_run = []
        for run in runs if len(runs) > 1 else []:
            sub = [dict(r, min_ns=run[i]["min_ns"]) for i, r in enumerate(rows) if r["target"] == target]
            per_run.append(fit([r for r in sub if r["set"] == fit_label], units))
        if per_run:
            print("weights ns/op by run:")
            for u, ws in zip(units, zip(*per_run)):
                print("  ", u.ljust(16), " ".join(f"{v:10.4f}" for v in ws))
        sets = {}
        for label in (fit_label, "heldout", "real"):
            s = summarize([r for r in mine if r["set"] == label], units, w, label)
            if s is None:
                continue
            sets[label] = s
            flag = "ok" if s["r2"] >= R2_BAR else "FAIL"
            ok &= s["r2"] >= R2_BAR
            wc = s["worst_class"]
            print(
                f"  {label:8s} n={s['n']:5d}  R2={s['r2']:.4f} [{flag}]  "
                f"median|rel|={100 * s['median_abs_rel']:.1f}%  p90|rel|={100 * s['p90_abs_rel']:.1f}%  "
                f"mean rel={100 * s['mean_rel']:+.1f}%  worst class {wc['class']} "
                f"(mean rel {100 * wc['mean_rel']:+.1f}%, R2 {wc['r2']:.3f})  "
                f"worst per-class R2 {s['worst_class_r2']:.3f}"
            )
        for label in (fit_label, "heldout", "real"):
            s = sets.get(label)
            if s is None:
                continue
            print(f"  residual by size, {label}: " + "; ".join(f"{k}: {v[1] * 100:+.1f}% (n={v[0]})" for k, v in s["by_size"].items()))
            if s["by_probes"]:
                print(f"  residual by probe results, {label}: " + "; ".join(f"{k}: {v[1] * 100:+.1f}% (n={v[0]})" for k, v in s["by_probes"].items()))
        for label in ("heldout", "real"):
            s = sets.get(label)
            if s is None:
                continue
            print(f"  residual by class, {label}: " + "; ".join(f"{k}: {v['mean_rel'] * 100:+.1f}%" for k, v in s["classes"].items()))
        report[target] = {"units": units, "weights_ns": list(map(float, w)), "weights_ps": [round(v * 1000) for v in w], "sets": sets}
    print("\nR^2 bar", R2_BAR, "->", "all pass" if ok else "SOME FAIL")
    if args.json:
        with open(args.json, "w") as fh:
            json.dump(report, fh, indent=2)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
