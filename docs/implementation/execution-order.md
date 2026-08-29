# Gordian End-to-End Execution Order

This document turns the Atom backlog into an execution strategy. GitHub issue numbers are temporary external identities. The causal relationships are the important part.

The implementation plan is not a linear ticket queue. Work should run concurrently only when interfaces and verification boundaries make the concurrency credible.

## 1. Mission

Build Gordian into a Rust-first, evidence-governed engineering coordination substrate that can:

1. represent a Project, Mission, alternative PlanRevisions, Initiatives, Atoms, and executor-local Quarks;
2. derive causal readiness independently from source history;
3. schedule humans and autonomous workers against exact source snapshots;
4. coordinate semantic intent without unrestricted shared mutation;
5. bind verification to exact candidates and relevant inputs/environment/verifiers;
6. integrate independent work explicitly through Jujutsu;
7. promote accepted state only through capability-gated, conflict-free, fresh-evidence admission;
8. persist and replay canonical state without repeating nondeterministic effects;
9. prove narrow substrate invariants and test the model-to-Rust bridge;
10. benchmark algorithms and falsify Gordian-specific architecture choices;
11. use its own Mission Graph to execute a real multi-worker Mission.

## 2. Global rules

### Foundation before dependence

No higher layer may rely on an algorithm, Jujutsu behavior, proof bridge, or storage choice that has not passed its qualification Atom.

### Reference before optimization

Every material graph, scheduling, evidence, replay, or query optimization begins with a simple deterministic oracle when practical.

### Safety outside policy ranking

Scheduling policies rank only work that already satisfies hard prerequisites, capability, authorization, resource, lease, effect, and conflict constraints.

### Exact subjects

Specifications, source candidates, evidence, artifacts, toolchains, and experiment runs use immutable revision identities. Logical evolving identities never substitute for exact verification subjects.

### One canonical semantics implementation

Rust owns production semantics. Python launches processes, builds experiment matrices, and aggregates raw measurements. Lean models narrow formal obligations. Neither Python nor Lean becomes an alternate production coordinator.

### Negative results simplify the system

Failed hypotheses create design changes or narrower scope. They are not buried beneath “future work.”

## 3. Entry state

The repository already contains:

- Apache-2.0 license and Rust workspace;
- `README.md`, `AGENT.md`, and canonical `AGENTS.md`;
- sharded JSON-LD research corpus;
- Rust `gordian-kg` traversal/validation tool;
- Lean models under `formal/`;
- a thin Python orchestration package;
- architecture, specification, algorithms, research, proof-boundary, falsification, and project-plan documents;
- the temporary GitHub Atom backlog;
- a local Jujutsu bootstrap script and protocol documentation.

These are a research and implementation scaffold. They are not evidence that Gordian’s runtime exists.

## 4. Execution spine

The shortest credible end-to-end causal spine is:

```text
#1 + #2 + #3 + #4 + #5 + #6 + #7 + #8
        |
        v
#9 -> #10 -> #11 -> #12 -> #13
        |
        +-----------------------+
        |                       |
        v                       v
#14 -> #15 -> #16 -> #17      #18
        |                       |
        +-----------+-----------+
                    v
                  #19
                    |
        +-----------+------------+
        |                        |
        v                        v
#20 -> #21 -> #22 -> #23 -> #24   #25 -> #26 -> #27 -> #28
        |                              |
        +---------------+--------------+
                        v
                  #29 -> #30 -> #31 -> #32 -> #33
                        |
                        v
                  #35 -> #36 -> #38
                        |
                        v
                       #49
                        |
                        v
                  #68 -> #69
```

This spine omits many necessary parallel and qualification Atoms for readability. It shows why implementing a UI or distributed worker first would be architecturally upside down.

## 5. Foundation and Falsification

### Execute first

- #1 qualify and pin Jujutsu;
- #2 stabilize Rust, Lean, Python, and CI;
- #3 deterministic workload generators;
- #4 simple reference algorithms;
- #5 benchmark and regression discipline;
- #6 verification-technique qualification;
- #7 Lean/Rust differential conformance;
- #8 research graph coverage and epistemic audit;
- #71 comprehensive research record schema;
- #72 ontology/closure/repository coverage enforcement;
- #73 epistemic traversal and impact queries;
- #74 reproducible acquisition and staleness propagation;
- #75 experiment run ledger and statistical contract.

