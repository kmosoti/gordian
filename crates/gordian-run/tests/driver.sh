#!/usr/bin/env bash
# Shell test for scripts/run-driver.sh and the extended scripts/check-no-oracle.sh.
#
# It copies the driver into a throwaway git repository, so it can create stale revisions and
# dirty trees without touching this one, and puts shims for pgrep, df and taskset first on PATH,
# so every refusal can be provoked whatever else the machine is doing (a real build on a shared
# machine would otherwise turn a revision test into a "build running" refusal). The runner
# `scripts/cgroup-run.sh` and the `gordian-run` binary are stubs here: this test is about the
# driver's own logic (refusals, pinning, arguments, timeout, the ratio), not about cgroups. The
# real launch is the coordinator's.
#
# Run from anywhere:  bash crates/gordian-run/tests/driver.sh

set -uo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$here/../../.." && pwd)"

work="$(mktemp -d "${TMPDIR:-/tmp}/gordian-driver-test.XXXXXX")"
cleanup() {
  [ -z "${sleeper:-}" ] || kill "$sleeper" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

fails=0
passes=0
check() { # check DESCRIPTION CONDITION-COMMAND...
  local what="$1"
  shift
  if "$@"; then
    passes=$((passes + 1))
    echo "PASS  $what"
  else
    fails=$((fails + 1))
    echo "FAIL  $what"
  fi
}
equals() { [ "$1" = "$2" ]; }

# ---- a throwaway repository holding the real scripts ----

repo="$work/repo"
mkdir -p "$repo/scripts" "$work/bin"
cp "$repo_root/scripts/run-driver.sh" "$repo/scripts/run-driver.sh"
cp "$repo_root/scripts/check-no-oracle.sh" "$repo/scripts/check-no-oracle.sh"
# The runner is a stub: it records its arguments, runs the command, writes a usage report.
cat > "$repo/scripts/cgroup-run.sh" <<'EOF'
#!/usr/bin/env bash
# stub of scripts/cgroup-run.sh for the driver test
echo "$@" > "$STUB_ARGS_FILE"
report=""
while [ $# -gt 0 ]; do
  case "$1" in
    --report) report="$2"; shift 2 ;;
    --name|--cpus|--cpu-quota|--memory) shift 2 ;;
    --) shift; break ;;
    *) shift ;;
  esac
done
set +e
"$@"
status=$?
set -e
printf '{ "name": "stub", "isolation": "v1", "exit_code": %s, "wall_ns": 1, "cpu_ns": %s, "peak_memory_bytes": 1, "oom_kills": 0 }\n' \
  "$status" "${STUB_CPU_NS:-1000000000}" > "$report"
exit "$status"
EOF
chmod +x "$repo/scripts/"*.sh
printf 'target/\nartifacts/runs/\n' > "$repo/.gitignore"
echo hello > "$repo/tracked.txt"
(
  cd "$repo"
  git init -q
  git config user.email test@example.invalid
  git config user.name test
  git add -A
  git commit -q -m initial
)
head="$(git -C "$repo" rev-parse HEAD)"
stale="0000000000000000000000000000000000000000"

# ---- shims, first on PATH ----

cat > "$work/bin/pgrep" <<'EOF'
#!/usr/bin/env bash
# pgrep -x NAME succeeds with a fake pid when NAME is listed in FAKE_PGREP_NAMES
name="${!#}"
for n in ${FAKE_PGREP_NAMES:-}; do
  if [ "$n" = "$name" ]; then echo 4242; exit 0; fi
done
exit 1
EOF
cat > "$work/bin/df" <<'EOF'
#!/usr/bin/env bash
# df -P -B1 DIR reports FAKE_DF_AVAIL bytes available (default 100 GB)
echo "Filesystem 1-blocks Used Available Capacity Mounted on"
echo "fake 200000000000 1 ${FAKE_DF_AVAIL:-100000000000} 1% /"
EOF
cat > "$work/bin/taskset" <<'EOF'
#!/usr/bin/env bash
echo "$@" >> "$TASKSET_LOG"
[ -z "${FAKE_TASKSET_FAIL:-}" ] || { echo "taskset: refused" >&2; exit 1; }
exit 0
EOF
chmod +x "$work/bin/"*
export PATH="$work/bin:$PATH"
export TASKSET_LOG="$work/taskset.log"
export STUB_ARGS_FILE="$work/runner-args"

# ---- manifests and stub binaries ----

manifest() { # manifest FILE REVISION RUN_ID TOLERANCE-JSON TIMEOUT-SECS
  jq -n --arg rev "$2" --arg id "$3" --argjson tol "$4" --argjson to "$5" '{
    run_id: $id, source_revision: $rev,
    isolation: {cpus: "0-2", cpu_quota_percent: 300, memory_bytes: 2147483648, timeout_secs: $to},
    internal_external_ratio: $tol }' > "$1"
}

