"""B3 diagnostics beside the table: what the headline numbers are made of.

Usage: b3_diagnostics.py   (after the held-out run and b3_table.py; writes experiments/exploration/
                            b3-transitions.csv, b3-never-noticed.csv, b3-leak.csv, b3-leak-offsets.csv,
                            b3-added-notices.csv, b3-decoys.csv, b3-paired-base.csv, b3-hold-effect.csv,
                            b3-gaming.csv)

Exploration (nothing here tests a hypothesis). All of it reads the held-out run's notice files; it
chooses nothing.

* transitions: per composed row against its own base (the rung at z = 3 for the rows over it, the later
  re-anchor for the rows over it, the re-anchor at z = 3 for the last row), over the same incidents: how many
  hard non-leak incidents are gained, lost, kept or never anchor-correct, and the same for noticed; and for
  the slow leak, by the same four counts. Per family.
* never-noticed: the hard non-leak incidents the re-anchor row never notices, one line each, with what every
  row of the table did with it (noticed, anchor-correct, offset of the first notice).
* leak: per slow-leak incident, for the rows that read values and the rows that do not: noticed, anchor
  correct, the first notice's latency from the incident's first observation (N4), and the anchor's offset
  of the notice nearest the first observation.
* leak offsets: for every row, the offsets of the anchors of the notices about slow leaks, in bands.
* added notices: what each composed row's notices are that its base's are not (by seed and anchor), and what
  the base's are that it does not have, by what the anchor belongs to (slow leak, other hard, plain, decoy,
  background): the ramp noticer's and the splitting noticer's own notices, and the base's that they
  absorbed. Rows with the hold are not read (the hold can change later noticing, `b3-hold-effect.csv`).
* decoys: per row, the decoy incidents noticed and the notices about them, beside the plain ones.
* paired base: the measures of the table, each composed row minus its own base, over the table's own
  resamples (the effect of the piece, not of the base).
* hold effect: whether the notice record of the run with the hold differs from the run without it, per row.
* gaming: precision, strict precision and notices per incident beside anchor-correct and leak noticed for
  every configuration played, one table, so that a flood is visible in one row.
"""

import numpy as np
import pandas as pd

import b2_stats as B
import b3_common as C
import b3_manifests as M
from gordian_analysis.load import load_stream_run

PAIRS = [  # (composed row, its base row)
    ("ramp_over_r3", "rung_z3"), ("split_over_r3", "rung_z3"), ("ramp_split_over_r3", "rung_z3"),
    ("ramp_over_re2", "reanchor"), ("split_over_re2", "reanchor"), ("ramp_split_over_re2", "reanchor"),
    ("ramp_split_over_re3", "reanchor_z3"),
]
BANDS = [(0.0, 0.0), (0.0, 1e9), (1e9, 2e9), (2e9, 5e9), (5e9, 10e9), (10e9, float("inf"))]
PAIRED_MEASURES = [
    "hard_noticed_share", "hard_anchor_correct_share", "leak_noticed_share", "leak_anchor_correct_share",
    "notices_on_background_per_stream", "notice_precision", "strict_precision", "notices_per_incident",
    "quality", "leak_quality", "cost_s_per_stream", "calls_per_stream", "notices_per_stream",
]


def hard_nonleak(ni):
    return ni[(ni.tier == "hard") & (ni.family != "slow_leak")]


def leaks(ni):
    return ni[ni.family == "slow_leak"]


def keyed(df):
    return df.set_index(["seed", "incident"])


def transitions(run):
    rows = []
    for new, old in PAIRS:
        an, ao = run.arms[C.arm_name(new)].notice_incidents, run.arms[C.arm_name(old)].notice_incidents
        for group, pick in (("hard non-leak", hard_nonleak), ("slow leak", leaks)):
            a, b = keyed(pick(ao)), keyed(pick(an))
            assert a.index.equals(b.index)
            fams = [f for f in C.FAMILIES if (f == "slow_leak") == (group == "slow leak")] + ["all"]
            for fam in fams:
                ia = a if fam == "all" else a[a.family == fam]
                ib = b.loc[ia.index]
                row = {"new": new, "old": old, "group": group, "family": fam, "incidents": len(ia)}
                for col in ("anchor_correct", "noticed"):
                    ca, cb = ia[col].astype(bool), ib[col].astype(bool)
                    row[f"{col}_kept"] = int((ca & cb).sum())
                    row[f"{col}_gained"] = int((~ca & cb).sum())
                    row[f"{col}_lost"] = int((ca & ~cb).sum())
                    row[f"{col}_never"] = int((~ca & ~cb).sum())
                rows.append(row)
    return pd.DataFrame(rows)


def nearest_offset(run, arm_stem, ids):
    """For each (seed, incident) in `ids`: the anchor offset of the notice about it nearest the
    incident's first observation, `NaN` when it has none."""
    ev = run.arms[C.arm_name(arm_stem)].notice_events
    n = ev[(ev.event == "notice") & ev.incident.notna()].copy()
    n["incident"] = n["incident"].astype(int)
    n["abs"] = n.anchor_offset_ns.abs()
    best = n.sort_values("abs").groupby(["seed", "incident"]).first()["anchor_offset_ns"]
    return best.reindex(ids)


