# Gordian Invariant Catalog

Status: **normative research specification**

This catalog defines safety properties independently of scheduler heuristics, model provider, UI, database, or deployment platform. Each invariant names the strongest currently applicable verification method and the remaining trust boundary.

Every section in this catalog carries a `**Coverage:**` line whose value is one of the six states
in [`../formal/theorem-catalog.md` "Formal coverage metric"](../formal/theorem-catalog.md):
`formalized`, `property-tested`, `model-checked`, `integration-tested`, `empirical-only`,
`unverified`. `scripts/check-invariant-coverage.sh` asserts that between every pair of `## `
headings there is exactly one `**Coverage:**` line and that its value is drawn from that list. The
coverage state describes what is verified **today**; where the strongest method is still owed by an
Atom, the section names the owing gap and Atom rather than claiming the method in advance.

### Newly required invariants introduced by this revision

Collected so that nothing is implicit. Every one is stated normatively below.

1. **Satisfaction requires admission.** No Atom is Satisfied without an admitted frontier
   containing its Candidate.
2. **Prerequisite-containing execution base.** No attempt is dispatched against a base that omits
   a prerequisite's admitted state.
3. **Declared compositionality.** A component verifier result crosses into an integration subject
   only through an explicit `compositional = true` manifest entry and a recorded
   `EvidenceInherited` event.
4. **Integration manifest coverage.** Every required verifier of every transitive parent is a
   member of the integration manifest.
5. **Batch-only re-verification.** Admission of a batch invalidates evidence only for candidates
   inside that batch.
6. **Event-log linearization.** The event log, not the source bookmark, is the accepted
   frontier's compare-and-swap target and its authoritative value.
7. **Admission progress.** No candidate is preempted indefinitely, and no admission intent
   blocks admission forever.
8. **Fencing at candidate freeze.** Every Candidate carries the token of a live exclusive lease
   on its logical change, and admission re-checks it transitively.
9. **Evidence provenance binding.** Every counted verifier record traces to a
   `VerificationStarted` event with a matching subject fingerprint.
10. **Conflict-aware verification.** A fresh failing record defeats a fresh passing record for
    the same `(verifier, fingerprint)`.
11. **Interface and artifact provision closure.** Exactly one `ProviderBinding` per requirement;
    unprovided and ambiguous plans are rejected.
12. **Derived-edge completeness.** Re-running validation on a frozen plan reproduces its derived
    edge set exactly.
13. **Integration acyclicity.** Every parent of an `IntegrationCandidate` was frozen at a strictly
    smaller `EventSeq`.
14. **Exclusive lease exclusion over `LeaseSubject`**, including `LogicalChange(x)`.
15. **Total retry policy.** Every one of the seven effect classes has exactly one rule.
16. **Single normative list.** For each of: the admission conjuncts, the `Satisfied` definition,
    the readiness predicate definitions, the closure record fields, and the Mission acceptance
    items, exactly one document is normative and every other cites it — enforced by a checker.
17. **Frontier-version compare-and-swap.** Admission's CAS target is `FrontierVersion`, never the
    global log head and never a bookmark; unrelated appends never fail an admission.
18. **Witness read-consistency.** The admission witness is evaluated at a named
    `ProjectionVersion` and committed under a guard on that version; nothing the witness read may
    have changed in between.
19. **Recorded witness.** `CandidateAdmitted` carries the witness itself, so recovery replays a
    decision rather than re-deciding it.
20. **All-or-nothing completion.** `FrontierMoved` and every `AtomSatisfied` of a batch are one
    transactional append, and both step 1 and step 4 are conditional, so an intent completes
    exactly once.
21. **Single admitting coordinator.** Admission requires a live exclusive lease on
    `LeaseSubject::Coordinator(project)`; takeover is keyed on lease liveness, not on process
    start.
22. **Compensated abort.** `AdmissionAborted` is preceded by a `reset_frontier` to the expected
    frontier, so a permanent publish failure cannot wedge admission.
