#!/usr/bin/env bash
# Run the Stage B manifests one after another through the driver (the order they were run in).
# Needs: make_manifests.py already run, a clean tree whose HEAD is each manifest's source_revision,
# the release binary built, no build in progress.
W="${GORDIAN_WORK:-/tmp/gordian-exploration}"
cd "$(dirname "${BASH_SOURCE[0]}")/../../.." || exit 1
for m in pilot c20000000 c250000 c100000 c60000 s1-c20000000 s1-c250000 s1-c100000 s1-c60000 \
         s2-c15000 s2-c20000 s2-c30000 s2-c40000 s2-c50000 s3-c20000000 s3-c250000 s3-c100000 s3-c60000; do
  start=$(date +%s)
  scripts/run-driver.sh --manifest "$W/manifests/$m.json" > "$W/log-$m.txt" 2>&1
  status=$?
  end=$(date +%s)
  echo "$m exit=$status wall_s=$((end - start))" >> "$W/run_all.log"
done
