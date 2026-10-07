# V1: criteria as code

Lab 2, PI report. Branch `criteria`, based on `main` at 06dbc82. Exploration; nothing here tests a
hypothesis.

## Result in five lines

1. **Schema.** A JSON specification names run roles, arms, measures (ratios of per-stream sums), the
   stream bootstrap, tested clauses (`value`, `paired`, `curve_slope`, `identity`), reports, and a
   verdict tree; `scripts/criterion.py SPEC --run ROLE=DIR --out DIR` writes `verdict.json` and
   `verdict.md`.
2. **Back-test.** 189 numbers and verdicts the review log prints for M2, B3, L1 and M3: 166 match to
   the printed precision, 11 differ by one unit in the last printed digit, 4 mismatch, 8 are not
   covered by the schema. Against the lab scripts' own CSVs, 4,209 cells (points and interval ends)
   agree to 4e-15 with no exception. Every verdict the log states is reproduced.
3. **Mutants.** Six of six caught by the back-test (each changes numbers; the wrong-filter mutant also
   flips M2's verdict) and by the unit tests (1 to 6 of 31 fail under each). The window mutant is caught only by L1's specification, because only L1 uses a window.
4. **A1b will need** a `not`/outcome-category node, a not-null filter, totals (counts) and a per-stream
   worst case or quantile, joins of two arms by incident, and a record of the order of the
   specification's commit against the run (details below).
5. **Least sure of:** the readings of the criterion text that I fixed (listed below), and that no
   specification here was written by someone other than the author of the measures.

## Verified by running, and assumed

Verified by running (all from the kept run directories under `/home/user/gordian/artifacts/runs/`,
read only, hashes of every file read recorded in each `verdict.json`):

- The four specifications evaluate; M2 and B3 hold, L1 and M3 do not (the log's verdicts).
- The module reproduces the lab scripts' CSVs at full precision (table below, 4,209 cells).
- Each printed number's agreement or disagreement in `v1-backtest.csv`.
- The 10 interval ends that missed by a digit are inside the spread of the same bootstrap under 40
  other seeds (`v1-seed-sensitivity.csv`).
- Six mutants, each run on all four specifications (`v1-mutants.csv`).
- 31 unit tests of the module on hand-written fixtures plus the script (`analysis/tests/test_criterion.py`);
  the whole analysis suite passes (507 passed, 4 deselected). One test compares the notice measures with
  `stream.notice_points` on the shared fixtures.
- Determinism: two runs of the script produce byte-identical `verdict.json` (a test) and the unseeded
  mutant does not.

Assumed:

- That the specifications' readings (below) are the chief's intent; the queue text leaves them open.
- That the kept run directories are the runs the log's numbers came from. Each manifest's `run_id` and
  `source_revision` are in the verdicts; I did not recompute byte identity (not my unit).
- Nothing about hidden simulator state is used anywhere; only evaluator output columns.

## What I did not do or did differently

- **The specifications were written from the queue's text before I read the log's result numbers.**
  Committed at `f3c407a` (schema README, `TEMPLATE.json`, four specifications), before the back-test.
  I had read the lab scripts (as ordered) and so knew their conventions (the comparator's three numbers
  are also in `m2_common.py`; I did not read any result CSV before the first run).
- **One amendment after the first back-test run, after reading the log:** `d52a26b` adds reports only
  (B3's decoy-noticed share; M3's rows paired against ramp + split over the re-anchor), because the log
  prints those numbers. No clause, test, bound, bootstrap setting or verdict changed (checked by
  diffing the files: only `measures` and `reports` differ). `m2.json` and `l1.json` are unchanged since
  `f3c407a`.
- **Scripts location.** The brief names the labs' scripts under `experiments/exploration/scripts/`; they
  are in `scripts/` on main (`m2_analyze.py`, `b2_stats.py`, `b3_table.py`, `b3_vs_m2.py`,
  `l1_stats.py`, `l1_analyze.py`, `m3_analyze.py`). `b3_stats.py` does not exist; B3's arithmetic is
  `b2_stats.py`. Immaterial.
- **Independence.** The module reads the raw CSVs with `pandas.read_csv` itself and uses nothing of
  `load.py`, `stream.py` or the lab scripts, so a defect there cannot hide in both.
- **Resources.** `pgrep -x gordian-run` showed a run in progress at my first evaluation; I run Python
  analysis only, no build. The first evaluation used cores 0-2 under the runner (13 s wall, 116 MB);
  everything after used core 2 only at a 100% quota. All under `scripts/cgroup-run.sh`, no OOM kills.
  No Rust was touched, no cargo run.
- The chief's note about A1a's hooks and E1's new `results.csv` columns (`recall_declarations`,
  `noticer_ns`) needs nothing from V1: terms read any column of any file by name.

## The schema

Full text in `experiments/criteria/README.md`; skeleton in `experiments/criteria/TEMPLATE.json`.

| part | content |
|---|---|
| `runs` | role to `{streams: {first_seed, count}}`; the script refuses a run whose streams differ |
| `arms` | name to `{run, dir}` |
| `bootstrap` | `unit: stream`, `resamples`, `seed`, `percentiles [5, 95]`, `interval: lower_higher | linear`, `chunk` |
| `measures` | `ratio` of two terms times `scale`; a term is `count` or `sum` of a linear combination of columns of one file, grouped by `seed`, filtered by `where` (`eq ne in not_in`), or `per_stream: c` |
| `clauses` | `value`, `paired` (arm minus arm, same resamples), `curve_slope` (W1's cumulative curve over the first k streams, optional paired difference of slopes), `identity` (two arms' files equal after ignoring columns); each with `window` (`first`, `last`, `slice`) and `tests` on `point`, `lower`, `upper` |
| `reports` | `table`, `paired_table`, `slope_table`: values with intervals, no tests |
| `verdict`, `observations` | trees of `all`/`any` over clause ids, named nodes recorded; observations are computed and not in the verdict |

Output: per clause the measure, value, interval, each test's observed value and outcome, whether the
clause is in the verdict; the verdict and every named node; the specification's SHA-256 of its
canonical JSON; per run role the path, `run_id`, `source_revision`, lockfile hash, manifest hash and the
SHA-256 of every file read. Exit status 0 holds, 1 does not hold, 2 error. Spec hashes at the
back-test: m2 `329cf5a7...`, b3 `7bc040df...`, l1 `c16c6b51...`, m3 `fc0bfb79...`.

Guards in the evaluator: streams must be exactly the declared range; seeds outside it are refused; a
file with one row per seed must cover every stream (so a missing stream is not silently zero); paired
arms must share streams; a clause in the verdict tree must have tests; unknown columns or arms are
errors.

## The readings I fixed (in each specification's `readings`)

1. "Paired lower bound above x" is the 5th percentile of the paired cluster bootstrap; the point
   difference must also meet the margin (M2, M3).
2. "Notices on background per stream not exceeding 6.82" and "strict precision at least 0.67" are on the
   medium's point estimate. The queue's text does not say point or interval; the lab scripts read it as
   a point.
3. M2 "holds" for the experiment means both results hold, as written, at one and the same tick length;
   the sweep is in per-tick nodes. M3 "holds" means result 1 holds at 500 ms and at 2 s under the
   background and strict-precision bounds: the four parts (margin, paired lower bound, background,
   strict precision) at each of the two ticks; result 2 and 100 ms are continuity observations.
4. M3's comparator is `ReanchorNoticer`; the review log's B3 entry says it was not re-fixed.
5. L1 clause 2's "slope over the first 100 streams" is the least-squares slope against stream number of
   W1's cumulative anchor-correct curve (cumulative anchor-correct over cumulative hard non-leak
   incidents seen, points before the first incident left out), per 100 streams, interval by resampling
   whole streams in place. Clause 3 is a point test, as the text gives no interval.
