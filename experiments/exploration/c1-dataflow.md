# C1: an incremental-dataflow noticer, against the hand-written rules and the medium

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation). Lab 2, unit C1 of
`docs/lab-queue.md`, branch `dataflow-noticer`. The criterion was fixed by the chief before any code: reproduction
and cost; no claim about which noticer is better. Code: `crates/gordian-run/src/stream/arms/dataflow/` (`engine.rs`,
`rules.rs`, `program.rs`, `noticing.rs`, `bench.rs`, `mod.rs`), `tests/stream_dataflow.rs`; scripts: `scripts/c1_*.py`;
outputs: `experiments/exploration/c1-*.csv`. The held-out run was played at `9341d16` (the manifest's revision; the
release binary SHA-256 `86a6517b…e3c1`); every later commit changed tests, scripts and result files only.

## The result in one paragraph

On the 200 held-out streams (seeds 20000-20199, b = 5, rho = 0.7, selection oracle at 16 s, the rung's context) **the
dataflow noticer's notice record equals B3's row `ramp_split_over_re2` row for row**: `notice_events.csv` (14,327 rows),
`notice_incidents.csv` (5,316), `notices.csv` (200) and `results.csv` are identical, and so is `incidents.csv` for the
unbilled control; `incidents.csv` of the billed arm differs in two columns, `first_correct_at_ns` and
`time_to_first_correct_ns`, by 0.7 to 30 us on every row, which is what the noticer's charge does to the logical clock
(below). On the 100 tuning streams (10000-10099) the same holds (7,236 notice events, 2,643 incidents; 0.7 to 32 us).
In the rung's call order, with every answer compared at every call, 100 tuning and 200 held-out streams give 3,632 and
7,196 notices and 3,604 and 7,133 retirements, no difference. Every measure of the table is therefore B3's, to the third
decimal, and the medium's are M2's (paired differences below). **Cost per stream**, at the declared prices (probe 20, write
45, scan 5, fire 5 ns): the dataflow noticer counts 39.8 thousand probes, 31.5 thousand writes, 112.6 thousand scanned rows
and 22.6 thousand fires, which model at 2.89 ms [2.85, 2.93] on the replay and 3.39 ms [3.34, 3.44] charged in the run; the
medium counts 72.0 thousand cell updates, 27.8 thousand traversals, 10.4 thousand routings and 5,995 ticks, which model at
16.71 ms [16.44, 16.98]. Measured wall time of the noticers' calls on the same public observations: hand-written B3 1.95 ms,
dataflow 5.64 ms, medium 10.46 ms. The dataflow noticer's rule code is 210 code lines, the relations and their evaluation
and the seam 979, the engine 206 (hand-written B3: 840 in four files; the medium's graph and adapters 956, its engine crate
3,207). The model prices the dataflow noticer's work at about half of what its wall time shows (ratio 2.0) and the medium's
at about 1.5 times what its wall time shows (ratio 0.6); the report does not tune either.

## 1. The engine choice (recorded before anything was built, commit `ccbf783`)

Requirement: incremental maintenance of joins and windows over persistent facts, deterministic, single-threaded, every
operation countable so that the work can be priced as the medium's is. Candidates:

- **Timely / differential dataflow.** Not used. Neither is in `Cargo.lock` or the local registry cache (a new dependency
  tree compiled in a 3-core, 3 GB envelope); the determinism obligations (single worker, no hash-ordered output) would be
  ours to discharge for code we did not write; the rules to reproduce are order-dependent folds (the attach rule reads the
  anomalies as the previous observation left them; a chain tie breaks by arrival order; an anchor move resets a timestamp
  a rebuild would not), which differential dataflow expresses through iterative scopes or custom stateful operators, where
  the engine contributes nothing and exact reproduction is harder to argue; and the operations inside its operators cannot
  be counted from outside, so a cost "priced like the medium's" would not exist. **This is a judgement from the libraries'
  documented model, not a measurement: nothing was built on them, and availability beyond the lockfile and the registry
  cache was not checked.**
- **The rung's hand-written incrementality** (the simpler alternative): it is what the unit compares against, so it is the
  oracle of the tests, not a candidate.
- **A hand-written incremental relational core.** Chosen: ordered tables (`BTreeMap`) with counted probes, writes and
  scanned rows and an optional delta log; an incremental projection operator (`project`) that touches its output only when
  an image changed. 206 code lines, no dependency, no hash, thread, clock or randomness.

Cost of the choice: the engine is mine, so what the unit measures is a measurement of this engine, not of timely or
differential dataflow, and **no claim is made about them**. The relations here are tens of rows; where a general engine's
strength would show (joins over large, churning relations) these rules do not exercise it.

## 2. What was built

`DataflowNoticer` (id `dataflow`) is B3's ramp and split over the later re-anchor over the rung's candidates, as
relations kept up to date by deltas, and every combination of its pieces (base the rung's or the re-anchor; ramp and split
each present or absent), because the B3 row is the rung's noticing with three rules added and an exact reproduction needs
all of it. Facts: delivered abnormal observations, counter readings past the store's cursor, the step's instant. Derived,
kept by deltas: `anomaly`, `member` (attached observations in the order taken), `by_site`, `pending`, `by_time` (the score's
window index), `by_obs`, `chain`, `chain_readings`, `dirty`, `timers`, `upstream`. Notices and retirements are read back out
of the relations. `rules.rs` holds the rules as pure functions of rows; `program.rs` the relations and their evaluation (the
module documentation has the schedule of a step and says what is incremental and what is not); `noticing.rs` the seam: a
`Tracked` view kept equal to the relations from their deltas (the rung reads `Tracked`), and the billing. Not built: B4's
follow-up rule (refused by validation), a retirement rule of its own, anything that reads a reasoner answer.

**Added outside the directory (all additive):**

- `crates/gordian-run/src/stream/arms/mod.rs`: one line, `pub mod dataflow;`. (The brief put the `mod` line in
  `arms/noticer.rs`; module declarations of this directory live in `arms/mod.rs`, so it is there.)
- `crates/gordian-run/src/stream/arms/noticer.rs`: a `Dataflow(super::dataflow::DataflowSpec)` variant beside `Medium`,
  `Learned` and `Composed`; its arm in `id()` (`DATAFLOW_ID`), in `validate()` and in `build()`; one row of the
  "noticers built" table in the file's documentation. Nothing else.
- `crates/gordian-run/Cargo.toml`: `criterion` as a dev-dependency (same version and features as three other crates) with
  the justification comment, and a `[[bench]] name = "dataflow" path = "src/stream/arms/dataflow/bench.rs"` so that the
  benchmark's source lives with the code it prices (and under the oracle guard). `Cargo.lock`: one line (`criterion` in
  `gordian-run`'s dependencies). The workspace `Cargo.toml` is untouched.
- Not edited, and a gap: `crates/gordian-run/HARNESS.md` (B3 documented its spelling there; it is outside this unit's
  files). The spelling is `{"noticer": "dataflow", "base": {...}, "ramp"?: {...}, "split"?: {...}, "billed"?: false}`,
  documented in `dataflow/mod.rs`.

## 3. Reproduction

Checks, all by running (`c1-reproduction.csv`, `c1-differences.csv`, `c1-clock-effect.csv`, `c1-tune-*.csv`):

1. **Per call, in the rung's order** (`tests/stream_dataflow.rs`, driving both noticers through `observe`, `notice`,
   `refresh`, `retirable`, `retire` as the rung does, comparing: the anomaly each observation attaches to, the notices of
   each step in order, the retirable set, every anomaly's id, site, region, anchor, attached observations in order, last
   instant, noticed instant, peak score bit for bit, evidence digest, and every score bit for bit): the 100 tuning streams
   (3,632 notices, 3,604 retirements), the 200 held-out streams (7,196, 7,133), two 150 s streams at every one of the 8
   combinations of the pieces, six 600 s streams in an ordinary test, and 240 generated scenarios built to reach every rule with randomized parameters (including
   a lower threshold, `Isolation::Any`, other windows): no difference. The two 100 and 200 stream tests are ordinary tests
   (about 35 s in a debug build), not ignored.
