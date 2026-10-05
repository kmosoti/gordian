# B4 oracle gap and headroom

Status: development run, `exploration` branch. Nothing here tests a hypothesis and nothing here may
later be cited as confirmation. The last section is a recommendation; the coordinator decides.

Data: the four B1 runs (seeds 1000 to 1499, 500 episodes per class and cell; run ids and hashes in
`experiments/exploration/b1-variance.md`), plus three supplementary families of runs described
there: S1 (a "select nothing" arm and longer periods), S2 (five lower budget levels) and S3
(noise rate 50). Truth distributions come from the generator itself, read with the privileged
accessor for analysis only (no policy sees it). They were first read by a scratch program outside
the repository; `crates/gordian-eval/examples/truth_table.rs` now regenerates the same table
inside the evaluator crate (the one place the repository's guard allows the accessor), and
`experiments/exploration/scripts/b4.py` calls it. The table itself is not committed.

## 1. The oracle gap, per class and budget level

`gap = success(oracle_evidence) - success(best non-privileged arm)`, per (class, budget). The best
arm is the one with the highest success on that cell among the eight public B1 arms
(`heuristic_only`, `all_components`, `random p=0.25`, `random p=0.5`, fixed verifier only, fixed
estimator only, fixed heuristic every 2 and every 4); on a tie, the first in that list. The
interval is a 90% percentile bootstrap over the 500 episodes of the cell (4,000 resamples, seed
20261005), paired because every arm and the oracle play the same episodes. Taking a maximum over
eight arms on the same data biases the best arm's success upward and the gap downward, so the
bootstrap **re-picks the best arm inside every resample** (the interval then allows for the
selection, up to the usual caveats of a bootstrap of a maximum), and the detail table adds the
interval with the arm held fixed and a split-sample estimate (arm chosen on seeds 1000 to 1249,
scored on 1250 to 1499). `oracle_immediate` has success 1.000 in every cell as well, so the gap to
it is the same.

**T1. Gap [90% interval, best arm re-picked per resample].**

| class | 20,000,000 | 250,000 | 100,000 | 60,000 |
|---|---|---|---|---|
| Ambiguous | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] |
| DelayedConfigChange | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] |
| NoiseFlood | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] |
| JointlyDecisive | 0.384 [0.348, 0.420] | 0.384 [0.348, 0.420] | 0.384 [0.348, 0.420] | 0.384 [0.348, 0.422] |
| NoFault | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] |
| CriticalFault | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] |
| QuietUrgent | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] |
| Duplicates | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] |
| FeedbackBait | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] |
| StaleMemory | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] |
| ComponentTimeout | 0.002 [0.000, 0.006] | 0.074 [0.054, 0.094] | 0.122 [0.098, 0.146] | 0.122 [0.098, 0.146] |

**T2. The cells with a non-zero gap, in detail.**

| class | compute (ns) | best non-privileged arm (first of ties) | its success | arms tied at best | oracle_evidence | gap | 90% interval, best arm re-picked in each resample | 90% interval, that arm fixed | gap, arm chosen on seeds 1000-1249 and scored on 1250-1499 (arm) |
|---|---|---|---|---|---|---|---|---|---|
| ComponentTimeout | 20,000,000 | all_components | 0.998 | 2 | 1.000 | 0.002 | [0.000, 0.006] | [0.000, 0.006] | 0.004 (all_components) |
| ComponentTimeout | 250,000 | random p=0.25 | 0.926 | 1 | 1.000 | 0.074 | [0.054, 0.094] | [0.054, 0.094] | 0.068 (random p=0.25) |
| ComponentTimeout | 100,000 | fixed heuristic every 4 | 0.878 | 1 | 1.000 | 0.122 | [0.098, 0.146] | [0.098, 0.146] | 0.120 (fixed heuristic every 4) |
| ComponentTimeout | 60,000 | fixed heuristic every 4 | 0.878 | 1 | 1.000 | 0.122 | [0.098, 0.146] | [0.098, 0.146] | 0.120 (fixed heuristic every 4) |
| JointlyDecisive | 20,000,000 | heuristic_only | 0.616 | 7 | 1.000 | 0.384 | [0.348, 0.420] | [0.348, 0.420] | 0.420 (heuristic_only) |
| JointlyDecisive | 250,000 | heuristic_only | 0.616 | 4 | 1.000 | 0.384 | [0.348, 0.420] | [0.348, 0.420] | 0.420 (heuristic_only) |
| JointlyDecisive | 100,000 | heuristic_only | 0.616 | 3 | 1.000 | 0.384 | [0.348, 0.420] | [0.348, 0.420] | 0.420 (heuristic_only) |
| JointlyDecisive | 60,000 | heuristic_only | 0.616 | 3 | 1.000 | 0.384 | [0.348, 0.422] | [0.348, 0.422] | 0.420 (heuristic_only) |

