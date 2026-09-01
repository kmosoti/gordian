#!/usr/bin/env bash
# check-mission-acceptance.sh: every acceptance row names real Atoms and a witness the runner
# lists; every Atom but #69 is named by some row; a pending witness names an open Atom of its own
# row. The fixture runner is a stand-in for scripts/mission-witness.sh that prints a listing file,
# so each case controls exactly what `--list` says. Every file the checker reads is written by
# make_fixture, so new_fixture is deliberately called with no paths (SC2119).
# shellcheck disable=SC1091,SC2119
set -euo pipefail
# shellcheck source=scripts/tests/harness.sh
. "$(dirname "$0")/harness.sh"

checker="$REPO_ROOT/scripts/check-mission-acceptance.sh"

make_fixture() {
  fixture="$(new_fixture)"
  mkdir -p "$fixture/docs/implementation" "$fixture/artifacts/atoms" "$fixture/scripts"
  cat > "$fixture/docs/implementation/project-plan.md" <<'EOF'
# Fixture Mission

## Mission acceptance

| # | Acceptance item | Atoms | Witness |
| --- | --- | --- | --- |
| 1 | ontology evidence | #50 | ontology-evidence |
| 2 | correction evidence | #54, #60 | correction-evidence |

## Something else

Not part of the table.
EOF
  cat > "$fixture/artifacts/atoms/issues.json" <<'EOF'
[
  {"number": 50, "state": "CLOSED"},
  {"number": 54, "state": "OPEN"},
  {"number": 60, "state": "OPEN"},
  {"number": 69, "state": "OPEN"}
]
EOF
  cat > "$fixture/scripts/mission-witness.sh" <<'EOF'
#!/usr/bin/env bash
# Fixture stand-in: `--list` prints scripts/witness-listing.txt, or fails when it is absent.
[ "${1:-}" = "--list" ] || exit 2
listing="$(dirname "$0")/witness-listing.txt"
[ -f "$listing" ] || exit 1
cat "$listing"
EOF
  printf 'ontology-evidence implemented\ncorrection-evidence pending #60\n' \
    > "$fixture/scripts/witness-listing.txt"
  printf '%s\n' "$fixture"
}

expect_output() {
  expected="$1"
  needle="$2"
  label="$3"
  shift 3
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
  if ! grep -Fq "$needle" <<<"$output"; then
    echo "   FAIL [$TEST_NAME] $label: output lacks $needle"
    printf '%s\n' "$output" | sed 's/^/          /'
    exit 1
  fi
  echo "   ok   [$TEST_NAME] $label"
}

edit_plan() {
  python3 - "$1/docs/implementation/project-plan.md" "$2" "$3" <<'PY'
import sys
path, old, new = sys.argv[1:]
text = open(path, encoding="utf-8").read()
if old not in text:
    raise SystemExit(f"fixture plan lacks {old!r}")
open(path, "w", encoding="utf-8").write(text.replace(old, new, 1))
PY
}

canonical="$(make_fixture)"
expect_output 0 "OK: 2 acceptance rows naming 3 distinct Atoms; 1 witness(es) implemented, 1 pending." \
  "a well-formed table whose witnesses the runner lists passes" bash "$checker" "$canonical"

no_table="$(make_fixture)"
edit_plan "$no_table" "## Mission acceptance" "## Mission contract"
expect_output nonzero "no '## Mission acceptance' table" "a missing table gates nothing and fails" \
  bash "$checker" "$no_table"

no_plan="$(make_fixture)"
rm "$no_plan/docs/implementation/project-plan.md"
expect_output nonzero "no '## Mission acceptance' table" "a missing plan fails" bash "$checker" "$no_plan"

no_runner="$(make_fixture)"
rm "$no_runner/scripts/mission-witness.sh"
expect_output nonzero "scripts/mission-witness.sh is missing" "a table without a runner names checks nothing can run" \
  bash "$checker" "$no_runner"

list_fails="$(make_fixture)"
rm "$list_fails/scripts/witness-listing.txt"
expect_output nonzero "scripts/mission-witness.sh --list failed" "a runner whose --list fails is a failure" \
  bash "$checker" "$list_fails"

three_cells="$(make_fixture)"
edit_plan "$three_cells" "| #50 | ontology-evidence |" "| #50 |"
expect_output nonzero "has 3 cells; expected 4" "a row of the wrong shape is a defect, not a skipped line" \
  bash "$checker" "$three_cells"

