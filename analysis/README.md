# gordian-analysis

Paired statistical analysis of Gordian run directories (work item A7). It reads the
`results.csv` and `measured.csv` that the recorder writes (and `usage.json` when present),
pairs two arms on `(seed, class)`, and reports the charter section 8 quantities. The unit of replication is the
episode; nothing here is computed per event.

```bash
python3 -m venv analysis/.venv
analysis/.venv/bin/pip install -e 'analysis[dev]'
cd analysis && .venv/bin/python -W error -m pytest -q          # the default suite (fast)
cd analysis && .venv/bin/python -W error -m pytest -m slow -q  # the long calibration simulation
```

## Commands

```bash
gordian-analyze compare --a RUNDIR --b RUNDIR --metric success --margin 0.01 \
    --higher-is-better --seed 1 [--alpha 0.05] [--interval bootstrap|t] \
    [--resamples 10000] [--planned-n N] [--json FILE]
gordian-analyze compare --a RUNDIR --b RUNDIR --relative-savings \
    --threshold 0.20 --seed 1 [--planned-n N] [--alpha 0.05] [--resamples 10000] \
    [--interval-method percentile|bca|studentized] [--json FILE]
gordian-analyze compare --a RUNDIR --b RUNDIR --relative-savings --declared-cost \
    --metric bill_compute --threshold 0.20 --seed 1
gordian-analyze power --sd 1 --margin 0.5 --alpha 0.05 --power 0.8 \
    [--true-diff 0] [--equivalence] [--json FILE]
gordian-analyze drift --run RUNDIR [--json FILE]
gordian-analyze position --arm ARMDIR [--paired-with ARMDIR] [--metric measured_total_ns] \
    [--margin 0.05] --seed 1 [--alpha 0.05] [--resamples 10000] [--permutations 10000] [--json FILE]
gordian-analyze cost-check --a ARMDIR --b ARMDIR --seed 1 [--wall measured_policy_ns] \
    [--alpha 0.05] [--resamples 10000] [--json FILE]
gordian-analyze stream-summary --run RUNDIR [--json FILE]
```

`stream-summary` reads a **stream run** (the stream harness, work items R3 and R3b), not an
episode run, and is described in "Stream runs" below.

`drift` and `position` are the diagnostics of interleaved runs (work item A8); they are described
in "Drift and position diagnostics" below. An interleaved run directory holds one arm directory
per arm (`RUN/a1`, `RUN/a2`, ...), each of which `compare` reads exactly as it reads a one-arm run
directory, and `RUN/drift.csv`, which `drift --run RUN` reads.

`--seed` is required, and exactly one of `--higher-is-better` / `--lower-is-better` is
required (there is no default, so a cost metric cannot silently be read with the wrong sign).
Metrics: `success`, `critical_miss`, `false_alarm`, `abstained`, `undecided`, `probes_used`,
`corrections`, `decision_at_ns`, each `bill_*` column, `ops_component`, `ops_sched`, each
`modelled_*_ns` column, each `measured_*_ns` column, and three derived ones computed in memory
(input files are never modified): `modelled_cost_ns`, the sum of the two modelled columns, which
is the charter's cost C; `measured_total_ns`, the sum of the three measured columns; and
`measured_policy_ns`, the component and scheduling wall times alone, which is the wall time of
what the modelled cost covers. Booleans are accepted as `true/false/0/1`. `--margin` is in the
metric's own units.

`bill_total` no longer exists. It added nanoseconds to probe counts and bytes, which have no
common unit; asking for it is an error that says so. The `bill_*` columns are declared cost,
one resource each, and are compared one at a time.

`decision_at_ns` is undefined for an undecided episode, so a comparison on it is refused while
either arm has one. It is not computed over the decided episodes alone, which would condition
the comparison on the outcome; compare `undecided` and `success` instead.

### Which cost: `--relative-savings`

The charter's cost `C` is the modelled cost (`docs/local-test-plan.md`, A8b): the operations the
components and the shared decision rule counted, weighted by calibrated nanoseconds per
operation. It is a deterministic column of `results.csv`, so host interference cannot move it
(wall time can: bursts of stolen CPU time, which land on one copy of an episode and not the
other, moved the S of an A/A run by several percent). So `--relative-savings` defaults to
`--metric modelled_cost_ns` and the report says "Cost basis: modelled cost, the charter's cost
C". With the modelled cost the report also gives, as a secondary check, S on the measured wall
time of the same pairs: `measured_policy_ns` (components and scheduling, which is what the
modelled cost covers) and `measured_total_ns` (the whole episode, harness work included). They
are labelled "MEASURED wall time ... not the charter's cost C", and the JSON carries them under
`secondary`. Other choices:

- another `modelled_*_ns` column is allowed and labelled as one part of the episode, not `C`;
- a measured wall time (`measured_total_ns`, `measured_policy_ns`, `measured_*_ns`) is allowed
  and labelled a secondary check, not `C`; no second check is added to it;
