# A6c: the shared rule decodes each output once, before and after

Status: development run, `decode-once` branch. Nothing here tests a hypothesis and nothing here may
later be cited as confirmation. Plan item A6c (`docs/local-test-plan.md`), diagnosis in
`b3-finding4.md`. "Before" is the tree at `da73030` (the code of the Stage B runs); "after" is the
tree at the commit that adds this file. Run directories are under `artifacts/runs/` (git-ignored,
not committed); the scripts are `scripts/a6c_table.py` and `scripts/a6c_finding4.py`.

## 1. The premise of A6c was wrong in the code; what changed instead

A6c as written says the rule "decodes, and is charged for decoding, every stored output on every
step, whether or not it changed", and asks that it decode "when it arrives (the component ran this
step)". The code at `da73030` already did that. The harness gives `decide` only the outputs of
components that ran and did not fail at this step; the rule keeps decoded sets between steps; a call
with no outputs counted no decoding (`tests/counted_ops.rs`, `taking_the_count_resets_it`). So a
stored output from an earlier step was never decoded, charged or counted again, and implementing
A6c literally would have changed nothing.

What the rule did repeat was decoding an output that a component *produced again*: an arm that runs
the verifier at every step gets, on a window that has not changed, the same bytes back, and the
rule decoded and was charged for them every time (46 hypotheses on a symptom-free window: 5,820 of
about 7,600 declared ns a step). That is the mechanism `b3-finding4.md` traced, and "arrives" has
to mean "arrives and differs from what is held" to remove it. That is what I implemented, and it is
a **deviation from the specification's definition of new**: the rule keeps the entries it decoded
from, and an arriving output whose entries are byte for byte equal to them keeps the held decoded
form (`POLICIES.md`, section 3.1). Alternatives I considered and did not take: having the harness
say whether the window changed (it would leak harness knowledge into the rule's inputs and assume
components are pure functions of the window), and hashing outputs (costs as much as comparing).
Neither the harness nor the `Policy` trait changed.

## 2. Verdicts are unchanged

- `tests/decode_once.rs`, `verdicts_at_the_default_budget_are_those_the_rule_gave_before_decode_once`:
  the ten B1 arms (both oracles included) at the default 20 ms compute budget, 20 seeds by 11 classes,
  through the recorder; every verdict column (success, critical miss, false alarm, abstained,
  undecided, probes used, corrections) compared per episode with a record written by the same test at
  the code of `da73030`. **All 2,200 rows are identical.** Cost and decision-time columns were not
  compared. At 20 ms the budget does not bind for any arm, so a changed verdict would have meant the
  rule's decisions depended on how often it decoded.
- The step-level check is stronger and independent of the budget: `Decider::without_reuse` (the old
  rule, kept as the reference) is fed the same inputs as the arm at every call, over four arms by
  11 classes by four seeds at the default budget and at 250,000 and 60,000 ns (18,619 calls, including
  final calls): the actions are identical at every call; the reference decoded 25,990 outputs and
  410,044 candidates, the rule 2,007 and 20,029; declared cost over those calls fell from 137.8 to
  24.8 million ns; calls, worlds and probe evaluations are equal call by call.

## 3. Counted-operation weights

See `CALIBRATION.md` section 9.10. The rule's units were re-measured on both paths a call can take
(the repeated call reaches only the comparison path, so `calibrate_ops` also times an alternating
call that decodes), five hot runs and three in-situ runs through `scripts/cgroup-run.sh --cpus 2
--cpu-quota 100`. R^2 against the minimum time per call: rule 0.9954 (4,282 fit calls) and 0.9966
(3,124 held-out calls); every component also still above 0.98. The rule has a new unit,
`compared_bytes` (0.0325 ns per byte hot, 45 ps shipped, 1% of the fitted time), and its decoding
weights moved (decoded_outputs 223 to 266 ns hot, decoded_ranked 110 to 115). Only the rule's table
was rewritten. The A8b non-identical-arm check, repeated: modelled ratio 9.77 for `all_components`
over `heuristic_only` against wall-time medians of 9.73 [9.28, 10.38], 9.54 [9.22, 10.02] and 10.04
[9.53, 10.41] in three runs; inside in all three.

