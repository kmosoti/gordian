"""R6 re-trace: for calls about hard incidents, with a context builder's context, which decisive
evidence that had already arrived is still missing, and why?

Usage: r6_trace.py DIAG_RUN_DIR LABELS_JSONL     (writes r6-trace*.csv under experiments/exploration/)

Exploration (nothing here tests a hypothesis). This is the one place the hidden decisive-evidence
labels are read, as the evaluator reads them (`LABELS_JSONL` is `r5_labels`'s output); nothing here
reaches an arm. It extends R5's trace (`r5_trace.py`, same inputs, same "arrived" rule, same
classification of why the rung's context misses evidence) to the builders of R6.

For every arm that is not a privileged ceiling (`oracle_escalation_privileged`,
`oracle_selection_context_*`: their contexts are the evidence by construction) and every accepted
escalation in its ledger whose focus belongs to a hard incident:

* the evidence that had arrived is the incident's decisive observations emitted no later than the
  step the call was made in (steps start at multiples of 500 ms), as in R5;
* the call is `complete` if its context holds all of it, `before_evidence` if none had arrived,
  otherwise `lacking`;
* each decisive observation that had arrived is classified by what R5's trace said of the *rung's*
  context for the same anomaly (`rung_class`, a property of the observation and the anomaly, not of
  the builder): `attached` (the rung would have admitted it), `service_not_attached` (its service is
  neither the anomaly's site nor a dependent of it in the public graph), `outside_window` (older
  than 2 s before the anchor, or at a dependent more than 400 ms after it);
* an arrived decisive observation absent from the builder's context is, for that builder, either
  `context_cap` (the builder's rule admits it and the cap dropped it) or `not_admitted` (its rule
  excludes it: service or time).

"Did the builder fix `service_not_attached`, `outside_window`, or neither" is then the share of
each class that is in the builder's context, against the same classes for the rung's own context
(the `rung` rows, the R5 trace on this run's streams).

**Validation.** Every traced call's whole context is predicted from the arm's builder, the public
graph and the observations, by an independent re-implementation here (not the Rust code), and
compared with what the ledger recorded: `predicted_context_matches`, summed in the printed output.
"""

import csv
import json
import sys
from collections import Counter, defaultdict
from pathlib import Path

import r6_common as C
from r5_trace import dependents, read_blocks, service_of

STEP_NS = 500_000_000
LOOKBACK_NS = 2_000_000_000
BURST_NS = 400_000_000
BURST_GAP_NS = 2_000_000_000
RUNG_CAP = 128
HIGH = 50
CHECK_HEALTH = 0x100 + 7
CATALOGUE = {0x100 + i for i in range(8)}
CEILING_PREFIXES = ("oracle_escalation_privileged", "oracle_selection_context")
OUT = C.OUT


def builder_of(manifest, arm):
    """The builder an arm of the diagnostic manifest uses: ('rung',) or (kind, params dict)."""
    for a in manifest["arms"]:
        if a["arm"] == arm:
            ctx = a.get("context") or manifest["rung"].get("context")
            if not ctx:
                return ("rung", {})
            kind = ctx["builder"]
            return (kind, {k: v for k, v in ctx.items() if k != "builder"})
    raise KeyError(arm)


def is_abnormal(obs, services):
    (kind, body), = obs.items()
    if kind == "Counter":
        return body["value"] >= HIGH
    if kind == "Message":
        tid = body["text_id"]
        return tid in CATALOGUE and tid != CHECK_HEALTH
    if kind == "Snapshot":
        return services[body["service"]]["config_hash"] != body["config_hash"]
    return False


def within_hops(services, site, hops):
    n = len(services)
    dist = [None] * n
    dist[site] = 0
    for h in range(hops):
        for i, s in enumerate(services):
            for d in s["depends_on"]:
                if dist[i] == h and dist[d] is None:
                    dist[d] = h + 1
                if dist[d] == h and dist[i] is None:
                    dist[i] = h + 1
    return [x is not None for x in dist]


def cap_head_tail(refs, cap):
    if len(refs) <= cap:
        return refs
    head = cap // 4
    return refs[:head] + refs[len(refs) - (cap - head):]


