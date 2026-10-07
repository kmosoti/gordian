"""B4, after the held-out run: do leaks and decoys separate publicly in the readings after the ramp's notice?

Usage: b4_separation.py DUMPS   (DUMPS: a directory of `seed-<n>.jsonl`, the `dump` example's public stream for
                                 each held-out seed, 20000-20199; writes experiments/exploration/
                                 b4-separation.csv and b4-separation-roc.csv)

Post hoc, on the held-out streams, and chooses nothing: the follow-up rule's parameters were fixed on the tuning
streams before the held-out run. It reads the evaluator's labels (the class of each notice's incident) beside the
public readings, which is an analysis of what the readings carry, not a policy input. The notices examined are the
ones the ramp adds: counter-anchored notices of the comparator noticer's arm whose anchor is not the anchor of a
notice of the same noticer without the ramp (`split_over_re2`), per seed. For each, from the public counter
readings at the anomaly's key: `completing` is the last reading at or before the notice, `follow` the readings
after it within `HORIZON_S` seconds, and the statistics are the number of follow readings, the last one minus
the completing one, the lowest one minus the completing one, and the largest fall below the peak since the
completing reading. `auc` is the probability that a leak's value of the statistic exceeds a decoy's (ties half).
`roc` lists, for the statistic `last - completing` (a notice with no follow reading counted at minus infinity,
as the follow-up rule withdraws it), the share of each class that a cutoff would keep.
"""

import json
import pathlib
import sys

import numpy as np
import pandas as pd

import b4_common as C

HORIZON_S = 6.0
NS = 1_000_000_000
CLASSES = ("leak", "decoy", "plain", "hard", "background")
STATS = ("n_follow", "last_minus_completing", "lowest_minus_completing", "largest_fall")
CMP_ARM = "alw16_" + C.COMPARATOR
BASE_ARM = "alw16_split_over_re2"


def counter_readings(path):
    """{key: [(id, at, value)]} of the public counter readings of one stream's dump."""
    out = {}
    with open(path) as fh:
        for line in fh:
            o = json.loads(line)
            c = o["obs"].get("Counter") if isinstance(o["obs"], dict) else None
            if c is not None:
                out.setdefault((c["service"], c["name"]), []).append((o["id"], o["at"], c["value"]))
    for seq in out.values():
        seq.sort(key=lambda r: (r[1], r[0]))
    return out


def statistics(seq, anchor_id, notice_at):
    """The four statistics of one notice from the key's readings `seq`, or None when the anchor is not a
    reading of this key."""
    if not any(r[0] == anchor_id for r in seq):
        return None
    upto = [r for r in seq if r[0] >= anchor_id and r[1] <= notice_at]
    if not upto:
        return None
    level0 = upto[-1][2]
    follow = [r for r in seq if notice_at < r[1] <= notice_at + HORIZON_S * NS]
    vals = [r[2] for r in follow]
    peak, fall = level0, 0
    for v in vals:
        peak = max(peak, v)
        fall = max(fall, peak - v)
    return {"n_follow": len(vals),
            "last_minus_completing": (vals[-1] - level0) if vals else -np.inf,
            "lowest_minus_completing": (min(vals) - level0) if vals else -np.inf,
            "largest_fall": fall if vals else np.inf,
            "completing": level0}


def class_of(row):
    if pd.isna(row["tier"]):
        return "background"
    return "leak" if row["family"] == "slow_leak" else row["tier"]


def ramp_notices(run_dir, dumps):
    """One row per notice the ramp adds in the comparator arm, with its class and statistics."""
    cmp_dir, base_dir = run_dir / CMP_ARM, run_dir / BASE_ARM
    ev = pd.read_csv(cmp_dir / "notice_events.csv")
    ev = ev[ev["event"] == "notice"]
    base = pd.read_csv(base_dir / "notice_events.csv")
    base = base[base["event"] == "notice"]
    base_keys = set(zip(base["seed"], base["anchor"]))
    inc = pd.read_csv(cmp_dir / "notice_incidents.csv")[["seed", "incident", "tier", "family"]]
    ev = ev.merge(inc, on=["seed", "incident"], how="left")
    rows = []
    for seed, g in ev.groupby("seed"):
        readings = counter_readings(pathlib.Path(dumps) / f"seed-{seed}.jsonl")
        by_id = {r[0]: k for k, seq in readings.items() for r in seq}
        for _, n in g.iterrows():
            a = int(n["anchor"])
            if (seed, a) in base_keys or a not in by_id:
                continue  # also a notice of the noticer without the ramp, or not a counter reading
            st = statistics(readings[by_id[a]], a, int(n["at_ns"]))
            if st is None:
                continue
            rows.append({"seed": seed, "anomaly": int(n["anomaly"]), "class": class_of(n), **st})
    return pd.DataFrame(rows)


def auc(x, y):
    """P(x > y) + P(x = y) / 2 over all pairs; infinities compare as numbers."""
    x, y = np.asarray(x, float), np.asarray(y, float)
    if len(x) == 0 or len(y) == 0:
        return float("nan")
    gt = (x[:, None] > y[None, :]).mean()
    eq = (x[:, None] == y[None, :]).mean()
    return float(gt + eq / 2)


def summary(df):
    rows = []
    for cls in CLASSES:
        sub = df[df["class"] == cls]
        row = {"class": cls, "notices": len(sub)}
        if len(sub):
            row["share_no_follow_reading"] = float((sub["n_follow"] == 0).mean())
            fin = sub[sub["n_follow"] > 0]
            row["median_n_follow"] = float(sub["n_follow"].median())
            row["median_last_minus_completing"] = float(fin["last_minus_completing"].median()) if len(fin) else np.nan
            row["share_last_at_least_completing"] = float((sub["last_minus_completing"] >= 0).mean())
            row["median_largest_fall"] = float(fin["largest_fall"].median()) if len(fin) else np.nan
        rows.append(row)
    pairs = []
    for other in ("decoy", "plain", "hard", "background"):
        for stat in STATS:
            pairs.append({"class": f"auc leak vs {other}", "stat": stat,
                          "auc": auc(df[df["class"] == "leak"][stat], df[df["class"] == other][stat])})
    return pd.DataFrame(rows), pd.DataFrame(pairs)


def roc(df):
    rows = []
    for cut in (-np.inf, -10, -5, 0, 3, 6, 10, 15, 20):
        row = {"keep_if_last_minus_completing_at_least": cut}
        for cls in CLASSES:
            sub = df[df["class"] == cls]
            row[f"{cls}_kept"] = float((sub["last_minus_completing"] >= cut).mean()) if len(sub) else np.nan
            row[f"{cls}_n"] = len(sub)
        rows.append(row)
    return pd.DataFrame(rows)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    df = ramp_notices(C.RUNS / C.run_id("heldout"), sys.argv[1])
    summ, pairs = summary(df)
    out = pd.concat([summ, pairs], ignore_index=True)
    out.to_csv(C.OUT / "b4-separation.csv", index=False)
    r = roc(df)
    r.to_csv(C.OUT / "b4-separation-roc.csv", index=False)
    with pd.option_context("display.width", 250, "display.max_columns", 30):
        print(summ.round(3).to_string(index=False))
        print(pairs.round(3).to_string(index=False))
        print(r.round(3).to_string(index=False))


if __name__ == "__main__":
    main()
