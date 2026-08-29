# Gordian Implementation Roadmap

This roadmap is ordered by **epistemic dependency**, not feature excitement. Each stage should create evidence needed to justify the next stage.

## M0 — Executable research corpus

**Goal:** make the architecture inspectable, traversable, and self-checking before building the coordinator runtime.

### Deliverables

- Mission Graph v0 specification;
- architecture synthesis;
- Jujutsu agent protocol;
- evidence methodology;
- JSON-LD research knowledge graph;
- Rust graph traversal/validation CLI;
- Lean proof kernel;
- Rust + Lean CI;
- falsification plan.

### Exit criteria

```text
knowledge graph structurally validates
Rust tests pass
Lean proofs type-check with sorry forbidden
major normative rules have an explicit verification class
unsupported empirical hypotheses are labeled as hypotheses
```

## M1 — Mission Graph reference model

**Goal:** implement the smallest in-memory Rust model capable of representing and validating a Mission Graph.

### Rust modules

```text
mission/
  identity
  spec
  decomposition
  dependency

execution/
  attempt
  candidate
  effects

coordination/
  claims
  lease
  capability

evidence/
  artifact
  binding
  predicate
  attestation

events/
  event
  projection
```

### Algorithms

- typed decomposition validation;
- Kahn/topological hard-dependency validation;
- readiness derivation;
- declared non-interference predicate;
- exact evidence compatibility;
- candidate-admission predicate;
- deterministic event projection.

### Verification

- table/unit tests;
- generated/property tests;
- differential tests against deliberately tiny reference semantics;
- mutation testing for safety predicates;
- Lean theorem mapping for the safety kernel.

### Exit criteria

No persistence, networking, agent integration, or UI is required. M1 succeeds if a generated Mission Graph can be validated, scheduled symbolically, executed as synthetic attempts, and replayed deterministically.

## M2 — Persistence and replay

**Goal:** establish durable canonical history before distributed execution.

### Components

```text
PostgreSQL canonical model
event append path
projection rebuild
content-addressed artifact metadata
schema/version migration framework
```

### Critical tests

- crash between event append and projection update;
- duplicate event delivery;
- projection deletion/rebuild;
- stale schema event;
- concurrent expected-version update;
- evidence retention after invalidation.

### Formal target

Introduce a state-transition system in Lean:

```text
State
Event
ValidTransition
Invariant
```

and begin proving invariant preservation across transitions.

## M3 — Jujutsu execution adapter

**Goal:** bind synthetic Mission Atoms to real source changes without introducing autonomous agents yet.

### Operations

```text
snapshot
workspace create
change association
candidate freeze
candidate inspect
integration candidate
revision verification with jj run
accepted-frontier CAS/promote
cleanup/recovery
```

### Adapter invariant

Gordian logical identities must remain independent of Jujutsu implementation details while exact candidates retain verifiable Jujutsu commit identities.

### Comparative experiment

Implement equivalent Git worktree orchestration and run E005 from the falsification plan.

Do not graduate Jujutsu from preferred adapter to mandatory substrate unless the experiment shows material benefit.

## M4 — Verifier/evidence substrate

**Goal:** make acceptance proof-carrying at the engineering level.

### Verifier interface

```text
Verifier {
  id
  version/digest
  accepts_subject
  declared_dependencies
  environment_contract
  execute
  normalize_result
}
```

Initial verifier classes:

- command/test verifier;
- Rust compiler/clippy/test verifier;
- Python ruff/pytest/property verifier;
- Lean theorem verifier;
- schema/policy verifier.

### Output

Every run generates evidence with an exact fingerprint and provenance.

### Fault suite

Execute evidence-mutation cases from F001 automatically.

## M5 — Local multi-worker coordinator

**Goal:** test coordination algorithms without distributed-systems noise.

One coordinator process, multiple isolated local worker processes/workspaces.

### Capabilities

- dependency-aware ready queue;
- worker leases;
- semantic claims;
- scope-expansion events;
- candidate handoff;
- integration verification;
- speculative alternative candidates;
- bounded resolver work for conflicts.

### Experiments

Run:

- E001 Atom granularity;
- E002 semantic conflict prediction;
- E003 isolation/coordination ablation;
- E004 snapshot/rebase comparison.

This stage determines whether major Mission Graph mechanisms survive measurement.

## M6 — Agent adapter

**Goal:** make model providers replaceable workers rather than architecture owners.

### Agent contract

Input:

```text
Atom spec revision
exact base state
allowed capabilities
workspace handle
semantic coordination view
budget/time constraints
```

Output/events:

```text
claim expansion
questions/blocked reason
candidate ready
abandonment
execution observations
```

Model responses are effectful activity results and are recorded rather than rerun during projection replay.

### Security

Worker credentials cannot:

- move accepted frontier;
- mutate arbitrary other workspaces;
- create canonical releases;
- access deployment authority;
- bypass verifier/admission policy.

## M7 — Distributed coordinator

**Goal:** add distribution only after single-node semantics are stable.

New problems introduced intentionally:

```text
coordinator failover
event delivery
lease/fencing semantics
worker heartbeat/liveness
artifact transfer
remote workspace allocation
split-brain protection
```

### Precondition

Do not begin M7 while fundamental state-machine semantics are still changing rapidly. Distributed systems multiply ambiguity.

## M8 — Human interface and projections

**Goal:** expose Mission Graph without turning the UI into canonical truth.

Views may include:

```text
Mission decomposition
causal dependency graph
ready/blocked frontier
active attempts
semantic contention map
candidate/integration DAG
evidence/provenance explorer
research knowledge graph
accepted/release/deployment frontier
```

Human edits generate canonical commands/events. Dragging a card should never mutate an unexplained status field behind the model.

## M9 — Adaptive scheduling research

Only after a substantial event corpus exists should Gordian consider learned scheduling/conflict priors.

A learned component may propose:

```text
likely conflict
likely task duration
candidate executor
useful decomposition refinement
verification risk
```

It may not bypass hard safety predicates.

Keep the architecture:

```text
probabilistic proposal
      ->
deterministic admission envelope
```

## Non-goals during early milestones

Do not prematurely build:

- a Jira clone;
- a giant web UI;
- a generic distributed queue;
- custom VCS storage;
- an RDF database;
- an LLM planner treated as canonical truth;
- Kubernetes infrastructure before local semantics work;
- elaborate plugin systems before the core contracts stabilize.

## Research milestone rule

Each milestone must answer a question, not merely add surface area.

If an experiment falsifies a Gordian-specific assumption, the roadmap should contract or change rather than preserve the feature because implementation effort has already been spent.
