"""B4 exploration: what the counter readings after a ramp's notice look like for leaks, decoys and the
other anomalies the ramp noticer opens, on the tuning streams (10000-10099) only.

Usage:
  b4_explore.py manifest     write the exploration manifest: the ramp + split noticer over the re-anchor
                             (B3's chosen parameters) under `never_escalate`, on the tuning streams
  b4_explore.py series DUMPS summarise, per class of the notice's anchor, the readings that follow the
                             completing reading: reads the run's notice files and the public dump of each
                             stream (DUMPS: a directory of `seed-<n>.jsonl`, written by the `dump` example)

Exploration (nothing here tests a hypothesis). It reads the evaluator's labels for the tuning streams (the
tier and family of the incident each notice's anchor belongs to) beside the public readings, the route
B2's gap, B3's ramp and M2's graph were chosen by; the report names it as a route by which hidden labels
shaped a constant. Nothing here is read on a held-out stream. The output is a table of what was seen, not
a rule: the follow-up rule's form is fixed in `noticer_follow.rs` and its grid in `b4_common.py`.
"""

import json
import sys

import numpy as np
import pandas as pd

import b2_manifests as M2
import b3_common as C3
import b3_manifests as M3
from gordian_analysis.load import load_stream_arm

NS = 1_000_000_000


def manifest():
    sel = C3.load_selected()
    rows = dict(M3.table_rows(sel))
    arm = "explore_ramp_split_re2_never"
    M2.write(
        "b4-explore-b5-rho0.7", C3.TUNING_SEEDS, [(arm, "never_escalate")],
        {arm: rows["ramp_split_over_re2"]}, 13_001, "exploration-b4-explore",
    )


def readings(dump_path):
    out = {}
    with open(dump_path) as fh:
        for line in fh:
            o = json.loads(line)
            ob = o["obs"]
            if "Counter" in ob:
                c = ob["Counter"]
                out.setdefault((c["service"], c["name"]), []).append((o["id"], o["at"], c["value"]))
    return out


def anchor_key(dump_path):
    """id -> (key, value) for the counter readings of a stream's dump."""
    out = {}
    with open(dump_path) as fh:
        for line in fh:
            o = json.loads(line)
            ob = o["obs"]
            if "Counter" in ob:
                c = ob["Counter"]
                out[o["id"]] = ((c["service"], c["name"]), c["value"], o["at"])
    return out


def series(dumps):
    import pathlib

    dumps = pathlib.Path(dumps)
    arm = load_stream_arm(C3.RUNS / "b4-explore-b5-rho0.7" / "explore_ramp_split_re2_never")
    ev = arm.notice_events
    ev = ev[ev["event"] == "notice"].copy()
    inc = arm.incidents.set_index(["seed", "incident"])
    rows = []
    for seed, g in ev.groupby("seed"):
        ak = anchor_key(dumps / f"seed-{seed}.jsonl")
        by_key = readings(dumps / f"seed-{seed}.jsonl")
        for _, n in g.iterrows():
            a = int(n["anchor"])
            if a not in ak:
                continue  # not a counter reading: not a ramp notice
            key, v0, t0 = ak[a]
            tier = family = "background"
            if pd.notna(n["incident"]):
                i = inc.loc[(seed, int(n["incident"]))]
                tier, family = i["tier"], i["family"]
            cls = "leak" if family == "slow_leak" else tier
            seq = [(i_, t, v) for i_, t, v in by_key[key] if i_ >= a]
            rows.append({"seed": seed, "anomaly": int(n["anomaly"]), "class": cls, "key": str(key),
                         "anchor_at": t0, "notice_at": int(n["at_ns"]),
                         "values": [v for _, _, v in seq[:16]],
                         "times_s": [round((t - t0) / NS, 1) for _, t, _ in seq[:16]]})
    df = pd.DataFrame(rows)
    df.to_json(C3.OUT / "b4-explore-series.json", orient="records", indent=1)
    print(df["class"].value_counts().to_string())
    for cls in ("leak", "decoy", "plain", "hard", "background"):
        sub = df[df["class"] == cls]
        print(f"\n== {cls}: {len(sub)} anchored counter notices; first 12 value series (10 readings from the anchor)")
        for _, r in sub.head(12).iterrows():
            print(r["seed"], r["values"][:12], r["times_s"][:12])


def main():
    stage = sys.argv[1] if len(sys.argv) > 1 else ""
    if stage == "manifest":
        manifest()
    elif stage == "series":
        series(sys.argv[2])
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
