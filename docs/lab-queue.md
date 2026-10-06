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

Status is one of `queued`, `running`, `reported`, `merged`, `rejected`.

| Id | Lab | Title | Depends on | Status |
|---|---|---|---|---|
| M1 | 1 | The medium crate: types, tick, archetypes, ports, determinism | — | merged |
| B1 | 2 | The `Noticer` seam, notice measures in the evaluator, public noticing baselines | — | merged |
| W1 | 3 | Event statistics per tick length; sample-efficiency and energy-proxy measures | — | merged |
| M1b | 1 | The oscillome: nested oscillations, phase gates, binding by phase, local oscillators, schedules; calibrated prices | M1 | merged |
| B2 | 2 | Site check and notice precision in the evaluator; notice-relative selection delay; a public later-re-anchor noticer | B1 | queued |
| M2 | 1 | The medium as a noticer on the stream world, against the public baselines | M1b, B1 | running |

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
**`RungNoticer` at z = 2 from B1's table** (anchor-correct 0.914 [0.890, 0.937]; leak noticed
0.460; 9.03 background notices per stream). *Amended 2026-10-06, before any M2 code or run:* the
original wording, "the best public noticer by that result's own measure", would have admitted a
flooding noticer (B1's sensitivity rows: 588 background notices per stream, anchor-correct
0.984), under which both results are unreachable; the review log records the error.

1. **Anchored noticing on hard incidents, slow leak excluded:** the medium's anchor-correct
   noticed share exceeds the comparator's by at least 0.03, with the paired lower bound above
   0.01, with the medium's notices on background per stream not exceeding 9.03.
2. **Noticing the slow leak:** the medium's leak noticed share exceeds the comparator's by at
   least 0.20, with the paired lower bound above 0.10, under the same background bound.

Reported beside, never folded in: leak anchor-correct (0.000 for every public noticer), notice
precision, notices per incident, notice latency, and hard-incident quality with the selection
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