def never_noticed(run):
    stems = [s for s, _ in M.table_rows(C.load_selected())]
    base = keyed(hard_nonleak(run.arms[C.arm_name(C.COMPARATOR)].notice_incidents))
    lost = base[~base.noticed.astype(bool)]
    out = lost[["family", "first_observation_at_ns"]].copy()
    out["first_observation_s"] = out.pop("first_observation_at_ns") / 1e9
    for stem in stems:
        ni = keyed(hard_nonleak(run.arms[C.arm_name(stem)].notice_incidents)).loc[out.index]
        out[f"{stem}__noticed"] = ni.noticed.astype(bool).to_numpy()
        out[f"{stem}__anchor_correct"] = ni.anchor_correct.astype(bool).to_numpy()
        out[f"{stem}__latency_s"] = (ni.notice_latency_ns / 1e9).to_numpy()
    return out.reset_index()


def leak_rows(run):
    stems = [s for s, _ in M.table_rows(C.load_selected())]
    base = keyed(leaks(run.arms[C.arm_name("rung_z3")].notice_incidents))
    out = base[["family", "first_observation_at_ns"]].copy()
    out["first_observation_s"] = out.pop("first_observation_at_ns") / 1e9
    for stem in stems:
        ni = keyed(leaks(run.arms[C.arm_name(stem)].notice_incidents)).loc[out.index]
        out[f"{stem}__noticed"] = ni.noticed.astype(bool).to_numpy()
        out[f"{stem}__anchor_correct"] = ni.anchor_correct.astype(bool).to_numpy()
        out[f"{stem}__latency_s"] = (ni.notice_latency_ns / 1e9).to_numpy()
        out[f"{stem}__offset_s"] = (nearest_offset(run, stem, out.index) / 1e9).to_numpy()
    return out.reset_index()


def leak_offsets(run):
    rows = []
    for stem, _ in M.table_rows(C.load_selected()):
        arm = run.arms[C.arm_name(stem)]
        ev, ni = arm.notice_events, arm.notice_incidents
        n = ev[(ev.event == "notice") & ev.incident.notna()].copy()
        n["incident"] = n["incident"].astype(int)
        lk = leaks(ni)[["seed", "incident"]]
        n = n.merge(lk, on=["seed", "incident"])
        off = n.anchor_offset_ns
        row = {"config": stem, "notices_on_leaks": len(n), "anchor_correct": int(n.anchor_correct.astype(bool).sum())}
        row["offset_before"] = int((off < 0).sum())
        row["offset_exactly_0"] = int((off == 0).sum())
        for lo, hi in BANDS[1:]:
            hi_s = "inf" if np.isinf(hi) else f"{hi / 1e9:g}"
            row[f"offset_after_{lo / 1e9:g}_to_{hi_s}_s"] = int(((off > lo) & (off <= hi)).sum())
        rows.append(row)
    return pd.DataFrame(rows)


def kind_of(ev, ni):
    """The notices of an arm, one line each, with what the anchor belongs to."""
    n = ev[ev.event == "notice"].copy()
    key = ni.set_index(["seed", "incident"])[["tier", "family"]]
    inc = n.incident.fillna(-1).astype(int)
    n["incident_i"] = inc
    j = n.join(key, on=["seed", "incident_i"])
    kind = np.where(n.incident.isna(), "background",
                    np.where(j.family == "slow_leak", "slow leak",
                             np.where(j.tier == "hard", "hard non-leak",
                                      np.where(j.tier == "plain", "plain", "decoy"))))
    n["kind"] = kind
    return n


def added_notices(run):
    rows = []
    for new, old in PAIRS:
        an, ao = run.arms[C.arm_name(new)], run.arms[C.arm_name(old)]
        nn, no = kind_of(an.notice_events, an.notice_incidents), kind_of(ao.notice_events, ao.notice_incidents)
        kn = set(zip(nn.seed, nn.anchor))
        ko = set(zip(no.seed, no.anchor))
        base_inc = set(zip(no.seed, no.incident.fillna(-1).astype(int)))
        added = nn[[(s, a) not in ko for s, a in zip(nn.seed, nn.anchor)]]
        gone = no[[(s, a) not in kn for s, a in zip(no.seed, no.anchor)]]
        for side, df in (("added (in the row, not in its base)", added), ("absorbed (in the base, not in the row)", gone)):
            row = {"new": new, "old": old, "side": side, "notices": len(df),
                   "per_stream": len(df) / run.arms[C.arm_name(new)].results["seed"].nunique()}
            for k in ("slow leak", "hard non-leak", "plain", "decoy", "background"):
                row[k] = int((df.kind == k).sum())
            row["anchor_correct"] = int(df.anchor_correct.astype(bool).sum())
            off = df.anchor_offset_ns
            row["anchored_on_first_observation"] = int((off == 0).sum())
            row["anchored_more_than_5s_after"] = int((off > 5e9).sum())
            row["median_offset_s_of_incident_anchored"] = float(off.dropna().median() / 1e9) if off.notna().any() else np.nan
            row["on_an_incident_the_base_also_noticed"] = int(
                sum((s_, int(i)) in base_inc for s_, i in zip(df.seed, df.incident) if pd.notna(i)))
            rows.append(row)
    return pd.DataFrame(rows)


