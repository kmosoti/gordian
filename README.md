# Gordian

Gordian is a research-driven coordination substrate for software development by humans and autonomous agents.

It is not a project tracker, a Git workflow wrapper, or a branch-management convention. Gordian treats software development as a closed-loop coordination problem: specify a desired state, decompose it into verifiable work, execute against isolated code snapshots, observe evidence, and admit only justified states into the accepted frontier.

## Core model

Gordian separates structures that conventional development systems frequently collapse:

```text
Mission Graph       what should become true
Change Graph        what code states exist (Jujutsu)
Execution History   what actually happened
Evidence Graph      what is justified by observation
Authority           who may mutate accepted/external reality
```

The canonical planning abstraction is the **Mission Graph**:

```text
Project
  Mission
    PlanRevision
      Initiative
        Atom
          Quark
```

This hierarchy is decomposition, not execution order. Hard execution dependencies form a separate DAG.

## Mission Graph semantics

- **Project** is a persistent namespace and system boundary.
- **Mission** defines desired state, constraints, and acceptance. Its identity survives replanning.
- **PlanRevision** is an immutable versioned strategy for satisfying a Mission.
- **Initiative** is a compound capability or subgoal.
- **Atom** is the smallest globally schedulable and independently verifiable work contract.
- **Quark** is an execution primitive local to an Atom.
- **ExecutionAttempt** records one concrete attempt against an immutable base.
- **Candidate** is a frozen exact implementation subject handed to verification.
- **Artifact** is a produced or consumed entity.
- **Evidence** is an observation relevant to an acceptance predicate.
- **Attestation** records who/what made a claim about which exact subjects and materials.

## Research thesis

Current multi-agent software-engineering evidence does not point simply toward “give each agent a branch.” Gordian's working synthesis is:

> **isolated code state + coordinated semantic state + dependency-aware scheduling + explicit integration + exact-artifact verification + capability-gated acceptance**

The individual ingredients draw on recent agent research and established work in hierarchical planning, dependency/build graphs, optimistic concurrency, workflow analysis, provenance, software-supply-chain attestation, durable execution, and formal methods.

The exact Gordian composition is still a hypothesis. The repository therefore separates:

```text
formal theorem
established scientific/engineering foundation
evidence-supported empirical conclusion
engineering deduction
Gordian hypothesis
explicit assumption
```

## Design rules

1. **Goal and plan are different objects.** Replanning must not redefine the Mission.
2. **Decomposition and dependency are different graphs.** “Part of” is not “requires.”
3. **Atoms are contracts, not tickets.** They expose assumptions, inputs, outputs, semantic claims, acceptance predicates, and verifier requirements.
4. **Quarks preserve black-box Atom boundaries.** Cross-Atom hard dependencies cannot target another Atom's Quark.
5. **Attempts are not specifications.** Attempts may fail without changing the logical work contract.
6. **State is derived from canonical facts where practical.** `blocked`, `ready`, `running`, `verifying`, and `satisfied` are projections rather than hand-maintained truth.
7. **Evidence is identity-bound.** Candidate, spec, inputs, dependencies, environment, and verifier identities determine whether evidence remains usable.
8. **Agents are probabilistic workers.** They propose and execute; safety-critical acceptance belongs to the substrate.
9. **Coordination is semantic, not merely textual.** Declared/observed resources and interfaces inform concurrency before final merge.
10. **Acceptance is capability-gated.** A Worker cannot redefine the accepted frontier or deployment state by default.

## Jujutsu execution model

| Gordian concept | Jujutsu representation |
| --- | --- |
| Accepted source frontier | `trunk()` |
| Public accepted name | `main` |
| Exact source state | commit ID |
| Logical evolving implementation | change ID |
| Worker execution environment | workspace |
| Independent work | sibling changes |
| Causally dependent work | parent/child changes |
| Candidate integration | multi-parent change |
| Revision-scoped verification | `jj run` |
| VCS recovery/history | operation log |
| External transport identity | bookmark |
| Release identity | tag |
| Production truth | separate deployment record |
| Permanent `develop` bookmark | none |

Operational rule:

> Bookmarks represent external identities. Changes represent evolving implementations. Workspaces represent execution. The DAG represents causality. Verification binds to exact commits.

## Executable research graph

The research corpus is machine-readable at [`knowledge/graph.jsonld`](knowledge/graph.jsonld).

Its node classes include:

```text
Concept
Source
Claim
Hypothesis
Algorithm
Theorem
Document
```

Relations include `supportedBy`, `qualifiedBy`, `challengedBy`, `groundedIn`, `formalizedBy`, `implementedBy`, and `testedBy`.

The Rust CLI can validate and traverse it:

```bash
cargo run -p gordian-kg -- validate
cargo run -p gordian-kg -- list --kind Hypothesis
cargo run -p gordian-kg -- evidence claim:isolation-plus-coordination
cargo run -p gordian-kg -- neighbors concept:atom
cargo run -p gordian-kg -- path concept:atom theorem:dispatch-requires-dependencies
cargo run -p gordian-kg -- theorems
```

Traversal is not logical entailment. The edge predicate determines what a path actually means.

## Formal kernel

Lean 4 sources live under [`formal/Gordian`](formal/Gordian).

The initial proof kernel covers:

- rank-certified hard-dependency acyclicity;
- Quark exclusion from global hard-dependency targets;
- dispatchability requiring dependency/precondition/authorization witnesses;
- evidence invalidation when bound identities differ;
- Worker exclusion from accepted-frontier promotion;
- accepted candidates carrying reconciliation/conflict-free/verification/freshness/authority witnesses;
- symmetry of the declared read/write non-interference predicate;
- replay equality for equal event history under a fixed pure projector.

```bash
lake build
```

A theorem is considered **machine checked** only after CI checks the exact revision with `sorry` forbidden. Formal proof establishes a proposition relative to the model and assumptions. It does not prove that the model perfectly captures real software behavior or that Gordian's architecture improves empirical outcomes.

## Documentation map

Start with [`docs/index.md`](docs/index.md).

```text
docs/
  index.md
  architecture.md
  knowledge-graph.md

  spec/
    mission-graph-v0.md
    data-model-v0.md

  protocols/
    jujutsu-agent-protocol.md

  algorithms/
    scheduling.md
    evidence-and-admission.md
    reconciliation.md

  formal/
    proof-boundary.md
    theorem-catalog.md

  research/
    methodology.md
    foundations.md
    agent-systems-2026.md
    evidence-synthesis.md

  testing/
    falsification-plan.md

  implementation/
    roadmap.md
```

## Verification

GitHub Actions runs two independent validation tracks:

### Rust

```text
rustfmt
clippy -D warnings
unit tests
knowledge-graph structural validation
```

### Lean

```text
lake build
independent proof checking through lean-action/nanoda
sorry forbidden
```

This does **not** make every Gordian claim formally proved. The theorem catalog explicitly records the applicable verification method and the boundary of each claim.

## Current phase

Gordian is at **M0: executable research corpus**.

The next milestone is a small in-memory Rust Mission Graph reference model, followed by persistence/replay, then the Jujutsu adapter. Autonomous agents intentionally arrive later, after the deterministic semantics are testable.

See [`docs/implementation/roadmap.md`](docs/implementation/roadmap.md) for the staged plan and [`docs/testing/falsification-plan.md`](docs/testing/falsification-plan.md) for the experiments that can invalidate current design assumptions.
