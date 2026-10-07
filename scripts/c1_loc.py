"""Lines of rule code against lines of graph code (work item C1): the three noticers' source, counted.

Usage: python3 -I scripts/c1_loc.py [OUT.csv]

For each file: total lines; code lines (not blank, not a comment line `//`, `///`, `//!`), outside
and inside a trailing `#[cfg(test)]` module. Groups, and what the partition means (it is the PI's, not
the repository's; the report says what each file holds):

- `hand-written B3`: the four noticer files of B3 (`noticer_rung.rs`, `noticer_reanchor.rs`,
  `noticer_ramp.rs`, `noticer_split.rs`): rules and the bookkeeping that evaluates them are one
  thing there, so all of it is rule code. `noticer.rs` (the seam, `Tracked`, the attach rule, the
  score, the spelling) is shared by every noticer and counted apart.
- `dataflow rules`: `dataflow/rules.rs` (pure rules). `dataflow graph`: `dataflow/program.rs` (the
  relations and their evaluation) and `dataflow/noticing.rs` (the seam adapter). `dataflow engine`:
  `dataflow/engine.rs` (general, names no rule). `dataflow spec`: `dataflow/mod.rs`.
- `medium graph`: `medium/graph.rs` (the hand-designed graph of cells), `medium/noticing.rs` and
  `medium/adapters.rs` (the sense, clock, effector and ledger adapters), `medium/mod.rs`.
  `medium engine`: `crates/gordian-medium/src/` (the substrate, general, names no rule).
"""
import csv
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ARMS = "crates/gordian-run/src/stream/arms"

GROUPS = [
    ("hand-written B3", "rules", [f"{ARMS}/noticer_{n}.rs" for n in ("rung", "reanchor", "ramp", "split")]),
    ("hand-written, shared", "seam", [f"{ARMS}/noticer.rs"]),
    ("dataflow", "rules", [f"{ARMS}/dataflow/rules.rs"]),
    ("dataflow", "graph", [f"{ARMS}/dataflow/program.rs", f"{ARMS}/dataflow/noticing.rs"]),
    ("dataflow", "engine", [f"{ARMS}/dataflow/engine.rs"]),
    ("dataflow", "spec", [f"{ARMS}/dataflow/mod.rs"]),
    ("dataflow", "bench", [f"{ARMS}/dataflow/bench.rs"]),
    ("medium", "graph", [f"{ARMS}/medium/graph.rs", f"{ARMS}/medium/noticing.rs",
                         f"{ARMS}/medium/adapters.rs", f"{ARMS}/medium/mod.rs"]),
    ("medium", "engine", sorted(str(p.relative_to(ROOT)) for p in (ROOT / "crates/gordian-medium/src").glob("*.rs"))),
]


def count(path):
    lines = (ROOT / path).read_text().splitlines()
    total = len(lines)
    cut = next((i for i, l in enumerate(lines) if l.strip() == "#[cfg(test)]"), len(lines))

    def code(ls):
        return sum(1 for l in ls if l.strip() and not l.strip().startswith("//"))

    return total, code(lines[:cut]), code(lines[cut:])


def main(out):
    rows = []
    for group, part, files in GROUPS:
        for f in files:
            total, code, test = count(f)
            rows.append({"group": group, "part": part, "file": f, "total_lines": total,
                         "code_lines": code, "test_module_code_lines": test})
    sums = {}
    for r in rows:
        k = (r["group"], r["part"])
        s = sums.setdefault(k, [0, 0, 0])
        s[0] += r["total_lines"]
        s[1] += r["code_lines"]
        s[2] += r["test_module_code_lines"]
    print("| group | part | total lines | code lines (outside tests) | test-module code lines |")
    print("|---|---|---|---|---|")
    for (g, p), (t, c, x) in sums.items():
        print(f"| {g} | {p} | {t} | {c} | {x} |")
    if out:
        with open(out, "w", newline="") as fh:
            w = csv.DictWriter(fh, fieldnames=list(rows[0]))
            w.writeheader()
            w.writerows(rows)


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else None)
