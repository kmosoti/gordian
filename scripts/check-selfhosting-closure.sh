#!/usr/bin/env bash
# The generated minimal self-hosting closure must match the native graph snapshot, and every
# actual registry identity must reach #49, #68, #69, or the explicit bootstrap/root set. GitHub
# issue numbers are not assumed contiguous: closed duplicates may consume identities.
set -euo pipefail
cd "$(dirname "$0")/.."

order=docs/implementation/execution-order.md
snapshot=artifacts/atoms/issues.json

if [ "$#" -eq 0 ]; then
  :
elif [ "$#" -eq 2 ] && [ "$1" = "--snapshot" ] && [ -n "$2" ]; then
  snapshot="$2"
else
  echo "usage: $0 [--snapshot PATH]" >&2
  exit 2
fi

for required in "$order" "$snapshot"; do
  if [ ! -f "$required" ]; then
    echo "FAIL: $required is missing"
    exit 1
  fi
done

PYTHONPATH="orchestration/src${PYTHONPATH:+:$PYTHONPATH}" python3 - "$order" "$snapshot" <<'PY'
import sys
from pathlib import Path

from gordian_orchestration.atom_registry import (
    SELFHOST_BEGIN,
    SELFHOST_END,
    render_selfhosting_block,
    selfhosting_sets,
)
from gordian_orchestration.derive_status import load_snapshot

order_path, snapshot_path = map(Path, sys.argv[1:])
text = order_path.read_text(encoding="utf-8")
issues = load_snapshot(snapshot_path)

try:
    actual = (
        SELFHOST_BEGIN
        + text.split(SELFHOST_BEGIN, 1)[1].split(SELFHOST_END, 1)[0]
        + SELFHOST_END
    )
except IndexError:
    print("FAIL: execution-order.md has no generated self-hosting marker pair")
    raise SystemExit(1)

expected = render_selfhosting_block(issues)
minimal, orphans = selfhosting_sets(issues)
print(f"computed closure(#49) = {list(minimal)}")
if actual != expected:
    print("FAIL: generated self-hosting closure block differs from native graph")
    raise SystemExit(1)
if orphans:
    print(f"FAIL: registry identities orphaned from Mission completion: {list(orphans)}")
    raise SystemExit(1)

print(
    f"OK: {len(minimal)} Atoms in closure(#49); "
    f"{len(issues)} registry identities covered with no orphan."
)
PY
