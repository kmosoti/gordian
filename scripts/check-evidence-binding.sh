#!/usr/bin/env bash
# The evidence binding has exactly seven components, and six sites must name the same seven:
#
#   docs/spec/data-model.md                    the `EvidenceBinding { ... }` record
#   docs/algorithms/evidence-and-admission.md  the conjuncts of `Fresh`
#   docs/algorithms/evidence-and-admission.md  the components of `Fingerprint`
#   docs/algorithms/evidence-and-admission.md  the components of `Subject`
#   formal/Gordian/Evidence.lean               the fields of `CandidateRef` and `EvidenceRef`
#   docs/formal/theorem-catalog.md             T004 "### Formal compatibility components"
#
# Set equality with cardinality 7, after snake/camel normalization. A name comparison alone
# passes while the linkage is missing, so the `Evidence` record must additionally declare a
# required `binding` field.
set -euo pipefail
cd "$(dirname "$0")/.."

model=docs/spec/data-model.md
admission=docs/algorithms/evidence-and-admission.md
lean=formal/Gordian/Evidence.lean
catalog=docs/formal/theorem-catalog.md

normalize() {
  python3 -c '
import re, sys
names = set()
for raw in sys.stdin:
    name = raw.strip().strip("`,;.")
    if not name:
        continue
    name = name.replace(" ", "_")
    name = re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()
    if re.fullmatch(r"[a-z][a-z0-9_]*", name):
        names.add(name)
for name in sorted(names):
    print(name)
'
}

# The body of a `Name {` ... `}` block in a fenced document.
record_fields() {
  local file=$1 opener=$2
  [ -f "$file" ] || return 0
  awk -v opener="$opener" '
    index($0, opener) == 1 { inside = 1; next }
    inside && /^\}/ { exit }
    inside { sub(/--.*$/, ""); gsub(/[[:space:],]/, ""); if (length($0)) print }
  ' "$file"
}

model_names=$(record_fields "$model" 'EvidenceBinding {' | normalize)

fresh_names=""
subject_names=""
fingerprint_names=""
if [ -f "$admission" ]; then
  fresh_names=$(awk '
    /^Fresh\(e, s, v\) :=/ { inside = 1; next }
    inside && /^```/ { exit }
    inside { print }
  ' "$admission" | sed -n 's/.*e\.binding\.\([a-z_]*\).*/\1/p' | normalize)
  subject_names=$(awk '
    /^Subject\(s, v\) = \{/ { inside = 1; next }
    inside && /^\}/ { exit }
    inside { sub(/--.*$/, ""); gsub(/[[:space:],]/, ""); if (length($0)) print }
  ' "$admission" | normalize)
  fingerprint_names=$(awk '
    /^Fingerprint\(s, v\) = H\(/ { inside = 1; next }
    inside && /^\)/ { exit }
    inside { print }
  ' "$admission" \
    | sed -e 's/^[[:space:]]*||[[:space:]]*//' -e 's/[[:space:]]*$//' \
          -e 's/(.*)$//' -e 's/^s\.//' | normalize)
fi

lean_names=""
if [ -f "$lean" ]; then
  lean_names=$(awk '
    /^structure (CandidateRef|EvidenceRef)/ { inside = 1; next }
    inside && /^[[:space:]]+[a-z][A-Za-z]*[[:space:]]*:/ {
      sub(/^[[:space:]]+/, ""); sub(/[[:space:]]*:.*$/, ""); print; next
    }
    inside { inside = 0 }
  ' "$lean" | normalize)
fi

catalog_names=""
if [ -f "$catalog" ]; then
  catalog_names=$(awk '
    /^### Formal compatibility components/ { inside = 1; next }
    inside && /^```/ { fence++; if (fence == 2) exit; next }
    inside && fence == 1 { print }
  ' "$catalog" | sed -e 's/^-[[:space:]]*//' -e 's/;$//' | normalize)
fi

present=0
for block in "$model_names" "$fresh_names" "$subject_names" "$fingerprint_names" "$lean_names" "$catalog_names"; do
  [ -n "$block" ] && present=$((present + 1))
done
if [ "$present" -eq 0 ]; then
  echo "SKIP: no evidence-binding component list found at any site yet."
  exit 0
fi

fail=0
check() {
  local label=$1 list=$2
  local count
  count=$(printf '%s\n' "$list" | grep -c . || true)
  if [ "$count" -ne 7 ]; then
    echo "FAIL: $label names $count binding components, expected 7"
    printf '%s\n' "$list" | sed 's/^/       /'
    fail=1
    return
  fi
  if [ "$list" != "$model_names" ]; then
    echo "FAIL: evidence binding drift: $label vs $model"
    diff <(printf '%s\n' "$model_names") <(printf '%s\n' "$list") || true
    fail=1
  fi
}

if [ -z "$model_names" ]; then
  echo "FAIL: no EvidenceBinding record found in $model"
  fail=1
else
  check "$model (EvidenceBinding)" "$model_names"
  check "$admission (Fresh)" "$fresh_names"
  check "$admission (Subject)" "$subject_names"
  check "$admission (Fingerprint)" "$fingerprint_names"
  check "$lean (CandidateRef/EvidenceRef)" "$lean_names"
  check "$catalog (T004)" "$catalog_names"
fi

# The linkage itself: Evidence must carry the binding, not merely agree with it by name.
if [ -f "$model" ] && ! grep -qE '^[[:space:]]*binding[[:space:]]*(:|,)' "$model"; then
  echo "FAIL: the Evidence record in $model declares no required 'binding' field"
  fail=1
fi

[ "$fail" -eq 0 ] && echo "OK: seven evidence-binding components, agreed across six sites."
exit $fail
