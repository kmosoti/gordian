"""W2: render the tables of the report from the committed `w2-*.csv`.

Usage: w2_tables.py [NAME]   (print one table, or all when no name)

Each function returns a Markdown table; `w2_assemble.py` puts them into the report template. Every
table names the seed range and side of its rows. Nothing is computed here that is not in a CSV.
"""

from __future__ import annotations

import sys

import pandas as pd

import w2_common as C

OUT = C.OUT
RANGE_A = ["a-tune", "a-heldout"]


def csv(name: str) -> pd.DataFrame:
    return pd.read_csv(OUT / f"w2-{name}.csv", keep_default_na=False)


def num(x, nd=3):
    if x == "" or pd.isna(x):
        return ""
    return f"{float(x):.{nd}f}"


def ci(p, lo, hi, nd=3):
    if p == "":
        return ""
    return f"{num(p, nd)} [{num(lo, nd)}, {num(hi, nd)}]"


def lab(df, r):
    """`10000-10099` style label of a range name."""
    return df.loc[df["range"] == r, "seeds"].iloc[0]


def md(rows: list[list], header: list[str]) -> str:
    out = ["| " + " | ".join(header) + " |", "|" + "|".join("---" for _ in header) + "|"]
    for r in rows:
        out.append("| " + " | ".join(str(c) for c in r) + " |")
    return "\n".join(out)


# ---- item 1 ---------------------------------------------------------------------------------


def _rec_summary(ranges, groups):
    d = csv("recurrence-summary")
    rows = []
    for r in ranges:
        for g in groups:
            x = d[(d["range"] == r) & (d["group"] == g)]
            if x.empty:
                continue
            x = x.iloc[0]
            rows.append([x["seeds"], g, x["incidents"], num(x["incidents_per_stream"], 2), x["recurrences"],
                         ci(x["recurrences_per_stream"], x["rec_per_stream_lo90"], x["rec_per_stream_hi90"]),
                         ci(x["recurrence_share"], x["share_lo90"], x["share_hi90"])])
    return md(rows, ["seeds (hidden)", "group", "incidents", "per stream", "recurrences", "recurrences per stream [90%]", "share of incidents [90%]"])


GROUPS = ["all", "tier=plain", "tier=hard", "tier=decoy", "hard/compound", "hard/cascade", "hard/split_brain", "hard/slow_leak"]
GROUPS_MODE = ["hard/compound/mimic", "hard/compound/contradict", "hard/cascade/mimic", "hard/cascade/contradict",
               "hard/split_brain/mimic", "hard/split_brain/contradict"]
GROUPS_PLAIN = ["plain/ResourceExhausted", "plain/ConfigDrift", "plain/DependencyDown", "plain/CredentialExpired",
                "plain/Intermittent", "plain/duo", "decoy/compound", "decoy/cascade", "decoy/split_brain", "decoy/slow_leak"]


def rec_summary_a():
    return _rec_summary(RANGE_A, GROUPS)


def rec_summary_modes():
    return _rec_summary(RANGE_A, GROUPS_MODE + GROUPS_PLAIN)


def rec_summary_b():
    return _rec_summary(["b"], GROUPS + GROUPS_MODE)


def rec_eligibility():
    d = csv("recurrence-eligibility")
    rows = [[r.seeds, r.incidents, r.with_eligible_template, r.recurrences, num(r.recurrence_share_of_eligible, 4),
             r.recurrences_without_eligible_template, r.recurrences_of_a_recurrence] for r in d.itertuples()]
    return md(rows, ["seeds (hidden)", "incidents", "with an eligible template", "recurrences", "recurrences / eligible",
                     "recurrences with none eligible", "of which repeat a recurrence"])


def rec_gaps():
    d = csv("recurrence-gaps")
    rows = []
    for r in RANGE_A + ["b"]:
        for g in ("all", "hard"):
            for m in ("gap_incidents", "gap_seconds", "gap_since_template_ended_seconds", "gap_hard_incidents"):
                x = d[(d["range"] == r) & (d["group"] == g) & (d["measure"] == m)]
                if x.empty:
                    continue
                x = x.iloc[0]
                if g == "all" and m == "gap_hard_incidents":
                    continue
                rows.append([x["seeds"], g, m, x["n"], num(x["mean"], 1), num(x["min"], 1), num(x["p10"], 1), num(x["p50"], 1),
                             num(x["p90"], 1), num(x["max"], 1)])
    return md(rows, ["seeds (hidden)", "recurrences", "gap", "n", "mean", "min", "p10", "p50", "p90", "max"])


