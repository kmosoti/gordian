"""B2 diagnostics beside the table: what the headline numbers are made of.

Usage: b2_diagnostics.py   (after the held-out run; writes experiments/exploration/
                            b2-transitions.csv, b2-residual.csv, b2-offsets.csv, b2-lifetimes.csv,
                            b2-hold-effect.csv, b2-gaming.csv, b2-vs-ceiling.csv, b2-paired-pure.csv and, when the
                            supplementary run exists, b2-flood-hold.csv)

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
* vs ceiling: each table noticer's quality, both readings, minus R10's injected-notice ceiling on the
  same streams (paired over the same resamples).
* flood hold: the flood-like noticers' quality with the retirement and with the hold at 16 s.
* paired pure: the re-anchor's own effect, the stage 1 choice at the rung's threshold against the rung.
"""

import numpy as np
import pandas as pd

import b2_common as C
import b2_stats as B
import r6_stats as S
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


def _arrays(arm):
    t = S.per_stream(arm)
    return t["quality_num"].to_numpy(float), t["quality_den"].to_numpy(float)


def paired_pooled(a, b, seed=C.BOOT_SEED, resamples=C.N_RESAMPLES, chunk=500):
    """(point, lower, higher) of the pooled ratio of `a` = (num, den) minus that of `b`, over the same
    multinomial resamples of whole streams the table's paired differences use."""
    (na, da), (nb, db) = a, b
    n = len(na)
    assert len(nb) == n
    rng = np.random.default_rng(seed)
    out, done = [], 0
    while done < resamples:
        k = min(chunk, resamples - done)
        w = rng.multinomial(n, np.full(n, 1.0 / n), size=k).astype(float)
        out.append((w @ na) / (w @ da) - (w @ nb) / (w @ db))
        done += k
    x = np.concatenate(out)
    lo, hi = S.interval(x)
    return float(na.sum() / da.sum() - nb.sum() / db.sum()), lo, hi


def vs_ceiling(run, sel):
    """Quality of each table noticer, both readings, against R10's injected-notice ceiling (the
    selection oracle told which observation begins each hard incident, the rung's context, R5's 16 s
    delay) on the same 200 streams: the gap that noticing leaves, per row."""
    r10 = load_stream_run(C.C1.R10_RUNS / f"r10-heldout-{C.setting_id(*C.PRIMARY)}")
    ceiling = _arrays(r10.arms["notice_rung_privileged"])
    same = _arrays(r10.arms["sel_rung_privileged"])
    mine_rung = _arrays(run.arms[C.arm_name("rung_z3")])
    # the rung's row of this run is R10's own `sel_rung_privileged` (B1 showed it; checked again here)
    assert (same[0] == mine_rung[0]).all() and (same[1] == mine_rung[1]).all()
    rows = [{"arm": "R10 injected-notice ceiling (notice_rung_privileged)", "reading": "ceiling",
             "quality": float(ceiling[0].sum() / ceiling[1].sum())}]
    for name in C.TABLE:
        for reading, arm in (("fixed 16 s, retirement", C.arm_name(name)), ("tuned delay, hold", C.hold_arm_name(name))):
            mine = _arrays(run.arms[arm])
            d, lo, hi = paired_pooled(mine, ceiling)
            rows.append({"arm": name, "reading": reading, "quality": float(mine[0].sum() / mine[1].sum()),
                         "minus_ceiling": d, "lower": lo, "higher": hi})
    return pd.DataFrame(rows)


def flood_hold(run):
    """The flood-like noticers' quality with the retirement and with the hold at R5's 16 s (the
    supplementary run), beside their calls and refusals."""
    supp = load_stream_run(C.RUNS / C.run_id("supp"))
    rows = []
    for arm in supp.arms:
        name = arm[len("selhold_"):-len("_d16_privileged")]
        fixed = run.arms[C.arm_name(name)]
        held = supp.arms[arm]
        qf, qh = _arrays(fixed), _arrays(held)
        d, lo, hi = paired_pooled(qh, qf)
        rows.append({
            "config": name, "quality_fixed": float(qf[0].sum() / qf[1].sum()),
            "quality_held_16s": float(qh[0].sum() / qh[1].sum()), "held_minus_fixed": d, "lower": lo,
            "higher": hi,
            "calls_per_stream_fixed": float(fixed.results["reasoner_calls"].mean()),
            "calls_per_stream_held": float(held.results["reasoner_calls"].mean()),
            "escalations_refused_fixed": int(fixed.results["escalations_refused"].sum()),
            "escalations_refused_held": int(held.results["escalations_refused"].sum()),
            "cost_s_per_stream_fixed": float(fixed.results["total_cost_ns"].mean()) / C.NS,
            "cost_s_per_stream_held": float(held.results["total_cost_ns"].mean()) / C.NS,
        })
    return pd.DataFrame(rows)


PURE_MEASURES = [
    "hard_noticed_share", "hard_anchor_correct_share", "hard_site_correct_share",
    "hard_anchor_site_correct_share", "leak_noticed_share", "notices_on_background_per_stream",
    "notice_precision", "strict_precision", "quality",
]


def paired_pure(run, sel):
    """The re-anchor's own effect: the stage 1 choice at the rung's default threshold against the
    rung at that threshold, over the same resamples as the table's paired differences."""
    s1 = sel["stage1"]["chosen"]
    new = C.arm_name(C.reanchor_name(s1["isolation"], s1["gap_ms"], s1["burst"]))
    old = C.arm_name("rung_z3")
    m = B.Measures(run, arms=[new, old])
    rows = []
    for measure in PURE_MEASURES:
        d, lo, hi = m.paired_difference(measure, new, old, C.BOOT_SEED, C.N_RESAMPLES)
        rows.append({"new": new, "old": old, "measure": measure, "difference": d, "lower": lo, "higher": hi})
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
    parts = [("transitions", transitions(run, sel)), ("residual", residual(run)),
             ("offsets", offsets(run)), ("lifetimes", lifetimes(run, sel)),
             ("hold-effect", hold_effect(run)), ("gaming", gaming()), ("vs-ceiling", vs_ceiling(run, sel)),
             ("paired-pure", paired_pure(run, sel))]
    if (C.RUNS / C.run_id("supp")).exists():
        parts.append(("flood-hold", flood_hold(run)))
    for name, df in parts:
        df.to_csv(C.OUT / f"b2-{name}.csv", index=False)
        print(f"== {name}")
        print(df.to_string(index=False, float_format=lambda x: f"{x:.3f}"))


if __name__ == "__main__":
    main()
