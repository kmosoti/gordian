#!/usr/bin/env bash
# check-closure-records.sh validates closure records AND attempt records with the same validator,
# which is what agent-runbook.md section 7 promises. No records is "not yet"; a record that
# violates the schema or the cross-field rules is "broken".
set -euo pipefail
# shellcheck source=scripts/tests/harness.sh
# shellcheck disable=SC1091
. "$(dirname "$0")/harness.sh"

checker="$REPO_ROOT/scripts/check-closure-records.sh"
schemas=(artifacts/schema/closure-record.schema.json artifacts/schema/attempt-record.schema.json)

attempt() {
  cat > "$1" <<JSON
{
  "record_format": "gordian-attempt-v1",
  "attempt_id": "20260830T142530Z-claude-code",
  "atom_id": "42",
  "actor": { "id": "gordian-agent/claude-code/run-7", "kind": "agent" },
  "outcome": "${2:-verifier_failed}",
  "reason": "cargo test failed twice on the same exact state",
  "started_at": "2026-08-30T14:25:30Z",
  "finished_at": "2026-08-30T15:00:00Z"
}
JSON
}

empty="$(new_fixture "${schemas[@]}")"
expect_ok "no records exits 0" bash "$checker" "$empty"

noschema="$(new_fixture artifacts/schema/closure-record.schema.json)"
expect_fail "a missing attempt schema fails" bash "$checker" "$noschema"

good="$(new_fixture "${schemas[@]}")"
mkdir -p "$good/artifacts/atoms/42/attempts"
attempt "$good/artifacts/atoms/42/attempts/20260830T142530Z-claude-code.json"
expect_ok "a valid attempt record passes" bash "$checker" "$good"

outcome="$(new_fixture "${schemas[@]}")"
mkdir -p "$outcome/artifacts/atoms/42/attempts"
attempt "$outcome/artifacts/atoms/42/attempts/20260830T142530Z-claude-code.json" "gave_up"
expect_fail "an outcome outside the closed set fails" bash "$checker" "$outcome"

misnamed="$(new_fixture "${schemas[@]}")"
mkdir -p "$misnamed/artifacts/atoms/42/attempts"
attempt "$misnamed/artifacts/atoms/42/attempts/first.json"
expect_fail "a file name that is not <attempt_id>.json fails" bash "$checker" "$misnamed"

wrongatom="$(new_fixture "${schemas[@]}")"
mkdir -p "$wrongatom/artifacts/atoms/43/attempts"
attempt "$wrongatom/artifacts/atoms/43/attempts/20260830T142530Z-claude-code.json"
expect_fail "an atom_id disagreeing with its directory fails" bash "$checker" "$wrongatom"

defect="$(new_fixture "${schemas[@]}")"
mkdir -p "$defect/artifacts/atoms/42/attempts"
attempt "$defect/artifacts/atoms/42/attempts/20260830T142530Z-claude-code.json" "contract_defect"
expect_fail "contract_defect with no repair issue fails" bash "$checker" "$defect"

repaired="$(new_fixture "${schemas[@]}")"
mkdir -p "$repaired/artifacts/atoms/42/attempts"
attempt "$repaired/artifacts/atoms/42/attempts/20260830T142530Z-claude-code.json" "contract_defect"
python3 - "$repaired/artifacts/atoms/42/attempts/20260830T142530Z-claude-code.json" <<'PY'
import json, sys
path = sys.argv[1]
record = json.load(open(path))
record["contract_defect_issue"] = 99
json.dump(record, open(path, "w"), indent=2)
PY
expect_ok "contract_defect naming its repair issue passes" bash "$checker" "$repaired"

stamp="$(new_fixture "${schemas[@]}")"
mkdir -p "$stamp/artifacts/atoms/42/attempts"
attempt "$stamp/artifacts/atoms/42/attempts/20260830T142530Z-claude-code.json"
python3 - "$stamp/artifacts/atoms/42/attempts/20260830T142530Z-claude-code.json" <<'PY'
import json, sys
path = sys.argv[1]
record = json.load(open(path))
record["started_at"] = "2026-01-01T00:00:00Z"
json.dump(record, open(path, "w"), indent=2)
PY
expect_fail "an attempt_id that disagrees with started_at fails" bash "$checker" "$stamp"

