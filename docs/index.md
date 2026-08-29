# Gordian Documentation

Gordian's documentation is organized around a separation between **what is specified**, **what is formally proved**, **what is supported by external evidence**, and **what remains an experimental hypothesis**.

## Start here

1. [`architecture.md`](architecture.md) — system synthesis: Mission Graph, Change Graph, execution history, evidence, and authority.
2. [`spec/mission-graph-v0.md`](spec/mission-graph-v0.md) — normative experimental protocol.
3. [`protocols/jujutsu-agent-protocol.md`](protocols/jujutsu-agent-protocol.md) — binding between Mission Atoms and Jujutsu execution.
4. [`knowledge-graph.md`](knowledge-graph.md) — machine-readable research graph and Rust traversal tooling.
5. [`formal/theorem-catalog.md`](formal/theorem-catalog.md) — theorem statements, assumptions, Lean targets, and proof status.
6. [`formal/proof-boundary.md`](formal/proof-boundary.md) — what formal proof can and cannot establish about Gordian.

## Algorithms

- [`algorithms/scheduling.md`](algorithms/scheduling.md) — dependency admission, semantic conflict prediction, leases, and scheduling.
- [`algorithms/evidence-and-admission.md`](algorithms/evidence-and-admission.md) — fingerprints, stale-evidence rejection, integration, and accepted-frontier promotion.
- [`algorithms/reconciliation.md`](algorithms/reconciliation.md) — event projection, observed-vs-desired state, repair, and replanning.

## Research

- [`research/methodology.md`](research/methodology.md) — evidence grading and falsification method.
- [`research/foundations.md`](research/foundations.md) — HTN planning, workflow soundness, dependency graphs, provenance, attestation, durable execution, and Jujutsu.
- [`research/agent-systems-2026.md`](research/agent-systems-2026.md) — CAID, STORM, AgentRoom, AgenticFlict, CodeTeam, and reliability synthesis.
- [`research/evidence-synthesis.md`](research/evidence-synthesis.md) — compact evidence ledger from the initial research pass.

## Validation and experiments

- [`testing/falsification-plan.md`](testing/falsification-plan.md) — experiments required before Gordian-specific design choices graduate from hypotheses.

## Executable artifacts

```text
knowledge/graph.jsonld
    Machine-readable Concept / Claim / Hypothesis / Algorithm / Theorem / Source graph.

crates/gordian-kg/
    Rust CLI and library for validating and traversing the research graph.

formal/Gordian/*.lean
    Minimal formal kernel for theorem-bearing Mission Graph invariants.
```

### Rust traversal examples

```bash
cargo run -p gordian-kg -- validate
cargo run -p gordian-kg -- list --kind Hypothesis
cargo run -p gordian-kg -- evidence claim:isolation-plus-coordination
cargo run -p gordian-kg -- neighbors concept:atom
cargo run -p gordian-kg -- path concept:atom theorem:dispatch-requires-dependencies
cargo run -p gordian-kg -- theorems
```

### Lean verification

```bash
lake build
```

CI additionally invokes an independent Lean type checker with `sorry` disallowed. A theorem is considered **machine checked** only when the CI proof job passes for the exact repository revision containing it.

## Epistemic labels

Gordian deliberately avoids collapsing all assertions into one confidence bucket.

| Label | Meaning |
| --- | --- |
| `formal theorem` | Proven from explicit definitions/assumptions in Lean. |
| `standard / established foundation` | Adopted from mature scientific or engineering literature/specification. |
| `evidence-supported conclusion` | Supported by empirical studies, with scope limitations retained. |
| `engineering deduction` | Follows plausibly from established mechanisms but is not itself directly measured. |
| `hypothesis` | Gordian-specific design claim requiring falsification. |
| `assumption` | Premise required by a proof or algorithm; must not be mistaken for a result. |

This distinction is part of the architecture, not editorial decoration. Gordian is intended to make unsupported certainty structurally difficult.
