#!/usr/bin/env bash
# formal/conformance/index.json matches the vectors on disk, and the suite is non-empty (G-212).
#
# Every vector is validated against the ConformanceVector schema fenced in
# docs/formal/conformance-vectors.md section 2 — the schema is extracted from the specification
# rather than restated here, so the document and the checker cannot drift.
#
# The subject does not exist yet: #7 owns formal/conformance/. An absent subject exits 0 AFTER
# asserting the format document still states the layout and names this checker, so the checker is
# never vacuous. A subject that exists and is malformed always fails, including an empty suite:
# docs/formal/conformance-vectors.md section 5 says an empty conformance suite MUST NOT report
# success.
#
# Usage: check-conformance-index.sh [ROOT]
set -euo pipefail
root="${1:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$root"

spec=docs/formal/conformance-vectors.md
suite=formal/conformance

if [ ! -f "$spec" ]; then
  echo "FAIL: $spec is missing; the vector format has no definition"
  exit 1
fi

if [ ! -d "$suite" ]; then
  fail=0
  if ! grep -qF 'scripts/check-conformance-index.sh' "$spec"; then
    echo "FAIL: $spec no longer names scripts/check-conformance-index.sh as its enforcing checker"
    fail=1
  fi
  if ! grep -qF 'index.json' "$spec"; then
    echo "FAIL: $spec no longer specifies the index.json layout this checker enforces"
    fail=1
  fi
  if [ "$fail" -ne 0 ]; then
    exit 1
  fi
  echo "OK: $suite/ does not exist yet (#7 owns it); the vector format and index rule are stated and unchanged."
  exit 0
fi

python3 - "$spec" "$suite" <<'PY'
"""index.json is exactly the vectors on disk, the suite is non-empty, every vector validates."""

import json
import os
import re
import sys

spec_path, suite = sys.argv[1:3]

with open(spec_path, encoding="utf-8") as handle:
    spec_lines = handle.read().splitlines()

# The vector schema is the first fenced json block after the "## 2. Vector schema" heading.
schema_text = []
in_section = False
in_fence = False
for line in spec_lines:
    if re.match(r"^## +2\. +Vector schema\s*$", line):
        in_section = True
        continue
    if not in_section:
        continue
    if re.match(r"^## +", line) and not in_fence:
        break
    if line.startswith("```"):
        if in_fence:
            break
        in_fence = line.strip() == "```json"
        continue
    if in_fence:
        schema_text.append(line)

problems = []
schema = None
if not schema_text:
    problems.append(f"{spec_path}: no fenced json schema under '## 2. Vector schema'")
else:
    try:
        schema = json.loads("\n".join(schema_text))
    except ValueError as error:
        problems.append(f"{spec_path}: the fenced vector schema is not valid JSON: {error}")


def validate(value, spec, path):
    """The subset of JSON Schema the vector schema uses."""
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
        allowed = tuple(kinds[name] for name in names if name != "null")
        if value is None:
            if "null" not in names:
                problems.append(f"{path}: expected {expected}, found null")
                return
        else:
            if "integer" in names and isinstance(value, bool):
                problems.append(f"{path}: expected integer, found boolean")
                return
            if allowed and not isinstance(value, allowed):
                problems.append(f"{path}: expected {expected}, found {type(value).__name__}")
                return
    if isinstance(value, str) and "pattern" in spec:
        if not re.search(spec["pattern"], value):
            problems.append(f"{path}: {value!r} does not match {spec['pattern']}")
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
        item_spec = spec.get("items")
        if item_spec:
            for position, item in enumerate(value):
                validate(item, item_spec, f"{path}[{position}]")


index_path = os.path.join(suite, "index.json")
index = None
if not os.path.isfile(index_path):
    problems.append(f"{index_path}: absent; the index is generated and checked in")
else:
    try:
        with open(index_path, encoding="utf-8") as handle:
            index = json.load(handle)
    except (OSError, ValueError) as error:
        problems.append(f"{index_path}: unreadable or malformed JSON: {error}")

on_disk = {}
for entry in sorted(os.listdir(suite)):
    directory = os.path.join(suite, entry)
    if not os.path.isdir(directory):
        continue
    for name in sorted(os.listdir(directory)):
        if not name.endswith(".json"):
            problems.append(f"{os.path.join(directory, name)}: not a .json vector")
            continue
        on_disk.setdefault(entry, []).append(f"{entry}/{name[:-len('.json')]}")

vector_count = sum(len(ids) for ids in on_disk.values())
if vector_count == 0:
    problems.append(
        f"{suite}: the conformance suite is empty; an empty suite MUST NOT report success"
    )

if isinstance(index, dict):
    indexed = {}
    for predicate, ids in index.items():
        if not isinstance(ids, list):
            problems.append(f"{index_path}: {predicate!r} must map to an array of vector_ids")
            continue
        if ids != sorted(ids):
            problems.append(f"{index_path}: {predicate!r} vector_ids are not sorted")
        indexed[predicate.lower()] = set(ids)
    for predicate, ids in sorted(on_disk.items()):
        listed = indexed.get(predicate, set())
        for missing in sorted(set(ids) - listed):
            problems.append(f"{index_path}: vector {missing} is on disk and absent from the index")
        for extra in sorted(listed - set(ids)):
            problems.append(f"{index_path}: vector {extra} is indexed and absent from disk")
    for predicate in sorted(set(indexed) - set(on_disk)):
        problems.append(f"{index_path}: predicate {predicate!r} is indexed with no directory on disk")
elif index is not None:
    problems.append(f"{index_path}: top level must be an object mapping predicate -> [vector_id]")

for predicate, ids in sorted(on_disk.items()):
    for vector_id in ids:
        path = os.path.join(suite, f"{vector_id}.json")
        try:
            with open(path, encoding="utf-8") as handle:
                vector = json.load(handle)
        except (OSError, ValueError) as error:
            problems.append(f"{path}: unreadable or malformed JSON: {error}")
            continue
        if schema is not None:
            validate(vector, schema, path)
        if vector.get("vector_id") != vector_id:
            problems.append(
                f"{path}: vector_id {vector.get('vector_id')!r} does not match its path {vector_id!r}"
            )
        declared = str(vector.get("predicate", "")).lower()
        if declared and declared != predicate:
            problems.append(
                f"{path}: predicate {vector.get('predicate')!r} does not match directory {predicate!r}"
            )

if problems:
    for problem in problems:
        print(f"FAIL: {problem}")
    raise SystemExit(1)

print(f"OK: {vector_count} conformance vector(s); index.json matches the files on disk.")
PY
