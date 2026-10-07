#!/usr/bin/env python3
"""W3 item 1 (run, hidden): W2's item 6, the perfect-memory ceiling, on world C, and what the
oracle arm did with the extra hard incidents.

Usage:
    w3_ceiling.py --run NAME=RUN_DIR[@HIDDEN_ROOT] ... [--hidden-root DIR] [--out-dir DIR]

Calls W2's own functions (`w2_ceiling`: `join_run`, `reach_flags`, `ceiling_rows`, `bill_row`,
`unasked_rows`, `hard_incident_rows`, `propagation_rows`) so that a number means what it meant in
W2. `RUN_DIR` is a run directory of `scripts/run-driver.sh` for the arm `sel_reanchor_privileged`
(the selection oracle at 16 s with the rung's context under B2's re-anchor noticer, b = 5, rho =
0.7), with the ledger kept; the hidden tables are `HIDDEN_ROOT/NAME` (default `--hidden-root`).
World A's runs are W2's (`artifacts/runs/w2/`, read only) with W2's hidden root, which carries the
owner maps. Adds `w3-ceiling-deadlines.csv`: per range and tier, what the arm noticed, asked,
declared correctly, declared correctly by the deadline, missed. Side: run joined to hidden.
"""

from __future__ import annotations

import argparse
import pathlib

import numpy as np
import pandas as pd

import w2_ceiling as W
import w2_common as C
from w2_common import boot_ratio, f

ROOT = C.ROOT
HIDDEN = ROOT / "artifacts" / "runs" / "w3" / "hidden"


def deadline_rows(name: str, t: dict, j: pd.DataFrame) -> list[dict]:
    seeds = t["seeds"]
    rows = []
    for tier in ("hard", "plain", "decoy"):
        for label, m in (("all", j["tier"] == tier), ("critical", (j["tier"] == tier) & (j["critical"] == 1))):
            d = j[m]
            if d.empty:
                continue
            n = W.per_stream_sum(d, seeds, lambda x: np.ones(len(x)))
            noticed = d["noticed"].fillna(False).astype(bool)
            correct = d["correct_declarations"] > 0
            by_dl = d["correct_by_deadline"].fillna(False).astype(bool)
            late = correct & ~by_dl
            asked = d["calls"] > 0
            row = {"range": name, "seeds": C.seed_range(seeds), "side": "run joined to hidden", "tier": tier, "set": label,
                   "incidents": int(n.sum()), "noticed": int(noticed.sum()), "asked": int(asked.sum()),
                   "declared_correctly": int(correct.sum()), "correct_by_deadline": int(by_dl.sum()),
                   "correct_but_late": int(late.sum()), "never_correct": int((~correct).sum())}
            for col, mask in (("correct_by_deadline", by_dl), ("correct_but_late", late), ("noticed", noticed), ("asked", asked)):
                x = W.per_stream_sum(d[mask], seeds, lambda y: np.ones(len(y)))
                p, lo, hi = boot_ratio(x, n)
                row[col + "_share"] = f(p)
                row[col + "_lo90"] = f(lo)
                row[col + "_hi90"] = f(hi)
            rows.append(row)
    return rows


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--run", action="append", required=True, metavar="NAME=DIR[@HIDDEN]")
    ap.add_argument("--hidden-root", type=pathlib.Path, default=HIDDEN)
    ap.add_argument("--out-dir", type=pathlib.Path, default=C.OUT)
    args = ap.parse_args(argv)
    ceil, bill, unasked, hard_rows, prop, dl = [], [], [], [], [], []
    for spec in args.run:
        name, rest = spec.split("=", 1)
        d, _, hroot = rest.partition("@")
        run_dir = pathlib.Path(d)
        hidden_root = pathlib.Path(hroot) if hroot else args.hidden_root
        t, j, res = W.join_run(name, run_dir, hidden_root)
        rec = t["rec"]
        key = pd.MultiIndex.from_frame(j[["seed", "incident"]])
        j["stale_any_row"] = rec.set_index(["seed", "incident"])["stale_any"].reindex(key).fillna(False).to_numpy(dtype=bool)
        j = W.reach_flags(j)
        ceil += W.ceiling_rows(name, t, j, res)
        ceil += W.ceiling_rows(name, t, j, res, W.SETS_CROSS, W.CROSS_SKIP, f" [streams {W.CROSS_SKIP + 1} onward]")
        bill.append(W.bill_row(name, t, j, res))
        unasked += W.unasked_rows(name, t, j)
        hard_rows += W.hard_incident_rows(name, t, j)
        prop += W.propagation_rows(name, t, j)
        dl += deadline_rows(name, t, j)
    C.write_csv(args.out_dir / "w3-ceiling.csv", ceil)
    C.write_csv(args.out_dir / "w3-ceiling-bill.csv", bill)
    C.write_csv(args.out_dir / "w3-ceiling-unasked.csv", unasked)
    C.write_csv(args.out_dir / "w3-ceiling-hard-incidents.csv", hard_rows)
    C.write_csv(args.out_dir / "w3-ceiling-propagation.csv", prop)
    C.write_csv(args.out_dir / "w3-ceiling-deadlines.csv", dl)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
