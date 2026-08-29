# Gordian

Gordian is a research-driven coordination substrate for software development by humans and autonomous agents.

It is not a project tracker wrapped around Git, and it is not a branch convention. Gordian treats engineering as a closed-loop coordination problem: specify what should become true, decompose the objective into verifiable contracts, execute against controlled source state, collect evidence, reconcile observations with intent, and admit only justified state into the accepted frontier.

Licensed under the **Apache License 2.0**.

## Core model

Gordian separates structures that conventional development tooling often collapses:

```text
Mission Graph       what should become true
Change Graph        what exact/evolving source states exist
Execution History   what actors actually attempted and observed
Evidence Graph      what claims are justified by which evidence
Authority Model     who may mutate accepted or external reality
```

The canonical planning abstraction is the **Mission Graph**:

```text
Project
  Mission
    PlanRevision
      Initiative
        Atom
          Quark
```

This is decomposition, not execution order. Hard causal prerequisites form a separate DAG.

- **Mission** is goal + constraints + acceptance, independent of one implementation strategy.
- **PlanRevision** is an immutable strategy for attempting the Mission.
- **Initiative** is a compound capability or subgoal.
- **Atom** is the smallest globally schedulable and independently verifiable work contract.
- **Quark** is executor-local structure inside an Atom.
- **ExecutionAttempt** is one concrete attempt against an exact base.
- **Candidate** is a frozen exact implementation subject handed to verification.
- **Evidence** is an observation relevant to an acceptance predicate.
- **Attestation** binds a claim to exact subjects, activity, actor, and materials.

The Atom/Quark boundary and even the Mission Graph ontology itself are hypotheses to test, not words made true by specification.

## Research thesis

The current multi-agent software-engineering evidence does not reduce to “give every agent a branch.” The bounded synthesis Gordian is testing is:

> **isolated code state + coordinated semantic state + dependency-aware scheduling + explicit integration + exact-artifact verification + capability-gated acceptance**

The ingredients draw from hierarchical and partial-order planning, DAG scheduling, resource-constrained scheduling, critical-path/work-span analysis, optimistic concurrency and snapshot isolation, workflow theory, provenance/attestation, content addressing, durable execution, capability security, formal methods, and recent agent-engineering studies.

The repository explicitly distinguishes:

```text
formal theorem
established scientific / engineering foundation
evidence-supported conclusion
engineering deduction
hypothesis
assumption
unresolved uncertainty
```

## Rust first, Python thin

**Rust is the production substrate.** It owns Mission Graph semantics, graph algorithms, scheduling, event projection, evidence/provenance, Jujutsu integration, persistence, authorization, leases, protocol parsing, and runtime hot paths.

**Python is a thin orchestration layer** under [`orchestration/`](orchestration/). It launches experiments/tools/agents, generates datasets, and aggregates measurements. It must not become a second implementation of readiness, evidence freshness, authorization, leases, acceptance, or Mission satisfaction.

**Lean is a development dependency** isolated under [`formal/`](formal/). Production Gordian does not require Lean at runtime.

## Verification-guided development

For safety-critical semantics Gordian follows a verification-guided pattern similar to the one demonstrated by Cedar:

```text
small executable Lean model
        ->
machine-checked model invariants
        ->
optimized Rust implementation
        ->
differential randomized conformance testing
        ->
property / mutation / fuzz / concurrency / integration tests
```

A Lean theorem proves the formal proposition under its assumptions. It does **not** prove that an empirical architecture is faster, that a semantic-resource declaration is complete, or that Rust refines the model unless that bridge is itself established.

The current Lean sources cover rank-certified dependency acyclicity, scheduling witness implications, evidence identity mismatch, authority separation, acceptance witnesses, declared non-interference symmetry, and deterministic replay identity.

```bash
cd formal
lake build
```

CI additionally uses an independent checker with `sorry` disallowed and audits axioms.

## Jujutsu Change Graph

Jujutsu is Gordian's preferred source-state adapter, subject to an explicit Git comparison experiment.

| Gordian concept | Jujutsu representation |
| --- | --- |
| accepted source frontier | `trunk()` / public `main` projection |
| exact source state | commit ID |
| evolving implementation identity | change ID |
| isolated worker state | workspace |
| independent work | sibling changes |
| causal source dependency | parent/child changes |
| integration candidate | multi-parent change |
| revision-scoped verification | `jj run` on supported releases |
| VCS recovery/history | operation log |
| external transport identity | bookmark |
| immutable release identity | tag |
| production truth | separate deployment record |
| permanent `develop` bookmark | **none** |

Operational rule:

> Bookmarks represent external identities. Changes represent evolving implementations. Workspaces represent execution. Graph topology represents causality. Verification binds to exact commits.

The reported Codex environment currently has **Jujutsu 0.23.0**, which predates capabilities this design intends to test, including `jj run`. The Foundation Initiative therefore upgrades, pins, and contract-tests the supported Jujutsu baseline before the source adapter is implemented.

