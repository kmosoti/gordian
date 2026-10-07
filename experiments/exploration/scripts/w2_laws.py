#!/usr/bin/env python3
"""W2 items 1 to 5 and 7: the learnable laws of the stream world, from the hidden tables.

Usage: w2_laws.py [--hidden-root DIR] [--out-dir DIR]

Reads `incidents.csv`, `regimes.csv`, `streams.csv` and `vocab.csv` that
`crates/gordian-stream/examples/laws` wrote for each range under the hidden root
(`artifacts/runs/w2/hidden/{a-tune,a-heldout,b}`; see `w2_hidden.sh`) and writes the `w2-*.csv`
tables and `w2-tables.md` into `experiments/exploration/`. Deterministic: no clock, no unseeded
draw. The ceiling (item 6) is `w2_ceiling.py`.

Ranges: `a-tune` 10000-10099 and `a-heldout` 40000-40199 at the default parameters, `b` 50000-50099
with recurrence 0.6 and regime changes at 150 s and 300 s. Items 3 and 4 and the collision tables
are world A only (the brief asks world B for items 1, 2 and 5). Every row names its seed range and
its side (see `w2_common`).
"""

from __future__ import annotations

import argparse
import pathlib

import numpy as np
import pandas as pd

import w2_common as C
from w2_common import boot_ratio, f, per_stream, quantiles

PLAIN_KINDS = ["ResourceExhausted", "ConfigDrift", "DependencyDown", "CredentialExpired", "Intermittent"]
HARD_FAMILIES = ["compound", "cascade", "split_brain", "slow_leak"]
MIN_STREAMS_FOR_CURVE = 20
PERMUTATIONS = 200


# ---- preparation ----------------------------------------------------------------------------


def prepare(t: dict) -> dict:
    """Join regime instants and each recurrence's template; check what the generator promises."""
    inc, reg = t["inc"], t["reg"]
    inf = np.inf
    t1 = reg.groupby("seed")["at_ns"].min()
    t_ss = reg[reg["kind"] == "signature_shift"].groupby("seed")["at_ns"].min()
    t_ea = reg[reg["kind"] == "edge_add"].groupby("seed")["at_ns"].min()
    inc["t_first"] = inc["seed"].map(t1).fillna(inf)
    inc["t_ss"] = inc["seed"].map(t_ss).fillna(inf)
    inc["t_ea"] = inc["seed"].map(t_ea).fillna(inf)
    inc["is_rec"] = inc["recurrence_of"].notna()
    tpl_cols = ["onset_ns", "live_end_ns", "busy_until_ns", "tier", "family", "mode", "site", "other",
                "critical", "kind_a", "kind_b", "is_rec"]
    tpl = inc.set_index(["seed", "incident"])[tpl_cols].add_prefix("tpl_")
    rec = inc[inc["is_rec"]].copy()
    rec["recurrence_of"] = rec["recurrence_of"].astype(int)
    rec = rec.join(tpl, on=["seed", "recurrence_of"])
    assert rec["tpl_tier"].notna().all(), "a recurrence names a template that does not exist"
    # The generator's promise (HIDDEN-DESIGN.md section 7): a recurrence is the template's tier,
    # family, mode, site, partner and criticality; its template is earlier and its services were free.
    for a, b in [("tier", "tpl_tier"), ("family", "tpl_family"), ("mode", "tpl_mode"), ("site", "tpl_site"),
                 ("critical", "tpl_critical"), ("kind_a", "tpl_kind_a"), ("kind_b", "tpl_kind_b")]:
        assert (rec[a] == rec[b]).all(), f"recurrence differs from its template in {a}"
    assert (rec["other"].fillna(-1) == rec["tpl_other"].fillna(-1)).all()
    assert (rec["recurrence_of"] < rec["incident"]).all()
    assert (rec["tpl_busy_until_ns"] <= rec["onset_ns"]).all(), "a template was still busy"
    rec["gap_incidents"] = rec["incident"] - rec["recurrence_of"]
    rec["gap_s"] = (rec["onset_ns"] - rec["tpl_onset_ns"]) / C.NS
    rec["since_end_s"] = (rec["onset_ns"] - rec["tpl_live_end_ns"]) / C.NS
    # Rank among the stream's hard incidents, for the gap in hard incidents.
    hard = inc[inc["tier"] == "hard"].copy()
    hard["hard_rank"] = hard.groupby("seed").cumcount()
    rank = hard.set_index(["seed", "incident"])["hard_rank"]
    hr = rec[rec["tier"] == "hard"].copy()
    hr["gap_hard"] = rank.reindex(pd.MultiIndex.from_frame(hr[["seed", "incident"]])).to_numpy() - rank.reindex(
        pd.MultiIndex.from_frame(hr[["seed", "recurrence_of"]])
    ).to_numpy()
    rec["gap_hard"] = hr["gap_hard"]
    # Stale by construction: the recurrence was altered by a change that took effect after its
    # template began (so the template's record describes physics that no longer holds).
    rec["straddles"] = rec["tpl_onset_ns"] < rec["t_first"]
    rec["straddles"] &= rec["onset_ns"] >= rec["t_first"]
    rec["stale_sig"] = (rec["sig_altered"] == 1) & (rec["tpl_onset_ns"] < rec["t_ss"])
    rec["stale_edge"] = (rec["edge_altered"] == 1) & (rec["tpl_onset_ns"] < rec["t_ea"])
    rec["stale_any"] = rec["stale_sig"] | rec["stale_edge"]
    t["rec"] = rec
    check_signature_shift(t)
    return t


