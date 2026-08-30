#!/usr/bin/env bash
# check-spec-digests.sh: no snapshots is "not yet". A CRLF snapshot, or a digest that does not
# recompute, is "broken" — and the CRLF case is the whole point: `gh issue view` returns GitHub
# bodies with CRLF, so an un-normalized capture produces a digest the stated rule does not define.
set -euo pipefail
# shellcheck source=scripts/tests/harness.sh
# shellcheck disable=SC1091
. "$(dirname "$0")/harness.sh"

checker="$REPO_ROOT/scripts/check-spec-digests.sh"
support=(docs/implementation/issue-index.md artifacts/schema/closure-record.schema.json)

record() {
  cat > "$1" <<JSON
{
  "record_format": "gordian-closure-v1",
  "atom_id": "42",
  "spec_digest": "$2",
  "actor": { "id": "gordian-agent/claude-code/run-7", "kind": "agent" },
  "exact_state_id": "abc",
  "logical_change_id": "def",
  "verifiers": [ { "verifier_id": "v", "command": "true", "exit_code": 0,
                   "artifact_path": "log.txt",
                   "artifact_sha256": "0000000000000000000000000000000000000000000000000000000000000000" } ],
  "benchmarks": [],
  "knowledge_graph_node_ids": [],
  "known_limitations": [],
  "closed_at": "2026-08-30T00:00:00Z"
}
JSON
}

empty="$(new_fixture "${support[@]}")"
expect_ok "no snapshots and no records exits 0" bash "$checker" "$empty"

silent="$(new_fixture "${support[@]}")"
: > "$silent/docs/implementation/issue-index.md"
expect_fail "no snapshots and a gutted rule fails rather than skipping" bash "$checker" "$silent"

good="$(new_fixture "${support[@]}")"
mkdir -p "$good/artifacts/atoms/42"
printf '## Objective\nDo the thing.\n' > "$good/artifacts/atoms/42/spec.md"
digest="$(python3 -c 'import hashlib,sys;print(hashlib.sha256(open(sys.argv[1],"rb").read()).hexdigest())' "$good/artifacts/atoms/42/spec.md")"
record "$good/artifacts/atoms/42/closure.json" "$digest"
expect_ok "an LF snapshot whose digest recomputes passes" bash "$checker" "$good"

crlf="$(new_fixture "${support[@]}")"
mkdir -p "$crlf/artifacts/atoms/42"
printf '## Objective\r\nDo the thing.\r\n' > "$crlf/artifacts/atoms/42/spec.md"
expect_fail "a snapshot captured with CRLF fails" bash "$checker" "$crlf"

drift="$(new_fixture "${support[@]}")"
mkdir -p "$drift/artifacts/atoms/42"
printf '## Objective\nDo the thing.\n' > "$drift/artifacts/atoms/42/spec.md"
record "$drift/artifacts/atoms/42/closure.json" \
  "1111111111111111111111111111111111111111111111111111111111111111"
expect_fail "a spec_digest that disagrees with its snapshot fails" bash "$checker" "$drift"

orphan="$(new_fixture "${support[@]}")"
mkdir -p "$orphan/artifacts/atoms/42"
record "$orphan/artifacts/atoms/42/closure.json" \
  "1111111111111111111111111111111111111111111111111111111111111111"
expect_fail "a spec_digest with no committed snapshot fails" bash "$checker" "$orphan"
