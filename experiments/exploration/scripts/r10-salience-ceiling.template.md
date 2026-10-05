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

{{criterion_primary}}

- **Result 1, salience headroom on hard incidents (slow leak excluded): holds.** +0.097
  [+0.062, +0.132] against a margin of 0.03 and a lower bound that had to exceed 0.01.
- **Result 2, salience headroom on the slow leak: holds.** +0.748 [+0.678, +0.813] against a margin
  of 0.20 and a lower bound that had to exceed 0.10.

Each is its own result; neither is an "either". Sensitivity (b = 2.5 and b = 8, rho = 0.7) and the
secondary pairing (`window` 40 s, N 256), the same two results in every cell:

{{criterion_all}}

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

{{runs}}

**Regression (R6's held-out manifest, b = 5, rho = 0.7).** Replayed with this branch's binary, run id
kept, `source_revision` the only change, into a separate directory: {{regression}} Hashes
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

{{points_primary}}

b = 2.5, rho = 0.7:

{{points_b25}}

b = 8, rho = 0.7:

{{points_b8}}

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

{{decomposition}}

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

{{latency}}

Held-out streams (200), default threshold:

{{latency_heldout}}

### Never-noticed incidents, by family and mode

Counts of hard incidents with no anomaly anchored on them. Slow-leak incidents have no mode.

{{never_summary}}

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

{{sweep}}

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

{{commits}}
