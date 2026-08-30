# Evidence, Freshness, and Accepted-Frontier Admission

Gordian treats completion as an evidence problem rather than a mutable-status problem.

## 0. Anchor stability

This document is renumbered whenever a section is inserted, and this revision inserts three.
The numbers are for human navigation only. **Every normative rule below lives under a named
`### <Name>` subsection, and every cross-reference in this repository targets that name-anchor,
never a number-anchor.** A number-anchor is invalidated by the next insertion, and there are
cross-references to this document from six other documents and from
`formal/Gordian/Acceptance.lean`.

```text
FORBIDDEN   evidence-and-admission.md#<section-number>-<slug>
REQUIRED    evidence-and-admission.md#frontier-reconciliation
```

`scripts/check-anchor-stability.sh` greps the repository for `evidence-and-admission\.md#[0-9]`
and fails on any hit. The stable anchors this document exports are:

```text
#the-evidence-subject
#the-fingerprint
#the-freshness-predicate
#the-verified-rule
#evidence-provenance-validity
#integration-fingerprint
#integration-manifest-derivation
#frontier-reconciliation
#the-algorithm
#the-recorded-witness
#the-admission-conjuncts-defined
#crash-recovery
#frontier-divergence-reconciliation
#re-verification-policy
#batch-assembly
#admission-fairness-bound
#admission-rejection
```

## 1. Evidence subject

### The evidence subject

A verification result is meaningful only relative to the exact subject and assumptions it
evaluated. The subject is a `VerificationSubject` — either a `Candidate` or an
`IntegrationCandidate` — never an Atom and never a logical change.

```text
Subject(s, v) = {
  spec_revision,             -- for an IntegrationCandidate: the canonical sorted set below
  exact_state_id,
  input_digest,
  dependency_digest,
  environment_digest,
  verifier_digest,
  canonicalization_scheme
}
```

