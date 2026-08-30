#!/usr/bin/env bash
# check-invariant-coverage.sh: the coverage state list is extracted from the theorem catalog, so a
# section with no Coverage line, two of them, or a value the catalog does not list must all fail.
set -euo pipefail
# shellcheck source=scripts/tests/harness.sh
# shellcheck disable=SC1091
. "$(dirname "$0")/harness.sh"

checker="$REPO_ROOT/scripts/check-invariant-coverage.sh"
docs=(docs/spec/invariants.md docs/formal/theorem-catalog.md)

expect_ok "the repository as it stands passes" bash "$checker" "$REPO_ROOT"

missing="$(new_fixture "${docs[@]}")"
python3 - "$missing/docs/spec/invariants.md" <<'PY'
import sys
path = sys.argv[1]
lines = open(path, encoding="utf-8").read().splitlines()
for index, line in enumerate(lines):
    if line.startswith("**Coverage:**"):
        del lines[index]
        break
open(path, "w", encoding="utf-8").write("\n".join(lines) + "\n")
PY
expect_fail "a section with no Coverage line fails" bash "$checker" "$missing"

doubled="$(new_fixture "${docs[@]}")"
python3 - "$doubled/docs/spec/invariants.md" <<'PY'
import sys
path = sys.argv[1]
lines = open(path, encoding="utf-8").read().splitlines()
for index, line in enumerate(lines):
    if line.startswith("**Coverage:**"):
        lines.insert(index + 1, line)
        break
open(path, "w", encoding="utf-8").write("\n".join(lines) + "\n")
PY
expect_fail "a section with two Coverage lines fails" bash "$checker" "$doubled"

invented="$(new_fixture "${docs[@]}")"
python3 - "$invented/docs/spec/invariants.md" <<'PY'
import sys
path = sys.argv[1]
lines = open(path, encoding="utf-8").read().splitlines()
for index, line in enumerate(lines):
    if line.startswith("**Coverage:**"):
        lines[index] = "**Coverage:** mostly-fine"
        break
open(path, "w", encoding="utf-8").write("\n".join(lines) + "\n")
PY
expect_fail "a coverage state the catalog does not list fails" bash "$checker" "$invented"

nolist="$(new_fixture "${docs[@]}")"
: > "$nolist/docs/formal/theorem-catalog.md"
expect_fail "a catalog with no coverage-state list fails" bash "$checker" "$nolist"
