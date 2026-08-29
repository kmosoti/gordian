# Evidence, Freshness, and Accepted-Frontier Admission

Gordian treats completion as an evidence problem rather than a mutable-status problem.

## 1. Evidence subject

A verification result is meaningful only relative to the exact subject and assumptions it evaluated.

For a candidate Atom revision `a`, define a conceptual verification subject:

```text
Subject(a) = {
  spec_revision,
  exact_candidate_commit,
  resolved_inputs,
  resolved_dependencies,
  relevant_environment,
  verifier_definition
}
```

The initial implementation may encode these components as canonical digests.

## 2. Fingerprint

```text
Fingerprint(a) = H(
    spec_revision
    || exact_candidate_commit
    || canonical(resolved_inputs)
    || canonical(resolved_dependencies)
    || canonical(relevant_environment)
    || verifier_digest
)
```

The hash function does not create correctness. It creates a compact identity over a set of facts whose completeness must be justified separately.

### Canonicalization requirements

The fingerprint is only stable if the encoded components are canonical.

For maps/sets:

- use deterministic key ordering;
- distinguish missing from empty when semantics differ;
- use explicit type/version tags;
- do not include volatile timestamps unless they are semantically relevant;
- normalize path/URI representations before hashing;
- record the canonicalization version.

A future fingerprint format should be versioned:

```text
gordian-evidence-v1:<digest>
```

## 3. Freshness predicate

Given evidence `e` and current subject `s`:

```text
Fresh(e, s) :=
  e.spec_revision       = s.spec_revision
  and e.commit_id       = s.commit_id
  and e.input_digest    = s.input_digest
  and e.environment     = s.environment
  and e.verifier_digest = s.verifier_digest
```

The Lean formal kernel proves that a mismatch in any required field contradicts this compatibility witness.

This is a **structural theorem**. Whether the selected fields are complete is a separate engineering question.

## 4. Why exact Jujutsu commit identity matters

Jujutsu separates logical change identity from exact commit identity.

A worker may preserve one change ID while rewriting its contents, producing a different commit ID.

Therefore:

```text
verification(change_id = X)
```

is unsafe as the primary evidence binding.

The required relation is:

```text
logical Atom -> logical jj change -> exact candidate commit -> evidence
```

If the candidate is rewritten after verification, the old result no longer has the exact subject identity required by the admission predicate.

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

## 8. Required verifier set

Each Atom revision has a verifier manifest:

```text
VerifierManifest {
  required: [V1, V2, ...]
  conditional: [...]
  aggregation_rule
}
```

A simple initial aggregation is conjunction:

```text
Verified(a) := all required verifiers have fresh passing evidence
```

More expressive future policies may include threshold/quorum or environment-specific verifiers, but should stay declarative and formally inspectable.

## 9. Integration evidence

Verification of individual candidates is not compositional by default.

When candidates `A` and `B` are integrated into `I`:

```text
Fingerprint(I) != Fingerprint(A)
Fingerprint(I) != Fingerprint(B)
```

`I` receives its own verifier manifest and evidence.

This catches failures such as:

- API assumptions that conflict only when composed;
- ordering-sensitive migrations;
- resource contention;
- changed transitive dependencies;
- configuration collisions;
- integration-only test failures.

## 10. Admission algorithm

Given candidate `c` and accepted frontier `t`:

```text
function admit(c, t):
    require reconciled(c, t)
    require no_unresolved_conflict(c)
    require verifier_manifest_complete(c)
    require all_required_evidence_passes(c)
    require all_required_evidence_fresh(c)
    require promoter_authorized(c)
    atomically move accepted frontier to c
    append admission event
```

The exact order matters operationally: the final frontier update should be protected by a compare-and-swap/version precondition so a concurrently advanced frontier cannot be silently overwritten.

Conceptually:

```text
CAS(frontier, expected=t, new=c)
```

If the CAS fails, the candidate returns to reconciliation rather than forcing the update.

## 11. Authority

The v0 model separates:

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

## 12. Evidence garbage collection

Stale evidence should usually remain **historically addressable** even when it no longer satisfies current acceptance.

Do not delete it merely because it is stale.

Reasons:

- reproducibility;
- debugging regressions;
- auditing verifier drift;
- understanding why a previous candidate was accepted;
- learning conflict/failure priors.

Storage policy can tier old byproducts separately while retaining immutable metadata and digests.

## 13. Failure modes to test

1. Candidate verified, then rewritten before admission.
2. Spec revision changes after verifier success.
3. Transitive dependency changes but fingerprint omits it.
4. Environment changes but is incorrectly declared irrelevant.
5. Verifier binary changes without verifier-digest invalidation.
6. Coordinator verifies frontier `t`, but another admission advances to `t+1` before promotion.
7. Signed attestation exists for the wrong candidate digest.
8. Worker attempts to move the accepted frontier directly.
9. Evidence store duplicates an old passing record under a new attempt identifier.
10. A nondeterministic verifier produces conflicting results for the same fingerprint.

The formal model handles only some of these. Fault-injection and implementation tests must cover the rest.
