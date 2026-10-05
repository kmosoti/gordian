# gordian-run: harness record

Work item A4 of `docs/local-test-plan.md`: the episode loop, the recorder, the driver. This file
states what the loop does, which budget enforces which resource, what is recorded where, and every
place the build departs from or fills a gap in the plan. What is built and what is not is stated
in section 9. The code is authoritative; this file explains it.

## 1. The loop

`harness::run_episode(spec, policy, components, limits) -> Result<EpisodeRecord, HarnessError>`.
One function; nothing else generates an episode or builds the evaluator's truth (a test checks
the source).

Setup, once per episode:

1. `generate(spec)`, then `Truth::from_episode` immediately. Truth is kept aside. What the harness
   needs from the episode (harness directives, the public stream, the horizon, seed and class) is
   copied out, then the episode moves into a `Simulator`. After that nothing but the simulator
   holds it, and no `Episode` is serialized anywhere (its serde output includes hidden state; see
   `docs/review-log.md`, A1).
2. One fresh `Budget` from the limits, one `Bill` over it, one `Ledger`, one `ManualClock`. None is
   reused across episodes and nothing is pre-spent (review log, A3).

Per step:

| # | What | Ledger |
|---|---|---|
| 1 | `sim.observe_until(now)`, plus the result of the previous probe or correction, merged in instant order and admitted into the `WorkingState` | one `Measurement` per observation, producer `harness/sensor`, stamped with the admission time; the observation's own instant is in the payload |
| 2 | charge `policy.declared_select_cost` under `Phase::Scheduling`; if accepted, advance the clock by its declared `Time` and call `policy.select` | `Accounting` (accepted or refused); one `Measurement` from `harness/timer` (declared-cost call plus `select`) |
| 3 | for each selected component, once: charge its declared cost under `Phase::Component(id)`; a refused charge means it does not run; an accepted one advances the clock by its busy time (its declared `Time`, or, when it declares none, its declared `Compute` nanoseconds; see 5), then runs (or fails, see 5) | `Accounting`; the component's entries as it produced them (`Hypothesis`), its requests (`ComputationRequest`); a `harness/timer` `Measurement` per run |
| 4 | `policy.decide(state, outputs)` | a `harness/timer` `Measurement`; `Decision` for an action |
| 5 | apply the action (section 2 for how the budgets meet) | `Outcome` (the simulator's, or a bill refusal); `Accounting` under `Phase::Sensing` for a carried-out probe or correction |
| 6 | keep the harness-owned parts of the working state: the top (at most eight) scored hypotheses of the latest scored output, and the pending requests (a request for another component stays pending until that component has run) | |
| 7 | a step lasts at least `step_ns` of logical time; stop checks | |

Stop, at the end of a step, in this order: a terminal outcome (checked first, `Terminal`); the
clock past the horizon (`Horizon`); no affordable work left (`BudgetExhausted`); the step cap
(`StepCap`). The stop reason is a column of `results.csv`. For the horizon and for no affordable
work the policy then gets its final call (below) and the loop stops after it. `Terminal` and
`FinalDeclaration` are decided episodes; every other stop means the arm never decided and the
verdict is `undecided` (evaluator R9).

*No affordable work* means: after paying the policy's declared scheduling cost, no component's
declared cost fits, and no probe and no correction fits. Declaring and abstaining are free and do
not count: an arm that can no longer compute or sense is out of means, even though it could still
declare. The policy has already had its `decide` call for that step. It is a stronger rule than
"the budget is spent": `Budget::exhausted` is true as soon as any declared resource reaches zero.

**The final call (A6b).** Because declaring is free, "out of means" must not mean "undecided": an
arm that ran out of affordable work before its own deadline would otherwise be scored on how long
its budget lasted and not on what it concluded (`docs/review-log.md`, A6, headroom probe). So when
the loop would stop for `Horizon` or `BudgetExhausted`, it first calls
`Policy::declared_final_cost` and `Policy::decide_final`, once. What happens, in order:

1. *Sensing.* The passive observations that arrived by `min(clock, horizon)` and the results of
   earlier probes that are ready by then are admitted into the working state, as at the start of
   any step, so that a probe bought in the last step is not wasted on a policy that cannot look at
   it. A result that is ready only after the horizon is not shown. `state.now` is that instant.
2. *Charge.* The declared final cost is charged under `Phase::Scheduling` at the clock, like any
   `decide`. **If the bill refuses it, the refusal is recorded (a refused `Accounting` entry) and
   the call is made anyway.** Declaring is free; making the verdict depend on whether the last few
   microseconds of compute remained would reintroduce the artifact the call removes. If accepted,
   the clock advances by the declared `Time`, if any (the shared rule declares none). The shared
   rule declares its ordinary cost without the probe-evaluation term, because the call never
   scores a probe (`POLICIES.md`, section 3).
3. *Decide.* `decide_final` is timed with the declared-cost call and recorded as one `decide`
   `Measurement`, so the `select` and `decide` entries still sum to `measured_sched_ns`. The call
   is not a step: `steps` does not count it.
4. *Act.* Only `Declare` and `Abstain` are carried out. Any other action is recorded as a
   `Decision` followed by an `Outcome` from `harness/final` (`{"refused": "final"}`), and not
   sent to the world, because nothing could follow it. The terminal step enters the trajectory
   like any other and is scored by the ordinary rules; `decision_at` is its instant.

*Its instant.* `min(clock, horizon)`, and never before the last trajectory step (evaluator R15).
The world refuses anything after the horizon, and at a `Horizon` stop the clock is already past it
(the step that crossed it ended at the next quantum), so the call is made *at* the horizon. Ledger
entries are stamped with the clock, which cannot go backwards, so for that stop the `Decision` and
`Outcome` entries are stamped later than the trajectory step's instant. If the last ordinary step
was itself refused as past the horizon (a component's busy time carried it over), the final call's
instant cannot be earlier than that and the world refuses it too: the episode stays undecided with
its original stop reason. Not a harness error, and rare (a test pins it).

*Stop reasons.* `final_declaration` means the terminal step came from the final call, and only
that. If the final call returns `None`, or an action that is not terminal, or the world refuses it,
the stop reason stays `budget_exhausted` or `horizon`: those now say "the arm had its final call
and did not close the episode". `terminal` means the episode closed at an ordinary step, so at
default limits, where every arm decides before it runs out of means, `results.csv` is byte for
byte what it was before the final call existed (checked: section 8, item 15).

*The step cap gets no final call.* It is a runaway guard, not a decision point. `Horizon` and
`BudgetExhausted` are facts about the episode's time and the arm's means, and an arm that hits
either did not decide *because it ran out of something*; the final call exists to remove that
artifact. The cap says nothing like that: the default cap (1000 steps of at least 50 ms) is five
times the number of steps that fit in the horizon, so an arm reaches it only by a configuration
that shortens the step quantum, or a policy that makes no progress. In either case the useful
output is a visible `step_cap`, undecided, row. A final call there would turn a runaway into an
ordinary-looking declaration. When the cap and the horizon coincide, the horizon (checked first)
wins and the call is made.

*The oracles.* A final call is the arm's own deadline arriving. `oracle_immediate` declares the
truth, as at every call (it never reaches the final call: it declared at step 1).
`oracle_evidence` declares if the public evidence identifies the truth and otherwise abstains,
exactly as at its patience deadline; it does not buy a probe and does not use the hidden state to
declare. Neither changes at default limits.

The scored trajectory is exactly the steps the simulator answered, in order. The loop ends at the
terminal step, so no attempt after the close exists to record (evaluator R14). `gordian_eval::score`
errors are returned as `HarnessError::Eval` and abort the run; they never become a row.

## 2. Two budgets, one bill

The simulator enforces a probe and time budget from the episode spec (`EpisodeSpec::budget`). The
`Bill` enforces whatever the harness charges. Both exist, so the question is which resource lives
where, and how to avoid counting a unit twice or not at all.

**Decision.** The `Bill` is the authority for every resource. The manifest's `limits` declare all
five (Compute, Memory, Time, Probes, Communication). The simulator's budget is derived from the
same numbers, `limits.probes` and `limits.time_ns`, and the harness *refuses* an episode spec whose
`budget` differs (`HarnessError::BudgetMismatch`); it does not overwrite it silently.

| Resource | Spent by | Enforced by | Charged to |
|---|---|---|---|
| Compute | components, policy scheduling | Bill | `Component(id)`, `Scheduling` |
| Time | declared `Time` of components and scheduling; latency of probes and corrections | Bill (and the simulator for actions) | `Component(id)`, `Scheduling`, `Sensing` |
| Probes | probes and corrections | Bill (and the simulator) | `Sensing` |
| Memory, Communication | nobody yet; declared so a charge can be refused rather than undeclared | Bill | whoever declares them |

How an action is applied:

1. Dry-run its declared cost (`action.cost()`) against the bill's budget. If the bill would refuse
   it, the world is not asked: the refusal is recorded through `Bill::charge_recorded`, which
   appends a *refused* `Accounting` entry, followed by an `Outcome` entry from `harness/bill`. No
   step enters the trajectory.
2. Otherwise `sim.apply`. A refusal by the world for another reason (unknown service, past the
   horizon) charges nothing anywhere and is a refused step in the trajectory (evaluator R12).
3. If the world carried it out, the same cost is charged to the bill under `Phase::Sensing`. The
   harness checks that the world's reported cost equals the declared cost, and treats a bill
   refusal here, or a world refusal for budget after the bill accepted, as `BillDisagreement`: a
   harness bug, not a result.

Why this is not double counting: the bill and the simulator each see an action's cost once, and
the bill's Sensing phase equals what the world charged (tested against the trajectory's `Outcome`
costs). Why nothing is uncounted: component and scheduling time reduce the same `Time` limit the
probes use, so the bill can be tighter than the simulator, never looser; the simulator's own check
is a second line that cannot fire first.

**Time is busy time.** `Resource::Time` counts logical time the arm spends: declared component and
scheduling time, and the latency of probes and corrections. Waiting is not billed. A policy that
waits for observations pays nothing for the wait; if experiments should charge delay, that is a
preregistered exchange rate (charter section 4), not something this loop assumes.

A consequence of sharing `Time`: with default limits (250 ms) a component that declared 50 ms of
`Time` would eat a fifth of the probe time. None of the A5 components declares `Time`; their
declared `Compute` nanoseconds are their time on the clock (section 5), but they are not billed
under `Resource::Time`, so the `Time` limit stays the probes' and a `Slow` directive moves the
clock and the `Compute` bill without touching the probe time.

## 3. The logical clock

`ManualClock`, moved only by the harness:

- by a component's busy time when its charge was accepted: its declared `Time`, or, for a
  component that declares no `Time` charge at all (the A5 components), its declared `Compute`
  nanoseconds (section 5);
- by the policy's declared scheduling `Time`, when charged (its declared `Compute` does not move
  the clock);
