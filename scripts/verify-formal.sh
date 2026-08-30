#!/usr/bin/env bash
# verifier:formal — the local, runnable form of the formal gate.
#
# CI runs lean-action with leanchecker and axiom-audit. Those are not installable as a
# local command, so an agent could not execute verifier:formal as previously declared.
# This script is the local equivalent and is what runbook section 6.6 invokes:
#
#   1. lake build must succeed;
#   2. no `sorry`, `admit`, `native_decide`, or project-declared `axiom` in the sources;
#   3. every theorem's axiom closure must lie inside the allowlist, checked by asking
#      Lean itself via `#print axioms` — the same question axiom-audit asks.
#
# Exit 0 only if all three hold.
set -euo pipefail

cd "$(dirname "$0")/.."
FORMAL="formal"
ALLOWED_RE='^(propext|Classical\.choice|Quot\.sound)$'

fail() { printf 'verifier:formal FAIL — %s\n' "$1" >&2; exit 1; }

# ---------------------------------------------------------------- 1. build
( cd "$FORMAL" && lake build ) >/dev/null || fail "lake build failed"

# ---------------------------------------------------------------- 2. banned tokens
# Strip line comments before matching so prose mentioning `sorry` does not trip this.
banned=$(
  find "$FORMAL/Gordian" "$FORMAL" -maxdepth 1 -name '*.lean' -o -path "$FORMAL/Gordian/*.lean" \
  | sort -u \
  | while read -r f; do
      sed 's|--.*$||' "$f" \
        | grep -nE '(^|[^A-Za-z_])(sorry|admit|native_decide)([^A-Za-z_]|$)|^[[:space:]]*axiom[[:space:]]' \
        | sed "s|^|$f:|" || true
    done
)
[ -z "$banned" ] || { printf '%s\n' "$banned" >&2; fail "banned token in formal sources"; }

# ---------------------------------------------------------------- 3. axiom closure
# Collect every declared theorem/lemma and ask Lean to print its axiom closure.
mapfile -t decls < <(
  grep -rhoE '^[[:space:]]*(theorem|lemma)[[:space:]]+[A-Za-z_][A-Za-z0-9_'"'"'.]*' "$FORMAL/Gordian" \
  | awk '{print $2}' | sort -u
)
[ "${#decls[@]}" -gt 0 ] || fail "no theorems found — the audit would be vacuous"

probe="$FORMAL/AxiomAudit.lean"
trap 'rm -f "$probe"' EXIT
{
  echo "import Gordian"
  for d in "${decls[@]}"; do echo "#print axioms Gordian.$d"; done
} > "$probe"

out=$( cd "$FORMAL" && lake env lean AxiomAudit.lean 2>&1 ) || {
  printf '%s\n' "$out" >&2; fail "axiom probe did not elaborate"; }

# Lean prints either "'X' does not depend on any axioms" or "'X' depends on axioms: [a, b]".
bad=$(
  printf '%s\n' "$out" | grep 'depends on axioms' | while read -r line; do
    axioms=${line#*depends on axioms: }
    printf '%s' "${axioms//[\[\]]/}" | tr ',' '\n' | sed 's/^ *//;s/ *$//' | while read -r a; do
      [ -z "$a" ] && continue
      printf '%s\n' "$a" | grep -qE "$ALLOWED_RE" || printf '%s :: %s\n' "$line" "$a"
    done
  done
)
[ -z "$bad" ] || { printf '%s\n' "$bad" >&2; fail "axiom outside allowlist"; }

printf 'verifier:formal OK — %d theorems, axiom closure within {propext, Classical.choice, Quot.sound}\n' "${#decls[@]}"
