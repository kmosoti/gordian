# Gordian Research Charter and Experimental Protocol

This document is the normative statement of what Gordian is for, what it must demonstrate, and
how a claim about it becomes evidence. Everything else in the repository serves it.

**Revision record.** Section 1.1 was added on 2026-10-06 following `docs/charter-aim-proposal.md` as approved by the user. Sections 1, 5, 6, 7 and 12 were revised on 2026-10-05, after Stage A and the
Stage B exploration, following `docs/charter-revision-proposal.md` as approved by the user. The
reasons are in `docs/review-log.md` (Stage B through A6d). The previous text is in git history. The
original EXP-001 to EXP-007 were never frozen; they are retired or re-scoped in section 6, and every
new experiment has a new id.

## 1. The research question

Gordian's scientific foundation is **resource-bounded cognition**: deciding what to compute, what
information to retain, and when further computation is no longer worth its cost.

That question becomes real only when computations differ in cost by orders of magnitude and the
expensive one is sometimes necessary. Gordian's arena is therefore the control of expensive
reasoning: a persistent, event-driven **substrate** that decides what deserves the attention of an
expensive reasoner (in practice, a large language model), what to remember, and which reasoning
resource to use.

> **Can a sparse, persistent, event-driven substrate decide what deserves expensive reasoning,
> what to remember, and which reasoning resource to use, and so achieve better verified decisions
> per unit of total cost than an LLM-centred agent using conventional memory and routing?**

- **H0:** an LLM-centred agent with strong conventional mechanisms (thresholds, statistical change
  detection, retrieval memory, summaries, a learned classifier router) performs as well or better
  at matched total cost.
- **H1:** the substrate plus the same reasoner achieves a better quality-to-cost trade-off, beyond
  preregistered margins.

A negative answer is a useful result. The charter is written so that the question can fail.

The substrate is judged on four functions:

| Function | What the substrate decides | Strongest conventional alternative |
|---|---|---|
| Salience | Whether an event deserves escalation | Threshold or anomaly score; statistical change detection |
| Routing | Which resource to use (cheap component, small model, large model, solver) | Learned classifier router; fixed cascade |
| Memory | What persists and what is recalled when | Retrieval over embeddings; rolling summary |
| Context construction | What the reasoner receives | Full history; truncated window; summary; retrieval |

**Candidate mechanism, not thesis.** The leading candidate is a cell substrate: stateful cells
`(type, parameters, state)` that share a library of archetype functions, typed gated connections
that compose cells into a transient program per event, and a shared field of context. The thesis
is about the four functions. If classical mechanisms match the cell substrate on all four, the cell
model is not retained, and each structural property (persistence, event-driven sparsity, per-event
composition, archetype sharing) must cost something measurable when ablated.

**What the substrate is not asked to do.** It does not replace the reasoner: language, broad
knowledge and explicit planning stay there. It is not scaled until it earns its cost at small
scale; storage at scale is dominated by connections, not functions, and connection sharing must
be measured before any scale claim.

Working description of the design:

> A persistent, event-driven control layer with resource-aware computation selection and explicit
> representation boundaries, around an expensive reasoner.

That is a starting point grounded in prior work, not a novelty claim.

### 1.1 The aim and its proxies

The program's aim is a different kind of intelligence rather than the status quo: capability at or
above the best conventional systems, at resource needs near a human's. "Superintelligence" is not
measurable; the aim is made testable through three resource gaps and two proxies.

| Resource | Human (order of magnitude) | Conventional system | Gap |
|---|---|---|---|
| Energy | 20 W | kilowatts per inference node | about 10³ |
| Data | 10⁹ words in a lifetime | 10¹³ or more tokens in training | about 10⁴ |
| Compute per decision | 10¹⁵ synaptic events per second available, nearly all idle | dense: every parameter every token | sparsity, not rate |

These figures are from memory and must be checked against primary sources (section 13) before any
report cites them. The data gap is the largest and the one the status quo has no answer for.

A result counts toward the aim only if it holds under hard resource limits and reports both:

1. **Decisions per unit cost:** verified correct decisions per unit of total modelled cost
   (section 4), with an energy proxy beside it (joules per correct decision) under one conversion
   fixed in this charter once it is calibrated, not per experiment. Until it is fixed, every use
   of the proxy is labelled as a placeholder.
2. **Improvement per unit experience:** the slope of correct decisions against incidents seen,
   compared with a matched conventional learner on the same stream. A mechanism that only matches
   the status quo at equal experience, or needs more experience to match it, does not count
   toward the aim, whatever its cost.

