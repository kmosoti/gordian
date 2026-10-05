# Charter revision proposal: a metacognitive substrate around expensive reasoning

Status: **proposal, not adopted.** Nothing here is normative until the user approves it and the
charter is edited. Adopting it would replace charter sections 1, 5, 6, 7 and 12, and retire
EXP-001 as currently defined without freezing it. The old EXP-001 result would be recorded as an
exploration finding, and every new experiment would get a new id.

## 1. Why revise

The evidence comes from this repository's own runs, recorded in `docs/review-log.md`.

1. **The current world offers almost nothing to select.**
   - A periodic heuristic holds 0.954 success at every budget for about 22k modelled ns per
     episode.
   - Against that baseline a perfect component selector could save about 10–18%, below
     EXP-001's 20% margin (Stage B).
   - The components are redundant generalists. Every computation is cheap, and none is both
     expensive and decisive.
2. **Deciding what to compute cost more than computing.** The shared decision rule was 65–89% of
   the cheap arms' cost. Two later defects were the rule repeating work on unchanged inputs (A6c,
   A6d). Change-triggered execution is the cheapest form of selective activation, and the
   charter's baseline registry does not name it.
3. **The user's two framings answer both problems.**
   - Cells as stateful functions, composed per event, supply specialization.
   - Treating the substrate as the control layer around a large language model (LLM) supplies the
     missing expensive, decisive computation.

The metareasoning question becomes real only when computations differ in cost by orders of
magnitude and the expensive one is sometimes necessary. Stage B's components span about one order
of magnitude, from 2k to 100k ns per call. A local or remote LLM call costs seconds and real
money, roughly six to nine orders of magnitude more than a component. In that regime selection
overhead is affordable, and the accuracy of the decision to escalate becomes what matters. That
is the regime the charter's section 4 was written for and the current world never entered.

## 2. Revised research question

> **Can a sparse, persistent, event-driven substrate decide what deserves expensive reasoning,
> what to remember, and which reasoning resource to use, and so achieve better verified decisions
> per unit of total cost than an LLM-centred agent using conventional memory and routing?**

- **H0:** an LLM-centred agent with strong conventional mechanisms performs as well or better at
  matched total cost. The conventional mechanisms are thresholds, statistical change detection,
  retrieval memory, summaries, and a learned classifier router.
- **H1:** the substrate plus the same reasoner achieves a better quality-to-cost trade-off, beyond
  preregistered margins.

The cell model is a **candidate mechanism** for the substrate, not the thesis. The thesis is about
four functions:

| Function | What the substrate decides | Strongest conventional alternative |
|---|---|---|
| Salience | Whether an event deserves escalation | Threshold or anomaly score; statistical change detection |
| Routing | Which resource to use (cheap component, small model, large model, solver) | Learned classifier router; fixed cascade |
| Memory | What persists and what is recalled when | Retrieval over embeddings; rolling summary |
| Context construction | What the reasoner receives | Full history; truncated window; summary; retrieval |

If classical mechanisms match the cell substrate on all four, the cell model is not retained.
This avoids the danger the user named: rebuilding a hard-to-train sparse recurrent network with
nothing structurally new. Each structural property must be ablated and must cost something when
removed: persistence, event-driven sparsity, per-event composition, and shared archetypes.

## 3. Prior work to acquire before any claim relies on it

This list is from memory. Each entry must be checked against the primary text before it is cited.

| Idea | Prior work | Relevance |
|---|---|---|
| Cells, synapses and a field as message passing | Battaglia et al. 2018 (graph networks); Xu et al. 2019 (aggregation expressiveness) | The update rule is a graph-network step; the choice of aggregator matters |
| One function, many stateful instances | Convolution; Mordvintsev et al. 2020 (neural cellular automata) | Archetype sharing works for parameters |
| Per-input composition of modules | Andreas et al. 2016 (neural module networks); Shazeer et al. 2017 and Fedus et al. 2021 (mixtures of experts) | Conditional computation works at scale; known failures are router collapse, imbalance and overhead |
| Cheap-to-expensive model cascades and routers | Chen, Zaharia and Zou 2023 (FrugalGPT); Ong et al. 2024 (RouteLLM); Madras et al. 2018 (learning to defer) | The routing baseline the substrate must beat |
| Agent memory around LLMs | Packer et al. 2023 (MemGPT); Park et al. 2023 (generative agents: recency, importance, relevance) | The memory baseline; "importance" is a salience score |
| Fast persistent learning beside slow broad learning | McClelland, McNaughton and O'Reilly 1995 (complementary learning systems) | The closest established analogue of "substrate plus LLM" |
| Local learning gated by a global signal | Frémaux and Gerstner 2016 (three-factor rules) | How the field could gate learning without gradients |
| Forgetting under continual learning | Kirkpatrick et al. 2017 | The adaptation experiment must measure it |
| Event-driven sparse execution | Neuromorphic hardware such as Loihi (Davies et al. 2018) | Change-triggered firing as the substrate's natural floor |

## 4. What the substrate must not be asked to do

- **Not replace the reasoner.** Language, broad knowledge and explicit planning stay in the
  reasoner. The substrate is judged on whether it makes the reasoner used less, used better, and
  given better input.
- **Not be scaled before it earns its cost.** No hundreds of millions of cells until the
  four-function experiments are positive at small scale.
  - Storage is dominated by connections, not functions: at 8 bytes per synapse and 10 synapses
    per cell, 500M cells need about 40 GB for connections alone.
  - Connection sharing, for example connectivity generated from rules, must be measured before
    any scale claim. This is a coordinator estimate, not a measurement.

## 5. Revised small world

