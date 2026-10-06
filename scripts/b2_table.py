"""The B2 acceptance table: B1's four noticers and `reanchor` on the 200 held-out streams at
b = 5, rho = 0.7, with the selection oracle and the rung's context, and 90% cluster-bootstrap
intervals over streams; the quality of every row both ways; the sensitivity table of every
configuration the run played; and the paired differences against the M2 comparator.

Usage: b2_table.py   (after the held-out run; writes experiments/exploration/b2-noticers-table.csv,
                      b2-noticers-sensitivity.csv, b2-noticers-latency.csv, b2-paired.csv and
                      b2-noticers-table.md)

The five rows are B1's rung at z = 3 and z = 2, ChangeTriggered and EarliestAnchor at the parameters B1
chose, and `reanchor` at the parameters `b2_select.py` chose on the tuning streams. Every other
configuration of the run is the sensitivity table; none of it was used to choose anything. The table
makes no claim about which noticer is better; the paired differences are numbers, and the M2
criterion's reading of them is the chief's.

Quality is read two ways for every row (the queue's B2): `quality`, the selection oracle at R5's fixed
16 s delay with the rung's 6 s retirement, as B1; and `quality_held`, the selection oracle at the delay
chosen for that noticer on the tuning streams, with `hold_until_asked`.
"""

import pandas as pd

import b2_common as C
import b2_stats as B
from gordian_analysis.load import load_stream_run
from gordian_analysis.stream import notice_latency

# The columns of the acceptance table.
ACCEPTANCE = [
    ("hard_noticed_share", "hard non-leak noticed", "{:.3f}"),
    ("hard_anchor_correct_share", "anchor-correct", "{:.3f}"),
    ("hard_site_correct_share", "site-correct", "{:.3f}"),
    ("hard_anchor_site_correct_share", "anchor-and-site-correct", "{:.3f}"),
    ("leak_noticed_share", "leak noticed", "{:.3f}"),
    ("notices_on_background_per_stream", "notices on background / stream", "{:.2f}"),
    ("notice_precision", "notice precision", "{:.3f}"),
    ("strict_precision", "strict precision", "{:.3f}"),
    ("quality", "quality, fixed 16 s, retirement", "{:.3f}"),
    ("quality_held", "quality, tuned delay, hold", "{:.3f}"),
    ("cost_s_per_stream", "cost s / stream", "{:.3f}"),
]
BESIDE = [
    ("leak_anchor_correct_share", "leak anchor-correct", "{:.3f}"),
    ("leak_site_correct_share", "leak site-correct", "{:.3f}"),
    ("site_correct_notice_share", "site-correct notices", "{:.3f}"),
    ("precision_plain", "precision on plain", "{:.3f}"),
    ("precision_hard", "precision on hard", "{:.3f}"),
    ("precision_decoy", "precision on decoys", "{:.3f}"),
    ("plain_noticed_share", "plain noticed", "{:.3f}"),
    ("notices_on_plain_per_stream", "notices on plain / stream", "{:.2f}"),
    ("notices_on_hard_per_stream", "notices on hard / stream", "{:.2f}"),
    ("notices_on_decoy_per_stream", "notices on decoys / stream", "{:.2f}"),
    ("notices_per_stream", "notices / stream", "{:.2f}"),
    ("notices_per_incident", "notices per incident", "{:.2f}"),
    ("leak_quality", "leak quality, fixed", "{:.3f}"),
    ("leak_quality_held", "leak quality, held", "{:.3f}"),
    ("calls_per_stream", "calls / stream", "{:.2f}"),
]
PAIRED_MEASURES = [
    "hard_noticed_share", "hard_anchor_correct_share", "hard_site_correct_share",
    "hard_anchor_site_correct_share", "leak_noticed_share", "notices_on_background_per_stream",
    "notice_precision", "strict_precision", "quality", "quality_held",
]


def label(name, sel):
    if name == "reanchor":
        c = sel["stage2"]["chosen"]
        return (f"ReanchorNoticer ({c['isolation']}, g = {c['gap_ms']} ms, burst >= {c['burst']}, "
                f"z = {c['z']:g})")
    kind, p = C.C1.GRID[name]
    if kind == "rung":
        return "RungNoticer, z = 3 (default)" if not p else f"RungNoticer, z = {p['z']:g}"
    if kind == "change_triggered":
        return f"ChangeTriggered, q = {p['q']:g} s"
    return f"EarliestAnchor, l = {p['l']:g} s"


def sensitivity_label(name):
    if name.startswith("reanchor_"):
        return name
    kind, p = C.C1.GRID[name]
    if kind == "rung":
        return "RungNoticer, z = 3 (default)" if not p else f"RungNoticer, z = {p['z']:g}"
    if kind == "change_triggered":
        return f"ChangeTriggered, q = {p['q']:g} s"
    return f"EarliestAnchor, l = {p['l']:g} s"


