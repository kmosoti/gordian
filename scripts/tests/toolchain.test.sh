#!/usr/bin/env bash
# check-toolchain.sh: valid pins pass, while a malformed pin fails without invoking a runtime
# installer or downloading anything.
set -euo pipefail
# shellcheck source=scripts/tests/harness.sh
# shellcheck disable=SC1091
. "$(dirname "$0")/harness.sh"

support=(
  scripts/check-toolchain.sh
  scripts/install-toolchains.sh
  rust-toolchain.toml
  Cargo.toml
  formal/lean-toolchain
  .python-version
  .python-source-sha256
  .github-cli-version
  .github-cli-sha256
  .cargo-deny-version
  .elan-version
  .elan-sha256
  orchestration/pyproject.toml
  .github/workflows/verify.yml
)

good="$(new_fixture "${support[@]}")"
expect_ok "the static pin contract passes" bash "$good/scripts/check-toolchain.sh"

malformed="$(new_fixture "${support[@]}")"
printf '%s\n' 'not-a-python-pin' > "$malformed/.python-version"
expect_fail "a malformed owning pin is rejected without runtime tools" \
  bash "$malformed/scripts/check-toolchain.sh"

installer_drift="$(new_fixture "${support[@]}")"
python3 - "$installer_drift/scripts/install-toolchains.sh" <<'PY'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
text = text.replace('"$root/formal/lean-toolchain"', '"$root/formal/missing-lean-toolchain"')
open(path, "w", encoding="utf-8").write(text)
PY
expect_fail "an installer that drops a pin-file read is rejected" \
  bash "$installer_drift/scripts/check-toolchain.sh"

floating_backend="$(new_fixture "${support[@]}")"
sed -i 's/hatchling==1\.27\.0/hatchling>=1.27/' \
  "$floating_backend/orchestration/pyproject.toml"
expect_fail "a floating Python build backend is rejected" \
  bash "$floating_backend/scripts/check-toolchain.sh"
