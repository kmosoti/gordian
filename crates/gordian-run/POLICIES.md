# gordian-run: the baseline policies

Work item A6 of `docs/local-test-plan.md`. This file states what the baselines do, the one
decision rule they share, what that rule costs, how the privileged arms get the truth, and every
place the build departs from the plan or fills a gap. The code is authoritative; this file
explains it. Everything described here is built unless a sentence says it is planned.

## 1. The constraint that shapes everything

EXP-001's intervention is the scheduling policy only: components, weights, inputs and evaluator
are fixed (charter section 6). A comparison between two arms is therefore valid only if the arms
differ in *which components they select* and in nothing that happens to those components'
outputs. So every non-privileged arm is a `Selector` (which components run at a step) wrapped in
`Arm`, which supplies the one shared decision rule in `src/policy/decide.rs`. A selector cannot
reach `decide`. `Arm` is the only non-privileged `Policy` implementation, and
`Policy::decision_rule` reports the same name for every arm. Three tests make the claim
checkable and not merely stated (`tests/baselines.rs`):

- a source scan shows that only `mod.rs` (the wrapper), `scripted.rs` (the harness's test
  instrument) and `oracle.rs` implement `Policy`, and that no other file defines a `decide`;
- every arm's `decision_rule()` is `expected-set-size`;
- a mirror policy feeds each `decide` call's inputs to a fresh reference `Decider` and asserts
  that the arm's answer is the reference's, over every non-privileged arm, all eleven classes and
  four seeds (over a thousand calls, more than fifty of them buying a probe), at the default
  limits and at two binding compute budgets, so that it also covers the final call (2.6): it
  asserts the arm's `decide_final` and its declared cost are the reference's, and that
  `decide_final` equals `decide` at a clock at the patience deadline with no outputs.

The rule's one parameter, the patience, is in the manifest's `decide` section and is the same for
every arm of a run. A per-arm patience would be a difference in `decide`.

## 2. The shared decision rule

`Decider::decide(state, outputs)` is a pure, deterministic function of the working state, the
outputs it has been shown and the remaining limits it was told. No clock, no randomness, no I/O.

**What it keeps.** The latest output of the verifier, of the estimator and of the heuristic, as
decoded candidate lists, together with the entries they were decoded from. A component that is not
selected leaves its previous output in place, so an arm that skips a component acts on stale
output; that staleness is part of what selection costs (a test shows a stale verifier set
outranking a newer heuristic answer). An output is decoded once: a stored output is never decoded
again at a later step, and an output that arrives byte for byte equal to the one held for its
component keeps the held decoded form instead of being decoded again (section 3.1). A component that
ran and produced nothing, or whose output says the window was damaged, replaces its stored output
with nothing. A `Fail` directive produces no output at all, so the previous one stays. The memory
lookup is not read (2.5).

**The candidate set.** The first available of

1. the verifier's consistent set;
2. the estimator's hypotheses tied for its best score (as far as its entry lists them: at most
   its top five);
3. the heuristic's candidates;

narrowed by the probe results and correction effects in the working state (2.4). If narrowing
leaves nothing, the next source is tried.

**The rule**, in order:

1. *No set.* Wait; at the patience, abstain.
2. *One hypothesis.* A fault: declare it at once. "No fault": declare it only once the clock has
   reached the patience. Silence is also what every fault looks like before its onset (the
   physics permits silence under every hypothesis), so a lone "no fault" taken from an early
   window is not evidence, and the heuristic proposes exactly that on any symptom-free window.
3. *The patience has passed.* Declare the first-ranked hypothesis of the set.
4. *"No fault" is one of several.* Wait. Buying probes in a symptom-free window gains little per
   probe and spends a budget of twelve units.
5. *Several hypotheses, all faults.* Buy the affordable probe with the smallest expected remaining
   set, if that is strictly smaller than the present set. Ties go to the cheaper probe (units,
   then time), then the earlier kind, then the lower service id. Otherwise wait.
6. *The final call (2.6).* The patience counts as passed, whatever the clock says. Nothing else
   changes.

The patience default is 3 s, as it was for the old `heuristic_only`: every generated symptom is
emitted within about half a second of an onset that is at most a quarter of the 10 s horizon.

**Expected set size (step 5).** Over hypotheses, not worlds. A world is a hypothesis plus the two
hidden parity bits (only `DependencyDown` and `Intermittent` have two options). Under a uniform
prior over the candidate worlds, a probe splits them by the result `probe_result` gives each, and
the expected size is the sum over results `r` of `P(r)` times the number of hypotheses among the
worlds giving `r`. The code keeps it as the integer `sum |W_r| * |H_r|` and compares it with
`|H| * |W|`, so no float is involved. Only candidate sites are tried (a probe elsewhere gives the
healthy answer in every world); when "no fault" is a candidate, every service is.

*Why that is the specification's `consistent_hypotheses` over hypothetical outcomes.* The worlds
that give result `r` are exactly the worlds `consistent_worlds` keeps once `r` is added to the
evidence, and a probe result is never an anchoring `ErrorRate`, so adding it changes nothing else
the checker concludes. A test checks the equality, as integers, against a reference that really
calls `consistent_hypotheses` on `evidence + [r]` for each possible `r`, over all eleven classes,
four seeds, two prefix lengths and zero to two bought probes: more than two thousand probes. The
rule does not call the checker; it scores from the worlds. It depends on `physics::probe_result`
and the public `ENTANGLED` pair (and, in the oracle and the tests, on `physics::consistent_worlds`).

**2.1 Where the rule's cost is charged.** Under `Phase::Scheduling`, through the policy's declared
select cost, for every non-privileged arm alike. `Arm::declared_select_cost` is the only place the
selector's cost and the rule's meet. Every arm pays the rule's base cost at every step, including
an arm that selects nothing, and pays the probe-evaluation term from the candidate set it holds and
the decoding and comparison terms for the outputs that arrived at the previous call.
Charging the rule only to arms that "use" it would make the cost of the rule depend on the
selector. The three trivial selectors declare no cost of their own (section 5).