In practice: every preregistered experiment reports sample efficiency and the energy proxy for
every arm; the learned substrate is tested on improvement per experience as soon as hand-designed
gating earns its cost (section 12), not after EXP-102 and EXP-103; and the conventional
comparators stay in every experiment, because the aim is shown by beating them, not by removing
them. If a learned substrate, given public observations and its own history only, cannot beat the
tuned threshold rung at noticing within a few hundred streams, the cell model has no claim to
sample efficiency on this world, and the aim is pursued through another mechanism.

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
| Composing stateful functions per event | Graph networks (Battaglia et al.); message-passing aggregation (Xu et al.); neural module networks (Andreas et al.) | A shared update over node state, edge functions and global context; per-input composition of modules | That a cell substrate beats a monolithic model of equal capacity |
| One function, many stateful instances | Weight sharing in convolution; neural cellular automata (Mordvintsev et al.) | Archetype sharing works for parameters | That connectivity can be shared as cheaply |
| Conditional routing at scale | Sparsely gated mixtures of experts (Shazeer et al.; Fedus et al.) | Conditional computation works; known failures are router collapse, imbalance and overhead | That routing overhead stays small relative to what it saves |
| Escalating from cheap to expensive models | Model cascades and learned routers (FrugalGPT, Chen et al.; RouteLLM, Ong et al.); learning to defer (Madras et al.) | Large cost reductions at matched quality on benchmark tasks | That a persistent substrate beats a learned router |
| Memory around an LLM | MemGPT (Packer et al.); generative agents (Park et al.) | Practical memory management and importance-scored retrieval | That substrate state beats retrieval memory |
| Fast persistent learning beside slow broad learning | Complementary learning systems (McClelland, McNaughton, O'Reilly) | A principled division of labour between fast episodic and slow general learning | Any particular engineering realization |
| Learning gated by a global signal | Three-factor learning rules (Frémaux and Gerstner) | Local plasticity modulated by a global signal | That local rules can learn routing for our tasks |

**The unproven part is the combination under Gordian's constraints.** None of these sources
establishes that Gordian's event protocol, representations, scheduler, and learned components
outperform a simpler system.

### Three earlier recommendations are demoted to hypotheses

1. **"Semantic messages are preferable to shared latent representations."** Sometimes. A semantic
   interface can discard information a downstream component needs. Explicit, versioned latent
   projections remain a legitimate alternative. Tested by EXP-105.
2. **"Multidimensional salience beats a scalar threshold."** Not automatically. A feature vector
   usually collapses to a scalar decision anyway. The question is whether it retains the right
   information about task, component, uncertainty, and cost. Tested by EXP-101.
3. **"Inactive components consume almost nothing."** Requires measurement. Resident memory,
   sensing, feature extraction, queueing, and scheduling all belong in the bill. Measured by
   the counted-operation cost built in Stage A; tested by EXP-105.

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

Do not begin with open conversation judged by another model. Use a controlled digital environment
whose hidden state and outcomes are available to an independent evaluator. **No model ever judges
an outcome.** Verified decisions are checked against hidden simulator state.

The first small world (software-diagnosis episodes, built in Stage A) stays as built and remains the
regression environment. Stage B showed it offers almost nothing to select: every computation is
cheap and none is both expensive and decisive. The revised world adds three properties.

| Property | What it means | Why |
|---|---|---|
| A resource ladder | The existing components are the cheap rung. A **simulated reasoner** has a declared cost several orders of magnitude higher; its accuracy depends on the incident's difficulty and on the quality of the context it receives. A small real-model rung is added for validation only | Metareasoning matters only when costs span orders of magnitude and the expensive computation is sometimes necessary |
| Persistent streams | One long-running world with many incidents, recurring faults, configuration drift and changes of regime, instead of isolated episodes | Persistence, memory and adaptation can only matter across time |
| Relevance that costs computation to judge | Noise is not separable for free; some decisive evidence is spread across components with partial views; most incidents do not need the expensive rung and some do | Salience has nothing to do if relevance is free to detect |

**The simulated reasoner encodes an assumption** ("better context gives better answers"). Its
accuracy model is a set of swept parameters, never a single setting, and every conclusion must hold
across the sweep and be checked with a real model (EXP-106).

**Headroom check before any freeze.** The privileged oracle escalation policy must beat the best
non-privileged baseline by a preregistered margin on the classes an experiment targets, or the world
is revised first.

**Software diagnosis is the first arena, not Gordian's identity.** After measurements are reliable,
add a second, *independently implemented* domain to test transfer. The second simulator must not
share generator or evaluator code with the first.

## 6. Experiment definitions

Every experiment follows `experiments/TEMPLATE.md`. Margins are **proposed engineering margins to
be fixed before the confirmatory run**, not predictions. Each experiment states what would count
against the idea.

### EXP-101 — Escalation control

- **Question:** does substrate salience decide when to escalate better than the conventional
  policies?
- **Arms:** never, always, periodic, change-triggered, threshold or anomaly-score, learned
  classifier router, substrate with hand-designed gating; privileged oracle escalation for
  headroom.
- **Primary measure:** verified decisions per unit of total cost, with critical misses under a
  separate preregistered bound. Report escalation precision and recall.
- **H0:** the substrate does not beat the best conventional policy beyond the margin at matched
  total cost. **H1:** it does.
- **Counts against:** a threshold-plus-cache policy ties. That outcome is predicted as likely and
  is a useful result.

### EXP-102 — Context construction

- **Question:** does substrate-built structured context let the reasoner decide as well as full
  history, at fewer tokens?
- **Arms:** full history, truncated window, rolling summary, retrieval, substrate context; same
  reasoner and parameter sweep.
- **Primary measure:** reasoner accuracy per token.
- **Counts against:** summary or retrieval ties at matched tokens.

### EXP-103 — Memory

- **Question:** does persistent substrate state beat retrieval memory on recurring incidents, stale
  records and drift?
- **Arms:** none, retrieval plus summary, substrate state, and a shuffled-retrieval control.
- **Primary measure:** cost and time to a correct decision on recurrences; stale-memory errors
  scored separately.
- **Counts against:** the benefit vanishes when memory and input budgets are matched, or stale
  memory causes persistent false conclusions.

### EXP-104 — Adaptation

- **Question:** after a change of regime, how fast does each system recover, and what does it
  forget?
- **Primary measures:** recovery time to a preregistered quality level; forgetting on the old
  regime.

### EXP-105 — Structure

- **Question:** do the cell properties matter?
- **Arms:** ablate persistence, event-driven sparsity, per-event composition and archetype sharing,
  against a matched-capacity monolithic recurrent model.
- **Primary measure:** the quality-to-cost frontier per ablation.
- **Counts against:** the monolithic model matches the substrate. Then the cell structure is an
  implementation choice, not a finding.

### EXP-106 — Real-model transfer

- **Question:** do the conclusions of EXP-101 and EXP-102 hold with a real model in place of the
  simulated reasoner?
- **Primary measure:** agreement of result category (beneficial, harmful, equivalent,
  unresolved).
- Sized to the operating budget before freezing. Every call is recorded at the boundary and never
  repeated to replay state.

### Retired and re-scoped definitions

The original EXP-001 to EXP-007 and EXP-I01 were never frozen. Their text is in git history.

| Old id | Disposition |
|---|---|
| EXP-001 selective activation | Retired. Stage B showed the first world leaves a tuned periodic baseline little to beat; recorded as an exploration finding. Its question returns as EXP-101 |
| EXP-002 richer salience | Re-scoped into EXP-101: salience features are EXP-101 arms and ablations |
| EXP-003 representation isolation | Re-scoped into EXP-105 |
| EXP-004 memory, EXP-I01 | Re-scoped into EXP-103 |
| EXP-005 world model, EXP-006 recurrent depth | Deferred until EXP-105 reports |
| EXP-007 low precision | Deferred indefinitely |

## 7. Baseline registry

A weak baseline makes any architecture look impressive. Every confirmatory experiment includes
the applicable rows.

| Baseline | What it challenges |
|---|---|
| Never escalate (cheap components and shared rule only) | Whether the expensive reasoner is needed at all |
| Always escalate | The quality ceiling at full cost; not assumed optimal |
| Periodic escalation, tuned | Whether timing needs to be state-dependent |
| Change-triggered execution (recompute only on input change) | Whether salience adds anything beyond skipping unchanged inputs |
| Threshold or anomaly-score escalation | Whether a single scalar statistic suffices |
| Learned classifier router (cascade) | Whether the substrate beats the established routing literature |
| Retrieval memory plus summary | Whether substrate memory beats conventional agent memory |
| Random escalation at matched cost | Whether selection matters, rather than merely escalating less |
| Matched-capacity monolithic recurrent model | Whether the cell structure matters, or only total capacity |
| Oracle escalation (privileged) | Whether the environment has measurable headroom |

Run both controlled ablations inside Gordian's engine *and* a simpler end-to-end operational
baseline. The simple baseline must not be made to pay for Gordian-specific machinery it does not
need. Stage B's lesson applies to every row: tune each baseline before comparing, per budget level.

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
| Measurement first (done) | Small world, evaluator, baselines, deterministic core, counted-operation cost | Can we measure correctness and cost reliably? |
| Exploration (done) | Stage B on the first world | Does the first world offer anything to select? It does not |
| World revision | Resource ladder, simulated reasoner, persistent streams, costly relevance; headroom check | Is there a real escalation problem with headroom? |
| Conventional baselines | Every section 7 row that needs no learning | How far do simple policies go? |
| Substrate prototype | Cells, typed gated connections, field; hand-designed gating | Does composition earn its cost before any learning? |
| Escalation and context | EXP-101, EXP-102 | Does the substrate decide better what deserves thought? |
| Memory, adaptation, structure | EXP-103, EXP-104, EXP-105, then learning | Which properties earn their cost; can routing be learned? |
| Real-model transfer | EXP-106 | Do the conclusions survive a real reasoner? |

No stage requires cloud GPUs or a custom foundation model. Real-model work uses a local small model
on this CPU where possible; a remote model costs money and needs credentials this environment does
not have. The approximate $30 operating target constrains deployment evaluation, but **operating
cost, training cost, reasoner cost, and engineering effort stay separately visible.** Total cost in
every experiment is modelled substrate cost plus modelled rule cost plus reasoner cost; reasoner
cost is reported in its own units (calls, tokens, declared latency) and through a preregistered
exchange rate (section 4).

### The first meaningful result

> On held-out streams, the substrate escalates to the expensive reasoner less often than the best
> tuned conventional policy, at no loss of verified quality beyond a preregistered margin and no
> increase in critical misses, with the conclusion holding across the simulated reasoner's
> parameter sweep and reproducing from a frozen experiment record.

Gordian does not begin by proving it is an intelligence. It begins by demonstrating that a specific
mechanism decides better what deserves thought, and retains that mechanism only when the evidence
supports it.

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
| Battaglia et al. | P. Battaglia et al., "Relational inductive biases, deep learning, and graph networks," 2018. arXiv:1806.01261 |
| Xu et al. | K. Xu, W. Hu, J. Leskovec, S. Jegelka, "How Powerful are Graph Neural Networks?," ICLR 2019. |
| Andreas et al. | J. Andreas, M. Rohrbach, T. Darrell, D. Klein, "Neural Module Networks," CVPR 2016. |
| Mordvintsev et al. | A. Mordvintsev, E. Randazzo, E. Niklasson, M. Levin, "Growing Neural Cellular Automata," Distill, 2020. |
| Shazeer et al. | N. Shazeer et al., "Outrageously Large Neural Networks: The Sparsely-Gated Mixture-of-Experts Layer," ICLR 2017. |
| Fedus et al. | W. Fedus, B. Zoph, N. Shazeer, "Switch Transformers," 2021. arXiv:2101.03961 |
| Chen et al. | L. Chen, M. Zaharia, J. Zou, "FrugalGPT," 2023. arXiv:2305.05176 |
| Ong et al. | I. Ong et al., "RouteLLM: Learning to Route LLMs with Preference Data," 2024. |
| Madras et al. | D. Madras, T. Pitassi, R. Zemel, "Predict Responsibly: Improving Fairness and Accuracy by Learning to Defer," NeurIPS 2018. |
| Packer et al. | C. Packer et al., "MemGPT: Towards LLMs as Operating Systems," 2023. |
| Park et al. | J. S. Park et al., "Generative Agents: Interactive Simulacra of Human Behavior," UIST 2023. |
| McClelland, McNaughton, O'Reilly | "Why there are complementary learning systems in the hippocampus and neocortex," Psychological Review 102(3), 1995. |
| Frémaux and Gerstner | N. Frémaux, W. Gerstner, "Neuromodulated Spike-Timing-Dependent Plasticity, and Theory of Three-Factor Learning Rules," Frontiers in Neural Circuits, 2016. |
| Kirkpatrick et al. | J. Kirkpatrick et al., "Overcoming catastrophic forgetting in neural networks," PNAS, 2017. |
| Davies et al. | M. Davies et al., "Loihi: A Neuromorphic Manycore Processor with On-Chip Learning," IEEE Micro, 2018. |
| Wang (NARS) | P. Wang, "Non-Axiomatic Reasoning System" (the assumption of insufficient knowledge and resources; priority-driven selection with feedback). **To acquire and check:** edition and primary text not yet identified; added 2026-10-06 as mandatory prior art for sections 1 and 4. |
| Kanerva (VSA) | P. Kanerva, "Hyperdimensional Computing: An Introduction to Computing in Distributed Representation with High-Dimensional Random Vectors," Cognitive Computation 1(2), 2009. **To acquire and check**; a candidate memory representation for EXP-103. |
| McSherry et al. | F. McSherry, D. Murray, R. Isaacs, M. Isard, "Differential dataflow," CIDR 2013. **To acquire and check**; the incremental-computation adversary of unit C1. |
| Jaeger | H. Jaeger, "The 'echo state' approach to analysing and training recurrent neural networks," GMD Report 148, 2001. **To acquire and check**; the reservoir comparator of unit L2. |

Citations are to be verified against the primary text before any claim in this repository relies
on them; a secondary summary is not acquisition.