Seven components, with exactly the names of the seven fields of `EvidenceBinding` in
[`../spec/data-model.md` `## Evidence`](../spec/data-model.md#evidence). The subject is
parameterised by the verifier `v` because `environment_digest` and `verifier_digest` are
verifier-specific. Each component is encoded as a canonical digest under the named
`canonicalization_scheme`.

## 2. Fingerprint

### The fingerprint

```text
Fingerprint(s, v) = H(
    canonicalization_scheme
    || spec_revision(s)
    || s.exact_state_id
    || input_digest(s)
    || dependency_digest(s)
    || environment_digest(s, v)
    || verifier_digest(v)
)
```

The fingerprint is computed per `(subject, verifier)` pair. The seven hashed components are
exactly the seven fields of `EvidenceBinding` and the seven components of `Subject(s, v)`; the
correspondence is total in both directions, `scripts/check-evidence-binding.sh` asserts it across
all five sites, and a `#[test]` MUST assert it field by field.

The hash function does not create correctness. It creates a compact identity over a set of facts
whose completeness must be justified separately.

### Canonicalization requirements

The fingerprint is only stable if the encoded components are canonical.

For maps/sets:

- use deterministic key ordering;
- distinguish missing from empty when semantics differ;
- use explicit type/version tags;
- do not include volatile timestamps unless they are semantically relevant;
- normalize path/URI representations before hashing;
- record the canonicalization version **inside** the hashed preimage, not only alongside it.

The fingerprint format is versioned:

```text
gordian-evidence-v1:<digest>
```

`canonicalization_scheme` is a required, non-defaultable field on every `EvidenceBinding` and on
every subject. Changing the scheme changes every fingerprint and therefore invalidates all prior
evidence. That is the intended behaviour and MUST NOT be worked around with a compatibility shim.

## 3. Freshness predicate

### The freshness predicate

Given evidence `e`, current subject `s`, and verifier `v`:

```text
Fresh(e, s, v) :=
      e.binding.canonicalization_scheme = canonicalization_scheme(s)
  and e.binding.spec_revision           = spec_revision(s)
  and e.binding.exact_state_id          = s.exact_state_id
  and e.binding.input_digest            = input_digest(s)
  and e.binding.dependency_digest       = dependency_digest(s)
  and e.binding.environment_digest      = environment_digest(s, v)
  and e.binding.verifier_digest         = verifier_digest(v)
```

Seven conjuncts, one per `EvidenceBinding` field. `dependency_digest` and
`canonicalization_scheme` are load-bearing and were previously absent from this predicate even
though the data model carried them: without `dependency_digest`, failure mode 3 (a transitive
dependency changes but the fingerprint omits it) is undetectable; without
`canonicalization_scheme`, evidence produced under an older encoding silently compares equal.

`e.binding` is the required `EvidenceBinding` field of the `Evidence` record
([`../spec/data-model.md` `## Evidence`](../spec/data-model.md#evidence)), not a separate record
reached by a join whose key is unstated: `Fresh` dereferences it seven times, so the field must
exist on `Evidence` for this predicate to be implementable at all.

`Fresh(e, s, v)` is equivalent to `e.subject_fingerprint = Fingerprint(s, v)`. The field-by-field
form is normative because provenance and debugging need the components separately queryable; the
digest form is the permitted fast path, and a property test MUST assert the two agree on
generated inputs.

Where this specification writes `Fresh(e, Subject(s, v))` it means exactly `Fresh(e, s, v)`; the
two spellings are the same predicate.

The Lean kernel proves that a mismatch in any required field contradicts the compatibility
witness (`formal/Gordian/Evidence.lean`). That is a **structural theorem**. Whether the selected
fields are complete is a separate engineering question, tracked as
`assumption:fingerprint-completeness`.

## 4. Why exact source-state identity matters

Source adapters separate logical change identity from exact state identity.

A worker may preserve one `logical_change_id` while rewriting its contents, producing a different
`exact_state_id`.

Therefore:

```text
verification(logical_change_id = X)
```

is unsafe as the primary evidence binding.

The required relation is:

```text
logical Atom -> logical_change_id -> exact_state_id -> evidence
```

If the candidate is rewritten after verification, the old result no longer has the exact subject
identity required by the admission predicate.

Jujutsu realizes `logical_change_id` as a change ID and `exact_state_id` as a commit ID. A Git
worktree adapter realizes them per
[`../protocols/source-adapter-contract.md`](../protocols/source-adapter-contract.md). The
predicate above is adapter-neutral by construction.

## 5. Evidence classes

Not all evidence has equal semantics.

### Deterministic verifier evidence

Examples:

- type check;
- compiler success;
- unit/property test result;
- theorem check;
- static policy query;
- schema compatibility checker.

Record:

```text
verifier identity
verifier version/digest
inputs
command/config
result
artifacts/byproducts
environment identity
```

### Empirical/benchmark evidence

Examples:

- latency distribution;
- throughput;
- memory use;
- fault-recovery behavior;
- agent completion quality.

This evidence should additionally capture trial design, sample count, seeds/randomization when applicable, warmup, hardware, and uncertainty/statistical summary.

### Human judgment evidence

Examples:

- architecture review;
- risk acceptance;
- user-experience judgment.

A human attestation records identity, decision, subject, and rationale. It should not masquerade as an executable theorem.

### Model judgment evidence

An LLM or other learned evaluator is an actor producing a judgment artifact. Record model identity, configuration, prompt/context digest, and output. Model judgment should normally be treated as defeasible evidence rather than authoritative safety proof.

## 6. Provenance model

Gordian adapts W3C PROV's separation of:

```text
Entity
Activity
Agent
```

A useful mapping is:

```text
SpecRevision / Artifact / Evidence -> Entity
ExecutionAttempt / VerificationRun -> Activity
Human / Agent process / Coordinator -> Agent
```

Relations include:

```text
used
wasGeneratedBy
wasDerivedFrom
wasAssociatedWith
wasAttributedTo
wasInformedBy
```

Internally Gordian can use relational/event structures and export a PROV-compatible projection rather than making RDF the runtime storage engine.

## 7. Attestation model

in-toto and SLSA provide mature vocabulary for authenticated process evidence.

Gordian should preserve analogous distinctions:

```text
Attestation {
  subject,
  predicate_type,
  actor,
  activity,
  materials,
  products,
  byproducts,
  resolved_dependencies,
  environment,
  identity/signature
}
```

An attestation says **who/what claims what about which exact objects**. It does not independently prove the predicate true.

## 8. Required verifier set and the Verified rule

Each Atom spec revision has a verifier manifest. Its canonical record is
[`../spec/data-model.md` `## Verifier manifest`](../spec/data-model.md#verifier-manifest);
reproduced here because the aggregation rule reads it:

```text
VerifierManifest {
  required: [ VerifierManifestEntry, ... ],
  conditional: [ VerifierManifestEntry, ... ],
  aggregation_rule
}

VerifierManifestEntry {
  verifier_id,
  verifier_digest,
  compositional,      -- bool, REQUIRED, no default
  applicability?      -- predicate selecting when a conditional entry becomes required
}
```

`compositional` declares that a passing result on a component candidate remains valid on an
integration candidate that contains it. It is an **assertion by the manifest author**, not a
proof, and it is the only mechanism by which integration admission avoids re-running every
verifier. Declaring it is a falsifiable claim: `experiment:compositional-verifier-inheritance`
measures how often an inherited pass co-occurs with an integration failure, and a material rate
forces the flag's removal.

`compositional` has no default, because a defaulted `true` would silently weaken admission and a
defaulted `false` would silently make batching cost `|B|` runs. Deserializing a manifest entry
without it is an error.

### The Verified rule

For a verification subject `s` and manifest `manifest(s)`, define the **subject of record** for
each entry:

```text
subject_of(s, m) :=
  if an EvidenceInherited event exists for (s, m)
    then declared_by(m), the parent Candidate whose manifest contributed m
    else s
```

The discriminator is the **recorded inheritance event**, not the `compositional` flag. Keying it
on the flag was unsatisfiable: an entry marked `compositional = true` whose parent record is
absent or stale is executed on `I` by
[`### Re-verification policy`](#re-verification-policy), producing evidence with
`subject_id = I.id` — but a flag-keyed `subject_of` would still look for the record at
`declared_by(m)`, so `R(m)` would be empty, `Verified(I)` false, and re-running the verifier could
never help. The candidate, and every future batch containing it, would be permanently
unadmittable. Reading the event instead makes `subject_of` describe what happened rather than what
was declared, and the two branches of the re-verification policy map one-to-one onto the two
branches here.

For a plain `Candidate` no `EvidenceInherited` event can exist, so `subject_of(s, m) = s` always.
Inheritable entries occur only in an `integration_manifest`
([`### Integration manifest derivation`](#integration-manifest-derivation)).

```text
Verified(s) :=
  for every required entry m in manifest(s):
    let u = subject_of(s, m)
    let R(m) = { e : e.verifier_id = m.verifier_id
                     and e.subject_id = u.id
                     and e.evidence_type = verifier_result
                     and Fresh(e, u, m) }
    R(m) is non-empty
    and latest_by_recorded_at_event(R(m)).result = pass
    and no e in R(m) has e.result = fail
```

The third conjunct is the point. The previous rule — "all required verifiers have fresh passing
evidence" — is satisfied by the mere existence of one pass, so a nondeterministic verifier that
produced both a pass and a fail for the same fingerprint (this document's own failure mode 10)
would admit. A fresh failing record for a `(verifier, fingerprint)` pair is a standing objection
and MUST NOT be outvoted by a fresh passing record for the same pair.

When `R(m)` contains both a fresh pass and a fresh fail, the reconciliation result for `s` is
`NeedVerification(s)`, not a pass and not a hard `NeedRepair`: the disagreement is about the
verifier, not yet about the candidate. The runtime SHOULD record a
`VerifierNondeterminismObserved { verifier_id, fingerprint, pass_evidence, fail_evidence }` event
so verifier flakiness is measurable rather than folklore.

`Verified` is total: `R(m)` empty yields false (missing fails closed); `inconclusive` results are
members of `R(m)` but are neither a pass nor a fail, so a manifest whose latest record is
`inconclusive` also fails closed.

More expressive future aggregation policies may include threshold/quorum or environment-specific
verifiers, but MUST stay declarative and formally inspectable, and MUST preserve the
no-fresh-failure conjunct.

### Evidence provenance validity

```text
EvidenceProvenanceValid(s) :=
  for every e counted by Verified(s):
    e.producer_attempt is present
    and there exists a VerificationStarted event vs with vs.attempt = e.producer_attempt
    and vs.subject_id = e.subject_id
    and vs.subject_fingerprint = e.subject_fingerprint
    and vs.recorded_at_event < e.recorded_at_event
```

This is what makes failure mode 9 detectable. Fingerprint equality alone cannot distinguish a
genuine re-run from an evidence store that copied an old passing record under a new attempt id;
requiring a preceding `VerificationStarted` for that exact fingerprint can.

## 9. Integration evidence

Verification of individual candidates is not compositional by default.

When candidates `A` and `B` are integrated into the IntegrationCandidate `I`:

```text
Fingerprint(I) != Fingerprint(A)
Fingerprint(I) != Fingerprint(B)
```

because `I.exact_state_id` differs from both. `I` receives its own verifier manifest and its own
evidence.

### Integration fingerprint

`Fingerprint(I)` is the per-verifier fingerprint of §2 instantiated at an integration subject;
it is written `Fingerprint(I)` when the verifier is clear from context:

```text
Fingerprint(I, v) = H(
    canonicalization_scheme
    || canonical(sorted set of (atom_id, spec_revision)
                 over transitive_parent_candidates(I))
    || I.exact_state_id
    || input_digest(I)
    || dependency_digest(I)
    || environment_digest(I, v)
    || digest(I.integration_manifest)
)
```

The `spec_revision` component of a component candidate is a single revision; for an integration
candidate over parents from different Atoms with different revisions it is the canonical sorted
set of `(atom_id, spec_revision)` pairs over the transitive parent candidates. The
`verifier_digest` component is replaced by the digest of `I.integration_manifest`, because `I`'s
verifier set is derived rather than authored. Both substitutions keep the component count at
seven, so `Fresh` and `EvidenceBinding` are unchanged.

### Integration manifest derivation

```text
integration_manifest(I).required =
      union over p in transitive_parent_candidates(I) of
          manifest(p.atom_spec_revision).required
  ∪ project_integration_verifiers(I.plan_revision)
  ∪ { entry in manifest(p.atom_spec_revision).conditional
      : applicability(entry) holds on I }
```

`project_integration_verifiers` is read from `I.plan_revision` — the plan frozen on the record —
and **never** from the active plan `AP`. Reading `AP` would make `integration_manifest(I)`, its
digest, and therefore `Fingerprint(I, v)` change the moment a `PlanSelected` event lands, so a
replay of the same history would compute a different fingerprint for an already-admitted
integration, `Fresh` would be false for every record counted at that admission, and every
`SatisfactionRecord.evidence_set` written there would reference evidence that no longer verifies.
The projector tests MUST assert that replaying across a `PlanSelected` reproduces every historical
fingerprint byte for byte.

with the aggregation rule of §8. Each entry carried in from a parent `p` is tagged:

```text
inheritable(entry)  :=  entry.compositional = true
declared_by(entry)  :=  p, the parent Candidate whose manifest contributed it
```

Entries from `project_integration_verifiers(AP)` are never inheritable and always run on `I`.
`project_integration_verifiers(AP)` is the plan-scoped set of verifiers that only make sense on a
composed state — cross-component contract tests, migration ordering checks, end-to-end suites —
declared once per `PlanRevision` and frozen at publication.

**Every required verifier of every transitive parent is a member of `integration_manifest(I)`.**
Inheritable entries stay in the manifest, marked inheritable; they are not removed from it. This
is what makes `Verified(I)` cover every parent Atom's manifest, and therefore what makes
`EvidenceBoundToExactCandidate`'s inheritance clause a live check rather than dead code. If
inheritable entries were dropped from the manifest instead, no admission conjunct would ever
examine an inherited verifier, and an integration could be admitted while an Atom inside it was
unsatisfiable.

The consequence, stated as a rule the projector relies on: because
`integration_manifest(I).required` contains every required verifier of every transitive parent,
`Verified(I)` discharges every parent Atom's manifest, so a successful `admit(I, t)` appends
`AtomSatisfied` for **every** Atom with a Candidate in `transitive_parent_candidates(I)`.

Integration verification catches failures such as:

- API assumptions that conflict only when composed;
- ordering-sensitive migrations;
- resource contention;
- changed transitive dependencies;
- configuration collisions;
- integration-only test failures.

## 10. Reconciliation relation

### Frontier reconciliation

`CurrentFrontierReconciled` / `reconciled(c, t)` was previously used as the first conjunct of
admission by three documents and one Lean field, and defined by none of them. It is defined here,
once.

The relation has three arguments, all of them explicit. A two-argument spelling left `B` free,
and `CurrentFrontierReconciled(c)` left `t` free as well, so four sites wrote the predicate at
four different arities and the Lean silently dropped two conjuncts.

```text
reconciled(c: IntegrationCandidate, t: ExactStateId, B: Set<CandidateId>) :=
      c.base_frontier = t
  and t is an ancestor of c.exact_state_id
  and c.parent_candidates = B
  and conflicts(c.exact_state_id) is empty
```

`B` is threaded from the `IntegrationBatch` record (`§12`): `B = batch(c.integration_batch).members`,
which is why `IntegrationCandidate` carries `integration_batch` and why `B` is never an ambient
variable. The first conjunct of the earlier spelling — "`c` is an `IntegrationCandidate`" — is now
carried by the argument's type rather than by a runtime test.

`t` is **not** a member of `c.parent_candidates`. `parent_candidates` holds `CandidateId` and
`IntegrationCandidateId` values; `t` is an `ExactStateId` and is carried in `c.base_frontier`.
The source plane records `t` as a parent of `c.exact_state_id` — that is what
`integrate([t] ++ members)` does — and the ancestry conjunct is what checks it. Conflating the two
would make the relation type-incoherent with its own record.

A single `Candidate` is reconciled with `t` only in the degenerate case where the coordinator
built a one-member batch; in that case `c` is still an `IntegrationCandidate`, with
`c.parent_candidates = { the single candidate }` and `c.base_frontier = t`. **Admission never
takes a bare `Candidate` as its subject.** This removes the ambiguity that a bare candidate whose
base merely equals `t` might be admissible, and it makes the admitted object uniform, which is
what lets `Satisfied` be defined over one record shape.

```text
CurrentFrontierReconciled(c, t) := reconciled(c, t, batch(c.integration_batch).members)
```

evaluated against the frontier `t` read at the `ProjectionVersion` that the `WitnessGuard` of
[`### The algorithm`](#the-algorithm) commits under. Every site spells it
`CurrentFrontierReconciled(I, t)` at arity two — `mission-graph.md` `## Accepted frontier`,
`admit()`, `jujutsu-agent-protocol.md` `## 17. Acceptance condition`, and
`Acceptance.lean` — and `scripts/check-acceptance-witness.sh` compares the argument lists, not
only the names, so an arity divergence is a build failure rather than a reading exercise.

`formal/Gordian/Acceptance.lean`'s `currentFrontierReconciled` MUST carry the ancestry and batch
conjuncts as well; a Lean model weaker than the prose it mirrors proves nothing about the prose.

`conflicts` is the source-adapter operation of
[`../protocols/source-adapter-contract.md`](../protocols/source-adapter-contract.md); an empty
result discharges the structural half of `NoUnresolvedConflict`.

`formal/Gordian/Acceptance.lean`'s `currentFrontierReconciled` carries a comment naming this
anchor. `docs/spec/mission-graph.md` `## Accepted frontier` and
`docs/protocols/jujutsu-agent-protocol.md` `## 17. Acceptance condition` link to
`evidence-and-admission.md#frontier-reconciliation`.

## 11. Admission algorithm

**This section is the normative admission predicate for the repository.** Every other document
that mentions admission links to [`### The algorithm`](#the-algorithm) or to
[`### The admission conjuncts, defined`](#the-admission-conjuncts-defined), and MUST NOT restate
a conjunct list that differs from the one below.

Admission is a four-step protocol over the canonical event log, not two writes over two stores.
The log is the linearization point; the local bookmark and the published bookmark are projections
of it. Step 1 and step 4 are both conditional appends, so an intent is created once and completed
once even under concurrent coordinators.

### The algorithm

```text
function admit(I, t, actor, expected_frontier_version, witness_version):
    -- step 0: exclusion and source-plane agreement, before anything is evaluated
    require actor holds a live exclusive Lease on LeaseSubject::Coordinator(project)
    require frontier_projections_agree()      -- #frontier-divergence-reconciliation

    -- The witness, evaluated in this order against the projection at witness_version.
    -- This list is byte-identical to mission-graph.md "## Accepted frontier", to
    -- theorem-catalog.md T006, and to the field order of AcceptanceWitness in
    -- formal/Gordian/Acceptance.lean.
    require CurrentFrontierReconciled(I, t)
    require ParentsUnadmitted(I)
    require NoUnresolvedConflict(I)
    require VerifierManifestComplete(I)
    require RequiredVerificationPasses(I)
    require EvidenceBoundToExactCandidate(I)
    require EvidenceFresh(I)
    require EvidenceProvenanceValid(I)
    require LeaseValidAtFreeze(I)
    require AuthorizedPromotion(actor, I)
    -- a require that evaluates false appends AdmissionRejected and stops: #admission-rejection

    -- step 1: the compare-and-swap. The target is FrontierVersion, never EventSeq.
    append [ CandidateAdmitted {
        expected_frontier       = t,
        expected_frontier_seq   = frontier_seq(t),
        new_frontier            = I.exact_state_id,
        integration_candidate   = I.id,
        batch                   = I.parent_candidates,
        owner                   = actor,
        owner_lease             = (coordinator lease id, its FencingToken),
        witness_version         = witness_version,
        witness                 = RecordedWitness { ... },   -- #the-recorded-witness
        witness_digest          = H(witness),
        promoter                = actor
    } ]
    with precondition {
        frontier_version = expected_frontier_version,
        witness_guard    = WitnessGuard { projection_version = witness_version,
                                          scope = scope_of(I, actor) }
    }
    -- rejected on frontier_version -> another admission intervened: a CAS loss, section 13
    -- rejected on witness_guard    -> the witness went stale: re-evaluate, section 13

    -- step 2: idempotent projection of the log onto the local source plane
    move_frontier(expected = t, new = I.exact_state_id)

    -- step 3: idempotent projection onto the published frontier
    publish_frontier(expected = t, new = I.exact_state_id)

    -- step 4: completion. ONE conditional transactional append, all-or-nothing.
    append [ FrontierMoved { frontier_seq = frontier_seq(t) + 1,
                             exact_state_id = I.exact_state_id,
                             integration_candidate = I.id,
                             admitted_intent = EventSeq of the step-1 event },
             AtomSatisfied { atom, spec_revision, frontier_seq, frontier_state,
                             integration_candidate, candidate, evidence_set }
               for every Atom whose Candidate is in transitive_parent_candidates(I),
             CandidateClaimReleased { candidate }
               for every member of I.parent_candidates ]
    with precondition { frontier_version = EventSeq of the step-1 event }
```

The two conjuncts this revision adds to the pre-existing set are, written in the snake_case
spelling used by earlier revisions of this document, `require evidence_provenance_valid(c)` and
`require lease_valid_at_freeze(c)`; `ParentsUnadmitted` is the tenth. The PascalCase spelling
above is normative because it is the spelling that must match `mission-graph.md`, T006, and the
Lean witness field names.

`CandidateAdmitted` is an **intent** event and `FrontierMoved` is the **completion** event. The
event log is the linearization point; the local bookmark and `main@origin` are projections.

### Why the CAS target is FrontierVersion

Predicating step 1 on the log head would reject an admission because an unrelated worker appended
`EvidenceRecorded` or `AttemptStarted` during the assembly window; with two or three workers
active no admission would ever commit, and `MAX_ADMISSION_ATTEMPTS` could not help, because each
retry re-reads a head that has already moved again. Predicating it only on "the `EventSeq` of the
`FrontierMoved` that produced `t`" has the opposite defect: the precondition stays satisfied no
matter what else was appended, so it guards nothing but a competing frontier move and provides no
read consistency for the other nine conjuncts of the ten.

`FrontierVersion` plus a `WitnessGuard` is the pair that gets both properties: the version fails
the append exactly when another admission intervened, and the guard fails it exactly when
something the witness read has changed
([`../spec/data-model.md` `## The frontier stream and log atomicity`](../spec/data-model.md#the-frontier-stream-and-log-atomicity)).
Unrelated appends never fail an admission, and a stale witness never commits one.

**Step 4 is conditional too, and this is what makes exactly-once completion possible.** Its
precondition is that the frontier stream's newest event is still this intent, so a second
coordinator that has already completed the same intent leaves a `FrontierMoved` in the stream and
the loser's completion append is rejected rather than duplicated. Without a precondition on step 4
two coordinators completing one intent both append `FrontierMoved { frontier_seq = n + 1 }`,
which contradicts the dense, gap-free `FrontierSeq` and breaks the frontier-chain premise that
`PrerequisiteContaining` relies on. `AtomSatisfied` application is additionally idempotent per
`(atom, frontier_seq)` in the projector
([`../spec/data-model.md` `## Projection`](../spec/data-model.md#projection)), so a duplicate
delivery is a no-op rather than a second `SatisfactionRecord`.

`AtomSatisfied` is appended for every Atom with a Candidate in `transitive_parent_candidates(I)`
without further condition, because `integration_manifest(I)` contains every required verifier of
every transitive parent and `RequiredVerificationPasses(I)` has therefore already discharged each
of those manifests
([`### Integration manifest derivation`](#integration-manifest-derivation)).

### The recorded witness

`CandidateAdmitted` carries the witness itself, not only `H(witness)`:

```text
RecordedWitness {
  frontier_seq,                 -- of t
  plan_revision,                -- I.plan_revision
  batch,                        -- the member CandidateIds
  counted_evidence,             -- [(atom, verifier_id, evidence_id)] for every required verifier
                                --   of every transitive parent
  inherited,                    -- [(candidate, verifier_id, evidence_id)] licensed by an
                                --   EvidenceInherited event
  parent_fencing_tokens,        -- [(candidate, fencing_token)]
  promoter_grant,               -- CapabilityId
  witness_version               -- ProjectionVersion the conjuncts were evaluated against
}
```

A digest is one-way. `AtomSatisfied` must carry `evidence_set` — one `EvidenceId` per required
verifier — and a coordinator recovering someone else's intent cannot read that out of `H(witness)`.
Its only alternative would be to re-evaluate `Verified(I)` at recovery time, by which point an
`EvidenceRetracted`, a fresh failing record, or a lease expiry may have changed the answer; it
would then either write an `evidence_set` that does not correspond to the witness the
compare-and-swap was granted for — leaving `witness_digest` unverifiable against any
reconstructible witness — or fail to complete the intent at all and stall admission globally.
Recording the witness makes recovery a **replay of a decision** rather than a fresh decision,
which is what an intent event is for. `witness_digest` is retained so that a tampered
`RecordedWitness` is detectable.

### Why not "atomically move the frontier, then append the event"

Because those are two writes over two stores and no store can compare-and-swap the other. A crash
between them leaves `project(H)` disagreeing with the source bookmark, which violates replay
stability. Making the log append the CAS reduces the protocol to one atomic write plus an
idempotent projection.

### Crash recovery

An intent is **incomplete** when its recorded effects are not all present:

```text
incomplete(e : CandidateAdmitted) :=
      no AdmissionAborted names e
  and ( no FrontierMoved names e
        or the set of AtomSatisfied events naming e is not exactly one per Atom of
           transitive_parent_candidates(e.integration_candidate) )
```

The predicate is **not** "a `CandidateAdmitted` with no matching `FrontierMoved`". A crash between
the `FrontierMoved` and the last `AtomSatisfied` of a multi-Atom batch is invisible to that test:
recovery would find nothing to do, while Atoms whose work is already inside the accepted frontier
carry no `SatisfactionRecord`, every dependent of them is `Blocked` forever, and there is no route
back — their candidates are ancestors of the frontier, so no future `IntegrationBatch` can contain
them and no future `admit()` would emit their `AtomSatisfied`. Because step 4 is a single
transactional append the second disjunct is unreachable under a conforming log; it is stated
anyway so that a log implementation which silently degrades to per-event appends fails a recovery
test instead of stranding a subtree. The route back for an Atom that has nevertheless lost its
record is `SatisfactionRestored`
([`../spec/mission-graph.md` `### Satisfaction`](../spec/mission-graph.md#satisfaction)).

Recovery, before serving any admission, over the incomplete intents in `EventSeq` order:

1. **acquire the exclusive `LeaseSubject::Coordinator(project)` lease.** Only its holder may
   complete or abort an intent — its own or a predecessor's. Takeover is keyed on that lease
   ceasing to be live, which is an `EventSeq` comparison against `expires_at_event`, not on a
   process having started: process startup cannot distinguish "the previous coordinator is dead"
   from "the previous coordinator is in a garbage-collection pause", and treating it as
   permission is how two coordinators come to complete one intent.
2. re-drive `move_frontier(expected = e.expected_frontier, new = e.new_frontier)` and then
   `publish_frontier(expected = e.expected_frontier, new = e.new_frontier)`. Both are idempotent,
   so a re-drive after a completed move or push returns `AlreadyAtNew`. **The push is part of
   recovery, not only of the happy path**: an intent that moved the local bookmark and crashed
   before publishing would otherwise leave the published frontier permanently behind the log,
   with every worker basing on a stale frontier and every dependent failing
   `PrerequisiteContaining` for reasons no event explains.
3. append the step-4 completion transaction, taking each `AtomSatisfied.evidence_set` from the
   intent's `RecordedWitness` rather than re-evaluating `Verified(I)`
   ([`### The recorded witness`](#the-recorded-witness)), under the same
   `frontier_version` precondition, so a concurrent completion by another actor loses cleanly
   instead of duplicating.
4. only then serve new admissions.

An implementation MUST NOT begin a new admission while an incomplete intent exists.

**The abort escape is mandatory, and it rolls the source plane back.** A `CandidateAdmitted` whose
`move_frontier` or `publish_frontier` fails with a `permanent` adapter error, or which has been
re-driven `MAX_REDRIVE_ATTEMPTS` (default `5`) times without reaching `Committed` or
`AlreadyAtNew`, MUST be closed as follows, in this order:

```text
1. reset_frontier(to = e.expected_frontier)   locally, then on the published frontier
2. append AdmissionAborted { candidate_admitted_event, reason, rolled_back_to }
   together with CandidateClaimReleased for every batch member, in one transaction
```

`reset_frontier` is an adapter operation with its own idempotency guarantee
([`../protocols/source-adapter-contract.md`](../protocols/source-adapter-contract.md)); it exists
because `move_frontier(expected, new)` cannot express a rollback once `expected` no longer
matches. Without the compensating move, an abort after a successful step 2 leaves the bookmark at
`I` while the log still says `t`: the next admission calls `move_frontier(expected = t, ...)`,
receives `Conflict { observed: I }` — a *permanent* condition, not a transient one — is re-driven
five times, aborts, and repeats identically forever. The escape hatch added to prevent deadlock
would itself be the deadlock, and the only stated repair would be an operator restarting the
coordinator, in a system whose goal is that no operator is present.

`AdmissionAborted` is the only event permitted to cancel an intent. It restores the previous
frontier expectation, returns the batch members to reconciliation, and is itself a member of the
frontier stream, so any coordinator holding a stale `FrontierVersion` expectation is forced to
re-read rather than proceeding on an expectation the abort invalidated.

### Frontier divergence reconciliation

The published frontier, the local bookmark, and `project(H).accepted_frontier` are three
representations of one value, and the log is authoritative. `frontier_projections_agree()` is
therefore a **precondition of every admission**, not a startup routine:

```text
frontier_projections_agree() :=
      local_frontier()     = project(H).accepted_frontier
  and published_frontier() = project(H).accepted_frontier
```

On disagreement the coordinator appends

```text
FrontierDivergenceObserved { expected, observed, source }
```

where `source` is one of the two named projections — `local_bookmark` or `published_bookmark` —
and never an unqualified "the bookmark", then re-drives that projection to the log's value with
`move_frontier` or `publish_frontier` respectively. The event log wins; neither bookmark is ever
authoritative.

The check runs at coordinator startup, before every admission, and on a timer
(`FRONTIER_DIVERGENCE_INTERVAL`, default `60s`) so that an out-of-band bookmark move is repaired
in steady state rather than at the next restart. This is the operational form of the
canonical-holder rule in
[`../spec/invariants.md` `## Accepted-frontier linearization`](../spec/invariants.md#accepted-frontier-linearization).

### Admission rejection

A witness conjunct that evaluates false is not silence. `admit()` MUST append

```text
AdmissionRejected { subject, conjunct, detail, batch }
```

naming the first false conjunct, and then:

- release the claim on every batch member (`CandidateClaimRelease`d), and remove the rejected
  members from the admission queue;
- for a conjunct whose cause is the member itself — `LeaseValidAtFreeze`,
  `EvidenceBoundToExactCandidate`, `EvidenceProvenanceValid`, `NoUnresolvedConflict` on a parent —
  append `AttemptFailed` for the producing attempt, which returns the Atom to `Dispatchable` with
  a new attempt rather than leaving it in no state at all;
- for a conjunct whose cause is transient or coordinator-side — `CurrentFrontierReconciled`,
  `ParentsUnadmitted`, `AuthorizedPromotion` — return the members to reconciliation and count the
  event under [`### Admission fairness bound`](#admission-fairness-bound).

Without this rule a rejected candidate is a black hole. `Active(a)` already ended at
`CandidateFrozen`, so its Atom is not Active, not Satisfied, has no failed attempt, and holds a
frozen candidate that is re-drawn into batch after batch and rejected each time with no counter,
no diagnosis, and no path back to `Dispatchable` — and the worker's entire body of work is
discarded silently. `AdmissionRejected` is a member of the frontier stream, so a rejection also
invalidates stale `FrontierVersion` expectations.

### The admission conjuncts, defined

```text
CurrentFrontierReconciled(I, t) := reconciled(I, t, batch(I.integration_batch).members)
                                                           -- #frontier-reconciliation

ParentsUnadmitted(I)            := for every p in transitive_parent_candidates(I):
                                     frontier_seq(p.exact_state_id) is None
                                     and no AtomSatisfied or SatisfactionRestored event names
                                         p as its candidate
                                   and I.exact_state_id != t
                                   and frontier_seq(I.exact_state_id) is None

NoUnresolvedConflict(I)         := conflicts(I.exact_state_id) is empty
                                   and for every p in transitive_parent_candidates(I):
                                       conflicts(p.exact_state_id) is empty

VerifierManifestComplete(I)     := integration_manifest(I) is derivable
                                   and every entry names a registered VerifierId
                                   and every entry carries an explicit compositional flag
                                   and every entry carries its inheritable / declared_by tags
                                   and every transitive parent's manifest is frozen at its
                                       pinned spec revision

RequiredVerificationPasses(I)   := Verified(I)             -- #the-verified-rule

EvidenceBoundToExactCandidate(I) :=
    every evidence record counted by Verified(I) has
        subject_id = I.id                                    for a non-inheritable entry, or
        subject_id = declared_by(entry).id                   for an inheritable entry,
    and in the inheritable case an EvidenceInherited event records the inheritance
    and that entry's compositional flag is true

EvidenceFresh(I)                := for every counted record e and its entry m,
                                   Fresh(e, subject_of(I, m), m) holds

evidence_provenance_valid(c)    := as defined at #evidence-provenance-validity
EvidenceProvenanceValid(I)      := evidence_provenance_valid(I)

lease_valid_at_freeze(c)        := for c, and when c is an IntegrationCandidate for every
                                   transitive parent Candidate p:
                                     p.fencing_token equals the highest FencingToken granted for
                                     LeaseSubject::LogicalChange(p.logical_change_id) at any
                                     EventSeq <= p.frozen_at_event, and that lease satisfies
                                     live(L, p.frozen_at_event)
LeaseValidAtFreeze(I)           := lease_valid_at_freeze(I)

AuthorizedPromotion(actor, I)   := a CapabilityGrant g to actor exists with
                                   capability = move_accepted_frontier at a scope containing the
                                   Project, with live_grant(g, witness_version), and actor's Role
                                   is coordinator, and actor holds a live exclusive Lease on
                                   LeaseSubject::Coordinator(project)
```

`ParentsUnadmitted` is the conjunct that stops a candidate being admitted twice. Nothing else in
the witness asserts that `I`'s parents are unconsumed: `Candidate` carries no admitted marker, and
after a CAS loss a member returns to the queue and can be re-batched over the *new* frontier that
already contains it, at which point the merge is content-free with respect to that member, every
other conjunct passes, and the frontier advances by a step that changes nothing while a second
`SatisfactionRecord` is written for its Atom at a different `frontier_seq`. The last two clauses
also reject a `FrontierMoved` that would not change the state, which is the rule the canonical
`Frontier` table relies on to keep `ExactStateId` unique across rows.

`live(L, at_event)` and `live_grant(g, at_event)` are the `EventSeq`-denominated definitions of
[`../spec/data-model.md` `## Lease`](../spec/data-model.md#lease) and
[`../spec/mission-graph.md` `### AuthorizationValid`](../spec/mission-graph.md#authorizationvalid).
Neither reads a wall clock. `lease_valid_at_freeze` previously compared a wall-clock `expires_at`
against the `EventSeq` `p.frozen_at_event`, which is not a comparison at all — no document defines
a map from `EventSeq` to time — and made every historical candidate evaluate `false` on any
later replay.

`LeaseValidAtFreeze` is the fencing check. The source plane offers no fencing of its own, so a
paused worker whose lease expired or was superseded can still hand off a commit; without this
conjunct that candidate is admissible. It is defined over the transitive parents as well as over
`I`, because otherwise the check is bypassable by wrapping a stale candidate in an integration.

`EvidenceFresh` evaluates an inherited record against the parent it was produced for, not against
`I` — an inherited record can never be `Fresh(e, I, m)`, since `I.exact_state_id` differs from the
parent's by construction. Inheritance is licensed by the `compositional` flag, recorded by an
`EvidenceInherited` event, and by nothing else.

## 12. Integration batches

The unit of admission is an `IntegrationCandidate` `I` built over the current frontier `t` and a
batch `B` of reconciled candidates. A single candidate is the one-member batch, so there is one
admission path and not two.

```text
IntegrationBatch {
  id,
  base_frontier,
  base_frontier_seq,
  members,              -- ordered list of CandidateId
  built_integration,    -- IntegrationCandidateId
  assembled_at_event
}
```

### Why batching is mandatory, not an optimization

`admit` requires both `reconciled(c, t)` and fresh evidence on the admitted subject. Under stable
snapshots a candidate's base is `F0 != t`, so reconciling it produces a new exact state whose
evidence is stale by §3. Admitting candidates one at a time therefore forces re-reconciliation and
re-verification of every other in-flight candidate after every admission: `O(N²)` verifier runs
for `N` concurrent workers. Batching makes the cost `O(1)` integration runs per batch.

### Re-verification policy

```text
for every required entry m of integration_manifest(I):
    if inheritable(m) and declared_by(m) has a fresh passing record for m:
        count that record as evidence for m on I;
        append EvidenceInherited { integration_candidate = I, from_candidate = declared_by(m),
                                   verifier_id = m.verifier_id, evidence_id }
    else:
        run m with subject I
```

- **A component verifier result may be inherited by `I` only when its manifest entry has
  `compositional = true`. All other required verifiers of `I` re-run on `I`.**
- **Admitting `I` requires fresh evidence on `I` only, so one batch costs one integration
  verification run rather than `|B|` re-verifications.** Component evidence is never re-run
  merely because a sibling was admitted.
- Admission of `I` does not invalidate the evidence of candidates outside `B`; those candidates
  re-reconcile against the new frontier when their own batch is assembled.
- Batch membership is recorded in `IntegrationCandidate.parent_candidates`, so `B` is
  reconstructible from canonical history.

Batching is a liveness and cost mechanism. It never relaxes an admission conjunct: `admit(I, t)`
evaluates the same ten predicates for a one-member batch as for any other.

### Batch assembly

1. The coordinator selects a candidate set from the admission queue whose declared writes are
   pairwise non-interfering
   ([`../spec/mission-graph.md` `## Parallel admission`](../spec/mission-graph.md#parallel-admission)),
   respecting `MAX_BATCH_SIZE` (default `8`) and `BATCH_ASSEMBLY_WINDOW` (default `30s`).
   **Selection claims each member exclusively**: the coordinator appends
   `CandidateClaimed { candidate, integration_batch, actor }`, and the append is rejected if a
   live claim already exists for that candidate. A candidate with no live claim is not in any
   batch; a candidate with one is in exactly one. Claims are released by
   `CandidateClaimReleased`, appended in the completion transaction of `admit()`, in an
   `AdmissionAborted`, or in an `AdmissionRejected`. Without an exclusive claim two assemblies
   draw the same member, both run integration verification for it, and the loser re-batches a
   candidate the winner has already admitted.
2. Every member with `frontier_seq(member.base) < frontier_seq(t)` is reconciled onto `t` by the
   adapter's `integrate` operation.
3. `I = integrate([t] ++ members)`; `I.parent_candidates` is the member ids, `I.base_frontier` is
   `t`, `I.integration_batch` is the batch id, and `I.plan_revision` is the plan in force at
   assembly.
4. If `integrate` reports a conflict, the coordinator determines the **attributable member** and
   removes it, appending
   `IntegrationConflictObserved { integration_batch, member, conflict_area }`. Assembly then
   restarts from step 3 with the remaining members, producing a **new `IntegrationCandidate` with
   its own identity, its own `exact_state_id`, its own manifest digest, and therefore its own
   fingerprint**. What survives the removal is the remaining members' *component* evidence, which
   is bound to their own exact states, and the batch membership minus `m`; the discarded `I`'s own
   evidence does not survive and MUST NOT be counted for the new one. An earlier wording said
   removing a member "MUST NOT invalidate `I`", which contradicts this document's own fingerprint
   rule: re-integrating changes `exact_state_id`, `transitive_parent_candidates`, and
   `integration_manifest`, so every record gathered on the previous `I` is stale by
   [`### The freshness predicate`](#the-freshness-predicate). The removed member's component
   evidence is likewise untouched.
5. Assembly restarts are bounded: each restart removes exactly one member, so a batch of `k`
   members restarts at most `k - 1` times before it is a one-member batch, and `k` is bounded by
   `MAX_BATCH_SIZE`. An assembly that has exhausted its members without producing a
   conflict-free `I` ends the window with no admission and every member returned to
   reconciliation, each having gained one `IntegrationConflictObserved` — which is what makes
   [`### Admission fairness bound`](#admission-fairness-bound) escalate them instead of
   re-running the same losing assembly forever.
6. Structured repair work for a removed member follows
   `docs/protocols/jujutsu-agent-protocol.md` `## 11. Conflict handling`.

**Conflict attribution.** "Attributable to member `m`" is defined, not left to the implementation:
for each conflict region reported by `integrate`, the coordinator recomputes
`integrate([t] ++ members \ {x})` for each member `x` in ascending order of
`admission_attempts(x)`, then ascending `frozen_at_event`, and attributes the conflict to the
first `x` whose removal clears the region. If no single removal clears it, the conflict is
attributed to the member with the **highest** `admission_attempts` among those touching the
region, so that a repeatedly-conflicting candidate escalates rather than a fresh one being
sacrificed. A conflict between two members is otherwise attributed arbitrarily, and the arbitrary
choice is exactly what starves a wide refactor against a stream of small fast candidates.

Batch assembly MUST include any candidate whose `admission_attempts` has reached
`MAX_ADMISSION_ATTEMPTS` (§13), ahead of newly queued candidates, under the ordering rule stated
there.

## 13. Admission progress and fairness

### Admission fairness bound

```text
MAX_ADMISSION_ATTEMPTS   = 3     -- escalate to an exclusive batch
MAX_EXCLUSIVE_ATTEMPTS   = 3     -- then reject terminally
```

Every candidate is already a batch member — `§12` establishes that the unit of admission is an
`IntegrationCandidate` and that admission never takes a bare `Candidate`, so a one-member batch is
still a batch. A rule whose remedy is "include it in the next batch" therefore moved a candidate
from a state to the identical state: it was a no-op, and the `## Admission progress` invariant
asserted a property no mechanism delivered. The remedy has to be materially different from the
status quo, and the counter has to count the failures that actually occur.

**The counter.** `admission_attempts(c)` is the projection counting **both**

```text
AdmissionPreempted          { candidate, integration_batch, expected_frontier, observed_frontier }
IntegrationConflictObserved { integration_batch, member, conflict_area }
```

events naming `c`. Counting only preemption missed the real starvation path entirely: a wide
refactor competing with small fast candidates is removed at batch assembly by
`IntegrationConflictObserved`, never reaches the compare-and-swap, and so never increments a
preemption counter — it could be starved indefinitely while its count stayed at zero.

**Preemption is per member.** A compare-and-swap failure preempts an `IntegrationCandidate`, not
one candidate, so the coordinator appends one `AdmissionPreempted` **per member** of
`I.parent_candidates`, in one transaction, each naming that member and the shared
`integration_batch`. The event's `candidate` field is a `CandidateId` and now always has one
unambiguous value.

```text
on CAS failure or witness-guard rejection for batch B:
    append [ AdmissionPreempted { candidate = m, ... } for every m in B,
             CandidateClaimReleased { candidate = m } for every m in B ]
    return every m to reconciliation
```

**Escalation.** A candidate with `admission_attempts(c) >= MAX_ADMISSION_ATTEMPTS` is
`escalated`. An escalated candidate is admitted in an **exclusive batch**: `B = { c }`, and the
coordinator MUST NOT assemble any other batch until `c` reaches a terminal outcome. That is a
different state from ordinary batch membership — it removes every source of conflict attribution
and every competitor for the frontier — which is what converts the livelock into a bounded
queueing delay.

**Ordering when several candidates are escalated.** `MAX_BATCH_SIZE` is `8`, so "include them all
in the next batch" is unsatisfiable past eight and the starvation returns with no stated
resolution. Escalated candidates are instead served **one at a time, in strict FIFO order of the
`EventSeq` of each candidate's first `AdmissionPreempted` or `IntegrationConflictObserved`
event**, ties broken by `CandidateId`. The order is a total function of the log, so two
coordinators recovering the same queue serve it identically.

**Terminal outcome.** A candidate that has failed `MAX_EXCLUSIVE_ATTEMPTS` exclusive admissions is
closed with

```text
AdmissionRejected { subject = c, conjunct = none, reason = exhausted_admission_attempts, batch }
```

and its Atom returns to `Dispatchable` with a new attempt
([`### Admission rejection`](#admission-rejection)). Every enqueued candidate therefore reaches a
terminal outcome — admitted, aborted, or rejected — within
`MAX_ADMISSION_ATTEMPTS + MAX_EXCLUSIVE_ATTEMPTS` admissions ahead of it plus its own exclusive
window, which is the bound the safety property claims.

The corresponding safety property is
[`../spec/invariants.md` `## Admission progress`](../spec/invariants.md#admission-progress).

## 14. Authority

The authority model separates:

```text
Worker
Coordinator
DeploymentAuthority
```

Default permissions:

```text
Worker:
  mutate assigned private hypothesis
  produce candidate/evidence

Coordinator:
  integrate candidates
  evaluate admission
  move accepted frontier

DeploymentAuthority:
  mutate external deployed state
```

This is intentionally stricter than giving every agent repository credentials.

The Lean theorem proves only that the abstract Worker role lacks the abstract accepted-promotion capability. Enforcement requires OS/process credentials, remote permissions, and secret isolation.

## 15. Evidence garbage collection

Stale evidence should usually remain **historically addressable** even when it no longer satisfies current acceptance.

Do not delete it merely because it is stale.

Reasons:

- reproducibility;
- debugging regressions;
- auditing verifier drift;
- understanding why a previous candidate was accepted;
- learning conflict/failure priors.

Storage policy can tier old byproducts separately while retaining immutable metadata and digests.

## 16. Failure modes to test

1. Candidate verified, then rewritten before admission.
2. Spec revision changes after verifier success.
3. Transitive dependency changes but the fingerprint omits it.
4. Environment changes but is incorrectly declared irrelevant.
5. Verifier binary changes without verifier-digest invalidation.
6. Coordinator verifies frontier `t`, but another admission advances it before promotion.
7. Signed attestation exists for the wrong candidate digest.
8. Worker attempts to move the accepted frontier directly.
9. Evidence store duplicates an old passing record under a new attempt identifier —
   caught by `EvidenceProvenanceValid`.
10. A nondeterministic verifier produces conflicting results for the same fingerprint —
    caught by the no-fresh-failure conjunct of `Verified`.
11. Crash between `CandidateAdmitted` and `FrontierMoved`.
12. `move_frontier` fails permanently, leaving an unmatched intent; `AdmissionAborted` must
    release it rather than deadlocking admission.
13. Source bookmark moved out of band, diverging from `project(H).accepted_frontier`.
14. A candidate is starved by repeated CAS preemption.
15. A batch member conflicts and is removed; the remaining batch must still admit.
16. A verifier declared `compositional` passes on a component and fails on the integration.
17. A paused worker whose lease was superseded hands off a candidate — prevented at the source by
    holder self-fencing (a freeze under a non-live lease is rejected) and caught at admission by
    `LeaseValidAtFreeze`.
18. A dependent Atom is dispatched against a base that does not contain its prerequisite's
    admitted state.
19. Unrelated events are appended throughout the batch assembly window; the admission MUST still
    commit. A CAS that fails here is a livelock, not a safety property.
20. A fresh failing `EvidenceRecorded`, an `EvidenceRetracted`, a `CapabilityRevoked`, a
    `LeaseRevoked`, or a `PlanSelected` lands between witness evaluation and the CAS; the
    `WitnessGuard` MUST reject the append.
21. Crash between `FrontierMoved` and the last `AtomSatisfied` of a multi-Atom batch; recovery
    MUST complete every satisfaction, and no Atom is left inside the frontier without a record.
22. A coordinator stalls in a long pause while a second coordinator completes its intent; exactly
    one `FrontierMoved { frontier_seq = n + 1 }` MUST exist, and no `SatisfactionRecord` is
    written twice.
23. `publish_frontier` fails permanently after a successful local `move_frontier`; the abort MUST
    roll the local bookmark back and the next admission MUST succeed.
24. Crash between the local move and the push; the published frontier MUST catch up without an
    operator restart.
25. A candidate is repeatedly removed from batches as the attributable conflicting member; it MUST
    reach an exclusive batch and then a terminal outcome.
26. A member is claimed by two assemblies; the second claim MUST be rejected.
27. An already-admitted candidate is re-batched; `ParentsUnadmitted` MUST reject it and no
    content-free frontier step is created.
28. A witness conjunct is false; `AdmissionRejected` MUST be appended and the Atom MUST return to
    a defined state.
29. A projection is destroyed and rebuilt from history a month later; every historical witness,
    fingerprint, and readiness answer MUST be identical, with no lease or capability expiring by
    wall clock during the replay.
30. An Atom's satisfaction is invalidated after its candidate is an ancestor of the frontier;
    `SatisfactionRestored` MUST be able to re-establish it from evidence on the current frontier.

Failure modes 11–30 exist because of this revision and MUST have fault-injection coverage before
#19 is closed. The formal model handles only some of these.
