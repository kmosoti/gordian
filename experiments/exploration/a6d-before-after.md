# A6d: the shared rule narrows each stored set once, before and after

Status: development run, `incremental-narrowing` branch. Nothing here tests a hypothesis and
nothing here may later be cited as confirmation. Plan item A6d (`docs/local-test-plan.md`), the
residue of A6c (`a6c-before-after.md` section 5). "Before" is the tree at `6c03235` (the rule as
merged by A6c, with the verdict fixture of this item added); "after" is the tree at `c40bddf` (the
cached narrowing and the refitted counted weights). Run directories are under `artifacts/runs/`
(git-ignored, not committed); the scripts are `scripts/a6c_table.py`, `scripts/a6d_fail_only.py`,
`scripts/a6d_explain.py` and `scripts/a6d_diag_patch.py`. Every deviation from the plan text is in
section 8.

## 1. What changed, and what the cache keys on

A6c left the rule narrowing the first stored hypothesis set against the bought probes, and scoring
the probes it could buy from it, at every call, and declaring both for every step from the set it
held. The narrowing is a function of the stored set, of the probe results and correction effects in
the working state (`Bought`) and of the world's public services. The rule now keeps its result with
the stored set (`src/policy/decide.rs`, `Held.narrowed`):

- **Per stored set.** Each of the verifier's, the estimator's and the heuristic's stored sets holds
  its narrowed view and, once a call has needed them, the scores of the probes of that view. A
  call looks at the sets in source order and narrows one only if it has no view made against the
  `Bought` of this call; it stops at the first view that has a world left, as before. A view that
  is empty is kept too.
- **The key is the stored set and the `Bought`.** The view lives inside `Held`, so replacing the
  stored set (an arriving output whose entries differ, byte for byte, from the held ones: the A6c
  test of "the same") drops the view with it; an arriving output equal to the held one keeps both.
  The `Bought` the view was made against is compared with the call's by `==` (the probe results and
  the corrections in window order, from the scan the rule already does each call). The services are
  not in the key: a rule plays one episode (the `Policy` contract) and its stored sets name that
  episode's services, so they cannot differ between its calls.
- **What is not cached.** Which probe is affordable (what is left to spend changes), and the scan of
  the window for bought probes (declared and counted inside the per-call unit, as before).
- **Decisions.** Narrowing and scoring are pure functions of the key, so a kept view is the view a
  recomputation would give. Tests check it against the rule as it was (section 3).
- **Declared cost.** The world and evaluation terms now carry the work of the previous call, as the
  decoding terms did: `declared = 45 + 0.95 window + 530 outputs_decoded + 115 hypotheses_decoded +
  0.033 bytes_compared + 10.5 worlds_narrowed + 36 probe_evaluations`, the last four from the
  previous call. A step that changed nothing it keeps is declared the base and the scan. The
  constants are those of A6; what they multiply changed from an upper bound on the set held to the
  work done (`POLICIES.md` sections 3 and 3.2). The final call's declared cost is the same formula.
- **Counted work.** `worlds` and `probe_evals` count the work done, so a call that kept its view
  counts none. Timed in a loop (the minimum over five runs, `calibrate_ops`, states recorded from
  episodes), a call that changes nothing it keeps costs a median 112 ns (10th to 90th percentile
  71 to 148), one whose probe results changed and so narrows and scores 749 ns (124 to 1,159), and
  one that also decodes a new output 4,519 ns (459 to 8,361).
- **References.** `Decider::without_cache` is the rule as it was after A6c, kept for tests;
  `Decider::without_reuse` is the rule as it was before A6c and now also narrows at every call.

## 2. Verdicts are unchanged

- `tests/incremental_narrowing.rs`, `verdicts_at_the_default_budget_are_those_the_rule_gave_before_incremental_narrowing`:
  the ten B1 arms (both oracles included) at the default 20 ms compute budget, 20 seeds by 11
  classes, through the recorder; every verdict column (success, critical miss, false alarm,
  abstained, undecided, probes used, corrections) compared per episode with a fixture committed
  before the rule changed (`6c03235`, written by this test with
  `GORDIAN_WRITE_A6D_VERDICT_FIXTURE=1`, untouched by the commit that changes the rule: the
  fixture is not in its diff). **All 2,200 rows are identical.** The fixture is also byte for byte the
  A6c fixture, as it should be, since A6c changed no verdict.
- The same comparison at B1's size: the B1 grid at 20 ms before and after (10 arms, 11 classes,
  seeds 1000 to 1499), every verdict column per episode: **55,000 rows, none differs.** (Cost and
  decision-time columns differ and were not compared.) At 20 ms no arm is near its limit, so a
  moved verdict would have meant the rule's decisions depended on how often it narrowed.
- The "before" rows of the grid below reproduce the "after" rows of `a6c-before-after.md` section 4
  for the same tree: all 40 (arm, budget) rows, success, critical miss and mean modelled cost,
  identical.

