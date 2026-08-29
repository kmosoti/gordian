# Gordian Atom Index

GitHub issues are the temporary execution substrate for Gordian's implementation. Each issue is treated as an **Atom-sized contract**, not as evidence of completion. Issue state and GitHub Project status remain external workflow metadata until Gordian can import the plan into its own Mission Graph.

The current implementation Mission contains **70 Atoms**. Their titles are grouped by Initiative below; dependency links in each issue body define causality more precisely than this document's reading order.

## Foundation and Falsification

These Atoms establish reproducibility, scientific controls, reference behavior, benchmark discipline, and epistemic coverage before the runtime grows.

| Issue | Atom |
| --- | --- |
| [#1](https://github.com/kmosoti/gordian/issues/1) | Qualify and pin the Jujutsu development baseline |
| [#2](https://github.com/kmosoti/gordian/issues/2) | Stabilize Rust, formal, Python, and CI foundations |
| [#3](https://github.com/kmosoti/gordian/issues/3) | Build deterministic benchmark and workload generators |
| [#4](https://github.com/kmosoti/gordian/issues/4) | Establish simple reference algorithm baselines |
| [#5](https://github.com/kmosoti/gordian/issues/5) | Establish performance benchmark and regression gates |
| [#6](https://github.com/kmosoti/gordian/issues/6) | Qualify the verification technique stack |
| [#7](https://github.com/kmosoti/gordian/issues/7) | Build Lean and Rust differential conformance testing |
| [#8](https://github.com/kmosoti/gordian/issues/8) | Complete research-graph coverage and epistemic audit |
| [#50](https://github.com/kmosoti/gordian/issues/50) | Run Mission Graph ontology ablation |
| [#51](https://github.com/kmosoti/gordian/issues/51) | Run Atom and Quark scheduling-granularity ablation |
| [#52](https://github.com/kmosoti/gordian/issues/52) | Benchmark semantic conflict prediction against path and module baselines |
| [#53](https://github.com/kmosoti/gordian/issues/53) | Compare stable snapshot execution with continuous active rebasing |
| [#54](https://github.com/kmosoti/gordian/issues/54) | Compare derived state with mutable workflow status |
| [#59](https://github.com/kmosoti/gordian/issues/59) | Measure stale-evidence prevention and invalidation cost |
| [#60](https://github.com/kmosoti/gordian/issues/60) | Measure formal-method defect yield and maintenance cost |
| [#61](https://github.com/kmosoti/gordian/issues/61) | Benchmark and qualify the knowledge-graph storage/query architecture |

## Rust Mission Graph Kernel

These Atoms create the deterministic domain model. They are deliberately independent of GitHub, a frontend, a model provider, and a particular persistence implementation.

| Issue | Atom |
| --- | --- |
| [#9](https://github.com/kmosoti/gordian/issues/9) | Implement typed identities and immutable specification revisions |
| [#10](https://github.com/kmosoti/gordian/issues/10) | Implement decomposition and hard-dependency validation |
| [#11](https://github.com/kmosoti/gordian/issues/11) | Implement ExecutionAttempt, Candidate, and effect semantics |
| [#12](https://github.com/kmosoti/gordian/issues/12) | Implement canonical events and deterministic projection |
| [#13](https://github.com/kmosoti/gordian/issues/13) | Implement acceptance predicates and derived work state |
| [#58](https://github.com/kmosoti/gordian/issues/58) | Implement Project resource registry and external identity model |

## Planning and Reconciliation

This Initiative closes the control loop. A Mission remains stable while plans are proposed, selected, executed, observed, repaired, and replaced.

| Issue | Atom |
| --- | --- |
| [#55](https://github.com/kmosoti/gordian/issues/55) | Implement PlanRevision proposal, comparison, and selection lifecycle |
| [#56](https://github.com/kmosoti/gordian/issues/56) | Implement planner proposal interface with validated decomposition ingestion |
| [#57](https://github.com/kmosoti/gordian/issues/57) | Implement desired-versus-observed reconciliation and repair planning |

## Evidence, Provenance, and Authority

These Atoms make “done” a justified claim about an exact subject rather than a mutable label.

| Issue | Atom |
| --- | --- |
| [#14](https://github.com/kmosoti/gordian/issues/14) | Implement content-addressed artifact storage |
| [#15](https://github.com/kmosoti/gordian/issues/15) | Implement exact evidence fingerprints and freshness |
| [#16](https://github.com/kmosoti/gordian/issues/16) | Implement verifier manifests and exact-subject execution |
| [#17](https://github.com/kmosoti/gordian/issues/17) | Implement provenance and attestations |
| [#18](https://github.com/kmosoti/gordian/issues/18) | Implement capability policy and evaluate Cedar |
| [#19](https://github.com/kmosoti/gordian/issues/19) | Implement candidate admission and accepted-frontier compare-and-swap |

## Scheduling and Coordination

Safety predicates determine which Atoms may be dispatched. Scheduling policies rank only work that is already admissible.

| Issue | Atom |
| --- | --- |
| [#20](https://github.com/kmosoti/gordian/issues/20) | Implement dependency-aware ready queue and critical-path analysis |
| [#21](https://github.com/kmosoti/gordian/issues/21) | Implement worker capability and resource compatibility |
| [#22](https://github.com/kmosoti/gordian/issues/22) | Implement semantic resource claims and scope observation |
| [#23](https://github.com/kmosoti/gordian/issues/23) | Implement lease and fencing arbitration |
| [#24](https://github.com/kmosoti/gordian/issues/24) | Implement and benchmark heterogeneous scheduling policies |

## Durable Persistence and Replay

| Issue | Atom |
| --- | --- |
| [#25](https://github.com/kmosoti/gordian/issues/25) | Implement PostgreSQL canonical persistence |
| [#26](https://github.com/kmosoti/gordian/issues/26) | Implement materialized projections and deterministic rebuild |
| [#27](https://github.com/kmosoti/gordian/issues/27) | Implement transactional frontier, lease, and plan-selection transitions |
| [#28](https://github.com/kmosoti/gordian/issues/28) | Build crash, duplicate-event, and recovery fault suite |

## Jujutsu Change Plane

| Issue | Atom |
| --- | --- |
| [#29](https://github.com/kmosoti/gordian/issues/29) | Implement low-level Jujutsu command adapter and fixture repository |
| [#30](https://github.com/kmosoti/gordian/issues/30) | Implement workspace and change lifecycle |
| [#31](https://github.com/kmosoti/gordian/issues/31) | Implement candidate freeze and exact commit handoff |
| [#32](https://github.com/kmosoti/gordian/issues/32) | Implement sibling integration and conflict repair workflow |
| [#33](https://github.com/kmosoti/gordian/issues/33) | Implement exact-revision verification with Jujutsu |
| [#34](https://github.com/kmosoti/gordian/issues/34) | Run the Jujutsu versus Git source-substrate experiment |

## Agent Execution and Thin Python Orchestration

| Issue | Atom |
| --- | --- |
| [#35](https://github.com/kmosoti/gordian/issues/35) | Define worker protocol and sandbox capability envelope |
| [#36](https://github.com/kmosoti/gordian/issues/36) | Implement generic process and agent worker adapter |
| [#37](https://github.com/kmosoti/gordian/issues/37) | Implement thin Python experiment orchestration |
| [#38](https://github.com/kmosoti/gordian/issues/38) | Implement local multi-worker coordinator |
| [#39](https://github.com/kmosoti/gordian/issues/39) | Run isolation and coordination ablation |
| [#62](https://github.com/kmosoti/gordian/issues/62) | Implement and qualify sandbox backends for untrusted workers |
| [#63](https://github.com/kmosoti/gordian/issues/63) | Implement capability-scoped secret and credential brokerage |

## Distributed Robustness

Distributed execution is intentionally downstream of a working, fault-tested single-node coordinator.

| Issue | Atom |
| --- | --- |
| [#40](https://github.com/kmosoti/gordian/issues/40) | Define remote worker transport and idempotent command protocol |
| [#41](https://github.com/kmosoti/gordian/issues/41) | Implement distributed lease and accepted-frontier coordination |
| [#42](https://github.com/kmosoti/gordian/issues/42) | Build deterministic distributed fault simulation |
| [#43](https://github.com/kmosoti/gordian/issues/43) | Instrument Gordian with OpenTelemetry-compatible observability |

## Human and Programmatic Interfaces

| Issue | Atom |
| --- | --- |
| [#44](https://github.com/kmosoti/gordian/issues/44) | Implement the Gordian CLI |
| [#45](https://github.com/kmosoti/gordian/issues/45) | Implement a headless typed API and event stream |
| [#46](https://github.com/kmosoti/gordian/issues/46) | Implement GitHub bootstrap and import adapter |
| [#47](https://github.com/kmosoti/gordian/issues/47) | Build Mission Graph and evidence explorer |

## Release, Operations, and Self-Hosting

| Issue | Atom |
| --- | --- |
| [#48](https://github.com/kmosoti/gordian/issues/48) | Import the Gordian implementation plan into native Mission Graph |
| [#49](https://github.com/kmosoti/gordian/issues/49) | Execute an end-to-end multi-agent Gordian Mission |
| [#64](https://github.com/kmosoti/gordian/issues/64) | Implement immutable release and deployment records |
| [#65](https://github.com/kmosoti/gordian/issues/65) | Build reproducible signed distribution artifacts |
| [#66](https://github.com/kmosoti/gordian/issues/66) | Implement backup, restore, migration, and compatibility qualification |
| [#67](https://github.com/kmosoti/gordian/issues/67) | Perform adversarial security and authority qualification |
| [#68](https://github.com/kmosoti/gordian/issues/68) | Publish the architecture falsification and retention report |
| [#69](https://github.com/kmosoti/gordian/issues/69) | Produce the end-to-end release qualification evidence bundle |

## Temporary GitHub bootstrap

| Issue | Atom |
| --- | --- |
| [#70](https://github.com/kmosoti/gordian/issues/70) | Reconcile repository Atoms into GitHub Project 9 |

The connected GitHub integration can create repository issues but does not expose GitHub Projects mutations. `scripts/sync_github_project.py` performs the remaining idempotent board reconciliation from an authenticated local `gh` environment.

## Causal implementation spine

The shortest useful path is not the same as the issue-number order. The principal spine is:

```text
Foundation and executable research
  -> typed Mission Graph kernel
  -> exact evidence and authority
  -> dependency scheduling and persistence
  -> Jujutsu workspace/candidate/integration plane
  -> sandboxed local workers and coordinator
  -> CLI/API
  -> native import and self-hosting Mission
  -> falsification decisions
  -> release qualification
```

The following work can proceed in parallel when its explicit issue dependencies permit:

```text
research graph and formal models
benchmark generators and reference algorithms
artifact/evidence model
capability threat model
Jujutsu qualification fixtures
Project resource registry
```

Distributed coordination, a rich explorer UI, and release packaging are deliberately excluded from the earliest validation loop.

## First Codex execution cut

The first coding sequence should be narrow and evidence-producing:

1. **#1** qualifies the actual Jujutsu release. The reported local `jj 0.23.0` is not assumed to satisfy the researched protocol.
2. **#2** makes Rust, formal, Python, and CI reproducible.
3. **#8** closes obvious research-graph coverage gaps and establishes audit rules.
4. **#3** builds deterministic workload generators.
5. **#4** builds simple reference algorithms.
6. **#9** begins the production Rust domain model only after those controls exist.

The first implementation should not start with PostgreSQL, an API server, a frontend, multi-agent execution, or an optimized scheduler. Those would turn unresolved semantics into expensive concrete commitments.

## Atom completion rule

Closing a GitHub issue is an external bookkeeping action. A Gordian Atom is satisfied only when its acceptance predicates evaluate against fresh, compatible evidence. During bootstrap, issue closure should therefore include links to the exact commit/candidate, verifier evidence, benchmark artifacts where required, and any formal or experimental result the issue claims.
