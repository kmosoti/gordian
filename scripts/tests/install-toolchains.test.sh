#!/usr/bin/env bash
# install-toolchains.sh: invalid targets fail before any network-backed installation path.
set -euo pipefail
# shellcheck source=scripts/tests/harness.sh
# shellcheck disable=SC1091
. "$(dirname "$0")/harness.sh"

fixture="$(new_fixture scripts/install-toolchains.sh)"
expect_fail "an unknown installation target is rejected without downloads" \
  env GORDIAN_TOOL_ROOT="$fixture/tool-root" \
  bash "$fixture/scripts/install-toolchains.sh" definitely-not-a-tool

lean_fixture="$(new_fixture scripts/install-toolchains.sh formal/lean-toolchain .elan-version .elan-sha256)"
mock_bin="$lean_fixture/mock-bin"
elan_home="$lean_fixture/elan"
curl_log="$lean_fixture/curl.log"
sha_log="$lean_fixture/sha256sum.log"
mkdir -p "$mock_bin"
python3 - "$mock_bin/curl" "$mock_bin/sha256sum" "$mock_bin/tar" <<'PY'
import os
import stat
import sys

def write_executable(path, script):
    with open(path, 'w', encoding='utf-8') as handle:
        handle.write(script)
    os.chmod(path, stat.S_IRWXU)

write_executable(sys.argv[1], r'''#!/bin/sh
set -eu
printf '%s\n' "$*" > "$MOCK_CURL_LOG"
output=
while [ "$#" -gt 0 ]; do
    if [ "$1" = -o ]; then
        output="$2"
        shift 2
    else
        shift
    fi
done
: > "$output"
''')
write_executable(sys.argv[2], r'''#!/bin/sh
set -eu
cat > "$MOCK_SHA_LOG"
''')
write_executable(sys.argv[3], r'''#!/bin/sh
set -eu
destination=
while [ "$#" -gt 0 ]; do
    if [ "$1" = -C ]; then
        destination="$2"
        shift 2
    else
        shift
    fi
done
cat > "$destination/elan-init" <<'ELAN_INIT'
#!/bin/sh
set -eu
mkdir -p "${ELAN_HOME:?}/bin"
printf '%s\n' "$*" > "${ELAN_HOME:?}/installer-args"
cat > "${ELAN_HOME:?}/bin/elan" <<'ELAN'
#!/bin/sh
set -eu
printf '%s\n' "$*" > "${ELAN_HOME:?}/elan-args"
[ "$1" = toolchain ] && [ "$2" = install ]
printf '%s\n' "$3" > "${ELAN_HOME:?}/installed"
ELAN
chmod +x "${ELAN_HOME:?}/bin/elan"
ELAN_INIT
chmod +x "$destination/elan-init"
''')
PY
if lean_output="$(env GORDIAN_TOOL_ROOT="$lean_fixture/tool-root" \
  ELAN_HOME="$elan_home" \
  MOCK_CURL_LOG="$curl_log" \
  MOCK_SHA_LOG="$sha_log" \
  PATH="$mock_bin:/usr/bin:/bin" \
  bash "$lean_fixture/scripts/install-toolchains.sh" lean 2>&1)"; then
  echo "   ok   [$TEST_NAME] missing elan bootstraps through the official installer without network access"
else
  echo "   FAIL [$TEST_NAME] missing elan bootstrap failed"
  printf '%s\n' "$lean_output" | sed 's/^/          /'
  exit 1
fi
grep -F -- "--proto =https --tlsv1.2 -fsSL https://github.com/leanprover/elan/releases/download/v4.2.4/elan-x86_64-unknown-linux-gnu.tar.gz" "$curl_log" >/dev/null
expect_ok "elan bootstrap verifies the pinned release digest" \
  test "$(grep -F -c -- "42b94d4244e8353142c456ec0e4ca6528fd898a6c604d4059f494e706e431f63" "$sha_log")" -eq 1
expect_ok "elan bootstrap disables default-toolchain drift" \
  test "$(cat "$elan_home/installer-args")" = "-y --default-toolchain none --no-modify-path"
expect_ok "elan installs the exact repository Lean pin" \
  test "$(cat "$elan_home/installed")" = "$(tr -d '\r\n' < "$lean_fixture/formal/lean-toolchain")"
expect_ok "elan bootstrap reports the bin directory needed by the next command" \
  grep -Fq "add $elan_home/bin to PATH" <<<"$lean_output"
