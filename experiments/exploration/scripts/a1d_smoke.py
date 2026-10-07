"""A1d's smoke table, from the run's files only: the decision columns of `incidents.csv` and
`results.csv` by tier, paired against the memoryless arm under the same selector, A1c's
declaration-level count beside them, and the engram arms' trace counters.

Usage: a1d_smoke.py   (reads artifacts/runs/a1d/a1d-smoke-b5-rho0.7; writes, in
                       experiments/exploration/: a1d-smoke.csv, a1d-smoke-paired.csv,
                       a1d-trace.csv, a1d-trace-class.csv, a1d-asked.csv)

Written before the run; nothing in it is chosen against what the run shows.

- **Decision columns** (`incidents.csv`, per tier): incidents; `correct_by_deadline`; `missed`;
  `critical_miss`; "wrong only" (missed with a wrong declaration); escalated incidents; and A1c's
  declaration-level count, "unasked wrong" (a wrong declaration and no escalation), for continuity.
  From `results.csv`, per arm: reasoner calls, `reasoner_cost_ns`, `bill_compute` (the arm's
  modelled compute: the noticer's and the layer's charges, the checks), `total_cost_ns` (omits the
  noticer's charge for medium arms since M2; reported, not used) and reasoner cost plus
  `bill_compute`.
- **Paired** (`a1d-smoke-paired.csv`): every engram arm (and the bind-off control) minus `m3`
  under the same selector, summed over the 20 streams, with a 90% interval from 10,000 bootstrap
  resamples of whole streams (seed 20261007, the same resamples for every row); and, per incident,
  "displaced" (correct by deadline in `m3`, not in the arm) and "gained" (the reverse), by tier.
- **Trace** (`a1d-trace.csv`): per engram arm, from its trace file: answers by the outcome kind the
  arm sees (not an incident, a known kind, a hard kind) and what bind did with them; recalls made,
  offered, admitted, dropped (consistent verdict, standing declaration, overtaken), unmatched,
  redundant, declared, not declared; contradictions; engrams held at the last segment's end;
  segments traced. The trace is checked complete: 20 `segment_end` rows per file.
- **By class, after the run** (`a1d-trace-class.csv`): the offered, admitted and declared recalls
  joined to the evaluator's class of the anomaly (`selection_notices.csv`: background, plain, hard,
  leak, decoy), and the median instant of a declared recall after the anomaly's notice
  (`notice_events.csv`). The evaluator's columns are read here to report, never by an arm.
- **What the selectors asked about** (`a1d-asked.csv`): per arm, answers by the arm's own kinds
  (from the trace, engram arms only) beside the evaluator's calls by class (`selection.csv`).
"""

import csv
import random
import statistics
from collections import Counter, defaultdict

import a1d_common as C

RUN = C.RUN_DIR / C.run_id("smoke")
TRACE = RUN / C.TRACE_DIR
TIERS = ["plain", "hard", "decoy"]
BOOT = 10_000
BOOT_SEED = 20261007


def rows(path):
    with open(path) as fh:
        return list(csv.DictReader(fh))


def b(x):
    return x == "true"


def incident_counts(inc):
    """Decision counts over incident rows `inc`."""
    return {
        "incidents": len(inc),
        "correct_by_deadline": sum(b(r["correct_by_deadline"]) for r in inc),
        "missed": sum(b(r["missed"]) for r in inc),
        "critical_miss": sum(b(r["critical_miss"]) for r in inc),
        "wrong_only": sum(b(r["missed"]) and int(r["wrong_declarations"]) > 0 for r in inc),
        "escalated": sum(int(r["escalations"]) > 0 for r in inc),
        "unasked_wrong": sum(int(r["wrong_declarations"]) > 0 and int(r["escalations"]) == 0
                             for r in inc),
    }


COST = ["reasoner_calls", "reasoner_cost_ns", "bill_compute", "total_cost_ns"]


def cost_counts(res):
    out = {k: sum(int(r[k]) for r in res) for k in COST}
    out["reasoner_plus_bill_ns"] = out["reasoner_cost_ns"] + out["bill_compute"]
    return out


