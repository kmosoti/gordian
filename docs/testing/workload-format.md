# Gordian Workload Format

This document is the normative contract for `gordian.workload.v1`, the interchange format
emitted by the workload generator. The JSON Schema at
[`experiments/schema/workload.schema.json`](../../experiments/schema/workload.schema.json) is the
machine-readable shape for this contract. A schema-compatible record is not, by itself, a valid
workload: the metric, envelope, digest, and source-manifest rules below are semantic validation
rules.

## Scope and versions

A workload is a deterministic input dataset for graph, scheduler, evidence, persistence, and agent
experiments. It is either synthetic or repository-derived and has exactly one named distribution
id. It is a dataset extractor, not Mission Graph semantic validation: it extracts graph-shaped
records and checks this format's structural and metric rules, but does not decide whether a Mission,
PlanRevision, Atom, acceptance predicate, or dependency contract is semantically valid.

The top-level `schema_version` is the compatibility identifier and is `gordian.workload.v1`. The top-level
fields are closed; unknown fields, duplicate JSON object keys, non-UTF-8 input, and a missing or
different format identifier are rejected. A format revision changes the identifier and requires an
explicit reader/writer compatibility decision. Within v1, additive optional metadata may be
introduced only when old readers can ignore it without changing the workload, and a required field,
changed meaning, changed canonicalization, or changed digest framing requires `v2` (or a later
explicit version). Producers must never silently reinterpret an older version. Rust/Serde readers
consume the schema and this contract; Python remains a generation and orchestration layer only.

## Canonical serialization and identity

Every workload is one canonical JSON document encoded as UTF-8 with no BOM and terminated by
exactly one LF (`\n`). Canonical JSON is compact (`,` and `:` separators), recursively sorted by
object key using Unicode code-point order, and contains no duplicate keys. Arrays have semantic
order: nodes are sorted by node id, edges by `(from,to)`, claims and resources by id, and history
records by their declared sequence. Integers are emitted as JSON integers; all bounded integers in
the schema are unsigned 64-bit values. Whitespace inside strings is data and is not normalized.

`workload_id` is `sha256:` followed by the lowercase hexadecimal SHA-256 digest of the canonical
bytes of the same payload after removing the `workload_id` member. The preimage still has the final
LF. A producer
must construct the payload, remove the identity field, canonicalize it, hash it, then insert the
result and canonicalize the complete document. An id copied from another payload or calculated
from a pretty-printed document is invalid. The `seed` is an explicit unsigned 64-bit integer; no
ambient clock, process id, absolute path, or global random state may affect generation.

The `generator` object records the generator name, generator version, and the complete parameter
object used for the run. Omitting a default is not allowed: a reader must be able to reproduce a
record from the stored parameters and seed without relying on the producer's current defaults.

## Top-level record

The v1 record contains these fields (the schema gives their exact types and required/optional
status):

| Field | Meaning |
| --- | --- |
| `schema_version` | `gordian.workload.v1`. |
| `workload_id` | SHA-256 identity defined above (serialized as `sha256:` plus 64 lowercase hexadecimal characters). |
| `distribution_id` | Exactly one of `rep-small`, `rep-wide`, `adv-deep`, `adv-contended`. |
| `seed` | Reproduction seed, `uint64`. |
| `generator` | Name, version, and complete generation parameters. |
| `origin` | `{"kind":"synthetic"}` or repository identity (`repository`, `revision`, and source digest). |
| `nodes` | Nodes of the acyclic workload graph. |
| `edges` | Directed prerequisite-to-dependent edges. |
| `workers` | Capability, cost, and duration distributions for worker experiments. |
| `events`, `evidence` | Event/evidence history volume and records used by replay/evidence experiments. |
| `resources` | Capacity-one resources referenced by nodes. |
| `dimensions` | Canonical measured shape metrics, including exact ratios. |

Nodes carry stable ids, positive `reference_duration_ticks`, semantic-claim ids, and resource
claim ids. Edges use `from` and `to` and point from prerequisite to dependent node. Node ids and edge endpoints must be
unique, all endpoints must exist, self-edges are rejected, and the graph must be a DAG. Claims,
resources, workers, events, and evidence records use stable ids and closed field sets defined by the
schema.

## Shape metrics

