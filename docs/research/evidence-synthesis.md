# Research Evidence Synthesis

Status: **living evidence ledger**

This document records the scientific and engineering basis for Gordian's current design. It separates established engineering principles, recent empirical evidence, architectural deductions, and Gordian-specific hypotheses.

The purpose is not to collect citations decoratively. Each source should constrain or challenge a design decision.

## 1. Evidence classes

Gordian uses four informal evidence classes:

- **Established foundation**: mature theory, specification, or engineering principle with broad independent use.
- **Empirical support**: experimental or observational evidence directly relevant to the design question.
- **Defeasible architectural deduction**: a design implication that follows reasonably from evidence but is not itself experimentally established.
- **Gordian hypothesis**: a project-specific claim that requires direct testing.

## 2. 2026 multi-agent software-engineering evidence

### 2.1 CAID: dependency-aware asynchronous isolated delegation

Source: Jiayi Geng and Graham Neubig, *Effective Strategies for Asynchronous Software Engineering Agents*, arXiv:2603.21489v2, 2026. Retrieved 2026-08-30.

https://arxiv.org/abs/2603.21489v2

The paper reports that Centralized Asynchronous Isolated Delegation (CAID) constructs dependency-aware task plans, executes subtasks concurrently in isolated workspaces, and integrates work with executable test-based verification.

The submission history holds exactly two versions, and the current one is v2:

```text
arXiv:2603.21489v1  23 Mar 2026  +26.7 PaperBench  +14.3 Commit0   superseded
arXiv:2603.21489v2   8 Jul 2026  +25.6 PaperBench  +14.7 Commit0   current, retrieved 2026-08-30
```

The cited improvements over single-agent baselines are therefore **25.6 percentage points on PaperBench** and **14.7 points on Commit0**. This ledger previously carried the v1 figures as current and described the v2 figures as an earlier research note, which reversed the drift; the correction is G-105. Version drift is recorded, not silently resolved in whichever direction is convenient — the pinned version and retrieval date above are what make the record checkable, and #71 adds the `SourceRevision` identity that lets the knowledge graph hold both rows rather than only the current one.

**Supported conclusion:** parallel software-agent work benefits from dependency-aware decomposition, isolated mutable state, structured integration, and executable verification on the studied workloads.

**Not established:** that every workload benefits from more agents; that Git branches are the optimal isolation primitive; or that the CAID architecture is globally optimal.

**Gordian implication:** scheduler parallelism should be derived from the Mission Graph rather than from an arbitrary agent count.

### 2.2 STORM: isolation without explicit state management is insufficient

Source: Mengyang Liu et al., *Multi-agent Collaboration with State Management*, arXiv:2605.20563, 2026.

https://arxiv.org/abs/2605.20563

STORM argues that isolated worktrees defer conflict discovery until post-hoc merge and instead mediates shared state so inconsistent views and conflicting edits can be detected earlier. The abstract reports improvements over a Git-worktree multi-agent baseline of **+18.7 on Commit0-Lite** and **+1.4 on PaperBench**.

**Supported conclusion:** workspace isolation alone is not sufficient coordination on the evaluated tasks; explicit shared state management can improve outcomes.

**Gordian implication:** isolated Jujutsu workspaces must be paired with a small semantic coordination plane exposing claims, interfaces, dependencies, leases, and events.

**Open question:** STORM's particular shared-state mechanism is not automatically the correct mechanism for Gordian.

### 2.3 AgentRoom: coordination appears more important than CRDT merge alone

Source: Seonglae Cho and Donghyun Lee, *AgentRoom: Concurrent Multi-Agent Coding in a CRDT-Backed Shared Workspace*, arXiv:2608.23740, 2026.

https://arxiv.org/abs/2608.23740

AgentRoom exposes file claims, status, and broadcast coordination through a CRDT-backed shared filesystem. The current abstract describes experiments across five coding CLI models and four backend tasks. It reports fewer abandoned tasks for two-agent AgentRoom than solo execution for CLI-stable models and says the bundle ablation ordering indicates that **coordination, not parallelism or CRDT merge, bears the load**.

**Evidence caution:** this is extremely recent and the task sample is small. It is directional evidence, not a settled result.

**Gordian implication:** the architecture should optimize explicit worker awareness and semantic coordination first. Shared textual convergence is not sufficient to guarantee semantic convergence.

### 2.4 AgenticFlict: integration conflict is common in agent-generated PRs

