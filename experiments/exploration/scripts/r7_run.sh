#!/usr/bin/env bash
# Run R7 manifests one after another through the driver, logging wall time per run.
#
# Usage: [OUT_PREFIX=p-] r7_run.sh MANIFEST...
# Needs a clean tree whose HEAD is each manifest's source_revision, the release binary built from
# that tree, and no build in progress. The driver does the isolation (scripts/cgroup-run.sh, cores
# 0-2, driver on core 3). Run ids and wall times go to artifacts/runs/_logs/r7-runs.log; each run's
# stderr to artifacts/runs/_logs/<run_id>.log. With OUT_PREFIX set, a run's directory is
# artifacts/runs/<OUT_PREFIX><run_id> (used to replay an R6 manifest under its own run id without
# touching any other directory). A run that fails is logged with its exit status and the next one
# still runs; nothing is deleted.
set -u
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$root" || exit 1
mkdir -p artifacts/runs/_logs
for m in "$@"; do
  id="$(jq -r .run_id "$m")"
  out="artifacts/runs/${OUT_PREFIX:-}$id"
  start=$(date +%s)
  scripts/run-driver.sh --manifest "$m" --out "$out" > "artifacts/runs/_logs/${OUT_PREFIX:-}$id.log" 2>&1
  status=$?
  end=$(date +%s)
  echo "${OUT_PREFIX:-}$id exit=$status wall_s=$((end - start))" >> artifacts/runs/_logs/r7-runs.log
done
