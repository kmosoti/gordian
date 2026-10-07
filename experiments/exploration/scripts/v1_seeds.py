"""V1: is a printed interval end that the fixed-seed script misses by one unit within the Monte Carlo
spread of the bootstrap? For each such print in the review log, the same paired cluster bootstrap is
run under 40 other seeds (9951-9990) and under the other interval convention; the printed value is
compared with the range of rounded values.

Usage: v1_seeds.py   writes experiments/exploration/v1-seed-sensitivity.csv
"""
import copy
import csv
import json
import pathlib
import sys
from decimal import ROUND_HALF_UP, Decimal

ROOT = pathlib.Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "analysis"))
from gordian_analysis import criterion as C  # noqa: E402

RUNS = pathlib.Path("/home/user/gordian/artifacts/runs")
DIRS = {"m2": {"heldout": RUNS / "m2/m2-heldout-b5-rho0.7"},
        "b3": {"b3": RUNS / "b3/b3-heldout-b5-rho0.7", "m2": RUNS / "m2/m2-heldout-b5-rho0.7"},
        "l1": {"fresh": RUNS / "l1/l1-fresh-b5-rho0.7"}, "m3": {"heldout": RUNS / "m3/m3-heldout-b5-rho0.7"}}
SEEDS = list(range(9951, 9991))
# unit, label, kind, args, field, negate, printed
ITEMS = [
    ("m2", "100 ms result 1 lower", ("paired", "medium_t100", "reanchor", "hard_anchor_correct_share", None), "lower", False, "+0.021"),
    ("m2", "500 ms result 1 lower", ("paired", "medium_t500", "reanchor", "hard_anchor_correct_share", None), "lower", False, "+0.010"),
    ("m2", "500 ms result 1 upper", ("paired", "medium_t500", "reanchor", "hard_anchor_correct_share", None), "upper", False, "+0.050"),
    ("b3", "medium - ramp+split anchor-correct upper", ("paired", "medium_t100", "ramp_split_over_re2", "hard_anchor_correct_share", None), "upper", False, "+0.034"),
    ("b3", "medium - ramp+split leak noticed lower", ("paired", "medium_t100", "ramp_split_over_re2", "leak_noticed_share", None), "lower", False, "-0.014"),
    ("b3", "medium - ramp+split leak anchor-correct lower", ("paired", "medium_t100", "ramp_split_over_re2", "leak_anchor_correct_share", None), "lower", False, "-0.492"),
    ("b3", "medium - ramp+split leak anchor-correct upper", ("paired", "medium_t100", "ramp_split_over_re2", "leak_anchor_correct_share", None), "upper", False, "-0.343"),
    ("l1", "clause 3, off minus frozen, upper", ("paired", "frozen", "learned_off", "hard_anchor_correct_share", {"last": 100}), "lower", True, "-0.015"),
    ("m3", "100 ms result 1 upper", ("paired", "medium_t100", "reanchor", "hard_anchor_correct_share", None), "upper", False, "+0.049"),
    ("m3", "2 s result 1 upper", ("paired", "medium_t2000", "reanchor", "hard_anchor_correct_share", None), "upper", False, "+0.022"),
]


def rnd(x, d):
    return Decimal(repr(float(x))).quantize(Decimal(1).scaleb(-d), rounding=ROUND_HALF_UP)


def main():
    out = []
    cache = {}
    for unit, label, (_, a, b, m, win), field, neg, printed in ITEMS:
        spec = C.load_spec(ROOT / f"experiments/criteria/{unit}.json")
        if unit not in cache:
            cache[unit] = C.RunSet(spec, DIRS[unit])
        runs = cache[unit]

        def value(seed, interval):
            s = copy.deepcopy(spec)
            s["bootstrap"]["seed"] = seed
            s["bootstrap"]["interval"] = interval
            est = C.Evaluator(s, runs).paired(a, b, m, win)
            v = est[field]
            return -v if neg else v

        # the specification's own value, then other seeds and the other convention
        own = value(spec["bootstrap"]["seed"], spec["bootstrap"]["interval"])
        other_conv = "linear" if spec["bootstrap"]["interval"] == "lower_higher" else "lower_higher"
        alt = value(spec["bootstrap"]["seed"], other_conv)
        vals = [value(s, spec["bootstrap"]["interval"]) for s in SEEDS]
        dec = len(printed.split(".")[1])
        p = Decimal(printed.replace("+", ""))
        rounded = [rnd(v, dec) for v in vals]
        out.append({
            "unit": unit, "interval_end": label, "printed": printed, "fixed_seed_value": f"{own:.6f}",
            "other_convention_value": f"{alt:.6f}", "other_convention_rounds_to": str(rnd(alt, dec)),
            "seeds_tried": len(SEEDS), "min_over_seeds": f"{min(vals):.6f}", "max_over_seeds": f"{max(vals):.6f}",
            "seeds_rounding_to_printed": sum(1 for r in rounded if r == p),
            "printed_within_range": bool(rnd(min(vals), dec) <= p <= rnd(max(vals), dec))})
    with open(ROOT / "experiments/exploration/v1-seed-sensitivity.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(out[0]))
        w.writeheader()
        w.writerows(out)
    for r in out:
        print(r)


if __name__ == "__main__":
    main()