- a `bill_*` column is refused unless `--declared-cost` is passed, and the report then says
  "DECLARED cost (a bill column), NOT the charter's cost C". The JSON carries `cost_basis`
  (`modelled`, `modelled_partial`, `measured`, `measured_partial`, `declared`);
- anything that is not a cost (`success`, `decision_at_ns`, ...) is refused.

An A/A comparison on the modelled cost is exactly `S = 0` with a zero-width interval, because
the cost is deterministic. That is not evidence that the model is right; the evidence is the
calibration's validity fits and `cost-check`, below.

### The non-identical-arm check: `cost-check`

`gordian-analyze cost-check --a RUN --b RUN --seed N` compares two different arms: the modelled
cost ratio `sum(B)/sum(A)` of `modelled_cost_ns`, and the wall-time ratio estimated as the median
over episodes of `B_i/A_i` with a paired percentile bootstrap interval (90% by default; whole
episodes are resampled as pairs). It reports whether the modelled ratio lies inside the interval,
and the interval's relative width, because an interval wide enough contains anything. The median
of episodes is the estimate a few interrupted episodes cannot move; the ratio of wall-time totals
is printed beside it for reference. `--wall` picks the wall time: `measured_policy_ns` (default;
components and scheduling, what the modelled cost covers) or `measured_total_ns`, which adds the
harness's own work (episode generation, simulator, ledger) that a policy does not choose and
that dilutes the ratio towards 1.

Undecided episodes stay in the totals: cost is spent whether or not the episode decided.
Relative savings says nothing about quality. An arm that never decides can cost much less, and
the report would say so; it has to be read next to the success comparison, as EXP-001 does.

Exit status is 0 on success and 2 on malformed input (message on stderr).

## Definitions

A is the baseline, B is the treatment. For each paired episode i:

```text
d_i = metric(B)_i - metric(A)_i           higher-is-better
d_i = -(metric(B)_i - metric(A)_i)        lower-is-better   (negated exactly once, in load.py)
```

Positive `d` always means B is better. The report states which convention was used.
`alpha` is the one-sided level of each test; every interval is the `1 - 2*alpha` interval
(90% at the default `alpha = 0.05`).

### load.py

`load_run(dir)` reads `results.csv` and `measured.csv` and joins them on `(seed, class)`. It
fails (`LoadError`) on:

- a missing file or column, or any column that is not in the schema the harness writes
  (`crates/gordian-run/src/results.rs`; a test compares the loader's lists with the header
  constants in that file). `confidence` is the one named optional column in `results.csv`,
  kept for the risk-coverage curve; the harness does not write it yet. `arm_position` is the one
  named optional column in `measured.csv` (work item A8: the arm's position in the order its
  episode was played in, 0 first, a non-negative integer); runs written before A8 lack it and
  still load, and a run that has it carries it in `Run.results`. It is not a metric;
- a value that is not boolean or not finite-numeric, more than one `run_id`, or duplicate
  `(seed, class)` in either file;
- `decision_at_ns` empty on a decided row, or present on an undecided one. It is empty exactly
  when `undecided` is true (evaluator R9) and loads as NaN there; every other column must be
  filled;
- keys in one file and not the other, or a `run_id` that differs between the two files. There
  is no silent drop, fill or outer join.

`stop_reason` and `directives_ignored` load as is; `stop_reason` must be non-empty and its
values are not checked against a list, so a new reason does not make a run unreadable.
`load.STOP_REASON_DECIDED` is the table of the harness's values: `terminal` and
`final_declaration` are decided episodes, `budget_exhausted`, `horizon` and `step_cap` are
undecided. `final_declaration` is a decision the arm made at the final call the harness gives it
when it has run out of affordable work or reached the horizon; it is scored like any decision
and is kept apart so that answers given with means and without can be separated. A test compares
the table with `crates/gordian-run/src/harness.rs`. `pair_runs` / `load_pair` join two arms on
`(seed, class)` and fail on any key present in only one arm; rows are aligned by sorted key,
not file order.

### intervals.py

`paired_bootstrap_ci(d, seed=..., confidence=0.90, n_resamples=10000)`: draw `n` indices with
replacement from the `n` episodes, take the mean of `d` over them, repeat `n_resamples` times,
and return the equal-tailed percentile interval (numpy linear-interpolation quantiles at
`(1-confidence)/2` and `1-(1-confidence)/2`). Resampling is over episodes, so each A/B pair
stays together; the arms are never resampled independently. The seed (`numpy.random.default_rng`)
is returned in the result and printed in the report.

`ratio_of_totals_ci(paired, metric, seed, resamples=10000, confidence=0.90)` is the relative
cost measure of charter EXP-001:

```text
S = 1 - sum_i B_i / sum_i A_i                (ratio of totals, not a mean of ratios)
```