def curve():
    d = csv("experience-curve")
    rows = []
    for r in RANGE_A + ["b"]:
        for scope, ns in (("all", [2, 4, 6, 8, 10, 15, 20, 25, 30]), ("hard", [1, 2, 3, 4, 5, 6])):
            for n in ns:
                x = d[(d["range"] == r) & (d["scope"] == scope) & (d["seen"] == n)]
                if x.empty:
                    continue
                x = x.iloc[0]
                rows.append([x["seeds"], scope, n, x["streams"], ci(x["mean_cum_recurrences"], x["cum_lo90"], x["cum_hi90"], 2),
                             num(x["recurrence_share_at_n"], 3)])
    return md(rows, ["seeds (hidden)", "incidents counted", "seen (n)", "streams reaching n", "mean cumulative recurrences [90%]",
                     "share of the n-th that is a recurrence"])


def stream_order():
    d = csv("experience-stream-order")
    rows = [[r.seeds, r.streams_seen, r.cum_hard_incidents, r.cum_hard_recurrences, num(r.share, 3)] for r in d.itertuples()
            if r.range in RANGE_A + ["b"]]
    return md(rows, ["seeds (hidden)", "streams seen", "cumulative hard incidents", "cumulative hard recurrences", "share"])


def stale():
    d = csv("stale")
    rows = []
    for r in RANGE_A + ["b"]:
        for g in ("all", "tier=plain", "tier=hard", "tier=decoy", "hard/compound", "hard/cascade", "hard/split_brain", "hard/slow_leak"):
            x = d[(d["range"] == r) & (d["group"] == g)]
            if x.empty:
                continue
            x = x.iloc[0]
            rows.append([x["seeds"], g, x["recurrences"], f"{x['straddles']} ({num(x['straddles_share'], 2)})",
                         x["stale_sig"], x["stale_edge"], ci(x["stale_any_share"], x["stale_any_lo90"], x["stale_any_hi90"], 3),
                         x["stale_any"]])
    return md(rows, ["seeds (hidden)", "group", "recurrences", "template before a change, repeat after (share)", "stale: signature shift",
                     "stale: added edge", "stale (either) share of recurrences [90%]", "stale (either) count"])


# ---- item 2 ---------------------------------------------------------------------------------


def elsewhere():
    d = csv("family-elsewhere")
    rows = []
    for r in RANGE_A + ["b"]:
        for key, groups in (("fam_mode", ["all hard", "compound", "cascade", "split_brain", "slow_leak"]), ("strict", ["all hard"])):
            for g in groups:
                x = d[(d["range"] == r) & (d["key"] == key) & (d["group"] == g)]
                if x.empty:
                    continue
                x = x.iloc[0]
                rows.append([x["seeds"], key, g, x["hard_incidents"], x["recurrences"], x["non_recurrence_hard"],
                             x["elsewhere_non_recurrence"], ci(x["elsewhere_per_stream"], x["elsewhere_lo90"], x["elsewhere_hi90"]),
                             ci(x["elsewhere_share_of_non_recurrence"], x["share_lo90"], x["share_hi90"]),
                             ci(x["family_reach_share_of_hard"], x["reach_lo90"], x["reach_hi90"])])
    return md(rows, ["seeds (hidden)", "key", "group", "hard", "recurrences", "non-recurrence hard", "same family-and-mode earlier at another site",
                     "per stream [90%]", "share of non-recurrence hard [90%]", "recurrence or elsewhere, share of hard [90%]"])


# ---- item 3 ---------------------------------------------------------------------------------


def vocab():
    d = csv("vocabulary")
    rows = [[r.seeds, r.sample, f"{r.streams_with_at_least_two_labels} of {r.streams}", num(r.messages_per_stream, 1),
             num(r.label_entropy_bits, 3), num(r.mi_bits, 3), num(r.mi_permutation_mean_bits, 3), num(r.mi_permutation_p95_bits, 3),
             ci(r.mi_excess_bits, r.excess_lo90, r.excess_hi90), num(r.share_ids_at_exactly_one_label, 3),
             num(r.share_ids_seen_twice_at_one_label, 3)] for r in d.itertuples()]
    return md(rows, ["seeds (public ids, hidden labels)", "sample", "streams with two or more families", "messages per stream",
                     "label entropy (bits)", "MI(id; family) (bits)", "permutation null mean", "null p95", "excess over null [90%]",
                     "share of ids at exactly one family", "same, ids seen at least twice"])


def vocab_precision():
    d = csv("vocabulary-precision")
    rows = [[r.seeds, r.messages, r.incident_borne, r.background, ci(r.precision_incident_borne, r.lo90, r.hi90),
             num(r.recall_of_incident_borne, 3)] for r in d.itertuples()]
    return md(rows, ["seeds (public ids, hidden labels)", "free-form messages whose id is one of the stream's hard vocabulary", "incident-borne",
                     "background", "share incident-borne [90%]", "share of incident-borne kept"])


