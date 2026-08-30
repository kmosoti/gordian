#!/usr/bin/env bash
# The banned-phrase list lives HERE, not in the document, so the document does not contain the
# strings it forbids. Hedging vocabulary converts a fixed value back into a judgement call, and
# is forbidden in the statistical contract, in any experiment manifest's analysis_plan, and in
# the acceptance section of an experiment issue.
set -euo pipefail
cd "$(dirname "$0")/.."

BANNED_PHRASES=(
  "where appropriate"
  "where feasible"
  "as needed"
  "reasonable"
  "as applicable"
  "if possible"
)

contract=docs/testing/statistical-contract.md

targets=()
[ -f "$contract" ] && targets+=("$contract")
while IFS= read -r file; do targets+=("$file"); done < <(find experiments -name 'protocol.json' 2>/dev/null | sort)
# A committed snapshot of the experiment issue bodies, when one exists.
[ -f artifacts/atoms/issues.json ] && targets+=(artifacts/atoms/issues.json)

if [ "${#targets[@]}" -eq 0 ]; then
  echo "SKIP: no statistical contract and no experiment manifests yet."
  exit 0
fi

fail=0
for file in "${targets[@]}"; do
  [ -e "$file" ] || continue
  for phrase in "${BANNED_PHRASES[@]}"; do
    if grep -niF -- "$phrase" "$file" >/dev/null; then
      echo "FAIL: banned hedging phrase '$phrase' in $file"
      grep -niF -- "$phrase" "$file" | sed 's/^/       /'
      fail=1
    fi
  done
done

# Every experiment manifest names a class, and every analysis_plan is complete.
while IFS= read -r file; do
  python3 - "$file" <<'PY' || fail=1
import json
import sys

path = sys.argv[1]
with open(path, encoding="utf-8") as handle:
    manifest = json.load(handle)

problems = []
if not manifest.get("class"):
    problems.append("no class")
plan = manifest.get("analysis_plan")
if not isinstance(plan, dict):
    problems.append("no analysis_plan")
else:
    for field in ("primary_metric", "effect_size", "min_n", "multiplicity", "stopping_rule"):
        if plan.get(field) in (None, "", [], {}):
            problems.append(f"analysis_plan.{field} missing")
if problems:
    print(f"FAIL: {path}: " + "; ".join(problems))
    raise SystemExit(1)
PY
done < <(find experiments -name 'protocol.json' 2>/dev/null | sort)

[ "$fail" -eq 0 ] && echo "OK: no hedging vocabulary; every experiment names a class."
exit $fail