Source: Daniel Ogenrwot and John Businge, *AgenticFlict: A Large-Scale Dataset of Merge Conflicts in AI Coding Agent Pull Requests on GitHub*, arXiv:2604.03551, 2026.

https://arxiv.org/abs/2604.03551

The dataset contains 142K+ agentic pull requests from 59K+ repositories. More than 107K were successfully processed through deterministic merge simulation; 29K+ exhibited textual merge conflicts, giving a reported **27.67% conflict rate** and 336K+ fine-grained conflict regions.

**Supported conclusion:** post-hoc integration conflict is not a negligible edge case in current agent-generated GitHub contributions.

**Limit:** textual conflict rate does not measure semantic conflict, and the dataset is observational rather than a randomized comparison of coordination protocols.

**Gordian implication:** “let agents work independently and merge everything afterward” is not an adequate default architecture. Gordian should detect predicted overlap before execution and verify composition after integration.

### 2.5 CodeTeam: machine-checkable architecture contracts before implementation

Source: Yifei Wang et al., *CodeTeam: An LLM-Powered Multi-Agent Framework for Repository-Level Code Generation*, arXiv:2606.22082, 2026.

https://arxiv.org/abs/2606.22082

CodeTeam separates planning, design selection, implementation, and QA. Its CTO stage normalizes a design into a machine-checkable contract specifying file ownership, public interfaces, and dependency constraints, then allocates developer agents under a dependency-aware scheduler.

The reported benchmarks show improvements over corresponding CodeS variants and higher test pass rates on the authors' evaluated repository-generation settings.

**Supported conclusion:** explicit interface/ownership/dependency structure can be useful coordination input for concurrent repository-level generation.

**Gordian deduction:** replace crude file ownership with richer semantic resource claims when possible: public types, APIs, schemas, modules, configuration contracts, migration namespaces, and other architecture-visible resources.

### 2.6 Reliability synthesis: the agent is a system, not merely a model

Source: Stephanie Jarmak, *Engineering Reliable Coding Agents: Evaluating and Operating the System Around the Model*, arXiv:2608.13867, 2026.

https://arxiv.org/abs/2608.13867

This structured review argues that coding-agent reliability depends on the full harness: execution state, retrieval, memory/state management, permissions, review interfaces, resource allocation, verification, and recovery. It synthesizes scholarly, practitioner, benchmark, and operated-system records.

**Supported conclusion:** model capability alone is an insufficient reliability boundary.

**Gordian implication:** correctness and promotion authority belong to the substrate. Autonomous workers are replaceable probabilistic components inside capability and verification boundaries.

## 3. Hierarchical planning foundations

Source: Ilche Georgievski and Marco Aiello, *HTN planning: Overview, comparison, and beyond*, Artificial Intelligence 222, 2015.

https://doi.org/10.1016/j.artint.2015.02.002

Hierarchical Task Network planning represents compound tasks that can be decomposed into primitive tasks and supports task networks with ordering constraints. Partially ordered decomposition allows tasks to remain unordered/parallel when constraints permit.

**Established foundation:** hierarchy is a useful representation for decomposition, while execution ordering may be a partial order rather than the hierarchy itself.

**Important caution from the literature:** HTN performance and correctness depend heavily on domain/decomposition knowledge.

**Gordian consequence:** `Mission -> PlanRevision -> Initiative -> Atom -> Quark` is a decomposition vocabulary, not execution truth. PlanRevision exists explicitly so decomposition strategy can be replaced without redefining Mission identity.

## 4. Contract reasoning

Source: C. A. R. Hoare, *An Axiomatic Basis for Computer Programming*, Communications of the ACM, 1969.

https://doi.org/10.1145/363235.363259

Hoare logic formalizes computation with preconditions and postconditions.

**Established foundation:** explicit conditions around transformations make assumptions and obligations inspectable.

**Gordian adaptation:** an Atom is modeled as a bounded contract with preconditions and acceptance/postcondition predicates.

**Limit:** ordinary software-engineering Atoms are not automatically formal proofs. Their predicates may be experimental, test-based, benchmark-based, static-analysis-based, human-attested, or mixed.

## 5. Dependency and hermeticity foundations

Source: Bazel documentation on dependencies and hermeticity.

https://bazel.build/concepts/dependencies

https://bazel.build/basics/hermeticity

Build systems distinguish declared dependencies from the resources actually required by an action. Hermetic execution tries to make outputs depend only on declared inputs and controlled environments.