# ---- item 4 ---------------------------------------------------------------------------------


def delays():
    d = csv("cascade-delays")
    rows = []
    for r in d.itertuples():
        if r.unit == "no partner alarm":
            rows.append([r.seeds, r.tier, r.family, r.mode, r.partner_is_dependent, r.n, r.unit, "", "", "", ""])
            continue
        rows.append([r.seeds, r.tier, r.family, r.mode, r.partner_is_dependent, r.n, r.unit, num(r.mean, 1), num(r.p10, 1), num(r.p50, 1),
                     num(r.p90, 1)])
    return md(rows, ["seeds (hidden)", "tier", "family", "mode", "partner is a dependent of the site", "n", "unit", "mean", "p10", "p50", "p90"])


def edgeadd():
    d = csv("edgeadd")
    rows = [[r.seeds, r.tier, r.edge_add_at_s, r.streams_with_an_edge, num(r.incidents_after_per_stream, 2), num(r.exposed_per_stream, 2),
             ci(r.alarm_over_new_edge_per_stream, r.lo90, r.hi90, 2), r.alarm_over_new_edge_total, num(r.share_of_after, 3)]
            for r in d.itertuples()]
    return md(rows, ["seeds (hidden)", "incidents", "edge added at (s)", "streams with an edge", "after it, per stream",
                     "newly downstream of the site or partner, per stream", "alarm over the new edge, per stream [90%]", "total", "share of those after"])


def pairs():
    d = csv("pair-recurrence")
    rows = [[r.seeds, r.family, r.tier, r.incidents, num(r.per_stream, 2), r.flagged_recurrences, r.pair_seen_earlier_ordered,
             r.pair_seen_earlier_unordered, r.root_seen_earlier, r.unflagged_pair_repeats, num(r.mean_incomparable_pairs_per_graph, 1)]
            for r in d.itertuples()]
    return md(rows, ["seeds (hidden)", "family", "tier", "incidents", "per stream", "flagged recurrences", "ordered pair seen earlier",
                     "unordered pair seen earlier", "root seen earlier", "pair repeats not flagged", "incomparable pairs per graph"])


# ---- item 5 ---------------------------------------------------------------------------------


def regime_effects(ranges):
    d = csv("regime-effects")
    rows = []
    for r in ranges:
        for chg in ("signature_shift", "edge_add"):
            for g in ("all", "tier=plain", "tier=hard", "tier=decoy", "hard/compound", "hard/cascade", "hard/split_brain", "hard/slow_leak",
                      "decoy/compound", "decoy/cascade"):
                x = d[(d["range"] == r) & (d["change"] == chg) & (d["group"] == g)]
                if x.empty:
                    continue
                x = x.iloc[0]
                rows.append([x["seeds"], f"{chg} at {x['at_s']} s", g, num(x["incidents_after_per_stream"], 2), x["affected"],
                             ci(x["affected_per_stream"], x["aff_lo90"], x["aff_hi90"]),
                             ci(x["affected_share_of_after"], x["share_lo90"], x["share_hi90"])])
    return md(rows, ["seeds (hidden)", "change", "group", "incidents after it, per stream", "affected", "affected per stream [90%]",
                     "share of those after it [90%]"])


def regime_a():
    return regime_effects(RANGE_A)


def regime_b():
    return regime_effects(["b"])


def hard_after():
    d = csv("regime-hard-after")
    rows = [[r.seeds, r.measure, r.value, r.n] for r in d.itertuples()]
    return md(rows, ["seeds (hidden)", "measure", "value", "n"])


# ---- item 6 ---------------------------------------------------------------------------------


def ceiling_bill():
    d = csv("ceiling-bill")
    rows = [[r.seeds, r.streams, f"{r.total_cost_ns:,}", f"{r.reasoner_modelled_ns:,}", num(r.reasoner_share_of_bill, 5), r.reasoner_calls,
             r.calls_on_hard, r.calls_on_other_incidents, r.calls_on_background, f"{int(r.mean_ns_per_call):,}", r.hard_incidents,
             r.hard_noticed, r.hard_asked, r.hard_declared_correctly] for r in d.itertuples()]
    return md(rows, ["seeds (run)", "streams", "total bill (modelled ns)", "reasoner (modelled ns)", "reasoner share of the bill", "calls",
                     "on hard incidents", "on plain or decoy", "on background", "mean ns per call", "hard incidents", "noticed", "asked",
                     "declared correctly"])


