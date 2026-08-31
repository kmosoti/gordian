# Gordian End-to-End Execution Order

This document turns the Atom backlog into an execution strategy. GitHub issue numbers are temporary external identities. The causal relationships are the important part.

The implementation plan is not a linear ticket queue. Work should run concurrently only when interfaces and verification boundaries make the concurrency credible.

**The native GitHub `blocked by` graph is the single authority for dependencies.** The maximum-path
arrows are generated from that graph, and phase membership is deliberately unordered. Where a
view and the graph disagree, the graph is right and `gordian-atom-registry check` fails.
`gordian-atom-registry render-spine --write` owns the marked block below; this is the executable
implementation of **G-518 and G-433, assigned to #70**.

Sections 5-15 are **phases of the causal spine**, not Initiatives. An Initiative is a GitHub
milestone ([`issue-index.md`](issue-index.md)); a phase groups Atoms by when their prerequisites
are discharged, and routinely spans milestones. No Atom appears in more than one phase membership
list. Those lists are sets, not schedules: the only executable order is the native
graph projected by `gordian-derive-status ready`.

## 1. Mission

Build Gordian into a Rust-first, evidence-governed engineering coordination substrate that can:

1. represent a Project, Mission, alternative PlanRevisions, Initiatives, Atoms, and executor-local Quarks;
2. derive causal readiness independently from source history;
3. schedule humans and autonomous workers against exact source snapshots;
4. coordinate semantic intent without unrestricted shared mutation;
5. bind verification to exact candidates and relevant inputs/environment/verifiers;
6. integrate independent work explicitly through a source adapter, of which Jujutsu is one realization;
7. promote accepted state only through capability-gated, conflict-free, fresh-evidence admission;
8. persist and replay canonical state without repeating nondeterministic effects;
9. prove narrow substrate invariants and test the model-to-Rust bridge;
10. benchmark algorithms and falsify Gordian-specific architecture choices;
11. use its own Mission Graph to execute a real multi-worker Mission.

## 2. Global rules

### Foundation before dependence

No higher layer may rely on an algorithm, source-adapter behavior, proof bridge, or storage choice that has not passed its qualification Atom.

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
- `README.md` and canonical `AGENTS.md`;
- sharded JSON-LD research corpus;
- Rust `gordian-kg` traversal/validation tool;
- Lean models under `formal/`;
- a thin Python orchestration package;
- architecture, specification, algorithms, research, proof-boundary, falsification, landing, source-adapter, crate-map, agent-runbook, and project-plan documents;
- the temporary GitHub Atom backlog;
- a local Jujutsu bootstrap script and protocol documentation.

These are a research and implementation scaffold. They are not evidence that Gordian’s runtime exists.

## 4. Execution spine

The spine is the set of Atoms lying on at least one **maximum-length** blocker path to #69 in the
post-revision graph. That path is 18 edges long, and every arrow drawn below is a real native
`blocked by` edge — nothing here is a reading preference.

<!-- BEGIN GENERATED: MAXIMUM-LENGTH SPINE -->

```text
#2 -> #8 -> #71 -> #72 -> #9 -> #11 -> #12 -> #58 -> #10 -> #13 -> #19 -> #26 -> #44 -> #45 -> #46 -> #48 -> #49 -> #68 -> #69
#2 -> #8 -> #71 -> #72 -> #9 -> #11 -> #12 -> #58 -> #10 -> #13 -> #19 -> #32 -> #44 -> #45 -> #46 -> #48 -> #49 -> #68 -> #69
#2 -> #8 -> #71 -> #72 -> #9 -> #11 -> #12 -> #58 -> #10 -> #13 -> #20 -> #26 -> #44 -> #45 -> #46 -> #48 -> #49 -> #68 -> #69
```

<!-- END GENERATED: MAXIMUM-LENGTH SPINE -->

