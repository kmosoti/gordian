"""R9's direct check on real contexts: the reader on the contexts R6's builders actually produced.

Usage: r9_direct.py DIAG_RUN_DIR LABELS_JSONL FIT_JSON OUT_DIR [--reader r9_reader] [--prefix r9-]

DIAG_RUN_DIR is `artifacts/runs/r6/r6-diag-b5-rho0.7` (every ledger kept; read in place),
LABELS_JSONL the `r5_labels` output for its streams (`r6-diag-labels.jsonl`), FIT_JSON the hard
evaluation set's `r9_stats.analyse` result (`r9-fit-hard.json`). Writes `{prefix}direct.csv` (one row per
arm) and `{prefix}direct-calls.csv` (one row per call).

For each arm that is not a privileged ceiling or R4's oracle, and every accepted escalation in its
ledger whose focus belongs to a hard incident:

* the call's context is the passive observations its ledger entry lists, as records
  `{"id", "at_ns", "obs"}` in time order, the call's focus is its anomaly's anchor, and the services
  are the stream's public graph: exactly what `r9_reader.read` takes, and nothing else;
* the truth is the incident's family (`labels`) at the service of its first observation. The reader
  answers `(kind, service of the focus)`, so a call whose anchor is not at the incident's site counts
  as wrong in the strict reading; the kind-only reading is reported beside it, with the number of such
  calls;
* slow-leak incidents are out (the questions of R9 exclude them, and the reader never names a leak);
  their calls are counted and reported;
* the arm's mean references per call is the mean of the context's length over the counted calls, and
  its mean `m` the mean number of those references that are not decisive evidence of the focus
  incident (what the penalty of R7 would have counted).

The prediction. Fitted curve: `p0 + (A(0) - p0) exp(-delta* x / 100)` of the evaluation set's hard
questions, at `x` = the arm's mean references per call (the plan's wording; `x` = mean `m` is reported
beside it). Nonparametric curve: the evaluation set's A(m) by linear interpolation between its five
levels (flat beyond 400). The accuracy interval is a 90% cluster bootstrap over streams (10,000
resamples, seed 9901). Ceiling arms (`oracle_selection_context*`: the decisive evidence delivered so
far, no distractor) are reported as a labelled reference with x = 0.

Labels are read here, as the evaluator reads them, to score answers and count decisive references;
the reader never sees them.

Exploration (nothing here tests a hypothesis).
"""

import argparse
import csv
import importlib
import json
from collections import defaultdict
from pathlib import Path

import numpy as np

from r5_trace import read_blocks, service_of

FAMILY_KIND = {"compound": "Compound", "cascade": "Cascade", "split_brain": "SplitBrain"}
CEILING_PREFIX = "oracle_selection_context"
SKIP_PREFIX = ("oracle_escalation_privileged",)
SEED = 9901
B = 10_000


def records(blk, ids):
    obs = blk["obs"]
    return sorted(({"id": i, "at_ns": obs[i]["at_ns"], "obs": obs[i]["observation"]} for i in ids),
                  key=lambda r: (r["at_ns"], r["id"]))


def predict_exp(fit, x):
    a0, p0, d = fit["A"][0], fit["p0"], fit["delta"]
    return p0 + (a0 - p0) * np.exp(-d * x / 100.0)


def predict_interp(fit, x):
    return float(np.interp(x, [0, 50, 100, 200, 400], fit["A"]))


def calls_of(arm_dir, labels, reader):
    """One row per accepted escalation about a hard incident of one arm."""
    rows = []
    for blk in read_blocks(arm_dir / "events-sample.jsonl"):
        seed = blk["seed"]
        if seed not in labels:
            continue
        inc = {i["id"]: i for i in labels[seed]}
        owner = {o: i["id"] for i in labels[seed] for o in i["observations"]}
        services = blk["public"]["services"]
        for did, dec in sorted(blk["decisions"].items()):
            action = dec["payload"]["action"]
            if "Escalate" not in action:
                continue
            out = blk["outcomes"].get(did)
            if out is None or "Escalated" not in out["payload"]:
                continue
            esc = action["Escalate"]
            focus_id = esc["question"]["Diagnose"]["focus"]
            incident = owner.get(focus_id)
            if incident is None or inc[incident]["tier"] != "hard":
                continue
            info = inc[incident]
            ids = [r["Passive"] for r in esc["context"] if "Passive" in r]
            decisive = set(info["decisive"])
            row = {"arm": arm_dir.name, "seed": seed, "incident": incident, "family": info["family"],
                   "refs": len(ids), "m": sum(1 for i in ids if i not in decisive),
                   "decisive_in_context": sum(1 for i in ids if i in decisive)}
            site_true = service_of(blk["obs"][info["observations"][0]]["observation"])
            focus_site = service_of(blk["obs"][focus_id]["observation"])
            row["focus_at_site"] = int(focus_site == site_true)
            if info["family"] in FAMILY_KIND:
                kind, site, _ = reader.read(services, records(blk, [focus_id])[0], records(blk, ids))
                truth = FAMILY_KIND[info["family"]]
                row.update({"answer": kind, "answer_site": site, "kind_correct": int(kind == truth),
                            "correct": int(kind == truth and site == site_true)})
            rows.append(row)
    return rows


