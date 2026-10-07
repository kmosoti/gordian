#!/usr/bin/env python3
"""W2 item 6: the perfect-memory ceiling, on the selection oracle's own runs.

Usage:
    w2_ceiling.py --run a-tune=RUN_DIR --run a-heldout=RUN_DIR [--hidden-root DIR] [--out-dir DIR]
                  [--l1-dir DIR]

`RUN_DIR` is a run directory of `scripts/run-driver.sh` for the manifest `w2_manifests.py` wrote:
one arm, `sel_reanchor_privileged` (the selection oracle at 16 s, the rung's context, B2's re-anchor
noticer), with the ledger kept for every stream (`events-sample.jsonl`). The hidden tables are those
of `w2_hidden.sh` for the same range. Side: *run* (the arm's evaluator files and ledger) joined to
*hidden* truth by (seed, incident index); the observation-to-incident owner map joins each
`Escalate` focus to the incident it was about.

What it measures, per range. The arm's reasoner cost per incident (calls, tokens, modelled ns; the
ledger's `Escalated` outcome, matched to its `Escalate` decision) and the set of hard incidents a
perfect memory could have answered without asking:

- **site-keyed reach** `R`: a hard incident that repeats (`recurrence_of`) an incident the arm
  declared correctly earlier, the declaration made before the repeating incident's first notice (or
  its onset when never noticed): the earliest an arm could recall;
- `R_chain`: the same with any earlier incident of the same site and full family identity, not only
  the one the generator names (a memory binds on any earlier occurrence);
- `R` without the recurrences stale by construction;
- **family-keyed reach** `F`: a hard incident with an earlier hard incident of the same family and
  mode (any site) that the arm declared correctly before its first notice; `F strict` also demands
  the same kinds (compound pair, cascade kind); `F_only` is `F` without `R`.

Each is reported as incidents, calls, modelled ns, and as a share of the arm's total bill
(`total_cost_ns`) and of its reasoner cost, with a 90% interval from a cluster bootstrap over streams.
`R_world` and `F_world` drop the condition that the arm declared the earlier incident correctly
(what the arm spent on incidents the world repeats). Checks that stop the script on failure: the
ledger's calls sum to `results.csv` per stream, and the calls per incident equal `incidents.csv`'s
`escalations`. With `--l1-dir` it also compares the held-out run with L1's fresh run of the same arm.
"""

from __future__ import annotations

import argparse
import json
import pathlib

import numpy as np
import pandas as pd

import w2_common as C
import w2_laws as L
from w2_common import boot_ratio, f

ARM = "sel_reanchor_privileged"


def load_calls(trace: pathlib.Path) -> pd.DataFrame:
    """Every accepted `Escalate` of the ledger: (seed, call, at_ns, focus, refs, tokens, modelled_ns, ready_at_ns)."""
    pending: dict[tuple[int, int], tuple[int, int, int]] = {}
    rows = []
    with open(trace) as fh:
        for line in fh:
            if '"Escalate":' in line:
                d = json.loads(line)
                if d.get("kind") != "Decision":
                    continue
                esc = d["payload"]["action"]["Escalate"]
                pending[(d["seed"], d["id"])] = (d["at_ns"], esc["question"]["Diagnose"]["focus"], len(esc["context"]))
            elif '"Escalated":' in line:
                d = json.loads(line)
                if d.get("kind") != "Outcome" or "Escalated" not in d["payload"]:
                    continue
                e = d["payload"]["Escalated"]
                at, focus, refs = pending.pop((d["seed"], d["inputs"][0]))
                rows.append((d["seed"], e["call"], at, focus, refs, e["cost"]["tokens"], e["cost"]["modelled_ns"], e["ready_at"]))
    assert not pending, "an Escalate decision with no outcome in the ledger"
    return pd.DataFrame(rows, columns=["seed", "call", "at_ns", "focus", "refs", "tokens", "modelled_ns", "ready_at_ns"])


def load_owners(path: pathlib.Path) -> dict[int, np.ndarray]:
    out = {}
    with open(path) as fh:
        for line in fh:
            d = json.loads(line)
            out[d["seed"]] = np.asarray(d["owner"], dtype=np.int64)
    return out


