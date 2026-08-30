import Std
import Gordian.Ids

namespace Gordian

/-- Three constructors, because Gordian excludes three different things: concurrent semantic
writes to a domain resource, concurrent rewriting of one evolving source change, and two
coordinators driving admission for one Project. Mirrors docs/spec/data-model.md#lease. -/
inductive LeaseSubject where
  | semanticResource (id : ResourceId)
  | logicalChangeId (id : LogicalChangeId)
  | coordinator (id : ProjectId)
  deriving Repr, DecidableEq

inductive LeaseMode where
  | read
  | writeSharedIfCommutative
  | writeExclusive
  deriving Repr, DecidableEq

structure Lease where
  id : String
  holderAttempt : String
  subject : LeaseSubject
  mode : LeaseMode
  fencingToken : Nat
  live : Bool
  deriving Repr, DecidableEq

def LiveExclusive (l : Lease) : Prop :=
  l.live = true ∧ l.mode = LeaseMode.writeExclusive

/-- A lease table that has enforced exclusivity over any `LeaseSubject`, including
`logicalChangeId`. -/
structure LeaseTable where
  leases : List Lease
  unique : ∀ l₁ ∈ leases, ∀ l₂ ∈ leases,
    LiveExclusive l₁ → LiveExclusive l₂ → l₁.subject = l₂.subject → l₁.id = l₂.id

theorem no_two_live_exclusive (T : LeaseTable) (s : LeaseSubject)
    {l₁ l₂ : Lease} (h₁ : l₁ ∈ T.leases) (h₂ : l₂ ∈ T.leases)
    (e₁ : LiveExclusive l₁) (e₂ : LiveExclusive l₂)
    (s₁ : l₁.subject = s) (s₂ : l₂.subject = s) : l₁.id = l₂.id :=
  T.unique l₁ h₁ l₂ h₂ e₁ e₂ (s₁.trans s₂.symm)

/-- A shared-write grant is never permitted on a logical change. -/
def sharedWriteWellFormed (l : Lease) : Prop :=
  l.mode = LeaseMode.writeSharedIfCommutative →
    ∃ r, l.subject = LeaseSubject.semanticResource r

theorem logical_change_never_shared_write {l : Lease} {x : LogicalChangeId}
    (wf : sharedWriteWellFormed l) (hs : l.subject = LeaseSubject.logicalChangeId x) :
    l.mode ≠ LeaseMode.writeSharedIfCommutative := by
  intro hm
  obtain ⟨r, hr⟩ := wf hm
  rw [hs] at hr
  cases hr

/-- The same argument for the coordinator subject: admission exclusion is never shared. -/
theorem coordinator_never_shared_write {l : Lease} {p : ProjectId}
    (wf : sharedWriteWellFormed l) (hs : l.subject = LeaseSubject.coordinator p) :
    l.mode ≠ LeaseMode.writeSharedIfCommutative := by
  intro hm
  obtain ⟨r, hr⟩ := wf hm
  rw [hs] at hr
  cases hr

/-- The candidate-freeze fencing check that substitutes for substrate-level fencing.
Mirrors LeaseValidAtFreeze in
docs/algorithms/evidence-and-admission.md#the-admission-conjuncts-defined. -/
def leaseValidAtFreeze (recorded highestAtFreeze : Nat)
    (expiredAtFreeze revokedAtFreeze : Bool) : Prop :=
  recorded = highestAtFreeze ∧ expiredAtFreeze = false ∧ revokedAtFreeze = false

theorem superseded_holder_rejected
    {recorded highest : Nat} (h : recorded < highest) :
    ¬ leaseValidAtFreeze recorded highest false false := by
  intro hv
  have heq : recorded = highest := hv.1
  omega

end Gordian
