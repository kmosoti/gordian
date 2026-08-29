# Gordian Verification Strategy

Gordian cannot make one technique carry more epistemic weight than it deserves. Lean proves propositions about a model. Rust types exclude classes of representation error. Property tests explore generated examples. Model checkers exhaust bounded state spaces. Concurrency tools manipulate schedules. Fault injection attacks recovery assumptions. Benchmarks measure selected workloads. Experiments test architectural hypotheses.

Robustness comes from overlapping methods with explicit boundaries, not from accumulating impressive tool names.

## 1. Verification objective

The target is:

> Every safety-critical Gordian claim has the strongest applicable verification method, an explicit assumption boundary, an implementation-conformance path, and an adversarial falsification path.

For formalizable substrate invariants, the stronger operational target is:

- no `sorry` or `admit`;
- no home-grown axioms hidden behind imports;
- every theorem compiled by the pinned Lean toolchain;
- proof terms checked by the Lean kernel;
- proof environment independently checked where supported;
- axioms audited against a narrow allowlist;
- theorem nodes linked to exact declarations and source digests;
- corresponding Rust behavior checked against the model.

This is not equivalent to proving Gordian correct in the real world.

## 2. Claim classes and methods

| Claim class | Primary method | Additional methods | What remains unresolved |
| --- | --- | --- | --- |
| Logical invariant of a finite model | Lean proof | independent checker, axiom audit | adequacy of the model |
| Rust function refines executable model | differential randomized testing | property tests, bounded verification | completeness of tested domain unless refinement is formally proved |
| Unsafe state unrepresentable | Rust type system | compile-fail tests, API review | FFI/external-system behavior |
| Bounded memory safety/correctness | Kani | unit/property tests | behavior outside configured bounds and unsupported features |
| Concurrent state-machine safety | Loom | Shuttle, stress, TLA/Stateright-style model | fidelity of instrumentation and explored state space |
| Probabilistic schedule robustness | Shuttle | deterministic seed replay, stress | absence of untested schedules |
| Network/recovery behavior | Turmoil-style deterministic simulation | fault injection, crash tests | real kernel/storage/network divergence |
| Durable projection/replay | model/property tests | crash injection, database fault tests | undeclared external effects |
| Scheduler optimality | exact solver on small instances | heuristic comparison, complexity analysis | large-instance optimality for NP-hard variants |
| Scheduler performance | Criterion/Divan and workload corpus | instruction/allocation profiling | generalization beyond workloads/hardware |
| Agent coordination effectiveness | controlled experiments and ablations | repeated seeds/models/repos | external validity across models/tasks |
| Semantic conflict prediction | labeled empirical corpus | precision/recall/calibration, ablation | unobserved semantic dependencies |
| Mission ontology usefulness | end-to-end comparative experiment | qualitative failure taxonomy | universal optimality |

## 3. Formal kernel

Lean belongs entirely under [`formal/`](../../formal/). It is a development dependency and specification oracle, not a production runtime dependency.

### Theorem admission requirements

A theorem is counted as checked only when:

```text
source compiles
and no sorry/admit is present
and axiom audit passes
and independent checker passes where supported
and theorem declaration is linked from the knowledge graph
and assumptions are enumerated
```

A theorem title is not evidence. A `.lean` file that merely restates assumptions as fields and proves projections may still be useful, but its engineering weight must match the proposition actually proved.

### Formal targets

Initial safety properties include:

1. a valid topological/rank certificate excludes hard-dependency cycles;
2. dispatch witnesses imply prerequisite satisfaction;
3. dispatch witnesses imply capability, resource, authorization, and lease conditions;
4. evidence with an incompatible subject fingerprint cannot establish current satisfaction;
5. changing an exact candidate invalidates candidate-bound evidence;
6. worker capability alone cannot authorize accepted-frontier promotion;
7. accepted-state witnesses exclude unresolved structural conflicts;
8. accepted-state witnesses require exact, fresh, passing evidence;
9. cross-Atom hard dependencies cannot target Quarks;
10. deterministic projection returns identical state for identical ordered event history;
11. stale fencing tokens cannot mutate a lease-protected resource;
12. compare-and-swap promotion rejects an obsolete frontier expectation;
13. duplicate idempotent events do not change projected state;
14. declared non-interference is symmetric for paired read/write sets.

### Proof boundaries

Lean does not establish these without additional bridges:

- semantic resource declarations are complete;
- source code has no hidden dependency;
- an LLM-generated plan is useful or correct;
- hashes never collide;
- clocks, filesystems, databases, or networks satisfy the model assumptions;
- an external verifier observed the intended environment;
- a benchmark generalizes;
- the Mission Graph ontology is optimal;
- the Rust implementation refines the Lean model.

Each requires implementation evidence, environmental qualification, or experiment.

