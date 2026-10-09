"""The B5 delay sweep (item 2): the selection oracle at 8, 12, 16 and 20 s after notice over the comparator
(ramp + split over the re-anchor), the re-anchor, M2's medium at 100 ms and M3's at 500 ms, and the
dataflow twins (billed) of the first two, on the 200 held-out streams, with 90% cluster-bootstrap intervals
and paired differences: each delay against 16 s within a row, and each row against the comparator at the
same delay.

Usage: b5_sweep.py   (after the held-out sweep run; writes experiments/exploration/b5-sweep.csv,
                      b5-sweep-paired.csv and b5-sweep.md)

The oracle asks only about hard incidents' notices (`hold_until_asked` off, R6 of `b5_common.py`), so its
plain accuracy is `never_escalate`'s and its quality is the hard quality of the notices the rung still
holds at the delay. `notices_hard_retired_before_share` is the share of notices anchored on hard incidents
that the rung had retired before the delay (never asked about): the part of the 16 s convention that is a
selector. No claim about which row is better; the differences are numbers.
"""

import pathlib
import sys

# B5's scripts live here (the unit's territory, E1's convention); the earlier units' modules they build
# on (B2 to B4, C1, M2) live in scripts/, and the analysis package is this checkout's, not an installed one.
_ROOT = pathlib.Path(__file__).resolve().parents[3]
sys.path.insert(0, str(_ROOT / "scripts"))
sys.path.insert(0, str(_ROOT / "analysis"))

import pandas as pd

import b2_table as T2
import b5_common as C
import b5_stats as S
from gordian_analysis.load import load_stream_run

MEAS = [
    ("quality", "hard quality", "{:.3f}"),
    ("leak_quality", "leak quality", "{:.3f}"),
    ("critical_misses_per_stream", "critical misses / stream", "{:.2f}"),
    ("verified_hard_per_stream", "verified hard / stream", "{:.3f}"),
    ("calls_per_stream", "calls / stream", "{:.2f}"),
    ("cost_s_per_stream", "cost s / stream", "{:.3f}"),
    ("notices_hard_retired_before_share", "hard notices retired before the delay", "{:.3f}"),
    ("notices_leak_retired_before_share", "leak notices retired before the delay", "{:.3f}"),
    ("esc_hard_per_stream", "calls on hard / stream", "{:.2f}"),
    ("esc_leak_per_stream", "calls on leaks / stream", "{:.2f}"),
]


def label(stem):
    import b3_table as T3

    if stem.startswith("df_"):
        return ("Dataflow (C1), comparator's rules, billed" if stem == "df_" + C.COMPARATOR
                else "Dataflow (C1), re-anchor alone, billed")
    if stem[:3] in ("m2_", "m3_"):
        return f"Medium {stem.split('_t')[1]} ms ({stem[:2].upper()}'s frozen graph), billed"
    return T3.label(stem, C.C3.load_selected()) + (" (comparator)" if stem == C.COMPARATOR else "") + ", unbilled"


def main():
    run = load_stream_run(C.RUNS / C.run_id("sweep"))
    seeds = [int(s) for s in next(iter(run.arms.values())).results["seed"]]
    assert seeds == list(range(C.HELDOUT_SEEDS[0], C.HELDOUT_SEEDS[0] + C.HELDOUT_SEEDS[1])), "held-out seeds"
    m = S.Measures(run)
    pts = m.points()
    ci = m.boot(C.BOOT_SEED, C.N_RESAMPLES)
    stems = [s for s, _, _ in C.sweep_rows()]
    billed = {s: b for s, _, b in C.sweep_rows()}
    rows = []
    for stem in stems:
        for d in C.SWEEP_DELAYS_S:
            arm = C.oracle_arm(stem, d)
            i = m.row[arm]
            row = {"row": stem, "billed": billed[stem], "delay_s": d, "arm": arm, "noticer": label(stem)}
            for key in S.MEASURES:
                row[key] = float(pts[key][i])
                row[f"{key}_lo"] = float(ci[key][0][i])
                row[f"{key}_hi"] = float(ci[key][1][i])
            rows.append(row)
    table = pd.DataFrame(rows)
    table.to_csv(C.OUT / "b5-sweep.csv", index=False)

    measures = [k for k, _, _ in MEAS]
    pairs, meta = [], []
    for stem in stems:
        for d in C.SWEEP_DELAYS_S:
            for measure in measures:
                if d != C.DELAY_S:
                    pairs.append((measure, C.oracle_arm(stem, d), C.oracle_arm(stem, C.DELAY_S)))
                    meta.append((stem, d, "delay_minus_16s", measure))
                if stem != C.COMPARATOR:
                    pairs.append((measure, C.oracle_arm(stem, d), C.oracle_arm(C.COMPARATOR, d)))
                    meta.append((stem, d, "row_minus_comparator", measure))
    res = m.paired(measures, pairs, C.BOOT_SEED, C.N_RESAMPLES)
    pd.DataFrame([
        {"row": stem, "delay_s": d, "contrast": name, "measure": measure,
         "difference": res[p][0], "lower": res[p][1], "higher": res[p][2]}
        for (stem, d, name, measure), p in zip(meta, pairs)
    ]).to_csv(C.OUT / "b5-sweep-paired.csv", index=False)

    text = []
    for stem in stems:
        sub = table[table["row"] == stem].copy()
        sub["noticer"] = sub["delay_s"].map(lambda d: f"{d} s")
        text += [f"### {label(stem)}", "", T2.md(sub, MEAS), ""]
    (C.OUT / "b5-sweep.md").write_text("\n".join(text))
    print("\n".join(text))


if __name__ == "__main__":
    main()
