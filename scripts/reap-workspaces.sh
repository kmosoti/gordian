#!/usr/bin/env bash
# Reclaim local build resources from Atom workspaces.
#
# Why this exists: on 2026-09-01 /tmp (a 7.8 GB tmpfs) reached 100%. Each Atom workspace
# carries its own cargo `target/` — roughly 300 MB — and ten of the eleven present belonged
# to Atoms that were already closed and landed. Nothing reaped them.
#
# A full filesystem does not announce itself as a disk error. It presents as flaky builds,
# gates that time out, and checkers that appear to flip pass/fail. That is exactly the
# shape the Mission spent hours inside, re-running one gate 340 times.
#
# Safety rules, in order of importance:
#   * only `target/` is ever removed. It is rebuildable by definition and gitignored.
#   * only workspaces whose Atom is CLOSED on GitHub are touched, unless --all is given.
#   * the workspace itself is never removed. Workspace lifecycle belongs to the
#     coordinator (see the identity-ownership table in the Mission goal).
#   * with no GitHub access it reaps nothing and says so, rather than guessing.
set -euo pipefail

ROOT="${GORDIAN_WORKSPACE_ROOT:-/tmp}"
REPO="${GORDIAN_REPO_SLUG:-kmosoti/gordian}"
THRESHOLD="${GORDIAN_DISK_THRESHOLD:-75}"
all=false
dry=false
for arg in "$@"; do
  case "$arg" in
    --all) all=true ;;
    --dry-run) dry=true ;;
    -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done

pct=$(df --output=pcent "$ROOT" | tail -1 | tr -dc '0-9')
avail=$(df -h --output=avail "$ROOT" | tail -1 | tr -d ' ')
echo "$ROOT is ${pct}% full (${avail} free); threshold ${THRESHOLD}%"

if [ "$pct" -lt "$THRESHOLD" ] && [ "$all" = false ]; then
  echo "below threshold; nothing to do (use --all to reap regardless)"
  exit 0
fi

closed=""
if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
  closed=$(gh issue list -R "$REPO" --state closed --limit 300 --json number --jq '.[].number' 2>/dev/null || true)
fi
if [ -z "$closed" ] && [ "$all" = false ]; then
  echo "cannot determine which Atoms are closed (no gh access): reaping nothing." >&2
  echo "Re-run with --all only if you accept rebuilding open Atoms' target dirs." >&2
  exit 78
fi

freed_kb=0
reaped=0
for target in "$ROOT"/gordian-atom*/workspace/target; do
  [ -d "$target" ] || continue
  ws=$(basename "$(dirname "$(dirname "$target")")")
  atom=$(printf '%s' "$ws" | sed -n 's/^gordian-atom\([0-9][0-9]*\).*/\1/p')
  if [ "$all" = false ]; then
    [ -n "$atom" ] || continue
    printf '%s\n' "$closed" | grep -qx "$atom" || { echo "  keep  $ws (Atom #${atom:-?} open)"; continue; }
  fi
  size_kb=$(du -sk "$target" | cut -f1)
  if [ "$dry" = true ]; then
    echo "  would reap $((size_kb / 1024)) MB  $target"
  else
    rm -rf "$target"
    echo "  reaped     $((size_kb / 1024)) MB  $target"
  fi
  freed_kb=$((freed_kb + size_kb))
  reaped=$((reaped + 1))
done

verb="reclaimed"
[ "$dry" = true ] && verb="would reclaim"
echo "$verb $((freed_kb / 1024)) MB from $reaped workspace(s)"
df --output=pcent "$ROOT" | tail -1 | tr -dc '0-9' | xargs -I{} echo "$ROOT now {}% full"
