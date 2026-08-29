import Std

namespace Gordian

structure AtomFacts where
  validSpec : Prop
  dependenciesSatisfied : Prop
  preconditionsHold : Prop
  compatibleExecutorAvailable : Prop
  resourcesAvailable : Prop
  authorizationValid : Prop
  leaseCompatible : Prop

structure EnabledWitness (f : AtomFacts) : Prop where
  validSpec : f.validSpec
  dependenciesSatisfied : f.dependenciesSatisfied
  preconditionsHold : f.preconditionsHold

abbrev Enabled (f : AtomFacts) : Prop := EnabledWitness f

structure DispatchWitness (f : AtomFacts) : Prop where
  enabled : Enabled f
  compatibleExecutorAvailable : f.compatibleExecutorAvailable
  resourcesAvailable : f.resourcesAvailable
  authorizationValid : f.authorizationValid
  leaseCompatible : f.leaseCompatible

abbrev Dispatchable (f : AtomFacts) : Prop := DispatchWitness f

theorem dispatchable_implies_enabled {f : AtomFacts} (h : Dispatchable f) : Enabled f :=
  h.enabled

theorem dispatchable_implies_dependencies_satisfied {f : AtomFacts} (h : Dispatchable f) :
    f.dependenciesSatisfied :=
  h.enabled.dependenciesSatisfied

theorem dispatchable_implies_preconditions {f : AtomFacts} (h : Dispatchable f) :
    f.preconditionsHold :=
  h.enabled.preconditionsHold

theorem dispatchable_implies_authorization {f : AtomFacts} (h : Dispatchable f) :
    f.authorizationValid :=
  h.authorizationValid

end Gordian
