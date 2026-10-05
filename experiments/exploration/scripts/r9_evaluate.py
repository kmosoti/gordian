"""R9: run the frozen reader on a question set and compute the whole estimate.

Usage: r9_evaluate.py SET_DIR OUT_DIR [--prefix r9-] [--reader r9_reader] [--workers 3]
                      [--allow-dev]

SET_DIR holds hard.jsonl and plain.jsonl (`r9_select.py`). For both tiers, at the five levels and the
control: the reader's answer for every question (`r9_dev.answer_all`: the reader is shown the services,
the focus and the context's records, and nothing else), then (`r9_stats.analyse`) A(m) with 90% cluster
bootstrap intervals over streams, p0, delta*, its interval, the misfit and the reader precondition; the
accuracy by family and by mode at every level; and, for every wrong answer above m = 0, three
counterfactual contexts that say what the error rested on (see `ablations`). Outputs, in OUT_DIR:

  {prefix}answers-{tier}.csv     every answer (question, level, answer, correct, what the trace cited)
  {prefix}levels-{tier}.csv      A(m) per level with its interval, and the control
  {prefix}fit-{tier}.json        the whole `r9_stats.analyse` result
  {prefix}groups-{tier}.csv      accuracy by family and by mode at every level
  {prefix}fooled-{tier}.csv      the counterfactuals, one row per wrong answer above m = 0
  {prefix}fooled-summary.csv     their shares at each level (the share of wrong answers at m = 400 that
                                 a look-alike free-form message explains is the row `lookalike_ff`)

The labels are read in this script, as the evaluator reads them: to score an answer and to build the
counterfactual contexts. The reader never sees them. Refuses a set that contains development seeds
unless `--allow-dev` (the dry run on development questions).

Exploration (nothing here tests a hypothesis).
"""

import argparse
import csv
import importlib
import json
from collections import defaultdict
from pathlib import Path

import numpy as np

import r9_common as C
import r9_dev as D
import r9_stats as S

LEVELS = list(C.LEVELS)
FF_LIMIT = 1 << 16


def is_ff(rec):
    o = rec["obs"]
    return "Message" in o and o["Message"]["text_id"] >= FF_LIMIT


def ablations(q, ctx):
    """The three counterfactual contexts of a question's context `ctx` (records in time order):

    * `lookalike_ff`: every free-form message that is not decisive evidence of the focus incident is
      removed (a look-alike message is a free-form message of the background, of another incident, or
      of the focus incident's own non-decisive observations);
    * `only_own`: only the decisive evidence and the focus incident's own observations stay, so
      everything of the background and of other incidents is removed;
    * `full_own`: every one of the focus incident's own observations in the pool is added, so none
      of its burst, heartbeats or closure is missing.

    Labels are used here and only here."""
    dec = {r["id"] for r in q["decisive"]}
    roles = C.role_of(q)
    out = {}
    out["lookalike_ff"] = [r for r in ctx if r["id"] in dec or not is_ff(r)]
    out["only_own"] = [r for r in ctx if r["id"] in dec or roles.get(r["id"], "").startswith("own:")]
    have = {r["id"] for r in ctx}
    extra = [r for r in q["pool"] if roles[r["id"]].startswith("own:") and r["id"] not in have]
    out["full_own"] = sorted(list(ctx) + extra, key=lambda r: (r["at_ns"], r["id"]))
    return out


def run_tier(tier, qs, reader, workers):
    answers = D.answer_all(qs, reader.__name__, workers)
    rows = []
    for q, ans in zip(qs, answers):
        dec = {r["id"] for r in q["decisive"]}
        for lv, kind, site, ok, trace in ans:
            cited = trace.get("cited_ff", [])
            rows.append({
                "seed": q["seed"], "incident": q["incident"], "tier": tier, "family": q["family"],
                "mode": q["mode"] or "", "level": lv, "answer": kind, "site": site, "correct": int(ok),
                "decided": trace.get("decided", ""),
                "cited_ff": len(cited), "cited_lookalike_ff": sum(1 for i in cited if i not in dec),
            })
    return answers, rows