def predict(builder, services, at, svc, abn, step_start, focus):
    """The builder's context (a list of observation ids) and the ids it admits before the cap,
    for the anomaly whose anchor is `focus`, at the step starting at `step_start`."""
    kind, p = builder
    delivered = [h for h in range(len(at)) if at[h] <= step_start]
    s0, a0 = svc[focus], at[focus]
    if kind == "rung":
        region = dependents(services, s0)
        cand = [h for h in delivered if svc[h] is not None and at[h] >= a0 - LOOKBACK_NS
                and (svc[h] == s0 or (region[svc[h]] and at[h] <= a0 + BURST_NS))]
        return cap_head_tail(cand, RUNG_CAP), cand
    if kind == "window":
        floor = step_start - p["window_ns"]
        cand = [h for h in delivered if at[h] >= floor]
        return list(reversed(cand))[: p["max_refs"]], cand
    if kind == "cooccur":
        member = [False] * len(services)
        member[s0] = True
        lo, hi = a0 - p["delta_ns"], a0 + p["delta_ns"]
        last = {}
        for h in delivered:
            if svc[h] is None or not abn[h]:
                continue
            prev = last.get(svc[h])
            if (prev is None or at[h] - prev >= BURST_GAP_NS) and lo <= at[h] <= hi:
                member[svc[h]] = True
            last[svc[h]] = at[h]
        cand = [h for h in delivered if at[h] >= a0 - LOOKBACK_NS and svc[h] is not None and member[svc[h]]]
        return cap_head_tail(cand, p["max_refs"]), cand
    if kind == "neighbourhood":
        member = within_hops(services, s0, p["hops"])
        cand = [h for h in delivered if at[h] >= a0 - LOOKBACK_NS and svc[h] is not None and member[svc[h]]]
        return cap_head_tail(cand, p["max_refs"]), cand
    raise KeyError(kind)