def join_run(name: str, run_dir: pathlib.Path, hidden_root: pathlib.Path):
    """The run's per-incident table joined to hidden truth and the calls' cost, with the checks."""
    arm = run_dir / ARM
    t = L.prepare(C.read_range(hidden_root, name))
    inc = t["inc"].copy()
    a = pd.read_csv(arm / "incidents.csv")
    ni = pd.read_csv(arm / "notice_incidents.csv")
    res = pd.read_csv(arm / "results.csv")
    assert list(res["seed"]) == list(t["seeds"]), "the run and the hidden tables cover different streams"
    j = inc.merge(a[["seed", "incident", "tier", "family", "correct_declarations", "wrong_declarations",
                     "first_correct_at_ns", "correct_by_deadline", "escalations", "correct_escalations"]]
                  .rename(columns={"tier": "arm_tier", "family": "arm_family"}), on=["seed", "incident"], how="left", validate="1:1")
    assert j["arm_tier"].notna().all() and len(j) == len(a), "an incident of the run has no hidden row, or the reverse"
    assert (j["arm_tier"] == j["tier"]).all(), "tier of the run's evaluator disagrees with the hidden truth"
    hardrows = j["tier"] == "hard"
    assert (j.loc[hardrows, "arm_family"] == j.loc[hardrows, "family"]).all(), "family disagrees"
    j = j.merge(ni[["seed", "incident", "first_notice_at_ns", "noticed"]], on=["seed", "incident"], how="left", validate="1:1")

    # The ledger's calls, matched to the incident each was about.
    calls = load_calls(arm / "events-sample.jsonl")
    owners = load_owners(hidden_root / name / "owners.jsonl")
    calls["incident"] = [int(owners[s][fc]) for s, fc in zip(calls["seed"], calls["focus"])]
    per_inc = calls[calls["incident"] >= 0].groupby(["seed", "incident"]).agg(
        calls=("call", "size"), refs=("refs", "sum"), tokens=("tokens", "sum"), ns=("modelled_ns", "sum"),
        last_ready_ns=("ready_at_ns", "max")).reset_index()
    j = j.merge(per_inc, on=["seed", "incident"], how="left")
    for col in ("calls", "refs", "tokens", "ns"):
        j[col] = j[col].fillna(0).astype(np.int64)
    # last_ready_ns stays NaN for an incident nobody asked about
    # Checks: the ledger and the evaluator agree, per incident and per stream.
    assert (j["calls"] == j["escalations"]).all(), "calls per incident differ from incidents.csv escalations"
    bg = calls[calls["incident"] < 0].groupby("seed").agg(bg_calls=("call", "size"), bg_ns=("modelled_ns", "sum"))
    per_stream = j.groupby("seed").agg(calls=("calls", "sum"), ns=("ns", "sum")).join(bg).fillna(0)
    per_stream = per_stream.reindex(res["seed"].to_numpy(), fill_value=0)
    tot_calls = (per_stream["calls"] + per_stream["bg_calls"]).to_numpy()
    tot_ns = (per_stream["ns"] + per_stream["bg_ns"]).to_numpy()
    assert (tot_calls == res["reasoner_calls"].to_numpy()).all(), "ledger calls differ from results.csv"
    assert (tot_ns == res["reasoner_modelled_ns"].to_numpy()).all(), "ledger modelled ns differ from results.csv"
    return t, j, res


