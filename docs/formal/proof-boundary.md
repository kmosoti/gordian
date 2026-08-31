# Formal Proof Boundary

Gordian uses Lean 4 to prove **substrate invariants**, not to decorate empirical design claims with mathematical theater.

This document defines what a Gordian proof means.

## 1. Three different questions

Consider the statement:

> Gordian never admits an unresolved-conflict candidate.

There are at least three separate questions hidden inside it.

### Q1. Is the abstract admission rule logically safe?

This can be formally proved.

If the abstract definition of `Acceptable` requires a `conflictFree` witness, then Lean can prove:

```text
Acceptable(c) -> conflictFree(c)
```

That theorem can be checked completely by the Lean kernel relative to the definitions and assumptions supplied to it.

### Q2. Does the Rust implementation faithfully implement that abstract rule?

Lean does **not** establish this merely because an analogous Rust function exists.

We need one or more of:

- extraction/code generation from a proved definition;
- a refinement proof connecting Rust semantics to the Lean model;
- differential tests against a reference model;
- property/model tests over the implementation;
- independent review of the correspondence.

Until such a bridge exists, the formal model and Rust implementation are related artifacts, not a proved refinement.

### Q3. Does `conflictFree` mean the real software is semantically correct?

That is an empirical/modeling question.

A textual-conflict detector can be complete relative to its syntax and still miss an incompatible API change in two different files. A semantic-resource model can be internally consistent while omitting a real dependency.

No proof of the abstract predicate removes this model-risk boundary.

## 2. What "100% proved" means in Gordian

A theorem may be labeled `machine checked` only when all of the following hold for the exact repository revision:

1. The Lean source contains no `sorry` or admitted placeholder relevant to the theorem.
2. `lake build` succeeds under the pinned Lean toolchain.
3. CI runs an independent Lean type checker with `sorry` disallowed.
4. The theorem statement and its assumptions are documented in the theorem catalog.
5. The theorem is not presented as proving a stronger real-world claim than its formal statement.

This is **100% proof of the formal proposition relative to Lean's logic, kernel, imported axioms, and the definitions in the model**.

It is not 100% proof that the proposition's names perfectly correspond to reality.

## 3. Trusted computing base

The formal result still has a trusted computing base.

For the initial kernel this includes, at minimum:

- Lean's logical kernel;
- the pinned Lean compiler/toolchain and standard library;
- the theorem definitions themselves;
- the build environment that selects the intended source revision.

Independent checking reduces implementation-risk in the primary checker but does not magically eliminate every hardware, compiler, or specification assumption.

## 4. The current theorem strategy

The current formal model intentionally focuses on propositions that are both important and crisp.

### Graph structure

Prove that a dependency relation supplied with a strictly decreasing natural-number rank cannot contain a directed cycle.

This theorem does **not** prove that every arbitrary graph can be ranked. Instead, the rank/decrease witness is a certificate of acyclicity.

### Abstraction boundary

Prove that the global hard-dependency target predicate defined by the Mission Graph Specification excludes Quarks.

This proves the policy encoding, not that Atom/Quark is the optimal abstraction.

### Scheduler safety

Represent dispatchability with a witness that contains:

- a valid specification;
- satisfied dependencies;
- satisfied preconditions;
- compatible executor availability;
- resources;
- authorization;
- lease compatibility.

Then prove dispatch implies the required logical witnesses.

This prevents the model from accidentally defining dispatch as a weaker predicate later.

### Evidence compatibility

The normative evidence boundary is [`data-model.md` `## Evidence`](../spec/data-model.md#evidence):
`EvidenceBinding` has exactly seven equality components, including the adapter-neutral exact state
identity, the dependency digest, and the canonicalization scheme. The mismatch theorems in
[`theorem-catalog.md` T004](theorem-catalog.md#t004--evidence-identity-binding) cover those seven
components. This document deliberately does not duplicate the field list; the data model and T004
are the normative definitions.

This proves stale evidence cannot satisfy the formal compatibility predicate.

### Authority

Define role capabilities and prove a Worker cannot produce the accepted-frontier promotion capability.

This is a policy theorem. Runtime credential leakage remains an implementation/security problem outside this theorem.

### Admission

The normative admission witness has exactly ten conjuncts. Their ordered names, arities, Lean
fields, and defining anchors are maintained in [`theorem-catalog.md` T006](theorem-catalog.md#t006--admission-witness)
and evaluated by the admission algorithm's [`admit()` definition](../algorithms/evidence-and-admission.md#the-admission-conjuncts-defined).
This document deliberately does not restate that list; T006 and the algorithm document are the
normative definitions.

The formal siblings prove that an admitted candidate carries each required property.

### Declared non-interference

Prove symmetry of Gordian's declared read/write non-interference predicate.

This is deliberately **not** called a proof of semantic independence. The predicate operates only over declared resource sets.

### Replay

Prove that the same event history passed to the same pure projector yields the same projected state.

This is nearly tautological, and that is useful: it locates the real engineering obligation at the purity/determinism boundary of the projector and event capture rather than pretending event sourcing itself creates determinism.

## 5. Proof obligations we should add later

### Reachability and readiness

Prove that a topological scheduler never emits a node before all hard predecessors have reached the required satisfaction predicate.

### Lease safety

Given a lease model, prove two incompatible exclusive write leases cannot simultaneously be valid for the same semantic resource.

### Candidate immutability

Model candidate handoff as an immutable identity and prove verification can never be attached to an un-frozen candidate state.

### Event transition preservation

Define the Mission execution state machine and prove every accepted transition preserves global invariants.

### Attestation authorization

Model attestation identity and policy, then prove an attestation can satisfy an authorization predicate only when its signer belongs to the required capability set.

### Integration composition

State carefully which properties are compositional and which require a fresh integration verifier. We should expect many useful software properties to be non-compositional.

## 6. Methods beyond proof

Formal proof is one instrument in a larger verification stack.

### Property testing

Use generated graphs, evidence records, candidate mutations, and event sequences to test executable Rust semantics over large input spaces.

### Model-based testing

Use the Lean definitions or a deliberately tiny executable reference model as an oracle for the Rust implementation.

### Mutation testing

Deliberately weaken admission/evidence rules and ensure tests fail. This estimates whether the executable test suite actually protects the invariants it claims to protect.

### State-machine/model checking

For bounded actor/resource configurations, exhaustively enumerate transitions to find deadlocks, illegal promotions, stale leases, or non-progress states.

### Differential integration experiments

Compare:

- Git branches/worktrees versus Jujutsu workspaces/change IDs;
- isolated workers versus isolated + semantic coordination;
- path claims versus semantic resource claims;
- continuous rebase versus stable snapshot execution.

### Fault injection

Kill workers, rewrite candidates after verification, corrupt evidence references, delay dependency events, duplicate messages, expire leases, and create concurrent coordinator operations.

A design that succeeds only on the happy path is not a reliable coordination substrate.

## 7. Rule for claims

Use this sentence test:

> What exact proposition did the proof checker establish, and what assumptions connect that proposition to the engineering claim?

If we cannot answer both halves, Gordian should not advertise the engineering claim as proved.
