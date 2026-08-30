#!/usr/bin/env bash
# The seven effect classes are stated in three places, and the three must be the same set:
#
#   docs/spec/data-model.md              the `enum EffectClass` variants
#   docs/algorithms/reconciliation.md    the rows of section 8
#   formal/Gordian/EffectClass.lean      the arms of `retryPolicy`
#
# Cardinality is checked too, so a silently added or dropped class fails.
set -euo pipefail
cd "$(dirname "$0")/.."

model=docs/spec/data-model.md
reconciliation=docs/algorithms/reconciliation.md
lean=formal/Gordian/EffectClass.lean

to_snake() {
  python3 -c '
import re, sys
for raw in sys.stdin:
    name = raw.strip()
    if not name:
        continue
    name = re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()
    print(name)
' | sort -u
}

model_classes=""
if [ -f "$model" ]; then
  model_classes=$(awk '
    /enum EffectClass[[:space:]]*\{/ { inside = 1; next }
    inside && /\}/ { exit }
    inside { gsub(/[[:space:],]/, ""); if (length($0)) print }
  ' "$model" | to_snake)
fi

reconciliation_classes=""
if [ -f "$reconciliation" ]; then
  reconciliation_classes=$(awk '
    /^## 8\./ { inside = 1; next }
    inside && /^## / { exit }
    inside && /^\| `[a-z_]+` \|/ { print }
  ' "$reconciliation" | sed -e 's/^| `//' -e 's/`.*$//' | to_snake)
fi

lean_classes=""
if [ -f "$lean" ]; then
  lean_classes=$(awk '
    /^def retryPolicy/ { inside = 1; next }
    inside && /^[[:space:]]*\|/ {
      arm = $0
      sub(/^[[:space:]]*\|[[:space:]]*\./, "", arm)
      sub(/[[:space:]]*=>.*$/, "", arm)
      print arm
      next
    }
    inside { exit }
  ' "$lean" | to_snake)
fi

if [ -z "$model_classes" ] && [ -z "$reconciliation_classes" ] && [ -z "$lean_classes" ]; then
  echo "SKIP: no EffectClass list at any of the three sites yet."
  exit 0
fi

fail=0
require_seven() {
  local label=$1 list=$2
  local count
  count=$(printf '%s\n' "$list" | grep -c . || true)
  if [ "$count" -ne 7 ]; then
    echo "FAIL: $label names $count effect classes, expected 7"
    fail=1
  fi
}
require_seven "$model (enum EffectClass)" "$model_classes"
require_seven "$reconciliation (section 8)" "$reconciliation_classes"
require_seven "$lean (retryPolicy)" "$lean_classes"

compare() {
  if [ "$2" != "$3" ]; then
    echo "FAIL: effect class drift: $1"
    diff <(printf '%s\n' "$2") <(printf '%s\n' "$3") || true
    fail=1
  fi
}
compare "$model vs $reconciliation" "$model_classes" "$reconciliation_classes"
compare "$model vs $lean" "$model_classes" "$lean_classes"

# Totality: the Lean match must have no wildcard arm.
if [ -f "$lean" ] && awk '/^def retryPolicy/{i=1;next} i&&/^[[:space:]]*\|/{print} i&&!/^[[:space:]]*\|/{exit}' "$lean" \
     | grep -qE '^\s*\|\s*_\s*=>'; then
  echo "FAIL: retryPolicy has a wildcard arm; adding a class must be a compile error"
  fail=1
fi

[ "$fail" -eq 0 ] && echo "OK: seven effect classes, agreed across three sites."
exit $fail