2. **In the harness**: the arms `dataflow`, `dataflow_unbilled` (a labelled control, `"billed": false`) and B3's
   `ramp_split_over_re2` in one run, on the tuning streams and on the held-out streams. `notice_events`,
   `notice_incidents`, `notices` and `results` (all but the run id, the wall-clock columns and the bill) are identical for
   both dataflow arms; `incidents.csv` is identical for the unbilled arm and differs for the billed arm only in
   `first_correct_at_ns` and `time_to_first_correct_ns`, on every row, by 0.7 to 29.97 us (mean 5.7 us; tuning: 0.7 to
   32 us). **Every difference, explained:** the arm's charge advances the logical clock by what it charges, within a step, so a
   declaration made later in the step is stamped a few microseconds later; the unbilled control, which does the same work and
   reports none of it, has none, and `bill_compute` differs by exactly the charge (mean 3.39 ms per stream). The medium's
   arm has the same property (it is billed the same way). The test `in_the_harness_...` pins it on two seeds.
3. The measures (`c1-table.csv`) equal B3's table (re-anchor ramp + split: 0.973 / 0.986 / 0.971 / 6.24 / 0.693) and M2's
   (medium at 100 ms: 0.992 / 0.993 / 0.554 / 5.44 / 0.501) to the third decimal, and the paired medium-minus-row
   differences are B3's supplementary comparison again, which is evidence that the three arms replay their earlier runs.

