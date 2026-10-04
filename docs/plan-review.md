# Review of the research plan

This records the review that accompanied the reset of the repository to the charter in
[`charter.md`](charter.md). It lists what the plan gets right, what it leaves underspecified, and
what was changed when the plan became the charter.

## What the plan gets right

- **It gives Gordian a falsifiable objective.** "Better verified decisions under a fixed budget
  than strong simpler alternatives" can lose. The old project had no comparable statement.
- **It separates chosen from tested.** Hexagonal boundaries are a requirement; their effect on
  quality is a hypothesis. Keeping those apart is what lets EXP-003 reject one without the other.
- **It names strong baselines.** Random activation at matched compute and the matched-capacity
  monolith are the two that most architectures never face. Both stay mandatory.
- **It anticipates the selection-bias trap** (a scheduler that only sees the outcomes of its own
  choices) and prescribes offline forks with charged cost.
- **It demands coverage next to error** and refuses proxy metrics such as "thoughts generated."

## Gaps the charter closes

1. **Cost units were undefined.** The value-of-computation objective subtracts milliseconds,
   bytes, and probes. The charter requires each experiment to preregister exchange rates into
   utility units or to treat each resource as a hard constraint. Without this, EXP-001's `S`
   is not computable.
2. **"Critical miss" was undefined.** The charter requires the small world to label a critical
   fault class at generation time and to score misses on it under a separate preregistered bound.
3. **Power was never mentioned.** A margin of one percentage point on episode success needs a
   sample size that must be computed before freezing. The charter makes power analysis part of
   the freeze; underpowered results are "unresolved," not "equivalent."
4. **Multiplicity.** Seven experiments with many secondary measures invite a positive somewhere.
   One primary comparison per experiment; secondaries are labelled exploratory.
5. **The second domain could share bugs with the first.** The charter requires the second
   simulator to share no generator or evaluator code with the first.
6. **Interaction experiments were mentioned but not named.** EXP-I01 (memory × scheduling) is
   preregistered before EXP-004 runs so that "memory helped" cannot be an afterthought.
7. **Fork rate for counterfactual evidence** is preregistered rather than left to tuning.
8. **"Verified decision" needed a definition.** It is a decision the independent evaluator
   checks against hidden simulator state, never against another model's opinion.

## Places where I disagree or would go further

- **EXP-001 should run before any learned component exists.** The plan's build order already
  implies this, but the experiment list could be read as requiring the learned estimator. Run
  EXP-001 with purely heuristic components first; the result is cleaner and cheaper.
- **The 20% compute-saving margin is arbitrary until exploration estimates variance.** Treat it as
  a placeholder. The plan says this; the charter enforces it by refusing confirmatory runs whose
  margins were not frozen after an exploration phase.
- **EXP-007 should be contingent, not scheduled.** If EXP-001 and EXP-002 return "harmful" or
  "equivalent," there is no mechanism worth quantizing. The charter orders it last and gates it.
- **The $30 operating target is a deployment constraint, not a research constraint.** The charter
  keeps operating, training, and engineering cost separately visible so that the target cannot
  be met by hiding training cost.

## What was dropped from the previous project

The previous repository was a coordination substrate for multi-agent software engineering: a
Mission Graph ontology, Jujutsu-based change management, Lean proposition models, a knowledge
graph crate, GitHub Project synchronisation, and roughly forty CI checker scripts. None of it
tests a hypothesis about resource-bounded cognition, and keeping it would make the new question
inherit its vocabulary. The license (Apache-2.0) and the Rust toolchain pin are retained. The
old history remains reachable in git.

## Unavailable input

The plan referenced a "Gordian Research Charter and Experimental Protocol" document in an
external sandbox that could not be read from this session. The charter here was reconstructed
from the plan text alone. If that document differs, reconcile it into `charter.md` rather than
keeping two versions.
