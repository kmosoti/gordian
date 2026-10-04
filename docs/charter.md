# Gordian Research Charter and Experimental Protocol

This document is the normative statement of what Gordian is for, what it must demonstrate, and
how a claim about it becomes evidence. Everything else in the repository serves it.

## 1. The research question

Gordian's scientific foundation is **resource-bounded cognition**: deciding what to compute, what
information to retain, and when further computation is no longer worth its cost.

The event core, isolated component representations, and salience-driven selection are
**testable mechanisms**. They are not evidence that intelligence emerges from connecting enough
components.

> **Can specialized components with private internal representations, coordinated through events
> and selective activation, produce better verified decisions under a fixed resource budget than
> strong simpler alternatives?**

A negative answer is a useful result. The charter is written so that the question can fail.

Working description of the initial design:

> A blackboard-inspired cognitive architecture with resource-aware computation selection and
> explicit representation boundaries.

That is a starting point grounded in prior work, not a novelty claim.

## 2. Foundations we borrow, and what each actually supports

| Gordian idea | Established foundation | What the evidence supports | What it does not establish |
|---|---|---|---|
| Isolated components and adapters | Cockburn, hexagonal architecture | Separating application behaviour from implementation technology; alternate test implementations | Any improvement in decision quality |
| Specialists coordinating through shared state | Hearsay-II and blackboard systems (Erman et al.) | Coordinating heterogeneous knowledge sources via a shared problem representation and a control mechanism | That the approach scales or beats a tuned pipeline |
| Attention selecting relevant internal content | LIDA (Franklin et al.); computational salience (Itti, Koch, Niebur) | Concrete architectures for perception, memory, attention competition, selective processing | General cognition |
| Choosing which computation to perform | Rational metareasoning (Russell and Wefald); learned computation selection (Hay et al.; Callaway et al.) | Treating a computation as a decision with expected benefit and cost; tractable approximations | That a cheap estimator of value of computation exists for our tasks |
| Maintaining state under incomplete observation | POMDPs (Kaelbling, Littman, Cassandra) | Reasoning about hidden state rather than treating observations as complete reality | Tractability for our state spaces |
| Predicting consequences before acting | Learned world models, including Dreamer (Hafner et al.) | Usefulness of learned dynamics within evaluated control tasks | That a world model helps diagnosis tasks |
| Reusing computation at inference time | Recurrent-depth models (Geiping et al.) | Additional latent computation can improve specific reasoning results under specific training and evaluation conditions | That the effect survives at tiny scale |

**The unproven part is the combination under Gordian's constraints.** None of these sources
establishes that Gordian's event protocol, representations, scheduler, and learned components
outperform a simpler system.

### Three earlier recommendations are demoted to hypotheses

1. **"Semantic messages are preferable to shared latent representations."** Sometimes. A semantic
   interface can discard information a downstream component needs. Explicit, versioned latent
   projections remain a legitimate alternative. Tested by EXP-003.
2. **"Multidimensional salience beats a scalar threshold."** Not automatically. A feature vector
   usually collapses to a scalar decision anyway. The question is whether it retains the right
   information about task, component, uncertainty, and cost. Tested by EXP-002.
3. **"Inactive components consume almost nothing."** Requires measurement. Resident memory,
   sensing, feature extraction, queueing, and scheduling all belong in the bill. Tested by
   EXP-001's cost accounting.

## 3. Engineering requirements versus empirical hypotheses

Some decisions are chosen; others are tested.

**Chosen (engineering requirements):** explicit component boundaries, replaceable
implementations, representation ownership, a deterministic reference core, hard resource limits
enabled in every condition.

**Tested (empirical hypotheses):** whether those boundaries preserve performance, what overhead
they add, whether they constrain learning, and whether any of the cognitive mechanisms earns its
cost.

### 3.1 Three kinds of state

**Event ledger: what entered and what happened.** Append-only record of admitted observations,
computation requests, results, decisions, and outcomes, with provenance: producer version, input
references, timing. Its references establish *recorded dependencies*, not real-world causation.

**Working state: what currently matters.** A bounded view: active task, relevant entities,
unresolved hypotheses, deadlines, pending computations. A component must not need to reread the
system's entire life to answer its next question.

