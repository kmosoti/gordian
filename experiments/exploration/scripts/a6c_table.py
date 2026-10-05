"""A6c before/after table: pooled success, critical-miss rate and mean modelled cost per arm and budget.

Stage B exploration script (development run; nothing here tests a hypothesis). Standard library only.

    a6c_table.py BEFORE_TAG AFTER_TAG [OLDW_TAG] [--runs DIR] [--oldw-label TEXT]

(`--oldw-label` names the weights the OLDW_TAG column carries; work item A6d used "A6c weights".)

OLDW_TAG, if given, is a run of the same tree as AFTER_TAG with the rule's counted weights put back
to their pre-A6c values (a scratch build; episodes and counts are identical, only the weighting of
`modelled_sched_ns` differs). It adds a column that separates what the cost fell by through doing
less work from what it moved by re-weighting.

Reads artifacts/runs/<TAG>-b1-c<budget>-s1000-1499/<arm>/results.csv for both tags (the B1 grid, ten
arms, four budgets, seeds 1000 to 1499, eleven classes) and prints a markdown table. "Pooled" is the
mean over every episode of the arm at that budget (11 classes x 500 seeds = 5,500 episodes), the same
pooling `b3-finding4.md` section 7 uses. Modelled cost is `modelled_component_ns + modelled_sched_ns`
(the charter's C). Before and after use their own weights, so the cost columns compare the two
revisions as shipped, not one set of weights applied twice.
"""
import csv
import os
import pathlib
import sys

ROOT = str(pathlib.Path(__file__).resolve().parents[3])
BUDGETS = [20_000_000, 250_000, 100_000, 60_000]
ARMS = [
    "heuristic_only", "all_components", "random_p025", "random_p050",
    "fixed_verifier_only", "fixed_estimator_only", "fixed_heuristic_every2",
    "fixed_heuristic_every4", "oracle_evidence_privileged", "oracle_immediate_privileged",
]


def pooled(run_dir, arm):
    n = succ = miss = 0
    cost = 0
    stops = {}
    with open(f"{run_dir}/{arm}/results.csv") as f:
        for row in csv.DictReader(f):
            n += 1
            succ += row["success"] == "true"
            miss += row["critical_miss"] == "true"
            cost += int(row["modelled_component_ns"]) + int(row["modelled_sched_ns"])
            stops[row["stop_reason"]] = stops.get(row["stop_reason"], 0) + 1
    return {"n": n, "success": succ / n, "miss": miss / n, "cost": cost / n, "stops": stops}


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    runs = f"{ROOT}/artifacts/runs"
    if "--runs" in sys.argv:
        runs = sys.argv[sys.argv.index("--runs") + 1]
        args = [a for a in args if a != runs]
    label = "A8b weights"
    if "--oldw-label" in sys.argv:
        label = sys.argv[sys.argv.index("--oldw-label") + 1]
        args = [a for a in args if a != label]
    before_tag, after_tag = args[:2]
    oldw_tag = args[2] if len(args) > 2 else None
    head = ("| arm | compute limit (ns) | success before | success after | critical miss before | critical miss after "
            "| mean modelled cost before (ns) | mean modelled cost after (ns) | cost after / before |")
    rule = "|---|---:|---:|---:|---:|---:|---:|---:|---:|"
    if oldw_tag:
        head += f" cost after, {label} (ns) | same / before |"
        rule += "---:|---:|"
    print(head)
    print(rule)
    for arm in ARMS:
        for b in BUDGETS:
            rid = f"b1-c{b}-s1000-1499"
            x = pooled(f"{runs}/{before_tag}-{rid}", arm)
            y = pooled(f"{runs}/{after_tag}-{rid}", arm)
            assert x["n"] == y["n"] == 5500, (arm, b, x["n"], y["n"])
            ratio = y["cost"] / x["cost"] if x["cost"] else float("nan")
            line = (f"| `{arm}` | {b:,} | {100 * x['success']:.1f} | {100 * y['success']:.1f} "
                    f"| {100 * x['miss']:.1f} | {100 * y['miss']:.1f} "
                    f"| {x['cost']:,.0f} | {y['cost']:,.0f} | {ratio:.3f} |")
            if oldw_tag:
                z = pooled(f"{runs}/{oldw_tag}-{rid}", arm)
                assert (z["success"], z["miss"]) == (y["success"], y["miss"])
                zr = z["cost"] / x["cost"] if x["cost"] else float("nan")
                line += f" {z['cost']:,.0f} | {zr:.3f} |"
            print(line)


if __name__ == "__main__":
    main()
