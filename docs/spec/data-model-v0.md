# Gordian Data Model v0

Status: **research specification / unstable**

This document translates Mission Graph concepts into an implementation-oriented canonical model. It is deliberately storage-agnostic: PostgreSQL is the expected first persistence layer, but the domain semantics must not depend on one database representation.

## 1. Identity rule

Logical identity and revision identity are separate.

A logical object survives revisions:

```text
Mission M-17
Atom A-73
```

A specification revision is immutable:

```text
M-17@3
A-73@7
```

The core rule is:

> Mutable meaning is represented by a new immutable revision, not by silently changing the meaning of an old revision.

## 2. Core identifiers

Initial strongly typed identifiers:

```text
ProjectId
MissionId
PlanRevisionId
InitiativeId
AtomId
QuarkId
SpecRevisionId
AttemptId
ArtifactId
EvidenceId
AttestationId
ActorId
CapabilityId
LeaseId
EventId
```

Do not use human display names as stable keys.

Identifiers should be opaque to callers. Hierarchy such as `MISSION-42/ATOM-7` may be useful for display, but hierarchy encoded into an identifier makes moves/replanning unnecessarily destructive.

## 3. Logical objects and immutable specs

Suggested separation:

```text
WorkObject {
  id,
  kind,
  project_id,
  created_at,
  retired_at?
}

SpecRevision {
  id,
  object_id,
  revision_number,
  canonical_digest,
  schema_version,
  body,
  created_at,
  supersedes?
}
```

`body` may initially use structured JSON, but safety-critical fields should migrate into typed relational/domain structures once their semantics stabilize.

## 4. Mission

Logical identity:

```text
Mission {
  id,
  project_id,
  current_spec_revision
}
```

Specification:

```text
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

A Mission specification does not contain a privileged implementation strategy.

## 5. PlanRevision

```text
PlanRevision {
  id,
  mission_id,
  mission_spec_revision,
  parent_plan_revision?,
  rationale,
  created_by,
  created_at,
  state
}
```

A published PlanRevision is immutable.

Selection of a current PlanRevision is a separate fact/event and must preserve previous revisions.

## 6. Initiative

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

Initiative satisfaction is evaluated from its acceptance semantics, not inferred mechanically from every child Atom being satisfied.

## 7. Atom

```text
Atom {
  id,
  initiative_id,
  current_spec_revision
}
```

Suggested Atom specification:

```text
AtomSpec {
  objective,
  preconditions,
  declared_inputs,
  declared_outputs,
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

An Atom specification is an execution contract, not an execution record.

## 8. Quark

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

A Quark is local to exactly one Atom in v0.

Global hard-dependency edges MUST NOT target Quarks.

## 9. Relations

Do not encode all relationships as parent IDs.

Use typed edges for relations whose semantics matter independently:

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

Containment/decomposition can remain structurally typed because its allowable parent-child kinds are narrow and normative.

## 10. Hard dependency

For globally schedulable work:

```text
HardDependency {
  depender_atom,
  prerequisite_atom,
  condition = Satisfied | custom_condition,
  declared_by,
  plan_revision
}
```

Hard dependency validation must reject cycles.

A topological rank/order may be stored as a cache or validation certificate, but it is derived from the graph and not canonical meaning.

## 11. Semantic resource identity

A semantic resource should be an addressable domain entity rather than a free-form string once the vocabulary stabilizes.

Initial URI-shaped identifiers are intentionally extensible:

```text
rust-crate://core/model
type://User
api://AuthMiddleware
schema://user.identity
config://auth/token_ttl
service://identity/lookup
artifact://schema/user-v3
```

Schema:

```text
SemanticResource {
  id,
  kind,
  canonical_name,
  project_id,
  metadata
}
```

Claims:

```text
ResourceClaim {
  atom_spec_revision,
  resource_id,
  mode: read | write | provide | require,
  confidence?,
  provenance
}
```

The declaration is a scheduling input. It is not evidence that the claim is complete.

## 12. Execution attempt

```text
ExecutionAttempt {
  id,
  subject_id,
  subject_spec_revision,
  actor_id,
  base_state,
  started_at,
  finished_at?,
  outcome,
  effect_class,
  workspace_id?,
  jj_change_id?,
  candidate_commit_id?,
  parent_attempt?
}
```

Outcome is about the attempt:

```text
running
candidate_produced
failed
timed_out
abandoned
cancelled
```

No outcome field mutates the logical meaning of the Atom.

## 13. Candidate

A candidate should become explicit once a worker freezes code for verification:

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

A candidate is immutable.

Further editing produces another candidate identity, even when the Jujutsu logical change ID remains unchanged.

## 14. Artifact

```text
Artifact {
  id,
  media_type,
  digest,
  size?,
  storage_locator?,
  logical_name?,
  generated_by_attempt?,
  metadata
}
```

Prefer content addressing for immutable artifacts.

Large bytes do not belong in the relational core merely because metadata does.

## 15. Evidence

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

Verification subject fingerprint components should be separately addressable for debugging rather than stored only as one opaque digest:

```text
EvidenceBinding {
  spec_revision,
  candidate_commit_id,
  input_digest,
  dependency_digest,
  environment_digest,
  verifier_digest,
  canonicalization_version
}
```

## 16. Acceptance predicates

An acceptance predicate is versioned executable/specification data:

```text
AcceptancePredicate {
  id,
  subject_spec_revision,
  predicate_kind,
  definition,
  verifier_requirements,
  policy_version
}
```

Initial kinds might include:

```text
boolean_verifier
metric_threshold
all_of
any_of
human_attestation
formal_theorem
```

Avoid embedding arbitrary Turing-complete policy into the database at v0. Keep evaluation engines explicit and versioned.

## 17. Attestation

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
  environment,
  identity_mechanism,
  signature_or_reference,
  created_at
}
```

Attestations authenticate claims and provenance. They do not imply the claim is logically true.

## 18. Actor and capability

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

Capabilities should be positive grants rather than a large collection of role-name conditionals in application code.

The initial Worker/Coordinator/DeploymentAuthority role model is a policy projection over capabilities.

## 19. Lease

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

Lease state should be derived from issuance/revocation/expiration facts.

Wall-clock expiration alone is insufficient to stop a paused stale process from writing to an external resource; downstream fencing-token enforcement is preferred where possible.

## 20. Event

Canonical history:

```text
Event {
  id,
  event_type,
  schema_version,
  actor_id,
  subject_id?,
  causal_parent_ids,
  occurred_at,
  recorded_at,
  payload,
  payload_digest
}
```

`occurred_at` is observational metadata, not a universal ordering primitive.

Causal references and store sequence/version should determine protocol ordering where correctness depends on it.

## 21. Projection tables

Mutable query-oriented projections may include:

```text
atom_state_projection
mission_state_projection
active_attempt_projection
lease_projection
fresh_evidence_projection
ready_queue_projection
accepted_frontier_projection
```

These are disposable caches.

A rebuild from canonical objects/events must reproduce their canonical fields.

## 22. Storage split

Expected first implementation:

```text
PostgreSQL
  logical identities
  immutable spec revisions
  typed relations
  attempts
  evidence metadata
  capabilities
  events
  projections

Content-addressed artifact store
  reports
  logs
  binaries
  benchmark samples
  large evidence payloads

Jujutsu repositories
  evolving/exact code state
```

The Mission Graph must remain representable independently of any one Jujutsu repository because a Project may span repositories and non-code resources.

## 23. Transaction boundaries

Operations that change globally visible protocol state should use explicit transactional/compare-and-swap semantics.

Examples:

```text
select current PlanRevision
issue exclusive lease
freeze candidate
record verifier result
move accepted frontier
revoke capability
```

The accepted-frontier move in particular should require an expected previous frontier version so concurrent coordinators cannot silently overwrite one another.

## 24. Schema evolution

Every serialized form needs a schema version.

Migration rules:

- old immutable records remain interpretable;
- migration cannot rewrite historical meaning without retaining provenance;
- new derived fields may be rebuilt;
- changes to evidence fingerprint semantics require a new fingerprint/canonicalization version;
- a verifier must declare which schema versions it understands.

## 25. Rust mapping target

The eventual Rust domain layer should prefer newtypes and enums over unstructured strings:

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

Invalid states should become unrepresentable where doing so does not destroy evolvability.

The formal Lean layer and Rust layer should share a small semantic kernel, but should not be forced into identical representations merely for aesthetic symmetry.
