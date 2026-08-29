# Gordian End-to-End Project Plan

This plan is the implementation decomposition for Gordian itself. GitHub issues temporarily represent **Atoms** while Gordian builds the Mission Graph substrate intended to supersede that workflow.

The plan is ordered by causal dependency and evidence needs, not by feature glamour.

## Project Mission

Build a Rust-first coordination substrate that can represent engineering intent as a Mission Graph, execute work through isolated Jujutsu source states, coordinate human/agent workers, record deterministic execution/provenance, verify exact candidates, and admit only evidence-supported state through explicit authority.

Python remains a thin orchestration and experimentation layer. Lean and all formal dependencies remain development-only under `formal/`.

## Mission acceptance

The Mission is satisfied when a clean installation can demonstrate the following end to end:

1. define a Project/Mission/PlanRevision/Initiative/Atom graph through a typed interface;
2. reject structurally invalid decomposition/dependency state;
3. persist canonical work/events and rebuild derived state deterministically;
4. derive ready/blocked work without mutable workflow-status truth;
5. schedule compatible Atoms across one or more heterogeneous workers;
6. create isolated Jujutsu workspaces from exact base commits;
7. associate evolving implementations with change IDs and frozen candidates with exact commit IDs;
8. coordinate declared/observed semantic resource claims and leases;
9. run verifiers against exact candidates and store provenance-bound evidence;
10. invalidate stale evidence when relevant identity changes;
11. integrate independent candidates explicitly and re-verify composition;
12. prevent Worker authority from moving accepted or deployed frontiers;
13. promote an accepted frontier with race-safe compare-and-swap semantics;
14. replay after process failure without repeating nondeterministic effects;
15. expose CLI/API surfaces usable by humans and agent harnesses;
16. run the project's own Atom workflow through Gordian as a self-hosting proof;
17. publish benchmark and falsification evidence for the major Gordian-specific hypotheses;
18. keep the research knowledge graph synchronized with implemented/falsified concepts.

## Engineering constraints

- Production semantics are Rust.
- Python is orchestration/analysis only.
- Lean is development-only under `formal/`.
- Safe Rust is the default; `unsafe` requires isolated proof/test/performance justification.
- Reference algorithms remain available where useful as semantic/performance oracles.
- Every performance-sensitive algorithm gets complexity analysis plus representative/adversarial benchmarks.
- Every formal claim states assumptions and empirical boundary.
- Every agent-facing source mutation operates on an exact base.
- Every verified source result identifies an exact candidate commit.
- No permanent `develop` bookmark exists.
- Distribution is deferred until single-node semantics survive model/property/fault testing.

# Initiative: Foundation and Falsification

This Initiative establishes whether the ideas Gordian will depend on are actually reliable enough to justify implementation.

## Atom: Qualify and pin the Jujutsu development baseline

**Objective:** replace the reported local `jj 0.23.0` with a supported release and executable contract tests for required semantics.

**Acceptance:**

- document minimum supported Jujutsu release;
- bootstrap/upgrade instructions are reproducible in the Codex environment;
- fixtures prove change-ID rewrite persistence, exact commit identity, workspace isolation, sibling/parent topology, multi-parent integration, conflict representation, operation recovery, tag behavior, and `jj run` where required;
- unsupported/missing behavior becomes an explicit adapter constraint rather than an assumption.

**Evidence:** executable CLI contract-test results and captured Jujutsu version.

## Atom: Stabilize Rust, formal, Python, and CI foundations

**Objective:** establish deterministic development commands and strict continuous verification.

**Acceptance:**

- Rust formatting/clippy/tests pass under the pinned toolchain;
- knowledge graph validates and audits;
- Lean builds from `formal/`, independent type checking passes with `sorry` forbidden, axiom audit passes;
- Python package is independently lint/testable without production-semantic duplication;
- CI exposes each verification layer separately.

## Atom: Build deterministic benchmark and workload generators

**Objective:** create reusable synthetic/repository-derived workload generation before optimizing algorithms.

**Dimensions:**

```text
node count
edge count / density
DAG width
critical-path ratio
fan-in / fan-out
resource contention
worker heterogeneity
estimated duration/error distributions
semantic claim overlap
history/event volume
```

**Acceptance:** generated workloads are seed-reproducible and persisted with experiment metadata.

## Atom: Establish reference algorithm baselines

**Objective:** implement simple auditable baselines for topological validation, critical path, ready-queue/list scheduling, graph traversal, and evidence compatibility.

