#!/usr/bin/env bash
# W2: write the hidden-side tables of the stream world's learnable laws for the three seed ranges.
#
# Usage: experiments/exploration/scripts/w2_hidden.sh [OUT_ROOT]    (default artifacts/runs/w2/hidden)
#
# Builds nothing: it runs the `laws` example binary that `cargo build -p gordian-stream
# --features reveal-hidden-state --example laws` made, through the isolation runner, one range at
# a time. A world-A range also writes `owners.jsonl` (the join key for the ceiling's trace).
# The three ranges are the brief's: 10000-10099 and 40000-40199 at the defaults (world A) and
# 50000-50099 with recurrence 0.6 and regime changes at 150 s and 300 s (world B).
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
out="${1:-$root/artifacts/runs/w2/hidden}"
bin="$root/target/debug/examples/laws"
[ -x "$bin" ] || { echo "w2_hidden: $bin is missing; build it first (no run in progress)" >&2; exit 21; }
run() { # name seed-from count world owners
  local name="$1" from="$2" count="$3" world="$4" owners="$5"
  local dir="$out/$name"
  mkdir -p "$dir"
  [ ! -e "$dir/incidents.csv" ] || { echo "w2_hidden: $dir already holds tables; not overwriting" >&2; exit 14; }
  "$root/scripts/cgroup-run.sh" --name "world-laws-run" --cpus 0-2 --memory 2G \
    --report "$dir/usage.json" -- \
    "$bin" --seed-from "$from" --count "$count" --world "$world" --out-dir "$dir" $owners
}
run a-tune    10000 100 a --owners
run a-heldout 40000 200 a --owners
run b         50000 100 b ""
