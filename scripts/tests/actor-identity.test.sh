#!/usr/bin/env bash
# check-actor-identity.sh: no readable commit range is "not yet"; an agent commit without a
# well-formed Gordian-Actor trailer, or one whose trailer disagrees with its author, is "broken".
set -euo pipefail
# shellcheck source=scripts/tests/harness.sh
# shellcheck disable=SC1091
. "$(dirname "$0")/harness.sh"

checker="$REPO_ROOT/scripts/check-actor-identity.sh"
support=(docs/implementation/agent-runbook.md)

# The subject file format is exactly `git log --format=%an%x1f%B%x00 <range>`.
commits() {
  target="$1"
  shift
  : > "$target"
  while [ "$#" -gt 0 ]; do
    printf '%s\037%s\000' "$1" "$2" >> "$target"
    shift 2
  done
}

absent="$(new_fixture "${support[@]}")"
expect_ok "no commit range exits 0" bash "$checker" "$absent"

silent="$(new_fixture "${support[@]}")"
: > "$silent/docs/implementation/agent-runbook.md"
expect_fail "no commit range and a gutted rule fails rather than skipping" bash "$checker" "$silent"

good="$(new_fixture "${support[@]}")"
commits "$good/commits" \
  "gordian-agent/claude-code/run-7" "Atom: #42

Gordian-Actor: gordian-agent/claude-code/run-7" \
  "A Human" "readme typo"
expect_ok "an agent commit with a matching trailer, beside a human commit, passes" \
  env GORDIAN_COMMIT_MESSAGES="$good/commits" bash "$checker" "$good"

missing="$(new_fixture "${support[@]}")"
commits "$missing/commits" "gordian-agent/claude-code/run-7" "Atom: #42"
expect_fail "an agent-authored commit with no trailer fails" \
  env GORDIAN_COMMIT_MESSAGES="$missing/commits" bash "$checker" "$missing"

malformed="$(new_fixture "${support[@]}")"
commits "$malformed/commits" \
  "gordian-agent/claude-code/run-7" "Atom: #42

Gordian-Actor: claude"
expect_fail "a trailer that does not match the regex fails" \
  env GORDIAN_COMMIT_MESSAGES="$malformed/commits" bash "$checker" "$malformed"

mismatched="$(new_fixture "${support[@]}")"
commits "$mismatched/commits" \
  "gordian-agent/claude-code/run-7" "Atom: #42

Gordian-Actor: gordian-agent/codex/run-9"
expect_fail "a trailer that disagrees with the author fails" \
  env GORDIAN_COMMIT_MESSAGES="$mismatched/commits" bash "$checker" "$mismatched"

human="$(new_fixture "${support[@]}")"
commits "$human/commits" \
  "A Human" "hand edit

Gordian-Actor: gordian-agent/claude-code/run-7"
expect_fail "a human-authored commit claiming an agent actor fails" \
  env GORDIAN_COMMIT_MESSAGES="$human/commits" bash "$checker" "$human"

record_fixture="$(new_fixture "${support[@]}")"
mkdir -p "$record_fixture/artifacts/atoms/42"
cat > "$record_fixture/artifacts/atoms/42/closure.json" <<'JSON'
{
  "record_format": "gordian-closure-v1",
  "atom_id": "42",
  "actor": { "id": "not an actor string", "kind": "agent" },
  "verifiers": [],
  "closed_at": "2026-08-30T00:00:00Z"
}
JSON
expect_fail "a closure record whose actor is not an actor string fails" \
  bash "$checker" "$record_fixture"

disagree="$(new_fixture "${support[@]}")"
mkdir -p "$disagree/artifacts/atoms/42"
cat > "$disagree/artifacts/atoms/42/closure.json" <<'JSON'
{
  "record_format": "gordian-closure-v1",
  "atom_id": "42",
  "actor": { "id": "gordian-agent/claude-code/run-7", "kind": "agent" },
  "recorded_by": { "id": "gordian-agent/coordinator/run-1", "kind": "coordinator" },
  "verifiers": [],
  "closed_at": "2026-08-30T00:00:00Z"
}
JSON
commits "$disagree/commits" \
  "gordian-agent/claude-code/run-7" "Atom: #42

Gordian-Actor: gordian-agent/claude-code/run-7"
expect_fail "a record recorded_by an actor absent from the change under review fails" \
  env GORDIAN_COMMIT_MESSAGES="$disagree/commits" bash "$checker" "$disagree"

agree="$(new_fixture "${support[@]}")"
mkdir -p "$agree/artifacts/atoms/42"
cp "$disagree/artifacts/atoms/42/closure.json" "$agree/artifacts/atoms/42/closure.json"
commits "$agree/commits" \
  "gordian-agent/claude-code/run-7" "Atom: #42

Gordian-Actor: gordian-agent/claude-code/run-7" \
  "gordian-agent/coordinator/run-1" "Closure record for #42

Gordian-Actor: gordian-agent/coordinator/run-1"
expect_ok "a record whose coordinator authored a commit in the change passes" \
  env GORDIAN_COMMIT_MESSAGES="$agree/commits" bash "$checker" "$agree"
