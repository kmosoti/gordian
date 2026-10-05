"""R8: the reference reader on the questions, beside the model.

Usage: r8_reference.py QUESTIONS.jsonl OUT.csv [--select N_HARD N_PLAIN] [--bootstrap B]

Applies `r8_rules.read` (a deterministic reader of the prompt's own rules, with no model) to every
level and the control of the questions in QUESTIONS.jsonl, and writes per tier and level the share
it gets right, with the same fit and cluster bootstrap R8 applies to the model (`r8_stats.analyse`).
With `--select N_HARD N_PLAIN` it first applies the design's selection rule
(`r8_common.select_questions`), so the rows are for the questions a run would use; without it, every
hard (non-slow-leak) incident of the file is used.

What it is for: it shows (1) whether the contexts are answerable in principle by reading the stated
rules, and (2) how much of a decline in accuracy with the number of distractors a program reading
those rules shows, because distractors include free-form messages and alarms that look like evidence.
That part of a model's delta is not the model's. It is not an arm, not a policy and not a baseline
for any experiment; it reads rendered observations and the public graph, never hidden state.

Exploration (nothing here tests a hypothesis).
"""

import argparse
import csv
import json

import numpy as np

import r8_common as C
import r8_rules as R
import r8_stats as S

COLUMNS = (0, 50, 100, 200, 400, "control")


def matrix(qs):
    mat = []
    for q in qs:
        ctxs = C.contexts(q)
        truth = C.truth_pair(q["truth"])
        mat.append(
            [
                1.0 if R.read(q["services"], q["focus"], ctxs[col]) == truth else 0.0
                for col in COLUMNS
            ]
        )
    return np.array(mat)


def rows_for(tier, qs, b):
    if not qs:
        return []
    mat = matrix(qs)
    res = S.analyse(mat, b=b)
    out = []
    for j, col in enumerate(COLUMNS):
        lo, hi = (res["A_interval"][j] if j < 5 else res["p0_interval"])
        out.append(
            {
                "set": tier,
                "n_incidents": len(qs),
                "level": col,
                "correct": int(mat[:, j].sum()),
                "A": float(mat[:, j].mean()),
                "boot_lo": lo,
                "boot_hi": hi,
            }
        )
    out.append(
        {
            "set": tier,
            "n_incidents": len(qs),
            "level": "delta_hat [90%]",
            "correct": "",
            "A": res["delta"],
            "boot_lo": res["delta_interval"][0],
            "boot_hi": res["delta_interval"][1],
        }
    )
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("questions")
    ap.add_argument("out")
    ap.add_argument("--select", nargs=2, type=int, default=None)
    ap.add_argument("--bootstrap", type=int, default=S.B)
    args = ap.parse_args()
    records = C.load_jsonl(args.questions)
    if args.select:
        hard, plain, _ = C.select_questions(records, *args.select)
    else:
        hard = [r for r in records if r["tier"] == "Hard" and r["pool_size"] >= C.MIN_POOL]
        plain = []
    rows = rows_for("hard", hard, args.bootstrap) + rows_for("plain", plain, args.bootstrap)
    with open(args.out, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0].keys()))
        w.writeheader()
        for r in rows:
            w.writerow({k: (f"{v:.6g}" if isinstance(v, float) else v) for k, v in r.items()})
    print(json.dumps(rows[-1]) if rows else "no rows")


if __name__ == "__main__":
    main()
