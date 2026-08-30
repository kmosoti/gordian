# Gordian Canonical Data Model

Status: **research specification**

This document translates Mission Graph semantics into implementation-oriented identities and records. It is storage-agnostic. PostgreSQL is the expected first durable store, but storage representation MUST NOT become the ontology by accident.

Naming is adapter-neutral. `logical_change_id` and `exact_state_id` are the canonical source-plane
field names; no canonical record is named after one VCS. Jujutsu adapter: `logical_change_id` =
change ID, `exact_state_id` = commit ID; Git adapter: synthesized change identity, commit SHA.

## Identity and revision

Logical identity and specification revision are distinct.

```text
Atom A-73          persistent logical identity
A-73 revision R9   immutable meaning used by one period of execution
```

Mutable meaning is represented by a new immutable specification revision rather than changing the meaning of a historical revision.

Human-readable numbers are display aids. Stable internal identifiers MUST be opaque and MUST
NOT encode hierarchy, plan membership, or ordering that would make replanning/decomposition
moves identity-destructive.

## Identity kernel

```text
ProjectId
MissionId
PlanRevisionId
InitiativeId
AtomId
QuarkId
SpecRevisionId
AttemptId
CandidateId
IntegrationCandidateId
ArtifactId
EvidenceId
AttestationId
ActorId
CapabilityId
LeaseId
EventId
SemanticResourceId
VerifierId
WorkspaceId
ExternalProvisionId
```

Source-plane identities are adapter-neutral and MUST NOT be named after one VCS:

```text
LogicalChangeId   evolving implementation identity; survives content rewrite
ExactStateId      one exact immutable source state; changes on every rewrite
```

`ExactStateId` is the only identity that verification evidence may bind to.
`LogicalChangeId` is the only identity that a write lease over a source change may name.

Five further scalar types are canonical, because the admission protocol compares them:

```text
EventSeq           u64, dense, gap-free, assigned by the canonical event log on append
FrontierSeq        u64, dense, gap-free, assigned by each successful FrontierMoved event
FrontierVersion    u64, the EventSeq of the newest event in the frontier stream
ProjectionVersion  u64, the EventSeq of the projection a predicate was evaluated against
FencingToken       u64, strictly monotonic per LeaseSubject, never reset
```

Rust MUST represent every identity above as a distinct newtype. `EventSeq`, `FrontierSeq`,
`FrontierVersion`, `ProjectionVersion`, and `FencingToken` are newtypes over `u64` and MUST NOT be
interchangeable.

## The frontier stream and log atomicity

The **frontier stream** is the subsequence of the canonical event log consisting of exactly the
`CandidateAdmitted`, `FrontierMoved`, `AdmissionAborted`, and `AdmissionRejected` events of one
Project.

```text
FrontierVersion(H) := EventSeq of the newest frontier-stream event in H, or 0 if none
```

`FrontierVersion` is the **only** compare-and-swap target of admission. `EventSeq` is not, and
"the canonical event log version" is not: `EventSeq` is one global append counter over every
event kind, so comparing it would make an unrelated `EvidenceRecorded`, `AttemptStarted`, or
`LeaseGranted` append fail an admission. With two or three workers appending at any nonzero rate
no admission would ever commit — a livelock produced entirely by unrelated events. A conditional
append predicated on `FrontierVersion` fails only when another admission actually intervened, and
`AdmissionAborted` / `AdmissionRejected` are members of the stream so that a cancelled intent
also invalidates a stale expectation.

The canonical event log MUST support a **conditional transactional append**:

```text
append(events: [Event], precondition: AppendPrecondition)
    -> Committed { seqs: [EventSeq] }
     | Rejected  { observed_frontier_version, invalidating_event? }
```

- `events` is a non-empty ordered list appended **all-or-nothing**, receiving consecutive
  `EventSeq` values. There is no partially applied append: a crash leaves either the whole list
  durable or none of it. This granularity is normative because three rules depend on it — the
  `FrontierMoved` plus per-Atom `AtomSatisfied` completion step, the recovery predicate that
  reads it, and the guarantee that a `SatisfactionRecord` is never written for a proper subset of
  a batch.
- A rejected append writes nothing and returns what it observed, so the caller re-evaluates
  rather than guesses.

`AppendPrecondition` is:

```text
AppendPrecondition {
  frontier_version?,    -- reject unless FrontierVersion(H) equals this value
  witness?              -- reject if any event after witness.projection_version
                        -- invalidates witness.scope
}
```

Both fields are optional and independent; when both are present the append succeeds only if both
hold.

```text
WitnessGuard {
  projection_version,             -- ProjectionVersion the witness was evaluated against
  scope: WitnessScope
}

WitnessScope {
  integration_candidate,
  transitive_parent_candidates,   -- CandidateId set
  parent_atoms,                   -- AtomId set
  manifest_verifiers,             -- VerifierId set
  counted_evidence,               -- EvidenceId set
  promoter_grant,                 -- CapabilityId
  lease_subjects,                 -- LeaseSubject set
  plan_revision                   -- PlanRevisionId the witness was evaluated under
}
```

An event **invalidates** a `WitnessScope` when it is any of:

```text
EvidenceRecorded         whose subject_id is the integration candidate or a transitive parent
EvidenceRetracted        naming a member of counted_evidence
LeaseRevoked             naming a member of lease_subjects
LeaseExpired             naming a member of lease_subjects
CapabilityRevoked        naming promoter_grant
CapabilityExpired        naming promoter_grant
PlanSelected             selecting a plan revision other than scope.plan_revision
SatisfactionInvalidated  naming a member of parent_atoms
AtomSatisfied            naming a member of parent_atoms
FrontierMoved            (already excluded by the frontier_version precondition, listed for
                          completeness of the read set)
```

This is the read set of the admission witness, written down so that "the witness was still true
when the compare-and-swap committed" is a checkable property rather than an assumption. Without
it only one of the ten conjuncts — the one that reads the frontier — is covered by the
precondition, and a fresh verifier failure, a capability revocation, a lease revocation, or a
plan switch landing between evaluation and the append is admitted anyway.

## Logical work object

```text
WorkObject {
  id,
  kind,
  project_id,
  created_at,
  retired_at?
}
```

Specification-bearing logical objects reference immutable revisions:

```text
SpecRevision {
  id,
  object_id,
  sequence,
  schema_id,
  canonical_digest,
  supersedes?,
  body,
  provenance
}
```

Safety-critical fields SHOULD become typed structures once semantics stabilize; JSON/JSONB is acceptable for extension surfaces, not as an excuse to avoid domain modeling.

## Mission

```text
Mission {
  id,
  project_id
}

MissionSpec {
  goal,
  constraints,
  acceptance,
  scope?,
  non_goals?,
  risk_constraints?,
  resource_budget?,
  deadline?,
  priority?
}
```

A Mission spec contains no privileged implementation plan.

## PlanRevision

```text
PlanRevision {
  id,
  mission_id,
  mission_spec_revision,
  parent_plan_revision?,
  rationale,
  created_by,
  created_at,
  lifecycle_state,        -- draft | published | superseded
  published_digest?       -- canonical digest of the frozen set, present iff published
}
```

Publication freezes exactly:

```text
the member Initiative set (via PlanMembership)
the member Atom set (via PlanMembership)
the pinned_spec_revision of every member
the declared HardDependency edge set scoped to this plan revision
the ProviderBinding set resolved by plan validation
the derived HardDependency edge set materialized from those bindings
the project_integration_verifiers set for this plan revision
```

Publication does not freeze:

```text
which PlanRevision is currently selected
evidence, attempts, candidates, admissions, leases
observed project state read by preconditions
```

Changing a member's pinned spec revision inside a published plan is **forbidden**. It requires a
new `PlanRevision` identity whose `parent_plan_revision` is the current one. There is no event
that mutates a published plan; the only plan-selection event is `PlanSelected { plan_revision }`,
which changes which published plan is active and preserves history.

`published_digest` MUST be recomputable from the frozen set under the declared canonicalization
scheme. A published `PlanRevision` whose recomputed digest differs from `published_digest` is a
hard validation failure.

## Initiative

```text
Initiative {
  id
}

InitiativeSpec {
  objective,
  acceptance,
  constraints?,
  risk_notes?
}
```

Initiative satisfaction is evaluated from its own acceptance semantics.

## Atom

```text
Atom {
  id,
  project_id,
  created_at,
  retired_at?
}

AtomSpec {
  objective,
  preconditions,
  declared_inputs,
  declared_outputs,
  hard_dependencies,
  semantic_reads,
  semantic_writes,
  required_interfaces,
  provided_interfaces,
  resource_requirements,
  effect_class,
  acceptance_predicates,
  verifier_manifest
}
```

`Atom` carries no `initiative_id`; `Initiative` carries no `plan_revision_id`. Containment is a
plan-scoped fact, not an intrinsic property of the work object, because a replanning move MUST
NOT change an Atom's identity.

The following fields are removed from the canonical records. Where an implementation keeps an
"authoring head" pointer for editing convenience it MUST be annotated exactly as shown, and MUST
NOT be read by `ValidSpec`, `Fresh`, `Fingerprint`, or admission:

```text
Mission.current_spec_revision       projection of the authoring head; not canonical
Initiative.current_spec_revision    projection of the authoring head; not canonical
Atom.current_spec_revision          projection of the authoring head; not canonical
```

The only spec revision any predicate in this specification reads is
`PlanMembership.pinned_spec_revision` for the active `PlanRevision`, and
`PlanRevision.mission_spec_revision` for the Mission.

`Quark.atom_id` is retained. Quark containment is intentionally identity-bearing and out of
scope for replanning moves, consistent with the rule that a Quark belongs to exactly one Atom and
cannot be a global hard-dependency target.

## Plan membership

```text
PlanMembership {
  id,
  plan_revision,
  parent,                -- MissionId | InitiativeId
  child,                 -- InitiativeId | AtomId
  pinned_spec_revision,
  valid_from,
  provenance
}
```

Rules:

- A membership change creates a new `PlanMembership` fact inside a new `PlanRevision`. It MUST
  NOT rewrite an `Atom` or `Initiative` identity, and MUST NOT rewrite an existing
  `PlanMembership` row.
