"""Fill `w2-learnable-laws.template.md` from the committed CSVs and write `w2-learnable-laws.md`.

Usage: w2_assemble.py
Exploration (nothing here tests a hypothesis). Every {{name}} is the output of `w2_tables.py name`.
"""

import pathlib
import re

import w2_common as C
import w2_tables as T

text = (C.OUT / "w2-learnable-laws.template.md").read_text()
for name, fn in T.TABLES.items():
    text = text.replace("{{" + name + "}}", fn())
left = re.findall(r"\{\{[a-z_0-9]+\}\}", text)
assert not left, left
(C.OUT / "w2-learnable-laws.md").write_text(text)
print(C.OUT / "w2-learnable-laws.md", len(text.splitlines()), "lines")