**2.2 The budget.** The rule learns what is left to spend from `Arm::select`, the one place a
policy sees the bill, the same way for every arm: probe units and probe time. A probe that does
not fit is not chosen. If the scheduling charge was refused at a step, `select` did not run and the
figure is a step old; the harness refuses any action the bill cannot pay and records the refusal.

**2.3 What the rule does not do.** It does not check passive observations for consistency, does
not use the memory lookup, and never issues `Correct` (three probes, and it shows only whether the
site was right). It reads no class, seed, horizon, directive or timing.

**2.4 Narrowing by bought probes.** Every probe the arm buys comes back as an observation in the
working state, and the rule uses those, whatever component produced the candidates, to narrow its
set. The reasons: it has to know which probes it already bought or it would buy the same one
again, and a negative probe result is exactly what the heuristic's rule table says it cannot use
("a negative probe changes nothing here", `heuristic.rs`). The consequence is that an arm that
selects only the heuristic gets, inside `decide` and for free, the part of the verifier's work that
concerns probes. It does not get the verifier's work on passive observations.

*How much that matters here, measured.* With narrowing removed (a temporary local patch, not
committed: the rule scores on the stored set unnarrowed and never re-buys a probe it already
bought), `heuristic_only` and `all_components` produced results identical to the committed rule on
all 220 episodes (20 seeds by 11 classes). The reason is visible in the rule table: a positive
probe at the true site turns the heuristic's candidates into a unique proposal, and in these
classes the first probe that is positive ends the search, so negative results never have to be
used. The choice would matter in an environment whose faults are resolved by elimination.

**2.5 The memory lookup is not an input.** A component whose output the rule never reads is pure
cost to any arm that selects it. A selective policy that learns to skip the lookup will "save" its
cost without losing anything. That is a property of the shared rule, not a finding about memory
(the lookup is right about one time in five outside `StaleMemory`; `docs/review-log.md`, A5), and
EXP-004 needs a memory the rule can use. `fixed_pipeline` and `all_components` run it by default,
so the savings S that EXP-001 reports against them include its cost. Preregistration (C1) should
say so. A test (`memory_is_not_an_input_of_the_shared_rule`) pins the property.

**2.6 The final call.** When the harness finds that no affordable work is left, or that the
horizon was reached, it calls `Decider::decide_final` once (`HARNESS.md`, section 1). It is the
rule at its patience deadline: `due` is true, so with one hypothesis left the rule declares it
("no fault" included), with several it declares the first-ranked, and with none it abstains. It
reads the working state, which the harness has refreshed with what had arrived, and the outputs it
stored; no component ran, so there are no new ones. It never buys a probe. The flag changes `due`
and nothing else (one private function serves both calls; a test compares `decide_final` with
`decide` at the deadline over every state the mirror test visits). Every non-privileged arm gets
it from `Arm`, which adds nothing, so it is shared like the rest of the rule.

*What it does and does not say about an arm.* It converts "ran out of affordable work before the
deadline" from an undecided episode into a declaration, scored by the ordinary rules. It does not
make the declaration informed. An arm that spent its budget before any symptom arrived (symptoms
start about a second in) holds, at best, the verifier's set over an empty window, whose first-ranked
hypothesis is "no fault", and declares that: right on `NoFault` (evaluator R5, which also scores an
abstention there as success), wrong, and a critical miss on a critical class, anywhere else. The
rerun probe in section 8.1 shows this is most of what the final call buys under a binding budget.
That is the rule at its deadline working as designed, not a defect, but a reader of a
binding-budget table must read `NoFault` success apart from the rest.

## 3. The cost of the rule

*Two costs.* This section is about the **declared** cost: what a policy is charged and the `Bill`
enforces. The cost an experiment reports, the charter's `C`, is the rule's **counted** work, in
the units of `RULE_UNITS` weighted by the constants next to them; their calibration and validity
fits are `crates/gordian-components/CALIBRATION.md`, section 9. The declared cost below is not
refitted by A8b and is not what `modelled_sched_ns` holds.

The declared cost of one `decide` call is, in `Resource::Compute` nanoseconds (constants in
`decide.rs`, stored in picoseconds so slopes of a few nanoseconds keep their precision):

```text
45 + 0.95 * window + 10.5 * worlds
   + 36 * (6 * targets * worlds)        only when the candidate set could be probed
   + 530 * outputs_decoded_last_call + 115 * hypotheses_decoded_last_call
   + 0.033 * bytes_compared_last_call
```

