# Gordian Research Knowledge Ontology

The knowledge graph is not a bibliography and not a decorative map. It is the repository's machine-readable record of what Gordian believes, why it believes it, what remains uncertain, which algorithms embody each idea, which theorems formalize narrow properties, and which experiments can falsify Gordian-specific design choices.

The canonical corpus is the sorted collection of JSON-LD shards under [`knowledge/graph/`](graph/). `gordian-kg` deterministically merges those shards into one indexed directed graph at runtime.

This document is normative for the corpus. Every `@type`, every predicate, every `status` string, and every `verification` field used in a shard must appear below. A vocabulary that the tooling accepts but no document enumerates is not a vocabulary; it is free text with a schema-shaped name.

## Required node classes

### `Concept`

A scientific, engineering, or Gordian domain concept.

Examples: Mission, optimistic concurrency control, critical path, provenance, candidate freeze.

A Concept that adapts an external field additionally carries `ScientificFoundation`, `EngineeringFoundation`, or `StandardFoundation` (see **Foundation markers**).

### `Source`

An externally retrievable evidence source: peer-reviewed paper, preprint, standard, official implementation documentation, dataset, or production-engineering account.

Sources identify provenance and scope. Source existence does not imply endorsement of every conclusion in it. Every Source carries the provenance fields required by **Source provenance fields**.

### `SourceRevision`

One dated, immutably located version of a `Source`. A `SourceRevision` exists whenever a source's material results changed and a Gordian decision depended on either version.

Fields: `source_id`, `version_label`, `revision_date`, `retrieved_at`, `immutable_locator`, `content_digest`, plus a `supersedes` relation to the revision it replaces and a `partOf` relation to its `Source`. `sourcerev:caid-v1` and `sourcerev:caid-v2` are the worked example.

### `Claim`

A proposition supported or qualified by evidence.

Claims must expose evidence-oriented relations or be flagged by the Rust audit.

### `Hypothesis`

A Gordian-specific proposition requiring experiment.

Every Hypothesis must point to an Experiment, and its `statement` must be falsifiable in the form required by **Hypothesis statements**.

### `Assumption`

A premise used by an algorithm, theorem, or experiment.

Assumptions are first-class because proving a theorem from a premise does not prove the premise describes reality. Every Assumption carries a `statement` naming the premise as a proposition.

### `Algorithm`

A computational procedure or scheduling/verification rule.

Algorithms should link to foundations, implementation artifacts, benchmarks, and theorem obligations where applicable.

### `Theorem`

A machine-checkable formal proposition.

A theorem node must contain a `statement`, a checker `target`, a `content` value, and a `class` value (see **Theorem weight**). The linked engineering claim must not exceed the theorem's actual formal statement.

### `Experiment`

A falsification or benchmark protocol with an executable/analysis target.

Experiments identify compared conditions, measurements, and a result that would force architectural revision. Their `verification[].target` is governed by **Experiment execution**.

### `Result`

The analysis output of one experiment's completed runs. Emitted by ingestion, not written by hand (see **Experiment execution**).

### `Decision`

A selected engineering tradeoff, with the Result and Hypothesis nodes that produced it. `#68`'s retain/revise/reject report is a projection of these nodes rather than a parallel prose document.

### `Tool`

A concrete implementation or verification tool whose applicability is being evaluated or used: Jujutsu, Lean, Kani, Loom, petgraph, and similar.

### `Standard`

A normative external model/format such as W3C PROV, in-toto, or SLSA.

A node may be both `Source` and `Standard`.

### `ImplementationArtifact`

A repository artifact such as a Rust crate, formal module, benchmark harness, or protocol adapter.

### `Document`

Human-readable repository documentation. A Document may additionally carry `Specification` or `Protocol`.

### `Initiative`, `Atom`, `Issue`, `PlanRevision`

Backlog identities. These are the classes the generated backlog shard uses, described under **Backlog mirror**; they exist so that `#72`'s coverage scan and `#48`'s provenance import have a target schema rather than an intention.

### Foundation markers

