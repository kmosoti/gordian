#!/usr/bin/env bash
# Run B5 manifests one after another through the driver, logging wall time per run.
#
# Usage: [OUT_PREFIX=p-] experiments/exploration/scripts/b5_run.sh MANIFEST...
# Needs a clean tree whose HEAD is each manifest's source_revision, the release binary built from
# that tree, and no build in progress. This machine is shared with other workers: the driver
# refuses while any cargo or rustc process exists, and this script waits for it, and for any other
# worker's gordian-run or benchmark, polling every 30 s and logging each wait (the patterns are
# `pgrep -x`, an exact process name, and a bracketed pattern, neither of which can match this shell).
# The driver does the isolation (scripts/cgroup-run.sh, cores 0-2, driver on core 3). Run ids, exit
# statuses and wall times go to artifacts/runs/_logs/b5-runs.log; each run's stderr to
# artifacts/runs/_logs/<run_id>.log. With OUT_PREFIX set, a run's directory is
# artifacts/runs/<OUT_PREFIX><run_id> (used to replay R6's manifest under its own run id without
# touching any other directory). A run that fails is logged with its exit status and the next one
# still runs; nothing is deleted.
set -u
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$root" || exit 1
mkdir -p artifacts/runs/_logs
for m in "$@"; do
  id="$(jq -r .run_id "$m")"
  out="artifacts/runs/${OUT_PREFIX:-}$id"
  waited=0
  while pgrep -x cargo >/dev/null || pgrep -x rustc >/dev/null || pgrep -x gordian-run >/dev/null || pgrep -f '[b]ench' >/dev/null; do
    echo "$(date +%T) ${OUT_PREFIX:-}$id waiting-for-other-process" >> artifacts/runs/_logs/b5-runs.log
    waited=$((waited + 1))
    sleep 30
  done
  start=$(date +%s)
  scripts/run-driver.sh --manifest "$m" --out "$out" > "artifacts/runs/_logs/${OUT_PREFIX:-}$id.log" 2>&1
  status=$?
  end=$(date +%s)
  echo "${OUT_PREFIX:-}$id exit=$status wall_s=$((end - start)) waits=$waited" >> artifacts/runs/_logs/b5-runs.log
done