closure="$(new_fixture "${schemas[@]}")"
mkdir -p "$closure/artifacts/atoms/42"
cat > "$closure/artifacts/atoms/42/closure.json" <<'JSON'
{
  "record_format": "gordian-closure-v1",
  "atom_id": "42",
  "spec_digest": "not-a-digest",
  "actor": { "id": "gordian-agent/claude-code/run-7", "kind": "agent" },
  "exact_state_id": "abc",
  "logical_change_id": "def",
  "verifiers": [],
  "benchmarks": [],
  "knowledge_graph_node_ids": [],
  "known_limitations": [],
  "closed_at": "2026-08-30T00:00:00Z"
}
JSON
expect_fail "a closure record with no verifier and a malformed spec_digest fails" \
  bash "$checker" "$closure"

# write_valid_closure ROOT [MUTATION]: a candidate, then a bookkeeping change holding the
# verifier log and closure.json, pushed as main. The mutation is applied BEFORE that commit:
# the checker reads artifacts from the bookkeeping change through JJ, not from disk, and a
# record edited after the commit fails for "differs from authoritative bookkeeping state"
# whatever else is wrong with it — which would let every case below pass for one reason.
write_valid_closure() {
  local root="$1"
  local mutation="${2:-}"
  (cd "$root" && jj git init --colocate >/dev/null && jj commit -m candidate >/dev/null)
  read -r exact logical <<EOF
$(cd "$root" && jj log -r @- -n 1 --no-graph -T 'commit_id ++ " " ++ change_id')
EOF
  mkdir -p "$root/artifacts/atoms/42/verifiers"
  python3 - "$root" "$exact" "$logical" <<'PY'
import hashlib
import json
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
exact, logical = sys.argv[2:]
command = "printf canonical verifier output"
# The shape scripts/capture-verifier.sh writes: subject line, command line, output, trailer.
artifact = (
    f"subject_exact_state_id={exact}\ncommand={command}\n".encode()
    + b"canonical verifier output\nexit_code=0\n"
)
artifact_path = root / "artifacts/atoms/42/verifiers/check.log"
artifact_path.write_bytes(artifact)
record = {
    "record_format": "gordian-closure-v1",
    "atom_id": "42",
    "spec_digest": "0" * 64,
    "actor": {"id": "gordian-agent/claude-code/run-7", "kind": "agent"},
    "exact_state_id": exact,
    "logical_change_id": logical,
    "verifiers": [{
        "verifier_id": "check",
        "command": command,
        "exit_code": 0,
        "artifact_path": "artifacts/atoms/42/verifiers/check.log",
        "artifact_sha256": hashlib.sha256(artifact).hexdigest(),
        "subject_exact_state_id": exact,
    }],
    "benchmarks": [],
    "knowledge_graph_node_ids": [],
    "known_limitations": [],
    "closed_at": "2026-08-30T00:00:00Z",
}
(root / "artifacts/atoms/42/closure.json").write_text(
    json.dumps(record, indent=2), encoding="utf-8"
)
PY
  if [ -n "$mutation" ]; then
    mutate_closure "$root" "$mutation"
  fi
  (cd "$root" && jj commit -m bookkeeping >/dev/null && jj bookmark create main -r @- >/dev/null && jj git remote add origin "$root/.git" >/dev/null 2>&1 || true && jj git push --bookmark main >/dev/null)
}

# expect_fail_for NEEDLE LABEL COMMAND...: non-zero exit AND the named reason in the output.
# A checker that fails for the wrong reason is not testing the rule the case is about.
expect_fail_for() {
  local needle="$1" label="$2"
  shift 2
  if output="$("$@" 2>&1)"; then
    echo "   FAIL [$TEST_NAME] $label: expected a non-zero exit, got 0"
    printf '%s\n' "$output" | sed 's/^/          /'
    exit 1
  fi
  if ! printf '%s\n' "$output" | grep -qF -- "$needle"; then
    echo "   FAIL [$TEST_NAME] $label: failed, but not for '$needle':"
    printf '%s\n' "$output" | sed 's/^/          /'
    exit 1
  fi
  echo "   ok   [$TEST_NAME] $label"
}

