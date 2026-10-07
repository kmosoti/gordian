# B4: a non-privileged selector, decoy accounting, leak versus decoy

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation). Lab 2, unit B4 of
`docs/lab-queue.md`, branch `public-selector`. This is EXP-101's feasibility table: every notice a noticer makes
is charged through a public selector, so that the cost of notices the selection oracle never asks about
(decoys, late plain incidents) shows. The table makes no claim about which noticer or which selector is better;
paired differences are numbers with intervals. Code: `public_threshold.rs`, `public_change.rs`,
`noticer_follow.rs` (`crates/gordian-run/src/stream/arms/`), `select.rs` (the evaluator's selection accounting,
rules E1 to E8 of `crates/gordian-stream-eval/RULES.md`); scripts: `scripts/b4_*.py`, `scripts/b4_run.sh`
(B1's to B3's are imported, not edited); files: `b4-*.csv`, `b4-selected.json`, `b4-summary-tables.md` and
`b4-public-selector-table.md` (generated). The unit was started by a predecessor stopped by an API usage limit
after the held-out run and before its analysis; this report covers both halves and says which is which.

## The result in one paragraph

On the 200 held-out streams at b = 5, rho = 0.7 (seeds 20000-20199: 5,316 incidents, of them 4,264 plain, 511
hard of which 139 are slow leaks, and 541 decoys), with 17 table rows (B3's eleven, M2's medium at 100 ms, and
the follow-up rule on the five ramp rows) under five selectors (104 arms in one run): **the two public selectors,
tuned for hard-incident quality per unit cost on the tuning streams, are barely selective.** They make 90-98%
(threshold) and 77-99% (change) of the calls of "always escalate at 16 s", and the bill is 4 to 12 times the
selection oracle's: for the comparator row (ramp + split over the re-anchor) the threshold rule makes 26.4
calls per stream [25.8, 27.0] at 7.95 s [7.75, 8.16] against the oracle's 2.6 calls and 0.79 s, and the change
rule 25.5 calls and 7.56 s. The bill is 74-82% plain-incident calls (the streams hold 21.3 plain incidents each;
the rules ask about 18-21 of them per stream); decoys are 3-6% of the reasoner's cost, background-anchored notices 3-13% and the leaks' notices
1-7%. **Charged through a public selector, the decoy and late-plain notices the ramp adds cost +0.19 s per
stream under the threshold rule** (decoys +0.085 s, plain +0.108 s; against the split-only row, whose notices the
ramp does not otherwise change) and +0.12 s under the change rule, which is 2.4% and 1.6% of the bill, against
+0.13 s for the leak notices the ramp exists to make; the oracle shows only the leak half (+0.12 s on a 0.67 s
bill). The medium at 100 ms costs +0.94 s [+0.79, +1.09] per stream under the threshold rule, +1.05 s under the
oracle and -0.11 s [-0.19, -0.02] under the change rule, against the comparator, with hard-incident quality
+0.027 [-0.003, +0.057], +0.030 [+0.000, +0.060] and -0.027 [-0.061, +0.008]: **no selector shows the medium's
extra anomalies paid for by its anchoring, and none shows them unpaid for** (the intervals include zero or touch
it). **Leaks and decoys do not separate publicly by the follow-up rule's readings:** the rule tuned on the
tuning streams (6 readings, 6 s, keep unless the counter is below where it was noticed) retired 0 leak notices of
the tuning streams' and 40 of 324 decoy notices; on the held-out streams it retires 48 decoy notices (8% of the
601 notices on decoys; 31% of the ramp's) and **7 leak notices: the start-anchored notice of 7 leaks, 5 of which were noticed again 11 to 15 s later at a
wrong anchor and asked about, and 2 lost (3 under the threshold rule)** (leaks noticed unchanged at 0.986, leak
quality -0.014 [-0.039, +0.008]); it retires 157 late plain notices, and under the threshold rule it spares **0.25 calls per stream (-1.0% cost)**,
almost all of them plain; the decoys it retires would have gone quiet before R5's 16 s question anyway (decoy
calls -0.010 [-0.025, +0.005]). Byte identity on R6's held-out run holds, 62 of 62, three times (at the tuning-stage
tree `522709c`, at `7d2e9a0`, and again after the container restart at `6f306e6`, the tree whose source is the final
one), each time recomputed by me independently of the gate script.

## State inherited, and what the two interruptions did

The unit was interrupted twice: by an API usage limit (after the held-out run, before any analysis) and by a
container restart caused by disk exhaustion (at about 00:45 on 2026-10-07, in the middle of the second
cargo-mutants run). This section is in two parts; the first is what was found at the first resumption and is
kept because the commits and the held-out run come from it, the second is what the restart changed and what
I did on resuming from it.

### After the container restart (the last resumption)

The branch had 16 commits ahead of main (the last, `b5c08d6`, "tests for what the first mutation run missed")
and an uncommitted tree. What each uncommitted item was, and what became of it:

| item | what it was | decision |
|---|---|---|
| `crates/gordian-stream-eval/src/select.rs` modified | one line, `*escalations.calls.slot(class) -= /* ~ changed by cargo-mutants ~ */ 1;`: a mutant that cargo-mutants (`--in-place`) had written into the working tree when the restart killed the run | **Reverted** with `git checkout -- <file>`; never committed. The tests for it (`every_fixture_produces_its_expected_result` and others) were then run on the clean file. |
| `crates/gordian-run/tests/stream_follow.rs` modified | a rustfmt reflow of one statement | **Kept** (formatting only); committed |
| `cargo fmt --all -- --check` | failed on a second file the draft had not touched: `crates/gordian-stream-eval/tests/selection_properties.rs`, committed in `eaf8d3b`, one hunk | fixed by `cargo fmt --all`; both files committed in `499c0d7` |
| `selection_properties.proptest-regressions` (3 seeds) | written by proptest when a mutant broke `the_scorer_agrees_with_the_reference` and `a_record_that_names_no_observation_is_refused` (file time 00:42:58, inside the mutation window; the seeds are the minimal counterexamples of the "call at the notice's own instant" and "observation past the end" mutants: a one-incident spec with one notice and one call) | **Deleted, not kept.** It records failures of mutants, not of the committed code: with the file present the property test passes on the committed code (4 of 4, run by me), and the rerun of the hand mutants wrote a file of three seeds again. |
| `b4-hand-mutants.csv` (untracked) | the second hand-mutant run at `b5c08d6`: 48 of 48 caught | **Reran** (`b4_hand_mutants.py`, exit 0, 119 s): the same 48 verdicts, all caught; committed in `a696db6` |
| `b4-public-selector.md` (untracked, 768 lines) | the report, written at 00:44 by the pre-restart session, with three placeholders (the mutation results, the second-run row, the gates) | **Kept and revised** (this file): every table in it was reproduced from the held-out run by rerunning `b4_table.py`, `b4_summary.py`, `b4_separation.py` (the committed CSVs came out byte-identical, `git status` clean) and `b4_provenance.py`; the placeholders are filled from runs of this session |
| the held-out run directory | intact (414 MB, 104 arm directories) | kept until the analysis was reproduced, then deleted (after its CSVs were verified unchanged) |
| the cross-check directory `xcheck6-...` | gone (the pre-restart session deleted it after recording its hashes) | so the gate was **rerun** (`xcheck7`, below), not recomputed from files |

The restart's effect, in one line: it cost the second cargo-mutants runs (the first runs had found 2 and 1
misses and the tests had been added, but nothing had yet shown that the tests kill those mutants) and left
one mutated line in the tree; nothing else was lost. No result of a run was affected: the held-out run's
files, hashes (`b4-results-sha256.csv`, unchanged by the provenance rerun) and manifest were intact.

### After the first interruption (the usage limit), as found then

Read from `git log main..HEAD` (11 commits at the start), the messages, the scripts, the run logs and the run
directories, then re-checked by running:

- **Done and committed by the predecessor:** the selection accounting in the evaluator (E1 to E8, ten hand-made
  fixture cases, a property test against a reference written from the rules), the two public selectors and the
  follow-up rule, the harness's selection record (two files per arm), the loader and per-stream measures, the
  byte-identity gate (62 of 62, `xcheck5`), the tuning scripts, both tuning stages with their choices (stage F:
  193 arms; stage S: 272 arms), `b4_table.py`, `b4_provenance.py` and tests of the two tuning rules.
- **Played and not committed:** the held-out run `b4-heldout-b5-rho0.7` (104 arms, 200 streams, driver exit 0,
  325 s, no waits, internal/external ratio 0.945, peak memory 0.82 GiB, no OOM kill), played at `6aca6aa`
  (the commit whose message still says "no held-out run has been played"). Its directory, manifest, log and
  `usage.json` were intact; nothing after it was committed. **The interruption's effect:** nothing was left
  half-written (the tree was clean; every scratch draft I compared with its committed version was the same file
  or an earlier copy of it, and none held anything uncommitted that the repository lacked), and the held-out
  run's record (driver log, run index, hashes) did not yet exist. Not done: the table, the CSVs, the report,
  the script tests of `b4_stats.py`'s per-stream frame, mutation testing, the gates, the final byte-identity rerun.
- **An aborted run:** `b4-runs.log` has 15 waits for `b4-explore-b5-rho0.7` and no exit line; there is no
  manifest or output for it. The predecessor wrote an exploration manifest and then used the stage F run's
  `fol_none` arm instead (`b4_explore.py` says so). It never ran.
- **Verified by running, not taken from the predecessor's word:**
  1. The held-out manifest equals, field for field, what `b4_manifests.py heldout` builds from the committed
     `b4-selected.json` (run id, experiment, run seed 13300, seeds 20000-20199, all 104 arms in order with their
     policies, all 104 noticers); its environment fields (stream parameters, rung, limits, exchange rate,
     lockfile hash `3f067036...`, which is `Cargo.lock`'s now) equal the tuning stages'.
  2. **The binary.** `target/release/gordian-run` (mtime 20:48, before every run) has SHA-256 `b2795f97...`; I
     touched every source file and rebuilt the release profile of `gordian-run` from the clean tree at `6aca6aa`
     under the runner (`cargo build --release --locked -p gordian-run`, 1 m 28 s, exit 0, peak 0.77 GB): the new
     binary is **bit-identical**. So the binary that played the held-out run is built from the commit the
     manifest names, and I did not need to rerun the held-out run.
  3. **The choices.** Both tuning rules, applied by the committed `b4_select.py` functions to the committed tuning
     CSVs, give the committed `b4-selected.json`: all 34 selector choices and the follow-up choice at tolerances
     0, 1, 2 and 4.
  4. The provenance script (`b4-vs-earlier.csv`, `b4-vs-b3.csv`): the held-out run's four selection-oracle arms
     for the rung at z = 3 and z = 2, the re-anchor and the medium equal M2's held-out arms byte for byte
     (modulo the run id, results, incidents and notice events), and B3's table is reproduced to 1.1e-16 over 110
     values; the incidents are identical in every arm; no calls about no notice in any comparison arm; 0
     step-capped segments; the bill refused nothing in any arm.

## Byte identity: the gate

R6's held-out manifest (b = 5, rho = 0.7, 62 arms), with only `source_revision` replaced, was replayed under its
own run id three times: `xcheck5` at `522709c` by the predecessor (driver exit 13 once, refused for an unclean tree,
nothing ran; then exit 0, 155 s), `xcheck6` at `7d2e9a0` before the restart (exit 0, 188 s, 4 waits for another
worker; internal/external ratio 0.955), and **`xcheck7` at `6f306e6` after the restart** (exit 0, 152 s, 0 waits,
cores 0-2 under the runner, peak memory 0.44 GiB, no OOM kill, ratio 0.950). The xcheck6 directory no longer
existed on resuming, so the gate was rerun rather than recomputed. For `xcheck7`, **I recomputed the
SHA-256 of `results.csv` and `incidents.csv` of each of the 62 arms myself, against
`experiments/exploration/r6-results-sha256.csv`: 62 of 62 identical in both files**, and the set of
arms equals the record's (`b4-regression-restart.csv`; the gate script agrees; the pre-restart session recorded the same
for `xcheck5` and `xcheck6`: `b4-regression.csv`, `b4-regression-final.csv`, and I did not recompute those two).
The binary was rebuilt at `6f306e6` (release, under the runner, exit 0, 48 s) and has SHA-256 `b2795f97...`, the
hash of the binary that played the held-out run: between `6aca6aa` (the commit the held-out manifest names) and the
tree of `xcheck7` only tests (`stream_follow.rs`, `selection_fixtures.rs`, `selection_properties.rs`), scripts and
experiment files changed (`git diff --stat 6aca6aa HEAD -- crates Cargo.toml Cargo.lock`). Run directory
deleted after recording.

