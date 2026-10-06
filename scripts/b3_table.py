"""The B3 acceptance table: B2's rows (the rung at z = 3 and z = 2, the later re-anchor) rerun beside the
ramp noticer, the splitting noticer and their compositions, on the 200 held-out streams at b = 5,
rho = 0.7, with the selection oracle and the rung's own context, and 90% cluster-bootstrap intervals
over streams; the quality of every row both ways; the sensitivity table of every configuration the run
played; and the paired differences against the re-anchor.

Usage: b3_table.py   (after the held-out run; writes experiments/exploration/b3-noticers-table.csv,
                      b3-noticers-sensitivity.csv, b3-noticers-latency.csv, b3-paired.csv and
                      b3-noticers-table.md)

The rows are `b3_manifests.table_rows`: the three comparators of B2 (as rerun in this run, so that the table
is one run), the new noticers over the rung's noticer at its default threshold and over the re-anchor,
and the two rows over the re-anchor at the rung's own threshold. Every other configuration of the run is
the sensitivity table; none of it was used to choose anything. The table makes no claim about which
noticer is better; the paired differences are numbers.

Quality is read two ways for every row (as B2): `quality`, the selection oracle at R5's fixed 16 s delay
with the rung's 6 s retirement; and `quality_held`, the selection oracle at the delay chosen for that
row on the tuning streams, with `hold_until_asked`. Cost is the harness's modelled cost per stream: it
counts the components and the shared rule's work and the reasoner's tokens, and not the noticers' own
operations (the rung's noticing is not billed either, so no row is charged for noticing).
"""

import pandas as pd

import b2_stats as B
import b2_table as T2
import b3_common as C
import b3_manifests as M
from gordian_analysis.load import load_stream_run
from gordian_analysis.stream import notice_latency

# The columns of the acceptance table.
ACCEPTANCE = [
    ("hard_noticed_share", "hard non-leak noticed", "{:.3f}"),
    ("hard_anchor_correct_share", "anchor-correct", "{:.3f}"),
    ("leak_noticed_share", "leak noticed", "{:.3f}"),
    ("leak_anchor_correct_share", "leak anchor-correct", "{:.3f}"),
    ("notices_on_background_per_stream", "notices on background / stream", "{:.2f}"),
    ("notice_precision", "notice precision", "{:.3f}"),
    ("strict_precision", "strict precision", "{:.3f}"),
    ("notices_per_incident", "notices per incident", "{:.2f}"),
    ("quality", "quality, fixed 16 s, retirement", "{:.3f}"),
    ("quality_held", "quality, tuned delay, hold", "{:.3f}"),
    ("cost_s_per_stream", "cost s / stream", "{:.3f}"),
]
BESIDE = [
    ("hard_site_correct_share", "site-correct", "{:.3f}"),
    ("hard_anchor_site_correct_share", "anchor-and-site-correct", "{:.3f}"),
    ("leak_site_correct_share", "leak site-correct", "{:.3f}"),
    ("leak_anchor_site_correct_share", "leak anchor-and-site-correct", "{:.3f}"),
    ("plain_noticed_share", "plain noticed", "{:.3f}"),
    ("notices_on_plain_per_stream", "notices on plain / stream", "{:.2f}"),
    ("notices_on_hard_per_stream", "notices on hard / stream", "{:.2f}"),
    ("notices_on_decoy_per_stream", "notices on decoys / stream", "{:.2f}"),
    ("notices_per_stream", "notices / stream", "{:.2f}"),
    ("leak_quality", "leak quality, fixed", "{:.3f}"),
    ("leak_quality_held", "leak quality, held", "{:.3f}"),
    ("calls_per_stream", "calls / stream", "{:.2f}"),
]
PAIRED_MEASURES = [
    "hard_noticed_share", "hard_anchor_correct_share", "leak_noticed_share", "leak_anchor_correct_share",
    "notices_on_background_per_stream", "notice_precision", "strict_precision", "notices_per_incident",
    "quality", "quality_held", "leak_quality", "leak_quality_held", "cost_s_per_stream",
    "hard_site_correct_share", "hard_anchor_site_correct_share", "notices_per_stream",
]


def ramp_text(p):
    return (f"gap {p['gap_ms'] / 1000:g} s, step <= {p['max_step']}, drop <= {p['max_drop']}, "
            f">= {p['min_readings']} readings, rise >= {p['min_rise']}")


def split_text(p):
    return f"gap {p['gap_ms'] / 1000:g} s, burst >= {p['min_burst']}"