# A stand-in for gordian-run: writes measured.csv whose columns sum to 800,000,000 ns.
cat > "$work/fake-run" <<'EOF'
#!/usr/bin/env bash
out=""
while [ $# -gt 0 ]; do
  case "$1" in --out) out="$2"; shift 2 ;; *) shift ;; esac
done
mkdir -p "$out"
printf 'run_id,seed,class,measured_component_ns,measured_sched_ns,measured_harness_ns\nr,1,Ambiguous,100000000,50000000,250000000\nr,2,NoFault,200000000,0,200000000\n' > "$out/measured.csv"
printf 'run_id,seed,class\nr,1,Ambiguous\n' > "$out/results.csv"
exit "${FAKE_RUN_EXIT:-0}"
EOF
printf '#!/usr/bin/env bash\nsleep 30\n' > "$work/fake-sleep"
chmod +x "$work/fake-run" "$work/fake-sleep"

reset_logs() { : > "$TASKSET_LOG"; rm -f "$STUB_ARGS_FILE"; }
driver() { # driver ARGS... ; runs the copied driver, stderr to $work/err, returns its status
  bash "$repo/scripts/run-driver.sh" "$@" 2> "$work/err"
}
status_of() { driver "$@"; echo $?; }
err_has() { grep -q -- "$1" "$work/err"; }

# ---- usage errors ----

check "no arguments is a usage error (2)" equals "$(status_of)" 2
check "an unknown option is a usage error (2)" equals "$(status_of --nope)" 2
check "a missing manifest file is refused (3)" equals "$(status_of --manifest "$work/none.json")" 3
echo '{ not json' > "$work/bad.json"
check "an unparseable manifest is refused (3)" equals "$(status_of --manifest "$work/bad.json")" 3
manifest "$work/unsafe.json" "$head" "../escape" null 60
check "an unsafe run_id is refused (3)" equals "$(status_of --manifest "$work/unsafe.json")" 3

# ---- refusals that happen before pinning ----

reset_logs
manifest "$work/m.json" "$head" "t1" null 60
export FAKE_DF_AVAIL=1073741824
check "under 2 GB free is refused (10)" equals "$(status_of --manifest "$work/m.json" --bin "$work/fake-run")" 10
check "  and says why" err_has "bytes free"
unset FAKE_DF_AVAIL

export FAKE_PGREP_NAMES="cargo"
check "a running cargo is refused (11)" equals "$(status_of --manifest "$work/m.json" --bin "$work/fake-run")" 11
check "  and names it" err_has "cargo("
export FAKE_PGREP_NAMES="rustc"
check "a running rustc is refused (11)" equals "$(status_of --manifest "$work/m.json" --bin "$work/fake-run")" 11
unset FAKE_PGREP_NAMES

manifest "$work/stale.json" "$stale" "t2" null 60
check "a stale source_revision is refused (12)" equals "$(status_of --manifest "$work/stale.json" --bin "$work/fake-run")" 12
check "  and names both revisions" err_has "$head"

echo changed >> "$repo/tracked.txt"
check "a modified tracked file is refused (13)" equals "$(status_of --manifest "$work/m.json" --bin "$work/fake-run")" 13
git -C "$repo" checkout -q -- tracked.txt
echo new > "$repo/untracked.txt"
check "an untracked, non-ignored file is refused (13)" equals "$(status_of --manifest "$work/m.json" --bin "$work/fake-run")" 13
rm "$repo/untracked.txt"
mkdir -p "$repo/artifacts/runs/ignored"
echo x > "$repo/artifacts/runs/ignored/file"
check "git-ignored run outputs do not dirty the tree" equals "$(git -C "$repo" status --porcelain)" ""

mkdir -p "$work/done"
echo "x" > "$work/done/results.csv"
check "an existing results.csv is never overwritten (14)" equals \
  "$(status_of --manifest "$work/m.json" --out "$work/done" --bin "$work/fake-run")" 14
check "a missing binary is refused (21)" equals \
  "$(status_of --manifest "$work/m.json" --out "$work/o-missing" --bin "$work/no-such-binary")" 21

check "no refusal above launched the runner or pinned" test ! -e "$STUB_ARGS_FILE"
check "no refusal above reached taskset" test ! -s "$TASKSET_LOG"

# ---- pinning ----

reset_logs
export FAKE_TASKSET_FAIL=1
check "a refused taskset stops the run (20)" equals \
  "$(status_of --manifest "$work/m.json" --out "$work/o-pin" --bin "$work/fake-run")" 20
check "  before the runner starts" test ! -e "$STUB_ARGS_FILE"
unset FAKE_TASKSET_FAIL

# ---- a run, with the runner stubbed ----

manifest "$work/ok.json" "$head" "t-ok" '{"min": 0.5, "max": 1.0}' 60
export STUB_CPU_NS=1000000000
reset_logs
check "a run whose ratio is inside the tolerance exits 0" equals \
  "$(status_of --manifest "$work/ok.json" --out "$work/o-ok" --bin "$work/fake-run")" 0
