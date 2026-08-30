# Falsification and Verification Plan

Gordian should fail cheaply in research before it fails expensively in architecture.

This plan turns the current design hypotheses into tests.

## Verification matrix

| Property / claim | Lean | Rust property/model tests | Fault injection | Controlled experiment | Production observation (planned) |
| --- | ---: | ---: | ---: | ---: | ---: |
| hard dependency acyclicity certificate | yes | yes | no | no | no |
| dispatch requires dependency satisfaction | yes | yes | yes | no | planned |
| stale identity invalidates evidence | yes | yes | yes | no | planned |
| worker lacks promotion authority | yes | yes | yes | no | planned |
| accepted candidate carries conflict-free witness | yes | yes | yes | no | planned |
| declared non-interference predicate symmetry | yes | yes | no | no | no |
| semantic claims predict real conflicts | no | yes | yes | **yes** | planned |
| Atom is the right global scheduling boundary | no | no | no | **yes** | planned |
| snapshot isolation beats continuous rebase for agents | no | no | yes | **yes** | planned |
| Jujutsu simplifies orchestration versus Git | no | yes | yes | **yes** | planned |
| Mission Graph improves coordination quality | no | yes | yes | **yes** | planned |

Gordian has no deployed runtime: there is no deployment, no telemetry, and no operational time
window from which a production observation could be drawn. The final column records intent, not
observation. A cell in that column may read `yes` only in a change that also adds a Source-typed
node to `knowledge/graph/` whose URL resolves to a deployment or telemetry record for that row's
property; the `concept:deployment-record` node does not meet that bar. This closes G-155.

Every experiment below is executed under the class, primary metric, minimum effect size, `n`, and
multiplicity policy fixed in
[`statistical-contract.md`](statistical-contract.md). Where this document lists several outcome
measures, exactly one is primary per that contract and the rest are secondary. Each `E` identifier
carries the issue that runs it; the identifier set is a projection of the `Hypothesis -testedBy->
Experiment` node set in `knowledge/graph/`, which #68 keeps in agreement with this list (G-436).

## Experiment E001 — Atom granularity (#51)

Class: `agent-trial`. Protocol: `experiments/atom-granularity/protocol.json`.

### Hypothesis

Atom is the right global scheduling and verifiability boundary, while Quark remains local
execution structure with no global scheduling identity.

### Conditions

Use the same repository-level Missions under three decompositions:

```text
coarse:      Initiative -> large work units                 (baseline)
atom-quark:  Initiative -> Atom -> local Quarks
fine:        Quark-like primitives globally scheduled
```

`coarse` is the baseline condition. Conditions are run in randomized order and paired on the
workload item.

### Primary metric

```text
wall-clock time to Mission satisfaction at a fixed token and tool budget
```

Exactly one metric is primary. Effect size is the paired Hodges-Lehmann shift against `coarse`,
with a BCa bootstrap 95% confidence interval over 10000 resamples.

### Secondary metrics

```text
planner effort / token cost
number of graph nodes
critical-path length
parallelism exposed
coordination events
cross-boundary dependency promotions
integration failures
replans
verification cost
human comprehensibility rating if humans participate
```

These ten are secondary. They are reported together as one pre-registered family under
Holm-Bonferroni correction, per section 2 of
[`statistical-contract.md`](statistical-contract.md). No conclusion is stated on a secondary
metric alone.

### Falsifier

The `atom-quark` decomposition is retained only if its median wall-clock to Mission satisfaction
is at least **15% lower** than the `coarse` baseline, measured over **150 trials per condition**
(30 workload items x 5 seeds, seed list `[1, 2, 3, 4, 5]`), with the 95% BCa interval on the
paired shift excluding zero.

If the observed reduction against `coarse` is below 15%, or if promotion and maintenance overhead
raises the primary metric in any of the three decompositions, Quark is removed as a first-class
concept and Atom decomposition is flattened. If `fine` beats `atom-quark` by 15% or more on the
same measurement, the global scheduling boundary moves down to the Quark-like primitive.

