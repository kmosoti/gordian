#!/usr/bin/env bash
# W3: write the hidden-side tables for world C and the floor features for worlds A and C.
#
# Usage: experiments/exploration/scripts/w3_hidden.sh [OUT_ROOT]    (default artifacts/runs/w3/hidden)
#
# Builds nothing: it runs the `laws` example binary that
#   scripts/cgroup-run.sh --name world-c-build --cpus 0-2 --memory 3G -- \
#     cargo build -p gordian-stream --features reveal-hidden-state --example laws
# made, through the isolation runner, one range at a time.
#   c-tune     60000-60099  world C (hard share x3), with owners and floor features
#   c-heldout  70000-70199  world C, with owners and floor features
#   a-tune     10000-10099  world A (the defaults), floor features (incidents.csv must equal W2's)
#   a-heldout  40000-40199  world A, floor features (same check)
#   a-a2       10000-10019  world A, alarms and graph for scoring A2's smoke
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
out="${1:-$root/artifacts/runs/w3/hidden}"
bin="$root/target/debug/examples/laws"
[ -x "$bin" ] || { echo "w3_hidden: $bin is missing; build it first (no run in progress)" >&2; exit 21; }
run() { # name seed-from count world extra...
  local name="$1" from="$2" count="$3" world="$4"; shift 4
  local dir="$out/$name"
  mkdir -p "$dir"
  [ ! -e "$dir/incidents.csv" ] || { echo "w3_hidden: $dir already holds tables; not overwriting" >&2; exit 14; }
  "$root/scripts/cgroup-run.sh" --name "world-c-run" --cpus 0-2 --memory 2G \
    --report "$dir/usage.json" -- \
    "$bin" --seed-from "$from" --count "$count" --world "$world" --out-dir "$dir" "$@"
}
run c-tune    60000 100 c --owners --floor
run c-heldout 70000 200 c --owners --floor
run a-tune    10000 100 a --floor
run a-heldout 40000 200 a --floor
run a-a2      10000 20  a --floor --alarms
