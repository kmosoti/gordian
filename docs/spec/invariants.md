# Gordian Invariant Catalog

Status: **normative research specification**

This catalog defines safety properties independently of scheduler heuristics, model provider, UI, database, or deployment platform. Each invariant names the strongest currently applicable verification method and the remaining trust boundary.

## Hard dependency acyclicity

The global hard-dependency relation MUST be acyclic.

**Formal method:** Lean proof from a strictly decreasing rank certificate.

**Runtime obligation:** validate the certificate or compute an independently checked topological ordering/cycle detection result.

**Does not prove:** workflow liveness or absence of semantic feedback outside the hard-dependency relation.

## Decomposition/dependency separation

Containment MUST NOT silently imply execution precedence.

Sibling creation time/order is not causality.

**Verification:** domain-model tests, generated graph tests, scheduler tests.

## Quark locality

A Quark MUST belong to one Atom and MUST NOT be a cross-Atom hard-dependency target.

**Formal method:** Lean type/predicate theorem.

**Empirical boundary:** Atom/Quark usefulness remains an experiment.

## Immutable published specification revisions

Published work-specification revisions MUST be immutable.

Changed meaning creates a new revision identity.

**Verification:** persistence constraints, API/state-machine tests, evidence invalidation tests.

## Attempt/specification separation

ExecutionAttempt outcomes MUST NOT mutate the semantic definition of the work contract attempted.

**Verification:** state-machine/model tests.

## Exact execution base

Every source-changing attempt MUST identify an exact base source state.

Changing base while execution is active requires an explicit transition/new lineage rather than invisible rebasing.

**Verification:** Jujutsu adapter tests and provenance assertions.

## One normal-path writer per evolving change

The normal execution path SHOULD grant one active writer to one Jujutsu change identity. Independent speculative executions receive separate logical changes from the same exact base.

Jujutsu divergence remains a recoverable repository state, not the intended ownership protocol.

**Verification:** lease/state-machine tests plus Jujutsu integration tests.

## Candidate freeze

Verification MUST target a frozen exact Candidate.

Subsequent source mutation creates a different verification subject.

**Formal support:** evidence identity mismatch theorem.

**Planned strengthening:** transition theorem that candidate-bound evidence can only be emitted for a frozen candidate.

## Evidence compatibility

Evidence MAY satisfy acceptance only when every identity declared relevant by the verifier policy matches the current subject.

Required candidate verification binding includes at least:

```text
specification revision
exact candidate commit
resolved input/dependency identity
relevant environment identity
verifier identity
canonicalization scheme
```

**Formal method:** Lean mismatch-implies-incompatibility theorems.

**Trust boundary:** the fingerprint may omit a real dependency; completeness must be tested/observed.

## Verification completeness

An Atom MUST NOT be Satisfied while a required verifier is missing, failing, stale, or incompatible.

**Verification:** Lean transition model when available, Rust property tests, mutation tests, end-to-end tests.

## Integration non-compositionality

Composing independently verified candidates creates a distinct integration candidate that MUST receive applicable integration verification.

```text
Verified(A) ∧ Verified(B)
```

does not imply:

```text
Verified(Integrate(A, B))
```

unless a specific property has itself been proven compositional.

## Conflict exclusion

A candidate with unresolved structural/VCS conflicts MUST NOT enter the accepted frontier.

**Formal method:** acceptance witness carries conflict-free evidence.

**Boundary:** textual/VCS conflict freedom is not semantic correctness.

## Worker authority boundary

A Worker MUST NOT possess accepted-frontier promotion authority by default.

**Formal method:** Lean role/capability theorem.

**Runtime obligation:** enforce with real credentials, policy, sandboxing, remote permissions, and secret isolation.

## Deployment separation

Accepted source state and deployed external state are different frontiers.

Coordinator authority MUST NOT imply deployment authority by default.

**Formal method:** Lean capability separation.

## Logical readiness safety

Dispatch MUST imply:

```text
valid specification
hard dependency conditions satisfied
preconditions hold
compatible executor available
resource policy satisfied
authorization valid
lease policy satisfied
```

**Formal method:** Lean dispatch witness implications.

