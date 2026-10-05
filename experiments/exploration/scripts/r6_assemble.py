"""Fill scripts/r6-context-headroom.template.md from the committed CSVs and write
../r6-context-headroom.md.

Usage: r6_assemble.py
Exploration (nothing here tests a hypothesis). Every {{name}} is the output of `r6_tables.py name`,
except {{commits}} (git log since R6's start) and {{heldout_frontier_inline}}.
"""

import pathlib
import re
import subprocess

import pandas as pd

import r6_common as C
import r6_tables as T

here = pathlib.Path(__file__).resolve().parent
text = (here / "r6-context-headroom.template.md").read_text()
for name, fn in T.TABLES.items():
    text = text.replace("{{" + name + "}}", fn())
hf = pd.read_csv(C.OUT / "r6-heldout-points.csv")
p = hf[(hf.setting == T.PRIMARY) & (hf.pairing == "selection oracle")]
from gordian_analysis.frontier import pareto_mask  # noqa: E402

parts = []
for kind in C.KINDS:
    g = p[p.kind == kind]
    parts.append(f"`{kind}` {int(pareto_mask(g.quality, g.refs_per_call).sum())} of {len(g)}")
text = text.replace("{{heldout_frontier_inline}}", ", ".join(parts) + " are still on their held-out frontier (primary setting)")
log = subprocess.run(
    ["git", "log", "--reverse", "--format=- `%h` %s", "1b7b0c7..HEAD"],
    cwd=C.ROOT, capture_output=True, text=True, check=True,
).stdout.strip()
text = text.replace("{{commits}}", log)
left = re.findall(r"\{\{[a-z_]+\}\}", text)
assert not left, left
(C.OUT / "r6-context-headroom.md").write_text(text)
print(C.OUT / "r6-context-headroom.md", len(text.splitlines()), "lines")
