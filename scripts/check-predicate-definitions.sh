#!/usr/bin/env bash
# Each of the seven readiness predicate names has exactly one `### <Name>` definition in
# docs/spec/mission-graph.md "## Readiness predicate definitions". docs/algorithms/scheduling.md
# and docs/architecture.md link to that section rather than restating a body, and no document
# outside the two defining documents reproduces a predicate body at all -- a reproduction that
# is not byte-identical is exactly the drift this check exists to stop, and the cheapest way to
# guarantee byte-identity is to have no second copy.
set -euo pipefail
cd "$(dirname "$0")/.."

spec=docs/spec/mission-graph.md
# CurrentFrontierReconciled is defined at full arity in the admission document; that document is
# the normative home of the reconciliation relation and is cited as such by the witness mapping.
admission=docs/algorithms/evidence-and-admission.md
anchor='mission-graph.md#readiness-predicate-definitions'
linkers=(docs/algorithms/scheduling.md docs/architecture.md)

names=(
  ValidSpec
  PreconditionsHold
  CompatibleExecutorAvailable
  RequiredResourcesAvailable
  AuthorizationValid
  LeaseCompatible
  CurrentFrontierReconciled
)

if [ ! -f "$spec" ] || ! grep -q '^## Readiness predicate definitions' "$spec"; then
  echo "SKIP: $spec has no '## Readiness predicate definitions' section yet."
  exit 0
fi

fail=0

for name in "${names[@]}"; do
  count=$(grep -c "^### ${name}\$" "$spec" || true)
  if [ "$count" -ne 1 ]; then
    echo "FAIL: $spec has $count '### $name' headings, expected exactly 1"
    fail=1
  fi
done

for name in "${names[@]}"; do
  while IFS= read -r hit; do
    file=${hit%%:*}
    case "$file" in
      "$spec"|"$admission") continue ;;
    esac
    echo "FAIL: restated definition of $name outside $spec: $hit"
    fail=1
  done < <(grep -rn --include='*.md' \
             --exclude-dir=.jj --exclude-dir=.git --exclude-dir=target \
             -E "^[[:space:]]*${name}\(.*\)[[:space:]]*:=" docs/ || true)
done

for file in "${linkers[@]}"; do
  [ -f "$file" ] || continue
  if ! grep -qF "$anchor" "$file"; then
    echo "FAIL: $file does not link to $anchor"
    fail=1
  fi
done

[ "$fail" -eq 0 ] && echo "OK: seven readiness predicates, one definition each."
exit $fail