**Acceptance:** each baseline states asymptotic complexity, has property tests, and can serve as a differential oracle for optimized implementations.

## Atom: Establish performance benchmark gates

**Objective:** prevent greedy/reference algorithms from silently becoming production bottlenecks.

**Acceptance:** benchmark harness records wall time, CPU/instruction signal where useful, allocations/memory, workload shape, and regression thresholds. Scheduler comparisons include critical-path lower bounds and heterogeneous HEFT-style baselines.

## Atom: Establish the verification technique matrix

**Objective:** determine which verification technique catches which defect class and at what engineering cost.

**Candidate methods:** Rust types, property testing, mutation testing, fuzzing, Kani, Loom, Shuttle, Turmoil, Lean, differential Lean/Rust testing, integration/fault testing.

**Acceptance:** seeded defect matrix records coverage and cost; tools without a named useful defect/risk class are not mandatory dependencies.

## Atom: Build Lean/Rust differential conformance harness

**Objective:** connect executable formal models to optimized Rust semantics using generated inputs, Cedar-style.

**Acceptance:** a mismatch fails CI/experiment; mismatches are shrunk/persisted as regression fixtures; docs explicitly distinguish DRT evidence from a universal refinement proof.

## Atom: Complete research-graph coverage and epistemic audit

**Objective:** ensure every architecture-changing research concept has explicit Source/Claim/Assumption/Algorithm/Theorem/Experiment/Implementation relationships where applicable.

**Acceptance:** structural validation passes, epistemic audit has no errors, and documentation concepts have traceable graph identities.

# Initiative: Rust Mission Graph Kernel

## Atom: Implement strongly typed identities and immutable specification revisions

Build Rust newtypes and immutable Project/Mission/PlanRevision/Initiative/Atom/Quark specifications with constructors that reject structurally invalid states.

**Depends on:** Foundation CI; research graph coverage.

## Atom: Implement decomposition and hard-dependency validation

Implement typed decomposition rules, hard-dependency storage, cycle detection/topological certificates, and generated graph tests.

**Depends on:** typed identities; reference algorithms.

## Atom: Implement acceptance predicates and derived work state

Implement acceptance predicate representation/evaluation contracts and derived `Blocked`, `Enabled`, `Active`, and `Satisfied` semantics without mutable status as canonical truth.

**Depends on:** dependency validation; evidence interfaces.

## Atom: Implement ExecutionAttempt, Candidate, and effect-class semantics

Represent exact execution base, attempt outcomes, candidate freeze identity, and effect-aware retry/replay constraints.

**Depends on:** typed identities.

## Atom: Implement canonical Event and deterministic projection model

Implement append-oriented events plus pure in-memory projection and rebuild tests.

**Depends on:** core identities/attempt model; reference algorithms.

# Initiative: Scheduling and Coordination

## Atom: Implement dependency-aware ready queue and critical-path analysis

Produce deterministic ready work and critical-path/slack metrics from Mission dependencies.

**Depends on:** dependency validation; benchmark generators.

**Performance acceptance:** benchmark scaling against reference implementation over sparse/dense/wide/deep DAGs.

## Atom: Implement worker capability and resource compatibility

Model worker capabilities/resources/cost estimates separately from logical readiness; `Enabled` and `Dispatchable` remain distinct.

**Depends on:** ready queue; capability primitives.

## Atom: Implement semantic resource claims and scope observation

Represent read/write/provide/require claims; add instrumented observation interfaces and scope-expansion events.

**Depends on:** core Atom model; execution events.

**Experiment:** compare conflict prediction against path/module baselines before making semantic claims mandatory.

## Atom: Implement lease and fencing arbitration

Implement exclusive/shared leases, expiration/revocation, monotonically increasing fencing identities, and illegal-overlap rejection.

**Depends on:** semantic resources; capability model.

**Verification:** Lean transition model, Loom/Shuttle schedules, fault injection.

## Atom: Implement and benchmark heterogeneous scheduling policies

Implement deterministic greedy/list and HEFT-inspired policies behind a common policy interface.

**Depends on:** capability/resource compatibility; critical-path; benchmark harness.

**Acceptance:** policy selection is evidence-driven, with scheduler overhead included in total cost; opaque learned scheduling is out of scope until a real event corpus exists.

# Initiative: Evidence, Provenance, and Authority

## Atom: Implement content-addressed artifact storage

Build an artifact interface and local backend for immutable verifier/log/benchmark payloads with digest integrity, deduplication, and relocation-safe identity.