Each resample draws episode indices with replacement, applies the same indices to A and B,
and recomputes S from the resampled totals. Positive S means B costs less. It applies no sign
flip and is only meaningful for cost-like metrics (lower is better), so the CLI mode refuses
`--higher-is-better`. It raises on negative values, on `sum(A) == 0`, and if any resample has
`sum(A) == 0`. `equivalence.exceeds(ci_low, threshold)` is `ci_low > threshold` (strict); for
EXP-001 the threshold is 0.20. `compare --relative-savings --threshold T` reports S, its
interval, and whether the lower limit exceeds T; `--margin` and `--interval` do not apply and
are refused, as is `--threshold` outside this mode. This mode does not produce a four-way
category, only exceeds / does not exceed (and, below the planned n, unresolved).

Three interval methods turn the same resamples into an interval (work item A7b):
`ratio_of_totals_ci(..., method=...)` and `--interval-method`.

```text
percentile   equal-tailed percentile interval of S*                       (the A7 interval)
bca          Efron's BCa interval of S: z0 = Phi^-1(share of S* below S-hat), acceleration from
             the leave-one-episode-out jackknife of S, adjusted levels read off the S* quantiles
studentized  bootstrap-t on theta = log(sum B / sum A):
               t*_b = (theta*_b - theta-hat) / se*_b,   se = sd_i(B_i - R A_i) / (sqrt(n) mean(B))
               theta in [theta-hat - se q_{0.95}(t*), theta-hat - se q_{0.05}(t*)]
             q are order statistics ceil((B+1) p); the interval is mapped back by S = 1 - exp(theta).
             se* is the same delta-method formula evaluated on each resample (no nested bootstrap).
```

**The default is `studentized`.** The percentile interval is anti-conservative on skewed costs
(A7 review); the calibration of A7b (`experiments/exploration/a7b-ratio-calibration.md`)
simulated true S exactly at 0.20 with B1's per-episode costs and with lognormal costs, 2,000
experiments per cell, and counted how often the 90% interval's lower limit exceeded 0.20. On the
four empirical populations the studentized interval had a false-exceedance rate of at most 0.0545
at every n from 40 to 1,713, including EXP-001's planning sizes (1,237 and 1,713; the worst cell,
0.0545 in 2,000 experiments, was 0.0484 in 10,000), where the percentile interval reached 0.064
(0.117 at n = 40) and BCa 0.0545 (0.0675 at n = 40). Heavy-tailed lognormal costs (sigma = 1.0) are
worse for every method, studentized included (0.072 at n = 1,237, 0.065 at n = 1,713), so a
borderline `exceeds` on costs much more skewed than B1's deserves the same caution as before.
Power at true S = 0.25 and 0.30 is 1.000 at the planning sizes on the empirical populations; at
n = 40 on the most skewed one the studentized interval's power is 0.16 at S = 0.25 where the
percentile interval's is 0.61 (which includes its excess false rejections). Size and power are
for the proxy pairs of B1, not for a selector.

Edge cases, by design and tested: a resample whose standard error is zero (for example one
episode drawn n times) has t = +-infinity, kept, not dropped, so the studentized lower limit can be
-infinity at tiny n (shown as `-inf`, null in JSON, with a warning; the decision is "does not
exceed"); a zero total in arm B makes the studentized interval undefined and it raises (percentile
and BCa do not); B proportional to A on every episode gives a point interval for all three; BCa
raises when every resample lies on one side of S-hat or an adjusted level is undefined, with no
clamp.

`calibration.py` is the simulation machinery of A7b, shared by the tests and by
`experiments/exploration/scripts/a7b_calibrate.py`: a sampler for an empirical null (paired
episodes resampled from a population whose arm B is rescaled by one constant so the true S is
exactly the chosen value) or a lognormal null, one random stream per experiment
(`SeedSequence(BASE_SEED, spawn_key=(population id, 1000 S, n, experiment index))`), and
`simulate_cell`, which builds every method's interval from one set of resamples per experiment.
`experiments/exploration/data/a7b-paired-costs.csv.gz` holds the per-episode modelled costs of two
of B1's proxy pairs at its four compute levels (regenerated from the repository by
`a7b_generate.py`; it matches `b1-variance.csv` cell by cell).

### equivalence.py

```text
se        = sd(d) / sqrt(n)                       sd with ddof = 1, df = n - 1
t interval = mean(d) +/- t_{1-(1-confidence)/2, n-1} * se
TOST lower: H0 mu <= -margin, t = (mean + margin)/se, p_lower = P(T_{n-1} > t)
TOST upper: H0 mu >= +margin, t = (mean - margin)/se, p_upper = P(T_{n-1} < t)
p_tost    = max(p_lower, p_upper)
```

`classify(ci_low, ci_high, margin)`:

```text
equivalent  if -margin < ci_low and ci_high < margin
beneficial  else if ci_low > 0
harmful     else if ci_high < 0
unresolved  otherwise
```

`noninferior(ci_low, margin)` is `ci_low > -margin`. The comparisons are strict. An interval
that straddles zero and is not inside the margin is `unresolved`; the report never maps "not
significant" to "equivalent". By construction `equivalent` requires `ci_high - ci_low < 2*margin`,
so an interval that is wide relative to the margin cannot be classified as equivalent.

