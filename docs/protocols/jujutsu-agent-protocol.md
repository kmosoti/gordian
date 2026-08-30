# Jujutsu Agent Execution Protocol

Status: **experimental protocol**

This document maps Gordian Mission Graph semantics onto Jujutsu execution primitives. It is
explicitly **one adapter's realization** of the adapter-neutral contract in
[`source-adapter-contract.md`](source-adapter-contract.md); the Jujutsu adapter is issue #29.

## 1. Principle

The Mission Graph and Jujutsu DAG are intentionally different graphs.

```text
Mission Graph
  what must become true
  what depends on what semantically
  what may run concurrently
  what constitutes satisfaction

Jujutsu Change Graph
  what evolving and exact code states exist
  how code states causally derive from one another
```

Gordian coordinates the mapping between them.

Gordian's canonical vocabulary is `logical_change_id` and `exact_state_id`. This document maps
those onto Jujutsu change IDs and commit IDs. Any statement here that constrains Gordian rather
than Jujutsu belongs in the adapter contract or in the specification, and is cited here, not
restated.

## 2. Canonical vocabulary

| Logical concept | Jujutsu representation |
| --- | --- |
| accepted source frontier | `trunk()` |
| public accepted name | `main` |
| exact accepted source state | commit ID |
| evolving implementation | change ID |
| isolated worker environment | workspace |
| independent implementation | sibling changes |
| causal implementation dependency | parent/child changes |
| integrated candidate | multi-parent change |
| revision-scoped command execution | `jj run` |
| repository operation history | operation log |
| external remote identity | bookmark |
| immutable release identity | tag |
| production truth | deployment record |

There is no permanent `develop` bookmark in the protocol.

## 3. Identity rule

Jujutsu change IDs and commit IDs have distinct semantics.

A change ID identifies an evolving logical code change and generally survives rewriting.

A commit ID identifies one exact immutable version of that change.

In Gordian's adapter-neutral vocabulary: `logical_change_id` = change ID, `exact_state_id` =
commit ID. A Git adapter synthesizes the change identity and uses the commit SHA.

Therefore:

> **Workers operate on change identities. Verification applies to commit identities.**

Example:

```text
Atom: AUTH-42
Change: qpvuntsm
Candidate commit: abc123
Verification: pass on abc123
```

If the worker rewrites the change:

```text
Atom: AUTH-42
Change: qpvuntsm
Candidate commit: def456
```

then verification of `abc123` is stale for `def456`.

## 4. Worker isolation

Each concurrently executing worker SHOULD have a separate Jujutsu workspace.

The Jujutsu adapter realizes `snapshot(base)` as:

```bash
jj workspace add ../worker-auth \
  --name worker-auth \
  -r <exact-base-commit>
```

Another independent worker receives another workspace from its own admitted base.

A workspace is an execution container for mutable source state, not a project-management identity.

## 5. Snapshot rule