6. B3's acceptance has no result clause ("no claim about which is better"). The verdict is the
   precondition of the supplementary comparison (the three rerun comparators identical across B3's and
   M2's runs); the leak-bar trigger (leak noticed >= 0.660 within the 6.82 budget) is an observation. I
   chose this; it is the weakest-grounded of the four.
7. Interval convention: B1 to M3's scripts take the 5th percentile with `method="lower"` and the 95th
   with `"higher"`; L1's scripts take `numpy.percentile` (linear). The specifications follow the lab of
   each unit: `lower_higher` for M2, B3 and M3, `linear` for L1.

## Back-test

`v1-backtest.csv` has one row per printed number or verdict (189), with the log entry, the printed text,
the computed value at full precision and at the printed precision, the units off, and a status.
`v1-backtest-labcsv.csv` has one row per lab CSV (14 files, 4,209 cells compared). The log's numbers are
"chief's own cluster bootstrap" in several entries; those are the ones that differ at the last digit.

| unit | match | off by 1 | mismatch | not covered | verdicts and clause outcomes the log states |
|---|---|---|---|---|---|
| M2 | 49 | 3 | 0 | 4 | all reproduced (holds at 100 ms; 500 ms and 2 s do not; result 2 holds at all three) |
| B3 | 41 | 4 | 1 | 1 | all reproduced (comparators identical across runs; the leak bar is reached within budget) |
| L1 | 27 | 1 | 3 | 3 | all reproduced (clauses 1 and 3 hold, clause 2 fails, conjunction false) |
| M3 | 49 | 3 | 0 | 0 | all reproduced (result 1 fails at 500 ms and 2 s; result 2 holds at every tick; M3 does not hold) |
| total | 166 | 11 | 4 | 8 | |

Lab CSV agreement (full precision, absolute difference): M2 table 714 cells and criterion 36; B3 table
759, paired 528, vs-M2 table 273 and paired 234; L1 criterion 9, table 540, paired 60, slopes 162; M3
table 714, criterion 54, control 36, B3-paired 90. Maximum difference 1.4e-14, zero cells differing
above 1e-9. Cells of those CSVs for arms or measures the specifications do not contain (136 in M3's
table, 225 in M3's B3-paired, 56 in B3's vs-M2 table) are not compared and are counted as such.

## Discrepancies and their diagnoses

**D1. Eleven printed interval ends or values are one unit off in the last digit, one is two off.**
M2: 100 ms result 1 lower +0.021 (mine 0.02174), 500 ms lower +0.010 (0.01055), upper +0.050
(0.04945). B3: medium minus ramp + split anchor-correct upper +0.034 (0.03323), leak noticed lower
-0.014 (-0.01307), leak anchor-correct lower -0.492 (-0.49296) and upper -0.343 (-0.34483, two units).
L1: clause 3 upper -0.015 (-0.01587). M3: 100 ms upper +0.049 (0.04959), 2 s upper +0.022 (0.02273).
*Diagnosis:* interval ends of a percentile bootstrap carry Monte Carlo error of the order of a
thousandth at 10,000 resamples. The script reproduces the lab scripts' fixed-seed (9950) ends to 1e-14
in every cell, so the lab scripts and the script agree; the log's table was recomputed by the chief with
its own bootstrap (the log says so) and a different seed. Each printed end is inside the range of the
same ends under 40 other seeds (`v1-seed-sensitivity.csv`, `printed_within_range` true for all ten;
between 10 and 36 of 40 seeds round to the printed value). Neither side is wrong. The consequence for
the criterion is nil here (see D5) but the log's third digit of an interval end is not reproducible
by anything but the seed that produced it; the specification fixes the seed, which is the cure.

**D2. The log prints 0.80 for the ramp + split row's cost per stream (B3 and M3 entries); the files give
0.7948, which is 0.79.** The lab's CSVs carry the same value (agreement above), so the log rounded
0.7948 to 0.795 and again to 0.80. Log side, immaterial (0.005), but "1.85 against 0.80" is a
double-rounded comparison.

**D3. L1 clause 2 as the chief recomputed it (+0.053 [-0.037, +0.085]) is not reproduced; the lab's
figure in parentheses (+0.043 [-0.021, +0.126]) is, exactly.** I tried: the all-hard curve (+0.028), the
incident-axis slope of the cumulative curve (+0.021 per 100 incidents), the curve with undefined early
points set to zero (+0.100), the lab's incident-level reading R3b (-0.0003), block slopes (+0.009 to
+0.018). None gives +0.053. The chief's definition is not recorded in the log, so I cannot say which
side is wrong; the verdict (lower bound below zero, fails) is the same under every reading I tried
including the lab's R3b. Finding: the log's "chief's own" slope has no recorded definition.

