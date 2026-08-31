#!/usr/bin/env bash
# Validate the committed declarative Atom-contract normalization manifest.
#
# This is intentionally source-only: it checks manifest coverage, canonical target
# ownership, and Closure wording without consulting GitHub or a cached snapshot.
set -euo pipefail
cd "$(dirname "$0")/.."

PYTHONPATH="orchestration/src${PYTHONPATH:+:$PYTHONPATH}" \
  python3 -m gordian_orchestration.atom_registry check-normalization
