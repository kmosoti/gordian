# Mission Graph Specification

Status: **research specification**

The Mission Graph is Gordian's canonical representation of engineering intent. It specifies desired state, decomposition, causal prerequisites, contracts, constraints, and the evidence required to justify satisfaction.

It is deliberately distinct from source history, execution history, and evidence history.

Normative words `MUST`, `MUST NOT`, `SHOULD`, `SHOULD NOT`, and `MAY` describe intended substrate behavior.

## Core separation

Gordian reasons over several linked structures:

```text
Mission Graph       desired engineering state and work contracts
Change Graph        evolving/exact source states, normally Jujutsu
Execution History   concrete attempts and effects
Evidence Graph      observations, verification, provenance, attestations
Authority Model     capabilities governing state-changing operations
```

No structure is a substitute for another.

## Decomposition vocabulary

```text
Project
  Mission
    PlanRevision
      Initiative
        Atom
          Quark
```

This is a **decomposition relation**, not a total order and not an execution DAG.

### Project

A Project is a persistent namespace and system boundary. It associates repositories, knowledge resources, policies, environments, releases, and Missions.

A Project MUST NOT use a particular forge, repository, or issue tracker as its logical identity.

### Mission

A Mission is a stable objective contract:

```text
Mission = Goal + Constraints + Acceptance
```

A Mission MUST define:

```text
id
goal
constraints
acceptance
```

It MAY additionally define scope, non-goals, budgets, deadlines, risk constraints, and priority.

A Mission MUST NOT make one implementation strategy part of its persistent identity.

### PlanRevision

A PlanRevision is an immutable strategy for attempting to satisfy a Mission.

It MUST identify:

- its Mission;
- the Mission specification revision it targets;
- its decomposition strategy;
- its provenance and rationale.

A Mission MAY have multiple historical or competing PlanRevisions.

Replanning produces a new PlanRevision rather than silently rewriting why prior work existed.

### Initiative

An Initiative is a compound capability or subgoal within a PlanRevision.

It MUST define its own objective and acceptance contract.

Satisfaction of every child Atom MUST NOT mechanically imply Initiative satisfaction unless that is exactly the Initiative's declared acceptance rule.

### Atom

An Atom is Gordian's smallest globally schedulable and independently verifiable work contract.

An Atom specification SHOULD expose:

```text
objective
preconditions
declared_inputs
declared_outputs
hard_dependencies
semantic_reads
semantic_writes
required_interfaces
provided_interfaces
resource_requirements
effect_class
acceptance_predicates
verifier_manifest
```

An Atom is not a ticket-shaped sentence. It is a bounded transformation/obligation whose assumptions and success conditions can be inspected.

### Quark

A Quark is executor-local structure used to realize an Atom.

A Quark MAY describe operation, inputs, outputs, local preconditions, local verification, and effect class.

Quarks SHOULD exist only when they improve execution, retry, delegation, observability, or provenance.

The substrate MUST NOT force humans to manually groom Quark state.

## Abstraction boundary

Global hard dependencies MUST NOT target another Atom's Quark.

If another Atom requires a Quark result, the producing Atom must expose the result through an Atom-level artifact/interface, or the relevant work must be promoted into an Atom.

This is a normative modularity rule, not an empirical claim that Atom/Quark is the optimal granularity. That claim remains experimentally testable.

## Orthogonal relations

### Decomposition

Answers:

> What larger objective is this part of?

### Hard dependency

Answers:

> What contract must already be satisfied before this work is logically enabled?

Hard dependencies MUST form a directed acyclic graph.

### Artifact/data flow

Answers:

> What immutable or identity-addressed entity does this work consume or produce?

### Semantic resource relation

Answers:

> Which domain resource/interface does this work expect to read, write, require, or provide?

### Provenance

Answers:

> Which actor/activity/materials produced this artifact, evidence, or assertion?

## Hard dependency semantics

For:

```text
A depends_on B
```

`A` MUST NOT be logically Enabled until `B` satisfies the dependency condition.

The default condition is `Satisfied(B)`.

Cycles MUST be rejected before scheduling.

Iteration, repair, retry, and replanning belong in attempts, events, control constructs, or new PlanRevisions rather than hard-dependency cycles.

## Semantic resource claims

An Atom MAY declare domain-level access:

```text
read     type://User
write    api://AuthMiddleware
provide  interface://AuthenticatedPrincipal
require  interface://UserIdentity
```

Claims are scheduling predictions, not proofs of independence.

Observed access SHOULD be reconciled against declarations. Previously undeclared writes SHOULD emit a scope-expansion event and trigger conflict/re-admission analysis before promotion.

## Parallel admission

For two candidate work transactions `i` and `j`, let `R` be declared reads and `W` declared writes.

A conservative pairwise non-interference predicate is:

```text
W_i ∩ R_j = ∅
W_i ∩ W_j = ∅
W_j ∩ R_i = ∅
```

Concurrent admission additionally requires no hard dependency ordering the pair.

This predicate is a heuristic safety filter over declared resources. It MUST NOT be described as proof of semantic serializability.

## Logical state predicates

Mutable status fields SHOULD NOT be canonical truth when state can be derived.

For Atom `a`:

```text
Blocked(a) :=
  exists d in hard_dependencies(a)
  where dependency_condition(d) is false
```

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

```text
Active(a) :=
  exists running ExecutionAttempt whose subject is a
```

```text
Satisfied(a) :=
  acceptance(a) evaluates true
  against current compatible evidence
```

UI labels may materialize these predicates but SHOULD remain reproducible from canonical facts.

## Specification and execution are different objects

An ExecutionAttempt is one concrete attempt to realize an Atom or Quark.

It MUST record at least:

```text
id
subject
subject_spec_revision
actor
exact_base_state
start_time
outcome
```

For source-changing attempts it SHOULD record:

```text
jj_workspace
jj_change_id
candidate_commit_id
```

Attempt outcomes include running, candidate-produced, failed, timed-out, abandoned, and cancelled.

Attempt outcome MUST NOT mutate the semantic specification it attempted.

## Stable snapshots

A source-changing worker MUST receive an exact base commit.

Gordian SHOULD NOT continuously rebase active worker state by default.

Instead:

```text
snapshot exact accepted state
        -> private execution
        -> candidate freeze
        -> reconcile with current accepted frontier
        -> integration verification
        -> promote or repair/replan
```

The benefit of this policy is an experimental hypothesis and must be benchmarked against continuous-rebase alternatives.

## Candidate identity

A Candidate is a frozen exact subject handed to verification.

A Candidate MUST identify:

```text
Atom
Atom spec revision
base source state
logical implementation/change identity
exact source commit identity
producer attempt
freeze event/time
```

Any subsequent source mutation creates a different candidate identity for verification purposes.

## Effect classes

Executable work SHOULD declare one of:

```text
pure
hermetic
external_read
idempotent_write
compensatable_write
irreversible
judgment
```

Retry policy MUST respect effect semantics.

Replay of projection state MUST NOT silently repeat external effects.

Irreversible work MUST require explicit authority and a policy for ambiguous completion.

## Artifacts

An Artifact is an immutable or identity-addressed entity consumed or produced by an activity.

Examples:

```text
source commit
binary
container image
schema
benchmark result
test report
formal proof artifact
release bundle
model response
```

Content addressing SHOULD be used where it improves exact identity and reproducibility.

## Evidence

Evidence is an observation relevant to evaluating an acceptance predicate.

Evidence MUST identify its subject and producer/source.

Verification evidence SHOULD bind every relevant identity:

```text
spec revision
exact candidate commit
resolved input/dependency identity
environment identity
verifier identity/digest
canonicalization scheme
```

A conceptual fingerprint is:

```text
H(
  spec_revision
  || exact_candidate
  || canonical(resolved_inputs)
  || canonical(resolved_dependencies)
  || canonical(relevant_environment)
  || verifier_digest
)
```

Evidence MUST NOT satisfy current acceptance when its compatibility rule fails.

The fingerprint's equality rule can be formally proven; completeness of the selected inputs remains an engineering/modeling obligation.

## Integration

Independent candidate changes MAY remain sibling changes when causally independent.

When candidates must be evaluated together, Gordian creates an explicit integration candidate.

The integration candidate MUST receive its own applicable verification.

Passing component candidates MUST NOT imply passing composition.

Unresolved VCS conflicts MAY exist as intermediate integration state but MUST NOT cross the accepted-frontier gate.

## Provenance and attestation

Gordian SHOULD be projectable into W3C PROV concepts:

```text
Artifact / SpecRevision / Evidence -> Entity
ExecutionAttempt / VerificationRun -> Activity
Human / software worker / service  -> Agent
```

Attestations SHOULD preserve subject, predicate type, actor, activity, materials, products, byproducts, resolved dependencies, environment, and signer/identity information in a form compatible with in-toto/SLSA concepts where applicable.

An authenticated attestation establishes provenance of a claim. It does not guarantee the claim is true.

## Authority

Capabilities, not model confidence, determine who may mutate high-consequence state.

Default separation:

```text
Worker:
  mutate assigned private execution state
  produce candidate/evidence

Coordinator:
  integrate candidates
  evaluate admission
  move accepted source frontier

DeploymentAuthority:
  mutate external deployed state
```

Worker authority MUST NOT imply accepted-frontier authority.

Coordinator authority MUST NOT imply deployment authority by default.

## Accepted frontier

A candidate may be promoted only if the admission policy establishes at least:

```text
CurrentFrontierReconciled(candidate)
NoUnresolvedConflict(candidate)
RequiredVerificationPasses(candidate)
EvidenceBoundToExactCandidate(candidate)
EvidenceFresh(candidate)
AuthorizedPromotion(actor, candidate)
```

The final mutation SHOULD use an expected-frontier/compare-and-swap precondition so concurrent coordinators cannot silently overwrite a newer accepted frontier.

## Replay and reconciliation

Canonical execution history SHOULD be append-oriented.

A deterministic projector derives query state from recorded facts.

```text
ProjectionState = project(EventHistory)
```

Effectful observations must become events before the projector consumes them.

Gordian then reconciles:

```text
PlannedWorld   = Mission Graph obligations
ObservedWorld  = evidence-supported state
```

Possible reconciliation outcomes include satisfied, need-execution, need-verification, need-repair, need-replan, blocked, and await-external-observation.

## Formal-method boundary

Formal proofs may establish properties of this model under explicit assumptions, including:

- dependency acyclicity certificates;
- dispatch requiring dependency/precondition/authorization witnesses;
- evidence mismatch invalidating formal compatibility;
- authority separation;
- acceptance carrying conflict-free/fresh/verified witnesses;
- invariant preservation across a future transition model.

They do not establish that:

- the Atom/Quark ontology is operationally optimal;
- semantic claims capture all real dependencies;
- a passing verifier proves every real-world requirement;
- the optimized Rust implementation matches the Lean model without a verified or differentially tested bridge;
- multi-agent execution improves productivity/performance.

Those claims require implementation validation and experiment.

## Research obligation

Every Gordian-specific design assumption MUST remain falsifiable.

The knowledge graph and experiment catalog must preserve evidence that supports, qualifies, or challenges these assumptions rather than allowing implementation momentum to turn hypotheses into doctrine.
