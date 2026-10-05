# Coordinator review log

What the coordinator checked for each merged unit, what it decided, and what it carried forward
to later units. Newest first. Reports from workers are model output; this log records what was
independently verified.

## Charter revised — approved by the user

The user approved `docs/charter-revision-proposal.md`. The charter's sections 1, 5, 6, 7 and 12
are rewritten accordingly, with new foundations and sources, and a revision record at the top.
EXP-001 is retired unfrozen and recorded as an exploration finding; the old EXP-002 to EXP-007 and
EXP-I01 are retired or re-scoped into EXP-101 to EXP-106. Change-triggered execution joins the
baseline registry. The plan gains Stage R (R1 stream world and simulated reasoner, R2 stream
evaluator, R3 stream harness and conventional baselines, R4 headroom check).

The user's message read "Inapprove the proposal"; the coordinator read it as "I approve" from
context and said so. If that reading is wrong, this change is reverted from git history.

## A6d incremental narrowing — merged; the shared rule no longer repeats work

**Re-verified.** Fixture committed before the rule change and untouched after it; gates on exit
codes. Worker evidence: 2,200 fixture rows and 55,000 B1 rows at 20 ms identical in every verdict
column; 30,403 calls step-equivalent to the cache-free reference; rule fit R² 0.996 / 0.997.

**Effect.** Narrowing work fell about 96% (30,007 against 764,647 world evaluations over the step
test). "A failed component raises success" fell from 102 to 35 episodes.

**The 35 are real cost, not repeated work.** In 31 of them the no-directive run ended short of
affordable work by a median of 37 ns, after paying about 8 µs once to read the verifier's first
output. A failed verifier never pays that bill, and at a 60–100 µs budget it is the margin. No
cache removes a first read. The other 4 also need the verifier's stale set to lose priority.
Documented in POLICIES.md §3.3; the rule is unchanged. Changing what reading an output costs would
be a change to the instrument and is not made.

**Accepted with a note.** Declared world and evaluation terms are now billed one call late, so the
hard limit can be overshot once per episode by at most about 8 µs. The work is still counted in
modelled cost. Under the revision proposal, any expensive reasoner call must be paid before it
runs, never in arrears.

**State of the shared rule.** Three successive fixes (A6b, A6c, A6d) made it rational at exhaustion
and incremental on unchanged inputs. It is now a strong cheap rung: change-triggered by
construction.

## A6c decode once — merged; the residue becomes A6d

**Re-verified.** The verdict fixture was committed before the rule change and is untouched by it;
all 2,200 rows (10 arms, 20 ms, 20 seeds × 11 classes) identical. Gates on exit codes. Rule
counted-operation fit R² 0.995 (fit) and 0.997 (held out).

**Correction to the plan text.** A6c's premise ("stored outputs are re-decoded every step") was
wrong: stored outputs never were. What repeated was a component *re-producing* byte-identical
output each step, which the rule decoded and charged again. The worker implemented the intent by
content equality and kept the old rule as `Decider::without_reuse`, a test-only reference.

**Effect.** Decoding fell 92%; at 20 ms mean modelled cost fell 40–47% for verifier-running arms
and 12–34% for the others. At 250 µs `all_components` went from 29.1% to 87.0% success. The
"failed component raises success" effect fell from 210 to 102 raised episodes but is not gone: the
rule still re-narrows the verifier's unchanged 46-hypothesis set every step (about 10.5 ns per
world, declared). Zeroing that term leaves 35 raised episodes, unexplained.

**Meta-finding.** Two defects in a row were the shared rule repeating work on inputs that had not
changed. Fixing them makes the baseline incremental. Change-triggered execution (recompute only
when inputs change) is the cheapest form of selective activation, and the charter's baseline
registry does not name it. A salience mechanism must beat it, not only periodic schedules.
Coordinator recommendation to the user: add "change-triggered (memoized) execution — whether
salience adds anything beyond skipping unchanged inputs" to charter section 7. Not made here,
because the charter is normative.

## Exploration follow-ups — merged; correction to the Stage B entry

**Re-verified.** Gates on exit codes; the truth table regenerated from
`crates/gordian-eval/examples/truth_table.rs` hashes to the scratch original; no crate source
changed beyond that allowlisted example.