## 3. Step-level equivalence with the reference rule

`tests/incremental_narrowing.rs`,
`the_rule_decides_as_the_reference_that_narrows_at_every_call_and_does_less_work`: six arms
(`heuristic_only`, `all_components`, `random p=0.5`, the verifier alone every step, the estimator
alone every step, the heuristic alone every fourth) by 11 classes by four seeds at the default budget
and at 250,000 and 60,000 ns, so that arms run out of affordable work and the final call is made.
A mirror hands every call's inputs to `Decider::without_cache` and to a second copy of the rule
under test. Over **30,403 calls (448 bought a probe, 131 were final calls)**:

- `decide` and `decide_final` give the **same action at every call** as the reference;
- `calls`, `decoded_outputs`, `decoded_ranked` and `compared_bytes` are equal call by call;
- `worlds` and `probe_evals` are never higher than the reference's at any call, and in total
  **30,007 against 764,647 worlds (96% fewer) and 18,666 against 47,562 probe evaluations (61%
  fewer)**; the declared cost over the episodes' scheduling calls is 13.2 against 30.4 million ns
  (57% less; the reference declares upper bounds at every step, the rule what it did);
- the declared cost follows the work: at a step whose previous call decoded, compared, narrowed and
  scored nothing, the declared cost equals a fresh rule's (the base and the scan), at every other it
  is higher (14,139 steps of the first kind, 46,006 of the second; the harness also asks the cost when
  it checks affordability), and the cost features of a call are what its count says (outputs
  decoded, bytes compared, worlds narrowed plus visited when scoring, probe evaluations).

A smaller test (`a_stored_set_is_narrowed_and_scored_again_only_when_it_or_the_probes_change`) walks
the key by hand: the first call narrows and scores; the same call again, and the same output arriving
again, do neither; a probe result in the window narrows and scores once; a different output narrows
and scores once, and the set coming back after another is narrowed again.
`an_emptied_set_is_remembered_and_the_next_source_acts` shows an empty view is kept.

## 4. Counted-operation weights

