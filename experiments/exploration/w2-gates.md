Gates, run under `scripts/cgroup-run.sh --name world-laws-build --cpus 0-2 --memory 3G` (never `--allow-unisolated`),
`CARGO_BUILD_JOBS=3 CARGO_PROFILE_DEV_DEBUG=0`, on the tree as committed (the Rust sources are unchanged since commit 85a9719; every later commit touches scripts, CSVs and this report):
`cargo fmt --all -- --check` exit 0; `cargo clippy --workspace --all-targets -- -D warnings` exit 0;
`cargo test --workspace` exit 0, 954 passed, 0 failed, 13 ignored (262 s wall, peak 1.78 GB, no OOM kill; the new
`tests::rebuild` tests are among them); `scripts/check-no-oracle.sh` prints `check-no-oracle: ok`. Disk after the
test build: 6.4 GB free (12 GB before the first build).
