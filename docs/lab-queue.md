# Labs and the experiment queue

Three labs. The agent working in a lab is its principal investigator (PI), as AGENTS.md's "Labs"
section defines the role: it owns one queued unit at a time in its own worktree and branch, with
the scientific execution of that unit, and reports with its own analysis.
The chief researcher designs the units, fixes each criterion before the unit runs, queues units to
labs, verifies every report independently, merges what survives, and analyses results across
units and perspectives (`docs/review-log.md`).

## Lab assignments

| Lab | Focus | PI model | File territory (no other lab edits these) |
|---|---|---|---|
| Lab 1, Substrate | the medium and its adapters | opus | `crates/gordian-medium/`, later `crates/gordian-run/src/stream/arms/medium*.rs` |
| Lab 2, Instruments and baselines | the harness seam, evaluator measures, public baselines | sonnet | `crates/gordian-run/src/stream/`, `crates/gordian-stream-eval/`, `analysis/gordian_analysis/` |
| Lab 3, World and measures | world statistics, tick-scale questions, resource and sample-efficiency measures | sonnet | `crates/gordian-stream/examples/`, `experiments/exploration/scripts/`, analysis reports |

Shared rules: `docs/local-test-plan.md` section 2 (resources) and "Sharing cores 0-2 between
concurrent workers". Workspace `Cargo.toml` is edited only by Lab 1 (adding its crate) in this
round.

## Queue