`CALIBRATION.md` section 9.11. `calibrate_ops` times the rule on a third path (the held outputs and a
state alternating with one holding one more probe result), because a repeated call no longer narrows
and without it `worlds` and `probe_evals` would only ever occur beside decoding. Five hot runs and
three in-situ runs through `scripts/cgroup-run.sh --cpus 2 --cpu-quota 100`. R^2 of the weighted
counts against the minimum time per call: **rule 0.9960 (6,435 fit calls) and 0.9972 (4,692 held-out
calls)**; the A8b bar of 0.9 is met by the rule and by every component (lowest 0.976, memory
held-out; the components' code did not change and their fits are not applied). Weights, ps per unit
(A6c in brackets): calls 197,000 (246,000), decoded_outputs 477,000 (365,000), decoded_ranked
195,000 (158,000), worlds 24,000 (20,000), probe_evals 41,400 (33,200), compared_bytes 20 (45).
The in-situ scale is 1.78 times the hot weights plus 45 ns a call (A6c: 1.37 and 130 ns). Single
in-situ calls: R^2 0.886 fit, 0.886 held out, 0.921 on states of the check arms (A6c: 0.905, 0.911,
0.932), so single calls are under 0.9 for the rule where A6c's were over; that is the scatter of
single calls that 9.5 describes, and the bar is the hot fit's. The non-identical-arm check, repeated
(`heuristic_only` against `all_components`, 20 seeds by 11 classes, three run seeds): modelled ratio
9.68, identical in the three runs, against wall-time medians of 9.60 [9.21, 10.16], 9.59 [9.24, 10.01]
and 9.54 [9.13, 9.87]: inside in all three.

## 5. B1 grid, before and after

Ten arms, four compute limits, seeds 1000 to 1499, all 11 classes (5,500 episodes per row), through
`scripts/run-driver.sh` with the binary of each tree. Success and critical miss are pooled
percentages. Cost is the mean modelled cost `C` of the episode, with each tree's own weights; the
last two columns put the A6c weights back on the after tree (a scratch build, identical episodes and
counts; success and critical miss were checked equal) to separate doing less work from re-weighting.

| arm | compute limit (ns) | success before | success after | critical miss before | critical miss after | mean modelled cost before (ns) | mean modelled cost after (ns) | cost after / before | cost after, A6c weights (ns) | same / before |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `heuristic_only` | 20,000,000 | 95.4 | 95.4 | 0.0 | 0.0 | 29,636 | 25,702 | 0.867 | 27,034 | 0.912 |
| `heuristic_only` | 250,000 | 95.4 | 95.4 | 0.0 | 0.0 | 29,636 | 25,702 | 0.867 | 27,034 | 0.912 |
| `heuristic_only` | 100,000 | 95.0 | 95.0 | 0.0 | 0.0 | 29,622 | 25,688 | 0.867 | 27,022 | 0.912 |
| `heuristic_only` | 60,000 | 94.6 | 94.6 | 0.0 | 0.0 | 29,557 | 25,654 | 0.868 | 26,992 | 0.913 |
| `all_components` | 20,000,000 | 96.5 | 96.5 | 0.0 | 0.0 | 290,273 | 249,181 | 0.858 | 249,672 | 0.860 |
| `all_components` | 250,000 | 87.0 | 93.5 | 1.7 | 0.2 | 272,637 | 242,276 | 0.889 | 242,723 | 0.890 |
| `all_components` | 100,000 | 25.4 | 30.2 | 16.4 | 15.2 | 147,350 | 141,350 | 0.959 | 141,383 | 0.960 |
| `all_components` | 60,000 | 18.2 | 18.2 | 18.2 | 18.2 | 89,395 | 86,499 | 0.968 | 85,759 | 0.959 |
| `random_p025` | 20,000,000 | 96.2 | 96.2 | 0.1 | 0.1 | 122,906 | 83,171 | 0.677 | 82,617 | 0.672 |
| `random_p025` | 250,000 | 95.9 | 96.1 | 0.1 | 0.1 | 122,860 | 83,155 | 0.677 | 82,601 | 0.672 |
| `random_p025` | 100,000 | 86.1 | 94.8 | 2.1 | 0.1 | 116,576 | 82,748 | 0.710 | 82,194 | 0.705 |
| `random_p025` | 60,000 | 48.0 | 79.1 | 11.0 | 3.6 | 89,996 | 75,807 | 0.842 | 75,297 | 0.837 |
| `random_p050` | 20,000,000 | 96.5 | 96.5 | 0.0 | 0.0 | 177,814 | 137,493 | 0.773 | 137,223 | 0.772 |
| `random_p050` | 250,000 | 95.5 | 95.7 | 0.0 | 0.0 | 177,326 | 137,178 | 0.774 | 136,908 | 0.772 |
| `random_p050` | 100,000 | 59.3 | 77.7 | 8.2 | 3.8 | 138,812 | 122,303 | 0.881 | 122,033 | 0.879 |
| `random_p050` | 60,000 | 26.4 | 34.9 | 16.2 | 14.1 | 91,699 | 86,461 | 0.943 | 86,030 | 0.938 |
| `fixed_verifier_only` | 20,000,000 | 95.3 | 95.3 | 0.0 | 0.0 | 221,984 | 180,194 | 0.812 | 181,432 | 0.817 |
| `fixed_verifier_only` | 250,000 | 94.3 | 94.3 | 0.0 | 0.0 | 221,064 | 179,461 | 0.812 | 180,700 | 0.817 |
| `fixed_verifier_only` | 100,000 | 66.4 | 81.6 | 6.6 | 2.9 | 182,482 | 166,637 | 0.913 | 167,883 | 0.920 |
| `fixed_verifier_only` | 60,000 | 31.8 | 44.4 | 14.8 | 11.9 | 125,803 | 126,541 | 1.006 | 127,522 | 1.014 |
| `fixed_estimator_only` | 20,000,000 | 95.4 | 95.4 | 0.0 | 0.0 | 65,265 | 58,121 | 0.891 | 59,256 | 0.908 |
| `fixed_estimator_only` | 250,000 | 94.9 | 94.9 | 0.0 | 0.0 | 65,197 | 58,047 | 0.890 | 59,187 | 0.908 |
| `fixed_estimator_only` | 100,000 | 94.4 | 94.4 | 0.0 | 0.0 | 62,955 | 57,422 | 0.912 | 58,563 | 0.930 |
| `fixed_estimator_only` | 60,000 | 70.8 | 73.8 | 5.6 | 4.9 | 51,530 | 48,266 | 0.937 | 49,567 | 0.962 |
| `fixed_heuristic_every2` | 20,000,000 | 95.4 | 95.4 | 0.0 | 0.0 | 22,273 | 18,374 | 0.825 | 19,715 | 0.885 |
| `fixed_heuristic_every2` | 250,000 | 95.4 | 95.4 | 0.0 | 0.0 | 22,273 | 18,374 | 0.825 | 19,715 | 0.885 |
| `fixed_heuristic_every2` | 100,000 | 95.4 | 95.4 | 0.0 | 0.0 | 22,273 | 18,374 | 0.825 | 19,715 | 0.885 |
| `fixed_heuristic_every2` | 60,000 | 95.1 | 95.1 | 0.0 | 0.0 | 22,271 | 18,370 | 0.825 | 19,714 | 0.885 |
| `fixed_heuristic_every4` | 20,000,000 | 95.4 | 95.4 | 0.0 | 0.0 | 18,682 | 14,834 | 0.794 | 16,217 | 0.868 |
| `fixed_heuristic_every4` | 250,000 | 95.4 | 95.4 | 0.0 | 0.0 | 18,682 | 14,834 | 0.794 | 16,217 | 0.868 |
| `fixed_heuristic_every4` | 100,000 | 95.4 | 95.4 | 0.0 | 0.0 | 18,682 | 14,834 | 0.794 | 16,217 | 0.868 |
| `fixed_heuristic_every4` | 60,000 | 95.4 | 95.4 | 0.0 | 0.0 | 18,682 | 14,834 | 0.794 | 16,217 | 0.868 |
| `oracle_evidence_privileged` | 20,000,000 | 100.0 | 100.0 | 0.0 | 0.0 | 0 | 0 | - | 0 | - |
| `oracle_evidence_privileged` | 250,000 | 100.0 | 100.0 | 0.0 | 0.0 | 0 | 0 | - | 0 | - |
| `oracle_evidence_privileged` | 100,000 | 100.0 | 100.0 | 0.0 | 0.0 | 0 | 0 | - | 0 | - |
| `oracle_evidence_privileged` | 60,000 | 100.0 | 100.0 | 0.0 | 0.0 | 0 | 0 | - | 0 | - |
| `oracle_immediate_privileged` | 20,000,000 | 100.0 | 100.0 | 0.0 | 0.0 | 0 | 0 | - | 0 | - |
| `oracle_immediate_privileged` | 250,000 | 100.0 | 100.0 | 0.0 | 0.0 | 0 | 0 | - | 0 | - |
| `oracle_immediate_privileged` | 100,000 | 100.0 | 100.0 | 0.0 | 0.0 | 0 | 0 | - | 0 | - |
| `oracle_immediate_privileged` | 60,000 | 100.0 | 100.0 | 0.0 | 0.0 | 0 | 0 | - | 0 | - |

Reading it.

- At 20,000,000 ns no success moves, and mean modelled cost falls for every arm that runs a
  component. With the A6c weights put back (so it is less work and nothing else): `all_components`
  by 14.0%, `random p=0.25` by 32.8%, `random p=0.5` by 22.8%, `fixed_verifier_only` by 18.3%,
  `fixed_estimator_only` by 9.2%, `heuristic_only` by 8.8%, `fixed_heuristic_every2` by 11.5% and
  `every4` by 13.2%. With each tree's own weights the heuristic family falls a little more (its
  rule steps are mostly calls that change nothing, which the refit prices lower: `calls` 197 ns
  against 246).