Twenty-one unique Atoms across three maximum-length paths. The paths branch at #13 and rejoin at
#44 before reaching the self-hosting and qualification tail. Everything not drawn is off the
longest path and can therefore be scheduled concurrently when its own prerequisites permit it —
which is the only claim a spine is entitled to make.

The spine deliberately does **not** show the off-path evidence and source work (#14-#18, #29-#31,
#33), persistence work #25, #27 and #28, or the experiments. They are prerequisites of Atoms in
the spine but sit on shorter paths, so drawing them here would state a false ordering. Their order
is in the phase sections below.

## 5. Qualification before dependence

### Kernel-start gate

The Atoms #9 declares as hard dependencies — the whole gate, narrowed by D1 — are exactly:

```text
#2  #3  #4  #8  #71  #72
```

**The native `blocked by` list of #9 is the definition of this gate.** This list is a mirror of
it, the `gate:foundation` label marks the same six issues, and the Project 9 "Foundation Gate"
view filters on that label. No wave range, and no other prose list, defines the gate (G-406).

- #2 stabilize Rust, Lean, Python, and CI;
- #3 deterministic workload generators;
- #4 simple reference algorithms;
- #8 research-graph coverage and epistemic audit;
- #71 comprehensive research record schema;
- #72 ontology, closure, and repository-coverage enforcement.

### Concurrent with the kernel

These do **not** gate #9. D1 re-attached each where it is actually consumed, so the kernel is not
held behind qualification work that no kernel Atom reads:

- #1 qualify and pin Jujutsu — consumed by #29 and #33;
- #5 benchmark and regression discipline — consumed directly by #10, #14, #24 and experiment work;
- #6 verification-technique pilot on #4 — consumed directly by #7, #18, #60 and #62;
- #7 Lean/Rust differential conformance — consumed directly by the kernel, evidence, scheduling,
  persistence and formal-method work that names it;
- #73 epistemic traversal and impact queries — consumed by #68;
- #74 reproducible acquisition and staleness propagation — consumed by #68;
- #75 experiment run ledger and statistical contract — consumed by #34, #37, #39 and the
  experiment Atoms #50-#54 and #59-#61.

### The D1 split of #37 uses two real issue numbers

`#37a` is not a representable node: GitHub issue numbers are integers, so `#37a` can never exist
in the native blocked-by graph that D3 makes authoritative, and
`scripts/check-selfhosting-closure.sh` — which recomputes closures from the live graph — would
never see it. The split is therefore:

```text
#37   retained as the foundation subset:  subprocess runner, seed matrix, raw artifact capture
#77   new issue, the worker-launch extension:  launching and supervising experiment workers
```

The complete current native edges are `#37 blocked_by #2, #3, #75`, `#77 blocked_by #2, #5, #35,
#36, #37`, and `#39 blocked_by #3, #37, #38, #75, #77`. Every reference that read `#37a` now
reads `#37`; every reference to "the later worker-launch extension" now reads `#77`.

### Parallelism

After #2:

```text
#1 Jujutsu qualification
#3 workload generators
#6 verification-technique pilot
#8/#71 knowledge schema acquisition
```

may proceed concurrently because they touch separate development substrates. Interfaces must be
published early, at these paths:

```text
workload format from #3                        -> orchestration/ generator module
reference algorithm contract from #4           -> crates/gordian-core reference oracles
proof/conformance test-vector format from #7   -> docs/formal/conformance-vectors.md
knowledge schema from #71                      -> knowledge/ontology.md
experiment manifest schema from #75            -> experiments/schema/
closure record schema                          -> artifacts/schema/closure-record.schema.json
source adapter contract from #29               -> docs/protocols/source-adapter-contract.md
crate layout                                   -> docs/implementation/crate-map.md
statistical contract                           -> docs/testing/statistical-contract.md
```

### Selecting among ready Atoms

The ready set usually has more than one member. The selection order is total and is stated here
once (G-530); [`agent-runbook.md`](agent-runbook.md) section 4 points at it and does not restate
it:

1. lowest `Wave` (longest-path depth over the native blocked-by graph);
2. then highest `Fan Out` (out-degree — unblocking the most work first);
3. then lowest issue number.

**At most three bootstrap Atoms may be claimed simultaneously.** The cap exists because
integration and admission are not yet implemented, so concurrent candidates are reconciled by
hand. It is lifted when #38 closes.

### Exit gate

Do not treat the foundation as complete until:

- Rust CI is green (#2);
- Lean build, the independent checker, and the axiom audit are green, with `sorryAx` and
  non-allowlisted axioms rejected by the axiom audit (#2);
- Jujutsu disposable-repository contract tests pass on the pinned release (#1);
- benchmark workload generators are seed-reproducible (#3);
- reference algorithms have stated complexity and correctness properties (#4);
- benchmark and regression gates reject a seeded regression (#5);
- knowledge-graph validation and `audit --strict` pass (#8, #72);
- the research record schema represents source revisions and retrieval dates (#71);
- experiment run manifests preserve failures and exclusions as well as successes (#37, #75);
- the Lean/Rust conformance harness catches intentionally injected disagreement (#7), using the
  vector format of [`../formal/conformance-vectors.md`](../formal/conformance-vectors.md);
- the verification-technique pilot reports availability, cost, and defect yield per tool (#6).

## 6. Typed kernel of the Mission Graph

### Members

- #9 typed identities and immutable specification revisions;
- #11 attempts, candidates, and effect classes;
- #12 canonical events and deterministic projection;
- #58 Project resource registry and external identities;
- #10 decomposition/dependency validation;
- #13 acceptance predicates and derived state;
- #55 PlanRevision lifecycle;
- #56 planner proposal/validation interface;
- #57 desired-versus-observed reconciliation and repair planning.

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

## 7. Exact evidence, provenance, and authority

### Members

- #14 content-addressed artifacts;
- #15 exact evidence fingerprints;
- #16 verifier manifests/exact-subject execution;
- #17 provenance and attestations;
- #18 capability policy and Cedar evaluation;
- #19 accepted-frontier admission/CAS.

#14 and #18 can begin in parallel after the identity model stabilizes. #19 cannot precede all evidence and authority semantics it is meant to enforce. The stale-evidence and formal-method experiments that consume this phase (#59, #60) are listed once, in the decision matrix of section 16.

### Safety gate

The normative admission predicate is the algorithm in
[`../algorithms/evidence-and-admission.md#the-algorithm`](../algorithms/evidence-and-admission.md#the-algorithm),
whose ten conjuncts are named in
[`../spec/mission-graph.md`](../spec/mission-graph.md#accepted-frontier) and defined in
[`../algorithms/evidence-and-admission.md#the-admission-conjuncts-defined`](../algorithms/evidence-and-admission.md#the-admission-conjuncts-defined).
This document does not carry a second conjunct list; a phase gate that restated it would be a
fourth divergent copy, which is what this revision removes.

The gate for this phase is that admission is implemented against exactly that predicate, and that
no model assertion, worker status, issue closure, or green UI projection can bypass it.

### Mutation gate

Use mutation testing to remove or invert each admission check. The suite must fail. A check that can be deleted without detection is not protected.

## 8. Scheduling and semantic coordination

### Members

- #20 ready queue and critical-path analysis;
- #21 worker capability/resource compatibility;
- #22 semantic claims and observed scope;
- #23 leases and fencing;
- #24 scheduler policy comparison;
- #52 semantic conflict predictor experiment.

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

### Members

- #25 PostgreSQL canonical persistence;
- #26 materialized projections/rebuild;
- #27 transactional frontier, lease, and plan-selection transitions;
- #28 crash/duplicate/recovery fault suite.

Backup, restore, and migration qualification (#66) reads this phase's persistence but is listed
once, in section 14.

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

## 10. The source plane

The plane is adapter-neutral. [`../protocols/source-adapter-contract.md`](../protocols/source-adapter-contract.md)
is the trait; Jujutsu and Git are two realizations of it, which is what makes #34 a controlled
comparison rather than a rewrite.

### Members

- #29 bounded Rust adapter over the source-adapter trait, plus a disposable fixture repo;
- #30 workspace/change lifecycle;
- #31 candidate freeze/exact handoff;
- #32 sibling integration/conflict repair;
- #33 exact-revision verification;
- #76 Git worktree adapter behind the same trait;
- #34 Jujutsu versus Git experiment.

The pinned-baseline qualification this phase depends on (#1) is listed once, in section 5 under
"Concurrent with the kernel".

### Local development bootstrap

Acquisition and bootstrap are one block, in
[`agent-runbook.md`](agent-runbook.md) section 0. It ends with:

```bash
bash scripts/bootstrap-jj.sh --install
```

The script is the single source of the pinned Jujutsu baseline and configures `origin`, tracking,
and `trunk()`. It does not push or rewrite source, and it is not a behavioral contract test: that
suite is #1.

### Adapter gate

Do not spread shell commands across the runtime. Each adapter owns:

- structured argv/cwd/env;
- supported-version/feature checks;
- bounded machine-readable parsing;
- exact change/commit/workspace identities;
- errors and evidence artifacts;
- disposable fixtures.

### Experiment gate

#34 must be permitted to reject Jujutsu-specific complexity. Hold the Mission Graph, scheduler, workers, verification, and workloads constant while comparing the source adapter. That control is only credible because both adapters are driven through one trait and #76 exists: a comparison against a Git integration written for the occasion would vary the harness as well as the substrate.

## 11. Agent execution and thin Python orchestration

### Members

- #37 subprocess runner, seed matrix, and raw artifact capture — the D1 foundation subset;
- #35 worker protocol and capability envelope;
- #36 generic process/agent adapter;
- #62 sandbox backend qualification;
- #63 secret/credential brokerage;
- #77 the worker-launch extension split out of #37: launching and supervising experiment
   workers after #35 and #36, and blocking #39;
- #38 local multi-worker coordinator;
- #39 isolation/coordination ablation.

The snapshot-versus-rebase experiment (#53) consumes this phase and is listed once, in section 16.

### Rust/Python boundary

Rust:

```text
worker protocol
attempt/candidate state
scheduler
capabilities
sandbox policy
source adapter
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

## 12. Robustness across processes

### Members

- #40 remote transport/idempotent commands;
- #41 distributed lease/frontier coordination;
- #42 deterministic fault simulation;
- #43 OpenTelemetry-compatible observability.

Adversarial security and authority qualification (#67) depends on this phase and is listed once,
in section 14.

Remote execution is not required for the first useful Gordian. Do not add it until local self-hosting reveals a real need and the canonical state machine is stable enough to simulate.

### Exit gate

- duplicate/reordered/delayed messages preserve safety;
- uncertain outcomes do not trigger blind irreversible retries;
- stale fencing tokens fail;
- coordinator failover cannot create two accepted-frontier writers;
- simulation seeds reproduce failures;
- real integration tests qualify the gap between simulation and operating system/network behavior.

## 13. Interfaces and the temporary GitHub projection

### Members

- #44 CLI;
- #45 typed API/event stream;
- #46 GitHub import adapter;
- #47 Mission/evidence explorer;
- #70 reconcile issues into GitHub Project 9.

The CLI should precede the API/UI because it exercises domain commands with the least transport/UI surface.

GitHub issues and Project 9 are external planning projections. Their status is not native evidence and cannot establish Atom satisfaction.

## 14. Release, operations, and security qualification

### Members

- #64 immutable release/deployment records;
- #65 reproducible signed artifacts;
- #66 backup/restore/migration;
- #67 adversarial security qualification.

Lean sources/checkers and large experiment corpora remain development artifacts. Runtime distributions should not accidentally ship them.

## 15. Self-hosting and architecture retention

### Members

- #48 import Gordian's own plan;
- #49 execute a real bounded multi-worker Mission;
- #68 publish retain/revise/reject decisions, once the experiments of section 16 have run;
- #69 produce the end-to-end qualification evidence bundle.

### Minimal self-hosting prerequisite set

The Atoms that MUST be closed before #49 can execute are generated from the native dependency
graph. Identity coverage is computed over the actual registry keys, never an assumed contiguous
number range; closed duplicates may consume GitHub issue numbers without becoming Atoms.

<!-- BEGIN GENERATED: SELF-HOSTING CLOSURE -->

`closure(#49)` contains **43 Atoms**:

```text
1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20
21 22 23 24 25 26 27 28 29 30 31 32 33 35 36 38 44 45 46 48
58 71 72
```

Registered Atoms orphaned from `closure(#49)`, `closure(#68)`, and `closure(#69)`: none.

<!-- END GENERATED: SELF-HOSTING CLOSURE -->

`scripts/check-selfhosting-closure.sh` recomputes the closure and orphan set from
`artifacts/atoms/issues.json` and compares this generated block byte-for-byte. On failure the
generator output is the repair; no historical arithmetic or issue-number range is authoritative.

During bootstrap, [`agent-runbook.md`](agent-runbook.md) section 4 restricts dispatch to this set
until #49 closes, **plus #70**. The exception is deliberate: this set states what #49 requires,
not what an agent may claim, and #70 owns the readiness command, the claim subcommands, and the
board recompute that the bootstrap loop itself runs on. Excluding it from dispatch would make the
loop unable to build the commands it depends on. #1, #2 and #70 are the three Wave-0 Atoms in the
current graph, and the sanctioned selection order ranks them #2, #1, #70 by Fan Out. #48 imports
exactly these 43 Atoms; #70 is not among them and is closed or archived when #48 lands.

### Self-hosting acceptance

The bounded Mission must include:

- at least two causally independent Atoms;
- isolated source-adapter workspaces from exact bases;
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

Every row is one Hypothesis node in the knowledge graph with exactly one Experiment node testing
it, and this table is a projection of that node set — **G-436 is assigned to #68**, which owns the
regeneration and the check that this table, #68's blockers, and
[`../testing/falsification-plan.md`](../testing/falsification-plan.md) name the same experiments.

#68 must propagate negative results into specifications, docs, knowledge nodes, code removal, and follow-up Atoms.

## 17. Critical performance suite

Before qualification, the following must be benchmarked. Each obligation is a **row with an
identity**, an owner, and a statement of whether it is required for the first qualification, so
that #69's Performance acceptance can cite row ids instead of restating prose and so that an
obligation cannot be silently dropped (G-475).

Row ids have the form `EO17-<AREA>-<n>` and are stable. Renaming an obligation keeps its id;
retiring one removes the row and the citation in the owning issue in the same change.

**In first qualification** is `yes` when the measurement is required by #69's end-to-end
qualification evidence bundle. `no` rows carry a one-line reason and are deferred to a later
qualification; they still have an owner.

| Row | Obligation | Owner | In first qualification |
| --- | --- | --- | --- |
| `EO17-MG-1` | Mission Graph validation and topological order | #10 | yes |
| `EO17-MG-2` | ready-queue updates | #13 | yes |
| `EO17-MG-3` | critical-path calculation | #21 | yes |
| `EO17-MG-4` | full and incremental reconciliation | #57 | yes |
| `EO17-MG-5` | memory by graph shape | #12 | yes |
| `EO17-SCHED-1` | ranking and matching latency | #20 | yes |
| `EO17-SCHED-2` | heuristic regret on small exactly solved instances | #24 | yes |
| `EO17-SCHED-3` | makespan and critical-path efficiency | #24 | yes |
| `EO17-SCHED-4` | contention, conflict, and verifier/retry cost | #22 | yes |
| `EO17-SCHED-5` | robustness to bad estimates | #24 | yes |
| `EO17-EVID-1` | canonical fingerprint generation | #15 | yes |
| `EO17-EVID-2` | fresh and stale evidence lookup | #15 | yes |
| `EO17-EVID-3` | artifact put, get, and verify | #14 | yes |
| `EO17-EVID-4` | provenance closure | #17 | yes |
| `EO17-EVID-5` | admission latency and contention | #19 | yes |
| `EO17-PERSIST-1` | event append | #25 | yes |
| `EO17-PERSIST-2` | projection update and rebuild | #26 | yes |
| `EO17-PERSIST-3` | transactional CAS and lease contention | #27 | yes |
| `EO17-PERSIST-4` | backup, restore, and migration | #66 | no — release and operations work outside `closure(#69)`; measured with the release evidence. |
| `EO17-KG-1` | parse, merge, canonicalize, and audit | #8 | yes |
| `EO17-KG-2` | typed path, closure, and impact queries | #73 | no — epistemic traversal lands after self-hosting; measured with #68's retention report. |
| `EO17-KG-3` | source-revision and metadata scaling | #71 | yes |
| `EO17-KG-4` | file/petgraph versus candidate graph backends | #61 | no — backend ablation experiment; measured with #68's retention report. |
| `EO17-JJ-1` | workspace operations | #30 | yes |
| `EO17-JJ-2` | exact identity and revset queries | #31 | yes |
| `EO17-JJ-3` | `jj run` materialization and parallel verification | #33 | yes |
| `EO17-JJ-4` | integration, conflict, and recovery | #32 | yes |
| `EO17-JJ-5` | scaling by changes, workspaces, and repository topology | #1 | yes |
| `EO17-COORD-1` | spawn and dispatch overhead | #38 | yes |
| `EO17-COORD-2` | idle and loaded memory | #38 | yes |
| `EO17-COORD-3` | worker-count scaling | #38 | yes |
| `EO17-COORD-4` | event and telemetry volume | #43 | yes |
| `EO17-COORD-5` | end-to-end Mission work and span efficiency | #49 | yes |

### Benchmark obligation sections

Every issue that owns a row above carries a `## Benchmark obligation` section whose text contains,
literally, each `EO17-*` id it owns. That is what makes the table a contract rather than a wish:
the id is the join key between this document, the issue body, and #69's Performance acceptance.
#69's Performance acceptance cites the `EO17-*` ids of the in-first-qualification rows and
restates none of their prose, so the set it cites equals the set of `yes` rows above.

`scripts/check-benchmark-obligations.sh` is the checker that asserts (a) every `EO17-*` id occurs
exactly once in the table, (b) every owner of a `yes` row is in #69's transitive closure computed
from GitHub's `blockedBy` node lists, (c) every owner issue body contains a `## Benchmark
obligation` section naming each id it owns, and (d) the ids cited in #69 equal the `yes` set. The
checker reads the committed native snapshot, so a missing snapshot is an explicit failure rather
than a silently skipped check. The issue-body sections and live synchronization remain part of
**G-475, assigned to #70**, which already owns the issue-body and drift automation of
[`issue-index.md`](issue-index.md#adding-or-splitting-an-atom). Until it lands, the table above is
maintained by the "Adding or splitting an Atom" checklist and every omission is a drift defect.

## 18. Definition of end-to-end implementation

The definition of done is the normative Mission acceptance table in
[`project-plan.md`](project-plan.md#mission-acceptance), and the machine-checkable stop condition
is [`agent-runbook.md`](agent-runbook.md) section 3. This document does not restate them; two
divergent prose lists is what this revision removes.

Until every row of that table resolves to a validating closure record, Gordian is an increasingly
rigorous experiment, not a completed autonomous development system.
