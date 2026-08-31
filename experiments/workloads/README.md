# Workload corpus

This directory contains deterministic workload inputs for graph, scheduler, evidence,
persistence, and agent experiments. It is a corpus, not a benchmark result.

## Layout

```text
experiments/workloads/
  README.md
  golden/
    manifest.json  mandatory closed corpus manifest, read-only checked by the verifier
    rep-small.json
    rep-wide.json
    adv-deep.json
    adv-contended.json
    repository-derived.json
  fixtures/     small fixtures and closed repository source manifests
```

The canonical `golden/manifest.json` is mandatory and has a closed top-level shape:
`schema_version` and `entries`, with `schema_version` equal to
`gordian.workload-golden-manifest.v1`. Every entry has exactly the common fields `path`, `sha256`,
`kind`, `seed`, and `distribution_id`. A `synthetic` entry additionally has exactly `node_count`
(an integer or null); a `repository-derived` entry instead has exactly `repository_path`,
`revision`, and `manifest_relative_path`. Entries are sorted by path, paths stay below `golden/`,
and the manifest names exactly one synthetic golden for each of the four distributions plus one
repository-derived golden. The manifest is read-only input to verification; the checker never
regenerates or updates it.

Each golden file is immutable evidence for the exact generator, format, and source identity recorded
inside it. The golden corpus deliberately contains one record per named distribution rather than
one file per seed. The in-memory acceptance matrix still covers every distribution at every seed
in `[1, 2, 3, 5, 8, 13, 21, 34]`. If generation changes, regenerate into a separate reviewable
candidate directory, compare identities and metrics, and replace the golden corpus only through an
explicit corpus change. Do not edit a golden JSON line by hand.

Fixtures are intentionally small and may be used for checker tests, negative cases, and local
development. They are not substitutes for the named distribution envelopes. Repository fixtures
carry a closed source manifest and are checked for path framing, exclusion rules, and rejection of
symlinks/special files.

The canonical format and all shape definitions are in
[`docs/testing/workload-format.md`](../../docs/testing/workload-format.md), and the schema consumed
by later Rust/Serde readers is
[`experiments/schema/workload.schema.json`](../schema/workload.schema.json).

## Seed matrix and acceptance

The acceptance matrix is the fixed list `[1, 2, 3, 5, 8, 13, 21, 34]`. Every named distribution
is generated in memory for every seed, and each record must contain its seed, complete generator
parameters, distribution id, canonical `workload_id`, and all flat exact shape fields. The checker
also verifies the five manifest golden entries, asserts that each shape matches exactly one
inclusive envelope, and enforces the canonical digest and repository-source rules.

Run the corpus acceptance checker from the repository root:

```bash
bash scripts/check-workloads.sh
```

The golden verifier is immutable; regeneration is a separate, explicitly reviewed action. A
passing corpus check is not a benchmark measurement and supplies no performance gate. Benchmark
execution and performance acceptance belong to #5, not #3.