## 4. B1 grid, before and after

Ten arms, four compute limits, seeds 1000 to 1499, all 11 classes (5,500 episodes per row), through
`scripts/run-driver.sh` from the committed manifests' generator. The "before" rows reproduce
`b3-finding4.md` section 7's "rule as committed" column. Success and critical miss are pooled
percentages. Cost is the mean modelled cost `C` of the episode, with each tree's own weights; the
last two columns put the pre-A6c weights back on the after tree (a scratch build, identical episodes
and counts) to separate doing less work from re-weighting.

| arm | compute limit (ns) | success before | success after | critical miss before | critical miss after | mean modelled cost before (ns) | mean modelled cost after (ns) | cost after / before | cost after, A8b weights (ns) | same / before |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `heuristic_only` | 20,000,000 | 95.4 | 95.4 | 0.0 | 0.0 | 42,637 | 29,636 | 0.695 | 30,425 | 0.714 |
| `heuristic_only` | 250,000 | 95.4 | 95.4 | 0.0 | 0.0 | 42,637 | 29,636 | 0.695 | 30,425 | 0.714 |
| `heuristic_only` | 100,000 | 94.8 | 95.0 | 0.0 | 0.0 | 42,571 | 29,622 | 0.696 | 30,413 | 0.714 |
| `heuristic_only` | 60,000 | 94.4 | 94.6 | 0.0 | 0.0 | 39,755 | 29,557 | 0.743 | 30,352 | 0.763 |
| `all_components` | 20,000,000 | 96.5 | 96.5 | 0.0 | 0.0 | 521,984 | 290,273 | 0.556 | 282,509 | 0.541 |
| `all_components` | 250,000 | 29.1 | 87.0 | 15.7 | 1.7 | 285,922 | 272,637 | 0.954 | 265,222 | 0.928 |
| `all_components` | 100,000 | 18.2 | 25.4 | 18.2 | 16.4 | 121,279 | 147,350 | 1.215 | 142,914 | 1.178 |
| `all_components` | 60,000 | 16.2 | 18.2 | 18.2 | 18.2 | 76,554 | 89,395 | 1.168 | 86,269 | 1.127 |
| `random_p025` | 20,000,000 | 96.2 | 96.2 | 0.1 | 0.1 | 174,495 | 122,906 | 0.704 | 118,744 | 0.681 |
| `random_p025` | 250,000 | 95.0 | 95.9 | 0.2 | 0.1 | 173,175 | 122,860 | 0.709 | 118,700 | 0.685 |
| `random_p025` | 100,000 | 46.0 | 86.1 | 11.7 | 2.1 | 120,703 | 116,576 | 0.966 | 112,556 | 0.933 |
| `random_p025` | 60,000 | 22.8 | 48.0 | 17.2 | 11.0 | 78,825 | 89,996 | 1.142 | 86,696 | 1.100 |
| `random_p050` | 20,000,000 | 96.5 | 96.5 | 0.0 | 0.0 | 288,125 | 177,814 | 0.617 | 172,433 | 0.598 |
| `random_p050` | 250,000 | 72.2 | 95.5 | 5.5 | 0.0 | 248,282 | 177,326 | 0.714 | 171,959 | 0.693 |
| `random_p050` | 100,000 | 21.4 | 59.3 | 17.5 | 8.2 | 123,066 | 138,812 | 1.128 | 134,351 | 1.092 |
| `random_p050` | 60,000 | 18.1 | 26.4 | 18.2 | 16.2 | 76,791 | 91,699 | 1.194 | 88,444 | 1.152 |
| `fixed_verifier_only` | 20,000,000 | 95.3 | 95.3 | 0.0 | 0.0 | 410,106 | 221,984 | 0.541 | 215,908 | 0.526 |
| `fixed_verifier_only` | 250,000 | 53.9 | 94.3 | 9.7 | 0.0 | 308,831 | 221,064 | 0.716 | 215,010 | 0.696 |
| `fixed_verifier_only` | 100,000 | 18.5 | 66.4 | 18.1 | 6.6 | 142,361 | 182,482 | 1.282 | 177,270 | 1.245 |
| `fixed_verifier_only` | 60,000 | 17.9 | 31.8 | 18.2 | 14.8 | 88,272 | 125,803 | 1.425 | 121,965 | 1.382 |
| `fixed_estimator_only` | 20,000,000 | 95.4 | 95.4 | 0.0 | 0.0 | 98,187 | 65,265 | 0.665 | 65,123 | 0.663 |
| `fixed_estimator_only` | 250,000 | 94.8 | 94.9 | 0.0 | 0.0 | 98,039 | 65,197 | 0.665 | 65,058 | 0.664 |
| `fixed_estimator_only` | 100,000 | 67.2 | 94.4 | 6.4 | 0.0 | 79,299 | 62,955 | 0.794 | 62,822 | 0.792 |
| `fixed_estimator_only` | 60,000 | 26.7 | 70.8 | 16.1 | 5.6 | 53,371 | 51,530 | 0.965 | 51,510 | 0.965 |
| `fixed_heuristic_every2` | 20,000,000 | 95.4 | 95.4 | 0.0 | 0.0 | 29,094 | 22,273 | 0.766 | 23,213 | 0.798 |
| `fixed_heuristic_every2` | 250,000 | 95.4 | 95.4 | 0.0 | 0.0 | 29,094 | 22,273 | 0.766 | 23,213 | 0.798 |
| `fixed_heuristic_every2` | 100,000 | 95.3 | 95.4 | 0.0 | 0.0 | 29,095 | 22,273 | 0.766 | 23,213 | 0.798 |
| `fixed_heuristic_every2` | 60,000 | 95.0 | 95.1 | 0.0 | 0.0 | 28,880 | 22,271 | 0.771 | 23,212 | 0.804 |
| `fixed_heuristic_every4` | 20,000,000 | 95.4 | 95.4 | 0.0 | 0.0 | 22,406 | 18,682 | 0.834 | 19,717 | 0.880 |
| `fixed_heuristic_every4` | 250,000 | 95.4 | 95.4 | 0.0 | 0.0 | 22,406 | 18,682 | 0.834 | 19,717 | 0.880 |
| `fixed_heuristic_every4` | 100,000 | 95.4 | 95.4 | 0.0 | 0.0 | 22,406 | 18,682 | 0.834 | 19,717 | 0.880 |
| `fixed_heuristic_every4` | 60,000 | 95.4 | 95.4 | 0.0 | 0.0 | 22,406 | 18,682 | 0.834 | 19,717 | 0.880 |
| `oracle_evidence_privileged` | 20,000,000 | 100.0 | 100.0 | 0.0 | 0.0 | 0 | 0 | - | 0 | - |
| `oracle_evidence_privileged` | 250,000 | 100.0 | 100.0 | 0.0 | 0.0 | 0 | 0 | - | 0 | - |
| `oracle_evidence_privileged` | 100,000 | 100.0 | 100.0 | 0.0 | 0.0 | 0 | 0 | - | 0 | - |
| `oracle_evidence_privileged` | 60,000 | 100.0 | 100.0 | 0.0 | 0.0 | 0 | 0 | - | 0 | - |
| `oracle_immediate_privileged` | 20,000,000 | 100.0 | 100.0 | 0.0 | 0.0 | 0 | 0 | - | 0 | - |
| `oracle_immediate_privileged` | 250,000 | 100.0 | 100.0 | 0.0 | 0.0 | 0 | 0 | - | 0 | - |
| `oracle_immediate_privileged` | 100,000 | 100.0 | 100.0 | 0.0 | 0.0 | 0 | 0 | - | 0 | - |
| `oracle_immediate_privileged` | 60,000 | 100.0 | 100.0 | 0.0 | 0.0 | 0 | 0 | - | 0 | - |

