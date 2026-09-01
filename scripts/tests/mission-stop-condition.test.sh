#!/usr/bin/env bash
# check-mission-stop-condition.sh: waivers are metric metadata, not Atom exemptions. Every
# referenced Atom needs a fully validating closure record, #69 must propagate waiver lines, and
# #69 must carry every row's witness as a verifier bound to `bash scripts/mission-witness.sh <id>`.
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

| # | Acceptance item | Atoms | Witness |
| --- | --- | --- | --- |
| 1 | ontology evidence | #50 | ontology-evidence |
| 2 | correction evidence | #54 | correction-evidence |

### Waived human metrics

$waiver50
$waiver54
EOF
  # The Atom verifier is a script committed in the candidate, so the record's command names an
  # executable that exists at the subject state (the closure validator's command rule).
  mkdir -p "$1/scripts"
  printf '#!/usr/bin/env bash\nprintf "canonical verifier output\\n"\n' > "$1/scripts/check.sh"
  chmod +x "$1/scripts/check.sh"
  (cd "$1" && jj git init --colocate >/dev/null && jj commit -m candidate >/dev/null)
}

# write_record FIXTURE ATOM LIMITATIONS_JSON [ARTIFACT_PATH] [DIGEST] [EXIT_CODE] [WITNESS...]
# Each WITNESS adds a verifier `<id>` bound to `bash scripts/mission-witness.sh <id>`; the form
# `<id>:<command>` binds it to another command instead, to show that the id alone is not enough.
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
  printf 'subject_exact_state_id=%s\ncommand=scripts/check.sh\ncanonical verifier output\nexit_code=0\n' \
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
  python3 - "$fixture" "$record" "$atom" "$limitations_json" "$path_override" "$digest_override" "$exit_code" "$exact" "$logical" "${@:7}" <<'PY'
import hashlib
import json
import sys
from pathlib import Path

fixture, record, atom, limitations, artifact_path, digest, exit_code, exact, logical = sys.argv[1:10]
verifiers = [{
    "verifier_id": "check",
    "command": "scripts/check.sh",
    "exit_code": int(exit_code),
    "artifact_path": artifact_path,
    "artifact_sha256": digest,
    "subject_exact_state_id": exact,
}]
for spec in sys.argv[10:]:
    witness, _, command = spec.partition(":")
    command = command or f"bash scripts/mission-witness.sh {witness}"
    log_path = f"artifacts/atoms/{atom}/verifiers/{witness}.log"
    log_bytes = (
        f"subject_exact_state_id={exact}\ncommand={command}\nOK: witness {witness} holds\n"
        "exit_code=0\n"
    ).encode()
    log_file = Path(fixture) / log_path
    log_file.parent.mkdir(parents=True, exist_ok=True)
    log_file.write_bytes(log_bytes)
    verifiers.append({
        "verifier_id": witness,
        "command": command,
        "exit_code": 0,
        "artifact_path": log_path,
        "artifact_sha256": hashlib.sha256(log_bytes).hexdigest(),
        "subject_exact_state_id": exact,
    })
