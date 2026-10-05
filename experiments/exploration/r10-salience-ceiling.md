# R10: the salience ceiling, what perfect noticing is worth

Status: exploration. Nothing here tests a hypothesis, and nothing may later be cited as
confirmation. Written by the R10 worker on branch `salience-ceiling`; the coordinator reviews it.

R10 asks what `oracle_notice_privileged` buys over `oracle_selection_privileged`: the same
escalation of exactly the hard incidents, once each, at the delay R5 tuned, with the same context
builder, plus one privilege, noticing. The criterion was fixed by the coordinator before any R10
code or run (`docs/local-test-plan.md`, 5R, R10) and is applied unchanged.

## 1. Verdict as written

Primary setting (b = 5, rho = 0.7), the rung's own context, 200 held-out streams (seeds
20000-20199), paired 90% cluster bootstrap over streams (10,000 resamples, seed 9950):

| result | notice | selection | difference [90% paired cluster bootstrap] | needs | verdict |
|---|---|---|---|---|---|
| 1: hard incidents, slow leak excluded | 218/372 = 0.586 | 182/372 = 0.489 | +0.097 [+0.062, +0.132] | point >= 0.03, lower bound > 0.01 | holds |
| 2: slow leak | 130/139 = 0.935 | 26/139 = 0.187 | +0.748 [+0.678, +0.813] | point >= 0.2, lower bound > 0.1 | holds |

- **Result 1, salience headroom on hard incidents (slow leak excluded): holds.** +0.097
  [+0.062, +0.132] against a margin of 0.03 and a lower bound that had to exceed 0.01.
- **Result 2, salience headroom on the slow leak: holds.** +0.748 [+0.678, +0.813] against a margin
  of 0.20 and a lower bound that had to exceed 0.10.

Each is its own result; neither is an "either". Sensitivity (b = 2.5 and b = 8, rho = 0.7) and the
secondary pairing (`window` 40 s, N 256), the same two results in every cell:

| pairing | setting (b, rho) | result | notice | selection | difference [90%] | verdict |
|---|---|---|---|---|---|---|
| rung's own context | b5-rho0.7 (primary) | 1: hard, leak excluded | 0.586 (218/372) | 0.489 (182/372) | +0.097 [+0.062, +0.132] | holds |
| rung's own context | b5-rho0.7 (primary) | 2: slow leak | 0.935 (130/139) | 0.187 (26/139) | +0.748 [+0.678, +0.813] | holds |
| rung's own context | b2.5-rho0.7 | 1: hard, leak excluded | 0.272 (101/372) | 0.247 (92/372) | +0.024 [-0.013, +0.059] | does not hold |
| rung's own context | b2.5-rho0.7 | 2: slow leak | 0.396 (55/139) | 0.072 (10/139) | +0.324 [+0.252, +0.398] | holds |
| rung's own context | b8-rho0.7 | 1: hard, leak excluded | 0.793 (295/372) | 0.694 (258/372) | +0.099 [+0.063, +0.135] | holds |
| rung's own context | b8-rho0.7 | 2: slow leak | 1.000 (139/139) | 0.223 (31/139) | +0.777 [+0.707, +0.840] | holds |
| window 40 s, N 256 | b5-rho0.7 | 1: hard, leak excluded | 0.952 (354/372) | 0.796 (296/372) | +0.156 [+0.119, +0.194] | holds |
| window 40 s, N 256 | b5-rho0.7 | 2: slow leak | 0.906 (126/139) | 0.288 (40/139) | +0.619 [+0.539, +0.696] | holds |
| window 40 s, N 256 | b2.5-rho0.7 | 1: hard, leak excluded | 0.538 (200/372) | 0.454 (169/372) | +0.083 [+0.041, +0.124] | holds |
| window 40 s, N 256 | b2.5-rho0.7 | 2: slow leak | 0.460 (64/139) | 0.137 (19/139) | +0.324 [+0.254, +0.393] | holds |
| window 40 s, N 256 | b8-rho0.7 | 1: hard, leak excluded | 0.997 (371/372) | 0.841 (313/372) | +0.156 [+0.125, +0.187] | holds |
| window 40 s, N 256 | b8-rho0.7 | 2: slow leak | 0.993 (138/139) | 0.295 (41/139) | +0.698 [+0.627, +0.767] | holds |

At b = 2.5 result 1 with the rung's own context does **not** hold: +0.024 [-0.013, +0.059], a point
estimate under the margin and an interval that reaches below zero. That is "not shown here", not
"equivalent to zero": the interval also contains gains of 0.05. At b = 2.5 even R4's oracle answers
only 0.538 of hard incidents, so the reasoner, not noticing, caps what any arm can gain there.
Everything else holds in every cell, including result 1 at b = 2.5 with the `window` builder.

