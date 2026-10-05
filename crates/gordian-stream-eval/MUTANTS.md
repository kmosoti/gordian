# Mutation testing of gordian-stream-eval

The plan (A2, and R2 "mutation testing as in A2") requires a kill rate of at least 90% of viable
mutants, with survivors listed here and a reason each. Command and result, from a run on this
machine (cargo-mutants 27.1.0, `CARGO_BUILD_JOBS=2`, `-j 2`):

```bash
cargo mutants -p gordian-stream-eval -j 2
# Found 152 mutants to test
# 152 mutants tested in 3m: 145 caught, 7 unviable
```

| | count |
|---|---|
| Mutants generated | 152 |
| Caught (a test failed) | 145 |
| Missed (all tests passed) | 0 |
| Timed out | 0 |
| Unviable (the mutant does not compile) | 7 |

Kill rate: 145 of 145 viable mutants, 100%. cargo-mutants excludes unviable mutants from the rate
because no test can run against them. Counting them as survivors the rate would be 145 of 152,
95.4%, still over the bar. All seven are listed below with what was done about each rather than
left as a footnote.

## Surviving mutants

None.

One change was made to the source because of an earlier run of this tool: two comparisons of the
form `x as usize >= len` in `score.rs` produced mutants (`as usize < len`) that do not parse (`<`
after a cast opens generic arguments), so four real logic mutants were unviable. The casts are
now parenthesised, `(x as usize) >= len`, and the mutants are viable and caught. The first run was
152 mutants, 143 caught, 9 unviable; the second, with the change, is the run above. No test was
added or changed in response to either run: there were no survivors.

## Unviable mutants

| Mutant | What it stands for | Hand-applied equivalent, result |
|---|---|---|
| `bridge.rs` `truth_from_stream` -> `Default::default()` | the truth of a stream is a constant | `StreamTruth` has no `Default`. Replaced by dropping the incidents of the revealed truth: caught by every test in `tests/generated.rs` that scores a real stream (first three named by the run: `a_declaration_the_stream_refused_scores_nothing`, `a_silent_trajectory_misses_every_incident_and_every_critical_one_critically`, `declaring_a_wrong_site_for_every_incident_scores_wrong_and_alarms_every_decoy`) |
| `bridge.rs` `calls_from_sim` -> `vec![Default::default()]` | the call summaries are a constant | `CallSummary` has no `Default`. Replaced by exactly one all-zero summary: caught by `tests/generated.rs` (`a_silent_trajectory_misses_every_incident_and_every_critical_one_critically` and others) |
| `score.rs` `check_truth`, `&&` -> `||` in a let-chain | any incident-labelled observation is an unknown incident, or the check is always off | A let-chain cannot contain `||`. Replaced by dropping the second condition, so that every incident-labelled observation is an error: caught by `a_correct_declaration_in_time_is_correct` and others |
| `score.rs` `check_named`, `&&` -> `||` in a let-chain | every observation of an incident is "not yet emitted" | Same reason. Replaced by dropping the second condition: caught by `a_correct_declaration_in_time_is_correct`, `a_wrong_declaration_never_removes_a_correct_one`, and others |
| `score.rs` `score_stream` -> `Ok(Default::default())` | `score_stream` returns a constant | `StreamVerdict` has no `Default`; deriving it only to make this viable would add a public constructor for a meaningless value (an empty verdict that looks like a stream with no incident). Replaced by returning an empty verdict after the truth check: caught by the fixtures, the properties and the generated-stream tests |
| `timeserde.rs` `instant::serialize` -> `Ok(Default::default())` | an `Instant` serializes to nothing | Replaced by `serialize_u64(0)`: caught by `instants_are_written_as_plain_nanoseconds` and `fixtures_survive_a_serialization_round_trip` |
| `timeserde.rs` `option_instant::serialize` -> `Ok(Default::default())` | an `Option<Instant>` serializes to nothing | Replaced by serializing `None`: caught by the same two tests and `truth_and_verdict_are_reproducible_and_survive_json` |

## Hand-applied mutants

cargo-mutants does not change constants, drop a call or reorder checks. These were applied by
hand, one at a time, to `score.rs`, with the crate's whole test suite run after each and the
source restored (`cargo test -p gordian-stream-eval`; a script held the list, and it recorded
which tests failed). All 35 were caught. The tests named are the first three the run reported as
failing, an observed attribution and not an expectation.

