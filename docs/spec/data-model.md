# Gordian Canonical Data Model

Status: **research specification**

This document translates Mission Graph semantics into implementation-oriented identities and records. It is storage-agnostic. PostgreSQL is the expected first durable store, but storage representation MUST NOT become the ontology by accident.

## Identity and revision

Logical identity and specification revision are distinct.

```text
Atom A-73          persistent logical identity
A-73 revision R9   immutable meaning used by one period of execution
```

Mutable meaning is represented by a new immutable specification revision rather than changing the meaning of a historical revision.

Human-readable numbers are display aids. Stable internal identifiers SHOULD be opaque and MUST NOT encode hierarchy that would make replanning/descomposition moves identity-destructive.

## Core identities

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
ArtifactId
EvidenceId
AttestationId
ActorId
CapabilityId
LeaseId
EventId
SemanticResourceId
VerifierId
```

Rust SHOULD represent these as distinct newtypes rather than interchangeable strings/UUIDs.

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
  project_id,
  current_spec_revision
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
  lifecycle_state
}
```

Published PlanRevisions are immutable. Selection of the active plan is a separate fact/event and preserves history.

## Initiative

```text
Initiative {
  id,
  plan_revision_id,
  current_spec_revision
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
  initiative_id,
  current_spec_revision
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

A Quark belongs to exactly one Atom. Global hard dependency edges cannot target Quarks.

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

```text
HardDependency {
  depender_atom,
  prerequisite_atom,
  condition,
  plan_revision,
  provenance
}
```

The default condition is prerequisite satisfaction. Validation MUST reject cycles.

Topological order/rank is derived and may be cached or used as a proof certificate, but is not canonical meaning.

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
  metadata
}

ResourceClaim {
  atom_spec_revision,
  resource_id,
  mode: read | write | provide | require,
  confidence?,
  provenance
}
```

Claims are predictions/coordination facts, not proof of completeness.

## ExecutionAttempt

```text
ExecutionAttempt {
  id,
  subject_id,
  subject_spec_revision,
  actor_id,
  exact_base_state,
  started_at,
  finished_at?,
  outcome,
  effect_class,
  workspace_id?,
  jj_change_id?,
  candidate_id?,
  parent_attempt?
}
```

Attempt outcomes describe execution, not work meaning.

## Candidate

```text
Candidate {
  id,
  atom_id,
  atom_spec_revision,
  base_commit_id,
  jj_change_id,
  jj_commit_id,
  frozen_at,
  produced_by_attempt
}
```

Candidate is immutable. Editing the same logical Jujutsu change after freeze creates a new Candidate because the exact commit identity has changed.

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
  subject_id,
  subject_fingerprint,
  evidence_type,
  result,
  producer_attempt?,
  external_source?,
  verifier_id?,
  verifier_digest?,
  created_at,
  payload_artifact?
}
```

Fingerprint components remain separately queryable:

```text
EvidenceBinding {
  spec_revision,
  candidate_commit_id,
  input_digest,
  dependency_digest,
  environment_digest,
  verifier_digest,
  canonicalization_scheme
}
```

A single opaque digest without inspectable constituents is insufficient for debugging and provenance.

## Acceptance predicate

```text
AcceptancePredicate {
  id,
  subject_spec_revision,
  predicate_kind,
  definition,
  verifier_requirements,
  policy_identity
}
```

Initial predicate classes may include:

```text
boolean_verifier
metric_threshold
all_of
any_of
human_attestation
formal_theorem
```

Arbitrary embedded scripting SHOULD be avoided in the safety kernel; evaluators must be explicit, versioned, and observable.

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
  issued_at,
  expires_at?,
  fencing_token?,
  issuer
}
```

Prefer positive capability grants to scattered role-name conditionals.

A policy engine such as Cedar is a candidate for fine-grained authorization because it is Rust-native and has a formally verified Lean specification plus differential testing; adoption remains subject to benchmarking and threat-model evaluation rather than assumption.

## Lease

```text
Lease {
  id,
  holder_actor,
  holder_attempt,
  semantic_resource,
  mode,
  fencing_token,
  issued_at,
  expires_at,
  revoked_at?
}
```

Possible modes:

```text
read
write_shared_if_commutative
write_exclusive
```

Where external resources support it, monotonically increasing fencing tokens SHOULD prevent a paused stale lease holder from successfully writing after a newer lease has been granted.

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
```

They are rebuildable caches. Canonical fields must be reproducible from durable history and immutable records.

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

Jujutsu repositories
  evolving and exact source states
```

A Project may span multiple repositories and non-code resources, so the Mission Graph cannot be stored only inside one VCS graph.

## Transaction boundaries

Globally visible mutations require explicit transactional or compare-and-swap semantics, including:

```text
select PlanRevision
issue/revoke exclusive lease
freeze candidate
record verifier decision
move accepted frontier
revoke capability
```

Accepted-frontier promotion MUST include an expected prior frontier/version to prevent lost updates between coordinators.

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
struct CommitId(String);

enum EffectClass {
    Pure,
    Hermetic,
    ExternalRead,
    IdempotentWrite,
    CompensatableWrite,
    Irreversible,
    Judgment,
}
```

Production semantics are Rust-first. Python orchestration consumes stable Rust interfaces and must not duplicate this domain model's decision rules.
