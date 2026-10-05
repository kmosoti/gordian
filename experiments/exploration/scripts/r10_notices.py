"""R10's reported-beside analyses that need to know what the rung noticed: the decomposition of
result 1 by whether the rung ever noticed the incident, the rung's notice latency per family and
mode, the list of never-noticed incidents, and the public threshold sweep.

Usage:
  r10_notices.py validate     check the notices against the diagnostic run's ledgers
  r10_notices.py decompose    r10-decomposition.csv, r10-notice-latency.csv, r10-never-noticed*.csv
  r10_notices.py sweep        r10-sweep.csv

Inputs: the notice dumps (`r10_notice_dump.sh`: the `r10_notices` example plays the manifest's
shared rung over each stream and writes what it noticed beside the incidents), the diagnostic run
(ledgers kept) and the held-out and sweep runs. Exploration (nothing here tests a hypothesis).

Readings the plan leaves open, fixed before any held-out number was seen:

* **From the ledgers.** The ledger holds the observations and every decision but not the rung's
  notices. The notices are therefore re-derived by playing the same rung, in Rust, over the same
  streams (`r10_notices.rs`, with a rule that escalates nothing; a call in flight keeps its anomaly
  from retiring, so the selection oracle's own count of anomalies can differ slightly, which
  `validate` measures against the arms' own `results.csv`), and
  `validate` checks them against the diagnostic ledgers: every call of `oracle_selection` in them
  is about an anomaly the dump says was noticed, `delay_ns` before the call (to within one step),
  and every call of `oracle_notice` is about its incident's first observation, `delay_ns` after
  the step that delivered it.
* **Noticed.** An incident is *noticed* when the rung noticed at least one anomaly whose anchor
  belongs to it (the selection oracle acts on an anomaly's anchor); otherwise it is *never
  noticed*. An incident whose anomaly is attached to by another incident's observations only is
  never noticed in this sense, and is flagged `attached_elsewhere`.
* **Late-noticed** is a noticed incident: the rung's notice instant is the first one among its
  anchored anomalies, and its latency is that instant minus the incident's first observation. The
  decomposition splits the noticed incidents by whether the selection oracle made its call
  (`noticed_called`) or the anomaly was retired by the rung before the delay (`noticed_uncalled`).
* **Mode** is `contradicts_early` (a hard incident's first moments already break the first
  world's rules) or `mimics_plain` (they imitate a plain incident), the evaluator's label.
* **False notices** are anomalies noticed whose anchor belongs to no incident (background) or to a
  plain incident; anomalies anchored on a decoy or a hard incident are reported separately.
"""

import json
import sys

import numpy as np
import pandas as pd

import r10_common as C
import r6_stats as R6S
import r10_stats as S
from gordian_analysis.load import load_stream_run

DUMPS = C.RUNS / "r10-notices"
STEP_NS = 500_000_000


def dump(name):
    """seed -> record of one notice dump."""
    out = {}
    for line in open(DUMPS / f"{name}.jsonl"):
        d = json.loads(line)
        out[d["seed"]] = d
    return out


def mode_of(inc):
    if inc["tier"] != "hard":
        return ""
    return {True: "contradicts_early", False: "mimics_plain"}.get(inc["contradicts_early"], "unknown")


def incident_table(records):
    """One row per incident of every stream: what the rung did about it."""
    rows = []
    for seed, rec in sorted(records.items()):
        by_inc, attached = {}, {}
        for n in rec["notices"]:
            if n["anchor_incident"] is not None:
                by_inc.setdefault(n["anchor_incident"], []).append(n)
            for i in n["attached_incidents"]:
                attached.setdefault(i, []).append(n)
        for inc in rec["incidents"]:
            ns = by_inc.get(inc["id"], [])
            first_notice = min((n["noticed_at_ns"] for n in ns), default=None)
            first_at = inc["first_at_ns"]
            rows.append({
                "seed": seed, "incident": inc["id"], "tier": inc["tier"],
                # a decoy carries the hard kind it imitates; the evaluator's `family` is hard incidents only
                "family": inc["family"] if inc["tier"] == "hard" else "",
                "mode": mode_of(inc), "critical": inc["critical"], "onset_ns": inc["onset_ns"],
                "first_obs": inc["first_obs"], "first_at_ns": first_at, "observations": inc["observations"],
                "noticed": bool(ns), "anchored_notices": len(ns),
                "first_notice_at_ns": first_notice,
                "latency_s": None if first_notice is None or first_at is None else (first_notice - first_at) / 1e9,
                "latency_from_onset_s": None if first_notice is None else (first_notice - inc["onset_ns"]) / 1e9,
                "anchored_at_first_obs": any(n["anchor"] == inc["first_obs"] for n in ns),
                "attached_elsewhere": (not ns) and bool(attached.get(inc["id"])),
            })
    return pd.DataFrame(rows)


