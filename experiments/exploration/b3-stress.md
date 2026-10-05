# B3 stress suite: every arm, every budget level, every stressor class

Status: development run, `exploration` branch. Nothing here tests a hypothesis and nothing here
may later be cited as confirmation. Failures are recorded as found. **No instrument was changed
and no arm was tuned.** Where I believe the instrument is defective, the evidence is in section 6
for the coordinator to judge.

Data: the four B1 runs (seeds 1000 to 1499, 500 episodes per class, so 500 episodes in every cell
below; run ids and hashes in `experiments/exploration/b1-variance.md`), ten arms, compute limits
20,000,000, 250,000, 100,000 and 60,000 declared ns. Full per-(arm, budget, class) numbers are in
`b1-variance.csv`. One further probe, with the generator's noise rate raised from 3 to its maximum
of 50, is described in section 5 (four more runs, labelled S3 in the B1 md, in
`b1-supplementary.csv`).

## 1. What each stressor is for, and what was read off

Charter section 10 pairs each stressor with the failure it exists to detect. The arms here have
no salience mechanism, no feedback path and no use of memory (the shared rule ignores the lookup,
`POLICIES.md` 2.5), so most of these stressors have nothing in the arms to catch. That is what
the numbers below test, not an assumption.

| Stressor class | Failure it should detect (charter 10) | What was read |
|---|---|---|
| `NoiseFlood` | attention captured by noise | success and cost against the reference class |
| `Duplicates` | redundant computation | success and cost against the reference class |
| `FeedbackBait` | components repeatedly reactivating each other | probes bought, components run, cost, against the reference class |
| `QuietUrgent` | starvation of a quiet urgent event | success, critical-miss rate (this class is generated critical), against the reference class |
| `StaleMemory` | confident reuse of invalid knowledge | wrong-declaration rate, cost, against the reference class |
| `ComponentTimeout` | unbounded waiting or budget leakage | success, abstentions, undecided rows, billed compute against the limit |

**Reference class.** There is no clean twin of any stressor class in the generator. I used
`Ambiguous` as the reference for all six. It is an exact twin only for `FeedbackBait` and
`StaleMemory` (both are "the plain ambiguous signal" plus the stressor, `DESIGN.md` section 2). For
the other four the signal differs (full signature, or one quiet message), so the reference
comparison says "the same arm, on a different class, does equally well", which is what a stressor
that bites would break. Success is compared with a Newcombe 95% interval for a difference of
proportions (500 against 500, different episodes of different classes, so unpaired); 40 cells per
stressor are compared, so about two chance exclusions per stressor would be expected if the
intervals were exact. Bold numbers in the success tables are cells whose interval excludes 0.

## 2. Results at the default noise rate (the B1 grid)

**Success (%) by stressor, per budget level.** Oracles are privileged ceilings.

### Compute limit 20,000,000 ns (never binds)

| arm | Ambiguous (reference) | NoiseFlood | Duplicates | FeedbackBait | QuietUrgent | StaleMemory | ComponentTimeout |
|---|---|---|---|---|---|---|---|
| heuristic_only | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | **87.8** |
| all_components | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 99.8 |
| random p=0.25 | 99.6 | 99.6 | 99.6 | 99.6 | 99.6 | 99.6 | 99.4 |
| random p=0.5 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 99.8 |
| fixed verifier only | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | **86.2** |
| fixed estimator only | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | **87.6** |
| fixed heuristic every 2 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | **87.8** |
| fixed heuristic every 4 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | **87.8** |
| oracle_evidence (privileged) | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 |
| oracle_immediate (privileged) | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 |

### Compute limit 250,000 ns

