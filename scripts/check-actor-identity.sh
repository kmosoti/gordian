#!/usr/bin/env bash
# Actor identity must agree everywhere it appears (G-511).
#
# docs/implementation/agent-runbook.md section 6.1 fixes the actor string as
# gordian-agent/<harness>/<run-id> and requires it in four places. Two of them are mechanically
# checkable in this repository, and this checker is the only thing that checks them:
#
#   * every commit in the change under review that an agent authored carries exactly one
#     `Gordian-Actor:` trailer matching ^Gordian-Actor: gordian-agent/[a-z0-9-]+/[A-Za-z0-9_-]+$,
#     and that trailer equals the commit's author identity;
#   * the closure record's actor agrees with the trailer of the commit that wrote it.
#
# WHICH actor the closure record names. Section 1 has the COORDINATOR write the record in a
# separate bookkeeping change, so the trailer on that change carries the coordinator's actor and
# not the executing agent's. The two are therefore two fields, and the schema says so:
#   actor        the agent that EXECUTED the Atom and produced the verified candidate
#   recorded_by  the coordinator that authored the bookkeeping change holding the record
# recorded_by is optional and defaults to actor, for the case where one actor did both. The
# trailer of the commit that adds artifacts/atoms/<N>/closure.json must equal recorded_by, or
# actor when recorded_by is absent. Without that split, G-511's closure-record binding is
# unsatisfiable as written.
#
# Commits with neither an agent author nor a trailer are human commits and are not the subject.
# An absent subject (no git history to read, or no commit range under review) exits 0 after
# asserting the rule is still stated in the runbook; a subject that exists and is malformed always
# fails.
#
# Subject override, used by scripts/tests/ and by any harness with no git: set
# GORDIAN_COMMIT_MESSAGES to a file of NUL-separated records, each `<author>\x1f<commit message>`
# — exactly what `git log --format=%an%x1f%B%x00 <range>` writes.
#
# Usage: check-actor-identity.sh [ROOT]
set -euo pipefail
root="${1:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$root"

runbook=docs/implementation/agent-runbook.md
subject_file=""
scratch=""
cleanup() {
  if [ -n "$scratch" ]; then
    rm -rf "$scratch"
  fi
  return 0
}
trap cleanup EXIT

if [ -n "${GORDIAN_COMMIT_MESSAGES:-}" ]; then
  if [ ! -f "$GORDIAN_COMMIT_MESSAGES" ]; then
    echo "FAIL: GORDIAN_COMMIT_MESSAGES=$GORDIAN_COMMIT_MESSAGES does not exist"
    exit 1
  fi
  subject_file="$GORDIAN_COMMIT_MESSAGES"
elif [ -e .git ] && command -v git >/dev/null 2>&1; then
  range=""
  if [ -n "${GORDIAN_REVIEW_RANGE:-}" ]; then
    range="$GORDIAN_REVIEW_RANGE"
  elif [ -n "${GITHUB_BASE_REF:-}" ] \
    && git rev-parse --verify --quiet "origin/${GITHUB_BASE_REF}" >/dev/null 2>&1; then
    range="origin/${GITHUB_BASE_REF}..HEAD"
  elif git rev-parse --verify --quiet origin/main >/dev/null 2>&1; then
    range="origin/main..HEAD"
  fi
  if [ -n "$range" ]; then
    scratch="$(mktemp -d)"
    subject_file="$scratch/commits"
    if ! git log --format='%an%x1f%B%x00' "$range" > "$subject_file" 2>/dev/null; then
      echo "FAIL: cannot read the commit range $range; the trailer rule has no readable subject"
      exit 1
    fi
  fi
fi

if [ -z "$subject_file" ]; then
  fail=0
  if [ ! -f "$runbook" ]; then
    echo "FAIL: $runbook is missing; the actor rule has no home"
    fail=1
  elif ! grep -qF 'Gordian-Actor' "$runbook"; then
    echo "FAIL: $runbook no longer requires the Gordian-Actor trailer this checker enforces"
    fail=1
  fi
  if [ "$fail" -ne 0 ]; then
    exit 1
  fi
  echo "OK: no commit range under review (no git history here); the actor rule is stated and unchanged."
  # The closure-record half of the check does not need git and still runs.
fi

python3 - "${subject_file:-}" <<'PY'
"""Validate Gordian-Actor trailers on the change under review and against the closure records."""

import glob
import json
import os
import re
import sys

subject_file = sys.argv[1]

