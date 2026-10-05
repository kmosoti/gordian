"""A7b step 2: calibration of the relative-savings interval, simulation and tables.

Work item A7b. Development run; nothing here tests a hypothesis. The method and its caveats are in
`experiments/exploration/a7b-ratio-calibration.md`; the machinery is
`analysis/gordian_analysis/calibration.py`, shared with the tests.

Populations (the paired-cost table comes from `a7b_generate.py`):

    P1@60k    heuristic_only vs fixed_heuristic_every2, compute 60,000 (binding; the most skewed)
    P1@100k   the same pair at 100,000 (binding)
    P1@20M    the same pair at 20,000,000 (non-binding; same as 250,000)
    P2@250k   all_components vs random_p050 at 250,000 (the widest paired ratio spread)
    LN-mild   lognormal costs, sigma_A = 0.5, sigma_ratio = 0.25
    LN-heavy  lognormal costs, sigma_A = 1.0, sigma_ratio = 0.5

Stages (each appends one JSON line per (population, true S, n) cell to RESULTS and skips cells it
already holds, so an interrupted run resumes):

    null     true S = 0.20, every size: the false-exceedance rate (the acceptance quantity)
    power    true S = 0.25 and 0.30, every size
    report   read RESULTS, print the markdown tables

Run it through the isolation runner, one process at a time:

    scripts/cgroup-run.sh --name a7b-null --cpus 3 --cpu-quota 100 --memory 2G -- \\
        analysis/.venv/bin/python experiments/exploration/scripts/a7b_calibrate.py null

Defaults: 2,000 experiments per cell, 10,000 bootstrap resamples (the CLI default), 90% interval.
Seeds: BASE_SEED in `calibration.py` with a per-experiment spawn key (population id, S in
thousandths, n, experiment index); both are written into every result line.
"""

import argparse
import hashlib
import json
import os
import pathlib
import platform
import subprocess
import sys
import time
from dataclasses import asdict

import numpy as np
import scipy

from gordian_analysis import calibration as cal
from gordian_analysis.intervals import DEFAULT_RESAMPLES, RATIO_METHODS

ROOT = pathlib.Path(__file__).resolve().parents[3]
DATA = ROOT / "experiments" / "exploration" / "data" / "a7b-paired-costs.csv.gz"
WORK = pathlib.Path(os.environ.get("GORDIAN_WORK", "/tmp/gordian-exploration-a7b"))
DEFAULT_RESULTS = WORK / "a7b-results.jsonl"

EMPIRICAL = {
    "P1@60k": (10, ("P1", 60_000)),
    "P1@100k": (11, ("P1", 100_000)),
    "P1@20M": (12, ("P1", 20_000_000)),
    "P2@250k": (13, ("P2", 250_000)),
}
LOGNORMAL = {"LN-mild": (20, 0.5, 0.25), "LN-heavy": (21, 1.0, 0.5)}
ALL_POPS = [*EMPIRICAL, *LOGNORMAL]
POWER_SAVINGS = (0.25, 0.30)


def population(name, savings):
    if name in EMPIRICAL:
        pop_id, key = EMPIRICAL[name]
        a, b = _data()[key]
        return cal.empirical_population(name, pop_id, a, b, savings)
    pop_id, sa, sr = LOGNORMAL[name]
    return cal.lognormal_population(name, pop_id, sa, sr, savings)


_DATA_CACHE = {}


def _data():
    if not _DATA_CACHE:
        _DATA_CACHE.update(cal.load_paired_costs(DATA))
    return _DATA_CACHE


def provenance():
    rev = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True).stdout.strip()
    return {
        "git_head": rev,
        "numpy": np.__version__,
        "scipy": scipy.__version__,
        "python": platform.python_version(),
        "base_seed": cal.BASE_SEED,
        "paired_costs_sha256": hashlib.sha256(DATA.read_bytes()).hexdigest(),
    }


def held(path):
    out = {}
    if pathlib.Path(path).exists():
        with open(path) as fh:
            for line in fh:
                r = json.loads(line)
                out[(r["population"], round(r["savings"], 3), r["n"], r["resamples"], r["first"],
                     r["n_exp"])] = r
    return out


def run_stage(path, pops, savings_list, sizes, n_exp, resamples, first=0, methods=RATIO_METHODS):
    done = held(path)
    prov = provenance()
    for savings in savings_list:
        for name in pops:
            pop = population(name, savings)
            for n in sizes:
                key = (name, round(savings, 3), n, resamples, first, n_exp)
                if key in done:
                    continue
                t = time.time()
                r = cal.simulate_cell(pop, n, n_exp, resamples=resamples, methods=methods, first=first)
                rec = {**asdict(r), "first": first, "seconds": round(time.time() - t, 1), **prov}
                with open(path, "a") as fh:
                    fh.write(json.dumps(rec) + "\n")
                rates = {m: r.rate(m) for m in methods}
                print(f"{name} S={savings} n={n} N={n_exp} B={resamples}: "
                      + " ".join(f"{m}={rates[m]:.4f}" for m in methods)
                      + f" failed={sum(r.failed.values())} ({rec['seconds']}s)", flush=True)


def table(rows, methods=RATIO_METHODS):
    """Markdown: one row per (population, n); rate +- Monte Carlo se per method."""
    out = ["| population | n | " + " | ".join(methods) + " |", "|---|---|" + "---|" * len(methods)]
    for r in rows:
        cells = []
        for m in methods:
            p = r["exceed"][m] / r["n_exp"]
            se = (p * (1 - p) / r["n_exp"]) ** 0.5
            flag = f" ({r['failed'][m]} failed)" if r["failed"][m] else ""
            cells.append(f"{p:.4f} +- {se:.4f}{flag}")
        out.append(f"| {r['population']} | {r['n']} | " + " | ".join(cells) + " |")
    return "\n".join(out)


