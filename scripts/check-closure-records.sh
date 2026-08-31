#!/usr/bin/env bash
# Every artifacts/atoms/*/closure.json validates against the closure schema and every
# verifier artifact_sha256 matches the exact file named by the record. Attempt records
# use the same shared validator and retain the runbook's cross-field checks.
#
# closure.json is excluded from artifact digest checks: a record cannot contain its own
# digest. The coordinator writes it after admission in a bookkeeping change.
#
# Usage: check-closure-records.sh [ROOT]
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
root="${1:-$(cd "$script_dir/.." && pwd)}"
cd "$root"

closure_schema="artifacts/schema/closure-record.schema.json"
attempt_schema="artifacts/schema/attempt-record.schema.json"
for required in "$closure_schema" "$attempt_schema"; do
  if [ ! -f "$required" ]; then
    echo "FAIL: $required is missing; the record has no normative definition"
    exit 1
  fi
done

# Keep the checker usable on focused fixture roots: only schemas and records are copied
# there, while the validator itself remains in this checkout.
validator_root="$(cd "$script_dir/../orchestration/src" && pwd)"
PYTHONPATH="$validator_root${PYTHONPATH:+:$PYTHONPATH}" \
  python3 -m gordian_orchestration.closure_validation "$PWD"
