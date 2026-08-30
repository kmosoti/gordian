import Std
import Gordian.Ids
import Gordian.Evidence

namespace Gordian

structure Candidate where
  atomId : AtomId
  specRevision : SpecRevisionId
  logicalChangeId : LogicalChangeId
  exactStateId : ExactStateId
  fencingToken : Nat
  deriving Repr, DecidableEq

structure ManifestEntry where
  verifier : VerifierId
  compositional : Bool
  deriving Repr, DecidableEq

structure Manifest where
  required : List ManifestEntry
  deriving Repr, DecidableEq

/-- The admitted subject. Admission never takes a bare Candidate; a one-member batch is
still an IntegrationCandidate. Parents are component Candidates or nested integrations, so
`transitiveParents` is a real recursion rather than a lookup into an unconstrained list.
Mirrors docs/spec/data-model.md#integration-candidate. -/
inductive IntegrationCandidate where
  | node (baseFrontier : ExactStateId)
         (componentParents : List Candidate)
         (nestedParents : List IntegrationCandidate)
         (integrationManifest : List ManifestEntry)
         (exactStateId : ExactStateId)
  deriving Repr

namespace IntegrationCandidate

def baseFrontier : IntegrationCandidate → ExactStateId
  | .node b _ _ _ _ => b

def componentParents : IntegrationCandidate → List Candidate
  | .node _ cs _ _ _ => cs

def integrationManifest : IntegrationCandidate → List ManifestEntry
  | .node _ _ _ m _ => m

def exactStateId : IntegrationCandidate → ExactStateId
  | .node _ _ _ _ e => e

end IntegrationCandidate

-- `transitive_parent_candidates(I)` of docs/spec/data-model.md#integration-candidate:
-- every Candidate reachable by repeated expansion of nested integration parents. Structural
-- recursion, so the closure is total.
mutual
  /-- Every Candidate reachable by repeated expansion of nested integration parents. -/
  def transitiveParents : IntegrationCandidate → List Candidate
    | .node _ cs ns _ _ => cs ++ transitiveParentsList ns
  /-- `transitiveParents` lifted over a list of nested integration parents. -/
  def transitiveParentsList : List IntegrationCandidate → List Candidate
    | [] => []
    | i :: rest => transitiveParents i ++ transitiveParentsList rest
end

/-- An admitted frontier row. Mirrors docs/spec/data-model.md#frontier field for field:
frontier_seq, exact_state_id, integration_candidate, admitted_at_event, previous_frontier. -/
structure Frontier where
  frontierSeq : FrontierSeq
  exactStateId : ExactStateId
  integrationCandidate : IntegrationCandidate
  admittedAtEvent : EventSeq
  previousFrontier : Option FrontierSeq
  deriving Repr

/-- Admission is membership in the admitted integration's transitive parent set — the same
relation docs/spec/mission-graph.md states. An earlier version tested membership in an
unconstrained `ancestors : List ExactStateId` field that nothing populated and nothing related
to the admitted integration, so the theorem below proved only that some string appeared in some
list. -/
def admitted (F : Frontier) (c : Candidate) : Prop :=
  c ∈ transitiveParents F.integrationCandidate

structure EvidenceRecord where
  verifier : VerifierId
  passed : Bool
  subject : EvidenceRef
  deriving Repr, DecidableEq

structure Store where
  records : List EvidenceRecord

/-- A fresh pass requires a compatible passing record and NO compatible failing record.
Mirrors the no-fresh-failure conjunct of
docs/algorithms/evidence-and-admission.md#the-verified-rule. -/
def freshPass (S : Store) (v : VerifierId) (s : CandidateRef) : Prop :=
  (∃ e ∈ S.records, e.verifier = v ∧ e.passed = true ∧ Compatible e.subject s) ∧
  ¬ (∃ e ∈ S.records, e.verifier = v ∧ e.passed = false ∧ Compatible e.subject s)

theorem freshPass_has_record {S : Store} {v : VerifierId} {s : CandidateRef}
    (h : freshPass S v s) : ∃ e ∈ S.records, e.verifier = v := by
  obtain ⟨⟨e, he, hv, _, _⟩, _⟩ := h
  exact ⟨e, he, hv⟩