Status is one of `queued`, `running`, `reported`, `merged`, `rejected`, `stopped` (merged as left on the user's stop order; criterion undecided).

| Id | Lab | Title | Depends on | Status |
|---|---|---|---|---|
| M1 | 1 | The medium crate: types, tick, archetypes, ports, determinism | — | merged |
| B1 | 2 | The `Noticer` seam, notice measures in the evaluator, public noticing baselines | — | merged |
| W1 | 3 | Event statistics per tick length; sample-efficiency and energy-proxy measures | — | merged |
| M1b | 1 | The oscillome: nested oscillations, phase gates, binding by phase, local oscillators, schedules; calibrated prices | M1 | merged |
| B2 | 2 | Site check and notice precision in the evaluator; notice-relative selection delay; a public later-re-anchor noticer | B1 | merged |
| B3 | 2 | A public benign-value (ramp) noticer for the leak; a splitting noticer for the never-noticed | B2 | merged |
| B4 | 2 | A non-privileged selector; decoy-notice accounting; leak-versus-decoy separation | B3 | merged |
| M3 | 1 | Sub-tick support pruning; mutation tests of M2; strict precision as a bound | M2 | merged |
| M4 | 1 | The anchoring-against-precision frontier; a ramp inhibit that spares ramp-noticed anomalies; parity with the best public row on all three measures | M3 | stopped (tuning done, no held-out run) |
| L1 | 3 | The learned noticer: M2's graph with constants learned online from public history, against the frozen graph and the re-anchor | M2 | merged |
| C1 | 2 | An incremental-dataflow noticer: the public rules on a general incremental engine, against the medium | B4 | merged |
| B5 | 2 | A call-budgeted public selector; a delay sweep; feature AUCs at the ask instant | C1 | stopped (selector built, identity passed) |
| C2 | 2 | An expiry-driven score window; one relational rule on both substrates (folds or joins) | B5 | queued |
| M5 | 1 | A less redundant graph at M4's anchoring and precision, in counted operations and wall time against C1's rows | M4 | queued |
| L2 | 3 | A self-supervised reservoir (ESN) noticer: the learned public comparator for the learning claim | L1 | stopped (reservoir built, identity passed) |
| M2 | 1 | The medium as a noticer on the stream world, against the public baselines | M1b, B1 | merged |
| A1 | 1 | The engram: memory in the medium (A1a build and identity; A1b the run, criterion fixed after W2, E1 and A1c) | M3 | A1a merged; A1b queued |
| A1c | 1 | The recall gate on the public consistency checker; a two-site key; identity; the smoke against A1a's table | A1a | merged (negative branch) |
| A1d | 1 | The engram under a non-privileged selector: plain outcomes bound, memory speaks only where no later declaration stands, trace counters; identity; the smoke on decision columns | A1c | running |
| W2 | 3 | The learnable laws of the stream world, measured from the hidden side; the perfect-memory ceiling; a second world parameterisation | — | merged |
| W3 | 3 | World C (three times the hard share) for A1b's power; the phase-2-keyed collision floor | W2 | queued |
| V1 | 2 | Criteria as code: `scripts/criterion.py`, `experiments/criteria/`, back-tested on M2, B3 and L1 from their kept runs | — | merged |
| E1 | 2 | Memory measures in the evaluator (`recurrence_of`, unasked-correct, stale errors, calls per correct decision); the public record rung | V1 | running |
| A2 | 1 | Anticipation: hidden edges learned from co-alarm timing, predictions scored against the hidden graph | A1, E1 | queued |

**Priority after the resumption of 2026-10-07** (charter section 1.2): A1, W2, V1, E1, then A1b and
A2. C2 and M5 stay queued behind them; M4, B5 and L2 stay `stopped`, resumable from their origin
branches with their criteria as fixed. EXP-101's registration draft waits on B5 and comes to the
user before any freeze.

## M1 The medium crate (Lab 1)

Build `crates/gordian-medium` as `docs/medium-ports.md` sections 3 to 7, 9 and 10 specify. No
world adapter, no noticer, no learning. The deliverable is the crate, its tests, a short
`crates/gordian-medium/DESIGN.md` that records every place the build departed from
`docs/medium-ports.md` and why, and a cost micro-benchmark (criterion, already a workspace
dev-dependency) of one tick at 10, 100 and 1,000 active cells, run under `scripts/cgroup-run.sh`,
reported as measured ns per operation beside the declared prices.

**Acceptance (fixed 2026-10-06).** All section-10 tests pass; the workspace gates pass; the crate
depends on nothing beyond `gordian-core`, `serde` and `proptest`; the measured ns per operation
in the micro-benchmark is within a factor of 5 of the declared prices, or the report proposes new
prices with the measurement. A departure from the design is allowed when the report states it;
a silent departure is a rejection.

## B1 The `Noticer` seam, notice measures, public baselines (Lab 2)

The shared rung bundles noticing, attaching, concluding and declaring. Separate noticing so that
an arm can swap it, and make noticing measurable without replaying ledgers (R10 had to).

1. **Seam.** A `Noticer` trait in `crates/gordian-run/src/stream/arms/`: given the public view
   (held observations with the public rules' verdicts, the public graph, the instant), it yields
   noticed anomalies (anchor, site, attached observations) and retirements. The rung's current
   noticing becomes `RungNoticer`, the default. Everything downstream (working state, components,
   the shared rule, declaring, context builders) is unchanged. **Gate:** R6's held-out run at
   b = 5, ρ = 0.7 replays byte-identical for all 62 arms against
   `experiments/exploration/r6-results-sha256.csv`.
2. **Measures.** The harness records every notice and retirement (anchor, site, instant, the
   noticer's id) in the run output, and the evaluator (`gordian-stream-eval`) scores per incident:
   noticed (any notice whose anchor belongs to the incident), notice latency from the incident's
   first observation, **anchor correctness** (the notice's anchor belongs to the incident and is
   within 1 s of its first observation), and per stream: notices on background, notices on plain
   incidents, notices per incident. Hand-written fixtures, mutation-checked as A2. The results
   schema gains these columns; `analysis/` loads them.
3. **Baselines**, each a `Noticer`, public information only:
   - `RungNoticer` at the default threshold and at z = 2 (R10's sweep).
   - `ChangeTriggered`: notices on the first abnormal observation at a node after a quiet period
     of `q` seconds, anchored there; the charter's change-triggered baseline, at the noticing
     level.
   - `EarliestAnchor`: the rung's noticer with its anchor moved to the earliest abnormal
     observation at the anomaly's site within `l` seconds before the rung's anchor. This is the
     cheapest public fix for the mis-anchoring R10 found.
   Each has one or two parameters tuned on R6's tuning streams (31000–31099 region, as the R6
   scripts do) and evaluated on the 200 held-out streams at b = 5, ρ = 0.7 with the selection
   oracle and the rung's context, so that quality rows are comparable with R10.

**Acceptance (fixed 2026-10-06).** The byte-identity gate; the evaluator fixtures with mutation
checks; a table of the four noticers on the held-out streams with: hard non-leak noticed share,
anchor-correct share, leak noticed share, notices on background per stream, hard-incident
quality with the selection oracle, cost per stream. No claim is made about which is better; the
table is the comparator for M2.

## W1 Event statistics per tick length; sample-efficiency and energy-proxy measures (Lab 3)

Two small studies, analysis-side, for the medium's design and the charter's aim.

1. **Tick statistics**, from a new feature-gated example in `crates/gordian-stream` (allowlisted
   like `dump`), over 200 streams at the defaults: for tick lengths 100 ms, 500 ms and 2 s, the
   distribution of events per tick (all, abnormal by the public rules, per node); for each hard
   family, how many ticks separate the incident's first observation from its first abnormal
   observation and from the partner's first alarm; what share of a hard incident's decisive
   evidence falls in the same tick as its first observation. Report as tables and a short
   reading: which tick length loses which structure.
2. **Measures for the aim.** From existing R10 and R9 outputs (`artifacts/runs/r10`,
   `artifacts/runs/r9`, read in place): for every arm, (a) **sample efficiency**: correct
   hard-incident decisions per hard incident seen, cumulative over stream order, as a curve; (b)
   **energy proxy**: modelled nanoseconds per correct decision, with a declared conversion of
   modelled ns to joules stated as an assumption (one figure for the cheap rung's operations, one
   for a reasoner call), so that every later experiment can report joules per correct decision
   beside cost. The conversion is a placeholder the charter revision will fix; the report says
   so. Add both as functions in `analysis/gordian_analysis/` with tests.

**Acceptance (fixed 2026-10-06).** The example and its test; the tables; the two analysis
functions with tests; a report `experiments/exploration/w1-tick-and-measures.md` that states what
is measured and what is assumed.

## M1b The oscillome (Lab 1, after M1)

Build `docs/medium-ports.md` section 4b into `crates/gordian-medium`, as the M1 PI proposed in
`crates/gordian-medium/DESIGN.md` ("Rhythms"), with the open questions it listed decided here.
Names: module `oscillome`, types `Oscillome` and `OscillomeEngine`.

**Decisions (chief, 2026-10-06).**

1. **The fastest oscillation is the base tick, whatever its length.** The `Oscillome` lists the
   slower periods only (first set: 10 s, 100 s). The claim "the fastest rhythm is 0.1 s" is
   dropped; at ticks of 500 ms and 2 s, burst order lives in `offset_ns`, and the `Coincidence`
   archetype gains an offset-aware form that orders events within a tick by `offset_ns`.
2. **Seconds to ticks.** Delays round to the nearest tick with a minimum of one tick (a delay of
   zero ticks, "next pass", is never produced by conversion). Lookbacks and holds round up.
   Decays given as time constants are converted once at build time: `exp(−tick/τ)` by a pinned
   series in `f64`, rounded to `f32`, with pinned-bit tests. The report carries the conversion
   table per tick length and lists every delay that collapses to one tick.
3. **Phases** are computed in integer nanoseconds, `(tick × tick_len_ns) mod period_ns`, and
   divided once into `f32`. Periods need not be multiples of the tick; a boundary is the tick in
   which the cycle index changes, and the unevenness is reported.
4. **Binding by phase and the sliding window both exist** as modes of `Coincidence`, so the
   ablation can compare bins with windows.
5. **The `Oscillator`** (phase-reset form of `Latch`) schedules its own next wake by a delayed
   self-message; it never runs every tick. Its cost is per cycle and counted.
6. **The engine keeps a per-cycle summary** (counts and activity since the last boundary of each
   oscillation) and hands it to the plasticity adapter at the boundary named in the spec.
7. **Retirement.** A `Latch` whose hold expires emits a proposal of kind `retire` citing its
   anchor, so that the end of an anomaly can reach the effector without a heartbeat. This is the
   medium-side answer to M1's "silence never propagates"; M2 may still add a heartbeat event if
   it needs one, priced like any event.
8. **Prices.** `Prices::DECLARED` becomes 200 / 25 / 40 / 2 ns (update, traversal, routing,
   field read), the M1 calibration, with a test, and the old values recorded in `DESIGN.md`.
9. **Every 4b element is switchable off in the spec**, and with all off the medium produces
   bytes identical to M1's for the same spec and events.

**Acceptance (fixed 2026-10-06).** The M1 PI's proposed acceptance: phases as a pure function
of tick index and tick length (a property test against an integer-arithmetic reference);
identical bytes after snapshot and restore at a boundary and mid-cycle; the all-off identity
with M1 (a test); the conversion table per tick length in the report; the benchmark rerun under
the cgroup with the prices of decision 8, every measurement within 0.7–1.4 of its model; the
workspace gates. `DESIGN.md` records every departure and restates where the analogy breaks.

## B2 Notice precision, a notice-relative selection delay, and a public later re-anchor (Lab 2, after B1)

B1 found the rung's noticing gap is mostly mis-anchoring onto background strays just before an
incident (17 of 31 never-noticed incidents), that the cheapest public fix (an earlier anchor)
makes it worse, that anchor correctness is gameable by flooding, and that the quality column is
confounded by the rung's 6 s retirement under the selection oracle's fixed 16 s delay. B2 closes
the instrument gaps and builds the strongest public comparator for exactly the failure the
medium targets.

1. **Evaluator** (`gordian-stream-eval`, rules N13 onward, fixtures and mutation checks as B1):
   a **site check** (the notice's site equals the incident's site), and **notice precision** per
   stream (notices anchored on an incident of any tier over all notices) and per tier. The
   analysis loader gains the columns.
2. **Selection oracle option** `hold_until_asked`: an anomaly the oracle will ask about stays
   live until it is asked, so a noticer is not penalised for the rung's retirement. The delay
   is tuned per noticer on the tuning seeds (10000–10099). Quality is reported both ways (fixed
   16 s with retirement, as B1; tuned delay with the hold) so that the confound is measured,
   not assumed. R6's held-out run must still replay byte-identical (the option is off there).
3. **`ReanchorNoticer`**, public information only: the rung's noticer with a *later* re-anchor.
   When an anomaly anchored on an isolated abnormal observation (no other abnormal observation
   at its site within gap `g`) later attaches a burst that begins after the anchor, the anchor
   moves to that burst's first observation. Parameters `g` and the burst definition are the
   PI's; they are tuned on the tuning seeds under the background budget (≤ 9.03 background
   notices per stream, the M2 comparator's), and the PI records every reading.

**Acceptance (fixed 2026-10-06).** Byte identity on R6's held-out run; fixtures with mutation
checks; the B1 table extended with `ReanchorNoticer`, site-correct, precision, and the two
quality readings for every row, on the 200 held-out streams with 90% cluster-bootstrap
intervals. No claim about which noticer is better. If `ReanchorNoticer` reaches 0.944
anchor-correct within the budget, the chief re-fixes M2's comparator to it before any M2 run,
and that is recorded as the public status quo closing the gap.

## B3 A public leak noticer and a splitting noticer (Lab 2, after B2)

The re-anchor closed most of the burst-family anchoring gap and left the leak untouched (0.460
noticed, 0.000 anchor-correct for every abnormal-only noticer). Before the medium claims the
leak, the status quo gets its fair chance at it, and at the 8 never-noticed incidents.

1. **`RampNoticer`**, public information only: a per-node trend detector on counter values,
   benign readings included (the public rules' "abnormal" verdict is not its trigger), that
   notices when a node's reading has risen monotonically, or by more than a slope threshold,
   over a window, anchored at the earliest observation of the rise. It composes with the rung's
   noticer (both notice; the rung's downstream is shared). Parameters tuned on seeds
   10000–10099 under the background budget of 6.82 notices per stream.
2. **`SplitNoticer`**: when an anomaly's attached observations form two bursts at different
   sites separated by more than a gap, the later burst becomes its own anomaly anchored at its
   first observation. Public information only; tuned as above.
3. The B2 table extended with both and their composition with the re-anchor, on the 200
   held-out streams, with leak anchor-correct, strict precision and notices per incident.

**Acceptance (fixed 2026-10-06).** Byte identity on R6's held-out run; fixtures and mutation
checks for any evaluator change; the extended table with intervals; no claim about which is
better. If a public row reaches leak noticed ≥ 0.660 within the budget, the chief re-fixes M2's
result-2 comparator if M2's held-out runs have not started, and otherwise records the
supplementary comparison in both reports.

## M3 Sub-tick support, mutation tests, strict precision (Lab 1, after M2)

M2 holds at 100 ms and not at 500 ms or 2 s, and the lookback table shows why: the emitter's
support is cut at the tick edge, so a stray that shares a burst's tick is cited and moves the
anchor. M2 also opens more anomalies per incident than the re-anchor (strict precision 0.50
against 0.67), which costs reasoner calls under the selection oracle.

1. **Sub-tick support pruning** in `crates/gordian-medium`: a support lookback expressed in
   nanoseconds and applied by `offset_ns` within the tick, so the anchoring rule's floor is no
   longer the tick. Off by default; the all-off identity with M1b's bytes holds (a test).
2. **Mutation tests** of the M2 adapters, graph and noticing code (cargo-mutants as Lab 2 does),
   with every survivor either killed by a test or recorded as equivalent.
3. **Strict precision as a tuning bound**: the graph retuned on seeds 10000–10099 at each tick
   length under both bounds, background ≤ 6.82 and strict precision ≥ 0.67 (the re-anchor's),
   frozen before the held-out run.

**Criterion, fixed by the chief before any M3 code or run (2026-10-06).** M2's two results, same
comparator (`ReanchorNoticer`, or the B3 row if the chief re-fixes it before M3's held-out
run), same margins and bounds, plus strict precision ≥ 0.67, at 500 ms and 2 s. "Holds" means
result 1 holds at both longer ticks; result 2 and 100 ms are reported for continuity. Cost per
stream, including reasoner calls, beside every row. Feasibility: M2's 500 ms row missed the
margin by 0.0004 with the tick-edge mechanism identified, and the 2 s row lost 31 incidents to
it; both outcomes are reachable.

## L1 The learned noticer (Lab 3, after M2)

The first reading of the charter's aim proxy 2 (section 1.1), improvement per unit experience.

1. **The arm.** M2's frozen graph with its tuned constants (the coincidence window, the onset
   integrator's time constant and threshold, the ramp detector's threshold, the emitter's
   lookback) replaced by parameters that start from public priors and are adjusted online from
   the stream's own public history, with no evaluator feedback and no hidden labels: the
   plasticity adapter, run at the 10 s rhythm's boundaries, updates them from per-cycle
   summaries (rates of abnormal observations per node, burst spacings observed, ramp slopes
   observed). The update rule is the PI's and is recorded before any run; it may use nothing
   the public rules and the stream do not give.
2. **Controls on the same stream order:** the frozen hand-designed graph (M2's), the
   re-anchor, and the learned arm with its learning switched off at its priors.
3. **Measures:** W1's sample-efficiency curve per arm (anchor-correct and leak-noticed decisions
   per hard incident seen, cumulative over stream order), the slope over the first 50, 100 and
   200 streams, and the end state's anchor-correct and leak-noticed shares; background and
   strict-precision bounds as M2.

**Criterion, fixed by the chief before any L1 code or run (2026-10-06).** The learned arm
counts toward the aim if, at 100 ms on 200 fresh streams (seeds 40000–40199, never used for
tuning), its anchor-correct share over the last 100 streams is at least the frozen graph's
over the same streams minus 0.01, with the paired lower bound above −0.03, **and** its slope
over the first 100 streams is positive with the lower bound above zero, **and** the
learning-off control's end state is below the frozen graph's by at least 0.02. The three
clauses are conjunctive; each is reported. Feasibility: the frozen graph is at 0.992 and the
priors can be set anywhere below it, so a slope is measurable; the first clause asks the
learner to reach the hand design, not beat it. A learner that matches the hand design from
weaker priors within 100 streams is the claim; one that does not is a result.

## B4 A non-privileged selector, decoy accounting, leak-versus-decoy (Lab 2, after B3)

B3 showed that the background budget is blind to notices anchored on decoys and late plain
incidents (of the 510 notices the ramp adds per 200 streams, 154 are decoys and 192 late
plain), and that the selection oracle hides their cost because it never asks about them. The
reframed EXP-101 charges every notice through a public selector. B4 builds that instrument.

1. **Non-privileged selectors** as `EscalationRule`s over any noticer's anomalies: a public
   threshold rule (escalate when the rung's conclusion is contradictory or silent for `t`
   seconds after notice) and a change-triggered rule (escalate once per anomaly when its
   attached evidence grows by `k` observations after notice), each tuned on seeds 10000–10099
   for hard-incident quality per unit cost, with the rung's context and R5's delay. The
   selection oracle stays as the labelled ceiling.
2. **Evaluator columns**: notices on decoys per stream and per decoy, escalations on decoys and
   on plain incidents per stream, and their cost share; fixtures and mutation checks as before.
3. **Leak versus decoy**: a public follow-up rule on a ramp-noticed anomaly, from readings
   after the first five (a decoy's readings turn benign; a leak's keep rising), that retires
   the anomaly or keeps it; tuned under the same budget; reported as decoy notices retired
   before escalation and leaks wrongly retired.
4. **Table**: every B3 row and M2's medium at 100 ms (read from `artifacts/runs/m2`) under each
   public selector: hard-incident quality, critical misses, plain accuracy, calls per stream,
   cost per stream, escalations on decoys and plain, with 90% cluster-bootstrap intervals and
   paired differences against ramp + split over the re-anchor.

**Acceptance (fixed 2026-10-06).** Byte identity on R6's held-out run; fixtures and mutation
checks; the table above; no claim about which arm is better. This table is EXP-101's
feasibility check: its numbers set EXP-101's margins before the freeze.

## M4 The anchoring-against-precision frontier (Lab 1, after M3)

M3 showed that sub-tick pruning removes the tick floor and that the precision devices (cluster
merge, confirmation in event time, the ramp inhibit) trade anchoring and leak noticing for
precision. M4 maps the trade and fixes the one named cause of the leak loss.

1. Each precision device switchable in the spec; the ramp inhibit gains a form that does not
   suppress the ramp detector when the open anomaly is itself ramp-noticed.
2. Tuning on seeds 10000–10099 at 500 ms (the tick where M3 reached public parity on
   anchoring; 100 ms and 2 s reported for continuity) over the device switches and the existing
   grids, with the tuning rule written before the run: the objective is the criterion's own
   conjunction, and ties break on the criterion's measures in the order anchoring, leak,
   precision; background is a bound only. Frozen with a named commit before the held-out run;
   byte identity on R6's held-out run.
3. Held-out run on the 200 streams: the frozen medium, each single-device-off ablation, M3's
   frozen 500 ms medium, M2's 100 ms medium, the re-anchor and ramp + split over the re-anchor.
   The frontier (anchor-correct against strict precision, with leak noticed as a third axis) from
   the tuning streams is reported as a table.

**Criterion, fixed by the chief before any M4 code or run (2026-10-07).** At 500 ms on the 200
held-out streams, against ramp + split over the re-anchor (anchor-correct 0.973, leak noticed
0.986, strict precision 0.693, background 6.24), the frozen medium meets all four: anchor-correct
≥ 0.963 with the paired lower bound above −0.03; leak noticed ≥ 0.976 with the paired lower
bound above −0.03; strict precision ≥ 0.693; background notices per stream ≤ 6.24. Conjunctive.
Feasibility: M3's 500 ms medium meets the anchoring and precision parts and misses the leak by
0.094 with a named cause; both outcomes are reachable. Cost per stream and leak anchor-correct
are reported beside, never in the criterion.

## B5 A call-budgeted public selector, a delay sweep, feature AUCs (Lab 2, after C1)

B4 showed that public selection rules on this world are nearly always-escalate, because the
cheap rung contradicts almost every plain anomaly; the cost column under them is the shared
plain-incident bill. EXP-101's cost axis is therefore call-budgeted: arms compared at a matched
number of reasoner calls per stream, asking about the anomalies a public score ranks highest.
B5 builds that selector and the two measurements EXP-101's margins need.

1. **The budgeted selector** as an `EscalationRule`: a public score per live anomaly at the ask
   instant (from the rung's own state: contradiction, silence, abnormal count, services
   involved, age, and the noticer's strict-precision proxy where it exists), a per-stream budget
   of `k` calls, ask about the top `k` by score as they become ready; `k` ∈ {2, 4, 8, 16} and
   the score's weights tuned on seeds 10000–10099 for verified decisions (plain and hard) per
   stream at each `k`, rule fixed before the run.
2. **A delay sweep** on the comparator (ramp + split over the re-anchor), the re-anchor and the
   medium at 100 ms and 500 ms (M2's and M3's frozen media): delays 8, 12, 16, 20 s after
   notice, with the selection oracle, so that the 16 s delay is no longer a hidden selector.
3. **Feature AUCs** at the ask instant, hard against plain anomalies and leak against decoy, for
   each public score feature, on the tuning streams, so that the selector's ceiling is known.
4. **The table** on the 200 held-out streams: every B3 row, C1's dataflow row (billed) and the
   B3 row unbilled, M2's and M3's media, under the budgeted selector at each `k`: verified
   decisions per stream (plain and hard), critical misses, hard quality, calls and cost per
   stream, with 90% cluster-bootstrap intervals and paired differences against the comparator at
   the same `k`; the selection oracle as the labelled hard-quality ceiling.

**Acceptance (fixed 2026-10-07).** Byte identity on R6's held-out run; the selector's readings
and tuning rule fixed before the run; the table; the delay sweep; the AUCs; no claim about which
arm is better. The table's paired half-widths set EXP-101's margins, which the chief fixes in the
registration draft after this unit.

## C1 An incremental-dataflow noticer (Lab 2, after B4)

The medium's one structural claim is sparse, event-driven computation: cells run only with
input, and messages travel only where synapses carry them. The public noticers that matched or
beat it (re-anchor, ramp, split) are hand-written incremental rules. The fair adversary for the
structural claim is the same rules expressed on a general incremental engine, so that what is
measured is the mechanism's generality and hand-design burden, not one author's code.

1. **Engine.** Timely/differential dataflow (Rust) or a hand-written incremental relational
   core, chosen under the dependency rule: the stated requirement is incremental maintenance of
   joins and windows over persistent facts; the simpler alternative is the rung's existing
   hand-written incrementality; the cost is the crate's size and determinism obligations. The PI
   records the choice and reason before building. Determinism is absolute: the noticer must be
   a pure function of the public stream; no threads inside the arm, no hash iteration order in
   any output.
2. **The noticer.** `DataflowNoticer`: B3's ramp + split over the re-anchor expressed as
   incremental relations (facts: observations; derived: bursts, isolated anchors, chains,
   splits, notices, retirements), producing notices through the `Noticer` seam. It must
   reproduce the B3 row's notice record on the 200 held-out streams exactly (a test), or the
   report lists every difference and why.
3. **Measured beside the medium at 100 ms** (M2's arm) on the same streams: anchor-correct,
   leak noticed, leak anchor-correct, background notices, strict precision, the noticer's own
   cost per stream (counted operations priced like the medium's, with a declared price table
   calibrated by a micro-benchmark under the cgroup as M1 was), measured wall time, and lines
   of rule code against lines of graph code.

**Criterion, fixed by the chief before any C1 code or run (2026-10-06).** C1 is an instrument
and a baseline; its criterion is reproduction and cost. It passes if the dataflow noticer's
notice record equals the B3 row's on the held-out streams (or every difference is explained)
and its own cost per stream is reported beside the medium's with calibrated prices. No claim
about which is better; the comparison enters EXP-101's table as a labelled row.

## L2 A self-supervised reservoir noticer (Lab 3, after L1)

The learned public comparator for the learning claim. L1 asks whether the medium's constants
can be learned online from public history; L2 asks whether a generic cheap temporal learner
does as well with no hand-designed graph at all.

1. **The arm.** An echo state network per node (or one shared reservoir with node inputs):
   fixed sparse random recurrent weights from a seed in the manifest, inputs are the node's
   public readings at each tick (values, abnormal-kind tags), and a linear readout trained
   online by recursive least squares to predict the next tick's readings. The stream carries no
   labels, so the readout is self-supervised; the noticing signal is the prediction residual,
   thresholded and anchored at the earliest tick of a residual run. Reservoir size, leak rate,
   spectral radius and the residual threshold are tuned on seeds 10000–10099 under the
   background bound (≤ 6.82) and strict-precision bound (≥ 0.67); the update rule and priors
   are written before any run.
2. **Controls on the same stream order** (seeds 40000–40199, as L1): L1's learned medium, M2's
   frozen graph, ramp + split over the re-anchor, and the ESN with learning off (readout at
   its initial weights).
3. **Measures:** L1's (sample-efficiency curves, slopes over the first 50, 100 and 200 streams,
   end states over the last 100), with the ESN's own cost per stream counted and priced.

**Criterion, fixed by the chief before any L2 code or run (2026-10-06; clause 2 amended
2026-10-07 after L1, before any L2 code, because a 100-stream slope of a cumulative ratio
cannot see learning that completes in a few streams).** On seeds 40000–40199 at 100 ms:
(1) the ESN's anchor-correct share over the last 100 streams is at least the frozen graph's
minus 0.01 with the paired lower bound above −0.03, **and** its background notices per stream
are ≤ 6.82 and its strict precision ≥ 0.67 over the same streams; (2) its background notices
per stream over streams 21–40 are at most the learning-off control's minus 10 with the paired
lower bound below −5 (the endpoint, stream 20, is fixed here); (3) the learning-off control's
end state is below the frozen graph's by at least 0.02. The three clauses are conjunctive. A
fourth, reported separately: the ESN's end state against L1's learned medium, paired. Feasibility: a reservoir's residual on this world's
bursts and ramps is a plausible noticing signal and an implausible anchoring signal at 100 ms
resolution; both outcomes are reachable, and an ESN that notices but mis-anchors is a result.

## M2 The medium as a noticer (Lab 1, after M1 and B1)

A hand-designed noticing graph on the medium (with the oscillome of M1b), fed by the stream sense adapter, emitting notices
into B1's `Noticer` seam, compared with B1's public noticers on the same held-out streams with
the selection oracle and the rung's context.

**Build (Lab 1, after M1b and B1; fixed 2026-10-06).**

1. **Sense and clock adapters** in `crates/gordian-run/src/stream/arms/medium/` (Lab 1's
   territory for M2): every `StreamEvent::Observed` becomes an `Event` with its value, benign
   readings included (the synthesis entry in the review log says why: the slow leak is benign
   under the public rules), its public-rule verdict as a tag, the service as the address node,
   the channel as counter, message, snapshot or probe; message ids folded to a `Tag` with the
   collision rule stated. The tick length is a manifest parameter; the clock adapter derives
   the tick from the harness's logical instant.
2. **Effector adapter** into B1's `Noticer` seam: proposals of kind `notice` become notices
   (anchor, site from the anchor's address, attached = the proposal's refs); `retire` proposals
   become retirements. The medium is the arm's noticer; everything downstream is the shared rung,
   the rung's context, and the selection oracle at R5's delay, as the comparator rows use.
3. **Ledger adapter**: the medium's operation counts at the calibrated prices (200 / 25 / 40 / 2)
   plus 200 ns per tick, charged to the arm's bill like a component call. The stream's hard
   limits stay on.
4. **The noticing graph**, hand-designed, public information only, with `max_passes` 3: sense
   cells per (node, channel); per-node integrators and novelty cells with time constants in
   seconds; coincidence across the public graph's neighbours (sliding, binned and ordered forms
   available); latches with retirement; emitters whose lookback is a tuned parameter. Designed
   and tuned on the tuning seeds 10000–10099 at each tick length, frozen (commit named) before
   any held-out run; the PI records every structural choice and what it was for.
5. **Runs** through the driver at b = 5, ρ = 0.7 on the 200 held-out streams: the medium at
   each tick length beside `RungNoticer` z = 2 and z = 3 (the comparator rows, rerun so the
   table is one run), plus the oscillome ablations as labelled arms at the best tick length
   (rhythms off; phase gates off; coincidence sliding instead of ordered or binned; oscillators
   off), and a medium with an abnormal-only sense adapter as a labelled control for the
   synthesis decision.
6. **Report** `experiments/exploration/m2-medium-noticer.md`: the criterion as written at each
   tick length with paired intervals; everything "reported beside"; anchor correctness as a
   function of the emitter lookback; latency per tick length; the medium's cost column; the
   ablations; the PI's analysis.

**Criterion, fixed by the chief before any M2 code or run (2026-10-06).** Two separate results,
each named, at b = 5, ρ = 0.7, 200 held-out streams, paired 90% cluster bootstrap over streams,
at each tick length in {100 ms, 500 ms, 2 s}, against one named comparator for both results,
**`ReanchorNoticer` from B2's table** (anchor-correct 0.952 [0.932, 0.970]; leak noticed 0.460;
6.82 background notices per stream). *Re-fixed 2026-10-06 under B2's acceptance clause, before
any M2 held-out run, from `RungNoticer` z = 2 (0.914; 0.460; 9.03); Lab 1 was told directly.
Feasibility: 18 of 372 hard non-leak incidents remain not anchor-correct for the comparator, so
result 1 means fixing at least 11 of them.* *Amended 2026-10-06, before any M2 code or run:* the
original wording, "the best public noticer by that result's own measure", would have admitted a
flooding noticer (B1's sensitivity rows: 588 background notices per stream, anchor-correct
0.984), under which both results are unreachable; the review log records the error.

1. **Anchored noticing on hard incidents, slow leak excluded:** the medium's anchor-correct
   noticed share exceeds the comparator's by at least 0.03, with the paired lower bound above
   0.01, with the medium's notices on background per stream not exceeding 6.82.
2. **Noticing the slow leak:** the medium's leak noticed share exceeds the comparator's by at
   least 0.20, with the paired lower bound above 0.10, under the same background bound.

Reported beside, never folded in: leak anchor-correct (0.000 for every public noticer), notice
precision and strict precision (B2's N16), notices per incident, notice latency, and hard-incident quality with the selection
oracle, whose fixed 16 s delay and the rung's 6 s retirement confound it (B1).

A result that holds at one tick length and not another is reported as such; "holds" for the
experiment means at the best tick length, with the sweep shown. Modelled cost per stream for the
medium, including its own operations, is reported beside every row and bounded by the stream's
hard limits; it is not in the criterion, and a medium that costs more than the rung is still a
result. Feasibility: R10's sweep shows the rung at 0.917 noticed and 0.46 on the leak at 4.0
background notices per stream, and the noticing oracle at 1.0; anchor-correct shares are
unknown until B1 and may be lower. Every outcome is reachable.

The medium's graph is designed on the tuning streams and frozen (commit named) before any
held-out run. It uses public information only. The PI records every structural choice and what
it was for.

## A1 The engram: memory in the medium (Lab 1, after M3)

The first unit of the purpose in charter section 1.2: experience converted into correct unasked
decisions. Two phases. **A1a** builds the mechanism and passes identity; **A1b** runs it against
the record rung once W2 has measured the world's recurrence and E1 has built the measures. A1a
has no criterion beyond its deliverables; A1b's criterion is fixed by the chief after W2 and E1
report and before any A1b run.

**A1a, the build.**

1. **The mechanism, in `crates/gordian-medium`.** A bind operation on the plasticity port: given
   a key (a public pattern the arm witnessed, as a set of features the PI defines) and an outcome
   (a diagnosis the reasoner gave), create or strengthen an engram: a latch cell keyed by the
   pattern with an emit that carries the outcome. Recall: a coincidence over the key's features
   fires the latch, and its emit carries the outcome with the site substituted from the firing
   support. Strength grows on each bind of the same key, decays at a rhythm boundary, and is
   weakened by contradiction (a later reasoner answer for the same key that disagrees). All of it
   deterministic, priced with the calibrated prices, persisted through the persist port across
   segments so experience accumulates in stream order (L1's `state_key` store is the precedent).
   Record the design in `crates/gordian-medium/DESIGN.md` before writing it.
2. **The adapter, in `crates/gordian-run/src/stream/arms/medium/`.** `Answered` events are the
   arm's own history and feed bind: the key is built from what the arm held about the incident
   the answer concerns (the abnormal kinds at the site, the message ids attached to it, the shape
   of the counters) and nothing else. A recall yields a `Declare` with no `Escalate`. The key
   definition and the confirmation policy (never confirm; confirm every k-th recall; confirm on
   contradiction) are written in the module documentation and committed before any run. **The
   key may use only public bytes.** Hidden-vocabulary message ids are public bytes once
   delivered; whether they mark a family is for the arm to find out from its history, never from
   the hidden record. The PI does not read `HIDDEN-DESIGN.md`; if it does, it says so.
3. **Site generalisation is a switch:** the key with the site as a variable (family-keyed) and
   the key with the site fixed (site-keyed), both built, because E1's record rung has the same
   two forms and the comparison is between like forms.
4. **Identity and gates.** R6's held-out manifest replays byte-identical for all 62 arms
   (`experiments/exploration/r6-results-sha256.csv`); the medium's existing identity tests pass
   unchanged; fmt, clippy, tests, the oracle guard. A smoke run on seeds 10000–10019 showing
   that recalls occur and declare, measured only by the existing per-incident columns
   (`correct_declarations` with `escalations` = 0), with no tuning against any measure.
5. **The report** records: the key definition, the confirmation policy, every place the build
   departed from `docs/medium-ports.md` and `DESIGN.md`, the smoke counts, what the PI is least
   sure of, and what A1b should measure that the chief's brief does not name.

**A1b, the run (rewritten after A1c, 2026-10-07; the criterion follows E1 and A1d).** A1b runs under B4's public selector, not the selection oracle, for every arm (review log, A1c): the memory must be able to bind plain outcomes and be contradicted. The selection-oracle rows are labelled ceilings. The first clause bounds displaced correct plain decisions (by deadline), paired against the memoryless arm under the same selector. Tuning on seeds 10000–10099 under a stale-error
bound and the cost column; held-out on seeds 40000–40199 in stream order; arms: the medium with
engrams (family-keyed and site-keyed), the medium with bind switched off, E1's record rung in
both forms, the re-anchor with no memory. Measures: E1's. Criterion to be fixed then, in
`experiments/criteria/a1b.json`.

## W2 The learnable laws, measured from the hidden side (Lab 3)

Experimenter-side measurement, nothing arm-side. Lab 3 reads `HIDDEN-DESIGN.md` and the stream
crate's internals; its scripts live in `crates/gordian-stream/examples/` and
`experiments/exploration/scripts/`, and every number comes with the seed range it was measured
on. Seeds 10000–10099 (tuning) and 40000–40199 (held-out) at the default parameters, reported
separately, and a second parameterisation (below).

1. **Recurrence.** Per stream: incidents, recurrences, recurrences by tier and family; for each
   recurrence, the index of the incident it repeats and the gap in incidents and seconds; the
   cumulative count of recurrences against incidents seen, averaged over streams (the
   experience curve a memory could at best follow); how many recurrences fall after a regime
   change that altered their family's physics (stale by construction).
2. **Same family, different site.** Per stream: non-recurrence hard incidents whose family and
   mode match an earlier incident at another site. This is what a family-keyed memory can reach
   and a site-keyed one cannot.
3. **The vocabulary.** From the public side only: for each stream, the mutual information
   between a message id and the hard family of the incident it belongs to, and the share of ids
   that appear at exactly one family; the same statistic over background free-form messages as
   the floor.
4. **Hidden edges.** Cascade root-to-partner delays; `EdgeAdd` instants and the number of
   incidents after each that alarm over the new edge; how often the same pair recurs.
5. **Regime changes.** Per stream: incidents affected by each change (by family), and the share
   of all hard incidents that fall after the first change.
6. **The perfect-memory ceiling.** On the selection oracle's own runs (R6's manifest at the
   default setting, or a fresh run of the re-anchor with the oracle on these seeds), the
   reasoner cost spent on incidents that are recurrences of an incident the arm declared
   correctly earlier: that is the most a site-keyed memory could save; and the same for
   family-keyed reach (item 2). Reported as calls, modelled ns, and as a share of the arm's
   total bill.
7. **A second parameterisation**, world B: recurrence 0.6, regime changes at 150 s and 300 s,
   same everything else, on seeds 50000–50099: items 1, 2 and 5 only. Its purpose is the later
   transfer test; nothing is tuned on it.

**Acceptance (fixed 2026-10-07).** Every number reproduces when the chief reruns the script
from the manifest; every statistic names its seed range and its side (hidden or public); the
report states which of items 1–6 bound A1b's feasibility and proposes, with its reasons, the
bounds the chief should fix for A1b's stale-error and unasked-correct clauses. Nothing in W2
reaches an arm.

## V1 Criteria as code (Lab 2)

The lesson of 2026-10-07 (charter section 11): the verdict is computed, not read.

1. **`scripts/criterion.py`**: given a criterion specification and one or more run directories,
   computes each clause and prints a verdict file (`verdict.json` and a Markdown table). A
   specification names: the run directories by role (arm, comparator, controls), the evaluator
   files and columns each measure reads, the measure (a share over incidents filtered by tier
   or family, a per-stream mean, a paired difference, a slope over stream order), the bootstrap
   (cluster by stream, resamples, seed, percentile), the bound or margin, and whether clauses
   are conjunctive. Measures are implemented once in `analysis/gordian_analysis/` and the
   script only composes them. Deterministic under a fixed bootstrap seed.
2. **`experiments/criteria/`**: a specification per criterion, starting with M2, B3's
   supplementary comparison, L1's three clauses and M3's four-part criterion, written from the
   queue's text without reading the review log's numbers first.
3. **Back-test.** Run each against the kept run directories (`artifacts/runs/m2`, `b3`, `l1`,
   `m3`) and compare with the review log's numbers.

**Criterion (fixed 2026-10-07).** The back-test reproduces every number the review log prints
for M2 (both results at all three tick lengths, with intervals and background), B3's table and
paired differences, L1's three clause verdicts and M3's verdict, to the printed precision, from
the raw files, under the script alone. Where it does not, the report says which number, which
side is wrong, and why; a discrepancy traced to the review log is a finding, not a failure of
V1. Mutation: six hand mutants of the measures (a wrong filter, an off-by-one in the window, a
dropped cluster) each change a verdict or a number.

## E1 Memory measures and the public record rung (Lab 2, after V1)

1. **Evaluator** (`gordian-stream-eval`): per incident, `recurrence_of` (the index of the
   incident it repeats, from the hidden side, or none), `same_family_earlier` (an earlier
   incident of the same family and mode at another site exists), `unasked_correct` (a correct
   declaration with no escalation whose focus belongs to this incident), `stale_wrong` (a
   wrong declaration with no such escalation), and per stream: unasked-correct share of
   recurrences, stale errors, reasoner calls per correct decision, and the experience curves
   over stream order that W1's machinery gives. Rules written into `RULES.md` with fixtures and
   mutation checks, as B1 did.
2. **The record rung**, a public arm: the re-anchor with a table from a key to the diagnosis
   last obtained from the reasoner for that key, consulted before escalating; two forms,
   site-keyed (site plus the public signature) and family-keyed (signature only, site free),
   and a confirmation policy with the same three options as A1's. The key uses only public
   bytes. Tuned on seeds 10000–10099 under a stale-error bound; its table on 40000–40199 in
   stream order is A1b's comparator row.
3. **Identity:** R6's held-out manifest replays byte-identical for all 62 arms.

**Criterion (fixed 2026-10-07).** Deliverables, identity, fixtures and mutation checks as B1's
standard; the record rung's held-out table reported with the W2 ceiling beside it. No claim.

**Amended after W2, before any E1 code (2026-10-07).** (a) The harness records, for every
declaration an arm makes from memory, the observation the memory was bound at (its source), in
the run output; the evaluator reads the source incident's truth and separates a wrong recall
whose source was right (collision or staleness) from one whose source was wrong (inherited).
`stale_wrong` is computed over recall-sourced declarations, and also as the paired excess of
unasked wrong declarations over the memoryless arm. (b) The record rung's family-keyed form uses
only stream-invariant public features (abnormal kinds, counter shapes, probe answers, timing),
never service or message ids, because both are regenerated per stream; its site-keyed form uses
ids inside a stream only. (c) The reset at the stream boundary is a switch on both forms, and the
held-out table reports each form with and without it. (d) The rung waits for evidence after the
first phase before recalling, and the module docs say what it waits for. (e) Its experience
curve across streams is reported for the family form (the only form that can have one).
(g) Amended after V1: the criterion schema gains, with tests, a `not` node and exclusive outcome
categories, not-null filters, count-with-total clauses, a per-stream quantile measure, and a
join of two arms by incident, so that `experiments/criteria/a1b.json` can express A1b's clauses.
(f) Amended after A1a: `results.csv` gains a `recall_declarations` column (declarations whose
source is `Source::Recall`, today folded into `cheap_declarations`), and `total_cost_ns` for
every arm includes the noticer's charge as a new column `noticer_ns` beside it, the existing
column unchanged so that byte identity holds.

## A2 Anticipation (Lab 1, after A1 and E1)

A hidden edge is learned from co-alarm timing in the arm's own history and used to predict the
partner's alarm before it arrives; predictions are recorded by the harness and scored against
the hidden graph. Brief and criterion to be written after A1b reports.

## W3 World C and the phase-2 collision floor (Lab 3, after W2)

1. **World C:** the default world with the hard share raised threefold (the mix parameter in
   `StreamParams`, nothing else changed), seeds 60000–60099 and 70000–70199, with W2's items 1,
   2, 5 and 6 (the ceiling with the same arm) measured on it. Its purpose is power for A1b: about
   120 reachable events instead of 40. Report whether anything else about the world changed with
   the mix (overlap, deadlines missed by the oracle arm, background share).
2. **The phase-2 collision floor:** W2's section 9 table recomputed for a key that includes the
   evidence classes that arrive after the first phase (the rule-breaking evidence, as the public
   rules define it), for the site-keyed and the invariant family-keyed forms, with and without
   the stream reset. This is the floor against which A1b's collision bound is read.
3. **Hidden-side only**, as W2; nothing reaches an arm.

**Acceptance (fixed 2026-10-07).** As W2's: every number reproduces from the script and the
manifest, names its range and side; the report says which of A1b's proposed bounds the new
floor moves and by how much.

## A1c The recall gate and the two-site key (Lab 1, after A1a)

A1a's smoke (review log) shows the generalising family form recalling on plain incidents 151
times more than the control in 20 streams, because "evidence after 2 s" is not the
rule-breaking evidence. No tuning; this is a mechanism unit with identity and a smoke.

1. **The recall gate.** A recall may fire only on an anomaly the public rules cannot explain:
   the rung's consistency checker (`AnomalyView::contradicted_since`, the public meaning of
   rule-breaking evidence). The gate lives in the arm (`arms/medium/`), where that view is
   visible. A recall gated so fires only where the cheap rung would have returned no
   hypothesis, so it displaces no correct cheap declaration; make that a test. The
   "late feature" requirement in the key stays as a switch, default off, reported both ways.
2. **The two-site key.** A key spanning the anomaly's service and one other service whose
   alarm the public graph does not connect to it, with the order and gap of their first alarms
   in bands, built from the invariant features of both. The engram's coincidence must span
   two nodes for this; record the design in `DESIGN.md` before the code.
3. **Identity:** R6's held-out replay 62 of 62; the existing identity tests unchanged; gates.
4. **The smoke:** A1a's manifest (seeds 10000–10019, the same eight arm roles plus the gated
   forms), reported beside A1a's table from `incidents.csv`, with the gated family form's
   plain unasked-wrong count as the first number. Nothing adjusted after seeing it.
5. **Report:** `experiments/exploration/a1c-recall-gate.md`, as A1a's.

**Acceptance (fixed 2026-10-07).** Deliverables, identity, the displacement test, the smoke
table. A gated family form whose plain unasked-wrong count exceeds the control's by more than
5 in 20 streams is reported as such and A1b's design is reconsidered before its criterion is
fixed; a count within 5 is the expected outcome. No claim either way.

## A1d The engram under a non-privileged selector (Lab 1, after A1c)

A1c showed the memory learns from a one-sided teacher: under the selection oracle only hard
incidents are asked about, so nothing plain is ever bound or contradicted. No tuning; a
mechanism unit with identity and a smoke on the decision columns.

1. **The selector.** The engram arm runs under B4's public selector (`public_threshold.rs`
   and `public_change.rs`, with B4's tuned constants as merged, nothing retuned), so that plain
   anomalies are sometimes asked about. Every answer, plain kinds included, feeds bind; the
   vote weakens a key whose outcomes disagree. Keep the selection-oracle form as a switch.
2. **Where memory may speak.** A recall declares only on an anomaly that carries no
   declaration made after the checker's last consistent verdict: memory corrects a cheap
   declaration the rules have since contradicted, and never adds to one that stands. Test it.
3. **Counters.** Through the medium's trace port into the arm's own trace file (not
   `results.csv`, which is E1's): recalls offered, gated, admitted, declared, with instants;
   binds by outcome tier as the arm sees it (kind only; the arm does not know tiers).
4. **Identity:** R6's held-out replay 62 of 62; A1a's and A1c's smoke arms reproduce exactly;
   gates.
5. **The smoke:** seeds 10000–10019, the memoryless arm and the engram forms (family,
   two-site, site) under the public selector and under the oracle, first values, nothing
   adjusted; reported on the decision columns (`correct_by_deadline`, `missed`,
   `critical_miss`, calls, cost) by tier, paired against the memoryless arm under the same
   selector, with the A1c declaration count beside for continuity.
6. **Report:** `experiments/exploration/a1d-negative-experience.md`.

**Acceptance (fixed 2026-10-07).** Deliverables, identity, the test in item 2, the smoke
table on decision columns. The expected outcome is stated as a prediction by the PI before
the run; whatever the table shows is reported. No claim.