#### Preregistered sample size gate

`classify` is a pure function of the interval. The sample-size gate is separate:
`gated_category(raw, n, planned_n)` returns `unresolved` when `planned_n` is given and
`n < planned_n`, and `raw` otherwise (charter section 8 item 3: underpowered experiments stay
unresolved; the freeze fixes the sample size). `--planned-n N` applies it in `compare`, in both
the standard and the `--relative-savings` modes.

```text
--planned-n absent       header "EXPLORATORY: no preregistered sample size"; category = raw
n_pairs >= planned_n     category = raw; header says "plan met"
n_pairs <  planned_n     category = UNRESOLVED, reason "n below preregistered sample size";
                         the raw category is printed on its own line labelled "Raw category
                         (before the sample-size gate)"
```

Below the planned n the non-inferiority line is printed as raw and marked not reportable as a
verdict (`noninferior_reportable: false` in the JSON), since the same underpowered result
would otherwise reappear there. Exceeding the planned n does not make a result confirmatory;
that depends on the experiment's frozen status, which this package does not know. The
existing small-n and degenerate-sample warnings are kept in every mode.

The category is computed from the bootstrap interval by default (`--interval t` for the t
interval). The report always shows both intervals, both categories, and the TOST p-values.

### drift.py: drift and position diagnostics (A8)

Why they exist: wall time on this VM drifts within a session and differs between sessions, and
the first play of an episode in a process may cost more than the next. The harness therefore
plays every arm of an experiment on each episode, back to back, in a randomly drawn order, and
times a fixed reference workload between episodes (`crates/gordian-run/HARNESS.md`, section 10).
These two diagnostics say how much machine variation the run saw. Neither sets a margin or a
tolerance; the experiment that uses a run preregisters them.

**`drift --run RUN`** reads `RUN/drift.csv` (`run_id, block, units_done, reps, ns, min_ns`: one
row per block of the fixed workload, which is the verifier run `reps` times on one fixed window;
`ns` sums the runs, `min_ns` is the shortest single run). It reports, for `ns` and for
`min_ns`, the coefficient of variation across blocks (sample sd, `ddof = 1`, over the mean) and
the ratio of the last block to the first, and prints the blocks. `ns` moves with preemption and
with interference from other processes on the VM; `min_ns` mostly does not, so a large CV of
`ns` beside a small CV of `min_ns` says the machine was disturbed rather than slowly drifting.
It needs two blocks or more. The first block runs right after process start-up and may be cold,
which pulls last/first below 1; the ratio is the specification's, and the table is there to read
it against. Nothing is concluded from a CV or a ratio without a tolerance preregistered by the
experiment. Run-to-run values from this package's own A/A runs are in `HARNESS.md`, section 10.

**`position --arm ARM [--paired-with ARM2]`** tests whether measured cost depends on
`arm_position`: whether playing first costs more (a cold cache, an allocator that is warm for the
second arm) or less than playing later. The estimand is on the log scale,

```text
theta = mean log(cost at arm_position 0) - mean log(cost at a later position)
```

so `exp(theta)` is a ratio of geometric means (1.03 means playing first costs 3% more). Costs
are multiplicative and span orders of magnitude across episode classes, which the log scale
handles; the relative-savings measure S is a ratio of totals, which is dominated by the dearest
episodes, and theta is not S. A position effect that is the same for every arm does not bias S
when the order is drawn at random (each arm is first about half the time), but it adds
variance, and a position effect that differs between arms (a dearer arm warms the cache for the
next one more) would bias S and is not detected by either estimator; see "What neither
estimator shows".

The plan says to use "a paired bootstrap interval on the log ratio where the same episode is not
available in both positions" and otherwise a permutation test over episodes. Read literally this
names the paired method for exactly the case in which pairing is impossible; it is read here as
"where the same episode *is* available in both positions". That gives two estimators, and the
data decide which applies:

1. *One arm, no copy (`--arm` alone): stratified permutation test.* Within one arm an episode is
   played once, at one position, so no episode is available in both positions and nothing can be
   paired. What is available is the design: the position of each arm in each episode was drawn
   independently of the episode (`crates/gordian-run/src/interleave.rs`). Under the sharp null
   that position does not change any episode's cost, the position labels are exchangeable among
   episodes, so permuting them is the exact randomization distribution of any statistic, with no
   distributional assumption on the costs. Classes differ in cost by an order of magnitude, so
   the labels are permuted within each class, and the statistic is stratified by class:
   `T = sum_c w_c d_c / sum_c w_c`, `d_c` = mean log cost at position 0 minus mean log cost
   later in class `c`, `w_c = n0_c n1_c / (n0_c + n1_c)`. The interval is the percentile
   bootstrap of `T`, resampling episodes with replacement *inside* each (class, first-or-later)
   cell so that each weight is kept. The p-value is two-sided, `(1 + #{|T*| >= |T|}) / (1 + B)`.
