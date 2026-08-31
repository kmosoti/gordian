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

**Rust is planned to be the production substrate.** It is planned to own Mission Graph semantics, graph algorithms, scheduling, event projection, evidence/provenance, the source adapter, persistence, authorization, leases, protocol parsing, and runtime hot paths. Today the workspace contains exactly one crate, [`crates/gordian-kg`](crates/gordian-kg), which owns the research knowledge graph and nothing else. Which crate each planned capability lands in is decided by [`docs/implementation/crate-map.md`](docs/implementation/crate-map.md), in the order of [`docs/implementation/execution-order.md`](docs/implementation/execution-order.md).

**Python is a thin orchestration layer** under [`orchestration/`](orchestration/). It launches experiments/tools/agents, generates datasets, and aggregates measurements. It must not become a second implementation of readiness, evidence freshness, authorization, leases, acceptance, or Mission satisfaction.

**Lean is a development dependency** isolated under [`formal/`](formal/). Production Gordian does not require Lean at runtime.

### Planned, not built

This repository is a specification and a research corpus with one tool in it. The table below is
the whole list of capabilities the documentation is allowed to describe in the planned tense, and
what makes each claim true. `scripts/check-capability-tense.sh` fails CI when this file or
[`AGENTS.md`](AGENTS.md) states one of them in the present tense before its Atoms have validating
closure records.

| Planned capability | Specified in | Becomes present tense when |
| --- | --- | --- |
| Rust owns canonical Mission Graph semantics | [`docs/spec/data-model.md`](docs/spec/data-model.md) | closure records for #9, #10, #12, #13 |
| A Lean-to-Rust conformance pipeline | [`docs/formal/conformance-vectors.md`](docs/formal/conformance-vectors.md) — no vectors exist yet | closure record for #7 |
| The scheduler dispatches Atoms across workers | [`docs/algorithms/scheduling.md`](docs/algorithms/scheduling.md) | closure records for #20, #21, #24 |
| The evidence store binds evidence to exact candidates | [`docs/spec/data-model.md`](docs/spec/data-model.md) | closure records for #15, #16, #17 |
| The Jujutsu adapter drives workspaces and candidates | [`docs/protocols/source-adapter-contract.md`](docs/protocols/source-adapter-contract.md) | closure records for #29, #30, #31 |

## Verification-guided development

For safety-critical semantics Gordian intends to follow a verification-guided pattern similar to the one demonstrated by Cedar. The pipeline below is **not yet implemented**; #7 owns the Lean/Rust conformance harness and the vector format it consumes, and no conformance vectors exist yet.

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

The current Lean sources cover rank-certified dependency acyclicity, scheduling witness implications, evidence identity mismatch, authority separation, acceptance witnesses, declared non-interference symmetry, and deterministic replay identity. They are **proposition-level models, not executable oracles**: nothing in Rust is differentially tested against them today, and a theorem here constrains the model, not an implementation.

```bash
scripts/verify-local.sh formal
```

The formal verifier builds with warnings treated as errors, replays the compiled environment with
the pinned toolchain's `leanchecker`, and runs [`formal/Gordian/Audit.lean`](formal/Gordian/Audit.lean).
`leanchecker` is a separate environment-replay pass through Lean's kernel, not an independently
implemented kernel.
The audit rejects `sorryAx`, project-declared axioms, and every transitive theorem dependency
outside `{propext, Classical.choice, Quot.sound}`. The same script is CI's command source.

## Source plane and the Jujutsu Change Graph

