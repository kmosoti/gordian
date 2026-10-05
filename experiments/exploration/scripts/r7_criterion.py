"""R7's criterion, as the plan fixed it (docs/local-test-plan.md, 5R, R7, "Criterion, fixed by the
coordinator before any R7 code or run (2026-10-05)"), applied to the held-out runs.

Usage: r7_criterion.py        (reads the six held-out runs and r7-selected.json; writes
                               r7-criterion.csv and r7-outcome.json)

Definitions, from the plan, unchanged here:

* G(delta) is the context-only ceiling's hard-incident quality (slow leak excluded) minus the
  selected builder's, on the same 200 held-out streams at the primary setting (b = 5, rho = 0.7).
  The ceiling is `oracle_selection_context` (the selection oracle's anomalies and delay, with the
  decisive evidence delivered so far); the selected builder is the carried configuration of highest
  tuning quality across builders, run with the selection oracle at R6's delay (`r7_select.py`).
* Its interval is a paired 90% cluster bootstrap over streams (whole streams resampled, 10,000
  resamples, 5th and 95th percentile), with R6's stream seeds.

Outcomes (the grid is delta in {0.05, 0.1, 0.2, 0.4}; b = 2.5 and b = 8 at delta 0.2 are
sensitivity rows and are not in the outcome):

* Robust: at every delta in the grid the upper bound of G(delta) is below 0.10.
* Fragile: at some delta <= 0.2, G(delta) >= 0.10 with its lower bound above 0.05.
* Fragile only under a strong penalty: the fragile condition holds at delta = 0.4 and nowhere below.
* Unresolved: anything else.

The conditions are mutually exclusive as written (Robust implies G < 0.10 at every delta, so the
fragile condition fails everywhere; Fragile and "only under a strong penalty" differ in whether it
holds at some delta <= 0.2), so the order the code tests them in does not matter.

Readings the plan leaves open, fixed here before any R7 run:

* "Seeds as in R6": the stream seeds are R6's (tuning 10000-10099, held-out 20000-20199); the
  bootstrap seed of run k (its index in `r7_common.RUNS_SPEC`) is 9600 + k, 9600 being the base of
  R6's own bootstrap seeds; both are recorded in `r7-criterion.csv`.
* "Upper bound" and "lower bound" are the 95th and 5th percentiles of the resampled G (method
  `higher` and `lower`, as in `r6_stats.interval`).
* "G >= 0.10" is the point estimate on the 200 held-out streams.
* At delta = 0 (R6 itself, not re-run) the same procedure is applied to R6's own files as a check
  of this pipeline and a reference row; it is not in the outcome.

Exploration (nothing here tests a hypothesis).
"""

import json
import os
import pathlib

import pandas as pd

import r6_stats as S
import r7_common as C
import r7_select as SEL
from gordian_analysis.load import load_stream_run

R6_RUNS = pathlib.Path(os.environ.get("R6_RUNS", "/home/user/gordian/artifacts/runs/r6"))


def fragile(g, lo):
    return bool(g >= C.FRAGILE_G and lo > C.FRAGILE_LOWER)


def outcome(rows):
    """The criterion's outcome from the primary setting's rows, one per delta of the grid:
    dicts with `delta`, `G`, `lo`, `hi`."""
    by = {r["delta"]: r for r in rows}
    assert sorted(by) == sorted(C.DELTAS), sorted(by)
    frag = {d: fragile(by[d]["G"], by[d]["lo"]) for d in C.DELTAS}
    robust = all(by[d]["hi"] < C.ROBUST_UPPER for d in C.DELTAS)
    low = any(frag[d] for d in C.LOW_DELTAS)
    strong_only = frag[C.STRONG_DELTA] and not low
    if low:
        name = "Fragile"
    elif strong_only:
        name = "Fragile only under a strong penalty"
    elif robust:
        name = "Robust"
    else:
        name = "Unresolved"
    return {"outcome": name, "robust_condition": robust, "fragile_at": {C.dlabel(d): frag[d] for d in C.DELTAS},
            "upper_below_0.10_at": {C.dlabel(d): by[d]["hi"] < C.ROBUST_UPPER for d in C.DELTAS}}


def g_row(run_dir, ceiling, selected_arm, seed):
    run = load_stream_run(run_dir)
    streams = S.Streams(run)
    g, lo, hi = S.paired(streams, ceiling, selected_arm, seed, C.N_RESAMPLES)
    pts = {n: S.point(run.arms[n]) for n in (ceiling, selected_arm)}
    return {
        "ceiling": ceiling, "selected_arm": selected_arm, "streams": streams.n,
        "ceiling_quality": pts[ceiling]["quality"], "ceiling_refs_per_call": pts[ceiling]["refs_per_call"],
        "selected_quality": pts[selected_arm]["quality"], "selected_refs_per_call": pts[selected_arm]["refs_per_call"],
        "hard_incidents_excl_leak": pts[ceiling]["quality_incidents"],
        "G": g, "lo": lo, "hi": hi, "fragile_condition": fragile(g, lo),
        "upper_below_0.10": hi < C.ROBUST_UPPER, "resamples": C.N_RESAMPLES, "seed": seed,
    }


