#!/usr/bin/env bash
# Evaluate the Mission stop condition and print the unsatisfied rows.
#
# The stop condition is a query in its bare form: an incomplete Mission is reported and exits 0.
# --gate is the final completion gate.  --preclose 69 is the coordinator's one permitted
# preclose check while the #69 closure record is written last.
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "$0")" && pwd -P)"
default_root="$(cd -- "$script_dir/.." && pwd -P)"
mode="report"
root_arg="$default_root"
root_was_set=0

usage() {
  echo "usage: $0 [--gate | --preclose 69] [ROOT]" >&2
  exit 2
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --gate)
      [ "$mode" = "report" ] || usage
      mode="gate"
      shift
      ;;
    --preclose)
      [ "$mode" = "report" ] || usage
      [ "$#" -ge 2 ] || usage
      [ "$2" = "69" ] || usage
      mode="preclose"
      shift 2
      ;;
    --)
      shift
      [ "$#" -gt 0 ] || usage
      [ "$root_was_set" -eq 0 ] || usage
      root_arg="$1"
      root_was_set=1
      shift
      [ "$#" -eq 0 ] || usage
      ;;
    -*|"")
      usage
      ;;
    *)
      [ "$root_was_set" -eq 0 ] || usage
      root_arg="$1"
      root_was_set=1
      shift
      ;;
  esac
done

if ! root="$(cd -- "$root_arg" 2>/dev/null && pwd -P)"; then
  echo "FAIL: repository root is not a readable directory: $root_arg" >&2
  exit 1
fi

plan="$root/docs/implementation/project-plan.md"
runbook="$root/docs/implementation/agent-runbook.md"
schema="$root/artifacts/schema/closure-record.schema.json"
snapshot="$root/artifacts/atoms/issues.json"

if [ ! -f "$plan" ]; then
  echo "FAIL: $plan is missing; the Mission acceptance contract cannot be evaluated." >&2
  exit 1
fi
if ! grep -q '^## Mission acceptance$' "$plan"; then
  echo "FAIL: $plan has no '## Mission acceptance' table." >&2
  exit 1
fi

if [ ! -f "$runbook" ]; then
  echo "FAIL: $runbook is missing; the stop condition has no runbook definition." >&2
  exit 1
fi
sentence='The Mission loop terminates when artifacts/atoms/69/closure.json validates against the closure schema and every row of the Mission acceptance table resolves to a validating closure record.'
runbook_sentence="$(tr '\n' ' ' < "$runbook" | sed -E 's/[[:space:]]+/ /g')"
if ! grep -qF "$sentence" <<<"$runbook_sentence"; then
  echo "FAIL: $runbook does not state the stop condition verbatim" >&2
  exit 1
fi

if [ ! -f "$schema" ]; then
  echo "FAIL: $schema is missing; closure records have no normative definition" >&2
  exit 1
fi

# Keep this checker and readiness/closure CI on the same cross-field and artifact validator.
validator_root="$(cd -- "$script_dir/../orchestration/src" && pwd -P)"
PYTHONPATH="$validator_root${PYTHONPATH:+:$PYTHONPATH}" \
  python3 - "$root" "$mode" "$plan" "$schema" "$snapshot" <<'PY'
from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any

from gordian_orchestration.closure_validation import (
    closure_problems,
    load_json,
    local_bytes_reader,
    repository_source_resolver,
)


root = Path(sys.argv[1]).resolve()
mode = sys.argv[2]
plan_path = Path(sys.argv[3])
schema_path = Path(sys.argv[4])
snapshot_path = Path(sys.argv[5])


def fail(message: str) -> None:
    print(f"FAIL: {message}")


def read_text(path: Path) -> str | None:
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        fail(f"{path}: unreadable text: {error}")
        return None


plan_text = read_text(plan_path)
if plan_text is None:
    raise SystemExit(1)


def acceptance_section(text: str) -> tuple[str | None, list[str]]:
    headings = [
        position
        for position, line in enumerate(text.splitlines())
        if line == "## Mission acceptance"
    ]
    if not headings:
        return None, []
    problems: list[str] = []
    if len(headings) != 1:
        problems.append("project-plan.md contains more than one '## Mission acceptance' heading")
    lines = text.splitlines()
    start = headings[0] + 1
    end = len(lines)
    for position in range(start, len(lines)):
        if lines[position].startswith("## "):
            end = position
            break
    return "\n".join(lines[start:end]), problems


section, table_problems = acceptance_section(plan_text)
if section is None:
    # The shell preflight handles this path, but retain a safe result if the document changes
    # between the shell check and this process.
    print(
        f"FAIL: {plan_path} no longer contains the '## Mission acceptance' table "
        "after shell preflight."
    )
    raise SystemExit(1)


