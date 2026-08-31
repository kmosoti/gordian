#!/usr/bin/env bash
# Bootstrap credentials fail closed and never initiate interactive authentication.
set -euo pipefail
cd "$(dirname "$0")/.."

module=orchestration/src/gordian_orchestration/bootstrap_claims.py
gh_module=orchestration/src/gordian_orchestration/gh.py
for required in "$module" "$gh_module"; do
  if [ ! -f "$required" ]; then
    echo "FAIL: $required is missing"
    exit 1
  fi
done

if grep -REn 'run_gh\(\["auth", "(login|refresh)"' orchestration/src >/dev/null; then
  echo "FAIL: orchestration invokes interactive gh authentication"
  grep -REn 'run_gh\(\["auth", "(login|refresh)"' orchestration/src
  exit 1
fi

if ! grep -q 'LEGACY_ADOPTION_ATOM = 70' "$module" || ! grep -q -- '--adopt-legacy' "$module"; then
  echo "FAIL: Atom 70 legacy-claim adoption path is missing"
  exit 1
fi

if ! grep -q 'run_gh_json_response' "$gh_module"; then
  echo "FAIL: GitHub JSON boundary does not preserve response metadata"
  exit 1
fi

gh_config_dir="$(mktemp -d "${TMPDIR:-/tmp}/gordian-gh-config.XXXXXX")"
trap 'rm -rf -- "$gh_config_dir"' EXIT

set +e
message="$({ env -u GORDIAN_GH_TOKEN -u GH_TOKEN -u GITHUB_TOKEN -u GH_ENTERPRISE_TOKEN \
  GH_CONFIG_DIR="$gh_config_dir" PYTHONPATH=orchestration/src \
  python3 -m gordian_orchestration.bootstrap_claims preflight; } 2>&1)"
code=$?
set -e
if [ "$code" -ne 78 ]; then
  echo "FAIL: isolated-credential preflight exited $code rather than 78"
  echo "$message"
  exit 1
fi
if [[ "$message" != *"GORDIAN_GH_TOKEN must be set to a non-empty token"* ]]; then
  echo "FAIL: isolated-credential preflight did not report the required credential"
  echo "$message"
  exit 1
fi

echo "OK: bootstrap preflight rejects isolated credentials with exit 78 and has no interactive auth path."
