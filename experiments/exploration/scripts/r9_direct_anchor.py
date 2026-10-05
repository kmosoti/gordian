"""R9 beside the direct check: how far is the anchor of R6's calls from the incident's onset, and is the
reader's error concentrated in calls whose anchor is late?

Usage: r9_direct_anchor.py DIAG_RUN_DIR LABELS_JSONL OUT_CSV

The frozen reader treats the first alarm as onset (phase 2 is 6 to 16 s after it). In R6's ledgers a
call's focus is its anomaly's anchor, which can be later than the incident's first observation. For the
ceiling arm and each builder arm, per counted call: the anchor's offset from onset, the reader's answer,
and whether the call's context holds all of the incident's decisive evidence that had arrived. Labels are
read here as the evaluator reads them. Exploration (nothing here tests a hypothesis).
"""

import csv
import importlib
import json
import sys
from pathlib import Path

import numpy as np

from r5_trace import read_blocks
from r9_direct import FAMILY_KIND, records, CEILING_PREFIX, SKIP_PREFIX

reader = importlib.import_module("r9_reader")
run = Path(sys.argv[1])
labels = {}
for line in open(sys.argv[2]):
    d = json.loads(line)
    labels[d["seed"]] = d["incidents"]
rows = []
arms = sorted(p.name for p in run.iterdir() if p.is_dir() and (p / "events-sample.jsonl").exists())
for arm in [a for a in arms if not a.startswith(SKIP_PREFIX)]:
    for blk in read_blocks(run / arm / "events-sample.jsonl"):
        seed = blk["seed"]
        inc = {i["id"]: i for i in labels[seed]}
        owner = {o: i["id"] for i in labels[seed] for o in i["observations"]}
        for did, dec in sorted(blk["decisions"].items()):
            act = dec["payload"]["action"]
            if "Escalate" not in act:
                continue
            out = blk["outcomes"].get(did)
            if out is None or "Escalated" not in out["payload"]:
                continue
            esc = act["Escalate"]
            focus = esc["question"]["Diagnose"]["focus"]
            incident = owner.get(focus)
            if incident is None or inc[incident]["tier"] != "hard" or inc[incident]["family"] not in FAMILY_KIND:
                continue
            info = inc[incident]
            ids = [r["Passive"] for r in esc["context"] if "Passive" in r]
            decisive = info["decisive"]
            T = dec["at_ns"]
            arrived = [m for m in decisive if blk["obs"][m]["at_ns"] <= (T // 500_000_000) * 500_000_000]
            kind, site, _ = reader.read(blk["public"]["services"], records(blk, [focus])[0], records(blk, ids))
            rows.append({
                "arm": arm, "seed": seed, "incident": incident, "family": info["family"],
                "anchor_offset_s": (blk["obs"][focus]["at_ns"] - info["onset_ns"]) / 1e9,
                "first_obs_offset_s": (blk["obs"][info["observations"][0]]["at_ns"] - info["onset_ns"]) / 1e9,
                "decisive": len(decisive), "arrived": len(arrived),
                "arrived_in_context": sum(1 for m in arrived if m in set(ids)),
                "correct": int(kind == FAMILY_KIND[info["family"]]),
            })
with open(sys.argv[3], "w", newline="") as fh:
    w = csv.DictWriter(fh, fieldnames=list(rows[0]))
    w.writeheader()
    w.writerows(rows)
for arm in sorted({r["arm"] for r in rows}):
    rs = [r for r in rows if r["arm"] == arm]
    off = np.array([r["anchor_offset_s"] for r in rs])
    late = off > 0.1
    comp = np.array([r["arrived_in_context"] == r["arrived"] for r in rs])
    acc = np.array([r["correct"] for r in rs])
    print(f"{arm[:36]:36s} calls {len(rs)} anchor late (>0.1 s) {late.sum()} median offset {np.median(off):.3f} s; "
          f"acc late {acc[late].mean() if late.any() else float('nan'):.3f} not late {acc[~late].mean():.3f}; "
          f"complete evidence {comp.mean():.3f}, acc complete {acc[comp].mean():.3f} incomplete {acc[~comp].mean() if (~comp).any() else float('nan'):.3f}")