The other 36 cells have gap 0.000: between 3 and 8 public arms tie the
oracle's 1.000 there, so the best arm named for them is only "first of the ties".

**Is the gap under 0.10 on every class at every level?** **No.** `JointlyDecisive` is 0.384
[0.348, 0.420] at all four levels, and `ComponentTimeout` is 0.122 [0.098, 0.146] at 100,000 and
60,000 (0.074 [0.054, 0.094] at 250,000, 0.002 at 20,000,000). Under the plan's rule (B4: "if the
gap is under 0.10 success on every class, revise A1 before anything is frozen") **the rule is not
triggered, at any level**, and a world revision is not indicated by this test. The headroom is
narrow: one class with a gap of 0.10 or more at 20,000,000 and 250,000 (`JointlyDecisive`), two at
100,000 and 60,000 (adding `ComponentTimeout`), and zero gap in nine or ten of the other classes. Pooled over the 11 classes the oracle is
0.035 above the best public arm at 20,000,000 (`all_components`) and 0.046 above it at the other
levels (the heuristic family), which is `0.384 / 11 = 0.035` from `JointlyDecisive` plus
`0.122 / 11 = 0.011` from `ComponentTimeout`.

### What the gap is made of

- **`JointlyDecisive` (0.384) is a property of the shared decision rule, not of scheduling.** At
  20,000,000 every public arm scores 0.616 on it (0.614 for `random p=0.25`), and at every binding
  level none exceeds 0.616 (the heuristic family sits at it exactly). That is the generator's
  prior for `DependencyDown` over the pair (0.616 in seeds 1000 to 1499, table T3), because the
  rule's one-step expected-set-size criterion never buys the two samples that decide the class
  (`POLICIES.md` section 4: "It stalls") and falls back to guessing the first of the tied pair.
  No selection of components changes that. It is headroom for a different decision rule (EXP-002
  territory), and EXP-001's intervention, the scheduling policy only, cannot reach it.
- **`ComponentTimeout` (0.122 where it is non-zero) is the one piece of headroom a selection
  policy could take**: an arm that depends on one component loses the episodes where a `Fail`
  directive lands on it (12.2% abstain), and an arm that also has another component to turn to
  does not (`all_components` 99.8% at 20,000,000). The policy has no signal that a component
  failed other than an empty output, which I did not investigate.
- The oracle's own cost is not modelled: its modelled cost and bill are 0 by construction
  (`POLICIES.md` section 6), though `oracle_evidence`'s planner takes measured wall time (about
  48 thousand ns per episode). It is a ceiling on quality, never a cost comparator.

## 2. Effective ambiguity

The generator's truth priors inside an ambiguity set are not uniform (review log, A1; `DESIGN.md`
section 5.1). Measured from the generator over the same 500 seeds per class:

