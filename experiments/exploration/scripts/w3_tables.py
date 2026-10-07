"""W3: render the report's tables from the committed `w3-*.csv`.

Usage: w3_tables.py [NAME]   (print one table, or all when no name)

Each function returns a Markdown table; `w3_assemble.py` puts them into the report template. Every
table names the seed range and side of its rows. Nothing is computed here that is not in a CSV.
"""

from __future__ import annotations

import sys

import pandas as pd

import w2_common as C

OUT = C.OUT
RANGES = ["a-tune", "a-heldout", "c-tune", "c-heldout"]
LABEL = {"a-tune": "A 10000-10099", "a-heldout": "A 40000-40199", "c-tune": "C 60000-60099", "c-heldout": "C 70000-70199"}


def csv(name: str) -> pd.DataFrame:
    return pd.read_csv(OUT / f"w3-{name}.csv", keep_default_na=False)


def md(rows: list[list], header: list[str]) -> str:
    out = ["| " + " | ".join(header) + " |", "|" + "|".join("---" for _ in header) + "|"]
    for r in rows:
        out.append("| " + " | ".join(str(c) for c in r) + " |")
    return "\n".join(out)


def num(x, nd=3):
    if x == "" or pd.isna(x):
        return ""
    return f"{float(x):.{nd}f}"


def ci(p, lo, hi, nd=3):
    if p == "" or pd.isna(p):
        return ""
    return f"{num(p, nd)} [{num(lo, nd)}, {num(hi, nd)}]"


# ---- item 1 ---------------------------------------------------------------------------------


def world_compare():
    d = csv("world-compare")
    measures = list(dict.fromkeys(d["measure"]))
    rows = []
    for m in measures:
        r = [m]
        for rng in RANGES:
            x = d[(d["range"] == rng) & (d["measure"] == m)].iloc[0]
            nd = 2 if float(x["value"]) >= 10 else 3
            r.append(f"{num(x['value'], nd)} [{num(x['lo90'], nd)}, {num(x['hi90'], nd)}]")
        rows.append(r)
    return md(rows, ["measure (hidden; value [90% cluster interval])"] + [LABEL[r] for r in RANGES])


def family_shares():
    d = csv("recurrence-summary")
    rows = []
    for rng in RANGES:
        h = d[(d["range"] == rng) & (d["group"] == "tier=hard")].iloc[0]
        r = [LABEL[rng], h["incidents"]]
        for fam in ("compound", "cascade", "split_brain", "slow_leak"):
            x = d[(d["range"] == rng) & (d["group"] == f"hard/{fam}")].iloc[0]
            r.append(f"{x['incidents']} ({int(x['incidents']) / int(h['incidents']):.3f})")
        rows.append(r)
    return md(rows, ["seeds (hidden)", "hard incidents", "compound (share)", "cascade", "split brain", "slow leak"])


def rec_summary():
    d = csv("recurrence-summary")
    rows = []
    for rng in RANGES:
        for g in ("all", "tier=plain", "tier=hard", "tier=decoy", "hard/compound", "hard/cascade", "hard/split_brain", "hard/slow_leak"):
            x = d[(d["range"] == rng) & (d["group"] == g)]
            if x.empty:
                continue
            x = x.iloc[0]
            rows.append([x["seeds"], g, x["incidents"], num(x["incidents_per_stream"], 2), x["recurrences"],
                         ci(x["recurrences_per_stream"], x["rec_per_stream_lo90"], x["rec_per_stream_hi90"]),
                         ci(x["recurrence_share"], x["share_lo90"], x["share_hi90"])])
    return md(rows, ["seeds (hidden)", "group", "incidents", "per stream", "recurrences", "recurrences per stream [90%]", "share of incidents [90%]"])