def reach_flags(j: pd.DataFrame) -> pd.DataFrame:
    """Add the reach sets, for hard incidents. An earlier incident `i` is usable by `j` when the arm
    declared it correctly (`first_correct_at_ns`) before `j`'s first notice (onset when none)."""
    j = j.copy()
    j["t_ref"] = j["first_notice_at_ns"].fillna(j["onset_ns"].astype(float))
    j["fc"] = j["first_correct_at_ns"]
    j["other_key"] = j["other"].fillna(-1)  # NaN never equals NaN: a family with no partner has key -1
    key = j.set_index(["seed", "incident"])
    j["tpl_fc"] = key["fc"].reindex(pd.MultiIndex.from_frame(
        j[["seed", "recurrence_of"]].fillna(-1).astype(int))).to_numpy()
    j["R"] = (j["tier"] == "hard") & j["is_rec"] & (j["tpl_fc"] < j["t_ref"])
    j["R_world"] = (j["tier"] == "hard") & j["is_rec"]
    R_chain, F, Fs, F_world, Fs_world = [], [], [], [], []
    for _, g in j.groupby("seed", sort=False):
        hard_prev: list = []
        for r in g.itertuples():
            if r.tier != "hard":
                R_chain.append(False); F.append(False); Fs.append(False); F_world.append(False); Fs_world.append(False)
                continue
            rc = fa = fs = fwa = fsw = False
            for p in hard_prev:
                used = (not np.isnan(p.fc)) and p.fc < r.t_ref
                same_fam = p.fam_mode == r.fam_mode
                same_strict = p.strict == r.strict
                if same_fam:
                    fwa = True
                    fa |= used
                if same_strict:
                    fsw = True
                    fs |= used
                    if p.site == r.site and p.other_key == r.other_key:
                        rc |= used
            R_chain.append(rc); F.append(fa); Fs.append(fs); F_world.append(fwa); Fs_world.append(fsw)
            hard_prev.append(r)
    # Family-keyed memory carried across streams in seed order: an earlier stream's hard incident
    # of the same key that the arm declared correctly (any time: the stream was over). Service ids
    # and message ids are per stream, so only a key without them can carry; this is that key's
    # ceiling, and it is not the site-keyed form.
    cross_fam: set = set()
    cross_strict: set = set()
    F_cross, Fs_cross = [], []
    for _, g in j.groupby("seed", sort=False):
        mine_fam, mine_strict = set(), set()
        for r in g.itertuples():
            if r.tier != "hard":
                F_cross.append(False); Fs_cross.append(False)
                continue
            F_cross.append(r.fam_mode in cross_fam)
            Fs_cross.append(r.strict in cross_strict)
            if not np.isnan(r.fc):
                mine_fam.add(r.fam_mode)
                mine_strict.add(r.strict)
        cross_fam |= mine_fam
        cross_strict |= mine_strict
    j["R_chain"] = R_chain
    j["F"] = F
    j["F_strict"] = Fs
    j["F_world"] = F_world
    j["F_cross"] = np.array(F_cross) | j["F"].to_numpy()
    j["F_strict_cross"] = np.array(Fs_cross) | j["F_strict"].to_numpy()
    j["F_strict_world"] = Fs_world
    assert (j["R"] <= j["F"]).all(), "a site-keyed reach that is not family-keyed reach"
    assert (j["R"] <= j["R_chain"]).all(), "R_chain must contain R (the template shares the recurrence's identity)"
    assert (j["R_chain"] <= j["F_strict"]).all()
    j["R_fresh"] = j["R"] & ~j["stale_any_row"]
    j["F_only"] = j["F"] & ~j["R"]
    j["F_only_world"] = j["F_world"] & ~j["R_world"]
    j["F_strict_only"] = j["F_strict"] & ~j["R"]
    return j


SETS = [
    ("all hard incidents", None),
    ("R  site-keyed: recurrence of an incident the arm declared correctly earlier", "R"),
    ("R_chain  site-keyed, any earlier same-site same-family incident declared correctly", "R_chain"),
    ("R_fresh  R without the recurrences stale by construction", "R_fresh"),
    ("F_only  family-keyed beyond R: same family and mode, earlier correct, not in R", "F_only"),
    ("F  family-keyed: same family and mode (any site) earlier declared correctly", "F"),
    ("F_strict  as F with the same kinds (compound pair, cascade kind)", "F_strict"),
    ("R_world  every hard recurrence (no condition on the arm)", "R_world"),
    ("F_world  every hard incident whose family and mode occurred earlier (no condition on the arm)", "F_world"),
]


