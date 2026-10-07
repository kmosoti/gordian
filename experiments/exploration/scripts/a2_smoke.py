"""A2's smoke tables, from the run's files only (public side): the learner's edges and predictions by
band, by stream and by time in the stream, the public follow rate against two public chance levels,
the joinable prediction files for Lab 3, and the decision columns against the memoryless arm.

Usage: a2_smoke.py   (reads artifacts/runs/a2/a2-smoke-b5-rho0.7; writes, in the run directory,
                      predictions-<form>.csv, first-alarms-<form>.csv and edges-<form>.csv, and in
                      experiments/exploration/: a2-smoke.csv, a2-smoke-streams.csv,
                      a2-smoke-time.csv, a2-smoke-decisions.csv)

Written before the run; nothing in it is chosen against what the run shows. Every number is read
from the trace files (`_trace/anticipation-trace-<key>.csv`), which hold only what the arm saw,
and from `incidents.csv` and `results.csv` for the decision columns. Nothing here reads the hidden
side; Lab 3 (W3, item 4) joins `predictions-<form>.csv` to it.

- **Edges learned** per stream: distinct ordered pairs `(a, b)` with a `held` mark in the stream, by
  band and in any band (a pair is read at every trial at `a`, so an edge held only between two
  trials at `a` is never seen, and is never used).
- **Predictions**: the `prediction` rows; **followed** on the public side: a first alarm at the
  predicted service within the band after the predicting alarm (`followed` rows). Lead times: from
  the step that made the prediction to the partner's alarm (negative when the partner's alarm was
  delivered in the same step, before the prediction's step instant), and from the predicting alarm.
- **Two public chance levels** for the follow rate: (1) every trial's follow rate in the band (all
  `follow` and `miss` rows: what any partner that was quiet did), beside the mean chance `q` the
  layer assigned (from the evidence values: a follow carries `1 - q`, a miss `-q`); (2) a
  permutation: for each prediction, the share of the other services unconnected to the predicting
  one that had a first alarm in the same window.
- **By time in the stream**: six 100 s bins of the predicting alarm's instant.
- **Decisions** (`a2-smoke-decisions.csv`): per incident, the decision columns of each layer arm
  against `m3` (`correct_declarations`, `wrong_declarations`, `correct_by_deadline`, `missed`,
  `critical_miss`, `escalations`, `informed_escalations`, `correct_escalations`), and the timing
  columns (`first_correct_at_ns`, `time_to_first_correct_ns`) separately; per stream, the
  `bill_compute` difference and reasoner calls.
"""

import csv
import json
import statistics
from collections import Counter, defaultdict

import a2_common as C

RUN = C.RUN_DIR / C.run_id("smoke")
TRACE = RUN / C.TRACE_DIR
BAND_NAMES = ["0.4s", "2s", "10s"]
FORMS = ["a2_learn", "a2_raw"]
DECISION = ["correct_declarations", "wrong_declarations", "correct_by_deadline", "missed",
            "critical_miss", "escalations", "informed_escalations", "correct_escalations"]
TIMING = ["first_correct_at_ns", "time_to_first_correct_ns"]
BIN_NS = 100 * C.NS


def rows(path):
    with open(path) as fh:
        return list(csv.DictReader(fh))


def write(path, header, data):
    with open(path, "w", newline="") as fh:
        w = csv.DictWriter(fh, header)
        w.writeheader()
        w.writerows(data)
    print(path)


def rate(n, d):
    return round(n / d, 4) if d else ""


def load_trace(form, seeds):
    """The trace of `form`, rows by kind, with `seed` added."""
    t = rows(TRACE / f"anticipation-trace-{C.KEYS[form]}.csv")
    ends = [r for r in t if r["kind"] == "segment_end"]
    assert len(ends) == len(seeds), f"{form}: {len(ends)} segment_end rows for {len(seeds)} seeds"
    by = defaultdict(list)
    for r in t:
        r["seed"] = seeds[int(r["segment"])]
        by[r["kind"]].append(r)
    return by


