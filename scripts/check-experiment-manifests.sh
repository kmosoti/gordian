#!/usr/bin/env bash
# Every experiments/**/protocol.json and experiments/**/runs/**/run.json validates against its
# schema in experiments/schema/; baseline_condition and analysis_plan.primary_metric resolve to
# members of conditions[] and metrics[]; every run's protocol_digest matches the protocol on disk
# (G-519).
#
# protocol_digest is the SHA-256, lowercase hex, of the LF-normalized bytes of the protocol.json
# the run executed under — the same normalization scripts/check-spec-digests.sh applies, so a
# digest computed on a CRLF checkout is not silently a different value.
#
# An absent subject exits 0 AFTER asserting the schemas are present and experiments/README.md
# still names this checker; it never skips silently. A malformed subject always fails.
#
# Usage: check-experiment-manifests.sh [ROOT]
set -euo pipefail
root="${1:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$root"

protocol_schema=experiments/schema/experiment-protocol.schema.json
run_schema=experiments/schema/experiment-run.schema.json

for required in "$protocol_schema" "$run_schema"; do
  if [ ! -f "$required" ]; then
    echo "FAIL: $required is missing; experiment manifests have no normative definition"
    exit 1
  fi
done

protocols=()
while IFS= read -r protocol; do
  protocols+=("$protocol")
done < <(find experiments -name 'protocol.json' -not -path 'experiments/schema/*' 2>/dev/null | sort)

runs=()
while IFS= read -r run; do
  runs+=("$run")
done < <(find experiments -path '*/runs/*' -name 'run.json' 2>/dev/null | sort)

if [ "${#protocols[@]}" -eq 0 ] && [ "${#runs[@]}" -eq 0 ]; then
  fail=0
  if [ ! -f experiments/README.md ]; then
    echo "FAIL: experiments/README.md is missing"
    fail=1
  elif ! grep -qF 'scripts/check-experiment-manifests.sh' experiments/README.md; then
    echo "FAIL: experiments/README.md no longer names scripts/check-experiment-manifests.sh"
    fail=1
  fi
  python3 -c 'import json,sys; [json.load(open(p)) for p in sys.argv[1:]]' \
    "$protocol_schema" "$run_schema" || fail=1
  if [ "$fail" -ne 0 ]; then
    exit 1
  fi
  echo "OK: no experiment manifests registered yet; both schemas parse and the rule is stated."
  exit 0
fi

python3 - "$protocol_schema" "$run_schema" "${#protocols[@]}" \
  "${protocols[@]+"${protocols[@]}"}" "${runs[@]+"${runs[@]}"}" <<'PY'
"""Validate experiment protocols and runs, then check the cross-field and digest rules."""

import hashlib
import json
import re
import sys

protocol_schema_path, run_schema_path, protocol_count = sys.argv[1:4]
rest = sys.argv[4:]
protocol_paths = rest[: int(protocol_count)]
run_paths = rest[int(protocol_count):]

problems = []

DATE_TIME = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$")


def load_schema(path):
    with open(path, encoding="utf-8") as handle:
        return json.load(handle)


def validate(value, spec, path):
    """The subset of JSON Schema the experiment schemas use."""
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
        minimum_items = spec.get("minItems")
        if minimum_items is not None and len(value) < minimum_items:
            problems.append(f"{path}: {len(value)} items, minimum {minimum_items}")
        item_spec = spec.get("items")
        if item_spec:
            for position, item in enumerate(value):
                validate(item, item_spec, f"{path}[{position}]")


def canonical_digest(path):
    """SHA-256 over the LF-normalized file bytes."""
    with open(path, "rb") as handle:
        return hashlib.sha256(handle.read().replace(b"\r\n", b"\n").replace(b"\r", b"\n")).hexdigest()


protocol_schema = load_schema(protocol_schema_path)
run_schema = load_schema(run_schema_path)

protocols = {}
for path in protocol_paths:
    parts = path.split("/")
    directory = parts[1] if len(parts) > 2 else ""
    try:
        with open(path, encoding="utf-8") as handle:
            protocol = json.load(handle)
    except (OSError, ValueError) as error:
        problems.append(f"{path}: unreadable or malformed JSON: {error}")
        continue

    validate(protocol, protocol_schema, path)

    experiment_id = protocol.get("experiment_id")
    if experiment_id != directory:
        problems.append(
            f"{path}: experiment_id {experiment_id!r} does not match its directory {directory!r}"
        )

    condition_ids = {
        c.get("id") for c in protocol.get("conditions", []) or [] if isinstance(c, dict)
    }
    metric_ids = {m.get("id") for m in protocol.get("metrics", []) or [] if isinstance(m, dict)}

    baseline = protocol.get("baseline_condition")
    if baseline not in condition_ids:
        problems.append(
            f"{path}: baseline_condition {baseline!r} is not the id of a member of conditions[] "
            f"{sorted(i for i in condition_ids if i)}"
        )
    plan = protocol.get("analysis_plan")
    if isinstance(plan, dict):
        primary = plan.get("primary_metric")
        if primary not in metric_ids:
            problems.append(
                f"{path}: analysis_plan.primary_metric {primary!r} is not the id of a member of "
                f"metrics[] {sorted(i for i in metric_ids if i)}"
            )

    protocols[directory] = {
        "path": path,
        "digest": canonical_digest(path),
        "conditions": condition_ids,
    }

for path in run_paths:
    parts = path.split("/")
    directory = parts[1] if len(parts) > 2 else ""
    try:
        with open(path, encoding="utf-8") as handle:
            run = json.load(handle)
    except (OSError, ValueError) as error:
        problems.append(f"{path}: unreadable or malformed JSON: {error}")
        continue

    validate(run, run_schema, path)

    if run.get("experiment_id") != directory:
        problems.append(
            f"{path}: experiment_id {run.get('experiment_id')!r} does not match its directory "
            f"{directory!r}"
        )
    owner = protocols.get(directory)
    if owner is None:
        problems.append(f"{path}: no registered protocol.json for experiment {directory!r}")
        continue
    if run.get("protocol_digest") != owner["digest"]:
        problems.append(
            f"{path}: protocol_digest {run.get('protocol_digest')!r} does not match "
            f"{owner['path']} ({owner['digest']}); this is a post-hoc run"
        )
    condition = run.get("condition")
    if owner["conditions"] and condition not in owner["conditions"]:
        problems.append(
            f"{path}: condition {condition!r} is not a condition of {owner['path']}"
        )

if problems:
    for problem in problems:
        print(f"FAIL: {problem}")
    raise SystemExit(1)

print(
    f"OK: {len(protocol_paths)} protocol(s) and {len(run_paths)} run(s) valid; "
    "baseline_condition and primary_metric resolve; every protocol_digest matches."
)
PY
