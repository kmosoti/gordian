#!/usr/bin/env bash
# Every `## ` section of docs/spec/invariants.md carries exactly one `**Coverage:**` line, and its
# value is drawn from the state list fenced under docs/formal/theorem-catalog.md's
# "Formal coverage metric" heading.
#
# The state list is EXTRACTED from the theorem catalog rather than restated here, so adding a
# state to the catalog is the only way to add one to the invariant catalog (G-226, G-251, G-253).
#
# Usage: check-invariant-coverage.sh [ROOT]   ROOT defaults to the repository root; scripts/tests/
# passes a fixture root.
set -euo pipefail
root="${1:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$root"

invariants=docs/spec/invariants.md
catalog=docs/formal/theorem-catalog.md

for required in "$invariants" "$catalog"; do
  if [ ! -f "$required" ]; then
    echo "FAIL: $required is missing; the coverage rule has no subject"
    exit 1
  fi
done

python3 - "$invariants" "$catalog" <<'PY'
"""Assert one Coverage line per invariant section, valued from the catalog's state list."""

import re
import sys

invariants_path, catalog_path = sys.argv[1:3]

with open(catalog_path, encoding="utf-8") as handle:
    catalog = handle.read().splitlines()

# The state list is the first fenced block under the "Formal coverage metric" heading.
states = []
in_section = False
in_fence = False
for line in catalog:
    if re.match(r"^#{1,3} +Formal coverage metric\s*$", line):
        in_section = True
        continue
    if in_section and re.match(r"^#{1,3} +", line) and not in_fence:
        break
    if not in_section:
        continue
    if line.startswith("```"):
        if in_fence:
            break
        in_fence = True
        continue
    if in_fence and line.strip():
        states.append(line.strip())

problems = []
if not states:
    problems.append(
        f"{catalog_path}: no fenced coverage-state list under '# Formal coverage metric'"
    )

with open(invariants_path, encoding="utf-8") as handle:
    lines = handle.read().splitlines()

sections = []  # (heading, start_index)
for index, line in enumerate(lines):
    if line.startswith("## "):
        sections.append((line[3:].strip(), index))

if len(sections) < 2:
    problems.append(f"{invariants_path}: found {len(sections)} '## ' sections; expected the catalog")

coverage = re.compile(r"^\*\*Coverage:\*\* +(.+?)\s*$")
for position, (heading, start) in enumerate(sections):
    end = sections[position + 1][1] if position + 1 < len(sections) else len(lines)
    found = []
    for offset in range(start + 1, end):
        match = coverage.match(lines[offset])
        if match:
            found.append((offset + 1, match.group(1)))
    if not found:
        problems.append(f"{invariants_path}:{start + 1}: '## {heading}' has no '**Coverage:**' line")
        continue
    if len(found) > 1:
        where = ", ".join(str(number) for number, _ in found)
        problems.append(
            f"{invariants_path}: '## {heading}' has {len(found)} '**Coverage:**' lines (at {where});"
            " exactly one is required"
        )
    for number, value in found:
        if states and value not in states:
            problems.append(
                f"{invariants_path}:{number}: coverage {value!r} is not one of {states}"
            )

if problems:
    for problem in problems:
        print(f"FAIL: {problem}")
    raise SystemExit(1)

print(
    f"OK: {len(sections)} invariant sections, each with exactly one "
    f"**Coverage:** line drawn from {len(states)} catalog states."
)
PY