**D4. The log says the chief "did not find a per-stream column to recompute" M3's strict precision.**
There is one: `notices.csv` carries `notices_anchor_site_correct` and `notices` per stream. The script
computes strict precision (pooled) as 0.7719 / 0.8067 / 0.8426 for 100 ms / 500 ms / 2 s, matching the
printed 0.77 / 0.81 / 0.84, with an interval. Finding about the chief's tooling, not about M3.

**D5. Sensitivity of the verdict to the seed (`v1-seed-flips.csv`).** Of the 60 tests in the four
specifications, one changes outcome under any of 40 other seeds: M2's 500 ms result 1 lower bound
("lower > 0.01": 0.01055 at the fixed seed, 0.00969 to 0.01078 across seeds, passing in 39 of 40). The
clause fails on its point test (0.0296 < 0.03) at every seed, so no verdict moves. It means the log's
printed "+0.010", which on the strict reading "above 0.01" would fail, and the fixed-seed 0.0106, which
passes, are both samples from a distribution that straddles the bound.

**D6. A back-test script error of my own, corrected.** My first comparison of the decoy figure (0.948)
used the ramp + split row (0.957); the log's "It" is the ramp noticer alone, 0.9482, which matches (the
re-anchor's 0.756 also matches). Not a discrepancy of the log.

