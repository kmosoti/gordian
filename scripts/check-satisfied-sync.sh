#!/usr/bin/env bash
# Fails if the normative Satisfied definition drifts between its three homes:
#   docs/spec/mission-graph.md   SATISFIED-DEF block
#   docs/architecture.md         SATISFIED-DEF block
#   formal/Gordian/Frontier.lean comment immediately above `def Satisfied`
#
# The three must be byte-identical and non-empty, and the block's branch discriminator must be
# the literal token EvidenceInherited -- the same token `subject_of` branches on -- so the
# declarative definition and the projector rule cannot drift into different conditions.
set -euo pipefail
cd "$(dirname "$0")/.."

md_a=docs/spec/mission-graph.md
md_b=docs/architecture.md
lean=formal/Gordian/Frontier.lean

extract_md() {
  [ -f "$1" ] || return 0
  awk '/<!-- BEGIN SATISFIED-DEF -->/{f=1;next} /<!-- END SATISFIED-DEF -->/{f=0} f' "$1" \
    | sed -e '/^```/d' -e 's/[[:space:]]*$//'
}

extract_lean() {
  [ -f "$1" ] || return 0
  awk '/^-- Satisfied\(a\) :=/{f=1} f && /^--/{sub(/^-- ?/,"");sub(/[[:space:]]*$/,"");print;next} f{exit}' "$1"
}

a=$(extract_md "$md_a")
b=$(extract_md "$md_b")
c=$(extract_lean "$lean")

if [ -z "$a" ] && [ -z "$b" ] && [ -z "$c" ]; then
  echo "SKIP: no SATISFIED-DEF block in $md_a, $md_b, or $lean yet."
  exit 0
fi

fail=0
[ -n "$a" ] || { echo "FAIL: empty or absent SATISFIED-DEF block in $md_a"; fail=1; }
[ -n "$b" ] || { echo "FAIL: empty or absent SATISFIED-DEF block in $md_b"; fail=1; }
[ -n "$c" ] || { echo "FAIL: empty or absent Satisfied comment block in $lean"; fail=1; }

if [ "$a" != "$b" ]; then
  echo "FAIL: drift: $md_a vs $md_b"
  diff <(printf '%s\n' "$a") <(printf '%s\n' "$b") || true
  fail=1
fi
if [ "$a" != "$c" ]; then
  echo "FAIL: drift: $md_a vs $lean"
  diff <(printf '%s\n' "$a") <(printf '%s\n' "$c") || true
  fail=1
fi

case "$a" in
  *EvidenceInherited*) ;;
  *) echo "FAIL: the SATISFIED-DEF branch discriminator is not the literal token EvidenceInherited"
     fail=1 ;;
esac

if [ -f "$lean" ] && ! grep -q '^def Satisfied' "$lean"; then
  echo "FAIL: no 'def Satisfied' under the comment block in $lean"
  fail=1
fi

[ "$fail" -eq 0 ] && echo "OK: the three Satisfied definitions agree."
exit $fail
