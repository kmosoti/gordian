#!/usr/bin/env bash
# Evaluates the Mission stop condition and prints the unsatisfied rows.
#
#   "The Mission loop terminates when artifacts/atoms/69/closure.json validates against the
#    closure schema and every row of the Mission acceptance table resolves to a validating
#    closure record."   -- docs/implementation/agent-runbook.md section 3
#
# It always checks the table itself: every Atom id must be a real issue number, and -- when a
# snapshot of the issue graph is committed -- must exist. That part is a CI gate.
#
# The stop condition proper is a *query*: by default this script reports the unsatisfied rows and
# exits 0, so an incomplete Mission does not fail every build. Run it with --gate (as an agent
# asking "am I done?") to make an unsatisfied Mission exit non-zero.
set -euo pipefail
cd "$(dirname "$0")/.."

gate=0
for argument in "$@"; do
  case "$argument" in
    --gate|--require-complete) gate=1 ;;
    *) echo "usage: $0 [--gate]" >&2; exit 2 ;;
  esac
done

plan=docs/implementation/project-plan.md
runbook=docs/implementation/agent-runbook.md
snapshot=artifacts/atoms/issues.json

if [ ! -f "$plan" ] || ! grep -q '^## Mission acceptance' "$plan"; then
  echo "SKIP: $plan has no '## Mission acceptance' table yet."
  exit 0
fi

sentence='The Mission loop terminates when artifacts/atoms/69/closure.json validates against the'
if [ -f "$runbook" ] && ! grep -qF "$sentence" "$runbook"; then
  echo "FAIL: $runbook does not state the stop condition verbatim"
  exit 1
fi

python3 - "$plan" "$snapshot" "$gate" <<'PY'
import json
import os
import re
import sys

plan_path, snapshot_path, gate = sys.argv[1], sys.argv[2], sys.argv[3] == "1"
text = open(plan_path, encoding="utf-8").read()
section = text.split("## Mission acceptance", 1)[1].split("\n## ", 1)[0]

rows = []
for line in section.splitlines():
    if not line.startswith("|"):
        continue
    cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
    if len(cells) != 3 or not cells[0].isdigit():
        continue
    atoms = [int(number) for number in re.findall(r"#(\d+)", cells[2])]
    rows.append((int(cells[0]), cells[1], atoms))

waived = {}
for line in text.splitlines():
    stripped = line.strip()
    if stripped.startswith("unresolved_human_metric:"):
        body = stripped.split(":", 1)[1].strip()
        found = re.findall(r"#(\d+)", body)
        if found:
            waived[int(found[0])] = body

problems = []
if not rows:
    problems.append("the Mission acceptance table has no rows")

known = None
if os.path.isfile(snapshot_path):
    with open(snapshot_path, encoding="utf-8") as handle:
        issues = json.load(handle)
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


def closure_state(atom):
    path = os.path.join("artifacts", "atoms", str(atom), "closure.json")
    if not os.path.isfile(path):
        return "no closure record"
    try:
        with open(path, encoding="utf-8") as handle:
            record = json.load(handle)
    except ValueError as error:
        return f"malformed closure record ({error})"
    if record.get("record_format") != "gordian-closure-v1":
        return "closure record is not gordian-closure-v1"
    if not record.get("verifiers"):
        return "closure record carries no verifier evidence"
    return None


unsatisfied = []
waived_hits = []
for number, item, atoms in rows:
    reasons = []
    for atom in atoms:
        state = closure_state(atom)
        if not state:
            continue
        if atom in waived:
            waived_hits.append((number, atom))
            continue
        reasons.append(f"#{atom}: {state}")
    if reasons:
        unsatisfied.append((number, item, reasons))

for number, atom in waived_hits:
    print(f"WAIVED       row {number} #{atom}: {waived[atom]}")

mission_record = closure_state(69)
if mission_record:
    print(f"UNSATISFIED  mission record artifacts/atoms/69/closure.json: {mission_record}")

for number, item, reasons in unsatisfied:
    print(f"UNSATISFIED  row {number}: {item}")
    for reason in reasons:
        print(f"             {reason}")

complete = not unsatisfied and mission_record is None
if complete:
    note = f" ({len(waived_hits)} human-judgment metric(s) waived)" if waived_hits else ""
    print(f"OK: the Mission stop condition holds; all {len(rows)} rows resolve.{note}")
    raise SystemExit(0)

print(
    f"Mission incomplete: {len(unsatisfied)} of {len(rows)} acceptance rows unsatisfied."
)
raise SystemExit(1 if gate else 0)
PY
