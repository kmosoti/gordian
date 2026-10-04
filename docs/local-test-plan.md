# Local test plan: what this machine can settle

This plan maps the charter's build order onto one machine with no GPU, and says for each
experiment how far it can be taken here, in what order, and what has to wait for different
hardware. It is a plan, not a preregistration; each experiment still gets its own
`experiments/EXP-NNN-<slug>/` file from the template before any confirmatory run.

## 1. The machine

| Resource | Value | Consequence |
|---|---|---|
| CPU | 4 vCPU Intel Xeon 2.8 GHz, AVX2, AVX-512F, AVX-512 VNNI, FMA, F16C | Enough for the simulator, all scheduler experiments, and tiny learned components. VNNI makes the int8 arm of EXP-007 measurable here |
| Memory | 15 GiB | Resident-memory measurements for EXP-001 and EXP-007 are credible; model sizes stay in the tens of millions of parameters at most |
| Disk | about 30 GB free | Enough for run ledgers at thousands of episodes. Raw per-event traces must be sampled, not kept wholesale |
| GPU | none | EXP-006 at the scale of its precedent is impossible here. Tiny-scale recurrence is possible and is honestly labelled |
| Toolchain | Rust 1.98 with clippy, rustfmt, Miri; Python 3.11; pip and crates.io reachable | Property testing, mutation testing, and benchmarks can be installed. Analysis can use numpy and scipy |

**Operating rule:** nothing in a confirmatory run may use more than 3 of the 4 cores. The fourth
is reserved for the evaluator and the run recorder so that resource contention from measurement
does not leak into measured cost.

## 2. What can be settled here, and what cannot

| Experiment | Here | Partially here | Needs more |
|---|---|---|---|
| EXP-001 selective activation | Fully, with heuristic components | | |
| EXP-002 richer salience | Fully, including the small learned cost-aware policy (logistic or tiny MLP, CPU training in minutes) | | |
| EXP-003 representation isolation | Fully. It is an engineering experiment, not a scale experiment | | |
| EXP-004 memory | Fully for fixed-window and budgeted retrieval. Recurrent state with a tiny GRU-class model on CPU | | |
| EXP-005 world model | Empirical transition table fully. Learned dynamics at small-world scale | | |
| EXP-006 adaptive depth | | Tiny-scale recurrence with compute-matched comparison. Result labelled "tiny scale" | Any claim at the scale of the recurrent-depth precedent |
| EXP-007 low precision | | Reference vs int8 (VNNI) latency and memory on this CPU; ternary variants only if a usable component exists | Retraining for ternary beyond trivial sizes |
| EXP-I01 memory × scheduling | Fully | | |
| Stress suite | Fully | | |
| Protocol replay | Fully | | |
| Numerical replay | On this CPU only; a second backend is needed to claim tolerance across targets | | |
| Statistical replication | Fully for the heuristic-only experiments. For learned arms, a handful of independent training runs each | | |

The first meaningful result in the charter (section 12) is reachable on this machine without
anything learned. That is the target of the first three stages below.

## 3. Stages and work items

Each stage names its deliverable, acceptance check, and rough wall-clock on this machine. Items
inside a stage are independent unless noted.

### Stage A: measurement instrument (no experiments yet)

The question is whether we can measure correctness and cost reliably. Nothing else is credible
until this is true.

| Item | Deliverable | Acceptance |
|---|---|---|
| A1 Small world | `crates/gordian-world`: dependency graph with resources, configuration, fault injection, counters, event messages, snapshots, probes; episode generator producing every class in charter section 5 (ambiguous symptoms, delayed configuration change, noise, jointly decisive probe pairs, no-fault, critical fault) | A generated episode can be replayed from its seed to the identical observation stream. Each episode class is reachable from the generator and labelled |
| A2 Evaluator | `crates/gordian-eval`: scores a decision against hidden state; reports success, critical miss, abstention, probe count, resources. Lives in its own crate with no dependency on any policy code | Hand-checked tiny cases (at least 30) in a fixture file, written independently of the generator. Mutation testing on the evaluator reaches a declared kill rate |
| A3 Cost accounting | Extend `gordian-core::Budget` with a `Bill` that attributes every charge to a phase: sensing, scheduling, component execution, communication, storage | Property test: the sum of phase bills equals total spend on every generated trace. Protocol replay reproduces the bill exactly |
| A4 Run recorder | Writes a run manifest (source revision, lockfile hash, toolchain, CPU flags, seeds, policy, limits) and an episode-level results table (one row per episode, never per event) to `artifacts/runs/<id>/` | A run can be re-executed from its manifest and produces an identical results table |
| A5 Fixed components | Heuristic analyzer, rule-based estimator, memory lookup, verifier. No learning. Each has a declared cost model | Each component's declared cost matches its measured cost within a stated tolerance on this CPU |
| A6 Baselines | Simple heuristic, tuned fixed pipeline, all-component execution, random activation at matched compute, small-world oracle | Each baseline runs end to end on 200 episodes under hard limits. The oracle scores near its ceiling, confirming the environment has headroom |
| A7 Analysis | Python package `analysis/`: paired bootstrap intervals, TOST equivalence test with declared margins, per-episode-class breakdown, coverage-vs-error curve. Reads only the results table | Tested against hand-computed values and against scipy where applicable |

