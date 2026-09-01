#!/usr/bin/env bash
# check-mission-stop-condition.sh: waivers are metric metadata, not Atom exemptions. Every
# referenced Atom needs a fully validating closure record, and #69 must propagate waiver lines.
set -euo pipefail
# shellcheck source=scripts/tests/harness.sh
# shellcheck disable=SC1091
. "$(dirname "$0")/harness.sh"

checker="$REPO_ROOT/scripts/check-mission-stop-condition.sh"
schema=(artifacts/schema/closure-record.schema.json)

waiver50='unresolved_human_metric: #50 — operator-comprehension rating — no machine substitute exists'
waiver54='unresolved_human_metric: #54 — operator/manual correction count — human intervention defines the count'

make_plan() {
  mkdir -p "$1/docs/implementation"
  cp "$REPO_ROOT/docs/implementation/agent-runbook.md" \
    "$1/docs/implementation/agent-runbook.md"
  cat > "$1/docs/implementation/project-plan.md" <<EOF
# Fixture Mission

## Mission acceptance

| # | Acceptance item | Atoms |
| --- | --- | --- |
| 1 | ontology evidence | #50 |
| 2 | correction evidence | #54 |

### Waived human metrics

$waiver50
$waiver54
EOF
  (cd "$1" && jj git init --colocate >/dev/null && jj commit -m candidate >/dev/null)
}

write_record() {
  fixture="$1"
  atom="$2"
  limitations_json="$3"
  path_override="${4:-artifacts/atoms/$atom/verifiers/check.log}"
  digest_override="${5:-}"
  exit_code="${6:-0}"
  record="$fixture/artifacts/atoms/$atom/closure.json"
  artifact="$fixture/$path_override"
  mkdir -p "$(dirname "$record")" "$(dirname "$artifact")"
  read -r exact logical <<EOF
$(cd "$fixture" && jj log -r @- -n 1 --no-graph -T 'commit_id ++ " " ++ change_id')
EOF
  # The shape scripts/capture-verifier.sh writes; an unbound log is not evidence.
  printf 'subject_exact_state_id=%s\ncommand=true\ncanonical verifier output\nexit_code=0\n' \
    "$exact" > "$artifact"
  actual_digest="$(python3 - "$artifact" <<'PY'
import hashlib
import sys
print(hashlib.sha256(open(sys.argv[1], "rb").read()).hexdigest())
PY
)"
  if [ -z "$digest_override" ]; then
    digest_override="$actual_digest"
  fi
  python3 - "$record" "$atom" "$limitations_json" "$path_override" "$digest_override" "$exit_code" "$exact" "$logical" <<'PY'
import json
import sys

record, atom, limitations, artifact_path, digest, exit_code, exact, logical = sys.argv[1:]
payload = {
    "record_format": "gordian-closure-v1",
    "atom_id": atom,
    "spec_digest": "0" * 64,
    "actor": {"id": "gordian-agent/codex/mission-stop-test", "kind": "agent"},
    "exact_state_id": exact,
    "logical_change_id": logical,
    "verifiers": [{
        "verifier_id": "check",
        "command": "true",
        "exit_code": int(exit_code),
        "artifact_path": artifact_path,
        "artifact_sha256": digest,
        "subject_exact_state_id": exact,
    }],
    "benchmarks": [],
    "knowledge_graph_node_ids": [],
    "known_limitations": json.loads(limitations),
    "closed_at": "2026-08-31T00:00:00Z",
}
with open(record, "w", encoding="utf-8") as handle:
    json.dump(payload, handle, indent=2)
    handle.write("\n")
PY
  (cd "$fixture" && jj commit -m bookkeeping >/dev/null && (jj bookmark set main -r @- >/dev/null 2>&1 || jj bookmark create main -r @- >/dev/null) && (jj git remote add origin "$fixture/.git" >/dev/null 2>&1 || true) && jj git push --bookmark main >/dev/null)
}

