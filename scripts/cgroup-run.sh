#!/usr/bin/env bash
# Run one command under hard CPU and memory limits and report what it actually used.
#
# Prefers cgroup v2 (unified hierarchy with cpu, cpuset and memory controllers). Falls back to
# cgroup v1 when v2 lacks those controllers, which is the layout of the current development
# machine. If neither is writable, refuses unless --allow-unisolated is given, in which case it
# only pins CPUs with taskset and caps address space with prlimit, and says so in the report.
#
# Usage:
#   scripts/cgroup-run.sh --name NAME [--cpus LIST] [--cpu-quota PCT] [--memory BYTES]
#                         [--report FILE] [--allow-unisolated] -- COMMAND [ARGS...]
#
#   --name        cgroup leaf name, e.g. exp001-selective-seed7
#   --cpus        cpuset, default 0-2 (core 3 is reserved for evaluator and recorder)
#   --cpu-quota   percent of one core the group may use in total, default 300
#   --memory      memory limit in bytes (suffixes K, M, G accepted), default 4G
#   --report      JSON file to write usage into, default stderr summary only
#
# The report records: isolation mode, the limits, exit code, wall nanoseconds, CPU nanoseconds
# consumed by the group, peak memory bytes, and OOM kill count. Those numbers are the resource
# side of an experiment's results table; the arm's own accounting must agree with them.

set -euo pipefail

name=""
cpus="0-2"
quota_pct=300
memory="4G"
report=""
allow_unisolated=0

to_bytes() {
  local v="$1"
  case "$v" in
    *K|*k) echo $(( ${v%?} * 1024 )) ;;
    *M|*m) echo $(( ${v%?} * 1024 * 1024 )) ;;
    *G|*g) echo $(( ${v%?} * 1024 * 1024 * 1024 )) ;;
    *) echo "$v" ;;
  esac
}

while [ $# -gt 0 ]; do
  case "$1" in
    --name) name="$2"; shift 2 ;;
    --cpus) cpus="$2"; shift 2 ;;
    --cpu-quota) quota_pct="$2"; shift 2 ;;
    --memory) memory="$2"; shift 2 ;;
    --report) report="$2"; shift 2 ;;
    --allow-unisolated) allow_unisolated=1; shift ;;
    --) shift; break ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done