**Finding 4 resolved, and it corrects Stage B.** The shared rule charges for decoding every stored
component output on every step, 530 ns per output plus 115 ns per hypothesis, even when the output
has not changed. On a symptom-free window the verifier's output lists 46 hypotheses, so re-decoding
it costs about 5,800 ns of the roughly 7,600 ns the rule charges per step. A failed verifier leaves
nothing to decode, the budget lasts longer, and success rises: that is the whole "failed component
helps" effect (removing the charge lifts `all_components` at 250 µs from 13.8% to 72.2% on these
episodes; removing the verifier's priority in the rule changes 0.2–1.8 points).

**Correction.** Stage B's collapse of verifier-running arms at binding budgets (`all_components`
0.291 at 250 µs) is mostly this re-decoding charge, not component compute. With it removed, the
worker's upper-bound variant gives `all_components` 0.730 and `fixed_verifier_only` 0.943 at
250 µs. The heuristic family is bit-identical either way, so the Stage B conclusion about the tuned
periodic baseline stands; the "naive pipelines collapse because components are expensive" reading
does not.

**Decided.** Re-billing an unchanged output is an artefact of the shared rule's implementation,
not a property of the world, and it biases every comparison involving a verifier-running arm. It
is fixed under every option of the pending decision (plan item A6c), before any experiment.

## Stage B exploration (B1–B4) — merged; EXP-001 not freezable as designed

**Re-verified.** Gates on exit codes; per-arm success, modelled cost and critical-miss rates
recomputed from `b1-variance.csv` match the worker's report. 220,000 B1 episodes, no failed or
excluded run, replay checks byte-identical.

**What the data say (exploration, not confirmation).**

- A periodic heuristic (`every` = 4) holds success 0.954 at every budget level for about 22k modelled
  ns per episode; `all_components` needs 522k for 0.965 and collapses to 0.16–0.29 when the budget
  binds. A strong simple baseline captures nearly all achievable quality at a few percent of the
  cost. This is the charter's baseline registry doing its job.
- The shared decision rule costs about 17.1k ns per episode, is charged at every step for every
  arm, and is 65–89% of the heuristic arms' cost. Against the tuned periodic pipeline, even a
  perfectly timed component selector can save at most about 10–18% (B4's estimate; arithmetic, not
  a run), below EXP-001's preregistered 20% margin. Quality headroom against that baseline is
  0.01–0.02.
- The one large oracle gap, JointlyDecisive (0.38), belongs to the shared rule's one-step probe
  choice; no component schedule can close it.
- Five of six stressors cannot fail any current arm: noise is separable for free by catalogue id,
  no arm has a salience mechanism, and the final declaration makes unbounded waiting invisible.
- Effective ambiguity in the ambiguous classes is 2.85 kinds, not 5; a prior-aware arm that never
  probes is right 41.6% of the time.

**Interpretation.** In the current small world, *which component runs* is not where the cost is.
The dominant computation is deliberation (the shared rule) and sensing (probes, which modelled cost
does not price). EXP-001 as designed would very likely return H0 for structural reasons of this
environment, not because selective activation fails in general. The charter anticipated the
mirror-image danger (a weak baseline making anything look good); here the strong baseline shows the
environment offers little to select.

**Instrument findings (not yet fixed).**

1. The rule is not schedulable: a selector cannot decide when to deliberate. Whether deliberation
   is part of what selective activation controls is a design decision.
2. Probes are not in the modelled cost, though the charter's `C` includes sensing; arms buy about
   two probes where one would do.
3. Stressors are toothless at default noise (see above).
4. A failed component can raise an arm's score at binding budgets (3 cells); suspected rule
   behaviour on an empty verifier output; not confirmed.
5. `b4.py` reads a truth table produced outside the repository; one exploration input is not
   regenerable.

**Decision required from the user** (not taken by the coordinator, because it shapes every later
experiment and freezing is irreversible):

- (a) Freeze EXP-001 in the current world and expect a bounded negative result.
- (b) Revise before freezing: make deliberation schedulable, price probes, and revise the world
  so that relevance is costly to determine (non-separable noise, specialist components with
  partial views), with the revision's properties fixed from the charter before any arm is run on
  it, and the generalist and periodic baselines kept.
- (c) Record (a) as an exploration finding only, and do (b).