Reading it.

- At 20,000,000 ns no success moves, and mean modelled cost falls for every arm that runs a component
  (`all_components` by 46%, `fixed_verifier_only` by 47%, `random p=0.5` by 40%, with the old weights
  put back; the heuristic family by 12 to 29%, `fixed_estimator_only` by 34%), because repeated outputs are no longer decoded. The
  re-weighting alone moves the total by at most a few percent.
- At the binding limits the success of verifier-running arms rises a lot (`all_components` at 250,000
  ns from 29.1 to 87.0, `fixed_verifier_only` from 53.9 to 94.3, `random p=0.5` from 72.2 to 95.5)
  and critical misses fall with it. Mean cost can *rise* at 100,000 and 60,000 ns (`all_components`
  121,279 to 147,350 at 100,000): the declared bill per step is lower, so the budget lasts for more
  steps and the arm does more work before it runs out. A lower cost at a binding budget now means
  the arm is cheaper per step, not that it did less in total; compare arms at equal success or at
  the non-binding level when reading cost.
- The arms that never run the verifier were *not* bit-identical this time (the Stage B note said the
  heuristic family was unchanged under the verifier-only variant): their repeated heuristic and
  estimator outputs are no longer decoded either, so `heuristic_only` moves from 94.8 to 95.0 at
  100,000 and 94.4 to 94.6 at 60,000, and `fixed_estimator_only` from 67.2 to 94.4 at 100,000 and 26.7
  to 70.8 at 60,000. The estimator's five-hypothesis output repeats at nearly every step.