## 2. What was built, and the readings

`oracle_notice` is in `crates/gordian-run/src/stream/oracle.rs` beside the other privileged arms; its
truth reaches it through the plan only, and it reads `PlanIncident::hard` and `PlanIncident::first`
and no other field of it (a test builds plans that differ in every other field and checks the arm
does not move). Behaviour, pinned by `crates/gordian-run/tests/stream_r10.rs`:

- For every hard incident it records a notice anchored at the incident's first observation at the
  first step that observation is delivered, `delay_ns` later asks the reasoner once about it, with
  the context the rung's configured builder makes for that anchor and site at the instant of the
  call. The rung's own notices cause no call. With no hard incident it is `never_escalate`
  (byte-equal row). It builds no context of its own and reads no decisive label.
- `Rung::context_at` is new: it applies the same function `Rung::context` applies to a noticed
  anomaly, to an observation the rung did not notice. `Rung::context` now calls the same code; a test
  checks `context_at(anchor)` equals `context(anomaly)` for every noticed anomaly on a real stream, for
  all four builders. `DirectCtx` gained a field, `contexts`, the only way a direct request reaches it.

Readings the plan left open (each fixed in the committed scripts or the arm before any held-out
number was seen):

1. **"Injects a noticed anomaly".** The injected notice is the arm's own record, not an anomaly of the
   shared rung. Injecting a real rung anomaly would change what the rung attaches, reviews and
   retires (the rung's attach rule routes later observations to any anomaly at the site, and a
   rung anomaly with no later abnormal observation retires after 6 s, before a 16 s delay). A
   record that lives until its call keeps "exactly the hard incidents, once each" true. The cost of
   this reading: where the rung noticed an incident and the selection oracle's anomaly retired before
   the delay (2 non-leak and 2 leak incidents among 270 in the diagnostic streams), the selection oracle
   makes no call and the notice oracle does.
2. **"The default hold on declaration".** `holds` is not overridden. The default holds a rung
   declaration while a call about that anomaly is in flight, and a direct call is not about a noticed
   anomaly, so it holds nothing. For the selection oracle the rung's declaration is usually made
   (3 s patience) before the 16 s call, so the hold rarely binds; the difference is not measured.
3. **"At the step that observation is delivered".** The first step whose `direct` hook runs after the
   observation was delivered. A step on which the bill refuses the arm's declared bookkeeping runs no hook,
   so a notice could only be late; the largest `bill_compute` of any run is 0.1% of its limit, so no step
   was refused. In the ledgers every notice-oracle call is 16 s after its incident's first observation
   plus at most 0.4998 s (one step).
4. **Delay.** R5's tuned 16 s at b = 5, 2.5 and 8 (rho = 0.7), the same for the slow leak.
5. **Bootstrap.** Whole streams resampled as clusters, the same multinomial counts for both arms and
   both results (`numpy.random.default_rng(9950)`, chunks of 500), 5th (`lower`) and 95th (`higher`)
   percentile; "at least" is tested on the point estimate, "lower bound above" strictly.
6. **Quality** is the pooled fraction of hard incidents declared correctly by their deadline; slow-leak
   quality is the same over the slow-leak family alone.
