#!/usr/bin/env python3
"""W3 item 1 (hidden side): W2's items 1, 2 and 5 on world C, with world A beside, and what else
changed with the tier mix.

Usage: w3_laws.py [--hidden-root DIR] [--out-dir DIR]

Reads the hidden tables `w3_hidden.sh` wrote under `artifacts/runs/w3/hidden/` for four ranges:
`a-tune` 10000-10099 and `a-heldout` 40000-40199 (world A, the defaults: these tables are
byte-identical to W2's) and `c-tune` 60000-60099 and `c-heldout` 70000-70199 (world C: hard share
x3, plain 600 / hard 300 / decoy 100 per mille). It calls W2's own functions (`w2_laws`) for items
1, 2 and 5 so that a number means the same thing in both worlds, and adds `w3-world-compare.csv`:
what else the mix changed (incidents per stream, overlap, skipped arrivals, the background share
of the stream, how long incidents live). Every row names its seed range and its side (hidden).
Nothing here reaches an arm.
"""

from __future__ import annotations

import argparse
import pathlib

import numpy as np
import pandas as pd

import w2_common as C
import w2_laws as L
from w2_common import boot_ratio, f, per_stream

ROOT = C.ROOT
HIDDEN = ROOT / "artifacts" / "runs" / "w3" / "hidden"
OUT = C.OUT
RANGES = {"a-tune": "a", "a-heldout": "a", "c-tune": "c", "c-heldout": "c"}


def world_compare(name: str, t: dict) -> list[dict]:
    """What else changed with the mix, per stream (hidden)."""
    inc, streams, seeds = t["inc"], t["streams"], t["seeds"]
    rows = []
    n_inc = per_stream(inc, inc["tier"].notna(), seeds)

    def add(measure, num, den=None, nd=4):
        p, lo, hi = boot_ratio(num, den)
        rows.append({"range": name, "seeds": C.seed_range(seeds), "side": "hidden", "measure": measure,
                     "value": f(p, nd), "lo90": f(lo, nd), "hi90": f(hi, nd)})

    add("incidents per stream", n_inc)
    for tier in ("plain", "hard", "decoy"):
        add(f"{tier} incidents per stream", per_stream(inc, inc["tier"] == tier, seeds))
        add(f"{tier} share of incidents", per_stream(inc, inc["tier"] == tier, seeds), n_inc)
    add("recurrences per stream", per_stream(inc, inc["is_rec"], seeds))
    add("hard recurrences per stream", per_stream(inc, inc["is_rec"] & (inc["tier"] == "hard"), seeds))
    add("share of hard incidents that recur", per_stream(inc, inc["is_rec"] & (inc["tier"] == "hard"), seeds),
        per_stream(inc, inc["tier"] == "hard", seeds))
    # Arrivals skipped because no suitable service was free.
    sk = streams.set_index("seed")["skipped_arrivals"].reindex(seeds).to_numpy(float)
    add("skipped arrivals per stream (no service free)", sk)
    add("share of arrivals skipped", sk, sk + n_inc)
    # Overlap: incidents whose live interval [onset, live_end] meets another incident's.
    ov_any, ov_hard, concurrent_hard, live_s, hard_live_s = [], [], [], [], []
    ov_hard_any_tier, busy_frac = [], []
    for s in seeds:
        g = inc[inc["seed"] == s]
        on = g["onset_ns"].to_numpy()
        le = g["live_end_ns"].to_numpy()
        bu = g["busy_until_ns"].to_numpy()
        tier = g["tier"].to_numpy()
        n = len(g)
        any_ov = np.zeros(n, bool)
        conc = np.zeros(n)
        for i in range(n):
            m = (on < le[i]) & (on[i] < le) & (np.arange(n) != i)
            any_ov[i] = m.any()
            conc[i] = m.sum()
        ov_any.append(any_ov.sum())
        ov_hard.append((any_ov & (tier == "hard")).sum())
        concurrent_hard.append(conc[tier == "hard"].sum())
        live_s.append(((le - on) / C.NS).sum())
        hard_live_s.append((((le - on) / C.NS)[tier == "hard"]).sum())
        # busy services-seconds per stream over the duration (per service, the occupation of services)
        busy_frac.append(((bu - on) / C.NS).sum())
    ov_any, ov_hard = np.array(ov_any, float), np.array(ov_hard, float)
    add("incidents overlapping another incident's live interval, per stream", ov_any)
    add("share of incidents overlapping another incident", ov_any, n_inc)
    add("hard incidents overlapping another incident, per stream", ov_hard)
    add("share of hard incidents overlapping another incident", ov_hard, per_stream(inc, inc["tier"] == "hard", seeds))
    add("other incidents live during a hard incident, mean per hard incident", np.array(concurrent_hard),
        per_stream(inc, inc["tier"] == "hard", seeds), 3)
    add("incident-live seconds per stream (sum over incidents of live_end - onset)", np.array(live_s), None, 2)
    add("mean live seconds per incident", np.array(live_s), n_inc, 2)
    add("mean busy seconds per incident (busy_until - onset)", np.array(busy_frac), n_inc, 2)
    # The share of the delivered stream that belongs to no incident (background).
    obs = streams.set_index("seed")["observations"].reindex(seeds).to_numpy(float)
    inc_obs = per_stream_sum(inc, "n_obs", seeds)
    add("observations per stream", obs, None, 1)
    add("background share of observations (not labelled with any incident)", obs - inc_obs, obs)
    add("incident-labelled observations per stream", inc_obs, None, 1)
    ff = streams.set_index("seed")["freeform_observations"].reindex(seeds).to_numpy(float)
    add("free-form messages per stream", ff, None, 1)
    # Deadline structure (hidden): critical share and the deadline of hard incidents.
    h = inc["tier"] == "hard"
    add("critical share of hard incidents", per_stream(inc, h & (inc["critical"] == 1), seeds), per_stream(inc, h, seeds))
    add("critical share of plain incidents", per_stream(inc, (inc["tier"] == "plain") & (inc["critical"] == 1), seeds),
        per_stream(inc, inc["tier"] == "plain", seeds))
    # The generator's own tier mix is per arrival; the first incident has no template to repeat.
    elig = inc["eligible_templates"] > 0
    add("arrivals with an eligible template, per stream", per_stream(inc, elig, seeds))
    # Where in the stream the hard incidents are.
    last = inc["seed"].map(t["reg"].groupby("seed")["at_ns"].max())
    add("share of hard incidents after the last regime change", per_stream(inc, h & (inc["onset_ns"] >= last), seeds),
        per_stream(inc, h, seeds))
    return rows


