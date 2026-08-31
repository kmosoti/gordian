#!/usr/bin/env bash
# The committed EO17 benchmark join-key contract must agree with the native Atom snapshot.
set -euo pipefail
cd "$(dirname "$0")/.."

snapshot=artifacts/atoms/issues.json
module=orchestration/src/gordian_orchestration/atom_registry.py
order=docs/implementation/execution-order.md

for required in "$module" "$order" "$snapshot"; do
  if [ ! -f "$required" ]; then
    echo "FAIL: $required is missing; the benchmark-obligation checker requires the registry module, execution order, and snapshot."
    exit 1
  fi
done

PYTHONPATH="orchestration/src${PYTHONPATH:+:$PYTHONPATH}" python3 -m gordian_orchestration.atom_registry \
  --snapshot "$snapshot" check-benchmarks
