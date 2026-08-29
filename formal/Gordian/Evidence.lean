import Std

namespace Gordian

structure CandidateRef where
  commitId : String
  specRevision : String
  inputDigest : String
  environmentDigest : String
  verifierDigest : String
  deriving Repr, DecidableEq

structure EvidenceRef where
  commitId : String
  specRevision : String
  inputDigest : String
  environmentDigest : String
  verifierDigest : String
  deriving Repr, DecidableEq

structure CompatibleWitness (e : EvidenceRef) (c : CandidateRef) : Prop where
  commitMatches : e.commitId = c.commitId
  specMatches : e.specRevision = c.specRevision
  inputsMatch : e.inputDigest = c.inputDigest
  environmentMatches : e.environmentDigest = c.environmentDigest
  verifierMatches : e.verifierDigest = c.verifierDigest

abbrev Compatible (e : EvidenceRef) (c : CandidateRef) : Prop := CompatibleWitness e c

theorem commit_mismatch_invalidates {e : EvidenceRef} {c : CandidateRef}
    (mismatch : e.commitId ≠ c.commitId) : ¬ Compatible e c := by
  intro h
  exact mismatch h.commitMatches

theorem spec_mismatch_invalidates {e : EvidenceRef} {c : CandidateRef}
    (mismatch : e.specRevision ≠ c.specRevision) : ¬ Compatible e c := by
  intro h
  exact mismatch h.specMatches

theorem input_mismatch_invalidates {e : EvidenceRef} {c : CandidateRef}
    (mismatch : e.inputDigest ≠ c.inputDigest) : ¬ Compatible e c := by
  intro h
  exact mismatch h.inputsMatch

theorem environment_mismatch_invalidates {e : EvidenceRef} {c : CandidateRef}
    (mismatch : e.environmentDigest ≠ c.environmentDigest) : ¬ Compatible e c := by
  intro h
  exact mismatch h.environmentMatches

theorem verifier_mismatch_invalidates {e : EvidenceRef} {c : CandidateRef}
    (mismatch : e.verifierDigest ≠ c.verifierDigest) : ¬ Compatible e c := by
  intro h
  exact mismatch h.verifierMatches

end Gordian
