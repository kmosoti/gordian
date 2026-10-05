"""Fill scripts/r7-distractor-law.template.md from the committed CSVs and write ../r7-distractor-law.md.

Usage: r7_assemble.py
Exploration (nothing here tests a hypothesis). Every {{name}} is the output of `r7_tables.py name`,
except {{commits}} (git log since R7's start, the plan's commit) and {{gates}} (`r7-gates.txt`, the
exit codes of the gate commands, one `name=code` per line).
"""

import pathlib
import re
import subprocess

import r7_common as C
import r7_tables as T

here = pathlib.Path(__file__).resolve().parent
text = (here / "r7-distractor-law.template.md").read_text()
for name, fn in T.TABLES.items():
    text = text.replace("{{" + name + "}}", fn())
gates = (C.OUT / "r7-gates.txt").read_text().split()
text = text.replace("{{gates}}", "; ".join(
    f"`{g.split('=')[0]}` exit {g.split('=')[1]}" for g in gates if "=" in g) + ".")
log = subprocess.run(
    ["git", "log", "--reverse", "--format=- `%h` %s", "5f04e74..HEAD"],
    cwd=C.ROOT, capture_output=True, text=True, check=True,
).stdout.strip()
text = text.replace("{{commits}}", log)
left = re.findall(r"\{\{[a-z_0-9]+\}\}", text)
assert not left, left
(C.OUT / "r7-distractor-law.md").write_text(text)
print(C.OUT / "r7-distractor-law.md", len(text.splitlines()), "lines")