**Private component state: how each component represents things.** A distribution, recurrent
state, graph, tensor, index, or cache. Other components interact through an explicit contract,
never through assumptions about that layout.

> **No accidental representation coupling. Explicit representation exchange is allowed.**

### 3.2 Perception can itself be inference

A measured counter is a *measurement*. "This counter pattern indicates credential expiry" is a
*hypothesis*. The ledger types must keep these distinct; calling both `Observation` erases the
distinction the whole program depends on.

## 4. Salience has a job: estimating value of computation

The objective for selecting a candidate computation `c`, given history and state `h` and task `g`:

```text
V(c | h, g) = E[ ΔU_task | h, g, c ] − Cost(c) − DelayPenalty(c)
```

- `ΔU_task` is the expected improvement in the downstream verified decision.
- `Cost` and `DelayPenalty` MUST be expressed in declared utility units or handled as separate
  hard constraints. Each experiment preregisters the exchange rate (for example utility per
  millisecond, per byte-second of resident memory, per probe). Adding milliseconds, dollars, and
  bytes without a declared policy is decorative mathematics.

**Salience features estimate `V`. They are not the objective.** Candidate features: novelty,
uncertainty, goal relevance, source reliability, evidence age, persistence, prediction residual,
expected runtime.

### Why surprise alone is insufficient

Input A: a constant stream of unpredictable random messages. Input B: one quiet indication that a
dependency is unavailable. A surprise-driven selector spends its budget on A. A useful system
attends to B when the task needs it. Prediction-error curiosity has documented failure in
stochastic environments (Burda et al.); the noise-flood stressor in section 9 is the ready-made
adversarial test.

### Why greedy one-step selection is insufficient

Two checks may be jointly decisive while each is individually worthless. Metareasoning
literature names this limitation of greedy selection (Hay et al.). The small world MUST contain
such episodes so that "run the highest score next" can lose.

### Initial proposal

**Cheap, task-conditioned selection with explicit stopping and hard resource limits.** Any more
sophisticated learned scheduler must beat that baseline at matched budget.

## 5. The small world: a domain where correctness is knowable

Do not begin with open conversation judged by another model. Begin with a controlled digital
environment whose hidden state and outcomes are available to an independent evaluator.

| Element | Initial implementation |
|---|---|
| Hidden world | Small dependency graph with resources, configuration, injected faults |
| Senses | Counters, event messages, state snapshots, optional diagnostic probes |
| Task | Identify the hidden condition; select a useful probe or simulated correction |
| Components | Heuristic analyzer, probabilistic estimator, small predictor, memory lookup, verifier |
| Ground truth | Simulator hidden state and actual consequences |
| Constraints | Limited time, computation, memory, probe budget |

Episode generators MUST include: ambiguous initial symptoms resolved only by a costly probe;
episodes requiring recall of an earlier configuration change; irrelevant noise; jointly decisive
check pairs; episodes with no fault (to measure false alarms); and a labelled **critical** fault
class whose misses are scored separately.

```text
partial evidence → state estimation → computation selection → verified decision
```

The first learned component estimates `P(fault | available history)`. A later dynamics model
estimates `P(next observation | state estimate, action)`. Neither requires deciding model scale
now.

**Software diagnosis is the first arena, not Gordian's identity.** After measurements are
reliable, add a second, *independently implemented* domain (partially observed switching
circuits, or dependency puzzles) to test transfer beyond the first simulator's conventions. The
second simulator must not share generator or evaluator code with the first.

## 6. Experiment definitions

Every experiment follows `experiments/TEMPLATE.md`. Margins below are **proposed engineering
margins to be fixed before the confirmatory run**, not predictions. Each experiment states what
would count against the idea.

### EXP-001 — Does selective activation earn its overhead?

- **Intervention:** scheduling policy only; components, weights, inputs, evaluator fixed.
- **Comparison:** selective activation vs a well-tuned fixed pipeline or periodic schedule.
- **Measures:** `D = Q_sel − Q_base` (verified episode success);
  `S = 1 − C_sel / C_base` where `C` includes sensing, scheduling, computation, communication,
  and relevant storage work.
- **H1:** `D > −0.01 ∧ S > 0.20`. **H0:** `D ≤ −0.01 ∨ S ≤ 0.20`.
- **Separate criterion:** critical-miss rate must not increase beyond its own preregistered bound.
- **Counts against:** selector overhead consumes the savings; or savings come from neglecting
  hard cases.