def check_signature_shift(t: dict) -> None:
    """An independent check of `sig_altered`: the flag comes from rebuilding the incident (the stream
    crate's accessor); here the same set is predicted from the rule read off `present.rs` and
    `incident.rs` (an incident is altered by a shift of kind K when it was built after the shift
    and emits K's characteristic message: a plain incident of kind K that is not a duo; a compound
    with K as `a`, or as `b` in a Contradict presentation or a hard Mimic's phase 2; a cascade with
    K as `a`). The two must agree on every incident, or the script stops."""
    inc, reg = t["inc"], t["reg"]
    ss = reg[reg["kind"] == "signature_shift"].set_index("seed")
    k = inc["seed"].map(ss["kind_a"])
    post = inc["onset_ns"] >= inc["t_ss"]
    plain = (inc["tier"] == "plain") & (inc["known_kind"] == k) & (inc["duo"] != 1)
    comp_a = (inc["family"] == "compound") & (inc["kind_a"] == k)
    comp_b = (inc["family"] == "compound") & (inc["kind_b"] == k) & ((inc["mode"] == "contradict") | (inc["tier"] == "hard"))
    casc = (inc["family"] == "cascade") & (inc["kind_a"] == k)
    predicted = (plain | comp_a | comp_b | casc) & post
    bad = int((predicted != (inc["sig_altered"] == 1)).sum())
    assert bad == 0, f"{bad} incidents where the rebuilt alteration disagrees with the rule read from the code"


def groups(inc: pd.DataFrame):
    """(name, mask) of every group the recurrence tables report: tier, then family within tier."""
    yield "all", pd.Series(True, index=inc.index)
    for tier in ("plain", "hard", "decoy"):
        yield f"tier={tier}", inc["tier"] == tier
    for k in PLAIN_KINDS:
        yield f"plain/{k}", (inc["tier"] == "plain") & (inc["known_kind"] == k)
    yield "plain/duo", (inc["tier"] == "plain") & (inc["duo"] == 1)
    for tier in ("hard", "decoy"):
        for fam in HARD_FAMILIES:
            yield f"{tier}/{fam}", (inc["tier"] == tier) & (inc["family"] == fam)
    for fm in sorted(inc.loc[inc["tier"] == "hard", "fam_mode"].unique()):
        yield f"hard/{fm}", (inc["tier"] == "hard") & (inc["fam_mode"] == fm)


# ---- item 1: recurrence ---------------------------------------------------------------------


def item1_summary(name: str, t: dict) -> list[dict]:
    inc, seeds, rng_label = t["inc"], t["seeds"], C.seed_range(t["seeds"])
    rows = []
    for g, mask in groups(inc):
        n = per_stream(inc, mask, seeds)
        r = per_stream(inc, mask & inc["is_rec"], seeds)
        if n.sum() == 0:
            continue
        p, lo, hi = boot_ratio(r)
        s, slo, shi = boot_ratio(r, n)
        pn, nlo, nhi = boot_ratio(n)
        rows.append({
            "range": name, "seeds": rng_label, "side": "hidden", "group": g, "streams": len(seeds),
            "incidents": int(n.sum()), "incidents_per_stream": f(pn), "recurrences": int(r.sum()),
            "recurrences_per_stream": f(p), "rec_per_stream_lo90": f(lo), "rec_per_stream_hi90": f(hi),
            "recurrence_share": f(s), "share_lo90": f(slo), "share_hi90": f(shi),
        })
    return rows


def item1_eligibility(name: str, t: dict) -> dict:
    inc = t["inc"]
    elig = inc["eligible_templates"] > 0
    return {
        "range": name, "seeds": C.seed_range(t["seeds"]), "side": "hidden", "incidents": len(inc),
        "with_eligible_template": int(elig.sum()), "recurrences": int(inc["is_rec"].sum()),
        "recurrence_share_of_eligible": f(inc["is_rec"].sum() / elig.sum()),
        "recurrences_without_eligible_template": int((inc["is_rec"] & ~elig).sum()),
        "recurrences_of_a_recurrence": int(t["rec"]["tpl_is_rec"].sum()),
    }


def item1_gaps(name: str, t: dict) -> list[dict]:
    rec, rng_label = t["rec"], C.seed_range(t["seeds"])
    rows = []
    for g, m in [("all", rec["tier"].notna()), ("plain", rec["tier"] == "plain"),
                 ("hard", rec["tier"] == "hard"), ("decoy", rec["tier"] == "decoy")]:
        sub = rec[m]
        if sub.empty:
            continue
        for what, col in [("gap_incidents", "gap_incidents"), ("gap_seconds", "gap_s"),
                          ("gap_since_template_ended_seconds", "since_end_s"), ("gap_hard_incidents", "gap_hard")]:
            x = sub[col].dropna()
            if x.empty:
                continue
            q = quantiles(x)
            rows.append({"range": name, "seeds": rng_label, "side": "hidden", "group": g, "measure": what,
                         "n": len(x), "mean": f(x.mean(), 2), "min": f(q[0], 2), "p10": f(q[1], 2),
                         "p25": f(q[2], 2), "p50": f(q[3], 2), "p75": f(q[4], 2), "p90": f(q[5], 2),
                         "max": f(q[6], 2)})
    return rows


