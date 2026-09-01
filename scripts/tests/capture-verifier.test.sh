#!/usr/bin/env bash
# capture-verifier.sh writes the only log shape check-closure-records.sh accepts as evidence,
# and the log can only claim the state it ran on. Each case below is a way an agent could
# otherwise produce a green-looking artifact that witnesses nothing.
# The fixtures here are bare Jujutsu repositories, not copies of repository files, so
# new_fixture is deliberately called with no paths (SC2119).
# shellcheck disable=SC1091,SC2119
set -euo pipefail
# shellcheck source=scripts/tests/harness.sh
. "$(dirname "$0")/harness.sh"

capture="$REPO_ROOT/scripts/capture-verifier.sh"
logs="$FIXTURE_PARENT/logs"

# A workspace whose @ is a described, non-empty candidate: the worker's shape (runbook 6.7).
repo="$(new_fixture)"
(cd "$repo" && jj git init --colocate >/dev/null && printf 'x\n' > file && jj describe -m candidate >/dev/null) 2>/dev/null
subject="$(cd "$repo" && jj log -r @ -n 1 --no-graph -T commit_id)"

expect_ok "a passing command writes a log and exits 0" \
  env -C "$repo" bash "$capture" --atom 42 --id check --log-root "$logs" -- 'printf canonical'
log="$logs/atom-42/check.log"
expected="$(printf 'subject_exact_state_id=%s\ncommand=printf canonical\ncanonicalexit_code=0\n' "$subject")"
if [ "$(cat "$log")" != "$expected" ]; then
  echo "   FAIL [$TEST_NAME] the log is not header, output, trailer:"; cat "$log"; exit 1
fi
echo "   ok   [$TEST_NAME] the log opens with the subject and command lines and closes with exit_code"

expect_fail "a failing command exits non-zero" \
  env -C "$repo" bash "$capture" --atom 42 --id failing --log-root "$logs" -- 'false'
grep -qx 'exit_code=1' "$logs/atom-42/failing.log" || { echo "   FAIL [$TEST_NAME] trailer"; exit 1; }
echo "   ok   [$TEST_NAME] the failing log records exit_code=1"

# `a; b` under plain bash -c reports only b. The capture must report a.
expect_fail "a failing member of a ;-chain fails the whole verifier" \
  env -C "$repo" bash "$capture" --atom 42 --id chain --log-root "$logs" -- 'false; true'

# The empty-child shape made by `jj workspace add -r candidate`: the subject is @-.
child="$(new_fixture)"
(cd "$child" && jj git init --colocate >/dev/null && printf 'x\n' > file && jj commit -m candidate >/dev/null) 2>/dev/null
parent="$(cd "$child" && jj log -r @- -n 1 --no-graph -T commit_id)"
expect_ok "an empty working-copy child binds to its parent" \
  env -C "$child" bash "$capture" --atom 7 --id check --log-root "$logs" -- 'true'
head -1 "$logs/atom-7/check.log" | grep -qx "subject_exact_state_id=$parent" \
  || { echo "   FAIL [$TEST_NAME] subject is not @-"; head -1 "$logs/atom-7/check.log"; exit 1; }
echo "   ok   [$TEST_NAME] the subject is the parent candidate, not the empty child"

# A command that edits the workspace changes the candidate; that log witnesses nothing.
mutating="$(new_fixture)"
(cd "$mutating" && jj git init --colocate >/dev/null && printf 'x\n' > file && jj describe -m candidate >/dev/null) 2>/dev/null
expect_status 70 "a command that changes the candidate is refused and its log discarded" \
  env -C "$mutating" bash "$capture" --atom 9 --id mutate --log-root "$logs" -- 'printf y >> file'
[ ! -e "$logs/atom-9/mutate.log" ] || { echo "   FAIL [$TEST_NAME] discarded log exists"; exit 1; }
echo "   ok   [$TEST_NAME] no log survives a candidate change"

expect_status 2 "a log root inside the workspace is refused" \
  env -C "$repo" bash "$capture" --atom 42 --id inside --log-root "$repo/logs" -- 'true'
expect_status 2 "a multi-line command cannot be bound" \
  env -C "$repo" bash "$capture" --atom 42 --id lines --log-root "$logs" -- "$(printf 'true\nfalse')"
expect_status 2 "an unsafe verifier id is refused" \
  env -C "$repo" bash "$capture" --atom 42 --id '../check' --log-root "$logs" -- 'true'
expect_status 2 "a non-workspace directory is refused" \
  env -C "$FIXTURE_PARENT" bash "$capture" --atom 42 --id check --log-root "$logs" -- 'true'
