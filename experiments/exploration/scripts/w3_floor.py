#!/usr/bin/env python3
"""W3 item 2 (hidden side): the collision floor of keys that wait for the evidence of phase 2.

Usage: w3_floor.py [--hidden-root DIR] [--out-dir DIR] [--e1-run DIR] [--ranges a-tune,a-heldout,c-tune,c-heldout]

W2's section 9 asked what a key read in the first seconds recalls wrongly when the memory fires on
every match. This asks the same of keys that include the evidence that arrives later, and of the
key forms E1 and A1c actually built. Everything is computed from `floor-rows.csv` and
`floor-incidents.csv` (`crates/gordian-stream/examples/laws/floor.rs`): for each incident, from its
own observations only (public bytes, segmented by the hidden label: the best case for any key), the
invariant features at a cutoff after its first abnormal observation. Side: hidden. The memory is a
table from a key to the truth of the most recent incident bound at that key; "right" is a recalled
truth equal to the target's (a hard family, `none` for a decoy, the plain kind for a plain
incident); a recall is *wrong* otherwise, so the collision share is wrong recalls over all recalls
and every source is a right source (the reasoner's own error, W2's inherited error, is not here).

Three families of tables:

- **ladder** (`w3-floor-ladder.csv`): invariant keys of increasing richness (`K1` the kinds at the
  site and the anchor's, as E1's `kinds`; `K2` adds the kinds at services outside the site's
  dependents, how many, and the public checker's verdict; `K3` adds the counter bands and A1c's
  partner relation; `K4` adds whether the site's readings have been benign five in a row, the
  public decoy rule) read at cutoffs 1 to 32 s after the first abnormal observation, in the site-
  keyed and family-keyed forms, with and without the stream reset, with a teacher of hard
  incidents and decoys (W2's) or hard incidents only (what the selection oracle asks).
- **E1 forms** (`w3-floor-e1.csv`): E1's keys as `key_of` builds them (kinds, bands, timing) at the
  snapshot (the public checker finds no hypothesis on the attached evidence, read one second
  later), site-keyed and family-keyed, reset and carried; sources are hard incidents with a
  snapshot, binding at the instant their answer would arrive (`LAG`) or at once (W2's reading).
- **checks**: my simulator against W2's published phase-1 table (code-derived classes), and my
  data-derived phase-1 partition against W2's code-derived one.
"""

from __future__ import annotations

import argparse
import pathlib

import numpy as np
import pandas as pd

import w2_common as C
import w2_laws as L
from w2_common import boot_ratio, f

ROOT = C.ROOT
HIDDEN = ROOT / "artifacts" / "runs" / "w3" / "hidden"
OUT = C.OUT
NS = 1_000_000_000
CUTS = [1, 2, 3, 4, 6, 8, 10, 13, 16, 20, 24, 32]
LAG_NS = 19 * NS  # the arm asks 16 s after the notice and the answer takes about 2 s (L1's arm)
DECOY_STREAK = 5  # the public decoy rule: five benign readings in a row at the site
LEVELS = ["K1", "K2", "K3", "K4"]


# ---- loading --------------------------------------------------------------------------------


def truth_class(r) -> str:
    if r.tier == "decoy":
        return "none"
    return f"plain:{r.known_kind}" if r.tier == "plain" else f"hard:{r.family}"


def load(root: pathlib.Path, name: str) -> dict:
    t = L.prepare(C.read_range(root, name))
    inc = t["inc"].copy()
    inc["tc"] = [truth_class(r) for r in inc.itertuples()]
    fr = pd.read_csv(root / name / "floor-rows.csv", keep_default_na=False, dtype=str)
    fi = pd.read_csv(root / name / "floor-incidents.csv", keep_default_na=False, na_values=[""])
    for col in ("seed", "incident", "cut_ns", "oth_n", "streak", "ff_own", "n_abn"):
        fr[col] = fr[col].astype(np.int64)
    fr["gate_delay_ns"] = pd.to_numeric(fr["gate_delay_ns"], errors="coerce")
    inc = inc.merge(fi[["seed", "incident", "t0_ns", "gate_att_ns", "gate_all_ns"]], on=["seed", "incident"], how="left",
                    validate="1:1")
    assert len(fr[fr["cut"] != "snap"]) == len(CUTS) * int(inc["t0_ns"].notna().sum()), "a cutoff row is missing"
    t["inc"], t["fr"] = inc, fr
    return t


