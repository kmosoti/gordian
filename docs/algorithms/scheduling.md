# Scheduling and Concurrency Algorithms

Gordian's scheduler operates over the Mission Graph, semantic resource claims, current evidence, executor capabilities, and leases. Jujutsu workspaces are execution containers for admitted work, not the scheduling model itself.

## 1. Hard-dependency readiness

Let the hard dependency relation be:

```text
A -> B
```

meaning `A` depends on `B`.

For Atom `a`:

```text
Blocked(a) := exists d in HardDeps(a) where not Satisfied(d)
```

```text
Enabled(a) :=
    ValidSpec(a)
    and not Blocked(a)
    and PreconditionsHold(a)
```

The distinction between **Enabled** and **Dispatchable** is deliberate.

```text
Dispatchable(a) :=
    Enabled(a)
    and CompatibleExecutorAvailable(a)
    and RequiredResourcesAvailable(a)
    and AuthorizationValid(a)
    and LeaseCompatible(a)
```

An Atom may therefore be logically ready even when no current executor or resource allocation can run it.

This avoids encoding infrastructure scarcity as a false dependency.

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

A useful initial lease tuple is:

```text
Lease {
  id,
  holder,
  subject_atom,
  resource,
  mode,
  issued_at,
  expires_at,
  fencing_token
}
```

Modes:

```text
read
write_shared_if_commutative
write_exclusive
```

The important future invariant is:

> Two valid exclusive write leases over the same semantic resource cannot coexist.

A monotonically increasing fencing token should be preferred over trusting wall-clock lease expiration alone when a downstream resource can reject stale actors.

## 8. Stable snapshot execution

Once admitted, an Atom receives an exact base candidate:

```text
base_commit = C_t
```

The worker executes against that snapshot in an isolated Jujutsu workspace.

Gordian does not continuously rebase the worker as accepted state advances.

Instead:

```text
snapshot
  -> work
  -> freeze candidate
  -> reconcile with current frontier
  -> integration verification
```

This is analogous to optimistic concurrency control in one limited sense: workers reason over stable snapshots, and staleness/conflict is checked at commit/integration time.

The analogy must not be stretched into a claim that source edits are database transactions. Software semantic conflicts are richer than database read/write conflicts.

## 9. Candidate-set integration

Suppose independent Atoms produce exact candidates:

```text
A@c1
B@c2
C@c3
```

Rather than forcing an arbitrary serial history, create an explicit integration candidate `I` over the selected parent states.

The integration candidate gets its own evidence fingerprint and verifier set.

```text
Verified(A) and Verified(B)
```

does not imply:

```text
Verified(Integrate(A, B))
```

This non-compositionality rule is central. Unit-level success cannot substitute for system-level verification.

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
