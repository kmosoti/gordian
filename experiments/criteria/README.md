# Criteria as code

A fixed criterion is a machine-evaluable specification committed before its unit runs
(charter section 11, "The verdict is computed, not read"). `scripts/criterion.py` evaluates one:

```bash
analysis/.venv/bin/python scripts/criterion.py experiments/criteria/m2.json \
    --run heldout=artifacts/runs/m2/m2-heldout-b5-rho0.7 --out artifacts/criteria/m2
```

It writes `verdict.json` (every clause with its measure, value, interval, tests and outcome; the
verdict; the specification's SHA-256; each run directory's identity and the SHA-256 of every file
read) and `verdict.md`. The measures are implemented once, in
`analysis/gordian_analysis/criterion.py`; the script only composes them. The output is
deterministic under the specification's bootstrap seed. The exit code is 0 when the verdict holds,
1 when it does not, 2 on a specification or input error.

## Specification

A JSON object. `TEMPLATE.json` is the skeleton; `m2.json`, `b3.json`, `l1.json`, `m3.json` are the
four encoded so far (their clauses are unchanged by work item E1's extensions, and the back-test
reproduces their verdicts byte for byte); `a1b.draft.json` is a draft of A1b's, which the chief fixes.

| key | meaning |
|---|---|
| `schema` | `gordian-criterion/1` |
| `id`, `title`, `source` | the unit, a line, and where the criterion is fixed (file, section, dates of fixing and amendments) |
| `readings` | every reading of the criterion's text that the text leaves open, written before the run |
| `runs` | run roles: `{role: {"streams": {"first_seed", "count"}}}`. The command line binds each role to a directory with `--run role=dir`. The streams of every arm read from a role must be exactly `first_seed ... first_seed + count - 1` (the script stops otherwise) |
| `arms` | `{name: {"run": role, "dir": subdirectory of that run}}`. An arm's evaluator files are read from there |
| `bootstrap` | `unit` is `stream` (whole streams are the clusters; nothing else is implemented); `resamples`; `seed`; `percentiles` `[lo, hi]`; `interval`: `lower_higher` takes `numpy.quantile(x, lo/100, method="lower")` and `(x, hi/100, method="higher")` (the B1 to M3 scripts), `linear` takes `numpy.percentile` (L1's script); `chunk` is the number of resamples drawn per call (the draw order is part of the seed's meaning) |
| `measures` | `{name: {"kind": "ratio", "num": TERM, "den": TERM, "scale"}}`: a ratio of sums over streams (never a mean of per-stream ratios), times `scale` (default 1); or `{"kind": "quantile", "q", "num", "den", "scale"}`: the `q`-quantile over streams of the per-stream value `num / den` (below) |
| `clauses` | the tested statements (below) |
| `reports` | tables of values and paired differences with intervals and no tests; "reported beside, never folded in" |
| `verdict` | a tree over clause ids; `{"name", "all": [...]}`, `{"name", "any": [...]}` or `{"name", "not": ITEM}`; items are clause ids or nested nodes. Every named node's value is in `verdict.json` |
| `observations` | `{name: tree}`: trees evaluated and reported but not part of the verdict |
| `outcomes` | `{"name", "default", "categories": [{"name", "when": tree}, ...]}`: exclusive outcome categories. The outcome is the first category, in the order written, whose tree holds, else `default`; every later category that also held is reported as shadowed, so no data can make the outcome ambiguous, and the order is the specification's reading. Optional; `verdict` stays the criterion's pass or fail |

**TERM.** `{"file", "agg": "count" | "sum", "columns": {column: coefficient}, "where": [...]}`
aggregates one evaluator file per stream (grouped by `seed`; a stream with no matching row gets 0):
`count` is the number of rows passing `where`; `sum` is the sum over those rows of the linear
combination of `columns` (boolean columns count as 0/1). Or `{"per_stream": c}`, the constant `c` for
every stream (a ratio over it is a mean per stream). A `where` item is `{"column", "op", "value"}`
with `op` in `eq`, `ne`, `in`, `not_in`, `notnull`, `isnull` (the last two take no `value`: an empty cell of a CSV is null, as in `recurrence_of` of `memory_incidents.csv`); an empty cell never equals a value, so `ne` is true for it.
The files are `notice_incidents.csv`, `notices.csv`, `incidents.csv`, `results.csv`, ...: any CSV of
an arm directory with a `seed` column. The set of streams of an arm is the seeds of its `results.csv`.