def gate_band(delay_ns: float) -> int:
    d = delay_ns
    return 0 if d < NS else 1 if d < 3 * NS else 2 if d < 6 * NS else 3 if d < 10 * NS else 4


# ---- keys -----------------------------------------------------------------------------------


def ladder_key(r, level: str) -> str:
    """The invariant key of one floor row at ladder level K1..K4 (no service id, no message id)."""
    k = f"{r.site_tags}|{r.anchor}"
    if level == "K1":
        return k
    k += f"|{r.oth_tags}|{r.oth_n}|{r.verdict_all}"
    if level == "K2":
        return k
    k += f"|{r.bands}|{r.partner}"
    if level == "K3":
        return k
    return k + f"|{int(r.streak >= DECOY_STREAK)}"


def e1_key(r, level: str) -> str:
    """E1's key at its levels (`noticer_record::key_of`): kinds and anchor; bands; the gate band."""
    k = f"{r.site_tags}|{r.anchor}"
    if level in ("bands", "timing"):
        k += f"|{r.bands}"
    if level == "timing":
        k += f"|g{gate_band(r.gate_delay_ns)}"
    return k


# ---- the memory -----------------------------------------------------------------------------


def simulate(inc: pd.DataFrame, seeds, keyed: dict, bind_at: dict, look_at: dict, form: str, carried: bool,
             teacher: tuple[str, ...], tau: float | None = None) -> pd.DataFrame:
    """Run the memory over the range in seed order. `keyed[(seed, incident)]` is the key (an incident
    without one neither binds nor looks up); `bind_at` and `look_at` give the time (any monotone
    number) at which an incident's answer enters the table and at which its key is looked up, within
    its stream. A lookup reads the most recent answer bound at its key before it, by another incident
    (an incident's own answer is not available to its own recall); with `carried` a key with no such
    answer in the stream falls back to the last answer bound at it in an earlier stream of the range.
    With `tau` the memory aggregates instead of keeping the last answer: it holds every answer bound at the
    key (before the lookup, by another incident) and recalls the class with the most answers only when its
    share of them is strictly above `tau` (a tie, or a leader at or below `tau`, is no recall): the
    aggregation W2's section 10 asks of any arm, and the policy under which one wrong source cannot make a key
    wrong for good. `tau = None` is the last-answer rule E1's rung uses.
    Returns one row per looked-up incident: seed, incident, tier, tc, recalled, right, is_rec."""
    by_seed = {s: g for s, g in inc.groupby("seed", sort=False)}
    is_rec = dict(zip(zip(inc["seed"], inc["incident"]), inc["is_rec"]))
    carry: dict = {}
    carry_list: dict = {}
    out = []
    for s in seeds:
        g = by_seed.get(s)
        if g is None:
            continue
        binds: dict[str, list] = {}
        looks = []
        for r in g.itertuples():
            k = keyed.get((s, r.incident))
            if k is None:
                continue
            site = f"s{r.site}|" if form == "site" else ""
            k = site + k
            if r.tier in teacher:
                binds.setdefault(k, []).append((bind_at[(s, r.incident)], r.incident, r.tc))
            looks.append((r.incident, k, look_at[(s, r.incident)], r.tc, r.tier))
        for lst in binds.values():
            lst.sort(key=lambda b: (b[0], b[1]))
        for i, k, t, tc, tier in looks:
            rec = None
            if tau is None:
                for bt, bi, btc in reversed(binds.get(k, ())):
                    if bt <= t and bi != i:
                        rec = btc
                        break
                if rec is None and carried:
                    rec = carry.get(k)
            else:
                held = list(carry_list.get(k, ())) if carried else []
                held += [btc for bt, bi, btc in binds.get(k, ()) if bt <= t and bi != i]
                if held:
                    counts = {}
                    for c in held:
                        counts[c] = counts.get(c, 0) + 1
                    top = max(counts.values())
                    leaders = [c for c, n in counts.items() if n == top]
                    if len(leaders) == 1 and top / len(held) > tau:
                        rec = leaders[0]
            out.append((s, i, tier, tc, rec is not None, rec is not None and rec == tc, bool(is_rec[(s, i)])))
        if carried:
            for k, lst in binds.items():
                carry[k] = lst[-1][2]
                carry_list.setdefault(k, []).extend(b[2] for b in lst)
    return pd.DataFrame(out, columns=["seed", "incident", "tier", "tc", "recalled", "right", "is_rec"])