2. *Two copies of one policy (`--paired-with`), as in an A/A run: paired bootstrap.* If another
   arm played every episode identically, the copy that was not first played the same episode in
   the other position, and the two timings are repeat measurements of the same work. For each
   episode in which one copy was at position 0, `d_e = log(cost at position 0) - log(cost at the
   other position)`. The interval is the percentile paired bootstrap of `mean(d_e)` over episodes
   (`paired_bootstrap_ci`, so a pair is never split) and the p-value is a sign-flip test (if
   position does nothing, which copy was first is a fair coin independent of the work, so `d_e`
   and `-d_e` are equally likely). The loader *checks* the premise: every `results.csv` column
   except `run_id` (the bill, the probes, the outcome, the stop reason) must agree on every
   episode, or the command refuses. Two arms that differ, including a `random_matched` pair
   (its generator is seeded by the arm name), are refused and need estimator 1.

Both are seeded (`--seed`, required) and report their resamples and permutations.

**Why both, and what the A/A showed about them.** Within a class, episodes differ a great deal
in cost (different worlds), and estimator 1 cannot remove that: which episodes happened to land
first is part of its noise. Estimator 2 removes it, because the two timings of an episode share
it. On the A/A runs of `HARNESS.md` section 10 (21 runs, 220 episodes per arm) the stratified
interval's median width on the log scale was 0.111 against 0.054 for the paired one (about
+/-5.5% against +/-2.7%), and the estimates themselves varied 2.7 times as much from run to run
(sd 0.043 against 0.016). In the first run the two copies' stratified estimates differed in sign
(+10% and -4%) while the paired one said +3%. The two stratified estimates of a two-arm run are
`theta + D` and `theta - D`, where `D` is the imbalance in episode cost between the episodes
that arm happened to play first and the rest; their mean is the paired estimate. Estimator 1
is the only one available for a real comparison of two *different* arms, and its interval must
be read with that width.

