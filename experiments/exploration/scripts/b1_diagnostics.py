"""B1 diagnostics beside the table: what the headline numbers are made of.

Usage: b1_diagnostics.py   (after the held-out run; writes experiments/exploration/
                            b1-families.csv, b1-offsets.csv, b1-lifetimes.csv, b1-flood-floor.csv)

Exploration (nothing here tests a hypothesis). All four read the held-out run's notice files.

* families: per hard family, incidents, noticed, anchor-correct and the median notice latency among
  the noticed, for every configuration.
* offsets: for notices about an incident that are not anchor-correct, how far after the incident's
  first observation the anchor is (the evaluator's `anchor_offset_ns`), in bands, per configuration.
* lifetimes: for notices anchored on a hard incident, how long the anomaly lived (retirement minus
  notice; an anomaly never retired lived to the stream's end) against the selection oracle's delay
  (16 s after notice): the oracle asks only about an anomaly still live when the delay has passed.
* flood floor: for each ChangeTriggered configuration, the share of its notices on background that
  fall in the first 30 s of the stream (a node's first abnormal observation is a notice under the
  noticer's reading that a node never seen is quiet), and what is left after.
"""

import numpy as np
import pandas as pd

import b1_common as C
from gordian_analysis.load import load_stream_run

DELAY_NS = 16_000_000_000
BANDS = [(1e9, 2e9), (2e9, 5e9), (5e9, 10e9), (10e9, 30e9), (30e9, float("inf"))]


def main():
    run = load_stream_run(C.RUNS / C.run_id("heldout"))
    fam, off, life, flood = [], [], [], []
    for name, kind, p in C.grid():
        arm = run.arms[C.arm_name(name)]
        ni, ev = arm.notice_incidents, arm.notice_events
        hard = ni[ni.tier == "hard"]
        for f in C.FAMILIES:
            g = hard[hard.family == f]
            lat = g["notice_latency_ns"].dropna().astype("int64") / 1e9
            fam.append({"config": name, "family": f, "incidents": len(g), "noticed": int(g.noticed.sum()),
                        "anchor_correct": int(g.anchor_correct.sum()),
                        "median_latency_s": float(lat.median()) if len(lat) else float("nan")})
        notices = ev[ev.event == "notice"]
        wrong = notices[(notices.incident.notna()) & (~notices.anchor_correct.astype(bool))]
        row = {"config": name, "notices_on_incidents": int(notices.incident.notna().sum()),
               "not_anchor_correct": len(wrong)}
        for lo, hi in BANDS:
            hi_s = "inf" if np.isinf(hi) else f"{hi / 1e9:g}"
            row[f"offset_{lo / 1e9:g}_to_{hi_s}_s"] = int(((wrong.anchor_offset_ns >= lo) & (wrong.anchor_offset_ns < hi)).sum())
        off.append(row)
        # lifetimes of the anomalies anchored on a hard incident
        hard_incidents = ni[ni.tier == "hard"][["seed", "incident"]]
        hn = notices.merge(hard_incidents, on=["seed", "incident"], how="inner")
        ret = ev[ev.event == "retire"][["seed", "anomaly", "at_ns"]].rename(columns={"at_ns": "retired_at_ns"})
        hn = hn.merge(ret, on=["seed", "anomaly"], how="left")
        lifetime = (hn.retired_at_ns - hn.at_ns).astype("float")  # NaN: never retired
        alive = lifetime.isna() | (lifetime >= DELAY_NS)
        life.append({"config": name, "notices_on_hard": len(hn), "live_when_the_delay_passes": int(alive.sum()),
                     "retired_before_the_delay": int((~alive).sum()),
                     "median_lifetime_s": float(np.nanmedian(lifetime) / 1e9) if lifetime.notna().any() else float("nan")})
        if kind == "change_triggered":
            bg = notices[notices.incident.isna()]
            early = int((bg.at_ns < 30e9).sum())
            flood.append({"config": name, "notices_on_background": len(bg), "in_first_30_s": early,
                          "after_30_s": len(bg) - early, "per_stream_in_first_30_s": early / len(arm.notices),
                          "per_stream_after_30_s": (len(bg) - early) / len(arm.notices)})
    for rows, name in ((fam, "families"), (off, "offsets"), (life, "lifetimes"), (flood, "flood-floor")):
        df = pd.DataFrame(rows)
        df.to_csv(C.OUT / f"b1-{name}.csv", index=False)
        print(f"== {name}")
        print(df.to_string(index=False, float_format=lambda x: f"{x:.2f}"))


if __name__ == "__main__":
    main()