- Oracle rows are unchanged (they declare nothing and count nothing).

### Against the upper-bound variant of `b3-finding4.md` section 7

That variant stopped charging the *verifier's* decoding altogether (a cost-only scratch patch); this
change stops decoding and charging *any* repeated output but still charges every output that
differs, plus the comparison. Pooled success, variant then this change:

| arm | 250,000 | 100,000 | 60,000 |
|---|---|---|---|
| `heuristic_only` | 95.4, 95.4 | 94.8, 95.0 | 94.4, 94.6 |
| `fixed_estimator_only` | 94.8, 94.9 | 67.2, 94.4 | 26.7, 70.8 |
| `all_components` | 73.0, 87.0 | 18.6, 25.4 | 18.2, 18.2 |
| `random p=0.25` | 95.9, 95.9 | 81.4, 86.1 | 41.5, 48.0 |
| `random p=0.5` | 95.3, 95.5 | 43.1, 59.3 | 20.3, 26.4 |
| `fixed_verifier_only` | 94.3, 94.3 | 72.3, 66.4 | 34.7, 31.8 |

It is higher than the variant wherever the estimator's and heuristic's repeated outputs mattered,
and lower for `fixed_verifier_only` at the two tight limits, where the verifier's *changed* outputs
(decoded in full here, free in the variant) are most of what is left of the bill. "Upper bound" was
accurate for the verifier only.

## 5. Is "a failed component raises success" gone? No: reduced, not removed

The paired `ComponentTimeout` comparison of `finding4.py` (500 episodes, seven arms, four budgets,
same manifest, seed and class; only which directives are applied differs), run with the same scratch
binary (the patch's harness hunk only; the rule is the tree's) before and after. Success % with no
directive, with only `Fail` directives, and episodes `Fail` alone raised / lowered:

| arm | limit | before: none, fail, raised/lowered | after: none, fail, raised/lowered |
|---|---|---|---|
| `all_components` | 250,000 (B3 cell) | 13.8, 24.0, 51/0 | 90.6, 91.6, 6/1 |
| `random p=0.5` | 250,000 | 70.4, 75.0, 23/0 | 100.0, 99.8, 0/1 |
| `random p=0.25` | 100,000 | 36.2, 46.2, 50/0 | 88.8, 90.8, 11/1 |
| `random p=0.5` | 100,000 (B3 cell) | 4.0, 10.6, 33/0 | 54.6, 59.6, 25/0 |
| `all_components` | 100,000 | 0.0, 0.2, 1/0 | 9.8, 11.6, 9/0 |
| `random p=0.25` | 60,000 (B3 cell) | 5.6, 14.8, 46/0 | 40.2, 48.0, 39/0 |
| `random p=0.5` | 60,000 | 0.0, 1.2, 6/0 | 11.2, 13.6, 12/0 |