## Experiment E002 — Semantic conflict prediction (#52)

Class: `classification`. Protocol: `experiments/semantic-conflict-prediction/protocol.json`.

### Hypothesis

Semantic resource claims outperform file/path overlap at predicting harmful concurrent work.

### Predictors

```text
P1: changed-path overlap        (baseline)
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

### Primary metric

```text
expected total cost per candidate pair, in normalized repair-cost units:
    cost = c_missed * P(missed harmful conflict) + c_serial * P(false serialization)
with c_missed and c_serial fixed in the protocol before scoring
```

Claim-generation and claim-maintenance overhead is added to the P3 and P4 arms as a per-pair cost
before the comparison, so the predictors are compared on delivered cost rather than on raw
accuracy.

### Secondary metrics

```text
precision
recall
false serialization rate
missed conflict rate
repair cost
claim-generation overhead
```

These six are secondary and are reported under Holm-Bonferroni correction over the
predictor-versus-baseline comparisons, per
[`statistical-contract.md`](statistical-contract.md).

### Falsifier

P3 or P4 is retained only if its expected total cost per candidate pair is at least **10% lower
than P1** in normalized repair-cost units, over a corpus of at least **200 labeled integration
episodes containing at least 50 harmful-conflict positives**, with a stratified bootstrap 95%
confidence interval (10000 resamples) whose upper bound stays below the P1 cost.

Exclusion rule, applied by script before scoring and never by judgement: a candidate pair is
excluded if it carries no independent conflict label, if two independent labelers disagree on its
category, or if the integration episode did not complete. Excluded pairs are retained in the
corpus with their exclusion reason, and the analysis reports how many pairs each clause removed.
The corpus is frozen before any predictor is scored.

If neither P3 nor P4 reaches the 10% threshold, semantic claims remain optional and the claim
vocabulary is reduced to the subset that P4's ablation shows carrying the cost reduction.

## Experiment E003 — Isolation and coordination ablation (#39)

Class: `agent-trial`. Protocol: `experiments/isolation-coordination-ablation/protocol.json`.

### Conditions

```text
A: solo worker
B: isolated parallel workers, merge at end
C: isolated workers + shared task state           (baseline for the D comparison)
D: isolated workers + semantic claims + signals
```

Keep model, budget, repository tasks, verifier, and trial policy fixed. Temperature is fixed at
`0.0` and the seed list at `[1, 2, 3, 4, 5]`, per section 3 of
[`statistical-contract.md`](statistical-contract.md); the equal per-arm tuning budget of 40
person-hours and 20,000,000 agent tokens is recorded in each run manifest.

### Primary metric

```text
success at fixed budget: fraction of workload items reaching Mission satisfaction
within the fixed token and tool budget
```

### Secondary metrics

```text
abandonment
integration conflicts
repair attempts
wall-clock
model/tool cost
verification failures
run-to-run variance
```

Secondary metrics are reported as one family under Holm-Bonferroni correction.

### Falsifier

The semantic coordination layer of condition D is retained only if D's success at fixed budget
exceeds C's by at least **10 percentage points**, measured over **30 repository tasks x 5 seeds =
150 trials per cell**, analysed as a **paired difference in proportions on the workload item**
with a BCa bootstrap 95% confidence interval over 10000 resamples whose lower bound stays above
zero.

If D fails to reach 10 percentage points over C, the semantic coordination layer is not expanded,
whatever its architectural appeal, and C's shared task state becomes the shipped coordination
plane. If B already matches C within the same 10-point band, shared task state is dropped as well.

## Experiment E004 — Snapshot versus continuous rebase (#53)

Class: `agent-trial`. Protocol: `experiments/snapshot-vs-rebase/protocol.json`.

### Conditions

```text
S: stable exact base until candidate handoff
R: periodically rebase worker onto moving accepted frontier   (baseline)
```

### Primary metric

```text
wall-clock time to Mission satisfaction at a fixed token and tool budget
```

### Secondary metrics

```text
agent restart/reasoning invalidation
conflicts at integration
work discarded
candidate quality
cost
```

Reported as one family under Holm-Bonferroni correction.

### Stratification

Task duration and frontier-change rate both drive the result, so the design is stratified on
both. Four cells, with these bin boundaries:

```text
duration.short : median Atom attempt wall-clock <= 20 minutes
duration.long  : median Atom attempt wall-clock  > 20 minutes
churn.quiet    : <= 2 frontier admissions per hour on the base frontier
churn.busy     :  > 2 frontier admissions per hour on the base frontier
```

Each of the four cells (`short x quiet`, `short x busy`, `long x quiet`, `long x busy`) runs
**150 trials per condition** (30 workload items x 5 seeds).

### Falsifier

Snapshot isolation is retained as an architectural invariant only if S reduces median wall-clock
to Mission satisfaction by at least **15% relative to R** in **every one of the four cells**, with
the 95% BCa interval on the paired Hodges-Lehmann shift excluding zero in the pooled analysis.

Decision rule:

```text
15% or better in all four cells   -> retain snapshot isolation as an invariant
15% or better in some cells only  -> demote to a scheduler policy conditioned on the
                                     cells where it holds; the invariant is withdrawn
