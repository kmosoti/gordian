# Backlog

- [ ] Make the pinned GitHub CLI installer architecture-aware — the verified asset currently targets Linux x86_64, which covers the CI baseline but should fail explicitly or select a separately verified asset elsewhere — `scripts/install-toolchains.sh`
- [ ] Clarify that workload histogram buckets have numeric meaning while canonical JSON orders their string keys lexicographically — the current format and implementation are deterministic, so this is wording-only — `docs/testing/workload-format.md`