The source plane is adapter-neutral. [`docs/protocols/source-adapter-contract.md`](docs/protocols/source-adapter-contract.md) is the trait; Jujutsu (#29-#33) and Git worktrees (#76) are two realizations of it, and #34 compares them with everything else held constant. Canonical records name a `logical_change_id` and an `exact_state_id`, never a backend's own vocabulary.

Jujutsu is Gordian's development-baseline source-state adapter, subject to that comparison. This
bounded adoption does not claim superiority over Git; issue #34 is the experiment for that claim.

| Gordian concept | Jujutsu representation |
| --- | --- |
| accepted source frontier | `trunk()` / public `main` projection |
| exact source state (`exact_state_id`) | commit ID |
| evolving implementation identity (`logical_change_id`) | change ID |
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

[`scripts/bootstrap-jj.sh`](scripts/bootstrap-jj.sh) is the **single source of the pinned Jujutsu baseline**; no document restates that version number, so the two cannot drift. The script installs and configures that release, adds and fetches `origin`, tracks `main@origin`, defines `trunk()`, and checks that the `jj run` subcommand is registered. It is not a behavioral contract test. Issue **#1 owns the contract suite** that qualifies change-ID rewrite persistence, exact state identity, workspace isolation, sibling topology, multi-parent integration, conflict representation, operation recovery, and `jj run` behavior on a disposable repository; until it closes, those semantics are assumed, not qualified.

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

The Rust `gordian-kg` tool deterministically merges and indexes the shards. This block is the complete subcommand surface; nothing asserts that it stays that way yet (see below):

```bash
cargo run -p gordian-kg -- validate
cargo run -p gordian-kg -- audit --strict
cargo run -p gordian-kg -- stats
cargo run -p gordian-kg -- list --kind Algorithm
cargo run -p gordian-kg -- show concept:atom
cargo run -p gordian-kg -- search "critical path"
cargo run -p gordian-kg -- hypotheses
cargo run -p gordian-kg -- theorems
cargo run -p gordian-kg -- evidence claim:semantic-state-vs-code-state
cargo run -p gordian-kg -- neighbors concept:atom
cargo run -p gordian-kg -- path concept:atom theorem:dispatch-requires-dependencies
cargo run -p gordian-kg -- export-dot --out /tmp/gordian.dot
```

`crates/gordian-kg/tests/cli.rs` runs every command above against the canonical graph and asserts
that this block and clap expose the same complete subcommand set.

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
LICENSE                         Apache-2.0
Cargo.toml                      Rust workspace
Cargo.lock                      pinned dependency graph
rust-toolchain.toml             Rust toolchain
.gitignore

.github/
  workflows/verify.yml           CI: Rust, formal, Python, specification consistency
  ISSUE_TEMPLATE/atom.yml        implementation Atom contract
  ISSUE_TEMPLATE/experiment.yml  falsifiable study contract
  PULL_REQUEST_TEMPLATE.md       closure-evidence checklist (a PR is not the admission gate)

crates/
  gordian-kg/                   executable research graph tooling (the only crate today)

formal/
  lean-toolchain                Lean development toolchain
  lakefile.lean                 isolated formal package
  lake-manifest.json
  Gordian.lean
  Gordian/*.lean                formal models and proofs

knowledge/
  ontology.md                   node and relation semantics
  acquisition.md                source revision identity and acquisition lifecycle
  graph/*.jsonld                sharded canonical research corpus

orchestration/
  pyproject.toml
  README.md
  src/gordian_orchestration/    thin Python process/experiment layer
  tests/

scripts/
  bootstrap-jj.sh               pinned Jujutsu baseline install and configuration
  sync_github_project.py        compatibility entrypoint into the Project reconciler
  check-*.sh                    specification-consistency checkers, all run by CI

artifacts/
  schema/closure-record.schema.json   the normative definition of Atom closure
  atoms/<N>/                          spec snapshot, verifier artifacts, closure.json
  project-9-reconciliation.json       generated board reconciliation report

docs/
  index.md
  architecture.md
  knowledge-graph.md
  spec/
    mission-graph.md
    data-model.md
    invariants.md
  protocols/
    source-adapter-contract.md
    jujutsu-agent-protocol.md
    jujutsu-development-environment.md
    landing.md
  algorithms/
    scheduling.md
    evidence-and-admission.md
    reconciliation.md
  formal/
    proof-boundary.md
    theorem-catalog.md
    conformance-vectors.md
  research/
    methodology.md
    foundations.md
    agent-systems-2026.md
    evidence-synthesis.md
    verification-strategy.md
  testing/
    falsification-plan.md
    statistical-contract.md
  implementation/
    project-plan.md
    execution-order.md
    issue-index.md
    agent-runbook.md
    crate-map.md
```

Planned trees, created by the Atom named beside them: `benches/` (#5), `experiments/` (#75), and
`formal/conformance/` (#7).

## Development contract

Read [`AGENTS.md`](AGENTS.md) before implementation. It is the repository's sole coding-agent instruction file.

Install the exact development toolchain pins, then run the complete local/CI gate:

```bash
scripts/install-toolchains.sh all
# add the installer-reported bin directories to PATH
scripts/check-toolchain.sh --runtime
scripts/verify-local.sh all
```

The installer reads every version and source digest from its owning pin file; it does not restate
them. `verify-local.sh` exposes the five evidence groups (`rust-check`, `kg-audit`, `formal`,
`python`, and `spec-consistency`) plus the aggregate `rust` group used by CI, so local evidence and
CI share one command source. The Rust checks include `cargo deny check`; `--strict` remains
mandatory for the graph audit.

Targeted validation layers include property testing, mutation testing, fuzzing, Kani, Loom/Shuttle, Turmoil, differential Lean/Rust testing, benchmark regression gates, and fault injection. Each tool must protect a named risk; Gordian does not collect verification tooling as ornaments.

## Project execution

**Start with [`docs/implementation/agent-runbook.md`](docs/implementation/agent-runbook.md).** It is the loop an autonomous agent executes end to end: acquire the repository and toolchain, derive readiness from the native GitHub `blocked by` graph with `gordian-derive-status ready`, claim an Atom, create an isolated workspace from an exact base, execute, verify, land, write the closure record, recompute the board, update the knowledge graph, and evaluate the Mission stop condition with `scripts/check-mission-stop-condition.sh --gate`.

The plan is 77 Atoms across 14 Initiatives.
[`docs/implementation/project-plan.md`](docs/implementation/project-plan.md) carries the normative
Mission acceptance table and a derived view of the whole set;
[`docs/implementation/execution-order.md`](docs/implementation/execution-order.md) carries the
causal spine, the kernel-start gate, and the 43-Atom minimal self-hosting prerequisite set;
[`docs/implementation/issue-index.md`](docs/implementation/issue-index.md) is the Initiative
register and defines the bootstrap satisfaction rule. **The executable Atom contracts are the
GitHub issue bodies**, used temporarily as Atom records while Gordian builds the substrate
intended to replace that workflow.

Closing a GitHub issue is bookkeeping. An Atom is closed when
[`artifacts/schema/closure-record.schema.json`](artifacts/schema/closure-record.schema.json)
validates its `artifacts/atoms/<N>/closure.json`.

The first Initiative is deliberately **Foundation and Falsification**: stabilize the toolchain and CI, build the benchmark corpus and reference algorithms, and make the research corpus mechanically auditable. Only #2, #3, #4, #8, #71, and #72 gate the Mission Graph kernel; Jujutsu qualification, benchmark gates, the verification pilot, conformance testing, and the acquisition layers run concurrently with it and are consumed where `execution-order.md` section 5 says they are.
