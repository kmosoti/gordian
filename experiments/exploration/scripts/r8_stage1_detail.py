"""R8 stage 1, in detail: what each model answered at m = 0 and at the control, by family.

Usage: r8_stage1_detail.py OUT.csv CALLS.jsonl [CALLS.jsonl ...]   (the model label is read from the
call file's run directory name, which is also written to the output)

One row per (run, level in {0, control}, family): calls, correct, and the answers by kind. Nothing
here is part of the criterion; it shows why the control's share correct is high (a model that names
a hard kind at the focus's service is right by luck about as often as the hard kinds' shares) and
which families the model gets from the evidence.
"""

import collections
import csv
import os
import sys

import r8_analysis as A


def main():
    out = sys.argv[1]
    rows = []
    for path in sys.argv[2:]:
        run = os.path.basename(os.path.dirname(os.path.abspath(path)))
        calls = [c for c in A.load_calls(path) if c["tier"] == "Hard"]
        for level in (0, "control"):
            for fam in sorted({c["family"] for c in calls}) + ["all"]:
                cs = [
                    c
                    for c in calls
                    if c["level"] == level and (fam == "all" or c["family"] == fam)
                ]
                if not cs:
                    continue
                ans = collections.Counter(
                    (c["parsed_kind"] or "unparsed") for c in cs
                )
                rows.append(
                    {
                        "run": run,
                        "level": level,
                        "family": fam,
                        "calls": len(cs),
                        "correct": sum(1 for c in cs if c["correct"]),
                        "answers": "; ".join(f"{k} {v}" for k, v in sorted(ans.items())),
                    }
                )
    with open(out, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0].keys()))
        w.writeheader()
        w.writerows(rows)
    for r in rows:
        print(r)


if __name__ == "__main__":
    main()