check "  pinned to core 3" grep -q -- "-cp 3" "$TASKSET_LOG"
check "  passed the manifest's isolation to the runner" grep -q -- \
  "--name t-ok --cpus 0-2 --cpu-quota 300 --memory 2147483648 --report $work/o-ok/usage.json" "$STUB_ARGS_FILE"
check "  and the manifest and output directory to the binary" grep -q -- \
  "-- $work/fake-run --manifest $work/ok.json --out $work/o-ok" "$STUB_ARGS_FILE"
check "  ratio is measured / cpu = 0.8" equals "$(jq '.internal_external_ratio' "$work/o-ok/usage.json")" 0.8
check "  measured sum is recorded" equals "$(jq '.measured_ns_sum' "$work/o-ok/usage.json")" 800000000
check "  tolerance is recorded and satisfied" equals \
  "$(jq '[.ratio_tolerance.min, .ratio_tolerance.max, .ratio_within_tolerance] == [0.5, 1, true]' "$work/o-ok/usage.json")" true
check "  the runner's own fields survive" equals "$(jq -r '.isolation' "$work/o-ok/usage.json")" v1

manifest "$work/tight.json" "$head" "t-tight" '{"min": 0.9, "max": 1.0}' 60
check "a ratio outside the tolerance exits 4" equals \
  "$(status_of --manifest "$work/tight.json" --out "$work/o-tight" --bin "$work/fake-run")" 4
check "  and is recorded as outside" equals "$(jq '.ratio_within_tolerance' "$work/o-tight/usage.json")" false
check "  and says the instruments disagree" err_has "instruments disagree"

manifest "$work/none.json" "$head" "t-none" null 60
check "no declared tolerance: exit 0" equals \
  "$(status_of --manifest "$work/none.json" --out "$work/o-none" --bin "$work/fake-run")" 0
check "  the ratio is recorded anyway" equals "$(jq '.internal_external_ratio' "$work/o-none/usage.json")" 0.8
check "  with no tolerance verdict" equals "$(jq '.ratio_within_tolerance' "$work/o-none/usage.json")" null
check "  and the first value is called a starting point" err_has "starting point"

export FAKE_RUN_EXIT=7
check "a failing run passes its exit status through (7)" equals \
  "$(status_of --manifest "$work/ok.json" --out "$work/o-fail" --bin "$work/fake-run")" 7
check "  and the report is kept" test -f "$work/o-fail/usage.json"
unset FAKE_RUN_EXIT

# ---- the backstop ----

manifest "$work/slow.json" "$head" "t-slow" null 1
start=$SECONDS
code="$(status_of --manifest "$work/slow.json" --out "$work/o-slow" --bin "$work/fake-sleep")"
elapsed=$((SECONDS - start))
check "the timeout backstop stops a run that overstays (124)" equals "$code" 124
check "  within a few seconds, not 30" test "$elapsed" -lt 10
check "  and says so" err_has "backstop"
check "  leaving no sleeper behind" test -z "$(pgrep -f "$work/fake-sleep" 2>/dev/null || true)"

# ---- the extended oracle guard ----

mkdir -p "$repo/crates/gordian-run/src/policy"
printf 'fn ok() {}\n// EpisodeClass and EpisodeSpec are legal\n' > "$repo/crates/gordian-run/src/policy/fine.rs"
check "a clean policy file passes the guard" bash "$repo/scripts/check-no-oracle.sh"
for bad in 'use gordian_eval::Truth;' 'let e: Episode;' 'let s: Simulator;' 'fn f() { reveal(&x); }' 'use gordian_world::oracle::reveal;'; do
  printf '%s\n' "$bad" > "$repo/crates/gordian-run/src/policy/bad.rs"
  check "the guard rejects a policy containing: $bad" bash -c "! bash '$repo/scripts/check-no-oracle.sh' 2>/dev/null"
done
rm "$repo/crates/gordian-run/src/policy/bad.rs"
printf 'fn f() { gordian_world::oracle::reveal(&e); }\n' > "$repo/crates/gordian-run/src/policy/oracle.rs"
check "policy/oracle.rs alone is exempt" bash "$repo/scripts/check-no-oracle.sh"
printf 'use gordian_eval::Truth;\n' > "$repo/crates/gordian-run/src/lib.rs"
check "the evaluator may be named outside policy/" bash "$repo/scripts/check-no-oracle.sh"
printf 'fn f() { gordian_world::oracle::reveal(&e); }\n' > "$repo/crates/gordian-run/src/harness.rs"
check "but the oracle accessor may not be named outside the allowlist" bash -c "! bash '$repo/scripts/check-no-oracle.sh' 2>/dev/null"

echo
echo "driver.sh: $passes passed, $fails failed"
[ "$fails" -eq 0 ]
