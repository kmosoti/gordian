"""R6 analysis of the held-out runs: the criterion as the plan fixed it, the per-builder table, the
per-family tables, the timing-against-context split, and the cross-check with R5.

Usage: r6_report.py        (reads the six held-out runs; writes r6-*.csv under experiments/exploration/)

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation).

The criterion (docs/local-test-plan.md, 5R, R6, "Criterion, fixed by the coordinator before any R6
run (2026-10-06)") is applied exactly as written; nothing in its numbers is changed here:

  EXP-102 has context-construction headroom if, at the primary setting (b = 5, rho = 0.7), with
  selection held at the selection oracle, either
  1. the privileged decisive-evidence context exceeds the best public builder by at least 0.10 in
     hard-incident quality (slow leak excluded) at equal or fewer references per call, with the 90%
     cluster-bootstrap lower bound above 0.05; or
  2. to come within 0.05 of the ceiling's quality, the best public builder needs at least 1.5 times
     the references per call, with the 90% interval of that ratio excluding 1.25.

Readings the plan leaves open, fixed in `r6_stats.criterion` before any held-out number was seen and
stated in the report:

* the ceiling is R4's `oracle_escalation_privileged` ("R4's oracle remains the decisive-evidence
  ceiling"); it also has the privilege of timing (it asks when all the decisive evidence has
  arrived), which the criterion does not separate. A labelled supplementary reading takes
  `oracle_selection_context` (selection oracle's anomalies and delay, decisive evidence delivered
  so far as the context) as the ceiling.
* the public builders are the selection-oracle arms of every frontier configuration of `window`,
  `cooccur`, `neighbourhood` and the `rung` (selection held at the selection oracle). The
  `always_escalate` pairings are reported beside them and are not in the criterion.
* clause 1: the best held-out quality among those arms with references per call no greater than the
  ceiling's, recomputed inside every resample; the empty context (quality 0) is the comparator when
  none is affordable. Clause 2: "within 0.05" is quality at least the ceiling's minus 0.05; "the
  best builder" is the one with the fewest references per call of those; the ratio is its
  references per call over the ceiling's, infinite when no builder reaches that quality, and
  "excluding 1.25" is a 5th-percentile ratio above 1.25 with the point estimate at least 1.5.
* b = 2.5 and b = 8 at rho = 0.7 (only b varies) and, additionally, at rho = 0.
"""

import hashlib
import json

import pandas as pd

import r6_common as C
import r6_stats as S
from gordian_analysis.drift import drift_report, load_drift
from gordian_analysis.load import load_stream_run

N_RESAMPLES = 10_000
SEED0 = 9_600


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def classify(name):
    """(role label, builder key or None, builder kind) of an arm name."""
    if name == C.ORACLE:
        return "R4 oracle (ceiling)", None, "ceiling"
    if name == C.ABLATION:
        return "ablation (hidden rules)", None, "ablation"
    if name == C.NEVER:
        return "never_escalate", None, "never"
    if name.startswith("oracle_selection_context"):
        return "context-only ceiling (supplementary)", None, "ceiling_context_only"
    key = C.kind_of_arm(name)
    if key is None:
        raise KeyError(name)
    kind = C.BUILDERS[key][0]
    pairing = "selection oracle" if name.startswith("sel_") else "always_escalate"
    return pairing, key, kind