SETS_CROSS = [
    ("all hard incidents", None),
    ("R  site-keyed, within the stream", "R"),
    ("F  family-keyed, within the stream", "F"),
    ("F_cross  family-keyed, memory carried across streams in seed order", "F_cross"),
    ("F_strict_cross  as F_cross with the same kinds", "F_strict_cross"),
]
CROSS_SKIP = 10  # the first ten streams only fill the carried memory


def ceiling_rows(name: str, t: dict, j: pd.DataFrame, res: pd.DataFrame, sets=SETS, first=0, note="") -> list[dict]:
    seeds = t["seeds"][first:]
    rng_label = C.seed_range(seeds)
    hard = j[(j["tier"] == "hard") & j["seed"].isin(seeds)]
    bill = res["total_cost_ns"].to_numpy(dtype=float)[first:]
    reasoner_all = res["reasoner_modelled_ns"].to_numpy(dtype=float)[first:]
    rows = []
    for label, col in sets:
        sub = hard if col is None else hard[hard[col]]
        n = per_stream_sum(sub, seeds, lambda d: np.ones(len(d)))
        calls = per_stream_sum(sub, seeds, lambda d: d["calls"].to_numpy(float))
        ns = per_stream_sum(sub, seeds, lambda d: d["ns"].to_numpy(float))
        ok = sub[sub["correct_declarations"] > 0]
        ns_ok = per_stream_sum(ok, seeds, lambda d: d["ns"].to_numpy(float))
        sb, lo, hi = boot_ratio(ns, bill)
        sr, rlo, rhi = boot_ratio(ns, reasoner_all)
        pc, clo, chi = boot_ratio(calls)
        rows.append({
            "range": name, "seeds": rng_label, "side": "run joined to hidden", "set": label + note, "streams": len(seeds),
            "incidents": int(n.sum()), "incidents_per_stream": f(n.mean()), "calls": int(calls.sum()),
            "streams_with_any": int((n > 0).sum()), "calls_per_stream": f(pc), "calls_lo90": f(clo), "calls_hi90": f(chi),
            "modelled_ns": int(ns.sum()), "share_of_total_bill": f(sb), "bill_lo90": f(lo), "bill_hi90": f(hi),
            "share_of_reasoner_cost": f(sr), "reasoner_lo90": f(rlo), "reasoner_hi90": f(rhi),
            "of_which_arm_was_already_correct_ns": int(ns_ok.sum()),
        })
    return rows


def per_stream_sum(df: pd.DataFrame, seeds, vals) -> np.ndarray:
    if df.empty:
        return np.zeros(len(seeds))
    s = pd.Series(vals(df), index=df["seed"].to_numpy()).groupby(level=0).sum()
    return s.reindex(seeds, fill_value=0.0).to_numpy(dtype=float)


def bill_row(name: str, t: dict, j: pd.DataFrame, res: pd.DataFrame) -> dict:
    seeds = t["seeds"]
    hard = j[j["tier"] == "hard"]
    return {
        "range": name, "seeds": C.seed_range(seeds), "side": "run", "streams": len(seeds),
        "total_cost_ns": int(res["total_cost_ns"].sum()), "reasoner_modelled_ns": int(res["reasoner_modelled_ns"].sum()),
        "reasoner_share_of_bill": f(res["reasoner_modelled_ns"].sum() / res["total_cost_ns"].sum(), 6),
        "reasoner_calls": int(res["reasoner_calls"].sum()),
        "calls_on_hard": int(hard["calls"].sum()), "calls_on_other_incidents": int(j.loc[j["tier"] != "hard", "calls"].sum()),
        "calls_on_background": int(res["reasoner_calls"].sum() - j["calls"].sum()),
        "mean_ns_per_call": f(res["reasoner_modelled_ns"].sum() / res["reasoner_calls"].sum(), 0),
        "hard_incidents": len(hard), "hard_noticed": int(hard["noticed"].fillna(False).astype(bool).sum()),
        "hard_asked": int((hard["calls"] > 0).sum()), "hard_declared_correctly": int((hard["correct_declarations"] > 0).sum()),
        "hard_declared_correctly_by_deadline": int(hard["correct_by_deadline"].fillna(False).astype(bool).sum()),
    }


