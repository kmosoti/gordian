#!/usr/bin/env bash
# verifier:formal — the local and CI entrypoint for the complete formal gate.
#
# The pinned Lean toolchain ships `leanchecker`, so all three checks are locally runnable:
# warning-free build, compiled-environment replay, and the Lean-native allowlisted axiom audit.
# Exit 0 only if all three hold. `--self-test` additionally proves that `sorry`, a
# non-allowlisted axiom in a foreign namespace, and a caller-forged exact-state evidence subject
# are rejected.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SELF_TEST=0
if [ "${1:-}" = "--self-test" ]; then
  SELF_TEST=1
elif [ "$#" -ne 0 ]; then
  echo "usage: scripts/verify-formal.sh [--self-test]" >&2
  exit 64
fi

FORMAL="$ROOT/formal"

fail() { printf 'verifier:formal FAIL — %s\n' "$1" >&2; exit 1; }

command -v lake >/dev/null || { echo "MISSING TOOL: lake" >&2; exit 78; }

(cd "$FORMAL" && lake build) || fail "lake build failed"
(cd "$FORMAL" && lake env leanchecker) || fail "leanchecker environment replay failed"
(cd "$FORMAL" && lake env lean Gordian/Audit.lean) || fail "axiom audit failed"

if [ "$SELF_TEST" -eq 1 ]; then
  sorry_probe='import Gordian\nset_option warningAsError true\ntheorem rejectedSorry : True := by sorry\n'
  if printf '%b' "$sorry_probe" | (cd "$FORMAL" && lake env lean --stdin) >/dev/null 2>&1; then
    fail "negative test: warningAsError accepted sorry"
  fi

  foreign_probe="$FORMAL/Gordian/AuditForeignNamespaceNegative.lean"
  [ ! -e "$foreign_probe" ] || fail "negative-test probe path already exists: $foreign_probe"
  trap 'rm -f "$foreign_probe"' EXIT
  (cd "$FORMAL" && lake env lean -o .lake/build/lib/lean/Gordian/Audit.olean \
    Gordian/Audit.lean) >/dev/null || fail "could not compile audit module for negative test"
  cat > "$foreign_probe" <<'LEAN'
import Gordian.Audit

namespace ForeignNamespace
axiom forbiddenAuditAxiom : False
theorem forbiddenAuditUse : False := forbiddenAuditAxiom
end ForeignNamespace

run_cmd Gordian.auditAxioms
LEAN
  if (cd "$FORMAL" && lake env lean Gordian/AuditForeignNamespaceNegative.lean) \
      >/dev/null 2>&1; then
    fail "negative test: axiom audit accepted a repository axiom in a foreign namespace"
  fi
  # Remove the repository-visible probe before the forged-state check invokes JJ.
  # Otherwise JJ auto-snapshots this temporary source and rewrites the exact
  # Candidate that the outer evidence writer is trying to verify.
  rm -f "$foreign_probe"

  fake_state=ffffffffffffffffffffffffffffffffffffffff
  if "$ROOT/scripts/write-formal-evidence.sh" \
      "$fake_state" \
      "${TMPDIR:-/tmp}/gordian-invalid-formal-evidence.json" >/dev/null 2>&1; then
    fail "negative test: formal evidence accepted a caller-controlled fake exact state"
  fi
fi

echo "verifier:formal OK — build, leanchecker, and allowlisted axiom audit passed"
