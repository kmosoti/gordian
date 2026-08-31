#!/usr/bin/env bash
# Write the exact-revision formal evidence artifact after the formal gate succeeds.
set -euo pipefail

state="${1:?usage: scripts/write-formal-evidence.sh EXACT_STATE_ID OUTPUT}"
output="${2:?usage: scripts/write-formal-evidence.sh EXACT_STATE_ID OUTPUT}"
[[ "$state" =~ ^[0-9a-f]{40,64}$ ]] || { echo "invalid exact state id: $state" >&2; exit 64; }

root="$(cd "$(dirname "$0")/.." && pwd)"
command -v realpath >/dev/null || { echo "MISSING TOOL: realpath" >&2; exit 78; }
output="$(realpath -m "$output")"
case "$output" in
  "$root" | "$root"/*)
    echo "evidence output must be outside the repository: $output" >&2
    exit 1
    ;;
esac
if [ -e "$root/.jj" ]; then
  command -v jj >/dev/null || {
    echo "cannot bind evidence: JJ metadata exists but jj is unavailable" >&2
    exit 78
  }
  # JJ snapshots the workspace tree into @. That exact commit is the Candidate
  # subject; it need not be in the immutable revset, because evidence remains
  # valid only for this commit ID and a later rewrite receives a new ID.
  if ! observed_state="$(jj -R "$root" log --no-graph -r '@' -T 'commit_id ++ "\n"')"; then
    echo "cannot bind evidence: jj could not resolve the working-copy commit" >&2
    exit 78
  fi
else
  command -v git >/dev/null || {
    echo "cannot bind evidence: no JJ metadata and read-only git is unavailable" >&2
    exit 78
  }
  if ! observed_state="$(git -C "$root" rev-parse --verify 'HEAD^{commit}')"; then
    echo "cannot bind evidence: git could not resolve the checked-out commit" >&2
    exit 78
  fi
  if ! git_status="$(git -C "$root" status --porcelain --untracked-files=all)"; then
    echo "cannot bind evidence: git could not inspect worktree cleanliness" >&2
    exit 78
  fi
  [ -z "$git_status" ] || {
    echo "cannot bind evidence: git worktree is dirty" >&2
    exit 1
  }
fi
[[ "$observed_state" =~ ^[0-9a-f]{40,64}$ ]] || {
  echo "cannot bind evidence: repository returned an invalid exact state id: $observed_state" >&2
  exit 78
}
[ "$state" = "$observed_state" ] || {
  echo "exact state mismatch: requested $state, checked out $observed_state" >&2
  exit 1
}

# Evidence is a claim about a verifier result, not a metadata assertion. Run the
# complete gate (including its negative self-tests) before collecting metadata
# or creating the output path. The self-test's forged-state probe terminates at
# the exact-state check above, so it cannot recurse into this gate indefinitely.
if ! (cd "$root" && bash "$root/scripts/verify-formal.sh" --self-test); then
  echo "cannot write formal evidence: complete formal gate failed" >&2
  exit 1
fi

# Re-bind after verification: a source rewrite or dirty worktree during the
# gate must invalidate the evidence rather than being silently included.
if [ -e "$root/.jj" ]; then
  observed_after="$(jj -R "$root" log --no-graph -r '@' -T 'commit_id ++ "\n"')"
  after_status=""
else
  observed_after="$(git -C "$root" rev-parse --verify 'HEAD^{commit}')"
  after_status="$(git -C "$root" status --porcelain --untracked-files=all)"
fi
[ "$observed_after" = "$state" ] || {
  echo "exact state changed while formal gate ran: was $state, now $observed_after" >&2
  exit 1
}
[ -z "$after_status" ] || {
  echo "source state became dirty while formal gate ran" >&2
  exit 1
}

lean_toolchain="$(tr -d '\r\n' < "$root/formal/lean-toolchain")"
lean_version="$(cd "$root/formal" && lake env lean --version | sed -n 's/^Lean (version \([^,]*\).*/\1/p')"
checker="$(cd "$root/formal" && lake env bash -c 'command -v leanchecker')"
checker_sha256="$(sha256sum "$checker" | awk '{print $1}')"
audit_sha256="$(sha256sum "$root/formal/Gordian/Audit.lean" | awk '{print $1}')"

mkdir -p "$(dirname "$output")"
python3 - "$output" "$state" "$lean_toolchain" "$lean_version" "$checker_sha256" \
  "$audit_sha256" "${GITHUB_RUN_ID:-local}" "${GITHUB_RUN_ATTEMPT:-1}" <<'PY'
import json
import sys

output, state, toolchain, version, checker_digest, audit_digest, run_id, attempt = sys.argv[1:]
record = {
    "record_format": "gordian-formal-evidence/1",
    "exact_state_id": state,
    "lean_toolchain": toolchain,
    "lean_version": version,
    "verdict": "pass",
    "checks": [
        {"id": "lake-build-warning-as-error", "verdict": "pass"},
        {
            "id": "leanchecker-environment-replay",
            "verdict": "pass",
            "binary_sha256": checker_digest,
        },
        {
            "id": "allowlisted-axiom-audit",
            "verdict": "pass",
            "source_sha256": audit_digest,
        },
    ],
    "github_run_id": run_id,
    "github_run_attempt": int(attempt),
}
with open(output, "w", encoding="utf-8") as handle:
    json.dump(record, handle, indent=2, sort_keys=True)
    handle.write("\n")
PY

echo "wrote formal evidence for $state to $output"
