import Gordian.Ids
import Gordian.Authority
import Gordian.Frontier

namespace Gordian

/-- The seventh readiness/admission predicate, decidable like the six in Scheduler.lean.
Mirrors docs/algorithms/evidence-and-admission.md#frontier-reconciliation **at its full
arity** — subject, frontier, batch — with the batch threaded from the IntegrationBatch record
rather than left as a free variable, and with the ancestry conjunct carried. An earlier version
omitted both `t is an ancestor of c.exact_state_id` and `c.parent_candidates = B`, so the model
admitted an integration whose exact state did not descend from the frontier: it was strictly
weaker than the prose it was cited as formalizing. `t` is compared against `baseFrontier`, never
against a member of the parents: `t` is an exact state id and the parents are candidates. -/
def currentFrontierReconciled
    (I : IntegrationCandidate) (t : ExactStateId) (B : List Candidate)
    (ancestorOf : ExactStateId → ExactStateId → Bool)
    (conflictsOf : ExactStateId → List String) : Bool :=
  (I.baseFrontier == t)
  && ancestorOf t I.exactStateId
  && decide (I.componentParents = B)
  && (! B.isEmpty)
  && (conflictsOf I.exactStateId).isEmpty

theorem reconciled_requires_current_frontier
    {I t B ancestorOf conflictsOf}
    (h : currentFrontierReconciled I t B ancestorOf conflictsOf = true) :
    I.baseFrontier = t := by
  simp [currentFrontierReconciled, Bool.and_eq_true] at h
  exact h.1.1.1.1

theorem reconciled_requires_ancestry
    {I t B ancestorOf conflictsOf}
    (h : currentFrontierReconciled I t B ancestorOf conflictsOf = true) :
    ancestorOf t I.exactStateId = true := by
  simp [currentFrontierReconciled, Bool.and_eq_true] at h
  exact h.1.1.1.2

theorem reconciled_requires_batch
    {I t B ancestorOf conflictsOf}
    (h : currentFrontierReconciled I t B ancestorOf conflictsOf = true) :
    I.componentParents = B := by
  simp [currentFrontierReconciled, Bool.and_eq_true] at h
  exact h.1.1.2

structure CandidateFacts where
  /-- Instantiated by `currentFrontierReconciled I t B ancestorOf conflictsOf = true`;
  docs/algorithms/evidence-and-admission.md#frontier-reconciliation -/
  currentFrontierReconciled : Prop
  /-- No transitive parent is already an ancestor of the frontier or already satisfied, and
  the integration's exact state is not itself an existing frontier;
  docs/algorithms/evidence-and-admission.md#the-admission-conjuncts-defined -/
  parentsUnadmitted : Prop
  noUnresolvedConflict : Prop
  verifierManifestComplete : Prop
  requiredVerificationPasses : Prop
  evidenceBoundToExactCandidate : Prop
  evidenceFresh : Prop
  evidenceProvenanceValid : Prop
  leaseValidAtFreeze : Prop
  promoter : Role

structure AcceptanceWitness (f : CandidateFacts) : Prop where
  currentFrontierReconciled : f.currentFrontierReconciled
  parentsUnadmitted : f.parentsUnadmitted
  noUnresolvedConflict : f.noUnresolvedConflict
  verifierManifestComplete : f.verifierManifestComplete
  requiredVerificationPasses : f.requiredVerificationPasses
  evidenceBoundToExactCandidate : f.evidenceBoundToExactCandidate
  evidenceFresh : f.evidenceFresh
  evidenceProvenanceValid : f.evidenceProvenanceValid
  leaseValidAtFreeze : f.leaseValidAtFreeze
  authorizedPromotion : CanPromoteAccepted f.promoter

abbrev Acceptable (f : CandidateFacts) : Prop := AcceptanceWitness f

theorem accepted_implies_reconciled {f} (h : Acceptable f) :
    f.currentFrontierReconciled := h.currentFrontierReconciled

theorem accepted_implies_parents_unadmitted {f} (h : Acceptable f) :
    f.parentsUnadmitted := h.parentsUnadmitted

theorem accepted_implies_conflict_free {f} (h : Acceptable f) :
    f.noUnresolvedConflict := h.noUnresolvedConflict

theorem accepted_implies_manifest_complete {f} (h : Acceptable f) :
    f.verifierManifestComplete := h.verifierManifestComplete

theorem accepted_implies_verified {f} (h : Acceptable f) :
    f.requiredVerificationPasses := h.requiredVerificationPasses

theorem accepted_implies_evidence_bound {f} (h : Acceptable f) :
    f.evidenceBoundToExactCandidate := h.evidenceBoundToExactCandidate

theorem accepted_implies_fresh_evidence {f} (h : Acceptable f) :
    f.evidenceFresh := h.evidenceFresh

theorem accepted_implies_provenance_valid {f} (h : Acceptable f) :
    f.evidenceProvenanceValid := h.evidenceProvenanceValid

theorem accepted_implies_lease_valid_at_freeze {f} (h : Acceptable f) :
    f.leaseValidAtFreeze := h.leaseValidAtFreeze

theorem accepted_implies_authorized_promoter {f} (h : Acceptable f) :
    CanPromoteAccepted f.promoter := h.authorizedPromotion

end Gordian