### How hard the tests are to pass (hand mutants, `c1-hand-mutants-first-run.csv`, `c1-hand-mutants.csv`)

22 one-line mutants of the rules and of the relations' upkeep (`scripts/c1_hand_mutants.py`): the first run, with the
integration test alone, caught 17 and **missed 5**, all exact-boundary conditions (the split's `<=` against `<` on the
silence, the instant a cluster becomes complete, a burst's reopening exactly one gap after, a burst "after the anchor" at
the anchor's own instant, expiry exactly at the gap; and one eviction tie) that real streams rarely land on. I added
unit tests of the rule functions at those boundaries; the final run caught **22 of 22**, six of them only by those unit
tests. cargo-mutants was not run (the unit's brief did not ask; B3's mutation numbers are for B3's code).

## 4. The cost columns

### Declared prices and the benchmark (`c1-prices.csv`; `bench.rs`; `scripts/c1_prices.py`)

Three runs of the criterion benchmark under `scripts/cgroup-run.sh --cpus 0-2 --cpu-quota 300 --memory 2G` (cgroup v1; wall
172.0 / 171.5 / 172.5 s, CPU 169.4 / 169.5 / 170.6 s, no OOM kill; the runner reported 9 MB peak, which is not credible as
the benchmark's whole footprint and is not relied on), each after the core-sharing wait (run 2 waited 4 polls). Median
nanoseconds per operation, tables of 10 / 100 / 1,000 rows, mean of the three runs:

| operation | measured ns | declared ns | how |
|---|---|---|---|
| probe (point read) | 6.1 / 9.3 / 14.1 | **20** | `probe` |
| write, insertion or removal | 19 / 25 / 30 unlogged, 22 / 27 / 32 logged | **45** | `write`, `write_logged` |
| write, replacement | 9.6 / 12.5 / 14.4 (logged) | (within the 45) | `replace_logged` |
| scanned row | 2.2 to 2.5 | **5** | `scan` |
| fire (a delta handled) | 0.01 (inlined away; not resolved) | **5** | `dispatch`, `project` less `replace_logged` |

The table primitives account for only part of the program's time. The five `program/*` workloads (the rules over generated
60 s scenarios, no seam adapter) measure 11.0 to 13.0 ns per counted operation (21.5 for the busiest, `mixed`); the same
through the seam adapter (`noticer/*`) 14.9 to 17.8 (26.4). The declared prices are one set of four numbers chosen so that
the program workloads sit as near M1b's band (0.7 to 1.4 of model) as one set can, rounded: of 15 program measurements 13 are
within the band, two are not (0.70, just under, on `splits` in run 1; 1.43 on `mixed` in run 3: the spread of the five
is wider than the band, which one price table cannot satisfy); through the adapter 0.89 to 1.20 except `mixed` (1.31 to
1.86). The primitives alone sit at 0.14 to 0.74 of this model (the fire-only `dispatch` aside) and the least-squares fit of four prices to the program
workloads is ill-conditioned (their count vectors are nearly proportional; the fire coefficient comes out negative), so
**the table is a judgement within the band, not a fit**, and the fire price is kept, as M1 kept its field read, because the
benchmark cannot resolve it.

**What the benchmark does not show.** On the real streams the whole noticer, adapter included, takes about twice what the
model gives (below). The benchmark's scenarios are 60 s over 10 services; real streams are 600 s with more concurrent
anomalies, and the cost per operation grows with the state (the generated `mixed` scenario already costs twice per
operation what the others do).

### Cost per stream (`c1-cost.csv`, `c1-costs-per-stream.csv`; 200 held-out streams; 90% cluster intervals)

The noticers replayed on the streams' public observations as the rung's steps deliver them (`replay_costs`), timed outside
the noticer, the least of three replays less the driver loop's own (0.165 ms):

| | counted operations | modelled | in-run charge | replay wall, anomalies read once a step | replay wall, three times a step |
|---|---|---|---|---|---|
| dataflow | 39.8k probes, 31.5k writes, 112.6k scans, 22.6k fires | 2.89 ms [2.85, 2.93] (3.84 at three readings) | 3.39 ms [3.34, 3.44] | 5.64 ms [5.52, 5.75] | 6.68 ms [6.53, 6.82] |
| medium, 100 ms | 5,995 ticks, 72.0k cell updates, 27.8k traversals, 10.4k routings | 16.71 ms [16.44, 16.98] | (as modelled: M2's test pins charge = replay) | 10.46 ms [10.21, 10.71] | 11.14 ms [10.90, 11.39] |
| B3, hand-written | none counted, not billed | none | none | 1.95 ms [1.90, 1.99] | 2.27 ms [2.22, 2.33] |

Paired wall differences, per stream: dataflow minus B3 +3.69 ms [3.61, 3.76]; medium minus dataflow +4.82 ms [4.67, 4.99];
medium minus B3 +8.51 ms [8.30, 8.73] (one reading a step; +4.40, +4.47, +8.87 at three). The rung reads its anomalies back
through `views` more than once a step; the in-run charge (3.39 ms) lies between the one-reading (2.89) and three-reading
(3.84) replays, which is the reason the second is shown. The in-run record (`c1-cost-run.csv`; the arm's bookkeeping time
`measured_sched_ns`, in which the noticer's time sits beside the shared rung's 7.8 ms): B3 7.80 ms, dataflow unbilled 12.34,
dataflow 14.08, medium 20.44; paired against B3 +4.54 [4.35, 4.73] unbilled, +6.28 [6.07, 6.50] billed, +12.64 [12.11, 13.20]
for the medium. The 1.74 ms between the billed and unbilled dataflow arms is the harness charging and recording the noticer's
cost every step, a cost any billed noticer pays (the medium's 20.44 ms includes it).

**Model against wall.** Wall over modelled is 2.0 for the dataflow noticer (1.7 with three readings) and 0.6 to 0.7 for the
medium. Of the dataflow noticer's time, the seam adapter is between a sixth and a third (the benchmark's `noticer/*` against
`program/*`: +18 to +44%), which the model, by the convention of the medium's own bill (its adapters are not billed
either), does not price.

**The two bills are of different scope.** The dataflow noticer's bill includes its attach rule, its score and its chains;
the medium's includes only its cells, synapses, routings and ticks, and not the rung's attach rule, scorer or its adapters
(which run in its arm and are not billed); B3's hand-written rules are not billed at all. The wall columns are the like for
like comparison; the modelled columns are not.

**Where the dataflow noticer's operations go** (`c1-op-breakdown.csv`, scanned rows per stream: `member` 30.2k, `by_time`
45.7k, `anomaly` 15.9k, `pending` 10.1k, `chain` 4.3k, `chain_readings` 2.9k, `upstream` 1.3k, `by_site` 2.3k; writes:
`chain` 5.5k, `chain_readings` 5.8k, `dirty` 5.1k, `member` 3.4k, `by_time` 3.4k, `by_obs` 3.4k, `anomaly` 2.3k). Most of the scanned
rows are time-driven, not event-driven: the score windows of noticed anomalies and the live-anomaly scans are re-read at each
step and each reading of the anomalies, because a window slides with the clock.

### Lines (`c1-loc.csv`; `scripts/c1_loc.py`; code lines: not blank, not comment)

| | file | total | code |
|---|---|---|---|
| hand-written B3 | `noticer_rung.rs`, `_reanchor.rs`, `_ramp.rs`, `_split.rs` | 1,322 | 840 (220 + 125 + 343 + 152) |
| shared by both | `noticer.rs` (seam, `Tracked`, attach rule, scorer, spelling) | 918 | 620 |
| dataflow, rules | `rules.rs` | 512 | 210 (+ 175 in unit tests) |
| dataflow, relations and evaluation | `program.rs` | 953 | 795 |
| dataflow, seam adapter and billing | `noticing.rs` | 238 | 184 |
| dataflow, engine | `engine.rs` | 419 | 206 (+ 100 in unit tests) |
| dataflow, spelling | `mod.rs` | 168 | 46 |
| medium, graph and adapters | `medium/graph.rs`, `noticing.rs`, `adapters.rs`, `mod.rs` | 1,338 | 956 (424 + 342 + 176 + 14) |
| medium, engine | `gordian-medium/src/` | 4,596 | 3,207 |

The partition into "rules" and "graph" is mine: the hand-written noticers do not separate them (the rule and the
bookkeeping that evaluates it are one function), and the dataflow noticer's `rules.rs` is what is left when the evaluation
is moved to `program.rs`. Read as numbers: the dataflow noticer is 1,441 code lines against B3's 840 (and still uses B3's
shared 620 for the `Tracked` the rung reads, its specs and its constants), 210 of them rules.