| arm | Ambiguous (reference) | NoiseFlood | Duplicates | FeedbackBait | QuietUrgent | StaleMemory | ComponentTimeout |
|---|---|---|---|---|---|---|---|
| heuristic_only | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | **87.8** |
| all_components | 13.8 | 13.2 | 12.6 | 13.8 | 13.4 | 13.8 | **18.4** |
| random p=0.25 | 98.8 | 98.8 | 98.8 | 98.8 | 98.8 | 98.8 | **92.6** |
| random p=0.5 | 70.2 | 69.6 | 69.0 | 70.0 | 69.8 | 70.2 | **62.2** |
| fixed verifier only | 47.2 | 46.8 | 46.6 | 47.2 | 46.6 | 47.2 | **34.0** |
| fixed estimator only | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | **81.2** |
| fixed heuristic every 2 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | **87.8** |
| fixed heuristic every 4 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | **87.8** |
| oracle_evidence (privileged) | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 |
| oracle_immediate (privileged) | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 |

### Compute limit 100,000 ns

| arm | Ambiguous (reference) | NoiseFlood | Duplicates | FeedbackBait | QuietUrgent | StaleMemory | ComponentTimeout |
|---|---|---|---|---|---|---|---|
| heuristic_only | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | **81.4** |
| all_components | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| random p=0.25 | 35.0 | 35.2 | 34.6 | 35.2 | 35.8 | 35.0 | 36.4 |
| random p=0.5 | 3.6 | 3.4 | 3.2 | 3.8 | 3.6 | 3.6 | **7.8** |
| fixed verifier only | 0.4 | 0.4 | 0.4 | 0.4 | 0.4 | 0.4 | 0.4 |
| fixed estimator only | 64.4 | 64.0 | 63.8 | 64.4 | 65.0 | 64.4 | **50.2** |
| fixed heuristic every 2 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | **86.8** |
| fixed heuristic every 4 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | **87.8** |
| oracle_evidence (privileged) | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 |
| oracle_immediate (privileged) | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 |

### Compute limit 60,000 ns

| arm | Ambiguous (reference) | NoiseFlood | Duplicates | FeedbackBait | QuietUrgent | StaleMemory | ComponentTimeout |
|---|---|---|---|---|---|---|---|
| heuristic_only | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | **76.6** |
| all_components | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| random p=0.25 | 5.0 | 5.2 | 5.0 | 5.4 | 5.0 | 5.0 | **11.2** |
| random p=0.5 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.6 |
| fixed verifier only | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| fixed estimator only | 11.4 | 11.0 | 10.8 | 11.4 | 11.8 | 11.4 | 8.4 |
| fixed heuristic every 2 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | **83.0** |
| fixed heuristic every 4 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | **87.8** |
| oracle_evidence (privileged) | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 |
| oracle_immediate (privileged) | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 |

**How the failures are made up.** "Wrong declaration % / abstained %" per cell. Every episode
ended with a declaration or an abstention: undecided is 0 in all 220,000 rows, because the final
call (A6b) turns budget exhaustion into a declaration. So **"unbounded waiting" cannot show up as
undecided in this instrument**; it shows as a wrong declaration or an abstention.

At 20,000,000:

| arm | NoiseFlood | Duplicates | FeedbackBait | QuietUrgent | StaleMemory | ComponentTimeout |
|---|---|---|---|---|---|---|
| heuristic_only | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 12.2 |
| all_components | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.2 |
| random p=0.25 | 0.4 / 0.0 | 0.4 / 0.0 | 0.4 / 0.0 | 0.4 / 0.0 | 0.4 / 0.0 | 0.4 / 0.2 |
| random p=0.5 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.2 |
| fixed verifier only | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 13.8 |
| fixed estimator only | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 12.4 |
| fixed heuristic every 2 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 12.2 |
| fixed heuristic every 4 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 12.2 |
| oracle_evidence (privileged) | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 |
| oracle_immediate (privileged) | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 |

At 250,000:

| arm | NoiseFlood | Duplicates | FeedbackBait | QuietUrgent | StaleMemory | ComponentTimeout |
|---|---|---|---|---|---|---|
| heuristic_only | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 12.2 |
| all_components | 86.8 / 0.0 | 87.4 / 0.0 | 86.2 / 0.0 | 86.6 / 0.0 | 86.2 / 0.0 | 81.4 / 0.2 |
| random p=0.25 | 1.2 / 0.0 | 1.2 / 0.0 | 1.2 / 0.0 | 1.2 / 0.0 | 1.2 / 0.0 | 7.2 / 0.2 |
| random p=0.5 | 30.4 / 0.0 | 31.0 / 0.0 | 30.0 / 0.0 | 30.2 / 0.0 | 29.8 / 0.0 | 37.6 / 0.2 |
| fixed verifier only | 53.2 / 0.0 | 53.4 / 0.0 | 52.8 / 0.0 | 53.4 / 0.0 | 52.8 / 0.0 | 52.2 / 13.8 |
| fixed estimator only | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 6.4 / 12.4 |
| fixed heuristic every 2 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 12.2 |
| fixed heuristic every 4 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 12.2 |
| oracle_evidence (privileged) | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 |
| oracle_immediate (privileged) | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 |

At 100,000:

| arm | NoiseFlood | Duplicates | FeedbackBait | QuietUrgent | StaleMemory | ComponentTimeout |
|---|---|---|---|---|---|---|
| heuristic_only | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 6.4 / 12.2 |
| all_components | 100.0 / 0.0 | 100.0 / 0.0 | 100.0 / 0.0 | 100.0 / 0.0 | 100.0 / 0.0 | 99.8 / 0.2 |
| random p=0.25 | 64.8 / 0.0 | 65.4 / 0.0 | 64.8 / 0.0 | 64.2 / 0.0 | 65.0 / 0.0 | 63.4 / 0.2 |
| random p=0.5 | 96.6 / 0.0 | 96.8 / 0.0 | 96.2 / 0.0 | 96.4 / 0.0 | 96.4 / 0.0 | 92.0 / 0.2 |
| fixed verifier only | 99.6 / 0.0 | 99.6 / 0.0 | 99.6 / 0.0 | 99.6 / 0.0 | 99.6 / 0.0 | 85.8 / 13.8 |
| fixed estimator only | 36.0 / 0.0 | 36.2 / 0.0 | 35.6 / 0.0 | 35.0 / 0.0 | 35.6 / 0.0 | 37.4 / 12.4 |
| fixed heuristic every 2 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 1.0 / 12.2 |
| fixed heuristic every 4 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 12.2 |
| oracle_evidence (privileged) | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 |
| oracle_immediate (privileged) | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 |

At 60,000:

| arm | NoiseFlood | Duplicates | FeedbackBait | QuietUrgent | StaleMemory | ComponentTimeout |
|---|---|---|---|---|---|---|
| heuristic_only | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 11.0 / 12.4 |
| all_components | 100.0 / 0.0 | 100.0 / 0.0 | 100.0 / 0.0 | 100.0 / 0.0 | 100.0 / 0.0 | 99.8 / 0.2 |
| random p=0.25 | 94.8 / 0.0 | 95.0 / 0.0 | 94.6 / 0.0 | 95.0 / 0.0 | 95.0 / 0.0 | 88.6 / 0.2 |
| random p=0.5 | 100.0 / 0.0 | 100.0 / 0.0 | 100.0 / 0.0 | 100.0 / 0.0 | 100.0 / 0.0 | 99.2 / 0.2 |
| fixed verifier only | 100.0 / 0.0 | 100.0 / 0.0 | 100.0 / 0.0 | 100.0 / 0.0 | 100.0 / 0.0 | 86.2 / 13.8 |
| fixed estimator only | 89.0 / 0.0 | 89.2 / 0.0 | 88.6 / 0.0 | 88.2 / 0.0 | 88.6 / 0.0 | 79.2 / 12.4 |
| fixed heuristic every 2 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 4.8 / 12.2 |
| fixed heuristic every 4 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 12.2 |
| oracle_evidence (privileged) | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 |
| oracle_immediate (privileged) | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 | 0.0 / 0.0 |