def unasked_rows(name: str, t: dict, j: pd.DataFrame) -> list[dict]:
    """What the arm's run says about the unasked-correct clause: the baseline that needs no memory.
    A plain incident is declared correctly with no escalation by the cheap rung alone."""
    seeds = t["seeds"]
    rows = []
    for tier in ("plain", "hard", "decoy"):
        sub = j[(j["tier"] == tier)]
        for label, m in (("all incidents", sub["tier"].notna()), ("recurrences", sub["is_rec"])):
            d = sub[m]
            if d.empty:
                continue
            unasked_correct = (d["correct_declarations"] > 0) & (d["calls"] == 0)
            stale_wrong = (d["wrong_declarations"] > 0) & (d["calls"] == 0)
            num = per_stream_sum(d[unasked_correct], seeds, lambda x: np.ones(len(x)))
            den = per_stream_sum(d, seeds, lambda x: np.ones(len(x)))
            sw = per_stream_sum(d[stale_wrong], seeds, lambda x: np.ones(len(x)))
            p, lo, hi = boot_ratio(num, den)
            q, qlo, qhi = boot_ratio(sw, den)
            rows.append({"range": name, "seeds": C.seed_range(seeds), "side": "run joined to hidden", "tier": tier,
                         "incidents": label, "n": int(den.sum()), "declared_correct_with_no_call": int(num.sum()),
                         "share": f(p), "lo90": f(lo), "hi90": f(hi),
                         "declared_wrong_with_no_call": int(sw.sum()), "wrong_share": f(q), "wrong_lo90": f(qlo), "wrong_hi90": f(qhi)})
    return rows


def hard_incident_rows(name: str, t: dict, j: pd.DataFrame) -> list[dict]:
    h = j[j["tier"] == "hard"].copy()
    out = []
    for r in h.itertuples():
        out.append({
            "range": name, "seed": r.seed, "incident": r.incident, "family": r.family, "mode": r.mode,
            "site": r.site, "onset_s": f(r.onset_ns / C.NS, 3), "recurrence_of": "" if not r.is_rec else int(r.recurrence_of),
            "first_notice_s": f(r.first_notice_at_ns / C.NS, 3) if pd.notna(r.first_notice_at_ns) else "",
            "calls": r.calls, "refs": r.refs, "modelled_ns": r.ns,
            "arm_correct": int(r.correct_declarations > 0), "first_correct_s": f(r.first_correct_at_ns / C.NS, 3) if pd.notna(r.first_correct_at_ns) else "",
            "tpl_first_correct_s": f(r.tpl_fc / C.NS, 3) if pd.notna(r.tpl_fc) else "",
            "R": int(r.R), "R_chain": int(r.R_chain), "R_fresh": int(r.R_fresh), "F_only": int(r.F_only), "F": int(r.F),
            "F_strict": int(r.F_strict), "stale_by_construction": int(r.stale_any_row),
        })
    return out