## 5. The table (200 held-out streams; `c1-table.csv`, `c1-paired.csv`; 90% cluster bootstrap, 10,000 resamples, B2's seed)

| arm | anchor-correct (hard non-leak) | leak noticed | leak anchor-correct | background / stream | strict precision |
|---|---|---|---|---|---|
| dataflow (billed) | 0.973 [0.959, 0.986] | 0.986 [0.967, 1.000] | 0.971 [0.945, 0.993] | 6.24 [5.89, 6.61] | 0.693 [0.682, 0.704] |
| dataflow, unbilled control | 0.973 [0.959, 0.986] | 0.986 [0.967, 1.000] | 0.971 [0.945, 0.993] | 6.24 [5.89, 6.61] | 0.693 [0.682, 0.704] |
| B3 row `ramp_split_over_re2` | 0.973 [0.959, 0.986] | 0.986 [0.967, 1.000] | 0.971 [0.945, 0.993] | 6.24 [5.89, 6.61] | 0.693 [0.682, 0.704] |
| medium, 100 ms | 0.992 [0.984, 0.998] | 0.993 [0.979, 1.000] | 0.554 [0.481, 0.623] | 5.44 [5.15, 5.74] | 0.501 [0.487, 0.516] |

Paired differences (same resamples): **dataflow minus the B3 row, and the unbilled control minus either: +0.000
[+0.000, +0.000] on every measure** (the table's five and hard noticed, precision, notices per incident, quality, cost).
Medium minus dataflow (identical to medium minus the B3 row):

| measure | difference |
|---|---|
| anchor-correct | +0.019 [+0.005, +0.033] |
| leak noticed | +0.007 [-0.013, +0.029] |
| leak anchor-correct | -0.417 [-0.493, -0.345] |
| background / stream | -0.80 [-1.29, -0.31] |
| strict precision | -0.192 [-0.208, -0.175] |
| hard noticed / precision / notices per incident / quality / cost s | +0.011 [0.000, 0.022] / +0.063 / +0.529 / +0.030 [0.000, 0.060] / +1.053 [0.87, 1.24] |

The cost column of the evaluator (`cost_s_per_stream`, 0.795 against 1.848) is the reasoner and components and does not
include either noticer's own work. No claim is made about which noticer is better.

## 6. Runs and exit statuses

| run | exit | wall | what |
|---|---|---|---|
| `c1-xcheck-r6-heldout-b5-rho0.7` (gate; kept) | 0 | 154 s (CPU 152.0 s, peak 477 MB, ratio 0.947) | R6's held-out manifest with only `source_revision` replaced (checked with `jq` against R6's), run id kept, own directory: **62 of 62** `results.csv` and `incidents.csv` hashes equal `r6-results-sha256.csv` (`c1-regression.csv`) |
| `c1-heldout-b5-rho0.7` (kept) | 0 | 16 s (ratio 0.966) | the four arms on 20000-20199 |
| `c1-tune-b5-rho0.7` | 0 | 6 s | dataflow, unbilled, B3 row on 10000-10099 (reproduction on the tuning streams; its CSVs are recorded, the directory is deleted) |
| `replay_costs` (release, under the runner) | 0 | 12 s each, three | held-out costs, one reading and three readings a step; breakdown |
| benchmark, three runs | 0, 0, 0 | 172 s each | above |
| `cargo fmt --all -- --check` | 0 | | |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | 0 | peak 0.48 GB | |
| `cargo test --locked --workspace --no-fail-fast` | 0 | peak 1.53 GB | 851 passed, 0 failed, 11 ignored |
| `./scripts/check-no-oracle.sh` | 0 | | (run as `./scripts/...`: the tool refused `bash scripts/...`) |
| pytest, `PYTHONPATH=analysis /home/user/gordian/analysis/.venv/bin/python -W error -m pytest -q analysis` | 0 | | 450 passed, 4 deselected (with `PYTHONPATH=analysis` for the reason B1 to B3 gave) |