**Runtime obligation:** differential/property testing against the formal predicate.

## Declared non-interference symmetry

The declared pairwise read/write non-interference predicate MUST be symmetric.

**Formal method:** Lean theorem.

**Boundary:** declared resource independence is not proof of semantic commutativity or complete conflict serializability.

## Declared/observed dependency reconciliation

Observed dependencies/access SHOULD be a subset of what policy treats as declared/authorized for the attempt.

Unexpected writes MUST trigger a scope-expansion fact and conflict re-evaluation.

**Verification:** instrumentation/property tests and controlled conflict experiments.

**Boundary:** observation has blind spots; unobserved does not imply nonexistent.

## Lease exclusivity

Two simultaneously valid exclusive write leases over the same semantic resource MUST NOT coexist.

A stale holder MUST NOT retain write authority after a newer fencing token has superseded it where fencing is supported.

**Verification:** planned Lean transition theorem, Loom/Shuttle concurrency exploration, fault injection.

## Replay purity

Reconstructing a projection from recorded events MUST NOT invoke nondeterministic/effectful work merely because replay occurred.

**Verification:** architecture/type boundary, replay tests, effect-spy tests.

## Replay stability

Given the same ordered canonical history and the same deterministic projector implementation, canonical projection state MUST be identical.

**Formal method:** basic functional Lean theorem plus future transition-model proofs.

**Implementation method:** destroy/rebuild projections and byte/semantic compare canonical state.

## Event idempotency

Duplicate delivery of the same canonical event identity MUST NOT duplicate its semantic effect.

**Verification:** property/state-machine tests, distributed simulation.

## Accepted-frontier linearization

Accepted-frontier mutation MUST use an expected previous frontier/version or equivalent single-writer/consensus mechanism so concurrent coordinators cannot silently lose updates.

**Verification:** compare-and-swap tests, Turmoil partition/retry simulation, later state-machine proof.

## Retry/effect safety

Automatic retry policy MUST depend on effect class.

Irreversible effects MUST NOT be automatically replayed/retried after ambiguous completion without explicit authority and recovery policy.

**Verification:** state-machine tests and fault injection around timeouts/crashes.

## Provenance closure

Every externally meaningful Artifact or verification Evidence record MUST identify its producing activity or external source and exact subject/material identity sufficient for its declared provenance contract.

**Verification:** schema constraints and property tests; compatibility projection to W3C PROV/in-toto/SLSA.

## Knowledge epistemic integrity

The research knowledge graph MUST distinguish Source, Claim, Hypothesis, Algorithm, Theorem, Experiment, Assumption, Concept, and Implementation Artifact.

A `formalizedBy` edge MUST NOT be interpreted as proof that the linked real-world hypothesis is true.

A `supportedBy` edge MUST preserve scope/limitations rather than imply deduction.

Material disconfirming evidence MUST be representable through `challengedBy` or `qualifiedBy` relations.

**Verification:** knowledge-graph schema/lint rules plus review.

## Rust/formal semantic agreement

For semantics modeled in Lean and implemented in Rust, Gordian SHOULD use differential randomized testing over generated inputs.

A release-critical mismatch MUST fail verification.

This follows the verification-guided-development pattern demonstrated by Cedar: proofs establish model properties, differential testing supplies evidence that optimized production Rust agrees with the executable model.

**Boundary:** randomized differential testing is evidence, not a universal refinement proof unless every input is exhaustively covered or a refinement theorem exists.

## Performance regression boundary

Performance-sensitive substrate algorithms MUST have benchmark baselines and regression thresholds before optimization becomes relied upon architecturally.

Benchmarks SHOULD record graph size/shape, critical path, density, worker heterogeneity, contention, allocation/memory, and wall-clock/CPU behavior.

A greedy/reference implementation SHOULD remain available as an oracle where practical.

**Verification:** benchmark suite, statistical comparison, deterministic instruction-level profiling where useful.

## Unsafe-code containment

Production Rust SHOULD remain safe Rust. Any `unsafe` block requires a written safety invariant, isolated surface, testing/model-checking appropriate to the risk, and measured justification.

**Verification:** code policy, Miri/sanitizers/Kani where applicable, focused review.
