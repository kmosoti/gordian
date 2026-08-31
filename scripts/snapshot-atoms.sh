#!/usr/bin/env bash
# Capture the executable Atom registry only after the live graph and every registry
# projection have passed their coherence gates (G-502).
set -euo pipefail
cd "$(dirname "$0")/.."

out=artifacts/atoms/issues.json

command -v gh >/dev/null || {
  echo "gh not found — this script needs network and credentials" >&2
  exit 78
}
gh auth status >/dev/null 2>&1 || {
  echo "gh cannot authenticate; see agent-runbook.md section 6.1" >&2
  exit 78
}

# Keep the candidate outside the canonical path. Nothing may replace the committed
# projection until the candidate has passed all live and offline checks below.
staging_dir="$(mktemp -d "${TMPDIR:-/tmp}/gordian-atom-registry.XXXXXX")"
staged="$staging_dir/issues.json"
cleanup() {
  rm -rf -- "$staging_dir"
}
trap cleanup EXIT

echo "fetching the complete live Atom registry and native blocked-by graph…"
PYTHONPATH="orchestration/src${PYTHONPATH:+:$PYTHONPATH}" \
  python3.14 -m gordian_orchestration.atom_registry \
    --repository kmosoti/gordian capture --output "$staged"

# Capture already runs these gates against the live fetch. Repeat them against the exact
# staged bytes so a later read cannot silently validate a different graph than the one to
# be installed. The three registry checks cover core mirrors, EO17 obligations, and target
# crate contracts; the self-hosting check covers closure and orphan reachability.
registry=(python3.14 -m gordian_orchestration.atom_registry --repository kmosoti/gordian \
  --snapshot "$staged")
PYTHONPATH="orchestration/src${PYTHONPATH:+:$PYTHONPATH}" "${registry[@]}" check
PYTHONPATH="orchestration/src${PYTHONPATH:+:$PYTHONPATH}" "${registry[@]}" check-benchmarks
PYTHONPATH="orchestration/src${PYTHONPATH:+:$PYTHONPATH}" "${registry[@]}" check-target-crates
bash scripts/check-selfhosting-closure.sh --snapshot "$staged"

# Atomic replacement is the only write to the canonical projection, and it is reached
# only after every gate above succeeds. A failed capture leaves any prior snapshot intact.
mkdir -p -- "$(dirname "$out")"
mv -- "$staged" "$out"
echo "snapshot written and all registry gates pass."
