# Gordian Architecture

## 1. Problem statement

Most software-development tooling conflates several different questions:

- What outcome are we trying to achieve?
- How have we decomposed the outcome into work?
- Which work causally depends on which other work?
- What code states currently exist?
- What did a human or agent actually execute?
- What evidence supports a claim of completion?
- Which state is authorized to become accepted or deployed reality?

Git branches, project cards, pull requests, CI statuses, and wiki pages each cover fragments of this space. When autonomous workers are introduced, the ambiguity becomes operationally expensive: isolated workers can drift, textual merges can succeed while semantic assumptions conflict, verification can become stale after rewrites, and mutable global state can be changed by actors that should not possess that authority.

Gordian therefore treats software development as a **closed-loop coordination system** rather than a ticket workflow.

The core architecture separates intent, code state, execution, evidence, and authority.

## 2. System decomposition

Define Gordian conceptually as:

```text
G = (M, C, X, E, A)
```

where:

- `M` is the Mission Graph: desired state, decomposition, dependencies, contracts, and constraints.
- `C` is the Change Graph: exact and evolving code states represented by Jujutsu.
- `X` is Execution History: attempts, leases, workers, effects, and observed access.
- `E` is the Evidence Graph: artifacts, verification results, attestations, and provenance.
- `A` is the Authority model: which actors may mutate which frontiers and effects.

These structures reference one another, but none is substituted for another.

## 3. Mission Graph

The Mission Graph is the semantic control plane of Gordian.

Its decomposition vocabulary is:

```text
Project
  Mission
    PlanRevision
      Initiative
        Atom
          Quark
```

This is a **decomposition relation**, not a total execution sequence.

### 3.1 Project

A Project is a persistent namespace and system boundary. It associates repositories, knowledge resources, policies, environments, releases, and Missions without making any of those resources the definition of the Project itself.

### 3.2 Mission

A Mission defines a desired state rather than an implementation plan.

A useful abstract form is:

```text
Mission = Goal + Constraints + Acceptance
```

The Mission identity survives replanning. This prevents strategy from becoming confused with purpose.

### 3.3 PlanRevision

A PlanRevision is a versioned strategy proposed for satisfying a Mission. A Mission may have many plan revisions over its lifetime.

This distinction is important because hierarchical planning is useful precisely because compound objectives can be decomposed in different ways. Planning literature also warns that hierarchical systems are sensitive to the quality of decomposition knowledge. Gordian therefore treats a decomposition as revisable, inspectable knowledge rather than permanent truth.

### 3.4 Initiative

An Initiative is a non-primitive subgoal or capability required by a PlanRevision. It may contain many Atoms and may expose its own acceptance predicates.

Initiative satisfaction is not defined as “all child tasks are marked done.” It is determined from the Initiative's acceptance predicates and valid evidence.

### 3.5 Atom

An Atom is the smallest unit that Gordian can independently schedule and verify as a contract.

Conceptually:

```text
Atom = {
  objective,
  preconditions,
  declared_inputs,
  declared_outputs,
  semantic_reads,
  semantic_writes,
  required_interfaces,
  provided_interfaces,
  resource_requirements,
  acceptance_predicates,
  verifier_manifest
}
```

The contract is deliberately reminiscent of Hoare-style reasoning:

```text
{ preconditions } Atom { postconditions }
```

Gordian does not assume that arbitrary software work can be fully proven correct this way. The value is structural: work enters execution with explicit assumptions and observable obligations rather than an informal sentence and a mutable status.

### 3.6 Quark

A Quark is an execution primitive internal to an Atom. Quarks exist to give executors enough structure to act, retry, observe, or delegate without exposing microscopic implementation detail as global project-management state.

A key abstraction invariant is:

```text
No hard cross-Atom dependency may target a Quark.
```

If another Atom requires a Quark's result, that result is architecturally significant enough to be promoted into an Atom-level contract or Artifact.