`ScientificFoundation`, `EngineeringFoundation` and `StandardFoundation` are secondary types marking a node as adapted from an external field, an external engineering practice, or an external standard. They are exactly the nodes audit rule **S1** applies to: a `*Foundation` node that is not itself a `Source` must carry at least one `supportedBy`, `qualifiedBy`, `challengedBy` or `groundedIn` edge whose target is a `Source`.

### Study markers

`EmpiricalStudy`, `DatasetStudy`, `EngineeringStudy`, `Review` and `ImplementationEvidence` are secondary types on `Source` nodes recording what kind of evidence the source is. They carry no audit obligation of their own; `source_kind` is the field the audit reads.

## Node identity

Node ids are globally unique and prefixed by primary class:

| Prefix | Primary `@type` |
| --- | --- |
| `concept:` | `Concept` |
| `foundation:` | `Concept` with a `*Foundation` marker adapted from an external field |
| `source:` | `Source` |
| `sourcerev:` | `SourceRevision` |
| `claim:` | `Claim` |
| `hypothesis:` | `Hypothesis` |
| `assumption:` | `Assumption` |
| `algorithm:` | `Algorithm` |
| `theorem:` | `Theorem` |
| `experiment:` | `Experiment` |
| `result:` | `Result` |
| `decision:` | `Decision` |
| `tool:` | `Tool` |
| `artifact:` | `ImplementationArtifact`, `Document` |
| `atom:` | `Atom`, `Issue` |
| `initiative:` | `Initiative` |

A `Hypothesis` never carries a `claim:` id and a `Claim` never carries a `hypothesis:` id. The seven hypotheses that formerly used `claim:` ids were renamed, and the `tests` edges that referenced them were repointed in the same change.

Shard placement is free. A node may live in any shard; ids remain globally unique and relations may cross any shard. There is deliberately no rule mapping id prefix to filename, because such a rule would make a correct graph fail for an editorial reason.

## Core relation semantics

Every predicate used in the corpus appears in this table. The **Target** column states the allowed `@type` of the relation target; `any` means the predicate carries no type restriction.

| Relation | Target | Meaning |
| --- | --- | --- |
| `contains` | any | conceptual containment |
| `partOf` | any | inverse of `contains`; also binds a `SourceRevision` to its `Source` |
| `decomposesInto` | any | hierarchical planning/decomposition |
| `dependsOn` | any | conceptual or implementation dependency |
| `requires` | any | required property/capability/input |
| `requiredBy` | any | inverse of `requires` |
| `provides` | any | provided interface/capability/output |
| `supportedBy` | `Source` | source contributes empirical/supporting evidence |
| `qualifiedBy` | `Source` | source narrows or weakens a conclusion |
| `challengedBy` | `Source` | source presents disconfirming evidence or a counterexample |
| `motivatedBy` | `Source` | source suggests a design direction without proving it |
| `motivates` | any | inverse of `motivatedBy` |
| `groundedIn` | any | mechanism adapts an established theory/standard/foundation |
| `informedBy` | any | weaker provenance relation for relevant knowledge; deliberately **not** restricted to `Source` targets, so an artifact informed by another artifact or by a claim is expressible without overstating the epistemic force |
| `formalizedBy` | `Theorem` | concept/algorithm/claim has a related formal theorem |
| `formalizes` | any | inverse of `formalizedBy`; theorem captures the named formal property |
| `testedBy` | `Experiment` | hypothesis/algorithm/concept is evaluated by an Experiment |
| `tests` | any | inverse of `testedBy` |
| `implementedBy` | `ImplementationArtifact`, `Tool`, `Algorithm` | concept/algorithm is realized by an implementation artifact/tool |
| `implements` | any | inverse of `implementedBy` |
| `benchmarkedBy` | `Experiment` | implementation/algorithm has a performance experiment |
| `verifiedBy` | `Tool`, `ImplementationArtifact` | implementation artifact has a verification method that actually runs |
| `uses` | any | algorithm/activity consumes another concept/tool/source |
| `usedBy` | any | inverse of `uses` |
| `produces` | any | algorithm/activity produces an entity/evidence class |
| `contrastsWith` | any | explicit alternative model/technique; carries no evidential force in either direction |
| `refines` | any | more specific formulation of another concept |
| `relatedTo` | any | symmetric editorial adjacency with no epistemic content |
| `projectsTo` | any | representation can be mapped into another standard/model |
| `specifies` | any | document normatively defines the target |
| `specifiedBy` | `Document` | inverse of `specifies` |
| `documents` | any | document describes the target without normative force |
| `documentedBy` | `Document` | inverse of `documents` |
| `executedAs` | any | specification is realized as a concrete execution record |
| `controlledBy` | `Algorithm` | state is mutated only through the named procedure |
| `representedBy` | any | abstract world is represented by a concrete structure |
| `hasRevision` | `SourceRevision` | source has this dated revision identity |
| `supersedes` | any | newer revision replaces an earlier active representation |
| `plannedIn` | `Atom`, `Issue` | the issue that will create a path this node's `verification[].target` names |
| `measures` | `Experiment` | a `Result` reports on this experiment |
| `decides` | `Hypothesis` | a `Decision` resolves this hypothesis |

