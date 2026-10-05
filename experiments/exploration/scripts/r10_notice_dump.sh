#!/usr/bin/env bash
# Play the shared rung over R10's streams and write what it noticed (the `r10_notices` example),
# one dump per manifest, each under scripts/cgroup-run.sh on cores 0-2.
#
# Usage: r10_notice_dump.sh
# Needs the release example built from the tree (`cargo build --release --example r10_notices`),
# no build in progress (it waits for one, polling every 30 s), and the manifests of the runs it
# replays in artifacts/runs/_manifests. A dump's stderr and the runner's report go beside it.
set -u
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$root" || exit 1
mkdir -p artifacts/runs/r10-notices
bin=target/release/examples/r10_notices
m=artifacts/runs/_manifests
# name manifest first-seed count
jobs=(
  "diag $m/r10-diag-b5-rho0.7.json 32000 100"
  "heldout-z3 $m/r10-heldout-b5-rho0.7.json 20000 200"
  "sweep-z2 $m/r10-sweep-z2-b5-rho0.7.json 20000 200"
  "sweep-z1 $m/r10-sweep-z1-b5-rho0.7.json 20000 200"
  "sweep-z0.5 $m/r10-sweep-z0.5-b5-rho0.7.json 20000 200"
)
for job in "${jobs[@]}"; do
  read -r name manifest first count <<<"$job"
  out="artifacts/runs/r10-notices/$name.jsonl"
  if [ -e "$out" ]; then echo "$name exists; kept"; continue; fi
  while pgrep -x cargo >/dev/null || pgrep -x rustc >/dev/null; do
    echo "$name waiting-for-build" >> artifacts/runs/_logs/r10-runs.log
    sleep 30
  done
  start=$(date +%s)
  scripts/cgroup-run.sh --name "r10-notices-$name" --cpus 0-2 --cpu-quota 300 --memory 2G \
    --report "artifacts/runs/r10-notices/$name.usage.json" -- \
    "$bin" --manifest "$manifest" --first "$first" --count "$count" \
    > "$out.partial" 2> "artifacts/runs/r10-notices/$name.log"
  status=$?
  end=$(date +%s)
  if [ "$status" -eq 0 ]; then mv "$out.partial" "$out"; fi
  echo "r10-notices-$name exit=$status wall_s=$((end - start))" >> artifacts/runs/_logs/r10-runs.log
done