def elsewhere():
    d = csv("family-elsewhere")
    rows = []
    for rng in RANGES:
        for key in ("fam_mode", "strict"):
            x = d[(d["range"] == rng) & (d["key"] == key) & (d["group"] == "all hard")].iloc[0]
            rows.append([x["seeds"], "family and mode" if key == "fam_mode" else "strict (+ kinds)", x["hard_incidents"], x["recurrences"],
                         x["non_recurrence_hard"], x["elsewhere_non_recurrence"],
                         ci(x["elsewhere_per_stream"], x["elsewhere_lo90"], x["elsewhere_hi90"]),
                         ci(x["elsewhere_share_of_non_recurrence"], x["share_lo90"], x["share_hi90"]),
                         ci(x["family_reach_share_of_hard"], x["reach_lo90"], x["reach_hi90"])])
    return md(rows, ["seeds (hidden)", "key", "hard incidents", "recurrences", "non-recurrence hard", "same family at another site", "per stream [90%]",
                     "share of non-recurrence [90%]", "family-keyed reach by experience, share of hard [90%]"])


def regime():
    d = csv("regime-hard-after")
    rows = []
    for rng in RANGES:
        x = d[d["range"] == rng]
        g = lambda s: x[x["measure"].str.startswith(s)]["value"].iloc[0]  # noqa: E731
        rows.append([x["seeds"].iloc[0], g("share of hard incidents with onset at or after the first change"),
                     g("  90% interval (first change)"), g("share of hard incidents with onset at or after the last change"),
                     g("streams with at least one hard incident affected by signature_shift"),
                     g("streams with at least one hard incident affected by edge_add")])
    return md(rows, ["seeds (hidden)", "hard after the first change", "[90%]", "hard after the last change", "streams with a hard incident altered by the shift", "by the added edge"])


def regime_effects():
    d = csv("regime-effects")
    rows = []
    for rng in RANGES:
        for ch in ("signature_shift", "edge_add"):
            x = d[(d["range"] == rng) & (d["change"] == ch) & (d["group"] == "tier=hard")].iloc[0]
            rows.append([x["seeds"], ch, x["incidents_after_per_stream"], x["affected"], ci(x["affected_per_stream"], x["aff_lo90"], x["aff_hi90"]),
                         ci(x["affected_share_of_after"], x["share_lo90"], x["share_hi90"])])
    return md(rows, ["seeds (hidden)", "change", "hard incidents after it, per stream", "hard incidents altered", "per stream [90%]", "share of those after it [90%]"])


def stale():
    d = csv("stale")
    rows = []
    for rng in RANGES:
        x = d[(d["range"] == rng) & (d["group"] == "tier=hard")].iloc[0]
        rows.append([x["seeds"], x["recurrences"], x["straddles"], x["stale_any"], ci(x["stale_any_share"], x["stale_any_lo90"], x["stale_any_hi90"])])
    return md(rows, ["seeds (hidden)", "hard recurrences", "whose template began before the first change they straddle", "stale by construction", "share [90%]"])


def ceiling():
    d = csv("ceiling")
    rows = []
    for rng in RANGES:
        sub = d[(d["range"] == rng) & ~d["set"].str.contains("streams")]
        for r in sub.itertuples():
            label = r.set.split("  ")[0]
            rows.append([r.seeds, label, r.incidents, num(r.incidents_per_stream, 3), r.streams_with_any, r.calls,
                         f"{int(r.modelled_ns) / 1e9:.1f}", ci(r.share_of_total_bill, r.bill_lo90, r.bill_hi90)])
    return md(rows, ["seeds (run, hidden)", "set", "hard incidents", "per stream", "streams with at least one", "calls", "modelled s", "share of the total bill [90%]"])


def ceiling_cross():
    d = csv("ceiling")
    rows = []
    for rng in RANGES:
        sub = d[(d["range"] == rng) & d["set"].str.contains("streams")]
        for r in sub.itertuples():
            rows.append([r.seeds, r.set.split("  ")[0] + (" (" + r.set.split("  ")[1].split("[")[0].strip() + ")" if "  " in r.set else ""), r.incidents,
                         ci(r.share_of_total_bill, r.bill_lo90, r.bill_hi90)])
    return md(rows, ["seeds (run, hidden)", "set [streams 11 onward]", "hard incidents", "share of the total bill [90%]"])