Nothing was refused by the driver and nothing was excluded or discarded. Along the way: a test of mine failed once (a
billed arm's `incidents.csv` against an unbilled arm's; that is the clock effect above, and the test was corrected to
compare the billed arm only on the notice files), two compile errors in my own test code, and the first cargo call waited 27
polls (13.5 min) behind another lab's mutation run; in all, 61 polls (30 min) waited over about 90 cargo, benchmark, replay and driver
calls. Every cargo call ran under `scripts/cgroup-run.sh --name lab2-build --cpus 0-2 --memory 3G`; peak memory of builds
and tests was at most 1.53 GB, no OOM kill; free disk did not fall under 9 GB. The release binary used for the runs is
`target/release/gordian-run` SHA-256 `86a6517bd4d5ff6daacf235bcad6fbb99d9c8f9c834e0560bd681e92cfc9e3c1`; a later
`cargo test --release` rebuilt a differently linked copy of the same bin at the same path (SHA-256 `dabfcfc2…59319`), and
`cargo build --locked --release` restored the original, bit for bit, which is the rebuild check.

## 7. Verified by running, and assumed

**Verified by running:** the reproduction (above); the byte-identity gate; the counts equal what the billing charges
(`the_cost_it_reports_is_...`: the sum of the reports equals the counts at the declared prices; in-run charge 3.39 ms per
stream is the difference of two arms' `bill_compute`); determinism (two replays equal, counts included); the relations'
delta upkeep through 22 mutants; no hash map, thread, clock, randomness or `unsafe` in the directory (`grep`, and the
workspace forbids `unsafe`).

**Assumed:** that the replay driver's call order (one `refresh` and `score` per anomaly per reading, `retirable`, `retire`
every retirable) is the rung's closely enough for timing (the real rung also declines a retirement while a call is in
flight; the harness run is the check on records, the replay only on time); that the cost per operation in the benchmark's
scenarios carries to real streams (it does not: 2.0 times); that the least of three replays is the noticer's own time (noise
is visible: the first replays of the tuning streams showed 4.4 to 10.1 ms for the dataflow noticer); that `Tracked`'s
constructors and `note_attached`, which build the seam view from the relations, are correct for the view (they are B3's code:
**the view is not independent of the hand-written side**, only the rules are); that the shared constants `MAX_CHAINS_PER_KEY`
and `MAX_CHAIN_READINGS`, the specs, `Notice` and `Tracked` carry no behaviour the rules should not share.

