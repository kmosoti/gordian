#!/usr/bin/env python3
"""W3: what the new power moves in A1b's proposed bounds (W2 section 10.2), from the committed CSVs.

Usage: w3_bounds.py [--out-dir DIR]

Reads `w3-ceiling.csv`, `w3-world-compare.csv` and writes `w3-bounds.csv`: for world A (W2's ranges)
and world C, on the tuning-like and held-out ranges, the quantities the bound's arithmetic needs:
the hard recurrences and the reach sets (`w3-ceiling.csv`), the smallest count of unasked-correct
hard recurrences whose 90% normal-approximation lower bound clears 0.075 (W2's "at least 0.15 of
them with a lower bound above 0.075": 12 of 83), the half-width of a paired difference in share when
a fixed fraction of the recurrences is one where two arms disagree (W2: about 10 of 83), and the
half-width of the ceiling on the bill. Deterministic; nothing is fitted.
"""

from __future__ import annotations

import argparse
import math
import pathlib

import pandas as pd

import w2_common as C

Z90 = 1.645
DISAGREE = 10 / 83  # W2's assumption: two arms disagree on 10 of the 83 hard recurrences


def min_count_for_lower_bound(n: int, bound: float = 0.075) -> int:
    for k in range(n + 1):
        p = k / n
        if p - Z90 * math.sqrt(p * (1 - p) / n) > bound:
            return k
    return n + 1


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--out-dir", type=pathlib.Path, default=C.OUT)
    a = ap.parse_args(argv)
    ce = pd.read_csv(a.out_dir / "w3-ceiling.csv", keep_default_na=False)
    rows = []
    for rng in ("a-tune", "a-heldout", "c-tune", "c-heldout"):
        d = ce[(ce["range"] == rng) & ~ce["set"].str.contains("streams")]
        get = lambda key: d[d["set"].str.startswith(key + "  ")].iloc[0]  # noqa: E731
        rw, rr, rf = get("R_world"), get("R"), get("R_fresh")
        n = int(rw["incidents"])
        k = min_count_for_lower_bound(n)
        hw_pair = Z90 * math.sqrt(DISAGREE * n) / n
        bill = float(rr["share_of_total_bill"])
        hw_bill = (float(rr["bill_hi90"]) - float(rr["bill_lo90"])) / 2
        rows.append({
            "range": rng, "seeds": rw["seeds"], "side": "run joined to hidden",
            "hard_recurrences": n, "R_site_keyed_reach": int(rr["incidents"]), "R_share_of_recurrences": f"{int(rr['incidents']) / n:.3f}",
            "R_fresh_signature_keyed_reach": int(rf["incidents"]), "R_fresh_share_of_recurrences": f"{int(rf['incidents']) / n:.3f}",
            "smallest_unasked_correct_count_with_lower_bound_above_0.075": k, "as_share_of_recurrences": f"{k / n:.3f}",
            "paired_margin_halfwidth_90_if_10_of_83_disagree": f"{hw_pair:.4f}",
            "R_share_of_bill": f"{bill:.4f}", "R_bill_halfwidth_90": f"{hw_bill:.4f}",
            "forty_percent_of_signature_ceiling": f"{0.4 * int(rf['incidents']) / n:.3f}",
            "record_rung_share_above_which_plus_0.10_is_out_of_reach": f"{0.73 * int(rf['incidents']) / n:.3f}",
        })
    C.write_csv(a.out_dir / "w3-bounds.csv", rows)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
