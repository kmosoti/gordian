#!/usr/bin/env bash
# Every artifacts/atoms/*/closure.json validates against artifacts/schema/closure-record.schema.json
# and every artifact_sha256 matches the file at its artifact_path.
#
# Every artifacts/atoms/*/attempts/*.json validates against artifacts/schema/attempt-record.schema.json
# with the SAME validator, which is what docs/implementation/agent-runbook.md section 7 promises
# when it says the attempt record "is validated in CI by the same validator as the closure record"
# (G-520). An attempt record is not optional bookkeeping: it is the only durable trace that a
# failed attempt happened, and section 7's abandon procedure writes it before releasing the claim.
#
# closure.json is excluded from the artifact digest check: a file cannot record its own hash. The
# record is written by the coordinator after admission, in a bookkeeping change outside the
# verified candidate, so the artifacts it names already exist when it is written.
#
# An empty subject (no records yet) exits 0 after parsing both schemas; a malformed one never does.
#
# Usage: check-closure-records.sh [ROOT]
set -euo pipefail
root="${1:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$root"

closure_schema=artifacts/schema/closure-record.schema.json
attempt_schema=artifacts/schema/attempt-record.schema.json

for required in "$closure_schema" "$attempt_schema"; do
  if [ ! -f "$required" ]; then
    echo "FAIL: $required is missing; the record has no normative definition"
    exit 1
  fi
done

records=()
while IFS= read -r record; do
  records+=("$record")
done < <(find artifacts/atoms -mindepth 2 -maxdepth 2 -name 'closure.json' 2>/dev/null | sort)

attempts=()
while IFS= read -r attempt; do
  attempts+=("$attempt")
done < <(find artifacts/atoms -mindepth 3 -maxdepth 3 -path '*/attempts/*' -name '*.json' 2>/dev/null | sort)

if [ "${#records[@]}" -eq 0 ] && [ "${#attempts[@]}" -eq 0 ]; then
  echo "SKIP: no closure or attempt records yet (both schemas present and parsed)."
  python3 -c 'import json,sys; [json.load(open(p)) for p in sys.argv[1:]]' \
    "$closure_schema" "$attempt_schema"
  exit 0
fi

python3 - "$closure_schema" "$attempt_schema" "${#records[@]}" \
  "${records[@]+"${records[@]}"}" "${attempts[@]+"${attempts[@]}"}" <<'PY'
"""Validate closure and attempt records against the subset of JSON Schema the schemas use."""

import hashlib
import json
import os
import re
import sys

closure_schema_path, attempt_schema_path, closure_count = sys.argv[1:4]
rest = sys.argv[4:]
record_paths = rest[: int(closure_count)]
attempt_paths = rest[int(closure_count):]

with open(closure_schema_path, encoding="utf-8") as handle:
    closure_schema = json.load(handle)
with open(attempt_schema_path, encoding="utf-8") as handle:
    attempt_schema = json.load(handle)

problems = []

DATE_TIME = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$")


def validate(value, spec, path):
    if "const" in spec and value != spec["const"]:
        problems.append(f"{path}: expected const {spec['const']!r}, found {value!r}")
        return
    if "enum" in spec and value not in spec["enum"]:
        problems.append(f"{path}: {value!r} is not one of {spec['enum']}")
        return
    expected = spec.get("type")
    if expected:
        kinds = {
            "object": dict,
            "array": list,
            "string": str,
            "integer": int,
            "number": (int, float),
            "boolean": bool,
        }
        names = expected if isinstance(expected, list) else [expected]
        if value is None:
            if "null" not in names:
                problems.append(f"{path}: expected {expected}, found null")
            return
        if "integer" in names and isinstance(value, bool):
            problems.append(f"{path}: expected integer, found boolean")
            return
        allowed = tuple(kinds[name] for name in names if name != "null")
        if allowed and not isinstance(value, allowed):
            problems.append(f"{path}: expected {expected}, found {type(value).__name__}")
            return
    if isinstance(value, str):
        if "pattern" in spec and not re.search(spec["pattern"], value):
            problems.append(f"{path}: {value!r} does not match {spec['pattern']}")
        minimum_length = spec.get("minLength")
        if minimum_length is not None and len(value) < minimum_length:
            problems.append(f"{path}: shorter than minLength {minimum_length}")
        if spec.get("format") == "date-time" and not DATE_TIME.match(value):
            problems.append(f"{path}: {value!r} is not an RFC 3339 date-time")
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        minimum = spec.get("minimum")
        if minimum is not None and value < minimum:
            problems.append(f"{path}: {value} is below minimum {minimum}")
    if isinstance(value, dict):
        properties = spec.get("properties", {})
        for name in spec.get("required", []):
            if name not in value:
                problems.append(f"{path}: required property {name!r} is absent")
        if spec.get("additionalProperties") is False:
            for name in value:
                if name not in properties:
                    problems.append(f"{path}: unexpected property {name!r}")
        for name, child in value.items():
            if name in properties:
                validate(child, properties[name], f"{path}.{name}")
    if isinstance(value, list):
        minimum = spec.get("minItems")
        if minimum is not None and len(value) < minimum:
            problems.append(f"{path}: {len(value)} items, minimum {minimum}")
        item_spec = spec.get("items")
        if item_spec:
            for position, item in enumerate(value):
                validate(item, item_spec, f"{path}[{position}]")


