#!/usr/bin/env bash
# No predicate body may read a wall clock.
#
# Every fenced block in docs/spec/mission-graph.md, docs/algorithms/scheduling.md, and
# docs/algorithms/evidence-and-admission.md is a predicate or algorithm body; none of them may
# contain the bare token `now`. Lease and capability liveness is EventSeq-denominated
# (expires_at_event, LeaseExpired, CapabilityExpired) everywhere.
set -euo pipefail
cd "$(dirname "$0")/.."

files=(
  docs/spec/mission-graph.md
  docs/algorithms/scheduling.md
  docs/algorithms/evidence-and-admission.md
)

present=0
fail=0

for file in "${files[@]}"; do
  [ -f "$file" ] || continue
  present=$((present + 1))
  hits=$(awk '
    /^```/ { fenced = !fenced; next }
    fenced && $0 ~ /(^|[^A-Za-z0-9_.])now([^A-Za-z0-9_]|$)/ { printf "%d: %s\n", NR, $0 }
  ' "$file")
  if [ -n "$hits" ]; then
    echo "FAIL: wall-clock token 'now' inside a predicate body in $file:"
    printf '%s\n' "$hits" | sed 's/^/       /'
    fail=1
  fi
done

if [ "$present" -eq 0 ]; then
  echo "SKIP: none of the predicate documents are present."
  exit 0
fi

# Liveness must be event-denominated where it is stated at all.
if grep -rqs 'expires_at' "${files[@]}" && ! grep -rqs 'expires_at_event' "${files[@]}"; then
  echo "FAIL: lease/capability expiry is stated without an EventSeq-denominated expires_at_event"
  fail=1
fi

[ "$fail" -eq 0 ] && echo "OK: no predicate body reads a wall clock."
exit $fail
