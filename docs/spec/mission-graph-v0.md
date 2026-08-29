# Mission Graph v0

Status: **research specification / unstable**

This document defines the first normative vocabulary and invariants for Gordian's Mission Graph. It is intentionally small enough to implement and falsify.

Normative words `MUST`, `MUST NOT`, `SHOULD`, `SHOULD NOT`, and `MAY` indicate intended protocol strength for the experimental implementation.

## 1. Purpose

The Mission Graph represents engineering intent and the conditions under which that intent may be considered satisfied.

It does not represent source history. Jujutsu owns source/change history.

It does not treat execution attempts as work specifications.

It does not treat manually edited status as authoritative completion truth.

## 2. Core identities

Every logical object MUST have a persistent logical identity independent of mutable revisions.

The initial object kinds are:

```text
Project
Mission
PlanRevision
Initiative
Atom
Quark
ExecutionAttempt
Artifact
Evidence
Attestation
Actor
Capability
Event
```

Objects that have mutable specifications MUST reference immutable specification revisions.

## 3. Decomposition model

The allowed decomposition relation is:

```text
Project -> Mission
Mission -> PlanRevision
PlanRevision -> Initiative
Initiative -> Atom
Atom -> Quark
```

This relation describes semantic decomposition only.

The system MUST NOT infer execution ordering merely from sibling order or creation time.

## 4. Mission

A Mission MUST define:

```text
id
goal
constraints
acceptance
```

A Mission MAY define:

```text
scope
non_goals
risk_constraints
resource_budget
deadline
priority
```

A Mission MUST NOT embed a single implementation strategy as part of its logical identity.

A strategy MUST be represented by a PlanRevision.

## 5. PlanRevision

A PlanRevision MUST:

- belong to exactly one Mission;
- be immutable once published;
- identify the decomposition strategy it proposes;
- identify the specification revision of the Mission it was designed to satisfy.

A Mission MAY have multiple PlanRevisions.

At most one PlanRevision SHOULD be considered the currently selected plan for a Mission, but historical revisions MUST remain addressable.

## 6. Initiative

An Initiative represents a compound capability or subgoal.

An Initiative MUST define:

```text
id
objective
acceptance
```

An Initiative MAY define child Atoms through the decomposition relation.

Initiative satisfaction MUST be derived from its own acceptance contract. Child satisfaction MAY be evidence used by that contract but MUST NOT automatically imply Initiative satisfaction.

## 7. Atom

An Atom is the minimum globally schedulable and independently verifiable work contract.

An Atom specification SHOULD expose:

```text
id
objective
preconditions
declared_inputs
declared_outputs
hard_dependencies
semantic_reads
semantic_writes
required_interfaces
provided_interfaces
resource_requirements
effect_class
acceptance_predicates
verifier_manifest
```

### 7.1 Atom abstraction rule

Global scheduling and hard dependency edges MUST target Atoms or higher-level contract objects, not another Atom's Quarks.

If a Quark produces something another Atom must depend upon, the producer MUST expose that result through an Atom-level output/interface or the Quark MUST be promoted into an Atom.

## 8. Quark

A Quark is an execution primitive local to an Atom.

A Quark MAY describe:

```text
operation
inputs
outputs
local_preconditions
local_verifier
effect_class
```

Quarks SHOULD be created only when they help execution, observability, retry, delegation, or provenance.

The system SHOULD NOT force humans to manually maintain Quark-level project state.

## 9. Dependency graph

Hard dependency edges MUST form a directed acyclic graph.

For every hard dependency edge:

```text
A depends_on B
```

`A` MUST NOT become logically Enabled until `B` is Satisfied, unless the dependency explicitly defines another admissible condition.

Cycles MUST be rejected at validation time.

Feedback, iteration, repair, and retry MUST be represented through PlanRevisions, ExecutionAttempts, events, or explicitly modeled control constructs rather than cycles in the hard dependency graph.

## 10. Declared semantic access

An Atom MAY declare semantic read and write claims.

Examples:

```text
read: rust-crate://core/model
read: type://User
write: api://AuthMiddleware
write: schema://user.identity
provide: interface://AuthenticatedPrincipal
require: interface://UserIdentity
```

These claims are scheduling and coordination inputs, not proofs.

Observed access SHOULD be compared against declared access.

A newly observed write outside declared scope SHOULD produce a scope-expansion event and SHOULD trigger conflict re-evaluation before candidate admission.

## 11. Parallel-admission heuristic

For candidate transactions `i` and `j`, define declared reads `R` and writes `W`.

They MAY be admitted concurrently only when:

```text
no hard dependency orders i and j
```

and the scheduler predicts no conflict, approximately:

```text
W_i ∩ (R_j ∪ W_j) = ∅
W_j ∩ (R_i ∪ W_i) = ∅
```

This rule is a heuristic. The scheduler MUST still perform integration verification because semantic conflicts may evade declarations and observation.

## 12. ExecutionAttempt

An ExecutionAttempt is one concrete attempt to execute an Atom or Quark.

An ExecutionAttempt MUST record:

```text
id
subject_work_id
subject_spec_revision
actor
base_state
start_time
outcome
```

When the attempt involves source mutation, it SHOULD additionally record:

```text
jj_workspace
jj_change_id
candidate_commit_id
```

An attempt outcome MAY be:

```text
running
candidate_produced
failed
timed_out
abandoned
cancelled
```

Attempt outcome MUST NOT directly mutate the logical specification of its Atom.

## 13. Effect classes

Every executable Atom or Quark SHOULD declare an effect class from:

```text
pure
hermetic
external_read
idempotent_write
compensatable_write
irreversible
judgment
```

Automatic retries MUST be policy-sensitive to effect class.

`irreversible` operations MUST require explicit authorization before dispatch.

Nondeterministic operations MUST record results as events/evidence rather than being silently reinvoked during replay.

## 14. Artifact

An Artifact is an immutable or identity-addressed Entity consumed or produced by execution.

Artifacts SHOULD use content digests whenever practical.

Examples include:

```text
source commit
compiled binary
container image
schema document
test report
benchmark output
release bundle
model response
```

## 15. Evidence

Evidence is an observation relevant to evaluating an acceptance predicate.

Evidence MUST identify:

```text
subject
producer_attempt or external_source
evidence_type
result
timestamp
```

Verification evidence SHOULD additionally bind:

```text
spec_revision
candidate_commit_id
input_digest
environment_digest
verifier_id
verifier_version
```

Evidence MUST NOT satisfy a current acceptance predicate if its compatibility/freshness rules fail.

## 16. Evidence fingerprint

An implementation SHOULD derive a verification fingerprint from relevant immutable inputs.

Conceptually:

```text
fingerprint = H(
  spec_revision
  || exact_candidate
  || resolved_inputs
  || resolved_dependencies
  || relevant_environment
  || verifier_definition
)
```

A verification result MUST NOT be reused when a relevant fingerprint component changes unless the verifier contract explicitly proves that component irrelevant.

## 17. Attestation

An Attestation is an authenticated claim about an activity, artifact, or evidence record.

It SHOULD support concepts compatible with software-provenance systems such as:

```text
subject
predicate_type
actor
activity
materials
products
byproducts
environment
signature/identity
```

Attestation does not imply correctness by itself. It establishes who or what made a claim and what exact objects the claim concerns.

## 18. Provenance compatibility

Gordian SHOULD permit projection into W3C PROV concepts:

```text
Artifact / SpecRevision / Evidence -> Entity
ExecutionAttempt                  -> Activity
Human / autonomous worker        -> Agent
```

Relevant relations SHOULD map where possible to:

```text
used
wasGeneratedBy
wasDerivedFrom
wasAssociatedWith
wasAttributedTo
wasInformedBy
```

Gordian is not required to use RDF or PROV as its internal storage model.

## 19. Derived state predicates

### 19.1 Satisfied

```text
Satisfied(x) := acceptance(x) evaluates true against current compatible evidence
```

### 19.2 Blocked

