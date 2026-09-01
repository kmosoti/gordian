#!/usr/bin/env bash
# Run one verifier against the frozen candidate and write the only log shape that
# scripts/check-closure-records.sh accepts as evidence.
#
# The log opens with two lines that bind the bytes to what they witness:
#
#   subject_exact_state_id=<commit id the verifier ran on>
#   command=<the exact string recorded in closure.json verifiers[].command>
#
# and closes with `exit_code=<N>`. Without the header a digest proves that a file exists,
# not that any command ran on any state: Atom #70's record cited one formal-verifier
# capture as the artifact of five different commands, and every digest matched.
#
# The subject is read from the workspace, not from an argument, so a log can only ever
# claim the state it actually ran on. It is `@` (the worker's frozen candidate, runbook
# section 6.7) or, when `@` is an empty child made by `jj workspace add`, `@-`. The
# working copy is snapshotted before and after the command; if the two ids differ the
# command changed the candidate and the log is discarded (exit 70), because the
# verified state would no longer be the frozen state.
#
# The command runs under `bash -e -o pipefail`, so `a; b; c` fails when any member fails.
# A plain `bash -c` would report only `c`, which is how a red `cargo fmt` hides behind a
# green `cargo test`.
#
# Usage:
#   scripts/capture-verifier.sh --atom N --id VERIFIER_ID [--log-root DIR] -- 'COMMAND STRING'
#
# Writes $LOG_ROOT/atom-N/VERIFIER_ID.log (LOG_ROOT defaults to $GORDIAN_LOG_ROOT, then
# ${TMPDIR:-/tmp}/gordian-logs) and MUST resolve outside the workspace: a log written inside
# it would be snapshotted into the candidate (runbook section 6.6). Exits with the command's
# status. Copy the log to artifacts/atoms/N/verifiers/VERIFIER_ID.log in the bookkeeping
# change (section 6.8) and record its SHA-256 as artifact_sha256.
set -euo pipefail

atom=""
verifier_id=""
log_root="${GORDIAN_LOG_ROOT:-${TMPDIR:-/tmp}/gordian-logs}"
while [ $# -gt 0 ]; do
  case "$1" in
    --atom) atom="${2:-}"; shift 2 ;;
    --id) verifier_id="${2:-}"; shift 2 ;;
    --log-root) log_root="${2:-}"; shift 2 ;;
    --) shift; break ;;
    -h|--help) sed -n '2,33p' "$0"; exit 0 ;;
    *) echo "capture-verifier: unknown argument $1" >&2; exit 2 ;;
  esac
done
if [ $# -ne 1 ]; then
  echo "capture-verifier: exactly one command string must follow --" >&2
  exit 2
fi
command_string="$1"

case "$atom" in
  ''|*[!0-9]*|0*) echo "capture-verifier: --atom must be a positive integer" >&2; exit 2 ;;
esac
if ! printf '%s' "$verifier_id" | grep -Eq '^[A-Za-z0-9][A-Za-z0-9._-]*$'; then
  echo "capture-verifier: --id must match ^[A-Za-z0-9][A-Za-z0-9._-]*\$" >&2
  exit 2
fi
case "$command_string" in
  ''|*$'\n'*|*$'\r'*)
    echo "capture-verifier: the command must be one non-empty line" >&2
    exit 2 ;;
esac

workspace="$(jj workspace root 2>/dev/null)" || {
  echo "capture-verifier: not inside a Jujutsu workspace" >&2
  exit 2
}
mkdir -p "$log_root/atom-$atom"
log_dir="$(cd "$log_root/atom-$atom" && pwd -P)"
case "$log_dir/" in
  "$workspace"/*)
    echo "capture-verifier: log root $log_dir is inside the workspace $workspace" >&2
    exit 2 ;;
esac
log="$log_dir/$verifier_id.log"

# The subject: `@`, or `@-` when `@` is an empty child. Snapshot happens here.
subject_of() {
  local head empty parent
  head="$(jj log -r @ -n 1 --no-graph -T 'commit_id')"
  empty="$(jj log -r @ -n 1 --no-graph -T 'if(empty, "true", "false")')"
  if [ "$empty" = true ]; then
    parent="$(jj log -r @- -n 1 --no-graph -T 'commit_id')"
    printf '%s\n' "$parent"
  else
    printf '%s\n' "$head"
  fi
}
subject="$(subject_of)"
if ! printf '%s' "$subject" | grep -Eq '^[0-9a-f]{40}$'; then
  echo "capture-verifier: could not resolve a single subject commit ($subject)" >&2
  exit 2
fi

{
  printf 'subject_exact_state_id=%s\n' "$subject"
  printf 'command=%s\n' "$command_string"
} > "$log"

set +e
bash -e -o pipefail -c "$command_string" < /dev/null 2>&1 | tee -a "$log"
status="${PIPESTATUS[0]}"
set -e
printf 'exit_code=%s\n' "$status" >> "$log"

after="$(subject_of)"
if [ "$after" != "$subject" ]; then
  rm -f "$log"
  echo "capture-verifier: the command changed the candidate ($subject -> $after); log discarded" >&2
  exit 70
fi

echo "capture-verifier: $verifier_id exit $status, log $log ($(sha256sum "$log" | cut -c1-64))" >&2
exit "$status"