**Margin.** `--margin M` (a relative cost, 0.05 for 5%) prints where the interval lies against
`L = log(1 + M)`, symmetric on the log scale: `exceeds_margin` if the whole interval is beyond
`+L` or beyond `-L` (a position effect larger than the margin is established; this is what
invalidates a run's cost comparison, per the plan), `within_margin` if the whole interval is
inside `(-L, +L)`, and `unresolved` otherwise. The comparisons are strict, an interval that is
merely wide is never `within_margin`, and a p-value above alpha is never reported as "no
effect". Without `--margin` no verdict is printed. The margin is preregistered by the
experiment; this package supplies none.

#### Assumptions

- *Randomized order.* Position is independent of the episode and of cost. The harness draws it
  from `(run_seed, seed, class)` before any arm plays, so it cannot depend on a result; the
  tests check the draw is a permutation and balanced. A run made some other way (a hand-edited
  `arm_position`) voids the permutation test.
- *Position-0 versus later is the only contrast.* With three or more arms, positions 1 and
  beyond are pooled. A gradient across later positions is not estimated.
- *Estimator 1 treats episodes in a class as exchangeable under the null.* That is what the
  randomization gives; it is not an assumption about the cost distribution. The *bootstrap
  interval* is approximate: it needs cells of more than a few episodes (a cell of one episode
  has no resampling variance, so many tiny cells make the interval too narrow). The A/A design,
  20 seeds per class and two arms, has about ten per cell.
- *Estimator 2 treats two copies as repeat measurements.* True when `results.csv` agrees (checked)
  and the policy is deterministic given the episode, which the reference core guarantees. It
  says nothing about arms that differ.
- *Costs are positive.* A zero measured cost has no log and is refused.
- *Measured cost is wall time in one process.* A position effect here is the sum of everything
  that depends on order within the process: instruction and data caches, branch predictors, the
  allocator, CPU frequency. It does not separate them.

#### What neither estimator shows

- *An arm-by-position interaction.* If arm X is more sensitive to being first than arm Y (a
  cheap arm that follows a dear one inherits a warm cache; the reverse does not), then X's
  and Y's costs are biased in opposite directions, S is biased, and a comparison of copies of
  one arm cannot see it: the copies have the same sensitivity. Estimator 1 applied to each arm
  of the real comparison is the only check, with its width.
- *Drift.* The two diagnostics are separate. An A/A in which each arm is first half the time
  has a drift effect that is symmetric noise, not bias, and it widens S's interval; `drift`
  reports how large the drift was.
- *Equivalence.* A position effect whose interval contains 0 is not shown to be zero.

### power.py

Normal approximation, `z_q` the standard normal quantile, `sd` the sd of the per-episode
differences:

```text
non-inferiority:  n = ((z_{1-alpha} + z_{1-beta}) * sd / (margin + true_diff))^2
equivalence:      n = ((z_{1-alpha} + z_{1-beta/2}) * sd / (margin - |true_diff|))^2
```

Both are rounded up. Defaults: `true_diff = 0`. A positive `true_diff` means the treatment
is truly better. With `sd = 1, margin = 0.5, alpha = 0.05, power = 0.8` the non-inferiority
formula gives 24.73, so n = 25.

The normal approximation uses z where the analysis uses t with `n - 1` degrees of freedom, so
it understates n for small samples. The result therefore includes `achieved_power_t`, the power
of the t analysis at the returned n (noncentral t). For equivalence it is the Bonferroni lower
bound `P(reject lower) + P(reject upper) - 1`. Measured values: n = 25 gives 0.783; n = 7
(sd 1, margin 1) gives 0.754; n near 100 or more is within 0.005 of target. The CLI prints a note when
the t power falls short of the target. The power also assumes the sd you pass in; an sd
estimated from exploratory runs carries its own uncertainty that is not propagated.

### breakdown.py

`per_class_table` (n, n_missing, mean, sd per class and metric; only `decision_at_ns` can be
missing, and its mean is over decided episodes, said so by `n_missing`); `coverage_error_table`
(an episode is answered when it neither abstained nor is undecided; `coverage = answered / n`,
`error_rate_answered = 1 - mean(success)` over answered episodes, NaN if none were answered,
`undecided_rate`, plus a pooled `ALL` row. An undecided episode never gave an answer, so it is
neither coverage nor an answered error; it is still a failure in the `success` metric); `risk_coverage_curve`
(requires `confidence`: over answered, i.e. decided and not abstained, episodes, for each distinct confidence value c take all
answered episodes with confidence >= c, so ties enter together; `coverage = accepted / N`
with N counting every episode including abstained and undecided ones; `risk` = fraction of accepted episodes
that are not successes); `paired_class_table` (per class n, mean A, mean B, mean d). Per-class
numbers are descriptive and exploratory; they carry no test and no multiplicity adjustment.

## Stream runs (R3b): `load_stream_run`, `gordian_analysis/stream.py`

A stream run directory is what `gordian-run` writes for a stream manifest: one subdirectory per
arm holding `results.csv` (one row per **stream**, keyed by `seed`), `incidents.csv` (one row per
incident, keyed by `(seed, incident)`), `measured.csv` (with `arm_position`) and the arm's own
`manifest.json`, and in the run directory the whole `manifest.json` and `drift.csv`. The
columns are in `crates/gordian-run/src/stream/results.rs` and `crates/gordian-run/HARNESS.md`,
section 11. The loader (`load.py`, a separate section from the episode loader, which is
unchanged) holds the same discipline as the episode one: the columns are exactly the harness's, in
order (`STREAM_RESULTS_COLUMNS`, `STREAM_INCIDENTS_COLUMNS`, `STREAM_MEASURED_COLUMNS`; a test
parses `RESULTS_HEADER`, `INCIDENTS_HEADER` and `MEASURED_HEADER` from the harness source and
compares), counts are read as exact integers (they reach 1e12), and the loader refuses a file
whose identities fail: reasoner calls are the sum of the three escalation classes, total cost is
substrate plus reasoner cost, a hard incident has a family and no other incident has one, a first
correct declaration exists exactly when there is a correct one, and `incidents.csv` agrees with
`results.csv` about which streams exist and how many incidents of each tier each holds.

**Pairing is on `seed`** (`pair_streams`, `pair_arms`): the stream is the unit of replication, a
stream holds about 27 incidents that share a graph, a noise process and recurrences, so incidents
of different streams are not independent replications. Pairing refuses a seed that only one arm
played, and refuses arms whose `(seed, incident)` keys, tiers, families or criticality differ,
because the incidents of a stream do not depend on the arm; arms from runs with other stream
parameters are not comparable.

**Pooled ratios** (`stream.py`) are ratios of counts summed over streams, never a mean of
per-stream ratios (a stream with one hard incident would weigh as much as one with five, and a
stream with none has no ratio). They match the evaluator's definitions
(`crates/gordian-stream-eval/RULES.md`, "Derived ratios"): escalation precision is call-level,
`needed / (needed + unneeded + background)`; escalation recall is incident-level,
`hard_incidents_escalated / incidents_hard`. Ten calls about one hard incident are ten needed
calls and one escalated incident, so read the two beside the counts and the cost. A ratio whose
denominator sums to zero is NaN (JSON `null`), which is not zero: an arm that never escalated has
no precision. `correct_rate` (per plain or hard tier), `critical_miss_rate`, `correct_per_cost`
(correct plain and hard incidents per modelled second of total cost) and `family_table` (per
hard-fault family, from `incidents.csv`) follow the same rule. They are point estimates and carry
no interval: an interval must resample whole streams, which is the headroom check's (R4).

`stream-summary --run RUN` prints, for every arm (`comparison` arms first, then the privileged
and ablation references, each in name order), the pooled counts and rates by tier, critical
misses, wrong declarations, false alarms, reasoner calls and tokens, escalation precision and
recall, and total modelled cost per stream. `--json` writes the same plus the per-family table.
**`tier`, `family`, `critical` and the verdict columns are evaluator output about hidden state.**
They are for analysis; they must never be a policy input or training data.

### Frontiers and cluster-bootstrap intervals (R4): `gordian_analysis/frontier.py`

The headroom check's arithmetic, so that the script that runs it decides nothing
(`docs/local-test-plan.md`, 5R, R4, "Margin"). `arm_point` / `points_table` give, per arm of a run
(with the arm's policy and parameters read from `manifest.json`), the **quality** (pooled fraction
of hard incidents declared correctly by their deadline, **excluding the slow-leak family**, which
is reported as `leak_rate`) and the **cost** (mean total modelled cost per stream), with plain
accuracy, critical misses, wrong declarations and false alarms beside them and never folded in.
`pareto_mask` / `frontier` give the non-dominated points (per policy family, or over everything);
`best_at_cost` and `cheapest_reaching` read a table of points. `cluster_bootstrap` resamples whole
streams (all arms of a run played the same streams, so one resample is applied to every arm),
recomputes every arm's quality and cost, picks in each resample the best baseline affordable at the
oracle's resampled cost, and returns the gap to the oracle, the cost the best baseline needs to
reach the oracle's quality, and equal-tailed percentile intervals (seed required and recorded).
The best of many noisy configurations is biased upward, which biases the gap against finding
headroom; `best_at_cost` on a table from *other* streams removes it. `headroom_verdict` applies a
margin and a lower bound that the caller passes in; it has no opinion about which settings count.
The tests (`tests/test_frontier.py`) check the ratio-of-sums rule, the slow-leak exclusion, dominance
with ties, that the interval is as wide as streams (not incidents) imply, and the affordability
rule inside a resample.

## Specification issues found and how they were resolved

1. The plan's A7 listing (`--margin-success`, `--margin-cost`, `equivalence.py` tested against
   `ttest_ind`) differs from the work-item spec used here (`--metric`, `--margin`, paired
   designs, `ttest_rel`). The work-item spec was followed; arms are paired, so `ttest_ind` would
   be the wrong test.