def bill():
    d = csv("ceiling-bill")
    rows = [[r.seeds, r.streams, f"{int(r.total_cost_ns) / 1e9:.1f}", f"{float(r.reasoner_share_of_bill):.5f}", r.reasoner_calls, r.calls_on_hard,
             r.calls_on_other_incidents, r.calls_on_background, r.hard_incidents, r.hard_noticed, r.hard_asked, r.hard_declared_correctly,
             r.hard_declared_correctly_by_deadline] for r in d.itertuples()]
    return md(rows, ["seeds (run)", "streams", "total bill, modelled s", "reasoner share", "calls", "on hard", "on plain or decoy", "on background",
                     "hard incidents", "noticed", "asked", "declared correctly", "correct by the deadline"])


def deadlines():
    d = csv("ceiling-deadlines")
    rows = []
    for r in d.itertuples():
        if r.tier == "decoy":
            continue
        rows.append([r.seeds, r.tier, r.set, r.incidents, ci(r.noticed_share, r.noticed_lo90, r.noticed_hi90),
                     ci(r.correct_by_deadline_share, r.correct_by_deadline_lo90, r.correct_by_deadline_hi90),
                     r.correct_but_late, ci(r.correct_but_late_share, r.correct_but_late_lo90, r.correct_but_late_hi90)])
    return md(rows, ["seeds (run, hidden)", "tier", "set", "incidents", "noticed [90%]", "declared correctly by the deadline [90%]", "correct but late (missed)", "share missed [90%]"])


def unasked():
    d = csv("ceiling-unasked")
    rows = []
    for r in d.itertuples():
        if r.tier == "decoy":
            continue
        rows.append([r.seeds, r.tier, r.incidents, r.n, r.declared_correct_with_no_call, ci(r.share, r.lo90, r.hi90)])
    return md(rows, ["seeds (run, hidden)", "tier", "incidents", "n", "declared correctly with no call", "share [90%]"])


def propagation():
    d = csv("ceiling-propagation")
    rows = [[r.seeds, r.source, r.hard_incidents_with_an_answered_source, r.source_answer_right, r.source_answer_wrong, r.mixed,
             ci(r.wrong_share, r.lo90, r.hi90)] for r in d.itertuples()]
    return md(rows, ["seeds (run, hidden)", "source of the recalled answer", "hard incidents with an answered source", "right", "wrong", "mixed", "wrong share [90%]"])


def bounds():
    d = csv("bounds")
    rows = [[r.seeds, r.hard_recurrences, f"{r.R_site_keyed_reach} ({r.R_share_of_recurrences})", f"{r.R_fresh_signature_keyed_reach} ({r.R_fresh_share_of_recurrences})",
             f"{r.smallest_unasked_correct_count_lower_bound_above_0p075} ({r.as_share_of_recurrences})", r.paired_margin_halfwidth_90_if_10_of_83_disagree, r.R_share_of_bill, r.R_bill_halfwidth_90,
             r.forty_percent_of_signature_ceiling, r.record_rung_share_above_which_plus_0p10_is_out_of_reach] for r in d.itertuples()]
    return md(rows, ["seeds (run, hidden)", "hard recurrences", "site-keyed reach R (share)", "signature-keyed reach R_fresh (share)",
                     "fewest unasked-correct with 90% lower bound above 0.075 (share)", "half-width of a paired margin (10 of 83 disagree)", "R, share of bill",
                     "half-width of R's interval", "0.4 x signature ceiling", "0.73 x signature ceiling"])


# ---- item 2 ---------------------------------------------------------------------------------