## Atom: Implement exact evidence fingerprints and freshness

Implement canonical bindings for specification, exact candidate, resolved inputs/dependencies, relevant environment, verifier, and canonicalization identity.

**Depends on:** Candidate semantics; artifact identity; Lean evidence model.

**Verification:** evidence mutation fault suite plus differential formal/Rust tests.

## Atom: Implement verifier manifests and verifier execution interface

Support required verifier sets, exact-subject execution, normalized results, byproducts, and environment capture.

**Depends on:** evidence fingerprint; artifact store.

## Atom: Implement provenance and attestations

Implement Entity/Activity/Actor provenance plus in-toto/SLSA-inspired attestation fields and export mappings.

**Depends on:** events; artifacts; evidence.

## Atom: Implement capability policy and evaluate Cedar

Build minimal capability semantics and benchmark/threat-model Cedar against an internal evaluator before deciding whether Cedar is production infrastructure.

**Depends on:** typed actors/capabilities; Foundation verification matrix.

## Atom: Implement candidate admission and accepted-frontier CAS

Implement reconciliation, structural conflict gate, required verification, freshness, authority check, and conditional accepted-frontier mutation.

**Depends on:** evidence, verifier, capability, event projection.

**Verification:** mutation tests must kill removed/inverted admission checks; concurrency/fault tests must expose no lost update.

# Initiative: Durable Persistence and Replay

## Atom: Implement PostgreSQL canonical persistence

Persist logical identities, immutable spec revisions, typed relations, attempts/candidates, evidence metadata, capabilities/leases, and canonical events transactionally.

**Depends on:** Rust core semantics.

## Atom: Implement materialized projections and deterministic rebuild

Persist query projections as disposable/rebuildable acceleration structures and continuously verify rebuild equality.

**Depends on:** PostgreSQL events; deterministic in-memory projector.

## Atom: Implement transactional frontier, lease, and plan-selection transitions

Use expected-version/CAS semantics and database constraints for globally visible state transitions.

**Depends on:** persistence; admission; lease arbitration.

## Atom: Build crash, duplicate-event, and recovery fault suite

Inject crashes between append/projection, duplicate/reordered messages, stale schemas, and uncertain acknowledgements.

**Depends on:** persistence/rebuild.

# Initiative: Jujutsu Change Plane

## Atom: Implement low-level Jujutsu command adapter and fixture repository

Use structured command execution/parsing behind a narrow Rust interface; no shell-string construction.

**Depends on:** qualified Jujutsu baseline.

## Atom: Implement workspace/change lifecycle

Spawn exact-base isolated workspaces, associate one normal-path writer with one logical change, observe change/commit identity transitions, and clean/recover workspaces safely.

**Depends on:** JJ command adapter; ExecutionAttempt model.

## Atom: Implement candidate freeze and exact commit handoff

Freeze candidate commit identity, make subsequent mutation create a distinct candidate, and bind provenance/evidence subject identity.

**Depends on:** workspace lifecycle; Candidate/evidence core.

## Atom: Implement sibling integration and conflict repair workflow

Preserve independent changes as siblings; create multi-parent integration candidates; represent conflict repair as bounded work; prohibit unresolved conflict admission.

**Depends on:** candidate handoff; admission.

## Atom: Implement exact-revision verification with Jujutsu

Use `jj run` or equivalently isolated exact-revision materialization on the supported baseline to run read-only verifier sets and collect exact-candidate evidence.

**Depends on:** JJ qualification; verifier interface.

## Atom: Run Jujutsu versus Git source-substrate experiment

Implement enough equivalent Git worktree orchestration to compare identity bookkeeping, recovery, integration, evidence invalidation, code complexity, and operator intervention.

**Depends on:** functional Jujutsu adapter and representative workload corpus.

# Initiative: Agent Execution and Thin Python Orchestration

## Atom: Define worker protocol and sandbox capability envelope

Define worker input/output/events, exact base/candidate semantics, allowed filesystem/process/network/secrets capabilities, resource budgets, cancellation, and abandonment.

## Atom: Implement generic process/agent worker adapter

Run a worker implementation without coupling Gordian semantics to a specific model vendor. Worker output is probabilistic/effectful input to deterministic validation.

**Depends on:** worker protocol; JJ workspace lifecycle; capability policy.

## Atom: Implement Python experiment orchestration package

Build thin runners for repeated trials, datasets/seeds, model/tool invocation, result collection, and statistical analysis while delegating all domain decisions to Rust.