## 8. Readings and deviations (each with its reason)

1. **The `mod` line is in `arms/mod.rs`**, not `noticer.rs` (that is where the directory's modules are declared).
2. **The benchmark is a `[[bench]]` with a source path inside the directory** (the oracle guard bans stream types under
   `arms/`; the benchmark uses only the engine, the program, world types and `ObsId`), and needs the `criterion`
   dev-dependency and one `Cargo.lock` line: both outside the files the brief named, both required by "a criterion
   micro-benchmark".
3. **Prices.** The brief asks for prices calibrated as M1's were. Isolating workloads price the table primitives; they
   price about half to two thirds of the program's time per operation (7 to 9 ns against 11 to 13), so I added workloads that run the program, and declared prices by
   judgement within the band (section 4). The fire price is unresolved. Two of 15 program measurements are outside the band.
4. **A fourth arm**, `dataflow_unbilled`, and a tuning-stage harness run, were not in the brief. The first separates what
   the bill's clock does from what the rules do (and showed it moves `incidents.csv` by microseconds); the second is the
   reproduction check on the tuning streams at the harness level.
5. **The meter's charge is labelled `noticer/medium` in the ledger** (a string in `meter.rs`, outside the territory, not
   edited). The dataflow noticer's charge is attributed on the bill to its own component id (18), and no ledger is kept.