- by a probe's or correction's latency (`ready_at - now`), after the action;
- at the end of a step, up to `step_start + step_ns` if it is not there yet.

So a step takes `max(step_ns, busy time)`. `step_ns >= 1` is required, so time always advances. The
result of a probe is delivered at the next step, merged by instant with whatever arrived while
the probe ran. The default `step_ns` is 50 ms and the default horizon 10 s, so an idle policy runs
201 steps (tested).

## 4. Measured cost is the primary cost

Declared cost is what policies see and what the `Bill` enforces. The charter's cost `C` is the
*measured* wall time (`docs/review-log.md`, A5; plan A4).

- `std::time::Instant` brackets each `Component::run` and each scheduling call. This is the one
  wall-clock read in the system, because it is a recorded boundary effect.
- Each timing is appended as `EntryKind::Measurement` from producer `harness/timer`, never as
  `Accounting`, so `Bill::replay` is undisturbed.
- Nothing the loop decides reads a timing, and a policy is never shown one.
- Per episode, three sums go to `measured.csv`: `measured_component_ns` (the component runs),
  `measured_sched_ns` (the policy's declared-cost, `select` and `decide` calls, exactly the
  `select` and `decide` timer entries), and `measured_harness_ns` (the rest of the episode's wall
  time: generation, simulator, charging, ledger, scoring, and the affordability check). A test
  checks that the three sum to no more than the caller's own wall time, and that the first two
  equal the timer entries in the events sample.