### Inverse pairs

These pairs are inverses. Both directions may exist between the same ordered pair of nodes, and only these:

| Forward | Inverse |
| --- | --- |
| `contains` | `partOf` |
| `requires` | `requiredBy` |
| `motivatedBy` | `motivates` |
| `formalizedBy` | `formalizes` |
| `testedBy` | `tests` |
| `implementedBy` | `implements` |
| `uses` | `usedBy` |
| `specifies` | `specifiedBy` |
| `documents` | `documentedBy` |

For any other ordered pair `(X, Y)`, at most one edge may exist whose predicate is in `{supportedBy, qualifiedBy, challengedBy, motivatedBy, groundedIn, motivates, informedBy}`. Asserting evidence in both directions between two nodes is a modelling error, not a richer graph: it says each node is the reason for the other.

New predicates must be added to this table before appearing in a shard. The graph may introduce additional predicates when the distinction materially changes interpretation.

## Status vocabulary

`status` is a closed enumeration, selected by the node's **primary class** — the first entry of its `@type` array. Every value below maps to one of the six assertion classes that `docs/research/methodology.md` section 1 defines, so that an epistemic question about a node is answerable from the node.

| Primary class | Allowed `status` | Class | Assertion class |
| --- | --- | --- | --- |
| `Concept` | `proposed` | unimplemented | engineering-deduction |
| | `specified` | unimplemented | engineering-deduction |
| | `established` | implemented | established-foundation |
| | `retired` | implemented | engineering-deduction |
| `Source` | `peer-reviewed` | implemented | established-foundation |
| | `preprint` | implemented | empirical-conclusion |
| | `standard` | implemented | established-foundation |
| | `documentation` | implemented | established-foundation |
| | `repository` | implemented | established-foundation |
| | `dataset` | implemented | empirical-conclusion |
| | `needs-source` | unimplemented | assumption |
| `SourceRevision` | `current`, `superseded` | implemented | established-foundation |
| `Claim` | `established-foundation` | implemented | established-foundation |
| | `empirical-conclusion` | implemented | empirical-conclusion |
| | `engineering-deduction` | implemented | engineering-deduction |
| `Hypothesis` | `untested` | unimplemented | hypothesis |
| | `supported`, `qualified`, `rejected` | implemented | empirical-conclusion |
| `Assumption` | `assumed` | unimplemented | assumption |
| | `tested`, `refuted` | implemented | empirical-conclusion |
| `Algorithm` | `planned`, `specified` | unimplemented | engineering-deduction |
| | `implemented` | implemented | engineering-deduction |
| `Theorem` | `planned` | unimplemented | hypothesis |
| | `proof-source-present` | unimplemented | engineering-deduction |
| | `model-only` | implemented | verified-formal-proposition |
| | `machine-checked` | implemented | verified-formal-proposition |
| `Experiment` | `planned`, `registered`, `running` | unimplemented | hypothesis |
| | `complete`, `abandoned` | implemented | empirical-conclusion |
| `Result` | `complete` | implemented | empirical-conclusion |
| `Decision` | `retain`, `revise`, `reject` | implemented | engineering-deduction |
| `Tool` | `candidate` | unimplemented | engineering-deduction |
| | `adopted`, `not-adopted`, `not-enabled` | implemented | engineering-deduction |
| `ImplementationArtifact` | `planned` | unimplemented | engineering-deduction |
| | `in-progress`, `implemented` | implemented | engineering-deduction |
| `Document` | `draft` | unimplemented | engineering-deduction |
| | `current`, `superseded` | implemented | engineering-deduction |