**Critical-miss rate (%) on `QuietUrgent`**, the only stressor class generated critical. A critical
miss is any failure on it, so the column is `100 - success` on that class; it is reported
separately only because the charter requires critical failures to be scored separately.

| arm | 20,000,000 | 250,000 | 100,000 | 60,000 |
|---|---|---|---|---|
| heuristic_only | 0.0 | 0.0 | 0.0 | 0.0 |
| all_components | 0.0 | 86.6 | 100.0 | 100.0 |
| random p=0.25 | 0.4 | 1.2 | 64.2 | 95.0 |
| random p=0.5 | 0.0 | 30.2 | 96.4 | 100.0 |
| fixed verifier only | 0.0 | 53.4 | 99.6 | 100.0 |
| fixed estimator only | 0.0 | 0.0 | 35.0 | 88.2 |
| fixed heuristic every 2 | 0.0 | 0.0 | 0.0 | 0.0 |
| fixed heuristic every 4 | 0.0 | 0.0 | 0.0 | 0.0 |
| oracle_evidence (privileged) | 0.0 | 0.0 | 0.0 | 0.0 |
| oracle_immediate (privileged) | 0.0 | 0.0 | 0.0 | 0.0 |

**How often a stressor class differs from the reference class.**

| stressor | cells compared (4 budgets x 10 arms) | success differs from Ambiguous (95% interval excludes 0) | of which lower |
|---|---|---|---|
| NoiseFlood | 40 | 0 | 0 |
| Duplicates | 40 | 0 | 0 |
| FeedbackBait | 40 | 0 | 0 |
| QuietUrgent | 40 | 0 | 0 |
| StaleMemory | 40 | 0 | 0 |
| ComponentTimeout | 40 | 22 | 19 |

