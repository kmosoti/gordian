# Gordian Documentation

Gordian's documentation separates **specified semantics**, **formal propositions**, **external evidence**, **implementation mechanisms**, **experiments**, and **temporary planning projections**. A document should not gain authority merely because it is polished prose.

## Start here

1. [`architecture.md`](architecture.md) synthesizes the Mission Graph, Jujutsu Change Graph, execution history, evidence, provenance, authority, and closed-loop reconciliation model.
2. [`spec/mission-graph.md`](spec/mission-graph.md) defines the normative Mission Graph semantics.
3. [`spec/data-model.md`](spec/data-model.md) defines storage-independent identities and canonical records.
4. [`spec/invariants.md`](spec/invariants.md) defines safety properties and verification boundaries.
5. [`implementation/project-plan.md`](implementation/project-plan.md) carries the **normative Mission acceptance table** and a derived view of the Initiative and Atom scope. The executable Atom contracts are the GitHub issue bodies, not this document.
6. [`implementation/execution-order.md`](implementation/execution-order.md) defines the causal implementation spine, the kernel-start gate, phase concurrency, the minimal self-hosting prerequisite set, experiment decisions, and performance qualification.
7. [`implementation/issue-index.md`](implementation/issue-index.md) maps the temporary GitHub Atoms into the 14 Initiatives, defines the bootstrap satisfaction rule, and defines the derived board fields — without treating issue state as completion evidence.
8. [`implementation/agent-runbook.md`](implementation/agent-runbook.md) is the loop an autonomous agent executes: acquiring the repository and toolchain, deriving readiness with `gordian-derive-status ready`, claiming, the workspace and its exact base, verification, landing, the closure record, the board recompute, the knowledge-graph update, and the Mission stop condition.
9. [`implementation/crate-map.md`](implementation/crate-map.md) decides which crate each Rust Atom writes into and what that crate may depend on.
10. [`protocols/source-adapter-contract.md`](protocols/source-adapter-contract.md) is the adapter-neutral source-plane trait; Jujutsu and Git are two realizations of it.
11. [`protocols/jujutsu-agent-protocol.md`](protocols/jujutsu-agent-protocol.md) binds Mission Atoms to source execution and exact candidates.
12. [`protocols/jujutsu-development-environment.md`](protocols/jujutsu-development-environment.md) qualifies and bootstraps the local Jujutsu environment.
13. [`protocols/landing.md`](protocols/landing.md) defines how an admitted integration candidate reaches the shared remote, and who may do it.
14. [`knowledge-graph.md`](knowledge-graph.md) explains the executable research graph and Rust traversal tooling.

## Algorithms

