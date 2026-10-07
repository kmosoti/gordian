# L1 the learned noticer (Lab 3)

Status: exploration, not a preregistered experiment. Nothing here may later be cited as
confirmation. The criterion is the chief's, fixed before any L1 code or run (`docs/lab-queue.md`,
"## L1"); it was not changed. Branch `learned-noticer`.

## What was built, and what was fixed before any run

M2's frozen 100 ms graph with three tuned constants replaced by parameters adjusted online from the
stream's public history: the coincidence window (burst and three-kind cells, one value), the burst
lookback (derived from the window, rounded down to whole ticks) and the ramp threshold. The onset
integrator is switched off in M2's frozen 100 ms graph, so it had no constants to replace. The
update rule and the priors are in the module documentation of
`crates/gordian-run/src/stream/arms/learned/mod.rs`, committed in `d227306` before any run of the
arm.

- **Priors:** window 400 ms (the rung's `burst_ns`, a public constant), ramp threshold 2 readings,
  lookback 400 ms (derived). Step 1/4 per 10 s boundary (log space for the window), marginal
  precision `p` = 1/2. A test pins that the learned arm never reads the structure's frozen values.
- **Window:** per abnormal observation, the gap to the nearest earlier abnormal observation of
  another kind at the same service (bins 1 ms to 1 s) against the chance of such a neighbour if the
  kinds arrived independently at the service's own smoothed rates. The window is the upper edge of
  the last bin before the first chance-dominated bin (fewer than `p` of its events beyond chance).
- **Ramp threshold:** eight replayed integrators (1.5 to 6) on every counter reading, counting the
  firings the cells would make, against the same arrivals with surrogate steps drawn from the
  steps between readings at least 6 s apart; the threshold is the lowest candidate whose band and
  every band above are not chance-dominated.
- **Input:** delivered observations, the public rules' verdict on each, the rung's public constants.
  No tier, label, incident, deadline, reasoner answer or notice score reaches it.
- **Carry:** the harness plays a fresh arm per segment; learned values and evidence are kept in a
  process-wide store keyed by `state_key`, so experience accumulates in stream order.

Order of commits: the rule and priors `d227306`; two amendments after development runs on seeds
10000-10099, both before the fresh run: A1 `31d503a` (the ramp statistic counts firings, not
excursion peaks) and A2 `84268ea` (hash-drawn surrogate steps, not a golden-ratio sequence). The
learner was tuned against nothing from the fresh seeds. Both amendments were made because the
threshold estimate was stuck at its prior in the learner's own trajectory dump, not from a score.
Development runs 1 to 4 (10000-10099) are recorded as `l1-dev*-*.csv`; the clause verdicts were the
same in all four (clauses 1 and 3 hold, clause 2 does not).

## The run

`l1-fresh-b5-rho0.7`: seeds 40000-40199 in stream order, selection oracle at R5's 16 s delay, b = 5,
rho = 0.7, run seed 14000, nine arms, one process under the cgroup runner (cores 0-2, 2 GB), exit 0,
69 s wall, peak memory 77 MB, no OOM, 1,800 segments, no step-capped segment, internal/external
ratio 0.976. The manifest's `source_revision` is `01471f4`; the release binary used is
byte-identical (SHA-256 `d622bebe...3910`) to one rebuilt from that tree after the container
restart.

Arms: the four of the criterion (`learned`, `learned_off`, `frozen` = M2's graph, `reanchor`), a
fifth control `ramp_split_over_re2` (B3's public noticer, as the B3 review asked), and four labelled
sensitivity arms, nothing chosen from them: `learned_nocarry` (learns within a segment only),
`learned_p080` (`p` = 0.8), `learned_w050` and `learned_w1000` (prior window 50 ms and 1000 ms).
Each learning arm has its own state key.

## The three clauses, as written

Last 100 streams = seeds 40100-40199; 90% cluster bootstrap over whole streams, 10,000 resamples,
paired where stated; "lower bound" = 5th percentile.

| Clause | Value | Interval | Holds |
|---|---|---|---|
| 1. learned anchor-correct (last 100) >= frozen - 0.01, paired lower bound > -0.03 | learned 0.9805, frozen 0.9805, difference 0.0000 | [-0.0106, +0.0102] | **yes** (both parts) |
| 2. slope of W1's anchor-correct curve over the first 100 streams positive, lower bound > 0 (reading R3) | +0.043 per 100 streams | [-0.021, +0.126] | **no** (point positive, lower bound below zero) |
| 3. learning-off end state below frozen by >= 0.02 | 0.9415 against 0.9805, difference 0.0390 | [+0.0159, +0.0637] | **yes** |

**Conjunction: false. The learned arm does not count toward the aim under the fixed criterion.**
Clause 2 fails under the alternative reading R3b (incident-level slope, stated before the run) as
well: -0.0003 per 100 incidents seen, [-0.035, +0.034].

## Curves and slopes, five arms

Anchor-correct decisions per hard non-leak incident seen, cumulative in stream order (W1's curve):

| Arm | 10 | 25 | 50 | 100 | 150 | 200 |
|---|---|---|---|---|---|---|
| learned | 0.929 | 0.974 | 0.978 | 0.984 | 0.984 | 0.982 |
| learned_off | 0.929 | 0.974 | 0.978 | 0.964 | 0.955 | 0.952 |
| frozen | 0.929 | 0.974 | 0.978 | 0.979 | 0.977 | 0.980 |
| reanchor | 0.929 | 0.947 | 0.924 | 0.922 | 0.932 | 0.940 |
| ramp + split over re-anchor | 0.929 | 0.974 | 0.946 | 0.943 | 0.955 | 0.957 |

The leak-noticed curve is 1.000 at every read for the learned, learning-off and frozen arms, 0.98 for
ramp + split, 0.47 for the re-anchor. The first 50 streams hold 92 incidents and the four medium
arms make the same decision on every one: no arm separates before stream 55, so the learner's
curve cannot differ from its controls early. The learning-off control first falls below the frozen
graph at stream 56 (15 streams differ in the 200); the learned arm differs from the frozen graph
on 3 incidents in 398 (+1, +1, -1).

Slope of the anchor-correct curve (least squares, efficiency per 100 streams, 90% interval), reading
R3:

| Arm | first 50 | first 100 | first 200 |
|---|---|---|---|
| learned | +0.114 [-0.006, +0.333] | +0.043 [-0.021, +0.126] | +0.010 [-0.014, +0.040] |
| learned_off | +0.114 [-0.006, +0.333] | +0.010 [-0.066, +0.095] | -0.009 [-0.037, +0.022] |
| frozen | +0.114 [-0.006, +0.333] | +0.034 [-0.035, +0.117] | +0.009 [-0.016, +0.038] |
| reanchor | -0.033 [-0.227, +0.203] | -0.026 [-0.114, +0.069] | +0.004 [-0.026, +0.039] |
| ramp + split | -0.005 [-0.201, +0.223] | -0.017 [-0.095, +0.072] | +0.006 [-0.019, +0.037] |

The positive slopes of arms that do not learn (frozen, learning-off) are the dilution of early
noise in a cumulative ratio, as the analysis script's header says. Paired against the learning-off
control the learned curve's slope is higher: +0.033 [+0.009, +0.063] over the first 100 streams and
+0.020 [+0.011, +0.029] over the first 200 (zero over the first 50). Against the frozen graph:
+0.009 [0.000, +0.027] and +0.002 [-0.003, +0.006]. Beside the clause, not the clause.

## End states (last 100 streams), noticing and bounds

| Arm | Anchor-correct | Leak noticed | Leak anchor-correct | Background / stream (bound 6.82) | Strict precision (re-anchor 0.685) |
|---|---|---|---|---|---|
| learned | 0.980 [0.959, 0.996] | 1.000 | 0.656 | **8.76** [8.23, 9.32] | 0.424 [0.407, 0.440] |
| learned_off | 0.941 [0.913, 0.967] | 1.000 | 0.688 | **31.55** [30.16, 32.99] | 0.242 [0.234, 0.249] |
| frozen | 0.980 [0.959, 0.996] | 1.000 | 0.562 | 5.53 [5.16, 5.90] | 0.497 [0.481, 0.514] |
| reanchor | 0.956 [0.932, 0.977] | 0.438 | 0.000 | 6.32 [5.77, 6.89] | 0.685 [0.668, 0.703] |
| ramp + split | 0.971 [0.950, 0.989] | 0.984 | 0.984 | 5.60 [5.08, 6.13] | 0.701 [0.684, 0.717] |
| learned_nocarry | 0.956 | 1.000 | 0.688 | 26.55 | 0.266 |
| learned_p080 | 0.980 | 1.000 | 0.562 | 5.53 | 0.497 |
| learned_w050 / w1000 | 0.980 | 1.000 | 0.656 | 8.76 | 0.424 |

Over all 200 streams the learned arm's background is 8.87 per stream, strict precision 0.423. Paired
differences, learned minus frozen (last 100): anchor-correct 0.000 [-0.011, +0.010]; background
+3.23 [+2.92, +3.56]; strict precision -0.074 [-0.078, -0.070]; leak anchor-correct +0.094
[0.000, +0.189] (over all 200 streams: +0.132 [+0.067, +0.198]). Learned minus ramp + split: anchor-correct
+0.010 [-0.010, +0.030]; leak anchor-correct -0.328 [-0.440, -0.218]; strict precision -0.277.

**The learned arm exceeds M2's background bound** (8.76 against 6.82, interval above the bound) and
its strict precision is 0.07 below the frozen graph's and 0.26 below the re-anchor's. Anchoring
parity is reached at the cost of a coarser, noisier notice stream. These bounds are reported beside
the clauses, as the brief says; they are not clauses.

## What the learner learned, from the trajectory dump

`l1-trajectory.csv` replays the learned arm over the fresh seeds and records its state after every
stream; `l1-trajectory-p080.csv` the same for `p` = 0.8.

- The `p` = 0.5 arm's window goes 400 ms to 100 ms after stream 1, 49 ms after stream 2, 30.0025 ms after stream 3, and sits
  at 30.000 ms from stream 4 to stream 200. The ramp threshold sits between 2.0 and 3.0, mostly at
  2.5. Each stream gives about 59 boundaries.
- The `p` = 0.8 arm's window goes to 100 ms after stream 1 and 20.001 ms after stream 2, and its ramp threshold
  settles at 2.5 to 3.0 (3.0 on most streams after stream 9): **M2's frozen constants (20 ms, 3.0), arrived at from the public chance structure
  with no evaluator feedback**, and its last-100 results equal the frozen graph's to every digit
  shown above.
- The prior does not matter: priors of 50 ms and 1000 ms end at the 400 ms prior's end state (the
  last-100 numbers are identical).
- Experience has to accumulate across streams: with carry off (`learned_nocarry`) a segment's own
  evidence (about 200 qualifying events are needed) is rarely enough, and the arm is near its priors
  (background 26.55, anchor-correct 0.956).
- Per-stream background notices (post hoc; chosen after the clauses showed they cannot register a
  learner that settles within a stream; `l1-perstream.csv`): learned 26 on stream 1 (the learning-off
  value, 30), 9 on stream 2, 8.3 mean over streams 2-5, 8.8 over 21-100 and 8.8 over 101-200; learning-off
  stays at 29-32; frozen 4.5-5.5.

## Every reading and deviation

- **R1-R6 of `scripts/l1_common.py`** were committed before the fresh run and are used unchanged.
  The slope clause is read as the least-squares slope of W1's curve (R3); the incident-level slope
  (R3b) is reported beside and also fails.
- **The fifth control row** (ramp + split over the re-anchor) was added after the B3 review, before
  the fresh run, outside the criterion.
- **The sensitivity arms** were in the manifests from development run 1, before any result.
  Nothing is chosen from them; `p` = 0.8 was not a candidate for the claim.
- **Per-stream counts** (`l1-perstream.csv`) and the background-by-stream reading were added after
  the clauses were seen; labelled post hoc, not a clause.
- **Restart.** The container restart (disk exhaustion) came after the fresh run and its
  analysis. The seven analysis CSVs left uncommitted were regenerated from the run directory by
  `scripts/l1_analyze.py` and are byte-identical to the files written before the crash; the two
  trajectory CSVs were regenerated by replaying the learner over the fresh seeds with the debug test
  binary (`learned_trajectory`) and are byte-identical to the earlier files; their per-stream notice
  counts equal the run's `notices.csv` counts for every stream. Nothing in the analysis changed.
- **Byte identity:** R6's held-out run at b = 5, rho = 0.7, replayed with the final tree (HEAD `7eed4cd`,
  crates unchanged since the merge of B3), writes `results.csv` and `incidents.csv` whose SHA-256
  equals `r6-results-sha256.csv` for all 62 arms (`l1-regression-final.csv`, recomputed independently
  of the script: 62 of 62, 62 of 62).
- **Run refusals:** `l1-dev` once exit 3 (relative manifest path); the byte-identity run once exit 13
  (dirty tree: uncommitted work). Nothing ran either time. Nothing excluded or relabelled.

## What this shows and does not show

- Something was learned: the window and the ramp threshold move from weak priors to stable values
  within two to four streams of experience, and the move is worth 22.8 background notices per stream
  [-24.0, -21.6] and 0.039 of anchor-correct share [+0.020, +0.061] against the same graph with
  learning off.
- The signal is the excess over independence of same-service abnormal pairs by gap, and the
  excess over surrogate steps of ramp-integrator firings. Nothing in it knows which clusters are
  incidents.
- The fixed slope clause cannot show this. The learner's gain is complete before the cumulative
  anchor-correct curve has left the stream-1 noise, and the prior's cost (the learning-off arm)
  appears only after stream 55. The clause tests the wrong place to look; the clause is not changed
  for that reason, and a result that motivates another measure motivates a new unit.
- The end state equals the hand design on anchor-correct (paired difference 0.000) but not on the
  bounds: more background and lower precision than the hand design. The `p` knob carries what the
  priors do not: with `p` = 0.5 the learner stops at 30 ms, with `p` = 0.8 at the tuned 20 ms. Which
  `p` is right is information about what an incident is, supplied by hand.
- No conventional learner was run on the same stream (the charter's aim proxy 2 asks for a
  matched one); no distribution shift; one world, one setting, one run seed.