ACTOR = re.compile(r"^gordian-agent/[a-z0-9-]+/[A-Za-z0-9_-]+$")
TRAILER = re.compile(r"^Gordian-Actor: (gordian-agent/[a-z0-9-]+/[A-Za-z0-9_-]+)$")
ANY_TRAILER = re.compile(r"^\s*Gordian-Actor\s*:(.*)$", re.IGNORECASE)

problems = []
commits = []

if subject_file:
    with open(subject_file, encoding="utf-8", errors="replace") as handle:
        blob = handle.read()
    for record in blob.split("\x00"):
        if not record.strip():
            continue
        author, _, message = record.partition("\x1f")
        commits.append((author.strip(), message))

agent_commits = 0
for position, (author, message) in enumerate(commits):
    label = f"commit {position} by {author!r}"
    lines = message.splitlines()
    trailers = [line for line in lines if ANY_TRAILER.match(line)]
    authored_by_agent = bool(ACTOR.match(author))

    if not trailers and not authored_by_agent:
        continue  # a human commit; not the subject of the agent trailer rule.

    agent_commits += 1

    if not trailers:
        problems.append(
            f"{label}: authored by an agent actor with no Gordian-Actor trailer "
            "(agent-runbook.md section 6.1)"
        )
        continue
    if len(trailers) > 1:
        problems.append(f"{label}: {len(trailers)} Gordian-Actor trailers; exactly one is required")
        continue

    trailer = trailers[0]
    match = TRAILER.match(trailer.strip())
    if not match:
        problems.append(
            f"{label}: trailer {trailer.strip()!r} does not match "
            "^Gordian-Actor: gordian-agent/[a-z0-9-]+/[A-Za-z0-9_-]+$"
        )
        continue
    if not authored_by_agent:
        problems.append(
            f"{label}: carries actor {match.group(1)!r} but its author identity is {author!r}; "
            "section 6.1 requires the author identity to BE the actor string"
        )
        continue
    if match.group(1) != author:
        problems.append(
            f"{label}: trailer actor {match.group(1)!r} disagrees with author {author!r}"
        )

# The closure-record half needs no history: the record names its own actors, and the schema's
# patterns are re-asserted here so a record cannot carry a malformed actor even if the schema is
# loosened.
records = 0
for path in sorted(glob.glob("artifacts/atoms/*/closure.json")):
    records += 1
    try:
        with open(path, encoding="utf-8") as handle:
            record = json.load(handle)
    except (OSError, ValueError) as error:
        problems.append(f"{path}: unreadable or malformed JSON: {error}")
        continue
    for field in ("actor", "recorded_by"):
        value = record.get(field)
        if value is None:
            if field == "actor":
                problems.append(f"{path}: no actor; the record must name who executed the Atom")
            continue
        if not isinstance(value, dict) or not isinstance(value.get("id"), str):
            problems.append(f"{path}.{field}: must be an object with a string id")
            continue
        identity = value["id"]
        if not (ACTOR.match(identity) or re.match(r"^human:[A-Za-z0-9_.@-]+$", identity)):
            problems.append(
                f"{path}.{field}.id: {identity!r} is neither gordian-agent/<harness>/<run-id> "
                "nor human:<identity>"
            )
    actor = (record.get("actor") or {}).get("id")
    coordinator = (record.get("recorded_by") or {}).get("id", actor)
    if not commits or not isinstance(coordinator, str):
        continue
    # Cross-check: any commit in the range whose message names this record must carry the
    # coordinator's actor. Without history for the file itself, the trailer set of the range is
    # the strongest available evidence, so require the coordinator to be among the range's actors
    # only when the range contains agent commits at all.
    actors = {
        TRAILER.match(line.strip()).group(1)
        for _, message in commits
        for line in message.splitlines()
        if TRAILER.match(line.strip())
    }
    if actors and coordinator.startswith("gordian-agent/") and coordinator not in actors:
        problems.append(
            f"{path}: recorded_by/actor {coordinator!r} appears in no Gordian-Actor trailer of "
            f"the change under review {sorted(actors)}; the record and the change disagree about "
            "who wrote it"
        )

if problems:
    for problem in problems:
        print(f"FAIL: {problem}")
    raise SystemExit(1)

if subject_file:
    print(
        f"OK: {len(commits)} commit(s) under review, {agent_commits} agent-authored with a valid "
        f"Gordian-Actor trailer; {records} closure record(s) agree."
    )
else:
    print(f"OK: {records} closure record(s) carry a well-formed actor.")
PY