rows: list[tuple[int, str, list[int]]] = []
atom_list_pattern = re.compile(r"#[1-9][0-9]*(?:, #[1-9][0-9]*)*")
for line_number, line in enumerate(section.splitlines(), start=1):
    if not line.startswith("|"):
        continue
    cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
    if cells and cells[0] in {"#", "---"}:
        continue
    if len(cells) != 3:
        table_problems.append(
            f"Mission acceptance line {line_number} has {len(cells)} cells; expected 3"
        )
        continue
    if not cells[0].isdigit():
        table_problems.append(
            f"Mission acceptance line {line_number} has a non-numeric row number {cells[0]!r}"
        )
        continue
    if cells[0] != str(int(cells[0])):
        table_problems.append(
            f"Mission acceptance line {line_number} uses non-canonical row number {cells[0]!r}"
        )
    atom_cell = cells[2]
    atom_tokens = re.findall(r"#(\d+)", atom_cell)
    atoms = [int(token) for token in atom_tokens]
    if atom_list_pattern.fullmatch(atom_cell) is None:
        for token in atom_tokens:
            if token != str(int(token)):
                table_problems.append(
                    f"Mission acceptance line {line_number} uses non-canonical Atom id #{token}"
                )
        table_problems.append(
            f"Mission acceptance line {line_number} has invalid Atom reference list "
            f"{atom_cell!r}; expected #N or #N, #N"
        )
    if len(atoms) != len(set(atoms)):
        table_problems.append(
            f"Mission acceptance line {line_number} repeats an Atom reference"
        )
    rows.append((int(cells[0]), cells[1], atoms))

if not rows:
    table_problems.append("the Mission acceptance table has no rows")

row_numbers = [number for number, _, _ in rows]
if len(row_numbers) != len(set(row_numbers)):
    table_problems.append("Mission acceptance row numbers must be unique")
expected_row_numbers = list(range(1, len(rows) + 1))
if row_numbers != expected_row_numbers:
    table_problems.append(
        "Mission acceptance row numbers must be exactly sequential 1..N "
        f"(found {row_numbers!r})"
    )

for number, item, atoms in rows:
    if not atoms:
        table_problems.append(f"row {number} ({item}) names no Atom")
    for atom in atoms:
        if atom < 1:
            table_problems.append(f"row {number} names a non-issue id #{atom}")


# The optional GitHub snapshot is deliberately not an authority for the stop condition.  If
# present, it may be inspected by the registry checker; this gate only evaluates the normative
# plan and committed closure records, so a stale or absent snapshot cannot make an otherwise valid
# Mission fail (or make an invalid one pass).


# A waiver is metadata about one human metric. It is intentionally never consulted when deciding
# whether an Atom or an acceptance row has a closure record.
waiver_lines: list[str] = []
table_atoms = {atom for _, _, atoms in rows for atom in atoms if atom > 0}
waiver_pattern = re.compile(r"^unresolved_human_metric: (#([0-9]+)) — ([^—]+) — ([^—]+)$")
plan_lines = plan_text.splitlines()
waiver_metrics: dict[str, int] = {}
for position, raw_line in enumerate(plan_lines):
    stripped = raw_line.strip()
    if not stripped.startswith("unresolved_human_metric:"):
        continue
    line_label = f"{plan_path}:{position + 1}"
    if raw_line != stripped:
        table_problems.append(f"{line_label}: waiver must occupy exactly one unindented line")
        continue
    match = waiver_pattern.fullmatch(raw_line)
    if match is None:
        table_problems.append(
            f"{line_label}: waiver must match exactly "
            "unresolved_human_metric: #N — metric — reason"
        )
        continue
    atom_token = match.group(2)
    atom = int(atom_token)
    metric = match.group(3)
    reason = match.group(4)
    if atom_token != str(atom):
        table_problems.append(f"{line_label}: waiver uses non-canonical Atom id #{atom_token}")
        continue
    if not metric.strip() or metric != metric.strip():
        table_problems.append(f"{line_label}: waiver metric must be nonempty")
        continue
    if not reason.strip() or reason != reason.strip():
        table_problems.append(f"{line_label}: waiver reason must be nonempty")
        continue
    if metric in waiver_metrics:
        table_problems.append(
            f"{line_label}: waiver metric {metric!r} is already used by "
            f"#{waiver_metrics[metric]}"
        )
        continue
    waiver_metrics[metric] = atom
    waiver_lines.append(raw_line)
    if atom not in table_atoms:
        table_problems.append(f"{line_label}: waiver names #{atom}, which is not in the table")
    if position + 1 < len(plan_lines) and plan_lines[position + 1].strip():
        next_line = plan_lines[position + 1]
        if next_line.startswith((" ", "\t")):
            table_problems.append(f"{line_label}: waiver must occupy exactly one line")