| Mutant | Caught by |
|---|---|
| on-time test is `<` instead of `<=` | `every_fixture_produces_its_expected_result`, `the_deadline_itself_is_on_time` |
| observation-not-yet-emitted test is `<=` | `a_wrong_declaration_never_removes_a_correct_one`, `every_fixture_produces_its_expected_result` |
| end-of-stream test is `>=` | `every_fixture_produces_its_expected_result`, `the_evaluators_reading_of_escalations_agrees_with_the_simulators_record` |
| backwards test is `<=` (equal instants rejected) | `a_wrong_declaration_never_removes_a_correct_one`, `every_fixture_produces_its_expected_result`, `refused_steps_and_probes_change_nothing` |
| first correct is the last correct | `every_fixture_produces_its_expected_result`, `scoring_agrees_with_the_reference` |
| a decoy can be missed | `a_silent_trajectory_misses_every_incident_and_every_critical_one_critically`, `declaring_every_incidents_truth_early_scores_every_incident_correct`, `every_fixture_produces_its_expected_result` |
| hard escalations counted unneeded | `every_fixture_produces_its_expected_result`, `scoring_agrees_with_the_reference`, `the_evaluators_reading_of_escalations_agrees_with_the_simulators_record` |
| silent decoy ignores wrong declarations | `every_fixture_produces_its_expected_result`, `scoring_agrees_with_the_reference` |
| refs disagreement not detected | `every_fixture_produces_its_expected_result` |
| focus disagreement not detected | `every_fixture_produces_its_expected_result` |
| ready_at disagreement not detected | `every_fixture_produces_its_expected_result` |
| at disagreement not detected | `every_fixture_produces_its_expected_result` |
| tokens read from `modelled_ns` | `every_fixture_produces_its_expected_result`, `scoring_agrees_with_the_reference`, `the_evaluators_reading_of_escalations_agrees_with_the_simulators_record` |
| refs counted as one per call | same three |
| plain incidents always counted escalated | `every_fixture_produces_its_expected_result`, `scoring_agrees_with_the_reference` |
| wrong declarations about plain incidents not summed | `declaring_a_wrong_site_for_every_incident_scores_wrong_and_alarms_every_decoy`, `every_fixture_produces_its_expected_result`, `scoring_agrees_with_the_reference` |
| decoy false alarms not added to the total | same three |
| background dismissal counted as a false alarm | `every_fixture_produces_its_expected_result`, `scoring_agrees_with_the_reference` |
| correct calls read from `informed` | `every_fixture_produces_its_expected_result`, `scoring_agrees_with_the_reference`, `the_evaluators_reading_of_escalations_agrees_with_the_simulators_record` |
| critical incidents counts every incident | `a_silent_trajectory_misses_every_incident_and_every_critical_one_critically`, `every_fixture_produces_its_expected_result`, `scoring_agrees_with_the_reference` |
| critical miss without criticality | `a_silent_trajectory_misses_every_incident_and_every_critical_one_critically`, `changing_the_kind_of_a_correct_declaration_makes_it_incorrect`, `changing_the_site_of_a_correct_declaration_makes_it_incorrect` |
| call index not checked | `every_fixture_produces_its_expected_result` |
| focus not checked for existence | `every_fixture_produces_its_expected_result` |
| anchor not checked for existence | `every_fixture_produces_its_expected_result` |
| time to first correct not relative to onset | `a_correct_declaration_in_time_is_correct`, `declaring_every_incidents_truth_early_scores_every_incident_correct`, `every_fixture_produces_its_expected_result` |
| critical decoy accepted | `every_fixture_produces_its_expected_result` |
| plain or hard incident without a deadline accepted | `every_fixture_produces_its_expected_result` |
| `Declare` accepts any outcome | `every_fixture_produces_its_expected_result` |
| `Escalate` accepts any outcome | `every_fixture_produces_its_expected_result` |
| `Probe` accepts any outcome | `every_fixture_produces_its_expected_result` |
| extra call summaries not detected | `every_fixture_produces_its_expected_result` |
| a refusal still checks the end | `every_fixture_produces_its_expected_result`, `refused_steps_and_probes_change_nothing` |
| repeat escalation counted once per incident | `every_fixture_produces_its_expected_result`, `scoring_agrees_with_the_reference`, `the_evaluators_reading_of_escalations_agrees_with_the_simulators_record` |
| informed total counts uninformed calls | same three |
| ids only checked for the first incident | `every_fixture_produces_its_expected_result` |

Many are caught only by the fixtures. That is the design: the fixtures are the specification of
each rule. The properties and the reference scorer catch the arithmetic mutants a second way.

## What this does not show

Mutation testing shows that the tests notice changes to the code that exists. It does not show
that the rules in `RULES.md` are the right rules, and the reference scorer in
`tests/properties.rs` was written from the same understanding as `score.rs`, so it catches
coding slips and not a shared misreading. The rules that are a judgement are listed in `RULES.md`
under "Where the rules are a judgement".
