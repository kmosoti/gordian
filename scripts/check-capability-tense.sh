#!/usr/bin/env bash
# README.md and AGENTS.md may claim a planned capability in the present tense only once its Atoms
# have validating closure records (D6).
#
# The list of claims is the "### Planned, not built" table in README.md itself — the left column is
# the banned phrase and the right column names the Atoms that unban it — so a capability cannot be
# claimed by editing prose, only by closing the Atom that makes the claim true. The table block is
# excluded from the search: it is where the claims are legitimately named.
#
# EXTRA_PHRASES below carries near-synonyms the label alone would miss. Each key is a distinctive
# fragment that MUST match exactly one row label, so a reworded row keeps its variants and a
# deleted or duplicated row fails loudly rather than silently disarming them.
#
# Schema-level validity of a closure record is scripts/check-closure-records.sh's job; this checker
# only asks whether a well-formed record for the Atom exists.
#
# Usage: check-capability-tense.sh [ROOT]
set -euo pipefail
root="${1:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$root"

for required in README.md AGENTS.md; do
  if [ ! -f "$required" ]; then
    echo "FAIL: $required is missing"
    exit 1
  fi
done

python3 - README.md AGENTS.md <<'PY'
"""Fail on a present-tense claim of a capability whose Atoms have no closure record."""

import json
import os
import re
import sys

readme_path, agents_path = sys.argv[1:3]

# Near-synonyms of a table label that prose would otherwise smuggle a present-tense claim through.
# Each key is matched as a case-insensitive substring against the row labels and MUST hit exactly
# one row.
EXTRA_PHRASES = {
    "Mission Graph semantics": [
        "Rust owns Mission Graph semantics",
        "Rust is the production substrate",
    ],
    "conformance pipeline": [
        "conformance pipeline checks",
        "the conformance pipeline compares",
    ],
    "scheduler": [
        "the scheduler dispatches",
    ],
    "evidence store": [
        "the evidence store binds",
    ],
    "Jujutsu adapter": [
        "the Jujutsu adapter drives",
    ],
}

problems = []

with open(readme_path, encoding="utf-8") as handle:
    readme_lines = handle.read().splitlines()

heading = None
for index, line in enumerate(readme_lines):
    if re.match(r"^#{2,4} +Planned, not built\s*$", line):
        heading = index
        break

if heading is None:
    print(f"FAIL: {readme_path}: no '### Planned, not built' section; the claim list is that table")
    raise SystemExit(1)

end = len(readme_lines)
for index in range(heading + 1, len(readme_lines)):
    if re.match(r"^#{1,6} +", readme_lines[index]):
        end = index
        break

block = readme_lines[heading:end]
rows = []
for line in block:
    if not line.startswith("|"):
        continue
    cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
    if len(cells) < 3:
        continue
    if cells[0].lower().startswith("planned capability") or set(cells[0]) <= {"-", ":", " "}:
        continue
    rows.append(cells)

if not rows:
    problems.append(f"{readme_path}: the 'Planned, not built' table has no rows")

for path in (readme_path, agents_path):
    with open(path, encoding="utf-8") as handle:
        if "scripts/check-capability-tense.sh" not in handle.read():
            problems.append(f"{path}: no longer names scripts/check-capability-tense.sh")

labels = [cells[0] for cells in rows]
variants = {}
for key, phrases in EXTRA_PHRASES.items():
    matched = [label for label in labels if key.lower() in label.lower()]
    if len(matched) != 1:
        problems.append(
            f"{readme_path}: EXTRA_PHRASES key {key!r} matches {len(matched)} row labels "
            f"{matched}; exactly one is required, or its variants stop applying unnoticed"
        )
        continue
    variants.setdefault(matched[0], []).extend(phrases)

# The searchable text: all of AGENTS.md, and README.md minus the table block that declares the
# claims. Line numbers are preserved so a failure points at the offending line.
searchable = [(readme_path, number + 1, line)
              for number, line in enumerate(readme_lines)
              if not heading <= number < end]
with open(agents_path, encoding="utf-8") as handle:
    searchable += [(agents_path, number + 1, line)
                   for number, line in enumerate(handle.read().splitlines())]


def record_exists(atom):
    path = os.path.join("artifacts", "atoms", atom, "closure.json")
    if not os.path.isfile(path):
        return False, f"{path} does not exist"
    try:
        with open(path, encoding="utf-8") as handle:
            record = json.load(handle)
    except (OSError, ValueError) as error:
        return False, f"{path} is unreadable or malformed: {error}"
    if record.get("record_format") != "gordian-closure-v1":
        return False, f"{path} is not a gordian-closure-v1 record"
    if record.get("atom_id") != atom:
        return False, f"{path} records atom_id {record.get('atom_id')!r}"
    return True, ""


for cells in rows:
    label = cells[0]
    unblock = cells[-1]
    atoms = re.findall(r"#(\d+)", unblock)
    if not atoms:
        problems.append(
            f"{readme_path}: row {label!r} names no Atom in its 'Becomes present tense when' cell"
        )
        continue
    reasons = []
    for atom in atoms:
        ok, reason = record_exists(atom)
        if not ok:
            reasons.append(reason)
    if not reasons:
        continue  # every Atom is closed; the claim may be stated in the present tense.
    phrases = [label] + variants.get(label, [])
    for phrase in phrases:
        needle = phrase.lower()
        for path, number, line in searchable:
            if needle in line.lower():
                problems.append(
                    f"{path}:{number}: present-tense claim {phrase!r} while "
                    f"{reasons[0]}: {line.strip()[:100]}"
                )

if problems:
    for problem in problems:
        print(f"FAIL: {problem}")
    raise SystemExit(1)

print(
    f"OK: {len(rows)} planned capabilities; none is claimed in the present tense without a "
    "closure record."
)
PY