## What was built

Everything below is built and was run; nothing here is planned.

1. **Two non-privileged selectors**, as `EscalationRule`s over any noticer's anomalies, each asking once per
   anomaly at R5's 16 s delay with the rung's context (so the only variable against the selection oracle is
   which anomalies are asked about):
   - `public_threshold` (`persist_ns` = `t`): ask when the rung has not resolved the anomaly by public
     information. The module documentation fixes the readings before any tuning: "the rung's conclusion" is
     the rung's declaration (made or not) and the public consistency checker's verdict (a consistent
     hypothesis, or none); "contradictory" is no consistent hypothesis in every check for `t` seconds; "silent"
     is no declaration by the rung `t` seconds after notice (the other reading, "the anomaly itself is
     quiet", is not adopted: the rung already retires an anomaly quiet for 6 s); "or" is either.
   - `public_change` (`k`): ask when the anomaly's attached evidence has grown by `k` observations since the
     rule first saw it (the notice's step); a falling count is not growth and the baseline is not lowered.
     For a ramp-noticed anomaly the evidence counts the chain's readings, benign ones included; for the medium,
     what its proposals cite.
2. **Evaluator columns** (`select.rs`, rules E1 to E8, with `selection.csv` and `selection_notices.csv` per
   arm): escalations by the class of their focus (background, plain, hard, leak, decoy), calls and declared
   cost by class, the share of reasoner cost per class, and for each notice whether it was asked about
   (attributed to the latest live notice at the focus, E1), retired, retired by a follow-up rule, or retired
   before escalation; notices on decoys per decoy. `score_stream` and `score_notices` are unchanged (the
   byte-identity gate shows `results.csv` and `incidents.csv` are).
3. **The leak-versus-decoy follow-up rule** (`noticer_follow.rs`, `FollowSpec`: `readings`, `horizon_ns`,
   `min_gain`, `max_fall`), inside the ramp noticer and acting only on ramp-opened anomalies: it watches the
   counter at the chain's key after the completing (fifth) reading; after `readings` follow-up readings, or at
   the horizon, it keeps the anomaly if the latest reading is at least `min_gain` above the completing
   reading and withdraws it otherwise (a reversal of more than `max_fall` below the peak withdraws it at once;
   `u32::MAX` never does). The notice stays on the record and no notice measure moves; only the retirement,
   which the rule marks as its own.

## Readings, tuning and the choices

**Stage F** (the follow-up rule), on seeds 10000-10099, under `always_escalate` at 16 s over the comparator
row's noticer, 192 configurations (readings {1, 2, 3, 4, 6, 8} x horizon {4, 6} s x min_gain {0, 3, 6, 10} x
max_fall {2, 4, 8, none}) beside the arm without the rule. The rule, fixed in `b4_common.py` before the run:
within the background budget (6.82), at most 0 notices anchored on a slow leak retired by the rule, hard
non-leak quality no more than 0.005 under the arm without it; then the most decoy notices retired by the rule
before escalation; ties by the fewest plain and hard notices retired, then the larger readings, min_gain,
max_fall, horizon. **Only 2 of 192 configurations retire no leak notice on the tuning streams**; the choice is
readings 6, horizon 6 s, min_gain 0, max_fall none: 40 of 324 decoy notices retired, 72 plain and 8 hard
notices retired as collateral, 0 leak notices, quality unchanged (0.588), 6.18 background notices per stream.
The sensitivity configurations at 1, 2 and 4 leak notices tolerated retire 39, 41 and 44 decoy notices; the
most any configuration retires is 86 of 325, at 45 leak notices retired. (`b4-tuning-follow.csv`.)

**Stage S** (the selectors), same streams, every row's grid (threshold t in {0, 1, 2, 4, 8, 16} s, change
k in {1, 2, 3, 5, 8, 12, 20}) beside never, always and the oracle, 272 arms. The rule, fixed before the run:
among the grid's configurations whose hard non-leak quality is at least 0.9 of the grid's best, the highest
quality per modelled second per stream (total cost, the rule's and the medium's own operations included);
ties the fewer calls, then the more selective value. **None of the 34 choices was a tie.** The parameters:

| row | threshold t (s) | tuning quality, cost s | change k | tuning quality, cost s |
|---|---|---|---|---|
| rung z=3 | 0 | 0.467, 7.53 | 8 | 0.482, 7.42 |
| rung z=2 | 0 | 0.518, 7.67 | 12 | 0.528, 7.33 |
| re-anchor | 1 | 0.563, 7.69 | 12 | 0.573, 7.30 |
| ramp / rung3 | 0 | 0.467, 7.75 | 8 | 0.482, 7.65 |
| split / rung3 | 0 | 0.467, 7.54 | 8 | 0.482, 7.43 |
| ramp+split / rung3 | 0 | 0.467, 7.76 | 8 | 0.482, 7.66 |
| ramp / re-anchor | 1 | 0.563, 7.92 | 12 | 0.573, 7.50 |
| split / re-anchor | 1 | 0.568, 7.71 | 8 | 0.583, 7.43 |
| ramp+split / re-anchor (comparator) | 1 | 0.568, 7.93 | 12 | 0.573, 7.51 |
| re-anchor z=3 | 0 | 0.503, 7.49 | 8 | 0.523, 7.37 |
| ramp+split / re-anchor z=3 | 0 | 0.503, 7.75 | 8 | 0.523, 7.62 |
| medium 100 ms | 1 | 0.583, 8.71 | 12 | 0.583, 7.37 |
| each follow-up row | as its row without the rule | | | |

(Each follow-up row was tuned again on its own arms and got the same parameters as its base row.) The 0.9 floor did not bind in
any of the 34 grids (the best efficiency was always above the floor), and **the threshold rule's `t` hardly matters** (below the
16 s delay it only changes whether a contradiction began within the last `t` seconds; held-out, comparator row,
t = 0, 1, 2, 4, 8, 16: quality 0.543, 0.570, 0.559, 0.573, 0.548, 0.516 and calls 26.4, 26.4, 26.4, 26.4, 26.0,
24.7). The change rule bites only at its largest `k`: k = 20 asks about 23.2 calls per stream (-14% against
always) and loses 0.07 of quality (0.492), and costs *more* per stream (8.21 s) because it asks late and about
long-lived anomalies, whose calls are bigger.

## The table

200 held-out streams, b = 5, rho = 0.7, 90% cluster-bootstrap intervals (resampling whole streams, 10,000
resamples, B2's seed, the same resamples for every arm and measure, so every difference is paired). "Hard quality"
is B2's: hard non-leak incidents declared correctly by their deadline over hard non-leak incidents (372); leaks are
reported beside as leak quality. **Selection oracle = the labelled ceiling**: it asks about the notices anchored on
hard incidents at 16 s and never about anything else, which is why its decoy and plain columns are zero; it is a
ceiling for cost, not for quality (the public rows are within 0.011 above to 0.062 below it on hard quality, mostly
inside the intervals). Column definitions: calls and cost are the reasoner's (accepted calls; declared modelled
seconds, with the substrate and the rule's own operations, which for the comparator row are 1.4 ms per stream
under the threshold rule (its consistency checker) and 0.5 ms under the others: priced, and negligible); "esc." columns are accepted calls by the class of the focus's incident (E2);
"crit. misses" are critical incidents missed per stream (plain and hard), beside quality as in R5.

The columns the queue names but this table does not carry in the condensed form are in
`b4-public-selector-table.md` and `b4-table.csv` (every column with intervals: anchor-correct, leak noticed,
leak quality, background notices, strict precision, notices on decoys per decoy, the fate of the decoy notices,
escalations by class and their cost shares, calls about no notice).

#### selection oracle (privileged ceiling)

| row | parameter | hard quality | crit. misses/stream | plain acc. | calls/stream | cost s/stream | esc. decoys/stream | esc. plain/stream |
|---|---|---|---|---|---|---|---|---|
| rung z=3 |  | 0.489 [0.448, 0.532] | 1.41 [1.25, 1.57] | 0.741 [0.726, 0.755] | 2.1 [1.9, 2.3] | 0.66 [0.60, 0.73] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| rung z=2 |  | 0.527 [0.482, 0.572] | 1.14 [1.00, 1.28] | 0.807 [0.794, 0.819] | 2.2 [2.0, 2.4] | 0.65 [0.59, 0.71] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| re-anchor |  | 0.551 [0.507, 0.597] | 0.99 [0.86, 1.13] | 0.860 [0.848, 0.871] | 2.2 [2.0, 2.4] | 0.67 [0.61, 0.73] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| ramp / rung3 |  | 0.489 [0.447, 0.532] | 1.27 [1.13, 1.43] | 0.745 [0.731, 0.759] | 2.5 [2.3, 2.7] | 0.79 [0.72, 0.85] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| split / rung3 |  | 0.492 [0.450, 0.535] | 1.39 [1.24, 1.55] | 0.746 [0.732, 0.761] | 2.1 [1.9, 2.3] | 0.67 [0.61, 0.73] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| ramp+split / rung3 |  | 0.492 [0.449, 0.535] | 1.25 [1.11, 1.40] | 0.750 [0.736, 0.764] | 2.5 [2.3, 2.7] | 0.79 [0.72, 0.85] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| ramp / re-anchor |  | 0.554 [0.509, 0.600] | 0.86 [0.74, 0.98] | 0.862 [0.850, 0.873] | 2.6 [2.4, 2.8] | 0.79 [0.73, 0.86] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| split / re-anchor |  | 0.559 [0.515, 0.605] | 0.97 [0.84, 1.10] | 0.870 [0.859, 0.881] | 2.2 [2.1, 2.4] | 0.67 [0.62, 0.73] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| ramp+split / re-anchor (comparator) |  | 0.562 [0.517, 0.608] | 0.83 [0.72, 0.95] | 0.872 [0.861, 0.883] | 2.6 [2.4, 2.9] | 0.79 [0.73, 0.86] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| re-anchor z=3 |  | 0.516 [0.474, 0.558] | 1.25 [1.10, 1.40] | 0.793 [0.779, 0.806] | 2.2 [2.0, 2.4] | 0.68 [0.62, 0.74] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| ramp+split / re-anchor z=3 |  | 0.524 [0.481, 0.567] | 1.09 [0.96, 1.23] | 0.807 [0.794, 0.820] | 2.6 [2.4, 2.8] | 0.82 [0.75, 0.88] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| medium 100 ms |  | 0.591 [0.550, 0.632] | 0.74 [0.63, 0.85] | 0.883 [0.871, 0.893] | 6.3 [5.6, 7.1] | 1.85 [1.63, 2.08] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| ramp / rung3 + follow-up |  | 0.487 [0.444, 0.529] | 1.28 [1.14, 1.44] | 0.745 [0.731, 0.759] | 2.5 [2.3, 2.7] | 0.78 [0.71, 0.84] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| ramp+split / rung3 + follow-up |  | 0.489 [0.447, 0.532] | 1.26 [1.12, 1.42] | 0.751 [0.736, 0.765] | 2.5 [2.3, 2.7] | 0.78 [0.71, 0.85] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| ramp / re-anchor + follow-up |  | 0.551 [0.507, 0.597] | 0.87 [0.75, 0.99] | 0.862 [0.850, 0.873] | 2.6 [2.4, 2.8] | 0.78 [0.72, 0.85] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| ramp+split / re-anchor (comparator) + follow-up |  | 0.559 [0.515, 0.605] | 0.84 [0.73, 0.96] | 0.872 [0.861, 0.883] | 2.6 [2.4, 2.8] | 0.79 [0.72, 0.85] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| ramp+split / re-anchor z=3 + follow-up |  | 0.522 [0.479, 0.564] | 1.10 [0.97, 1.24] | 0.807 [0.794, 0.820] | 2.6 [2.4, 2.8] | 0.81 [0.74, 0.87] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |

#### public threshold

