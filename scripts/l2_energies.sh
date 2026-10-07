#!/usr/bin/env bash
# Run the reservoir_energies tool (crates/gordian-run/tests/stream_reservoir.rs) under the runner.
#
# Usage: scripts/l2_energies.sh SEEDS OUT NOTICER_JSON
#   SEEDS         first:count of seeds, in stream order
#   OUT           the CSV to write
#   NOTICER_JSON  the manifest's JSON for one reservoir noticer
#
# Needs the release test binary (cargo test --release -p gordian-run --test stream_reservoir
# --no-run). Public observations and the noticer's own state only; no evaluator output is read.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bin="$(ls -t "$root"/target/release/deps/stream_reservoir-* | grep -v '\.d$' | head -1)"
L2_SEEDS="$1" L2_OUT="$2" L2_NOTICER="$3" "$root/scripts/cgroup-run.sh" --name lab3-energies \
  --cpus 0-2 --memory 2G -- "$bin" --ignored reservoir_energies