A worker MUST be admitted against an exact base state, and that base MUST satisfy
**PrerequisiteContaining**: it is an admitted frontier state `F` such that every hard dependency
of the Atom is Satisfied at a frontier `F' <= F`. See
[`../spec/mission-graph.md#stable-snapshots`](../spec/mission-graph.md#stable-snapshots). A base
chosen because it is merely "the current `trunk()`" is not sufficient on its own; the containment
check is a separate assertion the adapter MUST make at workspace creation.

Do not continuously rebase active workers onto a moving accepted frontier.

The preferred lifecycle is optimistic:

```text
admitted frontier F0  (frontier_seq = n, contains every prerequisite's admitted state)
       |
       +--> worker snapshot at F0
               |
               +--> mutable change (logical_change_id, write_exclusive lease, fencing_token)
                       |
                       +--> exact candidate C (exact_state_id, fencing_token recorded)

meanwhile the frontier may advance to F1 (frontier_seq = n + k)

C enters the admission queue
  -> integration batch B over F1
  -> I = integrate([F1] ++ B)
  -> integration verification of I
  -> admission (CandidateAdmitted / FrontierMoved) or return to reconciliation
```

The reconciliation step operates on a **batch**, not on `C` alone; per-candidate reconciliation
makes every admission invalidate every in-flight candidate's evidence. See
[`../algorithms/evidence-and-admission.md#batch-assembly`](../algorithms/evidence-and-admission.md#batch-assembly).

This gives the worker a stable reasoning context while moving-world reconciliation occurs at a
controlled boundary.

## 6. One writer per change

The protocol MUST maintain one live `write_exclusive` `Lease` whose subject is
`LeaseSubject::LogicalChange(x)` for each logical change `x` under normal-path execution. The
lease subject is the sum type of
[`../spec/data-model.md` `## Lease`](../spec/data-model.md#lease); it is not a bare Jujutsu change
ID and not a `SemanticResource`.

`LeaseSubject::LogicalChange` MUST NOT be granted in `write_shared_if_commutative` mode.

Jujutsu can represent divergent versions of the same change, but deliberate concurrent rewriting
of one logical change is a poor default coordination strategy.

For speculative execution, create separate changes from the same base:

```text
Atom AUTH-42
  candidate/change A   (its own LogicalChangeId, its own lease, its own fencing_token)
  candidate/change B
```

A verifier or synthesizer can then choose, combine, or reject the candidates.

## 7. Semantic coordination record

Workspace isolation MUST be paired with shared coordination state.

Each running attempt SHOULD publish:

```yaml
atom: AUTH-42
attempt: run-882
worker: agent-A7
base_exact_state_id: <exact-base>
logical_change_id: <logical-change>
lease_expires_at: <EventSeq>
claims:
  read:
    - model.User
    - config.Security
  write:
    - auth.token_validation
    - api.AuthMiddleware
  provide:
    - AuthenticatedPrincipal
  require:
    - UserIdentity
```

Workers SHOULD be able to observe coordination events such as:

```text
attempt_started
claim_expanded
interface_published
dependency_satisfied
candidate_ready
candidate_rewritten
verification_failed
integration_conflict
attempt_abandoned
```

## 8. Admission scheduler

Before dispatch, the scheduler evaluates:

1. hard Mission Graph dependencies;
2. declared semantic read/write overlap;
3. required/provided interfaces;
4. executor capabilities;
5. resource requirements;
6. lease compatibility;
7. effect/authority policy.

Parallelism follows the work graph rather than an arbitrary desired number of agents.

## 9. Worker lifecycle

### 9.1 Plan

A Mission or Initiative is decomposed into Atoms with explicit contracts, dependencies, semantic claims, and verifier manifests.

### 9.2 Admit

The scheduler selects logically Enabled and operationally Dispatchable Atoms whose predicted interactions are acceptable.

### 9.3 Snapshot

Each admitted Atom receives an exact base commit.

### 9.4 Spawn

Gordian creates an isolated workspace and a logical Jujutsu change for the worker.

### 9.5 Execute

The worker mutates only its authorized workspace/change and emits coordination events.

### 9.6 Observe

Gordian records files, symbols, interfaces, external resources, and other access observable from the execution substrate. Observed scope is compared with declared scope.

### 9.7 Candidate

The worker hands off an exact state identity together with the fencing token it held.

```yaml
candidate_handoff:
  atom: AUTH-42
  attempt: run-882
  logical_change_id: <logical-change>
  exact_state_id: <exact-state>
  base_exact_state_id: <exact-base>
  base_frontier_seq: <n>
  fencing_token: <token held on LeaseSubject::LogicalChange at freeze>
  frozen_at_event: <EventSeq of CandidateFrozen>
```

`fencing_token` is required. Jujutsu cannot reject a write from a stale actor, so the token is
carried forward and checked at admission by `LeaseValidAtFreeze`
([`../algorithms/evidence-and-admission.md#the-admission-conjuncts-defined`](../algorithms/evidence-and-admission.md#the-admission-conjuncts-defined)).
Without it, a paused worker whose lease expired or was superseded can hand off a commit and have
it admitted.

The candidate becomes logically frozen for the corresponding verification attempt.

### 9.8 Verify candidate

Task-local verifiers run against the exact candidate, never against a workspace's ambient working
copy.

Verification evidence carries the seven `EvidenceBinding` fields of
[`../algorithms/evidence-and-admission.md#the-evidence-subject`](../algorithms/evidence-and-admission.md#the-evidence-subject),
under exactly those names, plus the verifier outcome:

```yaml
atom: AUTH-42
logical_change_id: <logical-change>
binding:
  canonicalization_scheme: gordian-canon-v1
  spec_revision: <digest>
  exact_state_id: <exact-candidate>
  input_digest: <digest>
  dependency_digest: <digest>
  environment_digest: <digest>
  verifier_digest: <digest>
producer_attempt: run-882
verifier_id: <id>
verifier_version: <version>
result: pass|fail
```

`dependency_digest` and `canonicalization_scheme` are required, not optional: `Fresh(e, s, v)`
compares all seven fields, so evidence that omits either cannot be shown fresh. `exact_state_id`
is the exact candidate state; the surviving `logical_change_id` is provenance, never a
verification subject.

### 9.9 Integrate

Candidates that must coexist are composed into an explicit `IntegrationCandidate` over the current
accepted frontier plus the batch members
([`../algorithms/evidence-and-admission.md#batch-assembly`](../algorithms/evidence-and-admission.md#batch-assembly)).
Admission's subject is always an `IntegrationCandidate`, even for a one-member batch.

Independent siblings SHOULD remain siblings until an integration state is required. The DAG
encodes causal dependency, not finishing order.

### 9.10 Verify integration

The integrated state receives cross-component verification.

Component verification is necessary but not sufficient for integration acceptance.

### 9.11 Accept

Only an actor with the `move_accepted_frontier` capability **and** a live exclusive lease on
`LeaseSubject::Coordinator(project)` may promote. Promotion is the four-step event-log protocol of
[`../algorithms/evidence-and-admission.md#the-algorithm`](../algorithms/evidence-and-admission.md#the-algorithm):
append `CandidateAdmitted` under a `FrontierVersion` precondition and a `WitnessGuard`,
idempotently move the local bookmark, idempotently publish it, then append `FrontierMoved` and the
per-Atom `AtomSatisfied` events in one conditional transactional append. The bookmarks are
projections of the log and are never the compare-and-swap target. The executable sequence is
[`landing.md`](landing.md).

### 9.12 Release

Release is a separate authority transition. An immutable tag/artifact identifies a release. Deployment records identify what is actually running.

Accepted code and deployed code are related but distinct truths.

## 10. Integration as explicit state

Suppose independent Atoms produce changes `A`, `B`, and `C`.

The protocol SHOULD preserve them as sibling changes if no causal relation exists.

When they must be evaluated together, the coordinator creates a multi-parent integration state
over the current frontier and the batch. The Jujutsu adapter realizes `integrate(parents)` as:

```bash
jj new <F1> <A> <B> <C> -m "Integrate batch <batch-id>"
```

The integration change is itself an `IntegrationCandidate`
([`../spec/data-model.md` `## Integration candidate`](../spec/data-model.md#integration-candidate)):
a distinct record with its own id, `base_frontier`, `parent_candidates`, `integration_manifest`,
`exact_state_id`, and evidence. It is not a view over its parents, and it has no single `atom_id`
or `atom_spec_revision`.

## 11. Conflict handling

Jujutsu can represent unresolved conflicts in commits and continue operating on the graph.

Gordian uses this as a coordination feature, not as an acceptance relaxation.

An integration conflict SHOULD produce structured work:

```text
Integration I42
  parents: A, C
  conflict_area: model.User.identity

Resolution Atom R43
  requires: A, C
  acceptance: integration conflict resolved + required verification passes
```

No unresolved structural conflict may cross the accepted-frontier gate.

## 12. Revision-scoped verification with `jj run`

Jujutsu 0.43 introduced `jj run`, which can execute commands over selected changes using private working copies and parallel execution.

This makes revision-scoped verification a native fit for Gordian. The adapter's
`verify(state, manifest)` operation is realized as:

```bash
jj run \
  -r '<candidate-revset>' \
  -j <N> \
  --ignore-changes \
  -- ./tools/verify
```

A verifier manifest might expand to:

```text
ruff / ty
pytest / Hypothesis
cargo check
cargo test
property tests
Lean checks
schema validation
security checks
benchmark predicates
```

The exact command set is project policy, not hard-coded Gordian semantics.

## 13. Operation log and recovery

Jujutsu's operation log is a separate DAG of repository operations/views and supports reconciliation of concurrent repository operations.

Gordian SHOULD retain Jujutsu operation IDs around significant coordinator mutations so repository-control events can be correlated with Mission Graph events.

The Jujutsu operation log is not a replacement for Gordian execution history because it does not contain the full semantic work, authorization, or evidence model.

## 14. `trunk()` and immutability

Gordian SHOULD treat the accepted frontier and its ancestors as immutable under normal worker operation.

Workers MUST NOT receive authority to bypass immutability policy.

The coordinator MAY possess exceptional recovery authority, but bypass events MUST be auditable.

## 15. Why no `develop`

A permanent `develop` bookmark creates a shared mutable integration hotspot.

At agent scale this introduces questions that Gordian otherwise avoids:

- Which exact `develop` state did a worker observe?
- Which exact state did verification cover?
- Did unfinished sibling work leak into another worker's context?
- Was a failure caused by the worker or by an unrelated concurrent change?

Exact immutable bases and explicit integration candidates already solve the coordination problem more precisely.

## 16. Worker capability model

Default permissions:

| Capability | Worker | Coordinator |
| --- | ---: | ---: |
| read repository graph | yes | yes |
| edit assigned workspace | yes | yes |
| rewrite assigned change | yes | yes |
| create child hypotheses | yes | yes |
| run local verification | yes | yes |
| submit exact candidate | yes | yes |
| move `main` | no | yes |
| redefine `trunk()` | no | yes |
| canonical push | no | yes |
| bypass immutable frontier | no | exceptional |
| release | no | yes |
| deploy | no | separate authority |

## 17. Acceptance condition

An `IntegrationCandidate` `I` is eligible for promotion only if the ten conjuncts of
[`../algorithms/evidence-and-admission.md#the-algorithm`](../algorithms/evidence-and-admission.md#the-algorithm)
hold, in that order, under those names, and at those arities:

```text
CurrentFrontierReconciled(I, t)
ParentsUnadmitted(I)
NoUnresolvedConflict(I)
VerifierManifestComplete(I)
RequiredVerificationPasses(I)
EvidenceBoundToExactCandidate(I)
EvidenceFresh(I)
EvidenceProvenanceValid(I)
LeaseValidAtFreeze(I)
AuthorizedPromotion(actor, I)
```

`CurrentFrontierReconciled` is defined at
`evidence-and-admission.md#frontier-reconciliation`
([link](../algorithms/evidence-and-admission.md#frontier-reconciliation)) and takes the subject
and the frontier; the batch is threaded from `I.integration_batch` rather than left free.

This list is byte-identical, up to the subject placeholder, to `docs/spec/mission-graph.md`
`## Accepted frontier`, to `docs/formal/theorem-catalog.md` T006, and to the field order of
`AcceptanceWitness` in `formal/Gordian/Acceptance.lean`;
`scripts/check-acceptance-witness.sh` asserts the names **and the argument counts** match across
all four, so a site that quietly drops an argument fails the build. This document previously
carried a seventh, differently-named list, which is removed.

The model or worker that produced `I`'s parents does not appear in the correctness predicate
except as provenance and policy input.

## 18. Research basis and limits

The protocol is motivated by current evidence that:

- dependency-aware asynchronous execution with isolated workspaces and structured integration outperforms single-agent baselines on studied workloads;
- workspace isolation alone is insufficient and explicit state coordination can improve outcomes;
- agent-generated contributions show substantial textual merge-conflict prevalence at scale;
- repository-level multi-agent generation benefits from machine-checkable ownership/interface/dependency contracts;
- agent reliability is a property of the surrounding system, including state, permissions, recovery, retrieval, and verification.

These studies do not establish that Jujutsu is superior to Git for autonomous-agent coordination. That is a Gordian hypothesis to test.

## 19. Landing

See [`landing.md`](landing.md). This protocol does not define how a candidate reaches the remote;
the landing document does, because the landing step is where the accepted-frontier CAS is
realized and it must be executable by an agent with no human present.
