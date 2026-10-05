"""R8 stage 1: the identifiability precondition from the hard questions at m = 0 and the control.

Usage: r8_stage1.py CALLS.jsonl [--out OUT.json]

The precondition (`r8_stats.precondition`) depends on A(0) and the control only: A(0) at least 0.35
and its 90% lower bound above the control's estimate plus 0.15, on hard incidents, with the same
cluster bootstrap as the full analysis (10,000 resamples of incidents, seed 9800; the control and
level 0 of an incident are resampled together). It prints the decision and writes the numbers.
This is the whole of stage 1; it is applied once, to the whole question set of the stage.
"""

import argparse
import json

import numpy as np

import r8_analysis as A
import r8_stats as S


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("calls", nargs="+")
    ap.add_argument("--out")
    args = ap.parse_args()
    calls = A.load_calls(args.calls)
    bad = A.reparse(calls)
    if bad:
        raise SystemExit(f"stored parses disagree: {bad[:3]}")
    by = {}
    for c in calls:
        if c["tier"] == "Hard":
            by.setdefault(c["qkey"], {})[c["level"]] = int(c["correct"])
    keys = sorted(k for k, v in by.items() if 0 in v and "control" in v)
    mat = np.array([[by[k][0], by[k]["control"]] for k in keys], dtype=float)
    n = len(mat)
    idx = S.bootstrap_indices(n, S.B, S.SEED)
    boot = mat[idx].mean(axis=1)
    a0, p0 = float(mat[:, 0].mean()), float(mat[:, 1].mean())
    lo, hi = S.percentile_interval(boot[:, 0])
    plo, phi = S.percentile_interval(boot[:, 1])
    res = {
        "n_incidents": n,
        "A0": a0,
        "A0_interval": [lo, hi],
        "p0": p0,
        "p0_interval": [plo, phi],
        "precondition": S.precondition(a0, lo, p0),
        "calls": len(calls),
        "parse_failures": sum(1 for c in calls if c["parse_status"] != "ok"),
        "transport_errors": sum(1 for c in calls if c["error"] is not None),
    }
    print(json.dumps(res, indent=1))
    if args.out:
        with open(args.out, "w") as f:
            json.dump(res, f, indent=1)
            f.write("\n")


if __name__ == "__main__":
    main()