**Cost and behaviour against the reference class** (modelled cost is the charter's `C`):

| stressor | cost / Ambiguous cost, range over the 8 public arms at 20,000,000 | same at 60,000 | mean probes, stressor vs Ambiguous (public arms, 20,000,000) | mean components run, stressor vs Ambiguous (public arms, 20,000,000) |
|---|---|---|---|---|
| NoiseFlood | 0.86 to 0.98 | 0.86 to 1.00 | 0.14 vs 1.97 | 49.5 vs 51.9 |
| Duplicates | 0.86 to 0.98 | 0.86 to 1.00 | 0.14 vs 1.97 | 49.5 vs 51.9 |
| FeedbackBait | 1.00 to 1.02 | 1.00 to 1.02 | 1.97 vs 1.97 | 51.9 vs 51.9 |
| QuietUrgent | 0.84 to 0.98 | 0.84 to 1.00 | 0.00 vs 1.97 | 49.3 vs 51.9 |
| StaleMemory | 1.00 to 1.00 | 1.00 to 1.00 | 1.97 vs 1.97 | 51.9 vs 51.9 |
| ComponentTimeout | 0.80 to 0.85 | 0.78 to 0.84 | 0.12 vs 1.97 | 51.0 vs 51.9 |

The probes and components columns are only comparable for `FeedbackBait` and `StaleMemory`
(exact twins). The other four classes carry a full signature or a single catalogue message that
fixes the kind, so they need no probe, and the rule buys none: that is why their probe count is
near 0 beside `Ambiguous`'s 1.97, with the same success.

## 3. Findings by stressor

1. **`NoiseFlood`, `QuietUrgent`: no effect on any arm at any level.** 0 of 40 cells each differ
   from `Ambiguous`. This tests the review log's statement (A4) that they are toothless against
   components that read the whole window, **under binding budgets**: it still holds at all three
   binding levels (250,000, 100,000, 60,000). The cells that carry information are those where an
   arm sits between the floor and the ceiling, so that a stressor could push it either way:
   `random p=0.25` at 100,000 (35.0% on `Ambiguous`, 35.2% on `NoiseFlood`, 35.8% on `QuietUrgent`),
   `fixed estimator only` at 100,000 (64.4, 64.0, 65.0), `random p=0.5` at 250,000 (70.2, 69.6, 69.8),
   `fixed verifier only` at 250,000 (47.2, 46.8, 46.6). Where arms fail on these classes they fail
   because the compute ran out before the first symptom arrived (`POLICIES.md` 8.1: at 60,000 the
   `all_components` arm decides at a mean of 0.31 s, the symptoms start about a second in), and
   they fail equally on every class. The 100% critical-miss cells in the table above are that, not starvation
   by noise.
2. **`Duplicates`: no effect at the default noise rate** (0 of 40). Cost per episode is 0.86 to
   0.98 of the reference's, not above it: the classes decide earlier, so there is no redundant
   computation to see. (With noise 50 a small effect appears, section 5.)
3. **`FeedbackBait`: no effect** (0 of 40; cost 1.00 to 1.02 of `Ambiguous`, probes 1.97 against 1.97).
   The probes bought equal `Ambiguous`'s, which is consistent with the shared rule probing only
   candidate sites (`POLICIES.md` section 2; the decoy is never a candidate), and no arm requests
   a computation because a result suggested it, so the feedback loop the class builds has nothing
   to attach to. I did not look at which services were probed.
4. **`StaleMemory`: no effect** (0 of 40; cost exactly 1.00 of `Ambiguous` for every arm). The rule
   never reads the memory lookup's output, so a wrong record cannot be reused. This is a property
   of the rule, not evidence that stale memory is harmless to a rule that used it.
5. **`ComponentTimeout`: the only stressor that bites, and it bites the arms that depend on one
   component.** 22 of 40 cells differ from `Ambiguous` (19 lower, 3 higher). At 20,000,000 every arm
   that selects one component per step loses about 12 points (heuristic only 87.8%, verifier
   only 86.2%, estimator only 87.6%), with the abstentions explaining it: when the `Fail`
   directive lands on the component the arm uses, the rule has no candidate set and abstains at
   its patience (12.2% to 13.8% abstained). Arms that run several components every step are
   protected by redundancy (`all_components` 99.8%, `random p=0.5` 99.8%).

   Success (reference class `Ambiguous` in brackets), % :

| compute (ns) | heuristic_only | all_components | random p=0.25 | random p=0.5 | fixed verifier only | fixed estimator only | fixed heuristic every 2 | fixed heuristic every 4 |
|---|---|---|---|---|---|---|---|---|
| 20,000,000 | 87.8 (100.0) | 99.8 (100.0) | 99.4 (99.6) | 99.8 (100.0) | 86.2 (100.0) | 87.6 (100.0) | 87.8 (100.0) | 87.8 (100.0) |
| 250,000 | 87.8 (100.0) | 18.4 (13.8) | 92.6 (98.8) | 62.2 (70.2) | 34.0 (47.2) | 81.2 (100.0) | 87.8 (100.0) | 87.8 (100.0) |
| 100,000 | 81.4 (100.0) | 0.0 (0.0) | 36.4 (35.0) | 7.8 (3.6) | 0.4 (0.4) | 50.2 (64.4) | 86.8 (100.0) | 87.8 (100.0) |
| 60,000 | 76.6 (100.0) | 0.0 (0.0) | 11.2 (5.0) | 0.6 (0.0) | 0.0 (0.0) | 8.4 (11.4) | 83.0 (100.0) | 87.8 (100.0) |

   At binding budgets the arms that run every step also lose to `Slow` directives, which multiply
   the declared compute of the slowed component: `heuristic_only` falls from 87.8% at 20,000,000
   to 81.4% at 100,000 and 76.6% at 60,000, while `fixed heuristic every 4` stays at 87.8%. That is
   budget leakage being visible (`Slow` is described in `HARNESS.md` section 5; I did not isolate
   its effect from the rest of the directive mix). Three cells go the other way (`all_components` at 250,000 18.4%
   against 13.8%; `random p=0.5` at 100,000 7.8% against 3.6%; `random p=0.25` at 60,000 11.2%
   against 5.0%): a component that fails produces no output, and at these budgets an output of the
   verifier on a window with nothing in it is what makes the rule declare "no fault" and lose, so
   losing a component can help. This is a property of the shared rule's source order (the
   verifier's set is read first, `POLICIES.md` section 2) and the final declaration, not a
   discovery about components. It is not verified here beyond the pattern.

## 4. Hard limits held

Hard resource limits were enabled in every arm and every run. The largest declared compute billed
in any episode of any arm, over all 11 classes:

| compute limit (ns) | largest declared compute billed in any episode of any arm, all 11 classes |
|---|---|
| 20,000,000 | 1,551,884 |
| 250,000 | 250,000 |
| 100,000 | 100,000 |
| 60,000 | 60,000 |

At every binding level the largest bill equals the limit and none exceeds it. At 20,000,000 the
largest bill is 1.55 million, 8% of the limit, so that level never binds.

## 5. Does the suite bite at all? A noise-rate probe (supplementary, S3)

At the default noise rate of 3 the streams are small (the drift workload's note in `HARNESS.md`
says fewer than 100 observations at the default rate; I did not measure the largest, and the
flood classes raise their own rate), so I would not expect noise to push the signal out of the
256-observation working-state window. The generator
allows a noise rate of 50, set in the manifest's `episode_params` (a parameter, not a code
change). I ran the same ten arms, same seeds, same four budgets at noise rate 50 (four runs,
`b3n50-*`, `b1-supplementary.csv`). The reference class is `Ambiguous` at noise 50, so the
comparison is again the stressor against the plain class under the same noise. This is an
exploratory probe, not part of the specified B3.