2. The charter's EXP-001 cost measure is relative, `S = 1 - C_sel / C_base`, a ratio of totals,
   which the difference-of-means procedure cannot express. Resolved by the coordinator:
   `ratio_of_totals_ci`, `exceeds`, and `compare --relative-savings --threshold` (see
   intervals.py above). The measure and the 0.20 threshold are passed by the caller; nothing
   in this package fixes them.
3. Equivalence power for `true_diff != 0` is not specified. The specified formula is applied
   with `margin - |true_diff|`, keeping `beta/2`. This is conservative compared with the
   textbook form that uses `z_{1-beta}` for that case.
4. `classify` is a pure function of the interval, so it cannot know the sample is tiny. For
   very small n the percentile bootstrap undercovers and a degenerate sample (all differences
   equal) yields a zero-width interval. The category is left as specified, and the report
   adds warnings for n < 30, for zero variance, for bootstrap/t category disagreement, and
   when the category is equivalent but TOST does not reject. Treat a warned `equivalent` as
   unresolved. Resolved by the coordinator: `--planned-n` and `gated_category` force
   `unresolved` below the preregistered sample size; without `--planned-n` the report is
   labelled EXPLORATORY. The warnings remain because the gate does not catch a degenerate
   sample that meets the planned n.
5. One test per call. Which comparison is primary, and any multiplicity adjustment for
   secondary comparisons (charter section 8, item 4), is the caller's responsibility; this
   package applies no adjustment.

## Fixtures

`tests/fixtures/run_a` and `run_b` are hand-built (invented numbers with hand-computed
expectations). Their `measured.csv` is invented too: component, scheduler and harness
nanoseconds were chosen so `measured_total_ns` equals what the old `bill_total` was per row.

`tests/fixtures/real_a` and `real_b` are unedited `gordian-run` output, three seeds by eleven
classes, `heuristic_only`. They and `real_aa` were regenerated for work item A8b, when
`results.csv` gained the four counted-operation columns, from the harness at the commit that
carries the calibrated weights (`source_revision` in each `manifest.json`); the older ones were
made at `b187142` and `98a27bb`:

```bash
cargo build --release -p gordian-run
target/release/gordian-run init --run-id real-a --arm real-a --policy heuristic_only \
    --seed-start 1 --seed-count 3 --trace-sample-rate 0 --out a.json
# real-b: the same manifest with run_id and arm real-b and limits.max_steps edited 1000 -> 20
scripts/cgroup-run.sh --name N --memory 2G -- target/release/gordian-run --manifest a.json --out real_a
```

