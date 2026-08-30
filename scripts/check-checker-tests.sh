#!/usr/bin/env bash
# Runs the tests of the checkers themselves, in scripts/tests/.
#
# Several checkers guard a subject that does not exist yet. Each MUST exit 0 on an empty subject
# and MUST fail on a subject that exists and is malformed — the difference between "not yet" and
# "broken". A checker that confuses the two is worse than no checker, because it reports success
# forever. These tests are where that distinction is written down, and this script is what makes
# them run: it matches scripts/check-*.sh, so the verify workflow's loop picks it up with no
# workflow edit.
#
# An empty scripts/tests/ is a failure, not a skip.
#
# Usage: check-checker-tests.sh [ROOT]
set -euo pipefail
root="${1:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$root"

tests=()
while IFS= read -r test; do
  tests+=("$test")
done < <(find scripts/tests -name '*.test.sh' 2>/dev/null | sort)

if [ "${#tests[@]}" -eq 0 ]; then
  echo "FAIL: scripts/tests/ holds no *.test.sh; the empty-subject vs malformed-subject"
  echo "      distinction the checkers depend on is unproven"
  exit 1
fi

REPO_ROOT="$PWD"
export REPO_ROOT

failed=0
for test in "${tests[@]}"; do
  echo "-- $test"
  if ! bash "$test"; then
    echo "   FAILED: $test"
    failed=1
  fi
done

if [ "$failed" -ne 0 ]; then
  echo "FAIL: at least one checker test failed"
  exit 1
fi

echo "OK: ${#tests[@]} checker test file(s) passed."
