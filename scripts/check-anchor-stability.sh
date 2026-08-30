#!/usr/bin/env bash
# No cross-reference may target a numbered section anchor of a renumberable document.
#
# docs/algorithms/evidence-and-admission.md is renumbered by any insertion, so its section
# numbers are navigation, never an addressing scheme. Every normative rule in it lives under a
# name anchor; every cross-reference must use that name anchor.
#
#   FORBIDDEN   <that document>#<section-number>-<slug>
#   REQUIRED    <that document>#frontier-reconciliation
set -euo pipefail
cd "$(dirname "$0")/.."

pattern='evidence-and-admission\.md#[0-9]'

if grep -rn --binary-files=without-match \
     --include='*.md' --include='*.lean' --include='*.rs' --include='*.sh' \
     --include='*.py' --include='*.yml' --include='*.yaml' --include='*.json' \
     --include='*.jsonld' --include='*.toml' \
     --exclude-dir=.jj --exclude-dir=.git --exclude-dir=target --exclude-dir=.lake \
     -- "$pattern" . ; then
  echo "FAIL: numbered section anchors are forbidden; use the name anchors listed in the"
  echo "      anchor stability rule of the specification revision (section 0.1)."
  exit 1
fi

echo "OK: no numbered evidence-and-admission anchors."
exit 0
