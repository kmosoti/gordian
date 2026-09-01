#!/usr/bin/env bash
# Publish a state to `main` only after the full verifier set has passed on that exact state.
#
# This is step 0 of docs/protocols/landing.md made mechanical. The protocol has always said
# verification happens before admission, on a non-frontier ref, and that CI runs the same
# commands. Nothing enforced it, so the gap was procedural: a coordinator could run some of
# the checkers, judge that enough, and push. Twice that produced a red `main` — once from a
# Jujutsu identity that only CI lacked, once from a rewritten evidence snapshot that only
# check-spec-digests.sh looks at. Both were caught remotely, minutes and runner-hours after
# they could have been caught here.
#
# A red `main` is not just wasted CI. While `main` is red the landing gate cannot pass, so
# no Atom can close, and the whole Mission stalls behind it.
#
# Usage:
#   scripts/land.sh                 verify @ (or @- when @ is empty) and push it
#   scripts/land.sh -r <rev>        verify and push an explicit revision
#   scripts/land.sh --dry-run       verify only; never touch the remote
set -euo pipefail
shopt -s nullglob

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

rev=""
dry=false
while [ $# -gt 0 ]; do
  case "$1" in
    -r|--revision) rev="${2:?-r needs a revision}"; shift 2 ;;
    --dry-run) dry=true; shift ;;
    -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 64 ;;
  esac
done

command -v jj >/dev/null || { echo "MISSING TOOL: jj" >&2; exit 78; }

if [ -z "$rev" ]; then
  # The Jujutsu working copy is itself a commit. An empty `@` means the state to publish is
  # its parent; publishing `@` would land an empty change on top of the real one.
  if [ -z "$(jj log -r '@' --no-graph -T 'if(empty, "", "x")')" ]; then
    rev='@-'
  else
    rev='@'
  fi
fi

commit_id="$(jj log -r "$rev" --no-graph -T 'commit_id')"
change_id="$(jj log -r "$rev" --no-graph -T 'change_id')"
description="$(jj log -r "$rev" --no-graph -T 'description')"

echo "landing candidate: $rev -> ${commit_id:0:12} (change ${change_id:0:12})"

if [ -z "${description//[[:space:]]/}" ]; then
  echo "REFUSED: ${commit_id:0:12} has no description. An undescribed change cannot be" >&2
  echo "         audited after the fact; describe it before landing." >&2
  exit 65
fi

jj git fetch >/dev/null 2>&1 || echo "warning: could not fetch origin; verifying against the last known frontier" >&2

# Verifying a state that is not a descendant of the current frontier proves nothing about
# what the merge would produce, and the push would be rejected anyway.
if [ -z "$(jj log -r "main@origin & ::$rev" --no-graph -T 'commit_id' 2>/dev/null)" ]; then
  echo "REFUSED: ${commit_id:0:12} is not a descendant of main@origin." >&2
  echo "         Rebase onto the frontier first: jj rebase -d 'main@origin'" >&2
  exit 66
fi

# Verify the exact state, not the working copy as it happens to sit right now.
if [ "$(jj log -r '@' --no-graph -T 'commit_id')" != "$commit_id" ]; then
  echo "REFUSED: the working copy is not at ${commit_id:0:12}, so the verifiers would run" >&2
  echo "         against different content than the one being pushed." >&2
  echo "         Check it out first: jj edit $rev" >&2
  exit 67
fi

# actionlint parses the workflow and type-checks its expressions and embedded shell. The
# verifier groups never parse verify.yml at all, so a YAML break there reaches the runner
# unexamined — which is how a backslash continuation that escaped a block scalar got as far
# as a commit. It costs under a second and catches the whole class.
if command -v actionlint >/dev/null 2>&1; then
  echo "linting workflows ..."
  # Explicit paths, not a bare invocation: actionlint locates the repository by looking for
  # a .git directory, and a Jujutsu workspace does not have one. Bare `actionlint` exits
  # non-zero here with "no project was found in any parent directories".
  workflows=(.github/workflows/*.yml .github/workflows/*.yaml)
  [ "${#workflows[@]}" -gt 0 ] || { echo "REFUSED: no workflow files found" >&2; exit 1; }
  actionlint "${workflows[@]}" || {
    echo "REFUSED: workflow lint failed; nothing was pushed." >&2
    exit 1
  }
else
  echo "REFUSED: actionlint is not installed, so .github/workflows would go unchecked." >&2
  echo "         Install it with: bash scripts/install-toolchains.sh agent-tools" >&2
  exit 78
fi

echo "running the full verifier set against ${commit_id:0:12} ..."
if ! scripts/verify-local.sh all; then
  echo >&2
  echo "REFUSED: verification failed on ${commit_id:0:12}; nothing was pushed." >&2
  echo "         This is the run that would otherwise have turned main red." >&2
  exit 1
fi

if [ "$dry" = true ]; then
  echo "verified ${commit_id:0:12}; --dry-run, so the remote was not touched"
  exit 0
fi

jj bookmark set main -r "$rev"
jj git push --bookmark main
echo "landed ${commit_id:0:12} on main after a full green verifier set"
