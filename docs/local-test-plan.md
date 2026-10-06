# Local test plan: what this machine can settle, and how to build it

This plan maps the charter's build order onto one machine with no GPU. It is written so that an
implementer can pick up any work item without further design conversation: each names its crate,
modules, public types, tests, acceptance command, and resource envelope. It is a plan, not a
preregistration; each experiment still gets its own `experiments/EXP-NNN-<slug>/` file from the
template before any confirmatory run.

Read [`charter.md`](charter.md) first. Section numbers below refer to it where cited.

## 1. The machine

| Resource | Value | Consequence |
|---|---|---|
| CPU | 4 vCPU Intel Xeon (2.8 GHz in the first sessions, 2.10 GHz since a host change; manifests record the model and frequency), AVX2, AVX-512F, AVX-512 VNNI, FMA, F16C | Enough for the simulator, all scheduler experiments, and tiny learned components. VNNI makes the int8 arm of EXP-007 measurable here |
| Memory | 15 GiB | Resident-memory measurements are credible; learned components stay at most in the tens of millions of parameters |
| Disk | about 30 GB free | Enough for episode-level results tables at thousands of episodes. Per-event traces must be sampled, never kept wholesale |
| GPU | none | EXP-006 at the scale of its precedent is impossible here. Tiny-scale recurrence is possible and is labelled as such |
| cgroups | Hybrid: v1 `memory`, `cpu`, `cpuacct`, `cpuset` mounted read-write and enforceable; v2 unified mount exposes only `hugetlb` | `scripts/cgroup-run.sh` uses v2 when a machine offers it and v1 here. Verified: a 512 MB limit killed a 700 MB allocation and recorded peak usage and failure count |
| Toolchain | Rust 1.98 with clippy, rustfmt, Miri; Python 3.11; pip and crates.io reachable | Property testing, mutation testing, and benchmarks can be installed. Analysis uses numpy and scipy |

## 2. Resource governance

These rules apply to every build, test, and run on this machine. They exist so that measurement
never contaminates what is measured, and so that one runaway process cannot take the machine
down.

### 2.1 Core allocation

| Cores | Use |
|---|---|
| 0-2 | Experimental arms, builds, tests |
| 3 | Evaluator, run recorder, the shell driving the run |

A confirmatory run never puts an arm on core 3. The run driver pins itself there with
`taskset -c 3` before launching arms.

### 2.2 Every run goes through the runner

`scripts/cgroup-run.sh` is the only sanctioned way to launch an arm, a baseline, a benchmark, or a
training job. It:

1. detects cgroup v2 (needs `cpu`, `cpuset`, `memory` in `/sys/fs/cgroup/cgroup.controllers`),
   falls back to v1, and refuses to run unisolated unless told it is a development run;
2. creates a leaf group per run, sets `cpuset`, a CFS quota, and a hard memory limit with swap
   disabled where the controller allows;
3. runs the command inside the group;
4. reports JSON: isolation mode, limits, exit code, wall nanoseconds, CPU nanoseconds from the
   controller, peak memory bytes, OOM kill count;
5. removes the group.

Defaults: cores 0-2, 300% CPU quota, 4 GB memory. A confirmatory run states its limits in the
preregistration and passes them explicitly.

```bash
scripts/cgroup-run.sh --name exp001-selective-s7 --cpus 0-2 --cpu-quota 300 --memory 2G \
  --report artifacts/runs/<run-id>/usage.json -- \
  target/release/gordian-run --manifest artifacts/runs/<run-id>/manifest.json
```

The runner's CPU-nanoseconds and peak-memory figures are the external resource measurement. The
arm's own `Bill` (item A3) is the internal one. The results table carries both, and item A4's
acceptance test is that they agree within a declared tolerance. Disagreement is an instrument
defect, never something to tune away.

### 2.3 Builds and tests

- `cargo build` and `cargo test` run with `-j 3` so core 3 stays free. Put this in
  `.cargo/config.toml` as `[build] jobs = 3` so nobody has to remember it.
- `cargo test` runs single-threaded (`-- --test-threads=1`) for any test that measures time or
  memory. Property tests and unit tests may run in parallel.
- Never run a build concurrently with a measurement run. The driver script checks for a running
  `cargo` or `rustc` process and refuses to start a confirmatory run while one exists.
- Release builds for measurement: `--release` with `debug = 1` in the release profile so stack
  traces survive, `lto = "thin"`, `codegen-units = 1`. The same profile is recorded in the
  manifest.

### 2.4 Memory ceilings

| Process | Ceiling | Why |
|---|---|---|
| One arm | 2 GB default, preregistered per experiment | Three arms in parallel stay under 6 GB with room for the OS |
| Evaluator plus recorder | 1 GB | They touch one episode at a time |
| Training job (Stages D-F) | 6 GB, run alone | Tiny models do not need more; a job that does is out of scope for this machine |
| Everything together | 12 GB | 3 GB headroom for the page cache and the shell |

### 2.5 Disk

- `artifacts/runs/` is git-ignored. Each run directory holds `manifest.json`, `results.csv`,
  `usage.json`, and optionally `events-sample.jsonl`.
- Per-event traces are kept for a preregistered sample of episodes only (default 1%). A full
  trace for a single episode is on the order of kilobytes; a run of 10,000 episodes at 1% is
  therefore a few megabytes.
- `cargo clean` before a release measurement build is not required; `target/` is excluded from the
  disk budget but is deleted first if free space drops under 5 GB.
- The driver refuses to start a run if free space is under 2 GB.

### 2.6 Time

- Every arm has a wall-clock cap in its preregistration. The runner does not enforce it; the arm's
  own `ManualClock`-driven budget does, and `timeout(1)` wraps the runner as the backstop.
- No run is left unattended without the backstop.

## 3. What can be settled here, and what cannot

| Experiment | Here | Partially here | Needs more |
|---|---|---|---|
| EXP-001 selective activation | Fully, with heuristic components | | |
| EXP-002 richer salience | Fully, including the small learned cost-aware policy | | |
| EXP-003 representation isolation | Fully. An engineering experiment, not a scale experiment | | |
| EXP-004 memory | Fully for fixed-window and budgeted retrieval. Recurrent state with a tiny GRU-class model | | |
| EXP-005 world model | Transition table fully. Learned dynamics at small-world scale | | |
| EXP-006 adaptive depth | | Tiny-scale recurrence, compute-matched. Result labelled "tiny scale" | Any claim at the scale of the recurrent-depth precedent |
| EXP-007 low precision | | float32 vs int8 (VNNI) latency and memory on this CPU | Ternary beyond trivial sizes |
| EXP-I01 memory × scheduling | Fully | | |
| Stress suite | Fully | | |
| Protocol replay | Fully | | |
| Numerical replay | On this CPU only | | A second backend |
| Statistical replication | Fully for heuristic-only experiments; 5 independent training runs per learned arm | | |

The charter's first meaningful result (section 12) is reachable here with nothing learned. That is
the target of Stages A to C.

## 4. Workspace layout at the end of Stage A

```text
Cargo.toml                 workspace: core, world, eval, components, run
.cargo/config.toml         jobs = 3
crates/gordian-core        clock, budget, ledger (exists), bill (A3)
crates/gordian-world       simulator, episode generator, observation stream (A1)
crates/gordian-eval        scoring against hidden state; no dependency on any policy (A2)
crates/gordian-components  fixed heuristic components and their cost models (A5)
crates/gordian-run         run manifest, recorder, driver binary `gordian-run` (A4, A6)
analysis/                  Python: intervals, equivalence tests, breakdowns (A7)
scripts/cgroup-run.sh      isolation runner (exists)
scripts/run-driver.sh      preflight checks and launch of one manifest (A4)
experiments/               TEMPLATE.md plus one directory per experiment
artifacts/runs/            git-ignored run outputs
```

Dependency direction, enforced by review: `core` depends on nothing in the workspace. `world`
depends on `core`. `eval` depends on `core` and `world` and nothing else; it must never depend on
`components` or `run`. `components` depends on `core` and `world`. `run` depends on everything.

External crates permitted in Stage A, all with the requirement they serve:

| Crate | Kind | Requirement | Simpler alternative considered |
|---|---|---|---|
| `serde`, `serde_json` | production | manifests and results must be readable by the analysis package | hand-written JSON; rejected because the manifest schema will grow |
| `rand_chacha` | production | seeded, reproducible episode generation | `rand` default RNG is not stable across versions; a hand-rolled PCG is possible but gains nothing |
| `proptest` | dev | generated traces for replay and bill-sum properties | hand-written cases miss the shapes that break invariants |
| `criterion` | dev | cost-model calibration with statistical noise estimates | `std::time::Instant` loops; rejected because A5 needs variance, not a single number |
| `cargo-mutants` | tool, not a dependency | evaluator mutation kill rate | none; this is the only check that the evaluator's tests test anything |