### Parallelism

After #2:

```text
#1 Jujutsu qualification
#3 workload generators
#6 verification matrix
#8/#71 knowledge schema acquisition
```

may proceed concurrently because they touch separate development substrates. Interfaces must be published early:

- workload format from #3;
- reference algorithm contract from #4;
- proof/conformance test-vector format from #7;
- knowledge schema from #71;
- experiment manifest from #75.

### Exit gate

Do not treat the foundation as complete until:

- Rust CI is green;
- Lean build, independent checker, and axiom audit are green with no `sorry`;
- Jujutsu disposable-repository contracts pass on the selected release;
- benchmark workload generators are seed-reproducible;
- reference algorithms have stated complexity and correctness properties;
- knowledge graph validation and policy audit pass;
- experiment run manifests preserve failures/exclusions as well as successes;
- the Lean/Rust bridge catches intentionally injected disagreement.

## 6. Rust Mission Graph kernel

### Order

1. #9 typed identities and immutable specification revisions;
2. #58 Project resource registry and external identities;
3. #10 decomposition/dependency validation;
4. #11 attempts, candidates, and effect classes;
5. #12 canonical events and deterministic projection;
6. #13 acceptance predicates and derived state;
7. #55 PlanRevision lifecycle;
8. #56 planner proposal/validation interface;
9. #57 desired-versus-observed reconciliation and repair planning.

### Design gate

Before #10 stabilizes, prove and test:

- decomposition and dependency are separate edge classes;
- hard dependency cycles reject;
- creation or sibling order never creates causality;
- cross-Atom dependencies cannot target Quarks;
- immutable spec revisions preserve historical interpretation.

Before #13 stabilizes:

- attempt outcomes cannot mutate Atom meaning;
- candidate identity is exact and immutable;
- blocked/enabled/active are reproducible projections;
- satisfaction fails closed until compatible evidence exists.

### Benchmark gate

Measure:

- graph validation/topological order;
- incremental dependency changes;
- projection throughput/rebuild;
- reconciliation full scan versus incremental indexes;
- memory/allocation over wide, deep, sparse, and dense graphs.

## 7. Evidence, provenance, and authority

### Order

1. #14 content-addressed artifacts;
2. #15 exact evidence fingerprints;
3. #16 verifier manifests/exact-subject execution;
4. #17 provenance and attestations;
5. #18 capability policy and Cedar evaluation;
6. #19 accepted-frontier admission/CAS;
7. #59 stale-evidence prevention experiment;
8. #60 formal-method defect-yield experiment.

#14 and #18 can begin in parallel after the identity model stabilizes. #19 cannot precede all evidence and authority semantics it is meant to enforce.

### Safety gate

Admission must fail unless:

```text
candidate reconciles current frontier
and candidate has no unresolved conflict
and required verifiers passed
and evidence binds to the exact candidate
and evidence remains fresh
and actor has promotion authority
and compare-and-swap expectation still holds
```

No model assertion, worker status, issue closure, or green UI projection can bypass this predicate.

### Mutation gate

Use mutation testing to remove or invert each admission check. The suite must fail. A check that can be deleted without detection is not protected.

## 8. Scheduling and semantic coordination

### Order

1. #20 ready queue and critical-path analysis;
2. #21 worker capability/resource compatibility;
3. #22 semantic claims and observed scope;
4. #23 leases and fencing;
5. #24 scheduler policy comparison;
6. #52 semantic conflict predictor experiment.

### Scheduler separation

```text
Enabled       logical prerequisites and preconditions
Dispatchable  Enabled + executor/resources/auth/lease
Ranked        Dispatchable + selected heuristic priority
```

Never let a ranking policy manufacture dispatchability.

### Algorithm progression

```text
simple FIFO/list baseline
        ->
critical-path priority
        ->
resource/capability-aware list scheduling
        ->
HEFT-style heterogeneous comparison
        ->
contention-aware policy only if #52 supports semantic prediction
```

Use exact or brute-force solvers for small generated instances to measure heuristic regret.

### Exit gate

- safety properties hold independent of policy;
- inaccurate duration/conflict estimates are injected;
- policy overhead is separated from worker runtime;
- useful Mission progress and integration/verification cost are measured, not just worker utilization;
- the cheapest policy meeting measured goals is preferred.

