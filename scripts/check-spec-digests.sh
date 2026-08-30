#!/usr/bin/env bash
# The bootstrap spec revision of Atom N is ONE value with ONE definition (G-512):
#
#   SHA-256, lowercase hex, of the bytes of artifacts/atoms/<N>/spec.md after LF normalization.
#
# `gordian-canon-v1`, as closure-record.schema.json's spec_digest field names it, is the name of
# that normalization and adds nothing to it. This checker is what makes the definition binding:
#
#   1. a committed snapshot containing a CR byte is a hard failure, because `gh issue view` returns
#      GitHub bodies with CRLF and `sha256sum` on the raw capture is NOT the digest the rule
#      defines, so an un-normalized snapshot silently produces a digest nobody can reproduce;
#   2. every closure record's spec_digest is recomputed from its paired snapshot and compared;
#   3. a record that carries a spec_digest with no committed snapshot fails, and so does a snapshot
#      whose Atom has a closure record that omits the digest.
#
# An empty subject exits 0 after asserting the rule is still stated where the checker enforces it.
#
# Usage: check-spec-digests.sh [ROOT]
set -euo pipefail
root="${1:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$root"

snapshots=()
while IFS= read -r snapshot; do
  snapshots+=("$snapshot")
done < <(find artifacts/atoms -mindepth 2 -maxdepth 2 -name 'spec.md' 2>/dev/null | sort)

records=()
while IFS= read -r record; do
  records+=("$record")
done < <(find artifacts/atoms -mindepth 2 -maxdepth 2 -name 'closure.json' 2>/dev/null | sort)

if [ "${#snapshots[@]}" -eq 0 ] && [ "${#records[@]}" -eq 0 ]; then
  fail=0
  index=docs/implementation/issue-index.md
  if [ ! -f "$index" ]; then
    echo "FAIL: $index is missing; the bootstrap spec-revision rule has no home"
    fail=1
  else
    if ! grep -qF 'artifacts/atoms/<N>/spec.md' "$index"; then
      echo "FAIL: $index no longer defines the snapshot path artifacts/atoms/<N>/spec.md"
      fail=1
    fi
    if ! grep -qF 'spec_digest' "$index"; then
      echo "FAIL: $index no longer binds the snapshot digest to the closure record's spec_digest"
      fail=1
    fi
  fi
  if [ ! -f artifacts/schema/closure-record.schema.json ]; then
    echo "FAIL: artifacts/schema/closure-record.schema.json is missing"
    fail=1
  fi
  if [ "$fail" -ne 0 ]; then
    exit 1
  fi
  echo "OK: no Atom spec snapshots or closure records yet; the spec-digest rule is stated and unchanged."
  exit 0
fi

python3 - "${#snapshots[@]}" "${snapshots[@]+"${snapshots[@]}"}" "${records[@]+"${records[@]}"}" <<'PY'
"""Recompute every committed spec.md digest and compare it with its paired closure record."""

import hashlib
import json
import os
import sys

snapshot_count = int(sys.argv[1])
rest = sys.argv[2:]
snapshot_paths = rest[:snapshot_count]
record_paths = rest[snapshot_count:]

problems = []
digests = {}

for path in snapshot_paths:
    atom = os.path.basename(os.path.dirname(path))
    with open(path, "rb") as handle:
        raw = handle.read()
    if b"\r" in raw:
        problems.append(
            f"{path}: contains a CR byte. The capture must be LF-normalized before it is "
            "committed (pipe the gh output through `tr -d '\\r'`); the digest of a CRLF capture "
            "is not the digest the rule defines."
        )
        continue
    if not raw.strip():
        problems.append(f"{path}: empty; a spec revision with no text is not a specification")
        continue
    digests[atom] = hashlib.sha256(raw).hexdigest()

for path in record_paths:
    atom = os.path.basename(os.path.dirname(path))
    try:
        with open(path, encoding="utf-8") as handle:
            record = json.load(handle)
    except (OSError, ValueError) as error:
        problems.append(f"{path}: unreadable or malformed JSON: {error}")
        continue
    declared = record.get("spec_digest")
    snapshot = os.path.join(os.path.dirname(path), "spec.md")
    if declared is None:
        problems.append(f"{path}: no spec_digest; the record must name the spec revision it closes")
        continue
    if atom not in digests:
        if os.path.isfile(snapshot):
            continue  # the snapshot failed its own check above; that failure is already reported.
        problems.append(
            f"{path}: spec_digest {declared} has no committed snapshot at {snapshot}; the digest "
            "is unverifiable"
        )
        continue
    if declared != digests[atom]:
        problems.append(
            f"{path}: spec_digest {declared} disagrees with {snapshot}, which hashes to "
            f"{digests[atom]}; the issue body was edited after the claim"
        )

if problems:
    for problem in problems:
        print(f"FAIL: {problem}")
    raise SystemExit(1)

print(
    f"OK: {len(snapshot_paths)} spec snapshot(s) LF-normalized, "
    f"{len(record_paths)} closure record(s) carry a digest that recomputes."
)
PY
