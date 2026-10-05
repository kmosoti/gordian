#!/usr/bin/env python3
"""Turn the hot-loop weights into the weights of the cost C, using in-situ timings.

`calibrate_ops.py` fits, on fixed windows timed in a loop, the weights that show the counters
track the work. A call in an episode costs more than that: the same call, on the same window,
takes 1.2 to 2 times as long between the harness's other work. The cost an experiment pays is the
second, and a model of C that stopped at the first would, in the non-identical-arm check, price
the expensive arm 29% above what it costs (`CALIBRATION.md`, section 9.6).

This script keeps the *shape* of the hot weights (so that what the counters say about content is
unchanged) and fits, per target, how the harness changes it: `in_situ_ns = alpha * hot_ns + beta`,
by non-negative least squares on the minimum in-situ time of each call, weighted by 1/time. Two
numbers per component and per rule, ten in all, against thousands of calls on episodes of arms
that are not the arms of the check. The weights it writes are `alpha * hot` per unit, plus `beta`
on the unit that stands for the call (`calls`; `candidates` and `damaged` for the verifier, of
which exactly one counts per call).

Input: the hot fit (`calibrate_ops.py --json`), and one or more runs of
`examples/calibrate_insitu.rs` (the call-by-call minimum is taken across runs, as
`calibrate_ops.py` does). It reports, for the fit, held-out episodes and the check arms: the fit
of the model to single calls, and what matters for an experiment, the model's total against the
measured total per arm, and the ratio between the two arms of the check.

    calibrate_insitu.py HOT.json RUN.jsonl [RUN2.jsonl ...] [--json OUT]
    calibrate_insitu.py HOT.json RUN.jsonl ... --from-source    # judge the constants in the code
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

import numpy as np
from scipy.optimize import lsq_linear

sys.path.insert(0, str(Path(__file__).resolve().parent))
import calibrate_ops as hot_fit  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
TARGETS = ["heuristic", "estimator", "memory", "verifier", "rule"]
CALL_UNITS = {
    "heuristic": ["calls"],
    "estimator": ["calls"],
    "memory": ["calls"],
    "verifier": ["candidates", "damaged"],
    "rule": ["calls"],
}
SOURCES = {
    "heuristic": "crates/gordian-components/src/heuristic.rs",
    "estimator": "crates/gordian-components/src/estimator.rs",
    "memory": "crates/gordian-components/src/memory.rs",
    "verifier": "crates/gordian-components/src/verifier.rs",
    "rule": "crates/gordian-run/src/policy/decide.rs",
}


def source_weights(target: str) -> dict[str, float]:
    text = (ROOT / SOURCES[target]).read_text()
    return {n: int(w) / 1000.0 for n, w in re.findall(r'Unit \{ name: "([a-z_]+)", weight_ps: (\d+) \}', text)}


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("hot")
    ap.add_argument("runs", nargs="+")
    ap.add_argument("--json")
    ap.add_argument("--from-source", action="store_true")
    args = ap.parse_args()
    hot = json.loads(Path(args.hot).read_text())
    rows, _ = hot_fit.load(args.runs)
    by_target: dict[str, list[dict]] = defaultdict(list)
    for r in rows:
        by_target[r["target"]].append(r)

    out = {}
    model = {}  # target -> function(counts) -> predicted ns
    for target in TARGETS:
        mine = by_target[target]
        hot_w = dict(zip(hot[target]["units"], hot[target]["weights_ns"]))
        for r in mine:
            r["hot"] = sum(hot_w.get(u, 0.0) * c for u, c in r["counts"].items())
        fit = [r for r in mine if r["set"] == "fit"]
        x = np.array([[r["hot"], 1.0] for r in fit])
        y = np.array([r["min_ns"] for r in fit])
        alpha, beta = lsq_linear(x / y[:, None], np.ones(len(y)), bounds=(0, np.inf), method="bvls").x
        units = list(hot_w)
        weights = {u: alpha * w + (beta if u in CALL_UNITS[target] else 0.0) for u, w in hot_w.items()}
        if args.from_source:
            weights = source_weights(target)
        out[target] = {
            "units": units,
            "weights_ns": [weights[u] for u in units],
            "alpha": float(alpha),
            "beta_ns": float(beta),
            "hot_weights_ns": [hot_w[u] for u in units],
        }
        model[target] = lambda counts, w=weights: sum(w.get(u, 0.0) * c for u, c in counts.items())
        print(f"{target:10s} in situ = {alpha:.3f} x hot + {beta:.1f} ns per call "
              f"({len(fit)} calls fitted)")

    print("\nFit to single calls (R^2 and median |relative error| of the model against the minimum "
          "in-situ time of each call):")
    for target in TARGETS:
        line = [f"  {target:10s}"]
        for label in ("fit", "heldout", "real"):
            sel = [r for r in by_target[target] if r["set"] == label]
            if not sel:
                continue
            pred = np.array([model[target](r["counts"]) for r in sel])
            t = np.array([r["min_ns"] for r in sel])
            rel = np.abs(pred / t - 1)
            line.append(f"{label}: R2={_r2(pred, t):.3f} med|rel|={100 * np.median(rel):.1f}%")
        print("  ".join(line))

    print("\nTotals per arm (model / measured, summed over the calls of the set; hot / measured for "
          "the unscaled hot weights):")
    agg = defaultdict(lambda: defaultdict(float))
    for target in TARGETS:
        part = "rule" if target == "rule" else "comp"
        for r in by_target[target]:
            key = (r["set"], r["arm"])
            agg[key][part + "_meas"] += r["min_ns"]
            agg[key][part + "_pred"] += model[target](r["counts"])
            agg[key][part + "_hot"] += r["hot"]
    for key in sorted(agg):
        q = agg[key]
        tot_m = q["comp_meas"] + q["rule_meas"]
        print(f"  {key[0]:8s} {key[1]:16s} components {q['comp_pred'] / q['comp_meas']:.3f}  "
              f"rule {q['rule_pred'] / q['rule_meas']:.3f}  total {(q['comp_pred'] + q['rule_pred']) / tot_m:.3f}"
              f"   (hot total {(q['comp_hot'] + q['rule_hot']) / tot_m:.3f})")
    h, a = agg.get(("real", "heuristic_only")), agg.get(("real", "all_components"))
    if h and a:
        meas = lambda q: q["comp_meas"] + q["rule_meas"]  # noqa: E731
        pred = lambda q: q["comp_pred"] + q["rule_pred"]  # noqa: E731
        hotq = lambda q: q["comp_hot"] + q["rule_hot"]  # noqa: E731
        print(f"\nall_components / heuristic_only, the check's arms on the check's episodes: "
              f"measured {meas(a) / meas(h):.3f}, model {pred(a) / pred(h):.3f}, "
              f"hot weights {hotq(a) / hotq(h):.3f}")
    if args.json:
        Path(args.json).write_text(json.dumps(out, indent=2))
    return 0


def _r2(pred: np.ndarray, t: np.ndarray) -> float:
    return hot_fit.r2(pred, t)


if __name__ == "__main__":
    sys.exit(main())
