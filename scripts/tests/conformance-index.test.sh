#!/usr/bin/env bash
# check-conformance-index.sh: formal/conformance/ absent is "not yet". Present-and-empty is
# "broken" — an empty conformance suite MUST NOT report success.
set -euo pipefail
# shellcheck source=scripts/tests/harness.sh
# shellcheck disable=SC1091
. "$(dirname "$0")/harness.sh"

checker="$REPO_ROOT/scripts/check-conformance-index.sh"
spec=docs/formal/conformance-vectors.md

vector() {
  cat > "$1" <<'JSON'
{
  "vector_format": "gordian-conformance-v1",
  "vector_id": "dispatchable/000001",
  "predicate": "Dispatchable",
  "input": { "valid_spec": true },
  "evaluation_point": 1,
  "expected": { "result": true, "reason": null },
  "seed": 1,
  "lean_toolchain": "leanprover/lean4:v4.33.1",
  "rust_toolchain": "1.98.0",
  "source_commit": "0000000000000000000000000000000000000000",
  "canonicalization_scheme": "gordian-canon-v1"
}
JSON
}

absent="$(new_fixture "$spec")"
expect_ok "absent suite exits 0" bash "$checker" "$absent"

silent="$(new_fixture "$spec")"
: > "$silent/$spec"
expect_fail "absent suite with the format document gutted fails rather than skipping" \
  bash "$checker" "$silent"

empty="$(new_fixture "$spec")"
mkdir -p "$empty/formal/conformance"
printf '{}' > "$empty/formal/conformance/index.json"
expect_fail "a present but empty suite fails" bash "$checker" "$empty"

good="$(new_fixture "$spec")"
mkdir -p "$good/formal/conformance/dispatchable"
vector "$good/formal/conformance/dispatchable/000001.json"
printf '{"dispatchable": ["dispatchable/000001"]}' > "$good/formal/conformance/index.json"
expect_ok "index matching the files on disk passes" bash "$checker" "$good"

unindexed="$(new_fixture "$spec")"
mkdir -p "$unindexed/formal/conformance/dispatchable"
vector "$unindexed/formal/conformance/dispatchable/000001.json"
printf '{"dispatchable": []}' > "$unindexed/formal/conformance/index.json"
expect_fail "a vector on disk and absent from the index fails" bash "$checker" "$unindexed"

phantom="$(new_fixture "$spec")"
mkdir -p "$phantom/formal/conformance/dispatchable"
vector "$phantom/formal/conformance/dispatchable/000001.json"
printf '{"dispatchable": ["dispatchable/000001", "dispatchable/000002"]}' \
  > "$phantom/formal/conformance/index.json"
expect_fail "a vector indexed and absent from disk fails" bash "$checker" "$phantom"

invalid="$(new_fixture "$spec")"
mkdir -p "$invalid/formal/conformance/dispatchable"
vector "$invalid/formal/conformance/dispatchable/000001.json"
python3 - "$invalid/formal/conformance/dispatchable/000001.json" <<'PY'
import json, sys
path = sys.argv[1]
v = json.load(open(path))
v["predicate"] = "Enabled"
json.dump(v, open(path, "w"))
PY
printf '{"dispatchable": ["dispatchable/000001"]}' > "$invalid/formal/conformance/index.json"
expect_fail "a vector violating the fenced schema fails" bash "$checker" "$invalid"