**T3. What the public stream leaves open, and how much of that is real.** "Effective kinds" is
`2^H` for the entropy of the true kind within the set the full public stream leaves consistent;
"best zero-probe guess" is the accuracy of an arm that knows the generator's prior and never
probes; "uniform guess" is `1 / |set|` at the level of kinds.

| class | truth kind distribution (seeds 1000-1499) | hypotheses the full public stream leaves consistent (mean; kinds) | entropy of truth kind within that set (bits) | effective kinds 2^H | best zero-probe guess, prior known | uniform guess in set |
|---|---|---|---|---|---|---|
| Ambiguous | ResExh 0.416, CfgDrift 0.392, CredExp 0.192 | 5 (at one site); all five kinds | 1.51 | 2.85 | 0.416 | 0.200 |
| DelayedConfigChange | CfgDrift 1.000 | 1; the true kind alone | 0.00 | 1.00 | 1.000 | 1.000 |
| NoiseFlood | ResExh 0.228, CfgDrift 0.196, Interm 0.196, DepDown 0.192, CredExp 0.188 | 1; the true kind alone | 0.00 | 1.00 | 1.000 | 1.000 |
| JointlyDecisive | DepDown 0.616, Interm 0.384 | 2; DependencyDown, Intermittent | 0.96 | 1.95 | 0.616 | 0.500 |
| NoFault | no fault 1.000 | 41.6; all five kinds and no-fault | 0.00 | 1.00 | 1.000 | 0.167 |
| CriticalFault | ResExh 0.312, CfgDrift 0.306, CredExp 0.210, Interm 0.086, DepDown 0.086 | 3.01; five kinds in 251 of 500 episodes, one kind in the rest | 0.77 | 1.70 | 0.712 | 0.598 |
| QuietUrgent | ResExh 0.228, CfgDrift 0.196, Interm 0.196, DepDown 0.192, CredExp 0.188 | 1; the true kind alone | 0.00 | 1.00 | 1.000 | 1.000 |
| Duplicates | ResExh 0.228, CfgDrift 0.196, Interm 0.196, DepDown 0.192, CredExp 0.188 | 1; the true kind alone | 0.00 | 1.00 | 1.000 | 1.000 |
| FeedbackBait | ResExh 0.416, CfgDrift 0.392, CredExp 0.192 | 5 (at one site); all five kinds | 1.51 | 2.85 | 0.416 | 0.200 |
| StaleMemory | ResExh 0.416, CfgDrift 0.392, CredExp 0.192 | 5 (at one site); all five kinds | 1.51 | 2.85 | 0.416 | 0.200 |
| ComponentTimeout | ResExh 0.228, CfgDrift 0.196, Interm 0.196, DepDown 0.192, CredExp 0.188 | 1; the true kind alone | 0.00 | 1.00 | 1.000 | 1.000 |

- **`Ambiguous`, `FeedbackBait`, `StaleMemory`: nominal 5 kinds, effective 2.85.** The stream fits all
  five kinds at the true site, but truth is drawn only from `ResourceExhausted`, `ConfigDrift` and
  `CredentialExpired` (41.6%, 39.2%, 19.2% here; 40, 40, 20 by design), so `DependencyDown` and
  `Intermittent` are never true in these classes. An arm that knows the prior and never probes is
  right 41.6% of the time, against 20% if the set were uniform over five kinds. Exactly one of the
  three dedicated probes decides, so the oracle needs 1.0 probes on these classes against 1.97
  for every shared-rule arm (`b1-variance.csv`); a probe order that followed the prior would
  need about `0.416*1 + 0.392*2 + 0.192*2 = 1.58`, by arithmetic, not by a run.
- **`JointlyDecisive`: nominal 2, effective 1.95**, prior guess 0.616. The arms sit exactly at it
  (0.616), which is a second independent confirmation that nothing about the hidden kind leaks
  into the public stream (review log, A4, saw the same with 0.58 against 0.595).