| row | parameter | hard quality | crit. misses/stream | plain acc. | calls/stream | cost s/stream | esc. decoys/stream | esc. plain/stream |
|---|---|---|---|---|---|---|---|---|
| rung z=3 | t = 0 s | 0.476 [0.433, 0.520] | 1.26 [1.11, 1.42] | 0.811 [0.798, 0.823] | 23.9 [23.4, 24.4] | 7.66 [7.48, 7.85] | 0.95 [0.84, 1.07] | 18.2 [17.7, 18.7] |
| rung z=2 | t = 0 s | 0.508 [0.464, 0.553] | 0.99 [0.86, 1.12] | 0.869 [0.860, 0.877] | 25.7 [25.1, 26.2] | 7.74 [7.54, 7.94] | 1.22 [1.08, 1.36] | 18.9 [18.4, 19.5] |
| re-anchor | t = 1 s | 0.562 [0.517, 0.609] | 0.84 [0.72, 0.96] | 0.926 [0.918, 0.933] | 25.7 [25.1, 26.3] | 7.75 [7.55, 7.95] | 1.30 [1.16, 1.45] | 20.3 [19.7, 20.9] |
| ramp / rung3 | t = 0 s | 0.476 [0.432, 0.521] | 1.17 [1.02, 1.31] | 0.814 [0.801, 0.826] | 24.6 [24.1, 25.1] | 7.86 [7.67, 8.06] | 1.26 [1.13, 1.40] | 18.5 [18.0, 19.0] |
| split / rung3 | t = 0 s | 0.478 [0.435, 0.522] | 1.25 [1.09, 1.40] | 0.816 [0.803, 0.828] | 23.9 [23.4, 24.4] | 7.67 [7.48, 7.85] | 0.97 [0.85, 1.09] | 18.3 [17.8, 18.8] |
| ramp+split / rung3 | t = 0 s | 0.478 [0.434, 0.523] | 1.15 [1.00, 1.28] | 0.819 [0.806, 0.831] | 24.6 [24.1, 25.2] | 7.87 [7.67, 8.06] | 1.27 [1.14, 1.42] | 18.6 [18.1, 19.1] |
| ramp / re-anchor | t = 1 s | 0.562 [0.516, 0.609] | 0.74 [0.64, 0.85] | 0.927 [0.920, 0.934] | 26.4 [25.8, 27.0] | 7.95 [7.75, 8.16] | 1.58 [1.44, 1.74] | 20.7 [20.1, 21.2] |
| split / re-anchor | t = 1 s | 0.570 [0.525, 0.616] | 0.81 [0.69, 0.93] | 0.934 [0.927, 0.940] | 25.7 [25.1, 26.3] | 7.75 [7.55, 7.95] | 1.32 [1.19, 1.47] | 20.4 [19.8, 21.0] |
| ramp+split / re-anchor (comparator) | t = 1 s | 0.570 [0.524, 0.616] | 0.71 [0.61, 0.81] | 0.935 [0.928, 0.942] | 26.4 [25.8, 27.0] | 7.95 [7.75, 8.16] | 1.61 [1.46, 1.77] | 20.8 [20.2, 21.4] |
| re-anchor z=3 | t = 0 s | 0.505 [0.463, 0.549] | 1.10 [0.96, 1.25] | 0.866 [0.854, 0.877] | 23.8 [23.3, 24.4] | 7.61 [7.43, 7.80] | 1.04 [0.92, 1.17] | 19.4 [18.9, 19.9] |
| ramp+split / re-anchor z=3 | t = 0 s | 0.513 [0.470, 0.558] | 0.98 [0.85, 1.11] | 0.877 [0.866, 0.888] | 24.6 [24.0, 25.1] | 7.84 [7.65, 8.03] | 1.36 [1.23, 1.50] | 19.8 [19.3, 20.3] |
| medium 100 ms | t = 1 s | 0.597 [0.554, 0.639] | 0.62 [0.53, 0.72] | 0.941 [0.934, 0.947] | 31.0 [30.1, 31.9] | 8.89 [8.62, 9.18] | 1.85 [1.67, 2.05] | 22.9 [22.2, 23.7] |
| ramp / rung3 + follow-up | t = 0 s | 0.473 [0.430, 0.517] | 1.18 [1.03, 1.32] | 0.814 [0.802, 0.826] | 24.3 [23.7, 24.8] | 7.77 [7.58, 7.96] | 1.25 [1.11, 1.39] | 18.2 [17.7, 18.7] |
| ramp+split / rung3 + follow-up | t = 0 s | 0.476 [0.432, 0.520] | 1.16 [1.01, 1.30] | 0.819 [0.806, 0.831] | 24.3 [23.8, 24.8] | 7.77 [7.58, 7.96] | 1.26 [1.13, 1.41] | 18.2 [17.8, 18.7] |
| ramp / re-anchor + follow-up | t = 1 s | 0.559 [0.514, 0.606] | 0.76 [0.65, 0.86] | 0.927 [0.920, 0.934] | 26.1 [25.5, 26.7] | 7.87 [7.68, 8.08] | 1.57 [1.42, 1.73] | 20.4 [19.8, 21.0] |
| ramp+split / re-anchor (comparator) + follow-up | t = 1 s | 0.567 [0.522, 0.613] | 0.72 [0.62, 0.83] | 0.935 [0.928, 0.942] | 26.1 [25.5, 26.7] | 7.88 [7.68, 8.08] | 1.60 [1.45, 1.76] | 20.5 [19.9, 21.1] |
| ramp+split / re-anchor z=3 + follow-up | t = 0 s | 0.511 [0.468, 0.555] | 0.99 [0.86, 1.12] | 0.878 [0.866, 0.888] | 24.2 [23.7, 24.8] | 7.74 [7.55, 7.93] | 1.35 [1.22, 1.50] | 19.5 [19.0, 20.0] |

#### public change

| row | parameter | hard quality | crit. misses/stream | plain acc. | calls/stream | cost s/stream | esc. decoys/stream | esc. plain/stream |
|---|---|---|---|---|---|---|---|---|
| rung z=3 | k = 8 | 0.487 [0.446, 0.528] | 1.26 [1.11, 1.42] | 0.813 [0.800, 0.825] | 24.0 [23.5, 24.5] | 7.49 [7.31, 7.67] | 0.95 [0.84, 1.07] | 18.3 [17.8, 18.8] |
| rung z=2 | k = 12 | 0.519 [0.474, 0.565] | 0.99 [0.86, 1.12] | 0.871 [0.862, 0.880] | 24.8 [24.2, 25.4] | 7.39 [7.20, 7.59] | 0.81 [0.69, 0.92] | 19.1 [18.5, 19.6] |
| re-anchor | k = 12 | 0.543 [0.497, 0.589] | 0.84 [0.73, 0.96] | 0.928 [0.920, 0.935] | 24.8 [24.2, 25.5] | 7.37 [7.18, 7.57] | 0.85 [0.74, 0.97] | 20.3 [19.8, 20.9] |
| ramp / rung3 | k = 8 | 0.487 [0.445, 0.529] | 1.13 [0.99, 1.27] | 0.816 [0.803, 0.828] | 24.8 [24.2, 25.4] | 7.70 [7.51, 7.89] | 1.36 [1.22, 1.50] | 18.7 [18.2, 19.2] |
| split / rung3 | k = 8 | 0.489 [0.448, 0.531] | 1.25 [1.09, 1.40] | 0.818 [0.805, 0.830] | 24.0 [23.5, 24.6] | 7.49 [7.31, 7.68] | 0.97 [0.85, 1.09] | 18.4 [17.9, 18.9] |
| ramp+split / rung3 | k = 8 | 0.489 [0.448, 0.532] | 1.11 [0.97, 1.25] | 0.821 [0.808, 0.833] | 24.8 [24.3, 25.4] | 7.70 [7.51, 7.89] | 1.38 [1.24, 1.52] | 18.7 [18.2, 19.2] |
| ramp / re-anchor | k = 12 | 0.546 [0.500, 0.593] | 0.71 [0.61, 0.81] | 0.929 [0.922, 0.936] | 25.5 [24.8, 26.1] | 7.56 [7.35, 7.76] | 1.17 [1.03, 1.31] | 20.6 [20.0, 21.2] |
| split / re-anchor | k = 8 | 0.559 [0.516, 0.604] | 0.81 [0.69, 0.92] | 0.936 [0.930, 0.943] | 25.6 [25.0, 26.2] | 7.45 [7.25, 7.65] | 1.37 [1.23, 1.51] | 20.6 [20.0, 21.2] |
| ramp+split / re-anchor (comparator) | k = 12 | 0.556 [0.512, 0.602] | 0.68 [0.58, 0.78] | 0.937 [0.930, 0.943] | 25.5 [24.9, 26.1] | 7.56 [7.36, 7.77] | 1.19 [1.04, 1.33] | 20.7 [20.1, 21.3] |
| re-anchor z=3 | k = 8 | 0.513 [0.472, 0.555] | 1.10 [0.96, 1.24] | 0.868 [0.856, 0.879] | 24.0 [23.5, 24.5] | 7.44 [7.26, 7.62] | 1.05 [0.93, 1.18] | 19.5 [19.0, 20.0] |
| ramp+split / re-anchor z=3 | k = 8 | 0.522 [0.479, 0.564] | 0.94 [0.82, 1.07] | 0.880 [0.868, 0.890] | 24.8 [24.3, 25.4] | 7.68 [7.49, 7.87] | 1.47 [1.32, 1.62] | 20.0 [19.4, 20.5] |
| medium 100 ms | k = 12 | 0.530 [0.490, 0.569] | 0.70 [0.59, 0.81] | 0.941 [0.935, 0.948] | 26.4 [25.7, 27.1] | 7.45 [7.24, 7.68] | 1.16 [1.01, 1.30] | 21.1 [20.4, 21.8] |
| ramp / rung3 + follow-up | k = 8 | 0.484 [0.442, 0.526] | 1.14 [1.00, 1.28] | 0.816 [0.804, 0.829] | 24.5 [23.9, 25.0] | 7.61 [7.42, 7.80] | 1.35 [1.21, 1.50] | 18.3 [17.8, 18.8] |
| ramp+split / rung3 + follow-up | k = 8 | 0.487 [0.445, 0.528] | 1.12 [0.98, 1.26] | 0.821 [0.808, 0.833] | 24.5 [24.0, 25.0] | 7.61 [7.43, 7.80] | 1.36 [1.22, 1.51] | 18.4 [17.9, 18.9] |
| ramp / re-anchor + follow-up | k = 12 | 0.540 [0.495, 0.587] | 0.72 [0.62, 0.82] | 0.929 [0.922, 0.936] | 25.3 [24.6, 25.9] | 7.49 [7.29, 7.69] | 1.17 [1.02, 1.31] | 20.4 [19.8, 20.9] |
| ramp+split / re-anchor (comparator) + follow-up | k = 12 | 0.551 [0.507, 0.596] | 0.69 [0.59, 0.79] | 0.937 [0.930, 0.943] | 25.3 [24.6, 25.9] | 7.49 [7.29, 7.70] | 1.18 [1.04, 1.32] | 20.5 [19.9, 21.1] |
| ramp+split / re-anchor z=3 + follow-up | k = 8 | 0.519 [0.477, 0.561] | 0.95 [0.83, 1.08] | 0.880 [0.868, 0.891] | 24.5 [23.9, 25.0] | 7.58 [7.40, 7.77] | 1.46 [1.31, 1.61] | 19.6 [19.1, 20.1] |

#### always escalate at 16 s

