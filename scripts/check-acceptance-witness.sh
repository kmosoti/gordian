#!/usr/bin/env bash
# The admission witness has exactly ten conjuncts, and the five places that state them agree
# as ordered sequences:
#
#   docs/spec/mission-graph.md                  "## Accepted frontier" block
#   docs/algorithms/evidence-and-admission.md   the `require` lines of admit()
#   docs/protocols/jujutsu-agent-protocol.md    "## 17. Acceptance condition" block
#   formal/Gordian/Acceptance.lean              the fields of `structure AcceptanceWitness`
#   docs/formal/theorem-catalog.md              the T006 witness-mapping table
#
# The three document sites are additionally compared on argument count, because a name-only
# comparison cannot see that one site wrote CurrentFrontierReconciled(candidate) while another
# wrote CurrentFrontierReconciled(I, t).
set -euo pipefail
cd "$(dirname "$0")/.."

mission=docs/spec/mission-graph.md
admission=docs/algorithms/evidence-and-admission.md
protocol=docs/protocols/jujutsu-agent-protocol.md
lean=formal/Gordian/Acceptance.lean
catalog=docs/formal/theorem-catalog.md

# `Name(a, b)` -> `name:2`, lowerCamelCasing the head.
normalize() {
  python3 -c '
import sys
for raw in sys.stdin:
    line = raw.strip()
    if not line:
        continue
    head, _, rest = line.partition("(")
    head = head.strip()
    if not head or not head[0].isupper():
        continue
    args = rest.rsplit(")", 1)[0].strip()
    count = 0 if not args else len([a for a in args.split(",") if a.strip()])
    print(f"{head[0].lower()}{head[1:]}:{count}")
'
}

# The fenced block that follows a heading.
fenced_after() {
  local file=$1 heading=$2
  [ -f "$file" ] || return 0
  awk -v heading="$heading" '
    $0 == heading { inside = 1; next }
    inside && /^## / { exit }
    inside && /^```/ { fence++; if (fence == 2) exit; next }
    inside && fence == 1 { print }
  ' "$file"
}

mission_names=$(fenced_after "$mission" '## Accepted frontier' | normalize)
protocol_names=$(fenced_after "$protocol" '## 17. Acceptance condition' | normalize)

admission_names=""
if [ -f "$admission" ]; then
  admission_names=$(awk '
    /^function admit\(/ { inside = 1 }
    inside && /^[[:space:]]*require[[:space:]]+[A-Z]/ { sub(/^[[:space:]]*require[[:space:]]+/, ""); print }
    inside && /^```/ { exit }
  ' "$admission" | normalize)
fi

lean_names=""
if [ -f "$lean" ]; then
  lean_names=$(awk '
    /^structure AcceptanceWitness/ { inside = 1; next }
    inside && /^[[:space:]]+[a-z][A-Za-z]*[[:space:]]*:/ {
      sub(/^[[:space:]]+/, ""); sub(/[[:space:]]*:.*$/, ""); print; next
    }
    inside { exit }
  ' "$lean")
fi

catalog_names=""
if [ -f "$catalog" ]; then
  catalog_names=$(awk '
    /^## T006/ { inside = 1 }
    inside && /^## / && !/^## T006/ { exit }
    inside && /^\| `[A-Z]/ { print }
  ' "$catalog" | sed -e 's/^| `//' -e 's/`.*$//' | normalize)
fi

present=0
for block in "$mission_names" "$admission_names" "$protocol_names" "$lean_names" "$catalog_names"; do
  [ -n "$block" ] && present=$((present + 1))
done
if [ "$present" -eq 0 ]; then
  echo "SKIP: no acceptance-witness list found at any of the five sites yet."
  exit 0
fi

fail=0
report_missing() {
  if [ -z "$2" ]; then
    echo "FAIL: no acceptance-witness list extracted from $1"
    fail=1
  fi
}
report_missing "$mission" "$mission_names"
report_missing "$admission" "$admission_names"
report_missing "$protocol" "$protocol_names"
report_missing "$lean" "$lean_names"
report_missing "$catalog" "$catalog_names"

# Names, as an ordered sequence, at all five sites.
mission_only=$(printf '%s\n' "$mission_names" | cut -d: -f1)
admission_only=$(printf '%s\n' "$admission_names" | cut -d: -f1)
protocol_only=$(printf '%s\n' "$protocol_names" | cut -d: -f1)
catalog_only=$(printf '%s\n' "$catalog_names" | cut -d: -f1)

count=$(printf '%s\n' "$mission_only" | grep -c . || true)
if [ "$count" -ne 10 ]; then
  echo "FAIL: $mission states $count conjuncts, expected 10"
  fail=1
fi

compare() {
  local label=$1 left=$2 right=$3
  if [ "$left" != "$right" ]; then
    echo "FAIL: acceptance witness drift: $label"
    diff <(printf '%s\n' "$left") <(printf '%s\n' "$right") || true
    fail=1
  fi
}

compare "$mission vs $admission (names)" "$mission_only" "$admission_only"
compare "$mission vs $protocol (names)" "$mission_only" "$protocol_only"
compare "$mission vs $catalog (names)" "$mission_only" "$catalog_only"
compare "$mission vs $lean (names)" "$mission_only" "$lean_names"

# Argument counts, at the three document sites that write them.
compare "$admission vs $protocol (arities)" "$admission_names" "$protocol_names"
mission_arity=$(printf '%s\n' "$mission_names" | cut -d: -f2)
protocol_arity=$(printf '%s\n' "$protocol_names" | cut -d: -f2)
compare "$mission vs $protocol (arities)" "$mission_arity" "$protocol_arity"

[ "$fail" -eq 0 ] && echo "OK: ten conjuncts, same order, same arities, at five sites."
exit $fail