def delta0_reference():
    """R6's own files, the same procedure: the tuning-selected builder among R6's carried
    configurations at the primary setting, its held-out gap to the context-only ceiling."""
    b, rho = C.PRIMARY
    sid = C.setting_id(b, rho)
    tun = pd.read_csv(C.OUT / "r6-tuning-selection.csv")
    tun = tun[(tun["setting"] == sid) & (tun["kind"].isin(C.KINDS)) & tun["chosen"]].copy()
    best = sorted((r for _, r in tun.iterrows()), key=SEL.rank_key)[0]
    d = C.sel_delay_s(b, rho)
    row = g_row(R6_RUNS / f"r6-heldout-{sid}", C.ctxonly_name(d), C.sel_arm_name(best["builder"], d),
                C.BOOT_SEED0 + 100)
    row.update({"delta": 0.0, "setting": sid, "b": b, "rho": rho, "source": "R6's files (not re-run)",
                "run_id": f"r6-heldout-{sid}", "selected": best["builder"],
                "selected_tuning_quality": float(best["quality"])})
    return row


def selftest():
    """Each of the four outcomes is reachable, from synthetic rows (run: r7_criterion.py selftest)."""
    def rows(*specs):
        return [{"delta": d, "G": g, "lo": lo, "hi": hi} for d, (g, lo, hi) in zip(C.DELTAS, specs)]

    ok, tiny, big = (0.03, 0.0, 0.08), (0.02, 0.0, 0.05), (0.30, 0.20, 0.40)
    wide = (0.12, 0.02, 0.25)  # G >= 0.10 but the lower bound is not above 0.05
    cases = [
        (rows(ok, ok, tiny, ok), "Robust"),
        (rows(ok, ok, big, big), "Fragile"),
        (rows(big, ok, ok, ok), "Fragile"),
        (rows(ok, ok, ok, big), "Fragile only under a strong penalty"),
        (rows(ok, ok, ok, wide), "Unresolved"),
        (rows(ok, ok, wide, ok), "Unresolved"),
        (rows(ok, ok, ok, (0.09, 0.01, 0.10)), "Unresolved"),  # upper bound exactly 0.10 is not below it
    ]
    for given, want in cases:
        got = outcome(given)["outcome"]
        assert got == want, (given, got, want)
    print(f"selftest: {len(cases)} cases, all four outcomes reached")


def main():
    sel = C.load_json(C.OUT / "r7-selected.json")
    rows = []
    for k, (b, rho, delta) in enumerate(C.RUNS_SPEC):
        tid = C.run_id("tune", b, rho, delta)
        hid = C.run_id("heldout", b, rho, delta)
        s = sel[tid]
        d = s["selection_delay_s"]
        row = g_row(C.RUNS / hid, C.ctxonly_name(d), C.sel_arm_name(s["selected"], d), C.BOOT_SEED0 + k)
        primary = (b, rho) == C.PRIMARY
        row.update({"delta": delta, "setting": C.setting_id(b, rho), "b": b, "rho": rho,
                    "source": "R7" if primary else "R7 sensitivity (not in the outcome)", "run_id": hid,
                    "selected": s["selected"], "selected_tuning_quality": s["selected_tuning_quality"]})
        rows.append(row)
        print(f"delta {delta:g} {C.setting_id(b, rho)}: selected {s['selected']} "
              f"G {row['G']:.3f} [{row['lo']:.3f}, {row['hi']:.3f}] "
              f"{'FRAGILE-CONDITION' if row['fragile_condition'] else ''}")
    ref = delta0_reference()
    print(f"delta 0 (R6's files): selected {ref['selected']} G {ref['G']:.3f} [{ref['lo']:.3f}, {ref['hi']:.3f}]"
          f" ceiling {ref['ceiling_quality']:.3f} selected {ref['selected_quality']:.3f}")
    df = pd.DataFrame([ref] + rows)
    df.to_csv(C.OUT / "r7-criterion.csv", index=False, float_format="%.6g")
    prim = [r for r in rows if (r["b"], r["rho"]) == C.PRIMARY]
    res = outcome(prim)
    (C.OUT / "r7-outcome.json").write_text(json.dumps(res, indent=2) + "\n")
    print("OUTCOME:", res["outcome"])


if __name__ == "__main__":
    import sys

    selftest() if sys.argv[1:] == ["selftest"] else main()
