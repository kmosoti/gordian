# A2, anticipation of hidden edges — Lab 1 report

Exploration. A2's acceptance (`docs/lab-queue.md`, "## A2", fixed by the chief before the unit) is
deliverables, identity, the PI's prediction committed before the run, and the trace file of
predictions joinable by Lab 3. No claim, no tuning. I did **not** read
`crates/gordian-stream/HIDDEN-DESIGN.md`.

(The report proper is written after the run, below the prediction. The prediction section is
committed before any A2 run and is not edited afterwards.)

## Prediction (committed before any run)

Settings: `experiments/exploration/scripts/a2_common.py` (the same numbers are in `PREDICTION`
there). Seeds 10000–10019 in stream order, b = 5, rho = 0.7, the selection oracle at 16 s with the
rung's context, M3's frozen 100 ms medium under every arm. Arms: `m3` (no layer), `a2_learn` (the
anticipation layer at its first values, attach off, trace on: **the learner**), `a2_raw` (the
learner with `explained` off: every first alarm counts; a labelled control). Numbers are per
stream means over the 20 streams with my 80% ranges; probabilities are mine. "Edges learned" are
distinct ordered pairs `(a, b)` held at one or more reads in a stream (a pair is read at every
trial at `a`); "follow rate" is the public-side share of predictions followed by a first alarm at
the predicted service within the band.

**The model behind the numbers.** The rule counts follows beyond chance (departure 83), so with no
structure in the world a pair cell is a zero-mean walk and an edge is held only by its fluctuations
above 2. How often depends on two quantities I do not know and did not measure: `h`, the rate at
which a quiet service's counted (unexplained) bursts begin, and `n`, the trials per pair per
stream. From W1's published public statistics (an abnormal observation per service every 3.4 s on
average, 37.5% of 2 s cells holding one against 44% for a Poisson process of that rate, so mild
clustering), I put `h` at 0.1 to 0.3 per quiet second, centred at 0.2, and `n` at about 0.6 of
the predicting service's counted first alarms (`h` times its quiet time, roughly 360 s), so 20 to
60, centred at 40. The null model (`scripts/a2_null_model.py`, run before any A2 run, output
below) gives, for the rule as built:

| h, trials per pair | edges per stream (0.4 s, 2 s, 10 s) | predictions per stream (0.4 s, 2 s, 10 s) | follow rate (0.4 s, 2 s, 10 s) |
|---|---|---|---|
| 0.1, 20 | 1.7 (0.05, 1.2, 0.7) | 3.8 (0.1, 2.5, 1.2) | –, 0.34, 0.79 |
| 0.1, 40 | 10.4 (0.6, 5.6, 6.7) | 46 (1.1, 18.6, 26.6) | 0.00, 0.18, 0.65 |
| 0.2, 40 | 13.4 (2.5, 12.3, 0.9) | 63 (6.8, 54.0, 2.5) | 0.09, 0.30, 0.92 |
| 0.2, 60 | 23.3 (5.0, 20.6, 8.2) | 228 (23.4, 170, 34.6) | 0.08, 0.33, 0.84 |
| 0.3, 60 | 26.6 (9.4, 25.2, 0.6) | 328 (48, 279, 0.8) | 0.12, 0.46, 0.88 |

The world is not the null: incidents and background bursts cluster in time across services, which
gives every pair a small positive excess and raises the counts, and the hidden edges add a little
real structure, which W2 measured at about one alarm over an added edge per stream and a cascade
pair recurring once in twelve streams; no rule of this kind learns an edge from one sighting
without also learning noise, so I expect the true edges to be a small share of what is learned.
The 2 s band is where the null model's fluctuations most often cross the threshold (its chance is
near one half, where a zero-mean walk of bounded steps is widest); the 0.4 s band needs about two
follows close together (chance near 0.05); the 10 s band's chance is near 1, its steps toward the
threshold are small, and it crosses rarely.

**The learner (`a2_learn`), per stream:**

