# Gordian

Gordian is a research-driven coordination substrate for software development by humans and autonomous agents.

It is not a project tracker, a Git workflow wrapper, or a branch-management convention. Gordian models engineering work as a set of explicit, verifiable graphs and uses Jujutsu as the change-state substrate beneath execution.

## Core idea

Traditional project systems collapse planning, execution, source control, and status into mutable tickets and branches. Gordian separates them.

```text
Book of Work         what should happen
Jujutsu DAG          what code states exist
Execution history    what actually happened
Evidence graph       what has been verified
Accepted frontier    what is allowed to become reality
```

The Book of Work decomposes engineering intent through:

```text
Project
  Mission
    PlanRevision
      Initiative
        Atom
          Quark
```

This hierarchy is decomposition, not execution order. Hard execution dependencies form a separate DAG.

## Design principles

1. **Mission is goal, not plan.** A mission defines desired state, constraints, and acceptance. Plans may be revised without changing the mission identity.
2. **Atoms are verifiable contracts.** An Atom is the smallest independently schedulable unit with explicit inputs, preconditions, outputs, acceptance predicates, and evidence requirements.
3. **Quarks are executor primitives.** Quarks are internal implementation steps of an Atom and should not become cross-Atom dependency targets.
4. **Status is derived where possible.** Ready, blocked, running, verifying, and satisfied are projections over dependencies, attempts, and evidence rather than mutable labels treated as truth.
5. **Execution attempts are separate from specifications.** A failed attempt does not mean the work contract itself has failed.
6. **Evidence is bound to exact artifacts.** Verification attaches to immutable commit IDs, environment digests, verifier versions, and specification revisions.
7. **Agents are probabilistic workers.** Correctness belongs to the substrate: permissions, isolation, verification, provenance, and admission rules.
8. **Accepted state is coordinator-controlled.** Worker agents may mutate their assigned changes, but cannot redefine trunk, releases, or deployment state.

## Jujutsu model

Gordian uses Jujutsu according to these semantics:

| Gordian concept | Jujutsu representation |
| --- | --- |
| Accepted code frontier | `trunk()` |
| Public accepted name | `main` |
| Exact accepted state | commit ID |
| Logical evolving work | change ID |
| Agent execution environment | workspace |
| Independent work | sibling changes |
| Dependent work | parent/child changes |
| Integration | multi-parent change |
| Verification batch | `jj run` |
| Recovery history | operation log |
| External transport identity | bookmark |
| Release | tag |
| Production state | deployment record |
| Permanent `develop` branch | none |

The guiding rule is:

> Bookmarks represent external identities. Changes represent work. Workspaces represent execution. The DAG represents causality.

## Repository layout

```text
docs/
  architecture.md
  spec/
    bow-v0.md
  research/
    multi-agent-jujutsu.md
```

## Current phase

Gordian is in specification and falsification mode. The immediate objective is to make the work model, execution protocol, and safety invariants explicit enough to implement and test before introducing a large runtime.