- **`CriticalFault`** is half the plain ambiguous signal and half a full signature (251 of 500
  episodes carry five consistent kinds), so its effective ambiguity is 1.70 and its prior guess
  0.712.
- **The five classes with a full signature (`NoiseFlood`, `QuietUrgent`, `Duplicates`,
  `ComponentTimeout`, `DelayedConfigChange`) are identified by the stream**: nominal and effective
  ambiguity are 1, and all kinds appear roughly uniformly across them (18.8% to 22.8% each).
- **`NoFault`**: the stream leaves 41.6 hypotheses (every fault at every site is consistent with
  silence) while truth is "no fault" 100% of the time, so "no fault" is the prior's guess with
  accuracy 1.0 and a policy that declares it costs nothing. This is the "free" 1/11 of every
  pooled success (0.0909 for an arm that does nothing, `random p=0` in S1).

Consequence for B4 and for EXP-002: the world's headroom against a policy that has learned the
priors is smaller than its nominal ambiguity suggests. Five of the eleven classes are identified
by the stream, `NoFault` is decided by silence, `JointlyDecisive` is decided by the prior in
practice (the arms never buy its samples), and on the three three-kind classes the shared-rule
arms spend about one probe per episode more than the oracle.

## 3. Headroom for the scheduling policy, which is not the same as the oracle gap

EXP-001's intervention is scheduling only, with a shared decision rule. Three questions decide
whether there is room for it: whether quality can improve, whether cost can fall, and against
which baseline.

### 3.1 Quality: the best fixed periodic heuristic is within 0.011 to 0.019 of the best any arm reached

`0.9649` is the highest pooled success reached by any public arm at any level (`all_components` and
`random p=0.5` at 20,000,000). The table gives, per level, the naive pipeline
(`all_components`, which is `fixed_pipeline` with its defaults, `POLICIES.md` section 5), the
every-step heuristic, and the periodic heuristics.

| compute (ns) | `all_components` success (cost) | `heuristic_only` success (cost) | success of every 2 / 4 / 8 / 16 | best periodic heuristic: success (cost), period | shortfall of best periodic from 0.9649 | range of success over the public arms run at this level |
|---|---|---|---|---|---|---|
| 20,000,000 | 0.965 (521,984) | 0.954 (42,637) | 0.954 / 0.954 / 0.954 / 0.909 | 0.954 (19,258), every 8 | 0.011 | 0.909 to 0.965 (10 arms) |
| 250,000 | 0.291 (285,922) | 0.954 (42,637) | 0.954 / 0.954 / 0.954 / 0.909 | 0.954 (19,258), every 8 | 0.011 | 0.291 to 0.954 (10 arms) |
| 100,000 | 0.182 (121,279) | 0.948 (42,571) | 0.953 / 0.954 / 0.954 / 0.909 | 0.954 (19,258), every 8 | 0.011 | 0.182 to 0.954 (10 arms) |
| 60,000 | 0.162 (76,554) | 0.944 (39,755) | 0.950 / 0.954 / 0.954 / 0.909 | 0.954 (19,258), every 8 | 0.011 | 0.162 to 0.954 (10 arms) |
| 50,000 | 0.150 (64,720) | 0.881 (37,557) | 0.947 / 0.953 / 0.954 / 0.909 | 0.954 (19,258), every 8 | 0.011 | 0.150 to 0.954 (8 arms) |
| 40,000 | 0.135 (52,828) | 0.691 (33,482) | 0.945 / 0.951 / 0.954 / 0.909 | 0.954 (19,203), every 8 | 0.011 | 0.135 to 0.954 (8 arms) |
| 30,000 | 0.121 (41,414) | 0.491 (27,495) | 0.939 / 0.948 / 0.953 / 0.908 | 0.953 (18,844), every 8 | 0.012 | 0.121 to 0.953 (8 arms) |
| 20,000 | 0.107 (30,452) | 0.253 (19,278) | 0.631 / 0.944 / 0.949 / 0.908 | 0.949 (18,077), every 8 | 0.016 | 0.107 to 0.949 (8 arms) |
| 15,000 | 0.097 (22,016) | 0.205 (14,619) | 0.429 / 0.868 / 0.945 / 0.905 | 0.945 (17,557), every 8 | 0.019 | 0.097 to 0.945 (8 arms) |