def label(stem, sel):
    ramp = sel["ramp"]["chosen"]["params"]
    s1, s2 = sel["split_r3"]["chosen"]["params"], sel["split_re2"]["chosen"]["params"]
    base = {"r3": "rung z = 3", "re2": "re-anchor (z = 2)", "re3": "re-anchor at z = 3"}
    fixed = {
        "rung_z3": "RungNoticer, z = 3 (default)",
        "rung_z2": "RungNoticer, z = 2",
        "reanchor": "ReanchorNoticer (B2's row, z = 2)",
        "reanchor_z3": "ReanchorNoticer at z = 3",
    }
    if stem in fixed:
        return fixed[stem]
    parts = stem.split("_over_")
    pieces, b = parts[0], parts[1]
    text = []
    if "ramp" in pieces:
        text.append(f"RampNoticer ({ramp_text(ramp)})")
    if "split" in pieces:
        text.append(f"SplitNoticer ({split_text(s1 if b == 'r3' else s2)})")
    return " + ".join(text) + f" over {base[b]}"


def main():
    sel = C.load_selected()
    run = load_stream_run(C.RUNS / C.run_id("heldout"))
    seeds = [int(s) for s in next(iter(run.arms.values())).results["seed"]]
    assert seeds == list(range(C.HELDOUT_SEEDS[0], C.HELDOUT_SEEDS[0] + C.HELDOUT_SEEDS[1])), "held-out seeds"
    m = B.Measures(run)
    pts = m.points()
    ci = m.boot(C.BOOT_SEED, C.N_RESAMPLES)

    def row_of(arm, stem, text):
        i = m.row[arm]
        row = {"noticer": text, "config": stem, "arm": arm}
        for key in B.MEASURES:
            row[key] = float(pts[key][i])
            row[f"{key}_lo"] = float(ci[key][0][i])
            row[f"{key}_hi"] = float(ci[key][1][i])
        return row

    stems = [s for s, _ in M.table_rows(sel)]
    rows = []
    for stem in stems:
        row = row_of(C.arm_name(stem), stem, label(stem, sel))
        hold = m.row[C.hold_arm_name(stem)]
        for key, out in (("quality", "quality_held"), ("leak_quality", "leak_quality_held")):
            row[out] = float(pts[key][hold])
            row[f"{out}_lo"] = float(ci[key][0][hold])
            row[f"{out}_hi"] = float(ci[key][1][hold])
        row["held_delay_s"] = sel["delay"][stem]["delay_s"]
        for key in ("hard_anchor_correct_share", "leak_noticed_share", "notices_on_background_per_stream"):
            row[f"{key}_in_hold_run"] = float(pts[key][hold])
        row["within_budget"] = bool(row["notices_on_background_per_stream"] <= C.BACKGROUND_BUDGET)
        rows.append(row)
    table = pd.DataFrame(rows)
    table.to_csv(C.OUT / "b3-noticers-table.csv", index=False)

    # Sensitivity: every configuration the run played, at the fixed delay (reading 1).
    sens = []
    for arm in m.names:
        if not arm.startswith("sel_") or not arm.endswith("_privileged"):
            continue
        stem = arm[len("sel_"):-len("_privileged")]
        text = label(stem, sel) if stem in stems else stem
        sens.append(row_of(arm, stem, text))
    sens = pd.DataFrame(sens)
    sens["table_row"] = sens["config"].isin(stems)
    sens.to_csv(C.OUT / "b3-noticers-sensitivity.csv", index=False)

    lat = []
    for stem in stems:
        t = notice_latency(run.arms[C.arm_name(stem)])
        t.insert(0, "config", stem)
        lat.append(t)
    pd.concat(lat).to_csv(C.OUT / "b3-noticers-latency.csv", index=False)

    # Paired differences against the re-anchor row, the same resamples.
    paired = []
    for stem in stems:
        for measure in PAIRED_MEASURES:
            if measure in ("quality_held", "leak_quality_held"):
                a, b = C.hold_arm_name(stem), C.hold_arm_name(C.COMPARATOR)
                key = measure.replace("_held", "")
            else:
                a, b, key = C.arm_name(stem), C.arm_name(C.COMPARATOR), measure
            point, lo, hi = m.paired_difference(key, a, b, C.BOOT_SEED, C.N_RESAMPLES)
            paired.append({"config": stem, "against": C.COMPARATOR, "measure": measure,
                           "difference": point, "lower": lo, "higher": hi})
    pd.DataFrame(paired).to_csv(C.OUT / "b3-paired.csv", index=False)

    text = ["### Acceptance table", "", T2.md(table, ACCEPTANCE), "", "### Beside it", "",
            T2.md(table, BESIDE), "",
            "### Sensitivity: every configuration of the run, held-out (fixed 16 s delay, retirement)", "",
            T2.md(sens, [c for c in ACCEPTANCE if c[0] not in ("quality_held",)]), ""]
    (C.OUT / "b3-noticers-table.md").write_text("\n".join(text))
    print("\n".join(text))


if __name__ == "__main__":
    main()