expect_status_contains() {
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
      echo "   FAIL [$TEST_NAME] $label: expected non-zero, got 0"
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

missing="$(new_fixture "${schema[@]}")"
make_plan "$missing"
expect_status_contains 0 "UNSATISFIED  row 1" "bare mode reports waived #50/#54 as unsatisfied" \
  bash "$checker" "$missing"
expect_status_contains nonzero "UNSATISFIED  row 1" "gate mode rejects missing waived Atoms" \
  bash "$checker" --gate "$missing"
expect_status_contains nonzero "#54: no closure record" "gate names the second missing waived Atom" \
  bash "$checker" --gate "$missing"

preclose="$(new_fixture "${schema[@]}")"
make_plan "$preclose"
write_record "$preclose" 50 "[]"
write_record "$preclose" 54 "[]"
expect_status_contains 0 "PRE-CLOSE" "preclose permits only absent #69 after all row closures pass" \
  bash "$checker" --preclose 69 "$preclose"
expect_status_contains nonzero "no closure record" "final gate still requires #69" \
  bash "$checker" --gate "$preclose"

canonical="$(new_fixture "${schema[@]}")"
make_plan "$canonical"
limits="$(python3 -c 'import json,sys; print(json.dumps(sys.argv[1:]))' "$waiver50" "$waiver54")"
write_record "$canonical" 50 "[]"
write_record "$canonical" 54 "[]"
write_record "$canonical" 69 "$limits"
expect_status_contains 0 "OK: the Mission stop condition holds" "canonical records and exact waiver propagation pass" \
  bash "$checker" --gate "$canonical"

valid_list="$(new_fixture "${schema[@]}")"
make_plan "$valid_list"
python3 - "$valid_list/docs/implementation/project-plan.md" <<'PY'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
text = text.replace("| 1 | ontology evidence | #50 |", "| 1 | ontology evidence | #50, #54 |", 1)
open(path, "w", encoding="utf-8").write(text)
PY
write_record "$valid_list" 50 "[]"
write_record "$valid_list" 54 "[]"
write_record "$valid_list" 69 "$limits"
expect_status_contains 0 "OK: the Mission stop condition holds" \
  "documented comma-separated Atom list syntax is accepted" \
  bash "$checker" --gate "$valid_list"

propagation="$(new_fixture "${schema[@]}")"
make_plan "$propagation"
write_record "$propagation" 50 "[]"
write_record "$propagation" 54 "[]"
write_record "$propagation" 69 "$(python3 -c 'import json; print(json.dumps([]))')"
expect_status_contains nonzero "missing waiver line verbatim" "missing #69 waiver propagation fails" \
  bash "$checker" --gate "$propagation"

for invalid_case in digest path exit; do
  invalid="$(new_fixture "${schema[@]}")"
  make_plan "$invalid"
  case "$invalid_case" in
    digest)
      write_record "$invalid" 50 "[]" \
        "artifacts/atoms/50/verifiers/check.log" "$(printf '0%.0s' $(seq 64))"
      expected="hashes to"
      ;;
    path)
      write_record "$invalid" 50 "[]" \
        "artifacts/atoms/50/verifiers/noncanonical.log"
      expected="artifact_path"
      ;;
    exit)
      write_record "$invalid" 50 "[]" \
        "artifacts/atoms/50/verifiers/check.log" "" 7
      expected="exit_code"
      ;;
  esac
  expect_status_contains nonzero "$expected" "invalid $invalid_case evidence is a structural failure" \
    bash "$checker" "$invalid"
done

bad_waiver="$(new_fixture "${schema[@]}")"
make_plan "$bad_waiver"
python3 - "$bad_waiver/docs/implementation/project-plan.md" <<'PY'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
needle = "unresolved_human_metric: #50 — operator-comprehension rating — no machine substitute exists"
replacement = (
    "unresolved_human_metric: #50 — operator-comprehension rating —\n"
    "  no machine substitute exists"
)
text = text.replace(needle, replacement)
open(path, "w", encoding="utf-8").write(text)
PY
expect_status_contains nonzero "waiver" "a waiver split across physical lines fails" \
  bash "$checker" "$bad_waiver"

duplicate="$(new_fixture "${schema[@]}")"
make_plan "$duplicate"
printf '%s\n' "$waiver50" >> "$duplicate/docs/implementation/project-plan.md"
expect_status_contains nonzero "already used" "duplicate Atom/metric waivers fail" \
  bash "$checker" "$duplicate"

out_of_table="$(new_fixture "${schema[@]}")"
make_plan "$out_of_table"
printf '%s\n' 'unresolved_human_metric: #999 — invented metric — no machine substitute exists' \
  >> "$out_of_table/docs/implementation/project-plan.md"
expect_status_contains nonzero "not in the table" "waiver for an Atom absent from the table fails" \
  bash "$checker" "$out_of_table"

for invalid_table in atom_alias atom_residue atom_decimal atom_text row_duplicate row_gap atom_duplicate; do
  invalid="$(new_fixture "${schema[@]}")"
  make_plan "$invalid"
  case "$invalid_table" in
    atom_alias)
      python3 - "$invalid/docs/implementation/project-plan.md" <<'PY'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
text = text.replace("#50", "#050", 1)
open(path, "w", encoding="utf-8").write(text)
PY
      expected="non-canonical Atom id"
      ;;
    atom_residue)
      python3 - "$invalid/docs/implementation/project-plan.md" <<'PY'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
text = text.replace("| 1 | ontology evidence | #50 |", "| 1 | ontology evidence | #50foo |", 1)
open(path, "w", encoding="utf-8").write(text)
PY
      expected="invalid Atom reference list"
      ;;
    atom_decimal)
      python3 - "$invalid/docs/implementation/project-plan.md" <<'PY'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