**Join.** A term may carry `"join": {"arm": NAME, "on": ["seed", "incident"], "where": [...]}` (`on` defaults to `seed` and `incident` and must include `seed`): the arm's rows of the term's file are inner-joined, on those columns, to the other arm's rows of the same file that pass `join.where`, and the other arm's columns are named `column@NAME` for the term's `where` items and `columns`. The two arms must be on the same streams and hold exactly the same keys, each key naming one row, or the script stops: a join that silently dropped incidents would be a measure of nothing. It is how a measure reads two arms incident by incident (the correct declarations a recall displaced; the incidents on which an arm and its control differ); a difference of stream totals is the existing `paired` clause.

**Window.** `{"first": k}`, `{"last": k}` or `{"slice": [a, b]}` (0-based, half-open) over the
streams in seed order; omitted means every stream. The bootstrap of a windowed clause resamples the
streams of the window only.

**Clauses** (`id`, `kind`, `description`, `tests`):

- `value`: `arm`, `measure`, `window`. The pooled value with its interval.
- `paired`: `arm`, `minus`, `measure`, `window`. The value of `arm` minus that of `minus` over the
  same resampled streams; the interval is of the difference.
- `curve_slope`: `arm`, optional `minus`, `measure`, `window` (`first`), `per`. The least-squares
  slope against the stream number of the cumulative curve of the measure's numerator over its
  denominator (points before the first denominator are left out), times `per`; the interval
  resamples whole streams in place and recomputes the weighted cumulative curve. With `minus`, the
  difference of two slopes over the same resamples.
- `count`: `arm`, `measure`, `window`, with tests on `count` (the measure's numerator summed over the
  streams of the window) and `total` (its denominator): how many events a share rests on, so that
  power is a clause ("at least 70 hard recurrences"). No interval; `point`, `lower` and `upper` are
  not tests of a count clause, and `count` and `total` are tests of no other. A ratio measure only.
- `identity`: `a`, `b`, `files`, `ignore_columns`. The two arms' files are equal as text after the
  ignored columns are dropped. No interval.

`tests` is a list of `{"on": "point" | "lower" | "upper", "op": ">=" | ">" | "<=" | "<", "value"}`,
all of which must hold. A NaN fails. A clause without tests is reported and has no outcome.
`lower` and `upper` are the bootstrap interval's ends.

**The quantile measure.** The value of a stream is `num / den` times `scale` where `den` is positive (a stream with no denominator is left out); the measure is the smallest value whose cumulative weight over streams is at least `q` of the total weight (numpy's `inverted_cdf`, the ordinary quantile when every stream counts once), and on a resample the weights are that resample's stream counts. `value` and `paired` clauses read it (a paired clause is the difference of two quantiles over the same resamples); `curve_slope` and `count` do not.

**Reports** are `table` (`arms` x `measures`, `window`), `paired_table` (`pairs` of arm names, the
first minus the second, x `measures`, `window`) and `slope_table` (`arms`, `measure`, `firsts`,
`per`).

## Conventions

- The bootstrap draws the stream counts with `numpy.random.default_rng(seed).multinomial(n,
  [1/n]*n, size=k)` in chunks of `chunk`, once per distinct window length, so every measure and
  every arm of a window shares the same resamples and differences are paired.
- Resamples with an undefined statistic (a zero denominator) are dropped before the interval is
  taken.
- A clause's tests are written from the queue's text. Where the text leaves a reading open
  (which percentile, whether a bound is on a point or on an interval, which curve), the reading
  is in `readings`, and the choice is the specification's, made before the run.
- A specification is committed before the run it governs; a correction after the run is a new
  commit that says what changed and why, and the verdict file carries the specification's hash.
