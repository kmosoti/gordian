# Gordian Knowledge Acquisition Protocol

Gordian treats research knowledge as executable engineering input, not a pile of links. A source is not considered acquired merely because it appears in a bibliography. Acquisition is complete only when the relevant claims, scope, assumptions, limitations, alternatives, formal obligations, experiments, algorithms, implementation decisions, and repository artifacts can be traversed as one provenance-preserving graph.

The canonical corpus is the sorted set of JSON-LD shards under [`knowledge/graph/`](graph/). Human-readable research documents explain the synthesis. The machine-readable graph preserves the inspectable structure that the prose necessarily compresses.

## 1. Acquisition objective

For every material proposition used by Gordian, the repository should be able to answer:

1. What exactly is being claimed?
2. Who or what produced the claim?
3. What exact source revision, section, table, figure, experiment, commit, or documentation page supports it?
4. What population, task, model, repository, environment, and time period does the evidence cover?
5. What assumptions are necessary for the inference?
6. What limitations or threats to validity remain?
7. What evidence qualifies, challenges, or contradicts the claim?
8. Which Gordian algorithm, theorem, experiment, decision, document, issue, or implementation artifact depends on it?
9. What observation would falsify or force revision of the Gordian-specific conclusion?
10. Is the evidence current enough for the decision that depends on it?

A graph that cannot answer these questions is an index, not a knowledge substrate.

## 2. Epistemic layers

Gordian keeps different kinds of statements distinct.

| Layer | Meaning | Validating method |
| --- | --- | --- |
| Definition | stipulated vocabulary | internal consistency and use |
| Formal theorem | proposition derived in a formal model | Lean kernel plus independent checker and axiom audit |
| Established foundation | mature result adapted from an external field | primary literature, standards, textbooks, production evidence |
| Empirical observation | measured result in a stated experiment | reproducible data and analysis |
| Evidence-supported conclusion | bounded inference from observations | source quality, scope, alternatives, confounders |
| Engineering deduction | design consequence derived from premises | inspectable argument and counterexample search |
| Hypothesis | Gordian-specific proposition not yet established | falsification experiment |
| Assumption | premise accepted for a model or procedure | explicit scope and sensitivity analysis |
| Decision | selected engineering tradeoff | requirements, evidence, benchmark, alternatives |
| Unresolved uncertainty | material question without adequate evidence | acquisition or experiment backlog |

A node may participate in several layers, but each role must be explicit. A formalized hypothesis does not silently become an empirical fact. A passing benchmark does not silently become a theorem. A popular design pattern does not silently become evidence.

## 3. Required node classes

The ontology in [`ontology.md`](ontology.md) remains normative. The comprehensive target includes at least the following classes.

### Scientific and conceptual

- `Concept`
- `Definition`
- `Assumption`
- `Claim`
- `Hypothesis`
- `Alternative`
- `Counterexample`
- `Limitation`
- `ThreatToValidity`
- `Observation`
- `Result`

### Source and provenance

- `Source`
- `SourceRevision`
- `Study`
- `Standard`
- `Dataset`
- `RepositoryRevision`
- `DocumentationRevision`
- `CitationLocator`

### Engineering

- `Algorithm`
- `ComplexityBound`
- `Invariant`
- `Protocol`
- `Decision`
- `FailureMode`
- `Metric`
- `Benchmark`
- `Tool`
- `ImplementationArtifact`

### Formal methods

- `Theorem`
- `Lemma`
- `FormalModel`
- `ProofArtifact`
- `Axiom`
- `RefinementObligation`
- `ConformanceResult`

### Experimentation

- `Experiment`
- `ExperimentRun`
- `Workload`
- `Treatment`
- `Baseline`
- `Measurement`
- `StatisticalAnalysis`
- `Reproduction`

### Gordian delivery

- `Project`
- `Mission`
- `PlanRevision`
- `Initiative`
- `Atom`
- `Issue`
- `Document`
- `ReleaseArtifact`

The first Rust schema does not yet encode every field below as a statically typed member. That is an implementation gap recorded in the Atom backlog, not permission to omit the information indefinitely.

## 4. Source identity and revision policy

A URL alone is not stable provenance.

Every material source should identify, when available:

```text
source_id
source_kind
title
authors_or_maintainers
publisher_or_repository
publication_date
revision_date
retrieved_at
canonical_url
immutable_locator
doi_or_arxiv_id
repository_commit_or_tag
document_version
content_digest
review_status
license
```

The `immutable_locator` should be the narrowest durable locator available:

- DOI plus section, page, table, or figure;
- arXiv identifier plus revision and section;
- repository commit plus file and line range;
- documentation release plus heading;
- standard edition plus clause;
- dataset release plus record/query description;
- experiment artifact digest plus run manifest.

