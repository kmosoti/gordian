#!/usr/bin/env bash
# No knowledge-graph node may claim tool:nanoda verifies anything while CI leaves it disabled.
#
# .github/workflows/verify.yml is the authority on whether the independent checker actually
# runs. A `verifiedBy` edge to a checker that is switched off is a capability claimed in the
# present tense with nothing behind it.
set -euo pipefail
cd "$(dirname "$0")/.."

workflow=.github/workflows/verify.yml
graph=knowledge/graph

if [ ! -f "$workflow" ] || [ ! -d "$graph" ]; then
  echo "SKIP: no verify.yml or no knowledge graph."
  exit 0
fi

if ! grep -qE '^\s*nanoda:\s*false\s*$' "$workflow"; then
  echo "OK: verify.yml does not disable nanoda; verifiedBy edges to it are permitted."
  exit 0
fi

if grep -rn '"verifiedBy"[^}]*tool:nanoda' "$graph" ; then
  echo "FAIL: verify.yml sets 'nanoda: false' while a node claims tool:nanoda verifies it."
  echo "      Either enable nanoda in the formal job or demote the edge (contrastsWith)."
  exit 1
fi

echo "OK: nanoda is disabled in CI and no node claims it as a verifier."
exit 0