def per_stream(inc, res):
    """Per seed: the decision counts by tier and the cost counts."""
    out = {}
    for r in res:
        out[r["seed"]] = dict(cost_counts([r]))
    for seed in out:
        for tier in TIERS:
            c = incident_counts([r for r in inc if r["seed"] == seed and r["tier"] == tier])
            for k, v in c.items():
                out[seed][f"{tier}_{k}"] = v
    return out


PAIRED = (["plain_correct_by_deadline", "plain_missed", "plain_critical_miss", "plain_wrong_only",
           "plain_unasked_wrong", "hard_correct_by_deadline", "hard_missed", "hard_critical_miss",
           "hard_wrong_only", "hard_unasked_wrong", "decoy_unasked_wrong"]
          + COST + ["reasoner_plus_bill_ns"])


def boot_interval(diffs, idx):
    sums = sorted(sum(diffs[i] for i in sample) for sample in idx)
    return sums[int(0.05 * len(sums))], sums[int(0.95 * len(sums)) - 1]


def trace_rows(key):
    path = TRACE / f"engram-trace-{key}.csv"
    return rows(path) if path.exists() else None


def outcome_kind(tag):
    t = int(tag)
    return "not_incident" if t == 0 else ("known" if t <= 5 else "hard")


EVENTS = ["recall", "offered", "admitted", "gated_consistent", "gated_standing", "overtaken",
          "unmatched", "redundant", "confirmed", "declared", "not_declared"]
NOTES = ["created", "strengthened", "generalised", "too_few", "refused", "no_late", "elsewhere",
         "off", "unheld"]


