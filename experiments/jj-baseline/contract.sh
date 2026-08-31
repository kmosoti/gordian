#!/usr/bin/env bash
# Disposable contract qualification for the Jujutsu release pinned by
# scripts/bootstrap-jj.sh.  This intentionally tests only jj-run feature
# presence; its execution semantics belong to Atom #33.

set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/../.." && pwd)"
readonly SCRIPT_DIR REPO_ROOT
readonly DEFAULT_MANIFEST="$SCRIPT_DIR/manifest.json"

manifest_path="$DEFAULT_MANIFEST"
original_argv=("$@")
while (($#)); do
  case "$1" in
    --manifest)
      shift
      [[ -n "${1-}" ]] || { printf '%s\n' "--manifest requires a path" >&2; exit 2; }
      manifest_path="$1"
      ;;
    --manifest=*)
      manifest_path="${1#*=}"
      ;;
    --help|-h)
      cat <<'EOF'
Usage: contract.sh [--manifest PATH]

Run the disposable Jujutsu baseline contract fixtures and write a JSON
manifest. Set GORDIAN_JJ_CONTRACT_INJECT_FAILURE (or
JJ_CONTRACT_INJECT_FAILURE) to a fixture name, or any non-empty value, to
exercise the negative path. jj-run behavior is intentionally not tested here.
EOF
      exit 0
      ;;
    *)
      printf 'unknown argument: %s\n' "$1" >&2
      exit 2
      ;;
  esac
  shift
done

if [[ ! -r "$REPO_ROOT/scripts/bootstrap-jj.sh" ]]; then
  printf '%s\n' 'scripts/bootstrap-jj.sh is required to resolve the pinned version' >&2
  exit 1
fi

EXPECTED_JJ_VERSION="$(awk -F'"' '/DEFAULT_JJ_VERSION/ { print $2; exit }' "$REPO_ROOT/scripts/bootstrap-jj.sh")"
readonly EXPECTED_JJ_VERSION
[[ -n "$EXPECTED_JJ_VERSION" ]] || { printf '%s\n' 'could not resolve pinned Jujutsu version' >&2; exit 1; }

command -v jj >/dev/null 2>&1 || { printf '%s\n' 'jj is required' >&2; exit 1; }
command -v git >/dev/null 2>&1 || { printf '%s\n' 'git is required' >&2; exit 1; }
command -v python3 >/dev/null 2>&1 || { printf '%s\n' 'python3 is required' >&2; exit 1; }

JJ_PATH="$(command -v jj)"
JJ_VERSION_OUTPUT="$(jj --version)"
JJ_VERSION="$(awk 'NR == 1 { print $2; exit }' <<<"$JJ_VERSION_OUTPUT")"
readonly JJ_PATH JJ_VERSION_OUTPUT JJ_VERSION
[[ "$JJ_VERSION" == "$EXPECTED_JJ_VERSION" ]] || {
  printf 'unsupported Jujutsu version: found %s, expected %s\n' "$JJ_VERSION" "$EXPECTED_JJ_VERSION" >&2
  exit 1
}
GIT_VERSION_OUTPUT="$(git --version)"
readonly GIT_VERSION_OUTPUT
readonly FIXTURE_IDENTITY='jj-baseline-contract/disposable-repository'
readonly NEGATIVE_INJECTION="${GORDIAN_JJ_CONTRACT_NEGATIVE_INJECTION:-${GORDIAN_JJ_CONTRACT_INJECT_FAILURE:-${JJ_CONTRACT_INJECT_FAILURE:-}}}"
TEMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/gordian-jj-contract.XXXXXX")"
readonly RESULTS_FILE="$TEMP_ROOT/results.tsv"
readonly IDENTITIES_FILE="$TEMP_ROOT/identities.tsv"
STARTED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
ARGV_JSON="$(python3 -c 'import json,sys; print(json.dumps(sys.argv[1:]))' "${original_argv[@]}")"
readonly TEMP_ROOT STARTED_AT ARGV_JSON
mkdir -p "$(dirname -- "$manifest_path")"
: >"$RESULTS_FILE"
: >"$IDENTITIES_FILE"

