# Research Knowledge Graph

Gordian treats research provenance as executable project infrastructure.

A conventional bibliography answers **where a statement came from**. Gordian's research graph is intended to answer stronger questions:

- Which concepts does the architecture rely on?
- Which claims are supported, qualified, or challenged by which evidence?
- Which assumptions sit underneath a theorem or algorithm?
- Which design choices are deductions versus falsifiable Gordian hypotheses?
- Which theorem captures the narrow formal property behind a safety rule?
- Which algorithm operationalizes a concept?
- Which implementation artifact realizes the algorithm?
- Which experiment or benchmark can invalidate the design choice?

The canonical corpus is [`../knowledge/graph/`](../knowledge/graph/), governed by [`../knowledge/ontology.md`](../knowledge/ontology.md).

## Corpus structure

```text
knowledge/
  ontology.md
  graph/
    00-core.jsonld
    10-foundations.jsonld
    20-sources.jsonld
    30-studies-and-claims.jsonld
    40-algorithms.jsonld
    50-tools.jsonld
    60-formal.jsonld
    70-experiments.jsonld
    80-implementation.jsonld
```

Shard names are editorial only. `gordian-kg` loads every `.jsonld` shard in sorted order and builds one validated directed graph with globally unique node IDs.

This design allows the corpus to grow without creating one unreadable mega-file while retaining deterministic version-controlled state.

## What is represented

The graph currently reaches across several layers.

### Gordian domain

Mission Graph, Project, Mission, PlanRevision, Initiative, Atom, Quark, ExecutionAttempt, Candidate, integration candidate, evidence, artifact, attestation, semantic resources, capabilities, leases, accepted/deployment frontiers, projection, and reconciliation.

### Planning and scheduling science

HTN planning, partial-order causal-link planning, DAGs, topological ordering, critical path, work/span, RCPSP, list scheduling, HEFT/CPOP, and work stealing.

### Concurrency and distributed systems

Optimistic concurrency control, snapshot isolation, MVCC, conflict serializability, leases, fencing tokens, compare-and-swap, idempotency, deterministic replay, workflow nets, and Petri nets.

### Reproducibility and provenance

Hermeticity, content addressing, Merkle DAGs, W3C PROV, in-toto, SLSA, evidence fingerprints, and attestation.

### Agent-engineering evidence

CAID, STORM, AgentRoom, AgenticFlict, CodeTeam, and the current coding-agent reliability synthesis, with explicit bounded claims rather than benchmark values turned into universal laws.

### Formal/verification engineering

Lean, verification-guided development, differential randomized testing, property testing, Kani model checking, Loom/Shuttle concurrency exploration, Turmoil distributed simulation, coverage-guided fuzzing, mutation testing, and benchmark tooling.

## Node classes

The ontology includes:

```text
Concept
Source
Claim
Hypothesis
Assumption
Algorithm
Theorem
Experiment
Tool
Standard
ImplementationArtifact
Document
```

These classes are epistemically different. The graph should make it difficult to accidentally write:

```text
paper mentions X
therefore Gordian design X is proven
```

## Rust index

`gordian-kg` is Rust-native and currently indexes the merged corpus in `petgraph::StableDiGraph`.

The storage/index choice is provisional. The Foundation Initiative benchmarks it before Gordian depends on its performance characteristics.

### Structural validation

```bash
cargo run -p gordian-kg -- validate
```

Checks:

- every node has identity, name, and type;
- global IDs are unique across shards;
- relation predicates are non-empty;
- every relation target exists.

### Epistemic audit

```bash
cargo run -p gordian-kg -- audit
cargo run -p gordian-kg -- audit --strict
```

The initial audit flags structural epistemic holes such as:

- a Source with no provenance locator;
- a Claim with no evidence-oriented relation;
- a Hypothesis with no experiment target;
- a Theorem with no statement/checker target;
- an Experiment with no executable/analysis target.

This does not automate scientific judgment. It prevents missing scientific bookkeeping from becoming invisible.

### Corpus statistics

```bash
cargo run -p gordian-kg -- stats
```

Reports node/edge counts plus distributions by type and predicate. These statistics will later feed documentation coverage gates.

### Search and typed listing

```bash
cargo run -p gordian-kg -- search "snapshot"
cargo run -p gordian-kg -- list --kind Algorithm
cargo run -p gordian-kg -- hypotheses
cargo run -p gordian-kg -- theorems
```

### Neighborhood/evidence traversal

```bash
cargo run -p gordian-kg -- neighbors concept:atom
cargo run -p gordian-kg -- neighbors claim:exact-artifact-verification --predicate groundedIn
cargo run -p gordian-kg -- evidence claim:semantic-state-vs-code-state
```

### Directed paths

```bash
cargo run -p gordian-kg -- path concept:atom theorem:dispatch-requires-dependencies
```

The CLI uses breadth-first traversal to return a shortest directed path by edge count.

A graph path is navigation, **not automatic logical entailment**. `supportedBy`, `formalizedBy`, and `testedBy` have radically different semantics.

### DOT export

```bash
cargo run -p gordian-kg -- export-dot --out /tmp/gordian-knowledge.dot
```

This produces a Graphviz-compatible export for external visualization without making a diagram format part of canonical state.

## Why JSON-LD plus a Rust graph instead of a graph database?

At the current scale, a server/database would add operational state without increasing epistemic rigor.

The shard corpus provides:

- immutable repository snapshots;
- ordinary diff/review;
- deterministic merge/index construction;
- no required external service;
- linked-data-compatible identities;
- straightforward Rust processing;
- future export into RDF/PROV ecosystems.

If corpus scale or queries later justify SPARQL/RDF infrastructure, Sophia/Oxigraph can be evaluated against actual benchmark/query requirements. Gordian will not adopt a graph database merely because the noun “graph” appears in the architecture.

## JSON-LD scope

The corpus is a **Gordian JSON-LD application profile**, not a claim that the runtime is currently an RDF reasoner.

`@id`, `@type`, and `@context` provide linked-data identity. The explicit `relations` representation keeps canonical files readable and fast for Rust ingestion.

A future RDF exporter can map Gordian predicates to full IRIs without forcing core runtime semantics into an RDF-only representation.

## Completeness as a project invariant

A material concept is not fully acquired merely because it appears in a Markdown paragraph.

When research introduces a concept that affects Gordian, the expected closure is:

```text
Concept
  -> Source / Claim / Assumption
  -> Algorithm or design rule
  -> Theorem where formalizable
  -> Experiment where empirical
  -> ImplementationArtifact when built
  -> Documentation
```

Not every node needs every edge, but unexplained gaps should be visible.

## Epistemic invariant

> Traversability must never be confused with entailment.

A `supportedBy` edge records evidence, not proof.

A `formalizedBy` edge means a theorem captures a formal property, not that the real-world architecture has been proven superior.

A `testedBy` edge means the design is falsifiable, not that the experiment has passed.

That distinction is the difference between a research knowledge graph and a citation-shaped mood board.