def summarise(res: pd.DataFrame, inc: pd.DataFrame, seeds, extra: dict) -> dict:
    """The table row for one simulated setting, with 90% cluster-bootstrap intervals."""
    seeds = np.asarray(seeds)
    n_streams = len(seeds)

    def ps(mask, df=res):
        s = df.loc[mask].groupby("seed").size()
        return s.reindex(seeds, fill_value=0).to_numpy(float)

    recalls = ps(res["recalled"])
    wrong = ps(res["recalled"] & ~res["right"])
    right = ps(res["right"])
    hard = res["tier"] == "hard"
    n_hard_all = inc[inc["tier"] == "hard"].groupby("seed").size().reindex(seeds, fill_value=0).to_numpy(float)
    hard_rec = inc[(inc["tier"] == "hard") & inc["is_rec"]].groupby("seed").size().reindex(seeds, fill_value=0).to_numpy(float)
    hard_right = ps(res["right"] & hard)
    hard_rec_right = ps(res["right"] & hard & res["is_rec"])
    row = dict(extra)
    row.update({
        "streams": n_streams, "recalls": int(recalls.sum()), "right": int(right.sum()), "wrong": int(wrong.sum()),
        "recalls_on_plain": int(ps(res["recalled"] & (res["tier"] == "plain")).sum()),
        "recalls_on_decoy": int(ps(res["recalled"] & (res["tier"] == "decoy")).sum()),
        "recalls_on_hard": int(ps(res["recalled"] & hard).sum()),
        "wrong_on_plain": int(ps(res["recalled"] & ~res["right"] & (res["tier"] == "plain")).sum()),
        "wrong_on_decoy": int(ps(res["recalled"] & ~res["right"] & (res["tier"] == "decoy")).sum()),
        "wrong_on_hard": int(ps(res["recalled"] & ~res["right"] & hard).sum()),
    })
    p, lo, hi = boot_ratio(wrong, recalls)
    row.update({"wrong_share": f(p), "wrong_share_lo90": f(lo), "wrong_share_hi90": f(hi)})
    p, lo, hi = boot_ratio(wrong)
    row.update({"wrong_per_stream": f(p), "wrong_per_stream_lo90": f(lo), "wrong_per_stream_hi90": f(hi)})
    p, lo, hi = boot_ratio(hard_right, n_hard_all)
    row.update({"hard_recalled_right": int(hard_right.sum()), "hard_incidents": int(n_hard_all.sum()),
                "hard_recalled_right_share": f(p), "hard_right_lo90": f(lo), "hard_right_hi90": f(hi)})
    p, lo, hi = boot_ratio(hard_rec_right, hard_rec)
    row.update({"hard_recurrences": int(hard_rec.sum()), "hard_recurrences_recalled_right": int(hard_rec_right.sum()),
                "hard_recurrences_recalled_right_share": f(p), "hrr_lo90": f(lo), "hrr_hi90": f(hi)})
    return row


