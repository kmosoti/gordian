#!/usr/bin/env python3
"""W3 item 2 (instrument check): E1's actual recalls against the hidden-side floor, same keys, same seeds.

Usage: w3_e1join.py --e1-run DIR [--hidden-root DIR] [--out-dir DIR]

`DIR` is E1's held-out run (`artifacts/runs/e1/e1-heldout-b5-rho0.7`, seeds 40000-40199, read only):
one directory per arm, each with `recalls.csv` (seed, incident, tier, correct, source_class,
source_incident, source_age, source_seed). For every arm that never confirms a recall (policy
`never`; the confirm-every-k arms act on the same keys but change what is asked), this script runs
the hidden-side memory of `w3_floor.py` on the same key form, level and reset (the sources are hard
incidents with a snapshot, binding at their answer's instant) and compares the two recall sets
incident by incident. The floor is computed from each incident's own observations; E1's arm saw the
whole stream, so its anomaly evidence holds other incidents' and background symptoms. The
comparison measures how far that contamination moves the arm from its floor. Side: *run* (E1's
arm, joined to its truth by the evaluator) against *hidden* (the floor).
"""

from __future__ import annotations

import argparse
import pathlib

import pandas as pd

import w2_common as C
import w3_floor as F

ARMS = {  # directory -> (form, level, reset)
    "sel_rec_site_kinds_never_reset_privileged": ("site", "kinds", True),
    "sel_rec_site_kinds_never_carry_sens_privileged": ("site", "kinds", False),
    "sel_rec_site_bands_never_reset_sens_privileged": ("site", "bands", True),
    "sel_rec_site_bands_never_carry_sens_privileged": ("site", "bands", False),
    "sel_rec_site_timing_never_reset_sens_privileged": ("site", "timing", True),
    "sel_rec_site_timing_never_carry_privileged": ("site", "timing", False),
    "sel_rec_family_kinds_never_reset_sens_privileged": ("family", "kinds", True),
    "sel_rec_family_kinds_never_carry_sens_privileged": ("family", "kinds", False),
    "sel_rec_family_bands_never_reset_sens_privileged": ("family", "bands", True),
    "sel_rec_family_bands_never_carry_sens_privileged": ("family", "bands", False),
    "sel_rec_family_timing_never_reset_privileged": ("family", "timing", True),
    "sel_rec_family_timing_never_carry_privileged": ("family", "timing", False),
}


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--e1-run", required=True, type=pathlib.Path)
    ap.add_argument("--hidden-root", type=pathlib.Path, default=F.HIDDEN)
    ap.add_argument("--out-dir", type=pathlib.Path, default=F.OUT)
    args = ap.parse_args(argv)
    t = F.load(args.hidden_root, "a-heldout")
    inc, fr, seeds = t["inc"], t["fr"], t["seeds"]
    snap = fr[fr["cut"] == "snap"]
    snap_at = {(r.seed, r.incident): r.cut_ns for r in snap.itertuples()}
    start = {(r.seed, r.incident): r.t0_ns for r in inc.itertuples()}
    tier = {(r.seed, r.incident): r.tier for r in inc.itertuples()}
    rows = []
    for arm, (form, level, reset) in ARMS.items():
        rec = pd.read_csv(args.e1_run / arm / "recalls.csv", keep_default_na=False)
        e1 = {(int(r.seed), int(r.incident)) for r in rec.itertuples() if r.incident != ""}
        keys = {(r.seed, r.incident): F.e1_key(r, level) for r in snap.itertuples()}
        bind = {k: start[k] + F.LAG_NS for k in keys}
        look = {k: start[k] + snap_at[k] for k in keys}
        res = F.simulate(inc, seeds, keys, bind, look, form, not reset, ("hard",))
        mine = {(int(r.seed), int(r.incident)) for r in res[res["recalled"]].itertuples()}
        e1_gate = sum(1 for k in e1 if k in snap_at)
        no_gate = {k for k in e1 if k not in snap_at}
        both = e1 & mine
        e1_only = e1 - mine
        mine_only = mine - e1

        def by_tier(s):
            c = {"plain": 0, "hard": 0, "decoy": 0}
            for k in s:
                c[tier[k]] = c.get(tier[k], 0) + 1
            return c
        # E1 recalls with a source in the same stream: do the hidden keys of source and target agree?
        same = rec[(rec["source_age"].astype(str) == "0") & (rec["source_incident"].astype(str) != "") & (rec["incident"].astype(str) != "")]
        agree = tot = 0
        for r in same.itertuples():
            a, b = (int(r.seed), int(r.incident)), (int(r.source_seed), int(r.source_incident))
            if a in keys and b in keys:
                tot += 1
                agree += keys[a] == keys[b]
        rows.append({"arm": arm, "seeds": "40000-40199", "side": "run vs hidden", "form": form, "level": level, "reset": reset,
                     "e1_recalls": len(e1), "e1_recalls_on_incidents_with_my_snapshot": e1_gate,
                     "e1_recalls_without_own_gate_by_tier": "/".join(str(by_tier(no_gate)[k]) for k in ("plain", "hard", "decoy")),
                     "e1_by_tier_plain_hard_decoy": "/".join(str(by_tier(e1)[k]) for k in ("plain", "hard", "decoy")),
                     "floor_recalls_answered": len(mine), "floor_by_tier_plain_hard_decoy": "/".join(str(by_tier(mine)[k]) for k in ("plain", "hard", "decoy")),
                     "in_both": len(both), "e1_only": len(e1_only), "floor_only": len(mine_only),
                     "e1_same_stream_recalls_with_both_keys": tot, "of_which_hidden_keys_of_source_and_target_equal": agree})
    C.write_csv(args.out_dir / "w3-floor-e1-join.csv", rows)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
