#!/usr/bin/env python3
"""Write fitted weights into the unit tables of the Rust sources.

Reads the JSON written by `calibrate_ops.py --json`, rounds each weight to three significant
figures (picoseconds per unit: a fit of noisy timings has no more precision than that), and
rewrites the `UNITS` table of each component and the `RULE_UNITS` table of the shared rule, one
`Unit` per line. The record of what was measured and when is the comment above each table; this
script writes the date and the command given with `--record`, and `CALIBRATION.md`, section 9,
holds the rest.

    apply_weights.py FIT.json --record "2026-10-05, five runs, calibrate_ops.py"
"""

from __future__ import annotations

import argparse
import json
import math
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

TABLES = {
    "heuristic": ("crates/gordian-components/src/heuristic.rs", "UNITS", "The heuristic's"),
    "estimator": ("crates/gordian-components/src/estimator.rs", "UNITS", "The estimator's"),
    "memory": ("crates/gordian-components/src/memory.rs", "UNITS", "The lookup's"),
    "verifier": ("crates/gordian-components/src/verifier.rs", "UNITS", "The verifier's"),
    "rule": ("crates/gordian-run/src/policy/decide.rs", "RULE_UNITS", "The shared rule's"),
}


def three_figures(x: float) -> int:
    if x <= 0:
        return 0
    digits = 3 - int(math.floor(math.log10(x))) - 1
    return int(round(round(x, digits)))


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("fit")
    ap.add_argument("--record", required=True, help="what to write above each table")
    args = ap.parse_args()
    fit = json.loads(Path(args.fit).read_text())
    for target, (path, const, owner) in TABLES.items():
        # The weights are the unrounded NNLS ones, in ns; the table holds ps. The fit names its
        # units alphabetically (the calibration program writes a JSON object), so match by name.
        by_name = {
            u: three_figures(w * 1000)
            for u, w in zip(fit[target]["units"], fit[target]["weights_ns"])
        }
        file = ROOT / path
        text = file.read_text()
        pattern = re.compile(
            r"(?:// Weights, picoseconds per unit[^\n]*\n(?://[^\n]*\n)*)?"
            r"(?:#\[rustfmt::skip\]\n)?"
            r"(/// [^\n]*\n)"
            rf"pub const {const}: &\[Unit\] = &\[\n.*?\n\];\n",
            re.S,
        )
        m = pattern.search(text)
        assert m, f"no {const} table in {path}"
        names = re.findall(r'name: "([a-z_]+)"', m.group(0))
        assert sorted(names) == sorted(by_name), (
            f"{path}: the table has {names}, the fit has {sorted(by_name)}"
        )
        weights = [by_name[n] for n in names]
        units = names
        lines = "".join(
            f'    Unit {{ name: "{n}", weight_ps: {w} }},\n' for n, w in zip(units, weights)
        )
        block = (
            f"// Weights, picoseconds per unit: {args.record}.\n"
            "// Non-negative least squares on the minimum time per call, weighted by 1/time, with no\n"
            "// intercept beyond the explicit per-call unit; rounded to three figures. What each unit\n"
            "// counts, the fit, its validity and its limits: CALIBRATION.md, section 9. Recalibrate\n"
            "// after a change to the CPU, the release profile, or the code that is counted.\n"
            "#[rustfmt::skip]\n"
            f"{m.group(1)}pub const {const}: &[Unit] = &[\n{lines}];\n"
        )
        file.write_text(text[: m.start()] + block + text[m.end():])
        print(path, dict(zip(units, weights)))


if __name__ == "__main__":
    main()