# ---- ladder ---------------------------------------------------------------------------------


def ladder(name: str, t: dict, cuts=CUTS) -> list[dict]:
    inc, fr, seeds = t["inc"], t["fr"], t["seeds"]
    label = C.seed_range(seeds)
    rows = []
    arrival = {(r.seed, r.incident): r.incident for r in inc.itertuples()}
    start = {(r.seed, r.incident): r.t0_ns for r in inc.itertuples()}
    for c in cuts:
        sub = fr[fr["cut"] == str(c)]
        for level in LEVELS:
            keys = {(r.seed, r.incident): ladder_key(r, level) for r in sub.itertuples()}
            for form in ("site", "family"):
                for carried in (False, True):
                    if form == "site" and carried:
                        continue  # a site id means another service in the next stream: by construction wrong, E1 carries it as a labelled sensitivity
                    for teacher, tlab in ((("hard", "decoy"), "hard+decoy"), (("hard",), "hard")):
                        for mode in ("arrival", "answered"):
                            if mode == "arrival":
                                bind = {k: arrival[k] + 0.5 for k in keys}
                                look = {k: arrival[k] for k in keys}
                            else:
                                bind = {k: start[k] + LAG_NS for k in keys}
                                look = {k: start[k] + c * NS for k in keys}
                            res = simulate(inc, seeds, keys, bind, look, form, carried, teacher)
                            rows.append(summarise(res, inc, seeds, {
                                "range": name, "seeds": label, "side": "hidden", "cutoff_s": c, "key": level, "form": form,
                                "stream_reset": "no" if carried else "yes", "teacher": tlab,
                                "source_available": "at arrival order (W2)" if mode == "arrival" else "after its answer (19 s)"}))
    return rows


VOTE_TAUS = (0.5, 0.8)
VOTE_CUTS = [6, 10, 13, 16, 32]


def vote(name: str, t: dict) -> list[dict]:
    """The same memories with an aggregating rule (`tau`), family-keyed, hard teacher, sources after their answer:
    the ladder at a few cutoffs and E1's keys."""
    inc, fr, seeds = t["inc"], t["fr"], t["seeds"]
    label = C.seed_range(seeds)
    start = {(r.seed, r.incident): r.t0_ns for r in inc.itertuples()}
    rows = []
    settings = []
    for c in VOTE_CUTS:
        sub = fr[fr["cut"] == str(c)]
        for level in LEVELS:
            settings.append((f"ladder {level}", c, {(r.seed, r.incident): ladder_key(r, level) for r in sub.itertuples()}, {k: start[k] + c * NS for k in start}))
    snap = fr[fr["cut"] == "snap"]
    snap_at = {(r.seed, r.incident): r.cut_ns for r in snap.itertuples()}
    for level in ("kinds", "timing"):
        settings.append((f"E1 {level}", "snap", {(r.seed, r.incident): e1_key(r, level) for r in snap.itertuples()}, {k: start[k] + snap_at[k] for k in snap_at}))
    for key, c, keys, look_all in settings:
        look = {k: look_all[k] for k in keys}
        bind = {k: start[k] + LAG_NS for k in keys}
        for form in ("site", "family"):
            for carried in (False, True):
                if form == "site" and carried:
                    continue
                for tau in (None,) + VOTE_TAUS:
                    res = simulate(inc, seeds, keys, bind, look, form, carried, ("hard",), tau)
                    rows.append(summarise(res, inc, seeds, {
                        "range": name, "seeds": label, "side": "hidden", "cutoff_s": c, "key": key, "form": form,
                        "stream_reset": "no" if carried else "yes", "teacher": "hard", "rule": "last answer" if tau is None else f"vote, leader above {tau}"}))
    return rows