No async runtime, no database, no logging framework in Stage A. The ledger is the log.

## 5. Stage A: the measurement instrument

The question is whether we can measure correctness and cost reliably. Nothing else is credible
until this is true. Items A1 to A7 are independent except where noted; A4 depends on A3.

### A1 Small world (`crates/gordian-world`)

Branch: `small-world`.

**Modules and public types**

```text
graph.rs      Service { id, depends_on: Vec<ServiceId>, resource: ResourceKind, config: Config }
              World { services, edges, hidden: HiddenState }
fault.rs      Fault { kind: FaultKind, site: ServiceId, onset: Instant, critical: bool }
              FaultKind: ResourceExhausted | ConfigDrift | DependencyDown | CredentialExpired | Intermittent
sense.rs      Observation (a Measurement payload): Counter { service, name, value }
                                                 | Message { service, text_id, severity }
                                                 | Snapshot { service, config_hash }
              Probe { kind: ProbeKind, target: ServiceId }  -> ProbeResult, with declared cost
episode.rs    EpisodeSpec { seed, class: EpisodeClass, horizon, noise_rate, budget: Budget }
              EpisodeClass: Ambiguous | DelayedConfigChange | NoiseFlood | JointlyDecisive
                          | NoFault | CriticalFault | QuietUrgent | Duplicates | FeedbackBait
                          | StaleMemory | ComponentTimeout
              Episode { spec, world, faults, stream: Vec<(Instant, Observation)> }
              generate(spec) -> Episode              pure function of spec
step.rs       Simulator { episode, clock, cursor }    yields observations up to clock.now()
              apply(action: Action) -> Outcome        Action: Probe | Correct { site } | Declare { fault } | Abstain
```

`HiddenState` is `pub(crate)` and exposed only through `gordian_world::oracle::reveal(&Episode)`,
a function whose only permitted callers are `gordian-eval` and the oracle baseline. A policy that
imports `oracle` fails review; a `#[deprecated]`-style doc comment names the rule, and `run`
asserts at startup that no arm links the symbol (check `cargo tree`-level feature flag:
`oracle` is behind a feature `reveal-hidden-state` that only `eval` and the oracle arm enable).

**Episode classes and what each must contain**

| Class | Generator guarantee |
|---|---|
| Ambiguous | Two or more `FaultKind`s produce identical first symptoms; exactly one probe distinguishes them |
| DelayedConfigChange | A `Snapshot` with a changed `config_hash` arrives at least `k` observations before the first symptom; the correct declaration requires it |
| NoiseFlood | At least 80% of observations are irrelevant messages with high-entropy `text_id` |
| JointlyDecisive | Two probes are individually uninformative (posterior unchanged) and jointly decisive |
| NoFault | No fault; the only correct action is `Abstain` or `Declare(None)`; measures false alarms |
| CriticalFault | `critical = true`; a miss or a wrong correction here is scored under the separate bound |
| QuietUrgent | A NoiseFlood with exactly one low-severity message that is the true signal |
| Duplicates | Each true observation is repeated 2 to 5 times with identical content |
| FeedbackBait | Observations whose natural interpretation requests a computation whose result requests the same computation again |
| StaleMemory | A cross-episode memory record (see E2) that was true in an earlier episode and is false now |
| ComponentTimeout | A declared component is marked to fail or exceed its time cost in this episode |

**Tests**

- Unit: each class's guarantee above, checked by inspecting the generated episode.
- Property (proptest over `EpisodeSpec`): `generate(spec) == generate(spec)` structurally;
  stream is sorted by `Instant`; every `Fault.site` is a service in the world; a `Probe` against
  a non-service is refused.
- Property: for every class except `NoFault`, the oracle can reach the correct declaration within
  the episode budget. (This is the headroom check; it must pass before B4 can mean anything.)

**Acceptance**

```bash
cargo test -p gordian-world
cargo run -p gordian-world --example dump -- --seed 7 --class Ambiguous | sha256sum   # same hash twice
```

### A2 Evaluator (`crates/gordian-eval`)

Branch: `evaluator`.

```text
score.rs    Verdict { success: bool, critical_miss: bool, false_alarm: bool, abstained: bool,
                      probes_used: u32, decision_at: Instant }
            score(episode: &Episode, trajectory: &[(Instant, Action, Outcome)]) -> Verdict
fixtures/   tiny-cases.json: at least 30 hand-written episodes with expected verdicts,
            written from the class definitions, NOT generated by gordian-world
```

Rules: `success` is true only when the declared fault matches hidden state in kind and site, or
when the episode is `NoFault` and the arm abstained or declared none. `critical_miss` is true when
`critical` and not `success`. A trajectory that exceeds budget is scored as the last action
before exhaustion.

**Tests**

- Every fixture case produces its expected verdict.
- Property: `score` is a pure function (same inputs, same verdict).
- Property: changing the declared site on a successful trajectory makes `success` false.
- Mutation: `cargo mutants -p gordian-eval` kill rate at least 90%; surviving mutants are listed
  in `crates/gordian-eval/MUTANTS.md` with a reason each.

**Acceptance**

```bash
cargo test -p gordian-eval
cargo mutants -p gordian-eval --no-shuffle    # read the kill rate from the summary
```

### A3 Cost bill (`crates/gordian-core/src/bill.rs`)

Branch: `cost-bill` (together with A4).

```text
Phase: Sensing | Scheduling | Component(ComponentId) | Communication | Storage
Bill  { per_phase: BTreeMap<Phase, BTreeMap<Resource, u64>> }
Bill::charge(phase, Charge) -> Result<(), BudgetError>     delegates to an inner Budget
Bill::total(resource) -> u64
Bill::by_phase(resource) -> impl Iterator<(Phase, u64)>
```

Every charge in the system now goes through a `Bill`, never a bare `Budget`. The ledger receives
an `Accounting` entry per charge whose payload is the serialized `(Phase, Charge)`.

**Tests**

- Property: for every generated sequence of charges, `sum(by_phase(r)) == total(r)` for each `r`.
- Property: a refused charge leaves `Bill` equal to its state before the attempt (same as
  `Budget::charge_all`).
- Replay: rebuilding a `Bill` from the ledger's `Accounting` entries yields the same `Bill`.

### A4 Run recorder and driver (`crates/gordian-run`, `scripts/run-driver.sh`)

```text
manifest.rs   Manifest { run_id, experiment, arm, source_revision, lockfile_sha256,
                         toolchain, cpu_flags: Vec<String>, isolation: IsolationSpec,
                         seeds: Vec<u64>, episode_classes: Vec<(EpisodeClass, u32)>,
                         policy: PolicyId, limits: Budget, trace_sample_rate: f64 }
results.rs    one CSV row per episode:
              run_id, seed, class, success, critical_miss, false_alarm, abstained, undecided,
              probes_used, corrections, decision_at_ns, bill_compute, bill_memory, bill_time,
              bill_probes, bill_comm, bill_storage, components_run, components_skipped
measured.csv  one row per episode, nondeterministic, never compared byte-for-byte:
              run_id, seed, class, measured_component_ns, measured_sched_ns, measured_harness_ns
recorder.rs   writes manifest.json, results.csv, usage.json (copied from the runner's report),
              events-sample.jsonl for the sampled episodes
main.rs       gordian-run --manifest FILE     executes every (seed, class) in the manifest
```

`scripts/run-driver.sh --manifest FILE`:

1. refuses if free disk under 2 GB, if a `cargo`/`rustc` process is running, or if the manifest's
   `source_revision` does not match `git rev-parse HEAD` with a clean tree;
2. pins itself to core 3;
3. launches `gordian-run` through `scripts/cgroup-run.sh` with the manifest's `isolation` limits
   and `timeout` as a backstop;
4. after exit, compares `usage.json` CPU nanoseconds against the sum of all measured columns in
   `measured.csv` and records the ratio in `usage.json` as `internal_external_ratio`.

**Tests**

- Running the same manifest twice yields byte-identical `results.csv` (protocol replay).
- A manifest whose `source_revision` is stale is refused by the driver (shell test).
- `measured.csv` exists, has one row per results row, and is excluded from the replay comparison.
- `internal_external_ratio` is within a tolerance declared in the manifest; the first measured
  value becomes the tolerance's starting point and is recorded in this plan's revision history.

**Measured cost is the primary cost (decided in A5 review).** A5 found that declared component
costs fit the pooled average within 25% but deviate by up to 2× per episode class, and by 24× for
the verifier on late-anchor streams. A policy that skips expensive-content computations would be
misbilled if experiments used declared cost. Therefore the harness times every component call and
every scheduling decision with a monotonic clock at the boundary, records each timing in the
ledger as a `Measurement` entry from producer `harness/timer` (not `Accounting`: `Bill::replay`
decodes every `Accounting` entry and would reject a timing payload), and writes per-episode sums
to a separate `measured.csv` keyed by (seed, class). They stay out of `results.csv` because
wall-clock timings differ between runs, and `results.csv` must be byte-identical under protocol
replay. Declared cost remains what policies see and what `Bill` enforces as the hard
limit. The charter's cost `C` in every experiment is measured, not declared. The ratio of the two
is reported per class.

