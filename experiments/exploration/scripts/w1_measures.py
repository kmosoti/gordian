#!/usr/bin/env python3
"""W1: sample efficiency and the energy proxy for every arm of the R10 and R9 held-out runs.

Reads `artifacts/runs/r10/r10-heldout-b5-rho0.7` and
`artifacts/runs/r9/r9-heldout-b5-rho0.7-d0.0135` in place (nothing is copied, nothing moved),
applies `gordian_analysis.measures` to each arm's `results.csv`, and writes into this directory's
parent (`experiments/exploration/`):

- `w1-sample-efficiency.csv`: per arm, hard incidents seen, correct, the pooled ratio with a 90%
  stream-cluster bootstrap interval, the two half ratios, and the cumulative ratio after
  25, 50, 100, 150 and 200 streams.
- `w1-sample-efficiency-curves.csv`: the full cumulative curve of every arm.
- `w1-energy-proxy.csv`: per arm, modelled cost split into cheap operations and reasoner price,
  modelled ns per correct decision (plain and hard together, and hard alone), and the same in
  joules under the **placeholder** conversion, with the conversion written on every row.

Usage: w1_measures.py [--runs-root DIR] [--out-dir DIR]. The default runs root is the main
checkout's `artifacts/runs` (git-ignored, read in place).
"""

from __future__ import annotations

import argparse
import csv
from pathlib import Path

import numpy as np
import pandas as pd

from gordian_analysis.measures import PLACEHOLDER_LABEL, energy_proxy, sample_efficiency

RUNS = {
    "r10": "r10/r10-heldout-b5-rho0.7",
    "r9": "r9/r9-heldout-b5-rho0.7-d0.0135",
}
CHECKPOINTS = (25, 50, 100, 150, 200)
BOOTSTRAP_RESAMPLES = 2000
BOOTSTRAP_SEED = 1


def bootstrap_ratio(num: np.ndarray, den: np.ndarray, rng: np.random.Generator) -> tuple[float, float]:
    """90% percentile interval of `sum(num) / sum(den)` resampling whole streams."""
    n = len(num)
    idx = rng.integers(0, n, size=(BOOTSTRAP_RESAMPLES, n))
    d = den[idx].sum(axis=1)
    with np.errstate(invalid="ignore", divide="ignore"):
        r = np.where(d > 0, num[idx].sum(axis=1) / np.maximum(d, 1), np.nan)
    return float(np.nanpercentile(r, 5)), float(np.nanpercentile(r, 95))


def arms_of(run_dir: Path) -> list[Path]:
    return sorted(p for p in run_dir.iterdir() if (p / "results.csv").is_file())


def main(argv: list[str] | None = None) -> int:
    here = Path(__file__).resolve().parent
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--runs-root", type=Path, default=Path("/home/user/gordian/artifacts/runs"))
    ap.add_argument("--out-dir", type=Path, default=here.parent)
    args = ap.parse_args(argv)

    se_rows, curve_rows, ep_rows = [], [], []
    for run_key, rel in RUNS.items():
        run_dir = args.runs_root / rel
        for arm_dir in arms_of(run_dir):
            results = pd.read_csv(arm_dir / "results.csv")
            role = str(results["arm_role"].iloc[0])
            arm = arm_dir.name
            rng = np.random.default_rng(BOOTSTRAP_SEED)
            for tier in ("hard", "all"):
                se = sample_efficiency(results, tier)
                tiers = ("hard",) if tier == "hard" else ("hard", "plain")
                inc = sum(results[f"incidents_{t}"] for t in tiers).to_numpy(float)
                cor = sum(results[f"correct_{t}"] for t in tiers).to_numpy(float)
                lo, hi = bootstrap_ratio(cor, inc, rng)
                row = {
                    "run": run_key,
                    "arm": arm,
                    "role": role,
                    "tier": tier,
                    "streams": se.streams,
                    "incidents_seen": se.incidents,
                    "correct": se.correct,
                    "efficiency": round(se.efficiency, 4),
                    "ci90_lo": round(lo, 4),
                    "ci90_hi": round(hi, 4),
                    "first_half": round(se.first_half, 4),
                    "second_half": round(se.second_half, 4),
                }
                for k in CHECKPOINTS:
                    row[f"after_{k}"] = round(se.at(k), 4) if k <= se.streams else ""
                se_rows.append(row)
                if tier == "hard":
                    for r in se.curve.itertuples():
                        curve_rows.append(
                            {
                                "run": run_key,
                                "arm": arm,
                                "stream": r.stream,
                                "seed": r.key,
                                "cum_incidents": r.cum_incidents,
                                "cum_correct": r.cum_correct,
                                "efficiency": round(r.efficiency, 4),
                            }
                        )
            e_all = energy_proxy(results, "all")
            e_hard = energy_proxy(results, "hard")
            ep_rows.append(
                {
                    "run": run_key,
                    "arm": arm,
                    "role": role,
                    "streams": e_all.streams,
                    "correct_all": e_all.correct,
                    "correct_hard": e_hard.correct,
                    "ns_cheap": e_all.ns_cheap,
                    "ns_reasoner": e_all.ns_reasoner,
                    "ns_total": e_all.ns_total,
                    "ns_per_correct_all": round(e_all.ns_per_correct, 1),
                    "ns_per_correct_hard": round(e_hard.ns_per_correct, 1),
                    "joules_total_PLACEHOLDER": f"{e_all.joules_total:.6g}",
                    "joules_per_correct_all_PLACEHOLDER": f"{e_all.joules_per_correct:.6g}",
                    "joules_per_correct_hard_PLACEHOLDER": f"{e_hard.joules_per_correct:.6g}",
                    "conversion": e_all.assumptions,
                }
            )

    def write(name: str, rows: list[dict]) -> None:
        path = args.out_dir / name
        with open(path, "w", newline="") as fh:
            w = csv.DictWriter(fh, fieldnames=list(rows[0]), lineterminator="\n")
            w.writeheader()
            w.writerows(rows)
        print(f"wrote {path} ({len(rows)} rows)")

    write("w1-sample-efficiency.csv", se_rows)
    write("w1-sample-efficiency-curves.csv", curve_rows)
    write("w1-energy-proxy.csv", ep_rows)
    print(PLACEHOLDER_LABEL)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