- [`algorithms/scheduling.md`](algorithms/scheduling.md) covers DAG readiness, critical path, resource matching, semantic conflict prediction, leases, and scheduling policies.
- [`algorithms/evidence-and-admission.md`](algorithms/evidence-and-admission.md) covers exact-subject fingerprints, stale-evidence rejection, integration, and accepted-frontier promotion. **It carries the normative admission predicate**: the algorithm at [`#the-algorithm`](algorithms/evidence-and-admission.md#the-algorithm), whose conjuncts are defined at [`#the-admission-conjuncts-defined`](algorithms/evidence-and-admission.md#the-admission-conjuncts-defined). `spec/mission-graph.md` names the conjuncts; it does not define admission.
- [`algorithms/reconciliation.md`](algorithms/reconciliation.md) covers deterministic projection, desired-versus-observed delta, repair, and replanning.

## Research and acquisition

- [`../knowledge/ontology.md`](../knowledge/ontology.md) defines graph node and relation semantics.
- [`../knowledge/acquisition.md`](../knowledge/acquisition.md) defines source revision identity, exact claim scope, assumptions, limitations, contradiction handling, formal/experiment records, acquisition lifecycle, and repository coverage.
- [`research/methodology.md`](research/methodology.md) defines claim classes, negative evidence, reproducibility, falsification, and Goodhart defenses.
- [`research/foundations.md`](research/foundations.md) surveys planning, scheduling, concurrency, workflow, provenance, attestation, replay, Jujutsu, and formal-method foundations.
- [`research/agent-systems-2026.md`](research/agent-systems-2026.md) covers CAID, STORM, AgentRoom, AgenticFlict, CodeTeam, and agent-system reliability evidence.
- [`research/evidence-synthesis.md`](research/evidence-synthesis.md) preserves the architecture's initial evidence synthesis as provenance.
- [`research/verification-strategy.md`](research/verification-strategy.md) defines the layered proof, model-to-Rust, property, mutation, fuzz, bounded verification, concurrency, fault-injection, benchmark, and experiment strategy.

## Formal methods

- [`formal/theorem-catalog.md`](formal/theorem-catalog.md) lists exact theorem statements, assumptions, checker targets, and non-claims.
- [`formal/proof-boundary.md`](formal/proof-boundary.md) explains what a proof can and cannot establish about the implementation and external world.
- [`../formal/`](../formal/) is the isolated Lean development package. Production Gordian has no Lean runtime dependency.

Lean checks:

```bash
cd formal
lake build
```

CI re-checks the compiled environment with a separate replay pass and runs an **axiom audit**. The audit is what rejects `sorryAx` and any non-allowlisted axiom; the replay pass re-verifies the environment using Lean's kernel rather than policing `sorry`. It is not an independently implemented kernel. A theorem is machine checked only for the exact declaration, assumptions, formal sources, toolchain, and successful checker evidence.

## Validation and experiments

- [`testing/falsification-plan.md`](testing/falsification-plan.md) defines the architecture-ablation and fault-injection program.
- [`testing/statistical-contract.md`](testing/statistical-contract.md) fixes the analysis design per experiment class, so no manifest chooses its own statistics after the fact.
- [`formal/conformance-vectors.md`](formal/conformance-vectors.md) defines the Lean/Rust conformance vector format that #7 consumes.
- [`implementation/execution-order.md`](implementation/execution-order.md) turns experimental qualification into causal prerequisites rather than post-hoc validation.
- GitHub Experiment Atoms use [`.github/ISSUE_TEMPLATE/experiment.yml`](../.github/ISSUE_TEMPLATE/experiment.yml) to require a predeclared falsification and analysis contract.
- Ordinary implementation Atoms use [`.github/ISSUE_TEMPLATE/atom.yml`](../.github/ISSUE_TEMPLATE/atom.yml) to require causal dependencies, acceptance predicates, evidence, and benchmark obligations.

## Executable artifacts

```text
knowledge/graph/*.jsonld
    Sharded canonical research graph.

knowledge/ontology.md
knowledge/acquisition.md
    Ontology and acquisition/completeness protocol.

crates/gordian-kg/
    Rust loader, validator, epistemic auditor, index, traversal CLI, and DOT exporter.
    The only crate that exists today; crates/ is planned per implementation/crate-map.md.

formal/
    Isolated Lean package, models, proofs, checker configuration, and formal dependencies.

orchestration/
    Thin Python experiment, process, acquisition, and temporary Project orchestration.

artifacts/schema/closure-record.schema.json
    The single normative definition of what closing an Atom records.

artifacts/atoms/<N>/
    Per-Atom spec snapshot, verifier artifacts, attempt records, and closure.json.

scripts/bootstrap-jj.sh
    Safe Jujutsu baseline installation/configuration without push or rewrite. It is the single
    source of the pinned version; no document restates that number.

scripts/check-*.sh
    The specification-consistency checkers. The verify workflow runs every one of them.

scripts/sync_github_project.py
    Compatibility entrypoint into the packaged Project reconciliation command.
```

Three trees are declared here and do not exist yet. Each is created by the Atom named beside it,
and `gordian-kg audit --strict` reports any knowledge-graph verification target whose leading path
component is missing and whose entry status is not `planned` (**G-521, assigned to #72**):

```text
benches/                planned; owned by #5   performance benchmark and regression gates
experiments/            planned; owned by #75  protocol and run manifests, plus their schemas
formal/conformance/     planned; owned by #7   Lean/Rust conformance vectors and index.json
```

## Research graph commands

```bash
cargo run -p gordian-kg -- validate
cargo run -p gordian-kg -- audit --strict
cargo run -p gordian-kg -- stats
cargo run -p gordian-kg -- list --kind Hypothesis
cargo run -p gordian-kg -- hypotheses
cargo run -p gordian-kg -- evidence claim:semantic-state-vs-code-state
cargo run -p gordian-kg -- neighbors concept:atom
cargo run -p gordian-kg -- path concept:atom theorem:dispatch-requires-dependencies
cargo run -p gordian-kg -- theorems
cargo run -p gordian-kg -- export-dot --out /tmp/gordian.dot
```

These current commands provide basic structural traversal. Issues #71-#74 expand the schema, ontology enforcement, epistemic queries, source revision refresh, contradiction handling, and downstream-impact analysis.

`audit --strict` fails on warnings as well as errors, and is the form CI and every documented
check list uses. A bare `audit` is not a gate.

## Rust and Python boundary

Rust will own the following (**planned**; today the workspace contains one crate, `gordian-kg`,
and the crate each capability lands in is decided by
[`implementation/crate-map.md`](implementation/crate-map.md), on the order of
[`implementation/execution-order.md`](implementation/execution-order.md)):

```text
Mission Graph semantics
canonical identities and events
scheduling and leases
evidence, provenance, and admission
the source adapter
persistence and replay
capability and authority decisions
production hot paths
```

Rust owns today: knowledge-graph schema, validation, indexes, and queries (`gordian-kg`).

Python may launch tools, workers, experiments, source acquisition, GitHub projection, and statistical analysis. It must call Rust rather than independently implement readiness, satisfaction, evidence freshness, authorization, lease safety, or acceptance.

See [`../AGENTS.md`](../AGENTS.md) for the complete coding-agent contract.

## Temporary GitHub Project 9

After the runbook's deterministic GitHub credential injection:

```bash
python3.14 -m pip install -e './orchestration[dev]'
gordian-bootstrap preflight
gordian-project-sync reconcile --check
gordian-project-sync reconcile --report artifacts/project-9-reconciliation.json
```

`GH_TOKEN` is copied to `GH_TOKEN` for every `gh` subprocess and therefore overrides both
possible `hosts.yml` locations. The preflight responses, not the credential's label or storage
location, establish whether it has the required repository and Project capabilities. Interactive
credential changes are outside the unattended loop because they are not deterministic across
harnesses.

The command reconciles open issues into Project 9 and verifies the resulting URL set. It does not make the board canonical Gordian state.

### Reconciliation snapshot policy

`artifacts/project-9-reconciliation.json` is a tracked generated report. It is regenerated by the
`--report` command above, and only on these triggers: after an Atom is added, split, or closed,
and after any dependency edge changes. It carries no run identity today — no timestamp, no source
state, no tool version — so two copies cannot be ordered. **G-609 is assigned to #70**, which adds
`generated_at`, `source_change_id`, `source_commit_id`, and `tool_versions` to the emitted
object.

## Epistemic labels

| Label | Meaning |
| --- | --- |
| `formal theorem` | Kernel-checked proposition under explicit model assumptions. |
| `established foundation` | Mechanism adapted from mature scientific or engineering work or a standard. |
| `evidence-supported conclusion` | Empirical conclusion with study and task scope attached. |
| `engineering deduction` | Inspectable mechanism-based inference not directly established as an empirical result. |
| `hypothesis` | Gordian-specific design claim with an experiment capable of forcing revision. |
| `assumption` | Premise required by an algorithm, theorem, or experiment. |
| `unresolved uncertainty` | Material question for which current evidence cannot justify a stronger conclusion. |

This classification is part of the architecture. Gordian should make unsupported certainty structurally awkward.
