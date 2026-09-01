#!/usr/bin/env bash
# Run one Mission acceptance witness.
#
# Each row of the Mission acceptance table (docs/implementation/project-plan.md) names a
# witness: the executable check that demonstrates the row on a clean installation.  The
# closure records of the row's Atoms say the parts were built; the witness says they do what
# the row claims, together.  A row is satisfied only when every cited Atom has a validating
# closure record AND artifacts/atoms/69/closure.json carries a verifier for the witness whose
# command is exactly `bash scripts/mission-witness.sh <id>` — so the witness's own log, bound
# to the state it ran on, is the row's evidence.  Rows 1-18 were originally gated by closure
# records alone; a row whose Atoms were all closed was "demonstrated" by nothing.
#
# Usage: scripts/mission-witness.sh <witness-id>   run it; exit 0 means the row holds
#        scripts/mission-witness.sh --list          one line per witness: "<id> implemented"
#                                                   or "<id> pending #N"
#
# A pending witness cannot be written yet because Atom #N of its row has no closure record;
# it exits 3 without running anything, so it can never produce a green log.  When #N closes,
# scripts/check-mission-acceptance.sh fails until the witness is implemented here.  The same
# checker keeps `--list` and the table's Witness column equal as sets.
#
# Deliberately not named check-*.sh: scripts/verify-local.sh runs every check-*.sh on every
# push, and a witness is run when a row is being closed, not on every push.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd -P)"
cd "$root"

# id, status, pending Atom (for pending only).  One entry per acceptance row, in row order.
witnesses=(
  "typed-graph-interface pending 9"
  "invalid-decomposition-rejected pending 10"
  "deterministic-rebuild pending 12"
  "derived-readiness pending 13"
  "heterogeneous-scheduling pending 20"
  "isolated-workspaces pending 29"
  "change-and-state-identity pending 31"
  "resource-claims-and-leases pending 22"
  "provenance-bound-evidence pending 16"
  "stale-evidence-invalidation pending 15"
  "explicit-integration pending 32"
  "worker-authority-bounded pending 18"
  "frontier-promotion-cas pending 19"
  "replay-without-repeats pending 11"
  "cli-api-surfaces pending 44"
  "self-hosting-proof pending 48"
  "hypothesis-evidence pending 34"
  "knowledge-graph-sync pending 8"
  "toolchain-baseline implemented"
  "workload-baselines pending 4"
  "verification-stack pending 6"
  "sandboxed-workers pending 35"
  "distributed-robustness pending 40"
  "planning-reconciliation pending 56"
  "release-qualification pending 64"
  "bootstrap-mirror implemented"
)

# Row 19: the pinned Jujutsu, Rust, Lean, Python, and GitHub CLI baselines are what is
# installed, the jj contract holds against a disposable repository, and every verifier group
# passes on them — the same `scripts/verify-local.sh` string CI and the Atom records use.
# shellcheck disable=SC2317  # dispatched by name from the registry below
witness_toolchain_baseline() {
  bash scripts/check-toolchain.sh
  bash scripts/check-toolchain.sh --runtime
  bash experiments/jj-baseline/acceptance.sh
  bash scripts/verify-local.sh all
}

# Row 26: the Atom backlog is mirrored into GitHub Project 9.  Dry run: reads the project
# and the repository and reports the reconciliation it would apply; converged means nothing
# is missing.  Needs a GitHub token with the project scope, like the Atom #70 record's run.
# shellcheck disable=SC2317  # dispatched by name from the registry below
witness_bootstrap_mirror() {
  local report
  report="$(PYTHONPATH=orchestration/src python3.14 -m gordian_orchestration.github_project --dry-run)"
  printf '%s\n' "$report"
  python3 - "$report" <<'PY'
import json
import sys

report = json.loads(sys.argv[1])
if report.get("dry_run") is not True:
    raise SystemExit("FAIL: the witness must be a dry run")
missing = report.get("missing_before") or []
if missing:
    raise SystemExit(f"FAIL: {len(missing)} open issue(s) are absent from the project: {missing}")
if report.get("failed_urls"):
    raise SystemExit(f"FAIL: failed urls {report['failed_urls']}")
print(f"ok: {report.get('open_issue_count')} open issues all present in project {report.get('project_number')}")
PY
}

usage() {
  echo "usage: scripts/mission-witness.sh <witness-id> | --list" >&2
  exit 2
}

[ "$#" -eq 1 ] || usage
case "$1" in
  --list)
    for entry in "${witnesses[@]}"; do
      read -r id status atom <<<"$entry"
      if [ "$status" = implemented ]; then
        printf '%s implemented\n' "$id"
      else
        printf '%s pending #%s\n' "$id" "$atom"
      fi
    done
    exit 0
    ;;
  -*|"")
    usage
    ;;
esac

wanted="$1"
for entry in "${witnesses[@]}"; do
  read -r id status atom <<<"$entry"
  [ "$id" = "$wanted" ] || continue
  if [ "$status" != implemented ]; then
    echo "PENDING: witness $id awaits Atom #$atom; nothing to run" >&2
    exit 3
  fi
  "witness_${id//-/_}"
  echo "OK: witness $id holds"
  exit 0
done
echo "FAIL: unknown witness $wanted; see scripts/mission-witness.sh --list" >&2
exit 2