(Levels 20,000,000 to 60,000 are B1 with S1's every 8 and 16; the rest are S2. Success is pooled
over the 11 classes, so an arm that does nothing scores 0.091 from `NoFault` alone. `every k` is
`fixed_pipeline` with the heuristic only, run at every k-th step. "Cost" is modelled cost.)

- **`every 8` is the best of the four periods at every level from 15,000 up**, equals the
  heuristic's own ceiling of 0.954 from 40,000 up (0.953 at 30,000, 0.949 at 20,000, 0.945 at
  15,000), and costs 19.3 thousand ns, 3.7% of `all_components` at 20,000,000.
- **Where a fixed baseline needs tuning at all.** At 100,000 and 60,000 the every-step heuristic
  loses 0.6 and 1.0 points to the periodic ones (0.948 and 0.944 against 0.954). At 50,000 and
  below it loses 7 to 75 points (0.881 at 50,000, 0.205 at 15,000). `every 4` is within 1 point
  down to 20,000 and loses 8.6 points at 15,000; `every 2` loses 32 points at 20,000 and 53 at
  15,000. So a periodic schedule has to be tuned to the budget, and the tuned one is flat and
  cheap.
- **Quality headroom against the tuned periodic baseline is at most 0.011 to 0.019** at every
  level, almost all of it `ComponentTimeout` (a second component to fall back on).
- **Against the naive pipeline** (`all_components`) the headroom is large at every binding level:
  0.66 at 250,000, 0.77 at 100,000, 0.79 at 60,000. That is the weak-baseline case (charter
  section 7): the baseline is not tuned to the budget, and "beating" it needs no selection
  mechanism.

### 3.2 Cost: the shared rule is most of the bill, and every arm pays it on every step

| arm | mean heuristic calls | cost (modelled ns) | of which components | of which the shared rule | rule share | success | decision at (s) |
|---|---|---|---|---|---|---|---|
| heuristic_only | 38.7 | 42,637 | 14,711 | 27,925 | 65% | 0.954 | 1.89 |
| fixed_heuristic_every2 | 19.9 | 29,094 | 7,559 | 21,535 | 74% | 0.954 | 1.91 |
| fixed_heuristic_every4 | 10.6 | 22,406 | 3,988 | 18,418 | 82% | 0.954 | 1.95 |
| fixed_heuristic_every8 | 5.9 | 19,258 | 2,197 | 17,061 | 89% | 0.954 | 2.03 |
| fixed_heuristic_every16 | 3.5 | 17,931 | 1,258 | 16,673 | 93% | 0.909 | 2.18 |
| random_p000 | 0.0 | 17,141 | 0 | 17,141 | 100% | 0.091 | 3.00 |

At 20,000,000 the shared decision rule is 65% of a
`heuristic_only` episode's modelled cost, 82% of `every 4`'s, 89% of `every 8`'s. The harness
charges the rule at every 50 ms step for every non-privileged arm, "including an arm that selects
nothing" (`POLICIES.md` 2.1), and a selector can only choose components (`Selector`), not skip
the rule. Across the five heuristic arms cost is linear in the number of heuristic calls:
`cost = 15,146 + 707 * calls` ns (fitted to five points, so a rough fit). The intercept is what
an arm that reaches a decision at about 2 s with almost no heuristic work would pay. For
comparison `random p=0` (rule alone, no component ever selected, waiting for the 3 s patience)
costs 17,141; it pays more than the intercept because it decides later (3.0 s against 1.9 to 2.2 s)
and so takes more steps.

