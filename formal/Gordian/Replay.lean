import Std

namespace Gordian

universe u v

abbrev Projector (Event : Type u) (State : Type v) := List Event → State

def replay {Event : Type u} {State : Type v}
    (project : Projector Event State) (history : List Event) : State :=
  project history

theorem replay_same_history {Event : Type u} {State : Type v}
    (project : Projector Event State) {a b : List Event} (same : a = b) :
    replay project a = replay project b := by
  subst b
  rfl

theorem replay_is_functional {Event : Type u} {State : Type v}
    (project : Projector Event State) (history : List Event) :
    replay project history = project history := by
  rfl

end Gordian
