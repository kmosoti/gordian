"""Print and save the notice measures of every arm of a tuning run (tuning streams only).

Usage: m2_tune.py STAGE [--save]

Columns: hard non-leak noticed and anchor-correct shares, leak noticed and anchor-correct shares,
notices on background per stream, notices per incident, plain noticed share, hard quality under
the selection oracle, substrate seconds per stream. Pooled over the 100 tuning streams, as B1.
With --save, writes experiments/exploration/m2-tuning-<STAGE>.csv.
"""

import sys

import pandas as pd

import m2_common as C  # noqa: I001 (sets the path for the two below)
import b2_stats as B
from gordian_analysis.load import load_stream_run

COLS = [
    ("hard_noticed_share", "noticed"),
    ("hard_anchor_correct_share", "anchor_ok"),
    ("leak_noticed_share", "leak_noticed"),
    ("leak_anchor_correct_share", "leak_anchor_ok"),
    ("hard_anchor_site_correct_share", "anchor_site_ok"),
    ("notices_on_background_per_stream", "background"),
    ("strict_precision", "strict_prec"),
    ("notices_per_incident", "per_incident"),
    ("plain_noticed_share", "plain_noticed"),
    ("quality", "quality"),
    ("substrate_s_per_stream", "substrate_s"),
]


def table(stage):
    run = load_stream_run(C.RUNS / C.run_id(stage))
    seeds = [int(s) for s in next(iter(run.arms.values())).results["seed"]]
    assert seeds[0] >= C.TUNING_SEEDS[0] and seeds[-1] < C.TUNING_SEEDS[0] + C.TUNING_SEEDS[1], \
        "tuning streams only"
    m = B.Measures(run)
    pts = m.points()
    rows = []
    for arm in m.names:
        i = m.row[arm]
        row = {"arm": arm.removeprefix("sel_").removesuffix("_privileged")}
        row.update({short: float(pts[k][i]) for k, short in COLS})
        rows.append(row)
    return pd.DataFrame(rows)


def main():
    stage = sys.argv[1]
    df = table(stage)
    if "--save" in sys.argv:
        df.to_csv(C.OUT / f"m2-tuning-{stage}.csv", index=False)
    with pd.option_context("display.width", 200, "display.max_rows", 500):
        print(df.to_string(index=False, float_format=lambda x: f"{x:.3f}"))


if __name__ == "__main__":
    main()