### A5 Fixed components (`crates/gordian-components`)

```text
Component trait:
  fn id(&self) -> ComponentId
  fn declared_cost(&self, input: &WorkingState) -> Vec<Charge>
  fn run(&mut self, input: &WorkingState) -> ComponentOutput   // pure given input; WorkingState.now carries the clock
  ComponentOutput { entries: Vec<(EntryKind, Vec<u8>)>, requests: Vec<ComputationRequest> }

heuristic.rs    rule table: symptom pattern -> candidate FaultKinds
estimator.rs    counts-based posterior over (FaultKind, site) from observations so far
memory.rs       lookup of prior episodes' (symptom, resolution) pairs, budgeted by entries read
verifier.rs     checks a candidate declaration against the available observations for consistency
```

`WorkingState` is the charter's bounded working view (section 3.1): active task, relevant services,
unresolved hypotheses, pending computations, with a declared maximum size. It is built from the
ledger by `run`, not held by components.

**Tests**

- Each component: `run` is deterministic for the same input.
- Each component: declared cost matches criterion-measured cost on this CPU within 25% at the
  median (the tolerance is widened or narrowed after the first calibration and recorded).
- `WorkingState` never exceeds its declared size (property test over ledgers).

### A5b Checker complexity (follow-up from A5 review)

`gordian_world::physics::consistent_hypotheses` is quadratic in window length when the site's
anchoring `ErrorRate` arrives late: 24× declared cost at n = 256, 1.7 ms at n = 1025. Generated
streams rarely hit this shape, but a recency window that evicts the early anchor moves toward it.
Deliverable: keep the current function as `consistent_hypotheses_reference`, add an optimized
version with a stated complexity, a proptest equivalence test against the reference over generated
and adversarial late-anchor streams, and a criterion bench showing the late-anchor shape is linear.
Then recalibrate the verifier's cost model. Required before B1, because B1's cost variance would
otherwise be dominated by one pathological shape.

### A6 Baselines (`crates/gordian-run/src/policy/`)

```text
Policy trait: fn select(&mut self, state: &WorkingState, bill: &Bill, clock: Instant) -> Vec<ComponentId>
              fn decide(&mut self, state: &WorkingState) -> Option<Action>

heuristic_only.rs    runs the heuristic component once, declares its top candidate
fixed_pipeline.rs    runs all components in a fixed order every step; order tuned on exploration data
all_components.rs    runs every component every step with no ordering
random_matched.rs    each step, runs a uniformly random subset sized to match a target compute bill
oracle.rs            reads hidden state (feature `reveal-hidden-state`); declares correctly at the
                     earliest instant the evidence would permit; labelled privileged in every output
```

**Acceptance:** each policy completes 200 episodes (20 per class, seeds 0-19) under the default
limits via the driver; the oracle's success is at least 0.95 on every class except `NoFault`,
where false alarm is 0.

### A7 Analysis (`analysis/`)

Python package, installed with `pip install -e analysis[dev]`; depends on numpy, scipy, pandas.
No plotting dependency in Stage A.

```text
analysis/gordian_analysis/
  load.py        read results.csv and usage.json for a run id; join arms on (seed, class)
  intervals.py   paired bootstrap CI (10,000 resamples, seed recorded) for a difference in means
  equivalence.py TOST for a preregistered margin; returns beneficial/harmful/equivalent/unresolved
  power.py       episodes needed for a margin at given variance and power (B2)
  breakdown.py   per-class tables; coverage-vs-error curve from abstention
  cli.py         gordian-analyze compare --a RUN --b RUN --margin-success 0.01 --margin-cost 0.20
```

**Tests:** each function against hand-computed values; `equivalence.py` against scipy's
`ttest_ind` where applicable; the four result categories each reachable by a synthetic input.

### A6c Decode each component output once (required before any experiment)

Found in the Stage B follow-ups (`experiments/exploration/b3-finding4.md`). The premise as first
written ("decodes every stored output on every step") was wrong; as built, the repeated work was a
component re-producing byte-identical output each step, decoded and charged again (review log, A6c).
The text below is the original specification, kept for the record.

