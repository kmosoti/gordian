#!/usr/bin/env bash
# Complete, fixed acceptance verifier for Atom #3.  It never writes workload fixtures.
set -euo pipefail

if [ "$#" -ne 0 ]; then
  echo "FAIL: check-workloads.sh accepts no caller filters" >&2
  exit 2
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

python_bin="${PYTHON:-python3.14}"
if ! command -v "$python_bin" >/dev/null 2>&1; then
  python_bin="python3"
fi
if ! command -v "$python_bin" >/dev/null 2>&1; then
  echo "FAIL: no supported Python interpreter found" >&2
  exit 1
fi

test_log="$(mktemp "${TMPDIR:-/tmp}/gordian-workloads.XXXXXX")"
trap 'rm -f "$test_log"' EXIT
set +e
PYTHONPATH="$root/orchestration/src${PYTHONPATH:+:$PYTHONPATH}" \
  "$python_bin" -m unittest -v orchestration.tests.test_workloads >"$test_log" 2>&1
test_status=$?
set -e
cat "$test_log"
if [ "$test_status" -ne 0 ]; then
  echo "FAIL: workload acceptance tests exited $test_status" >&2
  exit "$test_status"
fi

# Keep this threshold deliberately explicit: a checker that silently discovers an empty module
# is not an acceptance verifier.  Increase it when adding tests; do not make it caller-selectable.
ran="$(sed -nE 's/^Ran ([0-9]+) tests?.*/\1/p' "$test_log" | tail -n 1)"
if [ -z "$ran" ] || [ "$ran" -lt 34 ]; then
  echo "FAIL: expected at least 34 workload acceptance tests, found ${ran:-0}" >&2
  exit 1
fi

golden_manifest="experiments/workloads/golden/manifest.json"
if [ ! -f "$golden_manifest" ]; then
  echo "FAIL: $golden_manifest is missing; immutable workload corpus is not registered" >&2
  exit 1
fi

# The golden validator is read-only.  This command must never regenerate or update a fixture.
PYTHONPATH="$root/orchestration/src${PYTHONPATH:+:$PYTHONPATH}" \
  "$python_bin" -m gordian_orchestration.workloads validate --manifest "$golden_manifest"