**Depends on:** stable Rust CLI/protocol surfaces.

## Atom: Implement local multi-worker coordinator

Coordinate multiple isolated workers with dependency scheduling, semantic signals, leases, candidate integration, verification, and repair on one machine.

**Depends on:** scheduler; coordination; Jujutsu; evidence; worker adapter; persistence.

## Atom: Run isolation and coordination ablation suite

Compare solo, isolated-only, shared-status, and semantic-coordination conditions under fixed budgets and repeated trials.

**Depends on:** local coordinator; Python experiment runner.

# Initiative: Distributed Robustness

## Atom: Define remote worker transport and idempotent command protocol

Specify message identities, acknowledgements, retries, causal references, artifact transfer, heartbeats/liveness, and cancellation without assuming exactly-once delivery.

**Depends on:** stable local worker protocol/event semantics.

## Atom: Implement distributed lease/frontier coordination

Extend fencing and frontier transitions across multiple coordinator/worker processes while retaining a single authoritative acceptance linearization mechanism.

**Depends on:** transport; persistence; lease/frontier semantics.

## Atom: Build deterministic distributed fault simulation

Use Turmoil or an equivalent controlled simulator to inject partitions, latency, disconnects, duplicate messages, failover, stale leases, and uncertain acknowledgements.

**Depends on:** distributed transport/coordination.

## Atom: Instrument Gordian with OpenTelemetry-compatible observability

Expose traces/metrics/events for scheduling latency, blocking, conflicts, retries, stale evidence, projection rebuild, lease contention, verifier cost, and accepted-frontier age without optimizing blindly for those proxies.

**Depends on:** runtime semantics sufficiently stable to define useful signals.

# Initiative: Human and Programmatic Interface

## Atom: Implement Gordian CLI

Expose Project/Mission/plan/initiative/atom queries, dependency/evidence traversal, execution/admission commands, and inspection of exact source/evidence identities.

**Depends on:** Rust core plus persistence.

## Atom: Implement headless API

Expose stable typed API/streaming event surfaces for humans, UIs, and agents without making a web frontend canonical state.

**Depends on:** CLI/domain interfaces and authentication/capability policy.

## Atom: Implement GitHub bootstrap/import adapter

Import existing Gordian GitHub issues and relevant metadata into Mission Graph objects/provenance without treating GitHub Project status as authoritative semantics.

**Depends on:** Mission Graph persistence/API.

## Atom: Build Mission Graph/evidence explorer

Provide a human view over decomposition, dependencies, ready/blocked work, Jujutsu candidate topology, semantic contention, attempts, evidence, provenance, and research graph.

**Depends on:** headless API. UI implementation must be a projection over canonical facts.

# Initiative: Self-Hosting and Acceptance

## Atom: Import the Gordian implementation plan into Gordian itself

Convert the temporary GitHub issue substrate into native Project/Mission/Initiative/Atom state while retaining GitHub issue links as provenance.

**Depends on:** GitHub bootstrap/import; local runtime.

## Atom: Execute an end-to-end multi-agent self-hosting Mission

Select a bounded real Gordian feature, plan it through native Mission Graph, execute concurrent Jujutsu workers, collect evidence, integrate, promote accepted source state, and reconstruct the run after restart.

**Depends on:** all local-runtime critical path capabilities.

## Atom: Publish architecture falsification report

Aggregate the experiment ledger and explicitly retain, revise, or reject the major hypotheses: Mission ontology, Atom boundary, semantic claims, snapshot execution, Jujutsu advantage, derived state, formalization value, and scheduling policy.

**Depends on:** relevant experiments.

A negative result is a successful research outcome if it removes unjustified complexity.

# Critical path

The high-level causal spine is:

```text
Foundation/Falsification
    -> Rust Mission Graph Kernel
    -> Evidence + Scheduling + Persistence
    -> Jujutsu Change Plane
    -> Local Agent Coordinator
    -> CLI/API
    -> Self-hosting Mission
```

Distributed execution and rich human UI are intentionally outside the shortest path to validating the substrate.

# Project-management bootstrap

Until Gordian can self-host:

- each GitHub issue represents one Atom;
- the issue body records Initiative, objective, dependencies, acceptance, verification, and benchmark obligations;
- GitHub issue state is **not** scientific evidence of Atom satisfaction;
- exact code verification remains attached to candidate source identities;
- the GitHub Project view is a convenience projection only.

Once native Mission Graph persistence exists, these issues should be imported as provenance and Gordian should become the canonical coordination substrate for its own development.