**Not covered (8 rows, status `not_covered`).** Median notice latencies (M2 9.8 s vs 14.2 s, B3 5.5 s;
a per-incident quantile, no such measure in the schema); the medium's own operation cost (a bill
column outside the notice files); counts of incidents one arm gets right and another wrong (M2's 18 and
3, the 31 lost at 2 s; L1's "three differing in 398" and "identical on all 92 incidents"; B3's
510 added notices by class): all need a join of two arms by incident, which the schema's
per-arm, per-stream terms cannot express; and the learner's internal state (L1). The shares behind the
counts are covered and consistent (372 hard non-leak incidents, 18 missed by the comparator, 3 by the
medium, difference 15; 137/139 and 135/139 leaks).

## Mutants

`v1-mutants.csv`. Each is a single-line edit of `analysis/gordian_analysis/criterion.py`, applied to a
copy and evaluated on all four specifications against the kept runs; counts are numbers changed of
numbers compared (clause values, interval ends, every report cell).

| mutant | edit | numbers changed (M2 / B3 / L1 / M3) | verdict |
|---|---|---|---|
| M1 wrong filter | an `eq` condition selects rows that differ | 588 / 1029 / 477 / 507 | **M2's verdict flips to does-not-hold**; 24 clause or node outcomes flip |
| M2 off-by-one window | `last k` starts one stream late | 0 / 0 / 291 / 0 | none flips; caught by L1's specification only |
| M3 dropped cluster | the last stream is never resampled | 659 / 1110 / 343 / 649 | none flips (intervals move by up to 0.1) |
| M4 wrong pairing | the subtrahend arm uses shifted resamples | 198 / 478 / 33 / 170 | none flips |
| M5 wrong percentile | 10th and 90th instead of 5th and 95th | 673 / 1154 / 423 / 667 | none flips |
| M6 unseeded bootstrap | `default_rng()` without the seed | 634 / 1004 / 394 / 594 | none flips; two runs of the mutant differ in 2,651 numbers, two runs of the script in none |

All six change numbers; only M1 changes a verdict, and only M2's. The other five do not flip a verdict
on these four runs because every clause's decision sits far from its bound (D5), so a verdict is a
poor detector of a bootstrap defect here; the numbers are the detector. The unit suite
(`test_criterion.py`, run from outside `analysis/` with the mutated copy first on `PYTHONPATH`) also
kills all six: 6, 2, 3, 1, 1 and 3 of its 31 tests fail under M1 to M6 respectively (column
`unit_tests_failed_of_31`; this check was run by hand and is not in `v1_mutants.py`). The M2 mutant being
invisible to three of four specifications is a property of those specifications (no window), not of
the mutant.

## What I am least sure of

- **The readings (items 1 to 7).** Reading 2 (bounds on a point) and reading 5 (which slope) carry the
  most weight. Under L1's reading R3b the clause also fails, so the verdict is robust to it; under a
  reading that put the background bound on an interval, M2's 100 ms background (5.44 [5.145, 5.74])
  is also inside 6.82, so M2 would hold either way; M3's 500 ms and 2 s rows fail on the point
  margin regardless.
- **B3's verdict definition.** I made it the identity precondition, because the queue gives no
  pass/fail. A reader could argue the trigger observation is the verdict.