def main():
    arms = C.arms()
    seeds = [str(s) for s in range(C.SMOKE_SEEDS[0], C.SMOKE_SEEDS[0] + C.SMOKE_SEEDS[1])]
    data = {}
    table = []
    for name, prefix, form, _, noticer in arms:
        inc = rows(RUN / name / "incidents.csv")
        res = rows(RUN / name / "results.csv")
        data[name] = (inc, res, per_stream(inc, res))
        for tier in TIERS + ["all"]:
            sub = inc if tier == "all" else [r for r in inc if r["tier"] == tier]
            row = {"arm": name, "selector": prefix, "form": form, "tier": tier}
            row.update(incident_counts(sub))
            row.update(cost_counts(res) if tier == "all" else {k: "" for k in COST + ["reasoner_plus_bill_ns"]})
            table.append(row)
    fields = list(table[0].keys())
    with open(C.OUT / "a1d-smoke.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, fields)
        w.writeheader()
        w.writerows(table)

    rng = random.Random(BOOT_SEED)
    idx = [[rng.randrange(len(seeds)) for _ in seeds] for _ in range(BOOT)]
    paired = []
    for name, prefix, form, _, _ in arms:
        if form == "m3":
            continue
        ctl = C.arm_name(prefix, "m3")
        a, c = data[name][2], data[ctl][2]
        row = {"arm": name, "selector": prefix, "form": form, "control": ctl}
        for k in PAIRED:
            diffs = [a[s][k] - c[s][k] for s in seeds]
            lo, hi = boot_interval(diffs, idx)
            row[k] = sum(diffs)
            row[f"{k}_lo90"] = lo
            row[f"{k}_hi90"] = hi
        ci = {(r["seed"], r["incident"]): r for r in data[ctl][0]}
        for tier in ("plain", "hard"):
            disp = gain = 0
            for r in data[name][0]:
                if r["tier"] != tier:
                    continue
                x, y = b(ci[(r["seed"], r["incident"])]["correct_by_deadline"]), b(r["correct_by_deadline"])
                disp += x and not y
                gain += y and not x
            row[f"{tier}_displaced"] = disp
            row[f"{tier}_gained"] = gain
        paired.append(row)
    with open(C.OUT / "a1d-smoke-paired.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, list(paired[0].keys()))
        w.writeheader()
        w.writerows(paired)

    traces = []
    by_class = []
    asked = []
    for name, prefix, form, _, noticer in arms:
        sel = rows(RUN / name / "selection.csv")
        ask = {"arm": name, "selector": prefix, "form": form}
        for cls in ("background", "plain", "hard", "leak", "decoy"):
            ask[f"eval_calls_{cls}"] = sum(int(r[f"calls_{cls}"]) for r in sel)
        if "engram" not in noticer:
            asked.append(ask)
            continue
        key = noticer["engram"]["state_key"]
        tr = trace_rows(key)
        if tr is None:
            print(f"{name}: no trace file for key {key}")
            asked.append(ask)
            continue
        t = {"arm": name, "selector": prefix, "form": form, "state_key": key}
        ev = Counter(r["event"] for r in tr)
        t["segments"] = ev["segment_end"]
        ends = [r for r in tr if r["event"] == "segment_end"]
        t["engrams_at_end"] = int(ends[-1]["value"]) if ends else ""
        answers = [r for r in tr if r["event"] == "answer"]
        t["answers"] = len(answers)
        for kind in ("not_incident", "known", "hard"):
            these = [r for r in answers if outcome_kind(r["outcome"]) == kind]
            t[f"answers_{kind}"] = len(these)
            t[f"bound_{kind}"] = sum(r["detail"] in ("created", "strengthened", "generalised")
                                     for r in these)
            ask[f"arm_answers_{kind}"] = len(these)
        for note in NOTES:
            t[f"bind_{note}"] = sum(r["detail"] == note for r in answers)
        t["contradicted"] = sum(int(r["value"]) for r in tr if r["event"] == "contradicted")
        for e in EVENTS:
            t[e] = ev[e]
        for kind in ("not_incident", "known", "hard"):
            t[f"admitted_{kind}"] = sum(r["event"] == "admitted" and outcome_kind(r["outcome"]) == kind
                                        for r in tr)
        traces.append(t)
        asked.append(ask)
        # By the evaluator's class of the anomaly, after the run.
        cls = {}
        for r in rows(RUN / name / "selection_notices.csv"):
            cls[(r["seed"], r["anomaly"])] = r["class"]
        noticed = {}
        for r in rows(RUN / name / "notice_events.csv"):
            if r["event"] == "notice":
                noticed[(r["seed"], r["anomaly"])] = int(r["at_ns"])
        counts = defaultdict(Counter)
        delays = defaultdict(list)
        for r in tr:
            if r["event"] not in ("offered", "admitted", "declared", "not_declared",
                                  "gated_consistent", "gated_standing", "overtaken"):
                continue
            seed = seeds[int(r["segment"])]
            k = (seed, r["anomaly"])
            c = cls.get(k, "unknown")
            counts[r["event"]][c] += 1
            if r["event"] == "declared" and k in noticed:
                delays[c].append((int(r["at_ns"]) - noticed[k]) / 1e9)
        for e, cnt in counts.items():
            for c, n in sorted(cnt.items()):
                by_class.append({"arm": name, "event": e, "class": c, "count": n,
                                 "median_s_after_notice": round(statistics.median(delays[c]), 2)
                                 if e == "declared" and delays[c] else ""})
    if traces:
        with open(C.OUT / "a1d-trace.csv", "w", newline="") as fh:
            w = csv.DictWriter(fh, list(traces[0].keys()))
            w.writeheader()
            w.writerows(traces)
    with open(C.OUT / "a1d-trace-class.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, ["arm", "event", "class", "count", "median_s_after_notice"])
        w.writeheader()
        w.writerows(by_class)
    keys = []
    for a in asked:
        keys += [k for k in a if k not in keys]
    with open(C.OUT / "a1d-asked.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, keys)
        w.writeheader()
        w.writerows(asked)

    # Completeness of the traces, and the oracle control against A1c's kept run.
    for t in traces:
        if t["segments"] != len(seeds):
            print(f"INCOMPLETE TRACE {t['arm']}: {t['segments']} segment_end rows")
    print("decision table (all tiers per arm in a1d-smoke.csv); paired against m3:")
    for p in paired:
        print(p["arm"], {k: p[k] for k in ("plain_correct_by_deadline", "hard_correct_by_deadline",
                                           "plain_critical_miss", "hard_critical_miss",
                                           "reasoner_calls", "reasoner_cost_ns", "bill_compute",
                                           "plain_unasked_wrong")})
    for t in traces:
        print("trace", t)


if __name__ == "__main__":
    main()
