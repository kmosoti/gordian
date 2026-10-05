"""R5 trace: for a call about a hard incident, made after some of the incident's decisive evidence
had arrived, why does its context lack decisive evidence that has already arrived?

Usage: r5_trace.py DIAG_RUN_DIR LABELS_JSONL        (writes r5-trace*.csv under experiments/exploration/)

Exploration (nothing here tests a hypothesis). This is the one place the hidden decisive-evidence
labels are read, as the evaluator reads them (the file is `r5_labels`'s output, which reads them the
way `truth_from_stream` does); nothing here reaches an arm.

Inputs: a run of the diagnostic manifest (every ledger kept), and the labels of its streams.
For every arm that is not the privileged R4 oracle and every accepted escalation of its ledger:

* the call's incident is the one its focus observation is labelled with; only calls about hard
  incidents are traced (a decoy's and a plain incident's "decisive" observations are other things);
* the evidence that had arrived is the incident's decisive observations emitted no later than the
  step in which the call was made (steps start at multiples of the 500 ms quantum; observations
  are delivered up to the step's start);
* a call is `complete` if its context holds all of that, `before_evidence` if none had arrived,
  and otherwise `lacking`; each missing observation is classified, in this order:
    - `service_not_attached`: its service is neither the anomaly's site nor a dependent of it in the
      public graph, so the rung's context builder never admits it;
    - `outside_window`: the rung admits only observations from 2 s before the anomaly's anchor
      (`context_lookback_ns`), and at a dependent only within 400 ms after the anchor (`burst_ns`);
    - `context_cap`: the rung would have admitted it but the context held the first quarter and the
      most recent three quarters of `context_max_refs` = 128 and it fell in between;
    - `other`: none of the above.
  The rule is the rung's own (`Rung::context`, `Anomaly::admits`), re-derived here from the public
  graph and the observations; it is validated by predicting every traced call's whole context and
  comparing it with what the ledger recorded (`predicted_context_matches`).
"""

import csv
import json
import sys
from collections import Counter, defaultdict
from pathlib import Path

import r5_common as C

STEP_NS = 500_000_000
LOOKBACK_NS = 2_000_000_000
BURST_NS = 400_000_000
CAP = 128
FAMILIES = ("compound", "cascade", "split_brain", "slow_leak")
ORACLE = C.ORACLE


def dependents(services, site):
    """The services that depend on `site`, directly or transitively (edges point to lower ids)."""
    n = len(services)
    member = [False] * n
    reach = [False] * n
    if site >= n:
        return reach
    member[site] = True
    for i in range(site + 1, n):
        if any(member[d] for d in services[i]["depends_on"]):
            member[i] = True
            reach[i] = True
    return reach


def service_of(observation):
    """The service a passive observation is about, or None (probe results, corrections)."""
    (kind, body), = observation.items()
    if kind in ("Counter", "Message", "Snapshot"):
        return body["service"]
    return None


def read_blocks(path):
    """Yield, per sampled seed, (seed, public_info, observations, decisions, outcomes)."""
    cur = None
    for line in open(path):
        d = json.loads(line)
        rec = d.get("record")
        if rec == "segment":
            if cur:
                yield cur
            cur = {"seed": d["seed"], "public": d["public_info"], "obs": None, "decisions": {}, "outcomes": {}}
        elif rec == "public_stream":
            cur["obs"] = d["observations"]
        elif rec == "entry":
            if d["kind"] == "Decision":
                cur["decisions"][d["id"]] = d
            elif d["kind"] == "Outcome":
                for parent in d["inputs"]:
                    cur["outcomes"][parent] = d
    if cur:
        yield cur