## 9. Persistence and replay

### Order

1. #25 PostgreSQL canonical persistence;
2. #26 materialized projections/rebuild;
3. #27 transactional frontier, lease, and plan-selection transitions;
4. #28 crash/duplicate/recovery fault suite;
5. #66 backup, restore, migration, and compatibility qualification.

### Storage rule

Relational columns and constraints own stable semantics. JSONB holds explicitly identified extension payloads, not opaque core state.

### Recovery gate

The system must survive:

- crash before/after event append;
- crash before/after projection update;
- duplicate canonical events;
- stale expected versions;
- lost acknowledgement/uncertain command result;
- projection deletion or corruption;
- migration interruption;
- clean restore into another environment.

The same canonical history must rebuild the same canonical projection under the stated deterministic assumptions.

## 10. Jujutsu Change Plane

### Order

1. #1 baseline qualification and local bootstrap;
2. #29 bounded Rust CLI adapter and disposable fixture repo;
3. #30 workspace/change lifecycle;
4. #31 candidate freeze/exact handoff;
5. #32 sibling integration/conflict repair;
6. #33 exact-revision verification;
7. #34 Jujutsu versus Git experiment.

### Local development bootstrap

At the local repository:

```bash
cd ~/projects/project-management-tools/gordian
bash scripts/bootstrap-jj.sh --install
```

The script configures the candidate release, `origin`, tracking, and `trunk()`. It does not push or rewrite source.

### Adapter gate

Do not spread shell commands across the runtime. The Rust adapter owns:

- structured argv/cwd/env;
- supported-version/feature checks;
- bounded machine-readable parsing;
- exact change/commit/workspace identities;
- errors and evidence artifacts;
- disposable fixtures.

### Experiment gate

#34 must be permitted to reject Jujutsu-specific complexity. Hold the Mission Graph, scheduler, workers, verification, and workloads constant while comparing the source adapter.

## 11. Agent execution and Python orchestration

### Order

1. #35 worker protocol and capability envelope;
2. #62 sandbox backend qualification;
3. #63 secret/credential brokerage;
4. #36 generic process/agent adapter;
5. #37 thin Python experiment orchestration;
6. #38 local multi-worker coordinator;
7. #39 isolation/coordination ablation;
8. #53 snapshot versus continuous-rebase experiment.

### Rust/Python boundary

Rust:

```text
worker protocol
attempt/candidate state
scheduler
capabilities
sandbox policy
Jujutsu adapter
evidence/admission
canonical events
```

Python:

```text
trial matrices
subprocess launching
source retrieval staging
raw result collection
statistical analysis orchestration
GitHub Project bootstrap
```

Python never independently decides ready, dispatchable, fresh, satisfied, authorized, or accepted.

### Local coordinator gate

Before remote workers:

- multiple deterministic fixture workers execute independent Atoms;
- exact bases/workspaces remain isolated;
- semantic signals and leases coordinate intent;
- candidate verification/integration/admission works end to end;
- worker crash and coordinator restart recover;
- performance traces separate useful work, verifier cost, conflict/repair, and coordinator overhead.

## 12. Distributed robustness

### Order

1. #40 remote transport/idempotent commands;
2. #41 distributed lease/frontier coordination;
3. #42 deterministic fault simulation;
4. #43 OpenTelemetry-compatible observability;
5. #67 adversarial security and authority qualification.

Remote execution is not required for the first useful Gordian. Do not add it until local self-hosting reveals a real need and the canonical state machine is stable enough to simulate.

### Exit gate

- duplicate/reordered/delayed messages preserve safety;
- uncertain outcomes do not trigger blind irreversible retries;
- stale fencing tokens fail;
- coordinator failover cannot create two accepted-frontier writers;
- simulation seeds reproduce failures;
- real integration tests qualify the gap between simulation and operating system/network behavior.

## 13. Interfaces and temporary GitHub projection

### Order

1. #44 CLI;
2. #45 typed API/event stream;
3. #46 GitHub import adapter;
4. #47 Mission/evidence explorer;
5. #70 reconcile issues into GitHub Project 9.

The CLI should precede the API/UI because it exercises domain commands with the least transport/UI surface.

