import Std
import Gordian.Ids

namespace Gordian

structure CandidateRef where
  exactStateId : String
  specRevision : String
  inputDigest : String
  dependencyDigest : String
  environmentDigest : String
  verifierDigest : String
  canonicalizationScheme : String
  deriving Repr, DecidableEq

structure EvidenceRef where
  exactStateId : String
  specRevision : String
  inputDigest : String
  dependencyDigest : String
  environmentDigest : String
  verifierDigest : String
  canonicalizationScheme : String
  deriving Repr, DecidableEq

structure CompatibleWitness (e : EvidenceRef) (c : CandidateRef) : Prop where
  exactStateMatches : e.exactStateId = c.exactStateId
  specMatches : e.specRevision = c.specRevision
  inputsMatch : e.inputDigest = c.inputDigest
  dependenciesMatch : e.dependencyDigest = c.dependencyDigest
  environmentMatches : e.environmentDigest = c.environmentDigest
  verifierMatches : e.verifierDigest = c.verifierDigest
  canonicalizationMatches : e.canonicalizationScheme = c.canonicalizationScheme

abbrev Compatible (e : EvidenceRef) (c : CandidateRef) : Prop := CompatibleWitness e c

theorem exact_state_mismatch_invalidates {e : EvidenceRef} {c : CandidateRef}
    (mismatch : e.exactStateId ≠ c.exactStateId) : ¬ Compatible e c := by
  intro h
  exact mismatch h.exactStateMatches

theorem spec_mismatch_invalidates {e : EvidenceRef} {c : CandidateRef}
    (mismatch : e.specRevision ≠ c.specRevision) : ¬ Compatible e c := by
  intro h
  exact mismatch h.specMatches

theorem input_mismatch_invalidates {e : EvidenceRef} {c : CandidateRef}
    (mismatch : e.inputDigest ≠ c.inputDigest) : ¬ Compatible e c := by
  intro h
  exact mismatch h.inputsMatch

theorem dependency_mismatch_invalidates {e : EvidenceRef} {c : CandidateRef}
    (mismatch : e.dependencyDigest ≠ c.dependencyDigest) : ¬ Compatible e c := by
  intro h
  exact mismatch h.dependenciesMatch

theorem environment_mismatch_invalidates {e : EvidenceRef} {c : CandidateRef}
    (mismatch : e.environmentDigest ≠ c.environmentDigest) : ¬ Compatible e c := by
  intro h
  exact mismatch h.environmentMatches

theorem verifier_mismatch_invalidates {e : EvidenceRef} {c : CandidateRef}
    (mismatch : e.verifierDigest ≠ c.verifierDigest) : ¬ Compatible e c := by
  intro h
  exact mismatch h.verifierMatches

theorem canonicalization_mismatch_invalidates {e : EvidenceRef} {c : CandidateRef}
    (mismatch : e.canonicalizationScheme ≠ c.canonicalizationScheme) : ¬ Compatible e c := by
  intro h
  exact mismatch h.canonicalizationMatches

end Gordian