Coordinator recommendation: (c). A preregistered experiment whose negative outcome is already
implied by exploration arithmetic has low information value, while the fixes in (b) are needed for
EXP-002 to EXP-004 anyway.

Independent of the decision, started then: A7b (ratio-interval calibration on B1's cost
distribution) and the two small exploration follow-ups, since merged (see the entry above).

## A8b counted operations — merged with a freeze gate

**Re-verified.** fmt, clippy (`--locked`), workspace tests with `--no-fail-fast`, oracle guard,
driver test, dump sha256 and the analysis suite on exit codes; reference checker functions
untouched.

**Accepted.** Counters follow the dominant loops of each component and the shared rule, are
deterministic, and cannot be seen or set by a policy (type-enforced, tested). Against hot-loop
minimum timings they fit with R² 0.98–0.99 on fit, held-out and real states.

**Scrutinized: in-situ rescaling.** With pure hot-loop weights the non-identical-arm check failed
(modelled ratio 15.66 against a wall-time median ratio of about 12.1). The worker found that a call
inside an episode costs 1.2–2.2× the same call in a loop, more so for arms that call components
sparsely, consistent with cache effects, and rescaled each weight by a per-component factor and
per-call constant fitted on other arms and seeds. The fit and the check share no arms or episodes,
and the worker reported the failure and offered rejection, so this is calibration, not tuning to
the test. Coordinator check on an unfitted sparse pattern (heuristic only against the verifier
every second step, seeds 200–219): modelled 5.25, wall-time median 5.40 [5.05, 5.85], inside.

**Residual risk.** The in-episode premium depends on the scheduling policy's call pattern, which is
exactly what EXP-001 varies. Fixed factors fitted on other arms priced the sparse arm about 5% off
in the worker's data and about 3% off in the coordinator's. The check is weak (intervals about 15%
wide).

**Carried forward to C1 (EXP-001 freeze gate).** Before freezing, run the cost check on the actual
EXP-001 arms (selective against the tuned periodic pipeline) on exploration seeds. If the modelled
ratio falls outside the wall-time interval, or the two disagree by more than a quarter of the
preregistered savings margin, the cost conclusion of EXP-001 is reported as unresolved, whatever
the modelled result.

**Host change.** The VM now reports a 2.10 GHz Xeon; earlier sessions reported 2.80 GHz. Modelled
cost is in calibration-host nanoseconds and does not change with the host; wall-time checks are
valid only on the host where they run. The manifest should record the CPU model and frequency, not
only flags (small follow-up).

## A8 interleaved arms — merged; A/A fails on an idle machine

**Re-verified.** Merged cleanly onto A6b; fmt, clippy (`--locked`), 293 workspace tests with
`--no-fail-fast`, the oracle guard, the driver test (62) and the analysis suite (212), on exit
codes.

**Worker's evidence.** Interleaving removes a large between-run effect: sequential A/A runs gave
12 of 20 intervals excluding 0 (sd of S 0.151), interleaved runs 0 of 21 (sd 0.037). The worker's
runs shared the machine with another worker.

**Coordinator's idle-machine A/A** (through the driver, cores 0–2, shell on core 3):

| Run | S | 90% interval | Contains 0 | Drift CV |
|---|---|---|---|---|
| heuristic only, run seed 1 | −0.7% | [−5.8%, +3.7%] | yes | 0.12 |
| heuristic only, run seed 2 | −2.9% | [−5.9%, +0.1%] | yes | 0.03 |
| heuristic only, run seed 3 | +5.4% | [+0.5%, +10.0%] | no | 0.27 |
| all components, run seed 1 | −6.9% | [−13.3%, −1.4%] | no | 0.11 |

The per-block minimum timing was stable to about 2% while the mean moved up to 80%; `/proc/stat`
shows non-zero steal time. In the all-components run five episodes carried 51% of the total
absolute difference, and S without them was +0.9%; the median per-episode log ratio was near 0 in
every run. Interference from the host arrives in bursts that hit one copy of an episode and not
the other, and a ratio of totals is sensitive to them.

**Decided.** The plan's A8 rule applies: counted operations are built before B1 (item A8b), and the
charter's cost `C` becomes deterministic modelled cost, with wall time as a secondary check. This
is a change of measurement made before any experiment is frozen.