So the largest savings a selector could show, if it kept the periodic arms' success by calling
the heuristic as few times as one call, are:

| baseline | its cost (modelled ns) | S of an arm that costs 15.9 thousand (intercept plus one heuristic call) | S of `random_p000` (the rule alone, 17,141) |
|---|---|---|---|
| heuristic_only | 42,637 | 0.628 | 0.598 |
| fixed_heuristic_every2 | 29,094 | 0.455 | 0.411 |
| fixed_heuristic_every4 | 22,406 | 0.292 | 0.235 |
| fixed_heuristic_every8 | 19,258 | 0.177 | 0.110 |
| fixed_heuristic_every16 | 17,931 | 0.116 | 0.044 |
| all_components (= `fixed_pipeline` defaults) | 521,984 | 0.970 | 0.967 |

Read the second column as "S of a hypothetical arm costing 15,853 ns", which is the intercept
plus one call; it assumes the arm decides at about the same time as the periodic ones and that
one call suffices. The third column is what an arm that does nothing at all gets (it abstains
on every episode, so it fails every faulted class).

- **Against `fixed heuristic every 8`, the cost-quality-optimal periodic pipeline found, S cannot
  exceed about 0.18 even for a perfectly timed selector; against `every 4` about 0.29; against
  `every 2` about 0.46.** EXP-001's H1 needs S > 0.20 and D > -0.01. Against the tuned periodic
  pipeline the H1 region is therefore empty or nearly empty *in this world with this cost model*,
  whatever the selector does; against `all_components` (0.97) it is trivially met by a heuristic
  alone. That is the central finding of this document: the tuned baseline cannot be beaten on cost by
  20% (and has almost no quality to gain), and the naive baseline is beaten without any selection
  mechanism.
- The sources of the bound are the harness's design choices (the rule is charged every step for
  every arm; probes are not in `C`, see below), not properties of the idea of selective
  activation. A harness in which the selector also decides when the rule runs, or a rule whose
  cost is not dominated by a per-step constant, would change it. That is a revision of the
  instrument, to be made before freezing, not after.
- Probes are **not in the modelled cost `C`** (it is components plus the rule, A8b), while the
  charter's `C` "includes sensing". Mean probes per episode are 0.70 for the heuristic arms and
  2.43 for `oracle_evidence`; on the three ambiguous classes the shared-rule arms buy 1.97 where one
  would do. That is a larger lever than component compute and it is currently priced at zero in
  `C` (it is in the `Probes` bill with no exchange rate, charter section 4 requires one).

### 3.3 Stress, as it bears on headroom

The stress classes add no discrimination except `ComponentTimeout` and, at noise 50, a corner of
`Duplicates` (`b3-stress.md`). They provide no headroom of their own.

## 4. Recommendation: which budget levels give EXP-001 meaningful headroom

This is a recommendation. The coordinator decides.

**Short answer: none of the four levels gives headroom against a tuned periodic baseline, and the
reason is not the budget. Of the four, 100,000 and 60,000 are the only ones at which the choice of
schedule changes success (not just cost), and 20,000,000 should not be used. I recommend not
freezing EXP-001's budget on any of them until the baseline definition and the cost accounting
below are decided.**

Per level, with the numbers above:

| compute (ns) | binds? | what scheduling can change here | verdict |
|---|---|---|---|
| 20,000,000 | no arm | cost only. Success of the sensible arms spans 0.954 to 0.965. `S` against the naive pipeline is 0.92 to 0.96 and against `every 8` at most about 0.18 | exclude: no quality headroom, and `S` is either trivial or capped |
| 250,000 | `all_components` (0.29), verifier only (0.54), `random p=0.5` (0.72) | quality against the naive pipeline (0.66). None against the heuristic family, which does not bind (0.954) | not recommended: gives nothing the two lower levels do not, and it is weak-baseline headroom |
| 100,000 | also estimator only (0.67), `random p=0.25` (0.46) | the every-step heuristic starts to bind (0.948 against 0.954); `ComponentTimeout` gap 0.122; two arms at intermediate success | acceptable if the baseline is tuned per level |
| 60,000 | everything except the periodic heuristics; the every-step heuristic hits its limit in 14% of episodes (0.944) | a tuned period measurably matters (0.950 to 0.954 for periodic against 0.944); `Slow` directives leak budget (`ComponentTimeout` 76.6%) | acceptable if the baseline is tuned per level |

