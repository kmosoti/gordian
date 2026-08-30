# Gordian Atom Register

GitHub issues are the temporary execution substrate for Gordian's implementation. Each issue is an **Atom-sized contract**, not evidence of completion. Issue state and GitHub Project status remain external workflow metadata until Gordian imports the plan into its own Mission Graph.

The current implementation Mission contains **75 open Atoms**. The exact objective, dependencies, acceptance predicates, verification methods, benchmark obligations, and falsification conditions live in each issue body. This register groups stable issue identities by Initiative without duplicating every issue title and contract, which would create another manually maintained source of drift.

## Canonical planning documents

- [`project-plan.md`](project-plan.md) describes the Initiative structure and implementation scope.
- [`execution-order.md`](execution-order.md) defines the causal spine, concurrency opportunities, qualification gates, experiments, performance suite, and end-to-end completion criteria.
- GitHub issue bodies are the current executable Atom contracts.
- GitHub Project 9 is only a temporary visual projection.

## Initiative register

| Initiative | Atom identities | Purpose |
| --- | --- | --- |
| [Foundation and Falsification](https://github.com/kmosoti/gordian/milestone/1) | #1–#8, #50–#54, #59–#61, #75 | Reproducible toolchains, Jujutsu qualification, workload generators, reference algorithms, benchmark gates, verification qualification, formal/Rust conformance, architecture ablations, and experiment run discipline. |
| [Research Knowledge Substrate](https://github.com/kmosoti/gordian/milestone/2) | #71–#74 | Rich source-revision/claim/experiment/proof schema, ontology and repository coverage, epistemic traversal, acquisition refresh, and staleness propagation. |
| [Rust Mission Graph Kernel](https://github.com/kmosoti/gordian/milestone/3) | #9–#13, #58 | Typed identities, immutable specifications, decomposition/dependency rules, attempts/candidates/effects, canonical events, derived state, and Project resources. |
| [Planning and Reconciliation](https://github.com/kmosoti/gordian/milestone/4) | #55–#57 | Alternative immutable plans, untrusted planner proposals, and desired-versus-observed repair/replanning. |
| [Evidence, Provenance, and Authority](https://github.com/kmosoti/gordian/milestone/5) | #14–#19 | Content-addressed artifacts, exact evidence fingerprints, verifier manifests, provenance/attestations, capabilities, and accepted-frontier admission. |
| [Scheduling and Coordination](https://github.com/kmosoti/gordian/milestone/6) | #20–#24 | Ready work, critical path, worker/resource matching, semantic claims, leases/fencing, and benchmark-selected scheduling policies. |
| [Durable Persistence and Replay](https://github.com/kmosoti/gordian/milestone/7) | #25–#28 | PostgreSQL canonical persistence, disposable projections, transactional transitions, crash/replay fault qualification. |
| [Jujutsu Change Plane](https://github.com/kmosoti/gordian/milestone/8) | #29–#34 | Bounded Rust adapter, workspace/change lifecycle, exact candidate handoff, integration/conflict repair, exact-revision verification, and the Jujutsu-versus-Git experiment. |
| [Agent Execution and Thin Python Orchestration](https://github.com/kmosoti/gordian/milestone/9) | #35–#39, #62–#63 | Worker protocol, sandboxing, secret brokerage, process/agent adapters, thin Python experiment control, local coordinator, and coordination ablation. |
| [Distributed Robustness](https://github.com/kmosoti/gordian/milestone/10) | #40–#43 | Idempotent remote protocol, distributed leases/frontier safety, deterministic fault simulation, and observability. |
| [Human and Programmatic Interface](https://github.com/kmosoti/gordian/milestone/11) | #44–#47 | Rust CLI, typed API/event stream, GitHub import, and Mission/evidence explorer. |
| [Self-Hosting and Acceptance](https://github.com/kmosoti/gordian/milestone/12) | #48–#49, #68–#69 | Native import, real multi-worker self-hosting Mission, architecture retention report, and release qualification evidence bundle. |
| [Release, Operations, and Acceptance](https://github.com/kmosoti/gordian/milestone/13) | #64–#67 | Immutable release/deployment state, reproducible signed artifacts, migration/recovery, and adversarial security qualification. |
| [Temporary GitHub Bootstrap](https://github.com/kmosoti/gordian/milestone/14) | #70 | Reconcile repository issues into user Project 9 without treating board status as canonical semantics. |

GitHub issue search remains the authoritative live list for the temporary substrate:

```text
https://github.com/kmosoti/gordian/issues?q=is%3Aissue+is%3Aopen
```

## Research-first execution cut

The first implementation sequence is deliberately conservative:

1. **#1** qualifies and pins the actual Jujutsu release. The reported local `jj 0.23.0` is not assumed to satisfy the protocol.
2. **#2** stabilizes Rust, formal, Python, and CI independently.
3. **#3–#7** establish deterministic workloads, reference algorithms, benchmark gates, verification qualification, and Lean/Rust conformance.
4. **#8 and #71–#75** make research, proof, experiment, and implementation provenance mechanically inspectable.
5. **#9** begins the production Mission Graph kernel only after those controls exist.

The first runtime implementation should not begin with PostgreSQL, a frontend, distributed workers, or an optimized scheduler. Those would concretize unresolved semantics before the project has reliable oracles and falsification machinery.

## Causal implementation spine

```text
Foundation and executable research
  -> typed Mission Graph kernel
  -> exact evidence and authority
  -> dependency scheduling and persistence
  -> Jujutsu workspace/candidate/integration plane
  -> sandboxed local workers and coordinator
  -> CLI and typed API
  -> native import and self-hosting Mission
  -> architecture retain/revise/reject decisions
  -> release qualification
```

See [`execution-order.md`](execution-order.md) for the complete graph, parallel work, safety gates, and performance obligations.

## Temporary Project 9 reconciliation

The connected repository integration can create issues but does not expose GitHub Projects mutations. The thin local orchestrator completes the visual projection through the authenticated GitHub CLI.

```bash
gh auth status
gh auth refresh -s project

python3.14 -m venv .venv
source .venv/bin/activate
python -m pip install -e ./orchestration

gordian-project-sync --dry-run
gordian-project-sync --report artifacts/project-9-reconciliation.json
```

The command:

- lists open repository issues once;
- lists Project items once before mutation;
- identifies missing and duplicate issue URLs;
- adds missing issues idempotently;
- lists Project items again after mutation;
- emits a machine-readable convergence report;
- does not infer readiness, satisfaction, evidence, or acceptance from GitHub fields.

`scripts/sync_github_project.py` is a compatibility entrypoint into the same packaged module, not a second implementation.

The package declares `requires-python = ">=3.14"`. On an interpreter older than 3.14 the
editable install is refused, but the module has no third-party dependencies and runs
directly:

```bash
PYTHONPATH=orchestration/src python3 -m gordian_orchestration.github_project --dry-run
```

Known limitation: the post-mutation item listing is issued immediately after the last
`gh project item-add` with no retry, so a large first run can report a false
`converged: false` because of GitHub read-after-write lag. Re-running the command is
idempotent and settles the report.

## Initiative milestones and native dependencies

Initiative membership and causal prerequisites are stored in GitHub's own primitives, not
in parallel bookkeeping.

**Initiative = milestone.** Each of the 14 Initiatives above is a GitHub milestone, and
every Atom belongs to exactly one. The milestone is the single source of truth for
Initiative membership, and it gives Initiative-level progress for free. This mapping holds
for the current single implementation Mission; a second concurrent Mission would require
Mission-qualified milestone titles.

**Dependencies = native issue relationships.** Every edge declared in an issue's
`## Dependencies` section is also a native GitHub `blocked by` relationship, created with
the `addBlockedBy` mutation. GitHub computes blocked/blocking state itself, and
`issueDependenciesSummary` exposes it without parsing Markdown.

The native relationship graph is authoritative. The `## Dependencies` prose is a
human-readable mirror, and the two must be re-checked for drift whenever a dependency
changes.

Project 9 carries two non-default fields. Everything else uses GitHub's native fields.

| Field | Source of truth | Meaning |
| --- | --- | --- |
| Milestone | Native | The Initiative the Atom belongs to. |
| Work Type | `type:*` label | `Atom` is a work contract. `Experiment` is a falsifiable study. |
| Wave | Derived | Dependency depth over the native relationship graph. |
| Status | Derived | `Ready` only when every prerequisite is satisfied. Otherwise `Blocked`. |

`Wave` is a derived projection, not an input. It is recomputed from the dependency graph,
is never edited by hand, and carries no authority. Waves 0-6 are the Foundation exit gate
of [`execution-order.md`](execution-order.md) §5, which #9 declares as hard dependencies
so the gate is enforced by the graph instead of by prose.

Sort the board by `Wave`, not by issue number. Issue numbers are arbitrary external
identities.

## Atom completion rule

Closing a GitHub issue is bookkeeping. During bootstrap, closure should include links to:

```text
exact implementation commit or Candidate
specification revision
required verifier evidence
benchmark artifacts where required
formal theorem/checker evidence where claimed
experiment run and analysis artifacts where claimed
known limitations and unresolved assumptions
```

A native Gordian Atom is satisfied only when its acceptance contract evaluates against fresh compatible evidence.