23. **Published-frontier convergence.** The published frontier is re-driven by crash recovery and
    by a divergence check that runs before every admission and on a timer.
24. **No double admission.** `ParentsUnadmitted` plus an exclusive admission-queue claim make a
    candidate admissible at most once.
25. **Rejection is an event.** A false witness conjunct appends `AdmissionRejected` and returns the
    Atom to a defined state.
26. **Bounded admission progress.** `admission_attempts` counts preemption and integration
    conflict; escalation is an exclusive batch in FIFO order; exhaustion is terminal.
27. **Replay purity.** No predicate reads a wall clock; expiry is event-denominated.
28. **Recoverable satisfaction.** Every `SatisfactionInvalidated` reason has a producer and a
    trigger, and `SatisfactionRestored` re-establishes satisfaction on the current frontier.
29. **Plan-frozen integration manifests.** `integration_manifest(I)` reads `I.plan_revision`, so a
    `PlanSelected` never retroactively changes a historical fingerprint.
30. **Evidence carries its binding.** `Evidence.binding` is a required field, so `Fresh` is
    implementable.

## Hard dependency acyclicity

The global hard-dependency relation MUST be acyclic.

**Formal method:** Lean proof from a strictly decreasing rank certificate.

**Runtime obligation:** validate the certificate or compute an independently checked topological ordering/cycle detection result.

**Does not prove:** workflow liveness or absence of semantic feedback outside the hard-dependency relation.

**Coverage:** formalized

## Decomposition/dependency separation

Containment MUST NOT silently imply execution precedence.

Sibling creation time/order is not causality.

**Verification:** domain-model tests, generated graph tests, scheduler tests.

**Planned formal method:** a `contains` relation distinct from `dependsOn` in
`formal/Gordian/Graph.lean` and a `blocked_ignores_containment` theorem over it — **G-249**, owned
by #10. Nothing in `formal/` models containment today, so this invariant is not Lean-covered.

**Coverage:** property-tested

## Quark locality

A Quark MUST belong to one Atom and MUST NOT be a cross-Atom hard-dependency target.

