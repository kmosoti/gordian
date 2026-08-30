#!/usr/bin/env bash
# Sourced by scripts/tests/*.test.sh. Not a test itself, and deliberately not named *.test.sh.
#
# A test builds a FIXTURE ROOT — a directory holding only the files the checker under test reads —
# and runs the checker against it. Every checker takes that root as its first argument, so a test
# needs no chroot, no mocking, and no copy of the checker.

REPO_ROOT="${REPO_ROOT:?source this from scripts/check-checker-tests.sh, which exports REPO_ROOT}"
TEST_NAME="$(basename "${BASH_SOURCE[1]}" .test.sh)"
FIXTURE_PARENT="$(mktemp -d "${TMPDIR:-/tmp}/gordian-${TEST_NAME}-XXXXXX")"

harness_cleanup() {
  rm -rf "$FIXTURE_PARENT"
  return 0
}
trap harness_cleanup EXIT

# new_fixture [relative-path ...] -> prints a fresh fixture root, with each named repository file
# copied into it at the same relative path.
new_fixture() {
  # mktemp, not a counter: new_fixture is called inside $( ), so a counter would increment in a
  # subshell and every fixture would silently be the same directory.
  fixture="$(mktemp -d "$FIXTURE_PARENT/fixture-XXXXXX")"
  for relative in "$@"; do
    mkdir -p "$fixture/$(dirname "$relative")"
    cp "$REPO_ROOT/$relative" "$fixture/$relative"
  done
  printf '%s\n' "$fixture"
}

expect_status() {
  expected="$1"
  label="$2"
  shift 2
  if output="$("$@" 2>&1)"; then
    actual=0
  else
    actual=$?
  fi
  if [ "$expected" = "nonzero" ]; then
    if [ "$actual" -eq 0 ]; then
      echo "   FAIL [$TEST_NAME] $label: expected a non-zero exit, got 0"
      printf '%s\n' "$output" | sed 's/^/          /'
      exit 1
    fi
  elif [ "$actual" -ne "$expected" ]; then
    echo "   FAIL [$TEST_NAME] $label: expected exit $expected, got $actual"
    printf '%s\n' "$output" | sed 's/^/          /'
    exit 1
  fi
  echo "   ok   [$TEST_NAME] $label"
}

expect_ok() { expect_status 0 "$@"; }
expect_fail() { expect_status nonzero "$@"; }
