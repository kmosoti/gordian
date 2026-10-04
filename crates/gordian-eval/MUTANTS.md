# Mutation testing of gordian-eval

The plan (A2) requires a kill rate of at least 90%. Command and result, both from a run on this
machine (cargo-mutants 27.1.0, `CARGO_BUILD_JOBS=2`):

```bash
cargo mutants -p gordian-eval -j 2
# Found 39 mutants to test
# 39 mutants tested: 35 caught, 4 unviable
```

| | count |
|---|---|
| Mutants generated | 39 |
| Caught (a test failed) | 35 |
| Missed (all tests passed) | 0 |
| Timed out | 0 |
| Unviable (the mutant does not compile) | 4 |

Kill rate: 35 of 35 viable mutants, 100%. cargo-mutants excludes unviable mutants from the rate
because no test can run against them. If they were counted as survivors the rate would be 35 of
39, 89.7%, under the plan's bar, so the four are listed below with what was done about each rather
than left as a footnote.

No test was added or changed in response to this run: there were no survivors.

## Surviving mutants

None.

## Unviable mutants

Each replaces a function body with `Default::default()` or `Ok(Default::default())`; the types
involved do not implement `Default`, so the mutant fails to compile. Deriving `Default` on
`Truth` and `Verdict` only to make these viable would add a public constructor for a meaningless
value (an all-false verdict that looks like a real result), so it was not done. The behaviour
each mutant stands for was checked by hand instead (see the next section).

| Mutant | What it stands for | Hand-applied equivalent, result |
|---|---|---|
| `score.rs:132` `score` -> `Ok(Default::default())` | `score` returns a constant | Not run as a mutant. Any constant verdict contradicts fixtures that expect different verdicts, so this is deduced, not measured. |
| `timeserde.rs:10` `instant::serialize` -> `Ok(Default::default())` | an `Instant` serializes to nothing | Replaced by `serialize_u64(0)`: caught by `instants_are_written_as_plain_nanoseconds` and the round-trip test. |
| `timeserde.rs:26` `option_instant::serialize` -> `Ok(Default::default())` | an `Option<Instant>` serializes to nothing | Replaced by serializing `None`: caught by the same tests. |
| `truth.rs:24` `Truth::from_episode` -> `Default::default()` | the truth of an episode is a constant | Replaced by dropping the faults, and separately by a fixed class: both caught by `tests/generated.rs`. |

## Hand-applied mutants

cargo-mutants does not change constants, swap iterator methods, or reorder checks. These were
applied by hand, one at a time, with the package's tests run after each and the source restored.
All 19 were caught. The script recorded only that some test failed; the tests named below are
the ones expected to catch each mutant, by reading the fixtures, not an observed attribution.

| Mutant | Expected to be caught by |
|---|---|
| `probes_used` increments by 2 | fixtures (`three-probes-two-corrections`, others), property |
| `corrections` increments by 2 | same |
| `probes_used` increments by 0 | same |
| `previous` instant never recorded (time check never fires) | the `error/time-goes-backwards*` fixtures |
| success ignores the site | `ambiguous/right-kind-wrong-site`, `a_wrong_site_is_never_a_success` |
| success ignores the kind | `ambiguous/right-site-wrong-kind`, `a_wrong_kind_is_never_a_success` |
| kind and site matched against different faults | `two-faults/kind-of-one-site-of-the-other` |
| `has_fault` uses `all` instead of `any` | `two-faults/declare-the-non-critical-one` |
| `has_critical_fault` uses `all` instead of `any` | `two-faults/abstain` |
| `decision_at` is a constant | fixtures |
| `undecided` is true only for an empty trajectory | `ambiguous/probe-then-budget-exhausted` and others |
| `from_episode` drops the faults | `tests/generated.rs` |
| `from_episode` fixes the class | `tests/generated.rs` |
| `instant::serialize` writes 0 | `instants_are_written_as_plain_nanoseconds` |
| `option_instant::serialize` writes `None` | same |
| a probe step accepts any outcome | the `error/*-outcome` fixtures |
| a correction step accepts any outcome | same |
| a declare or abstain step accepts any outcome | same (hand-applied on the formatted source) |
| R14 and R15 checks swapped | `error/after-terminal-beats-time-backwards` |

## What this does not show

Mutation testing shows that the tests notice changes to the code that exists. It does not show
that the rules in `RULES.md` are the right rules. Two rules are judgement calls and are stated as
such there: R5 with R6 (an arm can be `success` and `false_alarm` together) and the reading of
"attempted a `Correct`" as carried out.
