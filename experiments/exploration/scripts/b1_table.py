"""The B1 acceptance table: the four noticers on the 200 held-out streams at b = 5, rho = 0.7, with
the selection oracle and the rung's context, with 90% cluster-bootstrap intervals over streams.

Usage: b1_table.py   (after the held-out run and `b1_select.py`; writes
                      experiments/exploration/b1-noticers-table.csv, b1-noticers-sensitivity.csv,
                      b1-noticers-latency.csv and b1-noticers-table.md)

The four rows are the two `RungNoticer`s (default threshold z = 3, and z = 2) and the parameters
`b1_select.py` chose on the tuning streams for ChangeTriggered and EarliestAnchor. Every other
configuration of the grid was run on the held-out streams too and is the sensitivity table; none of
it was used to choose anything. The table makes no claim about which noticer is better.
"""

import json

import pandas as pd

import b1_common as C
import b1_stats as B
from gordian_analysis.load import load_stream_run
from gordian_analysis.stream import notice_latency

# The columns of the acceptance table (the queue's B1) and the ones kept beside them.
ACCEPTANCE = [
    ("hard_noticed_share", "hard non-leak noticed", "{:.3f}"),
    ("hard_anchor_correct_share", "anchor-correct", "{:.3f}"),
    ("leak_noticed_share", "leak noticed", "{:.3f}"),
    ("notices_on_background_per_stream", "notices on background / stream", "{:.2f}"),
    ("quality", "hard quality (selection oracle)", "{:.3f}"),
    ("cost_s_per_stream", "cost s / stream", "{:.3f}"),
]
BESIDE = [
    ("leak_anchor_correct_share", "leak anchor-correct", "{:.3f}"),
    ("plain_noticed_share", "plain noticed", "{:.3f}"),
    ("notices_on_plain_per_stream", "notices on plain / stream", "{:.2f}"),
    ("notices_on_hard_per_stream", "notices on hard / stream", "{:.2f}"),
    ("notices_on_decoy_per_stream", "notices on decoys / stream", "{:.2f}"),
    ("notices_per_stream", "notices / stream", "{:.2f}"),
    ("notices_per_incident", "notices per incident", "{:.2f}"),
    ("leak_quality", "leak quality", "{:.3f}"),
    ("substrate_s_per_stream", "substrate s / stream", "{:.4f}"),
    ("calls_per_stream", "calls / stream", "{:.2f}"),
]


def label(name):
    kind, p = C.GRID[name]
    if kind == "rung":
        return "RungNoticer, z = 3 (default)" if not p else f"RungNoticer, z = {p['z']:g}"
    if kind == "change_triggered":
        return f"ChangeTriggered, q = {p['q']:g} s"
    return f"EarliestAnchor, l = {p['l']:g} s"


def fmt(point, lo, hi, spec):
    f = spec.format
    return f"{f(point)} [{f(lo)}, {f(hi)}]"


def main():
    run = load_stream_run(C.RUNS / C.run_id("heldout"))
    seeds = [int(s) for s in next(iter(run.arms.values())).results["seed"]]
    assert seeds == list(range(C.HELDOUT_SEEDS[0], C.HELDOUT_SEEDS[0] + C.HELDOUT_SEEDS[1])), "held-out seeds"
    sel = json.load(open(C.OUT / "b1-selected.json"))
    m = B.Measures(run)
    pts = m.points()
    ci = m.boot(C.BOOT_SEED, C.N_RESAMPLES)
    primary = [
        "rung_z3",
        "rung_z2",
        sel["change_triggered"]["noticer"],
        sel["earliest_anchor"]["noticer"],
    ]

    def rows(names):
        out = []
        for name in names:
            arm = C.arm_name(name)
            i = m.row[arm]
            row = {"noticer": label(name), "config": name, "arm": arm}
            for key in B.MEASURES:
                row[key] = float(pts[key][i])
                row[f"{key}_lo"] = float(ci[key][0][i])
                row[f"{key}_hi"] = float(ci[key][1][i])
            out.append(row)
        return pd.DataFrame(out)

    table = rows(primary)
    table.to_csv(C.OUT / "b1-noticers-table.csv", index=False)
    sens = rows([n for n, _, _ in C.grid()])
    lat = []
    for name, _, _ in C.grid():
        t = notice_latency(run.arms[C.arm_name(name)])
        t.insert(0, "config", name)
        lat.append(t)
    # The rung's noticer at z = 1 and 0.5, run after the table was made (sensitivity only). Its own
    # run, the same 200 streams: the resamples are the same draws (the seed and the number of
    # streams are the same), so its intervals are comparable with the others'.
    extra_dir = C.RUNS / C.run_id("heldout-extra")
    if extra_dir.exists():
        xrun = load_stream_run(extra_dir)
        assert [int(s) for s in next(iter(xrun.arms.values())).results["seed"]] == seeds
        xm = B.Measures(xrun)
        xpts, xci = xm.points(), xm.boot(C.BOOT_SEED, C.N_RESAMPLES)
        extra = []
        for name, _, _ in C.extra_grid():
            i = xm.row[C.arm_name(name)]
            row = {"noticer": label(name), "config": name, "arm": C.arm_name(name)}
            for key in B.MEASURES:
                row[key] = float(xpts[key][i])
                row[f"{key}_lo"] = float(xci[key][0][i])
                row[f"{key}_hi"] = float(xci[key][1][i])
            extra.append(row)
            t = notice_latency(xrun.arms[C.arm_name(name)])
            t.insert(0, "config", name)
            lat.append(t)
        # in order of the rung's own line: z = 3, 2, 1, 0.5 first, then the others
        sens = pd.concat([sens.iloc[:2], pd.DataFrame(extra), sens.iloc[2:]], ignore_index=True)
    sens["primary"] = sens["config"].isin(primary)
    sens.to_csv(C.OUT / "b1-noticers-sensitivity.csv", index=False)
    pd.concat(lat).to_csv(C.OUT / "b1-noticers-latency.csv", index=False)

    def md(df, cols):
        head = ["noticer"] + [c[1] for c in cols]
        lines = ["| " + " | ".join(head) + " |", "|" + "|".join("---" for _ in head) + "|"]
        for _, r in df.iterrows():
            cells = [r["noticer"]] + [fmt(r[k], r[f"{k}_lo"], r[f"{k}_hi"], spec) for k, _, spec in cols]
            lines.append("| " + " | ".join(cells) + " |")
        return "\n".join(lines)

    text = ["### Acceptance table", "", md(table, ACCEPTANCE), "", "### Beside it", "", md(table, BESIDE), "",
            "### Sensitivity: every configuration of the grid, held-out", "", md(sens, ACCEPTANCE), "",
            md(sens, BESIDE), ""]
    (C.OUT / "b1-noticers-table.md").write_text("\n".join(text))
    print("\n".join(text))


if __name__ == "__main__":
    main()
