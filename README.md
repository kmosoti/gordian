# Gordian

Gordian is a research-driven coordination substrate for software development by humans and autonomous agents.

It is not a project tracker, a Git workflow wrapper, or a branch-management convention. Gordian treats software development as a closed-loop coordination problem: specify a desired state, decompose it into verifiable work, execute against isolated code snapshots, observe evidence, and admit only verified states into the accepted frontier.

## Core model

Gordian separates four structures that conventional project systems tend to collapse:

```text
Mission Graph       what should become true
Change Graph        what code states exist (Jujutsu)
Execution History   what actually happened
Evidence Graph      what is justified by observation
```

The canonical planning abstraction is the **Mission Graph**.

```text
Project
  Mission
    PlanRevision
      Initiative
        Atom
          Quark
```

This hierarchy describes decomposition, not execution order. Hard execution dependencies form a separate directed acyclic graph.

## Mission Graph semantics

- **Project** is a persistent namespace and system boundary.
- **Mission** defines a desired state, constraints, and acceptance criteria. It is intentionally independent of any particular implementation plan.
- **PlanRevision** is a versioned strategy for satisfying a Mission.
- **Initiative** is a non-primitive capability or subgoal.
- **Atom** is the smallest independently schedulable and verifiable work contract.
- **Quark** is an execution primitive internal to an Atom.
- **ExecutionAttempt** records one concrete attempt to realize an Atom or Quark.
- **Artifact** is a produced or consumed entity.
- **Evidence** is an observation relevant to an acceptance predicate.
- **Attestation** binds a claim to evidence, identity, execution context, and exact artifacts.

## Design principles

1. **Goal and plan are different objects.** Replanning must not mutate the identity of the Mission.
2. **Decomposition and dependency are different graphs.** Containment answers “part of what?” while dependency answers “requires what?”.
3. **Atoms are contracts, not tickets.** They expose preconditions, declared inputs, outputs, semantic read/write claims, acceptance predicates, and verifier requirements.
4. **Quarks preserve Atom abstraction boundaries.** Cross-Atom dependency on an internal Quark is forbidden; promote it to an Atom if another unit must depend on it.
5. **Attempts are not specifications.** Attempts can fail, time out, or be abandoned while the Atom remains a stable contract.
6. **State is derived from facts where possible.** `ready`, `blocked`, `running`, `verifying`, and `satisfied` are projections over graph structure, attempts, and evidence rather than manually edited truth.
7. **Evidence is content-bound.** Verification applies to exact specification revisions, commit IDs, inputs, environments, and verifier versions.
8. **Agents are probabilistic workers.** Correctness belongs to the deterministic substrate: isolation, permissions, provenance, verification, replay, and admission rules.
9. **Coordination is semantic, not merely textual.** Workers declare and observe interface, capability, read, and write claims rather than relying on file-level merge success.
10. **The accepted frontier is authority-controlled.** Worker agents cannot move `main`, redefine `trunk()`, create releases, or deploy by default.

## Jujutsu execution model

| Gordian concept | Jujutsu representation |
| --- | --- |
| Accepted code frontier | `trunk()` |
| Public accepted name | `main` |
| Exact accepted version | commit ID |
| Logical evolving code change | change ID |
| Worker execution environment | workspace |
| Independent work | sibling changes |
| Causally dependent work | parent/child changes |
| Candidate integration | multi-parent change |
| Revision-scoped verification | `jj run` |
| Repository recovery/history | operation log |
| External transport identity | bookmark |
| Release identity | tag |
| Production truth | deployment record pointing at an immutable release |
| Permanent `develop` bookmark | none |

The operational rule is:

> Bookmarks represent external identities. Changes represent evolving code. Workspaces represent execution. The DAG represents causality. Verification binds to exact commits.

## Repository structure

```text
docs/
  architecture.md
  spec/
    mission-graph-v0.md
  protocols/
    jujutsu-agent-protocol.md
  research/
    evidence-synthesis.md
```

## Current phase

Gordian is in **specification and falsification** mode. The current objective is to define the Mission Graph semantics, invariants, Jujutsu execution protocol, evidence model, and measurable hypotheses tightly enough that the first runtime can be small and experimentally challenged rather than prematurely feature-rich.
