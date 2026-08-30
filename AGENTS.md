# Gordian Agent Contract

This is the repository's sole coding-agent instruction file.

## Mission

Build Gordian as a research-driven coordination substrate for human and autonomous software engineering. The system must keep intent, source state, execution, evidence, provenance, and authority distinct, then reconcile them through explicit contracts rather than mutable project-status folklore.

## Required reading

Before substantial implementation, read the documents relevant to the Atom, beginning with:

1. [`README.md`](README.md)
2. [`docs/architecture.md`](docs/architecture.md)
3. [`docs/spec/mission-graph.md`](docs/spec/mission-graph.md)
4. [`docs/spec/invariants.md`](docs/spec/invariants.md)
5. [`docs/implementation/execution-order.md`](docs/implementation/execution-order.md)
6. [`knowledge/ontology.md`](knowledge/ontology.md)
7. [`knowledge/acquisition.md`](knowledge/acquisition.md)
8. [`docs/research/verification-strategy.md`](docs/research/verification-strategy.md)
9. [`docs/formal/proof-boundary.md`](docs/formal/proof-boundary.md)
10. [`docs/protocols/jujutsu-agent-protocol.md`](docs/protocols/jujutsu-agent-protocol.md)

Do not rely on README summaries when a normative specification or Atom contract is more precise.

## Language architecture

### Rust is the substrate

Production semantics belong in Rust.

Rust owns:

- Mission Graph domain types and invariants;
- canonical research knowledge schema, validation, indexes, and queries;
- graph algorithms and indexing;
- scheduling and concurrency control;
- event and state projection;
- evidence and provenance semantics;
- Jujutsu integration;
- persistence adapters;
- authorization and capability enforcement;
- protocol parsing and validation;
- CLI, server, and runtime hot paths.

Prefer safe Rust. `unsafe` requires a documented invariant, a demonstrated performance or interoperability need, focused tests, Miri or sanitizer coverage where applicable, and benchmark evidence showing why the safe design is insufficient.

Production crates should deny unsafe code until a reviewed Atom explicitly introduces a narrow exception.

### Python is a thin orchestration layer

Python may coordinate experiments, launch tools and processes, stage source acquisition, prepare datasets, aggregate benchmark results, perform statistical analysis, and reconcile temporary external projections.

Python MUST NOT become a second implementation of Mission Graph semantics, scheduler safety, evidence freshness, authorization, lease arbitration, accepted-frontier transitions, or research ontology semantics.

If Python needs a domain decision, call the Rust implementation through a stable CLI, IPC, or binding instead of duplicating logic.

### Lean is a development and formal dependency

All Lean source, toolchains, Lake configuration, formal fixtures, checker configuration, and proof-only dependencies live under `formal/`.

Lean MUST NOT be required to build or run the production Gordian binary.

Formal development follows verification-guided development:

1. write a small executable formal model for safety-critical semantics;
2. prove meaningful model properties under explicit assumptions;
3. implement optimized production semantics in Rust;
4. differentially test Rust against the executable model over generated inputs;
5. use property, mutation, fuzz, bounded verification, concurrency, integration, and fault tests outside the formal model.

A theorem about the model is never described as proof of an empirical performance claim or proof that Rust refines the model unless that refinement has actually been established.

## Architecture rules

1. **Mission is goal, PlanRevision is strategy.** Never fuse the identity of the objective with one implementation plan.
2. **Decomposition is not dependency.** Containment and causal prerequisites are separate graph relations.
3. **Atoms are the current global scheduling contract.** Quarks remain local execution structure unless experiment falsifies this boundary.
4. **Attempts are not specifications.** A failed attempt does not mutate the Atom contract.
5. **Status is a projection.** Derive readiness, blocking, activity, verification, and satisfaction from canonical facts wherever feasible.
6. **Evidence is exact-subject evidence.** Rewrites, relevant dependency changes, environment changes, or verifier changes invalidate incompatible evidence.
7. **Integration is a first-class candidate.** Passing sibling candidates do not imply their composition passes.
8. **Workers cannot redefine accepted reality.** Promotion, release creation, and deployment are distinct capabilities.
9. **Deterministic core, explicit effects.** Model calls, clocks, network reads, external writes, and other nondeterministic activities are recorded at the boundary and never repeated merely to replay state.
10. **Graph topology encodes causality, not completion order.** Do not serialize independent changes merely because one happened first.
11. **Reference semantics precede optimization.** Preserve a simple oracle or another explicit correctness bridge when practical.
12. **Negative evidence may delete architecture.** A failed Gordian hypothesis must simplify, condition, or replace the affected design.