| row | parameter | hard quality | crit. misses/stream | plain acc. | calls/stream | cost s/stream | esc. decoys/stream | esc. plain/stream |
|---|---|---|---|---|---|---|---|---|
| rung z=3 |  | 0.489 [0.448, 0.532] | 1.26 [1.11, 1.42] | 0.813 [0.800, 0.825] | 24.3 [23.8, 24.9] | 7.58 [7.40, 7.77] | 1.04 [0.92, 1.18] | 18.4 [17.9, 18.9] |
| rung z=2 |  | 0.527 [0.482, 0.572] | 0.98 [0.86, 1.11] | 0.872 [0.863, 0.880] | 26.2 [25.6, 26.8] | 7.61 [7.41, 7.80] | 1.37 [1.22, 1.52] | 19.2 [18.6, 19.8] |
| re-anchor |  | 0.551 [0.507, 0.597] | 0.83 [0.72, 0.95] | 0.929 [0.922, 0.936] | 26.2 [25.6, 26.8] | 7.58 [7.39, 7.77] | 1.46 [1.30, 1.61] | 20.6 [20.0, 21.1] |
| ramp / rung3 |  | 0.489 [0.447, 0.532] | 1.13 [0.99, 1.27] | 0.816 [0.803, 0.828] | 25.2 [24.6, 25.7] | 7.81 [7.61, 8.00] | 1.45 [1.29, 1.59] | 18.8 [18.3, 19.3] |
| split / rung3 |  | 0.492 [0.450, 0.535] | 1.25 [1.09, 1.40] | 0.818 [0.805, 0.830] | 24.3 [23.8, 24.9] | 7.59 [7.40, 7.77] | 1.06 [0.93, 1.19] | 18.5 [18.0, 19.0] |
| ramp+split / rung3 |  | 0.492 [0.449, 0.535] | 1.11 [0.97, 1.25] | 0.821 [0.808, 0.833] | 25.2 [24.6, 25.7] | 7.81 [7.62, 8.00] | 1.46 [1.31, 1.61] | 18.9 [18.4, 19.4] |
| ramp / re-anchor |  | 0.554 [0.509, 0.600] | 0.70 [0.60, 0.80] | 0.930 [0.923, 0.937] | 27.0 [26.4, 27.6] | 7.81 [7.61, 8.02] | 1.83 [1.67, 2.01] | 21.0 [20.4, 21.6] |
| split / re-anchor |  | 0.559 [0.515, 0.605] | 0.81 [0.69, 0.92] | 0.937 [0.930, 0.943] | 26.2 [25.6, 26.8] | 7.58 [7.39, 7.78] | 1.48 [1.32, 1.64] | 20.7 [20.1, 21.3] |
| ramp+split / re-anchor (comparator) |  | 0.562 [0.517, 0.608] | 0.67 [0.57, 0.77] | 0.938 [0.932, 0.945] | 27.0 [26.4, 27.6] | 7.81 [7.62, 8.02] | 1.86 [1.69, 2.04] | 21.1 [20.5, 21.7] |
| re-anchor z=3 |  | 0.516 [0.474, 0.558] | 1.10 [0.96, 1.24] | 0.868 [0.856, 0.879] | 24.3 [23.7, 24.8] | 7.52 [7.34, 7.71] | 1.14 [1.01, 1.27] | 19.6 [19.1, 20.1] |
| ramp+split / re-anchor z=3 |  | 0.524 [0.481, 0.567] | 0.94 [0.82, 1.07] | 0.880 [0.868, 0.890] | 25.1 [24.6, 25.7] | 7.78 [7.59, 7.97] | 1.56 [1.41, 1.72] | 20.1 [19.6, 20.6] |
| medium 100 ms |  | 0.591 [0.550, 0.632] | 0.56 [0.47, 0.65] | 0.944 [0.938, 0.950] | 34.2 [33.1, 35.4] | 9.46 [9.12, 9.81] | 2.25 [2.04, 2.48] | 23.6 [22.8, 24.4] |
| ramp / rung3 + follow-up |  | 0.487 [0.444, 0.529] | 1.14 [1.00, 1.28] | 0.816 [0.804, 0.829] | 24.8 [24.3, 25.3] | 7.71 [7.52, 7.90] | 1.43 [1.28, 1.58] | 18.4 [17.9, 18.9] |
| ramp+split / rung3 + follow-up |  | 0.489 [0.447, 0.532] | 1.12 [0.98, 1.26] | 0.821 [0.808, 0.833] | 24.8 [24.3, 25.4] | 7.71 [7.52, 7.90] | 1.45 [1.29, 1.60] | 18.5 [18.0, 19.0] |
| ramp / re-anchor + follow-up |  | 0.551 [0.507, 0.597] | 0.71 [0.61, 0.81] | 0.930 [0.923, 0.937] | 26.7 [26.1, 27.4] | 7.73 [7.54, 7.93] | 1.82 [1.65, 2.00] | 20.7 [20.1, 21.3] |
| ramp+split / re-anchor (comparator) + follow-up |  | 0.559 [0.515, 0.605] | 0.68 [0.58, 0.78] | 0.938 [0.932, 0.945] | 26.8 [26.2, 27.4] | 7.74 [7.54, 7.94] | 1.84 [1.68, 2.02] | 20.8 [20.3, 21.4] |
| ramp+split / re-anchor z=3 + follow-up |  | 0.522 [0.479, 0.564] | 0.95 [0.83, 1.08] | 0.880 [0.868, 0.891] | 24.8 [24.2, 25.3] | 7.67 [7.49, 7.86] | 1.54 [1.39, 1.70] | 19.7 [19.2, 20.2] |

#### never escalate

| row | parameter | hard quality | crit. misses/stream | plain acc. | calls/stream | cost s/stream | esc. decoys/stream | esc. plain/stream |
|---|---|---|---|---|---|---|---|---|
| rung z=3 |  | 0.000 [0.000, 0.000] | 1.69 [1.51, 1.87] | 0.741 [0.726, 0.755] | 0.0 [0.0, 0.0] | 0.00 [0.00, 0.00] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| rung z=2 |  | 0.000 [0.000, 0.000] | 1.43 [1.26, 1.58] | 0.807 [0.794, 0.819] | 0.0 [0.0, 0.0] | 0.00 [0.00, 0.00] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| re-anchor |  | 0.000 [0.000, 0.000] | 1.30 [1.15, 1.45] | 0.860 [0.848, 0.871] | 0.0 [0.0, 0.0] | 0.00 [0.00, 0.00] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| ramp / rung3 |  | 0.000 [0.000, 0.000] | 1.69 [1.51, 1.87] | 0.745 [0.731, 0.759] | 0.0 [0.0, 0.0] | 0.00 [0.00, 0.00] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| split / rung3 |  | 0.000 [0.000, 0.000] | 1.68 [1.50, 1.85] | 0.746 [0.732, 0.761] | 0.0 [0.0, 0.0] | 0.00 [0.00, 0.00] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| ramp+split / rung3 |  | 0.000 [0.000, 0.000] | 1.68 [1.50, 1.85] | 0.750 [0.736, 0.764] | 0.0 [0.0, 0.0] | 0.00 [0.00, 0.00] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| ramp / re-anchor |  | 0.000 [0.000, 0.000] | 1.30 [1.15, 1.45] | 0.862 [0.850, 0.873] | 0.0 [0.0, 0.0] | 0.00 [0.00, 0.00] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| split / re-anchor |  | 0.000 [0.000, 0.000] | 1.28 [1.14, 1.43] | 0.870 [0.859, 0.881] | 0.0 [0.0, 0.0] | 0.00 [0.00, 0.00] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| ramp+split / re-anchor (comparator) |  | 0.000 [0.000, 0.000] | 1.28 [1.14, 1.43] | 0.872 [0.861, 0.883] | 0.0 [0.0, 0.0] | 0.00 [0.00, 0.00] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| re-anchor z=3 |  | 0.000 [0.000, 0.000] | 1.55 [1.39, 1.72] | 0.793 [0.779, 0.806] | 0.0 [0.0, 0.0] | 0.00 [0.00, 0.00] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| ramp+split / re-anchor z=3 |  | 0.000 [0.000, 0.000] | 1.53 [1.38, 1.70] | 0.807 [0.794, 0.820] | 0.0 [0.0, 0.0] | 0.00 [0.00, 0.00] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| medium 100 ms |  | 0.000 [0.000, 0.000] | 1.20 [1.05, 1.34] | 0.882 [0.871, 0.892] | 0.0 [0.0, 0.0] | 0.00 [0.00, 0.00] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| ramp / rung3 + follow-up |  | 0.000 [0.000, 0.000] | 1.69 [1.51, 1.87] | 0.745 [0.731, 0.759] | 0.0 [0.0, 0.0] | 0.00 [0.00, 0.00] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| ramp+split / rung3 + follow-up |  | 0.000 [0.000, 0.000] | 1.68 [1.50, 1.85] | 0.751 [0.736, 0.765] | 0.0 [0.0, 0.0] | 0.00 [0.00, 0.00] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| ramp / re-anchor + follow-up |  | 0.000 [0.000, 0.000] | 1.30 [1.15, 1.45] | 0.862 [0.850, 0.873] | 0.0 [0.0, 0.0] | 0.00 [0.00, 0.00] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| ramp+split / re-anchor (comparator) + follow-up |  | 0.000 [0.000, 0.000] | 1.28 [1.14, 1.43] | 0.872 [0.861, 0.883] | 0.0 [0.0, 0.0] | 0.00 [0.00, 0.00] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |
| ramp+split / re-anchor z=3 + follow-up |  | 0.000 [0.000, 0.000] | 1.53 [1.38, 1.70] | 0.807 [0.794, 0.820] | 0.0 [0.0, 0.0] | 0.00 [0.00, 0.00] | 0.00 [0.00, 0.00] | 0.0 [0.0, 0.0] |

### Paired differences against the comparator (ramp+split / re-anchor), same selector

#### selection oracle (privileged ceiling)

| row | hard quality | crit. misses/stream | plain acc. | calls/stream | cost s/stream | esc. decoys/stream | esc. plain/stream |
|---|---|---|---|---|---|---|---|
| rung z=3 | -0.073 [-0.107, -0.039] | +0.57 [+0.47, +0.69] | -0.131 [-0.143, -0.120] | -0.5 [-0.6, -0.4] | -0.13 [-0.16, -0.10] | +0.00 [+0.00, +0.00] | +0.0 [+0.0, +0.0] |
| rung z=2 | -0.035 [-0.053, -0.018] | +0.30 [+0.22, +0.39] | -0.065 [-0.072, -0.059] | -0.5 [-0.6, -0.4] | -0.15 [-0.18, -0.12] | +0.00 [+0.00, +0.00] | +0.0 [+0.0, +0.0] |
| re-anchor | -0.011 [-0.020, -0.003] | +0.16 [+0.10, +0.21] | -0.012 [-0.015, -0.009] | -0.4 [-0.5, -0.3] | -0.12 [-0.15, -0.10] | +0.00 [+0.00, +0.00] | +0.0 [+0.0, +0.0] |
| ramp / rung3 | -0.073 [-0.108, -0.038] | +0.44 [+0.35, +0.54] | -0.127 [-0.138, -0.115] | -0.1 [-0.2, -0.1] | -0.01 [-0.03, +0.01] | +0.00 [+0.00, +0.00] | +0.0 [+0.0, +0.0] |
| split / rung3 | -0.070 [-0.105, -0.036] | +0.55 [+0.45, +0.67] | -0.125 [-0.137, -0.114] | -0.5 [-0.6, -0.4] | -0.13 [-0.16, -0.10] | +0.00 [+0.00, +0.00] | +0.0 [+0.0, +0.0] |
| ramp+split / rung3 | -0.070 [-0.105, -0.036] | +0.42 [+0.33, +0.52] | -0.121 [-0.133, -0.110] | -0.1 [-0.2, -0.1] | -0.01 [-0.03, +0.01] | +0.00 [+0.00, +0.00] | +0.0 [+0.0, +0.0] |
| ramp / re-anchor | -0.008 [-0.016, +0.000] | +0.03 [+0.00, +0.05] | -0.010 [-0.013, -0.008] | -0.0 [-0.0, -0.0] | -0.00 [-0.01, +0.00] | +0.00 [+0.00, +0.00] | +0.0 [+0.0, +0.0] |
| split / re-anchor | -0.003 [-0.008, +0.000] | +0.14 [+0.09, +0.18] | -0.002 [-0.003, -0.001] | -0.4 [-0.5, -0.3] | -0.12 [-0.15, -0.10] | +0.00 [+0.00, +0.00] | +0.0 [+0.0, +0.0] |
| re-anchor z=3 | -0.046 [-0.079, -0.013] | +0.41 [+0.32, +0.51] | -0.079 [-0.089, -0.070] | -0.5 [-0.6, -0.4] | -0.11 [-0.14, -0.09] | +0.00 [+0.00, +0.00] | +0.0 [+0.0, +0.0] |
| ramp+split / re-anchor z=3 | -0.038 [-0.070, -0.006] | +0.26 [+0.19, +0.34] | -0.065 [-0.074, -0.056] | -0.1 [-0.1, -0.0] | +0.02 [+0.01, +0.03] | +0.00 [+0.00, +0.00] | +0.0 [+0.0, +0.0] |
| medium 100 ms | +0.030 [+0.000, +0.060] | -0.09 [-0.18, -0.01] | +0.011 [+0.002, +0.020] | +3.7 [+3.1, +4.3] | +1.05 [+0.87, +1.24] | +0.00 [+0.00, +0.00] | +0.0 [+0.0, +0.0] |
| ramp / rung3 + follow-up | -0.075 [-0.110, -0.041] | +0.45 [+0.36, +0.55] | -0.127 [-0.138, -0.115] | -0.2 [-0.2, -0.1] | -0.02 [-0.04, +0.00] | +0.00 [+0.00, +0.00] | +0.0 [+0.0, +0.0] |
| ramp+split / rung3 + follow-up | -0.073 [-0.108, -0.039] | +0.43 [+0.34, +0.53] | -0.121 [-0.133, -0.110] | -0.2 [-0.2, -0.1] | -0.02 [-0.04, +0.00] | +0.00 [+0.00, +0.00] | +0.0 [+0.0, +0.0] |
| ramp / re-anchor + follow-up | -0.011 [-0.020, -0.003] | +0.04 [+0.01, +0.06] | -0.010 [-0.013, -0.008] | -0.0 [-0.1, -0.0] | -0.01 [-0.02, -0.00] | +0.00 [+0.00, +0.00] | +0.0 [+0.0, +0.0] |
| ramp+split / re-anchor (comparator) + follow-up | -0.003 [-0.008, +0.000] | +0.01 [+0.00, +0.02] | +0.000 [+0.000, +0.000] | -0.0 [-0.0, -0.0] | -0.01 [-0.02, -0.00] | +0.00 [+0.00, +0.00] | +0.0 [+0.0, +0.0] |
| ramp+split / re-anchor z=3 + follow-up | -0.040 [-0.073, -0.008] | +0.27 [+0.20, +0.35] | -0.065 [-0.074, -0.056] | -0.1 [-0.1, -0.0] | +0.01 [-0.00, +0.03] | +0.00 [+0.00, +0.00] | +0.0 [+0.0, +0.0] |