- A component a `Fail` directive made produce nothing is charged but not run, so it has no timing.

Per-class declared-to-measured ratios are the analysis package's job, from the two CSVs.

`internal_external_ratio` is the sum of the three measured columns divided by the runner's
`cpu_ns`. The harness does not include process start-up, manifest parsing, or writing the files, so
the ratio is expected to be a little under 1; its first measured value is the starting point of
the tolerance (plan A4), and is in the unit's report.

## 5. Harness directives

`ComponentDirective { component: i, mode }` maps to `ComponentId(i)` when a component with that id
is part of the run. An index with no such component is ignored and counted in `directives_ignored`
(`results.csv`). The world draws indices 0 to 7 and the A5 components are 0 to 3, so about half of
`ComponentTimeout`'s directives are ignored with the real components.

- `Fail`: the component is charged (so the bill moves) and its `run` is not called. The policy
  gets no entry for it in `outputs`. The ledger records a `ComputationResult` with the payload
  `{"component": i, "output": "none"}`, which does not say why: the events sample must not hold the
  directive.
- `Slow { factor }`: every declared `Resource::Time` and `Resource::Compute` amount of the
  component is multiplied by `factor`, in the charge (so the ledger and the bill hold the
  multiplied amounts) and, because the clock advances by the charged busy time, in the clock
  advance. Other resources are not multiplied.

  *Why both.* The coordinator's A4 review decision: a component's declared `Compute` nanoseconds
  are its time. The A5 components declare only `Compute`, so a directive that multiplied only
  `Time` changed nothing for them. *The clock rule that follows.* A component's busy time is its
  declared `Time` if it declares any `Time` charge (even a zero one), else its declared `Compute`
  nanoseconds. That applies to every component, slowed or not, so with the A5 components the
  clock now moves by a few microseconds per component run, where before it did not move at all.
  Steps are 50 ms, so no step boundary changes, but a decision made in the same step as a
  component run is stamped a few microseconds later than before. A test with the real A5
  components shows `Slow` changing the `Compute` bill and the clock by exactly `factor`
  (`tests/harness.rs`, `slow_changes_the_bill_and_the_clock_of_the_real_components`). The
  alternative reading, that only a slowed component's compute counts as time, would make an
  unslowed component instantaneous and a slowed one not; it was not taken.

