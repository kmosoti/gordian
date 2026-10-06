# W1: event statistics per tick length; sample efficiency and an energy proxy

Status: exploration. No hypothesis is tested and nothing here is a confirmation. Lab 3, unit W1
(`docs/lab-queue.md`). Branch `tick-stats`.

## 1. What is measured, what is assumed

**Measured (by running something).**

- `crates/gordian-stream/examples/ticks` (evaluator-side, behind `reveal-hidden-state`) generated
  200 streams at the default parameters, regime schedule included, seeds 33000 to 33199 (new; not
  R6, R9 or R10 seeds), 600 s each: 899,471 observations (4,497 per stream), of which 344,170
  (38.3%) are abnormal by the public rules. It binned them at tick lengths 100 ms, 500 ms and 2 s.
  The run was `scripts/cgroup-run.sh --name w1-ticks --cpus 0-2 --cpu-quota 300 --memory 2G`:
  exit 0, 1.05 s wall, 1.02 s CPU, 1.85 MB peak memory, 0 OOM kills
  (`artifacts/runs/w1/ticks-usage.json`, git-ignored). A second run reproduced the histogram,
  summary and decisive CSVs byte for byte (sha256 equal).
- `gordian_analysis.measures` (`sample_efficiency`, `energy_proxy`) applied to the `results.csv`
  of every arm in `artifacts/runs/r10/r10-heldout-b5-rho0.7` (6 arms) and
  `artifacts/runs/r9/r9-heldout-b5-rho0.7-d0.0135` (15 arms), 200 streams each (seeds
  20000 to 20199), read in place. 511 hard incidents seen per arm.

**Defined here (a choice, not a finding).**

- A tick is `floor(at / tick)`; the grid starts at instant zero. Same-tick shares depend on that
  phase (section 6).
- "Abnormal by the public rules" is a copy of the rung's `is_abnormal` (counter at or above
  `HIGH`, catalogue message other than `CheckHealth`, snapshot hash differing from the public
  graph). It is a copy, not shared code (`gordian-stream` cannot depend on `gordian-run`).
- "Partner's first alarm" is the first abnormal observation of the incident at the partner (the
  cascade's partner, the split brain's peer) service. "First observation" is the incident's
  first observation of any kind. A hard incident is a tier-`Hard` incident (decoys are excluded).
  503 of them: 363 non-leak and 140 slow leaks.
- Sample efficiency: correct hard-incident decisions (a correct declaration by the deadline, the
  evaluator's `correct_hard`) over hard incidents seen (`incidents_hard`, which includes every hard
  incident that occurred, noticed or not, and includes the slow leak), cumulative in seed order.
- Energy proxy: modelled ns per correct decision, `total_cost_ns` over correct decisions, with
  `total_cost_ns` split into the reasoner's price (`reasoner_cost_ns`) and the remainder.

**Assumed (placeholders; nothing computed from these is a finding about energy).**

- **PLACEHOLDER conversion of modelled ns to joules: 10 W for cheap-rung operations
  (1.0e-8 J per modelled ns) and 1,000 W for a reasoner call (1.0e-6 J per modelled ns).** Both
  are assumptions, not measurements. 10 W is a guess for a loaded CPU core; 1,000 W is the low end
  of `docs/charter-aim-proposal.md`'s "kilowatts per inference node", itself unchecked against a
  primary source. They treat the evaluator's modelled ns as time spent at that power. The charter
  revision is meant to replace them; the functions take the watts as parameters, and every result
  object carries the label.
- The modelled ns are the evaluator's priced costs, not measurements of this machine.

## 2. Tick statistics

All tables below are produced from the committed CSVs by `experiments/exploration/scripts/w1_tables.py`
(`w1-ticks-hist.csv`, `-summary.csv`, `-lags.csv`, `-decisive.csv`, `-burst.csv`).

### 2.1 Events per tick (200 streams)