`window` is the observations in the working state; `worlds` and `targets` come from the first
available stored candidate set (before narrowing, so the figure is an upper bound); the decoding
terms carry the *previous* call's decoding to the next step, and the comparison term the bytes the
previous call compared (3.1). That is a lag, chosen because which outputs a step brings is not known
when the harness asks for the scheduling cost, before `select`.
The decoding of every step is charged at the next one, so an episode is charged for all of it
except the last step's, which the final call (2.6) picks up when the harness makes one: its declared
cost is this formula without the probe-evaluation term (`Decider::declared_final_cost`), because
the call never scores a probe, and it is charged under `Phase::Scheduling` like any other call,
for the rule alone (no selector's cost). If the bill cannot pay it the call is made anyway
(`HARNESS.md`, section 1). A step that has no stored set pays only the base and the window.

### 3.1 An output is decoded once (work item A6c)

**What was found.** `b3-finding4.md` traced a failed verifier raising an arm's success to the rule's
charge for decoding. The diagnosis there was that the rule re-decodes every stored output at every
step. The code did not do that: the harness hands `decide` only the outputs of components that ran
at this step, and the rule keeps decoded sets between steps, so a stored output from an earlier step
was never decoded, charged or counted again (a test has asserted it since A8b: a call with no
outputs counts no decoding). What was decoded again at every step was an output a component
*produced again*: the verifier selected at every step, on a window that has not changed, returns the
same bytes, and the rule decoded and was charged for them each time, 530 + 115 x 46 = 5,820 ns on a
symptom-free window against about 7,600 ns for the whole call.

**What the rule does now.** It keeps, per component, the entries it last decoded from beside the
decoded candidate set. An arriving output whose entries equal them, kind and bytes (`Vec ==`),
keeps the held decoded form: nothing is decoded, `decoded_outputs` and `decoded_ranked` do not
count it, and the next step's declared cost has no decoding term for it. An output that differs, a
first output, and an output that comes back after a different one are decoded as before. "The same"
is decided by byte equality of the entries and nothing else, so the rule assumes nothing about how
a component produces its output. How the rule learns an output is new is therefore *by comparing
it*, not by the harness telling it: `Decider::decide`'s signature and the `Policy` trait are
unchanged, and so is the harness.

**What the comparison costs.** It is real work, so it is counted and declared: the unit
`compared_bytes` (payload bytes of an arriving output whose entries have the shape of the held ones,
so that `==` has to reach the bytes; an upper bound when they differ, since it stops at the first
differing byte) at a calibrated weight of 45 ps in the counted cost and 33 ps declared (the hot
weight, 32.5 ps; measured on a 1 KB output about 32 ns over a call that compares nothing, with a mean of 1,045 bytes compared per state). It is
about 1% of the time of the calls the weights were fitted to, and a repeated verifier output on a
silent window compares a few kilobytes, on the order of 100 ns, against 5,820 ns decoded.

**What is unchanged.** Every decision. `Decider::without_reuse` is the rule as it was (it decodes
whatever it is handed): the reference the tests hold the new rule to. On the inputs of real
episodes, four arms by 11 classes by four seeds at the default budget and at 250,000 and 60,000 ns
(18,619 calls, including the final call), the two rules decide identically at every call;
the reference decodes 25,990 outputs and 410,044 candidates and the rule 2,007 and 20,029 (92% and
95% fewer), and the declared cost over those calls falls from 137.8 to 24.8 million ns. All other
counted units are equal call by call. End to end, `tests/decode_once.rs` compares every verdict
column (success, critical miss, false alarm, abstained, undecided, probes used, corrections) of the
ten B1 arms at 20 ms over 20 seeds by 11 classes, per episode, with a record written by the code of
`da73030`: identical in all 2,200 rows.

**What it does not do.** It does not remove every charge that grows with an unchanged input. Each
step the rule still narrows the stored candidate set against the bought probes (10.5 ns declared and
20 ns counted per world) and, when the set can be probed, scores every probe against every world,
although neither the set nor the probes bought may have changed since the previous step. On a
symptom-free window the verifier's 46-hypothesis set costs the rule about 600 ns a step in
narrowing alone, against about 110 ns when a failed verifier leaves the estimator's five. That is
the residue of "a failed component raises success" that A6c leaves (`a6c-before-after.md`).

**How it was fitted.** An ignored test, `measure_the_rule_against_its_declared_cost`
(`cargo test --release -p gordian-run --test baselines -- --ignored --nocapture`), builds 396
states (11 classes, 12 seeds, three stream prefixes), runs the four components to get outputs, and
times `Decider::decide` 300 times in two modes: with a fresh rule given the outputs (decoding
included) and with the same rule given none. Weighted least squares on relative error, rounded.

| term | fitted (two runs) | what it is |
|---|---|---|
| base | 41 to 44 ns | stored-set handling |
| window | 0.96 to 1.15 ns per observation | the scan for bought probes |
| worlds | 10.1 to 10.3 ns per world | narrowing |
| probe evaluation | 35.9 to 36.2 ns per (probe, world) | scoring |
| decode, per output | about 497 to 527 ns | `serde_json` over one candidate entry |
| decode, per hypothesis | about 112 to 116 ns | |

**How well it holds.** The fit was run four times as the model changed. The two runs that had the
final features agree on the steady-state terms (the ranges in the table). The decoding terms come
from the two runs that were stable (about 497 to 527 ns per output, 112 to 116 ns per hypothesis);
in the last run the decoding regression went unstable (a negative per-hypothesis coefficient),
which is noise from a shared machine, and its decoding fit is not used. With the final constants,
over the 396 states, declared over measured, with decoding, has median 1.03 and range 0.28 to 1.57
(that last run's noise included). Without decoding the declared cost is an upper bound: 10 times
the measured one in states past the patience, where the rule declares without scoring, which is
the terminal step.

**After A6c.** The measurement test now also times the comparison path (`hit_ns`, `cmp_bytes`). On
the same 396 states, on 2026-10-05 on the 2.10 GHz host under `cargo test --release`, declared over
measured for a fresh rule decoding its outputs has median 1.14 (10th to 90th percentile 1.06 to
1.20, range 0.72 to 1.26): the decoding constants above were fitted on another host and were not
refitted for A6c, because they are what the bill enforces and a refit would move every binding
budget for a reason that is not the change; the clone of the held entries that decoding now makes
is inside that figure. For a rule handed outputs it already holds the median is 1.22. Without
decoding or comparing the declared cost is again an upper bound, about ten times the measured one
past the patience, as above.

**What this does not establish.** It was measured on a shared machine while another worker was
benchmarking, unpinned, because the sandbox refused `taskset` for the build; recalibrate under
`scripts/cgroup-run.sh` before B1. Measured cost is the primary cost anyway (plan A4):
`measured_sched_ns` in `measured.csv` includes the rule exactly as timed, and the declared cost
here is what the bill enforces. At the default limits (20 ms of compute) no arm comes near the
limit: the heaviest, `all_components`, spent about 0.5 ms per episode on average when this was measured
(section 8; since A6c, 0.19 ms on seeds 1000 to 1499 against 0.42 ms before it), so
the compute limit does not bind in any condition measured so far.

## 4. JointlyDecisive

*Does the expected-set-size rule probe it or stall?* **It stalls.** The class's two samples each
split the worlds two and two but leave both `DependencyDown` and `Intermittent` on each side, so
the expected number of hypotheses after one sample equals the number there are before it. The rule
needs a strict gain and never buys a sample; a test shows both samples score exactly the bound, and
that no non-privileged arm ever buys one in 12 episodes of the class. Every arm therefore ends in
the patience fallback and declares the first of the tied pair, `DependencyDown` (ties keep
fault-kind order), so it is right exactly when the truth is `DependencyDown`: 13 of 20 seeds here
(a test pins the equivalence, and that both truths occur). The class defeats one-step expected-set
size, as the charter says it should. It would not defeat a rule that scored over worlds, which sees
a gain from one sample (`gordian-world/DESIGN.md`, section 2; `docs/review-log.md`, A1); that rule
is not built, and nothing here special-cases the class. `oracle_evidence` buys both samples and
succeeds on all 20.

## 5. The policies

| id | selection | configuration (manifest) | declared selection cost |
|---|---|---|---|
| `heuristic_only` | the heuristic, every step | none | none |
| `fixed_pipeline` | the configured components in order, at the first step and every `every`-th | `components` (names `heuristic`, `estimator`, `memory`, `verifier`), `every` | none |
| `all_components` | the four components in id order, every step | none | none |
| `random_matched` | each of the four independently with probability `p` per step | `p` | none |
| `oracle_immediate` | none (privileged) | none | none |
| `oracle_evidence` | none (privileged) | none | none |

Every non-privileged arm declares the shared rule's cost (section 3) in addition.

- **`heuristic_only`** is the charter's simple task-specific heuristic. It had its own rule
  through A4 (declare a unique fault, otherwise wait and guess, never probe); it now has the shared
  one, which makes it a different arm with the same id. Results from before A6 are not comparable.
- **`fixed_pipeline`** defaults to all four components every step. With those defaults it is the
  same arm as `all_components` (the components read the working state and not each other's output,
  so their order inside a step changes nothing but which one a bill refusal would drop first); a
  test asserts the two give identical results. Tuning the subset, order and period is exploration
  run B1's job, on exploration data. Nothing here was tuned. The count of steps is calls to
  `select`, which is the step count unless the arm ran out of compute.
- **`all_components`** has no ordering, as the plan says, because order does not matter; id order.
- **`random_matched`** draws from ChaCha8 seeded from the episode seed and the arm name
  (`rng_seed`: FNV-1a over the name, combined with the seed, expanded with splitmix64, so that no
  library's seed expansion is a hidden input). The seed is derived in `policy::build`; the policy
  is handed 32 bytes and never sees the episode seed. It draws exactly one `u64` per component per
  step in id order whether or not it selects, so the stream does not depend on earlier outcomes. A
  test shows two arm names draw different streams. **`p` is a parameter and not a match.** B1 tunes
  it so that the arm's *measured* compute equals a target arm's (plan A4: measured cost is
  primary). The plan sketch says "a uniformly random subset sized to match a target compute bill";
  the brief changed that to independent selection with probability `p`, which is what is built. The
  default 0.5 is a placeholder. With `p = 0` it runs nothing (and so, having no candidates,
  abstains at the patience); with `p = 1` it equals `all_components` exactly, bill included,
  because both declare zero selection cost and share the rule. If its draws are ever given a
  declared cost, only the scheduling part of the bill may differ.

## 6. The privileged arms and how the truth reaches them

Built: `src/policy/oracle.rs` (referred to elsewhere as module `privileged`, see below).

**The path.** The `Policy` trait has no place for a truth, and no policy outside `oracle.rs` has a
constructor that takes one. The harness holds the episode's truth for scoring. To build a
privileged arm it calls `OracleFactory::build(&truth)` through `harness::run_episode_privileged`,
the one entry point that takes a factory instead of a policy; `run_episode` is unchanged. The
change to `harness.rs` is an enum `Source` (`Given(&mut dyn Policy)` or `Privileged(&OracleFactory)`)
and a match of a few lines at the point the truth is built. `OracleFactory` has private fields and one
constructor that takes no truth; the type it builds, `OraclePolicy`, is private to `oracle.rs` (a
`compile_fail` doctest shows it cannot be named). The registry returns `Built::Privileged(factory)`
for the two oracle ids and `Built::Public(policy)` for the rest, and the recorder calls the
matching entry point.

**Enforcement, with what each one is.**

- *Type level:* nothing public takes a truth and returns a policy except `OracleFactory::build`,
  and `OraclePolicy` has no public name. A policy cannot be built from the truth in any other file
  without naming `gordian_eval::Truth` there.
- *Textual:* `scripts/check-no-oracle.sh` allows `oracle.rs` and no other file under `policy/` to
  name the evaluator, `Truth`, the generated-episode type, the simulator or the accessor. A Rust
  test (`only_the_oracle_file_names_the_truth`) repeats the scan so `cargo test` fails without the
  shell script, and asserts `oracle.rs` is the only file naming `Truth`.
- *A naming consequence:* the guard flags the text `oracle::` anywhere outside the allowlist, so
  the module is declared `#[path = "oracle.rs"] pub mod privileged;` and the rest of the crate
  writes `privileged::`. The file path, which is what the allowlist names, is unchanged.
- *Labelling:* `Manifest::validate` rejects an oracle arm whose `arm` does not contain
  `privileged`; `gordian-run init` names such an arm `<policy>_privileged` by default and fails if
  given a name without the word. `results.csv` carries `run_id`, not the arm, so the label is in
  `manifest.json` and in the producer (`policy/oracle_evidence`) of every ledger entry of the
  events sample. A run id without the word still produces a results file that does not say it was
  privileged; join to the manifest.
- *Not enforced:* cargo unifies features across the build, so a runtime check cannot tell an arm
  that links the accessor from one that does not (`HARNESS.md`, 8, item 10), and a policy outside
  `policy/` could in principle call `generate` with a seed. `random_matched` therefore never
  holds the episode seed.

**`oracle_immediate`** declares the true hypothesis at the first step: the ceiling.

**`oracle_evidence`** is an ideal observer. It uses the truth only to choose probes. It declares
only when the public evidence in its working state, read with the world's public rules
(`consistent_worlds`), leaves exactly the true hypothesis, which a test recomputes from the ledger
at the declaration step for every declaration over eleven classes and thirty seeds. At each step
it plans the fewest probes that would make that so, buys the first and re-plans when the result
arrives.

- The plan is a minimum hitting set: for every remaining rival world and every bit assignment of
  the true hypothesis that the evidence still allows, the plan must contain a probe whose result
  tells them apart. **`Truth` carries the hypothesis and not the two hidden bits** (the
  evaluator's `Truth` has none), so the plan has to work whatever the bits are, which can cost a
  probe that exact knowledge of the bits would save. It is exact for the minimum number of probes
  (iterative deepening, branching on the smallest unhit constraint) with a node cap of 400,000 per
  depth; past the cap it plans nothing, which is "no plan", not a claim of minimality. Among plans
  of equal size it takes the first found, cheaper probes tried first.
- It starts at the first step, with an empty window, because probes are valid at any instant and
  it is the earliest instant the evidence permits. That spends probes a patient observer would
  not need (a `DelayedConfigChange` episode, whose snapshot arrives later, costs it one probe); it
  is the earliest-correct-decision ceiling, not a cheapest-cost one. On `NoFault` it can declare
  "no fault" only after probes have excluded every fault, which takes one health check per
  service (about 8 on average of the 12 units).
- With no plan that fits the remaining limits it waits, and at the patience it abstains. It
  never declares a hypothesis the public evidence has not identified.
- It reads the window the harness keeps (256 observations), like every arm: an ideal observer
  *of the bounded working state*, not of the whole stream.

Both arms select no components and declare a zero scheduling cost: they are a ceiling, not a
mechanism with a cost.

**The oracles at the final call (A6b).** The harness's final call (2.6) is an arm's deadline
arriving, and the oracles answer it the way they answer theirs. `oracle_immediate` declares the
truth, as at every call; it declared at step 1, so the call is never made for it. `oracle_evidence`
declares the truth if the public evidence identifies it and otherwise abstains: it buys no probe
(nothing could follow) and it does not read the hidden state to declare, because an ideal observer
that did would stop being one exactly when the budget binds. A final call that makes it abstain
therefore says "the evidence it could afford did not identify the fault", and on `NoFault` that
abstention scores as success (evaluator R5) like any other. Both oracles decided before any final
call in every episode at the default limits and their `results.csv` is unchanged (section 8.1).

## 7. The manifest

`policy` is a `PolicySpec`. In JSON a policy with nothing to configure is its id, which is also
what every manifest written before A6 holds:

```json
"policy": "heuristic_only"
"policy": {"policy": "fixed_pipeline", "components": ["heuristic", "verifier"], "every": 2}
"policy": {"policy": "random_matched", "p": 0.3}
```

A parameter the policy does not have, an unknown component, a repeated component, `every` of 0,
an empty component list or a `p` outside [0, 1] is a parse error. Every parameter is written
(`fixed_pipeline` always writes its components and period), so a manifest records what ran.
`decide: {"patience_ns": 3000000000}` is the shared rule's parameter; a manifest without it gets
the default, so older manifests still parse. What did *not* stay the same: an old manifest naming
`heuristic_only` now runs the shared rule (section 5). An id the registry does not know cannot be
written into a manifest, so `RunError::UnknownPolicy` now arises only for a caller-supplied
policy source (`execute_with`).

```text
gordian-run init --run-id ID --policy POLICY --seed-start N --seed-count N --out FILE
                 [--experiment NAME] [--arm NAME] [--trace-sample-rate R]
                 [--components heuristic,verifier] [--every K] [--p P] [--patience-ns N]
```

## 8. Measurements

A development run, release build, seeds 0 to 19 of every class, default limits, default patience,
`random_matched` at the placeholder p = 0.5, run first with `gordian-run init` and `gordian-run
--manifest ... --out` directly (unisolated), then again through `scripts/run-driver.sh` (cgroup v1,
cores 0-2, pinned driver; the sandbox did not refuse it). The two sets of `results.csv` are
byte-identical apart from the run id, for all six policies. Deterministic columns only below; the
runs are not kept (`artifacts/runs/` is git-ignored). `internal_external_ratio` from the driver
runs, first values and no tolerance declared: `heuristic_only` 0.879, `fixed_pipeline` 0.961,
`all_components` 0.967, `random_matched` 0.965, `oracle_immediate` 0.442 (about 3 ms of work, so
start-up dominates), `oracle_evidence` 0.879. It is exploration: nothing here is a confirmation of
anything. This table is the record of A6, taken before work item A6c; its `bill_compute` figures for
the arms that run components are higher than the rule now declares (3.1), and nothing else in it moved
(a test pins the verdict columns).

| policy | class | success | critical misses | false alarms | abstained | undecided | mean probes | mean bill_compute (ns) |
|---|---|---|---|---|---|---|---|---|
| heuristic_only | Ambiguous | 20/20 | 0 | 0 | 0 | 0 | 1.85 | 50,998 |
| heuristic_only | DelayedConfigChange | 20/20 | 0 | 0 | 0 | 0 | 0.00 | 3,563 |
| heuristic_only | NoiseFlood | 20/20 | 0 | 0 | 0 | 0 | 0.10 | 48,707 |
| heuristic_only | JointlyDecisive | 13/20 | 0 | 0 | 0 | 0 | 0.10 | 107,369 |
| heuristic_only | NoFault | 20/20 | 0 | 0 | 0 | 0 | 0.00 | 78,533 |
| heuristic_only | CriticalFault | 20/20 | 0 | 0 | 0 | 0 | 1.00 | 47,879 |
| heuristic_only | QuietUrgent | 20/20 | 0 | 0 | 0 | 0 | 0.00 | 48,097 |
| heuristic_only | Duplicates | 20/20 | 0 | 0 | 0 | 0 | 0.10 | 47,879 |
| heuristic_only | FeedbackBait | 20/20 | 0 | 0 | 0 | 0 | 1.85 | 50,149 |
| heuristic_only | StaleMemory | 20/20 | 0 | 0 | 0 | 0 | 1.85 | 50,998 |
| heuristic_only | ComponentTimeout | 16/20 | 0 | 0 | 4 | 0 | 0.10 | 66,405 |
| **heuristic_only** | **all** | 209/220 | 0 | 0 | 4 | 0 | 0.63 | 54,598 |
| fixed_pipeline (defaults) | Ambiguous | 20/20 | 0 | 0 | 0 | 0 | 1.85 | 501,288 |
| fixed_pipeline (defaults) | DelayedConfigChange | 20/20 | 0 | 0 | 0 | 0 | 0.00 | 48,451 |
| fixed_pipeline (defaults) | NoiseFlood | 20/20 | 0 | 0 | 0 | 0 | 0.10 | 487,760 |
| fixed_pipeline (defaults) | JointlyDecisive | 13/20 | 0 | 0 | 0 | 0 | 0.10 | 712,184 |
| fixed_pipeline (defaults) | NoFault | 20/20 | 0 | 0 | 0 | 0 | 0.00 | 813,811 |
| fixed_pipeline (defaults) | CriticalFault | 20/20 | 0 | 0 | 0 | 0 | 1.00 | 492,285 |
| fixed_pipeline (defaults) | QuietUrgent | 20/20 | 0 | 0 | 0 | 0 | 0.00 | 486,351 |
| fixed_pipeline (defaults) | Duplicates | 20/20 | 0 | 0 | 0 | 0 | 0.10 | 487,635 |
| fixed_pipeline (defaults) | FeedbackBait | 20/20 | 0 | 0 | 0 | 0 | 1.85 | 500,464 |
| fixed_pipeline (defaults) | StaleMemory | 20/20 | 0 | 0 | 0 | 0 | 1.85 | 501,288 |
| fixed_pipeline (defaults) | ComponentTimeout | 20/20 | 0 | 0 | 0 | 0 | 0.10 | 542,098 |
| **fixed_pipeline (defaults)** | **all** | 213/220 | 0 | 0 | 0 | 0 | 0.63 | 506,692 |
| all_components | Ambiguous | 20/20 | 0 | 0 | 0 | 0 | 1.85 | 501,288 |
| all_components | DelayedConfigChange | 20/20 | 0 | 0 | 0 | 0 | 0.00 | 48,451 |
| all_components | NoiseFlood | 20/20 | 0 | 0 | 0 | 0 | 0.10 | 487,760 |
| all_components | JointlyDecisive | 13/20 | 0 | 0 | 0 | 0 | 0.10 | 712,184 |
| all_components | NoFault | 20/20 | 0 | 0 | 0 | 0 | 0.00 | 813,811 |
| all_components | CriticalFault | 20/20 | 0 | 0 | 0 | 0 | 1.00 | 492,285 |
| all_components | QuietUrgent | 20/20 | 0 | 0 | 0 | 0 | 0.00 | 486,351 |
| all_components | Duplicates | 20/20 | 0 | 0 | 0 | 0 | 0.10 | 487,635 |
| all_components | FeedbackBait | 20/20 | 0 | 0 | 0 | 0 | 1.85 | 500,464 |
| all_components | StaleMemory | 20/20 | 0 | 0 | 0 | 0 | 1.85 | 501,288 |
| all_components | ComponentTimeout | 20/20 | 0 | 0 | 0 | 0 | 0.10 | 542,098 |
| **all_components** | **all** | 213/220 | 0 | 0 | 0 | 0 | 0.63 | 506,692 |
| random_matched (p=0.5) | Ambiguous | 20/20 | 0 | 0 | 0 | 0 | 1.85 | 265,527 |
| random_matched (p=0.5) | DelayedConfigChange | 20/20 | 0 | 0 | 0 | 0 | 0.00 | 27,346 |
| random_matched (p=0.5) | NoiseFlood | 20/20 | 0 | 0 | 0 | 0 | 0.00 | 255,358 |
| random_matched (p=0.5) | JointlyDecisive | 13/20 | 0 | 0 | 0 | 0 | 0.00 | 372,235 |
| random_matched (p=0.5) | NoFault | 20/20 | 0 | 0 | 0 | 0 | 0.00 | 414,524 |
| random_matched (p=0.5) | CriticalFault | 20/20 | 0 | 0 | 0 | 0 | 1.00 | 259,494 |
| random_matched (p=0.5) | QuietUrgent | 20/20 | 0 | 0 | 0 | 0 | 0.00 | 255,038 |
| random_matched (p=0.5) | Duplicates | 20/20 | 0 | 0 | 0 | 0 | 0.00 | 255,576 |
| random_matched (p=0.5) | FeedbackBait | 20/20 | 0 | 0 | 0 | 0 | 1.85 | 265,150 |
| random_matched (p=0.5) | StaleMemory | 20/20 | 0 | 0 | 0 | 0 | 1.85 | 265,527 |
| random_matched (p=0.5) | ComponentTimeout | 20/20 | 0 | 0 | 0 | 0 | 0.00 | 276,504 |
| **random_matched (p=0.5)** | **all** | 213/220 | 0 | 0 | 0 | 0 | 0.60 | 264,753 |
| oracle_immediate (privileged) | Ambiguous | 20/20 | 0 | 0 | 0 | 0 | 0.00 | 0 |
| oracle_immediate (privileged) | DelayedConfigChange | 20/20 | 0 | 0 | 0 | 0 | 0.00 | 0 |
| oracle_immediate (privileged) | NoiseFlood | 20/20 | 0 | 0 | 0 | 0 | 0.00 | 0 |
| oracle_immediate (privileged) | JointlyDecisive | 20/20 | 0 | 0 | 0 | 0 | 0.00 | 0 |
| oracle_immediate (privileged) | NoFault | 20/20 | 0 | 0 | 0 | 0 | 0.00 | 0 |
| oracle_immediate (privileged) | CriticalFault | 20/20 | 0 | 0 | 0 | 0 | 0.00 | 0 |
| oracle_immediate (privileged) | QuietUrgent | 20/20 | 0 | 0 | 0 | 0 | 0.00 | 0 |
| oracle_immediate (privileged) | Duplicates | 20/20 | 0 | 0 | 0 | 0 | 0.00 | 0 |
| oracle_immediate (privileged) | FeedbackBait | 20/20 | 0 | 0 | 0 | 0 | 0.00 | 0 |
| oracle_immediate (privileged) | StaleMemory | 20/20 | 0 | 0 | 0 | 0 | 0.00 | 0 |
| oracle_immediate (privileged) | ComponentTimeout | 20/20 | 0 | 0 | 0 | 0 | 0.00 | 0 |
| **oracle_immediate (privileged)** | **all** | 220/220 | 0 | 0 | 0 | 0 | 0.00 | 0 |
| oracle_evidence (privileged) | Ambiguous | 20/20 | 0 | 0 | 0 | 0 | 1.00 | 0 |
| oracle_evidence (privileged) | DelayedConfigChange | 20/20 | 0 | 0 | 0 | 0 | 1.00 | 0 |
| oracle_evidence (privileged) | NoiseFlood | 20/20 | 0 | 0 | 0 | 0 | 2.40 | 0 |
| oracle_evidence (privileged) | JointlyDecisive | 20/20 | 0 | 0 | 0 | 0 | 4.60 | 0 |
| oracle_evidence (privileged) | NoFault | 20/20 | 0 | 0 | 0 | 0 | 8.05 | 0 |
| oracle_evidence (privileged) | CriticalFault | 20/20 | 0 | 0 | 0 | 0 | 1.50 | 0 |
| oracle_evidence (privileged) | QuietUrgent | 20/20 | 0 | 0 | 0 | 0 | 2.40 | 0 |
| oracle_evidence (privileged) | Duplicates | 20/20 | 0 | 0 | 0 | 0 | 2.40 | 0 |
| oracle_evidence (privileged) | FeedbackBait | 20/20 | 0 | 0 | 0 | 0 | 1.00 | 0 |
| oracle_evidence (privileged) | StaleMemory | 20/20 | 0 | 0 | 0 | 0 | 1.00 | 0 |
| oracle_evidence (privileged) | ComponentTimeout | 20/20 | 0 | 0 | 0 | 0 | 2.40 | 0 |
| **oracle_evidence (privileged)** | **all** | 220/220 | 0 | 0 | 0 | 0 | 2.52 | 0 |

*Reading it.*

- Every arm that can use the shared rule is at or near the ceiling on every class except
  `JointlyDecisive` (section 4) and, for `heuristic_only`, `ComponentTimeout`, where a `Fail`
  directive on the heuristic leaves the rule with no set (4 of 20 abstain). `fixed_pipeline` and
  `all_components` are identical, as stated; `random_matched` at p = 0.5 has the same successes as
  `all_components` at 52% of the compute.
- Success is saturated, so on this world and this rule EXP-001's `D` is decided by a few classes
  and its `S` by compute. `heuristic_only` against the default pipeline reads `D = -0.018` and,
  on declared compute, `S = 0.89` (the charter defines `S` on measured cost, which this run
  records in `measured.csv` and this table does not use). Its entire `D` comes from the four
  `ComponentTimeout` episodes in which its one component was made to fail. The verifier and the
  estimator add nothing measurable here beyond that fallback: `fixed_pipeline` with only the
  estimator, or only the verifier, scores 210/220 at 25% and 62% of the default pipeline's compute,
  losing only three `ComponentTimeout` episodes (a `Fail` on its one component). `fixed_pipeline`
  with only the memory lookup abstains on every episode (20/220 successes, all `NoFault`, and 40
  critical misses): the rule reads no memory output (2.5). This is a statement about the
  instrument, for B4 and for A1's revision, not a finding about selection.
- The oracle gap to the best baseline is large on one class (`JointlyDecisive`, 0.35) and zero
  elsewhere. B4's rule (revise A1 if the gap is under 0.10 on *every* class) is not triggered by
  this, but the headroom is one class wide.
- Mean decision instant: `oracle_evidence` 0.05 s to 0.40 s by class; the baselines 0.17 s on
  `DelayedConfigChange` (the snapshot decides) and 1.8 s to 3.0 s elsewhere (the first symptoms
  arrive at 1 s to 2.5 s, and silence and `JointlyDecisive` wait for the patience).

### 8.1 Under a binding compute budget, before and after the final call (A6b)

Same manifests as the coordinator's headroom probe (`docs/review-log.md`, A6): seeds 0 to 19 of
every class, `limits.compute` edited in the manifest, each run through `scripts/cgroup-run.sh`
(cores 0-2, not pinned by the driver), once with the binary built from the commit before A6b and
once with the A6b binary. The "before" column reproduces the review log's table exactly
(heuristic only 187, random 20, verifier only 19, all components 18 at 60 microseconds; 209, 138,
78, 35 at 250). Successes of 220; the stop reasons are of the 220 episodes. Deterministic columns
only. `final` is `final_declaration`, `budget` is `budget_exhausted`.

| arm | compute | before: successes | before: stop reasons | after: successes | after: stop reasons | gained on `NoFault` | gained elsewhere |
|---|---|---|---|---|---|---|---|
| heuristic only | 250 us | 209 | terminal 220 | 209 | terminal 220 | 0 | 0 |
| heuristic only | 60 us | 187 | terminal 191, budget 29 | 207 | terminal 191, final 29 | 7 | 13 |
| random p = 0.5 | 250 us | 138 | terminal 138, budget 82 | 162 | terminal 138, final 82 | 18 | 6 |
| random p = 0.5 | 60 us | 20 | terminal 20, budget 200 | 40 | terminal 20, final 200 | 20 | 0 |
| verifier only | 250 us | 78 | terminal 81, budget 139 | 100 | terminal 81, final 139 | 20 | 2 |
| verifier only | 60 us | 19 | terminal 19, budget 201 | 39 | terminal 19, final 201 | 20 | 0 |
| all components | 250 us | 35 | terminal 35, budget 185 | 58 | terminal 35, final 185 | 20 | 3 |
| all components | 60 us | 18 | terminal 18, budget 202 | 38 | terminal 18, final 202 | 20 | 0 |
| estimator only | 250 us | 206 | terminal 218, budget 2 | 206 | terminal 218, final 2 | 0 | 0 |
| estimator only | 60 us | 20 | terminal 21, budget 199 | 40 | terminal 21, final 199 | 20 | 0 |

No episode is undecided after the change, in any arm or condition (the final call never returned
`None` and the world never refused one). Every row that changed was a row that had been
`budget_exhausted`; no row that had decided changed; no success was lost. At the default limits
(20 ms) all eight arms measured (the six above, `fixed_pipeline` defaults, and both oracles) wrote
a `results.csv` byte-identical to the pre-A6b binary's, so no row differs there.

*What the gain is.* Almost all of it is `NoFault`. What the arms declared at the final call, over
the 220 episodes (from a scratch run, not committed):

| arm | compute | declared "no fault" | declared a fault, right | declared a fault, wrong | abstained |
|---|---|---|---|---|---|
| all components | 250 us | 182 | 3 | 0 | 0 |
| all components | 60 us | 202 | 0 | 0 | 0 |
| random p = 0.5 | 250 us | 73 | 6 | 3 | 0 |
| random p = 0.5 | 60 us | 200 | 0 | 0 | 0 |
| verifier only | 250 us | 135 | 2 | 2 | 0 |
| verifier only | 60 us | 198 | 0 | 0 | 3 |
| heuristic only | 60 us | 9 | 13 | 7 | 0 |

At 60 us the arms that run components are out of budget at a mean logical time of 0.3 to 0.6 s
(318, 574 and 506 ms for all components, random and verifier only), before the first symptom, and
declare the silent hypothesis, which is right only on `NoFault`. At 250 us the means are 1.2 to
2.0 s and a few see enough to be right. Under the final call the review log's two effects
separate. The rule's defect (budget exhaustion shown as indecision) is gone: the 60 us floor moves
from 18 to 20 successes of 220 up to 38 to 40, and every one of the added 20 is a `NoFault`
episode. The real waste (`all_components` pays for 154 component calls on windows that have mostly
not changed, and then decides on an empty window) is unchanged, and is now visible as wrong
declarations and critical misses (40 of 40 on the two critical classes at 60 us, 36 at 250 us for
all components) rather than as undecided. The one arm that gains on faulted classes is
`heuristic_only` at 60 us, which is cheap enough to have seen symptoms before its budget ran out
(13 right, 7 wrong, 9 "no fault" among the 29). **Read any binding-budget success rate together
with the `NoFault` success and the critical-miss rate.** A preregistration (C1) that states
whether a `NoFault` abstention or silent declaration enters the primary outcome (review log, A2)
must also state it for these final declarations.

## 9. Departures, gaps, and what is least certain

1. **The decision rule is mine.** The brief gave the shape (declare, probe, wait or fall back);
   the rest is a choice: source priority, narrowing by bought probes, the silence handling, the
   tie-breaks, the patience default, ignoring memory. Section 2 gives the reasons; a different
   defensible rule would give different baselines, and EXP-001's result is conditional on this
   one. This is the largest source of uncertainty in the unit.
2. **A silence guard the brief did not state.** "Declare when exactly one hypothesis" would have
   declared "no fault" on every faulted episode at the first step (the heuristic's lone proposal on
   a symptom-free window). A lone "no fault" waits for the patience, and "no fault" among several
   blocks probing.
3. **The declared cost of the rule is fitted on an unpinned, shared machine** and has a one-step
   lag in its decoding term (section 3).
4. **Slow, and the clock.** HARNESS.md section 5 records how the coordinator's decision was
   implemented. One reading was taken where two were possible: a component with no declared `Time`
   takes its compute nanoseconds on the clock whether or not it is slowed. Decision instants of
   arms that run components are therefore a few microseconds later than before A6.
5. **`oracle_evidence`'s plan is robust to the hidden bits** because `Truth` does not carry them,
   and starts at t = 0 (section 6). The brief's "shortest probe sequence" is met in number of
   probes, not in cost.
6. **`random_matched` is independent selection with probability `p`**, per the brief, not the
   plan's "subset sized to match a target bill"; matching is B1's.
7. **`heuristic_only` changed meaning** (section 5).
8. **The module is `privileged`, the file `oracle.rs`** (section 6).
9. **Dependency added:** `rand_chacha` 0.10.0 and `rand_core` 0.10.1 for `gordian-run`, the same
   versions the world uses (already in the lockfile). Requirement: a seeded, reproducible stream
   for `random_matched`. Simpler alternative: a hand-written generator, which gains nothing.
10. **Isolation.** The six-policy development run was done directly and again through
    `scripts/run-driver.sh`, which this session's sandbox allowed, with identical results. The
    calibration of the rule's declared cost (section 3) and the earlier test runs were unisolated
    and unpinned (`taskset` was not used on cargo), on a machine another worker was using. No
    refusal by the sandbox was met that had to be worked around.
11. **Not measured:** the memory lookup's contribution under a rule that can use it; the verifier's
    value on late-anchor or evicted-window streams (A5b territory); any noise or stress beyond what
    the eleven classes generate; the effect of the compute limit (it never binds).
12. **Dependency on the checker's API.** The rule uses `probe_result`, `ENTANGLED` and
    (oracle, tests) `consistent_worlds`. A5b's optimized checker must keep `consistent_worlds`,
    or the oracle and the equivalence test must be adapted.
13. **The final call (A6b).** `Policy` gained `declared_final_cost` and `decide_final`; `Arm`
    delegates both to the shared rule, so the call is shared by construction and a test says so.
    The oracles answer it as their own deadline (section 6). The step cap gets none
    (`HARNESS.md`, section 1). Least certain: what a final declaration made on an empty window
    should be. The rule declares its first-ranked hypothesis, which is "no fault", as the plan
    says ("exactly as at the patience deadline"); a rule that abstained when it had seen no symptom
    would score the same on `NoFault` and differently (an abstention, not a wrong declaration) on
    faulted classes. That is a different rule and was not built (section 8.1).
