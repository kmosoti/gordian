"""Fill scripts/r5-decomposed-headroom.template.md from the committed CSVs and write
../r5-decomposed-headroom.md.

Usage: r5_assemble.py
Exploration (nothing here tests a hypothesis). Every {{name}} is the output of `r5_tables.py name`,
except {{commits}} (git log since main) and {{sha_rows}} (rows of r5-results-sha256.csv).
"""

import pathlib
import re
import subprocess

import pandas as pd

import r5_common as C
import r5_tables as T

here = pathlib.Path(__file__).resolve().parent
text = (here / "r5-decomposed-headroom.template.md").read_text()
tables = {
    "criterion": T.criterion, "decomposition": T.decomposition, "decomposition_diffs": T.decomposition_diffs,
    "families": T.families, "decoys": T.decoys, "tiers": T.tiers, "trace": T.trace,
    "sensitivity": T.sensitivity, "robustness": T.robustness, "tuning": T.tuning,
    "provenance": T.provenance, "drift": T.drift, "runs": T.runs, "regression": T.regression,
}
for name, fn in tables.items():
    text = text.replace("{{" + name + "}}", fn())
text = text.replace("{{sha_rows}}", str(len(pd.read_csv(C.OUT / "r5-results-sha256.csv"))))
log = subprocess.run(
    ["git", "log", "--reverse", "--format=- `%h` %s", "main..HEAD"],
    cwd=C.ROOT, capture_output=True, text=True, check=True,
).stdout.strip()
text = text.replace("{{commits}}", log)
left = re.findall(r"\{\{[a-z_]+\}\}", text)
assert not left, left
(C.OUT / "r5-decomposed-headroom.md").write_text(text)
print(C.OUT / "r5-decomposed-headroom.md", len(text.splitlines()), "lines")
