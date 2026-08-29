# Gordian Agent Contract

This file is the canonical repository instruction set for coding agents.

## Mission

Build Gordian as a research-driven coordination substrate for human and autonomous software engineering. The system must keep intent, source state, execution, evidence, provenance, and authority distinct, then reconcile them through explicit contracts rather than mutable project-status folklore.

## Language architecture

### Rust is the substrate

Production semantics belong in Rust.

Rust owns:

- Mission Graph domain types and invariants;
- graph algorithms and indexing;
- scheduling and concurrency control;
- event/state projection;
- evidence and provenance semantics;
- Jujutsu integration;
- persistence adapters;
- authorization/capability enforcement;
- protocol parsing and validation;
- CLI/server/runtime hot paths.

Prefer safe Rust. `unsafe` requires a documented invariant, a demonstrated performance or interoperability need, focused tests, Miri/sanitizer coverage where applicable, and benchmark evidence showing why the safe design is insufficient.

### Python is a thin orchestration layer

Python may coordinate experiments, launch tools/processes, prepare datasets, aggregate benchmark results, and provide lightweight research automation.

Python MUST NOT become a second implementation of Mission Graph semantics, scheduler safety, evidence freshness, authorization, or accepted-frontier transitions.

If Python needs a domain decision, call the Rust implementation through a stable CLI/IPC/binding instead of duplicating logic.

### Lean is a development/formal dependency

All Lean source, toolchains, Lake configuration, formal fixtures, and proof-only dependencies live under `formal/`.

Lean MUST NOT be required to run the production Gordian binary.

Formal development follows verification-guided development:

1. write a small executable formal model for safety-critical semantics;
2. prove meaningful model properties;
3. implement optimized production semantics in Rust;
4. differentially test Rust against the executable formal model over generated inputs;
5. use property, mutation, fuzz, concurrency, and integration tests for behavior outside the formal model.

A theorem about the model is never described as proof of an empirical performance claim or proof that the Rust implementation refines the model unless that refinement has actually been established.

## Architecture rules

1. **Mission is goal, PlanRevision is strategy.** Never fuse the identity of the objective with one implementation plan.
2. **Decomposition is not dependency.** Containment and causal prerequisites are separate graph relations.
3. **Atoms are the global scheduling contract.** Quarks remain local execution structure unless research falsifies this boundary.
4. **Attempts are not specifications.** A failed attempt does not mutate the Atom contract.
5. **Status is a projection.** Derive readiness, blocking, activity, verification, and satisfaction from canonical facts whenever feasible.
6. **Evidence is exact-subject evidence.** Rewrites, relevant dependency changes, environment changes, or verifier changes invalidate incompatible evidence.
7. **Integration is a first-class candidate.** Passing sibling candidates do not imply their composition passes.
8. **Workers cannot redefine accepted reality.** Promotion and deployment are distinct capabilities.
9. **Deterministic core, explicit effects.** LLM calls, clocks, network reads, external writes, and other nondeterministic activities are recorded at the boundary and never repeated merely to replay state.
10. **Graph topology encodes causality, not completion order.** Do not serialize independent changes merely because one happened first.

## Jujutsu workflow

Jujutsu is the preferred local change substrate. GitHub remains an external collaboration/transport system.

Before development, inspect:

```bash
jj --version
jj root
jj status
jj log -r '::@ | @::'
```

The initial Codex environment reported `jj 0.23.0`. That is substantially behind the current research target and predates capabilities Gordian expects, including `jj run` introduced in 0.43. Do not design around 0.23 behavior. The foundation initiative must establish and pin the supported Jujutsu baseline before implementation work depends on newer commands.

Repository policy:

```text
main / trunk()      accepted source frontier
change ID           logical evolving implementation
commit ID           exact candidate identity
workspace           isolated worker execution state
bookmark            external transport identity
tag                 immutable release identity
```

There is no permanent `develop` bookmark.

One active writer per logical change is the normal path. Speculative alternatives receive separate changes from the same exact base.

Never move `main`, create releases, or push canonical state from a worker flow unless the current Atom explicitly grants coordinator authority.