| Quantity | Central | 80% range |
|---|---|---|
| ordered pairs with pair cells | 33 | 25–40 |
| edges learned, any band | 12 | 3–28 |
| edges learned, 0.4 s band | 2 | 0–8 |
| edges learned, 2 s band | 10 | 2–25 |
| edges learned, 10 s band | 1 | 0–10 |
| predictions | 60 | 8–350 |
| share of predictions in the 0.4 s / 2 s / 10 s band | 0.12 / 0.80 / 0.04 | 0.03–0.30 / 0.55–0.95 / 0–0.15 |
| public follow rate, 0.4 s band | 0.10 | 0.03–0.30 |
| public follow rate, 2 s band | 0.33 | 0.18–0.50 |
| public follow rate, 10 s band | 0.85 | 0.60–0.97 |
| `bill_compute` added, ms per stream | 2.0 | 1.3–4.0 |

**Probabilities:**

- The follow rate of predictions in each band is within 0.10 of the follow rate of all trials in
  that band (every quiet partner of every counted alarm; the public chance level): **0.75**. That
  is, on the public side the predictions are at chance.
- The follow rate of all trials is at least the mean chance the layer assigned, in each band (the
  world's clustering gives a positive excess the Poisson chance does not include): **0.6**.
- Lab 3 will find fewer than 5% of the learner's predictions on a true hidden edge (cascade
  partner or added edge): **0.8**. (I cannot check this; it is stated so that W3's scoring tests
  it.)
- The learner's decision columns (`incidents.csv`: correct and wrong declarations, correct by
  deadline, missed, critical miss, escalations) equal the memoryless arm's for every incident:
  **0.9**. The timing columns may differ by microseconds (the layer's charges advance logical
  time, as A1c's monitoring did); a difference there is not a change of decision.
- No compute refusal, no truncated tick, every segment to the horizon: **0.95**.

**The control (`a2_raw`, `explained` off):** siblings under a common upstream co-alarm within
0.4 s on every incident there, and with the explanation off those alarms count. More 0.4 s-band
edges than the learner, at least twice as many: **0.7**; a higher 0.4 s-band follow rate than the
learner's: **0.7**; predictions per stream 150 (80%: 20–600).

**What would surprise me** (and what it would mean): a 0.4 s-band follow rate far above the all
trials' rate for the learner (structure the public graph does not explain, which is what the unit
is for); fewer than one edge per stream (the world quieter than W1's numbers suggest, or the
explanation removing most alarms); more than 300 predictions per stream (strong clustering, so the
Poisson chance is badly calibrated, which the second probability tests).

## Result in one paragraph

The learner held **5.4 edges per stream** (1.2 in the 0.4 s band, 4.85 at 2 s, 0.3 at 10 s) out
of 36.5 ordered unconnected pairs, and made **12.6 predictions per stream** (1.9, 10.1, 0.55 by
band). On the public side **60 of 251 predictions (0.24) were followed** within their band: 0.079
at 0.4 s, 0.243 at 2 s, 0.727 at 10 s. The all-trials follow rate in each band, the arm's own
public chance level, is 0.052, 0.199 and 0.527, and a permutation over the other unconnected
services gives 0.052, 0.222 and 0.533. The predictions are at the public chance level within
sampling noise at 0.4 s and 2 s; at 10 s, 8 of 11 against 0.53 is not resolvable. Every
quantity with a range in the prediction fell inside it, most below its central value; of the
stated probabilities, two that could be checked held and two failed (below). The learner's
decision columns equal the memoryless arm's for all 523 incidents. Whether any learned edge is a
hidden edge is Lab 3's to score.

## Commits (branch `anticipation`)

| Commit | What |
|---|---|
| `75552b0` | the A2 design in `crates/gordian-medium/DESIGN.md`, before any code |
| `5ddaed2` | the build: pair cells in the crate, the anticipation layer in the arm, the trace, the attach switch, two accessors in `noticer.rs`; 10 crate tests, 12 adapter tests; departures 78 to 81 |
| `b0f40df` | departures 82 and 83, before any run: the partner's rate per second of its quiet time; evidence counted as follows beyond chance; one test added |
| `394d490` | scripts (`a2_*.py`), the null model, and the prediction above, before any run |
| `46211ce` | identity: R6 62 of 62; A1c's 15, A1a's 8 and A1d's 15 smoke arms and A1d's 12 trace files reproduced; the smoke manifest names this revision |
| (this report) | the smoke tables (`a2-smoke*.csv`) and this report |