uncited="$(make_fixture)"
python3 - "$uncited/artifacts/atoms/issues.json" <<'PY'
import json
import sys
path = sys.argv[1]
issues = json.load(open(path, encoding="utf-8"))
issues.append({"number": 61, "state": "OPEN"})
json.dump(issues, open(path, "w", encoding="utf-8"))
PY
expect_output nonzero "1 issue(s) are named by no acceptance row, so closing them proves nothing: #61" \
  "an issue no row names is ungated work" bash "$checker" "$uncited"

cites_69="$(make_fixture)"
edit_plan "$cites_69" "| #50 | ontology-evidence |" "| #50, #69 | ontology-evidence |"
expect_output nonzero "cite #69, whose record holds the witnesses; it cannot witness itself" \
  "#69 is the one Atom no row may cite" bash "$checker" "$cites_69"

not_an_issue="$(make_fixture)"
edit_plan "$not_an_issue" "| #50 | ontology-evidence |" "| #50, #999 | ontology-evidence |"
expect_output nonzero "row 1 names #999, which is not an issue" "a row naming a non-issue can never resolve" \
  bash "$checker" "$not_an_issue"

unknown_witness="$(make_fixture)"
edit_plan "$unknown_witness" "| ontology-evidence |" "| ontology-proof |"
expect_output nonzero "row 1 names witness 'ontology-proof', which scripts/mission-witness.sh does not list" \
  "a witness the runner does not list can never run" bash "$checker" "$unknown_witness"
expect_output nonzero "scripts/mission-witness.sh lists witness 'ontology-evidence', which no row names" \
  "a runner entry no row names is reported too" bash "$checker" "$unknown_witness"

invalid_id="$(make_fixture)"
edit_plan "$invalid_id" "| ontology-evidence |" "| Ontology_Evidence |"
expect_output nonzero "row 1 has invalid witness id 'Ontology_Evidence'" "witness ids are [a-z][a-z0-9-]*" \
  bash "$checker" "$invalid_id"

duplicate_witness="$(make_fixture)"
edit_plan "$duplicate_witness" "| correction-evidence |" "| ontology-evidence |"
expect_output nonzero "row 2 reuses witness 'ontology-evidence' of row 1" "one witness cannot stand for two rows" \
  bash "$checker" "$duplicate_witness"

duplicate_listing="$(make_fixture)"
printf 'ontology-evidence implemented\n' >> "$duplicate_listing/scripts/witness-listing.txt"
expect_output nonzero "scripts/mission-witness.sh lists witness 'ontology-evidence' twice" \
  "a runner listing one witness twice is malformed" bash "$checker" "$duplicate_listing"

unparseable="$(make_fixture)"
printf 'ontology-evidence implemented\ncorrection-evidence waiting on #60\n' \
  > "$unparseable/scripts/witness-listing.txt"
expect_output nonzero "unparseable line 'correction-evidence waiting on #60'" \
  "a listing line outside the contract is a failure" bash "$checker" "$unparseable"

pending_off_row="$(make_fixture)"
printf 'ontology-evidence implemented\ncorrection-evidence pending #50\n' \
  > "$pending_off_row/scripts/witness-listing.txt"
expect_output nonzero "witness 'correction-evidence' is pending on #50, which row 2 does not cite" \
  "a pending witness must wait on an Atom of its own row" bash "$checker" "$pending_off_row"

pending_closed="$(make_fixture)"
mkdir -p "$pending_closed/artifacts/atoms/60"
printf '{}\n' > "$pending_closed/artifacts/atoms/60/closure.json"
expect_output nonzero "witness 'correction-evidence' is pending on #60, which has a closure record; implement the witness in scripts/mission-witness.sh" \
  "a witness still pending on a closed Atom is a row with no demonstration" bash "$checker" "$pending_closed"

row_gap="$(make_fixture)"
edit_plan "$row_gap" "| 2 | correction evidence" "| 3 | correction evidence"
expect_output nonzero "the acceptance rows are not numbered 1..2: [1, 3]" "rows are numbered 1..N" \
  bash "$checker" "$row_gap"

no_snapshot="$(make_fixture)"
rm "$no_snapshot/artifacts/atoms/issues.json"
expect_output 0 "OK: 2 acceptance rows naming 3 distinct Atoms" \
  "without a snapshot the table is still checked against the runner" bash "$checker" "$no_snapshot"