Only `results.csv`, `measured.csv` and `manifest.json` are kept. Arm B is not a second policy
(`heuristic_only` is the only real one): it is the same policy under a 20-step cap, which makes
30 of its 33 episodes `undecided` with `stop_reason=step_cap` and an empty `decision_at_ns`.
It had to share arm A's seeds, because arms are paired on `(seed, class)`, so a different seed
range or trace rate would not give a pair (the trace rate does not change `results.csv`).
The manifest edit is visible in `real_b/manifest.json`. The measured nanoseconds are one
machine's wall times and are only used as numbers in tests, never as expected values.

`tests/fixtures/real_aa` is unedited `gordian-run` output of an interleaved A/A run: two copies
(`a1`, `a2`) of `heuristic_only`, three seeds by eleven classes, with the drift workload every 10
episodes:

```bash
target/release/gordian-run init --run-id real-aa --experiment A8-AA --policy heuristic_only \
    --arms a1,a2 --run-seed 1 --drift-block 10 --seed-start 1 --seed-count 3 \
    --trace-sample-rate 0 --out real-aa.manifest.json
target/release/gordian-run --manifest real-aa.manifest.json --out tests/fixtures/real_aa
```

It was made with the binary directly, not through the driver, so it is neither pinned nor
isolated; its timings are one unpinned machine's and are never used as expected values. The
fixture is kept whole: the run's `manifest.json`, `drift.csv` and each arm's `manifest.json`,
`results.csv` and `measured.csv`.

## Tests

`python -m pytest -q` from `analysis/`. Tests include hand-computed values (arithmetic in
comments), `scipy.stats.ttest_rel` / `ttest_1samp` agreement, all four categories from synthetic
data, bootstrap reproducibility and pair-preservation, loader rejection of duplicate and
unmatched keys, of unknown columns, of a results/measured key mismatch, and of an empty
decision time on a decided row (`tests/test_real_runs.py`, also the real-output fixtures), power textbook cases and monotonicity, a Monte Carlo check of
`achieved_power_t`, and the CLI end to end on `tests/fixtures/run_a` and `run_b` and on `real_a` and `real_b`.

`tests/test_stream.py` (R3b): the schema guard (the loader's columns, roles, tiers and families are
the harness's constants and the evaluator's fields); strict loading (exact integers, unknown,
missing, reordered columns, malformed counts, families on non-hard incidents, identities,
`incidents.csv` against `results.csv`); pairing on seed and its refusals; pooled ratios against
hand counts, against the mean-of-ratios mistake and for zero denominators; `family_table`;
`stream-summary` end to end, its JSON and its errors. The fixtures are written in the harness's
format by `tests/stream_fixtures.py`; the real harness output is loaded by the command in the
R3b acceptance, not by a committed copy of a run.

`tests/test_modelled.py` (A8b): the loader's modelled and measured totals; the modelled cost as
the default of `--relative-savings` with hand-computed S and a labelled secondary check on wall
time; an A/A on the modelled cost being exactly zero while the wall times differ; the median of
episode ratios by hand, and unmoved by a few interrupted episodes where the ratio of totals moves a
long way; `cost-check` passing when the model tracks wall time and failing when it does not, and
the CLI end to end. `tests/test_real_runs.py` runs the real fixtures through the same defaults and
checks that the two copies of `real_aa` have identical modelled cost in every episode.

`tests/test_ratio_methods.py` (A7b): BCa against `scipy.stats.bootstrap(method="BCa")` and its
adjusted levels by hand; the studentized interval against a plain-loop bootstrap-t on the same
indices, and its standard error against the bootstrap spread of the log ratio; that the three
methods read the same resamples; the percentile interval unchanged; infinite t kept (n = 3 gives an
unbounded lower limit); a zero total in arm B; B proportional to A; method selection, refusal and
JSON through the CLI. `tests/test_calibration.py`: the null has the true S it claims (exact
rescaling, lognormal `E[B]/E[A]`, the committed paired costs equal B2's T3), experiments replay
from their own seeds and split by index, a failing method is counted not dropped, Wilson interval
by hand, and a small seeded run shows the percentile interval liberal and the studentized one
closer on skewed empirical costs. `tests/test_calibration_slow.py` (marker `slow`, excluded from
the default run; `pytest -m slow`) is the plan's acceptance: false exceedance of the default at
most 0.06 at n = 1,237 and 1,713 over 2,000 experiments, per empirical population (about 11
minutes each).

`tests/test_drift.py` (A8): hand-computed CV and last/first ratio and every `drift.csv`
rejection; the stratified statistic and the paired statistic against hand-computed values
(the sign-flip p-value against its exact enumeration for four episodes); recovery of a known
position effect with the interval covering it; Monte Carlo checks that both p-values reject
about 5% of the time under no effect (200 simulated runs each, with a floor so a test that never
rejects fails); that class heterogeneity does not produce an effect under the null; that the
paired estimator cancels episode difficulty the stratified one cannot (its interval is more than
ten times narrower on the same data); refusal of copies that did not play identically, of
different episode sets, of one-arm runs and of non-positive costs; the margin verdict's three
categories and its strict edges; and the CLI end to end. `tests/test_real_runs.py` runs all of it
on `fixtures/real_aa`.