`proof-source-present` is deliberately in the unimplemented class. It means a Lean declaration with that name exists in the repository; it does not mean the kernel checked it under the pinned toolchain with no `sorry` and no unreviewed axiom. Only `machine-checked` means that, and only the CI job may set it. The four Theorem values are exactly those `docs/formal/theorem-catalog.md` defines.

`verification[].status` is a separate closed enumeration: `planned`, `ci-check-required`, `passing`.

### Status transitions at Atom closure

A node moves from the unimplemented class to the implemented class only when the closure record for the owning Atom — the JSON document specified by `artifacts/schema/closure-record.schema.json` — records the corresponding field.

| Node class | Transition | Triggering closure-record field |
| --- | --- | --- |
| `Algorithm`, `ImplementationArtifact` | `planned`/`specified` -> `implemented` | `artifacts[]` naming the crate path, with `tests_passing: true` |
| `Theorem` | `proof-source-present` -> `machine-checked` | `formal.kernel_checked: true` and `formal.axiom_audit_clean: true` |
| `Experiment` | `planned` -> `registered` | `experiment.protocol_digest` present |
| `Experiment` | `registered` -> `complete` | `experiment.runs_completed >= analysis_plan.min_n` |
| `Hypothesis` | `untested` -> `supported`/`qualified`/`rejected` | the `Decision` node emitted by experiment ingestion |
| `Tool` | `candidate` -> `adopted`/`not-adopted` | `decisions[]` naming the tool |

A node in the implemented class whose `url` or whose `verification[].target` path does not exist on disk is an audit error. That is what stops the graph from saying `implemented` after the code was removed, and what stops it from saying `planned` after the code landed.

## Source provenance fields

Every `Source` carries `source_kind`, drawn from `{paper, standard, documentation, repository, dataset}`, and `retrieved_at` as an ISO-8601 date.

A Source of kind `paper` additionally carries `authors`, `year`, `venue_or_publisher` and `identifier`. `identifier` is a DOI, an arXiv identifier, or — for venues that mint neither, such as USENIX proceedings and dissertations — the publisher's stable URL, with the reason recorded in the node summary.

A Source of any other kind additionally carries `maintainer_or_publisher`, `version_or_commit_or_edition` and `canonical_url`.

A Source whose work is certain but whose durable locator has not been verified carries `status: needs-source` until it has. A node that has never been checked and a node that was checked and found dead must not look the same.

## Verification records

A `verification[]` entry carries:

```text
method            how the target is checked
target            a repo path, a repo path with a #declaration anchor, or an https:// URL
status            planned | ci-check-required | passing
note              optional prose
source_digest     optional; sha256 of the file named before '#' in target
digest            optional; sha256 of a named protocol manifest
```

`source_digest` exists so that a Theorem node can detect that the Lean file it cites changed underneath it. It is required on `method: Lean4` entries whose `status` is not `planned`; entries with `status: planned` are exempt, because their target file does not exist yet. Populating and checking it is `#72`'s work (gap `theorem-source-digests-missing`); the field is defined here so the shards are not silently dropping it in the meantime.

`digest` is the pre-registration binding for an `Experiment`: the sha256 of the protocol manifest the runs were executed under. `#75` owns the CI recomputation (gap `preregistration-mechanism-missing`).

## Theorem weight

A Theorem node carries two orthogonal fields beyond `status`.

