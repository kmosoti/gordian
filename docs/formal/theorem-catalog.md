# Gordian Theorem Catalog

This catalog maps normative Gordian rules to their formal statements and explicitly records what each theorem does **not** prove.

The canonical proof sources live under [`../../formal/Gordian`](../../formal/Gordian).

## Verification states

Every `## T0NN` section carries a `**State:**` line drawn from this table. The state records
whether a checker ran, not what the theorem says.

| State | Meaning |
| --- | --- |
| `proof-source-present` | Lean source exists but this document is not claiming a successful checker run for the current revision yet. |
| `machine-checked` | Pinned Lean build and independent checker pass in CI with `sorry` disallowed. |
| `model-only` | Formal statement exists but implementation correspondence is not proved. |
| `planned` | Theorem has been identified but not yet encoded. |

No entry in this catalog is `machine-checked` today. Promotion to that state requires the
per-revision CI evidence artifact specified in [`proof-boundary.md`](proof-boundary.md); building
that artifact is **G-259**, owned by **#2**. Until it exists, a `machine-checked` claim here would
be unbacked, so the state is withheld.

## Theorem classes

Every `## T0NN` section also carries a `**Class:**` line. The state says whether a checker ran;
the class says how much engineering weight the proposition can carry, so a title cannot borrow
authority from a one-line projection. `docs/research/verification-strategy.md` section 3 requires
this distinction.

| Class | Meaning |
| --- | --- |
| `regression-guard` | The proof is `rfl`, `simp [<definition>]`, or case analysis over a finite enumeration. It pins a policy decision so that widening the definition breaks the build. It derives nothing not already written in the definition. |
| `structural` | The proof derives the conclusion from a computable definition or from a structure field the statement did not itself assume, so the shape of the model is load-bearing. |
| `substantive` | The proof performs induction or case analysis over an inductive relation and establishes a fact present nowhere as a hypothesis. |

Carrying the same value on the `Theorem` nodes of `knowledge/graph/60-formal.jsonld`, adding the
`content` discriminator, and auditing `formalizes` edge notes are **G-154**, **G-248**, and
**G-264**, owned by **#72**.

## Index of formal targets

One row per numbered target in `docs/research/verification-strategy.md` `### Formal targets`.
Every non-`planned` anchor resolves to a `theorem` declaration in `formal/Gordian/`.

| # | Target | T-id | Lean anchor | Owner |
| --- | --- | --- | --- | --- |
| 1 | a valid topological/rank certificate excludes hard-dependency cycles | T001 | `formal/Gordian/Graph.lean#no_dependency_cycle` | #10 |
| 2 | dispatch witnesses imply prerequisite satisfaction | T003 | `formal/Gordian/Scheduler.lean#dispatchable_implies_dependencies_satisfied` | #13 |
| 3 | dispatch witnesses imply capability, resource, authorization, and lease conditions | T003 | `formal/Gordian/Scheduler.lean#dispatchable_implies_authorization` | #21 |
| 4 | evidence with an incompatible subject fingerprint cannot establish current satisfaction | T004 | `formal/Gordian/Evidence.lean#input_mismatch_invalidates` | #15 |
| 5 | changing an exact candidate invalidates candidate-bound evidence | T004 | `formal/Gordian/Evidence.lean#exact_state_mismatch_invalidates` | #15 |
| 6 | worker capability alone cannot authorize accepted-frontier promotion | T005 | `formal/Gordian/Authority.lean#worker_cannot_promote` | #18 |
| 7 | accepted-state witnesses exclude unresolved structural conflicts | T006 | `formal/Gordian/Acceptance.lean#accepted_implies_conflict_free` | #19 |
| 8 | accepted-state witnesses require exact, fresh, passing evidence | T006 | `formal/Gordian/Acceptance.lean#accepted_implies_fresh_evidence` | #19 |
| 9 | cross-Atom hard dependencies cannot target Quarks | T002 | `formal/Gordian/Graph.lean#globally_dependable_iff_atom` | #10 |
| 10 | deterministic projection returns identical state for identical ordered event history | T008 | `formal/Gordian/Replay.lean#replay_same_history` | #12 |
| 11 | stale fencing tokens cannot mutate a lease-protected resource | T018 | `formal/Gordian/Lease.lean#superseded_holder_rejected` | #23 |
| 12 | compare-and-swap promotion rejects an obsolete frontier expectation | T020 | `planned` | #19 |
| 13 | duplicate idempotent events do not change projected state | T021 | `planned` | #12 |
| 14 | declared non-interference is symmetric for paired read/write sets | T007 | `formal/Gordian/Conflict.lean#declared_noninterference_symmetric` | #22 |

