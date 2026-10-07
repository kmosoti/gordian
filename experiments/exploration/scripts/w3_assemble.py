"""Fill `w3-world-c.template.md` from the committed CSVs and write `w3-world-c.md`.

Usage: w3_assemble.py
Exploration (nothing here tests a hypothesis). Every {{name}} is the output of `w3_tables.py name`;
the sections marked in capitals in the template were written by the PI and are kept in
`w3-sections/` next to the template (plain Markdown, with the same {{name}} placeholders).
"""

import pathlib
import re

import w2_common as C
import w3_tables as T

text = (C.OUT / "w3-world-c.template.md").read_text()
sections = C.OUT / "w3-sections"
for marker, fname in (("FLOOR_ANSWER", "floor-answer.md"), ("FLOOR_BOUNDS", "floor-bounds.md"), ("FLOOR_SECTION", "floor.md"),
                      ("BOUNDS_SECTION", "bounds.md"), ("UNCERTAINTY_SECTION", "uncertainty.md"), ("GATES_SECTION", "gates.md")):
    p = sections / fname
    if p.exists():
        text = text.replace(marker, p.read_text().strip())
for _ in range(2):
    for name, fn in T.TABLES.items():
        text = text.replace("{{" + name + "}}", fn())
left = re.findall(r"\{\{[a-z_0-9A-Z]+\}\}", text) + re.findall(r"\b(?:FLOOR_ANSWER|FLOOR_BOUNDS|FLOOR_SECTION|BOUNDS_SECTION|UNCERTAINTY_SECTION|GATES_SECTION)\b", text)
assert not left, left
(C.OUT / "w3-world-c.md").write_text(text)
print(C.OUT / "w3-world-c.md", len(text.splitlines()), "lines")