### EXP-002 — Does richer salience improve selection?

- **Arms:** novelty threshold; tuned weighted feature score; component-conditioned score; small
  learned cost-aware policy.
- **H0:** at matched end-to-end budget, the richer policy does not improve verified quality
  beyond the margin. **H1:** it does, without unacceptable critical misses or latency.
- **Stressors:** noise-heavy streams, goal changes, quiet urgent signals, complementary
  computations. Ablate novelty, uncertainty, goal relevance, reliability, history separately.
- **Counts against:** a weighted rule ties; or the learned policy wins only on familiar event
  distributions. One scalar utility estimator sufficing is a *useful* result.

### EXP-003 — Does representation isolation preserve capability?

| Variant | Exchange mechanism |
|---|---|
| Shared representation | Components consume an explicitly shared state representation |
| Semantic-only boundary | Typed interpreted messages |
| Explicit mixed boundary | Typed messages plus immutable payload references or versioned projections |

Two independent hypotheses. **Quality:** H0 the isolated boundary loses at least the margin; H1
it stays within it. **Engineering:** H0 under predefined component replacements the isolated
design does not meaningfully reduce adaptation burden; H1 it does. Measure task quality,
communication cost, memory, consumers requiring change, adaptation data, validation effort
separately. **Counts against semantic-only:** information discarded before another component
can use it.

### EXP-004 — Does memory help beyond retaining more input?

- **Arms:** fixed-window history; compressed recurrent state; explicitly budgeted retrieval.
  Keep recurrent working state and cross-episode retrieval as *separate* treatments.
- **H0:** no practically meaningful improvement over the best equally resourced history baseline.
  **H1:** improvement beyond margin.
- **Controls:** delayed evidence, changed environments, stale memories, contradictory records, and
  a **shuffled-retrieval control** (right experience vs merely more material).
- **Counts against:** benefit vanishes when memory and input budgets are matched; or stale memory
  produces persistent false conclusions.

### EXP-005 — Does a world model improve decisions?

- **Arms:** empirical transition table; learned dynamics; equally resourced model-free
  alternative with the same information.
- **H0:** learned dynamics does not improve downstream decisions beyond margin or violates quality
  constraints. **H1:** it does, within constraints.
- **Measures:** prediction quality across horizons, useful probe selection, decision regret,
  simulated outcomes. Prediction loss is never the sole success metric.
- **Counts against:** predictor gets more accurate while decisions stay flat or worsen.

### EXP-006 — Does adaptive recurrent depth beat fixed computation?

- **Arms:** shallow fixed; deeper fixed; adaptive recurrence; repeated execution without learned
  recurrent state. Run **parameter-matched** and **compute-matched** comparisons separately.
- **H0:** adaptive recurrence does not improve the quality-cost frontier beyond margin. **H1:** it
  does.
- **Measure:** whether extra iterations help, saturate, oscillate, or worsen.
- **Counts against:** a fixed iteration count ties once stopping-policy overhead is charged.
  Geiping et al. is evidence of an architectural possibility at 3.5B parameters and 800B tokens,
  not evidence the effect appears in a tiny run.

### EXP-007 — Does low precision improve this specific system?

Runs only after a mechanism is shown useful. **Arms:** validated higher-precision reference;
conventional quantization; ternary variants. **H0:** representation exceeds permitted quality
loss or fails the required resource improvement. **H1:** satisfies both. Measure total resident
memory, state/cache memory, actual latency on the target CPU, and task quality, not packed weight
size alone. A storage improvement is a storage improvement, never "more intelligence."

### Preregistered interaction experiments

Mechanisms may help only jointly. At least one interaction experiment is preregistered before
EXP-004 runs: **EXP-I01** memory × scheduling policy (2×2 at matched budget).

## 7. Baseline registry

A weak baseline makes any architecture look impressive. Every confirmatory experiment includes
the applicable rows.

| Baseline | What it challenges |
|---|---|
| Simple task-specific heuristic | Whether the task needs learning at all |
| Tuned fixed pipeline | Whether adaptive coordination is necessary |
| Random activation at matched compute | Whether selection matters, rather than merely doing less |
| Tuned cost-aware selector | Whether the salience mechanism adds anything |
| All-component execution | What selection misses; not assumed to be an accuracy ceiling |
| Matched-capacity monolithic estimator | Whether modularity helps the task or only the codebase |
| Small-world oracle (privileged) | Whether the environment has measurable headroom |

