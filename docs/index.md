# Gordian Documentation

Gordian's documentation is organized around a strict separation between **specified semantics**, **formal propositions**, **external evidence**, **implementation mechanisms**, and **falsifiable hypotheses**.

## Start here

1. [`architecture.md`](architecture.md) — system synthesis: Mission Graph, Change Graph, execution history, evidence, and authority.
2. [`spec/mission-graph.md`](spec/mission-graph.md) — normative Mission Graph semantics.
3. [`spec/data-model.md`](spec/data-model.md) — storage-independent identities and canonical records.
4. [`spec/invariants.md`](spec/invariants.md) — safety properties and their verification boundaries.
5. [`implementation/project-plan.md`](implementation/project-plan.md) — end-to-end Initiative/Atom implementation plan.
6. [`protocols/jujutsu-agent-protocol.md`](protocols/jujutsu-agent-protocol.md) — binding between Mission Atoms and Jujutsu execution.
7. [`knowledge-graph.md`](knowledge-graph.md) — comprehensive research graph and Rust traversal/audit tooling.
8. [`formal/theorem-catalog.md`](formal/theorem-catalog.md) — theorem statements, assumptions, checker targets, and non-claims.
9. [`formal/proof-boundary.md`](formal/proof-boundary.md) — what proof can and cannot establish about the real implementation.

## Algorithms

- [`algorithms/scheduling.md`](algorithms/scheduling.md) — DAG readiness, critical path, semantic conflict prediction, leases, and scheduling.
- [`algorithms/evidence-and-admission.md`](algorithms/evidence-and-admission.md) — exact-subject fingerprints, stale-evidence rejection, integration, and accepted-frontier promotion.
- [`algorithms/reconciliation.md`](algorithms/reconciliation.md) — deterministic event projection, desired/observed state, repair, and replanning.

## Research

- [`research/methodology.md`](research/methodology.md) — claim classes, source-version drift, negative evidence, reproducibility, falsification, and Goodhart defenses.
- [`research/foundations.md`](research/foundations.md) — planning, scheduling, concurrency, workflow, provenance, attestation, replay, Jujutsu, and formal-method foundations.
- [`research/agent-systems-2026.md`](research/agent-systems-2026.md) — CAID, STORM, AgentRoom, AgenticFlict, CodeTeam, and coding-agent reliability evidence.
- [`research/evidence-synthesis.md`](research/evidence-synthesis.md) — original evidence synthesis retained as provenance for the architecture's first research pass.

## Validation and experiments

- [`testing/falsification-plan.md`](testing/falsification-plan.md) — experiment and fault-injection program for Gordian-specific hypotheses.
- [`implementation/project-plan.md`](implementation/project-plan.md) begins with the Foundation and Falsification Initiative, which turns those experiments into implementation prerequisites rather than post-hoc validation.

## Executable artifacts

```text
knowledge/graph/*.jsonld
    Sharded canonical research graph.

knowledge/ontology.md
    Node/relation semantics and completeness rules.

crates/gordian-kg/
    Rust loader, validator, epistemic auditor, petgraph index, traversal/query CLI, and DOT exporter.

formal/
    Isolated Lean development package, toolchain, executable formal models, and proofs.

orchestration/
    Thin Python experiment/process orchestration only.
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

## Formal verification

Lean is deliberately a development dependency rather than primary application code:

```bash
cd formal
lake build
```

CI additionally invokes an independent Lean type checker with `sorry` disallowed and audits the compiled environment for disallowed axioms.

A theorem is described as machine checked only when those checks pass for the exact repository revision. A theorem's engineering interpretation must remain bounded by its explicit assumptions and formal statement.

## Rust and Python boundary

Rust owns production semantics and performance-sensitive code.

Python may launch experiments, datasets, tools, worker processes, and analysis. It must call Rust rather than independently implement Mission Graph safety rules.

See [`../AGENTS.md`](../AGENTS.md) for the full coding-agent contract.

## Epistemic labels

| Label | Meaning |
| --- | --- |
| `formal theorem` | Checked proposition under explicit model assumptions. |
| `established foundation` | Mechanism adopted from mature scientific/engineering work or standards. |
| `evidence-supported conclusion` | Empirical conclusion whose study/task scope remains attached. |
| `engineering deduction` | Mechanism-based inference not directly established as an empirical result. |
| `hypothesis` | Gordian-specific design claim with an experiment capable of forcing revision. |
| `assumption` | Premise required by an algorithm/proof/experiment. |
| `unresolved uncertainty` | Material question for which the current evidence cannot justify a stronger conclusion. |

This classification is part of the architecture. Gordian should make unsupported certainty structurally awkward.