7. **Pairings.** Comparison arm `sel_rung_privileged` / treatment `notice_rung_privileged`, and
   `sel_win_w40_n256_privileged` / `notice_win_w40_n256_privileged` (R6's arm names; the comparison arms
   are byte-equal to R6's own arms, section 3). The b = 2.5 and 8 windows use the same builder.
8. **Notice, never noticed, late noticed, mode, false notice** are defined in
   `scripts/r10_notices.py`. An incident is noticed when the rung noticed an anomaly whose *anchor*
   belongs to it, which is what the selection oracle acts on. Mode is the evaluator's
   `contradicts_early` label (a hard incident's first moments already break the first world's rules)
   or its negation (they imitate a plain incident). A false notice is an anomaly anchored on
   background or on a plain incident, as the plan words it; anomalies anchored on a decoy are reported apart.
9. **"From the ledgers".** The ledger holds every observation and decision but not the rung's
   notices. They were re-derived by playing the same rung, in Rust (`crates/gordian-run/examples/r10_notices.rs`),
   over the same streams with a rule that escalates nothing, and checked against the ledgers and the
   arms' own `results.csv` (section 3). Ledgers were kept for all 100 diagnostic streams.
10. **Threshold sweep values.** The notice score is a z-score of the abnormal count in an 8 s window
    (about 0.8 expected), so the default of 3 needs five abnormal observations; the three values below it,
    fixed before any held-out run, are 2.0, 1.0 and 0.5, which need about four, three and two. They
    are on the 200 held-out streams at b = 5, rho = 0.7, `oracle_selection` with the rung's context
    at the 16 s delay (not re-tuned per threshold).

## 3. Provenance and the regression check

Source revision of every run: the commit that added the arm (see the commit list below); only
documentation and analysis files changed after it. Every run went through `scripts/run-driver.sh`
and exited 0; none failed, timed out, was refused or excluded. Nothing was built while a run was in
progress. The core-sharing check before every build and test (`pgrep` for a running `gordian-run`)
never found one, so I never had to wait; `r10_run.sh` also waits for a `cargo` or `rustc` process
before a run and logged no wait. One of the other worker's `cargo build` processes was visible when
I started, before any build of mine; I did not coordinate with its builds beyond that check.

| run | arms | streams | seeds | b, rho | notice z | ledgers kept | driver exit | wall s | peak MB | oom |
|---|---|---|---|---|---|---|---|---|---|---|
| `r10-diag-b5-rho0.7` | 3 | 100 | 32000-32099 | 5, 0.7 | 3 | 1 | 0 | 17 | 753 | 0 |
| `r10-heldout-b2.5-rho0.7` | 6 | 200 | 20000-20199 | 2.5, 0.7 | 3 | 0 | 0 | 18 | 14 | 0 |
| `r10-heldout-b5-rho0.7` | 6 | 200 | 20000-20199 | 5, 0.7 | 3 | 0 | 0 | 16 | 14 | 0 |
| `r10-heldout-b8-rho0.7` | 6 | 200 | 20000-20199 | 8, 0.7 | 3 | 0 | 0 | 18 | 15 | 0 |
| `r10-sweep-z0.5-b5-rho0.7` | 1 | 200 | 20000-20199 | 5, 0.7 | 0.5 | 0.1 | 0 | 5 | 74 | 0 |
| `r10-sweep-z1-b5-rho0.7` | 1 | 200 | 20000-20199 | 5, 0.7 | 1 | 0.1 | 0 | 5 | 69 | 0 |
| `r10-sweep-z2-b5-rho0.7` | 1 | 200 | 20000-20199 | 5, 0.7 | 2 | 0.1 | 0 | 4 | 66 | 0 |
| `xcheck-r6-heldout-b5-rho0.7` | 62 | 200 | 20000-20199 | 5, 0.7 | 3 | 0 | 0 | 186 | 103 | 0 |

**Regression (R6's held-out manifest, b = 5, rho = 0.7).** Replayed with this branch's binary, run id
kept, `source_revision` the only change, into a separate directory: 62 arms replayed; `results.csv` identical for 62, `incidents.csv` identical for 62. Hashes
against `r6-results-sha256.csv`: `r10-regression.csv`. Also, the arms R10 shares with R6
(`sel_rung`, `sel_win_w40_n256`, R4's oracle, `never_escalate`) write the same `results.csv` and
`incidents.csv` as R6's own files at the same setting once the `run_id` column is dropped
(`r10-vs-r6.csv`; R6 did not carry the `window` 40 s / 256 configuration at b = 2.5, so that cell is empty).
The held-out incidents are identical in all three settings. Notice-instant validation against the
ledgers: every one of the 215 calls of `oracle_selection` in the diagnostic ledgers is 16 s after the
notice the dump records for its anchor (to 7 microseconds), every one of the 270 calls of
`oracle_notice` is 16 s after its incident's first observation (to 0.4998 s, one step), the dump's
`anomalies_noticed` equals `never_escalate`'s and `oracle_notice`'s `results.csv` for all 200 held-out
streams, and `oracle_selection`'s in 199 (one stream differs by one anomaly: a call in flight keeps
its anomaly from retiring). `r10-notice-validation.csv`.

## 4. Every row, beside the criterion

Never folded into it. Primary setting, then the sensitivity settings. R4's oracle is a reference row
(its context is the decisive evidence and it asks when that has arrived: three privileges). Plain
accuracy is the same for every arm of a run because no arm escalates a plain incident; the slow
leak and the hard-incident columns are the criterion's.

b = 5, rho = 0.7:

| arm | hard quality (excl. leak) | slow-leak quality | plain accuracy | critical misses (hard + plain) | calls / stream | refs / call | cost s / stream | false alarms / stream | wrong declarations / stream | calls refused |
|---|---|---|---|---|---|---|---|---|---|---|
| `sel_rung_privileged` | 0.489 | 0.187 | 0.741 | 282 (99 + 183) | 2.10 | 43.3 | 0.66 | 5.52 | 6.59 | 0 |
| `notice_rung_privileged` | 0.586 | 0.935 | 0.741 | 229 (46 + 183) | 2.56 | 36.9 | 0.73 | 5.52 | 6.43 | 0 |
| `sel_win_w40_n256_privileged` | 0.796 | 0.288 | 0.741 | 255 (72 + 183) | 2.10 | 251.1 | 2.84 | 5.52 | 6.09 | 0 |
| `notice_win_w40_n256_privileged` | 0.952 | 0.906 | 0.741 | 194 (11 + 183) | 2.56 | 249.6 | 3.44 | 5.52 | 5.95 | 0 |
| `oracle_escalation_privileged` | 0.952 | 0.906 | 0.741 | 194 (11 + 183) | 2.56 | 5.1 | 0.32 | 5.52 | 4.29 | 0 |
| `never_escalate` | 0.000 | 0.000 | 0.741 | 338 (155 + 183) | 0.00 | - | 0.00 | 5.52 | 5.88 | 0 |

b = 2.5, rho = 0.7:

| arm | hard quality (excl. leak) | slow-leak quality | plain accuracy | critical misses (hard + plain) | calls / stream | refs / call | cost s / stream | false alarms / stream | wrong declarations / stream | calls refused |
|---|---|---|---|---|---|---|---|---|---|---|
| `sel_rung_privileged` | 0.247 | 0.072 | 0.741 | 311 (128 + 183) | 2.10 | 43.3 | 0.66 | 5.52 | 7.03 | 0 |
| `notice_rung_privileged` | 0.272 | 0.396 | 0.741 | 291 (108 + 183) | 2.56 | 36.9 | 0.73 | 5.52 | 7.27 | 0 |
| `sel_win_w40_n256_privileged` | 0.454 | 0.137 | 0.741 | 292 (109 + 183) | 2.10 | 251.1 | 2.84 | 5.52 | 6.75 | 0 |
| `notice_win_w40_n256_privileged` | 0.538 | 0.460 | 0.741 | 258 (75 + 183) | 2.56 | 249.6 | 3.44 | 5.52 | 6.95 | 0 |
| `oracle_escalation_privileged` | 0.538 | 0.432 | 0.741 | 275 (92 + 183) | 2.56 | 5.1 | 0.32 | 5.52 | 5.39 | 0 |
| `never_escalate` | 0.000 | 0.000 | 0.741 | 338 (155 + 183) | 0.00 | - | 0.00 | 5.52 | 5.88 | 0 |

b = 8, rho = 0.7:

| arm | hard quality (excl. leak) | slow-leak quality | plain accuracy | critical misses (hard + plain) | calls / stream | refs / call | cost s / stream | false alarms / stream | wrong declarations / stream | calls refused |
|---|---|---|---|---|---|---|---|---|---|---|
| `sel_rung_privileged` | 0.694 | 0.223 | 0.741 | 267 (84 + 183) | 2.10 | 43.3 | 0.66 | 5.52 | 6.26 | 0 |
| `notice_rung_privileged` | 0.793 | 1.000 | 0.741 | 205 (22 + 183) | 2.56 | 36.9 | 0.73 | 5.52 | 6.10 | 0 |
| `sel_win_w40_n256_privileged` | 0.841 | 0.295 | 0.741 | 250 (67 + 183) | 2.10 | 251.1 | 2.84 | 5.52 | 5.99 | 0 |
| `notice_win_w40_n256_privileged` | 0.997 | 0.993 | 0.741 | 185 (2 + 183) | 2.56 | 249.6 | 3.44 | 5.52 | 5.81 | 0 |
| `oracle_escalation_privileged` | 1.000 | 1.000 | 0.741 | 183 (0 + 183) | 2.56 | 5.1 | 0.32 | 5.52 | 4.14 | 0 |
| `never_escalate` | 0.000 | 0.000 | 0.741 | 338 (155 + 183) | 0.00 | - | 0.00 | 5.52 | 5.88 | 0 |

Reading of the rows:

- At the primary setting noticing alone with the `window` builder reaches 0.952, R4's oracle's
  quality, at 3.44 s a stream against the oracle's 0.32 s (the oracle's context is 5 references, the
  window's is 250). Hard-incident critical misses fall from 72 to 11 with the window and from 99 to 46
  with the rung's context.
- The notice arm makes 0.46 more calls a stream than the selection oracle (2.56 against 2.10): the
  incidents the rung never noticed. Its cost rises by 0.07 s a stream with the rung's context.
- No call was refused for budget in any row. False alarms, plain accuracy and plain critical misses
  are the same across the four R10 arms because none escalates a plain incident or a decoy.
- The notice arm's slow-leak quality at the primary setting, 0.935, is above R4's oracle's 0.906. R4's
  oracle asks once, when the last decisive observation has been delivered; the notice arm asks at 16 s after
  the leak's first observation. The slow leak is therefore an incident where *when* to ask matters as well as
  *whether*, and R10 does not separate the two; the selection oracle's 0.187 mixes both.

## 5. Where the gain on result 1 comes from (diagnostic run, 100 streams)

Seeds 32000-32099, ledgers kept, rung's own context, b = 5, rho = 0.7. The same incidents under
`oracle_selection` and `oracle_notice`, split by whether the rung ever noticed the incident (an anomaly
anchored on one of its observations). "Noticed called" is where the selection oracle made its call;
"noticed uncalled" is where the anomaly retired before the delay. Gains are in units of the quality
measure (divided by all incidents of the row's kind); intervals are paired cluster bootstraps over
the 100 streams (seed 9950). **A decomposition read off a smaller set of streams than the
criterion's: it is a mechanism check, not a second test.**

| incidents | group | n | selection correct | notice correct | gain (incidents) | gain in quality [90%] | R4 oracle correct |
|---|---|---|---|---|---|---|---|
| hard incidents, slow leak excluded | all | 190 of 190 | 82 | 96 | 14 | +0.074 [+0.021, +0.127] | 170 |
| hard incidents, slow leak excluded | never noticed | 20 of 190 | 0 | 6 | 6 | +0.032 [+0.012, +0.052] | 19 |
| hard incidents, slow leak excluded | noticed called | 168 of 190 | 82 | 89 | 7 | +0.037 [-0.011, +0.085] | 149 |
| hard incidents, slow leak excluded | noticed uncalled | 2 of 190 | 0 | 1 | 1 | +0.005 [+0.000, +0.015] | 2 |
| hard incidents, slow leak excluded | noticed | 170 of 190 | 82 | 90 | 8 | +0.042 [-0.006, +0.090] | 151 |
| slow leak | all | 80 of 80 | 13 | 76 | 63 | +0.787 [+0.691, +0.872] | 73 |
| slow leak | never noticed | 40 of 80 | 0 | 38 | 38 | +0.475 [+0.386, +0.558] | 38 |
| slow leak | noticed called | 38 of 80 | 13 | 36 | 23 | +0.287 [+0.205, +0.374] | 34 |
| slow leak | noticed uncalled | 2 of 80 | 0 | 2 | 2 | +0.025 [+0.000, +0.056] | 1 |
| slow leak | noticed | 40 of 80 | 13 | 38 | 25 | +0.312 [+0.233, +0.396] | 35 |

- On 190 non-leak hard incidents the gain is 14 (+0.074 [+0.021, +0.127], against +0.097 on the held-out
  streams; the intervals overlap). **Never-noticed incidents carry 6 of the 14**, late-noticed ones 8. The
  late-noticed part has an interval that includes zero (+0.042 [-0.006, +0.090]); the never-noticed part's
  does not (+0.032 [+0.012, +0.052]). With 14 incidents the split between the two is not resolved
  to better than that.
- Perfect noticing converts only 6 of the 20 never-noticed non-leak incidents. R4's oracle answers 19 of
  them, so the other 14 are a context problem (the rung's context at the first observation's site
  does not hold the decisive evidence), not a noticing one. R6's 0.091 "if every one were answered" is
  an upper bound the rung's context does not approach.
- On the slow leak, 40 of 80 incidents are never noticed and perfect noticing answers 38 of them
  (+0.475 of the +0.787). The other 40 are noticed late (median 17.8 s after the first observation; the
  selection oracle's delay then runs from there) and the notice arm gains 25 of them (+0.312).

### Notice latency, by family and mode (diagnostic streams)

Latency is the rung's first notice of the incident minus the incident's first observation. The
held-out streams, at the default threshold, are in `r10-notice-latency.csv` (source `heldout`) and
below.

| family | mode | hard incidents | noticed | never noticed | latency p25 / median / p75 / max (s) | median from onset (s) | anchored at the first observation |
|---|---|---|---|---|---|---|---|
| compound | contradicts_early | 23 | 21 | 2 | 0.3 / 2.3 / 4.2 / 8.0 | 2.3 | 20 |
| compound | mimics_plain | 29 | 28 | 1 | 3.3 / 5.2 / 6.1 / 15.4 | 5.2 | 24 |
| compound | ALL | 52 | 49 | 3 | 1.3 / 3.7 / 5.5 / 15.4 | 3.7 | 44 |
| cascade | contradicts_early | 39 | 36 | 3 | 0.5 / 2.1 / 4.7 / 27.4 | 2.1 | 35 |
| cascade | mimics_plain | 37 | 33 | 4 | 0.4 / 1.1 / 4.9 / 21.1 | 1.1 | 33 |
| cascade | ALL | 76 | 69 | 7 | 0.4 / 1.9 / 4.9 / 27.4 | 1.9 | 68 |
| split_brain | contradicts_early | 30 | 28 | 2 | 1.3 / 4.8 / 7.1 / 73.6 | 4.8 | 24 |
| split_brain | mimics_plain | 32 | 24 | 8 | 2.7 / 5.1 / 7.4 / 45.8 | 5.1 | 23 |
| split_brain | ALL | 62 | 52 | 10 | 1.5 / 5.1 / 7.4 / 73.6 | 5.1 | 47 |
| slow_leak | ALL | 80 | 40 | 40 | 15.3 / 17.8 / 23.7 / 59.6 | 19.0 | 0 |
| ALL | contradicts_early | 92 | 85 | 7 | 0.5 / 2.5 / 5.5 / 73.6 | 2.5 | 79 |
| ALL | mimics_plain | 98 | 85 | 13 | 1.1 / 4.1 / 6.1 / 45.8 | 4.1 | 80 |
| ALL | ALL | 270 | 210 | 60 | 1.4 / 4.9 / 13.0 / 73.6 | 4.9 | 159 |

Held-out streams (200), default threshold:

| family | mode | hard incidents | noticed | never noticed | latency p25 / median / p75 / max (s) | median from onset (s) |
|---|---|---|---|---|---|---|
| compound | contradicts_early | 71 | 65 | 6 | 0.3 / 0.5 / 3.1 / 16.3 | 0.5 |
| compound | mimics_plain | 48 | 45 | 3 | 2.2 / 3.8 / 6.7 / 29.9 | 3.8 |
| compound | ALL | 119 | 110 | 9 | 0.4 / 2.4 / 4.6 / 29.9 | 2.4 |
| cascade | contradicts_early | 47 | 41 | 6 | 0.3 / 2.7 / 4.7 / 14.3 | 2.7 |
| cascade | mimics_plain | 75 | 68 | 7 | 0.4 / 1.9 / 4.0 / 27.5 | 1.9 |
| cascade | ALL | 122 | 109 | 13 | 0.4 / 1.9 / 4.4 / 27.5 | 1.9 |
| split_brain | contradicts_early | 63 | 59 | 4 | 1.5 / 4.2 / 6.4 / 48.4 | 4.2 |
| split_brain | mimics_plain | 68 | 63 | 5 | 2.4 / 4.2 / 6.5 / 62.9 | 4.2 |
| split_brain | ALL | 131 | 122 | 9 | 2.2 / 4.2 / 6.5 / 62.9 | 4.2 |
| slow_leak | ALL | 139 | 64 | 75 | 14.0 / 16.1 / 18.1 / 52.7 | 17.5 |
| ALL | contradicts_early | 181 | 165 | 16 | 0.4 / 2.6 / 4.8 / 48.4 | 2.6 |
| ALL | mimics_plain | 191 | 176 | 15 | 1.0 / 3.3 / 5.7 / 62.9 | 3.3 |
| ALL | ALL | 511 | 405 | 106 | 0.7 / 3.8 / 7.6 / 62.9 | 3.8 |

### Never-noticed incidents, by family and mode

Counts of hard incidents with no anomaly anchored on them. Slow-leak incidents have no mode.

| family | mode | diag | heldout |
|---|---|---|---|
| cascade | contradicts_early | 3 | 6 |
| cascade | mimics_plain | 4 | 7 |
| compound | contradicts_early | 2 | 6 |
| compound | mimics_plain | 1 | 3 |
| slow_leak | unknown | 40 | 75 |
| split_brain | contradicts_early | 2 | 4 |
| split_brain | mimics_plain | 8 | 5 |

The full list is `r10-never-noticed.csv`. **Where their observations went**
(`r10-never-noticed-anchors-summary.csv`, supplementary): every one of the never-noticed hard
incidents, in both sets of streams, has abnormal observations held by an anomaly the rung did notice, but
anchored on something else. For the three burst families that anchor is background (a stray or blip at
the same site a median 0.3 s before the incident's first observation) in most cases, or a plain
incident, and for the slow leak it is background a median 6.4 to 6.9 s *after* its first observation.
"Never
noticed" in this sense is therefore not the rung failing to see activity: it is the rung anchoring
what it saw on the wrong observation, and then being asked about the wrong anchor. (Labels read
the way the evaluator reads them; no arm sees them.)

## 6. The public threshold sweep

Would a lower public notice threshold close the gap? `oracle_selection` with the rung's context at
the 16 s delay, b = 5, rho = 0.7, the 200 held-out streams, one run per threshold; the default is the
primary held-out run's `sel_rung_privileged`. Notices are counted from the replay of the same rung
(section 3); false notices are anomalies anchored on background or on a plain incident, per the plan.
Most of those are not false in the ordinary sense: at the default, 4.0 a stream are anchored on
background and 19.4 on plain incidents the cheap rung is meant to handle.

| notice z | hard incidents noticed (all / non-leak / leak) | anomalies / stream | false notices / stream (background + plain) | on decoys / stream |
|---|---|---|---|---|
| 3 (default) | 0.793 (405/511) / 0.917 (341/372) / 0.460 (64/139) | 27.55 | 23.41 | 1.88 |
| 2 | 0.814 (416/511) / 0.946 (352/372) / 0.460 (64/139) | 34.76 | 29.91 | 2.27 |
| 1 | 0.810 (414/511) / 0.941 (350/372) / 0.460 (64/139) | 58.55 | 52.77 | 2.65 |
| 0.5 | 0.793 (405/511) / 0.927 (345/372) / 0.432 (60/139) | 87.75 | 81.89 | 2.72 |

| notice z | hard quality (excl. leak) | slow-leak quality | plain accuracy | critical misses | calls / stream | refs / call | cost s / stream | false alarms / stream | calls refused | quality minus default [90%] | leak minus default [90%] |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 3 (default) | 0.489 | 0.187 | 0.741 | 282 | 2.10 | 43.3 | 0.66 | 5.52 | 0 | - | - |
| 2 | 0.527 | 0.216 | 0.807 | 228 | 2.16 | 39.8 | 0.65 | 10.65 | 0 | +0.038 [+0.006, +0.070] | +0.029 [-0.022, +0.079] |
| 1 | 0.513 | 0.237 | 0.805 | 226 | 2.20 | 37.4 | 0.63 | 32.41 | 0 | +0.024 [-0.008, +0.057] | +0.050 [+0.007, +0.098] |
| 0.5 | 0.478 | 0.245 | 0.766 | 250 | 2.08 | 36.5 | 0.59 | 61.70 | 0 | -0.011 [-0.047, +0.024] | +0.058 [+0.007, +0.111] |

- The share of non-leak hard incidents noticed rises from 0.917 to 0.946 at z = 2 and then falls back
  (0.941 at z = 1, 0.927 at z = 0.5); the slow leak's noticed share does not move (0.460, then 0.432)
  at any threshold, because its readings below the alarm level are not abnormal observations to any
  threshold. Anomalies a stream rise from 27.6 to 87.7 and the ones anchored on background from 4.0
  to 60.3.
- Quality gains are small and not monotone: +0.038 [+0.006, +0.070] at z = 2, +0.024 [-0.008, +0.057] at
  z = 1, -0.011 [-0.047, +0.024] at z = 0.5, against +0.097 for the notice oracle. The best of them
  pays with false alarms (5.5 a stream at the default, 10.7 at z = 2) and buys plain accuracy
  (0.741 to 0.807) and fewer critical misses (282 to 228) as a side effect of earlier notices.
- So, at these three values, the burst-family part of the gap is partly a tuning matter (about two
  fifths of it at z = 2, with the false-alarm cost shown) and the slow-leak part is not: it needs a
  mechanism that notices a ramp below the alarm level, and the never-noticed burst incidents
  point to anchoring, not the threshold. This is three points on one parameter, one setting, not a search.

## 7. What it means, and what it does not

Mine, for the coordinator to weigh.

- EXP-101 has salience headroom on both counts by the criterion as written, at b = 5 and b = 8. At
  b = 2.5 the burst-family headroom with the rung's context is not shown (it is with the `window`
  builder), and the leak headroom is (+0.324 [+0.252, +0.398]).
- The slow leak dominates. Its headroom (+0.75 of 139 incidents, 27% of hard incidents) is eight times
  the non-leak one in rate, is mostly incidents the rung never noticed and, for the rest, noticed late;
  but the ceiling here asks 16 s after the first observation of a ramp that is, to the rung, invisible
  until it crosses the alarm level. That is a privilege about *where an incident begins*, which a public
  detector can only approach by watching sub-threshold readings; whether it can is an EXP-101 design
  question, not something R10 measured.
- The burst-family headroom is small (+0.097) because the rung's context cannot use a notice: 14 of
  the 20 never-noticed incidents stay wrong with perfect noticing. With the `window` builder the same
  noticing is worth +0.156 and reaches R4's ceiling. Salience and context construction are not
  separable levers for the burst families: the value of the first depends on the second.
- The sweep says the burst-family gap is partly a threshold matter and that the leak gap is not.
  A comparison arm tuned on its threshold would close part of result 1; the criterion as written
  compares with a rung at its default threshold.
- What it does not say: that a public notice mechanism can reach the oracle (none was built); that
  the effect transfers to a real reasoner (the reasoner is simulated; R7-R9 concern its context law); that
  16 s is the right delay for the slow leak (it was R5's tuned value for the pooled hard incidents, not
  tuned for it); or anything about equivalence where an interval includes zero.

## 8. Verified by running, and assumed

Verified by running: the arm's behaviour (unit-level on hand-made sequences and over generated streams:
exactly the hard incidents, once each, anchored at the first observation, at the delivery step, with
the rung's context; no call on a no-hard stream; deterministic); `context_at` equals `context`; every
existing arm's output unchanged (62 arms, byte-identical); every gate in section 9; the notice instants
against the ledgers and the arms' own results; all numbers in this file from the committed CSVs by
the committed scripts (`scripts/r10_assemble.py`).

Assumed or not checked:

- The z-values of the sweep were not validated against the sweep arms' own `anomalies_noticed` (the
  default threshold's replay was, 200 streams for two arms and 199 for the third). A one-stream,
  one-anomaly difference in the selection arm at the default may recur at lower thresholds, where
  more anomalies are in flight.
- "Mode" is my reading of the plan's word as the evaluator's `contradicts_early` label.
- The reading that the injected notice need not be a rung anomaly (section 2, reading 1) changes the
  four incidents noted there; a variant that injects a rung anomaly was not built.
- The primary-setting numbers rest on one 200-stream held-out sample. 372 non-leak hard incidents and
  139 leak incidents are clustered in streams; the bootstrap resamples streams.

**Least sure of.** (1) Whether result 2 measures noticing or the ask time: the notice arm asks at a
fixed 16 s from the first observation while the selection oracle asks 16 s from a notice that is itself
a median 16 s late, so for the leak the two arms differ in the instant they ask by about 16 s as well
as in whether they ask. (2) The never-noticed anchor finding rests on the dump's attached-observation
snapshot at the last step an anomaly was seen; it is consistent across 60 and 106 incidents but is
not a reconstruction of the rung's anchoring decisions. (3) The decomposition's 14 versus 8 split
rests on 14 incidents.

**Hidden record.** I did not read `crates/gordian-stream/HIDDEN-DESIGN.md`. To choose a mode label I
read the field list and doc comments of `IncidentTruth` and `ShapeTruth` in
`crates/gordian-stream/src/oracle.rs` (the evaluator-side truth types the label example reads); the
arm reads none of it.

## 9. Gates, outputs, commits

Gates on exit codes at the final commit: `cargo fmt --all -- --check`,
`cargo clippy --locked --workspace --all-targets -- -D warnings`,
`cargo test --locked --workspace --no-fail-fast`, `scripts/check-no-oracle.sh`, and
`python -W error -m pytest -q analysis`: all exit 0 (572 Rust tests passed, none failed; 326 analysis
tests passed, 4 deselected as before). No existing test or guard expectation was edited; the one
structural test that failed on my first version (a unit test in `oracle.rs` called `generate(`) was
fixed by moving the test, not by changing the guard.

Run outputs: `artifacts/runs/` in the worktree (git-ignored), notice dumps in
`artifacts/runs/r10-notices/`. Committed here: `r10-*.csv`, `scripts/r10_*.py`,
`scripts/r10_run.sh`, `scripts/r10_notice_dump.sh`, the arm, its tests and the example.

- `0b8ebfe` Add oracle_notice_privileged: the selection oracle plus noticing (R10)
- `f23753c` R10 analysis: criterion, decomposition, notice latency, sweep, provenance