def who(name: str, t: dict) -> list[dict]:
    """Who the wrong recalls are: by tier, family and mode of the incident that recalled wrongly, for a few
    settings (family-keyed, hard teacher, sources after their answer)."""
    inc, fr, seeds = t["inc"], t["fr"], t["seeds"]
    label = C.seed_range(seeds)
    start = {(r.seed, r.incident): r.t0_ns for r in inc.itertuples()}
    info = inc.set_index(["seed", "incident"])[["tier", "family", "mode", "known_kind", "sig_altered", "edge_altered"]]
    rows = []
    settings = []
    for c, level in ((6, "K2"), (16, "K2"), (16, "K4"), (32, "K4")):
        sub = fr[fr["cut"] == str(c)]
        settings.append((f"ladder {level} at {c} s", {(r.seed, r.incident): ladder_key(r, level) for r in sub.itertuples()},
                         {(r.seed, r.incident): start[(r.seed, r.incident)] + c * NS for r in sub.itertuples()}))
    snap = fr[fr["cut"] == "snap"]
    settings.append(("E1 timing at the snapshot", {(r.seed, r.incident): e1_key(r, "timing") for r in snap.itertuples()},
                     {(r.seed, r.incident): start[(r.seed, r.incident)] + r.cut_ns for r in snap.itertuples()}))
    for lab, keys, look in settings:
        bind = {k: start[k] + LAG_NS for k in keys}
        for carried in (False, True):
            res = simulate(inc, seeds, keys, bind, look, "family", carried, ("hard",))
            w = res[res["recalled"] & ~res["right"]].join(info, on=["seed", "incident"], rsuffix="_i")
            g = w.groupby(["tier_i" if "tier_i" in w else "tier", "family", "mode"]).size()
            for (tier, fam, mode), n in g.items():
                rows.append({"range": name, "seeds": label, "side": "hidden", "setting": lab, "stream_reset": "no" if carried else "yes",
                             "wrong_recalls_on": f"{tier}/{fam}/{mode}" if fam else str(tier), "wrong": int(n)})
            pl = w[w["tier"] == "plain"] if "tier_i" not in w else w[w["tier_i"] == "plain"]
            for lab2, m in (("plain altered by the signature shift", pl["sig_altered"] == 1), ("plain altered by the added edge", pl["edge_altered"] == 1),
                            ("plain unaltered", (pl["sig_altered"] == 0) & (pl["edge_altered"] == 0))):
                rows.append({"range": name, "seeds": label, "side": "hidden", "setting": lab, "stream_reset": "no" if carried else "yes",
                             "wrong_recalls_on": lab2, "wrong": int(m.sum())})
    return rows


def e1_forms(name: str, t: dict) -> list[dict]:
    inc, fr, seeds = t["inc"], t["fr"], t["seeds"]
    label = C.seed_range(seeds)
    snap = fr[fr["cut"] == "snap"]
    snap_at = {(r.seed, r.incident): r.cut_ns for r in snap.itertuples()}
    start = {(r.seed, r.incident): r.t0_ns for r in inc.itertuples()}
    arrival = {(r.seed, r.incident): r.incident for r in inc.itertuples()}
    rows = []
    for level in ("kinds", "bands", "timing"):
        keys = {(r.seed, r.incident): e1_key(r, level) for r in snap.itertuples()}
        for form in ("site", "family"):
            for carried in (False, True):
                for mode in ("arrival", "answered"):
                    if mode == "arrival":
                        bind = {k: arrival[k] + 0.5 for k in keys}
                        look = {k: arrival[k] for k in keys}
                    else:
                        bind = {k: start[k] + LAG_NS for k in keys}
                        look = {k: start[k] + snap_at[k] for k in keys}
                    res = simulate(inc, seeds, keys, bind, look, form, carried, ("hard",))
                    rows.append(summarise(res, inc, seeds, {
                        "range": name, "seeds": label, "side": "hidden", "key": f"E1 {level}", "form": form,
                        "stream_reset": "no" if carried else "yes", "teacher": "hard",
                        "source_available": "at arrival order (W2)" if mode == "arrival" else "after its answer (19 s)"}))
    return rows


