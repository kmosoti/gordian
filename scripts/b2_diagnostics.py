"""B2 diagnostics beside the table: what the headline numbers are made of.

Usage: b2_diagnostics.py   (after the held-out run; writes experiments/exploration/
                            b2-transitions.csv, b2-residual.csv, b2-offsets.csv, b2-lifetimes.csv,
                            b2-hold-effect.csv, b2-gaming.csv)

Exploration (nothing here tests a hypothesis). All of it reads the held-out run's notice files.

* transitions: per hard non-leak incident, the rung's anchor-correct flag against each re-anchoring
  noticer's, at the same threshold (the re-anchor's own effect) and the table row against the nearest
  rung: how many incidents are gained, lost, kept, never anchor-correct; by family.
* residual: the hard non-leak incidents the `reanchor` row never anchors correctly: not noticed with a
  background-anchored notice within 1 s before the first observation (a stray still holding it), not
  noticed with none, noticed late; and anchor-correct but not site-correct, by family.
* offsets: for the `reanchor` row's notices about hard incidents that are not anchor-correct, the
  anchor's offset from the incident's first observation, in bands.
* lifetimes: for notices anchored on a hard incident, how many of the anomalies were retired before the
  fixed delay passed (so the selection oracle never asked), per table row: the confound the hold
  removes. And the same under the hold at the row's tuned delay.
* hold effect: whether the notice record of the run with the hold differs from the run without it,
  per row (notices, notices on background, anchor-correct).
* gaming: precision and notices per incident beside anchor-correct for every configuration played, in
  one table, so that a flood is visible in one row.
"""

import numpy as np
import pandas as pd

import b2_common as C
from gordian_analysis.load import load_stream_run

DELAY_NS = C.FIXED_DELAY_S * C.NS
BANDS = [(1e9, 2e9), (2e9, 5e9), (5e9, 10e9), (10e9, 30e9), (30e9, float("inf"))]


def hard_nonleak(ni):
    return ni[(ni.tier == "hard") & (ni.family != "slow_leak")]


def transitions(run, sel):
    pairs = [("reanchor_pure", C.reanchor_name(sel["stage1"]["chosen"]["isolation"],
                                                sel["stage1"]["chosen"]["gap_ms"],
                                                sel["stage1"]["chosen"]["burst"]), "rung_z3")]
    z = sel["stage2"]["chosen"]["z"]
    near = {3.0: "rung_z3", 2.0: "rung_z2", 1.0: "rung_z1"}.get(z)
    if near:
        pairs.append(("reanchor_row", "reanchor", near))
    rows = []
    for label, new, old in pairs:
        a = hard_nonleak(run.arms[C.arm_name(old)].notice_incidents).set_index(["seed", "incident"])
        b = hard_nonleak(run.arms[C.arm_name(new)].notice_incidents).set_index(["seed", "incident"])
        assert a.index.equals(b.index)
        for fam in [*C.FAMILIES, "all"]:
            ia = a if fam == "all" else a[a.family == fam]
            if fam == "slow_leak":
                continue
            ib = b.loc[ia.index]
            ca, cb = ia.anchor_correct.astype(bool), ib.anchor_correct.astype(bool)
            rows.append({
                "comparison": label, "new": new, "old": old, "family": fam, "incidents": len(ia),
                "kept_correct": int((ca & cb).sum()), "gained": int((~ca & cb).sum()),
                "lost": int((ca & ~cb).sum()), "never_correct": int((~ca & ~cb).sum()),
                "site_correct_old": int(ia.site_correct.astype(bool).sum()),
                "site_correct_new": int(ib.site_correct.astype(bool).sum()),
                "both_old": int(ia.anchor_site_correct.astype(bool).sum()),
                "both_new": int(ib.anchor_site_correct.astype(bool).sum()),
            })
    return pd.DataFrame(rows)


def residual(run):
    arm = run.arms[C.arm_name("reanchor")]
    ni, ev = arm.notice_incidents, arm.notice_events
    hard = hard_nonleak(ni)
    bg = ev[(ev.event == "notice") & ev.incident.isna()][["seed", "anchor_at_ns"]]
    out = []
    for fam in C.FAMILIES[:3]:
        g = hard[hard.family == fam]
        lost = g[~g.anchor_correct.astype(bool)]
        row = {"family": fam, "incidents": len(g), "anchor_correct": int(g.anchor_correct.sum()),
               "never_noticed": int((~lost.noticed.astype(bool)).sum()),
               "noticed_late": int(lost.noticed.astype(bool).sum()),
               "never_noticed_bg_notice_within_1s_before": 0, "never_noticed_bg_notice_1_to_5s": 0,
               "never_noticed_none_within_5s": 0,
               "anchor_correct_not_site_correct": int((g.anchor_correct.astype(bool) & ~g.site_correct.astype(bool)).sum())}
        for _, inc in lost[~lost.noticed.astype(bool)].iterrows():
            mine = bg[bg.seed == inc.seed].anchor_at_ns.astype("int64")
            gap = (int(inc.first_observation_at_ns) - mine)
            gap = gap[gap >= -int(1e9)].abs()
            near = gap.min() if len(gap) else np.inf
            key = ("never_noticed_bg_notice_within_1s_before" if near <= 1e9 else
                   "never_noticed_bg_notice_1_to_5s" if near <= 5e9 else "never_noticed_none_within_5s")
            row[key] += 1
        out.append(row)
    return pd.DataFrame(out)


