#!/usr/bin/env bash
# W2: the whole pipeline, in order, from a committed tree. Every step that takes more than a
# minute or touches a measurement goes through the isolation runner on cores 0-2.
#
#   1. build (no run in progress; `pgrep -x gordian-run` must print nothing):
#        scripts/cgroup-run.sh --name world-laws-build --cpus 0-2 --memory 3G -- \
#          cargo build -p gordian-stream --features reveal-hidden-state --example laws
#        scripts/cgroup-run.sh --name world-laws-build --cpus 0-2 --memory 3G -- \
#          cargo build --release -p gordian-run
#   2. the hidden-side tables of the three ranges:       w2_hidden.sh
#   3. the ceiling's runs (item 6), manifests then driver, one per range:
#        w2_manifests.py tune;    scripts/run-driver.sh --manifest artifacts/runs/w2/_manifests/w2-tune-b5-rho0.7.json    --out artifacts/runs/w2/w2-tune-b5-rho0.7
#        w2_manifests.py heldout; scripts/run-driver.sh --manifest artifacts/runs/w2/_manifests/w2-heldout-b5-rho0.7.json --out artifacts/runs/w2/w2-heldout-b5-rho0.7
#   4. this script: the tables and the report.
#
# Usage: w2_run.sh [PYTHON]    (default: the analysis virtual environment's python)
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
py="${1:-/home/user/gordian/analysis/.venv/bin/python}"
here="$root/experiments/exploration/scripts"
run() { "$root/scripts/cgroup-run.sh" --name world-laws-run --cpus 0-2 --memory 2G -- "$py" -B "$@"; }
run "$here/w2_laws.py" --hidden-root "$root/artifacts/runs/w2/hidden" --out-dir "$root/experiments/exploration"
run "$here/w2_ceiling.py" \
  --run a-tune="$root/artifacts/runs/w2/w2-tune-b5-rho0.7" \
  --run a-heldout="$root/artifacts/runs/w2/w2-heldout-b5-rho0.7" \
  --l1-dir "${L1_DIR:-/home/user/gordian/artifacts/runs/l1/l1-fresh-b5-rho0.7}" \
  --hidden-root "$root/artifacts/runs/w2/hidden" --out-dir "$root/experiments/exploration"
run "$here/w2_assemble.py"