```text
Blocked(a) := exists d in hard_dependencies(a) such that not Satisfied(d)
```

### 19.3 Enabled

```text
Enabled(a) :=
  ValidSpec(a)
  and not Blocked(a)
  and PreconditionsHold(a)
```

### 19.4 Dispatchable

```text
Dispatchable(a) :=
  Enabled(a)
  and CompatibleExecutorAvailable(a)
  and RequiredResourcesAvailable(a)
  and AuthorizationValid(a)
  and LeaseCompatible(a)
```

### 19.5 Active

```text
Active(a) := exists attempt r where r.subject = a and r.outcome = running
```

These predicates MAY be materialized for query performance but SHOULD remain reproducible from canonical facts.

## 20. Jujutsu binding

When an Atom mutates code:

- the worker MUST receive an exact base commit;
- the worker SHOULD receive an isolated Jujutsu workspace;
- a logical implementation SHOULD be associated with a Jujutsu change ID;
- a candidate handoff MUST identify an exact Jujutsu commit ID;
- verification MUST bind to the exact commit ID rather than only the change ID.

Rewriting the change and therefore changing its commit ID MUST invalidate commit-bound verification evidence.

## 21. Candidate freeze

When a worker declares a candidate ready for verification, that candidate commit MUST be treated as immutable for that verification attempt.

Further edits MUST produce a new candidate identity and require re-verification according to evidence freshness rules.

## 22. Integration

Independent sibling candidates MAY be composed into a distinct integration candidate.

The integration candidate MUST receive its own verification.

Success of all component candidates MUST NOT imply success of the integrated candidate.

Unresolved Jujutsu conflicts MAY be represented as intermediate integration state, but a conflicted state MUST NOT be admitted to the accepted frontier.

## 23. Accepted frontier

The canonical accepted source frontier is represented externally by Gordian as an authority-controlled state and may be projected through Jujutsu `trunk()` / `main` conventions.

A worker MUST NOT have default authority to move the accepted frontier.

## 24. Acceptance invariant

For a candidate `c`, admission MUST require at least:

```text
CurrentFrontierReconciled(c)
NoUnresolvedConflict(c)
RequiredVerificationPasses(c)
EvidenceBoundToExactCandidate(c)
EvidenceFresh(c)
CoordinatorAuthorized(c)
```

Formally, the implementation target is:

```text
Accept(c) =>
  CurrentFrontierReconciled(c)
  and NoUnresolvedConflict(c)
  and Verified(c)
  and FreshEvidence(c)
  and AuthorizedPromotion(c)
```

## 25. Replay invariant

Given the same canonical ordered event history and the same deterministic projection implementation, Gordian SHOULD derive the same Mission Graph execution projection.

Effectful or nondeterministic operations MUST NOT be implicitly reproduced during state replay.

## 26. Initial formal-method targets

The following invariants are candidates for Lean formalization:

1. hard dependency acyclicity;
2. dispatch implies dependency satisfaction;
3. stale evidence cannot establish satisfaction;
4. worker capability cannot authorize accepted-frontier mutation;
5. candidate rewrite invalidates commit-bound evidence;
6. accepted states contain no unresolved structural conflict;
7. deterministic event projection is replay-stable under its stated assumptions;
8. cross-Atom hard dependencies cannot target Quarks.

These proofs establish properties of the Gordian model and implementation semantics under assumptions. They do not prove that a real software implementation satisfies a Mission's real-world objective.

## 27. Falsification targets

Mission Graph v0 MUST be considered provisional until experiments test at least:

- whether semantic read/write claims predict integration conflict better than file-level claims;
- whether Atom granularity produces useful parallelism without excessive decomposition overhead;
- whether derived state reduces stale-status divergence;
- whether evidence invalidation catches realistic stale-verification failures;
- whether Jujutsu workspace/change semantics reduce orchestration complexity compared with Git worktrees/branches;
- whether shared semantic coordination improves completion and integration quality over isolated execution alone;
- whether formalized invariants catch defects that ordinary property/model tests do not.