#### public threshold

| row | hard quality | crit. misses/stream | plain acc. | calls/stream | cost s/stream | esc. decoys/stream | esc. plain/stream |
|---|---|---|---|---|---|---|---|
| rung z=3 | -0.094 [-0.132, -0.056] | +0.55 [+0.44, +0.66] | -0.124 [-0.136, -0.113] | -2.5 [-2.7, -2.2] | -0.29 [-0.36, -0.22] | -0.66 [-0.76, -0.56] | -2.6 [-2.9, -2.3] |
| rung z=2 | -0.062 [-0.086, -0.037] | +0.28 [+0.20, +0.35] | -0.066 [-0.073, -0.060] | -0.7 [-0.8, -0.6] | -0.22 [-0.25, -0.19] | -0.39 [-0.47, -0.32] | -1.8 [-2.0, -1.7] |
| re-anchor | -0.008 [-0.018, +0.000] | +0.12 [+0.07, +0.17] | -0.009 [-0.012, -0.007] | -0.7 [-0.8, -0.6] | -0.21 [-0.24, -0.18] | -0.31 [-0.38, -0.25] | -0.5 [-0.6, -0.4] |
| ramp / rung3 | -0.094 [-0.132, -0.057] | +0.45 [+0.35, +0.55] | -0.121 [-0.132, -0.110] | -1.8 [-2.0, -1.6] | -0.09 [-0.16, -0.03] | -0.35 [-0.42, -0.28] | -2.2 [-2.5, -2.0] |
| split / rung3 | -0.091 [-0.129, -0.054] | +0.53 [+0.42, +0.64] | -0.119 [-0.131, -0.108] | -2.4 [-2.7, -2.2] | -0.29 [-0.36, -0.22] | -0.64 [-0.74, -0.54] | -2.5 [-2.8, -2.3] |
| ramp+split / rung3 | -0.091 [-0.129, -0.054] | +0.43 [+0.34, +0.53] | -0.116 [-0.127, -0.106] | -1.7 [-1.9, -1.5] | -0.09 [-0.15, -0.03] | -0.34 [-0.41, -0.26] | -2.2 [-2.4, -1.9] |
| ramp / re-anchor | -0.008 [-0.016, +0.000] | +0.03 [+0.01, +0.06] | -0.008 [-0.010, -0.005] | -0.0 [-0.0, +0.0] | -0.00 [-0.01, +0.00] | -0.03 [-0.04, -0.01] | -0.1 [-0.2, -0.1] |
| split / re-anchor | +0.000 [-0.006, +0.006] | +0.10 [+0.06, +0.14] | -0.001 [-0.002, -0.000] | -0.7 [-0.8, -0.6] | -0.20 [-0.23, -0.18] | -0.29 [-0.35, -0.22] | -0.4 [-0.5, -0.3] |
| re-anchor z=3 | -0.065 [-0.100, -0.029] | +0.39 [+0.29, +0.48] | -0.069 [-0.079, -0.060] | -2.5 [-2.8, -2.3] | -0.34 [-0.42, -0.28] | -0.57 [-0.67, -0.48] | -1.4 [-1.6, -1.2] |
| ramp+split / re-anchor z=3 | -0.056 [-0.091, -0.022] | +0.27 [+0.19, +0.34] | -0.058 [-0.067, -0.049] | -1.8 [-2.0, -1.6] | -0.11 [-0.17, -0.06] | -0.25 [-0.31, -0.18] | -1.0 [-1.1, -0.8] |
| medium 100 ms | +0.027 [-0.003, +0.057] | -0.09 [-0.17, -0.01] | +0.006 [-0.002, +0.014] | +4.6 [+4.1, +5.2] | +0.94 [+0.79, +1.09] | +0.24 [+0.14, +0.35] | +2.2 [+1.8, +2.6] |
| ramp / rung3 + follow-up | -0.097 [-0.135, -0.059] | +0.46 [+0.36, +0.56] | -0.121 [-0.132, -0.110] | -2.1 [-2.3, -1.9] | -0.19 [-0.26, -0.13] | -0.36 [-0.44, -0.28] | -2.6 [-2.9, -2.3] |
| ramp+split / rung3 + follow-up | -0.094 [-0.132, -0.057] | +0.44 [+0.35, +0.54] | -0.116 [-0.127, -0.105] | -2.1 [-2.3, -1.9] | -0.19 [-0.25, -0.12] | -0.35 [-0.42, -0.27] | -2.5 [-2.8, -2.3] |
| ramp / re-anchor + follow-up | -0.011 [-0.020, -0.003] | +0.04 [+0.01, +0.07] | -0.008 [-0.010, -0.005] | -0.3 [-0.3, -0.2] | -0.08 [-0.10, -0.06] | -0.04 [-0.06, -0.01] | -0.4 [-0.4, -0.3] |
| ramp+split / re-anchor (comparator) + follow-up | -0.003 [-0.008, +0.000] | +0.01 [+0.00, +0.02] | +0.000 [+0.000, +0.000] | -0.2 [-0.3, -0.2] | -0.08 [-0.09, -0.06] | -0.01 [-0.03, +0.01] | -0.3 [-0.3, -0.2] |
| ramp+split / re-anchor z=3 + follow-up | -0.059 [-0.094, -0.025] | +0.28 [+0.20, +0.35] | -0.057 [-0.067, -0.049] | -2.1 [-2.4, -1.9] | -0.22 [-0.28, -0.15] | -0.26 [-0.32, -0.19] | -1.3 [-1.5, -1.1] |

#### public change

| row | hard quality | crit. misses/stream | plain acc. | calls/stream | cost s/stream | esc. decoys/stream | esc. plain/stream |
|---|---|---|---|---|---|---|---|
| rung z=3 | -0.070 [-0.108, -0.032] | +0.58 [+0.47, +0.70] | -0.124 [-0.135, -0.113] | -1.5 [-1.7, -1.2] | -0.07 [-0.14, -0.00] | -0.23 [-0.34, -0.12] | -2.4 [-2.7, -2.2] |
| rung z=2 | -0.038 [-0.057, -0.019] | +0.31 [+0.23, +0.39] | -0.066 [-0.073, -0.059] | -0.7 [-0.8, -0.6] | -0.17 [-0.20, -0.13] | -0.38 [-0.46, -0.30] | -1.7 [-1.9, -1.5] |
| re-anchor | -0.013 [-0.025, -0.003] | +0.16 [+0.11, +0.22] | -0.009 [-0.012, -0.007] | -0.7 [-0.8, -0.5] | -0.19 [-0.22, -0.15] | -0.33 [-0.41, -0.26] | -0.4 [-0.5, -0.3] |
| ramp / rung3 | -0.070 [-0.108, -0.033] | +0.45 [+0.35, +0.55] | -0.121 [-0.132, -0.110] | -0.7 [-0.9, -0.5] | +0.14 [+0.08, +0.20] | +0.17 [+0.09, +0.26] | -2.1 [-2.3, -1.8] |
| split / rung3 | -0.067 [-0.106, -0.029] | +0.57 [+0.45, +0.68] | -0.119 [-0.130, -0.108] | -1.5 [-1.7, -1.2] | -0.07 [-0.14, -0.00] | -0.21 [-0.33, -0.11] | -2.3 [-2.6, -2.1] |
| ramp+split / rung3 | -0.067 [-0.106, -0.030] | +0.43 [+0.33, +0.53] | -0.116 [-0.127, -0.105] | -0.7 [-0.9, -0.5] | +0.14 [+0.08, +0.20] | +0.19 [+0.10, +0.27] | -2.0 [-2.3, -1.8] |
| ramp / re-anchor | -0.011 [-0.020, -0.003] | +0.03 [+0.01, +0.06] | -0.008 [-0.010, -0.005] | -0.0 [-0.0, +0.0] | -0.00 [-0.01, +0.00] | -0.02 [-0.03, +0.00] | -0.1 [-0.2, -0.1] |
| split / re-anchor | +0.003 [-0.023, +0.028] | +0.13 [+0.07, +0.18] | -0.000 [-0.002, +0.001] | +0.1 [-0.1, +0.2] | -0.11 [-0.16, -0.07] | +0.18 [+0.08, +0.29] | -0.2 [-0.3, -0.1] |
| re-anchor z=3 | -0.043 [-0.079, -0.008] | +0.42 [+0.32, +0.52] | -0.069 [-0.078, -0.060] | -1.5 [-1.8, -1.3] | -0.12 [-0.19, -0.05] | -0.14 [-0.25, -0.03] | -1.2 [-1.4, -1.0] |
| ramp+split / re-anchor z=3 | -0.035 [-0.071, +0.000] | +0.26 [+0.19, +0.34] | -0.057 [-0.066, -0.049] | -0.7 [-0.9, -0.5] | +0.12 [+0.06, +0.18] | +0.28 [+0.20, +0.36] | -0.8 [-1.0, -0.6] |
| medium 100 ms | -0.027 [-0.061, +0.008] | +0.02 [-0.07, +0.11] | +0.004 [-0.003, +0.012] | +0.9 [+0.6, +1.2] | -0.11 [-0.19, -0.02] | -0.03 [-0.14, +0.08] | +0.4 [+0.1, +0.6] |
| ramp / rung3 + follow-up | -0.073 [-0.111, -0.035] | +0.46 [+0.36, +0.56] | -0.121 [-0.132, -0.110] | -1.0 [-1.2, -0.8] | +0.05 [-0.02, +0.11] | +0.17 [+0.08, +0.25] | -2.4 [-2.7, -2.2] |
| ramp+split / rung3 + follow-up | -0.070 [-0.109, -0.032] | +0.44 [+0.34, +0.54] | -0.116 [-0.127, -0.105] | -1.0 [-1.2, -0.8] | +0.05 [-0.01, +0.12] | +0.18 [+0.09, +0.26] | -2.3 [-2.6, -2.1] |
| ramp / re-anchor + follow-up | -0.016 [-0.027, -0.006] | +0.04 [+0.01, +0.07] | -0.008 [-0.010, -0.005] | -0.2 [-0.3, -0.2] | -0.07 [-0.09, -0.06] | -0.02 [-0.04, +0.00] | -0.4 [-0.5, -0.3] |
| ramp+split / re-anchor (comparator) + follow-up | -0.005 [-0.012, +0.000] | +0.01 [+0.00, +0.02] | +0.000 [+0.000, +0.000] | -0.2 [-0.3, -0.2] | -0.07 [-0.09, -0.06] | -0.01 [-0.02, +0.01] | -0.3 [-0.3, -0.2] |
| ramp+split / re-anchor z=3 + follow-up | -0.038 [-0.073, -0.003] | +0.27 [+0.19, +0.36] | -0.057 [-0.066, -0.048] | -1.0 [-1.3, -0.8] | +0.02 [-0.04, +0.09] | +0.27 [+0.19, +0.35] | -1.1 [-1.4, -0.9] |


### What the paired differences show, row by row (numbers; no ranking)

