# Reconciliation, Replay, and Repair

Gordian is intended to behave as a closed-loop engineering control system.

The central distinction is between:

```text
Planned world   W_p
Observed world  W_o
```

The Mission Graph describes `W_p`: what should become true and what evidence would justify saying it is true.

Execution history, artifacts, and observations describe `W_o`: what actually happened and what is currently known.

The controller operates over the unresolved delta.

## 1. Canonical event history

Mutable projections are convenient for query performance, but the canonical execution history should be append-oriented.

The canonical event set is:

```text
MissionCreated
PlanPublished
PlanSelected
PlanSuperseded
AtomDeclared
DependencyDeclared
ProviderBindingDeclared
ObservationRecorded
AttemptStarted
ResourceReserved
ResourceReleased
LeaseGranted
LeaseRevoked
LeaseExpired
CapabilityExpired
OperationCommutativityDeclared
ScopeExpanded
CandidateFrozen
VerificationStarted
EvidenceRecorded
EvidenceRetracted
EvidenceInherited
VerifierNondeterminismObserved
IntegrationCreated
CandidateClaimed
CandidateClaimReleased
IntegrationConflictObserved
ConflictObserved
AdmissionPreempted
AdmissionRejected
CandidateAdmitted
FrontierMoved
AtomSatisfied
SatisfactionInvalidated
SatisfactionRestored
AdmissionAborted
FrontierDivergenceObserved
DeploymentObserved
AttemptFailed
AttemptAbandoned
AttemptCancelled
CapabilityRevoked
IrreversibleRetryAuthorized
CompensationApplied
```