- At the binding limits success rises again where the rule's narrowing and scoring were a large part
  of the bill: `random p=0.25` at 60,000 ns from 48.0 to 79.1 and at 100,000 from 86.1 to 94.8,
  `random p=0.5` at 100,000 from 59.3 to 77.7, `fixed_verifier_only` at 100,000 from 66.4 to 81.6 and
  at 60,000 from 31.8 to 44.4, `all_components` at 250,000 from 87.0 to 93.5; critical misses fall
  with it. `all_components` at 60,000 does not move (18.2 before and after): its four components
  declare about 3.9 µs a step (the ledger), so it can afford about 15 steps, a little over 0.7 s,
  whichever way the rule is billed. The oracles do not move (they declare nothing
  and count nothing).
- Mean cost at a binding budget is still not a clean measure: the declared bill per step is lower, so
  the budget lasts for more steps and the arm does more work before it runs out
  (`fixed_verifier_only` at 60,000: 125,803 to 126,541 while success rises 12.6 points). Compare arms
  at equal success or at the non-binding level when reading cost, as A6c said.
- The arms that never run the verifier move too (`fixed_estimator_only` at 60,000 from 70.8 to 73.8),
  because the estimator's changed outputs are narrowed once and not at every step.

## 6. The Fail-only comparison, before and after