| stressor | cells compared | success differs from Ambiguous (95% interval excludes 0) | of which lower |
|---|---|---|---|
| NoiseFlood | 40 | 0 | 0 |
| Duplicates | 40 | 4 | 4 |
| FeedbackBait | 40 | 0 | 0 |
| QuietUrgent | 40 | 0 | 0 |
| StaleMemory | 40 | 0 | 0 |
| ComponentTimeout | 40 | 22 | 19 |

`NoiseFlood`, `QuietUrgent`, `FeedbackBait` and `StaleMemory` still show no effect (0 of 40
each). `ComponentTimeout` is unchanged (22 of 40). `Duplicates` now differs in 4 cells, all
lower:

| compute (ns) | arm | stressor | success | Ambiguous success | difference, 95% interval |
|---|---|---|---|---|---|
| 250,000 | random p=0.25 | Duplicates | 95.0 | 97.6 | -2.6 [-5.1, -0.2] |
| 250,000 | random p=0.5 | Duplicates | 59.6 | 66.0 | -6.4 [-12.3, -0.4] |
| 100,000 | fixed estimator only | Duplicates | 49.2 | 58.4 | -9.2 [-15.3, -3.0] |
| 60,000 | heuristic_only | Duplicates | 74.4 | 95.6 | -21.2 [-25.5, -17.0] |

One of those four is large and clearly outside noise (`heuristic_only` at 60,000: 74.4% against
95.6%); the other three have intervals whose upper limits are within 0.4 points of 0. With 40
comparisons per stressor, one to three of the marginal ones could be chance, and I did not
adjust for multiplicity. In the large cell the arm reaches its compute limit in 25.8% of
`Duplicates` episodes against 20.6% of `Ambiguous` ones (`final_declaration` stops), and its
decisions come earlier (1.71 s against 1.85 s), so the loss is not obviously "more computation":
I did not establish the mechanism. What the probe does show is that at noise 50 the arms'
cost does rise (for example `heuristic_only` on `Ambiguous` at 20,000,000: 40.7 thousand ns at
noise 3, 51.2 thousand at noise 50; its success at 60,000 on `Ambiguous` falls from 100% to
95.6%), so the generator can put pressure on a budget; the stressor classes still do not
separate from the plain class, except `Duplicates` in a corner.

