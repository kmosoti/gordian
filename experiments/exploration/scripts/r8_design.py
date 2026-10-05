"""R8: choose the questions of a phase and list its calls, before any call is made.

Usage: r8_design.py --phase pilot|main --questions Q.jsonl --out DESIGN.json
                    [--n-hard N] [--n-plain M] [--levels 0,50,100,200,400]
                    [--control] [--tiers hard,plain] [--first-hard K]

Q.jsonl is the dumper's output for the phase's seeds (pilot 29000 upward, main 30000 upward). The
rule that picks questions is `r8_common.select_questions`. The design file records the exclusions
(incidents whose pool is smaller than the largest level), the questions with what the analysis
needs about each, and the calls in the order the runner makes them: question by question, levels
in the order of `--levels` and then the control (with `--control`), so that an interrupted run holds
whole questions.

`--tiers` says which of the selected questions get calls in this design, so that a main run can be
made in stages from one question list: stage 1 is the hard questions at level 0 and the control (the
identifiability precondition depends on nothing else), stage 2 the other levels of the hard
questions and every level and the control of the plain ones. The stages are run in this order, and
which model runs stage 2 is decided by stage 1's precondition (the plan's fallback rule), never by
any other level's result.
"""

import argparse
import hashlib
import json

import r8_common as C


def build(records, phase, n_hard, n_plain, levels, control, tiers, first_hard=None, extra=None):
    hard, plain, ex = C.select_questions(records, n_hard, n_plain)
    if first_hard is not None:
        hard = hard[:first_hard]
    qs = hard + plain
    calls = []
    for q in qs:
        if q["tier"].lower() not in tiers:
            continue
        for m in levels:
            calls.append({"qkey": C.qkey(q), "level": m})
        if control:
            calls.append({"qkey": C.qkey(q), "level": "control"})
    if extra:
        level, k = extra
        for q in hard[:k]:
            calls.append({"qkey": C.qkey(q), "level": level})
    for i, c in enumerate(calls):
        c["call_id"] = i
    return {
        "phase": phase,
        "levels": list(levels),
        "control": control,
        "tiers": sorted(tiers),
        "n_hard": len(hard),
        "n_plain": len(plain),
        "exclusions": ex,
        "min_pool": C.MIN_POOL,
        "questions": [
            {
                "qkey": C.qkey(q),
                "seed": q["seed"],
                "incident": q["incident"],
                "tier": q["tier"],
                "family": q["family"],
                "mode": q["mode"],
                "duo": q["duo"],
                "recurrence_of": q["recurrence_of"],
                "pool_size": q["pool_size"],
                "n_decisive": len(q["decisive"]),
            }
            for q in qs
        ],
        "calls": calls,
        "questions_sha256": hashlib.sha256(
            "\n".join(json.dumps(r, sort_keys=True) for r in records).encode()
        ).hexdigest(),
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--phase", required=True, choices=["pilot", "main"])
    ap.add_argument("--questions", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--n-hard", type=int, default=0)
    ap.add_argument("--n-plain", type=int, default=0)
    ap.add_argument("--first-hard", type=int, default=None)
    ap.add_argument("--levels", default=",".join(str(m) for m in C.LEVELS))
    ap.add_argument("--control", action="store_true")
    ap.add_argument("--tiers", default="hard,plain")
    ap.add_argument(
        "--extra",
        default=None,
        help="LEVEL:K, extra calls at LEVEL for the first K hard questions, after all others",
    )
    args = ap.parse_args()
    records = C.load_jsonl(args.questions)
    levels = [int(x) for x in args.levels.split(",") if x != ""]
    tiers = set(args.tiers.split(","))
    extra = None
    if args.extra:
        lv, k = args.extra.split(":")
        extra = (int(lv), int(k))
    design = build(
        records,
        args.phase,
        args.n_hard,
        args.n_plain,
        levels,
        args.control,
        tiers,
        args.first_hard,
        extra,
    )
    with open(args.out, "w") as f:
        json.dump(design, f, indent=1)
        f.write("\n")
    print(
        f"{design['phase']}: {design['n_hard']} hard, {design['n_plain']} plain questions selected, "
        f"{len(design['calls'])} calls for {design['tiers']}, exclusions {design['exclusions']}"
    )


if __name__ == "__main__":
    main()
