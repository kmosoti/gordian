#!/usr/bin/env bash
# Single command source for local verification and every CI `run:` verifier step.
set -euo pipefail
shopt -s nullglob

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
group="${1:-all}"

require() {
  command -v "$1" >/dev/null || { echo "MISSING TOOL: $1" >&2; exit 78; }
}

verify_rust_check() {
  require cargo
  require cargo-deny
  scripts/check-toolchain.sh --runtime rust cargo-deny
  cargo fmt --all -- --check
  cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
  cargo test --locked --workspace
  cargo deny check
}

verify_kg_audit() {
  require cargo
  cargo run --locked -p gordian-kg -- validate
  cargo run --locked -p gordian-kg -- audit --strict
}

verify_rust() {
  verify_rust_check
  verify_kg_audit
}

verify_formal() {
  require lake
  scripts/check-toolchain.sh --runtime lean
  scripts/verify-formal.sh --self-test
}

verify_python() {
  require python3.14
  require ruff
  scripts/check-toolchain.sh --runtime python ruff
  ruff check orchestration
  python3.14 -m compileall -q orchestration/src
  python3.14 -m unittest discover -s orchestration/tests
}

verify_spec() {
  require shellcheck
  require python3
  checks=(scripts/check-*.sh)
  [ "${#checks[@]}" -gt 0 ] || { echo "FAILED: no scripts/check-*.sh found" >&2; exit 1; }
  for checker in "${checks[@]}"; do
    bash "$checker"
  done
  shellcheck scripts/*.sh scripts/tests/*.sh
  bash -n scripts/*.sh scripts/tests/*.sh
  python3 -m compileall -q scripts/
}

case "$group" in
  rust) verify_rust ;;
  rust-check) verify_rust_check ;;
  kg-audit) verify_kg_audit ;;
  formal) verify_formal ;;
  python) verify_python ;;
  spec-consistency) verify_spec ;;
  all)
    verify_rust
    verify_formal
    verify_python
    verify_spec
    ;;
  *) echo "usage: scripts/verify-local.sh [rust|rust-check|kg-audit|formal|python|spec-consistency|all]" >&2; exit 64 ;;
esac
