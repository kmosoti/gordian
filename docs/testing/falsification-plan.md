# Falsification and Verification Plan

Gordian should fail cheaply in research before it fails expensively in architecture.

This plan turns the current design hypotheses into tests.

## Verification matrix

| Property / claim | Lean | Rust property/model tests | Fault injection | Controlled experiment | Production observation |
| --- | ---: | ---: | ---: | ---: | ---: |
| hard dependency acyclicity certificate | yes | yes | no | no | no |
| dispatch requires dependency satisfaction | yes | yes | yes | no | yes |
| stale identity invalidates evidence | yes | yes | yes | no | yes |
| worker lacks promotion authority | yes | yes | yes | no | yes |
| accepted candidate carries conflict-free witness | yes | yes | yes | no | yes |
| declared non-interference predicate symmetry | yes | yes | no | no | no |
| semantic claims predict real conflicts | no | yes | yes | **yes** | yes |
| Atom is useful scheduling boundary | no | no | no | **yes** | yes |
| snapshot isolation beats continuous rebase for agents | no | no | yes | **yes** | yes |
| Jujutsu simplifies orchestration versus Git | no | yes | yes | **yes** | yes |
| Mission Graph improves coordination quality | no | yes | yes | **yes** | yes |

## Experiment E001 — Atom granularity

### Hypothesis

Atom is a useful global scheduling/verifiability boundary while Quark remains local execution structure.

### Conditions

Use the same repository-level Missions under at least three decompositions:

```text
coarse: Initiative -> large work units
v0:     Initiative -> Atom -> local Quarks
fine:   Quark-like primitives globally scheduled
```

### Measure

```text
planner effort / token cost
number of graph nodes
critical-path length
parallelism exposed
coordination events
cross-boundary dependency promotions
integration failures
replans
wall-clock completion
verification cost
human comprehensibility rating if humans participate
```

### Falsifier

If v0 Atom/Quark separation does not improve useful coordination versus simpler coarse work, or if promotion/maintenance overhead dominates, revise/remove Quark as a first-class concept.

## Experiment E002 — Semantic conflict prediction

### Hypothesis

Semantic resource claims outperform file/path overlap at predicting harmful concurrent work.

### Predictors

```text
P1: changed-path overlap
P2: module/package ownership
P3: semantic read/write/interface claims
P4: combined model
```

### Ground truth

Use observed integration results, categorized independently into:

```text
textual conflict
compile/type conflict
test/behavior conflict
schema/config conflict
no harmful conflict
```

### Metrics

```text
precision
recall
false serialization rate
missed conflict rate
repair cost
claim-generation overhead
```

### Falsifier

If P3/P4 do not materially improve expected total cost over P1/P2 after claim-maintenance cost, semantic claims should remain optional or be simplified.

## Experiment E003 — Isolation and coordination ablation

### Conditions

```text
A: solo worker
B: isolated parallel workers, merge at end
C: isolated workers + shared task state
D: isolated workers + semantic claims + signals
```

Keep model, budget, repository tasks, verifier, and trial policy fixed.

### Measure

```text
success
abandonment
integration conflicts
repair attempts
wall-clock
model/tool cost
verification failures
run-to-run variance
```

### Falsifier

If D adds coordination cost without robust quality/latency benefit over C, do not expand the semantic coordination layer merely because it is architecturally elegant.

## Experiment E004 — Snapshot versus continuous rebase

### Conditions

```text
S: stable exact base until candidate handoff
R: periodically rebase worker onto moving accepted frontier
```

### Measure

```text
agent restart/reasoning invalidation
conflicts at integration
work discarded
candidate quality
wall-clock
cost
```

### Important confounder

Task duration and frontier-change rate strongly affect the result. Stratify by both.

## Experiment E005 — Jujutsu versus Git substrate

### Goal

Test the claim that Jujutsu materially improves the orchestration substrate rather than merely being preferred ergonomically.

### Hold constant

```text
Mission Graph
scheduler
agent models
workload
verification
coordination protocol
```

### Vary

```text
Git: branches/worktrees/commits/merge/reflog-style recovery
JJ:  workspaces/change IDs/commit IDs/multi-parent integration/op log
```

### Measure

```text
orchestrator LOC / state transitions
identity bookkeeping failures
recovery effort
concurrent operation failures
candidate evidence invalidation mistakes
integration operations
operator interventions
```

### Falsifier

If Jujutsu's measurable benefit is small or orchestration complexity rises, keep VCS behind an adapter and avoid making JJ a hard platform dependency.

## Experiment E006 — Derived state versus mutable status

### Conditions

```text
M: manual/mutable workflow status fields
D: derived readiness/blocking/satisfaction from canonical facts
```

Inject event loss/delay, failed attempts, candidate rewrites, and dependency changes.

### Measure

```text
stale status divergence
operator correction count
time to detect inconsistency
incorrect scheduling decisions
```

## Fault suite F001 — Evidence mutation

Generate a verified candidate and mutate each fingerprint component one at a time:

```text
commit
spec
input
resolved dependency
environment
verifier
```

Expected result: old evidence cannot satisfy current compatibility.

## Fault suite F002 — Authority attack

Attempt accepted-frontier and deployment mutation from:

```text
worker credential
expired coordinator token
replayed token
stale fencing token
compromised workspace process
```

Lean proves the abstract role rule only. This suite checks enforcement.

## Fault suite F003 — Event replay

Generate random valid event traces, persist the final projection, erase the projection, rebuild from history, and compare canonical state.

Then inject:

```text
duplicate events
out-of-order delivery
missing event
unknown event version
partially written event
```

The runtime should either deterministically normalize/reject these cases according to protocol, never silently derive ambiguous state.

## Property tests

Rust property tests should target structural invariants such as:

- validation rejects duplicate IDs;
- validation rejects dangling graph relations;
- any path returned by the knowledge-graph BFS consists only of real directed edges;
- generated acyclic Mission dependency graphs receive valid topological orderings;
- no scheduler output contains a node whose hard predecessor predicate is false;
- evidence-field mutation invalidates compatibility;
- acceptance never succeeds for a Worker promoter;
- serialization/deserialization preserves canonical semantics.

## Mutation testing

Deliberately mutate executable rules:

```text
remove freshness check
invert authorization check
skip one dependency
accept conflicted candidate
reuse evidence after commit rewrite
```

The test suite MUST fail for these mutations before we claim it meaningfully protects the invariant.

## Bounded model checking

Before a distributed coordinator exists, model a small system with:

```text
2 coordinators
3 workers
3 Atoms
2 semantic resources
finite lease lifetimes
candidate/retry events
```

Enumerate reachable states or use a dedicated state-machine/model-checking tool.

Look for:

- dual accepted-frontier writers;
- stale lease holder writes;
- deadlocks;
- permanently blocked ready work;
- acceptance with stale evidence;
- retry of irreversible effects;
- state divergence after duplicate/reordered events.

Lean and model checking complement each other: theorem proof establishes general properties of the formal rules; bounded exploration is excellent at finding missing rules in a state-machine design.

## Graduation gate

No Gordian-specific hypothesis should become a mandatory architectural dependency merely because:

- it has a proof about an abstract surrogate;
- it appears elegant;
- an LLM generated a compelling rationale;
- one benchmark improved;
- the feature is difficult to remove later.

Graduation requires evidence that the mechanism's benefit survives its operational cost and credible alternatives.