def gate():
    d = csv("floor-gate")
    rows = []
    for g in ("plain", "plain, altered by the signature shift", "plain, altered by the added edge", "decoy", "hard", "hard/compound/mimic", "hard/compound/contradict",
              "hard/cascade/mimic", "hard/cascade/contradict", "hard/split_brain/mimic", "hard/split_brain/contradict", "hard/slow_leak",
              "decoy/compound", "decoy/cascade", "decoy/split_brain", "decoy/slow_leak"):
        r = [g]
        for rng in ("a-heldout", "c-heldout"):
            x = d[(d["range"] == rng) & (d["group"] == g)]
            if x.empty:
                r += ["", ""]
                continue
            x = x.iloc[0]
            r += [f"{x['gate_on_attached_evidence']}/{x['incidents']} = {num(x['share_attached'], 3)}"
                  + (f", median {num(x['median_delay_attached_s'], 2)} s" if x["median_delay_attached_s"] != "" else ""),
                  f"{num(x['share_all'], 3)}" + (f", median {num(x['median_delay_all_s'], 2)} s" if x["median_delay_all_s"] != "" else "")]
        rows.append(r)
    return md(rows, ["group (hidden)", "A 40000-40199: gate on the attached evidence", "gate on all own evidence", "C 70000-70199: attached", "all own"])


def ladder_rows(rng, form, reset, teacher, source, cuts, keys=("K1", "K2", "K3", "K4")):
    d = csv("floor-ladder")
    x = d[(d["range"] == rng) & (d["form"] == form) & (d["stream_reset"] == reset) & (d["teacher"] == teacher) & (d["source_available"] == source)]
    rows = []
    for c in cuts:
        for k in keys:
            y = x[(x["cutoff_s"] == c) & (x["key"] == k)]
            if y.empty:
                continue
            y = y.iloc[0]
            rows.append([y["seeds"], c, k, y["recalls"], f"{y['recalls_on_plain']}/{y['recalls_on_decoy']}/{y['recalls_on_hard']}", y["wrong"],
                         ci(y["wrong_share"], y["wrong_share_lo90"], y["wrong_share_hi90"]),
                         ci(y["wrong_per_stream"], y["wrong_per_stream_lo90"], y["wrong_per_stream_hi90"], 2),
                         f"{y['hard_recalled_right']}/{y['hard_incidents']}", f"{y['hard_recurrences_recalled_right']}/{y['hard_recurrences']}"])
    return rows


LADDER_HEAD = ["seeds (hidden)", "key read at (s)", "key", "recalls", "on plain/decoy/hard", "wrong", "collision share [90%]", "wrong per stream [90%]",
               "hard recalled right / hard", "hard recurrences recalled right / recurrences"]
ANS = "after its answer (19 s)"
ARR = "at arrival order (W2)"


def ladder_family_reset(rng_list=("a-heldout", "c-heldout"), teacher="hard", source=ANS, reset="yes", form="family"):
    rows = []
    for rng in rng_list:
        rows += ladder_rows(rng, form, reset, teacher, source, [6, 13, 16, 32])
    return md(rows, LADDER_HEAD)


def ladder_family_reset_hard():
    return ladder_family_reset()


def ladder_family_carried():
    return ladder_family_reset(reset="no")


def ladder_site_reset():
    return ladder_family_reset(form="site")


def ladder_family_reset_w2teacher():
    return ladder_family_reset(teacher="hard+decoy", source=ARR)


def ladder_tune():
    return ladder_family_reset(rng_list=("a-tune", "c-tune"))


