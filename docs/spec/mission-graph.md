# Mission Graph Specification

Status: **research specification**

The Mission Graph is Gordian's canonical representation of engineering intent. It specifies desired state, decomposition, causal prerequisites, contracts, constraints, and the evidence required to justify satisfaction.

It is deliberately distinct from source history, execution history, and evidence history.

Normative words `MUST`, `MUST NOT`, `SHOULD`, `SHOULD NOT`, and `MAY` describe intended substrate behavior.

Source-plane names are adapter-neutral. Jujutsu adapter: `logical_change_id` = change ID,
`exact_state_id` = commit ID; Git adapter: synthesized change identity, commit SHA.

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

A precondition MUST be an evaluable predicate over observed Project or environment state and MUST
NOT reference the identity of another Project, Mission, PlanRevision, Initiative, Atom, or Quark;
any prerequisite expressible as `Satisfied(x)` for a work object `x` MUST be declared as a hard
dependency instead. This is what keeps `PreconditionsHold` and `not Blocked` from being two names
for one check.

Plan validation rejects any `AtomSpec` precondition expression that references a work-object
identifier.

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

Allowed depender and prerequisite kinds are exactly the lists in
[`data-model.md` `### Global hard dependency target kinds`](data-model.md#global-hard-dependency-target-kinds):
**Atom** on both sides. No other kind is a global hard-dependency target. That list, the
`GloballyDependable` policy in `formal/Gordian/Graph.lean`, and `docs/formal/theorem-catalog.md`
T002 MUST name the identical set, and `scripts/check-dependency-kinds.sh` asserts it.

The default condition is `Satisfied(B)`, where `Satisfied` is the admitted-frontier predicate
defined in [`## Logical state predicates`](#logical-state-predicates) below.

Cycles MUST be rejected before scheduling, over the union of declared and derived edges.

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

Mutable status fields MUST NOT be canonical truth. Every predicate below is a total function of
the canonical event log projection and is decidable without consulting a worker, a model, or a
UI.

Every predicate name used below is defined either here or in
[Readiness predicate definitions](#readiness-predicate-definitions). No other document defines
them; other documents cite this one, and any reproduction of a block from this section MUST be
byte-identical to the original, which `scripts/check-predicate-definitions.sh` enforces.

Let `P = project(H)` be the projection of the canonical event history, `AP` the active
`PlanRevision` selected by the latest `PlanSelected` event, and
`rev(a) = PlanMembership(AP, a).pinned_spec_revision`.

### Satisfaction

Satisfaction is a property of an **admitted** frontier, not of a candidate sitting in a
workspace. A candidate that passed every verifier and was never admitted has verified nothing
about the accepted state of the project.

The following block is normative. It is reproduced byte-for-byte in
[`docs/architecture.md`](../architecture.md) `## 6. Derived state` and in the comment immediately
above `def Satisfied` in `formal/Gordian/Frontier.lean`; `scripts/check-satisfied-sync.sh`
enforces that identity and fails the build on drift.

<!-- BEGIN SATISFIED-DEF -->
```text
Satisfied(a) :=
  exists an admitted frontier F, with I = F.integration_candidate
  where candidate(a) is in transitive_parent_candidates(I)
  and for every required verifier v in manifest(a):
      if an EvidenceInherited event records v for (I, candidate(a))
        then FreshPass(v, candidate(a))
        else FreshPass(v, I)
```
<!-- END SATISFIED-DEF -->

where `candidate(a)` is the Candidate frozen for `a`; `F.integration_candidate` is the
`IntegrationCandidate` admitted at `F` ([`data-model.md` `## Frontier`](data-model.md#frontier));
`manifest(a)` is `verifier_manifest(rev(a))`; and:

```text
FreshPass(v, s) :=
      exists evidence e with e.verifier_id = v and e.result = pass
      and Fresh(e, Subject(s, v))
  and no evidence e' with e'.verifier_id = v and e'.result = fail
      and Fresh(e', Subject(s, v))
```

`Fresh` and `Subject` are defined in
[`evidence-and-admission.md` `### The freshness predicate`](../algorithms/evidence-and-admission.md#the-freshness-predicate)
and [`### The evidence subject`](../algorithms/evidence-and-admission.md#the-evidence-subject).

Both branches pass a `VerificationSubject` — a `Candidate` or an `IntegrationCandidate` — so
`FreshPass(v, .)` is type-correct in both. An earlier form of this block passed the frontier `F`
itself in the `else` branch; a frontier is not a `VerificationSubject`, has no `Subject(s, v)`
instance and no fingerprint, so that expression had no meaning in this specification's own type
system. The admitted `IntegrationCandidate` is the object that actually carries the integration's
evidence, and it is what `Verified` reads.

The branch discriminator is the **recorded `EvidenceInherited` event**, not the raw `compositional`
flag, and it is the identical discriminator used by `subject_of` in
[`### The Verified rule`](../algorithms/evidence-and-admission.md#the-verified-rule). This is what
makes the declarative definition and the `AtomSatisfied` rule the same condition: a
`compositional = true` entry whose parent record was stale, so that the verifier was re-run on
`I` under the re-verification policy, has no `EvidenceInherited` event, takes the `else` branch in
both places, and is counted on `I` in both places. Keying the branch on the flag instead made the
block demand a fresh parent record that the re-verification policy had already decided not to
use — a contradiction that left such an Atom permanently unsatisfiable.

The witness frontier is recorded, so it is unique rather than merely existential:

```text
SatisfactionRecord {
  atom,
  spec_revision,
  frontier_seq,          -- FrontierSeq of the frontier that admitted it
  frontier_state,        -- ExactStateId of that frontier
  integration_candidate, -- the IntegrationCandidate admitted at that frontier
  candidate,             -- the Atom's Candidate, in transitive_parent_candidates(I)
  evidence_set           -- one EvidenceId per required verifier
}
```

Operationally, `Satisfied(a)` is the projection lookup `P.satisfaction_index[a].is_some()`. The
projector inserts a `SatisfactionRecord` only while applying an `AtomSatisfied` event, and
`AtomSatisfied` may be appended only inside a successful `admit()` transaction
([`evidence-and-admission.md` `### The algorithm`](../algorithms/evidence-and-admission.md#the-algorithm)).
`Satisfied` therefore requires an admitted frontier by construction, and the recorded witness is
the frontier of the admission that emitted the event.

**The declarative block and the projector state the same condition, and that equality is
checked.** `admit(I, t)` appends `AtomSatisfied` for the Atoms of `transitive_parent_candidates(I)`
only after `RequiredVerificationPasses(I)` — that is, `Verified(I)` — has held, and
`integration_manifest(I).required` contains every required verifier of every transitive parent
([`### Integration manifest derivation`](../algorithms/evidence-and-admission.md#integration-manifest-derivation)).
`Verified(I)` evaluates each such verifier at `subject_of(I, m)`, which is `declared_by(m)` exactly
when an `EvidenceInherited` event exists for it and `I` otherwise — the two branches of the block
above. So the projector writes a record precisely when the block holds.
`scripts/check-satisfied-sync.sh` asserts the three copies of the block are byte-identical **and**
that the block's discriminator is the literal token `EvidenceInherited`, the same token
`subject_of` branches on, so the two cannot drift into different conditions while remaining
individually well-formed.

`SatisfactionInvalidated { atom, reason, trigger_event }` is appended when, and only when one of
the five reasons below holds. Each row names its **producer** (the component that appends it) and
its **trigger** (the event whose application obliges the producer to append it), because an event
with no producer and no trigger is a rule nothing executes:

| `reason` | Producer | Trigger | Append precondition |
| --- | --- | --- | --- |
| `spec_revision_superseded` | the projector, inside the same transactional append as the trigger | a `PlanSelected` whose new `AP` pins a `rev(a)` different from the one in `satisfaction_index[a].spec_revision` | `Satisfied(a)` holds immediately before |
| `plan_superseded` | the projector, same append as the trigger | a `PlanSelected` whose new `AP` has no `PlanMembership` for `a` | `Satisfied(a)` holds immediately before |
| `evidence_retracted` | the evidence store, same append as the trigger | an `EvidenceRetracted` naming a member of `satisfaction_index[a].evidence_set` | `Satisfied(a)` holds immediately before |
| `verifier_manifest_changed` | the projector, same append as the trigger | a `PlanSelected` under which `manifest(rev(a))` gained a required verifier with no counted record | `Satisfied(a)` holds immediately before |
| `integration_regression` | the coordinator, in the same append as the failing verification's `EvidenceRecorded` | an `EvidenceRecorded { result = fail }` on an `IntegrationCandidate` `J` for a verifier `v` **where `candidate(a)` is in `transitive_parent_candidates(J)`** and `v` is required by `manifest(a)` | `Satisfied(a)` holds immediately before |

`integration_regression` is **scoped to attributable failures**: the Atom's own candidate must be
inside the integration that failed. A failure on a batch that merely shares a verifier with `a` —
the common case, since project-level end-to-end verifiers are shared by construction — does not
invalidate `a`. Without that scope one flaky run on an unrelated batch would invalidate an
arbitrary Atom, and by the recovery rule below its dependents with it.

**Re-satisfaction.** An invalidated Atom is not dead. Its Candidate is already an ancestor of the
frontier, so it can never again be a member of an `IntegrationBatch` and no future `admit()` would
ever emit `AtomSatisfied` for it; without a second path an ordinary invalidation would
permanently `Block` its entire dependent subtree, and no `PlanRevision` would repair it.

```text
SatisfactionRestored { atom, spec_revision, frontier_seq, frontier_state,
                       integration_candidate, candidate, evidence_set }
```

is appended by the coordinator, under the same `FrontierVersion` and `WitnessGuard` preconditions
as `admit()`, when all of:

```text
an unsuperseded SatisfactionInvalidated for a is the newest satisfaction event for a
candidate(a) is in transitive_parent_candidates(F.integration_candidate) for the current F
for every required verifier v in manifest(a):
    FreshPass(v, F.integration_candidate)   -- re-verified on the CURRENT frontier
```

That is: satisfaction is re-established by fresh passing evidence for `a`'s manifest evaluated on
the current frontier, with no new candidate and no source change required. The coordinator runs
those verifiers against the current frontier state through the adapter's `verify` operation and
appends the restoration in one transactional append with the resulting `EvidenceRecorded` events.
`SatisfactionRestored` writes a `SatisfactionRecord` exactly as `AtomSatisfied` does, and is the
only other event permitted to.

Satisfaction is monotonic between invalidation events: a later admission never re-satisfies an
already-satisfied Atom, and never silently un-satisfies one.

```text
satisfaction_frontier_seq(a) := P.satisfaction_index[a].frontier_seq
```

defined exactly when `Satisfied(a)`.

### Readiness

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

The second and third clauses are why an Atom that requires an interface nobody provides is not
dispatchable. Before this rule, `Blocked` read only the hard-dependency table, so
`requires_interface` and `consumes` edges — which the data model declares and the admission
scheduler claims to evaluate — reached no predicate at all.

**Implementation rule.** Plan validation resolves exactly one
[`ProviderBinding`](data-model.md#provider-binding) per `(consumer_atom, requirement)` pair,
materializes each Atom-provider binding as one derived `HardDependency`
(`origin = derived_interface` / `derived_artifact`), and rejects publication of any plan
containing a requirement that is unbound, unprovided, or ambiguously provided. Because the
binding is unique, the existential in clauses 2 and 3 has at most one witness, so the conjunction
over the derived edges is extensionally equal to those clauses, and after publication the runtime
evaluates one loop over one edge table. The three-clause form is the semantics; the derived edge
set is the representation; the uniqueness rule is what makes them the same predicate.

**Invariant (newly required): derived-edge completeness.** For every published `PlanRevision`,
the set of derived edges materialized by validation equals the set computed by re-running
validation on the frozen plan. `scripts/check-derived-edges.sh` rebuilds it and compares.

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
  exists an ExecutionAttempt att with att.subject_id = a
  and att.outcome = running
  and no later AttemptFailed / AttemptAbandoned / AttemptCancelled
      / CandidateFrozen event names att
```

`Active` is a projection over attempt lifecycle events, not a status column.

An Atom may be `Enabled` and not `Dispatchable`: logical readiness and infrastructure
availability are different facts, and collapsing them encodes scarcity as a false dependency.

UI labels may materialize these predicates but MUST remain reproducible from canonical facts.
`docs/architecture.md` `## 6. Derived state` and `docs/algorithms/scheduling.md`
`## 1. Hard-dependency readiness` link to this section rather than defining anything.

## Readiness predicate definitions

Each predicate below is total, decidable, and defined only over named canonical records. No
definition body introduces an undefined helper name. `docs/algorithms/scheduling.md` and
`docs/architecture.md` link here instead of defining them.

### ValidSpec

Inputs: `PlanMembership(AP, a).pinned_spec_revision`, the `SpecRevision` record it names, the
`AtomSpec` body, the plan-scoped `HardDependency` table, `ProviderBinding`, `ExternalProvision`,
the registered `SemanticResource` table, the registered `VerifierId` table.

```text
ValidSpec(a) :=
  let r = PlanMembership(AP, a).pinned_spec_revision in
  r exists
  and r.object_id = a
  and canonical_digest(r.body, r.canonicalization_scheme) = r.canonical_digest
  and SpecRules(r.body) all hold
```

`SpecRules` is the closed list `V1..V9`; each is a decidable check over `AtomSpec` and the plan:

```text
V1  effect_class is one of the seven EffectClass values
V2  verifier_manifest.required is non-empty, or effect_class = judgment
V3  every entry of verifier_manifest names a registered VerifierId and carries an
    explicit compositional flag
V4  every hard_dependencies entry resolves to an Atom in AP
V5  no precondition expression references a work-object identifier
V6  every semantic_reads / semantic_writes / required_interfaces / provided_interfaces
    entry resolves to a registered SemanticResource in this Project
V7  every required_interfaces entry has exactly one ProviderBinding in AP
V8  every declared_inputs entry has exactly one ProviderBinding in AP
V9  the union of declared and derived hard-dependency edges over AP is acyclic
```

`V9` is plan-global and is evaluated once per plan publication; `ValidSpec(a)` reads its cached
rank certificate, validated edge-by-edge per T001.

### PreconditionsHold

Inputs: `AtomSpec.preconditions`, the `ObservedProjectState` projection (values recorded by
`ObservationRecorded` events only).

```text
PreconditionsHold(a) :=
  for every p in preconditions(rev(a)):
    EvalPredicate(p, ObservedProjectState) = true
```

`EvalPredicate` is the closed, versioned, non-scripting evaluator defined in
[`data-model.md` `## Acceptance predicate`](data-model.md#acceptance-predicate) over the `Expr`
grammar declared there. `ObservedProjectState` is the `observed_project_state` projection
(`data-model.md` `## Projection`), written only by `ObservationRecorded` events. A precondition
that cannot be evaluated because its observation is absent yields `false`, not `unknown`:
readiness fails closed. Because the grammar has no production naming a work object, `SpecRules`
`V5` is decided by the parser rather than by inspection.

Because preconditions may not name work objects (see [`### Atom`](#atom)), `PreconditionsHold`
and `not Blocked` are disjoint concerns by construction rather than by convention.

### CompatibleExecutorAvailable

Inputs: `AtomSpec.resource_requirements.executor_class`, `AtomSpec.effect_class`, the
`ExecutorDescriptor` registry.

```text
ExecutorDescriptor {
  id,
  executor_class,
  capabilities,             -- set of capability identifiers
  permitted_effect_classes, -- subset of EffectClass
  concurrency_limit,
  quiesced                  -- bool
}
```

```text
CompatibleExecutorAvailable(a) :=
  exists e in the ExecutorDescriptor registry where
    e.quiesced = false
    and e.executor_class = required_executor_class(rev(a))
    and required_capabilities(rev(a)) is a subset of e.capabilities
    and effect_class(rev(a)) is in e.permitted_effect_classes
    and active_attempts_on(e) < e.concurrency_limit
```

### RequiredResourcesAvailable

Inputs: `AtomSpec.resource_requirements`, the `ResourcePool` projection.

```text
ResourcePool {
  id,
  project_id,
  dimension,     -- cpu | memory | disk | token_budget | wallclock | licence | custom:<name>
  capacity,
  reserved       -- sum of live reservations
}
```

```text
RequiredResourcesAvailable(a) :=
  for every (dimension, amount) in resource_requirements(rev(a)):
    exists a ResourcePool with that dimension where
      capacity - reserved >= amount
```

Reservations are created by `ResourceReserved` events at dispatch and released by
`ResourceReleased`. The predicate reads only those events, never live host telemetry, so it stays
replay-pure.

### AuthorizationValid

Inputs: the dispatching `ActorId`, the `CapabilityGrant` table, `AtomSpec.effect_class`.

```text
live_grant(g, at_event) :=
      g.issued_at_event <= at_event
  and (g.expires_at_event is absent or at_event < g.expires_at_event)
  and (g.revoked_at_event is absent or at_event < g.revoked_at_event)

AuthorizationValid(a) :=
      exists g in CapabilityGrant where
        g.actor_id = dispatching_actor
        and g.capability = dispatch_atom
        and a is within g.scope
        and live_grant(g, evaluation_event)
  and (effect_class(rev(a)) != irreversible
       or a second grant exists with capability = perform_irreversible_effect
          covering a, live at the same evaluation_event)
```

`CapabilityGrant` carries `issued_at_event`, `expires_at_event?`, and `revoked_at_event?` as
`EventSeq` values; `CapabilityExpired { grant, at_event }` and `CapabilityRevoked { grant }` are
canonical events. **No wall clock appears here.** `evaluation_event` is the
`ProjectionVersion` the predicate is evaluated at, so replaying the same history at any later date
yields the same answer — which is what `## Replay stability` requires and what a `now` comparison
silently broke, since every historical grant is expired by the time a projection is rebuilt.

Model confidence is not an input. Capabilities, not confidence, determine who may mutate
high-consequence state.

### LeaseCompatible

Inputs: `AtomSpec.semantic_writes` / `semantic_reads`, the live `Lease` table, each resource's
`metadata.commutative_operations`.

```text
LeaseCompatible(a) :=
      for every w in semantic_writes(rev(a)):
        no live Lease L with L.subject = SemanticResource(w)
          and L.holder_attempt is not an attempt on a
          and (L.mode = write_exclusive
               or (L.mode = write_shared_if_commutative
                   and (claim_operation(a, w) is absent
                        or claim_operation(a, w) not in commutative_operations(w))))
  and for every r in semantic_reads(rev(a)):
        no live Lease L with L.subject = SemanticResource(r)
          and L.mode = write_exclusive
          and L.holder_attempt is not an attempt on a
```

`commutative_operations(x)` is `SemanticResource(x).metadata.commutative_operations`. A `Lease` is
**live** exactly as defined by `live(L, at_event)` in
[`data-model.md` `## Lease`](data-model.md#lease) — an `EventSeq` comparison against
`issued_at_event`, `expires_at_event`, and `revoked_at_event?`, never a wall-clock comparison
against `now`. `LeaseCompatible` does not itself acquire leases; acquisition happens at dispatch
and issues a strictly greater `FencingToken` for the subject.

`scripts/check-replay-purity.sh` greps the predicate bodies of `docs/spec/mission-graph.md`,
`docs/algorithms/scheduling.md`, and `docs/algorithms/evidence-and-admission.md` for the bare
token `now` and fails on a hit, so a future edit cannot quietly reintroduce a clock read into a
predicate this specification declares to be a total function of the event log.

### CurrentFrontierReconciled

Defined in
[`evidence-and-admission.md` `### Frontier reconciliation`](../algorithms/evidence-and-admission.md#frontier-reconciliation),
because its inputs are the accepted frontier and the integration batch rather than the Atom
specification. It is listed here so that all seven readiness/admission predicates named by this
specification have exactly one definition site.

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

For source-changing attempts it MUST record:

```text
workspace_id
logical_change_id
exact_state_id
base_frontier_seq
```

The adapter-neutral names are canonical; `docs/protocols/jujutsu-agent-protocol.md` maps them
onto one backend's vocabulary.

Attempt outcomes include running, candidate-produced, failed, timed-out, abandoned, and cancelled.

Attempt outcome MUST NOT mutate the semantic specification it attempted.

## Stable snapshots

A source-changing worker MUST receive an exact base state.

Gordian MUST NOT continuously rebase active worker state by default.

```text
snapshot exact admitted frontier state
        -> private execution
        -> candidate freeze
        -> reconcile into an integration batch over the current frontier
        -> integration verification
        -> promote or repair/replan
```

### PrerequisiteContaining

The exact base assigned to any `ExecutionAttempt` for Atom `b` MUST be an admitted frontier state
`F` such that for every `d` in `hard_dependencies(b)`, `Satisfied(d)` holds with witness frontier
`F'` where `F' = F` or `F'` is an ancestor of `F`.

Because admitted frontier states form a chain
([`invariants.md` `## Accepted-frontier linearization`](invariants.md#accepted-frontier-linearization)),
ancestry reduces to an integer comparison:

```text
PrerequisiteContaining(b, F) :=
  for every d in hard_dependencies(b):
    Satisfied(d)
    and satisfaction_frontier_seq(d) <= frontier_seq(F)
```

Without this rule the snapshot rule gave a worker "an exact base commit" and the protocol
snapshotted at the accepted frontier, but nothing required that base to contain the prerequisite
Atoms' admitted work. A worker for `b` could be dispatched against a frontier that does not
contain `a`'s change while `b` depends on `a`.

`PrerequisiteContaining` is checked at attempt creation and re-checked at candidate freeze,
because `hard_dependencies(b)` can gain a derived edge only through a new `PlanRevision`, which
invalidates the attempt.

The benefit of stable snapshots over continuous rebase remains an experimental hypothesis and
MUST be benchmarked against continuous-rebase alternatives (E004, see
[`../testing/statistical-contract.md`](../testing/statistical-contract.md)).

## Candidate identity

A Candidate is a frozen exact subject handed to verification.

A Candidate MUST identify:

```text
Atom
Atom spec revision
plan revision
base exact source state and its frontier sequence
logical_change_id
exact_state_id
fencing token held at freeze
producer attempt
freeze event
```

Any subsequent source mutation creates a different candidate identity for verification purposes.

An **IntegrationCandidate** is a distinct verification subject with its own fingerprint and its
own evidence, not a view over its parents. It has no single `atom_id` and no single
`atom_spec_revision`; its record is
[`data-model.md` `## Integration candidate`](data-model.md#integration-candidate) and its
fingerprint is
[`evidence-and-admission.md` `### Integration fingerprint`](../algorithms/evidence-and-admission.md#integration-fingerprint).

Passing component candidates MUST NOT imply a passing composition.

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

A candidate may be promoted only if the admission policy establishes exactly:

```text
CurrentFrontierReconciled(candidate, frontier)
ParentsUnadmitted(candidate)
NoUnresolvedConflict(candidate)
VerifierManifestComplete(candidate)
RequiredVerificationPasses(candidate)
EvidenceBoundToExactCandidate(candidate)
EvidenceFresh(candidate)
EvidenceProvenanceValid(candidate)
LeaseValidAtFreeze(candidate)
AuthorizedPromotion(actor, candidate)
```

This list restates, without altering, the normative predicate in
[`evidence-and-admission.md` `### The admission conjuncts, defined`](../algorithms/evidence-and-admission.md#the-admission-conjuncts-defined).
It is the identical set, in the identical order, as the `require` lines of `admit()` in
[`### The algorithm`](../algorithms/evidence-and-admission.md#the-algorithm), as the block under
`docs/protocols/jujutsu-agent-protocol.md` `## 17. Acceptance condition`, as the fields of
`AcceptanceWitness` in `formal/Gordian/Acceptance.lean`, and as the rows of
`docs/formal/theorem-catalog.md` T006. `scripts/check-acceptance-witness.sh` asserts that all five
sites match as ordered sequences — this block and those four — and compares the three document
sites on argument count as well as name, so a site that quietly drops an argument fails the build;
drift between them is a build failure. Previously the Lean carried five conjuncts, this section
six, and the algorithm document a different six.

`ParentsUnadmitted` is the tenth conjunct and is not decoration: without it nothing in the witness
asserts that `I`'s parents are still unadmitted, `Candidate` carries no consumed marker, and a
candidate drawn into two batches can be admitted twice — the second time contributing no content,
advancing the frontier chain by a step that changes nothing, and writing a second
`SatisfactionRecord` for the same Atom at a different `frontier_seq`, which makes
`satisfaction_frontier_seq` ambiguous for the integer comparison `PrerequisiteContaining` depends
on.

The witness MUST be evaluated against a single named projection version and committed under a
guard on that version
([`data-model.md` `## The frontier stream and log atomicity`](data-model.md#the-frontier-stream-and-log-atomicity)).
Evaluating ten conjuncts against a moving projection and guarding only the frontier admits a
candidate whose verifier failed, whose promoter's capability was revoked, or whose plan was
switched, in the window between evaluation and the append.

The subject of admission is always an `IntegrationCandidate`, including for a one-member batch:
see
[`evidence-and-admission.md` `### Frontier reconciliation`](../algorithms/evidence-and-admission.md#frontier-reconciliation).

The final mutation MUST use a compare-and-swap on **`FrontierVersion`** — the newest `EventSeq`
of the frontier stream — together with a `WitnessGuard` on the projection version the witness was
evaluated at. It is never a compare-and-swap on the source bookmark, and never on the global
event-log head, which every unrelated append advances: see
[`invariants.md` `## Accepted-frontier linearization`](invariants.md#accepted-frontier-linearization)
and [`data-model.md` `## The frontier stream and log atomicity`](data-model.md#the-frontier-stream-and-log-atomicity).


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
- dispatch requiring dependency/precondition/authorization witnesses (definitional today);
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
