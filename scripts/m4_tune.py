"""Print and save the notice measures of every arm of an M4 tuning run (tuning streams only), with
each arm's paired lower bounds against the comparator, so that the tuning rule can be applied
after the run directory is gone.

Usage: m4_tune.py STAGE [--save]

Columns: the measures of the criterion (anchor-correct, leak noticed, strict precision, background
notices per stream), each arm minus the comparator (`ramp_split_over_re2`, rerun in the stage) on
anchor-correct and leak noticed with the paired 90% interval (B2's bootstrap, seed BOOT_SEED, 10,000
resamples of whole streams: the same counts as `b2_stats.Measures.paired_difference`, for every arm
at once), and beside them leak anchor-correct, notices per incident, decoy notices, calls and cost
per stream. Pooled over the 100 tuning streams, as B1. With --save, writes
experiments/exploration/m4-tuning-<STAGE>.csv.
"""

import sys

import numpy as np
import pandas as pd

import m4_common as C  # noqa: I001 (sets the path for the ones below)
import b2_stats as B
import r6_stats as S
from gordian_analysis.load import load_stream_run

COLS = [
    ("hard_noticed_share", "noticed"),
    ("hard_anchor_correct_share", "anchor_ok"),
    ("leak_noticed_share", "leak_noticed"),
    ("leak_anchor_correct_share", "leak_anchor_ok"),
    ("notices_on_background_per_stream", "background"),
    ("strict_precision", "strict_prec"),
    ("notices_per_incident", "per_incident"),
    ("notices_on_decoy_per_stream", "decoy"),
    ("plain_noticed_share", "plain_noticed"),
    ("quality", "quality"),
    ("calls_per_stream", "calls"),
    ("cost_s_per_stream", "cost_s"),
    ("substrate_s_per_stream", "substrate_s"),
]
PAIRED = {"hard_anchor_correct_share": "ac", "leak_noticed_share": "ln"}


def paired_bounds(m, ref, measure, seed=C.BOOT_SEED, resamples=C.N_RESAMPLES, chunk=500):
    """(lower, higher) arrays over arms of `measure` of each arm minus that of `ref`, over the same
    resamples as `Measures.paired_difference(measure, arm, ref, seed)`."""
    rng = np.random.default_rng(seed)
    iref = m.row[ref]
    parts, done = [], 0
    while done < resamples:
        k = min(chunk, resamples - done)
        r = m._ratio(measure, m.counts(rng, k))
        parts.append(r - r[:, [iref]])
        done += k
    x = np.concatenate(parts)
    lo, hi = np.full(x.shape[1], np.nan), np.full(x.shape[1], np.nan)
    for a in range(x.shape[1]):
        col = x[:, a]
        col = col[~np.isnan(col)]
        if len(col):
            lo[a], hi[a] = S.interval(col)
    return lo, hi


def table(stage):
    run = load_stream_run(C.RUNS / C.run_id(stage))
    seeds = [int(s) for s in next(iter(run.arms.values())).results["seed"]]
    assert seeds == list(range(C.TUNING_SEEDS[0], C.TUNING_SEEDS[0] + C.TUNING_SEEDS[1])), \
        "tuning streams only"
    m = B.Measures(run)
    pts = m.points()
    ref = C.arm_name(C.COMPARATOR)
    bounds = {k: paired_bounds(m, ref, k) for k in PAIRED}
    # The vectorized bounds are B2's paired_difference, arm by arm: check one arm.
    probe = next(a for a in m.names if a != ref)
    for k in PAIRED:
        _, lo, hi = m.paired_difference(k, probe, ref, C.BOOT_SEED, C.N_RESAMPLES)
        assert (lo, hi) == (bounds[k][0][m.row[probe]], bounds[k][1][m.row[probe]]), k
    rows = []
    for arm in m.names:
        i = m.row[arm]
        row = {"arm": arm.removeprefix("sel_").removesuffix("_privileged")}
        row.update({short: float(pts[k][i]) for k, short in COLS})
        for k, short in PAIRED.items():
            row[f"{short}_diff"] = float(pts[k][i] - pts[k][m.row[ref]])
            row[f"{short}_lo"] = float(bounds[k][0][i])
            row[f"{short}_hi"] = float(bounds[k][1][i])
        rows.append(row)
    return pd.DataFrame(rows)


def main():
    stage = sys.argv[1]
    df = table(stage)
    if "--save" in sys.argv:
        df.to_csv(C.OUT / f"m4-tuning-{stage}.csv", index=False)
    with pd.option_context("display.width", 250, "display.max_rows", 1000):
        cols = ["arm", "anchor_ok", "leak_noticed", "strict_prec", "background", "ac_lo", "ln_lo",
                "leak_anchor_ok", "per_incident", "cost_s"]
        print(df[cols].to_string(index=False, float_format=lambda x: f"{x:.3f}"))


if __name__ == "__main__":
    main()
