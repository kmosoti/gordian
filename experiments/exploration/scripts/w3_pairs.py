#!/usr/bin/env python3
"""W3 item 4 (hidden side): how much a learner that needs an earlier sighting of a pair could ever predict.

Usage: w3_pairs.py [--hidden-root DIR] [--out-dir DIR]

A true partner event is a cascade incident (hard or decoy: the same draws) or an incident the added
edge altered. It is a pair `(site, partner)`; for the added edge the partner is any of the services
newly downstream of the site, so two events repeat a pair when they share the site and a newly
downstream service. Within a stream, for each event: how many earlier events are the same pair. A
learner whose evidence is co-alarm timing of one pair (A2: net follows beyond chance, threshold two)
can predict an event only if the pair occurred before in the same stream, because nothing carries
across streams (W2). The table is the ceiling on what such a learner can cover, per stream and per
range: events with at least one and with at least two earlier sightings of the pair. Side: hidden.
"""

from __future__ import annotations

import argparse
import pathlib

import pandas as pd

import w2_common as C
import w2_laws as L
from w2_common import boot_ratio, f

HIDDEN = C.ROOT / "artifacts" / "runs" / "w3" / "hidden"
RANGES = ["a-tune", "a-heldout", "c-tune", "c-heldout"]


def events(root: pathlib.Path, name: str) -> pd.DataFrame:
    t = L.prepare(C.read_range(root, name))
    inc = t["inc"]
    fi = pd.read_csv(root / name / "floor-incidents.csv", keep_default_na=False, na_values=[""], dtype={"new_down_site": str})
    inc = inc.merge(fi[["seed", "incident", "new_down_site"]], on=["seed", "incident"], how="left")
    rows = []
    for r in inc.itertuples():
        if r.family == "cascade" and r.tier in ("hard", "decoy") and pd.notna(r.other):
            rows.append((r.seed, r.incident, f"cascade {r.tier}", int(r.site), frozenset({int(r.other)}), r.onset_ns))
        elif r.edge_altered == 1 and isinstance(r.new_down_site, str) and r.new_down_site:
            rows.append((r.seed, r.incident, "added edge", int(r.site), frozenset(int(x) for x in r.new_down_site.split("|")), r.onset_ns))
    return pd.DataFrame(rows, columns=["seed", "incident", "kind", "a", "targets", "onset_ns"]), t["seeds"]


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--hidden-root", type=pathlib.Path, default=HIDDEN)
    ap.add_argument("--out-dir", type=pathlib.Path, default=C.OUT)
    ap.add_argument("--ranges", default=",".join(RANGES + ["a-a2"]))
    a = ap.parse_args(argv)
    out = []
    for name in a.ranges.split(","):
        ev, seeds = events(a.hidden_root, name)
        ev = ev.sort_values(["seed", "onset_ns"]).reset_index(drop=True)
        p1, p2 = [], []
        for _, g in ev.groupby("seed"):
            seen = []
            for r in g.itertuples():
                n = sum(1 for (a_, t_) in seen if a_ == r.a and (t_ & r.targets))
                p1.append(n >= 1)
                p2.append(n >= 2)
                seen.append((r.a, r.targets))
        ev["prior1"], ev["prior2"] = p1, p2
        for kind in ["all", "cascade hard", "cascade decoy", "added edge"]:
            e = ev if kind == "all" else ev[ev["kind"] == kind]
            n = e.groupby("seed").size().reindex(seeds, fill_value=0).to_numpy(float)
            n1 = e[e["prior1"]].groupby("seed").size().reindex(seeds, fill_value=0).to_numpy(float)
            n2 = e[e["prior2"]].groupby("seed").size().reindex(seeds, fill_value=0).to_numpy(float)
            p, lo, hi = boot_ratio(n)
            out.append({"range": name, "seeds": C.seed_range(seeds), "side": "hidden", "events": kind, "streams": len(seeds), "n_events": int(n.sum()),
                        "per_stream": f(p), "lo90": f(lo), "hi90": f(hi), "with_one_earlier_sighting_of_the_pair": int(n1.sum()),
                        "per_stream_with_one": f(n1.mean()), "with_two_earlier_sightings": int(n2.sum()), "per_stream_with_two": f(n2.mean())})
    C.write_csv(a.out_dir / "w3-pairs.csv", out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
