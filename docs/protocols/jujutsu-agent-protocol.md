# Jujutsu Agent Execution Protocol

Status: **experimental protocol**

This document maps Gordian Mission Graph semantics onto Jujutsu execution primitives.

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

Conceptually:

```bash
jj workspace add ../worker-auth \
  --name worker-auth \
  -r <exact-base-commit>
```

Another independent worker receives another workspace from its own admitted base.

A workspace is an execution container for mutable source state, not a project-management identity.

## 5. Snapshot rule

A worker MUST be admitted against an exact base commit.

Do not continuously rebase active workers onto a moving accepted frontier.

The preferred lifecycle is optimistic:

```text
accepted frontier F0
       |
       +--> worker snapshot at F0
               |
               +--> mutable change
                       |
                       +--> exact candidate C

meanwhile frontier may become F1

C + F1
  -> integration/reconciliation
  -> verification
  -> admission or rejection
```

This gives the worker a stable reasoning context while moving-world reconciliation occurs at a controlled boundary.

## 6. One writer per change

The protocol SHOULD maintain one active writer lease per Jujutsu change ID.

Jujutsu can represent divergent versions of the same change, but deliberate concurrent rewriting of the same logical change is a poor default coordination strategy.

For speculative execution, create separate changes from the same base:

```text
Atom AUTH-42
  candidate/change A
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
base_commit: <commit>
change_id: <change>
lease_expires_at: <time>
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

The worker hands off an exact commit ID.

The candidate becomes logically frozen for the corresponding verification attempt.

### 9.8 Verify candidate

Task-local verifiers run against the exact candidate.

Verification evidence records at least:

```yaml
atom: AUTH-42
spec_revision: <digest>
change_id: <logical-change>
commit_id: <exact-candidate>
base_commit: <exact-base>
environment_digest: <digest>
verifier_id: <id>
verifier_version: <version>
result: pass|fail
```

### 9.9 Integrate

Satisfied candidate changes that must coexist are composed into an integration candidate.

Independent siblings SHOULD remain siblings until an integration state is required. The DAG should encode causal dependency, not finishing order.

### 9.10 Verify integration

The integrated state receives cross-component verification.

Component verification is necessary but not sufficient for integration acceptance.

### 9.11 Accept

Only the coordinator may promote a verified, conflict-free, current integration state into the accepted frontier.

### 9.12 Release

Release is a separate authority transition. An immutable tag/artifact identifies a release. Deployment records identify what is actually running.

Accepted code and deployed code are related but distinct truths.

## 10. Integration as explicit state

Suppose independent Atoms produce changes `A`, `B`, and `C`.

The protocol SHOULD preserve them as sibling changes if no causal relation exists.

When they must be tested together, Gordian may create a multi-parent integration state conceptually equivalent to:

```bash
jj new <A> <B> <C> -m "Integrate candidate set"
```

The integration change is itself an object with evidence and outcome.

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

This makes revision-scoped verification a native fit for Gordian.

Conceptually:

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

A candidate `c` is eligible for promotion only if:

```text
CurrentFrontierReconciled(c)
NoUnresolvedConflicts(c)
AllRequiredVerifiersPass(c)
EvidenceTargetsExactCommit(c)
EvidenceMatchesCurrentSpec(c)
EvidenceMatchesRelevantEnvironment(c)
PromotionActorAuthorized(c)
```

The model or worker that produced `c` does not appear in the correctness predicate except as provenance and policy input.

## 18. Research basis and limits

The protocol is motivated by current evidence that:

- dependency-aware asynchronous execution with isolated workspaces and structured integration outperforms single-agent baselines on studied workloads;
- workspace isolation alone is insufficient and explicit state coordination can improve outcomes;
- agent-generated contributions show substantial textual merge-conflict prevalence at scale;
- repository-level multi-agent generation benefits from machine-checkable ownership/interface/dependency contracts;
- agent reliability is a property of the surrounding system, including state, permissions, recovery, retrieval, and verification.

These studies do not establish that Jujutsu is superior to Git for autonomous-agent coordination. That is a Gordian hypothesis to test.