below 15% in every cell           -> adopt R and remove the stable-exact-base requirement
S worse than R in any cell        -> file the cell as a defect before any adoption decision
```

## Experiment E005 — Jujutsu versus Git substrate (#34)

Class: `agent-trial`. Protocol: `experiments/jj-vs-git/protocol.json`.

### Goal

Test the claim that Jujutsu improves the orchestration substrate measurably, rather than merely
being preferred ergonomically.

### Hold constant

```text
Mission Graph
scheduler
agent models
workload
verification
coordination protocol
```

Both arms are driven through the single `SourceAdapter` trait of
[`../protocols/source-adapter-contract.md`](../protocols/source-adapter-contract.md), and nothing
else varies. The Jujutsu implementation is delivered by **#29**; the Git-worktree implementation
is delivered by **#76**, the D4 Atom created so that this experiment compares two adapters behind
one trait rather than an adapter against an ad-hoc script.

### Vary

```text
Git: worktrees/branches/commits/merge/reflog-style recovery   (baseline)
JJ:  workspaces/change IDs/commit IDs/multi-parent integration/op log
```

### Primary metric

```text
operator-intervention-free completion rate: fraction of workload items reaching
Mission satisfaction with zero operator interventions recorded
```

This metric is produced by the harness, not by either adapter's implementer.

### Secondary metrics

```text
orchestrator LOC / state transitions
identity bookkeeping failures
recovery effort
concurrent operation failures
candidate evidence invalidation mistakes
integration operations
```

`orchestrator LOC / state transitions` is secondary precisely because it is authored by the same
team that implements both adapters. It is counted by one pinned tool, **`tokei 12.1.2`**, invoked
identically over both adapter crates, with the full argv and the tool's own version output
recorded in the run manifest under `metric_tooling`. Hand-counting by an implementer is
prohibited and inadmissible. Secondary metrics are reported as one family under Holm-Bonferroni
correction.

### Falsifier

Jujutsu becomes a hard platform dependency only if its operator-intervention-free completion rate
exceeds the Git-worktree arm's by at least **10 percentage points**, over **150 trials per arm**
(30 workload items x 5 seeds), with the 95% BCa interval on the paired difference in proportions
excluding zero.

Below 10 percentage points, the source plane stays behind the adapter trait with both
implementations supported, and no Gordian component is permitted to name a Jujutsu concept
directly.

## Experiment E006 — Derived state versus mutable status (#54)

Class: `agent-trial`, stratified on the injected fault case. Protocol:
`experiments/derived-vs-mutable-state/protocol.json`.

### Conditions

```text
M: manual/mutable workflow status fields                          (baseline)
D: derived readiness/blocking/satisfaction from canonical facts
```

### Fault cases

Each trial injects exactly one case, drawn round-robin so every case receives the same count:

```text
event loss
event delay
failed attempt
candidate rewrite
dependency change
```

### Primary metric

```text
incorrect scheduling decisions per 100 dispatches
```

A dispatch is incorrect when the scheduler dispatches an Atom whose canonical facts do not satisfy
the readiness predicate, or withholds one that they do satisfy, adjudicated by replay against the
event history rather than by an observer.

### Secondary metrics

```text
stale status divergence events
operator correction count
time to detect inconsistency
```

Reported as one family under Holm-Bonferroni correction.

### Falsifier

Derived state is retained only if condition D shows at least a **50% relative reduction** in
incorrect scheduling decisions per 100 dispatches against the M baseline, over **150 trials per
condition** (30 workload items x 5 seeds), which gives **30 trials per fault case per condition**
— above the 20-seed floor the statistical contract sets for injected faults. The 95% BCa interval
on the paired ratio must exclude 1.0.

Decision rule:

```text
50% or better reduction, interval excludes 1.0  -> retain derived state
below 50%, or interval includes 1.0             -> mutable status fields are retained and
                                                   derivation is reduced to a reporting view
