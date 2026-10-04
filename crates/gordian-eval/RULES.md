# Evaluator rules

What `gordian_eval::score(truth, trajectory)` does, one numbered rule per row. The code cites
these ids (`src/score.rs`, `src/truth.rs`); every fixture in `fixtures/tiny-cases.json` names the
rules it pins in its `rules` field; `tests/fixtures.rs` fails if a rule here is pinned by no
fixture or a fixture names a rule that is not here.

`score` is a pure function: same `Truth` and same steps give the same `Result`. It is tested as
a property, not as a rule row.

## Inputs

- `Truth { class, faults }`. `class == NoFault` exactly when `faults` is empty (R18).
  `Truth::from_episode` builds one from `oracle::reveal(&Episode)` and `episode.spec().class`;
  fixtures build one by hand.
- `Step { at, action, outcome }`, the world's `Action` and `Outcome` unchanged. The outcome says
  what happened; the action says what was tried.

## Rules

| Id | Rule |
|---|---|
| R1 | **Terminal action.** The first step whose outcome is `Declared` or `Abstained`. A refused `Declare` or `Abstain` is not terminal and leaves the episode open. |
| R2 | **Success, faulted class.** The terminal action is `Declare { fault: Some((kind, site)) }` and some true fault has that kind and that site. |
| R3 | **Kind and site both.** The right site with a wrong kind, or the right kind at a wrong site, is not success. |
| R4 | **Declaring no fault on a faulted class** is not success. It is not a false alarm (R6 needs class `NoFault`). |
| R5 | **Success, `NoFault`.** The terminal action is `Abstain` or `Declare { fault: None }`. Success depends on the terminal action alone: a carried-out `Correct` earlier does not change it (it sets R6). |
| R6 | **False alarm.** Class is `NoFault` and either the terminal action is `Declare { fault: Some(_) }` or at least one `Correct` was carried out (R11). Never true for a faulted class. |
| R7 | **Critical miss.** Some true fault has `critical = true` and `success` is false. This includes abstaining and being undecided. A wrong `Correct` earlier is not a miss if the terminal declaration is right. |
| R8 | **Abstained.** The terminal action is `Abstain`. `Declare { fault: None }` on `NoFault` is an answer, not an abstention. |
| R9 | **Undecided.** There is no terminal action: the arm ran out of budget, reached the horizon, or stopped. `success` and `abstained` are false. A probe or correction is not a decision. |
| R10 | **`probes_used`.** The number of steps with action `Probe` and outcome `Probed`, counted as actions. An `Inconclusive` result counts. It is not the sum of probe resource units (a `LatencySample` costs 2 units and counts 1). |
| R11 | **`corrections`.** The number of steps with action `Correct` and outcome `Corrected`. |
| R12 | **Refusals count for nothing.** A step whose outcome is `Refused(_)` is not a probe (R10), a correction (R11), or a terminal action (R1), whatever the reason. |
| R13 | **`decision_at`.** The `at` of the terminal step; `None` when undecided. Not `ready_at`. |
| R14 | **Error: step after terminal.** Any step after the terminal action, refused or not, makes `score` return `ActionAfterTerminal` at the first such index. |
| R15 | **Error: time goes backwards.** A step whose `at` is strictly earlier than the previous step's makes `score` return `TimeWentBackwards`. Equal instants are allowed. |
| R16 | **Error: outcome mismatch.** `Probe` may be answered only by `Probed` or a refusal, `Correct` by `Corrected` or a refusal, `Declare` by `Declared` or a refusal, `Abstain` by `Abstained` or a refusal. Anything else is `OutcomeMismatch`. |
| R17 | **Error: `EpisodeOver` without a close.** A step refused with `Refused(EpisodeOver)` when no terminal step came before it is `EpisodeOverWithoutTerminal`. |
| R18 | **Error: invalid truth.** `NoFault` with faults, or any other class without a fault, is `InvalidTruth`. Checked before any step. |
| R19 | **More than one true fault.** Success needs one true fault matching in kind and site together (R2, R3). Critical miss needs any critical fault (R7). The generator makes at most one fault; the rules do not depend on that. |