payload = {
    "record_format": "gordian-closure-v1",
    "atom_id": atom,
    "spec_digest": "0" * 64,
    "actor": {"id": "gordian-agent/codex/mission-stop-test", "kind": "agent"},
    "exact_state_id": exact,
    "logical_change_id": logical,
    "verifiers": verifiers,
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
expect_status_contains 0 "'bash scripts/mission-witness.sh <witness>': ontology-evidence, correction-evidence" \
  "preclose lists the witnesses #69 must carry, in row order" \
  bash "$checker" --preclose 69 "$preclose"
expect_status_contains nonzero "witness ontology-evidence: #69 has no closure record to carry it" \
  "final gate still requires #69, which carries the witnesses" \
  bash "$checker" --gate "$preclose"
expect_status_contains 0 "witness correction-evidence: #69 has no closure record to carry it" \
  "bare mode reports the unresolved witnesses and exits 0" \
  bash "$checker" "$preclose"

canonical="$(new_fixture "${schema[@]}")"
make_plan "$canonical"
limits="$(python3 -c 'import json,sys; print(json.dumps(sys.argv[1:]))' "$waiver50" "$waiver54")"
write_record "$canonical" 50 "[]"
write_record "$canonical" 54 "[]"
write_record "$canonical" 69 "$limits" "" "" 0 ontology-evidence correction-evidence
expect_status_contains 0 "OK: the Mission stop condition holds" "canonical records and exact waiver propagation pass" \
  bash "$checker" --gate "$canonical"
expect_status_contains 0 "#69 carries their 2 witnesses" "the passing verdict counts the witnesses" \
  bash "$checker" --gate "$canonical"

no_witness="$(new_fixture "${schema[@]}")"
make_plan "$no_witness"
write_record "$no_witness" 50 "[]"
write_record "$no_witness" 54 "[]"
write_record "$no_witness" 69 "$limits"
expect_status_contains nonzero "witness ontology-evidence: #69's record has no verifier 'ontology-evidence' with command 'bash scripts/mission-witness.sh ontology-evidence'" \
  "closed Atoms without the row's witness in #69 leave the row unsatisfied" \
  bash "$checker" --gate "$no_witness"
expect_status_contains nonzero "UNSATISFIED  row 2: correction evidence" \
  "every row without its witness is named" \
  bash "$checker" --gate "$no_witness"

one_witness="$(new_fixture "${schema[@]}")"
make_plan "$one_witness"
write_record "$one_witness" 50 "[]"
write_record "$one_witness" 54 "[]"
write_record "$one_witness" 69 "$limits" "" "" 0 ontology-evidence
expect_status_contains nonzero "Mission incomplete: 1 of 2 acceptance rows unsatisfied" \
  "a row whose witness #69 carries resolves; the other does not" \
  bash "$checker" --gate "$one_witness"

wrong_command="$(new_fixture "${schema[@]}")"
make_plan "$wrong_command"
write_record "$wrong_command" 50 "[]"
write_record "$wrong_command" 54 "[]"
write_record "$wrong_command" 69 "$limits" "" "" 0 \
  "ontology-evidence:bash scripts/mission-witness.sh --list" correction-evidence
expect_status_contains nonzero "witness ontology-evidence: #69's record has no verifier 'ontology-evidence' with command" \
  "a verifier with the witness id but another command is not the witness" \
  bash "$checker" --gate "$wrong_command"

valid_list="$(new_fixture "${schema[@]}")"
make_plan "$valid_list"
python3 - "$valid_list/docs/implementation/project-plan.md" <<'PY'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
text = text.replace(
    "| 1 | ontology evidence | #50 | ontology-evidence |",
    "| 1 | ontology evidence | #50, #54 | ontology-evidence |",
    1,
)
open(path, "w", encoding="utf-8").write(text)
PY
write_record "$valid_list" 50 "[]"
write_record "$valid_list" 54 "[]"
write_record "$valid_list" 69 "$limits" "" "" 0 ontology-evidence correction-evidence
expect_status_contains 0 "OK: the Mission stop condition holds" \
  "documented comma-separated Atom list syntax is accepted" \
  bash "$checker" --gate "$valid_list"

propagation="$(new_fixture "${schema[@]}")"
make_plan "$propagation"
write_record "$propagation" 50 "[]"
write_record "$propagation" 54 "[]"
write_record "$propagation" 69 "$(python3 -c 'import json; print(json.dumps([]))')" "" "" 0 \
  ontology-evidence correction-evidence
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

for invalid_table in atom_alias atom_residue atom_decimal atom_text row_duplicate row_gap atom_duplicate \
  three_cells witness_invalid witness_duplicate cites_69; do
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
text = text.replace("| ontology evidence | #50 |", "| ontology evidence | #50foo |", 1)
open(path, "w", encoding="utf-8").write(text)
PY
      expected="invalid Atom reference list"
      ;;
    atom_decimal)
      python3 - "$invalid/docs/implementation/project-plan.md" <<'PY'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
text = text.replace("| ontology evidence | #50 |", "| ontology evidence | #50.0 |", 1)
open(path, "w", encoding="utf-8").write(text)
PY
      expected="invalid Atom reference list"
      ;;
    atom_text)
      python3 - "$invalid/docs/implementation/project-plan.md" <<'PY'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
text = text.replace("| ontology evidence | #50 |", "| ontology evidence | all atoms |", 1)
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
text = text.replace("| ontology evidence | #50 |", "| ontology evidence | #50, #50 |", 1)
open(path, "w", encoding="utf-8").write(text)
PY
      expected="repeats an Atom"
      ;;
    three_cells)
      python3 - "$invalid/docs/implementation/project-plan.md" <<'PY'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
text = text.replace("| #50 | ontology-evidence |", "| #50 |", 1)
open(path, "w", encoding="utf-8").write(text)
PY
      expected="expected 4"
      ;;
    witness_invalid)
      python3 - "$invalid/docs/implementation/project-plan.md" <<'PY'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
text = text.replace("| ontology-evidence |", "| Ontology_Evidence |", 1)
open(path, "w", encoding="utf-8").write(text)
PY
      expected="invalid witness id"
      ;;
    witness_duplicate)
      python3 - "$invalid/docs/implementation/project-plan.md" <<'PY'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
text = text.replace("| correction-evidence |", "| ontology-evidence |", 1)
open(path, "w", encoding="utf-8").write(text)
PY
      expected="reuses witness"
      ;;
    cites_69)
      python3 - "$invalid/docs/implementation/project-plan.md" <<'PY'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
text = text.replace("| #50 | ontology-evidence |", "| #50, #69 | ontology-evidence |", 1)
open(path, "w", encoding="utf-8").write(text)
PY
      expected="cannot witness itself"
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