The generator computes and stores the following metrics rather than asking consumers to infer
which interpretation was intended.

* **Node count** is `n`, the number of graph vertices. Empty graphs are rejected.
* **Edge density** is `edge_count / [n(n-1)/2]`. It is zero for a one-node graph (the denominator
  is stored as `1` for that degenerate case); otherwise the denominator is the number of possible
  directed edges in a simple DAG.
* **Width** is the exact maximum-antichain width: the largest set of pairwise incomparable
  vertices under graph reachability. A topological layer or a greedy ready-queue size is not a
  width estimate.
* **Depth** is the maximum vertex count on a directed path. A singleton path has depth one; an
  edge from `a` to `b` has path length two. It is not the number of edges and not the number of
  topological layers.
* **Critical-path ratio** is the maximum, over directed paths, of the sum of positive
  `reference_duration_ticks` on that path divided by the total node ticks. Every node duration is
  positive, so the denominator is positive.
* **Semantic-claim overlap** is the fraction of incomparable unordered node pairs sharing at least
  one semantic claim id. The numerator counts each qualifying pair once, even if it shares several
  claims.
* **Resource contention** is the fraction of incomparable unordered node pairs for which at least
  one shared capacity-one resource has a combined unit demand greater than its capacity. The pair
  is counted once, not once per resource. A resource is shared only when both nodes declare a
  positive demand for that resource.

Each ratio stores three flat exact fields: an integer numerator, an integer denominator, and a
floored `ppm` value. The denominator is positive. `ppm` is exactly
`floor(1_000_000 * numerator / denominator)`; it is a display/index value, never the authority for
acceptance. Envelope checks use exact cross multiplication (`lower_num * denominator <=
numerator * lower_den` and `numerator * upper_den <= upper_num * denominator`) with checked or
unbounded arithmetic, not rounded ppm comparisons. The flat fields are `density_numerator`,
`density_denominator`, `density_ppm`, `semantic_overlap_numerator`,
`semantic_overlap_denominator`, `semantic_claim_overlap_ppm`, `resource_contention_numerator`,
`resource_contention_denominator`, and `resource_contention_ppm`. Critical-path numerator and
denominator are `critical_path_duration_ticks` and `total_duration_ticks`, with
`critical_path_ratio_ppm` as their floor. Width and depth are the exact integer fields
`dag_width` and `dag_depth`; `width`, `depth`, and ratio-object aliases are not v1 fields.

Counts, exact per-node `fan_in` and `fan_out`, their `fan_in_histogram` and `fan_out_histogram`,
and `max_fan_in`/`max_fan_out` are also stored. Fan-in is direct prerequisite in-degree; fan-out is
direct dependent out-degree. Histograms are sorted maps keyed by integer degree and include zero
degrees. `critical_path_nodes` preserves the vertex count of the maximizing path.

## Named distributions and envelopes

The shape must match exactly one inclusive envelope. A generator rejects a shape matching none and
rejects a shape matching more than one; callers may not relabel a shape after generation. The four
envelopes constrain node count, exact width, critical-path ratio, and semantic-claim overlap as
follows. Bounds are inclusive and shown as exact fractions.

| Distribution id | Nodes | Width | Critical-path ratio | Semantic-claim overlap |
| --- | ---: | ---: | ---: | ---: |
| `rep-small` | 50–200 | 4–12 | 1/4–2/5 | 1/20–3/20 |
| `rep-wide` | 500–2000 | 40–120 | 1/20–3/20 | 1/20–1/5 |
| `adv-deep` | 200–800 | 2–4 | 3/5–9/10 | 0/1–1/10 |
| `adv-contended` | 300–1000 | 20–60 | 3/20–3/10 | 11/20–17/20 |

The schema and checker validate all four dimensions plus the structural validity of every other
metric. A valid ratio at an envelope boundary remains valid even when its floored ppm value loses
precision.

## Synthetic and repository-derived workloads

Synthetic generation is pure with respect to the stored `seed` and `generator.parameters`. The
generator records all distribution parameters, graph construction parameters, duration/resource/
claim distributions, worker parameters, and history-volume parameters, including explicit defaults.
The fixed acceptance seed matrix for the corpus is `[1, 2, 3, 5, 8, 13, 21, 34]`.

