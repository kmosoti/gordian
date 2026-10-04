#!/usr/bin/env bash
# Run one manifest: preflight checks, pin to core 3, launch `gordian-run` through the isolation
# runner under a wall-clock backstop, then compare the arm's own measured time with what the
# runner's controller says it used.
#
# Usage:
#   scripts/run-driver.sh --manifest FILE [--out DIR] [--bin PATH]
#
#   --manifest  the run manifest (JSON, as written by `gordian-run init`)
#   --out       the run directory; default artifacts/runs/<run_id>
#   --bin       the gordian-run binary; default target/release/gordian-run. The driver never
#               builds: a build must not run while a measurement does.
#
# Steps, as in docs/local-test-plan.md, A4:
#   1. refuse if free disk is under 2 GB, if a cargo or rustc process is running, or if the
#      manifest's source_revision is not `git rev-parse HEAD` of a clean tree;
#   2. pin this shell to core 3 (cores 0-2 are for arms; core 3 is for the driver and recorder);
#   3. launch gordian-run through scripts/cgroup-run.sh with the manifest's isolation limits and
#      timeout(1) as a backstop;
#   4. after it exits, divide the sum of the measured columns of measured.csv by the CPU
#      nanoseconds in usage.json, record the quotient in usage.json as internal_external_ratio,
#      and compare it with the tolerance the manifest declares.
#
# Exit status:
#    0  run completed and the ratio is inside the declared tolerance (or none is declared)
#    2  usage error
#    3  manifest unreadable or missing a field the driver needs
#    4  run completed but internal_external_ratio is outside the manifest's tolerance. The
#       instruments disagree; that is a defect to investigate, not something to tune away
#   10  less than 2 GB free
#   11  a cargo or rustc process is running
#   12  manifest source_revision is not HEAD
#   13  the working tree is not clean
#   14  the run directory already holds results
#   20  could not pin to core 3
#   21  the gordian-run binary is missing
#   124 the timeout backstop fired
#   else the exit status of gordian-run (through the runner)
#
# The driver passes no --allow-unisolated: a run that cannot be isolated is refused by the
# runner, not run.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

die() {
  local code="$1"
  shift
  echo "run-driver: $*" >&2
  exit "$code"
}

usage() {
  sed -n '2,/^$/p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//' >&2
}

manifest=""
out=""
bin=""
while [ $# -gt 0 ]; do
  case "$1" in
    --manifest) [ $# -ge 2 ] || die 2 "--manifest needs a value"; manifest="$2"; shift 2 ;;
    --out) [ $# -ge 2 ] || die 2 "--out needs a value"; out="$2"; shift 2 ;;
    --bin) [ $# -ge 2 ] || die 2 "--bin needs a value"; bin="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) usage; die 2 "unknown option: $1" ;;
  esac
done
[ -n "$manifest" ] || { usage; die 2 "--manifest is required"; }
command -v jq >/dev/null 2>&1 || die 3 "jq is required"
[ -r "$manifest" ] || die 3 "cannot read manifest $manifest"

field() { jq -er "$1" "$manifest" 2>/dev/null || die 3 "manifest has no usable $1"; }

run_id="$(field .run_id)"
source_revision="$(field .source_revision)"
cpus="$(field .isolation.cpus)"
quota="$(field .isolation.cpu_quota_percent)"
memory="$(field .isolation.memory_bytes)"
timeout_secs="$(field .isolation.timeout_secs)"
case "$run_id" in
  ''|*[!A-Za-z0-9._-]*|.|..) die 3 "run_id '$run_id' is not a safe name" ;;
esac

[ -n "$out" ] || out="$root/artifacts/runs/$run_id"
[ -n "$bin" ] || bin="$root/target/release/gordian-run"

# ---- 1. preflight ----

min_free=$((2 * 1024 * 1024 * 1024))
probe="$out"
while [ ! -e "$probe" ]; do probe="$(dirname "$probe")"; done
free="$(df -P -B1 "$probe" | awk 'NR==2 {print $4}')"
if [ -z "$free" ] || [ "$free" -lt "$min_free" ]; then
  die 10 "refusing: ${free:-unknown} bytes free under $probe, need at least $min_free"