## 6. Failures recorded and suspected instrument defects

None of these was fixed. Items 1 to 4 are for the coordinator to judge as defects or as
properties; items 5 to 7 are failures of arms.

1. **Five of the six stressors cannot fail any current arm at the default noise rate (suspected:
   the suite does not test what the charter says it tests).** 0 of 40 cells differ for each of
   `NoiseFlood`, `Duplicates`, `FeedbackBait`, `QuietUrgent`, `StaleMemory`, at every budget, for
   every arm. At the maximum noise rate the same holds for four of the five; `Duplicates` bites
   in four of 40 cells (section 5). Catalogue ids make noise separable by anyone who reads the rules
   (`gordian-world/DESIGN.md` section 5.2), the shared rule never reads memory, and no arm requests
   computations from results. The charter's "survives the stress suite" criterion for EXP-001
   would be met by any arm that reads the whole window, which is true of every baseline. If a
   selective arm's attention is to be tested, the stressors need a mechanism that costs attention
   (a larger window, a per-observation fee, or a policy that scores observations), and that is a
   world revision, not something to change inside a frozen experiment.
2. **`undecided` is structurally 0.** Because of the final call, an arm that waits forever is
   scored as a declaration. `ComponentTimeout`'s "unbounded waiting" is therefore invisible as a
   stop reason (all 220,000 rows are `terminal` or `final_declaration`). It is visible only as
   abstentions on the single-component arms.
3. **The shared rule's source order lets a failed component help** (section 3, item 5, last
   paragraph). It means that a stress class that removes a component can raise an arm's score. I
   have not confirmed the mechanism by experiment.
4. **`QuietUrgent`, the only generated-critical stressor, adds nothing to `Ambiguous` or
   `CriticalFault`**: its critical-miss rate is the arm's general failure rate on a faulted class
   (table in section 2), and `CriticalFault` shows the same numbers (`b1-variance.csv`). The
   charter's separate critical-failure bound is therefore not exercised by anything specific to
   quiet signals.
5. **Arms that depend on one component lose 12 points or more on `ComponentTimeout`**: 87.8%
   for the heuristic family at 20,000,000 and 250,000, down to 76.6% for `heuristic_only` at 60,000;
   86.2% and 87.6% for verifier only and estimator only at 20,000,000 (at binding budgets those two
   fail on every class for budget reasons).
6. **At the three binding budgets the arms that run every step collapse on every faulted class**
   (`all_components`: 13.8%, 0.0%, 0.0% on `Ambiguous`; its `QuietUrgent` critical-miss 86.6%,
   100%, 100%), for the budget-exhaustion reason in `POLICIES.md` 8.1; this is a failure of the
   arm at that budget, not of a stressor.
7. **`random p=0.25` has a 0.4% critical miss at 20,000,000** on `QuietUrgent` and its other
   classes alike (a random arm that sometimes selects nothing in the step that matters); no other
   arm has any critical miss at that level.

## 7. Not done, and what I am least sure of

- No controlled clean twin of any stressor class exists, so "differs from `Ambiguous`" mixes the
  stressor with the class's other differences (fault kind mix, signal shape). The 0 of 40 results
  are robust to that (nothing differs); the 22 of 40 for `ComponentTimeout` is attributed to the
  directives from the abstentions, which match the single-component arms' `Fail` exposure, but
  the directive set per episode was not examined.
- 40 comparisons per stressor with no multiplicity adjustment. The conclusions that rest on
  "nothing differs" are not affected; the `Duplicates` result at noise 50 is.
- Arms are baselines with no mechanism that a stressor targets. Failing to catch a selector is
  not shown: the selector does not exist.
- Wall time was not used anywhere in this document.
