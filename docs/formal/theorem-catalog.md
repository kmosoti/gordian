# Gordian Theorem Catalog

This catalog maps normative Gordian rules to their formal statements and explicitly records what each theorem does **not** prove.

The canonical proof sources live under [`../../formal/Gordian`](../../formal/Gordian).

## Verification states

| State | Meaning |
| --- | --- |
| `proof-source-present` | Lean source exists but this document is not claiming a successful checker run for the current revision yet. |
| `machine-checked` | Pinned Lean build and independent checker pass in CI with `sorry` disallowed. |
| `model-only` | Formal statement exists but implementation correspondence is not proved. |
| `planned` | Theorem has been identified but not yet encoded. |

## T001 — Ranked hard dependency acyclicity

**Lean:** `formal/Gordian/Graph.lean#no_dependency_cycle`

### Premises

A dependency graph supplies a natural-number rank function and a proof that every hard dependency edge strictly decreases rank.

```text
A depends_on B -> rank(B) < rank(A)
```

### Theorem

```text
DependsPath(g, A, A) -> False
```

### Engineering interpretation

A rank certificate is sufficient to prove the hard dependency relation acyclic.

### Does not prove

- that an arbitrary input graph automatically has such a rank;
- that a planner cannot create a cycle before validation;
- workflow liveness or guaranteed completion;
- absence of semantic feedback loops outside the hard-dependency relation.

---

## T002 — Quark global dependency exclusion

**Lean:** `formal/Gordian/Graph.lean#quark_not_globally_dependable`

### Theorem

```text
not GloballyDependable(Quark)
```

### Engineering interpretation

The v0 type policy cannot treat a Quark as an allowed global hard-dependency target.

### Does not prove

That Atom/Quark is the optimal decomposition boundary. That is an empirical Gordian hypothesis.

---

## T003 — Dispatch carries dependency satisfaction

**Lean:** `formal/Gordian/Scheduler.lean#dispatchable_implies_dependencies_satisfied`

### Theorem

```text
Dispatchable(f) -> dependenciesSatisfied(f)
```

Related theorems prove dispatchability also carries enabled, precondition, and authorization witnesses.

### Engineering interpretation

If runtime dispatch is shown to refine the formal `Dispatchable` predicate, the scheduler cannot legally dispatch an Atom while its required dependency predicate is false.

### Does not prove

That runtime code faithfully constructs `DispatchWitness`, or that `dependenciesSatisfied` accurately represents all real dependencies.

---

## T004 — Evidence identity binding

**Lean:** `formal/Gordian/Evidence.lean`

### Formal compatibility components

- exact candidate commit ID;
- spec revision;
- input digest;
- environment digest;
- verifier digest.

For every component there is a theorem of the form:

```text
field(evidence) != field(candidate)
  -> not Compatible(evidence, candidate)
```

### Engineering interpretation

The formal model cannot reuse evidence across a changed identity boundary when that boundary is declared relevant.

### Does not prove

- collision resistance of a chosen digest function;
- completeness of the fingerprint inputs;
- correctness of the verifier itself;
- secure capture of environment identity.

---

## T005 — Worker cannot promote accepted frontier

**Lean:** `formal/Gordian/Authority.lean#worker_cannot_promote`

### Theorem

```text
not CanPromoteAccepted(Worker)
```

The model separately grants promotion to Coordinator and deployment only to DeploymentAuthority.

### Engineering interpretation

The capability policy encodes separation of duties between execution and acceptance/deployment.

### Does not prove

Runtime credential isolation, operating-system sandboxing, or absence of implementation privilege escalation.

---

## T006 — Accepted candidates are conflict-free

**Lean:** `formal/Gordian/Acceptance.lean#accepted_implies_conflict_free`

### Theorem

```text
Acceptable(f) -> conflictFree(f)
```

Sibling theorems establish reconciliation, verification, fresh evidence, and authorized promotion.

### Engineering interpretation

Acceptance is modeled as a proof-carrying boundary rather than a mutable status transition.

### Does not prove

That the runtime conflict detector captures every semantic inconsistency. It proves only the abstract admission contract.

---

## T007 — Declared non-interference symmetry

**Lean:** `formal/Gordian/Conflict.lean#declared_noninterference_symmetric`

The v0 predicate is:

```text
writes(A) disjoint reads(B)
and writes(A) disjoint writes(B)
and writes(B) disjoint reads(A)
```

### Theorem

```text
DeclaredNonInterfering(A, B)
  -> DeclaredNonInterfering(B, A)
```

### Engineering interpretation

The admission rule does not produce a contradictory result merely because two candidate transactions are presented in the opposite order.

### Does not prove

Real semantic commutativity, conflict serializability of arbitrary effects, or completeness of declared resource sets.

---

## T008 — Replay stability for equal history

**Lean:** `formal/Gordian/Replay.lean#replay_same_history`

### Theorem

For a fixed pure projector:

```text
historyA = historyB
  -> replay(projector, historyA) = replay(projector, historyB)
```

### Engineering interpretation

Replay stability is a property of deterministic projection over recorded facts.

### Does not prove

- effect capture is complete;
- events are totally ordered correctly;
- distributed ingestion never duplicates or drops events;
- the projector's semantics correspond to external reality.

---

# Planned theorem families

## T009 — Topological scheduler safety

Prove a scheduler operating over a valid dependency DAG emits only currently enabled nodes.

## T010 — Exclusive semantic lease safety

Prove two live exclusive write leases for the same semantic resource cannot coexist under the lease transition system.

## T011 — Candidate freeze

Prove an evidence record can be generated only against a frozen candidate identity and any mutation creates a distinct subject.

## T012 — State-transition invariant preservation

Define Mission execution transitions and prove every legal transition preserves global invariants.

## T013 — No worker-originated frontier mutation

Strengthen T005 from static role policy into a transition theorem over the state machine.

## T014 — Evidence monotonicity under irrelevant change

Precisely state when evidence may remain valid after a change proven irrelevant to a verifier's declared dependency boundary.

This theorem is intentionally deferred because incorrectly formalizing “irrelevant” would create a dangerous false sense of reuse safety.

# Formal coverage metric

Gordian should eventually track formal coverage by **normative invariant**, not line count.

For each MUST-level safety rule in the specification, record one of:

```text
formalized
property-tested
model-checked
integration-tested
empirical-only
unverified
```

The goal is not 100% Lean coverage. The goal is to use the strongest applicable verification method for each claim without pretending mathematical proof can answer empirical questions.