`content` records what the proof does, not whether a checker ran:

- `projection` — the proof body is a single field or structure access on a hypothesis the statement already assumes;
- `definitional` — the proof is `rfl`, `simp [<definition>]`, `subst`, or congruence over a definition or enumeration;
- `mechanism` — the proof performs induction or case analysis over an inductive relation, or otherwise derives a fact not present as a hypothesis field.

`class` records engineering weight, and is a function of `content`: `projection` -> `regression-guard`, `definitional` -> `structural`, `mechanism` -> `substantive`. `docs/formal/theorem-catalog.md` carries the same values per T-number, and a script asserts the two agree.

The distinction exists because a corpus of twenty theorems, eighteen of which restate a witness field, is not twenty times the assurance of one, and a `status` field alone cannot say so.

## Hypothesis statements

A `Hypothesis` node's `statement` states the falsification condition, not the aspiration. It has the form:

```text
Under <named workload corpus>, <treatment> changes <exactly one named primary metric>
by >= <numeric value with unit> versus <named baseline condition>,
over >= <n> seeded trials; otherwise reject.
```

Mechanically: the statement contains a digit-bearing token, the substring `baseline`, and the substring `reject`. The words `materially`, `useful`, `adequate`, `meaningful`, `acceptable cost`, `representative workload` and `ambiguity` must not appear in a Hypothesis `statement` or `summary`; each of them names a quantity no observation can measure.

Where an experiment has a pre-registered manifest, the primary metric, threshold, baseline and `n` in the statement are the same values as the manifest's `analysis_plan`, and a script diffs the four fields. The structured fields that will hold them natively are `#71`'s work; until then the sentence is the record.

## Experiment execution

An `Experiment` node's `verification[].target` MUST be one of:

- `experiments/<experiment-id>/` for an executable protocol, where `<experiment-id>` is the node id without its `experiment:` prefix, so the path is derivable from the id rather than looked up;
- `benches/<name>` for a criterion-style benchmark harness;
- `formal/conformance` for a conformance suite.

A verification entry whose own `status` is not `planned` MUST name a path that exists: `gordian-kg audit --strict` fails on it (rule **S4**). An entry with `status: planned` is exempt from the existence check, because it records where the harness is going to be rather than where it is; `audit_verification_targets` skips such an entry before resolving the path. The exemption is bounded, not open: a planned entry still MUST use one of the three forms above, and the Atom that will create the path is named by a `plannedIn` edge, so a planned target cannot sit unowned. Eight Experiment targets currently rely on it — `benches/foundation`, `benches/knowledge-graph`, `formal/conformance`, and the `experiments/` directories for `evidence-invalidation-cost`, `formal-method-yield`, `knowledge-backend-qualification`, `sandbox-backend-qualification` and `release-qualification` — and creating them is gap `experiment-verification-targets-nonexistent` (`#75`, `#37`), while enforcing `plannedIn` is gap `kg-target-locators-two-forms-no-owner` (`#72`). When those land, the entries move off `planned` and the existence check applies to every Experiment without exception.

The exemption is a property of `audit_verification_targets`, not of rule S4, so it applies wherever that check runs. It also covers rule **S3**'s one planned Theorem target, `formal/Gordian/Transition.lean` on `theorem:transition-invariant-preservation`. Rule **S8** is what stops the exemption from being a loophole: a node whose own `status` is `implemented` must have a `url` and a `verification[].target` that resolve, so nothing can claim to exist while its evidence path is still planned. `ImplementationArtifact` nodes for crates that do not exist yet carry an empty `verification[]` rather than a planned entry naming an absent crate path.

Every Experiment additionally carries the `url` of the open issue that runs it, in the canonical form `https://github.com/kmosoti/gordian/issues/<n>`. The five nodes that previously used the pseudo-path `issues/<n>` as a `verification[].target` now use a repo path there and keep the issue in `url`.

The layout `experiments/<experiment-id>/{protocol.json, runs/<run-id>/run.json}` and its two JSON Schemas are specified in `docs/spec/data-model.md` and checked in under `experiments/schema/`.