Asserting that this table's numbering is gap-free 1-14, that every non-`planned` anchor matches a
`^theorem <name>` declaration, and that `knowledge/graph/60-formal.jsonld` carries a `Theorem`
node for every T-id here is **G-221** / **G-265**, owned by **#72**.

## T001 — Ranked hard dependency acyclicity

**Lean:** `formal/Gordian/Graph.lean#no_dependency_cycle`

**State:** proof-source-present

**Class:** substantive

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
- cycle-detection or rank computation algorithm correctness: no algorithm is modelled here, only the certificate it would have to produce, so a buggy implementation that emits an unsound rank is out of scope (obligation: #10);
- workflow liveness or guaranteed completion;
- absence of semantic feedback loops outside the hard-dependency relation.

---

## T002 — Hard-dependency target kinds

**Lean:** `formal/Gordian/Graph.lean#globally_dependable_iff_atom`,
`formal/Gordian/Graph.lean#quark_not_globally_dependable`,
`formal/Gordian/Graph.lean#initiative_not_globally_dependable`,
`formal/Gordian/Graph.lean#mission_not_globally_dependable`

**State:** proof-source-present

**Class:** regression-guard

### Theorem

```text
GloballyDependable(k) <-> k = Atom
```

### The set

Allowed depender kinds: `Atom`. Allowed prerequisite kinds: `Atom`.

This one set is stated in exactly three places and MUST be identical in all three:

```text
formal/Gordian/Graph.lean            GloballyDependable
docs/spec/data-model.md              ## Hard dependency
docs/formal/theorem-catalog.md       this section
```

`scripts/check-dependency-kinds.sh` extracts the `WorkKind` constructors mapped to `True` in the
Lean, the two kind lists in `data-model.md`, and this list, and asserts set equality. Previously
the Lean admitted `mission`, `initiative`, and `atom`, the data-model record had
`depender_atom` / `prerequisite_atom` fields, and the prose excluded only Quarks — three
inconsistent statements of one set.

### Does not prove

That Atom is the optimal global dependency granularity. That is `experiment:atom-granularity`.

That an edge *record* cannot name a Quark: the graph model carries no `WorkKind` on its nodes, so
the structural edge theorem `edge_target_globally_dependable` and Quark-ownership uniqueness are
**G-222** / **G-232**, owned by **#10**. Until they land, this section is a policy-encoding
regression guard and `docs/spec/invariants.md` must not claim more (**G-229**).

---

## T003 — Dispatch carries dependency satisfaction

**Lean:** `formal/Gordian/Scheduler.lean#dispatchable_implies_dependencies_satisfied`,
`formal/Gordian/Scheduler.lean#dispatchable_implies_preconditions`,
`formal/Gordian/Scheduler.lean#dispatchable_implies_authorization`,
`formal/Gordian/Scheduler.lean#dispatchable_implies_interfaces_provided`

**State:** proof-source-present

**Class:** structural

### Theorem

```text
dispatchable(env, s) = true -> for every d in s.hardDeps, env.satisfied d = true
```

`dispatchable` is a `Bool`-valued function over concrete records, not a record of opaque `Prop`
fields: the six readiness predicates are defined in `formal/Gordian/Scheduler.lean` exactly as
`docs/spec/mission-graph.md` `## Readiness predicate definitions` defines them, and the seventh,
`currentFrontierReconciled`, is defined in `formal/Gordian/Acceptance.lean`. The theorems above
therefore unfold a computation rather than project a hypothesis field.
`grep -n ': Prop' formal/Gordian/Scheduler.lean` returns nothing.

### Engineering interpretation

If runtime dispatch is shown to refine the formal `dispatchable` predicate, the scheduler cannot
legally dispatch an Atom while a hard prerequisite is unsatisfied, a required interface is neither
provided by a Satisfied Atom nor externally provided, a precondition is false, or the
authorization grant is invalid.

### Does not prove

That runtime Rust code computes the same answer as the Lean function. The foundation conformance
seed is deliberately narrower than this composite scheduler model: `HardDependenciesAcyclic`
over raw nodes and edges, compared with #4's deterministic reference cycle-validation/topological
order algorithm by #7's runner/generator at `crates/gordian-core/tests/conformance.rs`
(**G-204**, owned by **#7**). Conformance of the full scheduler predicate remains a later Atom's
obligation.

That `env.satisfied` accurately represents real dependency completion, or that the declared
semantic resource sets read by `leaseCompatible` match real accesses.

---

## T004 — Evidence identity binding

**Lean:** `formal/Gordian/Evidence.lean#exact_state_mismatch_invalidates`,
`formal/Gordian/Evidence.lean#spec_mismatch_invalidates`,
`formal/Gordian/Evidence.lean#input_mismatch_invalidates`,
`formal/Gordian/Evidence.lean#dependency_mismatch_invalidates`,
`formal/Gordian/Evidence.lean#environment_mismatch_invalidates`,
`formal/Gordian/Evidence.lean#verifier_mismatch_invalidates`,
`formal/Gordian/Evidence.lean#canonicalization_mismatch_invalidates`

**State:** proof-source-present

**Class:** regression-guard

### Formal compatibility components

```text
spec revision
exact state id
input digest
dependency digest
environment digest
verifier digest
canonicalization scheme
```

Seven components, exactly the seven fields of `EvidenceBinding`
(`docs/spec/data-model.md` `## Evidence`), the seven components of `Subject(s, v)` and of
`Fingerprint(s, v)`, and the seven conjuncts of `Fresh`
(`docs/algorithms/evidence-and-admission.md`). `dependency digest` and `canonicalization scheme`
were previously present in the data model and absent from both the Lean `Compatible` relation and
the freshness predicate, so failure mode 3 (a transitive dependency changes) and an encoding
change were both undetectable.

`scripts/check-evidence-binding.sh` asserts that the fields of `CandidateRef` / `EvidenceRef` in
`formal/Gordian/Evidence.lean`, the `EvidenceBinding` record in `data-model.md`, the conjuncts of
`Fresh`, the components of `Fingerprint`, and this list are the **same seven names** after
snake/camel normalization. Set equality, not order; cardinality is checked too, so a silently
added eighth component fails.

For every component there is a theorem of the form:

```text
field(evidence) != field(candidate)
  -> not Compatible(evidence, candidate)
```

`commitId` and `commit_mismatch_invalidates` are gone: the adapter-neutral names are
`exactStateId` and `exact_state_mismatch_invalidates`.

### Engineering interpretation

The formal model cannot reuse evidence across a changed identity boundary when that boundary is
declared relevant.

### Does not prove

- collision resistance of a chosen digest function;
- completeness of the fingerprint inputs (`assumption:fingerprint-completeness`);
- correctness of the verifier itself;
- secure capture of environment identity;
- that "declared relevant by the verifier policy" has been formalized — `Compatible` is
  unconditional record equality over all seven components, and a relevance-conditioned relation is
  **G-238**, owned by **#15**.

---

## T005 — Worker cannot promote accepted frontier

**Lean:** `formal/Gordian/Authority.lean#worker_cannot_promote`,
`formal/Gordian/Authority.lean#coordinator_can_promote`,
`formal/Gordian/Authority.lean#worker_cannot_deploy`,
`formal/Gordian/Authority.lean#coordinator_cannot_deploy_by_default`

**State:** proof-source-present

**Class:** regression-guard

### Theorem

```text
not CanPromoteAccepted(Worker)
```

The model separately grants promotion to Coordinator and deployment only to DeploymentAuthority.
All four theorems are `simp` over a three-constructor enumeration, so they pin the policy table
and derive nothing beyond it.

### Engineering interpretation

The capability policy encodes separation of duties between execution and acceptance/deployment.

### Does not prove

Runtime credential isolation, operating-system sandboxing, or absence of implementation privilege
escalation.

That "by default" has formal content: there is no grant or delegation mechanism in the model, so
`coordinator_cannot_deploy_by_default` states an unconditional prohibition rather than a
defeasible default. A `deploy_requires_explicit_grant` theorem over a real grant relation is
**G-211**, owned by **#18**; when it lands this section becomes `**Class:** structural`.

---

## T006 — Admission witness

**Lean:** `formal/Gordian/Acceptance.lean#accepted_implies_conflict_free` and its nine siblings
`formal/Gordian/Acceptance.lean#accepted_implies_reconciled`,
`formal/Gordian/Acceptance.lean#accepted_implies_parents_unadmitted`,
`formal/Gordian/Acceptance.lean#accepted_implies_manifest_complete`,
`formal/Gordian/Acceptance.lean#accepted_implies_verified`,
`formal/Gordian/Acceptance.lean#accepted_implies_evidence_bound`,
`formal/Gordian/Acceptance.lean#accepted_implies_fresh_evidence`,
`formal/Gordian/Acceptance.lean#accepted_implies_provenance_valid`,
`formal/Gordian/Acceptance.lean#accepted_implies_lease_valid_at_freeze`,
`formal/Gordian/Acceptance.lean#accepted_implies_authorized_promoter`;
and the reconciliation lemmas
`formal/Gordian/Acceptance.lean#reconciled_requires_current_frontier`,
`formal/Gordian/Acceptance.lean#reconciled_requires_ancestry`,
`formal/Gordian/Acceptance.lean#reconciled_requires_batch`

**State:** proof-source-present

**Class:** structural

### Theorem

```text
Acceptable(f) -> conflictFree(f)
```

with one sibling theorem per conjunct.

### Witness mapping

The admission witness has exactly ten conjuncts. Each row names the normative predicate, its
arity, the `AcceptanceWitness` field that discharges it, and its defining anchor. No row may be
unmapped in either direction.

| Predicate (`mission-graph.md` `## Accepted frontier`) | `AcceptanceWitness` field | Defined at |
| --- | --- | --- |
| `CurrentFrontierReconciled` (arity 2: subject, frontier) | `currentFrontierReconciled` | `evidence-and-admission.md#frontier-reconciliation` |
| `ParentsUnadmitted` | `parentsUnadmitted` | `evidence-and-admission.md#the-admission-conjuncts-defined` |
| `NoUnresolvedConflict` | `noUnresolvedConflict` | `evidence-and-admission.md#the-admission-conjuncts-defined` |
| `VerifierManifestComplete` | `verifierManifestComplete` | `evidence-and-admission.md#the-admission-conjuncts-defined` |
| `RequiredVerificationPasses` | `requiredVerificationPasses` | `evidence-and-admission.md#the-verified-rule` |
| `EvidenceBoundToExactCandidate` | `evidenceBoundToExactCandidate` | `evidence-and-admission.md#the-admission-conjuncts-defined` |
| `EvidenceFresh` | `evidenceFresh` | `evidence-and-admission.md#the-freshness-predicate` |
| `EvidenceProvenanceValid` | `evidenceProvenanceValid` | `evidence-and-admission.md#evidence-provenance-validity` |
| `LeaseValidAtFreeze` | `leaseValidAtFreeze` | `evidence-and-admission.md#the-admission-conjuncts-defined` |
| `AuthorizedPromotion` | `authorizedPromotion` (via `CanPromoteAccepted`) | `evidence-and-admission.md#the-admission-conjuncts-defined` |

Before this revision the Lean carried five conjuncts, `mission-graph.md` six, and
`evidence-and-admission.md` a different six; the union was seven distinct names and no document
agreed with the model.

Each `AcceptanceWitness` field is the predicate name lowerCamelCased, without exception, so the
mapping is computable rather than editorial. `scripts/check-acceptance-witness.sh` extracts the ten
names from `mission-graph.md` `## Accepted frontier`, the ten `require` lines of `admit()`, the ten
names of `jujutsu-agent-protocol.md` `## 17. Acceptance condition`, the ten fields of
`AcceptanceWitness`, and this table, lowerCamelCases where needed, and asserts all five are equal
**as ordered sequences**. It additionally compares the **argument counts** at the four document
sites, because a name-only comparison cannot see that one site wrote
`CurrentFrontierReconciled(candidate)` while another wrote `CurrentFrontierReconciled(I, t)` and
the Lean carried a third shape.

`currentFrontierReconciled` is the only conjunct that is computable in the model today: it is a
`Bool`-valued function over the real `IntegrationCandidate` record carrying the frontier-identity,
ancestry, batch-identity, non-emptiness, and conflict-free clauses, and the three
`reconciled_requires_*` theorems recover each of the first three from a `true` result. The other
nine remain `Prop` fields instantiated by the runtime. Making them computable — an executable
`isAcceptable` and a conflict model behind `noUnresolvedConflict` and `evidenceFresh` — is
**G-201** / **G-207** / **G-208**, owned by **#19**.

### Does not prove

That the runtime conflict detector captures every semantic inconsistency, that a `compositional`
manifest flag is true, or that a lease authority issued tokens correctly.

---

## T007 — Declared non-interference symmetry

**Lean:** `formal/Gordian/Conflict.lean#declared_noninterference_symmetric`

**State:** proof-source-present

**Class:** substantive

The declared predicate is:

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

That any admission rule consumes this predicate: the resource sets are non-executable
`R -> Prop` predicates and no admission conjunct is stated over them. An executable claim model
and the admission rule it supports are **G-217** / **G-246**, owned by **#22**.

---

## T008 — Replay stability for equal history

**Lean:** `formal/Gordian/Replay.lean#replay_same_history`

**State:** proof-source-present

**Class:** regression-guard

### Theorem

For a fixed pure projector:

```text
historyA = historyB
  -> replay(projector, historyA) = replay(projector, historyB)
```

The proof is `subst` followed by `rfl`: the projector is a Lean function, so purity and
determinism are properties of the encoding, not results derived from it.

### Engineering interpretation

Replay stability is a property of deterministic projection over recorded facts.

### Does not prove

- projector purity in Rust (by construction in Lean; obligation: #12 effect-spy tests);
- effect capture is complete;
- events are totally ordered correctly;
- distributed ingestion never duplicates or drops events;
- the projector's semantics correspond to external reality;
- anything about a concrete event type or projector — `Replay.lean` is generic over `Event` and
  `State`, and a canonical event set with a real projection is **G-218**, owned by **#12**.

---

## T015 — Satisfaction requires admission

**Lean:** `formal/Gordian/Frontier.lean#satisfied_requires_admission`

**State:** proof-source-present

**Class:** structural

```text
Satisfied W a -> exists F in W.frontiers, exists c, W.candidateOf a = some c and admitted F c
```

`admitted F c` is `c ∈ transitiveParents F.integrationCandidate` — the same relation
`docs/spec/mission-graph.md` `### Satisfaction` states, computed by a real structural recursion
over nested integration parents. An earlier model tested membership in an unconstrained
`ancestors : List ExactStateId` field that nothing populated and nothing related to the admitted
integration, so this theorem proved only that some string appeared in some list while two
invariants cited it as their `**Formal method:**` and claimed `**Coverage:** formalized`.

### Engineering interpretation

A candidate that passed every verifier in a private workspace and was never admitted has
established nothing about the accepted state. Dependents base on an admitted frontier, so
satisfaction must be a property of one.

### Does not prove

That the runtime appends `AtomSatisfied` only inside `admit()`; that is a projector obligation
covered by mutation tests.

---

## T016 — Missing required verifier blocks satisfaction

**Lean:** `formal/Gordian/Frontier.lean#not_satisfied_when_required_verifier_missing`

**State:** proof-source-present

**Class:** structural

```text
(m in manifest a) -> (no evidence record names m.verifier) -> not Satisfied W a
```

### Does not prove

Manifest completeness — that the required set contains the verifiers that matter.

---

## T017 — Integration needs its own evidence

**Lean:** `formal/Gordian/Frontier.lean#integration_needs_own_evidence`

**State:** proof-source-present

**Class:** structural

```text
Compatible e a -> a.exactStateId != (integrate a b sid vd).exactStateId
              -> not Compatible e (integrate a b sid vd)
```

The theorem is stated over the result of `integrate` itself, not over an arbitrary reference, so
it is a statement about the integration operation rather than about two unrelated subjects.

### Engineering interpretation

Composition changes the exact state, so component evidence is structurally incompatible with the
integration result unless the manifest entry declares `compositional = true`, which is an
author's assertion and not a proof.

### Does not prove

That `compositional = true` is ever safe. That is
`experiment:compositional-verifier-inheritance`.

---

## T018 — Exclusive lease exclusion over lease subjects

**Lean:** `formal/Gordian/Lease.lean#no_two_live_exclusive`,
`formal/Gordian/Lease.lean#logical_change_never_shared_write`,
`formal/Gordian/Lease.lean#coordinator_never_shared_write`,
`formal/Gordian/Lease.lean#superseded_holder_rejected`

**State:** proof-source-present

**Class:** structural

```text
l1 in T.leases -> l2 in T.leases -> LiveExclusive l1 -> LiveExclusive l2
  -> l1.subject = s -> l2.subject = s -> l1.id = l2.id
```

`subject : LeaseSubject`, a three-constructor sum over `SemanticResourceId`, `LogicalChangeId`,
and `ProjectId` (the admitting-coordinator subject).
This supersedes the planned T010, which was stated over semantic resources only and therefore
could not cover the one-writer-per-change invariant. It is a `theorem` over a `LeaseTable` whose
`unique` field carries the exclusivity obligation, not a `def` returning a `Prop` that proves
nothing.

### Does not prove

That a real backend enforces the lease. The source plane offers no fencing; enforcement is
`LeaseValidAtFreeze` at admission.

That the lease *transition system* preserves exclusivity: `LeaseTable.unique` is an invariant
assumed of a table, not established across acquire/renew/revoke steps. That transition theorem is
**G-227**, owned by **#23**, and lands with the transition model (**G-236**).

---

## T019 — Retry policy totality over effect classes

**Lean:** `formal/Gordian/EffectClass.lean#irreversible_never_auto_retried`,
`formal/Gordian/EffectClass.lean#judgment_never_overwrites`

**State:** proof-source-present

**Class:** regression-guard

`retryPolicy : EffectClass -> RetryRule` is defined by a match with one arm per constructor and no
wildcard, so the compiler is the totality checker; the two theorems pin the two rules whose
violation is unrecoverable.

### Does not prove

That a runtime honours the returned rule; that is fault injection around timeouts and crashes.

---

## Auxiliary lemmas

Declarations that exist in `formal/Gordian/` and support a numbered theorem without carrying a
T-number of their own. Every `theorem` in `formal/Gordian/*.lean` appears either under a `## T0NN`
section above or in this table; asserting that mechanically, and reconciling the same set against
`knowledge/graph/60-formal.jsonld`, is **G-242** / **G-265**, owned by **#72**.

| Declaration | Supports | Role |
| --- | --- | --- |
| `formal/Gordian/Graph.lean#path_decreases` | T001 | the induction over `DependsPath` that `no_dependency_cycle` instantiates at `a = b` |
| `formal/Gordian/Scheduler.lean#dispatchable_implies_enabled` | T003 | strips the four dispatch-only conjuncts so the `enabled` clauses can be unfolded |
| `formal/Gordian/Frontier.lean#freshPass_has_record` | T016 | extracts the witnessing record from a `freshPass` proof |
| `formal/Gordian/Replay.lean#replay_is_functional` | T008 | definitional unfolding of `replay` |
| `formal/Gordian/Conflict.lean#disjoint_symmetric` | T007 | symmetry of the underlying disjointness relation |

---

## Assigned formal-model obligations

The Lean model is a specification oracle, not a finished proof of Gordian. This table is the
complete list of formal-model work this revision identified and did **not** do, with the Atom that
owns each item. An Atom may not report itself complete while a row naming it is open, and no other
Atom may claim a row here closed. Rows are the gap register's ids, so the register and this
catalog cannot drift into different backlogs.

| Gap | Deliverable | Owner |
| --- | --- | --- |
| G-201 | an executable `isAcceptable` over `CandidateFacts`, so admission is a computable predicate rather than a witness of opaque `Prop`s | #19 |
| G-202 | an executable Lean `Evidence.isCompatible` over `EvidenceRef` / `CandidateRef` for evidence compatibility testing | #15 |
| G-204 | the Lean/Rust differential conformance harness itself: `formal/conformance/`, the `gordian-core` runner/generator at `crates/gordian-core/tests/conformance.rs`, the seeded `HardDependenciesAcyclic` vectors, the disagreement fixture, and the CI wiring | #7 |
| G-206 | `dependenciesSatisfied` defined over the real dependency graph and `Frontier.Satisfied`, rather than the `Environment.satisfied` oracle field the readiness definitions currently take as given | #13 |
| G-207 | `noUnresolvedConflict` defined through an executable conflict model instead of an assumed `Prop` | #19 |
| G-208 | `evidenceFresh` defined through `Evidence.Compatible` instead of an assumed `Prop` | #19 |
| G-209 | admission theorems that derive rather than project, once the nine remaining conjuncts are computable | #19 |
| G-211 | a grant and delegation relation, so "by default" in `coordinator_cannot_deploy_by_default` has formal content, and `deploy_requires_explicit_grant` | #18 |
| G-214 | evidence-compatibility content beyond the contrapositive of record equality | #15 |
| G-217 | executable semantic resource claims in `Conflict.lean` in place of `R -> Prop` predicates | #22 |
| G-218 | a canonical event type and a real projector in `Replay.lean` | #12 |
| G-220 | cross-module integration wherever a module still restates a specification bullet list rather than importing the definition | #19 |
| G-222 | `edge_target_globally_dependable`: dependency-graph nodes carrying a `WorkKind`, so an edge cannot target a Quark structurally and not merely by policy | #10 |
| G-223 | the candidate-freeze transition theorem, T011 | #31 |
| G-224 | duplicate-event idempotency, T021 | #12 |
| G-225 | accepted-frontier CAS linearization, T020 | #19 |
| G-227 | lease exclusivity across acquire / renew / revoke transitions, not merely as a `LeaseTable` invariant | #23 |
| G-231 | a rank computation and cycle-detection decision procedure, so T001's certificate has a producer | #10 |
| G-232 | Quark-ownership uniqueness ("a Quark MUST belong to one Atom") | #10 |
| G-233 | an executable rank-certificate checker | #10 |
| G-234 | dispatch theorems with transition content, once the transition model exists | #13 |
| G-236 | `formal/Gordian/Transition.lean` and the T012/T013 family: no Atom owns the transition model today, and creating that Atom is the gap | #13 |
| G-238 | a relevance-conditioned `Compatible`, so "declared relevant by the verifier policy" has a formal counterpart | #15 |
| G-241 | distinct `CandidateRef` and `EvidenceRef` types, so a candidate cannot be passed where evidence is expected | #15 |
| G-246 | the admission rule that consumes `DeclaredNonInterfering` | #22 |
| G-249 | containment in the graph model, so "containment is not dependency" is a statement and not a convention | #10 |
| G-259 | the per-revision CI evidence artifact that alone can promote a `**State:**` line to `machine-checked` | #2 |
| G-261 | replay theorems with content beyond congruence and `rfl` | #12 |
| G-154, G-242, G-248, G-264, G-265 | `content` and `class` on `Theorem` nodes, one status vocabulary, catalog/graph reconciliation, and anchor resolution checking | #72 |
| G-221 | the script that asserts this catalog's index table against the Lean declarations and `60-formal.jsonld` | #72 |

Closed by this revision and listed here only so a reader does not go looking for them: **G-205**
(`Satisfied` / `Blocked` / `Active` now exist, T015/T016), **G-210** (ten admission conjuncts),
**G-213** / **G-323** (`GloballyDependable` narrowed to `Atom`), **G-215** / **G-216** / **G-247**
(`dependency_digest`, `canonicalization_scheme`, and the adapter-neutral `exactStateId`),
**G-226** (integration non-compositionality, T017), **G-237** / **G-319** (the seven readiness
predicates are computable), **G-251** (`LeaseSubject` and a real `no_two_live_exclusive`, T018),
and **G-253** (`retryPolicy` total over `EffectClass`, T019). **G-203** remains the Lean
scheduler model's executable Boolean work; it is not the seeded conformance target. The raw-graph
`HardDependenciesAcyclic` runner and its Rust reference comparison remain **G-204**, owned by
**#7**.


# Planned theorem families

## T009 — Topological scheduler safety

Prove a scheduler operating over a valid dependency DAG emits only currently enabled nodes.

**State:** planned

## T010 — Exclusive semantic lease safety

**State:** planned

Superseded by T018, which generalizes the statement from semantic resources to the full
`LeaseSubject` sum. Retained as a numbered entry so external references do not dangle.

## T011 — Candidate freeze

Prove an evidence record can be generated only against a frozen candidate identity and any mutation creates a distinct subject.

**State:** planned — **G-223**, owned by **#31**.

## T012 — State-transition invariant preservation

Define Mission execution transitions and prove every legal transition preserves global invariants.

**State:** planned — `formal/Gordian/Transition.lean` does not exist and no Atom owns it; that is
**G-236**, an Atom-creation gap owned by **#13**.

## T013 — No worker-originated frontier mutation

Strengthen T005 from static role policy into a transition theorem over the state machine.

**State:** planned — depends on T012.

## T014 — Evidence monotonicity under irrelevant change

Precisely state when evidence may remain valid after a change proven irrelevant to a verifier's declared dependency boundary.

**State:** planned

This theorem is intentionally deferred because incorrectly formalizing "irrelevant" would create a dangerous false sense of reuse safety.

## T020 — Accepted-frontier CAS linearization

Prove that a compare-and-swap promotion against an obsolete `FrontierVersion` expectation is
rejected, so admitted frontiers are linearized.

**State:** planned — **G-225**, owned by **#19**. `docs/spec/invariants.md`
`## Accepted-frontier linearization` names this as its formal target.

## T021 — Duplicate-event idempotency

Prove that appending a duplicate idempotent event leaves the projected state unchanged.

**State:** planned — **G-224**, owned by **#12**.

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

Every section of `docs/spec/invariants.md` MUST carry a `**Coverage:**` line whose value is one of
the six states above. `scripts/check-invariant-coverage.sh` asserts that between every pair of
`## ` headings in `invariants.md` there is exactly one line matching `^\*\*Coverage:\*\* ` and
that its value is a member of this list. The list itself is extracted from this section, so adding
a state here is the only way to add one there.

The goal is not 100% Lean coverage. The goal is to use the strongest applicable verification method for each claim without pretending mathematical proof can answer empirical questions.