def notice_table(records):
    """One row per anomaly the rung noticed, with the tier of the incident its anchor belongs to."""
    rows = []
    for seed, rec in sorted(records.items()):
        tier = {i["id"]: i["tier"] for i in rec["incidents"]}
        for n in rec["notices"]:
            inc = n["anchor_incident"]
            rows.append({"seed": seed, "anomaly": n["anomaly"], "anchor_tier": "background" if inc is None else tier[inc],
                         "noticed_at_ns": n["noticed_at_ns"]})
    return pd.DataFrame(rows)


# ---- validation against the ledgers --------------------------------------------------------------


def ledger_calls(path):
    """seed -> list of (call instant ns, focus) of accepted escalations in an events sample."""
    out, cur, decisions, outcomes = {}, None, {}, {}

    def close():
        if cur is None:
            return
        calls = []
        for did, dec in decisions.items():
            action = dec["payload"]["action"]
            if "Escalate" not in action:
                continue
            o = outcomes.get(did)
            if o is None or "Escalated" not in o["payload"]:
                continue
            calls.append((dec["at_ns"], action["Escalate"]["question"]["Diagnose"]["focus"]))
        out[cur] = sorted(calls)

    for line in open(path):
        d = json.loads(line)
        rec = d.get("record")
        if rec == "segment":
            close()
            cur, decisions, outcomes = d["seed"], {}, {}
        elif rec == "entry":
            if d["kind"] == "Decision":
                decisions[d["id"]] = d
            elif d["kind"] == "Outcome":
                for parent in d["inputs"]:
                    outcomes[parent] = d
    close()
    return out


def validate():
    rid = C.run_id("diag", *C.PRIMARY)
    rd = C.RUNS / rid
    records = dump("diag")
    delay_ns = C.sel_delay_s(*C.PRIMARY) * C.NS
    rows = []
    for arm in (C.SEL_RUNG, C.NOTICE_RUNG):
        calls = ledger_calls(rd / arm / "events-sample.jsonl")
        n = ok = 0
        worst = 0.0
        for seed, rec in records.items():
            inc_of = {}
            for inc in rec["incidents"]:
                inc_of[inc["first_obs"]] = inc
            notice_of = {x["anchor"]: x for x in rec["notices"]}
            for at, focus in calls.get(seed, []):
                n += 1
                if arm == C.SEL_RUNG:
                    x = notice_of.get(focus)
                    if x is None:
                        continue
                    late = at - x["noticed_at_ns"] - delay_ns
                else:
                    inc = inc_of.get(focus)
                    if inc is None:
                        continue
                    late = at - inc["first_at_ns"] - delay_ns
                worst = max(worst, late / 1e9)
                ok += 0 <= late < 3 * STEP_NS
        rows.append({"arm": arm, "check": "each call is delay_ns after its notice (selection) / after its incident's first observation (notice), to within three steps",
                     "calls": n, "ok": ok, "worst_lateness_s": worst})
    rows.append({"arm": "streams", "check": "streams in the dump and in the ledgers", "calls": len(records),
                 "ok": len(ledger_calls(rd / C.SEL_RUNG / "events-sample.jsonl")), "worst_lateness_s": float("nan")})
    # the dump against the arms' own results, on the held-out streams at the default threshold
    hr = C.RUNS / C.run_id("heldout", *C.PRIMARY)
    held = {s: r["anomalies_noticed"] for s, r in dump("heldout-z3").items()}
    mine = pd.Series(held).sort_index()
    for a in (C.NEVER, C.NOTICE_RUNG, C.SEL_RUNG):
        theirs = pd.read_csv(hr / a / "results.csv").set_index("seed").anomalies_noticed.sort_index()
        rows.append({"arm": a, "check": "streams whose anomalies_noticed (results.csv) equals the dump's, held-out, default threshold",
                     "calls": len(mine), "ok": int((theirs == mine).sum()),
                     "worst_lateness_s": float(abs(theirs - mine).max())})
    df = pd.DataFrame(rows)
    df.to_csv(C.OUT / "r10-notice-validation.csv", index=False, float_format="%.6g")
    print(df.to_string(index=False))
    for r in rows[:2]:
        assert r["calls"] > 50 and r["ok"] == r["calls"], r
    for r in rows[3:5]:  # never_escalate and the notice arm: exactly the dump's
        assert r["ok"] == r["calls"], r


