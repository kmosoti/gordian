# Gordian Research Knowledge Ontology

The knowledge graph is not a bibliography and not a decorative map. It is the repository's machine-readable record of what Gordian believes, why it believes it, what remains uncertain, which algorithms embody each idea, which theorems formalize narrow properties, and which experiments can falsify Gordian-specific design choices.

## Required node classes

### `Concept`

A scientific, engineering, or Gordian domain concept.

Examples: Mission, optimistic concurrency control, critical path, provenance, candidate freeze.

### `Source`

An externally retrievable evidence source: peer-reviewed paper, preprint, standard, official implementation documentation, or production-engineering account.

Sources should identify provenance and scope. Source existence does not imply endorsement of every conclusion in it.

### `Claim`

A proposition supported or qualified by evidence.

Claims must expose evidence-oriented relations or be flagged by the Rust audit.

### `Hypothesis`

A Gordian-specific proposition requiring experiment.

Every Hypothesis must point to an Experiment or executable verification plan.

### `Assumption`

A premise used by an algorithm, theorem, or experiment.

Assumptions are first-class because proving a theorem from a premise does not prove the premise describes reality.

### `Algorithm`

A computational procedure or scheduling/verification rule.

Algorithms should link to foundations, implementation artifacts, benchmarks, and theorem obligations where applicable.

### `Theorem`

A machine-checkable formal proposition.

A theorem node must contain a statement and a checker target. The linked engineering claim must not exceed the theorem's actual formal statement.

### `Experiment`

A falsification or benchmark protocol with an executable/analysis target.

Experiments should identify compared conditions, measurements, and a result that would force architectural revision.

### `Tool`

A concrete implementation or verification tool whose applicability is being evaluated or used: Jujutsu, Lean, Kani, Loom, petgraph, and similar.

### `Standard`

A normative external model/format such as W3C PROV, in-toto, or SLSA.

A node may be both `Source` and `Standard`.

### `ImplementationArtifact`

A repository artifact such as a Rust crate, formal module, benchmark harness, or protocol adapter.

### `Document`

Human-readable repository documentation.

## Core relation semantics

| Relation | Meaning |
| --- | --- |
| `contains` | conceptual containment |
| `decomposesInto` | hierarchical planning/decomposition |
| `dependsOn` | conceptual or implementation dependency |
| `requires` | required property/capability/input |
| `provides` | provided interface/capability/output |
| `supportedBy` | source contributes empirical/supporting evidence |
| `qualifiedBy` | source narrows or weakens a conclusion |
| `challengedBy` | source presents disconfirming evidence or a counterexample |
| `groundedIn` | mechanism adapts an established theory/standard |
| `motivatedBy` | source suggests a design direction without proving it |
| `formalizedBy` | concept/algorithm/claim has a related formal theorem |
| `formalizes` | theorem captures the named formal property |
| `testedBy` | hypothesis/algorithm is evaluated by an Experiment |
| `implementedBy` | concept/algorithm is realized by an implementation artifact/tool |
| `benchmarkedBy` | implementation/algorithm has a performance experiment |
| `verifiedBy` | implementation artifact has a verification method |
| `informedBy` | weaker provenance relation for relevant knowledge |
| `uses` | algorithm/activity consumes another concept/tool |
| `produces` | algorithm/activity produces an entity/evidence class |
| `contrastsWith` | explicit alternative model/technique |
| `refines` | more specific formulation of another concept |
| `projectsTo` | representation can be mapped into another standard/model |

## Completeness rule

A material concept belongs in the graph when any of the following happens:

- it appears in a normative Gordian specification;
- an algorithm relies on it;
- a theorem assumes or formalizes it;
- research evidence changes how Gordian is designed;
- an experiment manipulates or measures it;
- a production dependency is introduced to implement it;
- a credible alternative is considered and rejected;
- a failure mode reveals it as a hidden assumption.

Documentation should not introduce a major architectural noun and leave it absent from the graph.

## Epistemic rules

### Support is not entailment

`A supportedBy B` means B provides evidence relevant to A within stated scope. It does not mean B deductively proves A.

### Formalization is not empirical validation

`H formalizedBy T` means T proves a property of a formal model relevant to H. It does not establish the empirical usefulness of H unless the empirical claim itself is formal and its assumptions are demonstrated.

### Negative evidence is mandatory infrastructure

The graph must support `challengedBy` and `qualifiedBy`; the audit and review process should resist graphs that only accumulate confirming edges.

### Source revisions matter

Fast-moving papers and software documentation can change. Materially changed results should eventually be represented through distinct source-revision identities rather than overwriting historical provenance.

## Mechanical checks

`gordian-kg validate` checks structural graph integrity.

`gordian-kg audit` checks basic epistemic completeness:

- Sources have provenance locators;
- Claims have evidence-oriented relations;
- Hypotheses have experiment targets;
- Theorems have formal statements/checker targets;
- Experiments have executable/analysis targets.

These checks cannot determine whether a scientific inference is good. They ensure the graph makes the inference inspectable.

## Storage strategy

`knowledge/graph.jsonld` is the canonical version-controlled corpus for now.

The Rust tooling builds an indexed `petgraph` view for traversal and can export DOT. A future RDF/SPARQL projection may use Sophia/Oxigraph if queries justify that complexity. The storage backend is intentionally not allowed to dictate ontology semantics.

## Research ingestion workflow

When a source or experiment changes Gordian's understanding:

1. create/update the Source node;
2. create/update the relevant Concept/Claim/Hypothesis nodes;
3. add supporting, qualifying, or challenging relations;
4. add algorithms/theorem/experiment relations affected by the result;
5. update normative documentation only if the evidence justifies a semantic change;
6. update implementation artifacts separately;
7. run graph validation/audit;
8. preserve uncertainty rather than resolving it editorially.