for problem in table_problems:
    fail(problem)
if table_problems:
    raise SystemExit(1)


try:
    schema: Any = load_json(schema_path)
except (OSError, ValueError, UnicodeError) as error:
    fail(f"{schema_path}: unreadable or malformed JSON: {error}")
    raise SystemExit(1)
if not isinstance(schema, dict):
    fail(f"{schema_path}: schema root must be a JSON object")
    raise SystemExit(1)


reader = local_bytes_reader(root)
source_resolver = repository_source_resolver(root)
if source_resolver is None:
    fail("cannot resolve accepted trunk() source for closure binding")
    raise SystemExit(1)
record_paths = sorted((root / "artifacts/atoms").glob("*/closure.json"))
records: dict[int, Any] = {}
structural_problems: list[str] = []
for record_path in record_paths:
    label = str(record_path)
    try:
        payload = load_json(record_path)
    except (OSError, ValueError, UnicodeError) as error:
        problems = [f"{label}: unreadable or malformed JSON: {error}"]
        structural_problems.extend(problems)
        continue
    expected_atom = record_path.parent.name
    if re.fullmatch(r"[1-9][0-9]*", expected_atom) is None:
        structural_problems.append(
            f"{label}: Atom directory must use a canonical positive decimal id"
        )
        continue
    relative_path = record_path.relative_to(root).as_posix()
    problems = closure_problems(
        payload,
        schema,
        label=label,
        expected_atom=expected_atom,
        record_path=relative_path,
        read_artifact=reader,
        resolve_source=source_resolver,
        source_binding_required=True,
    )
    if problems:
        structural_problems.extend(problems)
    else:
        try:
            record_atom = int(expected_atom)
        except ValueError:
            record_atom = None
        if record_atom is not None:
            records[record_atom] = payload

for problem in structural_problems:
    fail(problem)
if structural_problems:
    # A present but malformed record is a checker/contract defect in every mode, including the
    # reporting mode. Treating it as merely absent would hide an invalid evidence bundle.
    raise SystemExit(1)


def missing_rows() -> list[tuple[int, str, list[str]]]:
    unsatisfied: list[tuple[int, str, list[str]]] = []
    for number, item, atoms in rows:
        reasons: list[str] = []
        for atom in atoms:
            if atom not in records:
                reasons.append(f"#{atom}: no closure record at artifacts/atoms/{atom}/closure.json")
        if reasons:
            unsatisfied.append((number, item, reasons))
    return unsatisfied


unsatisfied = missing_rows()
mission_record_path = root / "artifacts/atoms/69/closure.json"
mission_record = records.get(69)
mission_missing = mission_record is None

if mission_missing and mode != "preclose":
    print(
        "UNSATISFIED  mission record artifacts/atoms/69/closure.json: "
        "no closure record"
    )
elif mission_missing:
    print(
        "PRE-CLOSE    #69 closure record is absent (permitted only for --preclose 69; "
        "waiver propagation will be checked when it is written)"
    )
elif waiver_lines:
    limitations = mission_record.get("known_limitations")
    # The shared schema has already established that this is an array for a valid record. Keep
    # this defensive branch explicit so a future schema cannot silently weaken propagation.
    if not isinstance(limitations, list):
        fail(
            f"{mission_record_path}: known_limitations must contain every waiver line verbatim"
        )
        raise SystemExit(1)
    for waiver in waiver_lines:
        if waiver not in limitations:
            fail(
                f"{mission_record_path}: known_limitations is missing waiver line verbatim: "
                f"{waiver}"
            )
            structural_problems.append(waiver)
if structural_problems:
    raise SystemExit(1)

for waiver in waiver_lines:
    print(f"WAIVER       {waiver}")

for number, item, reasons in unsatisfied:
    print(f"UNSATISFIED  row {number}: {item}")
    for reason in reasons:
        print(f"             {reason}")

if unsatisfied:
    print(
        f"Mission incomplete: {len(unsatisfied)} of {len(rows)} acceptance rows unsatisfied."
    )
    raise SystemExit(1 if mode != "report" else 0)

if mission_missing:
    print(
        f"Mission incomplete: all {len(rows)} acceptance rows resolve, but #69 has no closure "
        "record."
    )
    raise SystemExit(1 if mode == "gate" else 0)

note = f" ({len(waiver_lines)} human metric waiver(s) recorded)" if waiver_lines else ""
print(f"OK: the Mission stop condition holds; all {len(rows)} rows resolve.{note}")
raise SystemExit(0)
PY
