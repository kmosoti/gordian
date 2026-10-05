"""Fill scripts/r4-headroom.template.md from the committed CSVs and write ../r4-headroom.md.

Usage: r4_assemble.py
Exploration (nothing here tests a hypothesis). Every {{name}} is the output of `r4_tables.py name`,
except {{sha_rows}} (rows of r4-results-sha256.csv) and {{commits}} (git log since the margin commit).
"""

import pathlib
import re
import subprocess

import pandas as pd

import r4_common as C
import r4_tables as T

here = pathlib.Path(__file__).resolve().parent
text = (here / "r4-headroom.template.md").read_text()
tables = {
    "settings": T.settings, "robustness": T.robustness, "reach": T.reach, "families": T.families,
    "tiers": T.tiers, "decomposition": T.decomposition, "sizing": T.sizing, "frontier": T.frontier,
    "provenance": T.provenance, "drift": T.drift,
}
for name, fn in tables.items():
    text = text.replace("{{" + name + "}}", fn())
text = text.replace("{{sha_rows}}", str(len(pd.read_csv(C.OUT / "r4-results-sha256.csv"))))
log = subprocess.run(
    ["git", "log", "--reverse", "--format=- `%h` %s", "30091e3..HEAD"],
    cwd=C.ROOT, capture_output=True, text=True, check=True,
).stdout.strip()
text = text.replace("{{commits}}", log)
left = re.findall(r"\{\{[a-z_]+\}\}", text)
assert not left, left
(C.OUT / "r4-headroom.md").write_text(text)
print(C.OUT / "r4-headroom.md", len(text.splitlines()), "lines")