## 6. What is recorded where

| File | Holds | Deterministic |
|---|---|---|
| `manifest.json` | the canonical manifest | yes |
| `results.csv` | one row per episode, plan columns then `directives_ignored`, `stop_reason` (column semantics: `src/results.rs`) | yes, byte for byte |
| `measured.csv` | the three measured sums per episode | no, same keys |
| `events-sample.jsonl` | for a hashed sample of episodes: the public information, the passive stream, every ledger entry | except `harness/timer` payloads |
| `usage.json` | written by `scripts/cgroup-run.sh` into the run directory, then extended by the driver with `internal_external_ratio` and the tolerance verdict | no |

Rows are keyed by `(seed, class)` and written in execution order: the manifest's classes as listed,
each over the first `count` seeds as listed. A manifest may not repeat a seed or a class.

**The events sample** is chosen by a hash of `(seed, class)` and `trace_sample_rate` alone, so
every arm of an experiment keeps the same episodes. It contains no verdict and no hidden state.
Two tests check that: a list of hidden-state key names (the fields of the world's `Hidden`,
`Fault`, `ComponentDirective` and `StreamLabel`) must not occur, and the set of all keys in the
file must be a subset of an allowlist, so a new key fails the test until it is reviewed. The
`class` on each line is the join key to the CSV files, not a policy input; anything that trains on
the file must drop it. The sample is not small: a sampled episode of `heuristic_only` is about
100 KB, and a policy that runs every component at every step would be several times that, so the
default `trace_sample_rate` of 0.01 matters.

