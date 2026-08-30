import Std

namespace Gordian

inductive EffectClass where
  | pure | hermetic | externalRead | idempotentWrite
  | compensatableWrite | irreversible | judgment
  deriving Repr, DecidableEq

inductive RetryRule where
  | free
  | freeRecreatingInputs
  | newObservation
  | requiresIdempotencyKey
  | requiresCompensation
  | manualOnly
  | newJudgmentArtifact
  deriving Repr, DecidableEq

/-- Total: no wildcard arm, so adding a class is a compile error.
Mirrors docs/algorithms/reconciliation.md#8-retry-semantics-depend-on-effects. -/
def retryPolicy : EffectClass → RetryRule
  | .pure => .free
  | .hermetic => .freeRecreatingInputs
  | .externalRead => .newObservation
  | .idempotentWrite => .requiresIdempotencyKey
  | .compensatableWrite => .requiresCompensation
  | .irreversible => .manualOnly
  | .judgment => .newJudgmentArtifact

theorem irreversible_never_auto_retried :
    retryPolicy .irreversible = .manualOnly := by rfl

theorem judgment_never_overwrites :
    retryPolicy .judgment = .newJudgmentArtifact := by rfl

end Gordian