def _ceiling(sets_match, ranges=RANGE_A, cross=False):
    d = csv("ceiling")
    rows = []
    for r in ranges:
        x = d[d["range"] == r]
        x = x[x["set"].str.contains("streams 11 onward", regex=False) == cross]
        for s in x.itertuples():
            rows.append([s.seeds, s.set, s.incidents, num(s.incidents_per_stream, 3), s.streams_with_any, s.calls, f"{s.modelled_ns:,}",
                         ci(s.share_of_total_bill, s.bill_lo90, s.bill_hi90)])
    return md(rows, ["seeds (run, hidden)", "set", "hard incidents", "per stream", "streams with at least one", "calls", "modelled ns",
                     "share of the total bill [90%]"])


def ceiling():
    return _ceiling(None)


def ceiling_cross():
    return _ceiling(None, cross=True)


def unasked():
    d = csv("ceiling-unasked")
    rows = [[r.seeds, r.tier, r.incidents, r.n, r.declared_correct_with_no_call, ci(r.share, r.lo90, r.hi90),
             r.declared_wrong_with_no_call, ci(r.wrong_share, r.wrong_lo90, r.wrong_hi90)] for r in d.itertuples()]
    return md(rows, ["seeds (run, hidden)", "tier", "which", "n", "declared correctly, no call", "share [90%]",
                     "at least one wrong declaration, no call", "share [90%]"])


def propagation():
    d = csv("ceiling-propagation")
    rows = [[r.seeds, r.source, r.hard_incidents_with_an_answered_source, r.source_answer_right, r.source_answer_wrong, r.mixed,
             ci(r.wrong_share, r.lo90, r.hi90, 3)] for r in d.itertuples()]
    return md(rows, ["seeds (run, hidden)", "source of the recalled answer", "hard incidents with an answered source before their notice",
                     "source answer right", "wrong", "mixed", "wrong share [90%]"])


def identity():
    d = csv("ceiling-identity")
    rows = [[r.seeds if hasattr(r, "seeds") else "40000-40199", r.file, r.rows_run, r.rows_l1, r.identical_modulo_run_id] for r in d.itertuples()]
    return md(rows, ["seeds", "file", "rows (this run)", "rows (L1 fresh run)", "identical but for run_id and arm_role"])


# ---- collisions -----------------------------------------------------------------------------


def site_collisions():
    d = csv("site-collisions")
    rows = [[r.seeds, r.memory, r.incident_tier, r.incidents, r.site_in_memory, num(r.share_site_in_memory, 3), r.recalled_right, r.recalled_wrong,
             num(r.wrong_share_of_recalls, 3)] for r in d.itertuples()]
    return md(rows, ["seeds (hidden)", "memory", "incident", "incidents", "site in memory", "share", "recalled right", "recalled wrong",
                     "wrong share of recalls"])


def phase1_collisions():
    d = csv("phase1-collisions")
    rows = [[r.seeds, r.memory.replace(", most recent hard incident or decoy, same stream", ""), r.incident_tier, r.incidents, r.recalled,
             num(r.share_recalled, 3), r.recalled_right, r.recalled_wrong, num(r.wrong_share_of_recalls, 3), num(r.wrong_per_stream, 3)]
            for r in d.itertuples()]
    return md(rows, ["seeds (hidden)", "memory", "incident", "incidents", "recalled", "share", "right", "wrong", "wrong share of recalls",
                     "wrong per stream"])


def gates():
    """The gate results, written after the gates ran (`w2-gates.md` is not a table: a paragraph)."""
    path = OUT / "w2-gates.md"
    return path.read_text().strip() if path.exists() else "(gates not yet recorded)"


TABLES = {
    "gates": gates,
    "rec_summary_a": rec_summary_a, "rec_summary_modes": rec_summary_modes, "rec_summary_b": rec_summary_b,
    "rec_eligibility": rec_eligibility, "rec_gaps": rec_gaps, "curve": curve, "stream_order": stream_order, "stale": stale,
    "elsewhere": elsewhere, "vocab": vocab, "vocab_precision": vocab_precision, "delays": delays, "edgeadd": edgeadd, "pairs": pairs,
    "regime_a": regime_a, "regime_b": regime_b, "hard_after": hard_after, "ceiling_bill": ceiling_bill, "ceiling": ceiling,
    "ceiling_cross": ceiling_cross, "unasked": unasked, "propagation": propagation, "identity": identity, "site_collisions": site_collisions,
    "phase1_collisions": phase1_collisions,
}

if __name__ == "__main__":
    names = sys.argv[1:] or list(TABLES)
    for n in names:
        print(f"### {n}\n")
        print(TABLES[n]())
        print()
