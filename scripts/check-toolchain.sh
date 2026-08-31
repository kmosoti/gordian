#!/usr/bin/env bash
# Validate the repository's toolchain pins, or compare the current runtime with those pins.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

fail() { echo "FAIL: $*" >&2; exit 1; }
one_line() {
  local file="$1"
  local value
  value="$(tr -d '\r\n' < "$file")"
  [ -n "$value" ] || fail "$file is empty"
  [ "$(wc -l < "$file")" -le 1 ] || fail "$file must contain exactly one value"
  printf '%s' "$value"
}

rust_pin="$(sed -n 's/^channel *= *"\([^"]*\)"/\1/p' rust-toolchain.toml)"
cargo_rust="$(sed -n 's/^rust-version *= *"\([^"]*\)"/\1/p' Cargo.toml)"
lean_pin="$(one_line formal/lean-toolchain)"
python_pin="$(one_line .python-version)"
python_digest="$(one_line .python-source-sha256)"
gh_pin="$(one_line .github-cli-version)"
gh_digest="$(one_line .github-cli-sha256)"
deny_pin="$(one_line .cargo-deny-version)"
elan_pin="$(one_line .elan-version)"
elan_digest="$(one_line .elan-sha256)"
ruff_pin="$(sed -n 's/^dev = \["ruff==\([^"]*\)"\]$/\1/p' orchestration/pyproject.toml)"
hatchling_pin="$(sed -n 's/^requires = \["hatchling==\([^"]*\)"\]$/\1/p' orchestration/pyproject.toml)"

[[ "$rust_pin" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "invalid Rust pin"
[ "$cargo_rust" = "${rust_pin%.*}" ] || fail "Cargo rust-version does not match Rust channel"
[[ "$lean_pin" =~ ^leanprover/lean4:v[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "invalid Lean pin"
[[ "$python_pin" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "invalid Python pin"
[[ "$python_digest" =~ ^[0-9a-f]{64}$ ]] || fail "invalid Python source digest"
[[ "$gh_pin" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "invalid GitHub CLI pin"
[[ "$gh_digest" =~ ^[0-9a-f]{64}$ ]] || fail "invalid GitHub CLI digest"
[[ "$deny_pin" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "invalid cargo-deny pin"
[[ "$elan_pin" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "invalid elan pin"
[[ "$elan_digest" =~ ^[0-9a-f]{64}$ ]] || fail "invalid elan digest"
[[ "$ruff_pin" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "ruff must be exactly pinned"
[[ "$hatchling_pin" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "hatchling must be exactly pinned"

grep -q 'python-version-file: ".python-version"' .github/workflows/verify.yml \
  || fail "verify workflow does not consume .python-version"
grep -q 'cat .cargo-deny-version' .github/workflows/verify.yml \
  || fail "verify workflow does not consume .cargo-deny-version"
for pin in .python-version .python-source-sha256 .github-cli-version .github-cli-sha256 \
  .cargo-deny-version formal/lean-toolchain .elan-version .elan-sha256; do
  grep -q "$pin" scripts/install-toolchains.sh || fail "installer does not consume $pin"
done

if [ "${1:-}" != "--runtime" ]; then
  [ "$#" -eq 0 ] || { echo "usage: scripts/check-toolchain.sh [--runtime [tool...]]" >&2; exit 64; }
  echo "OK: static toolchain pins are single-sourced and internally consistent"
  exit 0
fi
shift
tools=("$@")
if [ "${#tools[@]}" -eq 0 ]; then
  tools=(rust lean python github cargo-deny ruff)
fi

for tool in "${tools[@]}"; do
  case "$tool" in
    rust)
      command -v rustc >/dev/null || exit 78
      actual="$(rustc --version | awk '{print $2}')"
      [ "$actual" = "$rust_pin" ] || fail "rustc $actual != $rust_pin"
      ;;
    lean)
      command -v lake >/dev/null || exit 78
      actual="$(cd formal && lake env lean --version | sed -n 's/^Lean (version \([^,]*\).*/\1/p')"
      [ "leanprover/lean4:v$actual" = "$lean_pin" ] || fail "Lean $actual != $lean_pin"
      (cd formal && lake env bash -c 'command -v leanchecker >/dev/null') \
        || fail "pinned Lean toolchain has no leanchecker"
      ;;
    python)
      command -v python3.14 >/dev/null || exit 78
      actual="$(python3.14 --version | awk '{print $2}')"
      [ "$actual" = "$python_pin" ] || fail "Python $actual != $python_pin"
      ;;
    github)
      command -v gh >/dev/null || exit 78
      actual="$(gh --version | sed -n '1s/^gh version \([^ ]*\).*/\1/p')"
      [ "$actual" = "$gh_pin" ] || fail "gh $actual != $gh_pin"
      ;;
    cargo-deny)
      command -v cargo-deny >/dev/null || exit 78
      actual="$(cargo deny --version | awk '{print $2}')"
      [ "$actual" = "$deny_pin" ] || fail "cargo-deny $actual != $deny_pin"
      ;;
    ruff)
      command -v ruff >/dev/null || exit 78
      actual="$(ruff --version | awk '{print $2}')"
      [ "$actual" = "$ruff_pin" ] || fail "ruff $actual != $ruff_pin"
      ;;
    *) fail "unknown runtime tool: $tool" ;;
  esac
done

echo "OK: runtime toolchain matches repository pins (${tools[*]})"