**Formal method:** policy-encoding regression guard only (T002: not GloballyDependable Quark). Structural edge theorem and Quark-ownership uniqueness: planned (#10).

**Planned formal method:** `formal/Gordian/Graph.lean#edge_target_globally_dependable` and a
Quark-ownership uniqueness declaration — **G-229** and **G-232**, owned by #10.

**Empirical boundary:** Atom/Quark usefulness remains an experiment.

**Coverage:** property-tested

## Immutable published specification revisions

Published work-specification revisions MUST be immutable.

Changed meaning creates a new revision identity.

**Verification:** persistence constraints, API/state-machine tests, evidence invalidation tests.

**Coverage:** integration-tested

## Attempt/specification separation

ExecutionAttempt outcomes MUST NOT mutate the semantic definition of the work contract attempted.

**Verification:** state-machine/model tests.

**Coverage:** property-tested

## Exact execution base

Every source-changing attempt MUST identify an exact base source state.

Changing base while execution is active requires an explicit transition/new lineage rather than invisible rebasing.

**Verification:** source-adapter tests and provenance assertions.

**Coverage:** integration-tested

## Prerequisite-containing execution base

The exact base assigned to any `ExecutionAttempt` for Atom `b` MUST be an admitted frontier state
`F` such that every hard dependency of `b` is Satisfied at a frontier `F' <= F`
([`mission-graph.md` `### PrerequisiteContaining`](mission-graph.md#stable-snapshots)).

**Verification:** scheduler property tests over generated dependency graphs; a dispatch that
violates it is a hard error, not a warning.

**Coverage:** property-tested

## One normal-path writer per evolving change

The normal execution path SHOULD grant one active writer to one logical change identity as
provided by the source adapter.
Independent speculative executions receive separate logical changes from the same exact base.

Two live `write_exclusive` Leases with the same `LeaseSubject` MUST NOT coexist. Equivalently: at
most one live `write_exclusive` Lease may have subject `LeaseSubject::LogicalChange(x)` for any
`x`, and a request for a second while the first is live MUST be refused.
`LeaseSubject::LogicalChange` MUST NOT be granted in `write_shared_if_commutative` mode.

Source-plane divergence — two exact states claiming one logical change — remains a recoverable
repository state, not the intended ownership protocol.

**Verification:** lease/state-machine tests plus source-adapter integration tests.

**Coverage:** formalized

Before this revision the invariant had no representable subject: `Lease` was keyed on
`semantic_resource`, and a source change is not a `SemanticResource`. It is now
`formal/Gordian/Lease.lean#no_two_live_exclusive`, stated at `LeaseSubject`.

## Candidate freeze

Verification MUST target a frozen exact Candidate.

Subsequent source mutation creates a different verification subject.

**Formal support:** evidence identity mismatch theorem.

**Planned strengthening:** transition theorem that candidate-bound evidence can only be emitted for a frozen candidate.

**Coverage:** formalized

## Evidence compatibility

Evidence MAY satisfy acceptance only when every identity declared relevant by the verifier policy matches the current subject.

Required candidate verification binding is exactly the seven fields of `EvidenceBinding`
([`data-model.md` `## Evidence`](data-model.md#evidence)):

```text
spec_revision
exact_state_id
input_digest
dependency_digest
environment_digest
verifier_digest
canonicalization_scheme
```

The freshness predicate compares exactly these seven and the fingerprint hashes exactly these
seven; a predicate that omits `dependency_digest` or `canonicalization_scheme` builds this
catalog's own failure mode into the normative rule. Aligning
[`../algorithms/evidence-and-admission.md`](../algorithms/evidence-and-admission.md) and
`formal/Gordian/Evidence.lean` with this list is **G-310**, owned by #15.

**Formal method:** Lean mismatch-implies-incompatibility theorems.

**Trust boundary:** the fingerprint may omit a real dependency; completeness must be tested/observed.

**Coverage:** formalized

## Verification completeness

An Atom MUST NOT be Satisfied while a required verifier is missing, failing, stale, conflicting,
or incompatible, and MUST NOT be Satisfied on the strength of a candidate that was never admitted.

**Formal method:** `formal/Gordian/Frontier.lean#not_satisfied_when_required_verifier_missing`
and `formal/Gordian/Frontier.lean#satisfied_requires_admission`.

**Verification:** Rust property tests, mutation tests, end-to-end tests, differential tests
against the Lean model.

**Coverage:** formalized

## Satisfaction requires admission

`Satisfied(a)` MUST NOT hold on the strength of evidence about a candidate that has not been
admitted into the accepted frontier. Satisfaction is recorded only by an `AtomSatisfied` event
appended inside a successful `admit()` transaction, or by a `SatisfactionRestored` event appended
under the same preconditions; it is removed only by a `SatisfactionInvalidated` event carrying one
of the five enumerated reasons, each of which has a named producer, trigger, and append
precondition.

Satisfaction MUST be recoverable. An Atom whose Candidate is already an ancestor of the frontier
can never be a batch member again, so an invalidation with no restoration path would permanently
`Block` its entire dependent subtree with no repair available to an autonomous agent.
`SatisfactionRestored` re-establishes it from fresh passing evidence for the Atom's manifest
evaluated on the **current frontier**, with no new candidate required
([`mission-graph.md` `### Satisfaction`](mission-graph.md#satisfaction)).

**Formal method:** `formal/Gordian/Frontier.lean#satisfied_requires_admission`.

**Verification:** projector mutation tests asserting no other code path writes
`satisfaction_index`; a test that invalidates a satisfied Atom whose candidate is an ancestor of
the frontier and asserts the dependent subtree is recoverable without a new `PlanRevision`.

**Coverage:** formalized

## Integration non-compositionality

Composing independently verified candidates creates a distinct integration candidate that MUST
receive applicable integration verification.

```text
Verified(A) ∧ Verified(B)
```

does not imply:

```text
Verified(Integrate(A, B))
```

unless the specific verifier's manifest entry declares `compositional = true`, which is an
author's assertion recorded as an `EvidenceInherited` event, is falsifiable, and is not a proof.

Evidence bound to a component candidate is not compatible with the integration result whenever
their `exact_state_id`s differ, which is always.

Every required verifier of every component remains a member of the integration manifest —
inheritable entries are marked, not removed — so integration admission cannot silently skip a
component Atom's manifest.

**Formal method:** `formal/Gordian/Frontier.lean#integration_needs_own_evidence`.

**Verification:** integration-verification tests, seeded compose-only defect injection,
`experiment:compositional-verifier-inheritance`.

**Coverage:** formalized

This was previously the only invariant in the catalog with neither a `**Verification:**` nor a
`**Formal method:**` line, and the Lean had no integration operation at all.

## Conflict exclusion

A candidate with unresolved structural/VCS conflicts MUST NOT enter the accepted frontier.

**Formal method:** acceptance witness carries conflict-free evidence.

**Boundary:** textual/VCS conflict freedom is not semantic correctness.

**Coverage:** formalized

## Worker authority boundary

A Worker MUST NOT possess accepted-frontier promotion authority by default.

**Formal method:** Lean role/capability theorem.

**Runtime obligation:** enforce with real credentials, policy, sandboxing, remote permissions, and secret isolation.

**Coverage:** formalized

## Deployment separation

Accepted source state and deployed external state are different frontiers.

Coordinator authority MUST NOT imply deployment authority by default.

**Formal method:** Lean capability separation.

**Coverage:** formalized

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

Each named condition is the predicate of the same name defined in
[`mission-graph.md` `## Readiness predicate definitions`](mission-graph.md#readiness-predicate-definitions).

**Formal definition:** Lean dispatch witness structure (definitional; no theorem content until the transition model exists).

**Planned formal method:** `theorem:transition-invariant-preservation`.

**Runtime obligation:** differential/property testing against the formal predicate.

**Coverage:** unverified

## Declared non-interference symmetry

The declared pairwise read/write non-interference predicate MUST be symmetric.

**Formal method:** Lean theorem.

**Boundary:** declared resource independence is not proof of semantic commutativity or complete conflict serializability. A resource's `metadata.commutative_operations` is therefore declared explicitly and MUST NOT be inferred from declared independence ([`data-model.md` `## Semantic resources`](data-model.md#semantic-resources)).

**Coverage:** formalized

## Declared/observed dependency reconciliation

Observed dependencies/access SHOULD be a subset of what policy treats as declared/authorized for the attempt.

Unexpected writes MUST trigger a scope-expansion fact and conflict re-evaluation.

**Verification:** instrumentation/property tests and controlled conflict experiments.

**Boundary:** observation has blind spots; unobserved does not imply nonexistent.

**Coverage:** property-tested

## Interface and artifact provision closure

Plan validation MUST reject publication of any `PlanRevision` containing a `required_interfaces`
or `declared_inputs` entry that has no `ProviderBinding`, or whose provider is ambiguous, or
whose provider is neither a plan member nor a registered `ExternalProvision`.

Exactly one `ProviderBinding` MUST exist per `(consumer_atom, requirement)` in a published plan,
which is what makes the derived hard-dependency edge set a function of the plan rather than a
choice made by whichever component resolves it first.

**Verification:** plan-validation property tests over generated plans with zero, one, and many
providers per requirement; `scripts/check-derived-edges.sh`.

**Coverage:** property-tested

## Lease exclusivity

Two simultaneously valid exclusive write leases over the same `LeaseSubject` MUST NOT coexist.

A stale holder MUST NOT retain write authority after a newer fencing token has superseded it.
Where the external resource cannot enforce fencing — the source plane does not — Gordian MUST
enforce it at the boundary where the holder's output is consumed:
[`LeaseValidAtFreeze`](../algorithms/evidence-and-admission.md#the-admission-conjuncts-defined) rejects
any candidate whose recorded `fencing_token` is not the highest granted for its
`LeaseSubject::LogicalChange` at freeze time.

**Formal method:** `formal/Gordian/Lease.lean#no_two_live_exclusive`,
`#logical_change_never_shared_write`, `#superseded_holder_rejected`.

**Verification:** Loom/Shuttle concurrency exploration, fault injection, admission-level fencing
tests.

**Coverage:** formalized

## Fencing at candidate freeze

Every `Candidate` MUST record the `FencingToken` of the producing attempt's live
`write_exclusive` lease on `LeaseSubject::LogicalChange(logical_change_id)` at freeze time, and
admission MUST re-check it for the candidate and for every transitive parent candidate.

**Verification:** lease/state-machine tests, admission rejection tests, fault injection that
pauses a holder past lease expiry and replays its handoff.

**Coverage:** property-tested

## Replay purity

Reconstructing a projection from recorded events MUST NOT invoke nondeterministic/effectful work merely because replay occurred.

Lean functions are pure, so the model cannot express this obligation; it lives entirely in the Rust projector (#12).

**Verification:** architecture/type boundary, replay tests, effect-spy tests.

**Coverage:** integration-tested

## Replay stability

Given the same ordered canonical history and the same deterministic projector implementation, canonical projection state MUST be identical.

**Formal method:** basic functional Lean theorem plus future transition-model proofs.

**Implementation method:** destroy/rebuild projections and byte/semantic compare canonical state.

**Coverage:** integration-tested

## Replay purity of predicates

No readiness, lease, capability, or admission predicate may read a wall clock, host telemetry, or
any value not derivable from the canonical event log at a named `ProjectionVersion`. Lease and
capability expiry are `EventSeq`-denominated (`expires_at_event`, with `LeaseExpired` and
`CapabilityExpired` events); `issued_at` and `expires_at` are retained as provenance and are read
by no predicate.

This is what makes `## Replay stability` true rather than aspirational. Under the previous wording
`AuthorizationValid` and `LeaseCompatible` compared against `now`, so rebuilding projections from
the same history a week later evaluated every historical lease as expired, recomputed a different
admission witness, and produced a witness digest that no longer matched the one recorded — the
invariant was violated by construction by the very procedure its own verification method
prescribes.

**Verification:** `scripts/check-replay-purity.sh` greps every predicate body in
`docs/spec/mission-graph.md`, `docs/algorithms/scheduling.md`, and
`docs/algorithms/evidence-and-admission.md` for the bare token `now` and fails on a hit; a
projector test rebuilds a recorded history at a simulated later date and asserts byte-identical
projections; the conformance vectors carry an explicit evaluation point so a clock-induced
divergence is representable as a vector.

**Coverage:** property-tested

## Event idempotency

Duplicate delivery of the same canonical event identity MUST NOT duplicate its semantic effect.

**Verification:** property/state-machine tests, distributed simulation.

**Coverage:** property-tested

## Accepted-frontier linearization

The accepted frontier derived from the canonical event log — `project(H).accepted_frontier` — is
**authoritative**. The source revset alias `trunk()`, the remote bookmark `main@origin`, and the
`accepted_frontier` query projection are projections of it and MUST NOT be read as the frontier
when they disagree.

Accepted-frontier mutation MUST use an expected previous frontier version. The compare-and-swap
target is **`FrontierVersion`** — the `EventSeq` of the newest event in the frontier stream
(`CandidateAdmitted` | `FrontierMoved` | `AdmissionAborted` | `AdmissionRejected`) — and never the
source bookmark and never the global `EventSeq` log head. The bookmark cannot be the target
because two stores cannot be compare-and-swapped together; the log head cannot be the target
because it advances on every unrelated append, so admission would livelock under any concurrent
worker activity. The protocol is: append `CandidateAdmitted` under a `FrontierVersion`
precondition **and** a `WitnessGuard` on the projection version the witness was evaluated at,
idempotently move the local bookmark, idempotently publish it, then append `FrontierMoved` and
every `AtomSatisfied` in one conditional transactional append
([`../algorithms/evidence-and-admission.md` `### The algorithm`](../algorithms/evidence-and-admission.md#the-algorithm)).

Admission additionally requires a live exclusive `LeaseSubject::Coordinator(project)` lease, so
one Project has one admitting coordinator; intent completion is a leased operation and takeover is
keyed on that lease ceasing to be live, never on a process having started.

Admitted frontier states form a **chain**: for every successful admission, `frontier_seq`
increases by exactly one and the previous frontier state is an ancestor of the new one. Ancestry
between admitted frontier states therefore reduces to comparing `FrontierSeq`, which is what makes
`PrerequisiteContaining` an integer comparison.

**Divergence reconciliation.** The local bookmark and the published bookmark are each re-driven to
the event log's value whenever they disagree with it. Any disagreement is recorded by appending
`FrontierDivergenceObserved { expected, observed, source }` — with `source` naming which of the two
projections diverged — before the re-drive. The check runs at coordinator start, **before every
admission**, and on a timer, because a divergence that arises in steady state and is only repaired
at the next restart leaves every worker basing on a stale frontier in the meantime.

**Verification:** compare-and-swap tests, Turmoil partition/retry simulation, crash injection
between the intent and completion events, later state-machine proof.

**Coverage:** model-checked

## Admission progress

A candidate that is repeatedly preempted at the frontier compare-and-swap, or repeatedly removed
from a batch as the attributable conflicting member, MUST reach a terminal outcome — admitted,
aborted, or `AdmissionRejected` — within a bounded number of admissions. It MUST NOT be deferred
indefinitely.

Concretely: a candidate whose `admission_attempts` — the projection counting **both**
`AdmissionPreempted` and `IntegrationConflictObserved` events naming it — has reached
`MAX_ADMISSION_ATTEMPTS` (default `3`,
[`../algorithms/evidence-and-admission.md` `### Admission fairness bound`](../algorithms/evidence-and-admission.md#admission-fairness-bound))
MUST be admitted in an **exclusive batch** in which it is the only member and during which no
other batch is assembled, ahead of newly queued candidates and in strict FIFO order of its first
attempt event; and one that then fails `MAX_EXCLUSIVE_ATTEMPTS` exclusive admissions MUST be
closed with `AdmissionRejected`. A remedy that merely re-batches the candidate is a no-op, because
every candidate is already a batch member.

A failed witness conjunct MUST NOT be silent: `admit()` appends `AdmissionRejected` naming the
false conjunct, releases the batch claims, and returns the affected Atom to a defined state
([`../algorithms/evidence-and-admission.md` `### Admission rejection`](../algorithms/evidence-and-admission.md#admission-rejection)).
A rejected candidate that produced no event is an Atom that is neither Active nor Satisfied nor
re-dispatchable.

An admission intent MUST NOT block admission forever: a `CandidateAdmitted` whose `move_frontier`
or `publish_frontier` cannot complete MUST be closed with `AdmissionAborted` after
`MAX_REDRIVE_ATTEMPTS`, **preceded by a compensating `reset_frontier` to the expected frontier**,
which is the only event permitted to cancel an intent. An abort that does not roll the source
plane back converts one permanent push failure into a permanent admission deadlock.

**Verification:** state-machine tests plus Turmoil/Shuttle contention simulation asserting that
under continuous arrival every enqueued candidate reaches a terminal outcome within a bounded
number of admissions, including the case where the candidate is always the attributable
conflicting member; a crash-injection test in which `move_frontier` fails permanently and the
next admission must still succeed; and a test asserting a rejected conjunct emits
`AdmissionRejected`.

**Coverage:** model-checked

## Retry/effect safety

Automatic retry policy MUST depend on effect class, and MUST be **total** over the seven
`EffectClass` values.

Irreversible effects MUST NOT be automatically replayed/retried after ambiguous completion
without explicit authority and recovery policy.

The per-class rules are the table in
[`../algorithms/reconciliation.md` `## 8. Retry semantics depend on effects`](../algorithms/reconciliation.md#8-retry-semantics-depend-on-effects).
A Rust implementation MUST realize it as `fn retry_policy(EffectClass) -> RetryRule` matching
exhaustively without a `_` wildcard.

**Formal method:** `formal/Gordian/EffectClass.lean#retryPolicy`,
`#irreversible_never_auto_retried`, `#judgment_never_overwrites`.

**Verification:** state-machine tests and fault injection around timeouts/crashes; an exhaustive
match test that fails to compile when a class is added.

**Coverage:** formalized

## Provenance closure

Every externally meaningful Artifact or verification Evidence record MUST identify its producing
activity or external source and exact subject/material identity sufficient for its declared
provenance contract.

For `evidence_type = verifier_result`, `producer_attempt` is required and MUST reference a
`VerificationStarted` event whose subject fingerprint equals the record's `subject_fingerprint`
([`../algorithms/evidence-and-admission.md` `### Evidence provenance validity`](../algorithms/evidence-and-admission.md#evidence-provenance-validity)).

**Verification:** schema constraints and property tests; compatibility projection to W3C
PROV/in-toto/SLSA.

**Coverage:** property-tested

## Knowledge epistemic integrity

The research knowledge graph MUST distinguish Source, Claim, Hypothesis, Algorithm, Theorem, Experiment, Assumption, Concept, and Implementation Artifact.

A `formalizedBy` edge MUST NOT be interpreted as proof that the linked real-world hypothesis is true.

A `supportedBy` edge MUST preserve scope/limitations rather than imply deduction.

Material disconfirming evidence MUST be representable through `challengedBy` or `qualifiedBy` relations.

**Verification:** knowledge-graph schema/lint rules plus review.

**Planned verification:** one stable audit rule ID per MUST above — `KG-EPI-001` (node-kind
distinction), `KG-EPI-002` (`formalizedBy` is not proof, and `supportedBy` preserves scope), and
`KG-EPI-003` (`challengedBy` / `qualifiedBy` representability) — emitted by
`gordian-kg audit --strict --format json` with a fixture test per rule. That is **G-250**, owned by
#72; none of the three MUSTs is machine-checked today, so this section stays `unverified` until
those rule IDs land, at which point it becomes `integration-tested`.

**Coverage:** unverified

## Rust/formal semantic agreement

For semantics modeled in Lean and implemented in Rust, Gordian SHOULD use differential randomized testing over generated inputs.

A release-critical mismatch MUST fail verification.

This follows the verification-guided-development pattern demonstrated by Cedar: proofs establish model properties, differential testing supplies evidence that optimized production Rust agrees with the executable model.

**Planned verification:** the conformance harness of
[`../formal/conformance-vectors.md`](../formal/conformance-vectors.md), wired into
`.github/workflows/verify.yml` as a required step with no `continue-on-error`, exiting non-zero on
a seeded mismatch. That is **G-228**, owned by #7. No Lean-modeled semantics has a Rust
implementation to differentially test today, so nothing is verified here yet; the coverage state
becomes `integration-tested` when that step is named and required.

**Boundary:** randomized differential testing is evidence, not a universal refinement proof unless every input is exhaustively covered or a refinement theorem exists.

**Coverage:** unverified

## Performance regression boundary

Performance-sensitive substrate algorithms MUST have benchmark baselines and regression thresholds before optimization becomes relied upon architecturally.

Benchmarks SHOULD record graph size/shape, critical path, density, worker heterogeneity, contention, allocation/memory, and wall-clock/CPU behavior.

A greedy/reference implementation SHOULD remain available as an oracle where practical.

**Verification:** benchmark suite, statistical comparison, deterministic instruction-level profiling where useful; the reporting rules are [`../testing/statistical-contract.md`](../testing/statistical-contract.md).

**Coverage:** empirical-only

## Unsafe-code containment

Production Rust SHOULD remain safe Rust. Any `unsafe` block requires a written safety invariant, isolated surface, testing/model-checking appropriate to the risk, and measured justification.

**Verification:** code policy, Miri/sanitizers/Kani where applicable, focused review.

**Coverage:** integration-tested