Order of checks for each step: after a terminal action (R14), then time going backwards (R15),
then the outcome (R16, R17). The first failing check on the first offending step is returned. An
error replaces the verdict: there is no partial result.

## How the world's types were read

- `Outcome::Probed` and `Outcome::Corrected` mean the world charged the action and carried it
  out. `Outcome::Declared` and `Outcome::Abstained` mean it closed the episode. `Outcome::Refused`
  means nothing was charged and nothing happened, except that `Refusal::EpisodeOver` can only
  follow a close (`Simulator::apply`).
- The world never reports whether a `Declare` was right or whether a correction resolved a fault
  to the policy. `Observation::Correction { resolved }` is in the trajectory, but scoring does not
  read it: R6 and R11 count corrections that were *carried out*, not corrections that *worked*.
- `Refusal::TimeWentBackwards` is judged by the simulator against every time it has seen,
  including `observe_until` calls, which the trajectory does not contain. A trajectory can hold
  that refusal at a non-decreasing `at`; that step is an ordinary refusal (R12). A trajectory
  whose recorded `at` does go backwards is rejected (R15) whether or not the world refused it.

## Deviations from `docs/local-test-plan.md` section 5, A2

| Plan | Here | Why |
|---|---|---|
| `score(episode, trajectory)` | `score(&Truth, &[Step])` | Fixtures must state truth and trajectory directly. If the evaluator took an `Episode`, every fixture would have to call the generator, and a generator bug could make both sides agree. `Truth::from_episode` is the only call into the oracle. |
| `trajectory: &[(Instant, Action, Outcome)]` | `&[Step]` with named fields | Same content, readable fixtures. |
| `score` returns `Verdict` | returns `Result<Verdict, EvalError>` | A trajectory with a step after the close, time going backwards, an impossible outcome, or an invalid truth is a harness bug. A verdict would hide it in a results table. |
| "A trajectory that exceeds budget is scored as the last action before exhaustion." | R9: no terminal action means `undecided`, `success = false` | A probe is not a decision. Scoring the last action would mark "ran out of budget while probing" as a decision the arm never made. The world refuses over-budget actions (`BudgetExceeded`) rather than letting a trajectory exceed budget, so what remains is a trajectory with no close. |
| `Verdict.decision_at: Instant` | `Option<Instant>` | There is none when undecided. |
| `Verdict` has no counters for corrections or undecided | adds `corrections: u32`, `undecided: bool` | Requested for the false-alarm and coverage accounting downstream. |

## Where the rules are a judgement

1. **R5 and R6 can both hold.** An arm that carries out a `Correct` on a `NoFault` episode and
   then abstains has `success = true` and `false_alarm = true`. The plan defines success by the
   terminal action and false alarm separately, and this follows it. A consumer that wants "right
   and quiet" must read both fields; the results table keeps both.
2. **A refused `Correct` is not a false alarm.** "Attempted a `Correct`" is read as *carried out*,
   consistent with R12 and with the world's "nothing happened". A reading that counted refused
   attempts would punish an arm for a budget refusal it could not avoid. Fixture
   `no-fault/refused-correction-then-abstain` pins this reading; changing it is a rule change.
3. **`probes_used` counts actions, not resource units** (R10). The run recorder's `bill_probes`
   carries units.
4. **A refusal after the close is an error (R14).** The simulator answers such a step with
   `EpisodeOver`, so a harness that records every attempt a policy makes after the close would
   have every such trajectory rejected. The specification says to reject; a harness that wants to
   log them must not put them in the trajectory it scores.
5. **R16 and R17 are additions.** The specification names R14 and R15 only. R16 and R17 reject
   trajectories the world could not have produced; R18 rejects a truth that contradicts its own
   class. Without them an inconsistent record would be scored by whichever of action and outcome
   the code happened to read.
