#!/usr/bin/env bash
# check-closure-records.sh validates closure records AND attempt records with the same validator,
# which is what agent-runbook.md section 7 promises. No records is "not yet"; a record that
# violates the schema or the cross-field rules is "broken".
set -euo pipefail
# shellcheck source=scripts/tests/harness.sh
# shellcheck disable=SC1091
. "$(dirname "$0")/harness.sh"

checker="$REPO_ROOT/scripts/check-closure-records.sh"
schemas=(artifacts/schema/closure-record.schema.json artifacts/schema/attempt-record.schema.json)

attempt() {
  cat > "$1" <<JSON
{
  "record_format": "gordian-attempt-v1",
  "attempt_id": "20260830T142530Z-claude-code",
  "atom_id": "42",
  "actor": { "id": "gordian-agent/claude-code/run-7", "kind": "agent" },
  "outcome": "${2:-verifier_failed}",
  "reason": "cargo test failed twice on the same exact state",
  "started_at": "2026-08-30T14:25:30Z",
  "finished_at": "2026-08-30T15:00:00Z"
}
JSON
}

empty="$(new_fixture "${schemas[@]}")"
expect_ok "no records exits 0" bash "$checker" "$empty"

noschema="$(new_fixture artifacts/schema/closure-record.schema.json)"
expect_fail "a missing attempt schema fails" bash "$checker" "$noschema"

good="$(new_fixture "${schemas[@]}")"
mkdir -p "$good/artifacts/atoms/42/attempts"
attempt "$good/artifacts/atoms/42/attempts/20260830T142530Z-claude-code.json"
expect_ok "a valid attempt record passes" bash "$checker" "$good"

outcome="$(new_fixture "${schemas[@]}")"
mkdir -p "$outcome/artifacts/atoms/42/attempts"
attempt "$outcome/artifacts/atoms/42/attempts/20260830T142530Z-claude-code.json" "gave_up"
expect_fail "an outcome outside the closed set fails" bash "$checker" "$outcome"

misnamed="$(new_fixture "${schemas[@]}")"
mkdir -p "$misnamed/artifacts/atoms/42/attempts"
attempt "$misnamed/artifacts/atoms/42/attempts/first.json"
expect_fail "a file name that is not <attempt_id>.json fails" bash "$checker" "$misnamed"

wrongatom="$(new_fixture "${schemas[@]}")"
mkdir -p "$wrongatom/artifacts/atoms/43/attempts"
attempt "$wrongatom/artifacts/atoms/43/attempts/20260830T142530Z-claude-code.json"
expect_fail "an atom_id disagreeing with its directory fails" bash "$checker" "$wrongatom"

defect="$(new_fixture "${schemas[@]}")"
mkdir -p "$defect/artifacts/atoms/42/attempts"
attempt "$defect/artifacts/atoms/42/attempts/20260830T142530Z-claude-code.json" "contract_defect"
expect_fail "contract_defect with no repair issue fails" bash "$checker" "$defect"

repaired="$(new_fixture "${schemas[@]}")"
mkdir -p "$repaired/artifacts/atoms/42/attempts"
attempt "$repaired/artifacts/atoms/42/attempts/20260830T142530Z-claude-code.json" "contract_defect"
python3 - "$repaired/artifacts/atoms/42/attempts/20260830T142530Z-claude-code.json" <<'PY'
import json, sys
path = sys.argv[1]
record = json.load(open(path))
record["contract_defect_issue"] = 99
json.dump(record, open(path, "w"), indent=2)
PY
expect_ok "contract_defect naming its repair issue passes" bash "$checker" "$repaired"

stamp="$(new_fixture "${schemas[@]}")"
mkdir -p "$stamp/artifacts/atoms/42/attempts"
attempt "$stamp/artifacts/atoms/42/attempts/20260830T142530Z-claude-code.json"
python3 - "$stamp/artifacts/atoms/42/attempts/20260830T142530Z-claude-code.json" <<'PY'
import json, sys
path = sys.argv[1]
record = json.load(open(path))
record["started_at"] = "2026-01-01T00:00:00Z"
json.dump(record, open(path, "w"), indent=2)
PY
expect_fail "an attempt_id that disagrees with started_at fails" bash "$checker" "$stamp"

closure="$(new_fixture "${schemas[@]}")"
mkdir -p "$closure/artifacts/atoms/42"
cat > "$closure/artifacts/atoms/42/closure.json" <<'JSON'
{
  "record_format": "gordian-closure-v1",
  "atom_id": "42",
  "spec_digest": "not-a-digest",
  "actor": { "id": "gordian-agent/claude-code/run-7", "kind": "agent" },
  "exact_state_id": "abc",
  "logical_change_id": "def",
  "verifiers": [],
  "benchmarks": [],
  "knowledge_graph_node_ids": [],
  "known_limitations": [],
  "closed_at": "2026-08-30T00:00:00Z"
}
JSON
expect_fail "a closure record with no verifier and a malformed spec_digest fails" \
  bash "$checker" "$closure"