[ -n "$name" ] || { echo "--name is required" >&2; exit 2; }
[ $# -gt 0 ] || { echo "no command given after --" >&2; exit 2; }
case "$name" in */*|.|..) echo "--name must be a single path component" >&2; exit 2 ;; esac

mem_bytes=$(to_bytes "$memory")
period_us=100000
quota_us=$(( quota_pct * period_us / 100 ))

mode="none"
v2_root=""
v1_paths=()

if [ -f /sys/fs/cgroup/cgroup.controllers ] \
   && grep -qw cpu /sys/fs/cgroup/cgroup.controllers \
   && grep -qw memory /sys/fs/cgroup/cgroup.controllers \
   && grep -qw cpuset /sys/fs/cgroup/cgroup.controllers; then
  mode="v2"
  v2_root="/sys/fs/cgroup/gordian"
elif [ -d /sys/fs/cgroup/memory ] && [ -d /sys/fs/cgroup/cpu ] && [ -d /sys/fs/cgroup/cpuset ] \
     && [ -w /sys/fs/cgroup/memory ]; then
  mode="v1"
fi

if [ "$mode" = "none" ] && [ "$allow_unisolated" -eq 0 ]; then
  echo "no writable cgroup v2 or v1 hierarchy; refusing. Pass --allow-unisolated for development runs only." >&2
  exit 3
fi

leaf=""
cleanup() {
  case "$mode" in
    v2) [ -n "$leaf" ] && rmdir "$leaf" 2>/dev/null || true ;;
    v1) for p in "${v1_paths[@]}"; do rmdir "$p" 2>/dev/null || true; done ;;
  esac
}
trap cleanup EXIT

case "$mode" in
  v2)
    mkdir -p "$v2_root"
    # Enable controllers for children of the gordian node.
    for c in cpu cpuset memory; do
      grep -qw "$c" "$v2_root/cgroup.subtree_control" 2>/dev/null \
        || echo "+$c" > "$v2_root/cgroup.subtree_control" 2>/dev/null || true
    done
    leaf="$v2_root/$name"
    mkdir "$leaf"
    echo "$cpus" > "$leaf/cpuset.cpus"
    echo "$quota_us $period_us" > "$leaf/cpu.max"
    echo "$mem_bytes" > "$leaf/memory.max"
    echo 0 > "$leaf/memory.swap.max" 2>/dev/null || true
    ;;
  v1)
    for c in memory cpu cpuacct cpuset; do
      p="/sys/fs/cgroup/$c/gordian-$name"
      mkdir -p "$p"
      v1_paths+=("$p")
    done
    echo "$mem_bytes" > "/sys/fs/cgroup/memory/gordian-$name/memory.limit_in_bytes"
    echo 0 > "/sys/fs/cgroup/memory/gordian-$name/memory.swappiness" 2>/dev/null || true
    echo "$period_us" > "/sys/fs/cgroup/cpu/gordian-$name/cpu.cfs_period_us"
    echo "$quota_us" > "/sys/fs/cgroup/cpu/gordian-$name/cpu.cfs_quota_us"
    # cpuset requires mems to be set before any task can join.
    cat /sys/fs/cgroup/cpuset/cpuset.mems > "/sys/fs/cgroup/cpuset/gordian-$name/cpuset.mems"
    echo "$cpus" > "/sys/fs/cgroup/cpuset/gordian-$name/cpuset.cpus"
    ;;
esac

start_ns=$(date +%s%N)
set +e
case "$mode" in
  v2)
    ( echo $BASHPID > "$leaf/cgroup.procs" && exec "$@" )
    status=$?
    ;;
  v1)
    (
      for c in memory cpu cpuacct cpuset; do
        echo $BASHPID > "/sys/fs/cgroup/$c/gordian-$name/cgroup.procs"
      done
      exec "$@"
    )
    status=$?
    ;;
  none)
    ( exec prlimit --as="$mem_bytes" taskset -c "$cpus" "$@" )
    status=$?
    ;;
esac
set -e
end_ns=$(date +%s%N)
wall_ns=$(( end_ns - start_ns ))

cpu_ns="null"
peak_bytes="null"
oom_kills="null"
case "$mode" in
  v2)
    usage_usec=$(awk '/^usage_usec/ {print $2}' "$leaf/cpu.stat")
    cpu_ns=$(( usage_usec * 1000 ))
    [ -f "$leaf/memory.peak" ] && peak_bytes=$(cat "$leaf/memory.peak")
    oom_kills=$(awk '/^oom_kill / {print $2}' "$leaf/memory.events")
    ;;
  v1)
    cpu_ns=$(cat "/sys/fs/cgroup/cpuacct/gordian-$name/cpuacct.usage")
    peak_bytes=$(cat "/sys/fs/cgroup/memory/gordian-$name/memory.max_usage_in_bytes")
    oom_kills=$(awk '/^oom_kill / {print $2}' "/sys/fs/cgroup/memory/gordian-$name/memory.oom_control")
    ;;
esac

json=$(cat <<EOF
{
  "name": "$name",
  "isolation": "$mode",
  "limits": { "cpus": "$cpus", "cpu_quota_percent": $quota_pct, "memory_bytes": $mem_bytes },
  "exit_code": $status,
  "wall_ns": $wall_ns,
  "cpu_ns": $cpu_ns,
  "peak_memory_bytes": $peak_bytes,
  "oom_kills": $oom_kills
}
EOF
)
if [ -n "$report" ]; then
  printf '%s\n' "$json" > "$report"
else
  printf '%s\n' "$json" >&2
fi
exit "$status"
