"""The B4 report's condensed tables, written from `b4-table.csv` and `b4-paired.csv` (no run output is read).

Usage: b4_summary.py   (writes experiments/exploration/b4-summary-tables.md)

Short row names, one table per selector with the acceptance columns and 90% cluster-bootstrap intervals, then the
paired differences against ramp + split over the re-anchor under the same selector. The full tables (every
column) are `b4-public-selector-table.md`. Numbers only: no row is marked better or worse.
"""

import pandas as pd

import b4_common as C

NAMES = {
    "rung_z3": "rung z=3", "rung_z2": "rung z=2", "reanchor": "re-anchor", "ramp_over_r3": "ramp / rung3",
    "split_over_r3": "split / rung3", "ramp_split_over_r3": "ramp+split / rung3",
    "ramp_over_re2": "ramp / re-anchor", "split_over_re2": "split / re-anchor",
    "ramp_split_over_re2": "ramp+split / re-anchor (comparator)", "reanchor_z3": "re-anchor z=3",
    "ramp_split_over_re3": "ramp+split / re-anchor z=3", "medium_t100": "medium 100 ms",
}
for _s in C.FOLLOW_BASES:
    NAMES[C.follow_stem(_s)] = NAMES[_s] + " + follow-up"

MAIN = [("quality", "hard quality", 3), ("critical_misses_per_stream", "crit. misses/stream", 2),
        ("plain_accuracy", "plain acc.", 3), ("calls_per_stream", "calls/stream", 1),
        ("cost_s_per_stream", "cost s/stream", 2), ("esc_decoy_per_stream", "esc. decoys/stream", 2),
        ("esc_plain_per_stream", "esc. plain/stream", 1)]
SELECTORS = [("oracle", "selection oracle (privileged ceiling)"), ("thr", "public threshold"),
             ("chg", "public change"), ("always", "always escalate at 16 s"), ("never", "never escalate")]


def cell(row, key, digits):
    f = f"{{:.{digits}f}}"
    return f"{f.format(row[key])} [{f.format(row[key + '_lo'])}, {f.format(row[key + '_hi'])}]"


def main():
    t = pd.read_csv(C.OUT / "b4-table.csv")
    p = pd.read_csv(C.OUT / "b4-paired.csv")
    out = []
    for sel, text in SELECTORS:
        sub = t[t.selector == sel]
        out += [f"#### {text}", "", "| row | parameter | " + " | ".join(h for _, h, _ in MAIN) + " |",
                "|---|---|" + "---|" * len(MAIN)]
        for _, r in sub.iterrows():
            par = "" if pd.isna(r["parameter"]) else r["parameter"]
            out.append(f"| {NAMES[r['config']]} | {par} | " + " | ".join(cell(r, k, d) for k, _, d in MAIN) + " |")
        out.append("")
    out += ["### Paired differences against the comparator (ramp+split / re-anchor), same selector", ""]
    for sel, text in SELECTORS[:3]:
        out += [f"#### {text}", "", "| row | " + " | ".join(h for _, h, _ in MAIN) + " |", "|---|" + "---|" * len(MAIN)]
        sub = p[(p.selector == sel) & (p.config != C.COMPARATOR)]
        for stem in [s for s in NAMES if s in set(sub.config)]:
            g = sub[sub.config == stem].set_index("measure")
            cells = []
            for k, _, d in MAIN:
                r = g.loc[k]
                f = f"{{:+.{d}f}}"
                cells.append(f"{f.format(r['difference'])} [{f.format(r['lower'])}, {f.format(r['higher'])}]")
            out.append(f"| {NAMES[stem]} | " + " | ".join(cells) + " |")
        out.append("")
    (C.OUT / "b4-summary-tables.md").write_text("\n".join(out))
    print("\n".join(out))


if __name__ == "__main__":
    main()