| tick | measure | mean | var/mean | share empty | share >= 2 | p50 | p90 | p99 | max |
|---|---|---|---|---|---|---|---|---|---|
| 100 ms | all events per tick | 0.750 | 1.32 | 0.486 | 0.168 | 1 | 2 | 4 | 38 |
| 100 ms | abnormal events per tick | 0.287 | 1.83 | 0.771 | 0.035 | 0 | 1 | 2 | 38 |
| 100 ms | all events per node per tick | 0.075 | 1.05 | 0.929 | 0.003 | 0 | 0 | 1 | 6 |
| 100 ms | abnormal events per node per tick | 0.029 | 1.14 | 0.973 | 0.001 | 0 | 0 | 1 | 6 |
| 500 ms | all events per tick | 3.748 | 1.42 | 0.029 | 0.872 | 3 | 6 | 10 | 46 |
| 500 ms | abnormal events per tick | 1.434 | 2.05 | 0.279 | 0.380 | 1 | 3 | 7 | 42 |
| 500 ms | all events per node per tick | 0.377 | 1.03 | 0.688 | 0.054 | 0 | 1 | 2 | 8 |
| 500 ms | abnormal events per node per tick | 0.144 | 1.13 | 0.871 | 0.011 | 0 | 1 | 2 | 7 |
| 2000 ms | all events per tick | 14.991 | 1.81 | 0.000 | 1.000 | 15 | 21 | 30 | 63 |
| 2000 ms | abnormal events per tick | 5.736 | 2.54 | 0.017 | 0.930 | 5 | 10 | 19 | 50 |
| 2000 ms | all events per node per tick | 1.507 | 1.22 | 0.272 | 0.439 | 1 | 3 | 5 | 12 |
| 2000 ms | abnormal events per node per tick | 0.576 | 1.37 | 0.625 | 0.149 | 0 | 2 | 4 | 9 |

Per-node cells are (service, tick) pairs over all services and all ticks, empty ones included
(11.94 M, 2.39 M and 0.60 M cells). `var/mean` is 1 for a Poisson count; it is above 1 everywhere,
most for abnormal events per tick, which is where incident bursts and background co-occur.
Active (service, tick) cells with at least one event, per stream: 4,261 at 100 ms, 3,726 at
500 ms, 2,175 at 2 s, against 4,497 events per stream at every length.

### 2.2 Ticks from an incident's first observation (hard incidents)

First observation to first abnormal observation: 0 ticks for all 363 non-leak hard incidents at
all three tick lengths (the first observation is the anchor `ErrorRate` reading, abnormal, at
offset under 1 ms). The slow leak is the exception:

| family | tick | n | mean | p50 | p90 | max |
|---|---|---|---|---|---|---|
| SlowLeak | 100 ms | 140 | 87.3 | 85 | 111 | 123 |
| SlowLeak | 500 ms | 140 | 17.5 | 17 | 22 | 25 |
| SlowLeak | 2 s | 140 | 4.4 | 4 | 6 | 6 |

(In ms: mean 8.7 s, median 8.6 s, maximum 12.3 s; no leak went unalarmed.)

First observation to the partner's first alarm, in ticks (the first abnormal observation of the
incident is its first observation, so first-abnormal-to-partner is the same table):

| family | mode | tick | n | mean | p50 | p90 | max | share in the same tick |
|---|---|---|---|---|---|---|---|---|
| Cascade | contradict | 100 ms | 57 | 1.00 | 1 | 2 | 2 | 0.210 |
| Cascade | contradict | 500 ms | 57 | 0.16 | 0 | 1 | 1 | 0.842 |
| Cascade | contradict | 2 s | 57 | 0.04 | 0 | 0 | 1 | 0.965 |
| SplitBrain | contradict | 100 ms | 60 | 0.67 | 1 | 1 | 2 | 0.367 |
| SplitBrain | contradict | 500 ms | 60 | 0.18 | 0 | 1 | 1 | 0.817 |
| SplitBrain | contradict | 2 s | 60 | 0.00 | 0 | 0 | 0 | 1.000 |
| Cascade | mimic | 100 ms | 56 | 110.73 | 112 | 150 | 160 | 0.000 |
| Cascade | mimic | 500 ms | 56 | 22.18 | 22 | 30 | 32 | 0.000 |
| Cascade | mimic | 2 s | 56 | 5.61 | 6 | 7 | 8 | 0.000 |
| SplitBrain | mimic | 100 ms | 58 | 92.95 | 101 | 151 | 160 | 0.103 |
| SplitBrain | mimic | 500 ms | 58 | 18.53 | 20 | 30 | 32 | 0.155 |
| SplitBrain | mimic | 2 s | 58 | 4.57 | 5 | 7 | 8 | 0.172 |

