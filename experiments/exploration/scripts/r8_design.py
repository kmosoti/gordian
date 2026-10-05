"""R8: choose the questions of a phase and list its calls, before any call is made.

Usage: r8_design.py --phase pilot|main --questions Q.jsonl --out DESIGN.json
                    --n-hard N --n-plain M
                    --spec TIER:LEVELS[:control] [--spec ...] [--first-hard K] [--extra LEVEL:K]
                    [--selected-out SELECTED.jsonl]

Q.jsonl is the dumper's output for the phase's seeds (pilot 29000 upward, main 30000 upward). The
rule that picks questions is `r8_common.select_questions`. The design file records the exclusions
(incidents whose pool is smaller than the largest level), the questions with what the analysis
needs about each, and the calls in the order the runner makes them: question by question (hard
ones first, then plain), each question's levels in the order given and then its control, so that an
interrupted run holds whole questions.

`--spec hard:0:control` makes the calls of the hard questions at level 0 and the control;
`--spec plain:0,50,100,200,400:control` those of the plain questions at every level and the control.
A main run is made in stages from one question list: stage 1 is `hard:0:control` (the identifiability
precondition depends on nothing else), stage 2 is `hard:50,100,200,400` and
`plain:0,50,100,200,400:control`. The stages are run in this order, and which model runs stage 2
is decided by stage 1's precondition (the plan's fallback rule), never by any other level's result.
`--first-hard K` keeps only the first K selected hard questions (the questions a second model runs).
"""

import argparse
import hashlib
import json

import r8_common as C


def parse_spec(items):
    spec = {}
    for item in items:
        parts = item.split(":")
        tier = parts[0]
        levels = [int(x) for x in parts[1].split(",") if x != ""]
        spec[tier] = (levels, len(parts) > 2 and parts[2] == "control")
    return spec


def build(records, phase, n_hard, n_plain, spec, first_hard=None, extra=None):
    hard, plain, ex = C.select_questions(records, n_hard, n_plain)
    if first_hard is not None:
        hard = hard[:first_hard]
    qs = hard + plain
    calls = []
    for q in qs:
        levels, control = spec.get(q["tier"].lower(), ([], False))
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
        "spec": {t: {"levels": v[0], "control": v[1]} for t, v in spec.items()},
        "extra": list(extra) if extra else None,
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
    ap.add_argument("--spec", action="append", required=True)
    ap.add_argument(
        "--selected-out",
        default=None,
        help="write the selected questions' full records here (the file the runner reads)",
    )
    ap.add_argument(
        "--extra",
        default=None,
        help="LEVEL:K, extra calls at LEVEL for the first K hard questions, after all others",
    )
    args = ap.parse_args()
    records = C.load_jsonl(args.questions)
    extra = None
    if args.extra:
        lv, k = args.extra.split(":")
        extra = (int(lv), int(k))
    design = build(
        records,
        args.phase,
        args.n_hard,
        args.n_plain,
        parse_spec(args.spec),
        args.first_hard,
        extra,
    )
    with open(args.out, "w") as f:
        json.dump(design, f, indent=1)
        f.write("\n")
    if args.selected_out:
        chosen = {q["qkey"] for q in design["questions"]}
        with open(args.selected_out, "w") as f:
            for r in records:
                if C.qkey(r) in chosen:
                    f.write(json.dumps(r) + "\n")
    print(
        f"{design['phase']}: {design['n_hard']} hard, {design['n_plain']} plain questions selected, "
        f"{len(design['calls'])} calls, spec {design['spec']}, exclusions {design['exclusions']}"
    )


if __name__ == "__main__":
    main()