The paired `ComponentTimeout` comparison of `finding4.py` and `a6c_finding4.py`, generalised
(`a6d_fail_only.py`): 500 episodes, seven arms, four budgets, same manifest, seed and class; only
which directives are applied differs. The binary is a build of a copy of the tree with the
harness hunk of `finding4-diagnostic.patch` (a switch for which directives apply) and nothing else,
and with the switch unset it reproduces the shipped binary on every deterministic column of all
14,000 rows (`a6d_fail_only.py control`). Success % with no directive, with only `Fail`
directives, and the episodes `Fail` alone raised / lowered. "Before" is the rule of A6c (it
reproduces A6c's section 5 table, 102 raised), "after" the rule of this item.

| arm | limit | before: none, fail, raised/lowered | after: none, fail, raised/lowered |
|---|---|---|---|
| `all_components` | 250,000 (B3 cell) | 90.6, 91.6, 6/1 | 99.4, 99.2, 0/1 |
| `random_p050` | 250,000 | 100.0, 99.8, 0/1 | 100.0, 99.8, 0/1 |
| `random_p025` | 100,000 | 88.8, 90.8, 11/1 | 99.6, 99.4, 0/1 |
| `random_p050` | 100,000 (B3 cell) | 54.6, 59.6, 25/0 | 80.0, 81.6, 8/0 |
| `all_components` | 100,000 | 9.8, 11.6, 9/0 | 16.0, 17.4, 7/0 |
| `random_p025` | 60,000 (B3 cell) | 40.2, 48.0, 39/0 | 80.4, 82.8, 13/1 |
| `random_p050` | 60,000 | 11.2, 13.6, 12/0 | 22.6, 23.8, 7/1 |

Before: over these seven cells `Fail` alone raised 102 episodes and lowered 3

After: over these seven cells `Fail` alone raised 35 episodes and lowered 5

Over the seven cells `Fail` alone raised 102 episodes before and raises 35 now. The cell the B3 note
started from (`all_components` at 250,000) has no raised episode left, and `random p=0.25` at 100,000
none. 25 and 39 episodes of 500 have become 8 and 13. What is left is explained in the next section;
it is not repeated work, and it is not removed by anything the cache can do.

## 7. The 35: what they are

**Reproducing A6c's 35.** A6c cut the 102 to 35 by building a scratch tree that declares the
narrowing term at zero. `a6d_diag_patch.py --tree=old` does that to a copy of the A6c tree
(`WORLD_PS` is not touched; a switch zeroes the term where the cost is built). The unmodified build
reproduces the 102 (6, 0, 11, 25, 9, 39 and 12 in A6c's seven cells) and the switch gives **35**:
8 `all_components` at 100,000, 8 `random p=0.5` at 100,000, 13 `random p=0.25` at 60,000 and 6
`random p=0.5` at 60,000. Of those 35, 31 are also among the 35 that remain after this item (the
sets differ in four episodes each way: seeds 1002, 1373, 1473 and 1483 left, 1021, 1260, 1374 and
1489 came), so the cache did what zeroing the term did, and the 35 are the same phenomenon.

**What the 35 are not.** They are not the verifier's stale set keeping priority, or not that alone.
They are also not repeated work: after the cache, the rule narrows, scores and decodes only what
changed, and the episodes are still there.

**Which channels of `Fail` they need.** A `Fail` directive has two effects on the rule: the rule is
not billed for reading the failed component's output (it decodes, compares and narrows nothing from
it), and its set is not among the stored sets, so it cannot win the source order. To say which of
the two raised an episode, the scratch build of the new rule (`a6d_diag_patch.py --tree=new`) reads
which components the episode's directives fail (without applying them) and imitates one effect at
a time in a run with no directive: `cost` leaves those components' outputs in use but does not
declare the rule's work on them; `priority` decides as if their sets were absent and bills as usual;
`both` does both. For each of the 35:

| channel imitated alone, run with no directive | episodes it gets right |
|---|---:|
| cost (the rule's work on the failed component's output is not billed) | 31 |
| priority (their stored set is not read; every charge unchanged) | 6 (all six also cost-sufficient) |
| neither alone, both together | 4 |
| both | 35 |

So **31 of 35 need nothing but the bill**, 6 of those would also be right if only the stale set were
ignored, and 4 need both. `both` reproduces the `Fail` outcome in all 35, so nothing else about a
`Fail` (the verifier's requests, the working state) is needed. The same experiment on A6c's zero-cost
variant gives 31 cost-sufficient, 15 priority-sufficient (all of them also cost-sufficient), 4 only
together, and one where imitating both is wrong while the cost channel alone is right (34 of 35 for
both).

**What the bill is, from the ledger.** Every one of the 35 was traced in the run with no directive
and the run with only `Fail` (`trace_sample_rate` 1; `a6d_explain.py`). In 30 the stop reason is
`final_declaration` without the directive and `terminal` with it, in 4 `final_declaration` in both
(the final call declares different things), in one `terminal` in both. In the 34 where the no-directive
run's last step was refused for want of compute, **the refused step was short of the budget by a
median of 37 ns (24 of 34 by under 100 ns, the largest 4,677 ns)**, and the rule had charged that run
a median of 8,133 ns more than the `Fail` run. That extra is the work the rule did on the failed
component's output, in the units the ledger counts: over the 35, 1,402 more listed hypotheses decoded,
about 1.12 million more payload bytes compared and 1,719 more worlds narrowed, which at the declared
constants is about 80% decoding (7% per output, 73% per hypothesis), 17% comparing and 8% narrowing,
less 5% for scoring the `Fail` run did and the other did not (its estimator's set became probeable).
In a typical episode (seed 1094, `random p=0.25`, 60,000 ns, `Fail` on the verifier) the two runs
are the same until the verifier first runs at 0.30 s. The no-directive run's next step is charged
7,848 ns: the verifier's silent-window set of 56 hypotheses decoded once (530 + 115 x 56 = 6,970 ns)
and narrowed once (78 worlds, 0.8 µs); eight later arrivals of the same 2.9 KB output cost about
0.1 µs each to compare. Over the episode the rule charged it 13,142 ns and the `Fail` run 4,547. At
2.55 s, the first step whose window lets the estimator name the fault (Intermittent at service 0),
the `Fail` run could pay the estimator (1,645 ns) and the verifier (2,229 ns), and declared it. The no-directive run had 32 ns of compute left, could pay neither, and was
refused the next step's 62 ns; its final call then declared "no fault" from the verifier's stored set
(last refreshed at 2.0 s, before the signature), which comes first in the source order.

**Why that is neither of the candidates.** The decode is of an output that changed (the first
verifier output, and each time its consistent set shrinks), the comparison is the price of
recognising the ones that did not (A6c), the narrowing is of a set that is new. All of it is work the
rule does and counts; none of it repeats. The rule pays for what it reads (`POLICIES.md` 2.1), and a
`Fail` charges the component but discards its output, so the rule is spared the bill for reading it.
At a budget of 60,000 to 100,000 ns, with the verifier's first output costing 5 to 8 µs of that,
an arm that reads it ends up a few nanoseconds short of the step that carries the decisive evidence
in a few episodes (7, 8, 13 and 7 of 500 in the four cells where it occurs), and what the arm does
at the final call depends on which stored set comes first (`POLICIES.md` 2): stale priority is the
second channel, needed in ten of the 35 (alone sufficient in six, needed together with the bill in
four).

**What I did.** Nothing to the rule. The explanation is a property of the cost model (what reading an
output costs) and of the documented source order, not a second artefact of repeated work, so the
instruction was to document it and leave the rule alone: `POLICIES.md` section 3.3. Changing the
source order would change decisions and would not remove the 31 episodes that need only the bill.
Removing them would need a different charge for reading an output (for example not decoding a set
that cannot be the source, or pricing a repeat below the comparison), which is a change to the
instrument and the coordinator's to make. The knife-edge is the finding: these are the episodes in
which the arm's budget ends within tens of nanoseconds of the decisive evidence, so any charge on any
path tips them, and a policy that reads fewer outputs gains them.

The 35, one row each (A6d's rule; "alone sufficient" is the imitated channel that gets the episode
right with no directive; "short by" is how far the no-directive run's refused step was beyond what was
left; the last columns are what the no-directive run's rule did that the `Fail` run's did not, over the
steps up to the refusal):

| arm | limit | seed | failed | stop without -> with | channels alone sufficient | short by (ns) | rule charged more (ns) | decoded listed / compared bytes / worlds narrowed, more |
|---|---:|---:|---|---|---|---:|---:|---|
| `all_components` | 100,000 | 1173 | mem, ver | final_declaration -> terminal | only together | 35 | 8,133 | +40 / +49,463 / +56 |
| `all_components` | 100,000 | 1175 | ver | final_declaration -> terminal | mcost | 2722 | 7,321 | +40 / +46,794 / +57 |
| `all_components` | 100,000 | 1235 | ver | final_declaration -> final_declaration | mcost | 12 | 8,817 | +40 / +52,380 / +38 |
| `all_components` | 100,000 | 1245 | ver | final_declaration -> terminal | mcost | 52 | 6,100 | +25 / +45,780 / +36 |
| `all_components` | 100,000 | 1246 | est, ver | final_declaration -> terminal | only together | 35 | 7,656 | +35 / +54,365 / +40 |
| `all_components` | 100,000 | 1279 | ver | final_declaration -> terminal | mcost | 41 | 9,009 | +45 / +52,010 / +63 |
| `all_components` | 100,000 | 1356 | est | final_declaration -> final_declaration | mcost | 1397 | 1,436 | +7 / +9,990 / -15 |
| `random_p050` | 100,000 | 1136 | ver | final_declaration -> terminal | mcost | 77 | 7,801 | +36 / +51,048 / +49 |
| `random_p050` | 100,000 | 1162 | ver | final_declaration -> terminal | mcost | 13 | 10,709 | +56 / +46,815 / +78 |
| `random_p050` | 100,000 | 1205 | ver | final_declaration -> terminal | mcost | 9 | 6,694 | +40 / +47,142 / +60 |
| `random_p050` | 100,000 | 1213 | est, ver | final_declaration -> terminal | mcost | 4677 | 9,669 | +60 / +62,872 / +68 |
| `random_p050` | 100,000 | 1316 | ver | final_declaration -> terminal | mcost, mprio | 32 | 11,625 | +62 / +56,322 / +78 |
| `random_p050` | 100,000 | 1324 | ver | final_declaration -> final_declaration | only together | 42 | 9,620 | +51 / +50,550 / +57 |
| `random_p050` | 100,000 | 1334 | ver | final_declaration -> terminal | mcost, mprio | 33 | 9,474 | +61 / +56,178 / +70 |
| `random_p050` | 100,000 | 1434 | ver | final_declaration -> terminal | mcost | 3 | 8,095 | +36 / +42,540 / +49 |
| `random_p025` | 60,000 | 1004 | est | final_declaration -> terminal | mcost | 445 | 2,405 | +10 / +3,700 / +5 |
| `random_p025` | 60,000 | 1021 | heur, est | final_declaration -> terminal | mcost | 18 | 3,511 | +11 / +4,383 / +5 |
| `random_p025` | 60,000 | 1036 | ver | final_declaration -> terminal | mcost, mprio | 47 | 11,242 | +66 / +26,132 / +78 |
| `random_p025` | 60,000 | 1094 | ver | final_declaration -> terminal | mcost | 30 | 8,595 | +51 / +22,450 / +76 |
| `random_p025` | 60,000 | 1162 | ver | final_declaration -> terminal | mcost | 3 | 10,394 | +57 / +34,331 / +78 |
| `random_p025` | 60,000 | 1250 | heur, ver | final_declaration -> terminal | mcost | 37 | 10,735 | +58 / +32,330 / +78 |
| `random_p025` | 60,000 | 1253 | heur | terminal -> terminal | mcost | - | 1,329 | +1 / +1,134 / -1 |
| `random_p025` | 60,000 | 1260 | ver | final_declaration -> terminal | mcost, mprio | 15 | 9,367 | +56 / +12,546 / +77 |
| `random_p025` | 60,000 | 1316 | ver | final_declaration -> terminal | mcost | 105 | 9,096 | +61 / +24,968 / +79 |
| `random_p025` | 60,000 | 1324 | ver | final_declaration -> terminal | only together | 32 | 8,580 | +56 / +25,830 / +72 |
| `random_p025` | 60,000 | 1434 | ver | final_declaration -> terminal | mcost | 8 | 6,956 | +36 / +25,524 / +49 |
| `random_p025` | 60,000 | 1436 | est, ver | final_declaration -> terminal | mcost | 10 | 10,111 | +61 / +37,030 / +77 |
| `random_p025` | 60,000 | 1474 | est | final_declaration -> terminal | mcost | 61 | 2,011 | +9 / +4,597 / +3 |
| `random_p050` | 60,000 | 1173 | mem, ver | final_declaration -> terminal | mcost, mprio | 25 | 9,590 | +52 / +26,391 / +62 |
| `random_p050` | 60,000 | 1217 | heur, ver | final_declaration -> terminal | mcost | 465 | 9,523 | +53 / +27,510 / +77 |
| `random_p050` | 60,000 | 1235 | ver | final_declaration -> final_declaration | mcost | 476 | 5,529 | +47 / +31,428 / +31 |
| `random_p050` | 60,000 | 1374 | heur | final_declaration -> terminal | mcost | 26 | 682 | +1 / +1,120 / -1 |
| `random_p050` | 60,000 | 1445 | ver | final_declaration -> terminal | mcost | 442 | 7,356 | +41 / +28,106 / +57 |
| `random_p050` | 60,000 | 1472 | est | final_declaration -> terminal | mcost | 393 | 1,327 | +9 / +6,660 / -2 |
| `random_p050` | 60,000 | 1489 | ver | final_declaration -> terminal | mcost, mprio | 677 | 5,888 | +32 / +21,255 / +35 |

## 8. Deviations from the plan text, and what I am least sure of

Deviations (each with its reason):

1. **The cache also holds the probe scores.** The plan says the narrowed view is cached. Scoring is a
   function of the same two inputs and was the other repeated term A6c named ("the same
   probe-scoring term when a set can be probed"), so it is kept with the view and cleared with it.
   Without it a set that can be probed would still be scored at every step.
2. **The declared world and evaluation terms became lagged work, not upper bounds.** "Declared cost
   follows the work actually done" needed a choice. The terms cannot be exact when asked (which
   outputs and probe results a step brings is unknown before `select`), and a forward term for a set
   that did not change would be the repeated charge itself. So they carry the previous call's work,
   as the decoding terms already did, and the final call's declared cost is the same formula (it no
   longer omits an evaluation term). Consequence: the work of a call that narrows or scores is billed
   one step after it is done, and the last such call's is counted and not billed when the final call
   is refused (it is made anyway, `HARNESS.md` 1). The hard limit is therefore overshot by at most one
   call's work, once per episode, at the largest about 8 µs (a first verifier output of 56
   hypotheses, decoded and narrowed). The old forward form survives in
   `Decider::without_cache`.
3. **Three existing tests changed**, each for a stated reason: the A6c step test now allows the arm
   to count fewer worlds and probe evaluations than its pre-A6c reference (equality would be false
   by design); the tiny-budget final-call test also runs a 10,000 ns budget, because at 60,000 the
   cheapest arm, `heuristic_only`, no longer runs out of means in any of its 55 episodes there (it
   still never ends undecided at either budget); the ignored measurement test prints the work the
   fresh call did instead of the forward features. The B1 grid helpers moved from `decode_once.rs`
   into `tests/common` in the fixture commit, before the rule changed.
4. **The services are not in the cache key** (section 1), a contract stated in the code, not enforced.
5. **A third timing path in `calibrate_ops`** (section 4).
6. **The 35 are documented, not fixed**, and are not what the plan guessed (section 7).
7. **Not done:** the components' weights and the declared cost's constants were not refitted (their
   code and the work per unit are unchanged); the measurement test behind the declared constants was
   not re-run, so declared over measured is not re-measured for this change (A6c's figure was a
   median 1.14).

What I am least sure of:

- The channel experiment is a counterfactual on a scratch build, per episode, with the episode's own
  failed components. It shows what each channel is worth, not that nothing else differs; `both`
  matches the `Fail` outcome in all 35 episodes, which is the check. The old-rule version (verifier
  only) was not exact for episodes that fail other components and gave one episode where imitating
  both is wrong while the cost channel alone is right.
- That "the rule pays for what it reads" is what the instrument should do. A selector that skips the
  verifier until a symptom arrives gains these episodes; whether that is a property of a good
  selector or an artefact of the declared constants is for the coordinator. The 31 depend on the
  declared constants (530 ns an output, 115 a hypothesis, fitted on another host and not refitted).
- The refit moved the weights by 20 to 31% (ns per unit: decoded_outputs 365 to 477, decoded_ranked
  158 to 195, worlds 20 to 24, probe_evals 33 to 41, calls 246 to 197), mostly through the in-situ
  scale (1.78, A6c 1.37). A repeated call, which is now a call of about 110 ns, has a median relative error
  of 21% in the hot fit (about 20 ns) and its in-situ price rests on a 45 ns constant per call. The
  check on two arms passes, with intervals 8 to 10% wide, which cannot tell a 5% error from none.
- One generator, seeds 1000 to 1499, one run of each condition; the 20 ms identity is exact, the
  binding-budget results are single runs. The scratch variants are builds of copies, not in the tree.
- Calibration ran on a machine where another job held core 3 throughout; the runner pinned the
  arms to core 2, and the minimum over runs is robust to bursts, but the host is shared.

## 9. Reproduction

```bash
# before: the tree at 6c03235; after: the tree at c40bddf. Each run needs a clean tree whose HEAD
# the manifests name, the release binary built, and no build running.
export GORDIAN_WORK=/tmp/gordian-exploration
python3 experiments/exploration/scripts/make_manifests.py            # at the commit being run
for c in 20000000 250000 100000 60000; do
  scripts/run-driver.sh --manifest $GORDIAN_WORK/manifests/c$c.json \
    --out artifacts/runs/a6d-after-b1-c$c-s1000-1499 --bin target/release/gordian-run
done      # and a6d-before-... with the binary built at 6c03235, a6d-oldw-... with the A6c rule weights put back
python3 experiments/exploration/scripts/a6c_table.py a6d-before a6d-after a6d-oldw --oldw-label "A6c weights"

# the paired runs and the channels, on a COPY of each tree with the switches (a6d_diag_patch.py)
mkdir $GORDIAN_WORK/diag-new && cp -r crates Cargo.toml Cargo.lock rust-toolchain.toml .cargo $GORDIAN_WORK/diag-new/
python3 experiments/exploration/scripts/a6d_diag_patch.py $GORDIAN_WORK/diag-new --tree=new   # --tree=old on a copy of 6c03235
(cd $GORDIAN_WORK/diag-new && cargo build --locked --release -p gordian-run &&
   cargo run --locked --release -q -p gordian-world --example directives > $GORDIAN_WORK/directives.tsv)
S=experiments/exploration/scripts/a6d_fail_only.py; B=$GORDIAN_WORK/diag-new/target/release/gordian-run
python3 $S grid new-all $B                                                      # control: no switch set
python3 $S control after new-all                                                # 14,000 rows, 0 differ
python3 $S grid new-none $B GORDIAN_DIAG_DIRECTIVES=none
python3 $S grid new-fail $B GORDIAN_DIAG_DIRECTIVES=fail
python3 $S grid new-mcost $B GORDIAN_DIAG_DIRECTIVES=none GORDIAN_DIAG_MIRROR=cost
python3 $S grid new-mprio $B GORDIAN_DIAG_DIRECTIVES=none GORDIAN_DIAG_MIRROR=priority
python3 $S grid new-mboth $B GORDIAN_DIAG_DIRECTIVES=none GORDIAN_DIAG_MIRROR=both
python3 $S trace new-none $B 60000 random_p025 1004,1021 GORDIAN_DIAG_DIRECTIVES=none   # and new-fail, per cell
# the A6c rule: --tree=old, tags old-none, old-fail; with GORDIAN_DIAG_WORLD_FREE=1 the zero-narrowing
# variant (oldw-*), and oldw-vun, oldw-vfree, oldw-vboth for GORDIAN_DIAG_VERIFIER_UNUSED / _FREE / both
python3 $S compare old new
python3 experiments/exploration/scripts/a6d_explain.py new --channels mcost,mprio,mboth --traces new --md
python3 experiments/exploration/scripts/a6d_explain.py oldw --channels vun,vfree,vboth --traces oldw
# calibration: CALIBRATION.md 9.9 and 9.11
```