When a paper, documentation page, or repository materially changes, create a distinct `SourceRevision` rather than rewriting history. New conclusions may supersede old ones, but the graph preserves which decision depended on which revision at the time.

## 5. Claim record

A material claim should contain or link to:

```text
statement
claim_kind
epistemic_status
scope
assumptions
supporting_evidence
qualifying_evidence
challenging_evidence
alternatives
limitations
temporal_validity
reviewed_at
owner
```

### Scope

Scope should be structured enough to prevent accidental generalization:

```text
task_or_problem
population_or_repository_set
model_or_actor_class
software_and_toolchain
environment
workload
measurement_window
sample_size
inclusion_and_exclusion_rules
```

For agent-engineering research, a result on four backend tasks is not represented as a universal statement about multi-agent development. For benchmark results, a latency result on one graph shape is not represented as an asymptotic bound.

### Effect and uncertainty

When a source reports quantitative results, preserve:

```text
metric
baseline
treatment
absolute_effect
relative_effect
uncertainty_interval
variance_or_dispersion
sample_count
statistical_test
multiple_comparison_policy
```

Do not manufacture uncertainty values when the source does not report them. Record the omission as a limitation.

## 6. Relations

Core relations are defined in [`ontology.md`](ontology.md). The acquisition process should additionally preserve distinctions such as:

| Relation | Meaning |
| --- | --- |
| `defines` | gives a meaning to a term |
| `refines` | narrows or strengthens another representation |
| `supportedBy` | contributes supporting evidence within scope |
| `qualifiedBy` | narrows the supported conclusion |
| `challengedBy` | presents disconfirming evidence or counterexample |
| `contradicts` | cannot jointly hold under the stated shared scope |
| `assumes` | relies on a premise |
| `alternativeTo` | credible competing explanation or design |
| `observedIn` | result arose in an experiment or study |
| `testedBy` | hypothesis is evaluated by an experiment |
| `falsifiedBy` | evidence defeats the stated hypothesis in scope |
| `formalizedBy` | model property has a formal proposition |
| `proves` | proof artifact kernel-checks a theorem |
| `implements` | artifact realizes an algorithm or protocol |
| `conformsTo` | implementation passed a model/conformance relation |
| `benchmarkedBy` | performance claim is measured by a benchmark |
| `dependsOn` | downstream knowledge or implementation dependency |
| `motivates` | suggests a direction without establishing it |
| `supersedes` | newer revision replaces an earlier active representation |
| `cites` | source/document citation without stronger semantics |
| `partOf` | decomposition or editorial containment |
| `produces` | activity creates an entity or result |
| `consumes` | activity uses an entity or input |
| `governs` | policy constrains a process or artifact |
| `projectsTo` | representation maps into another standard/model |

`contradicts` is not a decorative red edge. It requires an explicit shared scope or a note explaining the incompatible propositions. Mere disagreement in wording is not contradiction.

## 7. Contradiction and negative-evidence policy

Gordian must preserve evidence that weakens its preferred design.

When sources disagree:

1. preserve each source revision;
2. create separate claim nodes when propositions differ;
3. identify overlapping and non-overlapping scope;
4. connect qualifying or challenging evidence;
5. record plausible confounders and methodological differences;
6. avoid forced reconciliation when the evidence does not justify one;
7. mark downstream decisions as exposed to the unresolved conflict;
8. create an experiment only when Gordian can materially reduce the uncertainty.

A negative result should never be overwritten by a later positive result. Both remain addressable, and any supersession relation states why the active conclusion changed.

## 8. Formal theorem acquisition

A theorem node must identify:

```text
exact_proposition
formal_model
assumptions
Lean_module
Lean_declaration
Lean_toolchain
proof_source_digest
kernel_check_status
independent_checker_status
axiom_audit_status
engineering_claims_it_supports
claims_it_does_not_support
```

The proof target is not “prove Gordian correct.” The target is to prove every formalizable substrate invariant against an explicit model, without `sorry` or unreviewed axioms, and to maintain a visible bridge from model theorem to Rust conformance evidence.

Every theorem should expose the proof boundary. For example, a theorem that fresh evidence is required proves a relationship over modeled fingerprints. It does not prove that the production fingerprint includes every real dependency unless the implementation and dependency-observation assumptions are separately justified.

## 9. Experiment acquisition

An experiment node must identify:

```text
hypothesis
falsification_condition
independent_variables
dependent_variables
controls
baseline_algorithms
workload_generation
sampling_strategy
random_seeds
hardware_and_os
toolchain_versions
repository_commit
protocol
metrics
analysis_plan
stopping_rule
artifact_locations
```

An `ExperimentRun` additionally records exact inputs, outputs, timestamps, environment digest, failures, exclusions, and analysis result.

A benchmark is incomplete without a simple reference baseline when one is practical. An optimized algorithm is incomplete without a correctness oracle, differential test, or proof obligation. Wall-clock measurements must be supplemented by deterministic or lower-noise measures when scheduler noise can conceal regression.

