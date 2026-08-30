#!/usr/bin/env bash
# Every Atom id in the normative Mission acceptance table names a real issue.
#
# The table in docs/implementation/project-plan.md is the only normative definition of done;
# execution-order.md section 18 links to it. A row naming an issue that does not exist is a row
# that can never resolve, so this is a CI gate. Whether each row's Atoms are *closed* is the
# separate question scripts/check-mission-stop-condition.sh answers.
set -euo pipefail
cd "$(dirname "$0")/.."

plan=docs/implementation/project-plan.md
snapshot=artifacts/atoms/issues.json

if [ ! -f "$plan" ] || ! grep -q '^## Mission acceptance' "$plan"; then
  echo "SKIP: $plan has no '## Mission acceptance' table yet."
  exit 0
fi

python3 - "$plan" "$snapshot" <<'PY'
import json
import os
import re
import sys

plan_path, snapshot_path = sys.argv[1], sys.argv[2]
section = open(plan_path, encoding="utf-8").read()
section = section.split("## Mission acceptance", 1)[1].split("\n## ", 1)[0]

rows = []
for line in section.splitlines():
    if not line.startswith("|"):
        continue
    cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
    if len(cells) != 3 or not cells[0].isdigit():
        continue
    rows.append((int(cells[0]), cells[1], [int(n) for n in re.findall(r"#(\d+)", cells[2])]))

problems = []
if not rows:
    problems.append("the Mission acceptance table has no rows")

numbers = [number for number, _, _ in rows]
if numbers != list(range(1, len(numbers) + 1)):
    problems.append(f"the acceptance rows are not numbered 1..{len(numbers)}: {numbers}")

known = None
if os.path.isfile(snapshot_path):
    issues = json.load(open(snapshot_path, encoding="utf-8"))
    if isinstance(issues, dict):
        issues = issues.get("issues", [])
    known = {int(issue["number"]) for issue in issues if issue.get("number") is not None}

for number, item, atoms in rows:
    if not atoms:
        problems.append(f"row {number} ({item}) names no Atom")
    for atom in atoms:
        if atom < 1:
            problems.append(f"row {number} names a non-issue id #{atom}")
        elif known is not None and atom not in known:
            problems.append(f"row {number} names #{atom}, which is not an issue")

if problems:
    for problem in problems:
        print(f"FAIL: {problem}")
    raise SystemExit(1)

covered = sorted({atom for _, _, atoms in rows for atom in atoms})
print(f"OK: {len(rows)} acceptance rows naming {len(covered)} distinct Atoms.")
PY