def analyse(form, seeds):
    by = load_trace(form, seeds)
    seg_of = {s: i for i, s in enumerate(seeds)}
    # First alarms by (seed, obs) and by (seed, service), in time order.
    alarm = {}
    alarms_at = defaultdict(list)
    for r in by["first_alarm"]:
        alarm[(r["seed"], r["obs"])] = r
        alarms_at[(r["seed"], int(r["service"]))].append(int(r["at_ns"]))
    write(RUN / f"first-alarms-{form}.csv", ["seed", "at_ns", "service", "obs", "counted"],
          [{"seed": r["seed"], "at_ns": r["at_ns"], "service": r["service"], "obs": r["obs"],
            "counted": r["value"]} for r in by["first_alarm"]])
    # Pairs per segment (level_end rows name every pair cell).
    partners = defaultdict(set)
    for r in by["level_end"]:
        partners[(r["seed"], int(r["service"]))].add(int(r["partner"]))
    pairs_per_stream = [len({(int(r["service"]), int(r["partner"])) for r in by["level_end"]
                             if r["seed"] == s}) for s in seeds]
    # Resolutions of predictions.
    res = {}
    for r in by["followed"]:
        res[(r["seed"], r["obs"], r["partner"])] = r
    expired = {(r["seed"], r["obs"], r["partner"]) for r in by["expired"]}
    end_at = {r["seed"]: int(r["at_ns"]) for r in by["segment_end"]}
    preds = []
    for r in by["prediction"]:
        key = (r["seed"], r["obs"], r["partner"])
        a_alarm = alarm[(r["seed"], r["obs"])]
        k = int(r["band"])
        w = C.BANDS_NS[k]
        t_a = int(a_alarm["at_ns"])
        f = res.get(key)
        # A prediction whose window runs past the segment's last step is neither followed nor
        # expired (added after the run, reporting only: the stream ends inside its window).
        unresolved = f is None and key not in expired
        assert not (f is not None and key in expired), f"{form}: resolved twice: {key}"
        assert not unresolved or t_a + w > end_at[r["seed"]], f"{form}: unresolved: {key}"
        b_alarm = alarm.get((r["seed"], f["value"])) if f else None
        # Permutation chance: other services unconnected to a with a first alarm in the window.
        others = sorted(partners[(r["seed"], int(r["service"]))] - {int(r["partner"])})
        hits = [any(t_a < t <= t_a + w for t in alarms_at[(r["seed"], o)]) for o in others]
        preds.append({
            "seed": r["seed"], "segment": seg_of[r["seed"]], "made_at_ns": r["at_ns"],
            "alarm_at_ns": t_a, "predicting_service": r["service"],
            "predicted_service": r["partner"], "band": k, "band_ns": w, "alarm_obs": r["obs"],
            "anomaly": r["value"], "followed": int(f is not None),
            "unresolved_at_stream_end": int(unresolved),
            "follow_obs": f["value"] if f else "", "follow_at_ns": f["at_ns"] if f else "",
            "lead_from_alarm_ns": int(f["at_ns"]) - t_a if f else "",
            "lead_from_made_ns": int(f["at_ns"]) - int(r["at_ns"]) if f else "",
            "follow_counted": b_alarm["value"] if b_alarm else "",
            "permutation_chance": rate(sum(hits), len(hits)),
        })
    write(RUN / f"predictions-{form}.csv", list(preds[0].keys()) if preds else
          ["seed"], preds)
    # Edges held, per stream, pair and band.
    edges = {}
    for r in by["held"]:
        key = (r["seed"], int(r["service"]), int(r["partner"]), int(r["band"]))
        e = edges.setdefault(key, {"seed": r["seed"], "a": key[1], "b": key[2], "band": key[3],
                                   "first_held_at_ns": int(r["at_ns"]), "reads_held": 0,
                                   "max_level_milli": -10**9})
        e["reads_held"] += 1
        e["max_level_milli"] = max(e["max_level_milli"], int(r["value"]))
    write(RUN / f"edges-{form}.csv", ["seed", "a", "b", "band", "first_held_at_ns", "reads_held",
                                      "max_level_milli"], list(edges.values()))
    # Evidence: every trial's follow rate and mean chance, by band.
    ev = defaultdict(lambda: [0, 0, 0.0])  # follows, misses, sum of q
    for kind in ("follow", "miss"):
        for r in by[kind]:
            k = int(r["band"])
            v = int(r["value"]) / 1000.0
            ev[k][0 if kind == "follow" else 1] += 1
            ev[k][2] += (1.0 - v) if kind == "follow" else -v
    return by, preds, edges, ev, pairs_per_stream