Run both controlled ablations inside Gordian's engine *and* a simpler end-to-end operational
baseline. The simple baseline must not be made to pay for Gordian-specific machinery it does not
need.

## 8. Statistical procedure

1. **Exploration then confirmation.** Development runs find defects, estimate variance, and
   choose margins. Then freeze hypothesis, primary comparison, test split, budget, exclusions,
   and stopping rule. A redesign motivates a new experiment; it never rewrites the original pass
   condition.
2. **An episode is an episode.** A million events from one trajectory are not a million
   replications. Distinguish environment instances, training runs, and inference randomness. Pair
   arms on the same held-out scenarios where appropriate. Report intervals and per-task behaviour
   (Agarwal et al.), not one victorious average.
3. **"Not significant" is not "equivalent."** Non-inferiority and equivalence need explicit
   margins and suitable tests (Lakens). Result categories: **beneficial, harmful, practically
   equivalent within margin, unresolved.** Underpowered experiments stay unresolved. A power
   analysis against the preregistered margin is part of freezing.
4. **One primary comparison per experiment.** Secondary comparisons are reported, labelled
   exploratory, and multiplicity-adjusted when used for any claim.
5. **Match more than parameter count.** Track training data, tuning trials, inference budget,
   memory, permitted observations. Freeze components for scheduler experiments; report training
   and inference cost separately for architecture experiments.
6. **Prevent evaluator leakage.** Hidden simulator state never enters policy inputs. Split whole
   environments and trajectories, not adjacent events. Test unseen dependency structures and
   changed dynamics, not new seeds on identical templates. Use independently checked tiny cases
   or an independently implemented evaluator.

## 9. Selective activation creates selective evidence

A component that runs produces an observable result; a skipped one usually does not. **A scheduler
trained only on the outcomes of its own selections can grow more certain its preferences were
right.**

In the simulator: periodically fork a state and execute the alternative computations offline,
producing evidence about what was missed. Keep those results out of the deployed policy's
immediate inputs, and **charge their cost** to the training budget. The fork rate is
preregistered.

For external actions, historical replay is insufficient: a recorded trajectory does not reveal
what the world would have done under a different action. This is the off-policy evaluation
problem (Dudík, Langford, Li). **Provenance is necessary for research. It is not a substitute for
counterfactual evidence.**

## 10. Mandatory stress suite

Run in every condition, with hard limits always enabled.

| Stressor | Failure to detect |
|---|---|
| Irrelevant high-entropy input | Attention captured by noise |
| Repeated or duplicated messages | Redundant computation |
| Internal feedback loops | Components repeatedly reactivating each other |
| Quiet urgent event during a flood | Starvation |
| Stale or contradictory memory | Confident reuse of invalid knowledge |
| Component failure or timeout | Unbounded waiting or budget leakage |

Report **answer coverage alongside error** (Geifman and El-Yaniv): a system that abstains on
everything looks accurate while being useless.

**Primary outcomes:** verified task success, total resources spent across all attempted episodes,
critical failures scored separately. Never optimize "number of thoughts," "hypotheses generated,"
or "events processed."

## 11. Reproducibility: three distinct claims

- **Protocol replay.** The deterministic reference core makes the same admission and scheduling
  decisions from the same ledger.
- **Numerical replay.** Component outputs stay within declared tolerances on a specified backend
  and target.
- **Statistical replication.** Independently trained runs produce compatible aggregates.

Do not collapse these into "same seed means same result."

Every experiment records: source revision, toolchain, dependency lock, environment, hardware
capabilities, model hashes, data split, seeds, scheduler policy, resource limits, result
artifacts.

Property tests and model checking cover bounded queues, budget accounting, cancellation,
provenance retention, and allowed state transitions. **They cannot establish that the salience
policy improves cognition.** That stays empirical.

### Workflow for automated research agents

```text
register the claim → implement the comparison → verify the evaluator → freeze → execute → report every outcome
```

An agent may implement alternatives and analyse results. It may not silently change the metric,
weaken the null, discard failed runs, or modify the hidden evaluator to make its implementation
pass.