fi

busy=""
for name in cargo rustc; do
  pids="$(pgrep -x "$name" || true)"
  [ -z "$pids" ] || busy="$busy $name(${pids//$'\n'/,})"
done
[ -z "$busy" ] || die 11 "refusing: a build is running:$busy. A build must not overlap a measurement."

head="$(git -C "$root" rev-parse HEAD)"
if [ "$source_revision" != "$head" ]; then
  die 12 "refusing: manifest source_revision $source_revision is not HEAD $head"
fi
if [ -n "$(git -C "$root" status --porcelain --untracked-files=normal)" ]; then
  die 13 "refusing: the working tree at $root is not clean. Commit or stash first; keep run outputs under artifacts/runs (git-ignored)."
fi

if [ -e "$out/results.csv" ] || [ -e "$out/measured.csv" ]; then
  die 14 "refusing: $out already holds results; a recorded run is never overwritten"
fi
[ -x "$bin" ] || die 21 "gordian-run binary $bin is missing or not executable (build it first, with no run in progress)"

# ---- 2. pin ----

taskset -cp 3 $$ >/dev/null || die 20 "cannot pin to core 3; refusing to run unpinned"

# ---- 3. launch ----

mkdir -p "$out"
report="$out/usage.json"
rm -f "$report"
set +e
timeout --kill-after=30 "$timeout_secs" \
  "$root/scripts/cgroup-run.sh" \
    --name "$run_id" --cpus "$cpus" --cpu-quota "$quota" --memory "$memory" \
    --report "$report" -- \
    "$bin" --manifest "$manifest" --out "$out"
status=$?
set -e
if [ "$status" -eq 124 ]; then
  echo "run-driver: the ${timeout_secs}s wall-clock backstop fired; the run was stopped" >&2
fi

# ---- 4. compare the two measurements ----

if [ ! -f "$report" ]; then
  echo "run-driver: the runner wrote no usage report (exit status $status)" >&2
  exit "$status"
fi

measured_sum="null"
ratio="null"
if [ -f "$out/measured.csv" ]; then
  measured_sum="$(awk -F, '
    NR == 1 { for (i = 1; i <= NF; i++) col[$i] = i; next }
    { s += $col["measured_component_ns"] + $col["measured_sched_ns"] + $col["measured_harness_ns"] }
    END { printf "%.0f", s }' "$out/measured.csv")"
  cpu_ns="$(jq -r '.cpu_ns // empty' "$report")"
  if [ -n "$cpu_ns" ] && [ "$cpu_ns" != "null" ] && [ "$cpu_ns" -gt 0 ]; then
    ratio="$(jq -n --argjson s "$measured_sum" --argjson c "$cpu_ns" '$s / $c')"
  fi
fi

tolerance="$(jq -c '.internal_external_ratio' "$manifest")"
within="null"
if [ "$ratio" != "null" ] && [ "$tolerance" != "null" ]; then
  within="$(jq -n --argjson r "$ratio" --argjson t "$tolerance" '($r >= $t.min) and ($r <= $t.max)')"
fi

tmp="$report.tmp"
jq --argjson r "$ratio" --argjson s "$measured_sum" --argjson t "$tolerance" --argjson w "$within" \
  '. + {internal_external_ratio: $r, measured_ns_sum: $s, ratio_tolerance: $t, ratio_within_tolerance: $w}' \
  "$report" > "$tmp"
mv "$tmp" "$report"

echo "run-driver: measured ${measured_sum} ns, internal_external_ratio ${ratio}, tolerance ${tolerance}" >&2
if [ "$status" -eq 0 ] && [ "$tolerance" = "null" ] && [ "$ratio" != "null" ]; then
  echo "run-driver: no tolerance declared; this first value is the starting point for one" >&2
fi
if [ "$within" = "false" ]; then
  echo "run-driver: internal_external_ratio ${ratio} is outside ${tolerance}: the instruments disagree" >&2
  [ "$status" -ne 0 ] || exit 4
fi
exit "$status"
