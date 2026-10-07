"""V1: which tested clauses of the four specifications change outcome with the bootstrap seed?

Every clause is evaluated under the specification's seed and under 40 others (9951-9990); the table
says, per test, how many seeds give the other outcome. A test that depends on a point estimate or on
a count cannot flip; a test on an interval end can, when the end sits near its bound.

Usage: v1_seedflips.py   writes experiments/exploration/v1-seed-flips.csv
"""
import copy
import csv
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "analysis"))
from gordian_analysis import criterion as C  # noqa: E402

RUNS = pathlib.Path("/home/user/gordian/artifacts/runs")
DIRS = {"m2": {"heldout": RUNS / "m2/m2-heldout-b5-rho0.7"},
        "b3": {"b3": RUNS / "b3/b3-heldout-b5-rho0.7", "m2": RUNS / "m2/m2-heldout-b5-rho0.7"},
        "l1": {"fresh": RUNS / "l1/l1-fresh-b5-rho0.7"}, "m3": {"heldout": RUNS / "m3/m3-heldout-b5-rho0.7"}}
SEEDS = list(range(9951, 9991))


def main():
    rows = []
    for unit, dirs in DIRS.items():
        spec = C.load_spec(ROOT / f"experiments/criteria/{unit}.json")
        runs = C.RunSet(spec, dirs)
        base = {}
        per_seed = []
        for seed in [spec["bootstrap"]["seed"]] + SEEDS:
            s = copy.deepcopy(spec)
            s["bootstrap"]["seed"] = seed
            ev = C.Evaluator(s, runs)
            res = {}
            for c in s["clauses"]:
                if c["kind"] == "identity" or not c.get("tests"):
                    continue
                out = ev.clause(c)
                res[c["id"]] = out
            if seed == spec["bootstrap"]["seed"]:
                base = res
            else:
                per_seed.append(res)
        for cid, b in base.items():
            for i, t in enumerate(b["tests"]):
                flips = sum(1 for r in per_seed if r[cid]["tests"][i]["pass"] != t["pass"])
                vals = [r[cid][t["on"]] for r in per_seed if r[cid][t["on"]] is not None]
                rows.append({"unit": unit, "clause": cid, "test": f"{t['on']} {t['op']} {t['value']}",
                             "fixed_seed_value": f"{t['observed']:.5f}", "fixed_seed_outcome": t["pass"],
                             "min_over_seeds": f"{min(vals):.5f}", "max_over_seeds": f"{max(vals):.5f}",
                             "seeds_with_other_outcome": f"{flips}/{len(per_seed)}"})
    with open(ROOT / "experiments/exploration/v1-seed-flips.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(rows[0]))
        w.writeheader()
        w.writerows(rows)
    for r in rows:
        if not r["seeds_with_other_outcome"].startswith("0/"):
            print(r)


if __name__ == "__main__":
    main()