## 4. Model to Rust conformance

The formal model should remain small and executable. Gordian follows a verification-guided pattern:

```text
formal transition/model function
        |
        v
canonical test-vector generator
        |
        +----> Lean expected result
        |
        +----> Rust implementation result
        |
        v
exact comparison + minimized counterexample
```

### Differential randomized testing

For every model-backed Rust component:

1. define canonical serialized inputs;
2. generate valid and adversarial cases from deterministic seeds;
3. evaluate the Lean model or exported oracle;
4. evaluate Rust against the same input;
5. compare canonical results;
6. minimize disagreements;
7. store seed, toolchains, source commits, environment digest, and outputs;
8. attach the result to the theorem/refinement obligation in the knowledge graph.

Passing differential tests demonstrates conformance on tested cases. It is not a full refinement proof.

### Translation-risk controls

- Keep model types smaller than production types.
- Avoid duplicating opaque business logic independently in Lean and Rust.
- Generate schemas/test vectors from one canonical description where practical.
- Compare edge cases at numeric, ordering, identity, and serialization boundaries.
- Mutation-test the Rust implementation and verify the differential suite kills relevant mutants.
- Add intentionally incorrect model/implementation fixtures to prove the harness detects disagreement.

## 5. Rust verification ladder

Rust is the primary production language. The verification ladder should be risk-driven.

### Compiler and type design

Use newtypes, sealed state transitions, capability-bearing handles, immutable identities, and result types so invalid combinations are difficult to express. Forbid `unsafe` by default at workspace level. Any future `unsafe` boundary requires a narrow module, documented invariant, Miri/fuzz/property coverage, and benchmark justification.

### Unit and example tests

Use for local semantics and regression reproduction. Examples should not carry claims of broad coverage.

### Property tests

Use generated structures to validate algebraic and state-machine properties:

- canonicalization idempotence;
- fingerprint sensitivity to relevant inputs;
- topological output validity;
- projection determinism;
- duplicate-event behavior;
- serialization round trips;
- optimized/reference algorithm equivalence;
- scheduler safety under generated dependency/resource graphs.

### Model-based state-machine tests

Generate command histories against a simple executable reference model and the Rust state machine. Compare visible state and rejection behavior after each command.

### Mutation testing

Use mutation testing to answer whether tests detect incorrect behavior rather than merely execute code. Track surviving mutants by invariant and prioritize mutations around acceptance, authority, evidence freshness, lease fencing, and replay.

### Fuzzing

Fuzz parsers, protocol decoders, event ingestion, graph import, JSON-LD canonicalization, Jujutsu output parsing, and state-machine command sequences. Preserve minimized crash or disagreement corpora.

### Miri and sanitizers

Use Miri for unsafe-sensitive behavior and undefined-behavior detection when applicable. Use sanitizers for integration boundaries and dependencies where supported.

### Kani

Use bounded model checking for small, safety-critical Rust functions with finite domains and explicit harnesses. Appropriate targets include authorization decisions, acceptance predicates, fingerprint invalidation, sequence/fencing comparisons, and small graph invariants. Record the configured bound. A proof at bound `N` is not a proof beyond `N`.

## 6. Concurrency verification

Concurrency correctness is not established by a long stress test that happened not to fail.

### Loom

Use Loom for compact concurrent primitives where production synchronization can be modeled with Loom equivalents. Explore schedule permutations for:

- lease acquisition/release;
- compare-and-swap frontier promotion;
- event append plus projection visibility;
- worker handoff/candidate freeze;
- cancellation and completion races;
- claim registry updates.

The model must remain small enough for state exploration. Instrumentation fidelity is an explicit assumption.

### Shuttle

Use randomized deterministic scheduling to scale schedule exploration beyond exhaustive Loom models. Persist failing seeds and replay them in CI. Shuttle adds coverage; it does not establish soundness.

### Stress and production-like tests

Run high-concurrency tests on production primitives, but interpret them as empirical evidence. Capture event histories, seeds, timing, and resource pressure so failures are diagnosable.

## 7. Distributed and failure verification

Use deterministic simulation for coordinator/worker, lease, message, and recovery behavior before relying on wall-clock distributed tests.

### Simulated faults

- message delay, duplication, reordering, and loss;
- worker crash before/after candidate handoff;
- coordinator crash during promotion;
- stale lease holder resuming;
- network partition;
- repository/VCS command failure;
- database transaction abort;
- truncated or corrupted event/artifact writes;
- verifier timeout or partial evidence upload.

### Recovery invariants

- accepted frontier never advances without the acceptance predicate;
- stale workers cannot mutate lease-protected state;
- replay produces the same projection from the same canonical history;
- a candidate remains bound to its exact commit and evidence;
- duplicate messages/events are idempotent where declared;
- irreversible effects are never silently retried;
- unresolved conflicts cannot cross acceptance.

