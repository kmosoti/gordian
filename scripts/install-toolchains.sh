#!/usr/bin/env bash
# Install repository-pinned development tools without embedding duplicate version literals.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
tool_root="${GORDIAN_TOOL_ROOT:-${HOME:?}/.local/gordian-tools}"
mkdir -p "$tool_root"

bootstrap_elan() (
  local elan_version elan_digest archive scratch
  elan_version="$(tr -d '\r\n' < "$root/.elan-version")"
  elan_digest="$(tr -d '\r\n' < "$root/.elan-sha256")"
  [ "$(uname -m)" = x86_64 ] \
    || { echo "the pinned elan release asset requires x86_64" >&2; exit 78; }
  scratch="$(mktemp -d "${TMPDIR:-/tmp}/gordian-elan-XXXXXX")"
  trap 'rm -rf "$scratch"' EXIT
  archive="$scratch/elan-x86_64-unknown-linux-gnu.tar.gz"
  curl --proto '=https' --tlsv1.2 -fsSL \
    "https://github.com/leanprover/elan/releases/download/v$elan_version/elan-x86_64-unknown-linux-gnu.tar.gz" \
    -o "$archive"
  printf '%s  %s\n' "$elan_digest" "$archive" | sha256sum -c -
  tar -C "$scratch" -xzf "$archive"
  "$scratch/elan-init" -y --default-toolchain none --no-modify-path
)

install_lean() {
  local elan_bin
  if ! command -v elan >/dev/null 2>&1; then
    export ELAN_HOME="${ELAN_HOME:-$tool_root/elan}"
    bootstrap_elan
    export PATH="$ELAN_HOME/bin:$PATH"
  fi
  command -v elan >/dev/null 2>&1 || { echo "elan bootstrap did not install elan" >&2; exit 78; }
  elan toolchain install "$(tr -d '\r\n' < "$root/formal/lean-toolchain")"
  elan_bin="$(dirname "$(command -v elan)")"
  echo "installed Lean tooling at $elan_bin; add $elan_bin to PATH"
}

install_python() (
  local version digest archive scratch prefix
  version="$(tr -d '\r\n' < "$root/.python-version")"
  digest="$(tr -d '\r\n' < "$root/.python-source-sha256")"
  scratch="$(mktemp -d "${TMPDIR:-/tmp}/gordian-python-XXXXXX")"
  trap 'rm -rf "$scratch"' EXIT
  archive="$scratch/Python-$version.tar.xz"
  curl --proto '=https' --tlsv1.2 -fsSL \
    "https://www.python.org/ftp/python/$version/Python-$version.tar.xz" -o "$archive"
  printf '%s  %s\n' "$digest" "$archive" | sha256sum -c -
  tar -C "$scratch" -xf "$archive"
  prefix="$tool_root/python-$version"
  (cd "$scratch/Python-$version" && ./configure --prefix="$prefix" && make -j2 && make install)
  echo "installed Python at $prefix/bin/python3.14; add $prefix/bin to PATH"
)

install_github() (
  local version digest archive scratch asset prefix
  version="$(tr -d '\r\n' < "$root/.github-cli-version")"
  digest="$(tr -d '\r\n' < "$root/.github-cli-sha256")"
  asset="gh_${version}_linux_amd64.tar.gz"
  scratch="$(mktemp -d "${TMPDIR:-/tmp}/gordian-gh-XXXXXX")"
  trap 'rm -rf "$scratch"' EXIT
  archive="$scratch/$asset"
  curl --proto '=https' --tlsv1.2 -fsSL \
    "https://github.com/cli/cli/releases/download/v$version/$asset" -o "$archive"
  printf '%s  %s\n' "$digest" "$archive" | sha256sum -c -
  tar -C "$scratch" -xzf "$archive"
  prefix="$tool_root/gh-$version"
  mkdir -p "$prefix"
  cp -R "$scratch/gh_${version}_linux_amd64"/. "$prefix"/
  echo "installed GitHub CLI at $prefix/bin/gh; add $prefix/bin to PATH"
)

install_cargo_deny() {
  cargo install cargo-deny --locked --version "$(tr -d '\r\n' < "$root/.cargo-deny-version")"
}

install_python_package() {
  local version environment interpreter
  version="$(tr -d '\r\n' < "$root/.python-version")"
  environment="$tool_root/python-env-$version"
  interpreter="$tool_root/python-$version/bin/python3.14"
  if [ ! -x "$interpreter" ]; then
    interpreter="$(command -v python3.14)" \
      || { echo "Python $version is not installed; run this script with 'python' first" >&2; exit 78; }
  fi
  "$interpreter" -m venv "$environment"
  "$environment/bin/python" -m pip install --disable-pip-version-check -e "$root/orchestration[dev]"
  echo "installed Python tooling at $environment; add $environment/bin to PATH"
}

case "${1:-all}" in
  lean) install_lean ;;
  python) install_python ;;
  github) install_github ;;
  cargo-deny) install_cargo_deny ;;
  python-package) install_python_package ;;
  all)
    install_lean
    install_python
    install_github
    install_cargo_deny
    install_python_package
    ;;
  *) echo "usage: scripts/install-toolchains.sh [lean|python|github|cargo-deny|python-package|all]" >&2; exit 64 ;;
esac