## Verified by running something

- **Identity.** The release binary was built from `394d490` (its code is `b0f40df`'s; the later
  commits change scripts and CSVs only).
  - R6's held-out manifest (b = 5, rho = 0.7) replayed: all 62 arms have `results.csv` and
    `incidents.csv` byte-identical to `r6-results-sha256.csv` (`a2_gate.py all`, exit 0;
    `a2-regression.csv`). Kept: `artifacts/runs/a2/a2-xcheck-r6-heldout-b5-rho0.7`.
  - A1c's kept smoke manifest replayed (`a2-a1creplay-b5-rho0.7`): its 15 arms equal A1c's kept
    run and its 8 A1a arms equal A1a's kept run, both files row for row, `run_id` removed.
  - A1d's kept smoke manifest replayed (`a2-a1dreplay-b5-rho0.7`): its 15 arms equal A1d's kept
    run and its 12 engram trace files are byte-identical (`a2-reproduction.csv`, 50 of 50 items).
  - In the smoke, `sel_m3_privileged` is the same arm as A1a's, A1c's and A1d's (same manifest
    conventions); not compared separately.
- **Gates**, under `scripts/cgroup-run.sh --name anticipation-build --cpus 0-2 --memory 3G` on
  `b0f40df` (the code since): `cargo fmt --all -- --check` clean; `cargo clippy --workspace
  --all-targets -- -D warnings` clean; `cargo test --workspace` **1031 passed, 0 failed** (13
  ignored; A1d's 1008 plus 23 new); `scripts/check-no-oracle.sh` ok. The existing identity tests
  (`m1_identity.rs`, `m1b_identity.rs`, `subtick.rs`, the pinned M2/M3/M4 digests in
  `stream_medium.rs`, A1a's, A1c's and A1d's engram, gate and stale tests) pass unedited.
- **New tests.**
  - `crates/gordian-medium/tests/pair_cells.rs` (10): the cells built per pair and band; refused
    parameters; the level equals the integrator's own decay, bit for bit, and is the state the
    cell runs from; no floor; held from the threshold, named by its narrowest band; an evidence
    event for a tick already run goes to the next; a follow credits the latest open trial per
    partner and band once; a follow at the deadline counts, at the trial's instant does not; the
    chance; determinism of levels and counts.
  - `crates/gordian-run/tests/stream_medium_anticipation.rs` (13): first values and manifest text
    (an A1 manifest reads and writes as before); cells for every unconnected ordered pair and no
    other; three co-alarms within 0.4 s learn the edge and the fourth alarm predicts the partner,
    resolved `followed`; a partner in burst gets no trial and no prediction; an explained alarm is
    neither a trial nor a follow, and is with the switch off; the miss weight from the prior; the
    rate per second of quiet time; nothing carries into the next segment; **with `attach` on a
    learned edge attaches the partner's alarm to the anomaly, with it off it does not**; the switch
    never overrides the rung's first two rules; the trace's rows; the trace switch changes no
    charge; **on two real 150 s streams with the frozen M3 medium, the layer changes no notice and
    no decision column, and is charged** (at least 200 ns per tick more).
- **The smoke** ran once: `artifacts/runs/a2/a2-smoke-b5-rho0.7`, 3 arms, 20 streams, exit 0, 4 s
  wall, internal/external ratio 0.98, peak 16 MB, no OOM kill; every segment stopped at the
  horizon, no escalation refused; both trace files hold 20 `segment_end` rows.
- **Decisions.** `a2-smoke-decisions.csv`: both layer arms have all eight decision columns equal
  to `m3`'s on 523 of 523 incidents and the same 61 reasoner calls. `first_correct_at_ns` and
  `time_to_first_correct_ns` differ on 407 incidents by 1 to 42 µs (median 4.3 µs): the layer's
  charges advance logical time, as A1c's monitoring did. In `results.csv` only `bill_compute`
  differs: **+3.96 ms per stream** for the learner (max 6.3), +4.43 for the control.

**Resource record** (`scratchpad` log, copied in substance here). Free disk was 7.8 GB at the start
and fell to 6.1 GB after my debug build and Lab 2's; before the release build I removed my own
`target/debug` (924 MB; 7.0 GB free after), and every later check found at least 6.5 GB. The
release build waited 6 times (3 min) for Lab 2's run (`e1-xcheck5`) and started only when no
`gordian-run` existed; the R6 replay waited 3 times (1.5 min); the other runs and every debug
build and test waited none. No process of another lab was touched. I built with
`CARGO_INCREMENTAL=0`.

## The pair cell's rule, threshold and decay (as built)

- **Cells** (crate, `src/anticipation.rs`): per ordered pair `(a, b)` of services the time-zero
  public graph does not connect (neither a transitive dependent of the other) and per nested band
  (0.4 s, 2 s, 10 s), a `Sense` cell summing evidence events at the pair's edge node and an
  `Integrator` with leak `exp(-tick / 150 s)`, threshold 2, never reset. The **level** is the sum
  of the evidence, each decayed by `exp(-age / 150 s)`; no floor.
- **Counted first alarm** (adapter): an abnormal observation at a service with none there in the
  previous 2 s, which no service upstream of it (transitively) explains by a first alarm in the
  0.4 s before it.
- **Trial**: a counted first alarm at `a` opens, for each unconnected `b` that is quiet (no
  abnormal observation in the previous 2 s), one trial per band with chance
  `q = 1 - exp(-rate_b * w)`, `rate_b` = `b`'s counted first alarms per second of `b`'s quiet time,
  with the rung scorer's prior (3 over 30 s).
- **Evidence**: a counted first alarm at `b` within the band after the trial (crediting the latest
  open trial per `a` and band) adds `1 - q`; a trial whose window closes adds `-q`. The level
  counts follows beyond chance.
- **Learned edge**: level at least **2** (two follows beyond chance, net of decay) at the layer's
  next tick.
- **Prediction**: at a trial at `a`, for each quiet partner `b` with an edge held, one prediction
  in the narrowest band held, written to `anticipation-trace-<key>.csv` with the step's instant,
  the alarm's observation id and the anomaly owning it if any; resolved on the public side by
  any first alarm at `b` within the band. Nothing the arm does reads it.
- **Attach (off in the smoke)**: an abnormal observation at `b` that the rung's rule gives to no
  anomaly, or only to `b`'s own stale anomaly, goes to the anomaly at `a` whose current burst began
  within the held band before it.
- **Cost**: the pair-cell medium's ticks (200 ns each) and operations at the calibrated prices, and
  each level read as one cell update, charged with the noticer's.

## The smoke against the prediction

Learner (`a2_learn`), per stream over 20 streams (`a2-smoke.csv`, `a2-smoke-streams.csv`):

| Quantity | Predicted (80%) | Observed | In range |
|---|---|---|---|
| ordered pairs with cells | 33 (25–40) | 36.5 | yes |
| edges learned, any band | 12 (3–28) | 5.4 | yes, below centre |
| edges, 0.4 s / 2 s / 10 s | 2 / 10 / 1 (0–8 / 2–25 / 0–10) | 1.2 / 4.85 / 0.3 | yes, yes, yes |
| predictions | 60 (8–350) | 12.6 | yes, below centre |
| share 0.4 s / 2 s / 10 s | 0.12 / 0.80 / 0.04 | 0.15 / 0.80 / 0.04 | yes, yes, yes |
| follow rate 0.4 s | 0.10 (0.03–0.30) | 0.079 (3 of 38) | yes |
| follow rate 2 s | 0.33 (0.18–0.50) | 0.243 (49 of 202) | yes |
| follow rate 10 s | 0.85 (0.60–0.97) | 0.727 (8 of 11) | yes |
| `bill_compute` added, ms | 2.0 (1.3–4.0) | 3.96 | yes, at the top |

| Probability | Stated | Outcome |
|---|---|---|
| follow rate within 0.10 of all trials' in each band | 0.75 | **failed**: 0.4 s +0.027, 2 s +0.044, 10 s +0.200 (n = 11) |
| all trials' follow rate at least the mean chance in each band | 0.6 | **failed**: 0.4 s 0.052 ≥ 0.047; 2 s 0.199 < 0.215; 10 s 0.527 < 0.682 |
| fewer than 5% of predictions on a true hidden edge | 0.8 | Lab 3's to score |
| decision columns equal to `m3` | 0.9 | held, 523 of 523 |
| no refusal, every segment to the horizon | 0.95 | held (truncated ticks are not in the outputs; not verified) |

Control (`a2_raw`): 12.35 edges per stream (0.4 s band **4.35**, 3.6 times the learner's: the
probability-0.7 statement held); 38.75 predictions per stream (in 20–600); 0.4 s follow rate
**0.055**, below the learner's 0.079 (the probability-0.7 statement failed), against an all-trials
rate of 0.078 for the control. Its 0.4 s predictions were mostly not ahead of the step: the median
lead from the step that made them to the partner's alarm is −7 ms (the partner's alarm was
delivered in the same 500 ms step).

**By stream position.** First ten streams 121 predictions, 29 followed; last ten 130, 31 (flat, as
it must be: nothing carries). Per stream the learner made 0 to 27 predictions and held 0 to 11
edges; stream 10001 (8 unconnected pairs) and 10012 held none. **By time in the stream**
(`a2-smoke-time.csv`, 100 s bins of the predicting alarm): predictions 14, 46, 74, 34, 41, 42;
follow rates 0.50, 0.09, 0.28, 0.32, 0.15, 0.26. No bin stands out beyond what tens of
predictions allow; I read nothing into the bins.

**Lead times** (from the step that made the prediction to the partner's alarm, followed
predictions only): median 22 ms at 0.4 s, 717 ms at 2 s, 3.6 s at 10 s; 8 of the 60 followed
predictions were followed at or before the step's instant (the partner's alarm was delivered in the
same step, after the predicting alarm in delivery order).