- For a given `(plan_revision, child)` there MUST be exactly one `PlanMembership`. Validation
  rejects zero and rejects two.
- Allowed `(parent.kind, child.kind)` pairs are exactly `(Mission, Initiative)` and
  `(Initiative, Atom)`.
- `pinned_spec_revision` is the spec revision this plan revision commits the child to.

## Quark

```text
Quark {
  id,
  atom_id,
  current_spec_revision
}

QuarkSpec {
  operation,
  inputs,
  outputs,
  local_preconditions,
  local_verifier?,
  effect_class
}
```

A Quark belongs to exactly one Atom. Global hard dependency edges cannot target Quarks; the
complete allowed target set is
[`## Hard dependency` `### Global hard dependency target kinds`](#global-hard-dependency-target-kinds),
which permits `Atom` and nothing else.

## Typed relations

Relationships whose semantics matter independently SHOULD be explicit typed edges:

```text
Relation {
  id,
  source,
  predicate,
  target,
  valid_from,
  valid_until?,
  declared_by,
  provenance_event
}
```

Initial predicates include:

```text
depends_on
consumes
produces
requires_interface
provides_interface
semantic_read
semantic_write
verifies
implemented_by
informed_by
```

Containment may use stricter typed parent relations because allowed parent-child kinds are narrow and normative.

## Hard dependency

### Global hard dependency target kinds

Allowed **depender** kinds:

```text
Atom
```

Allowed **prerequisite** kinds:

```text
Atom
```

Missions, Initiatives, PlanRevisions, Projects, and Quarks are not global hard-dependency
targets. An Initiative-level prerequisite is expressed by depending on the Atom(s) whose
satisfaction the Initiative's own acceptance rule requires. This keeps `Satisfied` defined at
exactly one kind and keeps `Blocked` a decidable loop over one edge table.

This one set is stated in exactly three places and MUST be identical in all three:

```text
formal/Gordian/Graph.lean            GloballyDependable
docs/spec/data-model.md              this subsection
docs/formal/theorem-catalog.md       T002
```

`scripts/check-dependency-kinds.sh` extracts all three and asserts set equality.

### The record

```text
HardDependency {
  id,
  plan_revision,
  depender_atom,
  prerequisite_atom,
  condition,             -- see below
  origin,                -- declared | derived_interface | derived_artifact
  origin_binding?,       -- the ProviderBinding id when origin is derived
  provenance
}
```

`condition` is one of:

```text
prerequisite_satisfied                       -- default
prerequisite_satisfied_and(predicate_id)     -- conjoined evaluable predicate
```

`dependency_condition(d)` is total:

```text
dependency_condition(d) :=
  Satisfied(d.prerequisite_atom)
  and (d.condition has no predicate_id or PredicateHolds(d.condition.predicate_id))
```

Validation MUST reject cycles over the union of declared and derived edges. Topological rank is
derived and may be cached or used as a proof certificate, but is not canonical meaning.

## Provider binding

A required interface or a declared input may be satisfied by more than one member of a plan. If
the plan does not say which, then "the providing Atom" is not a function, the derived edge set is
not determined, and two conforming implementations can disagree about whether an Atom is
`Blocked`. Publication therefore resolves it.

```text
ProviderBinding {
  id,
  plan_revision,
  consumer_atom,
  requirement,           -- SemanticResourceId (interface://...) or declared-input logical name
  requirement_kind,      -- required_interface | declared_input
  provider,              -- AtomId | ExternalProvisionId
  provenance
}
```

Rules:

- For every published `PlanRevision` and every `(consumer_atom, requirement)` pair drawn from
  `required_interfaces` and `declared_inputs`, there MUST be **exactly one** `ProviderBinding`.
- Validation rejects **zero** bindings — an unprovided interface or unproduced input — and
  rejects **ambiguity**: when two or more plan members provide the same interface or produce the
  same declared output, the plan MUST name the provider explicitly, and a plan that does not is
  rejected at publication.
- Each `ProviderBinding` whose `provider` is an `AtomId` materializes exactly one derived
  `HardDependency` (`origin = derived_interface` or `derived_artifact`,
  `origin_binding = this binding's id`).
- A `ProviderBinding` whose `provider` is an `ExternalProvisionId` materializes no edge; the
  requirement is discharged by the registered external provision.

Because the binding is unique, the existential "some Satisfied Atom provides `q`" has at most one
witness, and the conjunction over derived edges is **extensionally equal** to it. That equality
is what lets the runtime evaluate one loop over one edge table without changing the stated
semantics. Without uniqueness the conjunction would be strictly stronger than the existential,
and the two forms would disagree exactly when a second provider exists.

## External provision

Not every required interface or declared input is produced inside the plan. Anything supplied
from outside the plan MUST be registered, so that `ValidSpec` and `Blocked` stay total.

```text
ExternalProvision {
  project_id,
  interface_or_resource_id,
  provider_resource,
  provenance
}
```

`interface_or_resource_id` is a `SemanticResourceId` (`interface://...`) or a declared-input
logical name. `provider_resource` names a registered Project resource per #58.

An `ExternalProvision` is an explicit, auditable admission that Gordian is not verifying the
provider. It MUST NOT be created implicitly by a scheduler or by a missing-edge fallback, and it
is the only way a `required_interfaces` or `declared_inputs` entry may be discharged without a
Satisfied producing Atom.

## Semantic resources

Semantic resources use stable project-scoped identities rather than file paths alone.

Examples:

```text
rust-crate://core/model
type://User
api://AuthMiddleware
schema://user.identity
config://auth/token_ttl
service://identity/lookup
artifact://schema/user
```

```text
SemanticResource {
  id,
  kind,
  canonical_name,
  project_id,
  metadata {
    commutative_operations,   -- set of operation identifiers; MAY be empty
    ...
  }
}

ResourceClaim {
  atom_spec_revision,
  resource_id,
  mode: read | write | provide | require,
  operation?,         -- operation identifier; REQUIRED when mode = write and the attempt
                      -- intends to hold a write_shared_if_commutative lease
  confidence?,        -- consumed only as a feature of the #52 conflict predictor
  provenance
}
```

`confidence` is a real number in `[0,1]` asserted by the declaring actor. It is
consumed only as a feature of the #52 conflict predictor and MUST NOT be read by any readiness,
non-interference, lease, or admission predicate.

`commutative_operations` lives inside `SemanticResource.metadata`. It is declared explicitly by
an actor holding the `declare_resource_commutativity` capability and recorded as an
`OperationCommutativityDeclared` provenance event naming the declaring actor and its
justification. `commutative_operations` MUST NOT be inferred from declared resource
independence, which is not proof of semantic commutativity (invariants
`## Declared non-interference symmetry`).

Claims are predictions/coordination facts, not proof of completeness.

## ExecutionAttempt

```text
ExecutionAttempt {
  id,
  subject_id,
  subject_spec_revision,
  plan_revision,
  actor_id,
  exact_base_state,            -- ExactStateId; MUST satisfy PrerequisiteContaining
  base_frontier_seq,           -- FrontierSeq of exact_base_state
  started_at,
  finished_at?,
  outcome,
  effect_class,
  workspace_id?,
  logical_change_id?,
  candidate_id?,
  parent_attempt?
}
```

`exact_base_state` is constrained by **PrerequisiteContaining**
([`mission-graph.md` `## Stable snapshots`](mission-graph.md#stable-snapshots)): it MUST be an
admitted frontier state `F` such that for every `d` in `hard_dependencies(subject)`,
`Satisfied(d)` holds and `satisfaction_frontier_seq(d) <= base_frontier_seq`. The constraint is
checked at attempt creation and re-checked at candidate freeze.

Attempt outcomes describe execution, not work meaning.

## Candidate

```text
Candidate {
  id,
  atom_id,
  atom_spec_revision,
  plan_revision,
  base_exact_state_id,
  base_frontier_seq,
  logical_change_id,
  exact_state_id,
  fencing_token,               -- required
  frozen_at,
  frozen_at_event,             -- EventSeq of the CandidateFrozen event
  produced_by_attempt
}
```

`Candidate` is immutable. Editing the same logical change after freeze creates a new
`Candidate`, because `exact_state_id` has changed.

`fencing_token` is the token of the producing attempt's write lease on
`LeaseSubject::LogicalChange(logical_change_id)` at freeze time. It is recorded so that admission
can reject a candidate handed off by a paused holder whose lease was superseded. The source plane
supports no fencing of its own, so the check has to live in Gordian.

## Integration candidate

```text
IntegrationCandidate {
  id,
  plan_revision,               -- the PlanRevision this integration was built under
  base_frontier,               -- ExactStateId of the frontier this was built over
  base_frontier_seq,           -- FrontierSeq of base_frontier
  parent_candidates,           -- non-empty ordered set of CandidateId | IntegrationCandidateId
  integration_batch,           -- IntegrationBatchId whose members these are
  integration_manifest,        -- VerifierManifest, frozen at build time
  exact_state_id,
  frozen_at,
  frozen_at_event,
  produced_by                  -- ActorId of the coordinator that built it
}
```

`plan_revision` is required and is **frozen on the record**. `integration_manifest` unions in
`project_integration_verifiers`, which is a plan-scoped frozen set; deriving it from whichever
plan happens to be selected at read time would make `digest(I.integration_manifest)` — and
therefore `Fingerprint(I, v)` — change when a `PlanSelected` event lands, retroactively
un-freshening every evidence record counted at that admission and breaking replay stability inside
the projector itself. It is read from `I.plan_revision`, never from the active plan.

`parent_candidates` MUST be a non-empty set of Candidate or IntegrationCandidate ids. The
frontier this integration was built over is **not** a member of `parent_candidates`: it is an
`ExactStateId`, carried separately in `base_frontier`. The source plane records it as a parent of
`exact_state_id`; the canonical record does not conflate the two, because `parent_candidates`
holds candidate identities and `base_frontier` holds a source-state identity.

An `IntegrationCandidate` is a distinct verification subject with its own fingerprint and its own
evidence. Evidence bound to a parent candidate is not evidence about the integration candidate
unless the parent's manifest entry for that verifier declares `compositional: true`.

`transitive_parent_candidates(I)` is the least set containing every `Candidate` reachable from
`I.parent_candidates` by repeated expansion of nested `IntegrationCandidate` parents.

**Invariant (newly required): integration acyclicity.** For every `p` in
`I.parent_candidates`, `p.frozen_at_event < I.frozen_at_event`. Validation rejects any
`IntegrationCandidate` that violates it. Without this the transitive closure is not total.

## Frontier

`frontier_seq(.)`, `satisfaction_frontier_seq`, and `PrerequisiteContaining` all compare frontier
sequence numbers, `Satisfied` quantifies over "an admitted frontier `F`", and `admit()` computes
`frontier_seq(t) + 1` — but no record held a frontier, so `frontier_seq` was an undefined partial
map and `F` was a value with no shape. It is canonical, not a cache:

```text
Frontier {
  frontier_seq,            -- FrontierSeq; dense from 0, one row per successful admission
  exact_state_id,          -- ExactStateId of the admitted integration state
  integration_candidate,   -- IntegrationCandidateId admitted at this step
  admitted_at_event,       -- EventSeq of the FrontierMoved that created the row
  previous_frontier        -- FrontierSeq of the predecessor; absent only for frontier_seq = 0
}
```

Rules:

- A `Frontier` row is created only by applying a `FrontierMoved` event, and is never mutated.
- `frontier_seq` is dense and gap-free, and `previous_frontier = frontier_seq - 1` for every row
  but the first. Two rows with the same `frontier_seq` are a hard projection error, not an
  upsert.
- The state at `frontier_seq = n` MUST have the state at `n - 1` as an ancestor. This chain
  premise is what lets `PrerequisiteContaining` compare integers instead of walking ancestry.
- The same `ExactStateId` MUST NOT appear in two rows. A `FrontierMoved` that would not change
  the state is rejected, which is the structural half of `ParentsUnadmitted`
  ([`../algorithms/evidence-and-admission.md` `### The admission conjuncts, defined`](../algorithms/evidence-and-admission.md#the-admission-conjuncts-defined)).

```text
frontier_seq : ExactStateId -> Option<FrontierSeq>
```

is the lookup over this table. It is `None` for a state that was never an admitted frontier, and
every predicate that calls it fails closed on `None`. `frontier_chain` in `## Projection` is the
query projection of this table, not a second definition of it, and
`formal/Gordian/Frontier.lean`'s `structure Frontier` mirrors these five fields exactly.

## Artifact

```text
Artifact {
  id,
  media_type,
  digest,
  size?,
  storage_locator?,
  logical_name?,
  generated_by_activity?,
  metadata
}
```

Prefer content addressing for immutable artifacts. Large payloads belong in an artifact store, not inline relational rows.

## Evidence

```text
Evidence {
  id,
  subject_id,                  -- CandidateId | IntegrationCandidateId
  subject_kind,                -- candidate | integration_candidate
  subject_fingerprint,
  evidence_type,               -- verifier_result | benchmark | human_attestation
                               -- | model_judgment | external_observation
  result,                      -- pass | fail | inconclusive
  producer_attempt,            -- REQUIRED when evidence_type = verifier_result
  external_source,             -- REQUIRED when evidence_type = external_observation
  verifier_id,                 -- REQUIRED when evidence_type = verifier_result
  verifier_digest,             -- REQUIRED when evidence_type = verifier_result
  binding,                     -- EvidenceBinding; REQUIRED and non-null on every record
  recorded_at_event,           -- EventSeq of the EvidenceRecorded event; total order key
  created_at,
  payload_artifact?
}
```

`producer_attempt` is **not optional** for verifier results. Fingerprint equality alone cannot
distinguish a genuine re-run from an evidence store that duplicated an old passing record under a
new attempt identifier, so provenance is what makes that failure mode detectable.

`recorded_at_event` is the ordering key for "latest evidence". Wall-clock `created_at` MUST NOT
be used to order evidence.

Fingerprint components remain separately queryable:

```text
EvidenceBinding {
  spec_revision,
  exact_state_id,
  input_digest,
  dependency_digest,
  environment_digest,
  verifier_digest,
  canonicalization_scheme
}
```

`binding` is a **required, non-null field of `Evidence`**, not a free-floating record beside it.
`Fresh` dereferences `e.binding.spec_revision`, `e.binding.exact_state_id`, and five more, so an
`Evidence` value that does not carry an `EvidenceBinding` cannot be passed to the freshness
predicate at all; `fn fresh(e: &Evidence, s: &VerificationSubject, v: VerifierId) -> bool` is
unwritable without it. `subject_fingerprint` remains the digest fast path over the same seven
components.

`EvidenceBinding` has exactly seven fields. The freshness predicate compares exactly these seven,
the fingerprint hashes exactly these seven, and `CandidateRef` / `EvidenceRef` in
`formal/Gordian/Evidence.lean` carry exactly these seven. Adding, removing, or re-meaning a field
is a canonicalization-scheme change and MUST bump `canonicalization_scheme`, which invalidates
all prior evidence by construction. `scripts/check-evidence-binding.sh` asserts the five lists
are the same seven names with cardinality 7, **and** asserts that the `Evidence` record itself
names a `binding` field of type `EvidenceBinding` — a name-comparison across five sites would
otherwise pass while the linkage between the two records was missing entirely.

A single opaque digest without inspectable constituents is insufficient for debugging and provenance.

## Acceptance predicate

`PreconditionsHold`, `dependency_condition`, and validation rule `V5` all evaluate predicate
expressions, so the expression language must be closed, versioned, and parseable. This section
previously defined a record with an opaque `definition` field and a list of classes prefixed
"may include" — which is not a language. `EvalPredicate` and `PredicateHolds` had nothing to
evaluate, and `V5` ("reject any precondition expression that references a work-object
identifier") could not be decided without a parser for a syntax that did not exist.

```text
AcceptancePredicate {
  id,
  grammar_version,       -- REQUIRED; "gordian-predicate-v1" for this document
  class,                 -- PredicateClass; closed list
  expression,            -- Expr, stored as a parsed tree, never as free text
  provenance
}
```

```text
PredicateClass =
  | boolean_observation      -- a named observation is true
  | metric_threshold         -- a named metric compares against a literal
  | artifact_presence        -- a named declared output exists with a given digest
  | conjunction
  | disjunction
  | negation
```

The class list is **closed**. Adding a class is a `grammar_version` bump, which invalidates every
stored expression, exactly as a canonicalization-scheme change invalidates evidence.

```text
Expr :=
    Observation(key: ObservationKey)
  | Metric(key: ObservationKey, op: CmpOp, value: Literal)
  | Artifact(name: DeclaredOutputName, digest: Digest)
  | And([Expr])
  | Or([Expr])
  | Not(Expr)

CmpOp          := lt | le | eq | ne | ge | gt
Literal        := integer | decimal | boolean | string
ObservationKey := "<namespace>:<name>", namespace in { project, environment, resource }
```

There is no function call, no variable binding, no iteration, and **no production that can name a
Project, Mission, PlanRevision, Initiative, Atom, or Quark**. `V5` is therefore decidable by
construction: an expression that tries to reference a work object fails to parse, and validation
rejects it with a parse error rather than a judgement call.

```text
EvalPredicate : Expr -> ObservedProjectState -> bool
```

is total and pure:

- `Observation(k)` is true iff `S[k]` exists and is boolean `true`;
- `Metric(k, op, v)` is true iff `S[k]` exists, is numerically comparable to `v`, and the
  comparison holds;
- `Artifact(n, d)` is true iff an `Artifact` record for `n` with digest `d` exists;
- `And([])` is true and `Or([])` is false.

**A missing, absent, or type-incompatible observation yields `false`, never `unknown`.** Readiness
fails closed and evaluation never blocks.

`PredicateHolds(predicate_id)` is
`EvalPredicate(lookup(predicate_id).expression, ObservedProjectState)` — the same evaluator, named
separately only because `HardDependency.condition` stores an id rather than an inline tree.

`ObservedProjectState` is the `observed_project_state` projection of
[`## Projection`](#projection), written only by `ObservationRecorded` events, so evaluation reads
no live host telemetry and replays identically. No clock, environment variable, or filesystem
probe is reachable from this grammar.

## Verifier manifest

The manifest is a canonical record, not only an inline block in an algorithm document, because
admission, integration, and `Satisfied` all read it.

```text
VerifierManifest {
  required: [ VerifierManifestEntry, ... ],
  conditional: [ VerifierManifestEntry, ... ],
  aggregation_rule
}

VerifierManifestEntry {
  verifier_id,
  verifier_digest,
  compositional,       -- bool, REQUIRED, no default
  applicability?       -- predicate selecting when a conditional entry becomes required
}
```

`compositional = true` declares that a passing result on a component candidate remains valid on
an integration candidate that contains it. It is an **assertion by the manifest author**, not a
proof, and it is the only mechanism by which integration admission avoids re-running every
verifier. It has no default: a defaulted `true` would silently weaken admission, and a defaulted
`false` would silently make every batch cost `|B|` runs. Deserializing an entry without it is an
error.

The aggregation rule that reads this record is
[`evidence-and-admission.md` `### The Verified rule`](../algorithms/evidence-and-admission.md#the-verified-rule).

## Attestation

```text
Attestation {
  id,
  subject,
  predicate_type,
  actor_id,
  activity_id,
  materials,
  products,
  byproducts,
  resolved_dependencies,
  environment,
  identity_mechanism,
  signature_or_reference,
  created_at
}
```

Attestation authenticates provenance. It does not guarantee semantic correctness.

## Actor and capability

```text
Actor {
  id,
  kind: human | software_agent | coordinator | service,
  identity,
  metadata
}

CapabilityGrant {
  id,
  actor_id,
  capability,
  scope,
  issued_at_event,
  expires_at_event?,
  revoked_at_event?,
  issued_at,
  expires_at?,
  fencing_token?,
  issuer
}
```

`issued_at_event`, `expires_at_event?`, and `revoked_at_event?` are `EventSeq` values and are the
only liveness inputs `AuthorizationValid` reads; `issued_at` and `expires_at` are wall-clock
provenance and MUST NOT be read by any predicate
([`mission-graph.md` `### AuthorizationValid`](mission-graph.md#authorizationvalid)).

Prefer positive capability grants to scattered role-name conditionals.

A policy engine such as Cedar is a candidate for fine-grained authorization because it is Rust-native and has a formally verified Lean specification plus differential testing; adoption remains subject to benchmarking and threat-model evaluation rather than assumption.

## Lease

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

`LeaseSubject` is a three-constructor sum because Gordian needs to exclude three different
things: concurrent semantic writes to a domain resource, concurrent rewriting of one evolving
source change, and two coordinators driving the admission protocol for one Project. A source
change is not a `SemanticResource`, so before this sum the one-writer-per-change rule had no
representable subject at all; and neither the coordinator role nor the frontier was leasable, so
nothing restricted a Project to one admitting coordinator.

**Liveness is event-denominated, never wall-clock.**

```text
live(L, at_event) :=
      L.issued_at_event <= at_event
  and at_event < L.expires_at_event
  and (L.revoked_at_event is absent or at_event < L.revoked_at_event)
```

`live(L)` with no argument means `live(L, FrontierVersion(H))` at the evaluation point. Expiry
leaves a trace in canonical history: the lease authority appends
`LeaseExpired { lease, subject, at_event }` at or before `expires_at_event`, and a projector
treats a lease whose `expires_at_event` has passed as not live whether or not the event has been
appended yet — so replaying the same history always yields the same answer. `issued_at` and
`expires_at` are retained as human-readable provenance and MUST NOT be read by any predicate:
`now` appears in no readiness, lease, or admission predicate in this specification, which is what
keeps `## Replay stability` true when a projection is rebuilt a week later.

Two live `write_exclusive` Leases with the same `LeaseSubject` MUST NOT coexist. Equivalently: at
most one live `write_exclusive` Lease may have subject `LeaseSubject::LogicalChange(x)` for any
`x`, and at most one may have subject `LeaseSubject::Coordinator(p)` for any Project `p`, and
granting a second while the first is live MUST be refused.

**A holder self-fences.** An actor MUST NOT write to, or freeze a Candidate from, a subject whose
lease is not live at the moment of the write. A `CandidateFrozen` appended by an actor whose
lease on `LeaseSubject::LogicalChange(logical_change_id)` is not live at the freeze event is
rejected by the log. Without this rule a paused holder resumes past its expiry, a repair actor
has already been granted the next token, and two writers mutate one logical change — a real
double-writer that nothing catches until admission rejects the handoff one hop downstream.

A `write_shared_if_commutative` grant is permitted only when all of:

- `subject` is a `SemanticResource`; a `LogicalChange` and a `Coordinator` MUST NOT be
  shared-write leased;
- every live holder's `ResourceClaim.operation` names an operation in that resource's
  `metadata.commutative_operations`;
- the requesting claim's `operation` also names an operation in that set.

`fencing_token` is issued by the lease authority, is strictly increasing per `LeaseSubject`, and
never resets. A holder writes its token into every candidate it freezes, and admission re-checks
it (`LeaseValidAtFreeze`).

## Event

```text
Event {
  id,
  event_type,
  schema_identity,
  actor_id,
  subject_id?,
  causal_parent_ids,
  occurred_at,
  recorded_at,
  payload,
  payload_digest
}
```

Wall-clock timestamps are metadata, not a universal correctness ordering primitive. Causal references and store sequence/version determine protocol order when ordering is material.

## Projection

Mutable query projections may include:

```text
atom_state
mission_state
active_attempts
leases
fresh_evidence
ready_queue
accepted_frontier
frontier_chain
satisfaction_index
admission_queue
admission_claims
admission_attempts
observed_project_state
executor_registry
resource_pools
capability_grants
```

They are rebuildable caches. Canonical fields MUST be reproducible from durable history and
immutable records.

`accepted_frontier` is a **projection of the canonical event log**, not a second source of
truth: see [`invariants.md` `## Accepted-frontier linearization`](invariants.md#accepted-frontier-linearization).

`frontier_chain` is the projection of the canonical `Frontier` table ([`## Frontier`](#frontier)),
keyed by `FrontierSeq`, and is what `frontier_seq(exact_state_id)` reads.

`satisfaction_index` is the projection `AtomId -> Option<SatisfactionRecord>` derived by the rule
in [`mission-graph.md` `## Logical state predicates`](mission-graph.md#logical-state-predicates).
It is written **only** while applying an `AtomSatisfied` event, and the write is **idempotent per
`(atom, frontier_seq)`**: applying a second `AtomSatisfied` for a pair already present is a no-op,
and applying one for the same Atom at a different `frontier_seq` while an unsuperseded record
exists is a hard projection error rather than an overwrite.

`admission_claims` is the projection `CandidateId -> Option<(IntegrationBatchId, ActorId)>`
derived from `CandidateClaimed` / `CandidateClaimReleased` events. A candidate carries at most one
live claim, which is what stops two assemblies from drawing the same member into two batches.

`admission_attempts` is the projection `CandidateId -> u32` counting **both** `AdmissionPreempted`
and `IntegrationConflictObserved` events naming the candidate; it is never a mutable counter. It
replaces the earlier `solo_cas_failures`, which counted only preemption and therefore never
incremented on the starvation path that actually occurs.

`observed_project_state` is the projection `ObservationKey -> ObservedValue` written only by
`ObservationRecorded` events; it is the sole input of `PreconditionsHold`.
`executor_registry`, `resource_pools`, and `capability_grants` are the projections of the
`ExecutorDescriptor`, `ResourcePool`, and `CapabilityGrant` records read by
`CompatibleExecutorAvailable`, `RequiredResourcesAvailable`, and `AuthorizationValid`. All four
are listed here because a predicate that reads a table the projection list does not contain is a
predicate an implementer cannot write.

## Storage split

Expected initial implementation:

```text
PostgreSQL
  identities
  spec revisions
  typed relations
  attempts/candidates
  evidence metadata
  capabilities/leases
  events
  materialized projections

Content-addressed artifact store
  logs
  reports
  binaries
  benchmark samples
  large evidence payloads

Source repositories, through one source adapter
  evolving (LogicalChangeId) and exact (ExactStateId) source states
```

A Project may span multiple repositories and non-code resources, so the Mission Graph cannot be stored only inside one VCS graph.

## Experiment manifests

Experiment protocols and runs are files, not rows, and their canonical on-disk layout is:

```text
experiments/<experiment-id>/protocol.json
experiments/<experiment-id>/runs/<run-id>/run.json
```

`experiments/schema/` holds the `ExperimentProtocol` and `ExperimentRun` JSON Schemas that those
files validate against, and `knowledge/ontology.md` states the rule mapping a completed run to
`Result` and `Decision` nodes. The schemas, the validating command, and the retargeting of the
`Experiment` `verification[].target` paths are **G-519**, owned by #75 and #37; this section
records only the layout the canonical records refer to, and defines no experiment record here.

## Transaction boundaries

Globally visible mutations require explicit transactional or compare-and-swap semantics. The CAS
target is named per mutation:

| Mutation | CAS target | Expected value |
| --- | --- | --- |
| select PlanRevision | `(mission_id, highest PlanSelected EventSeq)` | previously observed `PlanSelected` `EventSeq` |
| issue/revoke exclusive lease | `(LeaseSubject, highest FencingToken)` | previously observed highest token |
| freeze candidate | `(LeaseSubject::LogicalChange, highest FencingToken)` plus a live-lease precondition | the token the freezing attempt holds |
| record verifier decision | none | append-only, no expected value |
| move accepted frontier | **`FrontierVersion`** | `FrontierVersion(H)` observed when the witness was evaluated |
| revoke capability | `(CapabilityId, highest revision)` | previously observed grant revision |

No row's CAS target is "the canonical event log version". `EventSeq` is a single global append
counter, so predicating any of these mutations on it makes every unrelated append a conflict:
every worker's freeze would collide with every other worker's evidence record, and no admission
would commit while any worker was active. Each row above names a target scoped to the object
actually being mutated.

Accepted-frontier promotion MUST include the expected prior `FrontierVersion` **and** the
`WitnessGuard` of the witness that authorized it
([`## The frontier stream and log atomicity`](#the-frontier-stream-and-log-atomicity)). The source
bookmark is moved **after** the log append succeeds, is idempotent, and is never a CAS target,
because two stores cannot be compare-and-swapped together.

## Schema evolution

Compatibility versions MAY exist where serialization/protocol compatibility requires them; this is distinct from naming Gordian concepts with maturity labels.

Rules:

- immutable historical records remain interpretable;
- migrations cannot silently rewrite historical meaning;
- derived fields may be rebuilt;
- fingerprint/canonicalization changes create a distinct compatibility scheme;
- verifiers declare supported serialized forms.

## Rust representation

Prefer newtypes, enums, sealed domain constructors, and typestate where they reduce invalid states without crippling evolution.

```rust
struct AtomId(Uuid);
struct SpecRevisionId(Uuid);
struct LogicalChangeId(String);
struct ExactStateId(String);
struct EventSeq(u64);
struct FrontierSeq(u64);
struct FrontierVersion(u64);
struct ProjectionVersion(u64);
struct FencingToken(u64);

enum EffectClass {
    Pure,
    Hermetic,
    ExternalRead,
    IdempotentWrite,
    CompensatableWrite,
    Irreversible,
    Judgment,
}

enum LeaseSubject {
    SemanticResource(SemanticResourceId),
    LogicalChange(LogicalChangeId),
    Coordinator(ProjectId),
}

enum VerificationSubject {
    Candidate(CandidateId),
    Integration(IntegrationCandidateId),
}

enum PredicateClass {
    BooleanObservation,
    MetricThreshold,
    ArtifactPresence,
    Conjunction,
    Disjunction,
    Negation,
}
```

Every match on `EffectClass` in the retry path MUST be exhaustive without a `_` wildcard, so that
adding a class is a compile error rather than a silent policy hole.

Production semantics are Rust-first. Python orchestration consumes stable Rust interfaces and must not duplicate this domain model's decision rules.
