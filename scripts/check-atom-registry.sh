#!/usr/bin/env bash
# The committed Atom registry, issue-body dependency mirrors, project-plan tables, and
# maximum-length execution spine must agree. Native GitHub blockedBy links remain authoritative;
# this offline check becomes active when #70 captures artifacts/atoms/issues.json.
set -euo pipefail
cd "$(dirname "$0")/.."

snapshot=artifacts/atoms/issues.json
module=orchestration/src/gordian_orchestration/atom_registry.py
order=docs/implementation/execution-order.md

for required in "$module" "$order"; do
  if [ ! -f "$required" ]; then
    echo "FAIL: $required is missing"
    exit 1
  fi
done

grep -qF '<!-- BEGIN GENERATED: MAXIMUM-LENGTH SPINE -->' "$order" || {
  echo "FAIL: $order has no generated spine start marker"
  exit 1
}
grep -qF '<!-- END GENERATED: MAXIMUM-LENGTH SPINE -->' "$order" || {
  echo "FAIL: $order has no generated spine end marker"
  exit 1
}

if [ ! -f "$snapshot" ]; then
  echo "OK: $snapshot is pending #70; registry code and generated-spine markers are present."
  exit 0
fi

PYTHONPATH=orchestration/src python3 -m gordian_orchestration.atom_registry \
  --snapshot "$snapshot" check