def e1forms():
    d = csv("floor-e1")
    rows = []
    for rng in ("a-heldout", "c-heldout", "a-tune", "c-tune"):
        for key, form, reset in (("E1 kinds", "site", "yes"), ("E1 timing", "site", "yes"), ("E1 kinds", "family", "yes"), ("E1 timing", "family", "yes"),
                                 ("E1 timing", "family", "no"), ("E1 kinds", "family", "no"), ("E1 timing", "site", "no")):
            x = d[(d["range"] == rng) & (d["key"] == key) & (d["form"] == form) & (d["stream_reset"] == reset) & (d["source_available"] == ANS)]
            if x.empty:
                continue
            y = x.iloc[0]
            rows.append([y["seeds"], key[3:], form, "reset" if reset == "yes" else "carried", y["recalls"], f"{y['recalls_on_plain']}/{y['recalls_on_decoy']}/{y['recalls_on_hard']}",
                         y["wrong"], ci(y["wrong_share"], y["wrong_share_lo90"], y["wrong_share_hi90"]), ci(y["wrong_per_stream"], y["wrong_per_stream_lo90"], y["wrong_per_stream_hi90"], 2),
                         f"{y['hard_recalled_right']}/{y['hard_incidents']}", f"{y['hard_recurrences_recalled_right']}/{y['hard_recurrences']}"])
    return md(rows, ["seeds (hidden)", "E1 level", "form", "stream", "recalls", "on plain/decoy/hard", "wrong", "collision share [90%]", "wrong per stream [90%]",
                     "hard recalled right / hard", "hard recurrences recalled right / recurrences"])


def e1join():
    d = csv("floor-e1-join")
    rows = [[r.arm.replace("sel_rec_", "").replace("_privileged", "").replace("_sens", " (sens)"), r.e1_recalls, r.e1_recalls_on_incidents_with_my_snapshot,
             r.e1_by_tier_plain_hard_decoy, r.e1_recalls_without_own_gate_by_tier, r.floor_recalls_answered, r.floor_by_tier_plain_hard_decoy, r.in_both,
             r.e1_only, r.floor_only, f"{r.of_which_hidden_keys_of_source_and_target_equal}/{r.e1_same_stream_recalls_with_both_keys}"] for r in d.itertuples()]
    return md(rows, ["E1 arm (seeds 40000-40199)", "E1 recalls", "on an incident with a hidden-side snapshot", "E1 by tier plain/hard/decoy",
                     "E1 recalls with no hidden-side snapshot, by tier", "floor recalls", "floor by tier", "in both", "E1 only", "floor only",
                     "same-stream recalls whose hidden keys of source and target are equal"])


def vote():
    d = csv("floor-vote")
    rows = []
    for rng in ("a-heldout", "c-heldout"):
        for key, form, reset, c in (("E1 kinds", "family", "no", "snap"), ("E1 timing", "family", "no", "snap"), ("E1 timing", "family", "yes", "snap"),
                                    ("ladder K2", "family", "no", "16"), ("ladder K4", "family", "no", "32"), ("ladder K4", "family", "yes", "16")):
            for rule in ("last answer", "vote, leader above 0.5", "vote, leader above 0.8"):
                x = d[(d["range"] == rng) & (d["key"] == key) & (d["form"] == form) & (d["stream_reset"] == reset) & (d["cutoff_s"].astype(str) == c) & (d["rule"] == rule)]
                if x.empty:
                    continue
                y = x.iloc[0]
                rows.append([y["seeds"], f"{key}" + (f" at {c} s" if c != "snap" else " at the snapshot"), form, "reset" if reset == "yes" else "carried", rule, y["recalls"],
                             f"{y['recalls_on_plain']}/{y['recalls_on_decoy']}/{y['recalls_on_hard']}", y["wrong"], ci(y["wrong_share"], y["wrong_share_lo90"], y["wrong_share_hi90"]),
                             f"{y['hard_recalled_right']}/{y['hard_incidents']}"])
    return md(rows, ["seeds (hidden)", "key", "form", "stream", "rule", "recalls", "on plain/decoy/hard", "wrong", "collision share [90%]", "hard recalled right / hard"])


