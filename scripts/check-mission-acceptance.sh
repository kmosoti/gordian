#!/usr/bin/env bash
# Every row of the normative Mission acceptance table names real Atoms and a witness the runner
# knows, and every Atom is named by some row.
#
# The table in docs/implementation/project-plan.md is the only normative definition of done;
# execution-order.md section 18 links to it. A row naming an issue that does not exist can never
# resolve. An issue no row names is work the Mission can close without ever demonstrating — 26 of
# 77 Atoms were, including all four that had closed. A witness scripts/mission-witness.sh does
# not know can never be run, and one still marked pending on a closed Atom is a row whose Atoms
# are done and whose demonstration nobody wrote. Each of those is a CI failure here. Whether a
# row's Atoms are *closed* and its witness *recorded* in #69 is the separate question
# scripts/check-mission-stop-condition.sh answers.
#
# Usage: check-mission-acceptance.sh [ROOT]
set -euo pipefail
root="${1:-$(cd "$(dirname "$0")/.." && pwd -P)}"
cd "$root"

plan=docs/implementation/project-plan.md
snapshot=artifacts/atoms/issues.json
runner=scripts/mission-witness.sh

# The table is the definition of done, so its absence is a failure, not an empty subject: with
# no table nothing gates any Atom, and scripts/check-mission-stop-condition.sh fails the same way.
if [ ! -f "$plan" ] || ! grep -q '^## Mission acceptance$' "$plan"; then
  echo "FAIL: $plan has no '## Mission acceptance' table; no Atom is gated by anything"
  exit 1
fi
if [ ! -f "$runner" ]; then
  echo "FAIL: $runner is missing; the Witness column names checks nothing can run"
  exit 1
fi
if ! listing="$(bash "$runner" --list)"; then
  echo "FAIL: $runner --list failed"
  exit 1
fi

python3 - "$plan" "$snapshot" "$listing" <<'PY'
import json
import os
import re
import sys

plan_path, snapshot_path, listing = sys.argv[1:]
text = "\n" + open(plan_path, encoding="utf-8").read()
section = text.split("\n## Mission acceptance\n", 1)[1].split("\n## ", 1)[0]

WITNESS_ID = re.compile(r"^[a-z][a-z0-9-]*$")
ATOM_LIST = re.compile(r"^#[1-9][0-9]*(?:, #[1-9][0-9]*)*$")
MISSION_RECORD = 69  # the one Atom no row cites: its record carries the witnesses

problems = []
rows = []
for line in section.splitlines():
    if not line.startswith("|"):
        continue
    cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
    if cells and cells[0] in {"#", "---"}:
        continue
    # A row of the wrong shape is a defect, not a line to skip: the old three-cell parser
    # silently dropped anything else, and a dropped row is an ungated row.
    if len(cells) != 4:
        problems.append(f"table line {line!r} has {len(cells)} cells; expected 4")
        continue
    number, item, atom_cell, witness = cells
    if not number.isdigit():
        problems.append(f"table line {line!r} has a non-numeric row number")
        continue
    if ATOM_LIST.fullmatch(atom_cell) is None:
        problems.append(f"row {number} has invalid Atom reference list {atom_cell!r}; expected #N or #N, #N")
    if WITNESS_ID.fullmatch(witness) is None:
        problems.append(f"row {number} has invalid witness id {witness!r}; expected [a-z][a-z0-9-]*")
    rows.append((int(number), item, [int(n) for n in re.findall(r"#(\d+)", atom_cell)], witness))

if not rows:
    problems.append("the Mission acceptance table has no rows")

numbers = [number for number, _, _, _ in rows]
if numbers != list(range(1, len(numbers) + 1)):
    problems.append(f"the acceptance rows are not numbered 1..{len(numbers)}: {numbers}")

known = None
if os.path.isfile(snapshot_path):
    issues = json.load(open(snapshot_path, encoding="utf-8"))
    if isinstance(issues, dict):
        issues = issues.get("issues", [])
    known = {int(issue["number"]) for issue in issues if issue.get("number") is not None}

cited = {}
for number, item, atoms, _ in rows:
    if not atoms:
        problems.append(f"row {number} ({item}) names no Atom")
    for atom in atoms:
        if atom < 1:
            problems.append(f"row {number} names a non-issue id #{atom}")
        elif known is not None and atom not in known:
            problems.append(f"row {number} names #{atom}, which is not an issue")
        cited.setdefault(atom, []).append(number)

if MISSION_RECORD in cited:
    problems.append(
        f"row(s) {cited[MISSION_RECORD]} cite #{MISSION_RECORD}, whose record holds the "
        "witnesses; it cannot witness itself"
    )
if known is not None:
    uncited = sorted(atom for atom in known if atom not in cited and atom != MISSION_RECORD)
    if uncited:
        problems.append(
            f"{len(uncited)} issue(s) are named by no acceptance row, so closing them proves "
            f"nothing: {', '.join(f'#{n}' for n in uncited)}"
        )

# The runner's registry and the table's Witness column are two spellings of one list.
runner = {}
LISTING_LINE = re.compile(r"^([a-z][a-z0-9-]*) (implemented|pending #([1-9][0-9]*))$")
for entry in listing.splitlines():
    match = LISTING_LINE.fullmatch(entry)
    if match is None:
        problems.append(f"scripts/mission-witness.sh --list printed an unparseable line {entry!r}")
        continue
    if match.group(1) in runner:
        problems.append(f"scripts/mission-witness.sh lists witness {match.group(1)!r} twice")
    runner[match.group(1)] = int(match.group(3)) if match.group(3) else None

table = {}
for number, _, atoms, witness in rows:
    if witness in table:
        problems.append(f"row {number} reuses witness {witness!r} of row {table[witness][0]}")
        continue
    table[witness] = (number, atoms)

for witness in sorted(set(table) - set(runner)):
    problems.append(
        f"row {table[witness][0]} names witness {witness!r}, which scripts/mission-witness.sh "
        "does not list"
    )
for witness in sorted(set(runner) - set(table)):
    problems.append(f"scripts/mission-witness.sh lists witness {witness!r}, which no row names")

for witness, pending in runner.items():
    if pending is None or witness not in table:
        continue
    number, atoms = table[witness]
    if pending not in atoms:
        problems.append(
            f"witness {witness!r} is pending on #{pending}, which row {number} does not cite"
        )
    elif os.path.isfile(f"artifacts/atoms/{pending}/closure.json"):
        problems.append(
            f"witness {witness!r} is pending on #{pending}, which has a closure record; "
            "implement the witness in scripts/mission-witness.sh"
        )

if problems:
    for problem in problems:
        print(f"FAIL: {problem}")
    raise SystemExit(1)

implemented = sum(1 for pending in runner.values() if pending is None)
print(
    f"OK: {len(rows)} acceptance rows naming {len(cited)} distinct Atoms; "
    f"{implemented} witness(es) implemented, {len(runner) - implemented} pending."
)
PY