Against ramp + split over the re-anchor, same selector, hard quality (threshold / change / oracle): the rows
over the rung at z = 3 -0.07 to -0.10 (rung z = 3 alone -0.094 [-0.132, -0.056] / -0.070 [-0.108, -0.032] /
-0.073 [-0.107, -0.039]); re-anchor z = 3 -0.065 / -0.043 / -0.046; ramp + split over re-anchor z = 3 -0.056
[-0.091, -0.022] / -0.035 [-0.071, +0.000] / -0.038 [-0.070, -0.006]; rung z = 2 -0.062 / -0.038 / -0.035; the
re-anchor -0.008 [-0.018, +0.000] / -0.013 [-0.025, -0.003] / -0.011 [-0.020, -0.003]; ramp over re-anchor -0.008 /
-0.011 / -0.008; split over re-anchor +0.000 [-0.006, +0.006] / +0.003 [-0.023, +0.028] / -0.003 [-0.008, +0.000];
the medium +0.027 [-0.003, +0.057] / -0.027 [-0.061, +0.008] / +0.030 [+0.000, +0.060]. Critical misses per stream, the medium minus the
comparator: -0.090 [-0.175, -0.005] (threshold), +0.020 [-0.070, +0.110] (change), -0.095 [-0.185, -0.005]
(oracle). Cost per stream, the re-anchor minus the comparator: -0.207 [-0.237, -0.178] (threshold), -0.186
[-0.219, -0.153] (change), -0.124 [-0.149, -0.101] (oracle): the leak notices are what the ramp adds to the bill
at the oracle (+0.12 s, 18% of 0.67 s), and under a public selector they are +0.21 s on 7.75 s (2.7%).

### The follow-up rule, as a row beside its base

The rule's counts on the held-out streams (the comparator row, rule minus no rule; the counts of retirements by the
rule are identical under every selector, the rule acting some 6 s after the notice, before the 16 s question):

| per stream, 200 streams | retired by the rule | context |
|---|---|---|
| decoy notices | +0.240 [+0.175, +0.315] = 48 (all before any question) | 601 notices on 541 decoys (8.0%); the ramp adds 154 of them |
| plain notices | +0.785 [+0.690, +0.885] = 157 | the ramp adds about 194 |
| hard (non-leak) notices | +0.050 [+0.025, +0.080] = 10 | |
| leak notices | +0.035 [+0.015, +0.055] = 7 | 139 leaks |
| background notices | +0.035 [+0.015, +0.060] = 7 | |

Leaks lost to the rule (a leak with a notice retired by the rule and no notice asked about): 2 of 139 under the
oracle, always-escalate and the change rule, 3 under the threshold rule, 7 under never-escalate (+0.014 [+0.000,
+0.033] and +0.022 [+0.006, +0.044] as shares for the oracle and the threshold rule). **All 7 retired leak notices are the notice anchored at the leak's first observation
(offset 0)**; for 5 of the 7 leaks the counter's chain was noticed again 11 to 15 s later at a wrong anchor
(anchor-correct false) and that later notice was asked about, so the leak was not lost but anchored late; 2 were
never asked about. Leak quality, with the rule minus without it: -0.014 [-0.039, +0.008] (the same under the oracle, the
threshold and the change rule); leaks noticed 0.986 either way, because the notice is on the record. Hard
anchor-correct, background and every other notice measure are unchanged to the digit (+0.000) except
background notices per stream +0.105 [+0.070, +0.140] (the freed counters re-notice at background).
Paired effect of the rule on cost (threshold rule): calls -0.245 [-0.300, -0.190], cost -0.078 s [-0.094, -0.063]
(-1.0%), escalations on plain -0.260 [-0.315, -0.205], on decoys -0.010 [-0.025, +0.005]; hard quality -0.003
[-0.008, +0.000], critical misses +0.010 [+0.000, +0.020]. Under the oracle: calls -0.025 [-0.045, -0.010].

**Decoy notices retired before escalation, as the queue asks**, appear in two measures that differ and must not
be confused (E4, "judgement 3"): "retired before escalation" counts a retirement by quiet as well, so it is 1.000
for every oracle arm (the oracle asks about no decoy) and 0.45-0.61 (threshold) and 0.44-0.76 (change) across the
rows (0.462 and 0.604 for the comparator): it is a fate, not a saving. "Retired by the follow-up rule before
escalation" is the rule's own: 8.0% of the notices on decoys, and under the threshold rule it spares 0.010 calls
per stream.

### Do leaks and decoys separate publicly after the first five readings? (post hoc, `b4_separation.py`)

A reading of the 137 leak, 154 decoy, 194 plain, 11 hard and 15 background notices the ramp adds on the
held-out streams (comparator arm, anchors not shared with the split-only arm; the evaluator's own counts
differ by one or two in the small classes), with the readings that follow each notice within 6 s from the
public stream (`b4-separation.csv`, `b4-separation-roc.csv`). It chooses nothing: the rule's parameters were
fixed on the tuning streams, and it reads the evaluator's class labels as an analysis, not as an input.

| class | notices | median last reading minus completing reading | share whose last reading is at least the completing one | median largest fall below the peak |
|---|---|---|---|---|
| leak | 137 | +18 | 0.985 | 0 |
| decoy | 154 | +16 | 0.688 | 6 |
| plain | 194 | -17 | 0.232 | 55 |
| hard | 11 | -19.5 | 0.182 | 71 |
| background | 15 | +4 | 0.533 | 21 |

The probability that a leak's value of "last minus completing" exceeds a decoy's is **0.63** (0.62 for the lowest
reading, 0.36 for the largest fall, 0.52 for the number of follow-up readings): barely above a coin. Against plain
and hard notices it is 0.95 and 0.91: **the readings after the fifth separate leaks from the plain and hard
anomalies the ramp also opens (they fall), and do not separate them from decoys (most keep rising for 6 s)**. At a
cut of "last at least completing" 98.5% of the leak notices and 68.8% of the decoy notices are kept; at +10, 97.1%
and 66.2%; at +15, 80.3% and 55.2%: the best cuts keep 97-98% of the leak notices and drop 31-34% of the decoy
notices. So the rule's whole effect is the plain notices it retires (157 of 194), not the decoys (48 of 154),
and the queue's hypothesis ("a decoy's readings turn benign") holds for about a third of the decoys in this world
and fails for two thirds, within this family of statistics (latest reading against the completing one, a
reversal, a count and a window). A statistic of another family (slope, plateau length, the length of the chain
over a longer horizon) is untested; the tuning table (`b4-tuning-follow.csv`) is the whole of the grid searched.

### Sensitivity: nothing in it chose anything

`b4-sensitivity.csv`: the 19 arms of the run the table does not use. The comparator row under every grid value of
each selector; the follow-up rule at the leak tolerances 2 and 4 (the tolerance 1 configuration equals the chosen
one) under always, threshold, change and never. Tolerating 2 or 4 retired leak notices on the tuning streams
retires 9 leak notices on the held-out streams (0.045 per stream) and decoy notices 51 and 52 against 48: the
decoys gained are 3 or 4 notices for 2 more leak notices.

## What the numbers say (my analysis as PI)

Separated as the contract asks: *measured* (from the run), *deduced* (arithmetic on measured values), *inferred*
(a mechanism I believe but did not isolate), *untested*.

### 1. The two public selectors are close to "ask about everything", and that decides most of what follows

*Measured.* Across the 17 rows the threshold rule makes 90-98% of the calls of always-escalate and the change
rule 77-99%; the bill is 4.0-12.0 times the oracle's. By class (threshold, comparator row): plain 79% of the
reasoner's cost, hard non-leak 7.5%, decoys 5.8%, background 4.6%, leaks 2.9%. The rules ask about 18-21 of the 21.3
plain incidents per stream. *Quoted, not re-measured here (R5, through the selector's module documentation):* the public
consistency checker contradicts 97% of plain anomalies at some point, so "contradictory or silent" does not
exclude plain incidents; the change rule's growth by 8 or 12 observations is reached by any burst that keeps
attaching readings. *Inferred:* with a 16 s delay, anything that went quiet before 16 s is already retired by the
rung (46% of decoy notices under the threshold rule, 60% under the change rule at the comparator), and what is still
live at 16 s is nearly all asked about. *Deduced:* the tuning rule's choice of near-always parameters is not an
accident of the rule: no grid value is selective (the most selective, k = 20, loses 0.07 of quality for 14% of the
calls and costs more per call). A selector that is selective on this world's anomalies is a different instrument
from the two the queue named, and these two do not bound what a public selector can do; they bound what a
threshold on the rung's verdict and a threshold on evidence growth can do.

The selection oracle is **not** a quality ceiling: public rows exceed it by up to 0.011 on hard quality (inside the
intervals) and are far above it on plain accuracy (+0.058 to +0.075) and critical misses (0.04 to 0.185 fewer per
stream), which it buys nothing on because it never asks about plain incidents. *Deduced:* the public selectors'
extra 7 s per stream is not purely waste: about 1.3 more plain incidents per stream are declared correctly (0.063
x 21.3; about 5 s each if all the extra cost is put against them), and the critical misses fall by about 0.12 per stream.
Hard quality per unit cost as the tuning rule defined it (hard quality only) hides this and favours not asking
about plain incidents, which no public rule here can identify.

### 2. What each noticer's decoy and late-plain notices cost once charged

| row (threshold rule) | decoy calls/stream | decoy s/stream | decoy share of reasoner cost | background calls/stream | plain s/stream |
|---|---|---|---|---|---|
| rung z=3 | 0.95 | 0.281 | 0.037 | 2.72 | 5.84 |
| re-anchor | 1.30 | 0.373 | 0.048 | 1.91 | 6.15 |
| split / re-anchor | 1.32 | 0.379 | 0.049 | 1.76 | 6.19 |
| ramp+split / re-anchor | 1.61 | 0.464 | 0.058 | 1.38 | 6.30 |
| ramp+split / re-anchor z=3 | 1.36 | 0.403 | 0.051 | 0.84 | 6.35 |
| medium 100 ms | 1.85 | 0.516 | 0.058 | 1.88 | 6.60 |

*Measured (threshold rule, t = 0 or 1 s, so rows are close to like for like).* Decoy notices cost 0.28-0.52 s per
stream, 3.7-5.8% of the reasoner's cost, and rise with how many decoys a noticer notices (the ramp's shape is the
decoys' shape, B3). Against the split-only row, the ramp adds **+0.085 s of decoy calls and +0.108 s of plain
calls per stream (+0.19 s, 2.4% of the bill)**, and +0.128 s of leak calls and -0.125 s of background calls (it
takes the background budget's counters): net +0.204 s (the re-anchor minus the comparator, which also removes the split, is -0.207 s [-0.237,
-0.178]) Under the change rule the same difference is decoys -0.047 s, plain +0.166 s, leaks +0.113 s,
background -0.127 s (net +0.112 s): **the sign of the decoy term flips because `k` was tuned per row (8 or 12)**, so
the decoy cost under the change rule is not comparable across rows; only the threshold rule's is. *Deduced:* under the threshold
rule the late-plain notices the ramp adds cost more than its decoys (0.108 s against 0.085 s) and together they
cost 1.5 times the leak notices the ramp exists to make (0.128 s); the oracle's table shows only the leaks (+0.12 s on
0.67 s), which is the sense in which it hid the cost: not by much in absolute terms (+0.19 s of 7.95 s).
*Untested:* how the decoy cost moves with the delay (R5's 16 s is fixed here; a shorter delay asks about decoys
before they go quiet, and the cost would be larger).

### 3. Is the medium's extra anomaly count paid for by its anchoring?

*Measured.* The medium notices 1.77 times per decoy (comparator 1.11) and, under the oracle, is asked 3.17 more
times per stream about leaks [2.57, 3.81] (3.86 calls on the 0.695 leaks per stream, 5.5 per leak, against 1.0 for
the comparator) and 0.51 more about hard non-leak incidents [0.39, 0.63]; 86% of its extra oracle calls are about leaks. Its gain
in anchor-correct hard non-leak incidents is +0.019 [+0.005, +0.033] (about 7 of 372). Against the comparator, same
selector (oracle / threshold / change), hard quality +0.030 [+0.000, +0.060] / +0.027 [-0.003, +0.057] / -0.027
[-0.061, +0.008] at a cost of +1.05 / +0.94 / -0.11 s per stream; critical misses -0.095 [-0.185, -0.005] / -0.090
[-0.175, -0.005] / +0.020 [-0.070, +0.110]; **leak quality -0.065 [-0.121, -0.008] / -0.302 [-0.375, -0.237] /
-0.597 [-0.669, -0.524]**, though the medium notices as many leaks as the ramp (0.993 against 0.986). *Deduced:*
(a) under the threshold rule the medium's +0.027 is about +0.05 correct hard non-leak incidents per stream
(0.027 x 1.86) for +0.94 s, with an interval that includes zero gain; (b) the extra anomalies are mostly the leak's,
not the hard incidents', and they do not turn into leak quality (it is lower under every selector that asks, the
interval excluding zero in each). *Result:* the medium's extra anomalies are **not shown to be paid for**
by its anchoring on hard non-leak incidents (the gain is 7 incidents, its interval touches zero in quality), and
not paid for in leaks; they are also **not shown unpaid**: the +0.030 and +0.027 and the critical-miss reduction
are real point estimates whose intervals are too wide at n = 200 to separate from zero (the critical-miss
intervals exclude zero by 0.005 only). *Inferred:* the medium's many anomalies per leak (about five) each carry
little evidence, so the change rule's `k = 12` starves them (leak quality -0.60) and the threshold rule asks about
them all (cost +0.94 s).

