# Gordian End-to-End Project Plan

This plan is the implementation decomposition for Gordian itself. GitHub issues temporarily represent **Atoms** while Gordian builds the Mission Graph substrate intended to supersede that workflow.

The plan is ordered by causal dependency and evidence needs, not by feature glamour.

**This document is a view.** The executable Atom contracts are the GitHub issue bodies; the
authority for dependencies is the native GitHub `blocked by` graph; the authority for Initiative
membership is the milestone. The only normative content here is the Mission acceptance table
below. The Atom tables reproduce issue numbers, titles, target crates, and blockers so that a
reader can see the whole plan at once, and they are re-derived whenever an Atom or an edge
changes ([`issue-index.md`](issue-index.md#adding-or-splitting-an-atom)). The generator that will
emit them from the live issue list, and the CI job that regenerates and diffs, are **G-413 and
G-445, assigned to #70**.

## Project Mission

Build a Rust-first coordination substrate that can represent engineering intent as a Mission Graph, execute work through isolated source states, coordinate human/agent workers, record deterministic execution/provenance, verify exact candidates, and admit only evidence-supported state through explicit authority.

Python remains a thin orchestration and experimentation layer. Lean and all formal dependencies remain development-only under `formal/`.

## Mission acceptance

This list is **normative**. [`execution-order.md`](execution-order.md) section 18 links here and
states no second list. Previously project-plan.md carried 18 items, execution-order.md section 18
carried 13 different ones, neither referenced the other, and project-plan.md contained no issue
reference at all, so no item could be traced to work.

The Mission is satisfied when a clean installation can demonstrate the following end to end, and
each row's Atoms have validating closure records
([`../../artifacts/schema/closure-record.schema.json`](../../artifacts/schema/closure-record.schema.json)):

| # | Acceptance item | Atoms |
| --- | --- | --- |
| 1 | define a Project/Mission/PlanRevision/Initiative/Atom graph through a typed interface | #9, #55, #58, #44, #45 |
| 2 | reject structurally invalid decomposition/dependency state | #10 |
| 3 | persist canonical work/events and rebuild derived state deterministically | #12, #25, #26 |
| 4 | derive ready/blocked work without mutable workflow-status truth | #13 |
| 5 | schedule compatible Atoms across one or more heterogeneous workers | #20, #21, #24 |
| 6 | create isolated workspaces from exact base states | #29, #30 |
| 7 | associate evolving implementations with logical change ids and frozen candidates with exact state ids | #31 |
| 8 | coordinate declared/observed semantic resource claims and leases | #22, #23 |
| 9 | run verifiers against exact candidates and store provenance-bound evidence | #16, #17, #33 |
| 10 | invalidate stale evidence when relevant identity changes | #15 |
| 11 | integrate independent candidates explicitly and re-verify composition | #32 |
| 12 | prevent Worker authority from moving accepted or deployed frontiers | #18 |
| 13 | promote an accepted frontier with race-safe compare-and-swap semantics | #19, #27 |
| 14 | replay after process failure without repeating nondeterministic effects | #11, #26, #28 |
| 15 | expose CLI/API surfaces usable by humans and agent harnesses | #44, #45, #46 |
| 16 | run the project's own Atom workflow through Gordian as a self-hosting proof | #48, #49 |
| 17 | publish benchmark and falsification evidence for the major Gordian-specific hypotheses | #34, #37, #39, #50, #51, #52, #53, #54, #59, #60, #61, #68, #75, #76, #77 |
| 18 | keep the research knowledge graph synchronized with implemented/falsified concepts | #8, #71, #72, #73, #74 |

### Waived human-judgment metrics

A metric only a person can produce cannot gate an unattended Mission. Each line below waives one
**metric**, never a whole row, in the form `unresolved_human_metric: <atom> — <metric> — <why no
machine substitute exists>`. `scripts/check-mission-stop-condition.sh` parses these lines, and
[`agent-runbook.md`](agent-runbook.md) section 3 defines the rule.

```text
unresolved_human_metric: #50 — operator-comprehension rating — the ontology ablation scores how
  quickly a person understands a Mission's decomposition; no machine proxy measures comprehension,
  and substituting one would answer a different question than the hypothesis asks.
unresolved_human_metric: #54 — operator/manual correction count — counts corrections a human chose
  to make against a mutable-status baseline; the count is defined by human intervention, so it
  cannot be synthesized.
```

A waived metric does not waive its Atom: #50 and #54 still need closure records for every
machine-measurable part of their contract, and the waiver only stops the stop condition waiting on
the human number. Every waiver here MUST be reproduced verbatim in #69's evidence bundle under
*known limitations*, so a qualification that skipped human judgement can never be mistaken for one
that obtained it. Waiving a metric a machine could produce is a contract defect.

`scripts/check-mission-acceptance.sh` asserts that every issue number in the Atoms column exists,
and `scripts/check-mission-stop-condition.sh` asserts — for the stop condition — that each
referenced Atom has a validating closure record, printing the unsatisfied rows otherwise. The stop
condition itself is one sentence, in [`agent-runbook.md`](agent-runbook.md) section 3.

## Engineering constraints

- Production semantics are **planned** to be Rust; today the workspace contains one crate,
  `gordian-kg`. [`crate-map.md`](crate-map.md) decides where each Atom's code goes and what it may
  depend on.
- Python is orchestration/analysis only.
- Lean is development-only under `formal/`.
- Safe Rust is the default; `unsafe` requires isolated proof/test/performance justification.
- Reference algorithms remain available where useful as semantic/performance oracles.
- Every performance-sensitive algorithm gets complexity analysis plus representative/adversarial benchmarks.
- Every formal claim states assumptions and empirical boundary.
- Every agent-facing source mutation operates on an exact base.
- Every verified source result identifies an exact state id.
- No permanent `develop` bookmark exists.
- Distribution is deferred until single-node semantics survive model/property/fault testing.

## Initiatives and Atoms

Fourteen Initiatives, 77 Atoms. Each Initiative is a GitHub milestone and each Atom belongs to
exactly one. `Blocked by` reproduces the native `blocked by` edges after the D3 revision; `Target
crate` is the row that Atom owns in [`crate-map.md`](crate-map.md), and `—` means the Atom writes
no crate code.

# Initiative: Foundation and Falsification

| Atom | Title | Target crate | Blocked by |
| --- | --- | --- | --- |
| #1 | [Foundation] Qualify and pin the Jujutsu development baseline | — | — |
| #2 | [Foundation] Stabilize Rust, formal, Python, and CI foundations | — | #1 |
| #3 | [Foundation] Build deterministic benchmark and workload generators | — | #2 |
| #4 | [Foundation] Establish simple reference algorithm baselines | — | #2, #3 |
| #5 | [Foundation] Establish performance benchmark and regression gates | — | #3, #4 |
| #6 | [Foundation] Qualify the verification technique stack | — | #2 |
| #7 | [Foundation] Build Lean and Rust differential conformance testing | — | #2, #6 |
| #8 | [Foundation] Complete research-graph coverage and epistemic audit | `gordian-kg` | #2 |
| #50 | [Experiment] Run Mission Graph ontology ablation | — | #9, #10, #13, #37 |
| #51 | [Experiment] Run Atom and Quark scheduling-granularity ablation | — | #10, #20, #24, #37 |
| #52 | [Experiment] Benchmark semantic conflict prediction against path and module baselines | — | #22, #32, #37 |
| #53 | [Experiment] Compare stable snapshot execution with continuous active rebasing | — | #30, #32, #37, #38 |
| #54 | [Experiment] Compare derived state with mutable workflow status | — | #13, #26, #37 |
| #59 | [Experiment] Measure stale-evidence prevention and invalidation cost | — | #3, #15, #16, #31, #37 |
| #60 | [Experiment] Measure formal-method defect yield and maintenance cost | — | #6, #7, #10, #13, #15, #18, #19, #23, #27 |
| #61 | [Research Corpus] Benchmark and qualify the knowledge-graph storage/query architecture | — | #3, #5, #8 |
| #75 | [Foundation] Implement reproducible run ledger and statistical analysis contract | `gordian-experiments` | #3, #5, #71 |

# Initiative: Research Knowledge Substrate

| Atom | Title | Target crate | Blocked by |
| --- | --- | --- | --- |
| #71 | [Knowledge] Implement comprehensive research record schema and source revision identity | `gordian-kg` | #8 |
| #72 | [Knowledge] Enforce ontology, epistemic closure, and repository coverage | `gordian-kg` | #8, #71 |
| #73 | [Knowledge] Implement epistemic traversal, contradiction, and downstream-impact queries | `gordian-kg` | #61, #71, #72 |
| #74 | [Knowledge] Implement reproducible acquisition, source refresh, and staleness propagation | `gordian-kg` | #37, #71, #72 |

# Initiative: Rust Mission Graph Kernel

| Atom | Title | Target crate | Blocked by |
| --- | --- | --- | --- |
| #9 | [Mission Graph Kernel] Implement typed identities and immutable specification revisions | `gordian-core` | #2, #3, #4, #8, #71, #72 |
| #10 | [Mission Graph Kernel] Implement decomposition and hard-dependency validation | `gordian-core` | #4, #9, #58 |
| #11 | [Mission Graph Kernel] Implement ExecutionAttempt, Candidate, and effect semantics | `gordian-core` | #9 |
| #12 | [Mission Graph Kernel] Implement canonical events and deterministic projection | `gordian-core` | #4, #9, #11 |
| #13 | [Mission Graph Kernel] Implement acceptance predicates and derived work state | `gordian-core` | #7, #10, #11, #12 |
| #58 | [Mission Graph Kernel] Implement Project resource registry and external identity model | `gordian-core` | #9, #12 |

# Initiative: Planning and Reconciliation

| Atom | Title | Target crate | Blocked by |
| --- | --- | --- | --- |
| #55 | [Planning & Reconciliation] Implement PlanRevision proposal, comparison, and selection lifecycle | `gordian-core` | #9, #12, #13, #27 |
| #56 | [Planning & Reconciliation] Implement planner proposal interface with validated decomposition ingestion | `gordian-core` | #10, #18, #20, #35, #55 |
| #57 | [Planning & Reconciliation] Implement desired-versus-observed reconciliation and repair planning | — | #12, #13, #15, #26, #55, #56 |

# Initiative: Evidence, Provenance, and Authority

| Atom | Title | Target crate | Blocked by |
| --- | --- | --- | --- |
| #14 | [Evidence & Authority] Implement content-addressed artifact storage | `gordian-artifacts` | #2, #4 |
| #15 | [Evidence & Authority] Implement exact evidence fingerprints and freshness | `gordian-evidence` | #7, #11, #14 |
| #16 | [Evidence & Authority] Implement verifier manifests and exact-subject execution | `gordian-evidence` | #14, #15 |
| #17 | [Evidence & Authority] Implement provenance and attestations | `gordian-evidence` | #12, #14, #15, #16 |
| #18 | [Evidence & Authority] Implement capability policy and evaluate Cedar | `gordian-coordination` | #6, #9 |
| #19 | [Evidence & Authority] Implement candidate admission and accepted-frontier CAS | `gordian-coordination` | #13, #15, #16, #17, #18 |

# Initiative: Scheduling and Coordination

| Atom | Title | Target crate | Blocked by |
| --- | --- | --- | --- |
| #20 | [Scheduling & Coordination] Implement dependency-aware ready queue and critical-path analysis | `gordian-scheduler` | #3, #4, #10, #13 |
| #21 | [Scheduling & Coordination] Implement worker capability and resource compatibility | `gordian-scheduler` | #18, #20 |
| #22 | [Scheduling & Coordination] Implement semantic resource claims and scope observation | `gordian-scheduler` | #9, #11, #12 |
| #23 | [Scheduling & Coordination] Implement lease and fencing arbitration | `gordian-coordination` | #18, #22 |
| #24 | [Scheduling & Coordination] Implement and benchmark heterogeneous scheduling policies | `gordian-scheduler` | #3, #5, #20, #21, #23 |

# Initiative: Durable Persistence and Replay

| Atom | Title | Target crate | Blocked by |
| --- | --- | --- | --- |
| #25 | [Persistence & Replay] Implement PostgreSQL canonical persistence | `gordian-postgres` | #9, #10, #11, #12, #15, #18 |
| #26 | [Persistence & Replay] Implement materialized projections and deterministic rebuild | `gordian-postgres` | #12, #25 |
| #27 | [Persistence & Replay] Implement transactional frontier, lease, and plan-selection transitions | `gordian-postgres` | #19, #23, #25 |
| #28 | [Persistence & Replay] Build crash, duplicate-event, and recovery fault suite | — | #26, #27 |

# Initiative: Jujutsu Change Plane

| Atom | Title | Target crate | Blocked by |
| --- | --- | --- | --- |
| #29 | [Jujutsu Change Plane] Implement low-level Jujutsu command adapter and fixture repository | `gordian-source` (trait), `gordian-jj` | #1, #2 |
| #30 | [Jujutsu Change Plane] Implement workspace and change lifecycle | `gordian-jj` | #11, #29 |
| #31 | [Jujutsu Change Plane] Implement candidate freeze and exact commit handoff | `gordian-jj` | #15, #30 |
| #32 | [Jujutsu Change Plane] Implement sibling integration and conflict repair workflow | `gordian-jj` | #19, #31 |
| #33 | [Jujutsu Change Plane] Implement exact-revision verification with Jujutsu | `gordian-jj` | #1, #16, #29, #31 |
| #34 | [Jujutsu Change Plane] Run the Jujutsu versus Git source-substrate experiment | `gordian-source` | #3, #5, #29, #30, #31, #32, #33, #76 |
| #76 | [Source Plane] Implement the Git worktree source adapter | `gordian-git` | #29 |

# Initiative: Agent Execution and Thin Python Orchestration

| Atom | Title | Target crate | Blocked by |
| --- | --- | --- | --- |
| #35 | [Agent Execution] Define worker protocol and sandbox capability envelope | `gordian-runtime` | #18, #23, #30 |
| #36 | [Agent Execution] Implement generic process and agent worker adapter | `gordian-runtime` | #30, #35 |
| #37 | [Agent Execution] Implement thin Python experiment orchestration | `gordian-experiments` | #2, #5, #75 |
| #38 | [Agent Execution] Implement local multi-worker coordinator | `gordian-runtime` | #24, #27, #32, #33, #36 |
| #39 | [Agent Execution] Run isolation and coordination ablation | — | #3, #37, #38, #77 |
| #62 | [Agent Execution] Implement and qualify sandbox backends for untrusted workers | — | #6, #18, #35, #36 |
| #63 | [Agent Execution] Implement capability-scoped secret and credential brokerage | — | #18, #35, #58, #62 |
| #77 | [Agent Execution] Launch and supervise experiment workers | `gordian-experiments` | #37 |

# Initiative: Distributed Robustness

| Atom | Title | Target crate | Blocked by |
| --- | --- | --- | --- |
| #40 | [Distributed Robustness] Define remote worker transport and idempotent command protocol | — | #12, #27, #35, #38 |
| #41 | [Distributed Robustness] Implement distributed lease and accepted-frontier coordination | `gordian-coordination` | #23, #27, #40 |
| #42 | [Distributed Robustness] Build deterministic distributed fault simulation | — | #40, #41 |
| #43 | [Distributed Robustness] Instrument Gordian with OpenTelemetry-compatible observability | — | #38 |

# Initiative: Human and Programmatic Interface

| Atom | Title | Target crate | Blocked by |
| --- | --- | --- | --- |
| #44 | [Interfaces] Implement the Gordian CLI | `gordian-runtime` | #20, #25, #26 |
| #45 | [Interfaces] Implement a headless typed API and event stream | `gordian-runtime` | #18, #25, #44 |
| #46 | [Interfaces] Implement GitHub bootstrap and import adapter | `gordian-runtime` | #25, #45 |
| #47 | [Interfaces] Build Mission Graph and evidence explorer | — | #45 |

# Initiative: Temporary GitHub Bootstrap

| Atom | Title | Target crate | Blocked by |
| --- | --- | --- | --- |
| #70 | [Bootstrap] Reconcile repository Atoms into GitHub Project 9 | — | — |

# Initiative: Release, Operations, and Acceptance

| Atom | Title | Target crate | Blocked by |
| --- | --- | --- | --- |
| #64 | [Release & Deployment] Implement immutable release and deployment records | — | #17, #19, #25, #27, #58 |
| #65 | [Release & Deployment] Build reproducible signed distribution artifacts | — | #2, #14, #17, #44, #45, #64 |
| #66 | [Release & Deployment] Implement backup, restore, migration, and compatibility qualification | — | #14, #25, #26, #64, #65 |
| #67 | [Release & Deployment] Perform adversarial security and authority qualification | — | #18, #19, #23, #27, #35, #40, #62, #63, #65 |

# Initiative: Self-Hosting and Acceptance

| Atom | Title | Target crate | Blocked by |
| --- | --- | --- | --- |
| #48 | [Self-Hosting] Import the Gordian implementation plan into native Mission Graph | — | #38, #46 |
| #49 | [Self-Hosting] Execute an end-to-end multi-agent Gordian Mission | — | #28, #33, #38, #48 |
| #68 | [Self-Hosting] Publish the architecture falsification and retention report | — | #24, #34, #39, #49, #50, #51, #52, #53, #54, #59, #60, #61, #73, #74 |
| #69 | [Self-Hosting] Produce the end-to-end release qualification evidence bundle | — | #42, #43, #47, #49, #57, #65, #66, #67, #68 |

# Critical path

The spine is drawn once, in [`execution-order.md`](execution-order.md) section 4, as the set of
Atoms lying on a maximum-length blocker path to #69. The shape of it:

```text
Foundation and executable research
    -> typed Mission Graph kernel
    -> evidence, scheduling, and persistence
    -> the source plane
    -> local agent coordinator
    -> CLI and typed API
    -> native import and the self-hosting Mission
    -> architecture retain/revise/reject decisions
    -> release qualification
```

Distributed execution and the rich human UI are intentionally outside the shortest path to
validating the substrate, but they are not orphaned: `execution-order.md` section 15 states the
coverage rule that keeps every Atom in `closure(#68) ∪ closure(#69)`.

The **minimal self-hosting prerequisite set** — the 43 Atoms that must close before #49 can
execute — is in [`execution-order.md`](execution-order.md#minimal-self-hosting-prerequisite-set),
with the arithmetic that produced the number. #48 imports exactly those 43.

# Project-management bootstrap

Until Gordian can self-host:

- each GitHub issue represents one Atom;
- the issue body records Initiative, objective, dependencies, acceptance, verification, and benchmark obligations;
- the native `blocked by` graph, not the `## Dependencies` prose, is the authority for edges;
- GitHub issue state is **not** scientific evidence of Atom satisfaction; the bootstrap
  satisfaction rule is in [`issue-index.md`](issue-index.md#bootstrap-satisfaction-rule);
- exact code verification remains attached to candidate source identities;
- the GitHub Project 9 view is a derived projection only.

Once native Mission Graph persistence exists, these issues should be imported as provenance and Gordian should become the canonical coordination substrate for its own development.