## Jujutsu workflow

Jujutsu is the preferred local change substrate, subject to the Jujutsu-versus-Git experiment. GitHub remains an external collaboration and transport system.

The initial Codex environment reported `jj 0.23.0`. That release predates behavior Gordian intends to qualify, including `jj run`. Do not design around the old binary.

From the repository root, use the safe bootstrap and then execute issue #1's disposable-repository qualification suite:

```bash
bash scripts/bootstrap-jj.sh --install
jj --version
jj root
jj status
jj log -r '::@ | @::'
```

Repository policy:

```text
main / trunk()      accepted source frontier
change ID           logical evolving implementation
commit ID           exact candidate identity
workspace           isolated worker execution state
bookmark            external transport identity
tag                 immutable release identity
deployment record   observed deployed release state
```

There is no permanent `develop` bookmark.

One active writer per logical change is the normal path. Speculative alternatives receive separate changes from the same exact base.

Never move `main`, create releases, deploy, or push canonical state from a worker flow unless the current Atom explicitly grants the corresponding coordinator authority.

Verification applies to exact commit IDs. Rewriting a change after verification creates a new Candidate and invalidates commit-bound evidence even when the change ID remains stable.

## Black-box module rule

A module should expose contracts rather than internal structure.

Prefer:

```text
input contract -> deterministic or controlled transformation -> output contract
```

Internal representation may change without forcing callers to know about it.

Cross-crate dependencies must follow declared architecture direction. Avoid convenience imports that create hidden coupling.

## Performance rule

Correct-but-greedy is a baseline, not the finish line.

For algorithms on the Mission Graph, scheduler, evidence index, replay engine, persistence layer, Jujutsu adapter, and knowledge graph:

1. implement the simplest correct reference algorithm when useful;
2. state time and space complexity;
3. construct representative and adversarial benchmark datasets;
4. benchmark time, allocation, memory, throughput, tail behavior, and scaling shape as applicable;
5. compare optimized implementations against the reference for semantic equivalence;
6. preserve the reference implementation as a test oracle when cheap enough;
7. measure heuristic regret against an exact solver on small instances when the optimization problem permits it;
8. separate substrate overhead from worker, verifier, network, or model latency.

Never optimize from intuition alone. Never retain an elegant optimization whose benefit disappears under benchmark or whose complexity cost exceeds the measured gain.

Important scheduling lower bounds and baselines include total work, critical-path length, topological or list scheduling, resource constraints, and exact small-instance solutions. Heterogeneous workers should be evaluated with HEFT-style or comparable list-scheduling baselines rather than only a FIFO queue.

A scheduler that maximizes concurrent workers while increasing conflict, verification, repair, or model cost is optimizing the wrong proxy.

## Verification ladder

Use the strongest applicable method for each claim. A tool belongs only when it protects a named risk.

### Required everyday checks

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo run --locked -p gordian-kg -- validate
cargo run --locked -p gordian-kg -- audit

ruff check orchestration
python -m compileall -q orchestration/src
python -m unittest discover -s orchestration/tests