### 4. Do leaks and decoys separate publicly after the first five readings?

*Measured (held-out, post hoc):* AUC 0.63 for leak against decoy on "last reading minus completing reading",
0.95 against plain, 0.91 against hard (section above); the tuned rule retires 40 of 324 decoy notices and 0 leak
notices on the tuning streams, and on the held-out streams 48 of 601 decoy notices (31% of the ramp's 154) with 7
leak notices (2 leaks lost, 3 under the threshold rule), spares 0.01 decoy calls per stream and 0.25 calls in all.
*Result:* **not publicly separable by this family of statistics within a budget that forbids losing leaks**: only
2 of 192 configurations retire no leak notice on 100 tuning streams, and the one chosen, which retired none there,
retired 7 on 200 held-out streams (*inferred*: choosing, on 100 streams, the best of 192 configurations under a
zero-count constraint selects a configuration whose count of zero has a wide interval). The separation that does exist is leak against
plain and hard anomalies, which the rule exploits (157 plain notices retired). *Untested:* longer horizons, slope,
plateau and chain-length statistics; whether the 7 leaks are the same under another follow-up form.

### 5. Where the measures can still be gamed

1. **"Retired before escalation" is a fate, not a saving** (E4, judgement 3): 1.000 for every oracle arm and for
   never-escalate, which asks about nothing and scores 0.000 on hard quality. A selector that asks about nothing
   has zero decoy cost and every decoy notice retired before escalation. The decoy columns must be read with
   quality and critical misses, never alone.
2. **The class split of calls is the anchor's label** (E1, E2): a noticer that anchors a decoy notice on a
   neighbouring plain or background observation moves the call between columns and leaves the total unchanged. The
   total calls and total cost (class sums) are the measure that cannot be moved by anchoring; the decoy column is not.
3. **The follow-up rule's retirement counts include retirements that spare nothing:** a rule that withdraws every
   anomaly 6 s after notice "retires" every decoy before escalation (and every leak); the decoys' retirements
   spared 0.010 calls per stream because they would have gone quiet before 16 s. The counts must be read with
   calls spared and leaks lost, and the leak-loss constraint holds only on the streams it was tuned on (0 on
   tuning, 7 notices on held-out).
4. **Tuning leaks into the margins:** the selectors' parameters, the follow-up rule and its form were chosen on 100
   streams against the evaluator's labelled measures (the follow-up's form was designed after looking at labelled
   readings on tuning streams, `b4_explore.py`: the route B2's gap, B3's chain and M2's graph also took). Tied
   near-optima differ by less than the intervals, so a selector's parameter is a draw, and the rows' parameters
   differ (the change rule's `k` of 8 or 12), which confounds cost comparisons across rows under that rule.
5. **The delay is a hidden selector:** R5's 16 s, chosen for the oracle, already retires everything that goes
   quiet within it; the table fixes it for every row, so a noticer that opens more short-lived anomalies looks
   cheaper than it would at a short delay.
6. **The selector's own computation is priced at 1 ms per stream** (the checker's substrate), and the public
   noticers' own operations at nothing: a public selector or noticer that does arbitrary work on the public stream is
   free in the model, while the medium's operations are billed (the M2/B3 asymmetry, unchanged).
7. **The 'hard quality per cost' tuning objective ignores plain accuracy and critical misses**, which the
   public selectors buy with most of their cost (section 1): under it the oracle's cost efficiency (0.71 hard
   quality per second against 0.07) is not a feasible public target.
8. **Effective sample size:** 139 leaks and 372 hard non-leak incidents in 200 streams; the cluster bootstrap resamples
   streams, and the intervals on leak measures have fewer effective clusters than 200.

### 6. What margins EXP-101 could feasibly set from this table

*Deduced from the paired intervals* (median 90% half-widths at n = 200, one selector, rows against the comparator;
`b4-paired.csv`): hard quality ±0.035 (±0.032 at the oracle); critical misses per stream ±0.08 to ±0.11; plain
accuracy ±0.009; hard anchor-correct ±0.019 (the medium's: +0.019 [+0.005, +0.033]); calls per stream ±0.2 (±0.55 for
the medium under the threshold rule); cost per stream ±0.06 s (±0.15 s for the medium; about ±0.02 s between
same-family rows under the oracle); decoy calls ±0.07 to ±0.11 per stream.
- **Resolvable:** a hard-quality or critical-miss margin of about ±0.05 and ±0.15 respectively (anything below
  about the interval's half-width is indistinguishable from zero); a cost non-inferiority margin of 5% of the bill
  (0.4 s), which the medium's +0.94 s [+0.79, +1.09] under the threshold rule exceeds with room; the bound "anchor-correct
  not below the public row's minus 0.02" (the medium's paired lower bound is +0.005, the rung rows are 0.06-0.08
  below, a clear separation).
- **Not resolvable at this size:** a cost margin of 1% (0.08 s) between noticers of different families (half-width
  up to 0.15 s); the decoy cost itself (+0.085 s per stream: of the size of the paired half-widths of total cost within a family,
  ±0.01 to ±0.06 s, and moving with plain and leak terms of the same size, in a bill that is 79% plain calls common to
  every row); a 0.02 margin on hard quality.
- **The table argues against a primary of "cost under a public selector" as it stands:** with these two selectors
  the cost axis is the plain-incident bill, not the noticer's anomalies, and the noticers differ by 2-12% in it. A
  selector with real selectivity, or a call-budgeted comparison (every arm at the same calls per stream), has to
  come first, or EXP-101's cost claim will be about the selector. If it is built as queued, the margins above are
  the feasible ones, the decoy-cost claim should be dropped, and critical misses and plain accuracy must be bounds,
  not folded into a per-cost score (section 5, point 7).
- The follow-up rule's saving (-0.078 s, -1%, -0.25 calls) is below a 5% cost margin; its leak-loss measure (7 leak
  notices, 2-3 leaks of 139) needs more than 200 streams to bound.

### 7. What I would test next (smallest high-information first)

1. **Selectivity before selectors:** on the tuning streams, the AUC of each public feature of an anomaly at 16 s
   (age, evidence count, services touched, the rung's score and verdict history, the number of live neighbours)
   for "anchored on a hard incident" against "anchored on plain", and the same for decoy against hard. If no
   public feature separates hard from plain, no public selector can approach the oracle on this world and EXP-101's
   selector arm is a floor, not a design; if one does, the selector is that feature, tuned under a call budget.
2. **Matched-call comparison:** the same table with each row's selector at one calls-per-stream budget (for example 5),
   so the noticers' decoy and background notices compete for a fixed number of questions.
3. **The delay sweep** (EXP-101's delta sweep) on the comparator, the medium and the re-anchor, to see how much of
   the decoy cost the 16 s delay hides.
4. **A leak-versus-decoy statistic of another family** (slope, plateau length, chain length over 10 to 30 s) by AUC
   on the tuning streams, validated on the held-out ones; if none gets above 0.8 the queue's hypothesis is closed on
   this world.

## Readings and deviations (each with its reason)

The predecessor's, as I found and checked them:

1. **Rows.** "Every B3 row and M2's medium at 100 ms" is B3's eleven rows, the medium with M2's frozen graph (read
   from `m2-selected.json`, never changed), and, as rows of their own beside their bases, the follow-up rule on
   the five rows that have a ramp (17 rows). The brief's item 3 does not say where the follow-up rule's results
   belong in the table; the choice was the predecessor's and I kept it because it gives the rule's effect as a
   paired difference against its own base (`b4-followup.csv`).
2. **The follow-up rule was tuned once**, over the comparator row's noticer under `always_escalate` at 16 s (the
   arm that asks about every anomaly live at the delay, so a retired anomaly is a call spared), and the same
   parameters were applied to the other four ramp rows, where it was not re-tuned. "Tuned under the same budget" is read as the
   background budget of 6.82 notices per stream (B3's and M2's), plus a quality constraint.
3. **The zero-leak constraint** (no leak notice retired on the tuning streams) is the predecessor's reading of
   "leaks wrongly retired" as a bound; it left 2 of 192 configurations feasible, and the choice among them was
   by the objective (most decoy notices retired). The sensitivity tolerances (1, 2, 4) are reported only.
4. **The selectors' tuning objective** (quality per modelled second above a 0.9 floor) is the predecessor's
   reading of "hard-incident quality per unit cost"; it picked near-always parameters in every row (section 1 of the
   analysis). It was fixed before the stage S run and not changed after; I did not retune.
5. **Readings of the brief's words in the selectors** are in the module documentation, written before any run: the
   threshold's "silent" is the rung's silence, not the anomaly's; the change rule's baseline is the count at the first
   step it sees the anomaly.
6. **`escalations about no notice`** are attributed by focus equal to anchor (E1); zero in every comparison arm.
7. **Hard quality** is B2's (hard non-leak incidents); leaks beside; plain accuracy, critical misses beside.

Mine:

8. **I did not rerun the held-out run.** The brief's condition for rerunning (the binary's provenance cannot be
   established) did not hold: the rebuilt binary is bit-identical. The run was played by the predecessor, and the
   interruption left it complete; I read its manifest, log, usage and outputs, and recomputed nothing of it but the
   table from its files. *Not established:* that no other worker's process shared cores 0-2 during the run. The
   log records no wait before it started; the internal/external ratio (0.945) is within 0.01 of the byte-identity
   replays (0.949, 0.955). Modelled costs, the table's numbers, do not depend on wall time.
9. **I fixed `scripts/b4_stats.py`** (`per_stream` doubled two columns, so the first held-out table failed with an
   index error before writing anything); the fix changes no number, only makes the frame well-formed, and the
   test I added fails on the old code (I ran it against it).
10. **I added** `scripts/b4_separation.py` (the post hoc leak-versus-decoy reading; the brief asks what separates
    them publicly and the tuned rule's retirement counts alone cannot say) and `scripts/b4_summary.py` (the report's
    condensed tables). Neither chooses anything. `b4_separation.py` uses the `dump` example's public stream and the
    evaluator's labels on the held-out streams, after the table: an analysis, not an input to any rule.
11. **The byte-identity gate was rerun at the final tree**, once before the restart (`xcheck6`) and once after
    (`xcheck7`: its directory was gone), though the binary is the same as for `xcheck5`: the brief asks for the
    identity check and the gate at the final tree is the check by running.
12. **The mutation testing** was not in the brief's numbered list ("fixtures and mutation checks as before" in the
    queue's acceptance clause): the predecessor had written the hand-mutant script and not run it. I ran it and
    cargo-mutants, and added tests for what they found (below).