def interval(vals, seeds, rng_seed=SEED, b=B):
    """90% cluster bootstrap of the mean of `vals` over `seeds` (clusters)."""
    vals, seeds = np.asarray(vals, float), np.asarray(seeds)
    uniq = np.unique(seeds)
    sums = np.array([vals[seeds == s].sum() for s in uniq])
    cnts = np.array([(seeds == s).sum() for s in uniq])
    idx = np.random.default_rng(rng_seed).integers(0, len(uniq), size=(b, len(uniq)))
    m = sums[idx].sum(axis=1) / cnts[idx].sum(axis=1)
    return float(np.quantile(m, 0.05, method="lower")), float(np.quantile(m, 0.95, method="higher"))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("diag_run")
    ap.add_argument("labels")
    ap.add_argument("fit")
    ap.add_argument("out_dir")
    ap.add_argument("--reader", default="r9_reader")
    ap.add_argument("--prefix", default="r9-")
    a = ap.parse_args()
    reader = importlib.import_module(a.reader)
    fit = json.load(open(a.fit))
    labels = {}
    for line in open(a.labels):
        d = json.loads(line)
        labels[d["seed"]] = d["incidents"]
    run = Path(a.diag_run)
    arms = sorted(p.name for p in run.iterdir() if p.is_dir() and (p / "events-sample.jsonl").exists())
    arms = [x for x in arms if not x.startswith(SKIP_PREFIX)]
    all_calls, summary = [], []
    for arm in arms:
        rows = calls_of(run / arm, labels, reader)
        all_calls += rows
        scored = [r for r in rows if "correct" in r]
        leak = len(rows) - len(scored)
        ceiling = arm.startswith(CEILING_PREFIX)
        refs = float(np.mean([r["refs"] for r in scored]))
        mm = float(np.mean([r["m"] for r in scored]))
        x_refs = 0.0 if ceiling else refs
        x_m = 0.0 if ceiling else mm
        acc = float(np.mean([r["correct"] for r in scored]))
        lo, hi = interval([r["correct"] for r in scored], [r["seed"] for r in scored])
        summary.append({
            "arm": arm, "role": "ceiling (reference)" if ceiling else "builder",
            "hard_calls": len(rows), "slow_leak_calls_excluded": leak, "counted_calls": len(scored),
            "focus_not_at_site": sum(1 for r in scored if not r["focus_at_site"]),
            "mean_refs_per_call": refs, "mean_m_per_call": mm,
            "reader_accuracy": acc, "lo90": lo, "hi90": hi,
            "reader_kind_only_accuracy": float(np.mean([r["kind_correct"] for r in scored])),
            "predicted_exp_at_refs": float(predict_exp(fit, x_refs)),
            "predicted_exp_at_m": float(predict_exp(fit, x_m)),
            "predicted_interp_at_refs": predict_interp(fit, x_refs),
            "predicted_interp_at_m": predict_interp(fit, x_m),
            "accuracy_minus_exp_at_refs": acc - float(predict_exp(fit, x_refs)),
            "accuracy_minus_interp_at_refs": acc - predict_interp(fit, x_refs),
        })
        print(f"{arm}: {len(scored)} calls, refs {refs:.1f}, m {mm:.1f}, reader {acc:.3f} [{lo:.3f}, {hi:.3f}], "
              f"exp-predicted {predict_exp(fit, x_refs):.3f}")
    out = Path(a.out_dir)
    out.mkdir(parents=True, exist_ok=True)
    for name, rows in ((f"{a.prefix}direct.csv", summary), (f"{a.prefix}direct-calls.csv", all_calls)):
        keys = sorted({k for r in rows for k in r}, key=lambda k: list(rows[0]).index(k) if k in rows[0] else 99)
        with open(out / name, "w", newline="") as fh:
            w = csv.DictWriter(fh, fieldnames=keys)
            w.writeheader()
            w.writerows(rows)


if __name__ == "__main__":
    main()