## 4. Orthogonal graph relations

The decomposition hierarchy must not be overloaded with dependency semantics.

Gordian maintains distinct edge types.

### 4.1 Containment / decomposition

Answers:

> What larger objective is this work part of?

Examples:

```text
Mission -> PlanRevision
PlanRevision -> Initiative
Initiative -> Atom
Atom -> Quark
```

### 4.2 Hard dependency

Answers:

> What must already be satisfied for this unit to be logically enabled?

Hard dependencies must form a DAG.

Cycles indicate an unresolved feedback design, not executable ordering. Iteration belongs in plan revisions or execution attempts rather than in a cyclic hard-dependency graph.

### 4.3 Artifact and data flow

Answers:

> What does this activity consume and produce?

This graph may include source snapshots, binaries, schemas, benchmark reports, generated documents, test outputs, deployment artifacts, or abstract interface contracts.

### 4.4 Semantic claims

Workers declare expected access at a semantic level, for example:

```text
read  model.User
write auth.token_validation
write api.AuthMiddleware
provide AuthenticatedPrincipal
require UserIdentity
```

The goal is not to pretend semantic boundaries are perfectly knowable. They are predictions that can be compared with observed behavior.

### 4.5 Provenance

Answers:

> Which activity, actor, and inputs produced this entity or claim?

Gordian's provenance model should remain compatible with the conceptual structure of W3C PROV: Entity, Activity, Agent, and relations such as `used`, `wasGeneratedBy`, `wasDerivedFrom`, and `wasAssociatedWith`.

## 5. Specification versus execution

A work specification and an execution attempt are different ontological objects.

```text
Atom A-073
  revision 3

ExecutionAttempt run-441 -> failed
ExecutionAttempt run-442 -> failed
ExecutionAttempt run-443 -> candidate produced
```

The Atom itself did not “fail twice.” Two attempts failed to satisfy its contract.

This distinction removes much of the ambiguous state churn common in project trackers.

## 6. Derived state

Mutable status fields should not be treated as foundational truth when the state can be derived.

For an Atom `a`:

```text
Blocked(a) := exists d in hard_dependencies(a) where not Satisfied(d)
```

```text
Enabled(a) :=
  ValidSpec(a)
  and all hard dependencies are satisfied
  and all logical preconditions hold
```

```text
Dispatchable(a) :=
  Enabled(a)
  and a compatible executor is available
  and resources are available
  and authorization is valid
  and no conflicting lease prevents execution
```

```text
Satisfied(a) :=
  acceptance predicates evaluate successfully
  against compatible, fresh evidence
```

The UI may display `PLANNED`, `BLOCKED`, `READY`, `RUNNING`, `VERIFYING`, `SATISFIED`, or `ABANDONED`, but these are projections over underlying facts wherever feasible.

## 7. Evidence freshness

Verification must be bound to the exact thing verified.

Define a conceptual fingerprint:

```text
Fingerprint(a) = H(
  spec_revision
  || exact_code_state
  || declared_inputs
  || resolved_dependencies
  || relevant_environment
  || verifier_definition
)
```

Evidence is compatible only when its subject fingerprint matches the state under evaluation.

Consequences:

- rewriting a Jujutsu change invalidates verification bound to the old commit ID;
- changing an Atom specification invalidates incompatible evidence;
- changing a dependency invalidates evidence when that dependency is part of the verification boundary;
- environment changes can invalidate evidence when the contract declares them relevant.

This is inspired by hermetic build systems and supply-chain provenance rather than by project-management status semantics.

## 8. Declared versus observed dependencies

Build-system engineering gives Gordian a useful falsifiable invariant: actual dependencies should be contained within declared dependencies.

Let:

```text
D_declared = dependencies and semantic resources declared before execution
D_observed = dependencies and resources observed during execution
```

The desired safety property is:

```text
D_observed subset-of D_declared
```