# ---- checks ---------------------------------------------------------------------------------


def w2_replication(name: str, t: dict) -> list[dict]:
    """My simulator on W2's code-derived phase-1 classes against `w2_laws.phase1_collisions`."""
    inc, seeds = t["inc"], t["seeds"]
    ref = L.phase1_collisions(name, t)
    rows = []
    for form, keyf in (("site+class", lambda r: (r.site, L.phase1_class(r))), ("class only", lambda r: L.phase1_class(r))):
        keys = {(r.seed, r.incident): f"{keyf(r)}" for r in inc.itertuples()}
        look = {(r.seed, r.incident): r.incident for r in inc.itertuples()}
        bind = {k: v + 0.5 for k, v in look.items()}
        res = simulate(inc, seeds, keys, bind, look, "family", False, ("hard", "decoy"))
        for tier in ("all", "plain", "hard", "decoy"):
            d = res if tier == "all" else res[res["tier"] == tier]
            mine = (len(d), int(d["recalled"].sum()), int(d["right"].sum()), int((d["recalled"] & ~d["right"]).sum()))
            w2 = [r for r in ref if r["memory"].startswith(f"phase-1 {form}") and r["incident_tier"] == tier][0]
            theirs = (w2["incidents"], w2["recalled"], w2["recalled_right"], w2["recalled_wrong"])
            rows.append({"range": name, "seeds": C.seed_range(seeds), "side": "hidden", "form": form, "tier": tier,
                         "mine_incidents_recalled_right_wrong": "/".join(map(str, mine)),
                         "w2_incidents_recalled_right_wrong": "/".join(map(str, theirs)), "equal": mine == theirs})
    return rows


def partition_check(name: str, t: dict) -> list[dict]:
    """The data-derived phase-1 classes (K2 at 6 s after the first abnormal observation) against W2's
    code-derived ones: how many classes of one hold more than one of the other."""
    inc, fr = t["inc"], t["fr"]
    sub = fr[fr["cut"] == "6"].merge(inc[["seed", "incident", "tier", "family", "mode", "duo", "known_kind", "kind_a", "kind_b",
                                          "site", "sig_altered", "edge_altered"]], on=["seed", "incident"])
    sub["w2"] = [L.phase1_class(r) for r in sub.itertuples()]
    sub["mine"] = [ladder_key(r, "K2") for r in sub.itertuples()]
    rows = []
    for label, m in (("all incidents", sub["tier"].notna()), ("not altered by a regime change", (sub["sig_altered"] == 0) & (sub["edge_altered"] == 0)),
                     ("not hard slow leak", sub["family"] != "slow_leak")):
        d = sub[m]
        a = d.groupby("w2")["mine"].nunique()
        b = d.groupby("mine")["w2"].nunique()
        inc_a = d["w2"].map(a)
        inc_b = d["mine"].map(b)
        rows.append({"range": name, "seeds": C.seed_range(t["seeds"]), "side": "hidden", "subset": label,
                     "incidents": len(d), "w2_classes": int(d["w2"].nunique()), "data_classes": int(d["mine"].nunique()),
                     "incidents_whose_w2_class_has_one_data_class": int((inc_a == 1).sum()),
                     "incidents_whose_data_class_has_one_w2_class": int((inc_b == 1).sum())})
    return rows