text = text.replace("| 1 | ontology evidence | #50 |", "| 1 | ontology evidence | #50.0 |", 1)
open(path, "w", encoding="utf-8").write(text)
PY
      expected="invalid Atom reference list"
      ;;
    atom_text)
      python3 - "$invalid/docs/implementation/project-plan.md" <<'PY'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
text = text.replace("| 1 | ontology evidence | #50 |", "| 1 | ontology evidence | all atoms |", 1)
open(path, "w", encoding="utf-8").write(text)
PY
      expected="invalid Atom reference list"
      ;;
    row_duplicate)
      python3 - "$invalid/docs/implementation/project-plan.md" <<'PY'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
text = text.replace("| 2 | correction evidence", "| 1 | correction evidence", 1)
open(path, "w", encoding="utf-8").write(text)
PY
      expected="row numbers must be unique"
      ;;
    row_gap)
      python3 - "$invalid/docs/implementation/project-plan.md" <<'PY'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
text = text.replace("| 2 | correction evidence", "| 3 | correction evidence", 1)
open(path, "w", encoding="utf-8").write(text)
PY
      expected="exactly sequential"
      ;;
    atom_duplicate)
      python3 - "$invalid/docs/implementation/project-plan.md" <<'PY'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
text = text.replace("| 1 | ontology evidence | #50 |", "| 1 | ontology evidence | #50, #50 |", 1)
open(path, "w", encoding="utf-8").write(text)
PY
      expected="repeats an Atom"
      ;;
  esac
  expect_status_contains nonzero "$expected" "rejects $invalid_table in every gate input" \
    bash "$checker" "$invalid"
done

race_bin="$(mktemp -d)"
real_python3="$(command -v python3)"
cat > "$race_bin/python3" <<'SH'
#!/usr/bin/env bash
sed -i 's/^## Mission acceptance$/## Mission contract/' "$RACE_PLAN"
exec "$REAL_PYTHON3" "$@"
SH
chmod +x "$race_bin/python3"
for race_mode in report gate preclose; do
  race="$(new_fixture "${schema[@]}")"
  make_plan "$race"
  case "$race_mode" in
    report)
      race_args=("$race")
      ;;
    gate)
      race_args=(--gate "$race")
      ;;
    preclose)
      race_args=(--preclose 69 "$race")
      ;;
  esac
  expect_status_contains nonzero "no longer contains the '## Mission acceptance' table" \
    "heading removal after shell preflight fails $race_mode mode's Python fallback" \
    env PATH="$race_bin:$PATH" RACE_PLAN="$race/docs/implementation/project-plan.md" \
      REAL_PYTHON3="$real_python3" bash "$checker" "${race_args[@]}"
done
rm -rf "$race_bin"

duplicate_metric="$(new_fixture "${schema[@]}")"
make_plan "$duplicate_metric"
printf '%s\n' 'unresolved_human_metric: #54 — operator-comprehension rating — same metric on another Atom' \
  >> "$duplicate_metric/docs/implementation/project-plan.md"
expect_status_contains nonzero "already used" "waiver metrics are globally unique across Atoms" \
  bash "$checker" "$duplicate_metric"

for missing_contract in plan table runbook sentence; do
  invalid="$(new_fixture "${schema[@]}")"
  make_plan "$invalid"
  case "$missing_contract" in
    plan)
      rm -f "$invalid/docs/implementation/project-plan.md"
      ;;
    table)
      python3 - "$invalid/docs/implementation/project-plan.md" <<'PY'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
text = text.replace("## Mission acceptance", "## Mission contract", 1)
open(path, "w", encoding="utf-8").write(text)
PY
      ;;
    runbook)
      rm -f "$invalid/docs/implementation/agent-runbook.md"
      ;;
    sentence)
      python3 - "$invalid/docs/implementation/agent-runbook.md" <<'PY'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
text = text.replace("The Mission loop terminates when", "The Mission loop might terminate when", 1)
open(path, "w", encoding="utf-8").write(text)
PY
      ;;
  esac
  expect_status_contains nonzero "FAIL:" "missing $missing_contract fails bare mode" \
    bash "$checker" "$invalid"
  expect_status_contains nonzero "FAIL:" "missing $missing_contract fails --gate" \
    bash "$checker" --gate "$invalid"
  expect_status_contains nonzero "FAIL:" "missing $missing_contract fails --preclose" \
    bash "$checker" --preclose 69 "$invalid"
done

expect_status 2 "unsupported preclose id is rejected" bash "$checker" --preclose 70 "$canonical"
expect_status 2 "preclose cannot be combined with gate" bash "$checker" --preclose 69 --gate "$canonical"
expect_status 2 "preclose requires its Atom id" bash "$checker" --preclose