13. **Deleted:** the run directories of `xcheck5`, `xcheck6`, `xcheck7` (after their hashes and CSVs were recorded)
    and, at the end, `b4-heldout-b5-rho0.7` (after its table, provenance and separation CSVs were reproduced and
    found unchanged); the scratch dumps; the proptest regression file (above). The tuning
    stages' directories were deleted by the predecessor after their CSVs. The disk had 3.3 GB free at its lowest
    (another worker's builds took it; below the 4 GB the brief asks me to keep: I could not raise it, only not add to it).

## Verification

**By running (this session).**
- The held-out table, the paired differences, the follow-up pairs, the latency table, the sensitivity table, the
  summary tables and the separation analysis are regenerated from `artifacts/runs/b4-heldout-b5-rho0.7` by
  `b4_table.py`, `b4_summary.py` and `b4_separation.py` (exit 0 each); the first attempt of `b4_table.py` failed
  on the doubled columns (deviation 9), nothing written.
- Provenance (`b4_provenance.py`, exit 0, rerun after the final gate): byte identity 62 of 62 at both trees;
  tuning manifests on 10000-10099 and the held-out on 20000-20199; incidents identical in every arm; four oracle
  arms equal to M2's, B3's table reproduced to 1.1e-16; no unattributed calls; 0 step-capped segments; nothing
  refused by the bill.
- The binary's provenance (rebuild, bit-identical), the manifest's equality with what the committed code builds,
  and the tuning choices recomputed from the tuning CSVs (all 34 selector choices, the follow-up choice at four
  tolerances).
- Byte identity recomputed by my own script of SHA-256 over both files of 62 arms (`xcheck7`, after the restart); the
  pre-restart session did the same for `xcheck5` and `xcheck6`, which I took from its record.
- Script tests: `analysis/tests/test_b4_scripts.py`, 13 tests, built on hand fixtures: the policies' spellings,
  the row set, both tuning rules (feasibility, objective, ties, the nothing-feasible path), the selection measures
  as pooled counts, a paired difference over shared resamples, one number per arm per measure with no doubled
  column (fails on the old `b4_stats.py`: I ran it against it), and the separation statistics, AUC and ROC.
- **Evaluator columns: fixtures and mutation results.** Ten hand-made cases in
  `crates/gordian-stream-eval/fixtures/selection-cases.json` (classes and cost, attribution, retirement at the call's own
  instant, the follow-up counts, a stream with nothing recorded, and four error cases), with expected verdicts worked
  out by hand from `RULES.md`; tests that every documented rule E1 to E8 is pinned by a case and every error variant
  is exercised; a property test against a reference written from the rules without the scorer's index; and the class
  words. **Mutation checks, after the tests that killed the first run's misses, rerun after the restart:** hand mutants
  (`scripts/b4_hand_mutants.py`, 48 one-line changes to `select.rs`, `noticer_follow.rs`, `public_threshold.rs`,
  `public_change.rs`, `harness.rs`, `results.rs`): **48 of 48 caught** (`b4-hand-mutants.csv`; the first run, 42 caught,
  5 survived, 1 not applied, is `b4-hand-mutants-first-run.csv`). cargo-mutants on `select.rs` (44 mutants): **38
  caught, 6 unviable, 0 missed** (the first run missed 2, both `IncidentClass::as_str` replaced by a constant string:
  the class words are now pinned by a fixture test); on `noticer_follow.rs`, `public_threshold.rs`,
  `public_change.rs` (66 mutants): **58 caught, 8 unviable, 0 missed** (the first run missed 1, `Follower::opened -> 1`)
  (`b4-cargo-mutants.csv`, one row per mutant). Limits of the claim: "caught" means a test in the named suites
  (`selection_fixtures`, `selection_properties`; `stream_follow`, `stream_b4`) failed, not that the whole workspace
  did; the 14 unviable mutants do not compile and say nothing about the tests; `harness.rs` and `results.rs` were
  mutated by hand only (3 mutants), not by cargo-mutants; cargo-mutants' operators do not drop a call or swap two
  names, which is why the hand mutants exist; the mutants were chosen by the author of the code. The property test
  writes a `proptest-regressions` file when a mutant breaks it; those seeds record the mutants and were not kept.

**Gates, on the final tree** (exit codes; every cargo call under `scripts/cgroup-run.sh` on cores 0-2, 3 GB, no OOM kill):

| gate | exit | notes |
|---|---|---|
| `cargo fmt --all -- --check` | 0 | the first run on the resumed tree exited 1 on one committed file (`selection_properties.rs`); fixed in `499c0d7` |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | 0 | the first run exited 101: `clippy::large_enum_variant` on the test enum `Expected` in `selection_fixtures.rs` (committed in `9632c84`, passed over before the restart); fixed with an `#[allow]` and a stated reason (boxing the verdict would compare a box with a reference in the test's own comparison) |
| `cargo test --locked --workspace --no-fail-fast` | 0 | 812 passed, 0 failed, 9 ignored (not this unit's), 68 test binaries; 126 s on the rerun (the first run, which compiled, peaked at 841 MiB) |
| `bash scripts/check-no-oracle.sh` | 0 | "check-no-oracle: ok" |
| `PYTHONPATH=analysis .venv/bin/python -W error -m pytest -q analysis` | 0 | 435 passed, 4 deselected |

The pytest and oracle-guard runs preceded the last two edits (the clippy `allow`, this report); the oracle guard
was rerun after them (exit 0); the analysis suite does not read either file.

**Assumed, not shown by running.**
- That the percentile cluster bootstrap over 200 streams describes the sampling uncertainty (leaks are 139
  incidents in at most 200 streams; the intervals on leak measures have fewer effective clusters).
- That nothing else used cores 0-2 during the held-out run (the log records no wait before it started; the
  internal/external ratio agrees with the replays); modelled costs do not depend on it.
- That the tuning stages' run directories (deleted by the predecessor) were as their CSVs say: I recomputed the
  choices from the CSVs, I did not replay the tuning runs (the manifests are kept, and a replay is deterministic).
- That `check-no-oracle.sh` is the right guard for "no hidden state in a policy's inputs" for the three new rules (it
  checks the sources for the accessor; the module documentation lists what each reads).
- That the post hoc separation analysis's "ramp-added" notices (anchors not shared with the split-only arm) are the
  notices the rule watches; its counts differ from the evaluator's by one or two in the small classes (194 plain
  against 192, 15 background against 16), and the rule's 7 retired leak notices against the 2 the analysis predicts
  at the same cut: the difference is the notices the rule frees (a retired anomaly lets the chain be noticed again),
  which the no-rule arm cannot show.
- The tuning-to-held-out reading of the leak-loss constraint (section 4 of the analysis) as selection on a small count.

## Runs and exit statuses

(`b4-driver-log.csv`, `b4-run-index.csv`; outputs in `artifacts/runs/` of the worktree, git-ignored.)

| run | exit | wall s | who | what |
|---|---|---|---|---|
| `xcheck5-r6-heldout-b5-rho0.7` | 13, then 0 | 0, then 155 | predecessor | the gate at `522709c`; the first attempt was refused by the driver (unclean tree), nothing ran |
| `b4-explore-b5-rho0.7` | none | | predecessor | waited 15 polls for another worker, no manifest, no output, never ran (superseded by the stage F control arm) |
| `b4-tunefollow-b5-rho0.7` | 0 | 301 | predecessor | stage F: 193 arms on the tuning streams, 38 waits |
| `b4-tuneselect-b5-rho0.7` | 0 | 452 | predecessor | stage S: 272 arms, 35 waits |
| `b4-heldout-b5-rho0.7` | 0 | 325 | predecessor | the held-out run: 104 arms, 200 streams; 0 step-capped; ratio 0.945; peak 0.82 GiB |
| release build of `gordian-run` at `6aca6aa` | 0 | 92 | me | provenance of the binary; under the runner, 3 GB limit, peak 0.77 GB |
| `xcheck6-r6-heldout-b5-rho0.7` | 0 | 188 | me | the gate at `7d2e9a0` (4 waits for other workers); ratio 0.955 |
| hand mutants, first run | 1 (5 survived, 1 not applied) | 541 | me | 48 mutants |
| cargo-mutants, first runs | 2, 2 (misses found; expected) | 77, 420 | me | 44 and 66 mutants |
| new tests (eval, `stream_follow`) | 0, 0 | | me | the tests that kill the misses |
| after the restart: `cargo fmt --check` | 1, then 0 | 1 | me | one hunk in `selection_properties.rs`; fixed |
| after the restart: `cargo test -p gordian-stream-eval` (properties, fixtures) | 0 | 19 | me | with the regression file present: 4 of 4 and 8 of 8 |
| cargo-mutants, `select.rs` (44 mutants) | 0 | 83 | me | 38 caught, 6 unviable, 0 missed |
| cargo-mutants, the three arm files (66 mutants) | 0 | 429 | me | 58 caught, 8 unviable, 0 missed |
| hand mutants (48) | 0 | 119 | me | 48 caught |
| `cargo build --release --locked -p gordian-run` at `6f306e6` | 0 | 48 | me | after 4 waits for another worker; binary SHA-256 `b2795f97...` |
| `xcheck7-r6-heldout-b5-rho0.7` | 0 | 152 | me | the gate at `6f306e6`, no waits; 62 of 62; ratio 0.950; peak 0.44 GiB |
| `b4_table.py`, `b4_summary.py`, `b4_separation.py`, `b4_provenance.py` | 0 each | 120, few | me | reran on the held-out run; the committed CSVs came out byte-identical; provenance rerun extended to `b4-regression-restart.csv` |
| gates: fmt, clippy, test, oracle guard, pytest | 0, 101 then 0, 0, 0, 0 | see above | me | clippy fixed as above, then fmt and test rerun |

Excluded or refused: nothing was excluded and no run was discarded. The bill refused nothing in any arm of the
held-out run (`b4-refusals.csv` is empty of refusals) and no segment was step-capped.

## Hidden record

I did **not** read `crates/gordian-stream/HIDDEN-DESIGN.md` and did not search it. I used the evaluator's labels (the
class of each incident) in two places, both after the table: `b4_separation.py` on the held-out streams (an analysis
of what the public readings carry; it feeds no rule), and the evaluator's own selection accounting, which classifies
notices by the hidden incident labels by design (E2) and is the one place that is allowed to. For the predecessor, I
can say that its commits, scripts and module documentation cite no hidden rule, that `b4_explore.py`'s header says
the follow-up rule's form was chosen after reading the evaluator's labels beside the public readings on the tuning
streams (the route B2's gap, B3's chain and M2's graph also took), and that the guard passes (below); I cannot say
more about what it read.

## What I am least sure of

1. **That the public selectors' near-non-selectivity is a fact about this world and not about the two rules'
   grids.** The grids contain no setting that is selective except k = 20, which loses quality; the next experiment
   in section 7 of the analysis (feature AUCs) is what would tell.
2. **The leak-loss reading of the follow-up rule:** 0 leak notices retired on 100 tuning streams, 7 on 200 held-out. I
   believe it is selection on a small count (inferred), not a drift of the world, and have not tested it (a
   second tuning seed set would).
3. **Decoy cost under the change rule across rows**, where `k` differs by row (8 or 12): I have not separated the
   noticer's effect from the parameter's. Only the threshold rule's decoy costs are close to like for like.
4. **Whether "ramp-added" notices in `b4_separation.py` are the notices the rule watches** (assumed above).
5. **The medium's cost under the threshold rule** (8.89 s) includes 22.9 calls on plain incidents like every row; I read
   its +0.94 s as the medium's anomalies, which assumes the medium does not change which plain incidents are asked about.

## What the chief should examine most carefully

1. **The selectors are almost "always escalate".** Everything in the cost column depends on it. Look at
   `b4-tuning-select.csv` (the grids and the choices, all with efficiency within 6-23% across a grid), the
   sensitivity rows, and the module documentation's statement of why (the checker contradicts 97% of plain anomalies).
   If you want EXP-101's cost claim to be about noticers, a selector with real selectivity or a call-budgeted
   comparison is a precondition (section 6 of the analysis).
2. **The follow-up rule's leak loss** (7 notices, 5 re-noticed late, 2-3 lost) against the zero-leak constraint it was
   tuned under: whether to accept a rule whose constraint did not transfer, and whether a second tuning set is
   needed before anything cites it.
3. **The decoy and late-plain cost figure (+0.19 s)** is a subtraction of two rows (comparator minus split-only) under
   the threshold rule; B3 showed the two pieces compose additively (interaction 0.000), which is what licenses
   attributing the difference to the ramp. Check `b4-table.csv`'s `esc_*_s_per_stream` columns.
4. **The oracle is not a quality ceiling and "hard quality per cost" omits plain accuracy and critical misses** (the
   public selectors' extra cost buys +0.06 plain accuracy and 0.12 fewer critical misses): the choice of what
   EXP-101's value is decides whether the public selectors look wasteful.
5. **The held-out run was not played by me.** What establishes it: the bit-identical binary, the manifest's equality
   with the committed code and selection file, a clean tree at its revision (the driver enforces it), the oracle arms
   equal to M2's byte for byte, B3's table reproduced. What does not: that no other process shared the cores.
6. **The branch does not merge cleanly into current main.** It carries B3 by a merge commit made when main did not yet
   have it (`435b728`, B3's tip `2ff928b`); main has since merged B3 itself (`491e752`). A dry run
   (`git merge-tree --write-tree main HEAD`, nothing written to the tree or any ref) reports one content conflict, in
   `crates/gordian-run/src/stream/arms/noticer.rs`, the file the predecessor's own merge conflicted in. I did not
   resolve it (no merge was asked for).