def per_stream_sum(df: pd.DataFrame, col: str, seeds) -> np.ndarray:
    s = df.groupby("seed")[col].sum()
    return s.reindex(seeds, fill_value=0).to_numpy(dtype=float)


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--hidden-root", type=pathlib.Path, default=HIDDEN)
    ap.add_argument("--out-dir", type=pathlib.Path, default=OUT)
    args = ap.parse_args(argv)
    keys = ("recurrence-summary", "recurrence-eligibility", "recurrence-gaps", "experience-curve", "stale",
            "family-elsewhere", "regime-effects", "regime-hard-after", "recurrences", "world-compare")
    tables: dict[str, list[dict]] = {k: [] for k in keys}
    for name in RANGES:
        t = L.prepare(C.read_range(args.hidden_root, name))
        print(f"{name}: seeds {C.seed_range(t['seeds'])}, {len(t['inc'])} incidents, {len(t['rec'])} recurrences")
        tables["recurrence-summary"] += L.item1_summary(name, t)
        tables["recurrence-eligibility"].append(L.item1_eligibility(name, t))
        tables["recurrence-gaps"] += L.item1_gaps(name, t)
        tables["experience-curve"] += L.item1_curve(name, t)
        tables["stale"] += L.item1_stale(name, t)
        tables["family-elsewhere"] += L.item2(name, t)
        eff, after, _ = L.item5(name, t)
        tables["regime-effects"] += eff
        tables["regime-hard-after"] += after
        tables["recurrences"] += L.recurrence_table(name, t)
        tables["world-compare"] += world_compare(name, t)
    for k, rows in tables.items():
        C.write_csv(args.out_dir / f"w3-{k}.csv", rows)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