mutate_closure() {
  local root="$1"
  local expression="$2"
  python3 - "$root/artifacts/atoms/42/closure.json" "$expression" <<'PY'
import hashlib
import json
import pathlib
import sys

path, expression = sys.argv[1:]
root = pathlib.Path(path).parents[3]
record = json.loads(open(path, encoding="utf-8").read())
verifier = record["verifiers"][0]
log = root / verifier["artifact_path"]


def rewrite_log(content: bytes) -> None:
    # The digest stays honest: these mutations attack the binding, not the hash.
    log.write_bytes(content)
    verifier["artifact_sha256"] = hashlib.sha256(content).hexdigest()


header, _, body = log.read_bytes().partition(b"\n")
_, _, output = body.partition(b"\n")
if expression == "unbound-artifact":
    rewrite_log(output)
elif expression == "foreign-subject":
    rewrite_log(b"subject_exact_state_id=" + b"f" * 40 + b"\n" + body)
elif expression == "foreign-command":
    rewrite_log(header + b"\ncommand=true\n" + output)
elif expression == "duplicate-digest":
    twin = dict(verifier)
    twin["verifier_id"] = "twin"
    twin["artifact_path"] = "artifacts/atoms/42/verifiers/twin.log"
    (root / twin["artifact_path"]).write_bytes(log.read_bytes())
    record["verifiers"].append(twin)
elif expression == "multi-line-command":
    verifier["command"] = verifier["command"] + "\ntrue"
elif expression == "empty-id":
    verifier["verifier_id"] = ""
elif expression == "unsafe-id":
    verifier["verifier_id"] = "../check"
elif expression == "empty-path":
    verifier["artifact_path"] = ""
elif expression == "arbitrary-path":
    verifier["artifact_path"] = "README.md"
elif expression == "relative-path":
    verifier["artifact_path"] = "verifiers/check.log"
elif expression == "self-path":
    verifier["artifact_path"] = "artifacts/atoms/42/closure.json"
elif expression == "duplicate":
    record["verifiers"].append(dict(verifier))
elif expression == "nonzero":
    verifier["exit_code"] = 1
elif expression == "empty-command":
    verifier["command"] = ""
elif expression == "empty-digest":
    verifier["artifact_sha256"] = ""
elif expression == "missing-artifact":
    verifier["artifact_path"] = "artifacts/atoms/42/verifiers/missing.log"
else:
    raise SystemExit(f"unknown mutation {expression}")
with open(path, "w", encoding="utf-8") as handle:
    json.dump(record, handle, indent=2)
PY
}

canonical="$(new_fixture "${schemas[@]}")"
write_valid_closure "$canonical"
expect_ok "a canonical zero-exit verifier log with an exact digest passes" \
  bash "$checker" "$canonical"

while IFS='|' read -r mutation reason; do
  adversarial="$(new_fixture "${schemas[@]}")"
  write_valid_closure "$adversarial" "$mutation"
  expect_fail_for "$reason" "a closure verifier mutation ($mutation) fails closed for that reason" \
    bash "$checker" "$adversarial"
done <<'CASES'
empty-id|verifier_id must be a non-empty safe id
unsafe-id|verifier_id must be a non-empty safe id
empty-path|artifact_path must be a non-empty
arbitrary-path|must be exactly 'artifacts/atoms/42/verifiers/check.log'
relative-path|must be exactly 'artifacts/atoms/42/verifiers/check.log'
self-path|a record cannot record its own digest
duplicate|duplicate verifier_id 'check'
nonzero|exit_code must be exactly 0
empty-command|command must be a non-empty string
empty-digest|artifact_sha256 must be 64 lowercase hex characters
missing-artifact|must be exactly 'artifacts/atoms/42/verifiers/check.log'
unbound-artifact|artifact line 1 must be subject_exact_state_id=
foreign-subject|artifact line 1 must be subject_exact_state_id=
foreign-command|artifact line 2 must be command=printf canonical verifier output; found 'command=true'
duplicate-digest|artifact_sha256 duplicates verifiers[0]
multi-line-command|command must not contain line breaks
CASES

# And the check that the pre-commit path is the one being tested: a record edited on disk
# after bookkeeping is rejected as diverging from the authoritative state, on its own.
edited="$(new_fixture "${schemas[@]}")"
write_valid_closure "$edited"
mutate_closure "$edited" "nonzero"
expect_fail_for "closure record differs from authoritative bookkeeping state" \
  "a record edited after the bookkeeping commit is not the authoritative record" \
  bash "$checker" "$edited"
