"""R10's criterion, as the plan fixed it (docs/local-test-plan.md, 5R, R10, "Criterion, fixed by the
coordinator before any R10 code or run (2026-10-06)"), applied to the held-out runs.

Usage: r10_criterion.py     (reads the three held-out runs; writes r10-criterion.csv and
                             r10-points.csv under experiments/exploration/)

Two separate results, each named, neither an "either":

1. **Salience headroom on hard incidents, slow leak excluded:** at the primary setting
   (b = 5, rho = 0.7), with the rung's context, `oracle_notice` minus `oracle_selection` in
   hard-incident quality is at least 0.03, with the 90% paired cluster-bootstrap lower bound above
   0.01.
2. **Salience headroom on the slow leak:** the same difference on slow-leak incidents alone is at
   least 0.20, with the lower bound above 0.10.

Sensitivity: b = 2.5 and b = 8 at rho = 0.7. Secondary pairing: `window` 40 s, N 256. Delay as R5
tuned it (`r10_common.sel_delay_s`). All of it computed here and reported; nothing in the margins,
pairings, delay, settings or bootstrap is changed.

Readings the plan leaves open, fixed in `r10_common.py` / `r10_stats.py` before any held-out run:

* "paired 90% cluster bootstrap over streams": whole streams resampled (10,000 resamples, numpy
  seed 9950), the same stream counts applied to both arms and to both results; the interval is the
  5th (`lower`) and 95th (`higher`) percentile of the resampled difference of pooled ratios.
* "at least 0.03" and "at least 0.20" are tested on the point estimate on the 200 held-out streams;
  "lower bound above 0.01" and "above 0.10" are strict inequalities on the 5th percentile.
* "the same difference on slow-leak incidents alone" is the pooled fraction of slow-leak incidents
  declared correctly by their deadline, notice arm minus selection arm, same streams and counts.
* `oracle_notice` is the arm `notice_rung_privileged` (the rung's context) and the comparison arm
  `oracle_selection` is `sel_rung_privileged`, both at the delay R5 tuned; the secondary pairing
  is the same pair with the `window` 40 s / N 256 builder.

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation).
"""

import pandas as pd

import r10_common as C
import r10_stats as S
from gordian_analysis.load import load_stream_run


def main():
    crit, points = [], []
    for b, rho in C.SETTINGS:
        sid = C.setting_id(b, rho)
        rid = C.run_id("heldout", b, rho)
        run = load_stream_run(C.RUNS / rid)
        streams = S.Streams(run)
        delay = C.sel_delay_s(b, rho)
        for name in run.arms:
            p = S.point(run.arms[name])
            p.update({"setting": sid, "b": b, "rho": rho, "run_id": rid, "delay_s": delay})
            points.append(p)
        for label, sel, notice in C.PAIRINGS:
            for result, kind, margin, lower in (
                (1, "quality", C.RESULT1_MARGIN, C.RESULT1_LOWER),
                (2, "leak", C.RESULT2_MARGIN, C.RESULT2_LOWER),
            ):
                diff, lo, hi, nan_share = streams.paired(kind, notice, sel, C.BOOT_SEED, C.N_RESAMPLES)
                pa, pb = points_of(points, sid, notice), points_of(points, sid, sel)
                num, den = ("quality_correct", "quality_incidents") if kind == "quality" else ("leak_correct", "leak_incidents")
                crit.append({
                    "setting": sid, "b": b, "rho": rho, "primary": (b, rho) == C.PRIMARY,
                    "pairing": label, "result": result,
                    "what": "hard incidents, slow leak excluded" if kind == "quality" else "slow leak",
                    "treatment": notice, "comparison": sel,
                    "treatment_correct": pa[num], "comparison_correct": pb[num], "incidents": pa[den],
                    "treatment_rate": pa[num] / pa[den], "comparison_rate": pb[num] / pb[den],
                    "difference": diff, "lo": lo, "hi": hi,
                    "margin": margin, "lower_threshold": lower,
                    "point_at_least_margin": diff >= margin, "lower_above_threshold": lo > lower,
                    "holds": bool(diff >= margin and lo > lower),
                    "resamples": C.N_RESAMPLES, "seed": C.BOOT_SEED, "nan_share": nan_share,
                })
    cdf = pd.DataFrame(crit)
    cdf.to_csv(C.OUT / "r10-criterion.csv", index=False, float_format="%.6g")
    pdf = pd.DataFrame(points)
    cols = ["setting", "arm", "role", "streams", "quality", "quality_correct", "quality_incidents",
            "leak_quality", "leak_correct", "leak_incidents", "plain_acc", "critical_misses",
            "critical_misses_hard", "critical_misses_plain", "critical_incidents", "calls_per_stream",
            "refs_per_call", "tokens_per_stream", "cost_s", "reasoner_cost_s", "false_alarms_per_stream",
            "wrong_per_stream", "refused", "hard_incidents_escalated_per_stream", "decoys_alarmed_per_stream",
            "compound_rate", "cascade_rate", "split_brain_rate", "slow_leak_rate", "b", "rho", "run_id", "delay_s"]
    pdf[cols].to_csv(C.OUT / "r10-points.csv", index=False, float_format="%.6g")
    prim = cdf[(cdf.primary) & (cdf.pairing == C.PAIRINGS[0][0])]
    for _, r in prim.iterrows():
        print(f"RESULT {r.result} ({r.what}): {r.difference:+.4f} [{r.lo:+.4f}, {r.hi:+.4f}] "
              f"margin {r.margin} lower>{r.lower_threshold}: {'HOLDS' if r.holds else 'does not hold'}")
    print(cdf[["setting", "pairing", "result", "difference", "lo", "hi", "holds"]].to_string(index=False))


def points_of(points, sid, arm):
    for p in points:
        if p["setting"] == sid and p["arm"] == arm:
            return p
    raise KeyError((sid, arm))


if __name__ == "__main__":
    main()
