# Source Adapter Contract

Status: **normative research specification**

Gordian's canonical semantics do not name a version control system. This document defines the
`SourceAdapter` trait that every source plane implementation MUST satisfy, in adapter-neutral
terms, so that #34 can vary the substrate while holding the Mission Graph, scheduler, workers,
verification, and workloads constant.

Two implementations are in scope for the comparison:

- the Jujutsu adapter (#29), realized as [`jujutsu-agent-protocol.md`](jujutsu-agent-protocol.md);
- the Git worktree adapter (#76, per D4), which exists behind this same trait.

Nothing above this trait may reference a Jujutsu command, revset, or identity format.

## 1. Identities

```text
LogicalChangeId    evolving implementation identity; survives content rewrite
ExactStateId       one exact immutable source state
WorkspaceId        an isolated mutable execution container
```

Every backend MUST document, for each identity:

| Question | Requirement |
| --- | --- |
| Is `LogicalChangeId` stable across a content rewrite? | If natively yes (Jujutsu change ID), say so. If natively no (Git), the adapter MUST synthesize a stable id, persist the mapping durably, and document where. |
| Is `ExactStateId` unique per exact state? | MUST be. Two different tree/parent states MUST NOT share an id. |
| What happens when the backend cannot supply an identity natively? | The adapter MUST synthesize it, MUST record the synthesis as an adapter-scoped provenance fact, and MUST NOT silently reuse a backend id with different semantics. |

## 2. Operations

```text
snapshot(base: ExactStateId) -> Result<WorkspaceId, AdapterError>
new_change(base: ExactStateId) -> Result<LogicalChangeId, AdapterError>
freeze(change: LogicalChangeId) -> Result<ExactStateId, AdapterError>
integrate(parents: [ExactStateId]) -> Result<Integrated, AdapterError>
conflicts(state: ExactStateId) -> Result<Set<ConflictRegion>, AdapterError>
stage(state: ExactStateId, ref_name: StagingRef) -> Result<(), AdapterError>
verify(state: ExactStateId, manifest: VerifierManifest) -> Result<[Evidence], AdapterError>
move_frontier(expected: ExactStateId, new: ExactStateId) -> Result<MoveOutcome, AdapterError>
publish_frontier(expected: ExactStateId, new: ExactStateId) -> Result<MoveOutcome, AdapterError>
reset_frontier(to: ExactStateId, scope: FrontierScope) -> Result<ResetOutcome, AdapterError>
local_frontier() -> Result<ExactStateId, AdapterError>
published_frontier() -> Result<ExactStateId, AdapterError>
```

```text
enum Integrated    { State(ExactStateId), Conflict(Set<ConflictRegion>) }
enum MoveOutcome   { Committed, AlreadyAtNew, Conflict { observed: ExactStateId },
                     MovedLocallyNotPublished { local: ExactStateId, published: ExactStateId } }
enum ResetOutcome  { Reset, AlreadyAt, Refused { reason: String } }
enum FrontierScope { Local, Published, Both }
```

Twelve operations, not seven. `publish_frontier`, `reset_frontier`, `stage`, and the two frontier
readers exist because the admission protocol needs them and the trait was the only place they
could live: without `publish_frontier` the push is outside the adapter and therefore outside crash
recovery, without `reset_frontier` an `AdmissionAborted` cannot undo a completed local move,
without `stage` the integration state cannot be verified before it is admitted, and without the
two readers `frontier_projections_agree()` has nothing to read.

### Per-operation guarantees

| Operation | Guarantee every backend MUST document |
| --- | --- |
| `snapshot` | The workspace contains exactly `base` and nothing else; no ambient working-copy state leaks in. Returns a distinct `WorkspaceId` per call. |
| `new_change` | Creates a fresh `LogicalChangeId` whose only parent is `base`. Two calls with the same `base` return two distinct ids. |
| `freeze` | Returns the `ExactStateId` of the change's current content. Calling `freeze` twice without an intervening mutation returns the same id; calling it after a mutation returns a different one. |
| `integrate` | Deterministic in `parents` up to the declared ordering rule. Returns `Conflict` rather than a partially merged state; an implementation that cannot represent an unresolved conflict returns `Conflict` with the regions. |
| `conflicts` | Total on any `ExactStateId` the adapter produced. An empty result means the backend represents no unresolved structural conflict; it is not a claim of semantic correctness. |
| `verify` | Executes against the exact state without mutating it. MUST record the exact state, environment digest, verifier identity and configuration, exit behaviour, and outputs. MUST NOT read the ambient working copy. |
| `stage` | Makes `state` fetchable by an external verifier (a CI system, a remote runner) under a **non-frontier** ref, without moving any frontier and without implying admission. Idempotent per `(state, ref_name)`. A staged ref is garbage-collectable and MUST NOT be an ancestor requirement for anything. |
| `move_frontier` | **Idempotent** over the *local* frontier. `move_frontier(e, n)` when it is already `n` returns `AlreadyAtNew`, not an error. This is what lets the coordinator re-drive an incomplete `CandidateAdmitted` after a crash. Never the compare-and-swap target — the CAS is on `FrontierVersion` in the canonical event log. |
| `publish_frontier` | **Idempotent** over the *published* frontier, with the same contract. It is a distinct operation so that crash recovery re-drives the push as well as the local move; a push performed by an ad hoc shell command outside the adapter is invisible to recovery, and a coordinator that crashes between the two leaves the published frontier permanently behind the log. Returns `MovedLocallyNotPublished` when the local frontier is at `new` and the published one is not, which is the state the divergence check names. |
| `reset_frontier` | Moves a frontier **backwards** to `to`, for `Local`, `Published`, or `Both`. Idempotent: a reset to the current value returns `AlreadyAt`. It exists because `move_frontier(expected, new)` cannot express a rollback once `expected` no longer matches, and `AdmissionAborted` requires a compensating move — without it one permanent publish failure wedges admission forever. A backend that cannot rewind a published ref returns `Refused`, which the coordinator surfaces as a `permanent` error rather than retrying. |
| `local_frontier`, `published_frontier` | Total reads with no side effects, used by `frontier_projections_agree()` before every admission and by the divergence timer. They MUST distinguish the two refs by name in every error and event. |

## 3. Errors and evidence

`AdapterError` is a closed enum. Every variant MUST be mappable to a Gordian event
(`ConflictObserved`, `AttemptFailed`, `FrontierDivergenceObserved`, ...), and MUST be classified
`transient` or `permanent`, because
[`../algorithms/evidence-and-admission.md#crash-recovery`](../algorithms/evidence-and-admission.md#crash-recovery)
distinguishes a re-drivable `move_frontier` failure from one that must be closed with
`AdmissionAborted`. An adapter MUST NOT surface backend stderr as an opaque string in place of a
variant.

The adapter owns structured argv/cwd/env, supported-version and feature checks, bounded
machine-readable parsing, exact identities, errors and evidence artifacts, and disposable
fixtures. No shell command may be issued from outside the adapter.

## 4. Experiment obligation

#34 compares adapters through this trait and nothing else varies. Its manifest is pre-registered
per [`../testing/statistical-contract.md`](../testing/statistical-contract.md), class
**agent-trial**.