def fmt(point, lo, hi, spec):
    f = spec.format
    return f"{f(point)} [{f(lo)}, {f(hi)}]"


def md(df, cols):
    head = ["noticer"] + [c[1] for c in cols]
    lines = ["| " + " | ".join(head) + " |", "|" + "|".join("---" for _ in head) + "|"]
    for _, r in df.iterrows():
        cells = [r["noticer"]]
        for k, _, spec in cols:
            cells.append(fmt(r[k], r[f"{k}_lo"], r[f"{k}_hi"], spec) if k in r and pd.notna(r[k]) else "")
        lines.append("| " + " | ".join(cells) + " |")
    return "\n".join(lines)


def main():
    sel = C.load_selected()
    run = load_stream_run(C.RUNS / C.run_id("heldout"))
    seeds = [int(s) for s in next(iter(run.arms.values())).results["seed"]]
    assert seeds == list(range(C.HELDOUT_SEEDS[0], C.HELDOUT_SEEDS[0] + C.HELDOUT_SEEDS[1])), "held-out seeds"
    m = B.Measures(run)
    pts = m.points()
    ci = m.boot(C.BOOT_SEED, C.N_RESAMPLES)

    def row_of(arm, name, text):
        i = m.row[arm]
        row = {"noticer": text, "config": name, "arm": arm}
        for key in B.MEASURES:
            row[key] = float(pts[key][i])
            row[f"{key}_lo"] = float(ci[key][0][i])
            row[f"{key}_hi"] = float(ci[key][1][i])
        return row

    rows = []
    for name in C.TABLE:
        row = row_of(C.arm_name(name), name, label(name, sel))
        hold = m.row[C.hold_arm_name(name)]
        for key, out in (("quality", "quality_held"), ("leak_quality", "leak_quality_held")):
            row[out] = float(pts[key][hold])
            row[f"{out}_lo"] = float(ci[key][0][hold])
            row[f"{out}_hi"] = float(ci[key][1][hold])
        row["held_delay_s"] = sel["stage3"][name]["delay_s"]
        # The notice record of the run with the hold, beside, never mixed into the columns above.
        for key in ("hard_anchor_correct_share", "notices_on_background_per_stream", "hard_noticed_share"):
            row[f"{key}_in_hold_run"] = float(pts[key][hold])
        rows.append(row)
    table = pd.DataFrame(rows)
    table.to_csv(C.OUT / "b2-noticers-table.csv", index=False)

    # Sensitivity: every configuration the run played, at the fixed delay (reading 1).
    sens = []
    for arm in m.names:
        if not arm.startswith("sel_") or not arm.endswith("_privileged"):
            continue
        name = arm[len("sel_"):-len("_privileged")]
        sens.append(row_of(arm, name, sensitivity_label(name) if name != "reanchor" else label(name, sel)))
    sens = pd.DataFrame(sens)
    sens["table_row"] = sens["config"].isin(C.TABLE)
    sens.to_csv(C.OUT / "b2-noticers-sensitivity.csv", index=False)

    lat = []
    for name in C.TABLE:
        t = notice_latency(run.arms[C.arm_name(name)])
        t.insert(0, "config", name)
        lat.append(t)
    pd.concat(lat).to_csv(C.OUT / "b2-noticers-latency.csv", index=False)

    # Paired differences against the M2 comparator (RungNoticer at z = 2), the same resamples.
    paired = []
    for name in C.TABLE:
        for measure in PAIRED_MEASURES:
            if measure == "quality_held":
                a, b = C.hold_arm_name(name), C.hold_arm_name(C.COMPARATOR)
                measure_key = "quality"
            else:
                a, b = C.arm_name(name), C.arm_name(C.COMPARATOR)
                measure_key = measure
            point, lo, hi = m.paired_difference(measure_key, a, b, C.BOOT_SEED, C.N_RESAMPLES)
            paired.append({"config": name, "against": C.COMPARATOR, "measure": measure,
                           "difference": point, "lower": lo, "higher": hi})
    pd.DataFrame(paired).to_csv(C.OUT / "b2-paired.csv", index=False)

    text = ["### Acceptance table", "", md(table, ACCEPTANCE), "", "### Beside it", "", md(table, BESIDE), "",
            "### Sensitivity: every configuration of the run, held-out (fixed 16 s delay, retirement)", "",
            md(sens, [c for c in ACCEPTANCE if c[0] not in ("quality_held",)]), "",
            md(sens, [c for c in BESIDE if c[0] not in ("leak_quality_held",)]), ""]
    (C.OUT / "b2-noticers-table.md").write_text("\n".join(text))
    print("\n".join(text))


if __name__ == "__main__":
    main()
