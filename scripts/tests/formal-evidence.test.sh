#!/usr/bin/env bash
# write-formal-evidence.sh must establish the formal verdict itself. Tool
# metadata, a caller-controlled state, or a clean-looking fake verifier cannot
# mint a pass claim.
set -euo pipefail
# shellcheck source=scripts/tests/harness.sh
# shellcheck disable=SC1091
. "$(dirname "$0")/harness.sh"

state=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
fake_state=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa

make_git() {
  local fixture="$1"
  mkdir -p "$fixture/fake-bin"
  cat >"$fixture/fake-bin/git" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [ "${1:-}" = -C ]; then
  shift 2
fi
case "${1:-}" in
  rev-parse) printf '%s\n' "${FAKE_GIT_STATE:?}" ;;
  status) printf '%s\n' "${FAKE_GIT_STATUS:-}" ;;
  *) exit 64 ;;
esac
EOF
  chmod +x "$fixture/fake-bin/git"
}

# Matching a caller-forged state still cannot pass: the checked-out source has
# no verifier entrypoint and therefore cannot produce evidence.
no_verifier="$(new_fixture scripts/write-formal-evidence.sh)"
make_git "$no_verifier"
no_verifier_output="$FIXTURE_PARENT/no-verifier-evidence.json"
expect_fail "tool metadata without a verifier cannot mint evidence" \
  env PATH="$no_verifier/fake-bin:$PATH" FAKE_GIT_STATE="$state" \
  GITHUB_SHA="$fake_state" bash "$no_verifier/scripts/write-formal-evidence.sh" \
  "$state" "$no_verifier_output"
[ ! -e "$no_verifier_output" ] || {
  echo "   FAIL [$TEST_NAME] no-verifier path wrote evidence"
  exit 1
}

# Dirty formal sources are rejected before the formal gate or output path is
# touched, even when the fake repository reports the requested exact state.
dirty="$(new_fixture scripts/write-formal-evidence.sh)"
make_git "$dirty"
dirty_output="$FIXTURE_PARENT/dirty-evidence.json"
expect_fail "dirty formal sources cannot mint evidence" \
  env PATH="$dirty/fake-bin:$PATH" FAKE_GIT_STATE="$state" \
  FAKE_GIT_STATUS=' M formal/Gordian/Graph.lean' \
  bash "$dirty/scripts/write-formal-evidence.sh" "$state" "$dirty_output"
[ ! -e "$dirty_output" ] || {
  echo "   FAIL [$TEST_NAME] dirty-source path wrote evidence"
  exit 1
}

# A valid invocation must call the complete formal verifier before writing.
valid="$(new_fixture scripts/write-formal-evidence.sh)"
mkdir -p "$valid/formal/Gordian" "$valid/fake-bin"
printf 'leanprover/lean4:v4.19.0\n' >"$valid/formal/lean-toolchain"
printf 'audit source\n' >"$valid/formal/Gordian/Audit.lean"
printf '#!/usr/bin/env bash\nexit 0\n' >"$valid/formal/leanchecker"
chmod +x "$valid/formal/leanchecker"
cat >"$valid/scripts/verify-formal.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >"${FAKE_VERIFIER_LOG:?}"
EOF
chmod +x "$valid/scripts/verify-formal.sh"
cat >"$valid/fake-bin/lake" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
case "$*" in
  *'env lean --version'*) printf 'Lean (version 4.19.0, release)\n' ;;
  *'env bash -c command -v leanchecker'*) printf '%s/leanchecker\n' "$PWD" ;;
  *) exit 0 ;;
esac
EOF
chmod +x "$valid/fake-bin/lake"
make_git "$valid"
verifier_log="$valid/verifier.log"
valid_output="$FIXTURE_PARENT/valid-evidence.json"
expect_ok "valid invocation runs the complete verifier before writing" \
  env PATH="$valid/fake-bin:$PATH" FAKE_GIT_STATE="$state" \
  FAKE_VERIFIER_LOG="$verifier_log" bash "$valid/scripts/write-formal-evidence.sh" \
  "$state" "$valid_output"
grep -Fqx -- '--self-test' "$verifier_log" || {
  echo "   FAIL [$TEST_NAME] valid invocation omitted formal self-tests"
  exit 1
}
python3 - "$valid_output" <<'PY'
import json
import sys

record = json.load(open(sys.argv[1], encoding="utf-8"))
assert record["exact_state_id"] == "b" * 40
assert record["verdict"] == "pass"
assert all(check["verdict"] == "pass" for check in record["checks"])
PY

# A checked-out state mismatch remains a hard rejection; GITHUB_SHA is not an
# observation source.
mismatch="$(new_fixture scripts/write-formal-evidence.sh)"
make_git "$mismatch"
mismatch_output="$FIXTURE_PARENT/mismatch-evidence.json"
expect_fail "caller-controlled exact state is rejected" \
  env PATH="$mismatch/fake-bin:$PATH" FAKE_GIT_STATE="$state" \
  GITHUB_SHA="$fake_state" bash "$mismatch/scripts/write-formal-evidence.sh" \
  "$fake_state" "$mismatch_output"
[ ! -e "$mismatch_output" ] || {
  echo "   FAIL [$TEST_NAME] mismatch path wrote evidence"
  exit 1
}

inside="$(new_fixture scripts/write-formal-evidence.sh)"
make_git "$inside"
expect_fail "repository-local output cannot mutate the attested state" \
  env PATH="$inside/fake-bin:$PATH" FAKE_GIT_STATE="$state" \
  bash "$inside/scripts/write-formal-evidence.sh" "$state" "$inside/evidence.json"
[ ! -e "$inside/evidence.json" ] || {
  echo "   FAIL [$TEST_NAME] repository-local path wrote evidence"
  exit 1
}