## 10. Acquisition lifecycle

### Discover

Search primary literature, standards, official documentation, repositories, benchmark corpora, and credible disconfirming sources. Secondary sources may help navigation, but material technical claims should trace to primary evidence.

### Triage

Classify relevance, source quality, temporal stability, scope, and decision impact. Reject low-value acquisition that adds names without changing understanding.

### Extract

Capture exact propositions, methods, quantitative results, assumptions, limitations, and source locators. Preserve terminology before translating it into Gordian vocabulary.

### Normalize

Create or reuse stable concept identities. Separate synonymous labels from genuinely distinct concepts. Split compound claims that have different evidence or falsification conditions.

### Connect

Link claims to evidence, algorithms, theorems, experiments, decisions, documents, code, and issues. Add qualifying and challenging paths, not only supportive paths.

### Verify

Run structural validation, epistemic audit, source-locator checks, theorem checks, experiment-manifest checks, and repository coverage queries.

### Synthesize

Update prose documentation only after graph relationships make the inference inspectable. The synthesis must distinguish evidence from deduction and hypothesis.

### Operationalize

Create or update Atoms for proofs, experiments, algorithms, implementation, or migration. An architectural decision without an implementation or falsification path remains provisional.

### Refresh

Revisit temporally unstable sources, evolving preprints, tool documentation, standards drafts, and implementation revisions. Propagate stale or retracted status to downstream claims and decisions.

## 11. Mechanical integrity targets

The Rust tooling should eventually enforce the following.

### Structural

- globally unique stable node identities;
- ontology-defined node types and predicates;
- no dangling relations;
- valid source-revision and supersession chains;
- acyclic relations where the relation semantics require a DAG;
- canonical deterministic serialization and digesting.

### Epistemic

- every material Claim has support, qualification, challenge, formalization, or explicit unresolved status;
- every Hypothesis has a falsification condition and executable Experiment target;
- every Theorem has an exact declaration and checker evidence;
- every Experiment has protocol, metrics, exact revision/toolchains, and artifacts;
- every Decision has requirements, evidence, alternatives, and benchmark/proof obligations as applicable;
- every Source has a retrievable locator and immutable revision/digest when feasible;
- contradictory evidence remains traversable;
- stale or retracted sources expose downstream impact.

### Repository coverage

- every normative architectural concept exists in the graph;
- every formal declaration maps to a Theorem or ProofArtifact node;
- every material algorithm maps to documentation, implementation, and benchmark obligations;
- every active hypothesis maps to an Atom or completed experiment;
- every implementation Atom maps back to a Mission/Initiative and relevant knowledge nodes;
- every public protocol field maps to a definition and invariant.

Mechanical coverage cannot judge whether the inference is sound. It prevents missing provenance and invisible assumptions from hiding behind fluent prose.

## 12. Required traversal queries

The Rust query layer should make these questions routine:

```text
why does decision X exist?
which claims depend on source revision Y?
which assumptions remain untested?
which theorems lack Rust conformance evidence?
which experiments challenge an active decision?
which concepts appear in docs but not in the graph?
which graph concepts lack docs, proof, experiment, issue, or implementation paths?
what becomes suspect if a source is stale or retracted?
what is the evidence path from a Mission invariant to a source, theorem, and test?
what is the shortest dependency path from a hypothesis to production code?
which alternatives were considered and why were they rejected?
```

Traversal results must preserve edge predicates and direction. A shortest path without epistemic edge labels is often misleading.

## 13. Rust and Python responsibility

Rust owns:

- typed canonical graph schema;
- deterministic parse, merge, canonicalize, and digest;
- ontology validation;
- traversal and indexes;
- contradiction, impact, proof, and coverage queries;
- graph audit and policy gates;
- JSON-LD import/export;
- scalable storage/index implementation selected through benchmark.

Python may:

- orchestrate source retrieval and metadata normalization;
- invoke Rust ingestion and validation commands;
- run experiment matrices;
- aggregate raw measurements without redefining their semantics;
- synchronize temporary GitHub planning projections.

Python must not become an alternate knowledge model or query engine.

## 14. Completion criterion

Knowledge acquisition for a material concept is complete only when:

1. its definition and identity are stable enough for current use;
2. source revisions and exact locators are recorded;
3. claims are scoped and connected to support, qualification, and challenge;
4. assumptions, limitations, and alternatives are visible;
5. formal and empirical obligations are separated;
6. affected algorithms, experiments, decisions, documents, issues, and code are linked;
7. the graph passes structural and epistemic audits;
8. unresolved uncertainty remains explicit.

This criterion is intentionally expensive. Gordian should pay that cost for architecture-shaping knowledge, not for every incidental fact encountered during development.