## Black-box module rule

A module should expose contracts rather than internal structure.

Prefer:

```text
input contract -> deterministic/controlled transformation -> output contract
```

Internal representation may change without forcing callers to know about it.

Cross-crate dependencies must follow declared architecture direction. Avoid convenience imports that create hidden coupling.

## Performance rule

Correct-but-greedy is a baseline, not the finish line.

For algorithms on the Mission Graph, scheduler, evidence index, replay engine, and knowledge graph:

1. implement the simplest correct reference algorithm when useful;
2. state its asymptotic complexity;
3. construct representative and adversarial benchmark datasets;
4. benchmark time, allocation/memory, and scaling shape;
5. compare optimized implementations against the reference for semantic equivalence;
6. preserve the reference implementation as a test oracle when cheap enough.

Never optimize from intuition alone. Never retain an elegant optimization whose benefit disappears under benchmark or whose complexity cost exceeds the measured gain.

Important scheduling lower bounds and baselines include total work, critical-path length, topological/list scheduling, and resource constraints. Heterogeneous workers should be evaluated with HEFT-style/list-scheduling baselines rather than a single FIFO greedy queue.

## Verification ladder

Use the strongest appropriate method for each claim.

### Required everyday checks

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p gordian-kg -- validate
(cd formal && lake build)
```

### As applicable

- `proptest` / generated state-machine tests;
- differential randomized testing against Lean models;
- `cargo-mutants` mutation testing;
- `cargo-fuzz`/libFuzzer for parsers and protocol surfaces;
- Miri and sanitizers for memory/unsafe boundaries;
- Kani for bounded bit-precise safety/correctness harnesses;
- Loom for small concurrency state spaces;
- Shuttle for larger randomized concurrency schedules;
- Turmoil for deterministic network/failure simulation;
- benchmark suites with Criterion/Divan and deterministic instruction-level profiling where useful;
- fault injection for crashes, stale leases, duplicate events, partitions, and candidate rewrites.

Do not add every verifier indiscriminately. Each tool must protect a named risk or invariant.

## Research discipline

The knowledge graph is part of the engineering substrate.

Every material concept introduced by research or experiment should be represented in `knowledge/graph.jsonld` or its successor representation, with explicit relations to sources, claims, algorithms, theorems, experiments, and implementation artifacts.

Distinguish:

```text
formal theorem
established scientific/engineering foundation
evidence-supported conclusion
engineering deduction
hypothesis
assumption
unresolved uncertainty
```

Actively record disconfirming and qualifying evidence. A graph that only accumulates support edges is a citation shrine, not research infrastructure.

## Documentation style

Do not use maturity labels such as `v0`, `v1`, `v2`, `M0`, `M1`, or similar pseudo-roadmap version names for Gordian concepts, specifications, or phases.

Use semantic names:

```text
Mission Graph Specification
Foundation Initiative
Reference Kernel Initiative
Distributed Coordination Initiative
```

Technical protocol/schema versions are allowed only when compatibility genuinely requires an explicit version identifier.

Avoid status prose that will become stale. Prefer generated facts, experiment records, and links to canonical issues.

## Atom completion contract

An implementation issue is an Atom. Before declaring it complete, the agent must provide:

- the exact objective satisfied;
- implementation summary;
- tests and verifier evidence;
- benchmark evidence when the Atom touches a performance-sensitive algorithm;
- knowledge-graph updates for material new concepts/evidence;
- documentation updates for changed semantics;
- exact Jujutsu candidate identity used for verification when the execution substrate supports it;
- unresolved risks or explicitly deferred work.

Passing tests are necessary but not automatically sufficient when the Atom has stronger acceptance predicates.

## No cargo-cult dependencies

Before adding a crate, service, database, workflow engine, or formal tool, state:

- which requirement it serves;
- what simpler alternative exists;
- performance/operational costs;
- failure and maintenance surface;
- whether it becomes production or development-only dependency.

Gordian exists partly to avoid reproducing accidental industry complexity. The same standard applies to Gordian itself.