def who():
    d = csv("floor-who")
    rows = []
    for rng, lab in (("a-heldout", "A 40000-40199"), ("c-heldout", "C 70000-70199")):
        for setting in ("ladder K2 at 6 s", "ladder K2 at 16 s", "ladder K4 at 16 s", "ladder K4 at 32 s"):
            x = d[(d["range"] == rng) & (d["setting"] == setting) & (d["stream_reset"] == "yes") & (d["wrong"] > 0) & ~d["wrong_recalls_on"].str.startswith("plain ")]
            items = ", ".join(f"{r.wrong_recalls_on.replace('/', ' ').replace('known', '').strip()} {r.wrong}" for r in x.sort_values("wrong", ascending=False).itertuples())
            alt = d[(d["range"] == rng) & (d["setting"] == setting) & (d["stream_reset"] == "yes") & d["wrong_recalls_on"].str.startswith("plain ")]
            plain_alt = "; ".join(f"{r.wrong_recalls_on.replace('plain ', '')} {r.wrong}" for r in alt.itertuples())
            rows.append([lab, setting, items, plain_alt])
    return md(rows, ["seeds (hidden)", "family-keyed, stream reset, hard teacher", "wrong recalls by the incident that recalled (tier family mode, count)", "of the plain ones: altered by the shift / by the added edge / unaltered"])


def streak():
    d = csv("floor-streak")
    rows = []
    for g in ("decoy", "decoy/slow_leak", "decoy/other families", "hard", "hard/slow_leak", "plain"):
        r = [g]
        for c in (10, 16, 24, 32):
            x = d[(d["range"] == "a-heldout") & (d["cutoff_s"] == c) & (d["group"] == g)].iloc[0]
            r.append(num(x["share"], 3))
        rows.append(r)
    return md(rows, ["group (A 40000-40199, hidden)", "10 s", "16 s", "24 s", "32 s"])


def waiting():
    d = csv("floor-waiting")
    rows = []
    for c in (4, 6, 10, 16, 20, 24, 32):
        r = [c]
        for rng in ("a-heldout", "c-heldout"):
            for s in ("all hard", "critical hard"):
                x = d[(d["range"] == rng) & (d["cutoff_s"].astype(str) == str(c)) & (d["set"] == s)].iloc[0]
                r.append(num(x["deadline_before_cutoff_share"], 3))
        rows.append(r)
    return md(rows, ["cutoff, s after the first alarm", "A: all hard", "A: critical hard", "C: all hard", "C: critical hard"])


def check_w2():
    d = csv("floor-check-w2")
    rows = [[r.seeds, r.form, r.tier, r.mine_incidents_recalled_right_wrong, r.w2_incidents_recalled_right_wrong, r.equal] for r in d.itertuples()]
    return md(rows, ["seeds (hidden)", "form", "tier", "this script: incidents/recalled/right/wrong", "W2's table", "equal"])


def check_partition():
    d = csv("floor-check-partition")
    d = d[d["subset"] != "not hard slow leak"]
    rows = [[r.seeds, r.subset, r.incidents, r.w2_classes, r.data_classes, r.incidents_whose_w2_class_has_one_data_class, r.incidents_whose_data_class_has_one_w2_class] for r in d.itertuples()]
    return md(rows, ["seeds (hidden)", "subset", "incidents", "W2 code-derived classes", "data-derived classes (K2 at 6 s)",
                     "incidents whose W2 class holds one data class", "incidents whose data class holds one W2 class"])


# ---- item 4 ---------------------------------------------------------------------------------


def a2csv(kind):
    return pd.read_csv(OUT / f"w3-a2-a2_learn-{kind}.csv", keep_default_na=False)


def a2_precision():
    d = a2csv("precision")
    rows = [[r.group, r.predictions, f"{r.on_true_edge_pair} = {num(r.precision_pair)} [{num(r.pair_lo90)}, {num(r.pair_hi90)}]", num(r.chance_pair_expected),
             r.event_hits, f"{r.followed_hidden} = {num(r.followed_share)} [{num(r.followed_lo90)}, {num(r.followed_hi90)}]", r.followed_public,
             num(r.chance_followed_expected), num(r.lead_from_alarm_median_s, 2), num(r.lead_from_made_median_s, 2)] for r in d.itertuples()]
    return md(rows, ["group (seeds 10000-10019; hidden joined to public)", "predictions", "on a true hidden-edge pair [90%]", "chance (expected share)", "event hits",
                     "followed, hidden [90%]", "followed, A2's reading", "chance of following", "median lead from the alarm, s (followed)", "median lead from the step, s"])


