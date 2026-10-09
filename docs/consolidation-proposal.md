# Proposal: the learning line is the reasoner's law, learned through verification and consolidated across streams

Status: **proposed 2026-10-09**, awaiting the user's approval of the charter changes in section 6.
The two instrument units it needs (W4, and the resumption of B5) commit to no architecture and
were launched on the same day; the mechanism units (P1, S1) are queued with their criteria to be
fixed from W4's numbers as committed specifications.

## 1. What prompted it

The user's document of 2026-10-09 ("From the Human Brain to Artificial Superintelligence")
reformulates two hypotheses (the brain stores learned responses rather than experiences; internal
activity across timescales drives recollection) into one: intelligence as a continually evolving
system that rapidly builds temporary representations, uses recurrent dynamics to interpret and
predict, and periodically reorganises those representations into general, efficient, reusable
knowledge, with consolidation validated on held-out experience and committed only when it
generalises better. It proposes five kinds of state, three clocks, a three-factor fast learning
rule, a consolidation objective, and three separable experiments under matched budgets.

## 2. Assessment against Gordian's evidence

| Claim in the document | Gordian's evidence | Verdict |
|---|---|---|
| Store learned responses, not experiences | The engram (responses keyed on patterns) and the record rung (experiences) found the same shape on this world: clean and small inside a stream, colliding across streams at the world's floor (A1a–A1d, E1, W3) | The storing question is secondary. What decides is whether the world has invariant structure to learn; on this world the fault patterns are stream-local by design |
| Rhythms coordinate, but oscillation itself is not the computation | Phase gates and oscillators found no role; event order inside a tick did (M1b, M2); sub-tick support pruning fixed the tick floor (M3) | Agrees. The oscillome's value is ordering and bounded support, not rhythm |
| Three clocks that need not advance together | Stream time, the tick (internal), the plasticity rhythm (L1's 10 s, the engram's 100 s decay) all exist; internal iteration has never been made adaptive | Agrees, with one correction: on this world internal iteration is nearly free (the medium's bill is under 1% of an arm's) and the reasoner is 99.9% of cost, so "how long to think" is "when to stop thinking cheaply and ask" |
| Activity is not learning; a learning signal gates plasticity | The engram binds only on an answer (a teacher signal); without verification half the stored answers are wrong (W2: 40–54%) | Agrees, and it names the missing piece: the signal must be a verified consequence, not the teacher's word |
| Consolidation maximises transferable predictive structure, keeps exceptions, is validated on held-out experience | Not built. L1 is the one positive case of a transferable law learned and carried (the noticing constants, which are world constants) | The new mechanism, and the one Gordian's method applies to itself |
| Small partially observable world with hidden rules, delayed consequences, rare exceptions, conflicting new rules | The stream world has all of these (hidden families, deadlines, decoys, regime changes at 200 s and 400 s) | The experiments the document proposes can run here without a new world |

Citations in the document (a 2024 recurrent planning agent, a 2023 consolidation study, a 2025
theta-locked stimulation study, a 2026 generative-hippocampus perspective, Titans, Nested
Learning, continual backpropagation) are entered in the charter's source register as "to acquire
and check"; nothing here relies on them.

## 3. The decisive fact: what is invariant across streams on this world

W2, W3 and the hidden record say the fault patterns are regenerated per stream. What does not
change between streams at a fixed setting:

1. **The reasoner's law.** An answer is informed with probability `h(q, d)`, where `q` is the
   share of the incident's decisive evidence in the context; an informed answer is the truth; an
   uninformed one is a guess from the public rules. So the accuracy of a call depends on **when**
   it is asked (decisive evidence arrives over the incident's phases) and on **what** is included
   (decisive references, with distractors penalised under δ and charged at 5 ms each).
2. **The regime instants** (200 s and 400 s) and what a change does to the public rules.
3. **The deadline windows**, the burst spacing and ramp rates (L1 learned the last two), the
   decoy's resolution signature.
4. **What probes answer** after a given diagnosis: the verification channel. A diagnosis implies
   probe outcomes; the stream never reports correctness, but a probe does, at a bounded price
   (150 units and 1.5 s per stream).

None of these is encodable by an arm (items 1, 2 and 4 are hidden rules); all are learnable from
an arm's own history, because their footprints are public bytes.

## 4. The line

**Claim to test.** An arm that learns, from its own verified history, when to ask the reasoner,
what to include, and when to trust an answer, converts experience into verified decisions per
unit cost faster than the tuned fixed-delay call-budgeted selector (B5), under hard limits, and
keeps that advantage across streams and across the regime changes.

**Mechanism (planned).**

- *Fast store*: an episode per call, keyed on the public state of evidence at the ask instant
  (which evidence classes have arrived, the checker's verdict, the anomaly's age against the
  public deadline window, the context's size) with the answer and its **verified** outcome
  (probe pattern after the answer; incident end; a later contradiction).
- *Slow system*: at the slowest rhythm or the stream boundary, a consolidation step fits a rule
  (ask when evidence class K has arrived; include X; trust answer kind Y) on the older half of
  the store and validates it on the newer half; the rule is committed only if validation improves
  verified decisions per call. Exceptions (episodes the rule gets wrong) are kept, not merged.
- *Controls*: the fast store with no consolidation; consolidation without validation (commit
  always); random consolidation; the learned arm with learning off; the memoryless B5 selector at
  its tuned delay; the selection oracle as the ceiling.

**Two implementations, same measures:** a conventional learner (a logistic rule under 10,000
parameters, trained online from verified episodes, carried across streams: P1, Lab 2) and the
medium's version (engrams keyed on ask-state, a consolidation cell at the slow rhythm, the persist
port carrying the rule: S1, Lab 1). The charter's section 7 requires the conventional row; the
aim is shown by beating it, not by replacing it.

**Measures.** Verified decisions per unit cost (proxy 1); the slope of that quantity against
incidents seen (proxy 2), per arm, with and without carry across streams; recovery after each
regime instant (EXP-104's measure); critical misses bounded; the verification channel's own cost
in the bill. All as committed specifications; `H0: Δ ≤ 0` against B5's tuned selector at matched
cost.

**Feasibility first (W4, Lab 3).** The hidden side gives the lever sizes before any learner is
built: the accuracy a fixed delay can reach against the per-incident optimal ask instant; the
gain from a minimal decisive context against the rung's context under δ; the power of probes to
separate right from wrong answers, by family, at what cost; the accuracy loss after each regime
instant and what re-learning could recover. If the fixed delay is within a few points of the
optimal instant, the "when" lever is small and the line narrows to "what" and "trust".

## 5. Mapping the document's architecture onto the charter

| Document | Charter and code |
|---|---|
| Active state | Working state (3.1); the rung's views; the medium's field |
| Fast plastic memory | Private component state; the engram with a verified signal |
| Episodic memory | The event ledger's provenance, and the fast store's kept exceptions |
| Structural model | The consolidated rule; L1's learned constants are the first instance |
| Metacognitive controller | Section 4's value of computation; the selector; EXP-101 |
| Three clocks | Stream time; the tick and sub-tick; the oscillome's rhythms and the plasticity boundary |
| Experiment 1 (adaptive internal iteration) | On this world: when to stop checking cheaply and ask; EXP-101 as a learning experiment |
| Experiment 2 (dual-timescale learning, replay controls) | EXP-103's arms plus the consolidation arm and its controls above |
| Experiment 3 (oscillatory scheduling) | Already answered in the small by M1b/M2; re-asked only if consolidation needs a schedule |
| Model refinement versus model creation | Out of scope until the above has a result |

## 6. Charter changes proposed (applied only on approval)

1. Section 1.2, "Mechanism (planned, Lab 1)": replace the engram sentence with the mechanism of
   section 4 above, and add a line recording that the engram and the pair-cell learner were
   built and measured negative on this world (review log, A1a–A2, W3).
2. Section 3.1 gains a fourth kind of state, **consolidated structure**: rules an arm has
   validated on its own held-out history; distinct from private component state because its
   commit is gated by a validation the arm runs on itself.
3. Section 6: EXP-101 and EXP-103 are registered together as one learning experiment on the
   reasoner's law with the arms above; EXP-104's measure (recovery after a regime instant) is a
   secondary measure of the same experiment. A new id is used, because the original definitions
   keep their text.
4. Section 11: a rule that every learned arm reports its verification channel's cost in the bill
   and that a learner's "verified" label is only ever a public consequence (a probe answer, an
   incident's end), never the evaluator's truth.

## 7. What would change this proposal

W4 showing that the fixed-delay selector is within the margin of the per-incident optimum and
that probes cannot separate right from wrong answers at an affordable cost: then the reasoner's
law is not learnable at a price on this world either, and the honest next step is a world with
a consequence channel, labelled as built for learnability.