def write_csv(path, rows):
    with open(path, "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(rows[0].keys()))
        w.writeheader()
        w.writerows(rows)


def matrix(qs, answers):
    """(questions, 6) of 0/1: the five levels then the control."""
    return np.array([[int(a[3]) for a in ans] for ans in answers], dtype=int)


def levels_rows(tier, result):
    rows = []
    for k, lv in enumerate(LEVELS):
        lo, hi = result["A_interval"][k]
        rows.append({"tier": tier, "level": lv, "n": result["n_incidents"], "A": result["A"][k],
                     "lo90": lo, "hi90": hi})
    lo, hi = result["p0_interval"]
    rows.append({"tier": tier, "level": "control", "n": result["n_incidents"], "A": result["p0"],
                 "lo90": lo, "hi90": hi})
    return rows


def group_rows(tier, qs, answers):
    by = defaultdict(lambda: defaultdict(list))
    for q, ans in zip(qs, answers):
        keys = ["all", f"family={q['family']}"]
        if q["mode"]:
            keys += [f"mode={q['mode']}", f"{q['family']}/{q['mode']}"]
        for key in keys:
            for lv, _, _, ok, _ in ans:
                by[key][lv].append(int(ok))
    rows = []
    for key in sorted(by):
        for lv in LEVELS + ["control"]:
            v = by[key][lv]
            rows.append({"tier": tier, "group": key, "level": lv, "n": len(v), "correct": sum(v),
                         "accuracy": sum(v) / len(v)})
    return rows


def fooled_rows(tier, qs, answers, reader):
    rows = []
    for q, ans in zip(qs, answers):
        ctxs = C.question_contexts(q)
        for lv, kind, site, ok, trace in ans:
            if lv in (0, "control") or ok:
                continue
            abl = ablations(q, ctxs[lv])
            row = {"seed": q["seed"], "incident": q["incident"], "tier": tier, "family": q["family"],
                   "mode": q["mode"] or "", "level": lv, "answer": kind,
                   "cited_ff": len(trace.get("cited_ff", [])),
                   "cited_lookalike_ff": sum(1 for i in trace.get("cited_ff", [])
                                             if i not in {r["id"] for r in q["decisive"]})}
            for name, ctx in abl.items():
                k2, s2, _ = reader.read(q["services"], q["focus"], ctx)
                row[f"{name}_answer"] = k2
                row[f"{name}_repairs"] = int(C.correct((k2, s2), q))
            rows.append(row)
    return rows


def fooled_summary(all_rows, answers_by_tier):
    out = []
    for tier, rows in all_rows.items():
        wrong_by_level = defaultdict(int)
        for ans in answers_by_tier[tier]:
            for lv, _, _, ok, _ in ans:
                if lv not in (0, "control") and not ok:
                    wrong_by_level[lv] += 1
        for lv in LEVELS[1:]:
            rs = [r for r in rows if r["level"] == lv]
            n = len(rs)
            if n == 0:
                continue
            for name, cond in (
                ("trace_cites_a_lookalike_ff", lambda r: r["cited_lookalike_ff"] > 0),
                ("lookalike_ff", lambda r: r["lookalike_ff_repairs"]),
                ("only_own", lambda r: r["only_own_repairs"]),
                ("full_own", lambda r: r["full_own_repairs"]),
                ("lookalike_ff_or_only_own", lambda r: r["lookalike_ff_repairs"] or r["only_own_repairs"]),
            ):
                k = sum(1 for r in rs if cond(r))
                out.append({"tier": tier, "level": lv, "wrong_answers": n, "measure": name, "count": k,
                            "share_of_wrong": k / n})
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("set_dir")
    ap.add_argument("out_dir")
    ap.add_argument("--prefix", default="r9-")
    ap.add_argument("--reader", default="r9_reader")
    ap.add_argument("--workers", type=int, default=3)
    ap.add_argument("--allow-dev", action="store_true")
    a = ap.parse_args()
    reader = importlib.import_module(a.reader)
    out = Path(a.out_dir)
    out.mkdir(parents=True, exist_ok=True)
    all_fooled, answers_by_tier = {}, {}
    for tier in ("hard", "plain"):
        qs = list(C.iter_records(Path(a.set_dir) / f"{tier}.jsonl"))
        in_dev = [q["seed"] for q in qs if C.DEV_SEEDS[0] <= q["seed"] <= C.DEV_SEEDS[1]]
        if in_dev and not a.allow_dev:
            raise SystemExit("development seeds in an evaluation set; pass --allow-dev for a dry run")
        answers, rows = run_tier(tier, qs, reader, a.workers)
        answers_by_tier[tier] = answers
        write_csv(out / f"{a.prefix}answers-{tier}.csv", rows)
        res = S.analyse(matrix(qs, answers))
        res["tier"] = tier
        res["reader"] = a.reader
        (out / f"{a.prefix}fit-{tier}.json").write_text(json.dumps(res, indent=2) + "\n")
        write_csv(out / f"{a.prefix}levels-{tier}.csv", levels_rows(tier, res))
        write_csv(out / f"{a.prefix}groups-{tier}.csv", group_rows(tier, qs, answers))
        fr = fooled_rows(tier, qs, answers, reader)
        all_fooled[tier] = fr
        if fr:
            write_csv(out / f"{a.prefix}fooled-{tier}.csv", fr)
        print(f"{tier}: n={res['n_incidents']} A={[round(x, 3) for x in res['A']]} p0={res['p0']:.3f} "
              f"delta*={res['delta']:.4f} [{res['delta_interval'][0]:.4f}, {res['delta_interval'][1]:.4f}] "
              f"misfit={res['misfit']:.3f} precondition={res['precondition']['holds']}")
    write_csv(out / f"{a.prefix}fooled-summary.csv", fooled_summary(all_fooled, answers_by_tier))


if __name__ == "__main__":
    main()
