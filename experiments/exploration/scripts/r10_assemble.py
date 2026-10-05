"""Fill scripts/r10-salience-ceiling.template.md from the committed CSVs and write
../r10-salience-ceiling.md.

Usage: r10_assemble.py
Exploration (nothing here tests a hypothesis). Every {{name}} is the output of `r10_tables.py name`,
except {{commits}} (git log since R10's start).
"""

import pathlib
import re
import subprocess

import r10_common as C
import r10_tables as T

here = pathlib.Path(__file__).resolve().parent
text = (here / "r10-salience-ceiling.template.md").read_text()
for name, fn in T.TABLES.items():
    text = text.replace("{{" + name + "}}", fn())
log = subprocess.run(
    ["git", "log", "--reverse", "--format=- `%h` %s", "5dbaf38..HEAD"],
    cwd=C.ROOT, capture_output=True, text=True, check=True,
).stdout.strip()
text = text.replace("{{commits}}", log)
left = re.findall(r"\{\{[a-z_0-9]+\}\}", text)
assert not left, left
(C.OUT / "r10-salience-ceiling.md").write_text(text)
print(C.OUT / "r10-salience-ceiling.md", len(text.splitlines()), "lines")