def main():
    selected = C.load_json(C.OUT / "r6-selected.json")
    crit_rows, point_rows, supp_rows, split_rows, meta_rows, r5_rows = [], [], [], [], [], []
    for i, (b, rho) in enumerate(C.SETTINGS):
        sid = C.setting_id(b, rho)
        rid = C.run_id("heldout", b, rho)
        run = load_stream_run(C.RUNS / rid)
        streams = S.Streams(run)
        pts = {n: S.point(a) for n, a in run.arms.items()}
        names = list(run.arms)
        sel_pub = [n for n in names if n.startswith("sel_")]
        alw_pub = [n for n in names if n.startswith("alw_")]
        ctxonly = [n for n in names if n.startswith("oracle_selection_context")][0]

        c = S.criterion(streams, C.ORACLE, sel_pub, SEED0 + i, N_RESAMPLES)
        c.update({"setting": sid, "b": b, "rho": rho, "streams": streams.n,
                  "hard_incidents_excl_leak": pts[C.ORACLE]["quality_incidents"],
                  "n_public_builders": len(sel_pub), "ceiling": C.ORACLE})
        # supplementary: the context-only ceiling
        cs = S.criterion(streams, ctxonly, sel_pub, SEED0 + 50 + i, N_RESAMPLES)
        cs.update({"setting": sid, "ceiling": ctxonly})
        crit_rows.append(c)
        supp_rows.append(cs)
        # timing against context, paired over the same streams
        rung_sel = "sel_rung_privileged"
        best_any = max(sel_pub, key=lambda n: (pts[n]["quality"], -pts[n]["refs_per_call"]))
        for label, a, bb in (
            ("R4 oracle minus selection oracle with rung (timing + context + selection of evidence)", C.ORACLE, rung_sel),
            ("R4 oracle minus context-only ceiling (timing)", C.ORACLE, ctxonly),
            ("context-only ceiling minus selection oracle with rung (context)", ctxonly, rung_sel),
            ("context-only ceiling minus best public builder, any references (context left after the best public builder)", ctxonly, best_any),
            ("best public builder, any references, minus selection oracle with rung (what a public builder buys)", best_any, rung_sel),
        ):
            dq, lo, hi = S.paired(streams, a, bb, SEED0 + 200 + i + len(split_rows), N_RESAMPLES)
            split_rows.append({"setting": sid, "difference": label, "a": a, "b": bb,
                               "quality_diff": dq, "lo": lo, "hi": hi})
        # every arm's point
        for n, p in pts.items():
            role, key, kind = classify(n)
            row = dict(p)
            row.update({"setting": sid, "b": b, "rho": rho, "pairing": role, "builder": key, "kind": kind})
            if key:
                for k, v in C.BUILDERS[key][1].items():
                    row[f"p_{k}"] = v
            point_rows.append(row)
        # R5 cross-check inside this run: the selection oracle with rung against R5's arm
        r5arm = C.R5_RUNS / f"r5-heldout-{sid}" / f"oracle_selection_d{selected[sid]['selection_delay_s']:02d}_privileged"
        if (r5arm / "results.csv").exists():
            mine = pd.read_csv(C.RUNS / rid / rung_sel / "results.csv").drop(columns=["run_id"])
            theirs = pd.read_csv(r5arm / "results.csv").drop(columns=["run_id"])
            mi = pd.read_csv(C.RUNS / rid / rung_sel / "incidents.csv").drop(columns=["run_id"])
            ti = pd.read_csv(r5arm / "incidents.csv").drop(columns=["run_id"])
            r5_rows.append({"setting": sid, "r6_arm": rung_sel, "r5_arm": r5arm.name,
                            "results_equal_but_run_id": bool(mine.equals(theirs)),
                            "incidents_equal_but_run_id": bool(mi.equals(ti)), "streams": len(mine)})
        usage = run.usage or {}
        drift = drift_report(load_drift(C.RUNS / rid / "drift.csv"))
        meta_rows.append({
            "setting": sid, "run_id": rid, "manifest_sha256": sha(C.RUNS / rid / "manifest.json"),
            "arms": len(run.arms), "streams": streams.n, "cpu_model": run.manifest.get("cpu_model"),
            "cpu_mhz": run.manifest.get("cpu_mhz"), "source_revision": run.manifest["source_revision"],
            "wall_ns": usage.get("wall_ns"), "cpu_ns": usage.get("cpu_ns"),
            "peak_memory_bytes": usage.get("peak_memory_bytes"), "oom_kills": usage.get("oom_kills"),
            "exit_code": usage.get("exit_code"), "internal_external_ratio": usage.get("internal_external_ratio"),
            "drift_blocks": drift.n_blocks, "drift_cv_ns": drift.ns.cv,
            "drift_last_over_first_ns": drift.ns.ratio_last_first, "drift_cv_min_ns": drift.min_ns.cv,
            "drift_last_over_first_min_ns": drift.min_ns.ratio_last_first,
        })
        print(sid, "criterion 1 gap", round(c["c1_gap"], 3), [round(c["c1_gap_lo"], 3), round(c["c1_gap_hi"], 3)],
              "holds" if c["c1_holds"] else "no", "| 2 ratio", round(c["c2_ratio"], 2),
              [round(c["c2_ratio_lo"], 2), round(c["c2_ratio_hi"], 2)], "holds" if c["c2_holds"] else "no")
    for name, rows in (("criterion", crit_rows), ("criterion-supplementary", supp_rows),
                       ("heldout-points", point_rows), ("timing-context-split", split_rows),
                       ("provenance", meta_rows), ("r5-crosscheck-in-run", r5_rows)):
        df = pd.DataFrame(rows)
        for col in df.columns:
            if df[col].map(lambda v: isinstance(v, tuple)).any():
                df[col] = df[col].map(lambda v: json.dumps(list(v)) if isinstance(v, tuple) else v)
        df.to_csv(C.OUT / f"r6-{name}.csv", index=False, float_format="%.6g")
        print(C.OUT / f"r6-{name}.csv")


if __name__ == "__main__":
    main()
