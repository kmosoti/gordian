#!/usr/bin/env bash
# Every member of project_integration_verifiers names an adapter-executable command, never a
# CI workflow job name. A verifier that cannot be executed by the source adapter cannot be a
# member: admission needs the evidence before the compare-and-swap, and a workflow job on the
# published branch cannot produce evidence about an unpublished integration state.
set -euo pipefail
cd "$(dirname "$0")/.."

landing=docs/protocols/landing.md
workflows=.github/workflows

if [ ! -f "$landing" ] || ! grep -q '^## 3\. Verifiers and CI status' "$landing"; then
  echo "SKIP: $landing has no verifier list yet."
  exit 0
fi

members=$(awk '
  /^## 3\. Verifiers and CI status/ { inside = 1; next }
  inside && /^## / { exit }
  inside && /^```/ { fence++; if (fence == 2) exit; next }
  inside && fence == 1 && /^verifier:/ { print }
' "$landing")

if [ -z "$members" ]; then
  echo "FAIL: $landing section 3 lists no project_integration_verifiers members"
  exit 1
fi

# Job ids and human-readable job names declared by the workflows.
job_names=""
if [ -d "$workflows" ]; then
  job_names=$(grep -rhE '^  [a-z][a-z0-9_-]*:|^    name: ' "$workflows" 2>/dev/null \
    | sed -e 's/^  //' -e 's/^  *name: //' -e 's/:$//' -e 's/^"//' -e 's/"$//' \
    | sed 's/[[:space:]]*$//' | grep -v '^$' || true)
fi

fail=0
while IFS= read -r member; do
  id=${member%% *}
  command=${member#"$id"}
  command=$(printf '%s' "$command" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')
  if [ -z "$command" ]; then
    echo "FAIL: $id names no command"
    fail=1
    continue
  fi
  # One rule for what counts as a command, shared with closure_validation.py: a shell word, a
  # pinned tool, or a repository-relative path that exists here. `every scripts/check-*.sh` and
  # `true` are not commands under it.
  if ! problem=$(PYTHONPATH="orchestration/src${PYTHONPATH:+:$PYTHONPATH}" python3 - "$command" <<'PY'
import sys
from pathlib import Path

from gordian_orchestration.closure_validation import executable_command_problem


def read(relative):
    path = Path(relative)
    return path.read_bytes() if path.is_file() else None


problem = executable_command_problem(sys.argv[1], read)
if problem is not None:
    print(problem)
    raise SystemExit(1)
PY
  ); then
    echo "FAIL: $id: ${problem:-command rule could not be evaluated}: $command"
    fail=1
  fi
  while IFS= read -r job; do
    [ -n "$job" ] || continue
    if [ "$(printf '%s' "$command" | tr '[:upper:]' '[:lower:]')" = \
         "$(printf '%s' "$job" | tr '[:upper:]' '[:lower:]')" ]; then
      echo "FAIL: $id names the CI job '$job' rather than a command"
      fail=1
    fi
  done <<EOF
$job_names
EOF
done <<EOF
$members
EOF

[ "$fail" -eq 0 ] && echo "OK: every integration verifier names an adapter-executable command."
exit $fail