Violations indicate hidden dependencies or scope expansion.

Extra declared dependencies are not necessarily incorrect, but they reduce available parallelism and may hide architecture that is more coupled than necessary.

Observation is necessarily imperfect. Failure to observe a dependency does not prove it does not exist.

## 9. Effect model

Gordian must distinguish deterministic transformation from external effect.

Suggested effect classes:

| Class | Meaning | Default replay behavior |
| --- | --- | --- |
| `pure` | deterministic transformation | replay freely |
| `hermetic` | deterministic from declared inputs/environment | replay freely |
| `external_read` | observes mutable external state | record result |
| `idempotent_write` | repeated execution is equivalent | retry under policy |
| `compensatable_write` | effect has a defined compensation | retry with protocol |
| `irreversible` | destructive/non-repeatable external effect | explicit authority required |
| `judgment` | human/model decision | record as attestation/result |

The deterministic coordinator should not replay nondeterministic external work by simply invoking it again. It should replay the recorded event/result and schedule a new attempt only when policy explicitly calls for one.

This follows the same general reliability principle used by durable workflow systems such as Temporal: deterministic orchestration is separated from effectful Activities whose outcomes become durable history.

## 10. Jujutsu as the Change Graph

Jujutsu is unusually compatible with Gordian because it distinguishes a logical change identity from exact commit identities and supports multiple workspaces, multi-parent changes, first-class conflicts, revset-based selection, and an operation log.

Gordian adopts the following mapping:

```text
Mission Atom                 semantic work contract
Jujutsu change ID            evolving implementation identity
Jujutsu commit ID            exact implementation candidate
ExecutionAttempt             worker activity against an exact base
VerificationEvidence         result bound to exact candidate + environment
```

The central invariant is:

> Workers operate on logical change identities. Verification applies to exact commit identities.

A verification record against a change ID alone is insufficient because rewriting the change can preserve its change ID while producing a new commit ID.

## 11. Transactional execution model

Treat an admitted Atom approximately as a software-development transaction:

```text
T_i = (B_i, R_i, W_i, V_i)
```

where:

- `B_i` is the exact base revision;
- `R_i` is the declared read/dependency set;
- `W_i` is the declared write/effect set;
- `V_i` is the verification contract.

Two tasks are candidates for safe parallel execution when there is no hard dependency between them and their expected access sets do not conflict:

```text
W_i intersect (R_j union W_j) = empty
W_j intersect (R_i union W_i) = empty
```

This is a heuristic admission rule inspired by conflict serializability and optimistic concurrency control. It is not a proof of semantic independence because declared sets may be incomplete and semantic conflicts can span distinct files.

## 12. Snapshot isolation for workers

Workers receive an exact immutable base commit and execute in isolated Jujutsu workspaces.

They should not be continuously rebased onto a moving accepted frontier.

Instead:

```text
snapshot accepted frontier
        -> execute privately
        -> produce exact candidate
        -> reconcile against current accepted frontier
        -> verify integrated state
        -> admit or reject
```

This gives each worker a stable reasoning environment and shifts staleness handling to integration time, analogous to optimistic concurrency control.

## 13. Shared semantic coordination plane

Isolation alone is insufficient for effective multi-agent work.

The coordination plane exposes small, explicit facts such as:

```text
atom: AUTH-42
attempt: run-882
worker: agent-A7
base_commit: ...
change_id: ...

claims:
  read:
    - model.User
    - config.Security
  write:
    - auth.token_validation
    - api.AuthMiddleware
  provide:
    - AuthenticatedPrincipal

signals:
  - started
  - scope_expanded
  - interface_published
  - dependency_ready
  - candidate_ready
  - verification_failed
```

Workers coordinate through these semantic signals rather than by being granted unconstrained shared filesystem mutation.

## 14. Candidate integration

Independent work should remain sibling changes when it is semantically independent.

False serialization encodes chronology as causality and unnecessarily reduces parallelism.

