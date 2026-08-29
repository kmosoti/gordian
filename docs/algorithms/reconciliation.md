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

Representative events:

```text
MissionCreated
PlanPublished
AtomDeclared
DependencyDeclared
AttemptStarted
LeaseGranted
ScopeExpanded
CandidateFrozen
VerificationStarted
EvidenceRecorded
IntegrationCreated
ConflictObserved
CandidateAdmitted
DeploymentObserved
AttemptFailed
AttemptAbandoned
PlanSuperseded
```

Every event should have an immutable identity and enough causality/provenance metadata to interpret it.

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
Ready(atom)
Running(atom)
Verifying(atom)
Satisfied(atom)
StaleEvidence(atom)
Conflict(atom)
```

These labels should be reproducible from canonical facts.

A UI may cache/materialize them, but cache mutation must not become the authoritative source of truth.

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

For `pure` or `hermetic` work, retry may be routine.

For `external_read`, a retry observes a later world and therefore creates new evidence.

For `idempotent_write`, retry is permitted only under the declared idempotency contract.

For `compensatable_write`, retry may require a compensation protocol.

For `irreversible`, Gordian should require explicit high-authority action and should not auto-retry after ambiguous failure.

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

That is considerably stronger than proving isolated predicate lemmas, and it is the next major formal-method milestone after v0 compiles cleanly.