def width_table(rows, methods=RATIO_METHODS):
    out = ["| population | n | " + " | ".join(methods) + " |", "|---|---|" + "---|" * len(methods)]
    for r in rows:
        cells = []
        for m in methods:
            unb = f", {r['unbounded'][m]} unbounded" if r["unbounded"][m] else ""
            cells.append(f"{r['mean_width'][m]:.4f}{unb}")
        out.append(f"| {r['population']} | {r['n']} | " + " | ".join(cells) + " |")
    return "\n".join(out)


def report(path):
    rows = list(held(path).values())
    if not rows:
        sys.exit(f"no results in {path}")
    prov = rows[0]
    print(f"seeds: base_seed={prov['base_seed']}, spawn key (population id, round(1000 S), n, experiment)")
    print(f"numpy {prov['numpy']}, scipy {prov['scipy']}, python {prov['python']}, "
          f"paired-costs sha256 {prov['paired_costs_sha256']}\n")
    order = {p: i for i, p in enumerate(ALL_POPS)}
    # extension runs (first > 0) are reported pooled with the cell they extend, not as cells
    main_rows = [r for r in rows if r["first"] == 0]
    ext = [r for r in rows if r["first"] > 0]
    for e in ext:
        base = [r for r in main_rows if (r["population"], round(r["savings"], 3), r["n"], r["resamples"])
                == (e["population"], round(e["savings"], 3), e["n"], e["resamples"])]
        if not base:
            sys.exit(f"extension {e['population']} n={e['n']} has no first-run cell to pool with")
        pooled = {**e, "n_exp": e["n_exp"] + base[0]["n_exp"],
                  "exceed": {m: e["exceed"][m] + base[0]["exceed"][m] for m in e["exceed"]},
                  "failed": {m: e["failed"][m] + base[0]["failed"][m] for m in e["failed"]}}
        print(f"### extension, {e['population']} n={e['n']}, true S = {e['savings']}, {e['resamples']} "
              f"resamples: experiments {base[0]['n_exp']} + {e['n_exp']} = {pooled['n_exp']} "
              f"(first {base[0]['n_exp']}: separately, in the table of its cell)\n")
        print(table([pooled]))
        print()
    rows = main_rows
    for savings in sorted({round(r["savings"], 3) for r in rows}):
        for B in sorted({r["resamples"] for r in rows}):
            sel = [r for r in rows if round(r["savings"], 3) == savings and r["resamples"] == B]
            if not sel:
                continue
            sel.sort(key=lambda r: (order[r["population"]], r["n"]))
            kind = "false-exceedance rate" if savings == cal.THRESHOLD else "power"
            print(f"### true S = {savings}: {kind}, {B} resamples, "
                  f"{sorted({r['n_exp'] for r in sel})} experiments per cell\n")
            print(table(sel))
            print()
            if savings == cal.THRESHOLD:
                print(f"mean width of the interval, true S = {savings}, {B} resamples "
                      "(experiments with finite limits; unbounded = lower limit -inf)\n")
                print(width_table(sel))
                print()


def export(path, out):
    """One CSV row per (cell, method): the raw counts behind every rate in the document."""
    import csv

    rows = sorted(held(path).values(), key=lambda r: (r["savings"], r["resamples"], ALL_POPS.index(r["population"]),
                                                      r["n"], r["first"]))
    with open(out, "w", newline="") as fh:
        w = csv.writer(fh, lineterminator="\n")
        w.writerow(["population", "true_savings", "n", "first_experiment", "experiments", "resamples", "method",
                    "exceed", "rate", "mc_se", "failed", "unbounded", "mean_low", "mean_width"])
        for r in rows:
            for m in RATIO_METHODS:
                p = r["exceed"][m] / r["n_exp"]
                w.writerow([r["population"], r["savings"], r["n"], r["first"], r["n_exp"], r["resamples"], m,
                            r["exceed"][m], f"{p:.5f}", f"{(p * (1 - p) / r['n_exp']) ** 0.5:.5f}", r["failed"][m],
                            r["unbounded"][m], f"{r['mean_low'][m]:.6f}", f"{r['mean_width'][m]:.6f}"])
    print(f"wrote {out}: {len(rows)} cells")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("stage", choices=["null", "power", "report", "export"])
    ap.add_argument("--results", default=str(DEFAULT_RESULTS))
    ap.add_argument("--n-exp", type=int, default=2000)
    ap.add_argument("--resamples", type=int, default=DEFAULT_RESAMPLES)
    ap.add_argument("--pops", nargs="+", default=ALL_POPS, choices=ALL_POPS)
    ap.add_argument("--sizes", nargs="+", type=int, default=list(cal.SIZES))
    ap.add_argument("--first", type=int, default=0, help="index of the first experiment (extension runs)")
    ap.add_argument("--csv", default=str(ROOT / "experiments" / "exploration" / "data" / "a7b-calibration-cells.csv"),
                    help="output of the export stage")
    a = ap.parse_args()
    WORK.mkdir(parents=True, exist_ok=True)
    if a.stage == "report":
        report(a.results)
    elif a.stage == "export":
        export(a.results, a.csv)
    elif a.stage == "null":
        run_stage(a.results, a.pops, [cal.THRESHOLD], a.sizes, a.n_exp, a.resamples, a.first)
    else:
        run_stage(a.results, a.pops, POWER_SAVINGS, a.sizes, a.n_exp, a.resamples, a.first)


if __name__ == "__main__":
    main()