### Real integration tests

Deterministic simulation cannot validate every filesystem, Git/Jujutsu, database, kernel, or network behavior. Complement it with disposable repositories, process-kill tests, database fault injection, and platform matrices.

## 8. Scheduler verification

Scheduling combines provable safety constraints with empirically chosen heuristics.

### Safety

Prove/test that dispatched Atoms satisfy:

- hard prerequisites;
- specification validity;
- capability requirements;
- authorization policy;
- resource feasibility;
- lease/fencing rules;
- conflict-admission policy;
- effect/retry policy.

### Reference solvers

For small instances, retain exact or brute-force solvers as correctness/quality oracles. Use them to measure heuristic regret rather than assuming a heuristic is good because it is fast.

### Heuristic comparisons

Compare at least:

- FIFO ready queue;
- critical-path priority;
- longest-processing-time variants;
- resource-aware list scheduling;
- HEFT-style heterogeneous scheduling where task/worker estimates exist;
- contention-aware scheduling using semantic conflict predictions;
- bounded lookahead/beam variants where justified.

### Metrics

- makespan;
- weighted Mission progress;
- critical-path delay;
- worker utilization;
- verification/integration failure cost;
- conflict rate;
- rework and abandoned attempts;
- queue/admission latency;
- scheduler CPU/memory/allocations;
- regret against exact optimum on small instances;
- robustness under inaccurate duration/conflict estimates.

A fast scheduler that launches expensive conflicting work may optimize the wrong proxy.

## 9. Performance verification

Every performance-sensitive implementation begins with a simple correct baseline.

### Workload dimensions

- node/edge count;
- graph density and width;
- critical-path length;
- resource heterogeneity;
- semantic claim overlap;
- event-history length;
- evidence fan-in/fan-out;
- knowledge-graph source/claim density;
- Jujutsu repository size and change topology;
- artifact sizes and verifier output volume.

### Required comparisons

- baseline vs optimized result equivalence;
- representative vs adversarial shapes;
- warm vs cold caches;
- single-thread vs parallel where applicable;
- allocation and peak memory;
- throughput and tail latency;
- deterministic instruction/cycle proxies when wall-clock noise is material;
- repeated samples and uncertainty summaries.

### Regression policy

Do not gate on a single noisy timing. Use controlled runners or statistical thresholds, preserve baseline distributions, and require investigation rather than automatic architectural reversal for marginal changes.

## 10. Experimental verification of Gordian hypotheses

The following claims are not theorem-shaped and must remain experimentally vulnerable:

- the Mission/Initiative/Atom/Quark ontology improves planning and execution;
- Atom is the right global scheduling boundary;
- semantic access claims predict harmful conflicts better than file paths;
- isolated Jujutsu workspaces plus semantic coordination outperform branch/worktree baselines;
- snapshot isolation is better than continuous rebasing for agents;
- evidence invalidation reduces false completion without unacceptable cost;
- Jujutsu reduces orchestration complexity versus Git;
- derived state reduces status drift;
- formalization catches defects worth its maintenance cost.

Each experiment needs a baseline, repeated runs, exact environment, failure taxonomy, and a predeclared result that would cause the design to change.

## 11. CI tiers

### Per change

```text
format
clippy with warnings denied
unit/property tests in bounded profile
knowledge graph validation
Lean build
independent proof check
axiom audit
```

### Scheduled or pre-acceptance

```text
mutation testing
fuzz smoke/corpus replay
Kani harnesses
Loom/Shuttle models
benchmark smoke and regression comparison
Jujutsu disposable-repository contracts
```

### Research qualification

```text
full experiment matrix
fault-injection campaigns
large benchmark corpus
cross-model/repository agent trials
statistical analysis and artifact publication
```

Not every heavy method belongs on every edit. The acceptance policy records which layer protects which risk.

## 12. Evidence binding

Every verification result should bind to an immutable subject fingerprint containing the relevant subset of:

```text
specification_revision
exact_source_commit
resolved_dependencies
input/artifact digests
environment/toolchain digest
verifier identity and version
verification policy revision
```

A result is reusable only when compatibility rules establish that changed components are irrelevant. Convenience is not evidence freshness.

## 13. Definition of verified

A claim may be described as:

- **formally proved**, only for the exact proposition checked under named assumptions;
- **implementation-conformant**, only for the demonstrated model-to-code relation and tested domain;
- **experimentally supported**, only within the experiment scope;
- **benchmarked**, only for the published workloads/environment;
- **production-observed**, only for the observed deployment/time window;
- **unresolved**, when evidence is missing, conflicting, or stale.

“Verified Gordian” without a predicate, subject, scope, and evidence identity is prohibited wording.