Verification tooling added in this stage, all development-only: `proptest` (generated traces for
A1, A3), `cargo-mutants` (A2), `criterion` (A5 cost models). Each is justified by the row that
uses it.

Rough wall-clock: the longest item is A1. Expect this stage to dominate the calendar.

### Stage B: exploration runs

Development runs to find defects and estimate variance. No hypothesis is tested yet.

| Item | Deliverable |
|---|---|
| B1 Variance estimate | Run every baseline on 500 episodes per class. Record the standard deviation of success and cost per class |
| B2 Power table | From B1, compute the episode count needed to detect the candidate margins (1 point on success, 20% on cost) at 80% and 90% power. This number becomes the sample size in every freeze |
| B3 Stress suite | Implement the six stressors from charter section 10 as episode generator modes. Run every baseline through each. Record failures; fix instrument defects, never tune baselines to pass |
| B4 Oracle gap | Measure the gap between the best baseline and the oracle per class. If the gap is small everywhere, the small world is too easy and A1 is revised before any experiment is frozen |

### Stage C: the first result (EXP-001, heuristic only)

| Item | Deliverable |
|---|---|
| C1 Preregister | `experiments/EXP-001-selective-activation/` from the template. Margins from B2. Primary: paired difference in verified success and in total cost against the tuned fixed pipeline. Critical-miss bound stated separately. Sample size from B2 |
| C2 Selector | Cheap task-conditioned selector with explicit stopping, charter section 4's initial proposal. Hard limits enabled |
| C3 Freeze | Commit the preregistration. Record the freeze commit in the file |
| C4 Execute | Run on held-out environments generated from unseen dependency structures, 3 cores, one process per arm |
| C5 Report | Category (beneficial, harmful, equivalent, unresolved), intervals, per-class behaviour, stress suite pass/fail, every excluded episode with reason. Replay check: re-run from manifest, diff the results table |

If C5 returns "harmful" or "equivalent," the charter's falsification rule applies: the selective
mechanism is simplified or removed before Stage D starts. That is a result, not a setback.

### Stage D: selection mechanisms (EXP-002, EXP-003)

Both run entirely on this machine.

| Item | Deliverable |
|---|---|
| D1 EXP-002 arms | Novelty threshold; tuned weighted score; component-conditioned score; small learned cost-aware policy trained on exploration-run data with the counterfactual fork procedure from charter section 9 at a preregistered fork rate, cost charged |
| D2 EXP-002 ablations | Novelty, uncertainty, goal relevance, reliability, history removed one at a time |
| D3 EXP-003 variants | Shared representation, semantic-only, explicit mixed boundary. Three predefined component replacements for the engineering hypothesis |
| D4 EXP-003 engineering measures | Consumers changed, adaptation data, validation effort, recorded per replacement |

### Stage E: learned state and prediction (EXP-004, EXP-005, EXP-I01)

Learned components stay tiny: linear or GRU-class models, trained on CPU in minutes. Training
runs are repeated at least 5 times with different seeds for every learned arm so that statistical
replication is reported, not assumed.

| Item | Deliverable |
|---|---|
| E1 EXP-I01 first | The 2×2 memory × scheduling interaction, preregistered before E2. Fixed-window memory vs budgeted retrieval, crossed with fixed pipeline vs the EXP-001 selector |
| E2 EXP-004 | Fixed window, compressed recurrent state (tiny GRU), budgeted retrieval, shuffled-retrieval control. Delayed-evidence and stale-memory episode classes |
| E3 EXP-005 | Empirical transition table, tiny learned dynamics, matched model-free arm. Decision regret as a primary measure, not prediction loss |

### Stage F: execution efficiency (EXP-006 tiny, EXP-007 CPU)

Only if Stages C to E produce at least one "beneficial" mechanism.

| Item | Deliverable |
|---|---|
| F1 EXP-006 at tiny scale | Shallow fixed, deeper fixed, adaptive recurrence, repeated execution without recurrent state, on the tiny models from Stage E. Parameter-matched and compute-matched runs. Report labelled "tiny scale, CPU" |
| F2 EXP-007 int8 | Reference float32 vs int8 using VNNI on this CPU. Resident memory, state memory, latency, task quality. Ternary deferred unless a component is large enough for it to matter |

## 4. What is explicitly not claimed from this machine

- Any scaling statement about recurrent depth.
- Numerical replay across backends. Only the "same CPU, same flags" claim is available.
- Ternary quantization results beyond trivial sizes.
- Anything about a second domain until the second simulator exists, written without shared code.

## 5. Branch and run conventions

Branches are short and descriptive: `small-world`, `evaluator`, `exp-001-prereg`,
`exp-001-run`. No tool-generated prefixes.

Every run writes `artifacts/runs/<timestamp>-<exp>-<arm>/manifest.json` and `results.csv`. The
directory is git-ignored; the manifest hash and the results table hash are recorded in the
experiment file's outcome section so the run is citable without committing it.

## 6. Order of the next three branches

1. `small-world`: A1 with proptest-backed replay check.
2. `evaluator`: A2 with the independent fixture file and cargo-mutants.
3. `cost-bill`: A3 and A4 together, because the recorder's identity claim is only testable once
   the bill exists.