**Undecided episodes keep their row.** `decision_at_ns` is empty, not zero and not the stop time.
(The A7 loader currently rejects an empty `decision_at_ns`; see section 8.)

## 7. Reproduction claims

*Protocol replay.* The same manifest gives a byte-identical `results.csv` (tested), and a ledger
whose entries are identical except the payloads of `harness/timer` entries (tested). It does not
claim the same wall times. The ledger's accounting replays to the live bill
(`Bill::replay`), tested over every class and two policies.

*Not claimed.* Numerical replay and statistical replication (charter section 11) are out of this
unit's scope.

## 8. Departures from the plan and the specification, and gaps

1. **`run_episode` returns `Result`.** The specification's signature returns `EpisodeRecord`. An
   evaluator error "must fail the run loudly"; a `Result` does that without a panic and lets the
   recorder name the seed and class.
2. **Extra manifest field `episode_params`** (horizon, noise rate, service counts, `k`). The plan's
   manifest has none, but the generator's parameters determine the data and belong in the record.
   `limits` is a serializable struct (`Limits`), not `gordian_core::Budget`, which is not
   serializable; it carries the five limits plus the working-state window, the step quantum and
   the step cap.
3. **Extra `results.csv` columns** `directives_ignored` and `stop_reason`, appended after the plan's
   nineteen. The first records the "ignored and recorded" requirement for directives; the second
   says why an episode was undecided. The plan's columns keep their order and names.
4. **`bill_*` columns partition the bill.** `bill_storage` is everything attributed to
   `Phase::Storage` (the `Resource` enum has no Storage), and the other five columns exclude it, so
   the six sum to the whole bill and a total over columns counts nothing twice. Nothing charges
   `Storage` in Stage A, so it is 0.
5. **`usage.json` is not written by the recorder.** The plan says the recorder copies the runner's
   report; the runner writes it after the recorder has exited. The driver has the runner write it
   straight into the run directory and extends it.
6. **A refused-by-bill action is not in the trajectory.** The world was not asked, and putting a
   `Refused` outcome in the trajectory would invent an answer the world did not give. The refusal
   is in the ledger. Scores are the same either way (evaluator R12).
7. **`Slow` had no effect on the A5 components. Resolved in A6.** The specification multiplied
   *declared Time*, and the A5 components declare only `Resource::Compute`. The coordinator
   decided (A4 review) that a component's declared `Compute` nanoseconds are its time, so `Slow`
   now multiplies `Compute` and `Time`, and the clock advances by the multiplied compute
   nanoseconds (section 5 states the rule and what else it moves). The earlier test that asserted
   `Compute` is not multiplied was changed to assert that it is.
8. **`Resource::Time` is shared** between probe latency and component and scheduling time (section
   2). Harmless until a component declares `Time`; the A5 components do not, and their compute
   nanoseconds move the clock without being billed as `Time`.
9. **A7's loader and `bill_total`.** `analysis/gordian_analysis/load.py` requires a finite number in
   `decision_at_ns` and would reject any undecided row; it also defines `bill_total` as the sum of
   the six bill columns, which adds nanoseconds to probe units and bytes. Neither is changed here.