def classify(arm_dir, labels, rows_obs, rows_call, stats):
    arm = arm_dir.name
    for blk in read_blocks(arm_dir / "events-sample.jsonl"):
        seed = blk["seed"]
        if seed not in labels:
            continue
        inc = {i["id"]: i for i in labels[seed]}
        owner = {}
        for i in labels[seed]:
            for o in i["observations"]:
                owner[o] = i["id"]
        services = blk["public"]["services"]
        at = [o["at_ns"] for o in blk["obs"]]
        svc = [service_of(o["observation"]) for o in blk["obs"]]
        for did, dec in sorted(blk["decisions"].items()):
            action = dec["payload"]["action"]
            if "Escalate" not in action:
                continue
            out = blk["outcomes"].get(did)
            if out is None or "Escalated" not in out["payload"]:
                continue  # refused: not a call
            esc = action["Escalate"]
            focus = esc["question"]["Diagnose"]["focus"]
            context = [r["Passive"] for r in esc["context"] if "Passive" in r]
            incident = owner.get(focus)
            T = dec["at_ns"]
            step_start = (T // STEP_NS) * STEP_NS
            tier = inc[incident]["tier"] if incident is not None else "background"
            stats["calls_total"][arm] += 1
            if tier != "hard":
                stats["calls_not_hard"][arm] += 1
                continue
            family = inc[incident]["family"]
            decisive = inc[incident]["decisive"]
            arrived = [m for m in decisive if at[m] <= step_start]
            in_ctx = set(context)
            s0, a0 = svc[focus], at[focus]
            region = dependents(services, s0) if s0 is not None else []
            cand = [
                h for h in range(len(at))
                if at[h] <= step_start and svc[h] is not None and at[h] >= a0 - LOOKBACK_NS
                and (svc[h] == s0 or (region[svc[h]] and at[h] <= a0 + BURST_NS))
            ]
            dropped = set()
            if len(cand) > CAP:
                kept = cand[: CAP // 4] + cand[-(CAP - CAP // 4):]
                dropped = set(cand) - set(kept)
                cand = kept
            match = cand == context
            stats["predicted_total"][arm] += 1
            stats["predicted_match"][arm] += int(match)
            missing = [m for m in arrived if m not in in_ctx]
            status = "before_evidence" if not arrived else ("complete" if not missing else "lacking")
            rows_call.append(
                {
                    "arm": arm, "seed": seed, "incident": incident, "family": family,
                    "call_at_s": T / 1e9, "onset_to_call_s": (T - inc[incident]["onset_ns"]) / 1e9,
                    "decisive": len(decisive), "arrived": len(arrived), "in_context": len(arrived) - len(missing),
                    "context_refs": len(context), "status": status, "predicted_context_matches": match,
                }
            )
            reasons_here = set()
            for m in missing:
                sm, tm = svc[m], at[m]
                if sm != s0 and (sm is None or not region[sm]):
                    why = "service_not_attached"
                elif tm < a0 - LOOKBACK_NS or (sm != s0 and tm > a0 + BURST_NS):
                    why = "outside_window"
                elif m in dropped:
                    why = "context_cap"
                else:
                    why = "other"
                reasons_here.add(why)
                rows_obs.append(
                    {"arm": arm, "seed": seed, "incident": incident, "family": family, "obs": m,
                     "service": sm, "site": s0, "dt_from_anchor_s": (tm - a0) / 1e9, "reason": why}
                )
            stats["reasons_in_call"][(arm, family, status, tuple(sorted(reasons_here)))] += 1


def main():
    run = Path(sys.argv[1])
    labels = {}
    for line in open(sys.argv[2]):
        d = json.loads(line)
        labels[d["seed"]] = d["incidents"]
    stats = {k: Counter() for k in ("calls_total", "calls_not_hard", "predicted_total", "predicted_match",
                                    "reasons_in_call")}
    rows_obs, rows_call = [], []
    arms = sorted(p.name for p in run.iterdir() if p.is_dir() and (p / "events-sample.jsonl").exists())
    for arm in arms:
        if arm == ORACLE:
            continue
        classify(run / arm, labels, rows_obs, rows_call, stats)
    out = C.OUT
    for name, rows in (("r5-trace-calls.csv", rows_call), ("r5-trace-missing.csv", rows_obs)):
        with open(out / name, "w", newline="") as fh:
            w = csv.DictWriter(fh, fieldnames=list(rows[0].keys()))
            w.writeheader()
            w.writerows(rows)
    # summaries
    summary = []
    by_call = defaultdict(Counter)
    for r in rows_call:
        by_call[(r["arm"], r["family"])][r["status"]] += 1
    by_obs = defaultdict(Counter)
    for r in rows_obs:
        by_obs[(r["arm"], r["family"])][r["reason"]] += 1
    call_reason = defaultdict(Counter)
    for (arm, family, status, reasons), n in stats["reasons_in_call"].items():
        if status != "lacking":
            continue
        for why in reasons:
            call_reason[(arm, family)][why] += n
    keys = sorted(set(by_call) | set(by_obs))
    for (arm, family) in keys + [(arm, "ALL") for arm in arms if arm != ORACLE] + [("ALL ARMS", f) for f in FAMILIES] + [("ALL ARMS", "ALL")]:
        def pool(table):
            acc = Counter()
            for (a, f), c in table.items():
                if (arm in (a, "ALL ARMS")) and (family in (f, "ALL")):
                    acc.update(c)
            return acc
        cs, ob, cr = pool(by_call), pool(by_obs), pool(call_reason)
        summary.append(
            {
                "arm": arm, "family": family,
                "calls_about_hard": sum(cs.values()),
                "before_evidence": cs["before_evidence"], "complete": cs["complete"], "lacking": cs["lacking"],
                "missing_obs": sum(ob.values()),
                "obs_service_not_attached": ob["service_not_attached"], "obs_outside_window": ob["outside_window"],
                "obs_context_cap": ob["context_cap"], "obs_other": ob["other"],
                "calls_with_service_not_attached": cr["service_not_attached"],
                "calls_with_outside_window": cr["outside_window"],
                "calls_with_context_cap": cr["context_cap"], "calls_with_other": cr["other"],
            }
        )
    seen = set()
    uniq = []
    for r in summary:
        k = (r["arm"], r["family"])
        if k not in seen:
            seen.add(k)
            uniq.append(r)
    with open(out / "r5-trace.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(uniq[0].keys()))
        w.writeheader()
        w.writerows(uniq)
    for arm in arms:
        if arm == ORACLE:
            continue
        print(f"{arm}: calls {stats['calls_total'][arm]}, not about hard {stats['calls_not_hard'][arm]}, "
              f"predicted context matches {stats['predicted_match'][arm]} of {stats['predicted_total'][arm]}")
    for r in uniq:
        if r["arm"] == "ALL ARMS":
            print(r)


if __name__ == "__main__":
    main()