cleanup() {
  local command_rc=$?
  local manifest_rc
  trap - EXIT
  set +e
  python3 - "$manifest_path" "$RESULTS_FILE" "$IDENTITIES_FILE" "$JJ_VERSION_OUTPUT" "$JJ_PATH" "$GIT_VERSION_OUTPUT" "$FIXTURE_IDENTITY" "$EXPECTED_JJ_VERSION" "$STARTED_AT" "$command_rc" "$ARGV_JSON" <<'PY'
import hashlib
import json
import os
import platform
import socket
import sys
from datetime import datetime, timezone

manifest_path, results_path, identities_path, jj_output, jj_path, git_output, fixture, expected, started, rc, argv_text = sys.argv[1:]

def rows(path):
    output = []
    try:
        with open(path, encoding="utf-8") as handle:
            for line in handle:
                fields = line.rstrip("\n").split("\t", 2)
                if len(fields) == 3:
                    output.append({"name": fields[0], "passed": fields[1] == "true", "detail": fields[2]})
    except FileNotFoundError:
        pass
    return output

def identity_rows(path):
    output = {}
    try:
        with open(path, encoding="utf-8") as handle:
            for line in handle:
                key, value = line.rstrip("\n").split("\t", 1)
                output[key] = value
    except FileNotFoundError:
        pass
    return output

results = rows(results_path)
identity_values = identity_rows(identities_path)
identity = {
    "change_ids": [value for key, value in identity_values.items() if key.endswith("change_id")],
    "commit_ids": [value for key, value in identity_values.items() if key.endswith("commit")],
    "operation_ids": [value for key, value in identity_values.items() if key.endswith("operation_id")],
}
binary_sha256 = hashlib.sha256()
try:
    with open(jj_path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            binary_sha256.update(chunk)
except OSError:
    pass

manifest = {
    "schema_version": 1,
    "fixture_identity": fixture,
    "argv": json.loads(argv_text),
    "started_at": started,
    "completed_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "expected_jj_version": expected,
    "jj_version_output": jj_output,
    "jj_binary": {"path": os.path.realpath(jj_path), "sha256": binary_sha256.hexdigest()},
    "git_version_output": git_output,
    "os": {"name": platform.system(), "release": platform.release(), "machine": platform.machine()},
    "filesystem": {
        "path": os.path.dirname(os.path.realpath(manifest_path)),
        "statvfs": {
            "block_size": os.statvfs(os.path.dirname(os.path.realpath(manifest_path))).f_bsize,
            "fragment_size": os.statvfs(os.path.dirname(os.path.realpath(manifest_path))).f_frsize,
            "blocks": os.statvfs(os.path.dirname(os.path.realpath(manifest_path))).f_blocks,
            "free_blocks": os.statvfs(os.path.dirname(os.path.realpath(manifest_path))).f_bfree,
            "files": os.statvfs(os.path.dirname(os.path.realpath(manifest_path))).f_files,
            "free_files": os.statvfs(os.path.dirname(os.path.realpath(manifest_path))).f_ffree,
        },
    },
    "host": socket.gethostname(),
    "identities": identity,
    "pass_results": results,
    "passed": rc == "0" and bool(results) and all(item["passed"] for item in results),
    "exit_code": int(rc),
}
with open(manifest_path, "w", encoding="utf-8") as handle:
    json.dump(manifest, handle, indent=2, sort_keys=True)
    handle.write("\n")
PY
  manifest_rc=$?
  rm -rf -- "$TEMP_ROOT"
  if ((command_rc != 0)); then
    exit "$command_rc"
  fi
  exit "$manifest_rc"
}
trap cleanup EXIT

record_identity() {
  printf '%s\t%s\n' "$1" "$2" >>"$IDENTITIES_FILE"
}

pass_result() {
  printf '%s\ttrue\t%s\n' "$1" "${2:-pass}" >>"$RESULTS_FILE"
}

fail_result() {
  printf '%s\tfalse\t%s\n' "$1" "${2:-failure}" >>"$RESULTS_FILE"
  printf 'contract fixture failed: %s (%s)\n' "$1" "${2:-failure}" >&2
  exit 1
}

inject_or_continue() {
  local name="$1"
  if [[ -n "$NEGATIVE_INJECTION" && ( "$NEGATIVE_INJECTION" == "1" || "$NEGATIVE_INJECTION" == "all" || "$NEGATIVE_INJECTION" == "$name" ) ]]; then
    fail_result "$name" "negative injection requested"
  fi
}

repo_init() {
  local path="$1"
  mkdir -p "$path"
  (cd "$path" && jj git init --colocate >/dev/null)
}

rev_id() {
  local repo="$1" rev="$2" field="$3"
  (cd "$repo" && jj log -r "$rev" --no-graph -T "$field ++ \"\\n\"")
}

assert_nonempty() {
  local name="$1" value="$2"
  [[ -n "$value" ]] || fail_result "$name" 'empty command result'
  pass_result "$name"
}

assert_regex() {
  local name="$1" value="$2" expression="$3"
  [[ "$value" =~ $expression ]] || fail_result "$name" "value did not match $expression: $value"
  pass_result "$name"
}

# Feature and identity contract.
inject_or_continue 'version-and-run-capabilities'
[[ "$JJ_VERSION_OUTPUT" == *"$EXPECTED_JJ_VERSION"* ]] || fail_result 'version-and-run-capabilities' 'jj --version output omitted the pinned version'
run_help="$(jj run --help 2>&1)" || fail_result 'version-and-run-capabilities' 'jj run --help failed'
[[ "$run_help" == *'--ignore-changes'* ]] || fail_result 'version-and-run-capabilities' 'jj run --help omitted --ignore-changes'
pass_result 'version-and-run-capabilities'

repo="$TEMP_ROOT/repository"
repo_init "$repo"
(cd "$repo" && printf 'base\n' >base.txt && jj commit -m base >/dev/null)
base_commit="$(rev_id "$repo" '@-' commit_id)"
base_change="$(rev_id "$repo" '@-' change_id)"
record_identity base_commit "$base_commit"
record_identity base_change_id "$base_change"
assert_regex 'machine-readable-commit-id' "$base_commit" '^[0-9a-f]{40}$'
assert_regex 'machine-readable-change-id' "$base_change" '^[a-z0-9]{32}$'

inject_or_continue 'change-id-rewrite'
(cd "$repo" && jj edit "$base_commit" >/dev/null && jj describe -m 'base rewritten' >/dev/null)
rewritten_commit="$(rev_id "$repo" '@' commit_id)"
rewritten_change="$(rev_id "$repo" '@' change_id)"
record_identity rewritten_commit "$rewritten_commit"
record_identity rewritten_change_id "$rewritten_change"
[[ "$rewritten_change" == "$base_change" ]] || fail_result 'change-id-rewrite' 'change ID changed during ordinary rewrite'
[[ "$rewritten_commit" != "$base_commit" ]] || fail_result 'change-id-rewrite' 'commit ID did not change during ordinary rewrite'
pass_result 'change-id-rewrite'

# Create and commit a stable fixture base before capturing its exact ID, then
# create two independent child changes from that immutable parent.
(cd "$repo" && jj new "$rewritten_commit" >/dev/null && printf 'fixture root\n' >fixture-root.txt && jj commit -m 'fixture root' >/dev/null)
fixture_base="$(rev_id "$repo" '@-' commit_id)"
(cd "$repo" && jj new "$fixture_base" >/dev/null && printf 'left\n' >left.txt && jj commit -m left >/dev/null)
left_commit="$(rev_id "$repo" '@-' commit_id)"
(cd "$repo" && jj new "$fixture_base" -m 'right' >/dev/null && printf 'right\n' >right.txt && jj commit -m right >/dev/null)
right_commit="$(rev_id "$repo" '@-' commit_id)"
record_identity left_commit "$left_commit"
record_identity right_commit "$right_commit"

inject_or_continue 'sibling-topology'
left_in_right="$(cd "$repo" && jj log -r "$left_commit & ancestors($right_commit)" --no-graph -T 'commit_id ++ "\n"')"
right_in_left="$(cd "$repo" && jj log -r "$right_commit & ancestors($left_commit)" --no-graph -T 'commit_id ++ "\n"')"
[[ -z "$left_in_right" && -z "$right_in_left" ]] || fail_result 'sibling-topology' 'independent changes became causally related'
pass_result 'sibling-topology'

inject_or_continue 'causal-parent-child-topology'
(cd "$repo" && jj new "$left_commit" -m child >/dev/null && printf 'child\n' >child.txt && jj commit -m child >/dev/null)
child_commit="$(rev_id "$repo" '@-' commit_id)"
parent_match="$(cd "$repo" && jj log -r "$left_commit & parents($child_commit)" --no-graph -T 'commit_id ++ "\n"')"
ancestor_match="$(cd "$repo" && jj log -r "$fixture_base & ancestors($child_commit)" --no-graph -T 'commit_id ++ "\n"')"
[[ "$parent_match" == "$left_commit" ]] || fail_result 'causal-parent-child-topology' 'direct parent relation was not preserved'
[[ -n "$ancestor_match" ]] || fail_result 'causal-parent-child-topology' 'causal ancestor relation was not preserved'
record_identity child_commit "$child_commit"
pass_result 'causal-parent-child-topology'

inject_or_continue 'workspace-isolation'
isolated="$TEMP_ROOT/isolated-workspace"
(cd "$repo" && jj workspace add "$isolated" --name isolated -r "$fixture_base" >/dev/null)
(cd "$isolated" && printf 'isolated\n' >isolated.txt)
[[ ! -e "$repo/isolated.txt" ]] || fail_result 'workspace-isolation' 'workspace change leaked into the primary checkout'
isolated_status="$(cd "$isolated" && jj status)"
[[ "$isolated_status" == *isolated.txt* ]] || fail_result 'workspace-isolation' 'isolated workspace did not observe its own change'
pass_result 'workspace-isolation'

inject_or_continue 'multi-parent-integration'
(cd "$repo" && jj new "$left_commit" "$right_commit" -m integration >/dev/null && jj commit -m integration >/dev/null)
integration_commit="$(rev_id "$repo" '@-' commit_id)"
integration_parents="$(cd "$repo" && jj log -r "parents($integration_commit)" --no-graph -T 'commit_id ++ "\n"')"
[[ "$(wc -l <<<"$integration_parents")" -eq 2 ]] || fail_result 'multi-parent-integration' 'integration candidate did not retain two parents'
record_identity integration_commit "$integration_commit"
pass_result 'multi-parent-integration'

inject_or_continue 'conflict-persistence-and-repair'
conflict_repo="$TEMP_ROOT/conflict-repository"
repo_init "$conflict_repo"
(cd "$conflict_repo" && printf 'base\n' >shared.txt && jj commit -m base >/dev/null)
conflict_base="$(rev_id "$conflict_repo" '@-' commit_id)"
(cd "$conflict_repo" && jj new "$conflict_base" -m left >/dev/null && printf 'left\n' >shared.txt && jj commit -m left >/dev/null)
conflict_left="$(rev_id "$conflict_repo" '@-' commit_id)"
(cd "$conflict_repo" && jj new "$conflict_base" -m right >/dev/null && printf 'right\n' >shared.txt && jj commit -m right >/dev/null)
conflict_right="$(rev_id "$conflict_repo" '@-' commit_id)"
(cd "$conflict_repo" && jj new "$conflict_left" "$conflict_right" -m conflict >/dev/null)
conflict_state="$(rev_id "$conflict_repo" '@' conflict)"
[[ "$conflict_state" == true ]] || fail_result 'conflict-persistence-and-repair' 'multi-parent conflict was not represented as first-class state'
(cd "$conflict_repo" && jj restore --from "$conflict_left" -- shared.txt >/dev/null && jj commit -m repaired >/dev/null)
repaired_state="$(rev_id "$conflict_repo" '@-' conflict)"
[[ "$repaired_state" == false ]] || fail_result 'conflict-persistence-and-repair' 'repair left an unresolved conflict'
conflict_repaired_commit="$(rev_id "$conflict_repo" '@-' commit_id)"
record_identity conflict_repaired_commit "$conflict_repaired_commit"
pass_result 'conflict-persistence-and-repair'

inject_or_continue 'operation-log-restore'
operation_repo="$TEMP_ROOT/operation-repository"
repo_init "$operation_repo"
(cd "$operation_repo" && printf 'base\n' >base.txt && jj commit -m base >/dev/null)
operation_base="$(rev_id "$operation_repo" '@-' commit_id)"
(cd "$operation_repo" && jj bookmark create restore-marker -r "$operation_base" >/dev/null)
operation_id="$(cd "$operation_repo" && jj op log -n 1 --no-graph -T 'id ++ "\n"')"
record_identity operation_id "$operation_id"
(cd "$operation_repo" && jj new "$operation_base" -m mutation >/dev/null && printf 'mutation\n' >mutation.txt && jj commit -m mutation >/dev/null)
(cd "$operation_repo" && operation_mutation="$(rev_id "$operation_repo" '@-' commit_id)" && jj bookmark set restore-marker -r "$operation_mutation" >/dev/null)
mutated_marker="$(cd "$operation_repo" && jj log -r 'restore-marker' --no-graph -T 'commit_id ++ "\n"')"
[[ "$mutated_marker" != "$operation_base" ]] || fail_result 'operation-log-restore' 'fixture did not move the bookmark before restore'
(cd "$operation_repo" && jj op restore "$operation_id" >/dev/null)
restored_marker="$(cd "$operation_repo" && jj log -r 'restore-marker' --no-graph -T 'commit_id ++ "\n"')"
[[ "$restored_marker" == "$operation_base" ]] || fail_result 'operation-log-restore' 'operation restore did not restore the bookmark target'
pass_result 'operation-log-restore'

inject_or_continue 'local-tags'
tag_repo="$TEMP_ROOT/tag-repository"
repo_init "$tag_repo"
(cd "$tag_repo" && printf 'tagged\n' >tagged.txt && jj commit -m tagged >/dev/null)
tagged_commit="$(rev_id "$tag_repo" '@-' commit_id)"
(cd "$tag_repo" && jj tag set contract-baseline -r "$tagged_commit" >/dev/null)
tag_target="$(rev_id "$tag_repo" contract-baseline commit_id)"
[[ "$tag_target" == "$tagged_commit" ]] || fail_result 'local-tags' 'local tag did not point at the tagged commit'
record_identity tagged_commit "$tagged_commit"
pass_result 'local-tags'

printf 'Jujutsu contract fixtures passed (%s)\n' "$FIXTURE_IDENTITY"