def item1_curve(name: str, t: dict) -> list[dict]:
    inc, seeds, rng_label = t["inc"], t["seeds"], C.seed_range(t["seeds"])
    rows = []
    by_seed = {s: g for s, g in inc.groupby("seed")}
    for scope in ("all", "hard"):
        seqs = []
        for s in seeds:
            g = by_seed.get(s)
            if g is None:
                seqs.append(np.zeros(0, dtype=int))
                continue
            if scope == "hard":
                g = g[g["tier"] == "hard"]
            seqs.append(g["is_rec"].to_numpy(dtype=int))
        for n in range(1, max(len(s) for s in seqs) + 1):
            have = [s for s in seqs if len(s) >= n]
            if len(have) < MIN_STREAMS_FOR_CURVE:
                break
            cum = np.array([s[:n].sum() for s in have], dtype=float)
            haz = np.array([s[n - 1] for s in have], dtype=float)
            _, clo, chi = boot_ratio(cum)
            rows.append({"range": name, "seeds": rng_label, "side": "hidden", "scope": scope, "seen": n,
                         "streams": len(have), "mean_cum_recurrences": f(cum.mean()),
                         "cum_lo90": f(clo), "cum_hi90": f(chi), "recurrence_share_at_n": f(haz.mean())})
    return rows