When candidate sibling changes must be evaluated together, integration becomes its own explicit state, potentially a multi-parent Jujutsu change.

Integration is then verified independently. Individual candidate success does not imply compositional success.

Unresolved conflicts may exist as intermediate repository states, but they cannot cross the acceptance gate.

## 15. Authority boundaries

Worker agents are deliberately less privileged than the coordinator.

Default capability split:

| Operation | Worker | Coordinator |
| --- | ---: | ---: |
| edit assigned workspace | yes | yes |
| rewrite assigned change | yes | yes |
| create local child changes | yes | yes |
| read other visible changes | yes | yes |
| submit candidate commit | yes | yes |
| move accepted `main` / `trunk()` | no | yes |
| push canonical remote state | no | yes |
| bypass immutability policy | no | exceptional |
| create release | no | yes |
| deploy | no | separate deployment authority |

The worker can destroy its own hypothesis. It cannot redefine accepted reality.

## 16. Admission rule

A candidate code state `c` may be admitted to the accepted frontier only when all required conditions hold.

Conceptually:

```text
Accept(c) iff
  current_trunk is an ancestor of or explicitly reconciled into c
  and c contains no unresolved structural conflict
  and every required verifier passes
  and all evidence is bound to the exact candidate identity
  and evidence remains fresh for the current specification and inputs
  and the actor performing admission has coordinator authority
```

Notably, “the LLM says it is done” is not part of this predicate.

## 17. Closed-loop interpretation

Gordian can be viewed as a controller over engineering state.

```text
Observed state S_t
      |
      v
Mission / desired state S*
      |
      v
Planner -> Mission Graph revision
      |
      v
Scheduler -> enabled Atoms
      |
      v
Workers -> candidate changes/effects
      |
      v
Observers/verifiers -> evidence
      |
      v
Reconcile observed state with desired state
      |
      +----> replan / repair / accept
```

The central quantity is not “how many tickets are done?” but the unresolved delta between desired state and justified observed state.

## 18. What is established versus experimental

### Strongly grounded design ingredients

The following components are adaptations of established ideas:

- hierarchical decomposition and partial-order planning;
- explicit dependency DAGs;
- contract-style preconditions and postconditions;
- optimistic concurrency and snapshot isolation as coordination analogies;
- hermetic inputs and content-bound verification;
- deterministic replay with effectful activities recorded separately;
- provenance based on entities, activities, and agents;
- attestation of materials, products, execution identity, and outcomes;
- capability separation between workers and authority-bearing coordinators.

### Evidence-supported multi-agent conclusions

Recent 2026 software-engineering-agent research supports:

- dependency-aware planning plus isolated workers and structured integration;
- explicit shared state beyond workspace isolation alone;
- semantic/interface contracts before concurrent implementation;
- substantial integration-conflict prevalence in agent-generated pull requests;
- evaluating reliability at the harness/system level rather than treating model quality as the whole system.

### Gordian hypotheses requiring falsification

The following are design hypotheses, not established results:

- `Mission -> PlanRevision -> Initiative -> Atom -> Quark` is the right decomposition vocabulary;
- Atom is the best scheduling boundary;
- forbidding cross-Atom Quark dependencies produces useful modularity without excessive promotion overhead;
- semantic read/write claims can predict conflicts accurately enough to improve scheduling;
- derived state materially reduces stale coordination compared with mutable status fields;
- Jujutsu's change model provides enough benefit over Git-based branch orchestration to justify the integration complexity;
- formalizing substrate invariants in Lean measurably reduces runtime coordination defects.

These should become experiments, not doctrine.

## 19. Architectural target

The emerging system is best summarized as:

> **Mission Graph + MVCC-style coordination + Jujutsu change DAG + scheduler + provenance/evidence system + capability-gated acceptance.**

The result should behave less like a project-management board and more like a verifiable engineering control plane.