structure World where
  frontiers : List Frontier
  atoms : List AtomId
  store : Store
  candidateOf : AtomId → Option Candidate
  manifestOf : AtomId → Manifest
  /-- Fingerprint components for verifier `v` evaluated on exact state `s`. -/
  subjectOn : ExactStateId → VerifierId → CandidateRef
  /-- True when an EvidenceInherited event records `v` for (I, c). This is the discriminator
  the specification branches on, not the raw `compositional` flag. -/
  inherited : IntegrationCandidate → Candidate → VerifierId → Bool
  hardDeps : AtomId → List AtomId
  requiredInterfaces : AtomId → List ResourceId
  providedInterfaces : AtomId → List ResourceId
  declaredInputs : AtomId → List ResourceId
  declaredOutputs : AtomId → List ResourceId
  externallyProvided : List ResourceId
  running : AtomId → Prop

-- Satisfied(a) :=
--   exists an admitted frontier F, with I = F.integration_candidate
--   where candidate(a) is in transitive_parent_candidates(I)
--   and for every required verifier v in manifest(a):
--       if an EvidenceInherited event records v for (I, candidate(a))
--         then FreshPass(v, candidate(a))
--         else FreshPass(v, I)
def Satisfied (W : World) (a : AtomId) : Prop :=
  ∃ F ∈ W.frontiers, ∃ c : Candidate,
    W.candidateOf a = some c ∧
    admitted F c ∧
    ∀ m ∈ (W.manifestOf a).required,
      freshPass W.store m.verifier
        (W.subjectOn
          (if W.inherited F.integrationCandidate c m.verifier
             then c.exactStateId
             else F.integrationCandidate.exactStateId)
          m.verifier)

theorem satisfied_requires_admission {W : World} {a : AtomId} (h : Satisfied W a) :
    ∃ F ∈ W.frontiers, ∃ c : Candidate, W.candidateOf a = some c ∧ admitted F c := by
  obtain ⟨F, hF, c, hc, hadm, _⟩ := h
  exact ⟨F, hF, c, hc, hadm⟩

theorem not_satisfied_when_required_verifier_missing
    (W : World) (a : AtomId) (m : ManifestEntry)
    (hm : m ∈ (W.manifestOf a).required)
    (hmissing : ∀ e ∈ W.store.records, e.verifier ≠ m.verifier) :
    ¬ Satisfied W a := by
  intro h
  obtain ⟨_, _, _, _, _, hall⟩ := h
  obtain ⟨e, he, hv⟩ := freshPass_has_record (hall m hm)
  exact hmissing e he hv

def providedBySatisfied (W : World) (q : ResourceId) : Prop :=
  ∃ p ∈ W.atoms, Satisfied W p ∧ q ∈ W.providedInterfaces p

def producedBySatisfied (W : World) (i : ResourceId) : Prop :=
  ∃ p ∈ W.atoms, Satisfied W p ∧ i ∈ W.declaredOutputs p

def Blocked (W : World) (a : AtomId) : Prop :=
  (∃ d ∈ W.hardDeps a, ¬ Satisfied W d) ∨
  (∃ q ∈ W.requiredInterfaces a, ¬ providedBySatisfied W q ∧ q ∉ W.externallyProvided) ∨
  (∃ i ∈ W.declaredInputs a, ¬ producedBySatisfied W i ∧ i ∉ W.externallyProvided)

def Active (W : World) (a : AtomId) : Prop := W.running a

/-- Integration produces a distinct subject; it does not inherit its parents' identity.
Mirrors docs/algorithms/evidence-and-admission.md#integration-fingerprint. -/
def integrate (a b : CandidateRef) (exactStateId : String) (integrationVerifierDigest : String)
    : CandidateRef :=
  { exactStateId := exactStateId
    specRevision := a.specRevision ++ "|" ++ b.specRevision
    inputDigest := a.inputDigest ++ "|" ++ b.inputDigest
    dependencyDigest := a.dependencyDigest ++ "|" ++ b.dependencyDigest
    environmentDigest := a.environmentDigest
    verifierDigest := integrationVerifierDigest
    canonicalizationScheme := a.canonicalizationScheme }

/-- Evidence compatible with a component candidate is not compatible with the integration
result whenever their exact-state identities differ. The conclusion is stated about the
result of `integrate` itself, not about an unrelated reference.
Mirrors docs/spec/invariants.md#integration-non-compositionality. -/
theorem integration_needs_own_evidence
    {e : EvidenceRef} {a b : CandidateRef} {sid vd : String}
    (hcompat : Compatible e a)
    (hdiff : a.exactStateId ≠ (integrate a b sid vd).exactStateId) :
    ¬ Compatible e (integrate a b sid vd) := by
  intro h
  exact hdiff (hcompat.exactStateMatches.symm.trans h.exactStateMatches)

end Gordian