**Established engineering principle:** undeclared dependencies undermine reproducibility and safe caching; over-declaration reduces precision and concurrency.

**Gordian adaptation:** compare semantic/resources declared before execution with resources observed during execution:

```text
D_observed subset-of D_declared
```

A violation is evidence of hidden dependency or scope expansion.

**Limit:** observation cannot prove completeness; instrumentation can miss semantic dependencies.

## 6. Durable execution and deterministic replay

Source: Temporal documentation on workflow/event history and durable execution.

https://docs.temporal.io/workflow-execution/event

https://docs.temporal.io/evaluate/understanding-temporal

Temporal separates deterministic workflow orchestration from Activities that interact with nondeterministic external systems. Activity results are recorded in durable event history so workflow replay does not reproduce external side effects merely to reconstruct state.

**Established engineering pattern:** deterministic state reconstruction is easier when nondeterministic/effectful work is separated and recorded.

**Gordian adaptation:** the coordinator/event projector should be deterministic where practical. LLM calls, network calls, external writes, human judgments, and other effects become ExecutionAttempts and recorded results rather than hidden state transitions.

## 7. Provenance foundations

Source: W3C PROV Data Model.

https://www.w3.org/TR/prov-dm/

PROV provides domain-independent concepts including **Entity**, **Activity**, and **Agent**, with relations such as `used`, `wasGeneratedBy`, `wasDerivedFrom`, `wasAssociatedWith`, and `wasAttributedTo`.

**Established standard:** provenance should identify things, activities, responsible actors, and derivation relationships separately.

**Gordian adaptation:**

```text
Artifact / SpecRevision / Evidence -> Entity
ExecutionAttempt                  -> Activity
Human or autonomous worker       -> Agent
```

Gordian should support a PROV/JSON-LD projection without requiring RDF as the internal database model.

## 8. Attestation and supply-chain evidence

### 8.1 in-toto

Source: in-toto specification and documentation.

https://github.com/in-toto/docs/blob/master/in-toto-spec.md

https://in-toto.io/docs/getting-started/

in-toto models authorized functionaries executing defined steps and producing signed link metadata containing materials, products, byproducts, command/environment information, and identity.

**Established engineering pattern:** evidence about an execution step should be bound to exact inputs, outputs, actor identity, and execution context.

### 8.2 SLSA provenance

Source: SLSA Build Provenance specification.

https://slsa.dev/spec/v1.2-rc2/build-provenance

SLSA distinguishes build definition from run details and records resolved dependencies and output subjects.

**Gordian adaptation:** Evidence and Attestation records should separate the reusable verifier/build definition from a particular ExecutionAttempt and bind results to exact subjects and resolved dependencies.

## 9. Jujutsu engineering basis

Primary documentation:

https://docs.jj-vcs.dev/latest/glossary/

https://docs.jj-vcs.dev/latest/working-copy/

https://docs.jj-vcs.dev/latest/changelog/

https://docs.jj-vcs.dev/latest/revsets/

https://docs.jj-vcs.dev/latest/technical/architecture/

### 9.1 Change identity versus commit identity

Jujutsu documentation states that rewriting a commit produces a new commit ID while generally preserving the change ID.

**Gordian consequence:** logical work can bind to a change ID, while verification MUST bind to the exact commit ID.

### 9.2 Workspaces

Jujutsu supports multiple working copies backed by one repository, each with its own working-copy commit.

**Gordian consequence:** one isolated workspace per active worker is a natural execution mapping.

### 9.3 `jj run`

Jujutsu introduced `jj run`, which can run commands over a set of changes with private working copies and parallel execution.

**Gordian consequence:** revision-scoped verification can be built around a native Jujutsu primitive instead of only around push-triggered CI.

### 9.4 Operation log

Jujutsu's architecture stores repository operations in a separate operation DAG and can reconcile concurrent repository views.

**Gordian consequence:** Jujutsu already contains useful transactional/recovery semantics, but its operation log is not enough to represent Mission Graph semantics, worker authorization, or evidence.

### 9.5 Multi-parent and conflict states

Jujutsu supports merge commits, including octopus merges, and represents conflicts as repository state rather than requiring an immediate destructive resolution step.

**Gordian consequence:** integration may be an explicit candidate state and conflicts may become structured resolution work. Conflict-bearing states remain forbidden at acceptance.

## 10. Architectural synthesis

The evidence does not point toward “one branch per agent” as the fundamental abstraction.

The strongest synthesis is:

> **isolated code state + coordinated semantic state + dependency-aware scheduling + exact-artifact verification + capability-gated promotion**

Gordian therefore resembles a combination of:

```text
hierarchical/partial-order planner
+ typed work graph
+ optimistic-concurrency scheduler
+ Jujutsu change DAG
+ durable event history
+ provenance/evidence graph
+ capability-based control plane
```

This is a synthesis of established ideas. The exact composition is not itself established science.

## 11. Critical deductions

### 11.1 Project graph is not commit graph

The Mission Graph describes intent and semantic causality. The Jujutsu graph records code-state causality. Coupling them one-to-one would erase useful distinctions.

### 11.2 Textual convergence is not semantic convergence

Two workers can touch different files yet disagree on a public type or schema. Conversely, two edits in one file may be semantically independent. File conflicts are therefore only one observable proxy for semantic conflict.

### 11.3 Individual verification is not compositional verification

If candidates `A`, `B`, and `C` each pass independently, their combined state is not thereby proven correct. Integration must be its own exact candidate with its own evidence.

### 11.4 Stable goals require revisable plans

Mission identity must survive failed or superseded decomposition strategies. Otherwise historical comparison and automated replanning become semantically confused.

### 11.5 Completion must be evidence-relative

A status label such as `done` is an assertion. Gordian should instead derive satisfaction from acceptance predicates over fresh evidence wherever feasible.

### 11.6 Worker authority must be narrower than coordinator authority

Agent reliability evidence emphasizes harness boundaries and permissions. An implementation worker should be able to invalidate its own candidate, not mutate canonical accepted reality.

## 12. Gordian hypotheses and proposed falsification

### H1: Mission Graph decomposition improves long-horizon coordination

**Hypothesis:** explicit Mission/Initiative/Atom decomposition produces better completion/integration outcomes than flat issue/task lists for long-horizon agent work.

**Experiment:** matched repositories/tasks using flat work items versus Mission Graph plans, measuring final test success, abandoned work, integration repairs, and planning overhead.

### H2: semantic claims outperform file ownership as a scheduler signal

**Hypothesis:** semantic read/write/interface claims predict harmful concurrency better than file-overlap prediction alone.

**Experiment:** log declared claims, actual touched files/symbols/interfaces, merge conflicts, build failures, API incompatibilities, and integration repair cost. Compare predictors.

### H3: immutable worker snapshots reduce reasoning instability

**Hypothesis:** snapshot-isolated workers with reconciliation at handoff outperform continuously rebased workers on long-running tasks.

**Experiment:** control base drift across otherwise matched multi-agent runs.

### H4: exact evidence invalidation prevents false satisfaction

**Hypothesis:** binding evidence to specification, candidate, dependency, and environment fingerprints detects stale-success states that ordinary CI status models retain incorrectly.

**Experiment:** systematically mutate each fingerprint dimension after successful verification and measure false reuse prevented versus redundant reruns introduced.

### H5: explicit semantic coordination adds value beyond isolation

**Hypothesis:** isolated Jujutsu workspaces plus claims/events outperform isolated workspaces alone.

**Experiment:** reproduce the conceptual CAID/STORM contrast inside Gordian under controlled tasks.

### H6: Jujutsu reduces orchestration complexity

**Hypothesis:** persistent change identity, first-class workspaces/conflicts, multi-parent changes, revsets, and operation history reduce coordination code or failure recovery compared with a Git worktree/branch implementation.

**Experiment:** implement equivalent minimal protocols on Git and Jujutsu and compare orchestration LOC, state transitions, recovery cases, integration failures, and operator interventions.

### H7: Lean-checked substrate invariants add practical defect detection

**Hypothesis:** formalizing a small set of scheduler/evidence/authority invariants catches defects that property tests and runtime assertions miss at acceptable development cost.

**Experiment:** mutation-test the reference implementation and compare detection by Lean proof obligations, Rust type/system invariants, property tests, and integration tests.

## 13. Research posture

Gordian should avoid two opposite failure modes:

1. **cargo-culting the status quo**: reproducing branches, tickets, Kanban states, PR queues, and CI gates merely because they are familiar;
2. **novelty theater**: renaming familiar concepts without demonstrating measurable improvement.

Every nontrivial abstraction should eventually answer:

```text
What failure mode does this prevent?
What observable quantity should improve?
What simpler alternative are we comparing against?
What result would cause us to remove or redesign it?
```

That is the standard by which Mission Graph itself should be judged.
