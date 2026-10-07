Before every cargo command: `df -h /home/user` (9.3 GB free at the first build, 7.2 GB after the test build; the floor of 6 GB was never reached) and `pgrep -x gordian-run`.
**Waits and process record.** The first preflight (the first command of the unit) listed a `gordian-run` process (pid 11999); it was gone by the next check, a few minutes later, and no cargo command was issued
while it was listed (the first build started after a check that printed nothing). Every later check (before the release build, the run driver, the two clippy runs, the debug rebuild and the test build) found no
`gordian-run`; **no build or run waited**. The world-C runs were started by a script that polls for `cargo`, `rustc` and `gordian-run` every 30 s and records each wait: it waited zero times
(`runall.log`, scratch). Another process's `cargo test --workspace` was running when my release build finished; it had ended when the runs started. I never killed another lab's process and never passed
`--allow-unisolated`. The Python analyses (the floor takes 15 minutes) ran under the cgroup runner on cores 0-2 (the chief's gate builds may have run beside them; not recorded).

Gates, run under `scripts/cgroup-run.sh --name world-c-build --cpus 0-2 --memory 3G`, `CARGO_BUILD_JOBS=3 CARGO_PROFILE_DEV_DEBUG=0`, on the tree as committed (the Rust sources are unchanged since the
`cargo fmt` that followed the first commit; the outputs of the rebuilt example are byte-identical to the unformatted one's on the 20 streams of A2's range):
`cargo fmt --all -- --check` exit 0; `cargo clippy --workspace --all-targets -- -D warnings` exit 0 (and `cargo clippy -p gordian-stream --features reveal-hidden-state --all-targets -- -D warnings`,
which lints the `laws` example the workspace run skips for want of the feature, exit 0); `cargo test --workspace` exit 0, **1037 passed, 0 failed, 14 ignored** (269 s, peak 2.0 GB, no OOM kill);
`scripts/check-no-oracle.sh` prints `check-no-oracle: ok`. The Python tests (`w3_score_test.py`: 8; `w3_floor_test.py`: 11) pass; they are not part of the workspace gates.
No source under `crates/*/src` changed: nothing reaches an arm and the harness is the one W2 and E1 used.

**Reproducibility of the floor.** `w3_floor.py` (17.8 minutes under the runner, peak 0.25 GB, exit 0) was run in full twice for the ladder and the E1 forms, the second time after the memory gained its aggregating rule and its breakdowns:
`w3-floor-ladder.csv` (2304 rows) and `w3-floor-e1.csv` (96 rows) are `cmp`-identical across the two runs. The `streak` part was added after that run and ran alone (`--parts streak`).