The shared rule keeps
the latest output of each component and decodes, and is charged for decoding, every stored output
on every step, whether or not it changed. Deliverable: the rule decodes an output when it arrives
(the component ran this step) and keeps the decoded form; a stored output from an earlier step is
not decoded or charged again. The change must reduce real work, declared cost and counted
operations together, so the three stay consistent. Acceptance: every verdict column of
`results.csv` (success, critical_miss, false_alarm, abstained, undecided, probes_used, corrections)
is identical per episode at the 20 ms budget for every arm, with cost and decision-time columns
allowed to change; the decode units of the rule's counted-operation weights are re-validated
(A8b's R² bar); the B1 grid is re-run at the four budgets and `experiments/exploration/` gains a
before/after table.

### A6d Incremental narrowing in the shared rule (required before any experiment)

Residue of A6c. The rule re-narrows each stored hypothesis set against the bought probe results on
every step even when neither the stored set nor the probes changed. Deliverable: the narrowed view is
cached and recomputed only when a stored set or the probe results change; declared cost and counted
operations follow the work actually done. Explain the 35 episodes that A6c's zero-world-cost variant
still raised under `Fail` (the candidate is the verifier's stale set keeping priority in the rule);
if the explanation is a second artefact of the same kind, fix it here, and if it is a property of the
rule's source priority, document it and leave it. Acceptance: per-episode verdict identity at 20 ms
against a fixture committed before the change, as in A6c; counted-operation fit re-validated; the
Fail-only comparison re-run, with every remaining raised episode explained.

### A7b Ratio interval calibration (required before EXP-001 is frozen)

Found in A7 review. The percentile bootstrap for the relative-savings measure
S = 1 − ΣB/ΣA is anti-conservative on skewed costs. In the A7 worker's simulation with
lognormal per-episode costs and true S exactly at the 0.20 threshold, the 90% interval's lower
limit exceeded the threshold in 12% of experiments at n = 30, 8% at n = 100, and 7% at n = 300,
against a nominal 5%.

Deliverable: a BCa or studentized (bootstrap-t on the log ratio) interval in
`analysis/gordian_analysis/intervals.py`, selectable from the CLI, with the simulation committed
as a test. Acceptance: at EXP-001's planned n from B2, the false-exceedance rate at true S equal
to the threshold is at most 0.06 over at least 2,000 simulated experiments, using B1's empirical
cost distribution, not only lognormal. C1 may not set status `frozen` until this passes.

### A6b Final declaration when no affordable work is left (required before B1)

Found by the coordinator's headroom probe after A6 (see `docs/review-log.md`, A6). Under a binding
compute budget, almost every failure is `stop_reason = budget_exhausted`: the shared rule waits
for its patience deadline, the arm runs out of affordable work first, and the episode ends
undecided although declaring is free. Budget exhaustion then masquerades as indecision, and arms
are ranked by how long they last rather than by what they concluded.

Deliverable: when the harness determines that no affordable work is left (the existing
`BudgetExhausted` condition) or the horizon is reached, it gives the policy one final `decide` call
flagged as final; the shared rule then declares its first-ranked hypothesis, or abstains when it has
none, exactly as at the patience deadline. Only then does the episode stop. The final call is the
same for every arm (it lives in the shared rule), and its use is recorded (`stop_reason` gains
`final_declaration`, distinct from `terminal`). The analysis package's schema guard must be updated
in the same change. Tests: an arm with a tiny compute budget now declares rather than ending
undecided; arms with generous budgets produce byte-identical `results.csv` to before except where
they previously ended undecided; the oracles are unaffected.

### A8 Interleaved arms and drift control (required before B1)

Found in A5b review. Wall time on this VM drifts within a session (one benchmark moved 249, 268,
324, 325 µs over four consecutive runs) and differs between sessions (an unchanged component ran
13–34% faster in one session than in another). Hardware instruction counters are not available:
the VM exposes no PMU (`/sys/bus/event_source/devices` lists only software, tracepoint, breakpoint,
msr, power and uprobe). Measured cost compared across arms run one after another would therefore
carry machine drift as a treatment effect.

Deliverable:

1. **Interleaving.** A multi-arm manifest runs every arm of an experiment in one process. For each
   (seed, class) the episode is run once per arm, back to back, in an order drawn per episode from
   a ChaCha RNG seeded by (run seed, seed, class). Each arm still writes its own `results.csv` and
   `measured.csv`, so the analysis package is unchanged; `measured.csv` gains `arm_position` (the
   episode's position in that order). `results.csv` stays byte-identical under protocol replay for
   each arm.
2. **Drift control.** Every `drift_block` episodes (manifest field, default 50) the harness runs a
   fixed reference workload: the verifier on a fixed generated window, repeated a fixed number of
   times, timed. Results go to `drift.csv` (block index, ns). The analysis package reports the
   coefficient of variation across blocks and the ratio of last to first block.
3. **Order-effect check.** The analysis package gains a test of whether measured cost depends on
   `arm_position` (paired comparison of first-position against later-position runs of the same
   arm). A position effect larger than the preregistered margin invalidates the run's cost
   comparison.

Acceptance: an A/A run (two copies of the same arm) over 20 seeds × 11 classes shows a relative
measured-cost difference whose 90% interval contains 0, with drift and position diagnostics
reported. If it does not, interleaving is insufficient and the counted-operations alternative
(deterministic per-component work counters) is built before B1.

### A8b Counted operations as the primary cost (required before B1)

Triggered by A8's acceptance rule. On an idle machine, through the driver, two of four A/A runs gave
a 90% interval for S excluding 0 (+5.4% [+0.5%, +10.0%] and −6.9% [−13.3%, −1.4%]); about one in ten
is expected by chance. The cause is CPU time stolen by the VM host (`/proc/stat` steal is
non-zero): a few interrupted episodes dominate a ratio of totals. In the −6.9% run five episodes
carried 51% of the total absolute difference, and dropping them moved S to +0.9%. The per-block
minimum timing was stable to about 2% while the mean moved up to 80%. Interleaving removes slow
drift but not bursts that hit one copy of an episode and not the other.

Deliverable:

1. **Counters.** Every component, the shared decision rule, and the world's checker count the work
   they do in declared units: observations scanned, worlds or hypotheses evaluated, records
   compared, probe outcomes simulated. The checker's reference function stays unchanged; the
   counting goes in the optimized one or in a wrapper, with an equivalence test that counting does
   not change results. Counting must be deterministic and must not be settable by a policy.
2. **Recording.** Per episode, `results.csv` gains `ops_component` and `ops_sched` (deterministic,
   so protocol replay covers them). The schema guard in the analysis package follows.
3. **Weights.** A calibration step converts each component's operations to nanoseconds using the
   *minimum* over repeated timings of fixed windows (minimum, because interference only ever adds
   time), on an idle core through `scripts/cgroup-run.sh`. The weights are constants in code with
   their calibration record, like A5's cost constants.
4. **Validity.** Across generated windows of every class, weighted operations must track the
   minimum wall time per call with R² ≥ 0.9 for every component and for the rule; report the fit
   and its residual shape. If a component fails, its counter does not measure its work, and that
   is fixed before B1, not averaged away.
5. **Analysis.** `modelled_cost_ns` (weighted operations) becomes the default cost for
   `--relative-savings`; measured wall time stays available and is reported alongside as a
   secondary check.

Acceptance: the A/A check on `modelled_cost_ns` is exactly S = 0, because the cost is
deterministic. The meaningful test is therefore the validity fit in item 4, plus a
non-identical-arm check: `all_components` against `heuristic_only` gives a modelled-cost ratio
within the 90% interval of the wall-time ratio's median-of-episodes estimate.

The charter's `C` becomes modelled cost. This is a change of measurement, decided before any
experiment is frozen, and recorded in the review log.

## 5R. Stage R: the revised world (charter sections 1, 5 and 12, revised 2026-10-05)

Stage B showed the first world offers nothing to select. The charter now places Gordian's question
in the control of expensive reasoning. Stage R builds the world in which that question has
headroom. The first world (`gordian-world`) stays untouched as the regression environment; its
guarantees are not edited in place.

Items run in order R1, then R2 and R3 in parallel, then R4. Every item's acceptance gates on exit
codes.

### R1 Stream world and simulated reasoner (`crates/gordian-stream`)

A new crate depending on `gordian-core` and `gordian-world` (reusing its graph generation, fault
kinds, observations and public physics where they fit; anything changed is a new type in the new
crate, never an edit to `gordian-world`).

**Stream.** One long-running world per seed: a persistent service graph, a logical duration
(default 600 s), background observations, and incidents arriving as a seeded Poisson process. The
policy-facing API mirrors the first world's `Simulator`: `observe_until(now)` and
`apply(action, now)`; a policy never holds the stream.

**Incident tiers** (each generated with its tier as hidden label):

| Tier | Guarantee | Purpose |
|---|---|---|
| Plain | Identifiable by the cheap rung from the stream, or with one cheap probe | Most incidents; escalating them wastes cost |
| Hard | Not identifiable by the cheap rung within the incident's budget, by construction (for example a compound fault whose public rules the cheap components do not cover, or evidence spread so no single component sees it) | The incidents that need the expensive rung |
| Decoy | A symptom pattern that resembles an incident and resolves by itself; telling it apart needs evidence over time, which costs computation | Relevance that is not free to judge |

The tier mix is a stream parameter (default about 80% plain, 10% hard, 10% decoy).

**Persistence.** With probability `r` an incident repeats an earlier one (same kind and site,
different noise), so memory can shortcut it. At scheduled **regime changes**, part of the public
physics changes (for example one fault kind's symptom signature, or a dependency edge), announced
nowhere, so stale memory and stale rules mislead.

**Costly relevance.** Background noise is drawn from the catalogue and from plausible symptom
shapes, not from a separable id range. The first world's free noise filter must not work here;
a test shows it.

**Simulated reasoner.** A new action `Escalate { context, question }` where `context` is a list of
references to observations the policy holds. The reasoner lives on the hidden side, like the world.
Its answer is correct with probability `σ(a + b·q − c·d)` where `q` is the fraction of the incident's
decisive evidence present in the context (computed from hidden labels) and `d` is the incident's
difficulty; `(a, b, c)` are stream parameters swept in every experiment. Cost is declared as a
base plus a per-reference term, several orders of magnitude above a component call, and is paid
before the answer is produced, never in arrears. The answer's randomness comes from a ChaCha RNG
seeded by (stream seed, incident id, call index), so a stream replays exactly. The answer is a
hypothesis entry, never a measurement.

**Hidden state discipline** as in the first world: hidden labels (tiers, decisive evidence, true
faults, the reasoner's accuracy draws) are reachable only through an oracle module behind a feature,
and `scripts/check-no-oracle.sh` covers the new crate.

**Tests.** Determinism and replay; every tier's guarantee (in particular: the first world's four
components plus the shared rule cannot identify a hard incident within budget, checked by running
them; the catalogue noise filter fails on this world's noise); recurrence and regime change occur
as specified; the reasoner's empirical accuracy matches `σ(a + b·q − c·d)` within sampling error
over many calls; soundness of the public consistency checker on every prefix of the stream for the
parts of the physics it covers.

### R2 Stream evaluator (`crates/gordian-stream-eval`)

Independent of the generator and of every policy, as A2 was. Scores a stream trajectory against
hidden truth: per incident, whether and when a correct declaration was made, critical misses,
false declarations, decoys treated as incidents; per stream, escalations made, escalations that
were needed (hard incidents), escalations that were not (plain and decoy), and total cost by rung.
Hand-written fixtures written from the tier definitions, not generated; mutation testing as in A2.

### R3 Stream harness and conventional baselines (`crates/gordian-run`, new module)

**Arm knowledge.** Baselines and the substrate may use the stream's public rules and what they
learn from their own run history, never the hidden rules described in
`gordian-stream/HIDDEN-DESIGN.md` (AGENTS.md, "The rule that matters most"). A knowledge-injected
cheap rung is built only as a labelled ablation, so R4 can show how much hidden-rule knowledge
would be worth.

A worker writing an arm reads `gordian-stream/PUBLIC.md` and not `HIDDEN-DESIGN.md`. A worker
building the world, the evaluator or a privileged ceiling may read both. A worker who reads the
hidden record says so in its report. Exploration reports and the review log also describe hidden
structure; an arm worker treats a finding there as a hypothesis its arm must earn from public
evidence, never as a rule to encode.

**Dependency direction.** `gordian-stream` dev-depends on `gordian-run` for its cheap-rung tests.
When `gordian-run` gains a dependency on `gordian-stream`, move those tests into `gordian-run` and
drop the dev-dependency, rather than relying on a dev-dependency cycle.

A stream loop reusing the episode harness's accounting (fresh budget and bill per stream segment,
counted operations, measured timings, interleaved arms, drift control, privileged path). Baselines
that need no learning (charter section 7): never, always, periodic, change-triggered, threshold or
anomaly score, random at matched cost, and privileged oracle escalation. Total cost is modelled
substrate and rule cost plus reasoner cost in its own units and through a manifest exchange rate.

### R3b Integration of the stream evaluator (after R2 and R3)

Wire `gordian-stream-eval` into the stream harness in place of R3's count-only scorer; replace R3's
seam types with R2's; extend `results.csv` with the verdict columns; remove the
`gordian-stream-reveal` shim crate and its allowlist line if the evaluator's helpers make it
unnecessary; add `focus` to the stream's `oracle::calls` trace so the evaluator can fill
`CallSummary.focus`; add `gordian_stream_eval` to the policy-file ban; teach the analysis package
the stream results schema (one row per stream, verdict counts, reasoner cost in its own units,
total cost), with its schema guard. Acceptance: every arm on 10 streams through the driver,
verdicts recorded, analysis loads them; all gates on exit codes.

### R4 Headroom check

Tune every R3 baseline first (charter section 7): timing parameters (periodic period, threshold
τ and wait, escalation delay after notice, random p) per reasoner setting, on tuning streams
disjoint from the held-out streams. Then run every tuned baseline on held-out streams across the
reasoner sweep, including `ρ`. Report gaps per tier *and per hard-fault family*: the shared rung
never notices the slow-leak family, so its gap is salience headroom, not escalation-timing
headroom, and must be shown separately. Report, per tier and
parameter setting, the gap between oracle escalation and the best tuned non-privileged baseline in
verified decisions per unit of total cost, with intervals. If the gap is under the margin the
coordinator sets before the run on every setting, revise R1 before anything is frozen. The
recommendation of sweep settings and margins for EXP-101 is the deliverable.

**Margin, fixed by the coordinator before any R4 run (2026-10-06).**

- *Comparison by frontier, not by a single tuned point.* For each non-privileged baseline, sweep
  its parameters on tuning streams and trace its quality-to-cost frontier on held-out streams.
  No exchange rate between quality and cost is chosen for tuning.
- *Quality* is the pooled fraction of hard incidents declared correctly by their deadline,
  excluding the slow-leak family (reported separately as salience headroom). Plain-incident
  accuracy, critical misses and false alarms are reported beside it, never folded in.
- *Cost* is total modelled cost per stream (substrate and rule plus reasoner).
- *Headroom exists* if, on at least one reasoner setting, the privileged oracle's quality exceeds
  the best non-privileged frontier's quality at equal or lower cost by at least 0.10 absolute,
  with the 90% bootstrap interval's lower bound (streams resampled as clusters) above 0.05.
- If headroom exists only on settings where the reasoner is very informative and uncorrelated
  (high `b`, `ρ` = 0), that is reported as a conditional result, not as headroom for EXP-101.


### R5 Decomposed headroom (required before EXP-101 is preregistered)

R4 met its margin on every setting (smallest gap 0.489, lower bound 0.446), but the privileged
oracle it compared against bundles three privileges: it knows which anomalies are hard
(selection), it fires once the decisive evidence has arrived (timing), and its context is exactly
that evidence (context construction). EXP-101 concerns selection only; context construction is
EXP-102. R5 decomposes the headroom and adds the strongest public-information baseline before
anything is frozen.

**New arms.**

- `contradiction_escalation` (comparison): escalate after a delay once the cheap rung's hypothesis
  set for an anomaly is empty or contradictory, with a persistence requirement; delay and
  persistence tuned like every baseline. Uses public rules only.
- `oracle_selection_privileged`: knows which noticed anomalies are hard and escalates exactly
  those, with the rung's own context and a delay tuned on the tuning streams like the baselines.
  Isolates selection.
- `oracle_decoy_privileged`: knows which noticed anomalies are decoys and dismisses them, without
  escalating. Isolates decoy handling, which R4's oracle did not cover.

**Trace.** For delayed calls whose context lacks decisive evidence that has already arrived,
classify why (evidence at a service the rung did not attach; outside the window; context cap; other).

**Criterion, fixed by the coordinator before any R5 run (2026-10-06).** EXP-101 has selection
headroom if, at the primary setting (b = 5, ρ = 0.7), either:

1. `oracle_selection_privileged` exceeds the best non-privileged frontier, now including
   `contradiction_escalation`, at equal or lower cost by at least 0.10 in hard-incident quality
   (slow leak excluded), with the 90% cluster-bootstrap lower bound above 0.05; or
2. at quality within 0.05 of `oracle_selection_privileged`, the best non-privileged configuration
   costs at least 1.5 times as much, with the 90% interval of the cost ratio excluding 1.25.

The same two numbers are reported at b = 2.5 and b = 8 as sensitivity. If neither criterion holds at
the primary setting, EXP-101 as designed has too little to win, and the coordinator decides between
revising the world and reordering the program around context construction (EXP-102) before any
freeze. Seeds as in R4: tuning 10000–10099, held-out 20000–20199.

### R6 Context-construction headroom (before the substrate prototype)

R5 showed that perfect selection buys cost but no quality (selection oracle 0.489, the same as the
best public arm at any cost), while the remaining 0.46 of hard-incident quality, and most of the
reduction in critical misses, belongs to the context the reasoner receives. The trace attributes
the deficit to evidence at services the rung never attaches (738 missing observations) and outside
its window (330). Before any substrate is built to construct context, measure how far simple
public context builders go (charter section 7: strong simple baselines first).

**Context builders** (public information only; each a manifest option of the shared rung, so every
arm can use any builder):

- `rung` (current): the anomaly's attached observations.
- `window`: every observation of the last `W` seconds across all services, capped at `N`
  references, most recent first.
- `cooccur`: observations at any service whose abnormal readings began within `Δ` of the
  anomaly's, grouped by temporal co-occurrence.
- `neighbourhood`: observations at services within `k` hops of the anomaly's site in the public
  dependency graph.

**Arms.** Selection held fixed to isolate context: `oracle_selection_privileged` (tuned delay) with
each public builder, and the privileged decisive-evidence context (R4's oracle) as the ceiling.
Also `always_escalate` (tuned delay) with each builder, as the realistic public pairing.

**Criterion, fixed by the coordinator before any R6 run (2026-10-06).** EXP-102 has
context-construction headroom if, at the primary setting (b = 5, ρ = 0.7), with selection held at
the selection oracle, either:

1. the privileged decisive-evidence context exceeds the best public builder by at least 0.10 in
   hard-incident quality (slow leak excluded) at equal or fewer references per call, with the 90%
   cluster-bootstrap lower bound above 0.05; or
2. to come within 0.05 of the ceiling's quality, the best public builder needs at least 1.5 times
   the references per call, with the 90% interval of that ratio excluding 1.25.

Critical misses and plain-incident accuracy are reported beside quality for every row and are
never folded in. The same numbers are reported at b = 2.5 and b = 8. Seeds as in R4 and R5. If
neither clause holds, simple context builders capture the lever, and EXP-102 has too little to win
as designed.

### R7 Reasoner-law sensitivity: does R6 survive a reasoner hurt by irrelevant context?

R6 found that simple public builders come within 0.024 [0.000, 0.051] of the context-only
ceiling, at about 33 times the references per call. That finding rests on one property of the
simulated reasoner: extra references cost tokens but never lower accuracy (`HIDDEN-DESIGN.md`,
section 11). Published work reports the opposite for real models. Shi et al. 2023 report
irrelevant context lowering accuracy; Liu et al. 2023 ("Lost in the Middle") report accuracy
falling with context length and position. Both are cited from memory and must be checked against
the primary texts before any claim relies on them. R7 asks whether R6's conclusion holds when the
reasoner is hurt by references that are not evidence.

**World change** (`crates/gordian-stream`, hidden side only).

- Add `distractor_penalty: f64` to `ReasonerSpec`. It defaults to 0 through `serde(default)`, so
  existing manifests parse unchanged, and `normalized()` clamps it to at least 0.
- The informed probability becomes `h_δ = h(q, d) · exp(−δ · m / 100)`. Here `m` is the number of
  references in the context that are not decisive evidence of the focus incident, probes included.
- Accuracy stays `p0 + (1 − p0) h_δ`.
- The penalty uses the deterministic `det_exp`. It takes no new draw, so at δ = 0 every answer and
  every draw is unchanged.

This preserves the invariant: `h_δ(0, d) = 0`, so the answer still depends on the truth only
through the decisive evidence in the context.

`StreamPublic` must not carry δ. Extend the test that public information does not depend on any
hidden parameter.

**Tests** (in addition to the existing law tests):

1. At δ = 0, R6's held-out manifest at b = 5, ρ = 0.7 replays byte-identical in `results.csv` and
   `incidents.csv` for every arm, against `experiments/exploration/r6-results-sha256.csv`.
2. At δ > 0, the marginal law test (cluster-robust, as R1) passes, with `h_δ` recomputed in the
   test.
3. `h_δ` is non-increasing in `m` and equals `h` at `m = 0`.
4. Mutual information with no decisive evidence stays within its permutation baseline.

**Grid.** δ ∈ {0.05, 0.1, 0.2, 0.4} per 100 non-decisive references. At m = 250 these multiply `h`
by 0.88, 0.78, 0.61 and 0.37; at m = 50, by 0.98, 0.95, 0.90 and 0.82. δ = 0 is R6 itself and is
not re-run; test 1 establishes the identity.

- Primary setting: b = 5, ρ = 0.7, every δ.
- Sensitivity: b = 2.5 and b = 8, at ρ = 0.7 and δ = 0.2.

**Arms.** Selection is held at the selection oracle with its R6 delay. Only the context differs.

- The ceiling is `oracle_selection_context` (the selection oracle's anomalies and delay, with the
  decisive evidence delivered so far). It is the one privilege in the comparison. Its contexts
  hold only decisive evidence, so the penalty cannot touch it, and that is intended: the question
  is whether public builders lose ground.
- The public arms are the selection oracle with each R6 builder grid: `window`, `cooccur`,
  `neighbourhood` and the rung.
- R4's oracle is a reference row only.

No new builder, no `always_escalate` pairing, and no change to R6's grids.

**Tuning.** Per δ, every grid configuration runs on R6's 100 tuning streams. For each builder, the
configuration with the highest tuning quality is carried to the 200 held-out streams, with the
three next-best on R6's frontier. "The selected builder" is the carried configuration with the
highest tuning quality across builders.

R6 used the best builder on the held-out streams, which flatters the builders. R7 reports both
and uses the tuning-selected one in the criterion. At δ = 0, from R6's own files, the
tuning-selected builder is `window` 80 s, N 512: held-out 0.793, gap 0.027.

**Criterion, fixed by the coordinator before any R7 code or run (2026-10-05).** Definitions:

- G(δ) is the context-only ceiling's hard-incident quality minus the selected builder's, slow
  leak excluded, on the same 200 held-out streams at the primary setting.
- Its interval is a paired 90% cluster bootstrap over streams, with seeds as in R6.

Outcomes:

- **Robust:** at every δ in the grid, the upper bound of G(δ) is below 0.10. R6's conclusion
  survives, and EXP-102 claims references or cost at matched quality.
- **Fragile:** at some δ ≤ 0.2, G(δ) ≥ 0.10 with its lower bound above 0.05. Context construction
  has quality headroom under a plausible reasoner, and EXP-102 claims quality and references
  together, with δ as a preregistered sweep.
- **Fragile only under a strong penalty:** the fragile condition holds at δ = 0.4 and nowhere
  below.
- **Unresolved:** anything else.

Feasibility, checked before fixing the criterion:

- The ceiling is unaffected by δ (about 0.82).
- The selected builder can lie anywhere from 0 to about 0.80.
- So G can lie anywhere from about 0.02 to about 0.8, and every outcome is reachable.
- One privilege separates ceiling and comparator.

**Reported beside the criterion, never folded in:**

- For each δ:
  - critical misses and plain accuracy for every row;
  - references per call and cost per stream for every row;
  - the fewest references per call within 0.05 of the ceiling, with its ratio to the ceiling's;
  - the held-out-best builder and its gap.
- Each builder's tuning frontier per δ: quality against references per call. The question is
  whether the optimum moves to smaller contexts.
- The per-reference price, re-priced analytically from counted references at 5, 20 and 80 tokens
  per reference, for selection-oracle rows only.
  - It is valid only where no call was refused; the worker checks, per row, that no call was
    refused and that the re-priced total stays inside the budget.
  - The price changes cost, not quality, so this shows how the 8–11× cost ratio scales.

**Resource envelope.** About six tuning runs and six held-out runs. Each takes about 650 s and
240 s at R6's arm counts, and fewer arms here, so about 1.5 hours. Every run goes through the
driver; nothing is built while one runs.

**Reading the hidden record.** This worker changes the reasoner and so may read `HIDDEN-DESIGN.md`
section 11. It builds no public arm.

### R8 Distractor sensitivity of a real small model (before EXP-102 is preregistered)

R7 found that R6's context finding is fragile.

- Simple builders capture the context lever only if irrelevant references cost the reasoner about
  δ ≤ 0.05 per 100 references.
- At δ ≥ 0.1, choosing the references is worth 0.14 to 0.34 of hard-incident quality.

Simulation cannot say which regime holds. R8 measures one real model's δ on this world's own
questions. It is an exploration item, not EXP-106. Its result is an estimate for one model, not a
claim about models in general. Approved by the user on 2026-10-05.

**Model and runtime.**

- **Model:** Qwen2.5-1.5B-Instruct, GGUF, Q4_K_M quantization, downloaded once. Its sha256 is
  recorded and the file is kept outside git under `artifacts/models/` (ignored).
- **Fallback:** if the identifiability check below fails, Qwen2.5-3B-Instruct at the same
  quantization. This is chosen now, not after seeing results. If the 3B model also fails, R8 stops
  and reports that.
- **Runtime:** llama.cpp, built from a pinned release tag, used as its command-line or server
  binary, or through `llama-cpp-python` at a pinned version. The worker records which, with:
  - the requirement: local CPU inference with deterministic greedy decoding and prompt caching;
  - the simpler alternative considered;
  - the cost: build time and disk.
- **Where it lives:** in a separate virtual environment, never in `analysis/` or the Rust
  workspace.
- **Decoding:** greedy (temperature 0), a fixed seed, and a fixed maximum of output tokens.
- **Isolation:**
  - Every inference process runs under `scripts/cgroup-run.sh` on cores 0-2 with 3 threads and a
    6 GB memory limit.
  - Nothing else runs on cores 0-2 meanwhile.
  - One model is loaded at a time.

**Questions.** All questions are built from new stream seeds (30000 upward), never the tuning or
held-out seeds of R4 to R7. A new feature-gated example in `crates/gordian-stream` (beside `dump`,
behind the oracle feature) writes, per incident:

- the focus observation;
- the incident's full decisive evidence;
- its truth;
- a pool of candidate distractors: every other observation within ±40 s of the focus, from
  background and other incidents.

It is evaluator-side and is added to the oracle guard's allowlist like `dump`.

- **Families:** hard incidents, slow leak excluded. Plain incidents are a separate, secondary set.
- **Contexts:** all of the decisive evidence (q = 1) plus `m` distractors drawn without
  replacement from the pool by a seeded generator, sorted by time as a window builder sorts them.
  An incident whose pool is smaller than the largest `m` is excluded from the design before any
  call. The count is reported.
- **Levels:** m ∈ {0, 50, 100, 200, 400}. Each question appears at every level, so the comparison
  across levels is paired.
- **Control:** q = 0 at m = 50, with no decisive evidence. This estimates the guess rate `p0`.
- **Prompt:** one fixed system prompt states the knowledge the simulated reasoner has.
  - It includes the first world's public physics and the hard families' rules, from
    `HIDDEN-DESIGN.md` sections 4 and 4.2.
  - The reasoner is the hidden side's stand-in for broad knowledge. This is not an arm, and the
    arm-knowledge rule does not apply to it.
  - The prompt asks for one answer, in a fixed format, from the enumerated hypotheses: each known
    and hard kind at each service, or "not an incident".
  - An answer that does not parse is wrong. The parse-failure rate is reported per level.
- **Rendering and freezing:** the prompt, the rendering of an observation as text, and the
  parser are committed before any scored call. Report the measured tokens per reference against
  the stream's declared 20.

**Pilot (not scored).** 10 questions on a separate seed range (29000 upward), at m = 0 and
m = 400. It measures:

- prompt and answer throughput, to size the work;
- the parse rate.

The pilot decides two things before the main run:

- **N:** questions per level, at most 120, chosen so that the whole run fits in 4 hours of wall
  time on cores 0-2.
- **Whether to use the fallback model:** if pilot accuracy at m = 0 is under 0.2, switch to the
  fallback model before the main run. The rule and the reason are recorded.

Prompt wording may be fixed after the pilot, with every change recorded, but never after a scored
call.

**Recording.** Every call's prompt hash, full response, parsed answer, latency and token counts
are written at the boundary to `artifacts/runs/r8-*/calls.jsonl`. A call is never repeated to
replay state. The analysis reads only that file.

**Estimate.** Accuracy `A(m)` is the share of correct answers at each level. The fit is
`A(m) = p0 + (A(0) − p0) · exp(−δ · m / 100)`, by least squares over the five levels, with `p0`
fixed at the control's estimate. δ̂ gets a 90% cluster bootstrap over incidents (10,000
resamples, seed 9800), refitting in every resample. A nonparametric reading, `A(m)` with intervals
per level, is reported beside it. If the exponential misfits visibly, that is reported, and δ̂ is
then described as a summary, not a law.

**Criterion, fixed by the coordinator before any R8 code or call (2026-10-05).**

- **Identifiability precondition:** on hard incidents, A(0) ≥ 0.35, with its 90% lower bound above
  the control's estimate plus 0.15. If it fails for both models, the outcome is **unidentifiable
  with these models**.
  - In that case, δ̂ on plain incidents is reported as a labelled proxy and no regime is claimed.
- **R6 regime:** the 90% upper bound of δ̂ is below 0.05. Under this model, simple builders
  capture the context lever. EXP-102 claims references or cost at matched quality.
- **R7 regime:** the 90% lower bound of δ̂ is above 0.10. Under this model, choosing references
  has quality headroom. EXP-102 claims quality and references together.
- **Unresolved:** anything else, including an interval that straddles 0.05 to 0.10. A point
  estimate between the regions is reported as such.

Feasibility: δ̂ is unconstrained by the design, so every outcome is reachable. Before the main
run, the worker computes the half-width of δ̂'s interval implied by the pilot's accuracy and the
chosen N. If that half-width exceeds 0.05, so that neither regime could be claimed even at a
favourable point estimate, it says so in writing before the main run.

**Reported beside the criterion:**

- A(m) on plain incidents.
- Accuracy by the position of the first decisive reference: first, middle or last third.
- Parse failures per level.
- Tokens per reference.
- Wall time per call against context length.

**Resource envelope.**

- Model 1–2 GB.
- Build under 1 GB.
- Run outputs under 200 MB.
- At most 4 hours of inference.
- The worker checks free disk (at least 4 GB) before the download.

### R9 Grounding the distractor penalty in the world (before EXP-102 is preregistered)

R7 showed that R6's conclusion depends on δ, the loss of informed probability per 100 references
that are not evidence. R8 could not measure δ on a real model. R8 did show something else: in this
world, references that are not evidence include look-alikes. The hard kinds' decisive messages
draw their ids from the 48-id pool that 70% of background free-form messages also use
(`HIDDEN-DESIGN.md`, sections 7 and 9). A program that applies the hard families' rules fell from
0.93 to 0.36 as distractors rose from 0 to 400. The simulated reasoner at δ = 0 counts `q` from
hidden labels, so it is a reader that always knows which references are evidence. No reader
without the labels has that ability.

R9 replaces the free δ with a measurement: the loss suffered by the strongest reader that has the
reasoner's knowledge (the hidden rules) and nothing the reasoner would not have (no labels). It
then reruns R7's comparison at that δ. It does not change the reasoner's law.

**The reader.** A deterministic program, in `experiments/exploration/scripts/r9_reader.py`,
evaluator-side like `r8_rules.py`. It is not an arm, not a baseline, and never enters a policy.

- **It may use:** the context's observations at full time resolution, the public graph, the first
  world's public physics, and the hard families' rules (`HIDDEN-DESIGN.md` sections 4 and 4.2),
  including timing structure (the partner's alarms 20–230 ms after the first; phase-2 evidence in
  `[6 s, 16 s)`; `MixedSignals` at two services; the leak's ramp), counters, probe results in the
  context, and repetition of ids within the context.
- **It may not use:** hidden labels, tiers, the pool's per-stream vocabulary assignment, or any
  stream state outside the context. It answers from the context alone, as the reasoner does.
- **Development and evaluation are separate.** The reader is built and tuned on questions from
  seeds 29000–29999 only. Its evaluation questions come from seeds 30000 upward. Once the first
  evaluation number is computed, the reader is frozen; a later change starts a new evaluation on
  seeds 32000 upward and both are reported.
- The worker builds the strongest reader it can in bounded effort, states what it tried, and
  records that a stronger reader would give a smaller δ.

**Questions.** From R8's dumper (`crates/gordian-stream/examples/questions.rs`), regimes off, with
one change: the pool holds every other observation within ±40 s of the focus except the decisive
ones, **including the focus incident's own non-decisive observations**, because the simulator's
`m` counts those and a window builder carries them. Add this as a dumper option, keep the old
behaviour as the default, and test both.

- Hard incidents, slow leak excluded, at least 200 questions, each from a distinct stream segment
  where possible; cluster by stream.
- Plain incidents, 100 questions, as a secondary set.
- Levels m ∈ {0, 50, 100, 200, 400}, nested as in R8 (one seeded permutation per question).
- Control: q = 0 at m = 50.

**Estimate.** As R8: A(m) per level with 90% cluster bootstrap over streams; δ* fitted by least
squares to `A(m) = p0 + (A(0) − p0)·exp(−δ·m/100)` with `p0` the control's estimate; 90% cluster
bootstrap of δ* (10,000 resamples, seed 9900). The misfit of the exponential is reported: the
largest absolute residual over the five levels.

**Rerun.** R7's comparison at δ ∈ {δ*_lo, δ*, δ*_hi} (the interval's bounds and the point
estimate), at the primary setting (b = 5, ρ = 0.7), using R7's manifest writer, tuning and
held-out streams, selection rule and statistics unchanged. If a value coincides with R7's grid
to within 0.005, R7's run is reused and named. Three tuning and three held-out runs.

**Criterion, fixed by the coordinator before any R9 code or run (2026-10-06).**

- **Reader precondition:** on hard evaluation questions, A(0) ≥ 0.80 with its 90% lower bound
  above 0.70. If it fails, the outcome is **reader too weak**, δ* is reported as labelled, and the
  rerun still happens but decides nothing.
- **R6 regime:** at δ*_hi, the 90% upper bound of G is below 0.10 (G as R7 defines it: the
  context-only ceiling minus the tuning-selected builder).
- **R7 regime:** at δ*_lo, G ≥ 0.10 with its 90% lower bound above 0.05.
- **Unresolved:** anything else.

Feasibility: R7 measured G(0.05) = 0.097 [0.066, 0.129] and G(0.1) = 0.137 [0.104, 0.170], and G
rises with δ, so the R6 regime needs roughly δ*_hi < 0.05 and the R7 regime roughly δ*_lo > 0.1.
R8's crude reader gave δ̂ ≈ 0.44–0.85; a strong reader may land anywhere below that. Every
outcome is reachable, and one privilege separates ceiling and comparator in the rerun.

**Reported beside the criterion, never folded in:**

- **The direct check on real contexts.** R6's diagnostic run (`artifacts/runs/r6/r6-diag-b5-rho0.7`,
  every ledger kept) has 199 calls about hard incidents for each of seven arms, with the full
  context of each call. `r6_trace.py` already reconstructs those contexts. Run the reader on each
  call's context and report its accuracy per arm, against the accuracy predicted from the fitted
  curve at that arm's mean references per call. This tests whether a count-based δ transfers from
  randomly drawn distractors to the contexts the builders actually produce.
- A(m) on plain incidents, and δ* on them as a labelled proxy.
- The reader's accuracy by family and by mode (mimic, contradict) at each level.
- Which distractors the reader is fooled by: the share of wrong answers at m = 400 that a
  look-alike free-form message explains, from the reader's own trace.
- Critical misses and plain accuracy for every row of the rerun.

**Resource envelope.** The reader is Python on the dumped questions and runs on cores 0-2 under
`scripts/cgroup-run.sh`. The reruns go through the driver: about 15 minutes. Another worker (R10)
shares cores 0-2; see "Sharing cores" below.

### R10 Salience ceiling: what noticing is worth (before EXP-101 is preregistered)

R6's review found that the selection-oracle arms call only about anomalies the shared rung
noticed, and the rung never notices 34 of 372 hard non-leak incidents at the primary setting
(0.091 of hard-incident quality if every one were answered). The slow leak is worse: the rung
notices it only after the threshold crossing, and the context-only ceiling scores 0.28 on it
against R4's oracle's 0.91. EXP-101 is about escalation control, which starts with noticing. R10
measures what perfect noticing is worth, as one privilege.

**The arm.** `oracle_notice_privileged`, built by `OracleFactory` in `stream/oracle.rs` beside the
other privileged arms, private rule, truth through the plan only.

- It is `oracle_selection` plus one privilege: for every hard incident it injects a noticed
  anomaly anchored at the incident's first observation, at the step that observation is delivered,
  with the site that observation names. `PlanIncident::first` already carries it.
- Everything else is the selection oracle's: escalate exactly the hard incidents, once each,
  `delay_ns` after notice, with the context the rung's configured builder makes; the default hold
  on declaration; the rung's own later notice of the same incident is ignored for escalation.
- No other privilege: it does not read decisive labels, does not choose the instant beyond
  notice + delay, and builds no context of its own.
- The comparison arm is `oracle_selection_privileged` with the same builder and delay. The
  difference is noticing alone: the incidents the rung never noticed, and the earlier anchor for
  those it noticed late.
- R4's oracle is a reference row.

**Pairings.** Context held fixed at the rung's own context (primary: EXP-101's rung), and at
`window` 40 s, N 256 (secondary: R6's held-out best at δ = 0). Delay as R5 tuned it (16 s at the
primary setting; R5's per-setting values elsewhere). δ = 0 (the current reasoner default).

**Criterion, fixed by the coordinator before any R10 code or run (2026-10-06).** Two separate
results, each named, neither an "either":

1. **Salience headroom on hard incidents, slow leak excluded:** at the primary setting (b = 5,
   ρ = 0.7), with the rung's context, `oracle_notice` minus `oracle_selection` in hard-incident
   quality is at least 0.03, with the 90% paired cluster-bootstrap lower bound above 0.01.
2. **Salience headroom on the slow leak:** the same difference on slow-leak incidents alone is at
   least 0.20, with the lower bound above 0.10.

Feasibility: result 1 is bounded above by about 0.09 (the never-noticed share) plus whatever
earlier anchors add, and the selection oracle with the rung's context answers about half of what
it is asked, so a gain near 0.045 is plausible and the margin can go either way. Result 2 is
bounded above by about 0.6 (0.91 − 0.28) and the context-only ceiling shows the leak is
answerable once asked. One privilege separates ceiling and comparator. Sensitivity at b = 2.5 and
b = 8, ρ = 0.7, reported.

**Reported beside the criterion, never folded in:**

- The decomposition of result 1 by whether the rung ever noticed the incident (from the ledgers of
  a diagnostic run with ledgers kept, 100 streams): gain on never-noticed incidents against gain
  on late-noticed ones, with counts.
- The rung's notice latency (notice instant minus first observation) per family and mode, and the
  list of never-noticed incidents by family and mode.
- Critical misses, plain accuracy, calls and cost per stream for every row.
- **A public threshold sweep.** The rung's notice threshold (`RungConfig`) at three values below
  the default, one run each, with `oracle_selection` and the rung's context: for each, the share of
  hard incidents noticed, false notices per stream (anomalies anchored on background or plain
  observations, from the ledgers), hard-incident quality, calls and cost. This says whether the
  gap is a tuning matter or needs a mechanism.

**Resource envelope.** About eight held-out runs of a few arms each (two pairings at three
settings, plus the sweep) and one diagnostic run: under 30 minutes through the driver. Shares
cores 0-2 with R9; see below.

### Sharing cores 0-2 between concurrent workers

Two workers may hold worktrees at once. The driver refuses to start a run while a `cargo` or
`rustc` process exists, but nothing stops a build from starting during another worker's run. So:

- Before any `cargo build`, `cargo test` or `cargo clippy`, a worker checks for a running
  `gordian-run` process (`pgrep -f target/release/gordian-run`). If one exists it waits, polling
  every 30 s, until none does, then builds. It records in its report every time it waited.
- A worker never kills another worker's process.
- A worker keeps its scratch files in its own subdirectory of the session scratchpad, named by
  its worktree, and never writes to another's (a PI's helper was overwritten by another lab).
- Python analysis that takes more than a minute runs under `scripts/cgroup-run.sh` on cores 0-2,
  like a measurement, and is subject to the same check.

## 6. Stage B: exploration runs

Development runs. No hypothesis is tested; nothing here may later be cited as confirmation.

| Item | Procedure | Output |
|---|---|---|
| B1 Variance | Every A6 policy, 500 episodes per class, seeds 1000-1499, default limits | `artifacts/runs/b1-*`; a table of mean and standard deviation of success and total cost per (policy, class) committed as `experiments/exploration/b1-variance.csv` |
| B2 Power table | `gordian-analyze power` on B1's variance for margins 0.01 success and 0.20 cost at 80% and 90% power | `experiments/exploration/b2-power.md`; its numbers are the sample sizes in every later freeze |
| B3 Stress suite | Every policy through every stressor class. Record failures. Fix instrument defects; never tune a baseline to pass | `experiments/exploration/b3-stress.md` listing each (policy, stressor, outcome) |
| B4 Oracle gap | Best baseline vs oracle per class from B1 | If the gap is under 0.10 success on every class, revise A1 before anything is frozen; record the decision |

Resource envelope: B1 is the heaviest. Five policies × 11 classes × 500 episodes = 27,500 episodes
per policy. Run policies sequentially, each under the runner with 3 cores and 2 GB. Measure one
policy's wall time on 100 episodes first and extrapolate before launching the rest.

## 7. Stage C: the first result (EXP-001, heuristic components only)

| Item | Deliverable |
|---|---|
| C1 Preregister | `experiments/EXP-001-selective-activation/EXP-001.md` from the template. Primary comparison: paired difference against `fixed_pipeline` in verified success (margin from the charter's proposed 0.01, confirmed or revised by B2) and in total bill (margin 0.20). Critical-miss bound stated separately. Sample size from B2. Exchange rate or hard-constraint treatment for each resource stated |
| C2 Selector | `policy/selective.rs`: task-conditioned score per component from cheap features (novelty, goal relevance, declared cost), explicit stop rule, hard limits. No learning |
| C3 Freeze | Commit the preregistration; write the commit hash into its freeze record; change status to `frozen` |
| C4 Execute | Held-out seeds 5000+ on dependency structures not used in Stage B. One arm at a time via the driver. Each arm's `usage.json` and `results.csv` hashes recorded |
| C5 Report | `gordian-analyze compare`; category, intervals, per-class table, stress outcomes, every excluded episode with reason, replay diff of a re-execution from the manifest |

If C5 is "harmful" or "equivalent," the charter's falsification rule applies: the selective
mechanism is simplified or removed before Stage D. That is a result, not a setback.

## 8. Stage D: selection mechanisms (EXP-002, EXP-003)

| Item | Deliverable |
|---|---|
| D1 EXP-002 arms | `policy/novelty_threshold.rs`, `policy/weighted_score.rs`, `policy/conditioned_score.rs`, `policy/learned_selector.rs`. The learned selector is a logistic model or a one-hidden-layer MLP under 10,000 parameters, trained on Stage B traces plus counterfactual forks at a preregistered fork rate; fork cost charged to the training bill |
| D2 EXP-002 ablations | Feature flags on `conditioned_score` removing novelty, uncertainty, goal relevance, reliability, history one at a time |
| D3 EXP-003 variants | Three `Boundary` implementations behind one trait: `SharedState`, `SemanticOnly`, `MixedWithRefs`. Three predefined replacements: swap the estimator, swap the heuristic's rule table, add a new sense |
| D4 EXP-003 measures | Per replacement: consumers changed (count from the diff), adaptation data (bytes of calibration input), validation effort (test count and runtime) |

Training resource envelope: one job at a time, cores 0-2, 6 GB, run alone. Training data stays
under 1 GB on disk.

## 9. Stage E: learned state and prediction (EXP-I01, EXP-004, EXP-005)

Learned components are linear or GRU-class models under one million parameters, trained on CPU.
Every learned arm is trained 5 times with different seeds; the results table carries the training
seed, and the analysis reports across training runs, not within one.

| Item | Deliverable |
|---|---|
| E1 EXP-I01 | 2×2: `{fixed_window, budgeted_retrieval}` × `{fixed_pipeline, selective}`. Preregistered before E2 |
| E2 EXP-004 | `memory/fixed_window.rs`, `memory/recurrent.rs` (tiny GRU), `memory/retrieval.rs` with a read budget, `memory/shuffled.rs` control. Episode classes `DelayedConfigChange` and `StaleMemory` are primary |
| E3 EXP-005 | `dynamics/table.rs`, `dynamics/learned.rs`, and a matched model-free arm. Primary measure is decision regret against the oracle's earliest-correct decision, not prediction loss |

## 10. Stage F: execution efficiency (EXP-006 tiny, EXP-007 int8)

Only if Stages C to E produce at least one "beneficial" mechanism.

| Item | Deliverable |
|---|---|
| F1 EXP-006 | On the Stage E models: shallow fixed, deeper fixed, adaptive recurrence with a stopping rule, repeated execution without recurrent state. Parameter-matched and compute-matched runs. Every output labelled "tiny scale, CPU" |
| F2 EXP-007 | float32 reference vs int8 using VNNI through a small hand-written kernel or a crate justified under section 4's table. Measures: resident memory, state memory, latency, task quality. Ternary deferred |

## 11. What is explicitly not claimed from this machine

- Any scaling statement about recurrent depth.
- Numerical replay across backends. Only "same CPU, same flags" is available.
- Ternary quantization results beyond trivial sizes.
- Anything about a second domain until the second simulator exists, written without shared code.

## 12. Conventions

- Branches: short and descriptive (`small-world`, `evaluator`, `cost-bill`, `exp-001-prereg`).
- Every run directory: `artifacts/runs/<YYYYMMDD-HHMMSS>-<exp>-<arm>-<seedrange>/` with
  `manifest.json`, `results.csv`, `usage.json`, optional `events-sample.jsonl`. The directory is
  git-ignored; the SHA-256 of `manifest.json` and `results.csv` go in the experiment file.
- Before every push: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, and for the Python package `python -m pytest analysis`.

## 13. Order of the next branches

1. `small-world`: A1.
2. `evaluator`: A2.
3. `cost-bill`: A3 and A4.
4. `components`: A5.
5. `baselines`: A6, which also lands `.cargo/config.toml` and the release profile.
6. `analysis`: A7.
7. `exploration`: B1 to B4, committed as the three files under `experiments/exploration/`.
8. `exp-001-prereg`, then `exp-001-run`.

Each branch merges to main when its acceptance commands pass on this machine.