def item1_stream_order(name: str, t: dict) -> list[dict]:
    """Hard incidents seen and hard recurrences, cumulative over streams in seed order."""
    inc, seeds, rng_label = t["inc"], t["seeds"], C.seed_range(t["seeds"])
    hard = inc["tier"] == "hard"
    n = per_stream(inc, hard, seeds)
    r = per_stream(inc, hard & inc["is_rec"], seeds)
    rows = []
    for k in (10, 25, 50, 75, 100, 150, 200):
        if k > len(seeds):
            continue
        rows.append({"range": name, "seeds": rng_label, "side": "hidden", "streams_seen": k,
                     "cum_hard_incidents": int(n[:k].sum()), "cum_hard_recurrences": int(r[:k].sum()),
                     "share": f(r[:k].sum() / n[:k].sum()),
                     "share_first_half_of_range": f(r[: len(seeds) // 2].sum() / n[: len(seeds) // 2].sum()),
                     "share_second_half_of_range": f(r[len(seeds) // 2 :].sum() / n[len(seeds) // 2 :].sum())})
    return rows


def item1_stale(name: str, t: dict) -> list[dict]:
    inc, rec, seeds, rng_label = t["inc"], t["rec"], t["seeds"], C.seed_range(t["seeds"])
    rows = []
    for g, mask in groups(inc):
        if g.startswith("plain/") and g != "plain/duo":
            continue
        n_rec = per_stream(inc, mask & inc["is_rec"], seeds)
        if n_rec.sum() == 0:
            continue
        sub = rec[mask.reindex(rec.index, fill_value=False)]
        out = {"range": name, "seeds": rng_label, "side": "hidden", "group": g, "recurrences": int(n_rec.sum())}
        for col in ("straddles", "stale_sig", "stale_edge", "stale_any"):
            x = per_stream(sub, sub[col], seeds)
            p, lo, hi = boot_ratio(x, n_rec)
            out[col] = int(x.sum())
            out[col + "_share"] = f(p)
            out[col + "_lo90"] = f(lo)
            out[col + "_hi90"] = f(hi)
        rows.append(out)
    return rows


def per_stream_table(name: str, t: dict) -> list[dict]:
    """One row per stream: incidents and recurrences by tier, hard recurrences by family."""
    inc, rng_label = t["inc"], C.seed_range(t["seeds"])
    rows = []
    for s, g in inc.groupby("seed"):
        row = {"range": name, "seeds": rng_label, "side": "hidden", "seed": int(s), "incidents": len(g),
               "recurrences": int(g["is_rec"].sum())}
        for tier in ("plain", "hard", "decoy"):
            m = g["tier"] == tier
            row[f"incidents_{tier}"] = int(m.sum())
            row[f"recurrences_{tier}"] = int((m & g["is_rec"]).sum())
        for fam in HARD_FAMILIES:
            row[f"hard_recurrences_{fam}"] = int(((g["tier"] == "hard") & (g["family"] == fam) & g["is_rec"]).sum())
        rows.append(row)
    return rows


def recurrence_table(name: str, t: dict) -> list[dict]:
    """One row per recurrence: the incident it repeats, the gaps, and staleness."""
    rec, rng_label = t["rec"], C.seed_range(t["seeds"])
    rows = []
    for r in rec.itertuples():
        rows.append({"range": name, "seeds": rng_label, "side": "hidden", "seed": int(r.seed), "incident": int(r.incident),
                     "repeats_incident": int(r.recurrence_of), "tier": r.tier, "family": r.family, "mode": r.mode,
                     "site": int(r.site), "gap_incidents": int(r.gap_incidents), "gap_s": f(r.gap_s, 3),
                     "since_template_ended_s": f(r.since_end_s, 3), "stale_signature_shift": int(r.stale_sig),
                     "stale_added_edge": int(r.stale_edge)})
    return rows


# ---- item 2: same family, different site ----------------------------------------------------


def item2(name: str, t: dict) -> list[dict]:
    inc, seeds, rng_label = t["inc"], t["seeds"], C.seed_range(t["seeds"])
    hard = inc[inc["tier"] == "hard"]
    rows = []
    for key in ("fam_mode", "strict"):
        flags = []  # per hard incident: elsewhere, coincident same site, first of its key
        for _, g in hard.groupby("seed"):
            seen: dict[str, list[int]] = {}
            for r in g.itertuples():
                k = getattr(r, key)
                sites = seen.get(k, [])
                flags.append((r.Index, any(s != r.site for s in sites), any(s == r.site for s in sites), not sites))
                seen.setdefault(k, []).append(r.site)
        fl = pd.DataFrame(flags, columns=["idx", "elsewhere", "same_site", "first"]).set_index("idx")
        h = hard.join(fl)
        h["nonrec"] = ~h["is_rec"]
        h["elsewhere_nonrec"] = h["elsewhere"] & h["nonrec"]
        h["same_site_nonrec"] = h["same_site"] & h["nonrec"] & ~h["elsewhere"]
        # What a family-keyed memory could reach at best by experience: recurrences and elsewhere.
        h["reach_family"] = h["is_rec"] | h["elsewhere"]
        gs = [("all hard", h["tier"] == "hard")] + [(fam, h["family"] == fam) for fam in HARD_FAMILIES]
        gs += [(fm, h["fam_mode"] == fm) for fm in sorted(h["fam_mode"].unique())]
        for gname, m in gs:
            sub = h[m]
            if sub.empty:
                continue
            n_hard = per_stream(sub, sub["tier"] == "hard", seeds)
            n_non = per_stream(sub, sub["nonrec"], seeds)
            n_els = per_stream(sub, sub["elsewhere_nonrec"], seeds)
            n_ss = per_stream(sub, sub["same_site_nonrec"], seeds)
            n_rec = per_stream(sub, sub["is_rec"], seeds)
            n_first = per_stream(sub, sub["first"] & sub["nonrec"], seeds)
            n_reach = per_stream(sub, sub["reach_family"], seeds)
            pe, elo, ehi = boot_ratio(n_els)
            ps, slo, shi = boot_ratio(n_els, n_non)
            pr, rlo, rhi = boot_ratio(n_reach, n_hard)
            rows.append({
                "range": name, "seeds": rng_label, "side": "hidden", "key": key, "group": gname,
                "hard_incidents": int(n_hard.sum()), "recurrences": int(n_rec.sum()),
                "non_recurrence_hard": int(n_non.sum()), "elsewhere_non_recurrence": int(n_els.sum()),
                "elsewhere_per_stream": f(pe), "elsewhere_lo90": f(elo), "elsewhere_hi90": f(ehi),
                "elsewhere_share_of_non_recurrence": f(ps), "share_lo90": f(slo), "share_hi90": f(shi),
                "same_site_non_recurrence_only": int(n_ss.sum()),
                "first_of_its_key_non_recurrence": int(n_first.sum()),
                "family_reach_share_of_hard": f(pr), "reach_lo90": f(rlo), "reach_hi90": f(rhi),
            })
    return rows


# ---- item 5: regime changes -----------------------------------------------------------------


def item5(name: str, t: dict) -> tuple[list[dict], list[dict], list[dict]]:
    inc, reg, seeds, rng_label = t["inc"], t["reg"], t["seeds"], C.seed_range(t["seeds"])
    eff, after, kinds = [], [], []
    for kind, tcol, flag in (("signature_shift", "t_ss", "sig_altered"), ("edge_add", "t_ea", "edge_altered")):
        at = reg[reg["kind"] == kind]["at_ns"].iloc[0]
        post = inc["onset_ns"] >= inc[tcol]
        gs = [("all", inc["tier"].notna())] + [(f"tier={tier}", inc["tier"] == tier) for tier in ("plain", "hard", "decoy")]
        gs += [("plain/known", (inc["tier"] == "plain"))]
        gs += [(f"{tier}/{fam}", (inc["tier"] == tier) & (inc["family"] == fam)) for tier in ("hard", "decoy")
               for fam in HARD_FAMILIES]
        for gname, m in gs:
            if gname == "plain/known":
                continue
            n_post = per_stream(inc, m & post, seeds)
            n_alt = per_stream(inc, m & post & (inc[flag] == 1), seeds)
            n_exp = per_stream(inc, m & post & (inc["edge_exposed"] == 1), seeds) if kind == "edge_add" else None
            pa, alo, ahi = boot_ratio(n_alt)
            ps, slo, shi = boot_ratio(n_alt, n_post)
            row = {"range": name, "seeds": rng_label, "side": "hidden", "change": kind, "at_s": f(at / C.NS, 0),
                   "group": gname, "incidents_after_per_stream": f(n_post.mean()),
                   "affected": int(n_alt.sum()), "affected_per_stream": f(pa), "aff_lo90": f(alo), "aff_hi90": f(ahi),
                   "affected_share_of_after": f(ps), "share_lo90": f(slo), "share_hi90": f(shi)}
            row["exposed_per_stream"] = f(n_exp.mean()) if n_exp is not None else ""
            eff.append(row)
        h = inc["tier"] == "hard"
        streams_hit = (per_stream(inc, h & post & (inc[flag] == 1), seeds) > 0)
        after.append({"range": name, "seeds": rng_label, "side": "hidden", "measure": f"streams with at least one hard incident affected by {kind}",
                      "value": f(streams_hit.mean()), "n": len(seeds)})
    h = inc["tier"] == "hard"
    n_h = per_stream(inc, h, seeds)
    for label, col in (("first change", "t_first"),):
        n_after = per_stream(inc, h & (inc["onset_ns"] >= inc[col]), seeds)
        p, lo, hi = boot_ratio(n_after, n_h)
        after.append({"range": name, "seeds": rng_label, "side": "hidden",
                      "measure": f"share of hard incidents with onset at or after the {label}", "value": f(p),
                      "n": int(n_h.sum())})
        after.append({"range": name, "seeds": rng_label, "side": "hidden",
                      "measure": f"  90% interval ({label})", "value": f"[{f(lo)}, {f(hi)}]", "n": ""})
    last = inc["seed"].map(reg.groupby("seed")["at_ns"].max())
    n_after2 = per_stream(inc, h & (inc["onset_ns"] >= last), seeds)
    p, lo, hi = boot_ratio(n_after2, n_h)
    after.append({"range": name, "seeds": rng_label, "side": "hidden",
                  "measure": "share of hard incidents with onset at or after the last change", "value": f(p), "n": int(n_h.sum())})
    after.append({"range": name, "seeds": rng_label, "side": "hidden", "measure": "  90% interval (last change)",
                  "value": f"[{f(lo)}, {f(hi)}]", "n": ""})
    ss = reg[reg["kind"] == "signature_shift"]
    for (k, e), n in ss.groupby(["kind_a", "kind_b"]).size().items():
        kinds.append({"range": name, "seeds": rng_label, "side": "hidden", "shifted_kind": k, "now_emits": e, "streams": int(n)})
    return eff, after, kinds


# ---- item 4: hidden edges -------------------------------------------------------------------


def item4(name: str, t: dict):
    inc, seeds, rng_label = t["inc"], t["seeds"], C.seed_range(t["seeds"])
    delays, edge, pairs = [], [], []
    d = inc[(inc["family"].isin(["cascade", "split_brain"])) & inc["other_first_alarm_ns"].notna()
            & inc["site_first_alarm_ns"].notna()].copy()
    d["delay_ms"] = (d["other_first_alarm_ns"] - d["site_first_alarm_ns"]) / 1e6
    for (tier, fam, mode, dep), g in d.groupby(["tier", "family", "mode", "other_is_dependent"]):
        q = quantiles(g["delay_ms"])
        delays.append({"range": name, "seeds": rng_label, "side": "hidden", "tier": tier, "family": fam, "mode": mode,
                       "partner_is_dependent": "" if pd.isna(dep) else int(dep), "n": len(g),
                       "unit": "s" if mode == "mimic" else "ms",
                       "mean": f(g["delay_ms"].mean() / (1000 if mode == "mimic" else 1), 2),
                       "min": f(q[0] / (1000 if mode == "mimic" else 1), 2),
                       "p10": f(q[1] / (1000 if mode == "mimic" else 1), 2), "p50": f(q[3] / (1000 if mode == "mimic" else 1), 2),
                       "p90": f(q[5] / (1000 if mode == "mimic" else 1), 2), "max": f(q[6] / (1000 if mode == "mimic" else 1), 2)})
    # Cascades and split brains that never showed the partner (a decoy of a mimic).
    nopartner = inc[(inc["family"].isin(["cascade", "split_brain"])) & inc["other_first_alarm_ns"].isna()]
    for (tier, fam, mode), g in nopartner.groupby(["tier", "family", "mode"]):
        delays.append({"range": name, "seeds": rng_label, "side": "hidden", "tier": tier, "family": fam, "mode": mode,
                       "partner_is_dependent": "", "n": len(g), "unit": "no partner alarm", "mean": "", "min": "",
                       "p10": "", "p50": "", "p90": "", "max": ""})
    # EdgeAdd instants and incidents that alarm over the new edge.
    reg = t["reg"]
    ea = reg[reg["kind"] == "edge_add"]
    n_streams = len(seeds)
    post = inc["onset_ns"] >= inc["t_ea"]
    for tier in ("all", "plain", "hard", "decoy"):
        m = inc["tier"].notna() if tier == "all" else inc["tier"] == tier
        n_post = per_stream(inc, m & post, seeds)
        n_alt = per_stream(inc, m & post & (inc["edge_altered"] == 1), seeds)
        n_exp = per_stream(inc, m & post & (inc["edge_exposed"] == 1), seeds)
        p, lo, hi = boot_ratio(n_alt)
        edge.append({"range": name, "seeds": rng_label, "side": "hidden", "tier": tier,
                     "edge_add_at_s": f(ea["at_ns"].iloc[0] / C.NS, 0), "streams_with_an_edge": int((ea.groupby("seed").size() > 0).sum()),
                     "incidents_after_per_stream": f(n_post.mean()), "exposed_per_stream": f(n_exp.mean()),
                     "alarm_over_new_edge_per_stream": f(p), "lo90": f(lo), "hi90": f(hi),
                     "alarm_over_new_edge_total": int(n_alt.sum()),
                     "share_of_after": f(n_alt.sum() / max(n_post.sum(), 1))})
    # Pair recurrence.
    st = t["streams"].set_index("seed")
    for fam in ("cascade", "split_brain"):
        for tier in ("hard", "decoy"):
            g = inc[(inc["family"] == fam) & (inc["tier"] == tier) & inc["other"].notna()]
            if g.empty:
                continue
            same_ordered = same_unordered = same_root = 0
            for _, gg in g.groupby("seed"):
                seen_o, seen_u, seen_r = set(), set(), set()
                for r in gg.itertuples():
                    o = (r.site, int(r.other))
                    u = frozenset(o)
                    same_ordered += o in seen_o
                    same_unordered += u in seen_u
                    same_root += r.site in seen_r
                    seen_o.add(o), seen_u.add(u), seen_r.add(r.site)
            n_i = per_stream(g, g["tier"].notna(), seeds)
            n_flag = per_stream(g, g["is_rec"], seeds)
            pairs.append({"range": name, "seeds": rng_label, "side": "hidden", "family": fam, "tier": tier,
                          "incidents": int(n_i.sum()), "per_stream": f(n_i.mean()),
                          "flagged_recurrences": int(n_flag.sum()),
                          "pair_seen_earlier_ordered": int(same_ordered), "pair_seen_earlier_unordered": int(same_unordered),
                          "root_seen_earlier": int(same_root),
                          "share_pair_seen_earlier": f(same_ordered / n_i.sum()),
                          "share_flagged": f(n_flag.sum() / n_i.sum()),
                          "unflagged_pair_repeats": int(same_ordered - n_flag.sum()),
                          "mean_incomparable_pairs_per_graph": f(st.loc[seeds, "incomparable_pairs"].mean(), 2)})
    return delays, edge, pairs


# ---- item 3: the vocabulary -----------------------------------------------------------------


def _mi_bits(x: np.ndarray, y: np.ndarray) -> float:
    n = len(x)
    if n == 0:
        return 0.0
    xi = np.unique(x, return_inverse=True)[1]
    yi = np.unique(y, return_inverse=True)[1]
    tab = np.zeros((xi.max() + 1, yi.max() + 1))
    np.add.at(tab, (xi, yi), 1)
    p = tab / n
    px, py = p.sum(1, keepdims=True), p.sum(0, keepdims=True)
    nz = p > 0
    return float((p[nz] * np.log2(p[nz] / (px @ py)[nz])).sum())


def _entropy_bits(y: np.ndarray) -> float:
    _, c = np.unique(y, return_counts=True)
    p = c / c.sum()
    return float(-(p * np.log2(p)).sum())


def _vocab_sample(rows: pd.DataFrame, label_col: str, cls: str, exclude_none: bool):
    r = rows[rows["class"] == cls]
    if exclude_none:
        r = r[r[label_col] != "none"]
    ids = np.repeat(r["text_id"].to_numpy(), r["count"].to_numpy())
    lab = np.repeat(r[label_col].to_numpy(), r["count"].to_numpy())
    return ids, lab


def item3(name: str, t: dict) -> tuple[list[dict], list[dict], list[dict]]:
    vocab, seeds, rng_label = t["vocab"], t["seeds"], C.seed_range(t["seeds"])
    samples = [
        ("incident-borne", "incident-borne (hard incidents' out-of-catalogue messages)", "any_live_hard_family", "incident", False),
        ("background-any", "background free-form, labelled by the hard family live anywhere", "any_live_hard_family", "background", True),
        ("background-here", "background free-form, labelled by the hard family live at its service", "here_live_hard_family", "background", True),
    ]
    out, prec, per_stream_rows = [], [], []
    by_seed = {s: g for s, g in vocab.groupby("seed")}
    for short, sname, col, cls, excl in samples:
        stats = []
        for s in seeds:
            g = by_seed.get(s)
            if g is None:
                continue
            ids, lab = _vocab_sample(g, col, cls, excl)
            n = len(ids)
            if n < 2 or len(np.unique(lab)) < 2:
                stats.append(None)
                continue
            mi = _mi_bits(ids, lab)
            h = _entropy_bits(lab)
            # A fixed generator per (stream, sample): the permutation null replays exactly.
            rng = np.random.default_rng([C.BOOT_SEED, int(s), 1 if cls == "incident" else 2, 1 if col.startswith("here") else 0])
            perm = np.array([_mi_bits(ids, rng.permutation(lab)) for _ in range(PERMUTATIONS)])
            # Share of ids at exactly one family: all ids, and ids seen at least twice in the sample.
            u, inv, cnt = np.unique(ids, return_inverse=True, return_counts=True)
            fam_per_id = np.array([len(set(lab[inv == i])) for i in range(len(u))])
            one_all = float((fam_per_id == 1).mean())
            multi = cnt >= 2
            one_multi = float((fam_per_id[multi] == 1).mean()) if multi.any() else float("nan")
            stats.append((n, len(u), len(np.unique(lab)), mi, h, perm.mean(), np.quantile(perm, 0.95), one_all, one_multi,
                          int(multi.sum())))
            per_stream_rows.append({"range": name, "seeds": rng_label, "side": "public ids, hidden labels",
                                    "sample": short, "seed": int(s), "messages": n, "distinct_ids": len(u),
                                    "labels": len(np.unique(lab)), "mi_bits": f(mi), "label_entropy_bits": f(h),
                                    "mi_permutation_mean_bits": f(perm.mean()), "mi_permutation_p95_bits": f(np.quantile(perm, 0.95)),
                                    "share_ids_at_exactly_one_label": f(one_all)})
        used = [x for x in stats if x is not None]
        arr = np.array(used, dtype=float) if used else np.zeros((0, 10))

        def m(col_i):
            return float(np.nanmean(arr[:, col_i])) if len(arr) else float("nan")

        def ci(vals):
            if len(vals) < 2:
                return "", ""
            _, lo, hi = boot_ratio(np.asarray(vals, dtype=float))
            return f(lo), f(hi)

        excess = arr[:, 3] - arr[:, 5] if len(arr) else np.array([])
        norm = excess / arr[:, 4] if len(arr) else np.array([])
        elo, ehi = ci(excess)
        out.append({
            "range": name, "seeds": rng_label, "side": "public ids, hidden labels", "sample": sname,
            "streams_with_at_least_two_labels": len(used), "streams": len(seeds),
            "messages_per_stream": f(m(0), 1), "distinct_ids_per_stream": f(m(1), 1), "labels_per_stream": f(m(2), 2),
            "mi_bits": f(m(3)), "label_entropy_bits": f(m(4)), "mi_permutation_mean_bits": f(m(5)),
            "mi_permutation_p95_bits": f(m(6)), "mi_excess_bits": f(excess.mean() if len(excess) else float("nan")),
            "excess_lo90": elo, "excess_hi90": ehi, "excess_over_label_entropy": f(norm.mean() if len(norm) else float("nan")),
            "share_ids_at_exactly_one_label": f(m(7)), "share_ids_seen_twice_at_one_label": f(m(8)),
            "ids_seen_twice_per_stream": f(m(9), 2),
        })
    # What the id alone is worth in the whole public stream: among free-form messages whose id is
    # one of the stream's hard vocabulary (an id some hard incident of the stream carried), how many
    # are incident-borne, by the public cue (abnormal counter readings at the message's service in
    # the previous 10 s, at most 3).
    levels = [("any free-form message with a vocabulary id", 0), ("... with at least 1 abnormal reading at its service in the previous 10 s", 1),
              ("... with at least 2", 2), ("... with 3 or more", 3)]
    cnt_inc = {k: np.zeros(len(seeds)) for _, k in levels}
    cnt_bg = {k: np.zeros(len(seeds)) for _, k in levels}
    for i, s in enumerate(seeds):
        g = by_seed.get(s)
        if g is None:
            continue
        v = set(g.loc[g["class"] == "incident", "text_id"])
        if not v:
            continue
        gi = g[g["text_id"].isin(v)]
        inc_m = gi["class"] == "incident"
        bg_m = gi["class"] == "background"
        for _, k in levels:
            cue_ok = gi["cue"] >= k
            cnt_inc[k][i] = gi.loc[inc_m & cue_ok, "count"].sum()
            cnt_bg[k][i] = gi.loc[bg_m & cue_ok, "count"].sum()
    for label, k in levels:
        ni, nb = cnt_inc[k], cnt_bg[k]
        p, lo, hi = boot_ratio(ni, ni + nb)
        prec.append({"range": name, "seeds": rng_label, "side": "public ids, hidden labels", "messages": label,
                     "incident_borne": int(ni.sum()), "background": int(nb.sum()), "precision_incident_borne": f(p),
                     "lo90": f(lo), "hi90": f(hi), "recall_of_incident_borne": f(ni.sum() / cnt_inc[0].sum())})
    return out, prec, per_stream_rows


# ---- site collisions: what a weak key would recall wrongly ----------------------------------


def collisions(name: str, t: dict) -> list[dict]:
    """The exposure of a memory that keys on the site alone: recall the family and mode of the most
    recent hard incident at the incident's site. Hidden-side upper bound (an arm only knows an
    earlier incident it diagnosed). Within a stream, and across earlier streams of the range in
    seed order (service ids are numbered per stream, so across streams a site id means nothing)."""
    inc, seeds, rng_label = t["inc"], t["seeds"], C.seed_range(t["seeds"])
    rows = []
    by_seed = {s: g for s, g in inc.groupby("seed")}
    cross: dict[int, str] = {}
    res = {"within": [], "cross": []}
    for k, s in enumerate(seeds):
        g = by_seed.get(s)
        if g is None:
            continue
        within: dict[int, str] = {}
        for r in g.itertuples():
            for scope, memory in (("within", within), ("cross", cross)):
                if scope == "cross" and k < 10:
                    continue  # the first ten streams only fill the memory
                recalled = memory.get(r.site)
                res[scope].append((r.tier, recalled is not None, recalled is not None and r.tier == "hard" and recalled == r.fam_mode,
                                   recalled is not None and r.tier == "hard" and recalled == r.fam_mode and r.is_rec))
            if r.tier == "hard":
                within[r.site] = r.fam_mode
        for r in g.itertuples():
            if r.tier == "hard":
                cross[r.site] = r.fam_mode  # stream s's own hard incidents enter only after the stream
    for scope in ("within", "cross"):
        df = pd.DataFrame(res[scope], columns=["tier", "recalled", "right", "right_recurrence"])
        for tier in ("all", "plain", "hard", "decoy"):
            d = df if tier == "all" else df[df["tier"] == tier]
            if d.empty:
                continue
            memory = ("site-only, most recent hard incident, same stream" if scope == "within"
                      else "site-only, most recent hard incident in earlier streams (after the first 10)")
            rows.append({"range": name, "seeds": rng_label, "side": "hidden", "memory": memory, "incident_tier": tier,
                         "incidents": len(d), "site_in_memory": int(d["recalled"].sum()),
                         "share_site_in_memory": f(d["recalled"].mean()),
                         "recalled_right": int(d["right"].sum()), "recalled_wrong": int((d["recalled"] & ~d["right"]).sum()),
                         "wrong_share_of_recalls": f(((d["recalled"] & ~d["right"]).sum()) / max(d["recalled"].sum(), 1)),
                         "right_that_are_recurrences": int(d["right_recurrence"].sum())})
    return rows


def phase1_class(r) -> str:
    """The class of an incident's first seconds as the public rules see them (code-derived, from
    `present.rs` and `incident.rs`, not measured): a hard incident or decoy that mimics presents
    exactly as a plain incident of the imitated kind; a decoy presents as the hard family it pretends
    to be, until it resolves. Equal classes are indistinguishable by the evidence of phase 1."""
    if r.tier == "plain":
        return "duo" if r.duo == 1 else f"id:{r.known_kind}"
    if r.family == "compound":
        return f"id:{r.kind_a}" if r.mode == "mimic" else "union:" + "+".join(sorted([r.kind_a, r.kind_b]))
    if r.family == "cascade":
        return f"id:{r.kind_a}" if r.mode == "mimic" else f"cascade:{r.kind_a}+partner"
    if r.family == "split_brain":
        return "mixed" if r.mode == "mimic" else "mixed+peer"
    return "leak"


def truth_class(r) -> str:
    """What is true of an incident, without its site: a hard family, `none` for a decoy, or the
    known kind of a plain incident."""
    if r.tier == "decoy":
        return "none"
    return f"plain:{r.known_kind}" if r.tier == "plain" else f"hard:{r.family}"


def phase1_collisions(name: str, t: dict) -> list[dict]:
    """A memory that fires on the evidence of phase 1 (before the phase-2 evidence arrives), on the
    reasoner's answer for the most recent earlier hard incident or decoy of the same key, within a
    stream. Site-keyed form (site and class) and family-keyed form (class only, the site taken from
    the new incident). Right when the recalled truth equals the new incident's."""
    inc, seeds, rng_label = t["inc"], t["seeds"], C.seed_range(t["seeds"])
    res = {"site+class": [], "class only": []}
    for _, g in inc.groupby("seed"):
        mem_site: dict[tuple, str] = {}
        mem_class: dict[str, str] = {}
        for r in g.itertuples():
            pc, tc = phase1_class(r), truth_class(r)
            ks = (r.site, pc)
            for form, mem, key in (("site+class", mem_site, ks), ("class only", mem_class, pc)):
                rec = mem.get(key)
                res[form].append((r.tier, rec is not None, rec is not None and rec == tc))
            if r.tier in ("hard", "decoy"):
                mem_site[ks] = tc
                mem_class[pc] = tc
    rows = []
    for form, data in res.items():
        df = pd.DataFrame(data, columns=["tier", "recalled", "right"])
        for tier in ("all", "plain", "hard", "decoy"):
            d = df if tier == "all" else df[df["tier"] == tier]
            rows.append({"range": name, "seeds": rng_label, "side": "hidden",
                         "memory": f"phase-1 {form}, most recent hard incident or decoy, same stream",
                         "incident_tier": tier, "incidents": len(d), "recalled": int(d["recalled"].sum()),
                         "share_recalled": f(d["recalled"].mean()), "recalled_right": int(d["right"].sum()),
                         "recalled_wrong": int((d["recalled"] & ~d["right"]).sum()),
                         "wrong_share_of_recalls": f((d["recalled"] & ~d["right"]).sum() / max(d["recalled"].sum(), 1)),
                         "wrong_per_stream": f((d["recalled"] & ~d["right"]).sum() / len(seeds))})
    return rows


# ---- driver ---------------------------------------------------------------------------------


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--hidden-root", type=pathlib.Path, default=C.HIDDEN)
    ap.add_argument("--out-dir", type=pathlib.Path, default=C.OUT)
    args = ap.parse_args(argv)

    tables: dict[str, list[dict]] = {k: [] for k in (
        "recurrence-summary", "recurrence-eligibility", "recurrence-gaps", "experience-curve",
        "experience-stream-order", "stale", "family-elsewhere", "regime-effects", "regime-hard-after",
        "regime-shifted-kinds", "cascade-delays", "edgeadd", "pair-recurrence", "vocabulary",
        "vocabulary-precision", "vocabulary-perstream", "site-collisions", "phase1-collisions", "perstream", "recurrences")}
    for name in C.RANGES:
        t = prepare(C.read_range(args.hidden_root, name))
        world_b = C.RANGES[name]["world"] == "b"
        print(f"{name}: seeds {C.seed_range(t['seeds'])}, {len(t['inc'])} incidents, {len(t['rec'])} recurrences")
        tables["recurrence-summary"] += item1_summary(name, t)
        tables["perstream"] += per_stream_table(name, t)
        tables["recurrences"] += recurrence_table(name, t)
        tables["recurrence-eligibility"].append(item1_eligibility(name, t))
        tables["recurrence-gaps"] += item1_gaps(name, t)
        tables["experience-curve"] += item1_curve(name, t)
        tables["experience-stream-order"] += item1_stream_order(name, t)
        tables["stale"] += item1_stale(name, t)
        tables["family-elsewhere"] += item2(name, t)
        eff, after, kinds = item5(name, t)
        tables["regime-effects"] += eff
        tables["regime-hard-after"] += after
        tables["regime-shifted-kinds"] += kinds
        if world_b:
            continue
        d, e, p = item4(name, t)
        tables["cascade-delays"] += d
        tables["edgeadd"] += e
        tables["pair-recurrence"] += p
        v, pr, ps = item3(name, t)
        tables["vocabulary"] += v
        tables["vocabulary-perstream"] += ps
        tables["vocabulary-precision"] += pr
        tables["site-collisions"] += collisions(name, t)
        tables["phase1-collisions"] += phase1_collisions(name, t)
    for k, rows in tables.items():
        C.write_csv(args.out_dir / f"w2-{k}.csv", rows)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