## Analysis

**Best current model (inference).** On the public side, what the learner holds is the walk of a
zero-mean count crossing 2 by chance, slightly less often than the null model said. The reason it
is less often is measured, not assumed: the Poisson chance the layer assigns **overestimates** the
real chance of a follow in the wide bands (2 s: 0.215 assigned, 0.199 observed; 10 s: 0.682
against 0.527), so those cells drift down, and slightly underestimates it at 0.4 s (0.047 against
0.052), so that cell drifts up a little. The direction is that of overdispersion: a quiet service's
bursts come in clusters, so for the same mean rate the chance of at least one in a window is lower
than a Poisson process gives. My second probability bet on the other sign (clustering *across*
services adding excess) and lost; within a service the clustering dominates.

**What it shows.**

- The mechanism works as specified on the public side: cells per unconnected pair, trials at
  counted first alarms, evidence as follows beyond chance, learned edges, predictions written
  with what Lab 3 needs to join them (`artifacts/runs/a2/a2-smoke-b5-rho0.7/predictions-a2_learn.csv`:
  seed, instants, predicting and predicted service, band, the alarm's observation id, the owning
  anomaly, the public resolution and the follow's observation id).
- It costs 4 ms of modelled compute per stream (0.2% of the 2 s limit) and changes nothing the
  arm decides.
- Its predictions are not better than the public chance at 0.4 s and 2 s on this smoke (3 of 38 and
  49 of 202 followed against 0.052 and 0.199–0.222). That does not say they are not on hidden
  edges: a prediction on a true added edge is followed only when the predicting alarm is an
  incident that propagates, which W2 put at about one per stream.
- The public graph's explanation matters: with it off, the 0.4 s band learns 3.6 times as many
  edges, which I take to be siblings under a common upstream (not checked against the graph pair
  by pair), and those predictions are followed **less** often than chance (0.055 against 0.078):
  a sibling follows its sibling within 0.4 s on an upstream incident, which the learner has
  already counted as a follow, but the predicting alarm is usually not one.

**What it does not show.** Anything about hidden edges (Lab 3), held-out streams, other bands or
thresholds (none tried), or the attach switch on real streams (built and tested on constructed
cases only).

**Credible alternatives the chief could test next** (none tested, no claim):

1. Calibrate the chance from the arm's own history directly (each partner's empirical follow rate
   over all its trials, per band) instead of the Poisson formula: the measured miscalibration
   above is the largest systematic error in the rule.