In milliseconds (no grid): contradict cascade mean 89, median 93, maximum 150; contradict split
brain mean 69, median 60, maximum 142; mimic cascade mean 11.1 s, median 11.1 s, maximum 16.0 s;
mimic split brain mean 9.3 s, median 10.1 s, maximum 16.0 s. Compound has no partner. The
design record's "20 to 230 ms" is the partner's whole burst; the partner's first alarm is 20 to
150 ms (the partner's dependents alarm up to 80 ms after it, `present.rs::partner_alarm`).

The 10 of 58 mimic split brains in the same tick as their first observation at 2 s are not rule
breaking alarms: the peer is a service that depends on the site, and the site's own burst alarms
up to three dependents within 60 ms. Their partner alarm is an ordinary dependent alarm. A test
checks that only a dependent peer can alarm early.

### 2.3 Decisive evidence

| quantity | result |
|---|---|
| Share of a hard incident's decisive evidence in the same tick as its first observation | **0.000 for all 503 hard incidents at all three tick lengths** (mean, "any" and "all" variants) |
| First observation to first decisive observation, median (ms) | 6.3 to 7.7 s across the 7 family and mode groups; maximum 13.9 s |
| The same in ticks, median | 63 to 77 at 100 ms; 13 to 15 at 500 ms; 3 to 4 at 2 s |

The zero is by construction, not an empirical surprise: a hard incident's burst has the
`Presentation` role (shared with decoys, never decisive), and its decisive evidence is phase 2,
6 s to 16 s after onset (`HIDDEN-DESIGN.md` section 4). A test pins both facts. The question the
unit asked therefore has the answer "none at any tick length up to 2 s"; a tick would have to span
more than the observed 13.9 s to put them together. The informative figure is the lag in ticks.

### 2.4 The first-moments burst on the grid (an addition)

Share of an incident's burst observations in the tick of its first observation, share of
incidents whose whole burst fits one tick, mean ticks spanned:

| family | mode | 100 ms | 500 ms | 2 s |
|---|---|---|---|---|
| Compound | mimic | 0.877, 0.69, 1.31 | 0.959, 0.92, 1.08 | 1.000, 1.00, 1.00 |
| Compound | contradict | 0.823, 0.57, 1.43 | 0.965, 0.89, 1.11 | 0.990, 0.96, 1.04 |
| Cascade | mimic | 0.801, 0.57, 1.43 | 0.942, 0.88, 1.13 | 0.987, 0.98, 1.02 |
| Cascade | contradict | 0.604, 0.11, 2.26 | 0.942, 0.79, 1.21 | 0.987, 0.95, 1.05 |
| SplitBrain | mimic | 0.789, 0.52, 1.48 | 0.966, 0.90, 1.10 | 0.990, 0.97, 1.03 |
| SplitBrain | contradict | 0.595, 0.17, 1.90 | 0.889, 0.77, 1.23 | 1.000, 1.00, 1.00 |

## 3. Reading: which tick length loses which structure

These are readings of the tables; the design consequences in section 5 are deductions.

- **100 ms keeps the order and the delay of a contradicting burst.** The cascade's partner alarm
  (20 to 150 ms after the first alarm) falls in a later tick for 79% of contradict cascades and 63%
  of contradict split brains, so a one or two tick synapse delay can carry it; only 11% of contradict
  cascade bursts and 17% of contradict split brain bursts fit one tick. It loses nothing about the
  burst except sub-100 ms order for the 21% and 37% that share a tick. Its cost is time: the same
  6 to 16 s of phase-2 evidence and 6 to 14 s of leak ramp span 60 to 160 and 60 to 140 ticks.
  It is also the length at which an incident's concentration stands out from the background:
  2.7% of (service, tick) cells hold an abnormal event and 0.12% hold two or more.
- **500 ms loses the order, not the co-occurrence.** 84% of contradict cascades and 82% of
  contradict split brains have the partner's alarm in the same tick as the first. A tick difference
  can no longer encode the 20 to 150 ms delay; the order survives only in the event's offset inside
  the tick (`offset_ns`, `docs/medium-ports.md` section 3). Bursts stay coherent (77% to 92% in one
  tick).
- **2 s loses the order entirely and the contrast with background.** 96.5% and 100% of contradict
  partner alarms share the first alarm's tick. A split brain's definition ("the site is the one that
  alarmed first") then cannot be read from tick order at all. And the background is no longer
  sparse: 37.5% of (service, tick) cells hold an abnormal event and 14.9% hold two or more, so "two
  abnormal events at one service in one tick" fires 446 times per stream at 2 s against 75 at
  100 ms (and 133 at 500 ms), a gap of roughly 6 times for the same rule, against about 1.8
  non-leak hard incidents per stream (plain incidents and mini-bursts also trigger it, so these
  counts bound the rule's false triggers only loosely). Co-occurrence of alarms at two services the public
  graph does not connect, the contradict signature itself, survives at 2 s.
- **No tick length up to 2 s touches the decisive evidence.** It is 6 to 16 s after onset at every
  length. What the tick changes is how many steps of state the medium must carry between the first
  observation and it: about 70 at 100 ms, 14 at 500 ms, 4 at 2 s (medians).
- **The slow leak is, as `docs/medium-ports.md` section 8 expects, indifferent in seconds and not
  in ticks.** Its first alarm is a median 8.6 s after its first reading at every length, which is
  85, 17 and 4 ticks. The readings are one per 0.8 to 1.5 s (design record), so at 100 ms about one
  tick in eleven carries a leak reading; at 2 s every tick carries one or two (deduced from the
  design record's constants, not counted here).
- **Mimic partner alarms are not a tick matter.** They arrive 6 to 16 s after onset in every case
  but the dependent-peer ones above.

## 4. Sample efficiency and the energy proxy on R10 and R9

Both functions are in `analysis/gordian_analysis/measures.py`, with 15 tests on hand-written rows
(`analysis/tests/test_measures.py`; a mutation of each of the cost split, the half windows and the
pooled ratio fails some of them). Outputs: `w1-sample-efficiency.csv`, `w1-sample-efficiency-curves.csv`
(every arm's full curve), `w1-energy-proxy.csv`, written by `scripts/w1_measures.py`.

**Read these with the caveat that none of these arms learns.** `sel_*`, `notice_*` and
`oracle_escalation` are fixed policies with privilege; `never_escalate` is fixed. The expected
curve of a fixed policy is flat, so these curves test the instrument and give the baseline shape;
they say nothing about improvement per unit experience in the aim's sense, which needs a matched
learner.

### 4.1 R10 held-out, b = 5, rho = 0.7 (hard incidents, 511 seen; 90% stream-cluster bootstrap, 2,000 resamples, seed 1)

| arm | correct | efficiency | 90% interval | first half | second half | after 50 | after 100 |
|---|---|---|---|---|---|---|---|
| never_escalate | 0 | 0.000 | [0.000, 0.000] | 0.000 | 0.000 | 0.000 | 0.000 |
| sel_rung_privileged | 208 | 0.407 | [0.371, 0.447] | 0.393 | 0.421 | 0.384 | 0.393 |
| notice_rung_privileged | 348 | 0.681 | [0.644, 0.716] | 0.710 | 0.652 | 0.712 | 0.710 |
| sel_win_w40_n256_privileged | 336 | 0.657 | [0.621, 0.698] | 0.631 | 0.683 | 0.600 | 0.631 |
| notice_win_w40_n256_privileged | 480 | 0.939 | [0.923, 0.955] | 0.960 | 0.919 | 0.936 | 0.960 |
| oracle_escalation_privileged | 480 | 0.939 | [0.922, 0.955] | 0.941 | 0.938 | 0.936 | 0.941 |

Differences between halves are at most 0.06; a binomial back-of-envelope gives a standard error of
about 0.04 to 0.06 for the difference of two halves of about 255 incidents (ignoring clustering,
which makes it larger), so none of them is read as a trend.

### 4.2 R10: energy proxy (modelled ns; joules are PLACEHOLDER, 10 W cheap / 1,000 W reasoner, an assumption)

| arm | correct (plain+hard) | modelled ns, total | ns per correct (all) | ns per correct hard | J per correct (all), PLACEHOLDER |
|---|---|---|---|---|---|
| never_escalate | 3,159 | 6.83e7 | 2.16e4 | none (0 hard correct) | 2.2e-4 |
| oracle_escalation_privileged | 3,639 | 6.41e10 | 1.76e7 | 1.34e8 | 17.6 |
| sel_rung_privileged | 3,367 | 1.33e11 | 3.95e7 | 6.39e8 | 39.5 |
| notice_rung_privileged | 3,507 | 1.45e11 | 4.15e7 | 4.18e8 | 41.5 |
| sel_win_w40_n256_privileged | 3,495 | 5.68e11 | 1.63e8 | 1.69e9 | 162.5 |
| notice_win_w40_n256_privileged | 3,639 | 6.89e11 | 1.89e8 | 1.44e9 | 189.3 |

### 4.3 R9 held-out, b = 5, rho = 0.7, delta = 0.0135 (hard incidents, 511 seen)

| arm | correct | efficiency | 90% interval | ns per correct hard | cost per stream (modelled s) |
|---|---|---|---|---|---|
| oracle_escalation_privileged | 480 | 0.939 | [0.922, 0.955] | 1.34e8 | 0.32 |
| oracle_selection_context_d16_privileged | 344 | 0.673 | [0.636, 0.713] | 1.55e8 | 0.27 |
| sel_win_w40_n512_privileged | 327 | 0.640 | [0.602, 0.677] | 2.20e9 | 3.60 |
| sel_win_w40_n256_privileged | 320 | 0.626 | [0.589, 0.664] | 1.78e9 | 2.84 |
| sel_win_w80_n512_privileged | 320 | 0.626 | [0.588, 0.666] | 3.31e9 | 5.30 |
| sel_win_w20_n256_privileged | 292 | 0.571 | [0.534, 0.613] | 1.40e9 | 2.04 |
| sel_coc_d08000_n256 / d16000_n256, sel_nbh_k3_n256 / k4_n256 (four arms, 302 each) | 302 | 0.591 | [0.554, 0.630] | 1.37e9 to 1.46e9 | 2.07 to 2.20 |
| sel_coc_d04000_n256_privileged | 289 | 0.566 | [0.527, 0.605] | 1.24e9 | 1.80 |
| sel_coc_d02000_n256_privileged | 277 | 0.542 | [0.504, 0.583] | 1.04e9 | 1.44 |
| sel_nbh_k2_n256_privileged | 272 | 0.532 | [0.495, 0.572] | 1.40e9 | 1.91 |
| sel_nbh_k3_n128_privileged | 252 | 0.493 | [0.453, 0.533] | 1.22e9 | 1.54 |
| sel_rung_privileged | 207 | 0.405 | [0.369, 0.444] | 6.42e8 | 0.66 |

All 15 arms with their curves checkpoints, halves and joules are in the CSVs. `oracle_escalation`
is R4's oracle and has the same `results.csv` in the two runs (identical once the `run_id` column is dropped).

### 4.4 Do the proxies rank the arms differently from modelled cost?

Ranks (1 is best) from `w1_tables.py --only ranks`. Spearman correlation across arms:

| | cost per stream vs efficiency | cost vs ns per correct (all) | cost vs ns per correct hard |
|---|---|---|---|
| R10 (6 arms) | 0.49 | 1.00 | 0.80 |
| R9 (15 arms) | 0.26 | 1.00 | 0.98 |

- **ns per correct decision over plain and hard together is modelled cost in disguise.** Its rank
  correlation with cost is 1.00 in both runs. The denominator is 3,159 plain decisions every arm
  gets right plus 0 to 480 hard ones, so it varies by 15% while cost varies by a factor of 10^4
  (R10) or 20 (R9). It cannot discriminate and should not be the headline form.
- **ns per correct hard decision does rank differently where quality differs.** R10:
  `notice_win_w40` is 6th of 6 by cost and 4th by ns per correct hard; `notice_rung` 4th and 2nd;
  `oracle_escalation` 2nd and 1st; `never_escalate` 1st by cost and last by hard decisions (it has
  none). In R9 the order is almost cost's (0.98): the arms differ little in quality (0.41 to 0.67
  among the non-oracle ones) and a lot in cost.
- **Efficiency and cost are weakly related** (0.49, 0.26): `oracle_escalation` and
  `notice_win_w40_n256` both score 0.939, at 0.32 and 3.44 modelled s per stream; their ns per
  correct hard decision differ by 10.7 times (1.34e8 against 1.44e9). Neither measure alone sees
  that; the pair does.
- **The joule figure ranks exactly as ns does** in both runs, because the reasoner's price is at
  least 99.87% of every reasoner-calling arm's modelled cost, so a 100 times heavier weight on it
  changes nothing among them. The placeholder watts matter only for `never_escalate`
  (2.2e-4 J per correct decision against 15 to 300 J for the rest), and that gap is a property of
  the pricing (a reasoner call priced at 250 microseconds per token), not an estimate of a real
  ratio. Until the conversion is measured or fixed, report ns and the joule figure side by side.

## 5. What the tick statistics imply for the medium (deductions, not tested)

1. `docs/medium-ports.md` section 8 expects "bursts need the short tick, the leak does not care,
   cost is roughly inverse in tick length". Supported: the leak is indifferent in seconds. Qualified:
   what a short tick buys is the order and delay of a contradicting burst, which `offset_ns` can
   carry at any tick length if the medium uses it; a graph that reads only tick-level activity
   cannot distinguish "site then partner" from "partner then site" at 500 ms or 2 s. Not supported
   as stated: input-driven cost is not inverse in tick length. Routed events are constant (4,497
   per stream) and active (service, tick) cells fall only from 4,261 to 2,175 (a factor 2 for a
   factor 20 in tick length). Only per-tick fixed work (decay, scans) scales inversely. M2's cost
   report should separate the two.
2. Decisive evidence is 6 to 16 s after onset, so any noticing or attribution mechanism that must
   connect the burst to it needs state of that horizon in seconds; delays and decays are in ticks,
   so the same design is 60 to 160 steps at 100 ms and 3 to 8 at 2 s. A design that holds this in
   cell state with a decay constant should give the constant in seconds.
3. 38% of observations are abnormal; abnormal is not a rare symptom. A rule of the form "abnormal
   events per node per tick above k" has a base rate that grows from 0.12% to 15% (k = 2) across
   the sweep, so thresholds cannot be shared between tick lengths and the noticing comparison
   against the best public noticer in M2 must tune per tick length.
4. For the contradict families a medium that wants the partner structure should either run at
   100 ms or carry `offset_ns` into its first-alarm cells. At 500 ms about 5 in 6 pairs would be
   processed in the same tick.

## 6. What this does not show, and what I am least sure of

- **Grid phase.** Ticks start at zero. A different phase moves some events across a boundary and
  changes the same-tick shares by a few points (not measured). The qualitative ordering (100 ms
  splits, 500 ms and 2 s merge) should not depend on it.
- **One draw of 200 streams.** Counts per group are 56 to 140 incidents. Shares such as 0.842
  carry a binomial standard error near 0.05; no intervals were computed for the tick tables.
- **The abnormal predicate is a copy** of the rung's. The test checks it against the world's own
  labels for four noise processes, not against the rung's function on the same input. The
  coordinator can compare the two on one stream (the `ticks` example does not need the rung).
- **The efficiency curves describe fixed policies.** The "first half vs second half" statistic is
  a crude proxy for improvement with experience, with no interval. The conversion to joules is a
  placeholder. Neither ranks arms by anything a real energy measurement would.
- **"Hard incidents seen" includes the slow leak** (140 of 503 hard incidents in the tick streams, 28%);
  efficiency on non-leak hard incidents alone is not shown here.
- The unit's same-tick question has a trivial answer (zero); I added the first-decisive lag and
  the burst columns so that the table says something about the structure.

## 7. What I would test next

1. **A learning arm against a matched learner on the same stream order**, so that the efficiency
   curve and its half difference can have a nonzero slope to be read (the aim's second proxy). The
   smallest version is an online logistic noticer over public per-service tick counts, with the
   curve at 25, 50, 100, 200 streams, a bootstrap over streams, and a never-learning control.
2. **A measured cheap-operations conversion.** If the host exposes energy counters (RAPL), measure
   the rung's joules per modelled ns on a held-out stream under the cgroup runner; the reasoner
   figure cannot be measured here and stays an assumption. This would turn half the placeholder
   into a measurement.
3. **The tick sweep inside M2** with the active-cell and routed-event counts reported separately,
   and a variant that reads `offset_ns`, to see whether the lost partner order at 500 ms and 2 s
   matters for anchoring (the R10 mis-anchoring) or only for the split brain's site.
4. **An interval for the energy proxy** (a stream bootstrap of the ratio of totals, as for
   efficiency); I gave one only for efficiency.

## 8. What the chief should examine most carefully

1. **`stats.rs::is_abnormal` against `rung.rs::is_abnormal`** (copy by reading, not by test), and
   the partner definition (first abnormal at the partner service, including a dependent peer).
2. **How the example is wired.** `examples/ticks/` is a directory with a `cfg` fallback `main`
   when the feature is off, because `crates/gordian-stream/Cargo.toml` is outside my territory (the
   idiomatic fix is a three-line `[[example]] name = "ticks" required-features =
   ["reveal-hidden-state"]`, as `questions` has). The test includes `stats.rs` with `include!` into
   a module that declares `extern crate self as gordian_stream`, so the tested code is the
   example's code.
3. **The tick grid phase and the "zero" decisive share** (section 2.3): check that I answered the
   question asked and not a different one.
4. **The ranking claim in 4.4**: ns per correct over plain and hard is cost in disguise. If you
   agree, the aim-proxy convention for later experiments should be hard-decision denominators (and
   the efficiency figure beside it), which differs from the default tier I gave `energy_proxy`
   (`"all"`, matching `stream.correct_per_cost`).
5. **Gate caveat.** The analysis venv's `gordian_analysis` is an editable install of the main
   checkout, so a pytest run in this worktree imports the main checkout's package and cannot see
   `measures`; I ran it with `PYTHONPATH` set to this worktree's `analysis` (see the report).

## 9. Deviations from the brief

- The example is `examples/ticks/{main,stats}.rs` with a `cfg` fallback (above), not `ticks.rs`
  with `required-features`.
- Three additions beyond the brief's list: first observation to first decisive observation, the
  burst span, and first abnormal to partner alarm (identical to the first-observation form here).
- `scripts/check-no-oracle.sh`: the crate prefix was already allowlisted, so only its comment was
  updated to name the example.
- Streams include the default regime schedule (R9's question dump turns it off; "at the defaults"
  was taken literally).
- No interval for the tick tables or the energy proxy.

Hidden record read: `crates/gordian-stream/HIDDEN-DESIGN.md` sections 4, 4.1, 4.2, 9 and 10 (the
interface, read in passing), and, to check the partner timing and the decisive-evidence timing, the
generator sources `incident.rs`, `present.rs`, `stream.rs`, `labels.rs`, `oracle.rs`. The hidden
labels were used only by the evaluator-side example, never by an arm.