A completed run maps into the graph by this rule, executed by `cargo run -p gordian-experiments -- ingest <experiment-id>`:

```text
for each experiment with at least analysis_plan.min_n completed runs per cell:
    create Result node   result:<experiment-id>-<analysis-digest>
        relations: measures -> experiment:<experiment-id>
                   supportedBy | qualifiedBy | challengedBy -> hypothesis:<...>
    create Decision node decision:<experiment-id>-<date>
        relations: decides -> hypothesis:<...>
        status: retain | revise | reject
```

`supportedBy` is emitted only when the observed primary effect exceeds `analysis_plan.effect_size.minimum_relevant` in the hypothesized direction; when the interval spans the minimum effect the edge is `qualifiedBy`; otherwise it is `challengedBy`. This is the only place in the corpus where an evidential edge is created automatically, and it is why the sign and threshold are pre-registered rather than chosen after the run.

## Backlog mirror

The knowledge graph mirrors the backlog through one generated shard, `knowledge/graph/95-backlog.jsonld`, produced from the native GitHub graph as the single source:

- one node per open issue, `@type` `Atom` when the issue carries `type:atom` and `Experiment` when it carries `type:experiment`;
- `@id` derived from the issue number; `url` is the issue's html_url;
- one `dependsOn` edge per native `blocked_by` edge — the native graph is authoritative for dependencies, and the prose mirrors in issue bodies are projections of it;
- one `Initiative` node per milestone, with a containment edge to its members.

CI regenerates the shard and fails on any diff against the committed copy. That comparison, not `gordian-kg audit --strict`, is the check against GitHub: the audit binary loads only `knowledge/graph/` and has no network.

The generator, the regenerate-and-diff CI step, and the audit rules over the generated shard are `#72`'s work (gap `kg-backlog-mirror-undecided`); `#48`'s provenance import names the same path.

## JSON-LD context

Every shard carries a byte-identical `@context` declaring `@vocab`, all node-id prefixes, all `@type` terms, and term definitions for the `relations` and `verification` sub-object keys, retaining the `@id` coercion on `target`, `url` and `canonical_url`. Eight of the ten shards previously carried no context at all, which meant a conforming JSON-LD processor expanding them dropped every property except `@id` and `@type` — the corpus claimed linked-data identity it did not have.

`gordian-kg validate` must reject any shard whose resolved context differs from the canonical one, and a test must expand each shard with a JSON-LD 1.1 processor and assert that the count of non-`@`-prefixed properties after expansion equals the count in the raw JSON. Wiring both is `#71`'s work (gap `jsonld-context-absent-in-8-shards`).

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

### A citation must match the comparison the source ran

`A supportedBy S` requires that S ran a comparison whose result bears on A. Citing a source for a comparison it did not perform is an error, not an approximation, and audit rule **S7** treats it as one. Three edges in this corpus failed that test and were re-predicated: `claim:coordination-beyond-isolation` now carries `challengedBy -> source:caid`, because CAID's Table 3 compares worktree isolation against soft isolation and finds isolation wins; `hypothesis:snapshot-better-than-rebase` and `claim:semantic-state-vs-code-state` now carry `motivatedBy -> source:caid`, because the paper contains no snapshot-versus-rebase comparison and no code-plane-versus-semantic-plane comparison at all.

### Formalization is not empirical validation

`H formalizedBy T` means T proves a property of a formal model relevant to H. It does not establish the empirical usefulness of H unless the empirical claim itself is formal and its assumptions are demonstrated.

### Negative evidence is mandatory infrastructure

The graph must carry `challengedBy` and `qualifiedBy` edges, and the audit and review process must resist graphs that only accumulate confirming edges. Every `Hypothesis` carries at least one `challengedBy` or `qualifiedBy` edge to a `Source`, with a `note` naming the disconfirming or qualifying result. A hypothesis whose graph neighbourhood contains only supporting evidence has not been researched; it has been advocated.

### Evidence edges are acyclic