## A6b final declaration — merged

**Re-verified.** All gates on exit codes; the `all_components` row at 60 µs reproduced exactly
(202 final declarations, 18 terminal, 38 successes, 40 critical misses).

**What it showed.** Under a binding budget, component-running arms exhaust their compute at
0.3–0.6 s of logical time, before any symptom has arrived, by re-running components on windows with
nothing new in them. At the final call they declare "no fault", which is right only on `NoFault`.
Nearly all of the success gained by the fix is `NoFault`; on faulted classes the failures are now
visible as wrong declarations and critical misses instead of undecided rows.

**Carried forward to C1 (EXP-001 preregistration).**

- An arm that does nothing and declares "no fault" scores `NoFault` for free. The primary outcome
  must be read per class with the critical-miss rate, or exclude `NoFault` from the success
  average and score it through false alarms; the preregistration chooses and states which.
- Whether the shared rule at its deadline should declare "no fault" on a window with no symptom,
  or abstain, is a rule choice that changes faulted-class scores. It is fixed before freezing and
  applies to every arm.
- The final call's declared cost is derived, not separately calibrated.

## A6 baselines — merged; headroom probe

**Re-verified.** fmt, clippy with `--locked`, the workspace tests, the oracle guard and the
driver shell test on the merged tree. Accepted the clock interpretation: a component's declared
Compute nanoseconds are its time for every component, and `Slow` multiplies them.

**Coordinator error, fixed forward.** The merge was pushed after a test summary showed one failure,
because the command chain gated on a parsed summary rather than cargo's exit code. The failure was
a random-count coverage floor in the A5b equivalence tests (about 2–3% flake rate; the checker
agreed with the reference on every case). Fixed with a deterministic sweep and a floor at about 3.9
standard deviations. Coordinator merges now gate on exit codes only.

**The worker's finding.** With the shared rule and default limits every arm is at or near the
ceiling (209–213 of 220; oracles 220). Taken alone, this would make EXP-001 trivially pass.

**Coordinator headroom probe** (20 seeds × 11 classes, cgroup-isolated, successes of 220):

| Arm | compute 20 ms (default) | 250 µs | 60 µs | 25 µs |
|---|---|---|---|---|
| heuristic only | 209 | 209 | 187 | 39 |
| estimator only | 210 | 206 | 20 | 20 |
| random p = 0.5 | 213 | 138 | 20 | 13 |
| verifier only | 210 | 78 | 19 | 10 |
| all components | 213 | 35 | 18 | 4 |
| oracles | 220 | | | |

Under a binding budget, quality depends strongly on scheduling. Two effects are mixed in this
table and must be separated:

1. *A shared-rule defect.* Nearly every failure is `budget_exhausted`: the rule waits for its
   patience deadline, runs out of affordable work first, and never declares although declaring is
   free. Plan item A6b gives every arm a final declaration.
2. *Real waste.* `all_components` ran 154 component calls per episode on windows that had mostly
   not changed (median decision at about 1.9 s, a step every 50 ms). Avoiding that waste is what
   selective activation claims to do, and a tuned periodic schedule is the charter's adversary.

**Decided.**

- No world revision for EXP-001. It is testable under a binding budget; B1 sweeps budget levels
  and the preregistration fixes the levels from B1.
- The saturation at default limits is a property of a budget that never binds, not evidence for
  or against any mechanism.

**Open, for the user.** One cheap component (the heuristic) is nearly sufficient alone: the
components are redundant generalists that all read the whole window with the full public rules,
not the specialists the charter's question is about. That limits the quality headroom available to
EXP-002 and EXP-003. Making components specialized (partial views, complementary evidence) is a
research-design choice that risks tailoring the environment to the hypothesis; it is raised to the
user rather than built.

## A5b checker — merged

**Re-verified.** fmt, clippy with `--locked` and all features, 220 workspace tests, oracle guard,
dump sha256 unchanged; the diff to `gordian-components` is cost constants only. The pinned
calibration recheck waits until no other worker is compiling.