## Executable research knowledge graph

The research corpus is a canonical set of JSON-LD shards under [`knowledge/graph/`](knowledge/graph/), governed by [`knowledge/ontology.md`](knowledge/ontology.md).

It contains concepts, sources, claims, hypotheses, assumptions, algorithms, theorems, experiments, tools, standards, implementation artifacts, and documents across:

- Mission Graph semantics;
- HTN and partial-order causal-link planning;
- DAG/topological/critical-path scheduling;
- RCPSP, list scheduling, HEFT/CPOP, work/span, and work stealing;
- OCC, MVCC, snapshot isolation, conflict reasoning, leases, fencing, CAS;
- workflow nets, Petri nets, CRDTs, semantic convergence;
- hermeticity, content addressing, Merkle DAGs;
- W3C PROV, in-toto, SLSA;
- deterministic replay and effect boundaries;
- CAID, STORM, AgentRoom, AgenticFlict, CodeTeam, reliability synthesis;
- Lean, Cedar-style verification-guided development, DRT, property/mutation/fuzz/model/concurrency testing;
- Rust implementation and benchmark choices.

The Rust `gordian-kg` tool deterministically merges and indexes the shards:

```bash
cargo run -p gordian-kg -- validate
cargo run -p gordian-kg -- audit
cargo run -p gordian-kg -- stats
cargo run -p gordian-kg -- list --kind Algorithm
cargo run -p gordian-kg -- hypotheses
cargo run -p gordian-kg -- evidence claim:semantic-state-vs-code-state
cargo run -p gordian-kg -- neighbors concept:atom
cargo run -p gordian-kg -- path concept:atom theorem:dispatch-requires-dependencies
cargo run -p gordian-kg -- export-dot --out /tmp/gordian.dot
```

Traversal is not entailment. Edge type determines the epistemic meaning.

## Performance is a design constraint

A correct greedy implementation is a **reference baseline**, not the destination.

Performance-sensitive algorithms must state complexity, retain a simple oracle when practical, and benchmark representative plus adversarial workloads before Gordian depends on an optimization.

The Foundation Initiative includes:

- graph generation across size, density, width, and critical-path shapes;
- critical-path and topological baselines;
- FIFO/greedy list scheduling;
- HEFT-style heterogeneous scheduling comparison;
- semantic-conflict predictor ablations;
- knowledge-index scaling;
- event/projection and persistence scaling;
- allocation/memory measurements;
- deterministic instruction-level profiling where wall-clock noise obscures regressions.

The objective is not “maximize agents running.” It is useful Mission progress under correctness, verification, resource, and cost constraints.

## Repository map

```text
AGENTS.md                       canonical coding-agent contract
AGENT.md                        human-facing pointer to AGENTS.md
LICENSE                         Apache-2.0
Cargo.toml                      Rust workspace
rust-toolchain.toml             Rust toolchain

crates/
  gordian-kg/                   executable research graph tooling

formal/
  lean-toolchain                Lean development toolchain
  lakefile.toml                 isolated formal package
  Gordian.lean
  Gordian/*.lean                formal models and proofs

knowledge/
  ontology.md
  graph/*.jsonld                sharded canonical research corpus

orchestration/
  pyproject.toml
  src/gordian_orchestration/    thin Python process/experiment layer

docs/
  index.md
  architecture.md
  knowledge-graph.md
  spec/
    mission-graph.md
    data-model.md
    invariants.md
  protocols/
    jujutsu-agent-protocol.md
  algorithms/
    scheduling.md
    evidence-and-admission.md
    reconciliation.md
  formal/
    proof-boundary.md
    theorem-catalog.md
  research/
    methodology.md
    foundations.md
    agent-systems-2026.md
    evidence-synthesis.md
  testing/
    falsification-plan.md
  implementation/
    project-plan.md
```

## Development contract

Read [`AGENTS.md`](AGENTS.md) before implementation. Current Codex tooling recognizes `AGENTS.md` as the repository instruction file; [`AGENT.md`](AGENT.md) exists as the requested singular human entrypoint.

Baseline checks:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p gordian-kg -- validate
cargo run -p gordian-kg -- audit
(cd formal && lake build)
```

Targeted validation layers include property testing, mutation testing, fuzzing, Kani, Loom/Shuttle, Turmoil, differential Lean/Rust testing, benchmark regression gates, and fault injection. Each tool must protect a named risk; Gordian does not collect verification tooling as ornaments.

## Project execution

The implementation plan is [`docs/implementation/project-plan.md`](docs/implementation/project-plan.md). GitHub issues are used temporarily as Atom records while Gordian builds the substrate intended to replace that workflow.

The first Initiative is deliberately **Foundation and Falsification**: qualify Jujutsu, stabilize proof checking, build the benchmark corpus, establish Rust/Lean conformance testing, and measure alternative algorithms before higher layers rely on them.