def load(path):
    try:
        with open(path, encoding="utf-8") as handle:
            return json.load(handle)
    except (OSError, ValueError) as error:
        problems.append(f"{path}: unreadable or malformed JSON: {error}")
        return None


for record_path in record_paths:
    label = record_path
    record = load(record_path)
    if record is None:
        continue

    validate(record, closure_schema, label)

    expected_atom = os.path.basename(os.path.dirname(record_path))
    if record.get("atom_id") not in (None, expected_atom):
        problems.append(
            f"{label}: atom_id {record['atom_id']!r} does not match its directory {expected_atom!r}"
        )

    record_dir = os.path.dirname(record_path)
    for position, verifier in enumerate(record.get("verifiers", []) or []):
        if not isinstance(verifier, dict):
            continue
        artifact = verifier.get("artifact_path")
        digest = verifier.get("artifact_sha256")
        if not artifact or not digest:
            continue
        if os.path.basename(artifact) == "closure.json":
            problems.append(
                f"{label}.verifiers[{position}]: a record cannot record its own digest"
            )
            continue
        candidates = [artifact, os.path.join(record_dir, artifact)]
        resolved = next((c for c in candidates if os.path.isfile(c)), None)
        if resolved is None:
            problems.append(f"{label}.verifiers[{position}]: artifact_path {artifact} does not exist")
            continue
        with open(resolved, "rb") as handle:
            actual = hashlib.sha256(handle.read()).hexdigest()
        if actual != digest:
            problems.append(
                f"{label}.verifiers[{position}]: {artifact} hashes to {actual}, record says {digest}"
            )

for attempt_path in attempt_paths:
    label = attempt_path
    attempt = load(attempt_path)
    if attempt is None:
        continue

    validate(attempt, attempt_schema, label)

    attempts_dir = os.path.dirname(attempt_path)
    if os.path.basename(attempts_dir) != "attempts":
        problems.append(f"{label}: attempt records live in artifacts/atoms/<atom_id>/attempts/")
        continue
    expected_atom = os.path.basename(os.path.dirname(attempts_dir))
    if attempt.get("atom_id") not in (None, expected_atom):
        problems.append(
            f"{label}: atom_id {attempt['atom_id']!r} does not match its directory {expected_atom!r}"
        )

    attempt_id = attempt.get("attempt_id")
    expected_name = f"{attempt_id}.json"
    if attempt_id and os.path.basename(attempt_path) != expected_name:
        problems.append(
            f"{label}: attempt_id {attempt_id!r} requires the file name {expected_name!r}"
        )

    started = attempt.get("started_at")
    if isinstance(attempt_id, str) and isinstance(started, str) and DATE_TIME.match(started):
        stamp = re.sub(r"[-:]", "", started.split(".")[0])
        stamp = stamp.replace("+0000", "").rstrip("Z") + "Z"
        if not attempt_id.startswith(stamp):
            problems.append(
                f"{label}: attempt_id {attempt_id!r} does not open with started_at {stamp!r}"
            )

    finished = attempt.get("finished_at")
    if isinstance(started, str) and isinstance(finished, str) and finished < started:
        problems.append(f"{label}: finished_at {finished} precedes started_at {started}")

    outcome = attempt.get("outcome")
    defect_issue = attempt.get("contract_defect_issue")
    if outcome == "contract_defect" and defect_issue is None:
        problems.append(
            f"{label}: outcome contract_defect requires contract_defect_issue; the runbook's "
            "section 7 forbids editing the Atom's acceptance bullets instead"
        )
    if outcome != "contract_defect" and defect_issue is not None:
        problems.append(
            f"{label}: contract_defect_issue is set but outcome is {outcome!r}"
        )

if problems:
    for problem in problems:
        print(f"FAIL: {problem}")
    raise SystemExit(1)

print(
    f"OK: {len(record_paths)} closure record(s) valid with every artifact digest matching, and "
    f"{len(attempt_paths)} attempt record(s) valid."
)
PY