No cycle may exist over `{supportedBy, qualifiedBy, challengedBy, motivatedBy, groundedIn, informedBy}`. A cycle means a node is ultimately its own evidence. The corpus contained exactly one — `foundation:crdt --qualifiedBy--> claim:crdt-not-semantic-correctness --groundedIn--> foundation:crdt` — and it was broken by retyping the first edge to `contrastsWith` and giving the foundation a real source (`source:shapiro-2011`).

### Source revisions matter

Fast-moving papers and software documentation change. Materially changed results are represented through distinct `SourceRevision` identities rather than by overwriting historical provenance, so the graph preserves which decision depended on which revision at the time.

## Shards

The current corpus separates concerns without separating the logical graph:

```text
knowledge/graph/
  00-core.jsonld
  10-foundations.jsonld
  20-sources.jsonld
  30-studies-and-claims.jsonld
  40-algorithms.jsonld
  50-tools.jsonld
  60-formal.jsonld
  70-experiments.jsonld
  80-implementation.jsonld
  90-project-plan.jsonld
  95-backlog.jsonld   generated; see Backlog mirror
```

This list must equal `ls knowledge/graph/*.jsonld`, and CI greps both this document and `docs/knowledge-graph.md` for every shard filename. Shard boundaries are editorial only.

## Mechanical checks

```bash
cargo run -p gordian-kg -- validate
cargo run -p gordian-kg -- audit --strict
cargo run -p gordian-kg -- stats
```

`validate` checks structural graph integrity: unique ids, allowlisted `@type` values and predicates, no dangling relation targets, and a canonical `@context`.

`audit --strict` checks epistemic completeness, with every rule an error rather than a warning. `.github/workflows/verify.yml` runs `audit --strict`; it previously ran only `validate`, which is why 24 of 43 foundation nodes carried no Source edge and the corpus contained zero `challengedBy` edges.

| Rule | Statement |
| --- | --- |
| S1 | every `*Foundation` node that is not itself a `Source` has at least one `supportedBy`/`qualifiedBy`/`challengedBy`/`groundedIn` edge to a `Source` |
| S2 | every `Source` has a resolvable locator and a `retrieved_at`, and carries the provenance fields its `source_kind` requires |
| S3 | every `Theorem` has a `statement`, a `content`, a `class`, and a checker `target`; where the entry's `status` is not `planned`, the target file exists on disk and its `#declaration` anchor matches a real declaration |
| S4 | every `Experiment` has a `url` naming its open issue, and every `verification[]` entry whose `status` is not `planned` has a `target` that exists on disk; a `planned` entry is exempt from the existence check and is owned by a `plannedIn` edge (see **Experiment execution**) |
| S5 | every `Hypothesis` has an `Experiment`, a falsifiable `statement`, and at least one noted `challengedBy`/`qualifiedBy` edge to a `Source` |
| S6 | no cycles over the evidence predicates, and no mutual asymmetric evidence pair |
| S7 | no evidence-predicate edge targets a non-`Source` node, and no `Claim` cites a `Source` for a comparison that source did not run |
| S8 | every node `status` is in the enumeration for its primary class, and every node in the implemented class has a `url` and `verification[].target` that resolve |

These checks cannot determine whether a scientific inference is good. They ensure the inference can be inspected.

## Storage and indexing strategy

The JSON-LD shards remain the canonical version-controlled research corpus.

The Rust tooling currently builds an indexed `petgraph` view for traversal, neighborhood queries, shortest directed paths, statistics, audit, and DOT export.

This choice is provisional and benchmarked by `hypothesis:jsonld-petgraph-sufficient`. A future RDF/SPARQL projection may use Sophia or Oxigraph if actual query requirements justify the operational complexity. Storage and index implementation must not dictate ontology semantics.

## Research ingestion workflow

When a source or experiment changes Gordian's understanding:

1. create/update the Source node, with its provenance fields and a `SourceRevision` if the result changed;
2. create/update relevant Concept/Claim/Hypothesis nodes;
3. add supporting, qualifying, or challenging relations, checking that each names a comparison the source ran;
4. add affected algorithms/theorems/experiments/implementation artifacts;
5. update normative documentation only if the evidence justifies a semantic change;
6. update implementation artifacts separately;
7. run `validate`, `audit --strict`, and applicable benchmarks;
8. preserve uncertainty rather than resolving it editorially.

A material new concept is not fully acquired until the graph, docs, and relevant experiment/proof/implementation links agree on what it means.

## Open gaps assigned to Atoms

The vocabulary above is normative now. The tooling that enforces it is not yet written, and this section names who writes it so the obligation is addressed rather than implied.

| Gap | Owner | Deliverable |
| --- | --- | --- |
| `status-vocabulary-unenumerated` | `#72` | `knowledge/status-vocabulary.json` loaded by `gordian-kg`; audit rule S8 with a stable rule id |
| `ontology-vocabulary-unenforced` | `#72` | `@type` and predicate allowlists in `validate`, matching this document |
| `kg-closure-update-rules` | `#72` | the closure-record transition table above, enforced by audit; `audit --strict` wired into `verify.yml` |
| `theorem-weight-unclassified`, `theorem-strength-not-recorded` | `#72` | `content` and `class` on `pub struct Node`; audit rules over them; catalog/graph agreement script |
| `theorem-source-digests-missing` | `#72` | `source_digest` population and recomputation |
| `theorem-anchor-existence-unchecked` | `#72` | audit rule S3's anchor resolution, plus the same check over `theorem-catalog.md` |
| `kg-backlog-mirror-undecided` | `#72` | `95-backlog.jsonld` generator and regenerate-and-diff CI step |
| `issue-to-kg-backlinks-absent` | `#72` | the issue-body lint that resolves graph ids |
| `kg-target-locators-two-forms-no-owner` | `#72` | `plannedIn` edges and the three canonical target forms, enforced |
| `source-revision-schema-missing`, `source-provenance-fields-unpopulated` | `#71` | `SourceRevision` and the Source provenance fields as typed members of `pub struct Node` |
| `jsonld-context-absent-in-8-shards` | `#71` | context validation and the JSON-LD expansion test |
| `source-url-resolvability-unchecked` | `#74` | the scheduled locator-resolution job and its committed manifest |
| `preregistration-mechanism-missing` | `#75` | `digest` recomputation in CI and the rerun-from-manifest test |
| `experiment-execution-path-absent` | `#75` | `experiments/schema/`, the runner, and the ingestion command named above |
| `experiment-verification-targets-nonexistent` | `#75`, `#37` | the `experiments/` tree the retargeted paths now name |
| `evidence-predicates-target-non-sources` | `#72` | audit rule S7 and the mutual-pair check |
| `catalog-covers-8-of-25-theorems`, `formal-target-lists-disagree` | `#72` | the catalog index table with one row per numbered formal target, and the script comparing it to `formal/Gordian/*.lean` and to this corpus |
| `evidence-ref-missing-canonicalization-scheme`, `evidence-field-names-not-adapter-neutral` | `#15` | `canonicalizationScheme` and `exactStateId` in `formal/Gordian/Evidence.lean`; the Theorem nodes naming those anchors already exist here |
| `inv-frontier-linearization-cas-theorem-absent` | `#19` | `cas_rejects_stale` and `frontier_moved_has_intent` in `formal/Gordian/Frontier.lean`, and the Theorem nodes for them |
| `executable-model-absent-scheduler` | `#13` | `AtomSpec`, `isEnabled`, `isDispatchable` and both `_iff` theorems in `formal/Gordian/Scheduler.lean`, and the two Theorem nodes for them |
| `lean-rust-bridge-absent` | `#7` | `formal/conformance/` and `crates/gordian-conformance/`, which the retargeted `experiment:lean-rust-conformance` now names |
| `nanoda-disabled-but-claimed` | `#2` | either enabling nanoda in `verify.yml` or keeping the corpus's current position, in which `tool:nanoda` is `not-enabled`, carries no `verifiedBy` edge, and is described as not enabled in CI |