2. Learn the 0.4 s band only, where chance is about 0.05 and one follow is strong evidence; a
   threshold of 2 there needs two co-alarms in a few minutes, which a true added edge produces
   about once per stream (W2), so a threshold of 1 with a stricter chance correction is the
   form that could catch one; it would also hold more chance edges, and W3's permutation level
   would say by how much.
3. Condition trials on the predicting alarm being an incident onset (a notice), not any burst
   start: propagation happens on incidents, and most first alarms are not.

## Departures from `DESIGN.md` and the brief, with reasons

- **Departures 78–83** (in `DESIGN.md`): the layer's sense port keeps events by tick (evidence is
  not produced in time order); trials and predictions resolved in time order at each observation;
  the trace's end-of-segment reads are never billed (and the trace switch changes no charge);
  two read-only accessors on `Tracked`; **82**, the partner's rate per second of its quiet time
  (the design's elapsed-time rate underestimates the chance from a quiet partner); **83**, evidence
  as follows beyond chance (`1 - q`, `-q`) instead of a follow 1 and a miss at the odds. 82 and
  83 were found by analysing the rule while writing the prediction (`a2_null_model.py`), before
  any run of any A2 arm and with no data from the world; they change the rule against what the
  design section said, and the chief should judge whether that was design or tuning (I hold it
  was design: the null model has no world data in it, and both fixes restore a property the design
  stated, zero drift at chance and a threshold in follows beyond chance).
- **Nested bands**, not A1c's disjoint ones: a prediction "within band" is a bound on the gap.
- **"No alarm at b yet"** is read as "b quiet: no abnormal observation at b in the previous 2 s",
  and the same condition gates the trials, so the cell measures the follow rate of exactly the
  predictions it licenses.
- **Counted alarms**: alarms the public graph explains (an upstream burst began within 0.4 s) are
  neither trials nor follows. This is my reading of "pairs the public graph does not connect"
  applied to alarms as well as pairs; the control with it off is reported beside.
- **A second learner form** (`a2_raw`, a labelled control) beyond the brief's two arms; predictions
  change no action, so it costs nothing in decisions.
- **"Edges learned"** counts pairs held at a read (every trial at `a` reads all its partners); an
  edge held only between two reads is never seen and never used.
- **The trace file**: `<run>/_trace/anticipation-trace-<trace_key>.csv`, beside A1d's engram
  traces, through A1d's variable `GORDIAN_ENGRAM_TRACE_DIR` (the brief's sibling name). The arm
  does not know its seed; the trace has the segment ordinal and `a2_smoke.py` writes the joined
  files with a `seed` column (`predictions-<form>.csv`, `first-alarms-<form>.csv`,
  `edges-<form>.csv`) in the run directory.