(cd formal && lake build)
```

CI additionally runs an independent Lean checker with `sorry` disallowed and audits the compiled environment for disallowed axioms.

### As applicable

- generated property and state-machine tests;
- differential randomized testing against executable Lean models;
- mutation testing with `cargo-mutants` or an equivalent method;
- fuzzing for parsers, protocols, event histories, JSON-LD, Jujutsu output, and command sequences;
- Miri and sanitizers for unsafe and memory-sensitive boundaries;
- Kani for bounded bit-precise safety or correctness harnesses;
- Loom for compact concurrent state spaces;
- Shuttle for broader deterministic randomized schedules;
- deterministic network and failure simulation such as Turmoil where appropriate;
- benchmark suites with Criterion or Divan and lower-noise instruction or allocation profiling where useful;
- process, storage, database, network, lease, stale-worker, duplicate-event, candidate-rewrite, and recovery fault injection.

Record configured bounds, seeds, toolchains, source commits, environment identity, and limitations. Bounded verification is not unbounded proof. A long stress test is not concurrency proof. A passing benchmark is not a complexity theorem.

## Research discipline

The knowledge graph is part of the engineering substrate.

Every material concept introduced by research, experiment, proof, specification, dependency, or failure must be represented in the canonical JSON-LD shards under `knowledge/graph/`, with inspectable relations to:

```text
source revisions and exact locators
claims and scope
assumptions and limitations
supporting, qualifying, challenging, or contradictory evidence
algorithms and complexity obligations
theorems and proof artifacts
experiments and exact runs
engineering decisions and alternatives
documents, issues, and implementation artifacts
```

Follow [`knowledge/acquisition.md`](knowledge/acquisition.md). A link in prose is not complete acquisition.

Distinguish:

```text
formal theorem
established scientific or engineering foundation
evidence-supported conclusion
engineering deduction
hypothesis
assumption
unresolved uncertainty
```

Actively record disconfirming and qualifying evidence. A graph that only accumulates support edges is a citation shrine, not research infrastructure.

When a preprint, standard, documentation page, repository, or dataset materially changes, create a source revision and propagate staleness or changed scope. Do not overwrite historical provenance.

## Experiment discipline

An experiment must state before execution:

- bounded hypothesis;
- falsification and architecture-revision rule;
- treatments, baselines, controls, and alternatives;
- workloads, sampling, seeds, budgets, and stopping rule;
- primary and secondary metrics;
- analysis plan and uncertainty method;
- exact source, tools, model/provider, environment, and hardware identity;
- raw artifact and run-ledger destinations;
- threats to validity.

Retain failed, timed-out, excluded, and negative runs with reasons. Do not keep only favorable trials. Conclusions remain bounded to the actual study scope.

Use [`.github/ISSUE_TEMPLATE/experiment.yml`](.github/ISSUE_TEMPLATE/experiment.yml) for experiment Atoms.

## Documentation style

Do not use maturity labels such as `v0`, `v1`, `v2`, `M0`, `M1`, or similar pseudo-roadmap version names for Gordian concepts, specifications, or phases.

Use semantic names:

```text
Mission Graph Specification
Foundation and Falsification Initiative
Reference Kernel Initiative
Distributed Coordination Initiative
```

Technical protocol, package, source, schema, and wire versions are allowed only when compatibility or reproducibility genuinely requires an explicit identifier.

Avoid status prose that will become stale. Prefer generated facts, experiment records, exact evidence, and links to canonical Atom identities.

## Atom contract

Use [`.github/ISSUE_TEMPLATE/atom.yml`](.github/ISSUE_TEMPLATE/atom.yml) for new implementation Atoms.

An Atom must define:

- Initiative and bounded objective;
- non-goals where scope could leak;
- causal dependencies and required interfaces;
- declared inputs, outputs, semantic reads/writes/provides/requires as applicable;
- effect class;
- acceptance predicates;
- verification and exact evidence subjects;
- benchmark obligation for performance-sensitive work;
- falsification or simplification trigger for experimental architecture.

Before declaring an Atom complete, provide:

- exact objective satisfied;
- implementation summary;
- tests and verifier evidence;
- benchmark evidence when required;
- knowledge-graph updates for material concepts or results;
- documentation updates for changed semantics;
- exact Jujutsu Candidate identity when supported;
- unresolved risks, assumptions, or deferred work.

Passing tests are necessary but not automatically sufficient when the Atom has stronger acceptance predicates. Closing a GitHub issue is bookkeeping, not native satisfaction evidence.

## Temporary GitHub substrate

GitHub issues and Project 9 are temporary external projections while Gordian builds its native planning substrate.

After granting local `gh` project scope:

```bash
gh auth refresh -s project
python -m pip install -e ./orchestration
gordian-project-sync --dry-run
gordian-project-sync --report artifacts/project-9-reconciliation.json
```

The reconciler may add missing issue URLs and report duplicates. It must not infer readiness, status, satisfaction, evidence, or acceptance from GitHub fields.

## No cargo-cult dependencies

Before adding a crate, service, database, workflow engine, agent framework, graph store, sandbox, or formal tool, state:

- which requirement it serves;
- what simpler alternative exists;
- performance and operational costs;
- failure, security, and maintenance surface;
- whether it becomes a production or development-only dependency;
- which benchmark, experiment, proof obligation, or threat model justifies retention.

Gordian exists partly to avoid reproducing accidental industry complexity. The same standard applies to Gordian itself.
