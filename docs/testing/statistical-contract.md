# Statistical Contract

Status: **normative research specification**

`docs/research/methodology.md` section 9 previously specified no test, no effect-size measure, no
power or precision target, no number of seeds, and no multiplicity policy;
`docs/research/verification-strategy.md` required repeated runs without an `n`. Under that wording
two honest agents can analyse the same data and reach opposite conclusions, and neither is
violating the specification. This document fixes the values. It closes G-122 and, together with
section 3, G-110.

Every experiment belongs to exactly one **class**. The class fixes the design; the manifest fills
in the class's named slots. An `ExperimentProtocol` whose `class` is absent, or whose
`analysis_plan` omits any of the five required fields, is invalid
(`experiments/schema/experiment-protocol.schema.json`, enforced by #75).

## 1. Universal rules

1. **One primary metric.** Exactly one metric is primary. It is named before any data is
   inspected. Every other metric is secondary and is reported under the class's multiplicity
   policy. A conclusion MUST NOT be stated on a secondary metric alone.
2. **Minimum effect size.** The manifest states the smallest effect that would change the
   architecture. An observed effect below it is reported as "no architecturally relevant
   difference", whatever its p-value. Statistical significance is not the decision rule; the
   pre-registered minimum effect is.
3. **Fixed n, no peeking.** The manifest states `min_n` as an integer per cell. Analysis runs
   once, after all runs complete. Optional stopping requires a pre-registered sequential design.
4. **Retention.** Failed, timed-out, excluded, and negative runs are retained with reasons. The
   exclusion rule is pre-registered and is applied by a script, never by judgement.
5. **Baseline.** Every experiment names a baseline condition, and the simplest credible
   alternative is included whenever a comparison can be run at all.
6. **Parsimony.** When the primary metric shows no architecturally relevant difference, the
   simpler condition wins by default. Complexity requires positive evidence.

## 2. Classes

| Slot | `agent-trial` | `benchmark` | `classification` | `fault-injection` |
| --- | --- | --- | --- | --- |
| Design | paired on workload item; conditions in randomized order | paired on input instance | single labeled corpus, all predictors scored on it | enumerated fault x seed grid |
| Primary metric | one of: success at fixed budget, wall-clock to Mission satisfaction, total token+tool cost | one of: median wall-clock, allocations, peak RSS | one of: PR-AUC, expected total cost | fail-closed rate (detected and refused) |
| Effect size | paired difference (proportions) or Hodges-Lehmann shift (continuous) | ratio of medians | delta versus the P1 baseline | exact proportion |
| Uncertainty | BCa bootstrap 95% CI, 10000 resamples, seed recorded | BCa bootstrap 95% CI, 10000 resamples | stratified bootstrap 95% CI, 10000 resamples | Clopper-Pearson 95% interval |
| `min_n` | 30 workload items x 5 seeds = 150 trials per cell | 10 warmup + 30 measured iterations per instance, at least 5 instance shapes | at least 200 labeled integration episodes with at least 50 positives | 20 seeds per enumerated fault |
| Multiplicity | Holm-Bonferroni over the pre-registered secondary family | Holm-Bonferroni over the benchmark family | Holm-Bonferroni over predictor-vs-baseline comparisons | none; each fault is its own gate |
| Precision target | 95% CI half-width at most 0.05 on the primary | lower CI bound of the ratio used as the regression gate | 95% CI half-width at most 0.05 on the primary | zero observed failures required |
| Stopping rule | fixed n; no interim analysis | fixed n; re-run only on a documented environment change | fixed corpus; frozen before scoring | fixed grid; any failure stops the run and files a defect |

Notes that are part of the contract, not commentary:

- `agent-trial` pairs on the workload item because agent variance between items dwarfs variance
  between conditions. An unpaired agent-trial requires an explicit justification field.
- `fault-injection` deliberately runs no null-hypothesis test. A safety gate that fails closed
  most of the time has failed. The Clopper-Pearson upper bound is reported so the residual
  uncertainty from `n = 20` is visible, not to license a nonzero failure rate.
- `benchmark` reports a ratio, not a difference, so thresholds survive hardware changes.

## 3. Confounders and controls

Three confounders are systematic rather than statistical: no amount of `n` removes them, and each
is closed by a fixed value recorded in the run manifest rather than by an analyst's judgement.
Each row states the control, the numeric value, and the manifest field that carries it.

| Confounder | Control | Fixed value | Manifest field |
| --- | --- | --- | --- |
| Asymmetric tuning: the Gordian arm receives more engineering and tuning effort than its baselines | Equal engineering-and-tuning budget per arm, spent before any measured run and frozen at the first measured run | 40 person-hours **and** 20,000,000 agent tokens per arm; any arm that overspends either figure invalidates the cell | `tuning_budget_per_arm` (required in `ExperimentRun`; one entry per condition id, each `{ unit, value }`) |
| Non-blind metrics: the implementer of an arm authors the metric that judges it | Every complexity metric is computed by one pinned tool at one exact version across all arms; hand-counting by an implementer is prohibited and no implementer-authored count is admissible as a primary metric | `tokei 12.1.2` for lines-of-code and file counts; the full argv and the tool's own version output are recorded verbatim | `metric_tooling` (required; array of `{ metric_id, tool, version, invocation }`) |
| LLM nondeterminism: run-to-run variance from sampling swamps the condition effect | Fixed temperature, a fixed seed list, and repeats per cell at or above the class `min_n` | temperature `0.0`; seed list `[1, 2, 3, 4, 5]`; repeats per cell at least 150 for `agent-trial` (30 workload items x 5 seeds), at least 20 per fault for `fault-injection` | `nondeterminism_controls` (required; `{ temperature, seed_list, repeats_per_cell }`) |

All three fields are non-nullable in `experiments/schema/experiment-run.schema.json`. #75 carries a
`#[test]` per field asserting that a run manifest omitting it fails to deserialize, on the same
footing as the five `analysis_plan` fields of section 5. Issues #34 and #39 restate the three
values that bind them in their own bodies; a link is not a restatement.

## 4. Assignments

| Experiment | Class | Primary metric | Minimum effect |
| --- | --- | --- | --- |
| E001 atom granularity (#51) | agent-trial | wall-clock to Mission satisfaction at fixed budget | 15% relative reduction versus `coarse` |
| E002 conflict prediction (#52) | classification | expected total cost (missed conflict + false serialization) | 10% relative cost reduction versus P1 |
| E003 isolation/coordination (#39) | agent-trial | success at fixed budget | 10 percentage points |
| E004 snapshot vs rebase (#53) | agent-trial | wall-clock to Mission satisfaction at fixed budget | 15% relative |
| E005 source substrate (#34) | agent-trial | operator-intervention-free completion rate | 10 percentage points |
| E006 derived vs mutable state (#54) | agent-trial | incorrect scheduling decisions per 100 dispatches | 50% relative reduction |
| Foundation benchmarks (#3, #5) | benchmark | median wall-clock per shape | 5% ratio gate |
| Scheduler benchmarks (#24) | benchmark | median wall-clock per shape | 5% ratio gate |
| Knowledge-graph benchmarks (#61) | benchmark | median query wall-clock | 5% ratio gate |
| F001-F003, #28, #42, #67 | fault-injection | fail-closed rate | zero failures |
| `experiment:compositional-verifier-inheritance` | classification | rate of an inherited pass co-occurring with an integration failure | any rate above 1% removes the `compositional` flag |

E001 previously listed eleven outcome metrics with no primary. Ten of them are now secondary and
reported under Holm; the eleventh is the decision.

## 5. Manifest binding

`ExperimentProtocol.analysis_plan` has five required, non-nullable fields:

```text
primary_metric      one metric id, must appear in metrics[]
effect_size         { measure, minimum_relevant }
min_n               integer, per cell
multiplicity        holm | none  (none is legal only for class = fault-injection)
stopping_rule       fixed_n | sequential:<pre-registered design>
```

#75 MUST deserialize a manifest missing any one of the five as an error, and a `#[test]` asserts
exactly that, once per field. There is no default and no inference from the class: the class table
in section 2 says what to write, and the manifest must still say it, because the manifest is what
the analysis code reads.

Pre-registration is enforced by digest, not by good faith. `ExperimentRun.protocol_digest` is the
digest of the `protocol.json` the run executed under; a run whose digest does not match the
protocol now on disk is a post-hoc run and is reported as one. G-124 assigns the registry, the CI
digest recomputation, and the `gordian-kg` audit rule that rejects an Experiment node without a
well-formed digest to **#75**.

## 6. Forbidden wording

Hedging vocabulary that converts a fixed value back into a judgement call MUST NOT appear in this
document, in an experiment manifest's `analysis_plan`, or in the acceptance section of an
experiment issue. The banned list is held in `scripts/check-statistical-contract.sh` as
`BANNED_PHRASES`, **not** in this document, precisely so that this document does not contain the
strings it forbids. The checker greps this file, every `analysis_plan`, and every experiment issue
body, and fails on a hit.

## 7. Amendment

The constants above are engineering judgements, not the output of a power analysis on measured
variance. They are revisable, by a commit that changes this document, states the rationale, and
re-registers every affected `protocol.json` under a new digest. They are never revised while an
experiment is running: a change to a constant mid-experiment invalidates every run already
recorded against the superseded digest.
