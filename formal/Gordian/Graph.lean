import Std

namespace Gordian

inductive WorkKind where
  | project
  | mission
  | planRevision
  | initiative
  | atom
  | quark
  deriving Repr, DecidableEq

def GloballyDependable : WorkKind → Prop
  | .atom => True
  | _ => False

theorem quark_not_globally_dependable : ¬ GloballyDependable .quark := by
  simp [GloballyDependable]

theorem initiative_not_globally_dependable : ¬ GloballyDependable .initiative := by
  simp [GloballyDependable]

theorem mission_not_globally_dependable : ¬ GloballyDependable .mission := by
  simp [GloballyDependable]

/-- The whole policy in one statement, so docs/formal/theorem-catalog.md T002 can cite
a theorem rather than a definition, and a widened definition breaks the proof. -/
theorem globally_dependable_iff_atom {k : WorkKind} :
    GloballyDependable k ↔ k = .atom := by
  cases k <;> simp [GloballyDependable]

universe u

structure RankedDependencyGraph (Node : Type u) where
  rank : Node → Nat
  dependsOn : Node → Node → Prop
  decreases : ∀ {a b}, dependsOn a b → rank b < rank a

inductive DependsPath {Node : Type u} (g : RankedDependencyGraph Node) : Node → Node → Prop where
  | direct {a b} : g.dependsOn a b → DependsPath g a b
  | step {a b c} : g.dependsOn a b → DependsPath g b c → DependsPath g a c

theorem path_decreases {Node : Type u} (g : RankedDependencyGraph Node) {a b : Node}
    (h : DependsPath g a b) : g.rank b < g.rank a := by
  induction h with
  | direct hab =>
      exact g.decreases hab
  | step hab _ ih =>
      exact Nat.lt_trans ih (g.decreases hab)

theorem no_dependency_cycle {Node : Type u} (g : RankedDependencyGraph Node) (a : Node)
    (h : DependsPath g a a) : False := by
  have hlt : g.rank a < g.rank a := path_decreases g h
  exact (Nat.lt_irrefl _ hlt)

end Gordian