# ---- decomposition ---------------------------------------------------------------------------------


def decompose():
    rid = C.run_id("diag", *C.PRIMARY)
    run = load_stream_run(C.RUNS / rid)
    records = dump("diag")
    inc = incident_table(records)
    sel = run.arms[C.SEL_RUNG].incidents.set_index(["seed", "incident"])
    nov = run.arms[C.NOTICE_RUNG].incidents.set_index(["seed", "incident"])
    ref = run.arms[C.ORACLE].incidents.set_index(["seed", "incident"])
    inc = inc.set_index(["seed", "incident"])
    assert inc.index.equals(sel.index) and (inc["tier"].to_numpy() == sel["tier"].to_numpy()).all()
    assert (inc["family"].to_numpy() == sel["family"].fillna("").to_numpy()).all()
    inc["sel_correct"] = sel["correct_by_deadline"].astype(int)
    inc["notice_correct"] = nov["correct_by_deadline"].astype(int)
    inc["oracle_correct"] = ref["correct_by_deadline"].astype(int)
    inc["sel_calls"] = sel["escalations"]
    inc["notice_calls"] = nov["escalations"]
    inc = inc.reset_index()
    hard = inc[inc.tier == "hard"].copy()
    hard["group"] = np.where(~hard.noticed, "never_noticed", np.where(hard.sel_calls > 0, "noticed_called", "noticed_uncalled"))
    hard.to_csv(C.OUT / "r10-diag-hard-incidents.csv", index=False, float_format="%.6g")

    seeds = sorted(records)
    nseed = len(seeds)
    pos = {s: i for i, s in enumerate(seeds)}
    rows = []
    rng = np.random.default_rng(C.BOOT_SEED)
    counts = []
    done = 0
    while done < C.N_RESAMPLES:
        m = min(500, C.N_RESAMPLES - done)
        counts.append(rng.multinomial(nseed, np.full(nseed, 1.0 / nseed), size=m).astype(float))
        done += m
    w = np.concatenate(counts)

    def per_stream(df, col):
        v = np.zeros(nseed)
        for s, g in df.groupby("seed"):
            v[pos[s]] = g[col].sum()
        return v

    for label, sub in (("hard incidents, slow leak excluded", hard[hard.family != "slow_leak"]),
                       ("slow leak", hard[hard.family == "slow_leak"])):
        den = per_stream(sub, "sel_correct") * 0 + per_stream(sub.assign(one=1), "one")
        total_den = den.sum()
        for g in ("all", "never_noticed", "noticed_called", "noticed_uncalled", "noticed"):
            if g == "all":
                part = sub
            elif g == "noticed":
                part = sub[sub.group != "never_noticed"]
            else:
                part = sub[sub.group == g]
            gain = per_stream(part, "notice_correct") - per_stream(part, "sel_correct")
            point = gain.sum() / total_den
            boot = (w @ gain) / (w @ den)
            rows.append({
                "incidents": label, "group": g, "n_incidents": len(part), "of_total": int(total_den),
                "selection_correct": int(part.sel_correct.sum()), "notice_correct": int(part.notice_correct.sum()),
                "gain_incidents": int(part.notice_correct.sum() - part.sel_correct.sum()),
                "gain_in_quality": point, "lo": float(np.quantile(boot, 0.05, method="lower")),
                "hi": float(np.quantile(boot, 0.95, method="higher")),
                "oracle_correct": int(part.oracle_correct.sum()),
                "rate_selection": part.sel_correct.mean() if len(part) else float("nan"),
                "rate_notice": part.notice_correct.mean() if len(part) else float("nan"),
                "resamples": C.N_RESAMPLES, "seed": C.BOOT_SEED, "streams": nseed,
            })
    dec = pd.DataFrame(rows)
    dec.to_csv(C.OUT / "r10-decomposition.csv", index=False, float_format="%.6g")
    print(dec.to_string(index=False))

    # notice latency and never-noticed, per family and mode, in the diagnostic streams and the
    # held-out streams at the default threshold
    lat_rows, never_rows = [], []
    sources = {"diag": inc[inc.tier == "hard"], "heldout": incident_table(dump("heldout-z3")).query("tier == 'hard'")}
    for src, h in sources.items():
        for fam in ("compound", "cascade", "split_brain", "slow_leak", "ALL"):
            for mode in ("contradicts_early", "mimics_plain", "ALL"):
                g = h[((h.family == fam) | (fam == "ALL")) & ((h["mode"] == mode) | (mode == "ALL"))]
                if not len(g):
                    continue
                nz = g[g.noticed]
                q = nz.latency_s.quantile([0.25, 0.5, 0.75]).to_numpy() if len(nz) else [np.nan] * 3
                lat_rows.append({
                    "source": src, "family": fam, "mode": mode, "hard_incidents": len(g), "noticed": len(nz),
                    "never_noticed": len(g) - len(nz), "never_attached_elsewhere": int(g.attached_elsewhere.sum()),
                    "noticed_share": len(nz) / len(g),
                    "latency_p25_s": q[0], "latency_median_s": q[1], "latency_p75_s": q[2],
                    "latency_max_s": nz.latency_s.max() if len(nz) else np.nan,
                    "latency_median_from_onset_s": nz.latency_from_onset_s.median() if len(nz) else np.nan,
                    "anchored_at_first_obs": int(nz.anchored_at_first_obs.sum()) if len(nz) else 0,
                })
        nn = h[~h.noticed].copy()
        nn.insert(0, "source", src)
        never_rows.append(nn)
    pd.DataFrame(lat_rows).to_csv(C.OUT / "r10-notice-latency.csv", index=False, float_format="%.6g")
    nev = pd.concat(never_rows)[["source", "seed", "incident", "family", "mode", "critical", "onset_ns", "first_at_ns",
                                 "observations", "attached_elsewhere"]]
    nev.to_csv(C.OUT / "r10-never-noticed.csv", index=False)
    summ = nev.groupby(["source", "family", "mode"]).size().rename("never_noticed").reset_index()
    summ.to_csv(C.OUT / "r10-never-noticed-summary.csv", index=False)
    print(pd.DataFrame(lat_rows).to_string(index=False))
    print(summ.to_string(index=False))