- **The attach switch overrides the rung's third rule** (a service's own stale anomaly), not only
  "no anomaly": without that, after a service's first anomaly in a stream the switch would almost
  never apply.
- **Outside my territory (Lab 2's `noticer.rs`)**: two read-only accessors on `Tracked`
  (`burst_open_at`, `last_site_at`), needed by the attach switch, inert for every arm. Nothing in
  `rung.rs` or `arms/mod.rs` changed.
- **Changed after the run, reporting only**: `a2_smoke.py` asserted that every prediction is
  followed or expired; one control prediction made at 598.75 s with a 2 s band outlived the stream.
  The script now counts such predictions (`unresolved_at_stream_end`: 0 for the learner, 1 for the
  control) instead of stopping. Nothing about the arms or the measure changed.

## What I am least sure of

1. **Whether the explanation filter removes alarms over a true added edge.** If the new dependent
   of an added edge is also a public transitive dependent of a service that bursts at the same
   time, its alarm is explained and never counted. I cannot check this from the public side.
2. **Departures 82 and 83 as design rather than tuning** (above): they were made on a model, not
   on data, but they changed the rule's numbers after the design was committed.
3. **The chance model.** It is measurably wrong in the wide bands (overdispersion), so every
   wide-band count in this report is biased toward fewer edges than an exact chance would give.
4. **Lead time in action terms.** With 500 ms steps a 0.4 s-band prediction is made at the same
   step as the partner's alarm more often than not; "before the decisive evidence" holds only in
   delivery order within a step for that band.
5. **Truncated ticks and step errors** of the pair-cell medium are counted in the layer's stats
   but not exported; the outputs do not show them. The tests run the layer on real streams without
   error, but the smoke itself does not prove none occurred.

## What the chief should examine most carefully

1. **Departures 82 and 83** and the null model behind them (`scripts/a2_null_model.py`, its output
   in the prediction above): a rule changed between design and run.
2. **The public chance comparison** (`a2-smoke.csv`: `follow_rate` against `all_trials_follow_rate`
   and `permutation_chance` by band): recompute it from `predictions-a2_learn.csv` and the trace.
3. **The attach switch's rule-3 override** and the two accessors in `noticer.rs`.
4. **That the layer changes no decision** (523 of 523; timing shifts of microseconds) and the
   identity evidence (`a2-regression.csv`, `a2-reproduction.csv`).

## What A2b's criterion (the use of learned edges) should bound

1. **Misattachment, paired against the memoryless arm**: observations the switch attaches to an
   anomaly that the evaluator places in another incident (or background), per stream, and the
   decisions it displaces (correct by deadline, by tier), since an attachment changes what the
   rung's context and checker see.
2. **A precondition from W3**: learned-edge precision against the hidden graph above W3's
   permutation level in the band A2b uses; on this smoke the public follow rate is at chance, so
   without it A2b would be testing attachments made by chance edges.
3. **The band**: only bands whose public follow rate exceeds the all-trials rate, with the chance
   calibrated empirically (alternative 1), or the 0.4 s band alone.
4. **Gained decisions reported beside displaced ones**, the attach count per stream, and the cost
   (`bill_compute`, about 4 ms per stream here).
5. **Power**: about one alarm over an added edge per stream (W2) means hundreds of streams (or W3's
   world C) before a gain can be told from zero; the criterion should state the streams it needs.

## Not done, and why

- No held-out run, no mutation testing, no tuning (not in the brief).
- The attach switch was not run on streams (the brief: off in the smoke).
- Nothing joined to the hidden side (W3's).

## Files

- Crate: `crates/gordian-medium/src/anticipation.rs`, `src/lib.rs`, `DESIGN.md` (A2 section,
  departures 78–83), `tests/pair_cells.rs`.
- Adapter: `crates/gordian-run/src/stream/arms/medium/anticipation.rs` (new), `noticing.rs`,
  `graph.rs`, `mod.rs`; hook: `crates/gordian-run/src/stream/arms/noticer.rs` (two accessors).
- Tests: `crates/gordian-run/tests/stream_medium_anticipation.rs`.
- Scripts: `experiments/exploration/scripts/a2_{common,manifests,run,gate,smoke,null_model}.py`.
- Outputs: `experiments/exploration/a2-regression.csv`, `a2-reproduction.csv`, `a2-smoke.csv`,
  `a2-smoke-streams.csv`, `a2-smoke-time.csv`, `a2-smoke-decisions.csv`.
- Kept runs (git-ignored): `artifacts/runs/a2/a2-xcheck-r6-heldout-b5-rho0.7`,
  `a2-a1creplay-b5-rho0.7`, `a2-a1dreplay-b5-rho0.7`, `a2-smoke-b5-rho0.7` (with `_trace/` and the
  joined `predictions-*.csv`, `first-alarms-*.csv`, `edges-*.csv`), logs in `_logs/`.