def offsets(run):
    rows = []
    for name in C.TABLE:
        ev = run.arms[C.arm_name(name)].notice_events
        n = ev[(ev.event == "notice") & ev.incident.notna()]
        ni = run.arms[C.arm_name(name)].notice_incidents
        hard = ni[ni.tier == "hard"][["seed", "incident"]]
        n = n.merge(hard, on=["seed", "incident"])
        wrong = n[~n.anchor_correct.astype(bool)]
        row = {"config": name, "notices_on_hard": len(n), "not_anchor_correct": len(wrong)}
        for lo, hi in BANDS:
            hi_s = "inf" if np.isinf(hi) else f"{hi / 1e9:g}"
            row[f"offset_{lo / 1e9:g}_to_{hi_s}_s"] = int(((wrong.anchor_offset_ns >= lo) & (wrong.anchor_offset_ns < hi)).sum())
        rows.append(row)
    return pd.DataFrame(rows)


def lifetimes(run, sel):
    rows = []
    for name in C.TABLE:
        for reading, arm_name, delay_ns in (
            ("fixed", C.arm_name(name), DELAY_NS),
            ("held", C.hold_arm_name(name), sel["stage3"][name]["delay_s"] * C.NS),
        ):
            arm = run.arms[arm_name]
            ni, ev = arm.notice_incidents, arm.notice_events
            notices = ev[ev.event == "notice"]
            hard = ni[ni.tier == "hard"][["seed", "incident"]]
            hn = notices.merge(hard, on=["seed", "incident"], how="inner")
            ret = ev[ev.event == "retire"][["seed", "anomaly", "at_ns"]].rename(columns={"at_ns": "retired_at_ns"})
            hn = hn.merge(ret, on=["seed", "anomaly"], how="left")
            lifetime = (hn.retired_at_ns - hn.at_ns).astype("float")
            alive = lifetime.isna() | (lifetime >= delay_ns)
            rows.append({"config": name, "reading": reading, "delay_s": delay_ns / C.NS,
                         "notices_on_hard": len(hn), "live_when_the_delay_passes": int(alive.sum()),
                         "retired_before_the_delay": int((~alive).sum())})
    return pd.DataFrame(rows)


def hold_effect(run):
    rows = []
    for name in C.TABLE:
        a, b = run.arms[C.arm_name(name)], run.arms[C.hold_arm_name(name)]
        rows.append({
            "config": name,
            "notices_fixed": int(a.notices.notices.sum()), "notices_held": int(b.notices.notices.sum()),
            "background_fixed": int(a.notices.notices_on_background.sum()),
            "background_held": int(b.notices.notices_on_background.sum()),
            "hard_anchor_correct_fixed": int(hard_nonleak(a.notice_incidents).anchor_correct.sum()),
            "hard_anchor_correct_held": int(hard_nonleak(b.notice_incidents).anchor_correct.sum()),
            "hard_noticed_fixed": int(hard_nonleak(a.notice_incidents).noticed.sum()),
            "hard_noticed_held": int(hard_nonleak(b.notice_incidents).noticed.sum()),
        })
    return pd.DataFrame(rows)


def gaming():
    sens = pd.read_csv(C.OUT / "b2-noticers-sensitivity.csv")
    keep = ["config", "hard_anchor_correct_share", "hard_site_correct_share", "hard_anchor_site_correct_share",
            "leak_noticed_share", "notices_on_background_per_stream", "notices_per_incident",
            "notice_precision", "strict_precision", "quality"]
    return sens[keep]


def main():
    sel = C.load_selected()
    run = load_stream_run(C.RUNS / C.run_id("heldout"))
    for name, df in (("transitions", transitions(run, sel)), ("residual", residual(run)),
                     ("offsets", offsets(run)), ("lifetimes", lifetimes(run, sel)),
                     ("hold-effect", hold_effect(run)), ("gaming", gaming())):
        df.to_csv(C.OUT / f"b2-{name}.csv", index=False)
        print(f"== {name}")
        print(df.to_string(index=False, float_format=lambda x: f"{x:.3f}"))


if __name__ == "__main__":
    main()