def waiting_cost(name: str, t: dict) -> list[dict]:
    """What waiting costs: of the hard incidents, the share whose deadline has already passed (or whose
    reasoner call would already be due) when the cutoff is reached, by cutoff, and the share that have a
    snapshot at all."""
    inc, seeds = t["inc"], t["seeds"]
    h = inc[inc["tier"] == "hard"].copy()
    h["to_deadline_s"] = (h["deadline_ns"] - h["t0_ns"]) / NS
    rows = []
    for c in CUTS:
        for label, m in (("all hard", h["tier"] == "hard"), ("critical hard", h["critical"] == 1), ("non-critical hard", h["critical"] == 0)):
            d = h[m]
            rows.append({"range": name, "seeds": C.seed_range(seeds), "side": "hidden", "cutoff_s": c, "set": label, "hard_incidents": len(d),
                         "deadline_before_cutoff_share": f((d["to_deadline_s"] <= c).mean())})
    d = h
    for label, m in (("all hard", d["tier"] == "hard"),):
        sub = d[m]
        rows.append({"range": name, "seeds": C.seed_range(seeds), "side": "hidden", "cutoff_s": "gate_att", "set": label,
                     "hard_incidents": len(sub), "deadline_before_cutoff_share": f(sub["gate_att_ns"].notna().mean())})
    return rows


def gate_table(name: str, t: dict) -> list[dict]:
    """How often the public checker finds no hypothesis on a family's own evidence, and when."""
    inc = t["inc"]
    rows = []
    groups = [("plain", inc["tier"] == "plain"), ("decoy", inc["tier"] == "decoy"), ("hard", inc["tier"] == "hard")]
    groups += [(f"hard/{fam}", (inc["tier"] == "hard") & (inc["family"] == fam)) for fam in L.HARD_FAMILIES]
    groups += [(f"hard/{fm}", (inc["tier"] == "hard") & (inc["fam_mode"] == fm)) for fm in sorted(inc.loc[inc["tier"] == "hard", "fam_mode"].unique())]
    groups += [(f"decoy/{fam}", (inc["tier"] == "decoy") & (inc["family"] == fam)) for fam in L.HARD_FAMILIES]
    groups += [("plain, after the signature shift", (inc["tier"] == "plain") & (inc["onset_ns"] >= inc["t_ss"])),
               ("plain, altered by the signature shift", (inc["tier"] == "plain") & (inc["sig_altered"] == 1)),
               ("plain, altered by the added edge", (inc["tier"] == "plain") & (inc["edge_altered"] == 1))]
    for g, m in groups:
        d = inc[m]
        if d.empty:
            continue
        ga, gl = d["gate_att_ns"] / NS, d["gate_all_ns"] / NS
        rows.append({"range": name, "seeds": C.seed_range(t["seeds"]), "side": "hidden", "group": g, "incidents": len(d),
                     "gate_on_attached_evidence": int(ga.notna().sum()), "share_attached": f(ga.notna().mean()),
                     "median_delay_attached_s": f(ga.median()) if ga.notna().any() else "",
                     "gate_on_all_evidence": int(gl.notna().sum()), "share_all": f(gl.notna().mean()),
                     "median_delay_all_s": f(gl.median()) if gl.notna().any() else ""})
    return rows


# ---- driver ---------------------------------------------------------------------------------


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--hidden-root", type=pathlib.Path, default=HIDDEN)
    ap.add_argument("--out-dir", type=pathlib.Path, default=OUT)
    ap.add_argument("--ranges", default="a-tune,a-heldout,c-tune,c-heldout")
    ap.add_argument("--parts", default="ladder,e1,check-w2,check-partition,waiting,gate,vote,who")
    args = ap.parse_args(argv)
    ap2 = args.parts.split(",")
    tables: dict[str, list[dict]] = {k: [] for k in ("ladder", "e1", "check-w2", "check-partition", "waiting", "gate", "vote", "who") if k in ap2}
    for name in args.ranges.split(","):
        t = load(args.hidden_root, name)
        print(name, C.seed_range(t["seeds"]), len(t["inc"]), "incidents", flush=True)
        for part, fn in (("check-w2", w2_replication), ("check-partition", partition_check), ("waiting", waiting_cost), ("gate", gate_table),
                         ("e1", e1_forms), ("ladder", ladder), ("vote", vote), ("who", who)):
            if part in tables:
                tables[part] += fn(name, t)
    for k, rows in tables.items():
        C.write_csv(args.out_dir / f"w3-floor-{k}.csv", rows)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