**Accepted.** Root cause found by step counting before any change: `anchored(site)` rescanned the
informative prefix at every dependent alarm, about k·d steps. The fix carries the minimum anchor
instant per world; exact for unsorted instants. About 55,000 equivalence comparisons against the
kept reference across generated, adversarial and random inputs; mutation shows the tests can
fail. Late-anchor shape linear; 78× faster at n = 2048. 0–20% slower on shapes the reference
already handled linearly; not tuned, per spec.

**Correction to the A5 entry.** The "24× declared at n = 256" and "1.7 ms at n = 1025" figures
were not reproduced by the checker alone (0.9–1.0 ms at n = 1024 on the old build). The 24× was a
component-level declared-to-measured ratio from one session; read it as an order of magnitude,
not a constant.

**Decided: machine drift is a threat to every measured-cost comparison.** Wall time drifted up
to 30% within one session and 13–34% between sessions. No hardware counters exist in this VM.
Plan item A8: interleave arms per episode in randomized order, log a drift-control workload, test
for position effects, and pass an A/A check before B1.

## A4 run harness — merged; worker report lost

A container restart stopped the worker after it committed and before it reported. The commit
survived; the coordinator reviewed it without a report.

**Re-verified.** fmt, clippy with all features, 208 workspace tests, oracle guard. Read
`HARNESS.md` in full. Ran `heuristic_only` end to end through `scripts/run-driver.sh` under cgroup
v1 isolation: 550 episodes, 0 undecided, internal-to-external ratio 0.81 (first value; becomes the
tolerance starting point). A second run of the same manifest gave a byte-identical `results.csv`;
`measured.csv` differed, as designed. The events sample contained none of the hidden-state key
names. The episode class does not reach policies (`PublicInfo` has no class; the policy directory
guard bans `Episode`, `Simulator`, `gordian_eval`).

**Independent evidence from the smoke run.** `heuristic_only` never probes, and its success on
hidden-kind classes equals the generator's prior for the kind it guesses (JointlyDecisive 0.58
against a 0.595 prior; Ambiguous 0.44 against 0.40). A policy without hidden state should sit at
its prior; this is a second confirmation that the public stream does not leak the kind.

**Decided.**

- The budget reconciliation stands: the `Bill` is the authority for every resource; the episode
  spec's budget must equal the limits or the harness refuses the episode.
- A component's declared Compute nanoseconds are its time. `Slow` multiplies Compute as well as
  Time (assigned to A6; `HARNESS.md` section 8 item 7).
- Analysis must accept undecided rows, drop `bill_total` (it summed ns, probes and bytes), and
  read `measured.csv`, with measured cost the default for relative savings (separate unit).
- Every non-oracle baseline shares one decision rule; arms differ only in selection, because
  EXP-001's intervention is the scheduling policy only (assigned to A6).

**Carried forward.**

- To D1: the events sample carries `class` as a join key. Anything that trains on it must drop
  `class`; it is a strong label.
- To B3 and EXP-002: `QuietUrgent` and `NoiseFlood` are toothless against a component that reads
  the whole window, because catalogue messages are separable from noise for free (`heuristic_only`
  scored 1.00 on both). The stressors test only policies that pay to look.
- `FeedbackBait` is identifiable from public graph data (`unreliable_health`); it shares its prior
  with Ambiguous and StaleMemory, so identification reveals little about the kind. Documented in
  the world's DESIGN.md.

## A5b checker — restarted from a WIP snapshot

The restart stopped the worker with uncommitted changes. The coordinator snapshotted them as a
WIP commit on `checker-perf` and a new worker resumed from it, instructed to verify rather than
trust the unfinished work and to commit after each milestone.

## A5 components — merged after one revision

**Decided.**

- Measured cost, not declared cost, is the charter's `C` in every experiment. Declared costs fit
  the pooled average within 25% but deviate up to 2× per class and 24× for the verifier on
  late-anchor streams; a selective policy could be misbilled in its favour. Plan section 5/A4
  now requires per-call boundary timing in the ledger and the results table.
- The verifier's quadratic shape is the world checker's, not the component's. Plan item A5b:
  optimize behind an equivalence test with the current function kept as reference; required
  before B1.
- The lookup's `Resource::Memory` per-read charge was removed: `Memory` means resident memory,
  measured by the runner. Record reads remain charged as Compute.

**Accepted with notes.**

- The heuristic proposes "no fault" on any symptom-free window and is right 100/100 on `NoFault`.
  That is a property of the physics (every fault permits silence), not skill. Analyses must
  read `NoFault` success together with the critical-miss rate on faulted classes.