def decoys(run):
    """Per row: the decoy incidents (look-alikes with no diagnosis) noticed, and the notices about them, beside the
    plain incidents', so that a noticer that cannot tell a leak from its look-alike shows."""
    rows = []
    for stem, _ in M.table_rows(C.load_selected()):
        arm = run.arms[C.arm_name(stem)]
        ni = arm.notice_incidents
        d, p = ni[ni.tier == "decoy"], ni[ni.tier == "plain"]
        n = len(arm.results)
        rows.append({
            "config": stem, "decoy_incidents": len(d), "decoys_noticed": int(d.noticed.astype(bool).sum()),
            "decoys_noticed_share": float(d.noticed.astype(bool).mean()),
            "notices_on_decoys_per_stream": float(d.notices.sum() / n),
            "decoy_notices_per_slow_leak_notice": float(d.notices.sum() / max(1, leaks(ni).notices.sum())),
            "plain_incidents": len(p), "plain_noticed": int(p.noticed.astype(bool).sum()),
            "notices_on_plain_per_stream": float(p.notices.sum() / n),
        })
    return pd.DataFrame(rows)


def paired_base(run):
    stems = [s for s, _ in PAIRS]
    arms = [C.arm_name(s) for s, _ in PAIRS] + [C.arm_name(b) for _, b in PAIRS]
    m = B.Measures(run, arms=list(dict.fromkeys(arms)))
    rows = []
    for new, old in PAIRS:
        for measure in PAIRED_MEASURES:
            d, lo, hi = m.paired_difference(measure, C.arm_name(new), C.arm_name(old), C.BOOT_SEED, C.N_RESAMPLES)
            rows.append({"new": new, "base": old, "measure": measure, "difference": d, "lower": lo, "higher": hi})
    # the interaction of the two pieces over the re-anchor (ramp+split minus ramp minus split plus base)
    for base, ramp, split, both in (("reanchor", "ramp_over_re2", "split_over_re2", "ramp_split_over_re2"),
                                    ("rung_z3", "ramp_over_r3", "split_over_r3", "ramp_split_over_r3")):
        pts = B.Measures(run, arms=[C.arm_name(s) for s in (base, ramp, split, both)]).points(PAIRED_MEASURES)
        for measure in PAIRED_MEASURES:
            v = pts[measure]
            rows.append({"new": both, "base": f"interaction over {base}", "measure": measure,
                         "difference": float(v[3] - v[1] - v[2] + v[0]), "lower": np.nan, "higher": np.nan})
    return pd.DataFrame(rows)


def hold_effect(run):
    rows = []
    for stem, _ in M.table_rows(C.load_selected()):
        a, b = run.arms[C.arm_name(stem)], run.arms[C.hold_arm_name(stem)]
        ka = set(zip(a.notice_events.seed, a.notice_events.anchor, a.notice_events.event))
        kb = set(zip(b.notice_events.seed, b.notice_events.anchor, b.notice_events.event))
        rows.append({
            "config": stem, "notices_fixed": int(a.notices.notices.sum()), "notices_held": int(b.notices.notices.sum()),
            "background_fixed": int(a.notices.notices_on_background.sum()),
            "background_held": int(b.notices.notices_on_background.sum()),
            "hard_anchor_correct_fixed": int(hard_nonleak(a.notice_incidents).anchor_correct.sum()),
            "hard_anchor_correct_held": int(hard_nonleak(b.notice_incidents).anchor_correct.sum()),
            "leak_noticed_fixed": int(leaks(a.notice_incidents).noticed.sum()),
            "leak_noticed_held": int(leaks(b.notice_incidents).noticed.sum()),
            "same_notice_and_retirement_records": bool(ka == kb),
        })
    return pd.DataFrame(rows)


def gaming():
    sens = pd.read_csv(C.OUT / "b3-noticers-sensitivity.csv")
    keep = ["config", "table_row", "hard_anchor_correct_share", "leak_noticed_share", "leak_anchor_correct_share",
            "notices_on_background_per_stream", "notices_per_incident", "notice_precision", "strict_precision",
            "quality", "cost_s_per_stream"]
    return sens[keep]


def main():
    run = load_stream_run(C.RUNS / C.run_id("heldout"))
    parts = [("transitions", transitions(run)), ("never-noticed", never_noticed(run)), ("leak", leak_rows(run)),
             ("leak-offsets", leak_offsets(run)), ("added-notices", added_notices(run)), ("decoys", decoys(run)),
             ("paired-base", paired_base(run)), ("hold-effect", hold_effect(run)), ("gaming", gaming())]
    for name, df in parts:
        df.to_csv(C.OUT / f"b3-{name}.csv", index=False)
        print(f"== {name}")
        print(df.to_string(index=False, float_format=lambda x: f"{x:.3f}"))


if __name__ == "__main__":
    main()
