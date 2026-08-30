import Std
import Gordian.Ids
import Gordian.Authority

namespace Gordian

/-- Mirrors docs/spec/mission-graph.md#readiness-predicate-definitions. -/
structure AtomSpecView where
  specRevision : SpecRevisionId
  digestMatches : Bool
  specRulesHold : Bool
  preconditions : List Bool          -- EvalPredicate results over ObservedProjectState
  hardDeps : List AtomId
  requiredInterfaces : List ResourceId
  declaredInputs : List ResourceId
  semanticWrites : List ResourceId
  semanticReads : List ResourceId
  requiredExecutorClass : String
  requiredCapabilities : List String
  effectClass : String
  resourceRequirements : List (String × Nat)
  deriving Repr

structure ExecutorDescriptor where
  executorClass : String
  capabilities : List String
  permittedEffectClasses : List String
  concurrencyLimit : Nat
  activeAttempts : Nat
  quiesced : Bool
  deriving Repr

structure ResourcePool where
  dimension : String
  capacity : Nat
  reserved : Nat
  deriving Repr

structure LeaseView where
  subjectResource : Option ResourceId
  mode : String                      -- "read" | "shared" | "exclusive"
  heldByOther : Bool
  live : Bool
  operationCommutative : Bool
  deriving Repr

structure Environment where
  satisfied : AtomId → Bool
  providedBySatisfied : ResourceId → Bool
  producedBySatisfied : ResourceId → Bool
  externallyProvided : ResourceId → Bool
  executors : List ExecutorDescriptor
  pools : List ResourcePool
  leases : List LeaseView
  dispatchGrantValid : Bool
  irreversibleGrantValid : Bool

def validSpec (s : AtomSpecView) : Bool :=
  s.digestMatches && s.specRulesHold

def preconditionsHold (s : AtomSpecView) : Bool :=
  s.preconditions.all id

def blocked (env : Environment) (s : AtomSpecView) : Bool :=
  s.hardDeps.any (fun d => ! env.satisfied d)
  || s.requiredInterfaces.any
       (fun q => ! (env.providedBySatisfied q || env.externallyProvided q))
  || s.declaredInputs.any
       (fun i => ! (env.producedBySatisfied i || env.externallyProvided i))

def compatibleExecutorAvailable (env : Environment) (s : AtomSpecView) : Bool :=
  env.executors.any fun e =>
    ! e.quiesced
    && e.executorClass == s.requiredExecutorClass
    && s.requiredCapabilities.all (fun c => e.capabilities.contains c)
    && e.permittedEffectClasses.contains s.effectClass
    && e.activeAttempts < e.concurrencyLimit

def resourcesAvailable (env : Environment) (s : AtomSpecView) : Bool :=
  s.resourceRequirements.all fun req =>
    env.pools.any fun p => p.dimension == req.1 && req.2 ≤ p.capacity - p.reserved

def authorizationValid (env : Environment) (s : AtomSpecView) : Bool :=
  env.dispatchGrantValid
  && (s.effectClass != "irreversible" || env.irreversibleGrantValid)

def leaseCompatible (env : Environment) (s : AtomSpecView) : Bool :=
  s.semanticWrites.all (fun w =>
    ! env.leases.any fun l =>
        l.live && l.heldByOther && l.subjectResource == some w
        && (l.mode == "exclusive" || (l.mode == "shared" && ! l.operationCommutative)))
  && s.semanticReads.all (fun r =>
    ! env.leases.any fun l =>
        l.live && l.heldByOther && l.subjectResource == some r && l.mode == "exclusive")

def enabled (env : Environment) (s : AtomSpecView) : Bool :=
  validSpec s && (! blocked env s) && preconditionsHold s

def dispatchable (env : Environment) (s : AtomSpecView) : Bool :=
  enabled env s
  && compatibleExecutorAvailable env s
  && resourcesAvailable env s
  && authorizationValid env s
  && leaseCompatible env s

theorem dispatchable_implies_enabled {env s}
    (h : dispatchable env s = true) : enabled env s = true := by
  simp [dispatchable, Bool.and_eq_true] at h
  exact h.1.1.1.1

theorem dispatchable_implies_dependencies_satisfied {env s}
    (h : dispatchable env s = true) : ∀ d ∈ s.hardDeps, env.satisfied d = true := by
  intro d hd
  have he : enabled env s = true := dispatchable_implies_enabled h
  have hb : blocked env s = false := by
    simp [enabled, Bool.and_eq_true] at he
    exact he.1.2
  simp [blocked, List.any_eq_false] at hb
  exact hb.1.1 d hd

theorem dispatchable_implies_preconditions {env s}
    (h : dispatchable env s = true) : preconditionsHold s = true := by
  have he : enabled env s = true := dispatchable_implies_enabled h
  simp [enabled, Bool.and_eq_true] at he
  exact he.2

theorem dispatchable_implies_authorization {env s}
    (h : dispatchable env s = true) : authorizationValid env s = true := by
  simp [dispatchable, Bool.and_eq_true] at h
  exact h.1.2

/-- The interface clause of Blocked is load-bearing: a required interface that no
Satisfied Atom provides and that has no ExternalProvision blocks dispatch. -/
theorem dispatchable_implies_interfaces_provided {env s}
    (h : dispatchable env s = true) :
    ∀ q ∈ s.requiredInterfaces,
      env.providedBySatisfied q = true ∨ env.externallyProvided q = true := by
  intro q hq
  have he : enabled env s = true := dispatchable_implies_enabled h
  have hb : blocked env s = false := by
    simp [enabled, Bool.and_eq_true] at he
    exact he.1.2
  simp [blocked, List.any_eq_false] at hb
  cases hp : env.providedBySatisfied q
  · exact Or.inr (hb.1.2 q hq hp)
  · exact Or.inl rfl

end Gordian