- Prior records outside `StaleMemory` are right about 1 time in 5, so the lookup is mostly
  noise. EXP-004 needs a memory that is sometimes useful; revisit the world's record generator
  when E2 is designed, as a new world revision, not by editing A1's guarantees in place.
- Worker subagents were refused `taskset` by their sandbox; the coordinator is not. Pinned
  measurement runs (B1 onward) are launched by the coordinator through `scripts/cgroup-run.sh`.

## A2 evaluator — merged

**Re-verified.** 118 workspace tests, clippy with all features, oracle guard. Eight randomly
sampled fixtures recomputed by hand from `RULES.md` without reading the scorer; all matched.

**Accepted with notes.** A `Correct` on a `NoFault` episode followed by `Abstain` scores both
success and false alarm. Each preregistration must state whether false alarms enter its primary
outcome. `Correct` reveals whether the site was right, at three probes; baselines should show
whether any arm uses it as an expensive probe.

## A1 small world — merged

**Re-verified.** Acceptance commands rerun on the branch and on merged main: 67 tests with all
features, clippy with warnings denied, oracle guard, dump sha256 identical across runs and across
the lockfile regeneration (`8be119bf…51fe2`).

**Independent check.** Plug-in mutual information between the true fault kind and coarse public
views of the stream (signal set, signal count, counter abnormality pattern, message severities),
3,000 episodes per class for Ambiguous, FeedbackBait, StaleMemory and JointlyDecisive, against
50 permutation baselines. No view exceeded its baseline. Ordered-sequence views had too many
distinct values for this test to discriminate; the worker's same-seed kind-swap test covers that
case by construction. Not checked: higher-order statistics, timing side channels in code.

**Accepted with notes.**

- Truth priors inside ambiguity sets are non-uniform (Ambiguous: ConfigDrift 40%,
  ResourceExhausted 40%, CredentialExpired 20%; JointlyDecisive 60/40). A learned policy can
  exploit this. It is a property of the environment distribution, not leakage, but B4 must
  report effective ambiguity, and A5 components must not bake the generator's priors in.
- JointlyDecisive defeats one-step selection on the decision value but not entropy-based
  information gain over (kind, bit) worlds. That distinction is itself testable in EXP-002.
- Core stays dependency-free; world keeps its serde shims (`BudgetSpec`, `CostSummary`). Revisit
  only if the shims cause a bug.

**Carried forward.**

- To A4: `Episode` derives `Serialize` including hidden state. Never serialize an `Episode` into
  any channel a policy can read. The recorder writes the public stream and the evaluator's
  verdict, not the episode.
- To A5: an empty `consistent_hypotheses` set means a bounded window dropped evidence, not a
  contradictory world.
- To A2: score from `(Truth, trajectory)` rather than `Episode`, so hand-written fixtures stay
  independent of the generator.

## A7 analysis — merged after one revision

**Re-verified.** 120 tests with `-W error`; no build artefacts committed.

**Revision requested and delivered.** The first submission could not test the charter's
EXP-001 cost measure (a ratio of totals) and could label a tiny sample "equivalent" with only a
warning. Added: paired-bootstrap ratio of totals with a threshold decision; a preregistered
sample-size gate forcing `unresolved` below plan, raw category still shown.

**Accepted with a hard follow-up.** The percentile bootstrap on the ratio is anti-conservative on
skewed costs (7–12% false exceedance at nominal 5%). Recorded as plan item A7b; EXP-001 may not be
frozen until it passes.

## A3 cost bill — merged

**Re-verified.** 39 tests; read `charge_recorded` and `replay`: a charge is applied only after
both budget acceptance and ledger append succeed, and replay uses the same atomic path.

**Carried forward to A4.**

- Build a `Bill` only from a fresh `Budget`. A pre-spent budget silently breaks
  sum-over-phases = total.
- Keep exactly one `Bill` per `Ledger`. `Bill::replay` folds every Accounting entry it finds.

## Process note

One coordinator merge (A7) hit a `.gitignore` conflict; a non-fail-fast command chain then
committed the conflicted tree locally. It was caught before push and repaired. Coordinator
merges now run under `set -euo pipefail`.
