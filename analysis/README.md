# gordian-analysis

Paired statistical analysis of Gordian run directories (work item A7). It reads the
`results.csv` and `measured.csv` that the recorder writes (and `usage.json` when present),
pairs two arms on `(seed, class)`, and reports the charter section 8 quantities. The unit of replication is the
episode; nothing here is computed per event.

```bash
python3 -m venv analysis/.venv
analysis/.venv/bin/pip install -e 'analysis[dev]'
cd analysis && .venv/bin/python -m pytest -q
```

## Commands

```bash
gordian-analyze compare --a RUNDIR --b RUNDIR --metric success --margin 0.01 \
    --higher-is-better --seed 1 [--alpha 0.05] [--interval bootstrap|t] \
    [--resamples 10000] [--planned-n N] [--json FILE]
gordian-analyze compare --a RUNDIR --b RUNDIR --relative-savings \
    --threshold 0.20 --seed 1 [--planned-n N] [--alpha 0.05] [--resamples 10000] [--json FILE]
gordian-analyze compare --a RUNDIR --b RUNDIR --relative-savings --declared-cost \
    --metric bill_compute --threshold 0.20 --seed 1
gordian-analyze power --sd 1 --margin 0.5 --alpha 0.05 --power 0.8 \
    [--true-diff 0] [--equivalence] [--json FILE]
```

`--seed` is required, and exactly one of `--higher-is-better` / `--lower-is-better` is
required (there is no default, so a cost metric cannot silently be read with the wrong sign).
Metrics: `success`, `critical_miss`, `false_alarm`, `abstained`, `undecided`, `probes_used`,
`corrections`, `decision_at_ns`, each `bill_*` column, each `measured_*_ns` column, and
`measured_total_ns`. `measured_total_ns` is the sum of the three measured columns, computed in
memory; input files are never modified. Booleans are accepted as `true/false/0/1`. `--margin`
is in the metric's own units.

`bill_total` no longer exists. It added nanoseconds to probe counts and bytes, which have no
common unit; asking for it is an error that says so. The `bill_*` columns are declared cost,
one resource each, and are compared one at a time.

`decision_at_ns` is undefined for an undecided episode, so a comparison on it is refused while
either arm has one. It is not computed over the decided episodes alone, which would condition
the comparison on the outcome; compare `undecided` and `success` instead.

### Which cost: `--relative-savings`

The charter's cost `C` is measured wall time (`docs/local-test-plan.md`, A4 and A5). So
`--relative-savings` defaults to `--metric measured_total_ns` and the report says
"Cost basis: measured wall time, the charter's cost C". Other choices:

- another `measured_*_ns` column is allowed and labelled as one part of the episode, not `C`;
- a `bill_*` column is refused unless `--declared-cost` is passed, and the report then says
  "DECLARED cost (a bill column), NOT the charter's cost C". The JSON carries `cost_basis`
  (`measured`, `measured_partial`, `declared`);
- anything that is not a cost (`success`, `decision_at_ns`, ...) is refused.

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
  kept for the risk-coverage curve; the harness does not write it yet;
- a value that is not boolean or not finite-numeric, more than one `run_id`, or duplicate
  `(seed, class)` in either file;
- `decision_at_ns` empty on a decided row, or present on an undecided one. It is empty exactly
  when `undecided` is true (evaluator R9) and loads as NaN there; every other column must be
  filled;
- keys in one file and not the other, or a `run_id` that differs between the two files. There
  is no silent drop, fill or outer join.

`stop_reason` and `directives_ignored` load as is; `stop_reason` must be non-empty and its
values are not checked against a list. `pair_runs` / `load_pair` join two arms on
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
and recomputes S from the resampled totals; the interval is the equal-tailed percentile
interval of those S values. Positive S means B costs less. It applies no sign flip and is only
meaningful for cost-like metrics (lower is better), so the CLI mode refuses `--higher-is-better`.
It raises on negative values, on `sum(A) == 0`, and if any resample has `sum(A) == 0`.
`equivalence.exceeds(ci_low, threshold)` is `ci_low > threshold` (strict); for EXP-001 the
threshold is 0.20. `compare --relative-savings --threshold T` reports S, its interval, and
whether the lower limit exceeds T; `--margin` and `--interval` do not apply and are refused,
as is `--threshold` outside this mode. This mode does not produce a four-way category, only
exceeds / does not exceed (and, below the planned n, unresolved).

Caveat measured here, not proved: with skewed per-episode costs (lognormal, true S exactly
0.20), the percentile bootstrap lower limit exceeded 0.20 in 12% of 300 simulated experiments at
n = 30, 8% at n = 100, and 7% at n = 300, against a nominal 5%. The decision is therefore
somewhat liberal at moderate n when costs are heavy-tailed. Treat a borderline `exceeds` with
that in mind; a bias-corrected interval was not added because it is outside the specification.

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
classes, `heuristic_only`, from the harness at commit `b187142`:

```bash
CARGO_BUILD_JOBS=1 cargo build --release -p gordian-run
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

## Tests

`python -m pytest -q` from `analysis/`. Tests include hand-computed values (arithmetic in
comments), `scipy.stats.ttest_rel` / `ttest_1samp` agreement, all four categories from synthetic
data, bootstrap reproducibility and pair-preservation, loader rejection of duplicate and
unmatched keys, of unknown columns, of a results/measured key mismatch, and of an empty
decision time on a decided row (`tests/test_real_runs.py`, also the real-output fixtures), power textbook cases and monotonicity, a Monte Carlo check of
`achieved_power_t`, and the CLI end to end on `tests/fixtures/run_a` and `run_b` and on `real_a` and `real_b`.
