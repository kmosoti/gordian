# E1: memory measures, the public record rung, schema extensions for A1b

Exploration (Lab 2, unit E1). Worktree `memory-measures`. Everything below is from runs and tests on
this machine; what was assumed rather than run is marked "assumed". Numbers are from the held-out
run `e1-heldout-b5-rho0.7` (seeds 40000-40199, 200 whole streams, 90% percentile intervals of a
cluster bootstrap over streams, 10,000 resamples, seed 9950) unless a line says otherwise.

## The result in one paragraph

The evaluator measures what a memory declared without being asked and what it got wrong (rules K1
to K10, `crates/gordian-stream-eval/RULES.md`), and the harness can run an arm that declares from a
table of earlier answers (the record rung, `noticer_record.rs`). The record rung is a public
baseline: its key is built from what the rung observes, with no hidden-side knowledge. Stream-
boundary reset is what separates a rung that is safe from one that is not. Without the reset it
captures a family-keyed share of hard recurrences (0.205) and costs 27.6% less, but 81% of the
recalls whose stored answer was right for its own incident come back wrong, 1.52 per stream, and
the plain incidents pay for it (excess +1.435 wrong unasked per stream). With the reset the rung is
nearly silent (0.06 recalls per stream) and its stale errors are a handful. No cell of the rung
meets W2's stale-error bound (collision share at most 0.20, at most 0.25 per stream) while also
capturing a useful share; the only cell that meets the bound at all (site-keyed, kinds, reset)
captures 7 of 83 hard recurrences (0.084). A1b's draft, evaluated with the record rung standing in
for the engram, comes out `stale_errors_exceed_the_bound`: every clause evaluates, which was the
point of the dry run, and the dry run is not a result about an engram.

## Commits, branch, gates

Branch `memory-measures` (base 2b453c3), pushed. Final commit: see the head of the branch (the hash
is in the hand-back message). Final gates, run at the final tree through `scripts/cgroup-run.sh`:

| gate | result |
|---|---|
| `cargo fmt --all -- --check` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo test --workspace --no-fail-fast` | 1013 passed, 0 failed, 14 ignored |
| `scripts/check-no-oracle.sh` | ok |
| analysis suite (`pytest -W error`, default marks) | 542 passed, 4 deselected |
| V1 back-test (four specifications, `v1_backtest.py`) | `verdict.json` and `verdict.md` of b3, l1, m2, m3 byte-identical to before the schema extension; `v1-backtest*.csv` unchanged |
| cargo-mutants on `memory.rs` (final code) | 108 mutants, 104 caught, 4 unviable, 0 missed |
| 21 hand mutants (`e1_hand_mutants.py`) | 20 caught; 1 survivor, a documented equivalent mutant |

The 14 ignored tests include the one that reproduces W2's counts (83 hard recurrences, 51 hard
elsewhere on held-out; 44 and 33 on tuning) from the evaluator; it was run explicitly and passes.

## Verified by running, and assumed

Verified: the evaluator's rules against 22 hand-written fixture cases and generated streams;
W2's counts reproduced; identity of the changed harness against the previous binary (below); the
held-out run replayed byte-for-byte (twice: once at 0181b7d, once at the final binary); the schema
extensions on the four existing specifications; the record rung's behaviour on tuning and held-out
streams.

Assumed, not run: that the engram of A1b will implement `recall_source` as specified (below); that
the held-out numbers carry to other worlds (one world, `b5-rho0.7`); that the cost model prices the
rung fairly (table lookups and binds are not priced, see "Cost").

## What was built

### Evaluator (K1 to K10)

`crates/gordian-stream-eval/src/memory.rs`; rules in `RULES.md` (section "Memory (work item E1)").

- K1 recurrence: a hard incident that repeats an earlier incident (same site, same hard kind).
- K2 same-family-earlier: class = hard kind plus mode `contradicts_early`; an earlier hard incident
  of that class at another site, and no recurrence. K1 and K2 together are "reachable" by a
  family-keyed memory.
- K3 unasked correct: a correct declaration about the incident and no escalation about it, whoever
  made it. K4 unasked wrong, likewise.
- K5 recall outcome and the source's class (right, wrong, unknown). K6 six cells (recall right or
  wrong against stored answer right or wrong) and `stale_wrong`. K7 totals. K8 errors. K9 site. K10
  purity (the measures read the same files an arm's public run produces plus the truth files the
  evaluator already reads; nothing reaches an arm).
- A recall carries a source (`obs`, `diagnosis`, `age`); a carried memory's source is judged against
  the truth of the stream it came from (`SourceTruth::Earlier`), not the current stream's.
- The analysis-side measures (collision share, inherited share, paired excess, curves) are in the
  "Derived measures" prose section, with the analysis code in
  `analysis/gordian_analysis/memory.py`.
- Tests: `tests/memory_fixtures.rs` (22 cases), `tests/memory_generated.rs`.

### Harness hooks (Lab 2's files, merge risk for Lab 1)

`Proposed.recall: Option<noticer::RecallSource>`; Noticer default methods `recall_source`,
`wants_consistency`, `gated_recalls`; `Rung` passthroughs; `StreamArm::handle_recalls`;
a block for gated recalls after the consistency checks; `set_monitor(rule.monitors() ||
noticer_wants_consistency())`. These are in `arms/mod.rs`, `rung.rs` and `noticer.rs`. Lab 1's
recall-gate branch touches the same files; I expect textual conflicts there, not semantic ones
(assumed, not tried).

New outputs per arm: `memory.csv`, `memory_incidents.csv`, `recalls.csv` (with `source_age`,
`source_seed`). `results.csv` gains `recall_declarations,noticer_ns` as the last two columns.
`cheap_declarations` still includes recalls; `total_cost_ns` is unchanged.

### The record rung (`arms/noticer_record.rs`)

`RecordNoticer<B: Noticer>` wraps the later re-anchor. The key is built from what the rung
observes: the kinds of the observations, their bands, and a gate-delay band (cumulative levels
`kinds`, `bands`, `timing`); the site form adds the site id; message ids are optional and off by
default. The table is last-answer-wins with a `disputed` flag. A gate on the rung's own consistency
checker (`contradicted_since`, with `settle_ns` = 1 s) decides whether a recall may be declared.
Confirmation policies: `never`, every k-th recall (count carried across segments), and
`on_contradiction`. Reset is a switch; with no reset a process-wide carry store holds
`(recall_count, segments, table)` across streams in seed order. Spec ids `record_site` and
`record_family`.

The module documentation, the tuning rule and the two open tuning cases were written and committed
(75e7298, ca3b3a1) before any run. Decisions on the open cases: if no level meets the bound the
level is `timing` and the cell is marked `fails_bound`; if no k meets it, the k with the smallest
collision share is chosen, ties to the larger k.

## Tuning (seeds 10000-10099, never used for the held-out)

Stage 1 (policy `never`, three levels per cell; `e1-tuning-stage1.csv`, `e1-selected.json`):
site/reset keeps `kinds` (meets the bound: 0 collisions in 8 right-source recalls); site/carry,
family/reset and family/carry keep `timing` and are marked `fails_bound` (collision shares 0.62,
0.54, 0.82). Stage 2 (k of 2, 4, 8 and on_contradiction at the stage 1 level): k = 4 (site, reset),
4 (site, carry), 8 (family, reset), 2 (family, carry). The bound "met" in the tuning tables is a
property of 100 streams and few recalls (4 to 11 recalls for the reset cells); it is weak
evidence, and the held-out confirms the reset cells at one collision in 9 recalls with an interval
of [0, 0.333].

A first attempt of tuning stage 1 failed with a harness defect: a carried memory's source
observation numbers belong to an earlier stream and were judged against the current one. The
failed run is kept as `artifacts/runs/e1/e1-tune1-b5-rho0.7.failed-1-harness-defect-carried-source`
(not committed; `artifacts/runs/` is ignored), the defect fixed by adding `age`, `focus_truths`,
and `SourceTruth::Earlier`, and stage 1 rerun. No result of the failed attempt was used.

## Held-out results (200 streams)

Control: the memoryless re-anchor under the same selection oracle, the same arm W2's ceiling uses:
459 reasoner calls, 512 hard incidents, 83 recurrences, 51 hard elsewhere; 0.690 s and 2.295 calls
per stream. Full table: `e1-heldout-table.md` (22 arms; every sensitivity row is labelled).

| arm | hard recurrences, unasked correct | hard elsewhere | all hard, unasked correct | collision share | collisions per stream | cost per stream |
|---|---|---|---|---|---|---|
| control (memoryless) | 0/83 | 0/51 | 0/512 | n/a | 0 | 0.690 s, 2.295 calls |
| site, kinds, reset (k4) | 7/83 = 0.084 [0.038, 0.138] | 0/51 | 7/512 = 0.014 | 1/9 = 0.111 [0, 0.333] | 0.005 | 0.672 s, 2.235 calls |
| family, timing, reset (k8) | 4/83 = 0.048 [0.013, 0.089] | 0/51 | 4/512 = 0.008 | 4/8 = 0.50 [0.20, 0.80] | 0.02 | 0.682 s |
| site, timing, carried (k4) | 0.060 (5/83) | 0/51 | 12/512 = 0.023 | 24/36 = 0.667 | 0.12 | 0.670 s |
| family, timing, carried, never | 17/83 = 0.205 [0.135, 0.278] | 4/51 = 0.078 | 68/512 = 0.133 [0.108, 0.159] | 304/375 = 0.811 [0.773, 0.846] | 1.52 | 0.499 s, 1.680 calls |
| family, timing, carried, k2 | 1/83 = 0.012 | 2/51 | 10/512 = 0.020 | 58/75 = 0.773 | 0.29 | 0.920 s, 3.505 calls |

Beside W2's ceiling (`e1-ceiling.csv`; a ceiling is what a perfect memory reaches, the numbers
below are shares of hard incidents or of the whole bill):

- Site-keyed: W2's ceiling R = 41/83 = 0.49 of hard recurrences are recurrences of an incident the
  arm declared correctly earlier. The best site cell (reset, kinds) reaches 7/83 = 0.084 of all
  recurrences, which is 7 of those 41. Why the other 34 are not recalled (the key, the gate, the
  settle time) was not diagnosed; the arm-internal counters that would say are not recorded.
- Family-keyed carried: W2's F_cross ceiling is 486 of 494 hard incidents from stream 11 onward
  (0.98 of the bill). The rung reaches 68/512 = 0.133 of hard incidents against that 0.98, with 277
  recalls whose stored answer was already wrong ("inherited": 1.385 per stream) and only 80 of its
  661 recalls right.

Stale errors, in incidents with an unasked wrong declaration that the memory caused (K6
`stale_wrong`), summed over the 200 streams:

| arm | plain | hard | decoy | wrong recalls (right source / wrong source) |
|---|---|---|---|---|
| site, kinds, reset | 2 | 3 | 1 | 1 / 6 |
| site, kinds, carried (sensitivity) | 279 | 52 | 85 | 227 / 214 |
| family, timing, reset | 6 | 2 | 0 | 4 / 4 |
| family, timing, carried, never | 372 | 47 | 120 | 304 / 277 |
| family, timing, carried, k2 | 75 | 31 | 35 | 58 / 109 |

Paired excess over the control, unasked wrong declarations per stream (`e1-heldout-excess.csv`;
the control's own errors cancel): family, timing, carried, never: plain +1.435 [1.245, 1.63], hard
+0.575 [0.48, 0.675] (120 hard incidents with an unasked wrong declaration against 5 in the
control), decoy -0.005, all +2.005 [1.78, 2.235]. Family reset: plain +0.02, hard +0.03.

What the family-carried rung captured (hard incidents declared correctly unasked, by family,
`e1-heldout-captured.csv`): compound 36 of 117, cascade 5 of 132, split brain 22 of 149, slow leak
5 of 114. It captured hard-correct declarations without adding any: hard declared correct stays
231 as in the control, and wrong declarations rise from 1551 to 1895. Across streams the cumulative
unasked-correct share rises with slope +0.046 per 100 streams over 200 streams [0.023, 0.069]
(first 100: +0.083); the reset forms are flat (slope -0.005 [-0.020, 0.006]).

Cost (cost = `total_cost_ns + noticer_ns`, modelled): the site reset cell saves 2.6% of cost
against W2's 8.5% bill ceiling for that set; the family carried cell saves 27.6% by making 0.6 fewer
reasoner calls per stream (2.295 to 1.680), most of the avoided calls on incidents it then declared
wrongly (see the stale-error table).

### What it does not show

- It does not show that a memory cannot meet the bound. The rung's key is as coarse as a public
  signature allows; an engram with a richer key could do better. The held-out says only that this
  key does not.
- It does not show the rung's numbers hold in another world: one world (`b5-rho0.7`), one stream
  order, one selection oracle.
- The unasked-correct share is "no escalation about the incident", not "decided without any
  help": a recall that came out right after another incident's escalation counts (the evaluator's
  definition, K3).

## Findings the chief should weigh

1. The public gate fires on plain incidents. In family-carried never, 382 of 661 recalls were on
   plain incidents, 125 on decoys, 130 on hard incidents and 24 on background; 654 of the 661 came
   from a source in an earlier stream (`recalls_source_age_pos`, `e1-heldout-captured.csv`). `contradicted_since` is a check that something changed after
   the declaration; plain incidents contradict early as well. This bears on A1c's gate: a gate of
   this form does not separate a hard recurrence from a plain incident.
2. Family carried with confirmation every second recall (k2) is worse than never confirming on
   hard-correct (176 against 231 for the control, 10 unasked-correct against 68 for never) and
   calls the reasoner more than the control (701 calls against 459). I did not diagnose this. My
   guess, which is a guess: confirmation asks early, with less context, and also asks plain
   incidents. It needs an experiment (id of its own) before anyone relies on it.
3. Site-carried is as unsafe as family-carried at lower scale: collision share 0.647, 0.22
   collisions per stream. Resetting is not a tuning decision; it is what makes the site form
   admissible.
4. In family-carried never, 277 of the 286 recalls with a wrong source (0.969) were wrong
   themselves: a wrong stored answer is nearly always repeated. Last-answer-wins with a `disputed`
   flag (set when a later answer bound to the key differs from the stored one, line 604 of
   `noticer_record.rs`) does not catch an answer that was wrong when stored until a different
   answer is bound to the same key.

## Schema extensions for A1b

In `analysis/gordian_analysis/criterion.py` (specification `gordian-criterion/1`, backward
compatible):

- a `not` node; `outcomes` (ordered, first match wins, shadowed outcomes are reported);
  `notnull` and `isnull` filters; a `count` clause with `on: count | total`; a `quantile` measure
  (inverted CDF, weighted resamples); a term `join` of two arms by (seed, incident) with
  `col@arm` columns.
- 24 tests in `tests/test_criterion_e1.py`. README and `TEMPLATE.md` describe the new constructs.
- V1 back-test: the four existing specifications (b3, l1, m2, m3) give byte-identical
  `verdict.json` and `verdict.md`; `v1_backtest.py` reproduces its committed CSVs.

`experiments/criteria/a1b.draft.json` is a DRAFT; the chief fixes the numbers and writes `a1b.json`
before any A1b run. Numbers marked PROPOSAL are mine; W2's are marked W2. Dry run on the held-out
with the record rung standing in for the engram (family carried never as "engram", family carried
k2 as "record"; a stand-in to show every clause evaluates):

| clause | stand-in result |
|---|---|
| power (83 recurrences at least 70; 134 reachable at least 120) | pass |
| floor_recurrences (at least 0.15, lower above 0.075) | 0.205 [0.135, 0.278]: pass |
| margin_over_record (+0.10, lower above 0) | +0.193 [0.124, 0.267]: pass |
| reachable_over_record (beside) | +0.134 [0.086, 0.187]: pass |
| collision_share_bound (at most 0.20, upper at most 0.30) | 0.811 [0.773, 0.846]: FAIL |
| collisions_per_stream (at most 0.25) | 1.52: FAIL |
| collisions_q90 (at most 1) | 4: FAIL |
| inherited not above record | +0.84 per stream: FAIL |
| plain_excess (upper at most 0.25) | +1.435 [1.245, 1.63]: FAIL |
| plain_displaced (point 0) | 0.02: FAIL |
| cost under record | -0.42 s: pass |
| outcome | `stale_errors_exceed_the_bound` |

The draft's own floor and margin clauses are met by a family-carried memory that is wrong most of
the time it speaks; the bound on stale errors is what stops it. That is the design (W2 10.2) and
the dry run shows it does stop it.

## Identity (the changed harness against the previous binary)

`e1_gate.py` replays the `r6-heldout-b5-rho0.7` run (62 arms) and compares each arm: the existing
`results.csv` columns (the last two stripped) re-hashed, `incidents.csv` hash, the new columns all
zero, `recalls.csv` empty. Result: 62 of 62 on four replays (`e1-regression.csv`, `-2`, `-3`, `-4`;
the last is at the final binary). The first held-out replay and the final one:
`e1-heldout-b5-rho0.7` and `e1-heldout-final-b5-rho0.7` agree on every file except the
wall-clock ones (`measured.csv`, `drift.csv`) and the manifests (source revision); 221 other files
are byte-identical. The final replay was made after the dead-computation removal in `memory.rs`,
at HEAD bc32553 with a clean tree.

## Overlap with other labs' processes (disclosure)

- Lab 1 reported a release build (08:27 to 08:29 UTC) that started while my `gordian-run` (pid
  29138) was live. The live run was identity replay 2 (`e1-xcheck2-r6-heldout-b5-rho0.7`,
  08:25:55 to 08:28:49, wall 174 s). It is flagged: its wall-time and resource figures are not
  clean. I reran it as `e1-xcheck3` (clean of overlap, 62/62) and again as `e1-xcheck4` (62/62).
  The results compared are logical-time and byte-compared, so they are unaffected either way. No
  held-out or tuning run was live in that window.
- One violation of my own: an early `cargo check` started at about 07:43 while Lab 1's
  `gordian-run` pid 1772 was live. My preflight printed the pid but the command chain did not stop
  on it. It is in `preflight.log` (scratch, not committed). No run of mine was live.
- Every other cargo command and run was preceded by the preflight (disk at least 6 GB free; wait
  while a `gordian-run` exists), and the waits are in the log.

## After the merge of `main` (A1c, A1d)

`main` (78f7fac) was merged into the branch (merge commit 106ff76). Conflicts were in `arms/mod.rs`,
`arms/rung.rs` and `arms/noticer.rs`: E1 and A1c built the same seam twice. Lab 1's implementation
is the one kept:

- `Noticer::needs_verdicts` (A1c) replaces E1's `wants_consistency`; the monitor is switched on from
  `rung.noticer_needs_verdicts()`.
- A1c's step order (consistency checks first, then one recall path for gated and ungated noticers),
  `Noticer::gated_recalls(now, views)` and `Rung::take_gated_recalls(now)` (with A1d's standing
  declarations) stay. E1's second gated block and its `take_gated_recalls(now, views)` are gone.
- E1 keeps what A1c lacks: `Noticer::recall_source`, `RecallSource`, `Rung::recall_source`, and
  `StreamArm::handle_recalls`, which records the recall's source on the proposal and reports
  `recall_declared` to the noticer (A1d), as the inline loop did.
- One addition: `Noticer::gated_recalls_in(now, store, views)`, default `gated_recalls`, which the
  rung calls. The record rung needs the store (it keys a recall on the observations held when the
  checker first found the anomaly inconsistent); A1c's engram does not and keeps overriding
  `gated_recalls`, unchanged. `RecordNoticer::gated_by` is the inherent form the unit tests call.

Behaviour changed by the resolution: for the record rung, none (its held-out run, replayed after the
merge as `e1-heldout-postmerge-b5-rho0.7`, is byte-identical to `e1-heldout-b5-rho0.7` on all 220
logical files; wall-clock files and manifests excluded). For an arm with a rule that monitors and a
noticer with recalls, the checks now run before the recalls (A1c's change, Lab 1's to verify; the
reproductions below cover the arms that exist). For every other arm, none.

Gates after the merge (through `scripts/cgroup-run.sh`): fmt clean (one `cargo fmt --all` write
pass was run outside the runner: it builds nothing), clippy `-D warnings` clean, `cargo test
--workspace` 1037 passed / 0 failed / 14 ignored, `check-no-oracle` ok, analysis suite 542 passed.
R6 identity, replay 5 (`e1-regression-5.csv`): 62 of 62 on the stripped-column reading.
`e1_merge_gate.py` (`e1-merge-reproduction.csv`): A1a's 8 arms, A1c's 15 arms and A1d's 15 arms
reproduce the kept runs under `/home/user/gordian/artifacts/runs/{a1a,a1c,a1d}/` (`incidents.csv`
and the existing columns of `results.csv`, `run_id` ignored), and A1d's 12 trace files are
byte-identical. For these medium arms the two new `results.csv` columns are not zero (they charge a
noticer and declare recalls) and `recall_declarations` equals the rows of `recalls.csv` in all 38
arms. A1d's `recall_source` is not implemented by the engram, so their recalls carry an unknown
source in `memory.csv`.

## Brief problems and the resolutions I chose

1. A tuning rule with two cases the brief left open (no level meets the bound; no k meets it):
   decided in a docs addendum before any run (above).
2. The brief's "`results.csv` gains columns": appended at the end so every consumer that reads by
   position keeps working; old fixtures compare on the columns they have
   (`without_appended_columns`).
3. The carried memory needs a source's age to be judged against the right stream; the brief's
   interface had no such field. Added `age` and the earlier-truth path. The medium engram must
   implement `recall_source` including `age`, or A1b's stale-error split (collision against
   inherited) is impossible for it.
4. The ban-word check (`check-no-oracle`) forced renaming in docs and re-exports.

## What I am least sure of

- That the record rung is the strongest public baseline. Its gate and its key are one choice among
  several; the sensitivity rows (bands, kinds, message ids) show the table moves a lot with the
  key, and a different key could change which side of the bound the family form lands on.
- The tuning bound is judged on 100 streams and 4 to 11 recalls for the reset cells; the held-out
  collision share for site/kinds/reset (1/9) has an interval up to 0.333, above W2's 0.30. It
  "meets the bound" on the point estimate and not on the upper limit; the A1b draft uses both.
- The k2 behaviour (finding 2) is undiagnosed.
- Cost: the table lookups and binds of the record rung are not priced; the rung's modelled cost is
  `total_cost_ns + noticer_ns`, where `noticer_ns` is the accepted `charge_noticer`. The
  27.6% saving is therefore an upper estimate of what a real implementation saves.
- Arm-internal statistics (snapshot sizes, number of unbound or disputed entries) are not
  measured; the table's growth and what the rung stores are inferred from its outputs.
- Mutation testing: the evaluator's `memory.rs` is covered (108, 104 caught, 4 unviable, 21 hand
  mutants). The arm-level mutation testing of `noticer_record.rs` was not done; its tests are the
  unit and generated-stream tests in `stream_record.rs`.

## What the chief should examine most carefully

1. K2's class definition (hard kind plus mode `contradicts_early`, at another site): it fixes the
   "reachable" population and so the draft's `reachable_*` clauses.
2. The collision/inherited split (K5, K6) with a carried source judged against its own stream's
   truth: it decides the A1b stale-error clauses.
3. That a recall counts as unasked correct even when the same-stream memory table was filled by an
   earlier incident's escalation (K3 reading).
4. The record rung's gate (finding 1) before A1c reuses it.
5. The hooks in `arms/mod.rs`, `rung.rs`, `noticer.rs` against Lab 1's recall-gate branch.

## What I would test next

- A new experiment id for the gate: does a gate that separates hard recurrences from plain
  incidents exist among public signals (A1c)?
- A diagnosis run for the k2 confirmation behaviour with arm-internal counters (asks per tier,
  context at ask time).
- Add arm-internal statistics to the recorder (snapshots, unbound counts) so the next memory arm's
  growth is measured rather than inferred.
- A second world, to see whether the family form's carried-across-streams numbers move.

## Files

- Evaluator: `crates/gordian-stream-eval/src/memory.rs`, `RULES.md`, `fixtures/memory-cases.json`,
  `tests/memory_fixtures.rs`, `tests/memory_generated.rs`.
- Rung and harness: `crates/gordian-run/src/stream/arms/noticer_record.rs`, `arms/mod.rs`,
  `rung.rs`, `noticer.rs`, `recorder` changes; tests `crates/gordian-run/tests/stream_record.rs`.
- Analysis: `analysis/gordian_analysis/{criterion,load,memory}.py`, tests `test_criterion_e1.py`,
  `test_memory.py`; `scripts/criterion.py`; `experiments/criteria/{README.md,TEMPLATE.md,a1b.draft.json}`.
- Scripts (`experiments/exploration/scripts/`): `e1_common.py`, `e1_gate.py`, `e1_run.py`,
  `e1_manifests.py`, `e1_select.py`, `e1_table.py`, `e1_a1b_draft.py`, `e1_hand_mutants.py`.
- Outputs (`experiments/exploration/`): `e1-regression{,-2,-3,-4}.csv`, `e1-tuning-stage{1,2}.csv`,
  `e1-selected.json`, `e1-heldout-{table,paired,excess,captured,curves,within}.*`, `e1-ceiling.csv`,
  `e1-hand-mutants{,-first-run}.csv`.
- Raw runs (git-ignored): `artifacts/runs/e1/`.
