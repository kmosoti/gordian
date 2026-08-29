import Gordian.Authority

namespace Gordian

structure CandidateFacts where
  reconciledWithCurrentFrontier : Prop
  conflictFree : Prop
  requiredVerificationPasses : Prop
  evidenceFresh : Prop
  promoter : Role

structure AcceptanceWitness (f : CandidateFacts) : Prop where
  reconciled : f.reconciledWithCurrentFrontier
  conflictFree : f.conflictFree
  verified : f.requiredVerificationPasses
  fresh : f.evidenceFresh
  authorized : CanPromoteAccepted f.promoter

abbrev Acceptable (f : CandidateFacts) : Prop := AcceptanceWitness f

theorem accepted_implies_reconciled {f : CandidateFacts} (h : Acceptable f) :
    f.reconciledWithCurrentFrontier :=
  h.reconciled

theorem accepted_implies_conflict_free {f : CandidateFacts} (h : Acceptable f) :
    f.conflictFree :=
  h.conflictFree

theorem accepted_implies_verified {f : CandidateFacts} (h : Acceptable f) :
    f.requiredVerificationPasses :=
  h.verified

theorem accepted_implies_fresh_evidence {f : CandidateFacts} (h : Acceptable f) :
    f.evidenceFresh :=
  h.fresh

theorem accepted_implies_authorized_promoter {f : CandidateFacts} (h : Acceptable f) :
    CanPromoteAccepted f.promoter :=
  h.authorized

end Gordian
