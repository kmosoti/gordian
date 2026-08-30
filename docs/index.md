# Gordian Documentation

Gordian's documentation separates **specified semantics**, **formal propositions**, **external evidence**, **implementation mechanisms**, **experiments**, and **temporary planning projections**. A document should not gain authority merely because it is polished prose.

## Start here

1. [`architecture.md`](architecture.md) synthesizes the Mission Graph, Jujutsu Change Graph, execution history, evidence, provenance, authority, and closed-loop reconciliation model.
2. [`spec/mission-graph.md`](spec/mission-graph.md) defines the normative Mission Graph semantics.
3. [`spec/data-model.md`](spec/data-model.md) defines storage-independent identities and canonical records.
4. [`spec/invariants.md`](spec/invariants.md) defines safety properties and verification boundaries.
5. [`implementation/project-plan.md`](implementation/project-plan.md) defines the end-to-end Initiative and Atom scope.
6. [`implementation/execution-order.md`](implementation/execution-order.md) defines the causal implementation spine, parallel work, gates, experiment decisions, and performance qualification.
7. [`implementation/issue-index.md`](implementation/issue-index.md) maps the current 75 temporary GitHub Atoms into Initiatives without treating issue state as completion evidence.
8. [`protocols/jujutsu-agent-protocol.md`](protocols/jujutsu-agent-protocol.md) binds Mission Atoms to source execution and exact candidates.
9. [`protocols/jujutsu-development-environment.md`](protocols/jujutsu-development-environment.md) qualifies and bootstraps the local Jujutsu environment.
10. [`knowledge-graph.md`](knowledge-graph.md) explains the executable research graph and Rust traversal tooling.

## Algorithms

- [`algorithms/scheduling.md`](algorithms/scheduling.md) covers DAG readiness, critical path, resource matching, semantic conflict prediction, leases, and scheduling policies.
- [`algorithms/evidence-and-admission.md`](algorithms/evidence-and-admission.md) covers exact-subject fingerprints, stale-evidence rejection, integration, and accepted-frontier promotion.
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

CI also invokes an independent checker with `sorry` disallowed and audits the compiled environment for disallowed axioms. A theorem is machine checked only for the exact declaration, assumptions, formal sources, toolchain, and successful checker evidence.

## Validation and experiments

- [`testing/falsification-plan.md`](testing/falsification-plan.md) defines the architecture-ablation and fault-injection program.
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

formal/
    Isolated Lean package, models, proofs, checker configuration, and formal dependencies.

orchestration/
    Thin Python experiment, process, acquisition, and temporary Project orchestration.

scripts/bootstrap-jj.sh
    Safe Jujutsu candidate-baseline installation/configuration without push or rewrite.

scripts/sync_github_project.py
    Compatibility entrypoint into the packaged Project reconciliation command.
```

## Research graph commands

```bash
cargo run -p gordian-kg -- validate
cargo run -p gordian-kg -- audit
cargo run -p gordian-kg -- stats
cargo run -p gordian-kg -- list --kind Hypothesis
cargo run -p gordian-kg -- hypotheses
cargo run -p gordian-kg -- evidence claim:semantic-state-vs-code-state
cargo run -p gordian-kg -- neighbors concept:atom
cargo run -p gordian-kg -- path concept:atom theorem:dispatch-requires-dependencies
cargo run -p gordian-kg -- theorems
cargo run -p gordian-kg -- export-dot --out /tmp/gordian.dot
```

These current commands provide basic structural traversal. Issues #71–#74 expand the schema, ontology enforcement, epistemic queries, source revision refresh, contradiction handling, and downstream-impact analysis.

## Rust and Python boundary

Rust owns:

```text
Mission Graph semantics
canonical identities and events
scheduling and leases
evidence, provenance, and admission
Jujutsu adapter
persistence and replay
capability and authority decisions
knowledge graph schema, validation, indexes, and queries
production hot paths
```

Python may launch tools, workers, experiments, source acquisition, GitHub projection, and statistical analysis. It must call Rust rather than independently implement readiness, satisfaction, evidence freshness, authorization, lease safety, or acceptance.

See [`../AGENTS.md`](../AGENTS.md) for the complete coding-agent contract.

## Temporary GitHub Project 9

After local GitHub CLI authorization:

```bash
gh auth refresh -s project
python -m pip install -e ./orchestration
gordian-project-sync --dry-run
gordian-project-sync --report artifacts/project-9-reconciliation.json
```

The command reconciles open issues into Project 9 and verifies the resulting URL set. It does not make the board canonical Gordian state.

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