10. **The oracle-link assertion at startup** (plan A1: "`run` asserts at startup that no arm links
    the symbol") is not built. The evaluator enables `reveal-hidden-state` for the whole build,
    and cargo unifies features, so a runtime or build-time check in this binary cannot tell an arm
    that links the accessor from one that does not. The textual guard is the check:
    `scripts/check-no-oracle.sh` now checks `crates/gordian-run/src/policy/` more strictly than the
    rest of the tree (no `gordian_eval`, `Truth`, `Episode`, `Simulator`, `reveal`), except
    `policy/oracle.rs`, which A6 built (see `POLICIES.md`, section 6).
11. **Default limits are provisional.** `Limits::default()` (20 ms compute, the world's 12 probes
    and 250 ms, 50 ms steps, a window of 256, a cap of 1000 steps) is a placeholder for exploration
    runs, not a preregistered budget.
12. **Defaults for `heuristic_only`.** Its 3 s patience was a choice for a smoke-test policy. A6
    replaced its own rule with the rule every arm shares; the 3 s is now that rule's patience, in
    the manifest's `decide` section (`POLICIES.md`).
13. **A privileged entry point (A6).** `run_episode_privileged` takes an `OracleFactory` instead of
    a policy, and builds the policy inside the loop from the episode's truth. It is the only
    change to the loop for the oracle arms (an enum `Source` and a few lines at the point the
    truth is built). `run_episode` is unchanged for every other caller. `POLICIES.md`, section 6.
14. **The manifest's policy is an enum (A6).** `policy` is a `PolicySpec`: a bare id as before, or
    an object with parameters; `decide` is a new section with a default, so a manifest written
    before A6 still parses and means what it meant (`heuristic_only` now runs the shared rule,
    which is a change of behaviour, not of syntax).
15. **The final call (A6b); departures and choices.** Section 1 states the call. What was chosen
    where the plan was silent:
    - *Two trait methods, not a flag on `decide`.* The plan says "a final `decide` call flagged as
      final". `Policy` gains `declared_final_cost` and `decide_final`, both required, so a policy
      cannot forget to say what it does there; `Decider::decide_final` and `decide` share one
      private function whose only difference is that the deadline counts as passed, and a test
      checks `decide_final` equals `decide` at a clock at the deadline. A flag on `decide` would
      have changed every existing call site and left the cost question open: the selector's cost
      must not be charged for a selection that does not happen.
    - *The step cap gets no call, and the horizon's call is made at the horizon* (section 1).
    - *The call senses first* (section 1), a thing the plan did not specify. Without it an arm that
      bought a probe in its last step declares without looking at the answer.
    - *Measured at default limits.* Eight arms (`heuristic_only`, `all_components`,
      `random_matched` p = 0.5, `fixed_pipeline` defaults, verifier only and estimator only, both
      oracles), 20 seeds by 11 classes, through `scripts/cgroup-run.sh`: `results.csv` is
      byte-identical to the one the pre-A6b binary wrote for the same manifest, for every arm. No
      row differs, because no arm at default limits ends undecided (every stop is `terminal`). A
      permanent test (`tests/final_call.rs`) compares each arm with itself wrapped so that it
      makes no final declaration, at default and two binding budgets, and requires every
      difference to be a row that would have ended `budget_exhausted` or `horizon`.

## 9. Built and not built

Built: the loop, the policy trait, the scripted policy, the manifest, both CSV files, the events
sample, the `gordian-run` binary (`--manifest`/`--out`, and `init` to write a manifest for the
current checkout, with every policy's parameters), `scripts/run-driver.sh`, and the shell test
`tests/driver.sh`. Built in A6: the baselines (`heuristic_only`, `fixed_pipeline`,
`all_components`, `random_matched`, and the two privileged oracle arms) and the decision rule they
share; `POLICIES.md` states them. Built in A6b: the final call.

Not built: any training or forking of counterfactual episodes, the analysis of a run, and anything
that charges `Memory`, `Communication` or `Storage`.