Keep the evaluator, the hidden-state discipline and ground truth. Change three things.

1. **A resource ladder.**
   - The existing components remain the cheap rung.
   - Add a **simulated reasoner** with a declared cost several orders of magnitude above a
     component.
   - Its accuracy depends on the episode's difficulty and on the quality of the context it
     receives.
   - Simulation comes first because it is cheap, deterministic and has knowable ground truth.
   - Its accuracy model encodes an assumption ("better context gives better answers"). That model
     is a set of swept parameters, never a single setting, and the conclusion must hold across
     the sweep.
   - A small real-model validation follows (section 8).
2. **Persistent streams instead of isolated episodes.**
   - A long-running world with many incidents, configuration drift, recurring faults and changes
     of regime, so that persistence and memory can matter.
   - The unit of replication becomes a stream segment, with dependence handled explicitly
     (charter section 8 still applies).
3. **Relevance that costs computation to judge.**
   - Noise is no longer separable for free by catalogue id.
   - Some decisive evidence is spread across components with partial views.
   - Some incidents need the expensive rung; most do not.
   - The headroom check (old B4) must pass before any experiment is frozen.

## 6. Revised baseline registry

| Baseline | What it challenges |
|---|---|
| Never escalate (cheap components and shared rule only) | Whether the expensive reasoner is needed at all |
| Always escalate (reasoner on every incident) | The quality ceiling at full cost; not assumed optimal |
| Periodic escalation | Whether timing needs to be state-dependent |
| Change-triggered execution (recompute only on input change) | Whether salience adds anything beyond skipping unchanged inputs |
| Threshold or anomaly-score escalation | Whether a single scalar statistic suffices |
| Learned classifier router (cascade) | Whether the substrate beats the established routing literature |
| Retrieval memory plus summary | Whether substrate memory beats conventional agent memory |
| Matched-capacity monolithic recurrent model | Whether the cell structure matters, or only total capacity |
| Oracle escalation (privileged) | Headroom of the escalation problem |

## 7. Revised experiment program

New ids, so that no frozen or reported id changes meaning.

| Id | Question | Primary measure |
|---|---|---|
| EXP-101 | Escalation control: does substrate salience beat threshold, change-triggered, periodic and learned-router escalation at matched total cost? | Verified decisions and critical misses per unit cost; escalation precision and recall |
| EXP-102 | Context construction: does substrate-built structured context let the reasoner decide as well as full history, at fewer tokens? | Reasoner accuracy per token against full history, summary and retrieval |
| EXP-103 | Memory: does persistent substrate state beat retrieval memory on recurring incidents, stale records and drift? | Time and cost to correct decision on recurrences; stale-memory errors |
| EXP-104 | Adaptation: after a change of regime, how fast does each system recover, and what does it forget? | Recovery time; forgetting on the old regime |
| EXP-105 | Structure: do the cell properties matter? Ablate persistence, sparsity, composition and archetype sharing against the monolithic baseline | Quality-to-cost frontier per ablation |
| EXP-106 | Real-model transfer: do EXP-101 and EXP-102 conclusions hold with a real small model in place of the simulated reasoner? | Agreement of category (beneficial, harmful, equivalent, unresolved) |

The old EXP-002 to EXP-007 are re-scoped under these. Salience features belong to EXP-101, memory
to EXP-103, world models and recurrent depth after EXP-105. Low precision is deferred
indefinitely.

## 8. Measurement additions

- **Total cost** = modelled substrate cost + modelled rule cost + reasoner cost. Reasoner cost is
  reported in its own units (calls, tokens, declared latency) and also through a preregistered
  exchange rate. Charter section 4 already requires the rate.
- **Counterfactual labels** (charter section 9). In simulation, run the reasoner offline on every
  incident to learn what a skipped escalation would have produced. Charge this to evaluation,
  never to an arm.
- **Real-model runs** stay within the operating budget.
  - A local small model on this CPU is slow but free.
  - A remote model costs money and needs credentials this environment does not have.
  - EXP-106 is sized to the budget before freezing.
  - Every call is recorded at the boundary and never repeated to replay state (AGENTS.md).
- **No model judges an outcome.** Verified decisions stay checked against hidden simulator state.

## 9. What Stage A and B keep

- **Kept as built:** the core, ledger, bill, evaluator, harness, interleaving, counted operations,
  analysis package and review discipline.
- **Shared decision rule:** it becomes the cheap rung's decision procedure. A6d still applies.
- **Stage B:** its findings stand as exploration results about the current world. They are the
  reason for this revision.

## 10. Risks and falsification

- **A strong simple baseline captures nearly all the value.** Stage B showed this once already.
  A threshold-plus-cache escalation policy may be as good as anything.
  - Prediction: it will be hard to beat.
  - If it ties, the substrate's salience is not retained.
- **The simulated reasoner bakes in the conclusion.** Mitigations: the parameter sweep, plus
  EXP-106.
- **Local learning cannot train the routing.** Learning is tested only after EXP-101 shows that
  hand-designed substrate gating earns its cost.
- **The persistent world makes statistics harder.** Pre-register the stream-segment unit and the
  dependence handling before any freeze.

## 11. Build order

1. Approve or amend this proposal; edit the charter accordingly.
2. World revision: resource ladder, simulated reasoner, persistent streams, costly relevance.
   Headroom check.
3. Baselines from section 6 that need no learning.
4. Substrate prototype with hand-designed gating (cells, typed gated routes, field).
5. EXP-101 preregistration and freeze. Then EXP-102 and EXP-103.
6. Learning, EXP-104 and EXP-105.
7. EXP-106 with a real model, sized to budget.