def a2_perm():
    d = a2csv("perm-summary").iloc[0]
    rows = [["true-edge pair precision", f"{d['observed_true_edge']}/{d['predictions']}", d["uniform_true_edge_precision_mean"],
             f"[{d['uniform_true_edge_precision_p05']}, {d['uniform_true_edge_precision_p95']}]", d["p_true_edge_ge_observed_uniform"]],
            ["followed within the band (hidden)", f"{d['observed_followed_hidden']}/{d['predictions']}", d["uniform_followed_mean"],
             f"[{d['uniform_followed_p05']}, {d['uniform_followed_p95']}]", d["p_followed_ge_observed_uniform"]]]
    return md(rows, ["verdict", "observed", "mean under permutation of the predicted service", "5th-95th percentile", "share of permutations at or above the observed"])


def a2_coverage():
    d = a2csv("coverage")
    rows = [[r.group, r.true_partner_alarms, r.lead_within_10s, r.plus_predicting_alarm_counted,
             f"{r.lost_site_alarm_explained_by_an_upstream_burst} / {r.lost_site_alarm_not_a_first_alarm}", r.plus_partner_quiet_trial_licensed,
             r.edge_held_at_some_read_of_the_stream, r.covered, r.covered_loose] for r in d.itertuples()]
    return md(rows, ["true partner alarms (seeds 10000-10019)", "all", "lead within the widest band (10 s)", "+ the site's alarm was one the learner counted",
                     "of those lost: explained by an upstream burst / not a first alarm",
                     "+ partner quiet (trial licensed)", "an edge for the pair was held at some read of the stream", "covered (predicted on this alarm, in the window)",
                     "covered, loose (any prediction on the pair whose window holds it)"])


def a2_edges():
    d = a2csv("edges")
    rows = [[r.group, r.edges, f"{r.on_true_edge_pair} = {num(r.precision)} [{num(r.lo90)}, {num(r.hi90)}]", num(r.chance_expected),
             r.relation_cascade, r.relation_added_edge, r.relation_cascade_rev, r.relation_added_edge_rev, r.relation_peer, r.relation_none] for r in d.itertuples()]
    return md(rows, ["learned edges (seed, a, b, band)", "edges", "on a true hidden-edge pair [90%]", "chance (expected share)", "cascade", "added edge",
                     "cascade, reversed", "added edge, reversed", "split-brain peer", "none"])


def pairs():
    d = csv("pairs")
    rows = []
    for r in d.itertuples():
        rows.append([LABEL.get(r.range, "A 10000-10019 (A2's smoke)"), r.events, r.n_events, ci(r.per_stream, r.lo90, r.hi90, 2),
                     f"{r.with_one_earlier_sighting_of_the_pair} ({num(r.per_stream_with_one, 3)} per stream)",
                     f"{r.with_two_earlier_sightings} ({num(r.per_stream_with_two, 3)} per stream)"])
    return md(rows, ["seeds (hidden)", "true partner events", "events", "per stream [90%]", "with at least one earlier sighting of the pair in the stream",
                     "with at least two"])


TABLES = {n: f for n, f in list(globals().items()) if callable(f) and not n.startswith("_") and n not in (
    "csv", "md", "num", "ci", "a2csv", "ladder_rows", "ladder_family_reset") and f.__module__ == __name__}

if __name__ == "__main__":
    for name, fn in TABLES.items():
        if len(sys.argv) > 1 and sys.argv[1] != name:
            continue
        print(f"## {name}\n")
        print(fn())
        print()