# ---- the threshold sweep -----------------------------------------------------------------------------


def paired_across(run_a, arm_a, run_b, arm_b, kind):
    """Quality of (run_a, arm_a) minus that of (run_b, arm_b), cluster-bootstrapped over the common
    streams (the two runs played the same seeds)."""
    ta, tb = R6S.per_stream(run_a.arms[arm_a]), R6S.per_stream(run_b.arms[arm_b])
    assert (ta.index.to_numpy() == tb.index.to_numpy()).all()
    num, den = ("quality_num", "quality_den") if kind == "quality" else ("leak_num", "leak_den")
    n = len(ta)
    rng = np.random.default_rng(C.BOOT_SEED)
    out, done = [], 0
    while done < C.N_RESAMPLES:
        m = min(500, C.N_RESAMPLES - done)
        w = rng.multinomial(n, np.full(n, 1.0 / n), size=m).astype(float)
        ra = (w @ ta[num].to_numpy(float)) / (w @ ta[den].to_numpy(float))
        rb = (w @ tb[num].to_numpy(float)) / (w @ tb[den].to_numpy(float))
        out.append(ra - rb)
        done += m
    x = np.concatenate(out)
    point = ta[num].sum() / ta[den].sum() - tb[num].sum() / tb[den].sum()
    return float(point), float(np.quantile(x, 0.05, method="lower")), float(np.quantile(x, 0.95, method="higher"))