Every event MUST have an immutable identity, a dense `EventSeq` assigned on append, and enough
causality/provenance metadata to interpret it. Appends are **transactional over a list of
events**: a list is applied all-or-nothing and receives consecutive `EventSeq` values
([`../spec/data-model.md` `## The frontier stream and log atomicity`](../spec/data-model.md#the-frontier-stream-and-log-atomicity)).

`CandidateAdmitted`, `FrontierMoved`, `AdmissionAborted`, and `AdmissionRejected` form the
**frontier stream**, whose newest `EventSeq` is the `FrontierVersion` that admission
compare-and-swaps on. `CandidateAdmitted` is the admission **intent** event and carries the
expected `FrontierVersion`, the `WitnessGuard`, and the recorded witness; `FrontierMoved` is the
**completion** event; `AtomSatisfied` and `SatisfactionRestored` are the only events that may
create a `SatisfactionRecord`, and their application is idempotent per `(atom, frontier_seq)`;
`SatisfactionInvalidated` is the only event that may remove one, and every one of its five reasons
has a named producer and trigger
([`../spec/mission-graph.md` `### Satisfaction`](../spec/mission-graph.md#satisfaction));
`AdmissionAborted` is the only event that may cancel an incomplete intent, and MUST be preceded by
a compensating `reset_frontier`; `AdmissionRejected` records a false witness conjunct, so a
rejected candidate leaves a trace instead of disappearing; `CandidateClaimed` /
`CandidateClaimReleased` carry the exclusive admission-queue claim that stops one candidate
entering two batches; `LeaseExpired` and `CapabilityExpired` make expiry a fact of canonical
history rather than a wall-clock comparison, which is what keeps readiness replay-pure;
`FrontierDivergenceObserved` records that a **named** projection of the frontier — `local_bookmark`
or `published_bookmark` — disagreed with the log, and is appended whenever the divergence check
runs, not only at startup. See
[`evidence-and-admission.md` `### The algorithm`](evidence-and-admission.md#the-algorithm).

## 2. Projection

Let:

```text
H = [e1, e2, ... en]
```

be a canonical ordered event history and:

```text
P : EventHistory -> ProjectionState
```

be a deterministic projector.

Then:

```text
State = P(H)
```

The current Lean theorem proves only the basic functional property:

```text
H1 = H2 -> P(H1) = P(H2)
```

The difficult engineering problem is making `P` deterministic and making `H` a sufficiently complete, well-ordered record.

## 3. Deterministic core, effectful boundary

A projector MUST NOT perform hidden effects such as:

- calling an LLM;
- reading the current wall clock as business input;
- fetching a mutable URL;
- querying a live database whose result is not in the event;
- invoking a nondeterministic service.

If an external observation matters, record the result as an event first.

Conceptually:

```text
external read
   -> ObservationRecorded(value, source, identity, time)
   -> deterministic projection consumes recorded value
```

This follows the durable-execution principle used by systems such as Temporal: replayable orchestration is separated from effectful activities, whose outcomes become durable history.

## 4. Derived execution state

Projection derives convenience state such as:

```text
Blocked(atom)
Ready(atom)          -- Enabled and Dispatchable
Running(atom)        -- Active
Verifying(atom)
Satisfied(atom)      -- an unsuperseded SatisfactionRecord exists
StaleEvidence(atom)
ConflictingEvidence(atom)
Conflict(atom)
```

`Satisfied(atom)` is a lookup into the `satisfaction_index` projection, which the projector writes
only while applying an `AtomSatisfied` event. `ConflictingEvidence(atom)` is derived from a fresh
pass and a fresh fail coexisting for one `(verifier, fingerprint)` pair, and maps to the
`NeedVerification` reconciliation class.

These labels MUST be reproducible from canonical facts. A UI may cache them; cache mutation must
not become the authoritative source of truth.

## 5. Reconciliation function

At a high level:

```text
reconcile(MissionGraph, ProjectionState) -> ReconciliationResult
```

Possible result classes:

```text
Satisfied
NeedExecution(ready_atoms)
NeedVerification(subjects)
NeedRepair(failures)
NeedReplan(reason)
Blocked(reason)
AwaitExternalObservation(condition)
```

This makes “what next?” an explicit derivation rather than a person or agent scanning a board and guessing.

## 6. Desired/observed delta

For each acceptance predicate `p`:

```text
Desired(p) = required
Observed(p) = evidence state
```

A simple delta is:

```text
Delta(p) = Desired(p) - JustifiedObserved(p)
```

The implementation should avoid pretending arbitrary goals form a numeric vector. “Delta” is conceptual unless a domain defines a real metric.

For Boolean predicates:

```text
missing
passing
failing
stale
unknown
```

is sufficient.

## 7. Repair versus replanning

A failed attempt should not automatically rewrite the Mission.

Classify failure:

### Execution-local failure

Example: compilation error within the selected implementation strategy.

Response:

```text
repair / retry Atom
```

### Integration failure

Example: two individually verified candidates violate a composed interface assumption.

Response:

```text
create repair Atom or revise affected Initiative plan
```

### Invalid decomposition

Example: required capability cannot be produced by the current set of Initiatives/Atoms.

Response:

```text
new PlanRevision
```

### Mission contradiction/infeasibility

Example: constraints are mutually impossible under established facts.

Response:

```text
surface Mission-level contradiction
```

Do not silently weaken the Mission to make the plan succeed.

## 8. Retry semantics depend on effects

Replay and retry are different operations.

```text
Replay = recompute projection from recorded history.
Retry  = execute a new effectful attempt.
```

Retry policy is **total over `EffectClass`**. The row labels below are exactly the seven variants
of `enum EffectClass` in
[`../spec/data-model.md` `## Rust representation`](../spec/data-model.md#rust-representation),
snake-cased. `scripts/check-effect-classes.sh` extracts the enum variants, these row labels, and
the arms of `retryPolicy` in `formal/Gordian/EffectClass.lean`, and asserts set equality with
cardinality 7.

| Effect class | Definition | Automatic retry | Rule |
| --- | --- | --- | --- |
| `pure` | No I/O; output is a function of its declared arguments alone. | permitted, unbounded within the attempt budget | Retry is free. A differing result on retry is a defect in the implementation's purity claim and MUST emit `ScopeExpanded`. |
| `hermetic` | Effects confined to declared inputs/outputs; reproducible from the declared input set. | permitted, unbounded within the attempt budget | Retry MUST recreate the declared input set. A differing result indicates an undeclared input and MUST emit `ScopeExpanded`. |
| `external_read` | Observes state Gordian does not own. | permitted | Each retry observes a later world and therefore produces **new evidence** with a new `ObservationRecorded`; it MUST NOT overwrite or reuse the prior observation. |
| `idempotent_write` | Write whose repetition is defined to be equivalent to one application. | permitted only under a declared idempotency key | The attempt MUST carry an idempotency key recorded before the effect. Retry without a key is forbidden. |
| `compensatable_write` | Write with a defined inverse. | permitted only after compensation | Retry MUST first run the declared compensation for the ambiguous attempt and record `CompensationApplied`; a retry without a completed compensation is forbidden. |
| `irreversible` | No inverse and no idempotency contract. | **forbidden** | After ambiguous completion Gordian MUST NOT auto-retry. It requires an explicit action by an actor holding `perform_irreversible_effect`, recorded as an `IrreversibleRetryAuthorized` attestation naming the ambiguous attempt and the recovery rationale. |
| `judgment` | A defeasible assessment by a human or model evaluator. | permitted | Retry produces a **new** judgment artifact with its own identity and provenance. It MUST NOT overwrite, silently supersede, or average with the prior judgment. Both judgments remain addressable, and a disagreement surfaces as `NeedVerification`, not resolved by recency. |

`pure` and `hermetic` are separate rows because their failure diagnoses differ: an unstable `pure`
result means the code is not pure, while an unstable `hermetic` result means an input was not
declared. Collapsing them loses the distinction that makes the retry observation useful.
`judgment` previously had no rule at all.

The Rust realization is:

```rust
fn retry_policy(class: EffectClass) -> RetryRule {
    match class {
        EffectClass::Pure => RetryRule::Free,
        EffectClass::Hermetic => RetryRule::FreeRecreatingInputs,
        EffectClass::ExternalRead => RetryRule::NewObservation,
        EffectClass::IdempotentWrite => RetryRule::RequiresIdempotencyKey,
        EffectClass::CompensatableWrite => RetryRule::RequiresCompensation,
        EffectClass::Irreversible => RetryRule::ManualOnly,
        EffectClass::Judgment => RetryRule::NewJudgmentArtifact,
    }
}
```

No `_` wildcard. Adding a class MUST be a compile error.

## 9. Distributed event ingestion

If Gordian becomes distributed, event history raises standard distributed-systems questions:

- duplicate delivery;
- reordering;
- actor clock skew;
- split-brain coordinators;
- lost acknowledgements;
- retry after uncertain commit;
- stale lease holders.

The initial design should prefer:

- stable event IDs and idempotent ingestion;
- causality references rather than wall-clock ordering alone;
- single-writer or consensus-backed accepted-frontier mutation;
- compare-and-swap/version checks;
- fencing tokens for exclusive lease-protected resources.

Exactly-once network delivery should not be assumed.

## 10. Rebuild test

A critical implementation invariant is:

```text
materialized_state_before_restart
  ==
project(all_canonical_events_after_restart)
```

This should be tested continuously.

Procedure:

1. execute generated state-machine traces;
2. persist events and materialized projection;
3. discard the projection;
4. rebuild from the event log;
5. compare canonical fields;
6. fail on divergence.

This is stronger than unit testing individual event handlers because it verifies replay composition.

## 11. Observability

The controller itself needs observability.

Useful signals include:

```text
ready_atom_count
blocked_atom_count
stale_evidence_count
scope_expansion_rate
candidate_reverification_rate
integration_conflict_rate
lease_contention
attempt_abandonment_rate
replan_rate
projection_rebuild_mismatch
accepted_frontier_age
```

These metrics are measurements, not objectives to optimize blindly. Goodhart pressure is especially dangerous around “tasks completed,” “agent utilization,” or “parallelism.”

The actual objective remains Mission satisfaction under evidence and constraints.

## 12. Formal roadmap

The current replay theorem is intentionally minimal.

A stronger formal model should eventually define:

```text
State
Event
ValidTransition : State -> Event -> State -> Prop
Invariant : State -> Prop
```

and prove:

```text
Invariant(s)
and ValidTransition(s, e, s')
-> Invariant(s')
```

This transition-preservation theorem family is the appropriate place to prove properties such as:

- no unauthorized accepted-frontier transition;
- no stale evidence transition to satisfaction;
- no dispatch transition with unsatisfied hard dependencies;
- no simultaneous incompatible exclusive leases.

That is considerably stronger than proving isolated predicate lemmas, and it is the next major
formal-method milestone once the reference kernel compiles cleanly.

The model itself lives at `formal/Gordian/Transition.lean` and is owned by a dedicated Atom, whose
prerequisites are the concrete `Event` type (#12) and the derived-state predicates (#13). Gap
`G-236` tracks the fact that this milestone is stated here and in
[`../formal/theorem-catalog.md`](../formal/theorem-catalog.md) (T012, T013) without an owning Atom;
it is closed by creating that Atom, not by this document.
