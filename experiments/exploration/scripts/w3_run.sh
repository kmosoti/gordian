#!/usr/bin/env bash
# W3: the whole pipeline, in order, from a committed tree. Every step that takes more than a minute
# or touches a measurement goes through the isolation runner on cores 0-2.
#
#   0. preflight before every cargo command and every run: `df -h /home/user` (stop under 6 GB) and
#      `pgrep -x gordian-run` (wait while a run is in progress, polling every 30 s).
#   1. build (no run in progress):
#        scripts/cgroup-run.sh --name world-c-build --cpus 0-2 --memory 3G -- \
#          cargo build -p gordian-stream --features reveal-hidden-state --example laws
#        scripts/cgroup-run.sh --name world-c-build --cpus 0-2 --memory 3G -- cargo build --release -p gordian-run
#   2. the hidden-side tables of the five ranges:      w3_hidden.sh
#   3. the ceiling's runs on world C, manifests then driver, one per range (commit first: the
#      manifest records the revision and the driver refuses a dirty tree):
#        w3_manifests.py ctune;   scripts/run-driver.sh --manifest artifacts/runs/w3/_manifests/w3-ctune-b5-rho0.7.json   --out artifacts/runs/w3/w3-ctune-b5-rho0.7
#        w3_manifests.py cheldout; scripts/run-driver.sh --manifest artifacts/runs/w3/_manifests/w3-cheldout-b5-rho0.7.json --out artifacts/runs/w3/w3-cheldout-b5-rho0.7
#   4. this script: the tables and the report (needs W2's kept runs under /home/user/gordian/artifacts/runs/w2,
#      E1's held-out run and A2's smoke files; set W2_ROOT, E1_RUN, A2_RUN to relocate them).
#
# Usage: w3_run.sh [PYTHON]    (default: the analysis virtual environment's python)
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
py="${1:-/home/user/gordian/analysis/.venv/bin/python}"
here="$root/experiments/exploration/scripts"
e1="${E1_RUN:-/home/user/gordian/artifacts/runs/e1/e1-heldout-b5-rho0.7}"
a2="${A2_RUN:-/home/user/gordian/artifacts/runs/a2/a2-smoke-b5-rho0.7}"
run() { "$root/scripts/cgroup-run.sh" --name world-c-run --cpus 0-2 --memory 2G -- "$py" -B "$@"; }
run "$here/w3_laws.py"
"$here/w3_ceiling.sh" "$py"
run "$here/w3_bounds.py"
run "$here/w3_pairs.py"
run "$here/w3_floor.py"
run "$here/w3_e1join.py" --e1-run "$e1"
for arm in a2_learn a2_raw; do
  run "$here/w3_score.py" --predictions "$a2/predictions-$arm.csv" --first-alarms "$a2/first-alarms-$arm.csv" \
    --edges "$a2/edges-$arm.csv" --hidden "$root/artifacts/runs/w3/hidden/a-a2" --arm "$arm"
done
run "$here/w3_provenance.py"
run "$here/w3_assemble.py"