- **The module has one author.** The independence is from the lab scripts, not from a second
  reader. The lab-CSV agreement is strong evidence of the same arithmetic, not that the arithmetic is
  what the charter means (a shared reading, not shared code, is the weak point).
- **`curve_slope` with a paired `minus`** is implemented and tested against the difference of
  slopes but not used by any specification, so it has no back-test beyond L1's reported paired slope
  differences, which I did not compare (the lab's `l1-paired.csv` curve-slope rows are not in the
  specification's reports).

## What the chief should examine most carefully

1. The `readings` of each specification: they are where my judgement replaced the text.
2. D3: the definition of the chief's own L1 slope, which no script in the repository reproduces.
3. D5 and the 500 ms M2 row: a clause whose lower bound is seed-sensitive near 0.01 is a candidate for
   a stated margin of safety in A1b's bounds; the fixed seed makes the verdict reproducible but not
   robust where a bound sits within about 0.001 of the estimate.
4. That `verdict.json` records the specification's hash but nothing enforces that the specification was
   committed before the run. Order is by git history only.
5. The amendment `d52a26b` (reports only).

## What A1b's specification will need that the schema does not yet express

From `docs/lab-queue.md` "## A1" and the W2 entry's decisions 1 and 2 (shares over filtered
populations; a floor with a lower bound; a paired margin against the record rung with a pre-accepted
"the record rung captures the lever" outcome; stale clause split into absolute and paired; every arm
with and without the reset):

- **Not-null and not-equal-to-column filters.** "Hard incidents reachable by the key form" depends on
  `recurrence_of` and `same_family_earlier` (E1), likely `where` `notnull`/`isnull`; the schema's
  `where` compares a column with a constant only. Cheap to add.
- **Outcome categories with a pre-accepted alternative.** The charter's four categories (beneficial,
  harmful, practically equivalent within margin, unresolved) and a named alternative result need a
  `not` node and an exclusive outcome node (exactly one of a listed set holds), reported by name. The
  tests on `lower` and `upper` already express each category's condition; the combination does not.
  This is the largest gap.
- **Totals and denominators as clauses.** "A floor that is unresolvable under 41 reachable recurrences"
  needs a clause on a count (a `sum` kind, not a ratio) so that power is a tested precondition.
- **Per-stream and per-recall bounds.** Per recall is a ratio over `recall_declarations` and fits.
  "Per stream" as a worst case or a quantile across streams (a bound that must hold in every stream, or
  in 90% of them) is a different statistic: a distribution measure over per-stream values, with a
  bootstrap of its own.
- **Evaluation with and without the reset.** Arms already carry it; what is missing is a way to say
  that a clause must hold for each variant and form (a generator over arms) rather than writing the
  cross product out. The `all` node is enough, written long; a template could shorten it.
- **Arm-to-arm joins by incident.** "Inherited error" as the paired excess over the memoryless arm is
  a ratio of sums and fits; "the same incidents the record rung gets right" does not. Needed for the
  reports that the log already prints in words (discordance counts).
- **Quantile measures** (latency medians), since E1's `noticer_ns` and calls per correct decision may
  want them.
- **Order of commit and run.** A field in the specification naming the commit it was frozen at, and a
  check in the script that the run directory's manifest `source_revision` and creation time follow it,
  or at least print both.
- **Multiplicity.** Secondary comparisons "multiplicity-adjusted when used for any claim" (charter 8.4)
  are not expressible; the schema has no way to mark a clause as primary or secondary.

## Files

- `scripts/criterion.py`; `analysis/gordian_analysis/criterion.py`; `analysis/tests/test_criterion.py`.
- `experiments/criteria/{README.md, TEMPLATE.json, m2.json, b3.json, l1.json, m3.json}`.
- `experiments/exploration/v1-backtest.csv`, `v1-backtest-labcsv.csv`, `v1-seed-sensitivity.csv`,
  `v1-seed-flips.csv`, `v1-mutants.csv`.
- `experiments/exploration/scripts/v1_backtest.py`, `v1_seeds.py`, `v1_seedflips.py`, `v1_mutants.py`.

To reproduce one verdict:

```bash
PYTHONPATH=analysis analysis/.venv/bin/python scripts/criterion.py experiments/criteria/l1.json \
  --run fresh=artifacts/runs/l1/l1-fresh-b5-rho0.7 --out /tmp/l1
```