## 12. Build order

| Stage | Deliverable | Question answered |
|---|---|---|
| Measurement first | Small world, evaluator, fixed baselines, explicit-clock core | Can we measure correctness and cost reliably? |
| Selective execution | Fixed components, competing scheduling policies | Does salience earn its complexity? |
| Representation boundaries | Controlled component replacements | Does isolation preserve information and reduce coupling? |
| Learned state and prediction | Small memory and dynamics experiments | Which learned mechanisms improve decisions? |
| Adaptive depth and precision | Recurrence and quantization comparisons | How should useful cognition be executed efficiently? |

The first stages require no cloud GPUs and no custom foundation model. Any provisioned training
sits behind a specific experiment with a finite budget, an artifact-return requirement, and a
cleanup policy. The approximate $30 operating target constrains deployment evaluation, but
**operating cost, training cost, and engineering effort stay separately visible.**

### The first meaningful result

> On held-out digital environments, Gordian's selective execution preserves verified quality
> within a preregistered margin, reduces measured total compute by a worthwhile amount, survives
> the stress suite, and reproduces from a frozen experiment record.

Gordian does not begin by proving it is an intelligence. It begins by demonstrating that a
specific mechanism makes better use of limited computation, and retains that mechanism only when
the evidence supports it.

## 13. Primary source register

| Key | Source |
|---|---|
| Cockburn | A. Cockburn, "Hexagonal Architecture," 2005. https://alistair.cockburn.us/hexagonal-architecture |
| Erman et al. | L. D. Erman, F. Hayes-Roth, V. R. Lesser, D. R. Reddy, "The Hearsay-II Speech-Understanding System," ACM Computing Surveys 12(2), 1980. |
| Franklin et al. | S. Franklin, T. Madl, S. D'Mello, J. Snaider, "LIDA: A Systems-level Architecture for Cognition, Emotion, and Learning," IEEE TAMD 6(1), 2014. |
| Itti, Koch, Niebur | L. Itti, C. Koch, E. Niebur, "A Model of Saliency-Based Visual Attention for Rapid Scene Analysis," IEEE TPAMI 20(11), 1998. |
| Russell and Wefald | S. Russell, E. Wefald, "Principles of Metareasoning," Artificial Intelligence 49, 1991. |
| Hay et al. | N. Hay, S. Russell, D. Tolpin, S. Shimony, "Selecting Computations: Theory and Applications," UAI 2012. |
| Callaway et al. | F. Callaway, S. Gul, P. Krueger, T. Griffiths, F. Lieder, "Learning to Select Computations," UAI 2018. arXiv:1711.06892 |
| Kaelbling, Littman, Cassandra | "Planning and Acting in Partially Observable Stochastic Domains," Artificial Intelligence 101, 1998. |
| Hafner et al. | D. Hafner, J. Pasukonis, J. Ba, T. Lillicrap, "Mastering Diverse Control Tasks through World Models," Nature, 2025. |
| Geiping et al. | J. Geiping et al., "Scaling up Test-Time Compute with Latent Reasoning: A Recurrent Depth Approach," 2025. arXiv:2502.05171 |
| Burda et al. | Y. Burda, H. Edwards, D. Pathak, A. Storkey, T. Darrell, A. Efros, "Large-Scale Study of Curiosity-Driven Learning," 2018. arXiv:1808.04355 |
| Agarwal et al. | R. Agarwal, M. Schwarzer, P. S. Castro, A. Courville, M. Bellemare, "Deep RL at the Edge of the Statistical Precipice," NeurIPS 2021. arXiv:2108.13264 |
| Lakens | D. Lakens, "Equivalence Tests: A Practical Primer," Social Psychological and Personality Science 8(4), 2017. |
| Dudík, Langford, Li | "Doubly Robust Policy Evaluation and Learning," ICML 2011. arXiv:1103.4601 |
| Geifman and El-Yaniv | "Selective Classification for Deep Neural Networks," NeurIPS 2017. arXiv:1705.08500 |
| Ma et al. | S. Ma et al., "The Era of 1-bit LLMs: All Large Language Models are in 1.58 Bits," 2024. arXiv:2402.17764 |

Citations are to be verified against the primary text before any claim in this repository relies
on them; a secondary summary is not acquisition.