What I would want before a level is chosen:

1. **Define the baseline as the cost-quality-tuned periodic pipeline** (per budget level, tuned on
   exploration seeds), not `fixed_pipeline` with its defaults. Against the defaults the mechanism is
   not tested (charter section 7: "a weak baseline makes any architecture look impressive").
   On this exploration data the tuned one is `every 8` at every level from 15,000 up.
2. **Decide whether the selector may skip the shared rule** (that is, whether "selective
   activation" includes deciding when to compute a decision at all). If not, the available S is
   bounded near 0.18 against `every 8` by the structure of `C`, and EXP-001 would test whether the
   selector can reach a bound it cannot exceed; H1 as stated (`S > 0.20`) is then not testable
   against the tuned baseline. If yes, the harness needs a way for a selector to skip a step, and
   the rule's per-step cost has to be recalibrated.
3. **Price probes** (an exchange rate or a hard constraint, charter section 4) before any selector
   that can change how many probes it buys is compared on cost.
4. **If a level must be picked from this sweep: 100,000, with 60,000 as the stress level.** They
   are the two at which the quality of the choice of schedule shows up and the tuned baseline is
   not at a flat ceiling by the width of the budget alone. Between them they cover the
   `ComponentTimeout` budget-leak behaviour. The supplementary S2 sweep suggests the informative
   region for a *tuned* periodic baseline is lower still (15,000 to 40,000 declared ns, where
   `every 4` falls to 0.87 at 15,000 while `every 8` holds 0.945), which is outside the levels
   specified for B1. I did not recommend those levels because they are one sweep of five points
   and a lower floor on what the cheapest sensible pipeline costs, not a result about selection.

## 5. Caveats and what I am least sure of

- **The bound in 3.2 is arithmetic on a five-point linear fit.** It assumes the cost of a heuristic
  call is the same in every episode and that an ideal selector would decide at about the same time
  as the periodic arms. A selector that decides earlier saves rule steps and would beat the
  bound; one that needs more than one call would not reach it. I did not build or run a selector.
- **The "best non-privileged arm" is the best of eight arms that were not tuned for this
  purpose**, with periods 2 and 4 specified and 8 and 16 added by me (S1). A better tuned or
  differently built public arm could close more of the `ComponentTimeout` gap.
- **The rule-bound quality ceiling of 0.9649 is the best pooled success any public arm reached, not
  a proof of a ceiling.** Other component subsets and orders were not run.
- **Success is pooled over classes that include `NoFault`** (1/11 of the weight, scored by an
  arm that does nothing). The conclusions here do not depend on it (the gaps are per class), but
  the pooled success values do, and C1 must say whether `NoFault` enters the primary outcome
  (review log, A2 and A6b).
- **Truth distributions were read from the generator's hidden state** for analysis. They are
  properties of the generator's seeds 1000 to 1499, not of a policy's inputs.
- **All numbers are one generator, one set of seeds, default `episode_params` (and one noise-50
  probe).** Held-out dependency structures (C4) may differ.
- **The question this changes.** The charter asks whether selection earns its overhead. The data
  here sharpen it to: *what part of the cost is selectable at all?* In this harness, by the
  estimate in 3.2, about 0.18 of the tuned periodic pipeline's cost (0.29 of `every 4`'s), because
  the rule that decides is charged at every step for every arm and the components are a small
  part of what is left.
