# Scheduling and Concurrency Algorithms

Gordian's scheduler operates over the Mission Graph, semantic resource claims, current evidence, executor capabilities, and leases. Jujutsu workspaces are execution containers for admitted work, not the scheduling model itself.

## 1. Hard-dependency readiness

Let the hard dependency relation be `A -> B`, meaning `A` depends on `B`. Allowed depender and
prerequisite kinds are `Atom` on both sides
([`../spec/data-model.md` `### Global hard dependency target kinds`](../spec/data-model.md#global-hard-dependency-target-kinds)).

The readiness predicates are **defined once**, in
`docs/spec/mission-graph.md#logical-state-predicates`
([link](../spec/mission-graph.md#logical-state-predicates)), and the seven named sub-predicates under
`## Readiness predicate definitions`, `docs/spec/mission-graph.md#readiness-predicate-definitions`
([link](../spec/mission-graph.md#readiness-predicate-definitions)). This document defines none of
them. The two blocks reproduced below are byte-identical copies of the normative blocks, and
`scripts/check-predicate-definitions.sh` fails the build if they drift.

```text
Blocked(a) :=
  exists d in hard_dependencies(a)
  where dependency_condition(d) is false
  or exists q in required_interfaces(a)
     such that no Atom p with Satisfied(p) has q in provided_interfaces(p)
     and q has no ExternalProvision record
  or exists i in declared_inputs(a)
     such that no Atom p with Satisfied(p) has i in declared_outputs(p)
     and i has no ExternalProvision record
```

`Satisfied` here is the **admitted-frontier** predicate: an Atom is Satisfied only when its
Candidate was admitted into the accepted frontier as part of an `IntegrationCandidate` whose
integration verification discharged the Atom's manifest. A verified-but-unadmitted candidate does
not unblock a dependent, because a dependent's execution base is a frontier state and an
unadmitted candidate is in no frontier state.

### Computing Blocked in O(in-degree)

Plan validation resolves exactly one `ProviderBinding` per requirement and materializes each
Atom-provider binding as one derived `HardDependency` (`origin = derived_interface` /
`derived_artifact`), rejecting publication of any plan whose requirement is unbound, unprovided,
or ambiguously provided. Because the binding is unique, the existential in clauses 2 and 3 has at
most one witness, so the conjunction over derived edges is extensionally equal to the three-clause
form and the runtime evaluates one loop over one edge table:

```text
blocked(a) = hard_dep_edges[a].iter().any(|d| !dependency_condition(d))
```

with `dependency_condition(d)` reading `satisfaction_index[d.prerequisite_atom]`, an O(1)
projection lookup. Readiness is therefore O(in-degree), not a graph walk.

The distinction between **Enabled** and **Dispatchable** is deliberate.

```text
Enabled(a) :=
  ValidSpec(a)
  and not Blocked(a)
  and PreconditionsHold(a)
```

```text
Dispatchable(a) :=
  Enabled(a)
  and CompatibleExecutorAvailable(a)
  and RequiredResourcesAvailable(a)
  and AuthorizationValid(a)
  and LeaseCompatible(a)
```

An Atom may therefore be logically ready even when no current executor or resource allocation can
run it. This avoids encoding infrastructure scarcity as a false dependency.

## 2. DAG validation

Hard dependencies MUST be acyclic.

A practical validation implementation can use either:

- Kahn's topological-sort algorithm;
- depth-first search with temporary/permanent marks;
- a rank certificate supplied by a planner and validated edge-by-edge.

### Kahn validation

Given `V` work nodes and `E` hard dependency edges:

1. compute each node's in-degree;
2. enqueue all zero-in-degree nodes;
3. repeatedly remove one, decrementing successors;
4. count removed nodes;
5. if the count is less than `|V|`, at least one directed cycle exists.

Complexity:

```text
Time:  O(V + E)
Space: O(V + E)
```

The current Lean theorem uses a different certificate-oriented formulation: if every dependency edge strictly decreases a natural-number rank, a cycle is impossible. That proof is simple enough to inspect and can later support a planner that emits a topological rank along with the plan.

## 3. Why decomposition is not dependency

Suppose Mission `M` decomposes into Initiatives `I1` and `I2`, and each Initiative decomposes into Atoms.

The decomposition graph answers:

> Which objective is this work part of?

The dependency DAG answers:

> Which result must exist before this work can proceed?

Sibling order, creation time, or nesting MUST NOT silently create hard dependencies.

This matters because false serialization turns chronology into causality and destroys available parallelism.

## 4. Declared semantic access

Before execution, an Atom may declare semantic resources:

```text
R_i = expected reads
W_i = expected writes
P_i = interfaces provided
Q_i = interfaces required
```

Resources may represent:

- source module;
- Rust crate;
- Python package;
- public type;
- function/API contract;
- database table/schema;
- configuration key;
- protocol message;
- migration namespace;
- generated artifact;
- service endpoint.

The scheduler should not reduce these claims to filenames. File overlap is an observable signal, but semantic conflict can cross path boundaries.

## 5. Pairwise admission predicate

For two transactions `i` and `j`, a conservative declared-access check is:

```text
W_i ∩ R_j = empty
W_i ∩ W_j = empty
W_j ∩ R_i = empty
```

Equivalent compact notation:

```text
W_i ∩ (R_j ∪ W_j) = empty
W_j ∩ (R_i ∪ W_i) = empty
```

If this predicate is false, the scheduler should usually serialize, refine the contracts, or insert an interface-producing dependency before concurrent execution.

If the predicate is true, concurrency is merely **admissible**, not proven safe.

Reasons for false negatives include:

- undeclared semantic resources;
- dynamic imports;
- hidden shared databases/services;
- convention-level coupling;
- behavior changes not captured by the resource vocabulary;
- nondeterministic external effects.

The Lean kernel proves symmetry of the declared predicate. It intentionally does not call this semantic serializability.

## 6. Declared versus observed access

Borrowing from build-system dependency engineering, Gordian distinguishes:

```text
D_declared
D_observed
```

The desired relation is approximately:

```text
D_observed subset-of D_declared
```

Bazel applies the corresponding principle to build dependencies: actual dependencies must be represented within declared dependencies, while excessive declarations reduce performance and modularity.

Gordian generalizes the idea to work execution.

During an attempt, instrumentation may discover:

- file reads/writes;
- imported modules;
- compiler/linker inputs;
- database/schema access;
- network endpoints;
- generated interfaces;
- configuration reads;
- process/environment dependencies.

An observed access outside declared scope should emit:

```text
ScopeExpanded {
  attempt,
  resource,
  access_kind,
  observed_at
}
```

and force concurrency/conflict re-evaluation.

Absence of observed access is not proof of absence. Instrumentation coverage must itself be documented.

## 7. Lease model

Semantic claims are predictions. Leases are runtime coordination controls.

The canonical record is
[`../spec/data-model.md` `## Lease`](../spec/data-model.md#lease), reproduced here field for
field because the scheduler reads every one of them; the two blocks MUST stay identical:

```text
LeaseSubject =
  | SemanticResource(SemanticResourceId)
  | LogicalChange(LogicalChangeId)
  | Coordinator(ProjectId)

Lease {
  id,
  holder_actor,
  holder_attempt?,             -- absent for a Coordinator lease, which no attempt holds
  subject,                     -- LeaseSubject
  mode,                        -- read | write_shared_if_commutative | write_exclusive
  operation?,                  -- REQUIRED when mode = write_shared_if_commutative
  fencing_token,               -- FencingToken, strictly monotonic per subject
  issued_at_event,             -- EventSeq of the LeaseGranted event
  expires_at_event,            -- EventSeq at or after which the lease is not live
  issued_at,                   -- wall clock; provenance only, never read by a predicate
  expires_at,                  -- wall clock; provenance only, never read by a predicate
  revoked_at_event?            -- EventSeq of the LeaseRevoked event
}
```

Liveness is denominated in `EventSeq`, never in wall-clock time: a scheduler that compared
`expires_at` against a clock would answer readiness differently on every replay.

The lease subject is a three-constructor sum, not a bare semantic resource, because Gordian
excludes three different things: concurrent semantic writes to a domain resource, concurrent
rewriting of one evolving source change, and two coordinators admitting for one Project. A change
identity is not a `SemanticResource`, and neither is the coordinator role.

The invariants are:

> Two live exclusive write leases over the same `LeaseSubject` MUST NOT coexist.

> At most one live `write_exclusive` lease may have subject `LogicalChange(x)` for any `x`.

A `write_shared_if_commutative` grant is permitted only on a `SemanticResource` subject, and only
when the requesting and every live holder's `ResourceClaim.operation` is a member of that
resource's `metadata.commutative_operations`. Commutativity is declared explicitly by a
capability-holding actor and recorded as an `OperationCommutativityDeclared` event; it is never
inferred from declared resource independence, which is not proof of semantic commutativity.

A monotonically increasing `FencingToken` is issued per `LeaseSubject` and MUST be preferred over
trusting wall-clock lease expiration alone. Because the source plane cannot reject a stale actor,
the token is recorded on the `Candidate` at freeze and checked at admission
(`LeaseValidAtFreeze`).

## 8. Stable snapshot execution

Once admitted for execution, an Atom `b` receives an exact base state constrained by
**PrerequisiteContaining**
([`../spec/mission-graph.md` `## Stable snapshots`](../spec/mission-graph.md#stable-snapshots)):

```text
base = F  where
    F is an admitted frontier state
    and for every d in hard_dependencies(b):
        Satisfied(d) and satisfaction_frontier_seq(d) <= frontier_seq(F)
```

In the common case `F` is the current accepted frontier and the constraint is discharged by the
fact that `b` was dispatched only when `not Blocked(b)`. It is stated separately because a
scheduler may deliberately dispatch against an older frontier — to reuse a warm workspace, or to
reproduce a failure — and doing so against a base that predates a prerequisite's admission is
exactly the bug this rule forbids.

The worker executes against that snapshot in an isolated workspace.

Gordian does not continuously rebase the worker as accepted state advances. Instead:

```text
snapshot
  -> work
  -> freeze candidate (records fencing_token, exact_state_id, base_frontier_seq)
  -> enter admission queue
  -> integration batch over the current frontier
  -> integration verification
  -> admission or return to reconciliation
```

Reconciliation happens in **batches**, not per candidate: see
[`evidence-and-admission.md` `### Re-verification policy`](evidence-and-admission.md#re-verification-policy).
Per-candidate reconciliation costs `O(N²)` verifier runs for `N` concurrent workers, because each
admission invalidates every other in-flight candidate's evidence.

This is analogous to optimistic concurrency control in one limited sense: workers reason over
stable snapshots, and staleness/conflict is checked at integration time. The analogy must not be
stretched into a claim that source edits are database transactions.

## 9. Candidate-set integration

Suppose independent Atoms produce exact candidates `A@c1`, `B@c2`, `C@c3`.

Rather than forcing an arbitrary serial history, the coordinator assembles an `IntegrationBatch`
and creates an explicit `IntegrationCandidate` `I` whose `base_frontier` is the current accepted
frontier `t` and whose `parent_candidates` are `{c1, c2, c3}`.

`I` is a first-class record with its own identity, its own `integration_manifest`, and its own
evidence fingerprint
([`../spec/data-model.md` `## Integration candidate`](../spec/data-model.md#integration-candidate)).

```text
Verified(A) and Verified(B)
```

does not imply:

```text
Verified(Integrate(A, B))
```

except for verifiers whose manifest entry declares `compositional = true`, whose inheritance is
recorded as an `EvidenceInherited` event and whose entry remains in `integration_manifest(I)`
marked inheritable. This non-compositionality rule is central: unit-level success cannot
substitute for system-level verification, and the `compositional` flag is the single, auditable,
falsifiable exception.

## 10. Scheduling objective

A mature scheduler may optimize multiple quantities:

```text
maximize expected useful parallelism
minimize predicted conflict cost
minimize critical-path completion time
minimize executor/resource cost
minimize verification invalidation
```

subject to hard safety constraints.

This is a multi-objective optimization problem rather than a single queue-priority number.

Initially, Gordian should prefer understandable heuristics over an opaque learned scheduler. A learned policy can be introduced later only with a deterministic safety envelope around its proposals.

## 11. Research questions

The scheduler is where several Gordian hypotheses become measurable:

1. Do semantic claims improve conflict precision/recall over file overlap?
2. Does Atom-level scheduling expose enough concurrency without excessive decomposition cost?
3. What is the cost of false-positive serialization versus false-negative conflict?
4. Does stable snapshot execution improve agent completion quality enough to offset integration staleness?
5. Can historical provenance learn better conflict priors without turning prior correlation into hard dependency?

These are experimental questions. The formal kernel should constrain unsafe states while the experiments optimize policy within those constraints.