def classify(run, arm, builder, labels, rows_obs, rows_call, stats):
    for blk in read_blocks(run / arm / "events-sample.jsonl"):
        seed = blk["seed"]
        if seed not in labels:
            continue
        inc = {i["id"]: i for i in labels[seed]}
        owner = {o: i["id"] for i in labels[seed] for o in i["observations"]}
        services = blk["public"]["services"]
        at = [o["at_ns"] for o in blk["obs"]]
        svc = [service_of(o["observation"]) for o in blk["obs"]]
        abn = [is_abnormal(o["observation"], services) for o in blk["obs"]]
        for did, dec in sorted(blk["decisions"].items()):
            action = dec["payload"]["action"]
            if "Escalate" not in action:
                continue
            out = blk["outcomes"].get(did)
            if out is None or "Escalated" not in out["payload"]:
                continue
            esc = action["Escalate"]
            focus = esc["question"]["Diagnose"]["focus"]
            context = [r["Passive"] for r in esc["context"] if "Passive" in r]
            incident = owner.get(focus)
            T = dec["at_ns"]
            step_start = (T // STEP_NS) * STEP_NS
            tier = inc[incident]["tier"] if incident is not None else "background"
            stats["calls_total"][arm] += 1
            if tier != "hard":
                continue
            family = inc[incident]["family"]
            decisive = inc[incident]["decisive"]
            arrived = [m for m in decisive if at[m] <= step_start]
            in_ctx = set(context)
            s0, a0 = svc[focus], at[focus]
            region = dependents(services, s0)
            predicted, admitted = predict(builder, services, at, svc, abn, step_start, focus)
            match = predicted == context
            stats["predicted_total"][arm] += 1
            stats["predicted_match"][arm] += int(match)
            admitted = set(admitted)
            dropped = admitted - set(context)
            missing = [m for m in arrived if m not in in_ctx]
            status = "before_evidence" if not arrived else ("complete" if not missing else "lacking")
            rows_call.append({
                "arm": arm, "kind": builder[0], "seed": seed, "incident": incident, "family": family,
                "call_at_s": T / 1e9, "onset_to_call_s": (T - inc[incident]["onset_ns"]) / 1e9,
                "decisive": len(decisive), "arrived": len(arrived), "in_context": len(arrived) - len(missing),
                "context_refs": len(context), "status": status, "predicted_context_matches": match,
            })
            for m in arrived:
                sm, tm = svc[m], at[m]
                if sm != s0 and (sm is None or not region[sm]):
                    rung_class = "service_not_attached"
                elif tm < a0 - LOOKBACK_NS or (sm != s0 and tm > a0 + BURST_NS):
                    rung_class = "outside_window"
                else:
                    rung_class = "attached"
                present = m in in_ctx
                why = ""
                if not present:
                    why = "context_cap" if m in dropped else "not_admitted"
                rows_obs.append({
                    "arm": arm, "kind": builder[0], "seed": seed, "incident": incident, "family": family,
                    "obs": m, "service": sm, "site": s0, "dt_from_anchor_s": (tm - a0) / 1e9,
                    "age_at_call_s": (step_start - tm) / 1e9,
                    "rung_class": rung_class, "in_context": present, "missing_reason": why,
                })


def main():
    run = Path(sys.argv[1])
    labels = {}
    for line in open(sys.argv[2]):
        d = json.loads(line)
        labels[d["seed"]] = d["incidents"]
    manifest = json.load(open(run / "manifest.json"))
    stats = {k: Counter() for k in ("calls_total", "predicted_total", "predicted_match")}
    rows_obs, rows_call = [], []
    arms = sorted(p.name for p in run.iterdir() if p.is_dir() and (p / "events-sample.jsonl").exists())
    traced = [a for a in arms if not a.startswith(CEILING_PREFIXES)]
    for arm in traced:
        classify(run, arm, builder_of(manifest, arm), labels, rows_obs, rows_call, stats)
    for name, rows in (("r6-trace-calls.csv", rows_call), ("r6-trace-observations.csv", rows_obs)):
        with open(OUT / name, "w", newline="") as fh:
            w = csv.DictWriter(fh, fieldnames=list(rows[0].keys()))
            w.writeheader()
            w.writerows(rows)
    summary = []
    calls_by = defaultdict(Counter)
    for r in rows_call:
        calls_by[(r["arm"], r["kind"], r["family"])][r["status"]] += 1
    obs_by = defaultdict(Counter)
    for r in rows_obs:
        k = (r["arm"], r["kind"], r["family"])
        obs_by[k]["arrived"] += 1
        obs_by[k][f"{r['rung_class']}"] += 1
        if r["in_context"]:
            obs_by[k]["in_context"] += 1
            obs_by[k][f"{r['rung_class']}_in"] += 1
        else:
            obs_by[k][f"missing_{r['missing_reason']}"] += 1
            obs_by[k][f"{r['rung_class']}_missing"] += 1
    ctx_sizes = defaultdict(list)
    for r in rows_call:
        ctx_sizes[(r["arm"], r["kind"], r["family"])].append(r["context_refs"])
    keys = sorted(set(calls_by) | set(obs_by))
    allfam = ("compound", "cascade", "split_brain", "slow_leak", "ALL")
    for arm in traced:
        kind = builder_of(manifest, arm)[0]
        for fam in allfam:
            cs, ob, sz = Counter(), Counter(), []
            for (a, k, f) in keys:
                if a == arm and fam in (f, "ALL"):
                    cs.update(calls_by[(a, k, f)])
                    ob.update(obs_by[(a, k, f)])
                    sz += ctx_sizes[(a, k, f)]
            if not cs and not ob:
                continue
            summary.append({
                "arm": arm, "kind": kind, "family": fam,
                "calls_about_hard": sum(cs.values()),
                "before_evidence": cs["before_evidence"], "complete": cs["complete"], "lacking": cs["lacking"],
                "mean_context_refs": sum(sz) / len(sz) if sz else float("nan"),
                "arrived_obs": ob["arrived"], "in_context": ob["in_context"],
                "missing": ob["missing_context_cap"] + ob["missing_not_admitted"],
                "missing_context_cap": ob["missing_context_cap"], "missing_not_admitted": ob["missing_not_admitted"],
                "attached": ob["attached"], "attached_in": ob["attached_in"],
                "service_not_attached": ob["service_not_attached"], "service_not_attached_in": ob["service_not_attached_in"],
                "outside_window": ob["outside_window"], "outside_window_in": ob["outside_window_in"],
            })
    with open(OUT / "r6-trace.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(summary[0].keys()))
        w.writeheader()
        w.writerows(summary)
    for arm in traced:
        print(f"{arm}: calls {stats['calls_total'][arm]}, predicted context matches "
              f"{stats['predicted_match'][arm]} of {stats['predicted_total'][arm]}")


if __name__ == "__main__":
    main()