def propagation_rows(name: str, t: dict, j: pd.DataFrame) -> list[dict]:
    """The error a memory fed by the reasoner inherits: the stored answer was wrong. For each hard
    incident whose source (the template, for the site-keyed form; the most recent earlier hard
    incident of the same family and mode, for the family-keyed form) had its answer delivered
    before the incident's first notice, the share whose source answer was wrong. An answer is wrong
    when the source was asked and none of its calls was right, right when all were; a source with
    both is `mixed`. This is not staleness and not collision: it is the reasoner's own error
    carried forward, and it is the same whatever key is used."""
    seeds = t["seeds"]
    key = j.set_index(["seed", "incident"])
    answered = key["last_ready_ns"].notna()
    right = answered & (key["correct_escalations"] == key["escalations"])
    wrong = answered & (key["correct_escalations"] == 0)
    rows = []
    for form in ("site-keyed (the template)", "family-keyed (most recent same family and mode)"):
        out = []  # (seed, usable, right, wrong, mixed)
        for seed, g in j.groupby("seed", sort=False):
            prev: dict[str, tuple] = {}
            n_u = n_r = n_w = n_m = 0
            for r in g.itertuples():
                if r.tier != "hard":
                    continue
                src = None
                if form.startswith("site"):
                    if r.is_rec:
                        src = (seed, int(r.recurrence_of))
                else:
                    src = prev.get(r.fam_mode)
                if src is not None and answered.get(src, False) and key.loc[src, "last_ready_ns"] < r.t_ref:
                    n_u += 1
                    n_r += bool(right.get(src, False))
                    n_w += bool(wrong.get(src, False))
                    n_m += not (right.get(src, False) or wrong.get(src, False))
                if answered.get((seed, r.incident), False):
                    prev[r.fam_mode] = (seed, r.incident)
            out.append((n_u, n_r, n_w, n_m))
        a = pd.DataFrame(out, columns=["usable", "right", "wrong", "mixed"], index=[s for s, _ in j.groupby("seed", sort=False)])
        a = a.reindex(seeds, fill_value=0)
        pw, lo, hi = boot_ratio(a["wrong"].to_numpy(float), a["usable"].to_numpy(float))
        rows.append({"range": name, "seeds": C.seed_range(seeds), "side": "run joined to hidden", "source": form,
                     "hard_incidents_with_an_answered_source": int(a["usable"].sum()), "source_answer_right": int(a["right"].sum()),
                     "source_answer_wrong": int(a["wrong"].sum()), "mixed": int(a["mixed"].sum()),
                     "wrong_share": f(pw), "lo90": f(lo), "hi90": f(hi)})
    return rows


def identity(run_dir: pathlib.Path, l1_dir: pathlib.Path) -> list[dict]:
    rows = []
    for fname in ("incidents.csv", "results.csv", "notice_incidents.csv", "notices.csv"):
        a = pd.read_csv(run_dir / ARM / fname).drop(columns=["run_id", "arm_role"])
        b = pd.read_csv(l1_dir / ARM / fname).drop(columns=["run_id", "arm_role"])
        same = a.shape == b.shape and a.equals(b)
        rows.append({"file": fname, "rows_run": len(a), "rows_l1": len(b), "identical_modulo_run_id": same})
    return rows


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--run", action="append", required=True, metavar="NAME=DIR")
    ap.add_argument("--hidden-root", type=pathlib.Path, default=C.HIDDEN)
    ap.add_argument("--out-dir", type=pathlib.Path, default=C.OUT)
    ap.add_argument("--l1-dir", type=pathlib.Path, default=None, help="L1's fresh run directory (held-out identity)")
    args = ap.parse_args(argv)
    ceil, bill, unasked, hard_rows, ident, prop = [], [], [], [], [], []
    for spec in args.run:
        name, d = spec.split("=", 1)
        run_dir = pathlib.Path(d)
        t, j, res = join_run(name, run_dir, args.hidden_root)
        rec = t["rec"]
        key = pd.MultiIndex.from_frame(j[["seed", "incident"]])
        j["stale_any_row"] = rec.set_index(["seed", "incident"])["stale_any"].reindex(key).fillna(False).to_numpy(dtype=bool)
        j = reach_flags(j)
        ceil += ceiling_rows(name, t, j, res)
        ceil += ceiling_rows(name, t, j, res, SETS_CROSS, CROSS_SKIP, f" [streams {CROSS_SKIP + 1} onward]")
        bill.append(bill_row(name, t, j, res))
        unasked += unasked_rows(name, t, j)
        hard_rows += hard_incident_rows(name, t, j)
        prop += propagation_rows(name, t, j)
        if name == "a-heldout" and args.l1_dir is not None:
            for r in identity(run_dir, args.l1_dir):
                ident.append({"range": name, "against": "L1 fresh run, sel_reanchor_privileged", **r})
    C.write_csv(args.out_dir / "w2-ceiling.csv", ceil)
    C.write_csv(args.out_dir / "w2-ceiling-bill.csv", bill)
    C.write_csv(args.out_dir / "w2-ceiling-unasked.csv", unasked)
    C.write_csv(args.out_dir / "w2-ceiling-hard-incidents.csv", hard_rows)
    C.write_csv(args.out_dir / "w2-ceiling-propagation.csv", prop)
    if ident:
        C.write_csv(args.out_dir / "w2-ceiling-identity.csv", ident)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