Over the three verifier-running arms at the three binding limits `Fail` alone raised 210 episodes
and now raises 102. The original cell (`all_components` at 250,000) is essentially gone (51 to 6
raised, 24.0 against 13.8 becoming 91.6 against 90.6). The other two B3 cells and the 100,000 and
60,000 rows remain: 25 and 39 episodes of 500, a gain of 5 to 8 points, still with the stop reason
`final_declaration` without the directive and `terminal` with it, in 25 of 25 and 38 of 39 raised episodes of the
two cells.

Why it remains, by the ledger and by one more scratch variant. Seed 1007, `random p=0.25`, 60,000 ns
(`finding4.py trace`, `show`): with the verifier alive the rule's declared charge is 717 to 800 ns at
a step (it was 7,600 to 8,300 for `all_components` before), with it failed 109 to 125 ns. The
difference is no longer decoding. It is the rule narrowing the verifier's stored 46-hypothesis set
against the probes bought, every step, at 10.5 ns per world declared, though neither the set nor
the probes have changed. A failed verifier leaves the estimator's five. A scratch build that declares
the narrowing at zero (`WORLD_PS = 0` in the rule, nothing else; the grids rerun) cuts the raised
episodes in those arms and budgets from 102 to 35. What is left of the 35 is unexplained here; the
stale-priority effect of the verifier's set measured at 0.2 to 1.8 points in `b3-finding4.md`
section 5 is the candidate. I did not change the rule for this: it is outside "decode each output
once", it is real work the rule does every call and counts as `worlds`, and caching a narrowed view
until the stored sets or the bought probes change is its own change to the rule with its own
equivalence argument (and the same probe-scoring term when a set can be probed). It would need a
decision before EXP-001 is frozen if a failed component must not help.

## 6. What I am least sure of

- That the deviation in section 1 is what the coordinator wanted. A literal A6c is a no-op in the
  code, and the effect it was meant to remove is only mostly removed by the version here.
- The declared cost's decoding constants were not refitted (declared over measured is 1.14 at the
  median now against 1.03 before, on another host); the declared comparison term is the hot weight.
- The padded-output trick in `calibrate_ops` exercises the decoding path with outputs that differ in
  length from the held ones, so the comparison ends at a length. Real changed outputs often have the
  same length and differ later; `compared_bytes` counts all of their bytes (an upper bound), at 1% of
  the time, so the error is small, but it is not measured.
- One generator, seeds 1000 to 1499, one run of each condition; the scratch variants (the diagnostic
  harness hunk, `WORLD_PS = 0`, the old weights) are builds of copies, not in the tree.
- The cost comparison at binding budgets is confounded by the budget lasting longer (section 4).

## 7. Reproduction

```bash
# before: the tree at da73030; after: this tree. Each needs a clean tree whose HEAD the manifests
# name, the release binary built, no build running.
export GORDIAN_WORK=/tmp/gordian-exploration
for c in 20000000 250000 100000 60000; do
  python3 experiments/exploration/scripts/mkmanifest.py b1-c$c-s1000-1499 $c 1000 500 1 3600 $GORDIAN_WORK/manifests/c$c.json
  scripts/run-driver.sh --manifest $GORDIAN_WORK/manifests/c$c.json --out artifacts/runs/after-b1-c$c-s1000-1499
done   # and the same at da73030 into artifacts/runs/before-b1-c...
python3 experiments/exploration/scripts/a6c_table.py before after [oldw]
# the paired runs: finding4.py grid SIDE-COND COND for COND in all none fail slow, with the patch's
# harness hunk applied to a copy of the tree (see finding4.py's docstring), then
python3 experiments/exploration/scripts/a6c_finding4.py before after
```