D worse than M on any single fault case         -> that case is filed as a defect and the
                                                   experiment is re-run after the fix
```

## Fault suite F001 — Evidence mutation

Class: `fault-injection`. Protocol: `experiments/evidence-mutation/protocol.json`. Related
Atoms: #15 (freshness comparison) and #16 (required verifier evidence).

Generate a verified candidate and mutate each fingerprint component one at a time:

```text
commit
spec
input
resolved dependency
environment
verifier
```

Expected result: old evidence cannot satisfy current compatibility. The gate is a **fail-closed
rate of 1.0 with zero observed failures** over **20 seeds per enumerated mutation** (6 mutations x
20 seeds = 120 runs); the Clopper-Pearson 95% interval is reported so the residual uncertainty at
`n = 20` is visible. A single run in which mutated evidence satisfies compatibility stops the
suite and files a defect.

## Fault suite F002 — Authority attack

Class: `fault-injection`. Protocol: `experiments/authorization-engine/protocol.json`. Related
Atom: #67.

Attempt accepted-frontier and deployment mutation from:

```text
worker credential
expired coordinator token
replayed token
stale fencing token
compromised workspace process
```

Lean proves the abstract role rule only. This suite checks enforcement. The gate is a
**fail-closed rate of 1.0 with zero observed failures** over **20 seeds per enumerated attack**
(5 attacks x 20 seeds = 100 runs). Any successful mutation stops the suite and files a defect.

## Fault suite F003 — Event replay

Class: `fault-injection`. Protocol: `experiments/replay-faults/protocol.json`. Related Atom: #28.

Generate random valid event traces, persist the final projection, erase the projection, rebuild
from history, and compare canonical state.

Then inject:

```text
duplicate events
out-of-order delivery
missing event
unknown event version
partially written event
```

The runtime MUST either deterministically normalize or deterministically reject each case
according to protocol, and never silently derive ambiguous state. The gate is **zero observed
failures** over **20 seeds per enumerated injection** (5 injections x 20 seeds = 100 runs), where
a failure is any rebuilt projection that differs from the recorded canonical state or any
ambiguous state accepted without a rejection event.

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

The test suite MUST fail for these mutations before we claim it meaningfully protects the
invariant.

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

Graduation requires evidence that the mechanism's benefit survives its operational cost and credible alternatives, stated as the pre-registered minimum effect of
[`statistical-contract.md`](statistical-contract.md) and measured against the named baseline
condition. An experiment whose falsifier was never written before the data was inspected does not
graduate anything.
