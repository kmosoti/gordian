#!/usr/bin/env bash
# The global hard-dependency target kinds are stated in exactly three places, and the three
# must name the same set:
#
#   formal/Gordian/Graph.lean       the WorkKind constructors GloballyDependable maps to True
#   docs/spec/data-model.md         "### Global hard dependency target kinds", two lists
#   docs/formal/theorem-catalog.md  T002 "### The set"
set -euo pipefail
cd "$(dirname "$0")/.."

lean=formal/Gordian/Graph.lean
model=docs/spec/data-model.md
catalog=docs/formal/theorem-catalog.md

# Lean: `| .atom => True` -> Atom
lean_kinds=""
if [ -f "$lean" ]; then
  lean_kinds=$(awk '
    /^def GloballyDependable/ { inside = 1; next }
    inside && /^[[:space:]]*\|/ {
      if ($0 ~ /True[[:space:]]*$/) {
        ctor = $0
        sub(/^[[:space:]]*\|[[:space:]]*\.?/, "", ctor)
        sub(/[[:space:]]*=>.*$/, "", ctor)
        sub(/^WorkKind\./, "", ctor)
        if (ctor != "_") print ctor
      }
      next
    }
    inside { exit }
  ' "$lean" | python3 -c 'import sys
for line in sys.stdin:
    kind = line.strip()
    if kind:
        print(kind[0].upper() + kind[1:])' | sort -u)
fi

# data-model.md: the fenced block after each of the two "Allowed ... kinds:" sentences.
kinds_after() {
  local marker=$1
  [ -f "$model" ] || return 0
  awk -v marker="$marker" '
    index($0, marker) { armed = 1; next }
    armed && /^```/ { fence++; if (fence == 2) exit; next }
    armed && fence == 1 { gsub(/^[[:space:]]+|[[:space:]]+$/, ""); if (length($0)) print }
  ' "$model" | sort -u
}
model_depender=""
model_prerequisite=""
if [ -f "$model" ] && grep -q '^### Global hard dependency target kinds' "$model"; then
  model_depender=$(kinds_after 'Allowed **depender** kinds:')
  model_prerequisite=$(kinds_after 'Allowed **prerequisite** kinds:')
fi

# theorem-catalog.md T002: "Allowed depender kinds: `Atom`. Allowed prerequisite kinds: `Atom`."
catalog_line=""
if [ -f "$catalog" ]; then
  catalog_line=$(awk '/^## T002/{inside=1} inside && /^## /&&!/^## T002/{exit} inside && /Allowed depender kinds:/{print}' "$catalog")
fi
# shellcheck disable=SC2016  # the backticks are markdown, not a command substitution
catalog_kinds=$(printf '%s\n' "$catalog_line" | grep -o '`[A-Za-z]*`' | tr -d '`' | sort -u || true)

if [ -z "$lean_kinds" ] && [ -z "$model_prerequisite" ] && [ -z "$catalog_kinds" ]; then
  echo "SKIP: no hard-dependency kind list at any of the three sites yet."
  exit 0
fi

fail=0
require_nonempty() {
  if [ -z "$2" ]; then echo "FAIL: no kind list extracted from $1"; fail=1; fi
}
require_nonempty "$lean (GloballyDependable)" "$lean_kinds"
require_nonempty "$model (depender kinds)" "$model_depender"
require_nonempty "$model (prerequisite kinds)" "$model_prerequisite"
require_nonempty "$catalog (T002)" "$catalog_kinds"

compare() {
  if [ "$2" != "$3" ]; then
    echo "FAIL: hard-dependency kind set drift: $1"
    diff <(printf '%s\n' "$2") <(printf '%s\n' "$3") || true
    fail=1
  fi
}
compare "$model depender vs prerequisite" "$model_depender" "$model_prerequisite"
compare "$lean vs $model" "$lean_kinds" "$model_prerequisite"
compare "$catalog vs $model" "$catalog_kinds" "$model_prerequisite"

[ "$fail" -eq 0 ] && echo "OK: one hard-dependency kind set: $(printf '%s' "$lean_kinds" | tr '\n' ' ')"
exit $fail