6. **Own cost is reported three ways** (model on a replay, charge in the run, wall on a replay) because they differ and the
   differences are findings; the medium's in-run charge was not separated (no unbilled control for it), M2's test pins it
   equal to its replay.
7. **The tuning-stage and held-out reproduction tests are ordinary tests**, not ignored, because they take 35 s.
8. **Hand mutants, not cargo-mutants**: 22 by hand, in a script, restored after each; the tree was checked clean.
9. **Dependency rule, timely/differential**: decided from documentation, not tried (section 1).

## 9. Hidden record

I did not read `crates/gordian-stream/HIDDEN-DESIGN.md`. I read `PUBLIC.md` (the first 150 lines), the evaluator's outputs
through the analysis package, B3's and M2's reports, the review-log entries B4, B3, M2 and M1, B3's and M2's selection files,
`docs/lab-queue.md` and `docs/local-test-plan.md` sections 2 and the sharing rules, the noticer code and the medium
adapters. **I did not read `docs/charter.md` in full** (the brief listed the other documents; I grepped it). No parameter
of the noticer was chosen by me: they are B3's, read from `b3-selected.json`. The prices were fitted to generated scenarios
built from public observation types, not to streams.

## 10. What I am least sure of, and what the chief should examine

1. **That "reproduces exactly" is about the rules and not the seam.** The rules are re-expressed and none of B3's
   functions is called; the `Tracked` the rung reads is built with B3's constructors, and the specs and two constants are
   shared. A flaw common to both would not show. The generated scenarios and 300 real streams are the evidence against a
   flaw only in mine.
2. **The six exact-boundary conditions** that only unit tests pin (section 3). Real streams do not land on them; a
   world whose instants were coarser (a 100 ms grid, say) would.
3. **The price table.** Judgement within a band, two points outside it, a fire price the benchmark cannot see, and an
   in-situ ratio of 2.0 that the benchmark's scenarios do not show. Read the wall columns, not the modelled ones, for like
   against like.
4. **The wall times** are replays on this shared VM, least of three; the in-run differences (+4.5 to +6.3 ms, medium
   +12.6 ms) include the rung's other work for the extra anomalies the arms open and the billing's recording, and are not the
   noticers' alone.
5. **That the clock effect of billing is the only effect.** It moved microseconds on every row of `incidents.csv` and
   changed no notice, no result. It could change a row if a declaration landed within microseconds of a step boundary; on
   200 streams it did not.

## 11. Analysis (the PI's)

