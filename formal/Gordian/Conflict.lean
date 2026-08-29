import Std

namespace Gordian

universe u

abbrev ResourceSet (R : Type u) := R → Prop

def Disjoint {R : Type u} (a b : ResourceSet R) : Prop :=
  ∀ r, a r → b r → False

theorem disjoint_symmetric {R : Type u} {a b : ResourceSet R} (h : Disjoint a b) :
    Disjoint b a := by
  intro r hb ha
  exact h r ha hb

structure TransactionClaims (R : Type u) where
  reads : ResourceSet R
  writes : ResourceSet R

def DeclaredNonInterfering {R : Type u} (a b : TransactionClaims R) : Prop :=
  Disjoint a.writes b.reads ∧
  Disjoint a.writes b.writes ∧
  Disjoint b.writes a.reads

theorem declared_noninterference_symmetric {R : Type u} {a b : TransactionClaims R}
    (h : DeclaredNonInterfering a b) : DeclaredNonInterfering b a := by
  rcases h with ⟨aWrite_bRead, aWrite_bWrite, bWrite_aRead⟩
  exact ⟨bWrite_aRead, disjoint_symmetric aWrite_bWrite, aWrite_bRead⟩

end Gordian
