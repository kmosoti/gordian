# Research Knowledge Graph

Gordian treats research provenance as executable project infrastructure.

A conventional bibliography answers **where a statement came from**. Gordian's research graph is intended to answer stronger questions:

- Which claims are supported by which studies or standards?
- Which design choices are deductions versus experiments still waiting to happen?
- Which theorem formalizes a normative rule?
- Which assumptions does that theorem require?
- Which algorithm implements a concept?
- Which document specifies the algorithm?
- Which empirical result would falsify a Gordian hypothesis?

The canonical graph is [`../knowledge/graph.jsonld`](../knowledge/graph.jsonld).

## Node classes

### Concept

A domain object or architectural abstraction, for example:

- Mission Graph
- Atom
- Quark
- Evidence
- Accepted Frontier
- semantic read/write claims

Concept nodes should not carry an implication of truth merely by existing.

### Source

An external research paper, standard, production engineering design, or other evidentiary basis.

Examples include:

- HTN planning literature
- W3C PROV
- Bazel dependency semantics
- in-toto
- SLSA provenance
- Jujutsu documentation
- CAID / STORM / AgentRoom empirical studies

A Source's presence means it is relevant, not that every Gordian inference drawn from it is established by the source.

### Claim

A proposition whose support can be discussed independently of Gordian's implementation.

Example:

> Isolated code state should be paired with explicit coordination for concurrent software agents.

A Claim can have `supportedBy`, `challengedBy`, and `groundedIn` relations.

### Hypothesis

A Gordian-specific proposition requiring experiment.

Examples:

- Atom is the appropriate global scheduling boundary.
- semantic claims predict harmful concurrency better than path ownership.
- snapshot-isolated agents outperform continuously rebased agents.

A hypothesis should have an explicit test target before it is allowed to quietly harden into doctrine.

### Algorithm

A specified computational procedure or decision rule, such as:

- dependency-aware admission
- evidence freshness
- candidate admission
- reconciliation

Algorithms may be partly formalized, property-tested, benchmarked, or merely specified. Those are separate states.

### Theorem

A proposition represented in the Lean kernel.

The theorem node records:

- human-readable statement;
- Lean source target;
- verification method;
- current verification state;
- important scope notes.

The graph must not label an empirical statement a theorem merely because a formal surrogate is easy to prove.

### Document

A human-readable specification, research note, or protocol in this repository.

## Relation vocabulary

The initial vocabulary intentionally stays small.

| Relation | Meaning |
| --- | --- |
| `contains` | conceptual containment |
| `decomposesInto` | decomposition relation |
| `documentedBy` | explanatory document |
| `specifiedBy` | normative specification |
| `supportedBy` | external evidence supports a claim within stated scope |
| `challengedBy` | source presents material counterevidence or limitation |
| `qualifiedBy` | source weakens or scopes an otherwise useful conclusion |
| `groundedIn` | concept/algorithm adapts an established mechanism or standard |
| `motivatedBy` | source suggests the design direction without proving it |
| `formalizedBy` | points from concept/claim/algorithm to a theorem |
| `formalizes` | inverse conceptual relationship |
| `implementedBy` | algorithm or concept has executable implementation |
| `testedBy` | points to a hypothesis/experiment or executable test |
| `uses` | algorithm/activity consumes another concept |
| `produces` | activity produces an entity/evidence class |

## Rust tooling

`gordian-kg` deliberately implements only graph mechanics. It does not decide epistemic truth itself.

### Validate

```bash
cargo run -p gordian-kg -- validate
```

Checks:

- every node has a non-empty ID;
- IDs are unique;
- every node has a name;
- relation predicates are non-empty;
- every relation target exists.

This catches structural rot but not bad scientific reasoning.

### Search

```bash
cargo run -p gordian-kg -- search "snapshot"
```

Searches IDs, names, summaries, and claim/theorem statements.

### Neighborhood traversal

```bash
cargo run -p gordian-kg -- neighbors claim:isolation-plus-coordination
```

Shows both incoming and outgoing relationships. A predicate can be selected:

```bash
cargo run -p gordian-kg -- neighbors claim:isolation-plus-coordination --predicate supportedBy
```

### Evidence view

```bash
cargo run -p gordian-kg -- evidence claim:exact-artifact-verification
```

Restricts traversal to evidence-oriented relations such as `supportedBy`, `challengedBy`, `groundedIn`, `formalizedBy`, and `testedBy`.

### Path query

```bash
cargo run -p gordian-kg -- path concept:atom theorem:dispatch-requires-dependencies
```

Uses breadth-first search over directed relations and returns a shortest path by edge count.

This is primarily a navigation feature. A path in the graph is **not** automatically a valid logical proof chain because relation predicates have different semantics.

## Why not use a graph database yet?

The current graph is small enough that introducing Neo4j, RDF triplestores, or a distributed graph service would increase operational complexity without increasing epistemic quality.

The JSON-LD file gives us:

- versioned research state in the repository;
- ordinary code review;
- deterministic snapshots;
- Rust traversal;
- a future interoperability path to RDF/PROV tooling;
- no infrastructure requirement for early experiments.

A graph database becomes justified only when query volume, graph size, multi-user mutation, or reasoning requirements make the file representation measurably inadequate.

## JSON-LD scope

`graph.jsonld` is a **Gordian JSON-LD application profile**, not a claim that Gordian currently implements a complete RDF reasoner.

The `@context`, `@id`, and `@type` structures provide linked-data-compatible identities while `relations` remains deliberately easy for the Rust tool to consume.

If full RDF interoperability becomes necessary, an exporter should translate the application profile into explicit RDF predicates rather than forcing the runtime model to become RDF-native.

## Epistemic invariant

The most important rule for the knowledge graph is:

> Traversability must never be confused with entailment.

`A supportedBy B` records an evidentiary relationship. It does not mean B deductively proves A.

`A formalizedBy T` means T captures a formal property relevant to A. It does not mean T proves A's real-world usefulness.

This separation is essential to keeping the knowledge graph scientifically honest.
