# B2 power: sample sizes for EXP-001's two criteria, from proxy pairs

Status: development run, `exploration` branch, nothing here tests a hypothesis and nothing here
may later be cited as confirmation. The selective arm (plan item C2) does not exist yet, so every
variance below comes from **proxy pairs** of baselines, not from a selector. Read the tables as
"what the instrument's variance looks like", and the generic planning table (T2) as the thing a
freeze can plan against until C2 exists.

Source data: the four B1 runs (`experiments/exploration/b1-variance.md` lists run ids and hashes),
seeds 1000 to 1499, all 11 classes, so 5,500 paired episodes per pair per budget level, paired on
`(seed, class)`. Cost is `modelled_cost_ns` (the charter's `C`, review log A8b). All tests are
one-sided at 0.05 (the 90% interval that `gordian-analyze compare` reports).

## 1. The proxy pairs, and why they are poor proxies

| | baseline A (plays the fixed pipeline) | arm B (plays the selective arm) |
|---|---|---|
| P1 | `heuristic_only` | `fixed_pipeline`, heuristic only, `every` = 2 |
| P2 | `all_components` | `random_matched`, p = 0.5 |

`D = success(B) - success(A)` and `S = 1 - sum(cost B) / sum(cost A)`, per EXP-001.

Three things limit what these pairs can tell us. All are measured, not assumed.

1. **At the highest budgets the pairs have identical success on every episode.** At
   20,000,000 and 250,000 ns P1 has `sd(d) = 0` (neither arm of P1 runs out of budget there, and
   the two never differ in success), and P2 has `sd(d) = 0` at 20,000,000. A paired sd of zero
   gives no information about the sd of a real selector's paired differences, and the formula
   `n = ((z_a + z_b) sd / margin)^2` returns 0, which is not a sample size. (Identical success is
   what was measured; whether the two arms also declared identically was not checked.)
2. **At binding budgets the difference is one-sided and says the baseline is starved.** At
   100,000 and 60,000 ns, and for P2 at 250,000 ns, B is right where A is wrong on every
   discordant episode (the `up / down` column is `k / 0`). `all_components` has spent its compute
   before any symptom has arrived (A6b entry in `docs/review-log.md`). The sd of such a `d` measures
   how badly A is starved, not the noise in a comparison between two sensible arms. P2 at 250,000
   has `mean d = +0.43`; its "n at true diff 0" of 15,171 is for a true difference that the data
   show to be false, and its "n at the observed difference" (8) is for a baseline nobody would
   preregister.
3. **The cost proxies have a small ratio variance.** `fixed_heuristic_every2` costs a stable
   fraction of `heuristic_only` (`sigma_1`, defined in section 3 and tabulated in T3, is 0.047 at
   20,000,000), so the paired ratio has little variance. A selector whose cost depends on episode
   content will have more. The cost n in T4 is therefore a
   lower bound.

## 2. Success: non-inferiority at margin 0.01

H1 needs `D > -0.01`. For a paired normal-approximation non-inferiority test the package computes
`n = ((z_{1-alpha} + z_{1-beta}) * sd / (margin + true_diff))^2`, rounded up; it is
`gordian-analyze power --sd SD --margin 0.01 --alpha 0.05 --power {0.8,0.9} --true-diff X`, and
every number in T1 and T2 was produced by that command (a script calls it once per cell). "True
diff 0" assumes the selective arm loses nothing; "true diff = observed" plugs in the proxy's own
mean difference (a positive mean lowers n, a negative one beyond -0.01 would make it undefined).
"Classes: 10" drops `NoFault`, which an arm that does nothing and declares "no fault" scores for
free (review log, A5 and A6b); whether `NoFault` enters the primary outcome is a C1 decision, so
both are shown.

**T1. Proxy pairs, success difference `d = success(B) - success(A)` per episode.**

| pair | compute (ns) | classes | n pairs | mean d | sd d | discordant | up / down | n at 80%, true diff 0 | n at 90%, true diff 0 | n at 80%, true diff = observed | n at 90%, true diff = observed |
|---|---|---|---|---|---|---|---|---|---|---|---|
| P1 | 20,000,000 | all 11 | 5,500 | +0.0000 | 0.0000 | 0.0000 | 0 / 0 | degenerate | degenerate | degenerate | degenerate |
| P1 | 20,000,000 | 10, no NoFault | 5,000 | +0.0000 | 0.0000 | 0.0000 | 0 / 0 | degenerate | degenerate | degenerate | degenerate |
| P1 | 250,000 | all 11 | 5,500 | +0.0000 | 0.0000 | 0.0000 | 0 / 0 | degenerate | degenerate | degenerate | degenerate |
| P1 | 250,000 | 10, no NoFault | 5,000 | +0.0000 | 0.0000 | 0.0000 | 0 / 0 | degenerate | degenerate | degenerate | degenerate |
| P1 | 100,000 | all 11 | 5,500 | +0.0049 | 0.0699 | 0.0049 | 27 / 0 | 303 | 419 | 136 | 189 |
| P1 | 100,000 | 10, no NoFault | 5,000 | +0.0054 | 0.0733 | 0.0054 | 27 / 0 | 333 | 461 | 141 | 194 |
| P1 | 60,000 | all 11 | 5,500 | +0.0058 | 0.0761 | 0.0058 | 32 / 0 | 358 | 496 | 143 | 199 |
| P1 | 60,000 | 10, no NoFault | 5,000 | +0.0064 | 0.0798 | 0.0064 | 32 / 0 | 394 | 545 | 147 | 203 |
| P2 | 20,000,000 | all 11 | 5,500 | +0.0000 | 0.0000 | 0.0000 | 0 / 0 | degenerate | degenerate | degenerate | degenerate |
| P2 | 20,000,000 | 10, no NoFault | 5,000 | +0.0000 | 0.0000 | 0.0000 | 0 / 0 | degenerate | degenerate | degenerate | degenerate |
| P2 | 250,000 | all 11 | 5,500 | +0.4316 | 0.4953 | 0.4316 | 2374 / 0 | 15,171 | 21,014 | 8 | 11 |
| P2 | 250,000 | 10, no NoFault | 5,000 | +0.4748 | 0.4994 | 0.4748 | 2374 / 0 | 15,421 | 21,360 | 7 | 10 |
| P2 | 100,000 | all 11 | 5,500 | +0.0322 | 0.1765 | 0.0322 | 177 / 0 | 1,926 | 2,668 | 109 | 150 |
| P2 | 100,000 | 10, no NoFault | 5,000 | +0.0354 | 0.1848 | 0.0354 | 177 / 0 | 2,112 | 2,925 | 103 | 142 |
| P2 | 60,000 | all 11 | 5,500 | +0.0187 | 0.1356 | 0.0187 | 103 / 0 | 1,137 | 1,575 | 138 | 191 |
| P2 | 60,000 | 10, no NoFault | 5,000 | +0.0206 | 0.1421 | 0.0206 | 103 / 0 | 1,248 | 1,729 | 134 | 185 |

Reading it: where the paired sd is not zero, the sample size at "true diff 0" is 303 to 545
episodes (28 to 50 seeds per class) for P1 at 100,000 and 60,000, 1,137 to 1,729 for P2 at 60,000,
and 1,926 to 2,925 for P2 at 100,000. Those rows are the only ones in which two different arms
disagree by a small amount; everything else is degenerate or starved-baseline. None is an
estimate of a selector's n.

**T2. Planning table: n as a function of how often the selective arm's success differs from the
baseline's.** A paired difference of success takes values in {-1, 0, +1}. If a fraction `q` of
episodes are discordant and the discordance is symmetric (true diff 0), `sd = sqrt(q)`. The two
right-hand columns assume the selective arm truly loses half the margin (-0.005) with the same
`q`, which is the case a non-inferiority design exists to catch.

| discordant fraction q | sd = sqrt(q) | n at 80% | n at 90% | seeds per class at 80% (n / 11) | seeds per class at 90% | n at 80%, selective arm truly loses 0.005 | n at 90%, loses 0.005 |
|---|---|---|---|---|---|---|---|
| 0.0010 | 0.0316 | 62 | 86 | 6 | 8 | 242 | 334 |
| 0.0025 | 0.0500 | 155 | 215 | 15 | 20 | 613 | 848 |
| 0.0050 | 0.0707 | 310 | 429 | 29 | 39 | 1,231 | 1,705 |
| 0.0100 | 0.1000 | 619 | 857 | 57 | 78 | 2,467 | 3,417 |
| 0.0200 | 0.1414 | 1,237 | 1,713 | 113 | 156 | 4,940 | 6,843 |
| 0.0500 | 0.2236 | 3,092 | 4,282 | 282 | 390 | 12,359 | 17,120 |
| 0.1000 | 0.3162 | 6,183 | 8,564 | 563 | 779 | 24,725 | 34,247 |
| 0.2000 | 0.4472 | 12,366 | 17,128 | 1,125 | 1,558 | 49,455 | 68,503 |

How to use it: the selector has not been built, so its `q` against `fixed_pipeline` is unknown.
The one empirical anchor is the proxy pairs' own discordance, between 0.5% and 3.2% at the budgets
where two sensible arms differ at all. At `q` = 2% the freeze needs 1,237 episodes for 80% power
and 1,713 for 90% with no true loss (113 and 156 seeds per class if the 11 classes are weighted
equally); at `q` = 5%, 3,092 and 4,282. If the selector truly loses half the margin the figures
quadruple. B1 holds 500 seeds per class (5,500 episodes), so the largest figures cannot be
checked on exploration data and a held-out set of that size would be new generation, not new
runs of anything slow: the runs themselves are cheap (B1's 55,000 arm-episodes took 26 s at the
non-binding level), so the constraint on n is held-out dependency structures (C4), not machine
time.

## 3. Relative savings: S > 0.20

**Estimand and test.** `S = 1 - sum_i B_i / sum_i A_i` over paired episodes (ratio of totals).
H1 is accepted when the lower limit of the 90% paired percentile-bootstrap interval for `S`
exceeds 0.20, which is what `gordian-analyze compare --relative-savings --threshold 0.20` does.
There is no closed-form power for that procedure, so two estimates are given. First, the
brief's per-pair quantity, the sd of the paired per-episode difference in modelled cost:

**T3. Paired per-episode cost, `B_i - A_i`, in modelled ns (5,500 pairs per row).**

| pair | compute (ns) | mean cost A | mean cost B | mean B - A | sd of B - A | S = 1 - sum B / sum A | sigma_1 |
|---|---|---|---|---|---|---|---|
| P1 | 20,000,000 | 42,637 | 29,094 | -13,543 | 7,671 | +0.3176 | 0.0468 |
| P1 | 250,000 | 42,637 | 29,094 | -13,543 | 7,671 | +0.3176 | 0.0468 |
| P1 | 100,000 | 42,571 | 29,095 | -13,476 | 7,679 | +0.3166 | 0.0500 |
| P1 | 60,000 | 39,755 | 28,880 | -10,875 | 7,416 | +0.2736 | 0.1602 |
| P2 | 20,000,000 | 521,984 | 288,125 | -233,859 | 121,037 | +0.4480 | 0.0651 |
| P2 | 250,000 | 285,922 | 248,282 | -37,641 | 53,173 | +0.1316 | 0.1886 |
| P2 | 100,000 | 121,279 | 123,066 | +1,787 | 10,744 | -0.0147 | 0.0873 |
| P2 | 60,000 | 76,554 | 76,791 | +237 | 8,925 | -0.0031 | 0.1165 |

(The sd of a cost difference is dominated by the class mix, since episode classes differ in cost
by an order of magnitude; the ratio statistic `sigma_1` below is the one that enters the power of
the savings test.)

*Delta method (analytic).* Treat `S` as `1 - R`, `R = sum B / sum A`. To first order
`Var(S_hat) = var_i(B_i - R A_i) / (n * mean(A)^2)`. Rejecting means `S_hat - z_{0.95} se > 0.20`,
so for true `S` the n for power `1 - beta` is
`n = ((z_{0.95} + z_{1-beta}) * sigma_1 / (S - 0.20))^2`, with
`sigma_1 = sd_i(B_i - R A_i) / mean(A)` read off the 5,500 empirical pairs. It does not exist
when `S <= 0.20`, and is unreliable below roughly n = 30 (normality of a ratio).

*Simulation from B1's empirical paired distribution.* For each n on a grid, draw n of the 5,500
pairs with replacement, compute the lower limit of the 90% percentile-bootstrap interval of `S`
(400 bootstrap resamples; the code is the package's algorithm, and was checked against
`ratio_of_totals_ci` on three samples, agreeing to within bootstrap noise, 0.0000 to 0.0002), and
call it a rejection if the lower limit exceeds 0.20. Power is the fraction of 400 replications
that reject, so its Monte Carlo standard error is at most 0.025. Resampling pairs from the pooled
5,500 means the class mix of a simulated experiment varies, where the real design fixes it.

*Scenarios.* "empirical" uses the pairs as they are. "rescaled to S = x" multiplies every `B_i` by
`(1 - x) / R`, which sets the true savings to `x` and keeps the paired structure (each episode's
relative cost and its covariance with `A_i`), so it answers "if the selector saved `x` with the
cost variability the proxies show".

**T4. Power for the savings criterion.** Columns `P(n=...)` are the simulated power at that n.

| pair | compute (ns) | scenario | true S | delta-method n at 80% | at 90% | P(n=10) | P(n=20) | P(n=40) | P(n=80) | P(n=160) | P(n=320) | P(n=640) | P(n=1280) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| P1 | 20,000,000 | empirical | +0.318 | 1.0 | 1.4 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 |
| P1 | 20,000,000 | rescaled to S=0.22 | +0.220 | 44.2 | 61.2 | 0.40 | 0.49 | 0.73 | 0.94 | 0.99 | 1.00 | 1.00 | 1.00 |
| P1 | 20,000,000 | rescaled to S=0.25 | +0.250 | 6.5 | 9.0 | 0.87 | 0.97 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 |
| P1 | 20,000,000 | rescaled to S=0.3 | +0.300 | 1.4 | 2.0 | 0.99 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 |
| P1 | 250,000 | empirical | +0.318 | 1.0 | 1.4 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 |
| P1 | 250,000 | rescaled to S=0.22 | +0.220 | 44.2 | 61.2 | 0.40 | 0.53 | 0.72 | 0.94 | 1.00 | 1.00 | 1.00 | 1.00 |
| P1 | 250,000 | rescaled to S=0.25 | +0.250 | 6.5 | 9.0 | 0.87 | 0.98 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 |
| P1 | 250,000 | rescaled to S=0.3 | +0.300 | 1.4 | 2.0 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 |
| P1 | 100,000 | empirical | +0.317 | 1.1 | 1.6 | 0.98 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 |
| P1 | 100,000 | rescaled to S=0.22 | +0.220 | 50.3 | 69.6 | 0.41 | 0.50 | 0.69 | 0.90 | 0.99 | 1.00 | 1.00 | 1.00 |
| P1 | 100,000 | rescaled to S=0.25 | +0.250 | 7.4 | 10.3 | 0.81 | 0.96 | 0.99 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 |
| P1 | 100,000 | rescaled to S=0.3 | +0.300 | 1.6 | 2.2 | 0.98 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 |
| P1 | 60,000 | empirical | +0.274 | 29.3 | 40.6 | 0.52 | 0.60 | 0.81 | 0.99 | 1.00 | 1.00 | 1.00 | 1.00 |
| P1 | 60,000 | rescaled to S=0.22 | +0.220 | 457.4 | 633.6 | 0.35 | 0.24 | 0.26 | 0.31 | 0.47 | 0.63 | 0.90 | 0.99 |
| P1 | 60,000 | rescaled to S=0.25 | +0.250 | 67.7 | 93.7 | 0.47 | 0.41 | 0.57 | 0.81 | 0.97 | 1.00 | 1.00 | 1.00 |
| P1 | 60,000 | rescaled to S=0.3 | +0.300 | 14.7 | 20.4 | 0.59 | 0.83 | 0.97 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 |
| P2 | 20,000,000 | empirical | +0.448 | 0.4 | 0.6 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 |
| P2 | 20,000,000 | rescaled to S=0.22 | +0.220 | 130.7 | 181.0 | 0.23 | 0.27 | 0.43 | 0.60 | 0.83 | 0.98 | 1.00 | 1.00 |
| P2 | 20,000,000 | rescaled to S=0.25 | +0.250 | 19.3 | 26.8 | 0.63 | 0.84 | 0.95 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 |
| P2 | 20,000,000 | rescaled to S=0.3 | +0.300 | 4.2 | 5.8 | 0.98 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 |
| P2 | 250,000 | empirical | +0.132 | unreachable (S below 0.20) | unreachable | 0.01 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 |
| P2 | 250,000 | rescaled to S=0.22 | +0.220 | 443.8 | 614.7 | 0.09 | 0.13 | 0.17 | 0.28 | 0.45 | 0.66 | 0.92 | 1.00 |
| P2 | 250,000 | rescaled to S=0.25 | +0.250 | 65.6 | 90.9 | 0.28 | 0.42 | 0.66 | 0.91 | 0.99 | 1.00 | 1.00 | 1.00 |
| P2 | 250,000 | rescaled to S=0.3 | +0.300 | 14.3 | 19.8 | 0.80 | 0.98 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 |
| P2 | 100,000 | empirical | -0.015 | unreachable (S below 0.20) | unreachable | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 |
| P2 | 100,000 | rescaled to S=0.22 | +0.220 | 69.6 | 96.4 | 0.35 | 0.53 | 0.73 | 0.95 | 1.00 | 1.00 | 1.00 | 1.00 |
| P2 | 100,000 | rescaled to S=0.25 | +0.250 | 10.3 | 14.3 | 0.95 | 0.99 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 |
| P2 | 100,000 | rescaled to S=0.3 | +0.300 | 2.2 | 3.1 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 |
| P2 | 60,000 | empirical | -0.003 | unreachable (S below 0.20) | unreachable | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 |
| P2 | 60,000 | rescaled to S=0.22 | +0.220 | 126.8 | 175.7 | 0.22 | 0.30 | 0.44 | 0.68 | 0.90 | 1.00 | 1.00 | 1.00 |
| P2 | 60,000 | rescaled to S=0.25 | +0.250 | 18.8 | 26.0 | 0.76 | 0.95 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 |
| P2 | 60,000 | rescaled to S=0.3 | +0.300 | 4.1 | 5.7 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 |

Reading it:

- At the empirical `S` the criterion is satisfied by a few tens of episodes at most wherever `S`
  is clearly above 0.20 (P1 at 20,000,000, 250,000 and 100,000 with `S` = 0.32 and P2 at 20,000,000
  with `S` = 0.45 need under 10; P1 at 60,000, `S` = 0.27, needs about 30 to 40). At those n the bootstrap itself
  is not trustworthy (see the size check), so read them as "the cost criterion is not the binding
  one", not as n = 1.
- Where the true `S` is only slightly above the threshold it gets expensive: at `S` = 0.22 the
  delta-method n for 80% power runs from 44 (P1, 20,000,000) to 457 (P1, 60,000), and 444 for P2 at
  250,000. The spread comes from the per-episode variability of cost: `sigma_1` is 0.047 for P1 at
  20,000,000 and 0.189 for P2 at 250,000, and n goes with its square.
- P2 at 250,000 (empirical `S` = 0.13), 100,000 (-0.015) and 60,000 (-0.003) cannot reach 0.20:
  the compute limit caps `all_components` and `random_p050` at almost the same spend. That is a
  property of a binding budget, and it says that **`S` against a baseline that is itself capped by
  the budget is not a measure of selection**.

**T5. Size check: rejection rate when true `S` is exactly 0.20** (should be at most 0.05).

| pair | compute (ns) | n=10 | n=20 | n=40 | n=80 | n=160 | n=320 | n=640 | n=1280 |
|---|---|---|---|---|---|---|---|---|---|
| P1 | 20,000,000 | 0.100 | 0.100 | 0.070 | 0.062 | 0.062 | 0.058 | 0.058 | 0.055 |
| P1 | 250,000 | 0.120 | 0.102 | 0.080 | 0.080 | 0.055 | 0.065 | 0.068 | 0.030 |
| P1 | 100,000 | 0.130 | 0.070 | 0.080 | 0.065 | 0.085 | 0.070 | 0.052 | 0.062 |
| P1 | 60,000 | 0.350 | 0.170 | 0.128 | 0.092 | 0.070 | 0.077 | 0.072 | 0.043 |
| P2 | 20,000,000 | 0.048 | 0.062 | 0.062 | 0.052 | 0.048 | 0.068 | 0.062 | 0.062 |
| P2 | 250,000 | 0.043 | 0.048 | 0.040 | 0.033 | 0.045 | 0.052 | 0.050 | 0.058 |
| P2 | 100,000 | 0.020 | 0.035 | 0.037 | 0.037 | 0.030 | 0.030 | 0.033 | 0.052 |
| P2 | 60,000 | 0.040 | 0.033 | 0.035 | 0.015 | 0.035 | 0.052 | 0.060 | 0.052 |

At n of 160 or more the rejection rate is 0.03 to 0.085, in line with the A7b finding that the
percentile interval is anti-conservative on skewed costs (7% to 12% false exceedance at nominal
5%). At n of 10 to 40 it reaches 0.35 (P1 at 60,000). The powers in T4 are therefore for the
uncorrected interval and are somewhat optimistic; A7b's corrected interval (not yet built) will
need its own power run. The Monte Carlo standard error on a rate near 0.05 with 400 replications
is about 0.011.

## 4. What a freeze can take from this

1. **The success criterion binds, not the cost criterion.** For any true savings comfortably
   above 0.20 the cost criterion is met with tens of episodes; non-inferiority at margin 0.01
   needs from hundreds to several thousand, depending on how often the selector's decisions
   differ from the baseline's (T2).
2. **Do not freeze an n from T1.** Its non-degenerate rows are baselines being starved, or small
   discordance between two cheap arms, and none of them is a selector.
3. **Rerun this analysis on exploration data from C2's selector against the chosen baseline**
   before C3. `q` and `sigma_1` are the two numbers that matter, and both are measured in a few
   seconds from a run of that pair on seeds 1000 to 1499.

## 5. Assumptions and caveats, all of them

- The selective arm is replaced by a baseline. No variance here is a selector's.
- Paired episodes are the unit of replication; the 5,500 are exchangeable draws from one fixed
  mix of 11 classes, 500 each, from one generator with default `episode_params`. Held-out
  dependency structures (C4) may have different variance.
- One-sided alpha 0.05; power 0.8 and 0.9; margin 0.01; threshold 0.20. These are the charter's
  proposed values, not frozen ones.
- The normal approximation understates n when n is small (the package reports
  `achieved_power_t`); at the n in T1 and T2 above 60 the shortfall is small. For the planning
  table, the achieved t power at the returned n was not tabulated.
- A paired difference of success is modelled as a three-valued variable with a given discordant
  fraction and a given mean. Episodes that share a seed across classes are treated as independent;
  that was not checked.
- The success outcome is all-class success. Excluding `NoFault` raises the sd slightly (T1
  shows both).
- Modelled cost is deterministic given the episode and the arm, so the cost variance is entirely
  between-episode variance. Measured wall time was not used.
- The percentile bootstrap is the package's current interval; its size is not at its nominal
  level for small n (T5).
- The simulation uses 400 replications and 400 bootstrap resamples per cell for speed; its
  Monte Carlo error is of the order 0.01 to 0.025 in power.
- Delta-method and simulation agree where both apply (for example P1 at 60,000, `S` = 0.25:
  delta n(80%) = 68 and simulated power 0.815 at n = 80, 0.575 at n = 40), which is the only
  cross-check done.
