#!/usr/bin/env bash
# verify-local.sh: an unknown verification group must fail before any tool-dependent work.
set -euo pipefail
# shellcheck source=scripts/tests/harness.sh
# shellcheck disable=SC1091
. "$(dirname "$0")/harness.sh"

fixture="$(new_fixture scripts/verify-local.sh)"
expect_fail "an unknown verification group is rejected" \
  bash "$fixture/scripts/verify-local.sh" definitely-not-a-group