GitHub issues and Project 9 are external planning projections. Their status is not native evidence and cannot establish Atom satisfaction.

## 14. Release and operations

### Order

1. #64 immutable release/deployment records;
2. #65 reproducible signed artifacts;
3. #66 backup/restore/migration;
4. #67 adversarial security qualification.

Lean sources/checkers and large experiment corpora remain development artifacts. Runtime distributions should not accidentally ship them.

## 15. Self-hosting and architecture retention

### Order

1. #48 import Gordian’s own plan;
2. #49 execute a real bounded multi-worker Mission;
3. complete architecture experiments #34, #39, #50–#54, #59–#61;
4. #68 publish retain/revise/reject decisions;
5. #69 produce the end-to-end qualification evidence bundle.

### Self-hosting acceptance

The bounded Mission must include:

- at least two causally independent Atoms;
- isolated Jujutsu workspaces from exact bases;
- shared semantic coordination;
- exact candidate handoff and verification;
- explicit integration and re-verification;
- authorized acceptance;
- coordinator kill/restart and replay;
- complete provenance/evidence;
- critical-path, useful-parallelism, verifier, integration, and coordinator-cost reporting.

A run that eventually succeeds but is pathologically slow is not sufficient.

## 16. Experimental decision matrix

| Hypothesis | Primary Atom | Decision affected |
| --- | --- | --- |
| Mission hierarchy earns its overhead | #50 | keep/simplify ontology |
| Atom/Quark is useful scheduling boundary | #51 | granularity and abstraction |
| Semantic claims beat path/module predictors | #52 | claim/lease complexity |
| Stable snapshots beat active rebasing | #53 | worker source policy |
| Derived state reduces drift | #54 | projection/status semantics |
| Exact fingerprinting prevents stale completion | #59 | evidence boundary |
| Formal methods add unique defect yield | #60 | Lean scope |
| Current graph backend remains sufficient | #61 | storage/query backend |
| Coordination beats isolation alone | #39 | shared semantic plane |
| Jujutsu beats Git for Gordian | #34 | source adapter dependency |

#68 must propagate negative results into specifications, docs, knowledge nodes, code removal, and follow-up Atoms.

## 17. Critical performance suite

Before qualification, benchmark at least:

### Mission Graph

- validation/topological order;
- ready-queue updates;
- critical-path calculation;
- full/incremental reconciliation;
- memory by graph shape.

### Scheduler

- ranking and matching latency;
- heuristic regret on small exact-solved instances;
- makespan/critical-path efficiency;
- contention, conflict, and verifier/retry cost;
- robustness to bad estimates.

### Evidence and provenance

- canonical fingerprint generation;
- fresh/stale lookup;
- artifact put/get/verify;
- provenance closure;
- admission latency/contention.

### Persistence

- event append;
- projection update/rebuild;
- transactional CAS/lease contention;
- backup/restore/migration.

### Knowledge graph

- parse/merge/canonicalize/audit;
- typed path/closure/impact queries;
- source-revision and metadata scaling;
- file/petgraph versus candidate backends.

### Jujutsu

- workspace operations;
- exact identity/revset queries;
- `jj run` materialization/parallel verification;
- integration/conflict/recovery;
- scaling by changes/workspaces/repository topology.

### Coordinator

- spawn/dispatch overhead;
- idle/load memory;
- worker-count scaling;
- event/telemetry volume;
- end-to-end Mission work/span efficiency.

## 18. Definition of end-to-end implementation

Gordian is implemented end to end only when:

- a clean installation can create and query native Mission Graph state;
- all canonical semantics are Rust-owned;
- formal proof jobs and Rust conformance evidence are green for retained obligations;
- Jujutsu behavior is contract-qualified and adapter-bounded;
- workers execute in enforceable capability/sandbox boundaries;
- scheduling is dependency/resource/authority safe and benchmark-selected;
- candidate evidence is exact and stale results fail closed;
- persistence/replay/recovery/backup have executable evidence;
- accepted, release, and deployment frontiers are distinct;
- the system executes its own bounded multi-worker Mission;
- architectural hypotheses have retain/revise/reject decisions;
- the qualification bundle binds every claim to exact source, tools, environment, artifacts, and evidence;
- unresolved limitations are published plainly.

Until then, Gordian is an increasingly rigorous experiment, not a completed autonomous development system.
