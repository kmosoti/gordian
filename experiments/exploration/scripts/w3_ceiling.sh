#!/usr/bin/env bash
# W3: the ceiling tables of world C (and of world A, from W2's kept runs, for the comparison).
# W2's run directories and hidden tables are read, never written; pass W2_ROOT to relocate them.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
py="${1:-/home/user/gordian/analysis/.venv/bin/python}"
w2="${W2_ROOT:-/home/user/gordian/artifacts/runs/w2}"
"$root/scripts/cgroup-run.sh" --name world-c-run --cpus 0-2 --memory 2G -- "$py" -B "$root/experiments/exploration/scripts/w3_ceiling.py" \
  --run a-tune="$w2/w2-tune-b5-rho0.7@$w2/hidden" \
  --run a-heldout="$w2/w2-heldout-b5-rho0.7@$w2/hidden" \
  --run c-tune="$root/artifacts/runs/w3/w3-ctune-b5-rho0.7" \
  --run c-heldout="$root/artifacts/runs/w3/w3-cheldout-b5-rho0.7"