Repository-derived generation accepts a closed source manifest. The manifest enumerates the source
graph `repository`, `nodes`, and `edges` using only the documented keys; unknown keys, duplicate
node/edge identities, dangling edges, cycles, and invalid node records are rejected. The resulting
record carries the repository revision, source schema version, and extractor/generator version.
The manifest is the graph input; it is not a file list and does not define which repository files
participate in the source digest.

The normalized source digest is SHA-256 over every included regular repository file using this
unambiguous byte framing:

```text
ASCII("gordian-source-digest-v1\0")
|| u64_be(file_count)
|| repeat(
     u64_be(relative_posix_path_utf8_byte_length) || relative_posix_path_utf8_bytes
     || u64_be(file_byte_length)                 || file_bytes
   )
```

Files are sorted by the UTF-8 bytes of their relative POSIX path. Paths contain `/`, never a
platform separator, and cannot contain an absolute prefix, `.` or `..` component. The framing makes
path/file boundaries unambiguous and includes bytes exactly; no text newline or Unicode
normalization is applied to file contents. Absolute checkout paths and mtimes are excluded from
the digest. `.git/`, `.jj/`, `.hg/`, `.svn/`, `target/`, virtualenv and tool caches, `artifacts/`,
generated/build/dist/output directories, and workload golden/generated output are excluded.
Symlinks, sockets, devices, fifos, and other special files are rejected rather than followed. A
changed revision, extractor version, manifest, or included byte changes the source identity;
historical records are not overwritten.

## Consumer contract

The workload is intentionally adapter-neutral. Later Rust/Serde consumers read
`experiments/schema/workload.schema.json`; Python may invoke the generator, stage files, and pass
records onward but must not reimplement these metric or Mission Graph decisions.

| Consumer | Fields it consumes | Meaning it must preserve |
| --- | --- | --- |
| Graph/reference algorithms | `nodes`, `edges`, `dimensions.node_count`, `dimensions.density_*`, `dimensions.dag_width`, `dimensions.dag_depth`, node fan-in/out | DAG topology and exact shape metrics; no semantic Mission validation is implied. |
| Scheduler | node `reference_duration_ticks`, `workers`, node `resource_claims`, `dimensions.critical_path_*`, `dimensions.resource_contention_*`, fan-in/out | timing/capability/resource inputs and the distinction between incomparable pairs and graph edges. |
| Evidence | node `semantic_claims`, `events`, `evidence`, `origin` revision/source identity | declared overlap and history volume are test inputs, not evidence of correctness or freshness. |
| Persistence/replay | `events`, `evidence`, graph ids, `workload_id`, generator/origin identity | stable event order and dataset identity; replay must not regenerate nondeterministic effects. |
| Agent experiments | complete record, `distribution_id`, `seed`, generator parameters, all dimensions | paired/repeated trials can name the exact workload and reproduce it without post-hoc relabeling. |

## Generation, checking, and benchmark boundary

The current workload module exposes the `synthetic`, `derive`, and `validate` subcommands. These
examples show that interface; they do not imply that a benchmark has run:

```bash
PYTHONPATH=orchestration/src python3 -m gordian_orchestration.workloads synthetic \
  --distribution rep-small --seed 1 --output experiments/workloads/golden/rep-small-seed-1.json
PYTHONPATH=orchestration/src python3 -m gordian_orchestration.workloads validate \
  experiments/workloads/golden/rep-small-seed-1.json
PYTHONPATH=orchestration/src python3 -m gordian_orchestration.workloads derive \
  --repository /path/to/repository --revision REVISION-ID --seed 1 \
  --output /tmp/workload.json
# Lower-level derivation from an already normalized source manifest is also supported:
PYTHONPATH=orchestration/src python3 -m gordian_orchestration.workloads derive \
  --source experiments/workloads/fixtures/source-manifest.json \
  --distribution rep-small --seed 1 --output /tmp/workload.json
```

The golden verifier is immutable evidence: changing the generator requires a separately regenerated
corpus and an explicit review of changed identities. Run `scripts/check-workloads.sh` to validate
the schema, canonical identity, shape metrics, all four envelopes, source-manifest rules, and the
seed matrix. #3 owns generation and corpus checking only. Benchmark measurements, performance
claims, and benchmark gates belong to #5; #3 owns no `EO17-*` row.