def sweep():
    b, rho = C.PRIMARY
    base = load_stream_run(C.RUNS / C.run_id("heldout", b, rho))
    rows = []
    for z in (C.DEFAULT_Z,) + C.SWEEP_Z:
        if z == C.DEFAULT_Z:
            run, name = base, "heldout-z3"
        else:
            run, name = load_stream_run(C.RUNS / C.run_id("sweep", b, rho, z)), f"sweep-{C.zname(z)}"
        recs = dump(name)
        inc = incident_table(recs)
        nt = notice_table(recs)
        hard = inc[inc.tier == "hard"]
        nonleak, leak = hard[hard.family != "slow_leak"], hard[hard.family == "slow_leak"]
        streams = len(recs)
        arm = run.arms[C.SEL_RUNG]
        p = S.point(arm)
        row = {
            "notice_z": z, "run_id": run.manifest["run_id"], "streams": streams,
            "hard_noticed_share": hard.noticed.mean(), "hard_noticed": int(hard.noticed.sum()), "hard_incidents": len(hard),
            "nonleak_noticed_share": nonleak.noticed.mean(), "nonleak_noticed": int(nonleak.noticed.sum()), "nonleak_incidents": len(nonleak),
            "leak_noticed_share": leak.noticed.mean(), "leak_noticed": int(leak.noticed.sum()), "leak_incidents": len(leak),
            "leak_latency_median_s": leak.latency_s.median(), "nonleak_latency_median_s": nonleak.latency_s.median(),
            "anomalies_per_stream": len(nt) / streams,
            "false_notices_per_stream": int(nt.anchor_tier.isin(["background", "plain"]).sum()) / streams,
            "on_background_per_stream": int((nt.anchor_tier == "background").sum()) / streams,
            "on_plain_per_stream": int((nt.anchor_tier == "plain").sum()) / streams,
            "on_decoy_per_stream": int((nt.anchor_tier == "decoy").sum()) / streams,
            "on_hard_per_stream": int((nt.anchor_tier == "hard").sum()) / streams,
            "quality": p["quality"], "quality_correct": p["quality_correct"], "quality_incidents": p["quality_incidents"],
            "leak_quality": p["leak_quality"], "plain_acc": p["plain_acc"], "critical_misses": p["critical_misses"],
            "calls_per_stream": p["calls_per_stream"], "refs_per_call": p["refs_per_call"], "cost_s": p["cost_s"],
            "reasoner_cost_s": p["reasoner_cost_s"], "false_alarms_per_stream": p["false_alarms_per_stream"],
            "wrong_per_stream": p["wrong_per_stream"], "refused": p["refused"],
            "hard_incidents_escalated_per_stream": p["hard_incidents_escalated_per_stream"],
        }
        if z != C.DEFAULT_Z:
            for kind in ("quality", "leak"):
                d, lo, hi = paired_across(run, C.SEL_RUNG, base, C.SEL_RUNG, kind)
                row[f"{kind}_minus_default"], row[f"{kind}_minus_default_lo"], row[f"{kind}_minus_default_hi"] = d, lo, hi
        # consistency: every incident the selection oracle called has an anchored notice
        called = arm.incidents.merge(inc[["seed", "incident", "noticed"]], on=["seed", "incident"])
        called = called[(called.tier == "hard") & (called.escalations > 0)]
        row["called_but_not_noticed"] = int((~called.noticed).sum())
        rows.append(row)
    df = pd.DataFrame(rows)
    df.to_csv(C.OUT / "r10-sweep.csv", index=False, float_format="%.6g")
    with pd.option_context("display.width", 250, "display.max_columns", 60):
        print(df.T.to_string())
    assert (df.called_but_not_noticed == 0).all()


# ---- where a never-noticed incident's observations went ----------------------------------------------


def elsewhere():
    """For each never-noticed hard incident: the noticed anomalies that hold some of its observations
    (the rung attached them, anchored on something else), the tier of what each is anchored on, and
    how long before the incident's first observation that anchor was. Supplementary: it says whether
    "never noticed" is the rung not seeing the incident or the rung anchoring what it saw elsewhere."""
    rows = []
    for name in ("diag", "heldout-z3"):
        for seed, rec in sorted(dump(name).items()):
            tier = {i["id"]: i["tier"] for i in rec["incidents"]}
            anchored = {n["anchor_incident"] for n in rec["notices"] if n["anchor_incident"] is not None}
            for inc in rec["incidents"]:
                if inc["tier"] != "hard" or inc["id"] in anchored:
                    continue
                holders = [n for n in rec["notices"] if inc["id"] in n["attached_incidents"]]
                kinds = sorted({"background" if n["anchor_incident"] is None else tier[n["anchor_incident"]] for n in holders})
                gaps = [(inc["first_at_ns"] - n["anchor_at_ns"]) / 1e9 for n in holders]
                rows.append({
                    "source": name, "seed": seed, "incident": inc["id"], "family": inc["family"], "mode": mode_of(inc),
                    "holding_anomalies": len(holders), "holder_anchor_tiers": "+".join(kinds) or "none",
                    "nearest_holder_anchor_before_first_obs_s": min(gaps, key=abs) if gaps else float("nan"),
                })
    df = pd.DataFrame(rows)
    df.to_csv(C.OUT / "r10-never-noticed-anchors.csv", index=False, float_format="%.6g")
    summ = (df.groupby(["source", "family", "holder_anchor_tiers"]).agg(n=("incident", "size"),
            median_gap_s=("nearest_holder_anchor_before_first_obs_s", "median")).reset_index())
    summ.to_csv(C.OUT / "r10-never-noticed-anchors-summary.csv", index=False, float_format="%.6g")
    print(summ.to_string(index=False))


if __name__ == "__main__":
    {"validate": validate, "decompose": decompose, "sweep": sweep, "elsewhere": elsewhere}.get(sys.argv[1] if len(sys.argv) > 1 else "", lambda: sys.exit(__doc__))()