def table(form, seeds, by, preds, edges, ev, pairs_per_stream):
    n = len(seeds)
    out = []
    for k in [0, 1, 2, "any"]:
        ps = [p for p in preds if k == "any" or p["band"] == k]
        fol = sum(p["followed"] for p in ps)
        leads = sorted(p["lead_from_made_ns"] for p in ps if p["followed"])
        e_k = {(e["seed"], e["a"], e["b"]) for e in edges.values() if k == "any" or e["band"] == k}
        perm = [p["permutation_chance"] for p in ps if p["permutation_chance"] != ""]
        f, m, q = ev[k] if k != "any" else [sum(ev[j][i] for j in range(3)) for i in range(3)]
        out.append({
            "arm": form, "band": BAND_NAMES[k] if k != "any" else "any", "streams": n,
            "pair_cells_pairs_per_stream": round(sum(pairs_per_stream) / n, 2),
            "edges_learned_per_stream": round(len(e_k) / n, 2),
            "predictions": len(ps), "predictions_per_stream": round(len(ps) / n, 2),
            "followed": fol, "follow_rate": rate(fol, len(ps)),
            "follow_rate_followed_by_counted_alarm": rate(
                sum(1 for p in ps if p["follow_counted"] == "1"), len(ps)),
            "all_trials": f + m, "all_trials_follow_rate": rate(f, f + m),
            "all_trials_mean_chance": round(q / (f + m), 4) if f + m else "",
            "permutation_chance": round(statistics.mean(perm), 4) if perm else "",
            "lead_from_made_median_ms": round(leads[len(leads) // 2] / 1e6, 1) if leads else "",
            "followed_not_ahead_of_the_step": sum(1 for x in leads if x <= 0),
            "predictions_with_anomaly": sum(1 for p in ps if p["anomaly"] != "-1"),
            "unresolved_at_stream_end": sum(p["unresolved_at_stream_end"] for p in ps),
        })
    streams = []
    for i, s in enumerate(seeds):
        ps = [p for p in preds if p["seed"] == s]
        row = {"arm": form, "segment": i, "seed": s,
               "first_alarms": sum(1 for r in by["first_alarm"] if r["seed"] == s),
               "counted": sum(1 for r in by["first_alarm"] if r["seed"] == s and r["value"] == "1"),
               "trial_bands_resolved": sum(1 for kind in ("follow", "miss") for r in by[kind]
                                          if r["seed"] == s),
               "pair_cells_pairs": pairs_per_stream[i],
               "edges_any": len({(e["a"], e["b"]) for e in edges.values() if e["seed"] == s})}
        for k, name in enumerate(BAND_NAMES):
            pk = [p for p in ps if p["band"] == k]
            row[f"edges_{name}"] = len({(e["a"], e["b"]) for e in edges.values()
                                         if e["seed"] == s and e["band"] == k})
            row[f"predictions_{name}"] = len(pk)
            row[f"followed_{name}"] = sum(p["followed"] for p in pk)
        streams.append(row)
    time_rows = []
    for b in range(6):
        ps = [p for p in preds if b * BIN_NS <= int(p["alarm_at_ns"]) < (b + 1) * BIN_NS]
        held = [r for r in by["held"] if b * BIN_NS <= int(r["at_ns"]) < (b + 1) * BIN_NS]
        row = {"arm": form, "from_s": b * 100, "to_s": (b + 1) * 100, "held_marks": len(held),
               "predictions": len(ps), "followed": sum(p["followed"] for p in ps),
               "follow_rate": rate(sum(p["followed"] for p in ps), len(ps))}
        for k, name in enumerate(BAND_NAMES):
            pk = [p for p in ps if p["band"] == k]
            row[f"predictions_{name}"] = len(pk)
            row[f"follow_rate_{name}"] = rate(sum(p["followed"] for p in pk), len(pk))
        time_rows.append(row)
    return out, streams, time_rows


def decisions(seeds):
    base = C.arm_name("m3")
    inc0 = rows(RUN / base / "incidents.csv")
    res0 = {r["seed"]: r for r in rows(RUN / base / "results.csv")}
    out = []
    for form in FORMS:
        arm = C.arm_name(form)
        inc = rows(RUN / arm / "incidents.csv")
        res = {r["seed"]: r for r in rows(RUN / arm / "results.csv")}
        assert [(r["seed"], r["incident"]) for r in inc] == [(r["seed"], r["incident"]) for r in inc0]
        same = sum(all(a[c] == b[c] for c in DECISION) for a, b in zip(inc, inc0))
        same_t = sum(all(a[c] == b[c] for c in TIMING) for a, b in zip(inc, inc0))
        bill = [int(res[str(s)]["bill_compute"]) - int(res0[str(s)]["bill_compute"]) for s in seeds]
        calls = [int(res[str(s)]["reasoner_calls"]) - int(res0[str(s)]["reasoner_calls"])
                 for s in seeds]
        out.append({
            "arm": form, "incidents": len(inc), "decision_columns_equal": same,
            "timing_columns_equal": same_t,
            "bill_compute_added_ms_per_stream_mean": round(statistics.mean(bill) / 1e6, 3),
            "bill_compute_added_ms_per_stream_max": round(max(bill) / 1e6, 3),
            "reasoner_calls_difference": sum(calls),
            "stop_reasons": ";".join(sorted({res[str(s)]["stop_reason"] for s in seeds})),
            "escalations_refused": sum(int(res[str(s)]["escalations_refused"]) for s in seeds),
        })
    return out


def main():
    m = json.load(open(RUN / "manifest.json"))
    seeds = m["seeds"]
    smoke, streams, times = [], [], []
    for form in FORMS:
        by, preds, edges, ev, pps = analyse(form, seeds)
        a, b, c = table(form, seeds, by, preds, edges, ev, pps)
        smoke += a
        streams += b
        times += c
    out = C.OUT
    write(out / "a2-smoke.csv", list(smoke[0].keys()), smoke)
    write(out / "a2-smoke-streams.csv", list(streams[0].keys()), streams)
    write(out / "a2-smoke-time.csv", list(times[0].keys()), times)
    dec = decisions(seeds)
    write(out / "a2-smoke-decisions.csv", list(dec[0].keys()), dec)
    for r in smoke:
        print(r)
    for r in dec:
        print(r)
    print("prediction:", json.dumps(C.PREDICTION))


if __name__ == "__main__":
    main()
