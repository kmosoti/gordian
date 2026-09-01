#!/usr/bin/env bash
# Atom #1's acceptance, as one executable: the checks its closure record cites as the
# `atom-1-acceptance` verifier.  The first record described this run in prose
# ("contract positive, injected-negative, and manifest-write-failure paths; committed
# manifest; ..."); a description binds nothing, so the record was re-closed with this
# script as the command.  Each block below is one bullet of artifacts/atoms/1/spec.md
# `## Acceptance`, in order; everything is asserted, nothing is described.
#
# Usage: bash experiments/jj-baseline/acceptance.sh
# Exit 0 only when every assertion holds.  Scratch output goes under a temporary
# directory that is removed on exit, never into the repository.
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
root="$(cd -- "$script_dir/../.." && pwd -P)"
cd "$root"

scratch="$(mktemp -d "${TMPDIR:-/tmp}/gordian-atom-1-acceptance-XXXXXX")"
trap 'rm -rf "$scratch"' EXIT

fail() { echo "FAIL: $*" >&2; exit 1; }
step() { echo "== $*"; }

# 1. The minimum supported release is documented in one place and the README points at it.
step "pinned release is single-sourced"
pin="$(awk -F'"' '/^readonly DEFAULT_JJ_VERSION=/ { print $2; exit }' scripts/bootstrap-jj.sh)"
[[ "$pin" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "scripts/bootstrap-jj.sh pins no DEFAULT_JJ_VERSION"
grep -q '^## Baseline rationale$' experiments/jj-baseline/README.md \
  || fail "README lacks the '## Baseline rationale' section"
grep -q 'DEFAULT_JJ_VERSION' experiments/jj-baseline/README.md \
  || fail "README does not name DEFAULT_JJ_VERSION as the pin's source"
if grep -qF "$pin" experiments/jj-baseline/README.md; then
  fail "README restates the pinned version $pin instead of pointing at the pin"
fi

# 2. The bootstrap is reproducible: the installed jj is the pinned release.
step "installed jj matches the pin"
bash scripts/bootstrap-jj.sh --install-only >/dev/null || fail "bootstrap-jj.sh --install-only failed"
installed="$(jj --version | awk 'NR == 1 { print $2 }')"
[ "$installed" = "$pin" ] || fail "jj $installed is installed, pin is $pin"

# 3. The contract fixtures cover the listed semantics and pass against a disposable repository.
step "contract fixtures pass and match the committed manifest"
bash experiments/jj-baseline/contract.sh --manifest "$scratch/positive.json" > "$scratch/positive.log" 2>&1 \
  || { cat "$scratch/positive.log"; fail "contract.sh failed"; }
python3 - "$scratch/positive.json" experiments/jj-baseline/manifest.json "$pin" <<'PY'
import json
import sys

fresh_path, committed_path, pin = sys.argv[1:]
fresh = json.load(open(fresh_path, encoding="utf-8"))
committed = json.load(open(committed_path, encoding="utf-8"))
required = {
    "version-and-run-capabilities",  # `jj run` presence; its semantics belong to #33
    "machine-readable-commit-id",
    "machine-readable-change-id",
    "change-id-rewrite",
    "sibling-topology",
    "causal-parent-child-topology",
    "workspace-isolation",
    "multi-parent-integration",
    "conflict-persistence-and-repair",
    "operation-log-restore",
    "local-tags",
}
problems = []
for label, manifest in (("fresh", fresh), ("committed", committed)):
    names = [result["name"] for result in manifest["pass_results"]]
    if manifest.get("passed") is not True or manifest.get("exit_code") != 0:
        problems.append(f"{label} manifest did not pass")
    if set(names) != required:
        problems.append(f"{label} manifest fixtures {sorted(names)} != {sorted(required)}")
    if len(names) != len(set(names)):
        problems.append(f"{label} manifest repeats a fixture")
    failed = [r["name"] for r in manifest["pass_results"] if r.get("passed") is not True]
    if failed:
        problems.append(f"{label} manifest has failed fixtures {failed}")
    if manifest.get("expected_jj_version") != pin:
        problems.append(f"{label} manifest expects {manifest.get('expected_jj_version')!r}, pin is {pin!r}")
    if pin not in manifest.get("jj_version_output", ""):
        problems.append(f"{label} manifest recorded {manifest.get('jj_version_output')!r}, not the pin")
    if not manifest.get("jj_binary", {}).get("sha256"):
        problems.append(f"{label} manifest records no jj binary digest")
if problems:
    print("\n".join("FAIL: " + p for p in problems))
    raise SystemExit(1)
print(f"ok: {len(required)} fixtures passed fresh and in the committed manifest at jj {pin}")
PY

# 4. The negative path is live: an injected fixture failure is reported, not absorbed.
step "injected fixture failure fails"
if GORDIAN_JJ_CONTRACT_INJECT_FAILURE=change-id-rewrite \
  bash experiments/jj-baseline/contract.sh --manifest "$scratch/negative.json" > "$scratch/negative.log" 2>&1; then
  fail "contract.sh exited 0 with an injected failure"
fi
grep -q 'contract fixture failed: change-id-rewrite' "$scratch/negative.log" \
  || { cat "$scratch/negative.log"; fail "the injected failure was not named"; }
python3 - "$scratch/negative.json" <<'PY'
import json
import sys

manifest = json.load(open(sys.argv[1], encoding="utf-8"))
if manifest.get("passed") is not False or manifest.get("exit_code") == 0:
    raise SystemExit("FAIL: negative manifest claims success")
print("ok: negative manifest records the failure")
PY

# 5. A manifest that cannot be written is a failure, not a pass without a record.  The
#    path is a directory, so the write fails for any user; the fixtures themselves pass.
step "unwritable manifest path fails"
mkdir -p "$scratch/blocked/manifest.json"
if bash experiments/jj-baseline/contract.sh --manifest "$scratch/blocked/manifest.json" > "$scratch/unwritable.log" 2>&1; then
  fail "contract.sh exited 0 without writing its manifest"
fi
[ -d "$scratch/blocked/manifest.json" ] || fail "the blocking directory was replaced"

# 6. Unsupported behavior is explicit, in the README's own words.
step "unsupported behavior is stated"
grep -q '^## Constraints$' experiments/jj-baseline/README.md || fail "README lacks '## Constraints'"
grep -q 'does not promise' experiments/jj-baseline/README.md \
  || fail "README Constraints does not say what the qualification does not promise"
grep -qF 'The #33 Atom owns' experiments/jj-baseline/README.md \
  || fail "README does not assign jj-run semantics to #33"

# 7. The benchmark harness runs in its bounded mode and validates its own output.
step "benchmark smoke run"
python3 experiments/jj-baseline/benchmark.py --smoke --output "$scratch/smoke.json" > "$scratch/smoke.log" 2>&1 \
  || { cat "$scratch/smoke.log"; fail "benchmark.py --smoke failed"; }
python3 - "$scratch/smoke.log" <<'PY'
import json
import sys

summary = json.loads(open(sys.argv[1], encoding="utf-8").read().strip().splitlines()[-1])
if summary.get("valid") is not True or summary.get("failures") != 0:
    raise SystemExit(f"FAIL: smoke summary {summary!r}")
print("ok: smoke run valid with no failures")
PY

# 8. The registered experiment manifests and the statistical contract still check out.
step "experiment registry checks"
bash scripts/check-experiment-manifests.sh
bash scripts/check-statistical-contract.sh

echo "OK: Atom #1 acceptance holds for jj $pin"
