#!/usr/bin/env bash
# R8: run one phase of calls against llama-server, isolated as the plan requires.
#
# Usage: r8_run.sh RUN_ID DESIGN.json QUESTIONS.jsonl MODEL.gguf [MODEL_LABEL]
#
# * The server (the only process that does inference) runs under scripts/cgroup-run.sh: cores 0-2,
#   a 300% CPU quota, 6 GB memory, 3 threads, one slot, one model.
# * The client (r8_run.py: builds prompts, records calls) runs pinned to core 3.
# * The run directory is artifacts/runs/RUN_ID under the repository this script is in.
# * It refuses to start while cargo, rustc or another llama process runs, or with under 4 GB free.
# * timeout(1) is the backstop: the whole run is stopped after RUN_TIMEOUT seconds (default 15000).
#
# The server is stopped with SIGTERM when the client exits; its exit status is therefore not 0, and
# the usage report (usage.json) is the runner's account of CPU, memory and OOM kills.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
scripts="$root/experiments/exploration/scripts"
run_id="$1"
design="$2"
questions="$3"
model="$4"
label="${5:-$(basename "$model")}"

llama="${LLAMA_BIN:-/home/user/gordian/artifacts/runtime/build/bin/llama-server}"
port="${R8_PORT:-8089}"
timeout_s="${RUN_TIMEOUT:-15000}"
out="$root/artifacts/runs/$run_id"

if pgrep -x cargo >/dev/null || pgrep -x rustc >/dev/null || pgrep -f 'llama-(server|cli|bench)' >/dev/null; then
    echo "r8_run: a build or another inference process is running; refusing" >&2
    exit 3
fi
free_kb=$(df --output=avail -k "$root" | tail -1)
if [ "$free_kb" -lt 4194304 ]; then
    echo "r8_run: under 4 GB free; refusing" >&2
    exit 3
fi
mkdir -p "$out"

# The server: greedy decoding is requested per call; the server's own seed is fixed too. The
# prompt cache in host memory is off (it would hold earlier prompts' states and grow); the slot's
# own prefix reuse (cache_prompt) is what keeps the shared system prompt and examples cached.
"$root/scripts/cgroup-run.sh" --name "$run_id" --cpus 0-2 --cpu-quota 300 --memory 6G \
    --report "$out/usage.json" -- \
    timeout "$timeout_s" "$llama" -m "$model" -c 16384 -t 3 -tb 3 -np 1 -cram 0 -fa on \
    --host 127.0.0.1 --port "$port" --seed 1 --no-webui \
    >"$out/server.log" 2>&1 &
server_wait=$!

status=0
taskset -c 3 python3 "$scripts/r8_run.py" --design "$design" --questions "$questions" \
    --demos "$scripts/../r8-demos.json" --out "$out" --url "http://127.0.0.1:$port" \
    --model-label "$label" >>"$out/client.log" 2>&1 || status=$?

# Stop the server process only (not the runner around it, which must write usage.json).
pkill -TERM -x llama-server || true
wait "$server_wait" || true
echo "client exit status: $status" >>"$out/client.log"
exit "$status"