**What the general engine made easier.** Index upkeep. The rules touch the anomaly set at six places (create, attach,
move an anchor, split, adopt, forget or retire); in the hand-written code each place must keep `region`, `last_site_at`,
`burst_open_at` and the position in a vector consistent, and the three timestamps are exactly where B3's rules have path
dependence (a move resets the burst's opening to the anchor, a split recomputes it from the rows). Here `by_site`,
`pending`, `by_time` and `by_obs` are projections of two logged tables, maintained in one place (`settle`) and never touched
by a rule; the mutants that broke an index's use (the lowest anomaly at a site instead of the highest, a view
appended in place after rows left) were caught. The
score reads the rows inside its window through an ordered index instead of filtering all rows; the split is examined only
when an anomaly's rows changed or the instant its answer depends on has passed (a timer), where B3 examines every anomaly
every step; the counts come free, because every access goes through a counted table. Writing the rules as pure functions
of rows also made their boundaries testable on their own, which the differential test could not do.

**What it made harder.** Reproduction. The rules are not relational in the way the engine suggests: they are folds over the
observations in delivery order with state that depends on the path (an attach rule that reads the anomalies as the last
observation left them, a chain pick that breaks ties by arrival order, `owns` checks that make the attached order differ
from the id order, a ramp's anomaly that is in the set before the base's score is read). An exact reproduction meant
writing each of those quirks down as a reading and keeping the evaluation order of a step, which the engine contributes
nothing to; the module documentation says "a requirement of reproduction, not a property the rules have by themselves".
It also meant double bookkeeping: the relations are the truth and the rung reads a `Tracked` view, so the seam is 184 code
lines and between a sixth and a third of the noticer's time. The total is 1,441 code lines against 840 for the hand-written rules.

**Where the medium's sparsity buys something a dataflow engine lacks, and where it does not.** The numbers on this world:
the medium does 16 cell updates per observation (72,000 for 4,472 observations, the ticks and heartbeats included), which at
200 ns each is its whole bill; the dataflow noticer does 46 counted operations per observation (206.5 thousand for 4,472), almost all of them cheap scanned rows
(5 ns), most of those not event-driven. The breakdown says why: the cost that is proportional to the clock, not to events,
is the re-reading of score windows and live anomalies at each step, because a sliding window changes without any fact
arriving. The medium pays that as ticks (5,995 per stream, 200 ns, 7% of its bill) and heartbeats; my engine pays it as
scans, and the engine has no expiry events (only the split has a timer). A differential engine would retract a window's
rows at their expiry timestamps and so make the clock-driven part incremental too; mine does not, and that is a limit of my
engine and not of the idea. What the medium's sparsity does not buy here: it is not cheaper in wall time per stream than the
hand-written rules (10.5 against 1.95 ms) or than the dataflow noticer (10.5 against 5.6), and its larger count of
operations per observation comes from the graph's own redundancy (a cell per kind of evidence per service, confirmation
paths), not from the substrate. What a general engine cannot do and the medium can is not shown by this unit: the medium's
cells carry an oscillome, ports and learning (L1), none of which these rules use.

**What I would test next.**

1. The expiry-driven version of the score window (a timer per anomaly's oldest in-window row, so that nothing is re-read
   until a row enters or leaves): predicted to remove most of the 45.7k `by_time` and 15.9k `anomaly` scans per stream. If it
   does, "incremental" would be doing more than the table says now; if it does not, the cost was in the rules.
2. The same rules on timely/differential dataflow, once the dependency is available, to test the claim this unit could not:
   whether a production engine reproduces the answers and what its iterative scopes cost the order-dependent rules.
3. The call pattern B5 will impose: a selector reading `score` of every anomaly at each ask multiplies the readings back,
   and this noticer's cost grows with them (2.89 to 3.84 ms modelled, 5.6 to 6.7 ms measured from one to three readings a
   step) while the medium's barely moves.
4. A noticer whose rules are relational by nature (a join over many services' histories) on both substrates, because the
   rules here are not, and the unit's result is about these rules.

**The improved question.** Not "is a general engine a fair adversary for the medium's structural claim" but "which of the
hand-written rules are folds and which are joins, and does the medium's sparsity pay on the folds or on the joins".
