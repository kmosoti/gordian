# Coordinator review log

What the coordinator checked for each merged unit, what it decided, and what it carried forward
to later units. Newest first. Reports from workers are model output; this log records what was
independently verified.

## R9 grounded distractor penalty — merged; the world's look-alikes cost a strong reader little

**Provenance.**

- The reader was frozen at `cccbcf2` (tag `r9-reader-frozen`) before any evaluation number;
  the evaluation commit `a16f839` follows it. Development on seeds 29000–29999, evaluation on
  30000–30399.
- Three tuning runs were refused once by the driver because the R10 worker's `cargo test` was
  running; they wrote nothing and were rerun. The core-sharing rule worked as intended.
- The ceiling and R4's oracle are byte-identical to R6's in all six runs.
- Gates on exit codes on the merged tree: fmt, clippy `--locked`, 576 Rust tests, the oracle
  guard, 339 analysis tests. Run outputs moved to `artifacts/runs/r9/` (ignored).

**Re-verified from raw files.**

- Reader on 312 hard evaluation questions: A(m) = 0.958, 0.939, 0.904, 0.913, 0.933 at
  m = 0, 50, 100, 200, 400; control 0.045; δ* = 0.0137 [0.0054, 0.0223] with the coordinator's
  own 2,000-resample cluster bootstrap (worker: 0.0135 [0.0055, 0.0225]).
- Rerun, tuning-selected builder, G = ceiling − builder:

  | δ | Selected builder | G [90%] |
  |---|---|---|
  | 0.0055 | `window` 80 s, N 512 | 0.032 [0.011, 0.054] |
  | 0.0135 | `cooccur` 16 s or `neighbourhood` k 4 (tie at 0.734) | 0.073 [0.046, 0.099] |
  | 0.0225 | `neighbourhood` k 4, N 256 | 0.089 [0.059, 0.118] |

  All match the worker.

**Verdict as written: unresolved.** The R6 regime needs G's upper bound below 0.10 at δ*_hi
(it is 0.118); the R7 regime needs G ≥ 0.10 at δ*_lo (it is 0.032).

**Coordinator error, recorded.** The feasibility note said the R6 regime needs roughly
δ*_hi < 0.05. It needs about 0.012, because G(0.05) was already 0.097 in R7. I read the regime
boundaries off R7's grid without interpolating. The criterion stands as written; the outcome is
unresolved on its own terms, and the error is in the feasibility note, not the clauses.

**What it means.**

- **In this world, look-alike distractors cost a strong reader very little.** A reader that
  knows the hidden rules and has learned from labelled development questions loses at most
  0.054 of accuracy at any distractor count up to 400, and recovers to 0.933 at 400. R8's crude
  reader lost 0.57. The world's confusability is real but small for a competent reader.
- **The curve is not exponential in count.** It dips at m = 100 and recovers, because random
  draws from the ±40 s pool include the incident's own non-decisive observations, which help
  (a split brain's burst). δ* is a summary, not a law, and the rerun turns it into a
  count-based penalty that the reader's curve does not quite obey.
- **Read as a point estimate, the context lever is modest.** At δ* the perfect-context ceiling
  beats the best simple builder by 0.073 [0.046, 0.099] on hard incidents, at 36× the references.
  That is more than R6's 0.024 and less than R7's "fragile" 0.14–0.34. EXP-102 can claim both a
  small quality margin and a large reference saving, with the quality margin's interval reaching
  from 0.03 to 0.12 across δ*'s interval.
- **Count-based penalties transfer to builder contexts, not to the rung's.** On R6's actual
  contexts the reader is within 0.05 of the curve for `window`, `cooccur` and `neighbourhood`.
  On the rung's own context it reads 0.659 against a predicted 0.953: the rung's context lacks
  the cascade's partner, which is missing information, not distraction. The simulated reasoner's
  `q` already models that loss.
- **What δ* is not.** It is the loss of one program with the hidden rules and learned weights
  on this world's questions. It says nothing about real models (R8 could not), and a stronger
  reader would give a smaller value, as every version during development did.

**Accepted with notes.**

- The reader learned softmax weights from labelled development questions. The plan allowed
  the hidden rules and forbade labels at evaluation; it did not say whether labelled training
  was allowed. The coordinator accepts it: the reasoner is the hidden side's stand-in, and a
  model trained on labelled diagnoses is what it stands in for. The reader is therefore stronger
  than a rule-only reader, and δ* is smaller for it.
- Six of 170 direct-check calls had an anchor more than 0.1 s after onset and the reader was
  wrong on all six. Its first-alarm-is-onset assumption is a weakness, post hoc.
- The tuning-selected builder is an unstable comparator at small δ: two builders tie at δ*, and
  the held-out-best builder would pass the R6 clause's upper bound at δ*_hi (0.097). The
  criterion is kept as written.

**Decided.**

1. **The reasoner's default for every preregistration is δ = 0.0135**, with the sweep
   {0, 0.0055, 0.0225, 0.05} required beside it, so that every claim is read across the interval
   and at the old default. This replaces the free δ = 0 default in the reasoner's spec for
   experiments, not in the code (the code's default stays 0 so that R6 and R7 replay).
2. **EXP-102's claim is references and cost at matched quality, with a secondary quality margin**
   against the best public builder, preregistered at 0.05 with the δ sweep. The worker's
   "fewest references within 0.05 of the ceiling" reading is the primary measure.
3. **The simulated reasoner's distractor penalty remains a count-based stand-in.** A
   composition-based law (penalising look-alike free-form messages rather than every
   reference) would fit the reader's curve better. It is not built now: the count-based law
   with the measured δ is within 0.05 of the reader on every builder context, and a new law
   would need a new byte-identity gate and would reopen R7. Recorded as a known weakness of
   the world, with the direct-check table as the evidence for revisiting it.
4. With R5, R6, R7, R9 and R10 done, the simulation-side headroom work for EXP-101 and EXP-102
   is complete. The next unit is EXP-101's preregistration.

## R10 salience ceiling — merged; noticing is worth 0.10 on burst families and 0.75 on the leak

**Provenance.**

- The new privileged arm reads two plan fields, `hard` and `first`, and nothing else. The
  coordinator grepped the diff for every plan accessor.
- R6's held-out run at b = 5, ρ = 0.7 replays byte-identical with the new binary: 62 of 62 arms,
  hashes recomputed by the coordinator. The R10 comparison arms equal R6's own files once the
  `run_id` column is dropped.
- 8 driver runs and 5 replays, all exit 0, none refused or excluded. No waits on the other
  worker.
- Gates on exit codes on the merged tree: fmt, clippy `--locked`, 572 Rust tests, the oracle
  guard, 326 analysis tests. Run outputs moved to `artifacts/runs/r10/` (ignored).

**Re-verified from raw files, rung's own context, 90% paired cluster bootstrap.**

| Setting | Result 1: hard, no leak | Result 2: slow leak |
|---|---|---|
| b = 5 (primary) | 0.586 − 0.489 = +0.097 [+0.062, +0.132], holds | 0.935 − 0.187 = +0.748 [+0.678, +0.814], holds |
| b = 2.5 | +0.024 [−0.013, +0.060], not shown | +0.324 [+0.252, +0.400], holds |
| b = 8 | +0.099 [+0.063, +0.136], holds | +0.777 [+0.709, +0.841], holds |

All match the worker's report to 0.001.

**Verdict as written.** Both results hold at the primary setting. Result 1 does not hold at
b = 2.5, where it is "not shown", not "equivalent": the interval reaches 0.06 and the full oracle
itself reaches only 0.538 there.

**What it means.**

- **Noticing is a real lever, and a larger one than the R6 correction estimated.** The R6
  entry predicted at most 0.091 from the never-noticed incidents. The measured gain is 0.097 with
  the rung's context and 0.156 with the window builder. The extra comes from asking earlier
  about incidents the rung notices late.
- **With a good context, noticing alone reaches the full oracle's quality.** `notice` with the
  window builder scores 0.952 on hard incidents, equal to R4's oracle, at 3.44 s per stream
  against 0.32 s. So R4's oracle decomposes as: noticing (this item) plus context (R6) plus
  selection (R5, cost only). Timing beyond notice + delay is worth little, which agrees with
  the R6 correction.
- **The slow leak is almost entirely a noticing problem.** The rung sees the leak only after
  its threshold crossing, a median 17.8 s late, and never for 40 of 80 diagnostic leaks.
  Noticed at its first reading, the leak is answered in 0.935 of cases with the rung's own
  context.
- **"Never noticed" is mostly mis-anchoring.** Every never-noticed hard incident has abnormal
  observations attached to an anomaly the rung anchored elsewhere, usually on background
  about 0.3 s earlier. The rung sees the activity and files it under the wrong anchor. This is
  a segmentation failure, which is close to what the charter's salience function is for.
- **The threshold sweep says the burst-family gap is partly tuning and the leak gap is not.**
  Lowering the notice threshold to z = 2 recovers 0.038 of the 0.097 at the price of doubled
  false alarms. No threshold helps the leak, because its sub-alarm readings are not abnormal
  observations at any threshold.

**Accepted with notes.**

- The injected notice is the arm's own record, not a rung anomaly, so it does not retire after
  6 s of quiet. Four of 270 diagnostic incidents were called by `notice` whose selection
  anomaly had retired before the delay. This is a small timing privilege inside the ceiling.
  It is recorded, not corrected; a ceiling that retires like the rung would be slightly lower.
- Result 2 mixes "whether it asks" with "when it asks" (about 16 s apart). The notice arm
  beats R4's oracle on the leak (0.935 against 0.906), which shows the ask time matters there.
- The never-noticed / late-noticed decomposition rests on 14 incidents and the late part's
  interval includes zero.
- The worker reported ignoring an instruction that arrived inside a tool result. Correct.

**Decided.**

1. EXP-101's registration names noticing as its first function, not threshold-versus-oracle
   selection. Its privileged ceiling is `oracle_notice` with the rung's context; its public
   baselines include the threshold sweep at z = 2 and a change-triggered rung.
2. The slow leak gets its own preregistered secondary measure in EXP-101, since it is where
   noticing matters most and where no threshold helps.
3. The anchoring finding is the first concrete job for a substrate mechanism: attribute
   activity to the right anchor. A public baseline for it is designed before any substrate is
   built.
4. R9's result decides whether the context half of the oracle is a quality or a cost lever.

## R8 real-model distractor sensitivity — merged; unidentifiable with these models

**Provenance.**

- **Code:** an evaluator-side question dumper behind the oracle feature, allowlisted in the guard;
  llama.cpp pinned at tag `b11429`, built outside the workspace in `artifacts/runtime/` (ignored);
  Qwen2.5 1.5B and 3B, Q4_K_M, with sha256 equal to the published values, in `artifacts/models/`
  (ignored).
- **Calls:** 277 in total, 260 of them scored. No parse failures, no errors, none truncated, none
  repeated.
- **Gates:** on exit codes on the merged tree: fmt, clippy `--locked`, 560 Rust tests, the oracle
  guard, 326 analysis tests.
- **Run outputs:** in `artifacts/runs/r8/`. The coordinator checked that they are identical to the
  worktree's before removing it.

**Re-verified from `calls.jsonl`.**

| Model | Hard questions | A(0) | 90% lower bound | Guess rate (control) |
|---|---|---|---|---|
| 1.5B | 28 | 0.500 | 0.357 | 0.357 |
| 3B | 16 | 0.250 | — | 0.250 |

- For the 1.5B, A(m) at m = 50, 100, 200 and 400 is 0.357, 0.393, 0.429 and 0.357, all at the guess
  rate. All of these match the worker's report.

**Verdict as written: unidentifiable with these models.** The failure is structural, not a matter
of sample size.

- The 1.5B model gains only 0.14 from having all of the evidence. That is below the 0.15 the
  precondition requires, so it would fail at any N.
- The 3B does no better than its own guess rate on the main questions.
- Neither model reads this world's evidence well enough for its loss to distractors to be
  measured.

**Coordinator notes.**

- **The plan's sizing was optimistic.**
  - The reasoner prompt needs about 5,200 tokens: the rules plus 14 worked examples.
  - Prompt evaluation on three cores runs at about 50–75 tokens per second.
  - So the 4-hour envelope allowed 28 hard questions, not up to 120.
  - The worker computed, before the main run, that even a model passing the precondition could not
    have produced an interval narrow enough for either regime at this N. The pilot did its job;
    the plan should have expected this.
- **Prompt development used 12 rounds on pilot-range questions.**
  - The 3B scored 6/12 on development questions and 4/16 on the main ones. That is within sampling
    error, but some fitting to the development questions cannot be excluded.
  - No change was made after a scored call.
- **The reference rule-reader is the more informative output, and it needs careful reading.**
  - It is a program that applies the prompt's rules to the rendered context.
  - On the same contexts it falls from 0.93 at m = 0 to 0.36 at m = 400 (δ̂ 0.44 [0.24, 0.88]).
  - The cause is that hard-incident evidence messages share an ID pool with background messages,
    so distractors include look-alikes that the reader's crude windowed rule accepts.
  - This is not the loss an optimal public reader would suffer; a reader that learned each stream's
    vocabulary from its own history might lose much less.
- **What it does establish is a property of the simulated reasoner.**
  - Its `q` is counted from hidden labels, so at δ = 0 it behaves as a reader that always knows
    which references are evidence.
  - No reader without hidden labels has that ability in this world.
  - The δ = 0 law is therefore not "a strong model". It is an oracle-labelled reader. R6's
    "simple builders suffice" was measured under that oracle.
  - This moves the prior toward R7's regime, without estimating δ.
- **Deviation accepted.** The incident's own non-decisive observations were kept out of both the
  contexts and the pool, whereas the simulator's `m` counts them as distractors. It does not affect
  the verdict.

**Decided.**

1. EXP-102 is not preregistered on the current reasoner law. The law's δ = 0 default is an
   oracle-labelled reader, and no real model available here can estimate δ.
2. The options are put to the user:
   - **(a) Simulation-only.** Ground the distractor law in the world itself: measure how a strong
     public reader degrades with look-alike distractors, then let the simulated reasoner's
     informed probability depend on confusable distractors rather than on a free δ.
   - **(b) A stronger real model.**
     - A local 7B model is not practical on this CPU: about 4–5 minutes per call at these context
       sizes.
     - A remote model needs credentials and money, which only the user can supply.
   - **(c) The salience ceiling for EXP-101.** This is independent of the context question.
3. The models (3.1 GB) and runtime (325 MB) are kept for now; free disk is 7.7 GB. They can be
   deleted if (b) is not chosen.

## R7 reasoner-law sensitivity — merged; R6's context finding is fragile

**Provenance.**

- The world change adds a distractor penalty to the hidden reasoner. It multiplies the informed
  probability by `exp(−δ·m/100)` and takes no new draw.
  - The coordinator read the `reasoner.rs`, `params.rs` and `sim.rs` diffs.
  - `m` counts references that are not decisive evidence of the focus incident, probes included,
    as R7 specifies.
- At δ = 0, R6's held-out run at b = 5, ρ = 0.7 replays byte-identical: 62 of 62 arms.
- In all six held-out runs, the context-only ceiling and R4's oracle are byte-identical to R6's.
- 13 runs, all exit 0, none excluded.
  - Five tuning runs were refused by the driver's clean-tree preflight before starting, because the
    worker had created untracked files during a run.
  - They were set aside and rerun at the new HEAD (`r7-stale-manifests.csv`). No output was
    affected.
- Gates on exit codes on the merged tree: fmt, clippy `--locked`, 553 Rust tests, the oracle guard,
  308 analysis tests.
- Run outputs moved to `artifacts/runs/r7/` (ignored).

**Re-verified from raw files.** The coordinator rebuilt the tuning selection from the raw tuning
`incidents.csv` and recomputed G(δ) with its own cluster bootstrap:

| δ | Selected builder | G [90%] |
|---|---|---|
| 0.05 | `window` 20 s, N 256 | 0.097 [0.066, 0.129] |
| 0.1 | `window` 20 s, N 256 | 0.137 [0.104, 0.170] |
| 0.2 | `cooccur` 1 s, N 256 | 0.280 [0.233, 0.327] |
| 0.4 | `cooccur` 1 s, N 128 or N 256 | 0.34 |

- The builder choices match the worker's. The bounds agree to within 0.001.
- At δ = 0.4, two configurations tie on tuning quality. The worker's tie-break toward fewer
  references gives 0.339, and the other gives 0.341.

**Verdict as written: Fragile.** At δ = 0.1 and 0.2, G ≥ 0.10 with the lower bound above 0.05.
"Robust" fails at every δ in the grid. The sensitivity settings (b = 2.5 and b = 8, at δ = 0.2)
agree.

**What it means.**

- **The result is conditional, and the condition is the finding.**
  - The ceiling's context holds only decisive evidence, so it is immune to the penalty by
    construction, and G must rise with δ.
  - What R7 measures is where R6's conclusion breaks. "Simple builders capture the context lever"
    holds only while the penalty is about 0.05 per 100 irrelevant references or less. That is at
    most about a 12% relative loss of informed probability at 250 references.
  - Above that, choosing which references to send is worth 0.14 to 0.34 of hard-incident quality.
- **Under a penalty, the best public context shrinks.**
  - The best context goes from about 490 references per call to about 85.
  - The winning builder changes from a broad `window` to a narrow `cooccur`.
  - At δ = 0.4 the best builder is barely above the rung's own context.
  - So compaction becomes the lever: deciding which few references carry the evidence. A
    substrate could plausibly do this, but no public builder here does it well.
- **EXP-102's design therefore depends on one unknown: the δ of a real reasoner** on contexts
  like these.
  - Simulation cannot supply it.
  - The charter puts real-model work in EXP-106, after EXP-101 and EXP-102.
  - R7 shows that the order matters: without an estimate of δ, EXP-102 cannot say whether its
    claim is quality or references.

**Accepted with notes.**

- The worker's readings were fixed in scripts before the runs, and are reasonable.
- The functional form (exponential in count, no position effect) is the plan's assumption, not a
  measurement.
- The selection delay was not re-tuned at δ > 0, as the plan said. A shorter delay might change
  context sizes slightly.
- The literature anchors (Shi et al. 2023; Liu et al. 2023) are still unchecked against the
  primary texts. No number relies on them.

**Decided.**

1. Before EXP-102 is preregistered, estimate δ on a real model. The smallest form is a local
   small model on this CPU, asked R1-style diagnosis questions with controlled numbers of
   irrelevant references.
   - This needs a model download, a runtime dependency and disk.
   - It is put to the user before it is planned in detail.
2. EXP-102, when registered, sweeps δ as a preregistered parameter and states its claim per δ
   region. It never states a single conclusion.
3. The salience ceiling from the R6 correction (an oracle that notices, with the rung's context and
   delay) remains the next simulation-only item, for EXP-101.

## R6 context-construction headroom — merged; simple builders capture the context lever in quality

**Provenance.** 26 runs through the driver, all exit 0; none failed, timed out or was excluded.
Eleven manifests refused by the driver's revision preflight were set aside and rewritten, not
deleted (`r6-stale-manifests.csv`). The six R5 held-out runs were replayed: 317 arm-runs are
byte-identical to R5's hashes. Gates on exit codes on the merged tree: fmt, clippy `--locked`, 545
Rust tests, the oracle guard, 308 analysis tests. Run outputs moved to `artifacts/runs/r6/`
(ignored).

**Re-verified at b = 5, ρ = 0.7 from raw files** (hard quality excludes slow leak).

| Arm | Hard quality | Refs/call | Critical misses |
|---|---|---|---|
| R4 oracle (`oracle_escalation_privileged`) | 0.952 | 5.1 | 194 |
| Context-only ceiling (`oracle_selection_context_d16_privileged`, supplementary) | 0.820 | 5.3 | 255 |
| Selection oracle + `window` 40 s, N 256 | 0.796 | 251.1 | 255 |
| Selection oracle + rung's own context | 0.489 | 43.3 | 282 |
| `always_escalate` + `window` 40 s, N 256 | 0.500 | 249.0 | 268 |

All match the worker's report.

**Verdict as written.** Both clauses hold at every setting, and the verdict is uninformative.
Clause 1 compares at no more than the ceiling's 5 references per call. No public builder is that
small, so the comparator is the empty context. Clause 2 is unreachable because R4's oracle carries
privileges no builder can supply.

**Coordinator error, recorded (second time).** This is the same mistake as R4.

- I took R4's oracle as the context ceiling although it bundles more than context.
- I wrote a clause (equal or fewer references) that no public builder could satisfy.

Lessons, applied to every criterion from here on:

- A ceiling isolates exactly one privilege. Its comparison arm differs from the public arms in
  that privilege only.
- Each clause is checked for feasibility against what a public arm can do, before any run.
- The worker's supplementary context-only ceiling is the correct comparator. It was labelled and
  did not replace the verdict.

**What it means.**

- With selection held at the oracle, simple builders capture almost all of the context lever in
  quality.
  - `window` reaches 0.796, against 0.820 for the context-only ceiling: a gap of 0.024
    [0.000, 0.051].
  - They do it with 33× the references per call (31.7 to 49.5), which is 8–11× the cost per
    stream.
  - EXP-102 can therefore claim references or cost at matched quality, not quality.
- Binding evidence across services and over the following seconds is solved by simple builders.
  Evidence that precedes the anchor by more than 2 s is not: only a long `window` carries it.
- The realistic public pairing, `always_escalate` with a builder, tops out at 0.723 for 17.4 s per
  stream.
  - The reasoner's token budget refuses up to 1153 calls in 200 streams.
  - Plain accuracy falls to 0.756 with `window`.

**Correction to the worker's report: the 0.132 is mostly salience, not timing.** The worker read
R4's oracle minus the context-only ceiling as timing (readiness). The coordinator counted, from
`incidents.csv`, hard non-leak incidents with no reasoner call at all.

- The selection-oracle arms make no call on 34 of 372 such incidents. R4's oracle calls on all of
  them. The selection-oracle arms call only about anomalies the shared rung noticed, so these 34
  are incidents the public rung never noticed.
- That alone accounts for 0.091 of the gap at every setting:

  | Setting | Gap | No-call part | Called but wrong |
  |---|---|---|---|
  | b5 | 0.132 | 0.091 | 0.040 |
  | b8 | 0.137 | 0.091 | 0.046 |
  | b2.5 | 0.067 | 0.091 | −0.024 |
  | b5, ρ0 | 0.121 | 0.091 | 0.030 |

- The decomposition subtracts counts. It is not paired per incident and has no interval, so it is
  coordinator arithmetic, not a measured lever.

Consequences:

- The worker's proposed first substrate job, readiness and compaction, loses most of its readiness
  half. Timing given a call is worth about 0.03–0.05 at b ≥ 5 and nothing at b = 2.5.
- Noticing is the larger item. A public rung that misses 9% of non-leak hard incidents, plus the
  slow-leak family (0.28 against 0.91), is a salience gap. It belongs to EXP-101 and needs its own
  privileged ceiling: an oracle that notices, with the rung's context and delay.

**Assumption that carries the result.** In the simulated reasoner, extra references "cost but never
hurt" (no distractor penalty, `gordian-stream/DESIGN.md` section 11).

- Under that law a broad window loses only tokens, which is why `window` is competitive.
- Published evidence for real models runs the other way (Shi et al. 2023; Liu et al. 2023, "Lost in
  the Middle"; to be checked against the primary texts).
- R6 swept b and ρ only. Until a distractor penalty and the per-reference price are swept, the
  0.024 gap is a property of this reasoner, not of context construction.

**Hidden-document exposure.** While looking for the public sections of `gordian-stream/DESIGN.md`,
the worker read sections 4, 5 and 11, which describe the hidden rules, and disclosed it.

- The grids (starting at 0.25 s; a 2 s lookback) may be influenced.
- The best builder, `window`, encodes no timing, and every builder is a tested pure function of the
  public view.
- The coordinator accepts the result with this caveat. The cause is structural: the public and
  hidden design share one file.

**Decided.**

1. Separate the hidden-rule sections of `gordian-stream/DESIGN.md` into their own file, so that
   workers can read the public design without exposure.
2. R7, reasoner-law sensitivity. Rerun the R6 comparison with:
   - a swept distractor penalty, including zero;
   - a swept per-reference price.

   The criterion will be fixed before any run, with one privilege per ceiling and every clause
   checked for feasibility.
3. The salience gap above becomes part of EXP-101's headroom: a noticing oracle with the rung's
   context is a separate ceiling.
4. EXP-102, when registered, claims references or cost at matched quality, against `window` and
   `cooccur` tuned by the frontier method. It bounds critical misses and plain accuracy.

## R5 decomposed headroom — merged; selection buys cost, context buys quality

**Provenance.** A container restart interrupted the run; a new worker resumed in place, retained
the interrupted replay, and re-ran it. All 237 R4 regression arm-runs match R4's committed hashes.
Gates on exit codes on the merged tree. Run outputs moved to `artifacts/runs/r5/` (ignored).

**Re-verified at b = 5, ρ = 0.7 from raw files.**

| Arm | Hard quality | Plain accuracy | Critical misses | Cost s/stream | Calls/stream |
|---|---|---|---|---|---|
| Selection oracle | 0.489 | 0.741 | 282 | 0.66 | 2.1 |
| `always_escalate`, 14 s | 0.489 | 0.815 | 256 | 7.36 | 24.7 |
| `contradiction_escalation` | 0.478 | 0.804 | 275 | 7.86 | 23.5 |
| R4 oracle (selection + timing + context) | 0.952 | 0.741 | 194 | 0.32 | 2.6 |
| Decoy oracle | 0 | 0.741 | 338 | 0 | 0 |

**Verdict as written.** Both clauses of the R5 criterion hold at every setting (primary: gap 0.433
[0.389, 0.476]; cost ratio 10.75 [9.08, 11.79]).

**What it means.**

- Perfect selection buys about 11× lower cost at matched hard-incident quality, but no quality, and
  it pays for the saving with lower plain accuracy and more critical misses than escalating
  everything, which the criterion did not count. An EXP-101 freeze must bound critical misses and
  plain accuracy, not only hard-incident quality.
- The public contradiction signal does not select: the public checker contradicts 97% of plain
  anomalies at some point. Telling hard anomalies from plain ones with public information is
  genuinely hard in this world.
- Context construction holds the remaining 0.46 of quality and most of the critical-miss reduction
  (194 against 282). The cause is binding: evidence at services the rung never attaches (738
  missing observations) and outside its window (330).
- Decoy handling has measurable headroom: the decoy oracle cuts false alarms by 1.88 per stream
  [1.68, 2.08] at no cost.

**Decided.** Before any substrate is built, R6 measures how far simple public context builders go,
under a criterion fixed before it runs. The common thread of R5's findings is evidence binding
across services and time; if simple builders capture it, EXP-102 has little to win, and if not, the
substrate prototype's first job is binding.

## R4 headroom — merged; the margin was met but did not discriminate

**Re-verified.** Gates on exit codes; at b = 5, ρ = 0.7 the coordinator recomputed from raw
`incidents.csv` and `results.csv`: oracle 354/372 = 0.952 at 0.32 modelled s per stream; best
baseline at the oracle's cost 0.008; `always_escalate` with a 14 s delay 0.489 at 7.36 s;
hidden-rules ablation 0.427 at zero reasoner cost. All match the worker's report.

**Verdict as written.** Headroom on all six settings (smallest gap 0.489, lower bound 0.446); the
conditional clause did not trigger. R1 is not revised.

**Coordinator error, recorded.** The margin compared baselines against an oracle that bundles
selection, timing and context. EXP-101 concerns selection only; the worker's arithmetic splits
roughly 0.49 selection and 0.46 context at b = 5. A margin met by a factor of five against such a
comparator discriminates nothing. R5 decomposes the headroom with separate privileged ceilings
and adds a public-information cascade baseline, under a new criterion fixed before any R5 run. This
is a new pre-run criterion for a new question, not a reinterpretation of R4's verdict.

**Other findings carried forward.**

- At escalation time, `always_escalate`'s contexts held none of the decisive evidence in 17 of 22
  burst incidents; with a 14 s delay, contexts were complete in only 22 of 66. Why contexts miss
  evidence that has arrived is traced in R5.
- R4's oracle does not handle decoys (it alarms as `never_escalate` does); decoy headroom is
  unmeasured until R5's decoy ceiling.
- Hidden-rule knowledge is worth a lot (ablation 0.427 at no reasoner cost); at b = 2.5 it nearly
  equals the oracle.
- Periodic configurations with periods up to 20 s, and `change_triggered`, are bound by the
  reasoner token budget.

## R1 to R3 — stream world, evaluator and harness merged

**R1 stream world.** Re-verified on exit codes. Two reasoner fixes required before merge and
delivered: no information from no evidence (truth enters an answer only through the informed
branch, probability zero without decisive evidence; checked in code), and repeated questions are
keyed by context fingerprint with copula-correlated correctness (`ρ` default 0.7). The
policy-facing answer carries only focus and diagnosis.

**R2 stream evaluator.** Re-verified on exit codes; six randomly sampled fixtures recomputed by
hand from RULES.md without reading the scorer, all matched. Judgements left to each
preregistration: wrong declarations do not cancel a correct one (spam), and alarms on a decoy
before it resolves count as false alarms.

**R3 stream harness and baselines.** Re-verified on exit codes; read the guard diff (a tightening
plus one allowlisted shim directory) and the privileged oracle's surface. Merge conflicts with R2
were additive and resolved as the union. Accepted the worker's reading that oracle escalation
fires once the hard incident's decisive evidence is delivered (escalating earlier is empty-handed
under the revised reasoner law).

**Carried forward to R4.**

- The R3 smoke parameters are placeholders. `always_escalate` escalates at notice, before any
  decisive evidence, and got no hard incident right in the worker's diagnostic; periodic and
  change-triggered hit the reasoner budget. Every baseline is tuned before comparison.
- The shared rung never notices the slow-leak family; that headroom belongs to salience, not to
  escalation timing, and is reported separately.
- Plain-incident accuracy of the cheap rung fell from about 84% to 73% after the first regime
  change in the worker's diagnostic, by design (its rules are not updated).

## A7b ratio interval — merged with a freeze condition

**Re-verified.** Only `analysis/` and `experiments/exploration/` touched; the analysis suite passes
on the branch and on the merged tree with `-W error` (262 passed, 4 slow tests deselected).

**Result.** On B1's empirical paired costs at the planning sizes (1,237 and 1,713), false
exceedance at true S = 0.20, out of a nominal 0.05, was: percentile up to 0.064, BCa up to 0.0675
(at n = 40), studentized up to 0.0545 (0.0484 when that worst cell was extended to 10,000
experiments). The studentized interval is the new default. Power at true S = 0.25 and 0.30 is
essentially 1 at the planning sizes.

**Noted.**

- The rule choosing the default was stated after the simulation table was seen. It chooses an
  instrument among reported alternatives, not a hypothesis outcome, so it is accepted, but it is
  not preregistered.
- The 0.06 bar is met on point estimates: three of eight acceptance cells have a 95% upper bound
  above 0.06 at 2,000 experiments. On heavily skewed lognormal costs no method meets 0.06.
- Derived data committed (paired-cost table, 360 KB; cell table, 25 KB) because the tests need it
  without the binary; both are regenerable by script. Accepted.

**Carried forward (freeze condition).** Before any experiment freezes on a ratio-of-totals
criterion, rerun `experiments/exploration/scripts/a7b_calibrate.py` on that experiment's own
exploration paired costs; the studentized interval must meet 0.06 with its 95% upper bound, not
only its point estimate, at the frozen n. The original condition was written for EXP-001; it now
applies to EXP-101 onward.

## Charter revised — approved by the user

The user approved `docs/charter-revision-proposal.md`. The charter's sections 1, 5, 6, 7 and 12
are rewritten accordingly, with new foundations and sources, and a revision record at the top.
EXP-001 is retired unfrozen and recorded as an exploration finding; the old EXP-002 to EXP-007 and
EXP-I01 are retired or re-scoped into EXP-101 to EXP-106. Change-triggered execution joins the
baseline registry. The plan gains Stage R (R1 stream world and simulated reasoner, R2 stream
evaluator, R3 stream harness and conventional baselines, R4 headroom check).

The user's message read "Inapprove the proposal"; the coordinator read it as "I approve" from
context and said so. If that reading is wrong, this change is reverted from git history.

## A6d incremental narrowing — merged; the shared rule no longer repeats work

**Re-verified.** Fixture committed before the rule change and untouched after it; gates on exit
codes. Worker evidence: 2,200 fixture rows and 55,000 B1 rows at 20 ms identical in every verdict
column; 30,403 calls step-equivalent to the cache-free reference; rule fit R² 0.996 / 0.997.

**Effect.** Narrowing work fell about 96% (30,007 against 764,647 world evaluations over the step
test). "A failed component raises success" fell from 102 to 35 episodes.

**The 35 are real cost, not repeated work.** In 31 of them the no-directive run ended short of
affordable work by a median of 37 ns, after paying about 8 µs once to read the verifier's first
output. A failed verifier never pays that bill, and at a 60–100 µs budget it is the margin. No
cache removes a first read. The other 4 also need the verifier's stale set to lose priority.
Documented in POLICIES.md §3.3; the rule is unchanged. Changing what reading an output costs would
be a change to the instrument and is not made.

**Accepted with a note.** Declared world and evaluation terms are now billed one call late, so the
hard limit can be overshot once per episode by at most about 8 µs. The work is still counted in
modelled cost. Under the revision proposal, any expensive reasoner call must be paid before it
runs, never in arrears.

**State of the shared rule.** Three successive fixes (A6b, A6c, A6d) made it rational at exhaustion
and incremental on unchanged inputs. It is now a strong cheap rung: change-triggered by
construction.

## A6c decode once — merged; the residue becomes A6d

**Re-verified.** The verdict fixture was committed before the rule change and is untouched by it;
all 2,200 rows (10 arms, 20 ms, 20 seeds × 11 classes) identical. Gates on exit codes. Rule
counted-operation fit R² 0.995 (fit) and 0.997 (held out).

**Correction to the plan text.** A6c's premise ("stored outputs are re-decoded every step") was
wrong: stored outputs never were. What repeated was a component *re-producing* byte-identical
output each step, which the rule decoded and charged again. The worker implemented the intent by
content equality and kept the old rule as `Decider::without_reuse`, a test-only reference.

**Effect.** Decoding fell 92%; at 20 ms mean modelled cost fell 40–47% for verifier-running arms
and 12–34% for the others. At 250 µs `all_components` went from 29.1% to 87.0% success. The
"failed component raises success" effect fell from 210 to 102 raised episodes but is not gone: the
rule still re-narrows the verifier's unchanged 46-hypothesis set every step (about 10.5 ns per
world, declared). Zeroing that term leaves 35 raised episodes, unexplained.

**Meta-finding.** Two defects in a row were the shared rule repeating work on inputs that had not
changed. Fixing them makes the baseline incremental. Change-triggered execution (recompute only
when inputs change) is the cheapest form of selective activation, and the charter's baseline
registry does not name it. A salience mechanism must beat it, not only periodic schedules.
Coordinator recommendation to the user: add "change-triggered (memoized) execution — whether
salience adds anything beyond skipping unchanged inputs" to charter section 7. Not made here,
because the charter is normative.

## Exploration follow-ups — merged; correction to the Stage B entry

**Re-verified.** Gates on exit codes; the truth table regenerated from
`crates/gordian-eval/examples/truth_table.rs` hashes to the scratch original; no crate source
changed beyond that allowlisted example.

**Finding 4 resolved, and it corrects Stage B.** The shared rule charges for decoding every stored
component output on every step, 530 ns per output plus 115 ns per hypothesis, even when the output
has not changed. On a symptom-free window the verifier's output lists 46 hypotheses, so re-decoding
it costs about 5,800 ns of the roughly 7,600 ns the rule charges per step. A failed verifier leaves
nothing to decode, the budget lasts longer, and success rises: that is the whole "failed component
helps" effect (removing the charge lifts `all_components` at 250 µs from 13.8% to 72.2% on these
episodes; removing the verifier's priority in the rule changes 0.2–1.8 points).

**Correction.** Stage B's collapse of verifier-running arms at binding budgets (`all_components`
0.291 at 250 µs) is mostly this re-decoding charge, not component compute. With it removed, the
worker's upper-bound variant gives `all_components` 0.730 and `fixed_verifier_only` 0.943 at
250 µs. The heuristic family is bit-identical either way, so the Stage B conclusion about the tuned
periodic baseline stands; the "naive pipelines collapse because components are expensive" reading
does not.

**Decided.** Re-billing an unchanged output is an artefact of the shared rule's implementation,
not a property of the world, and it biases every comparison involving a verifier-running arm. It
is fixed under every option of the pending decision (plan item A6c), before any experiment.

## Stage B exploration (B1–B4) — merged; EXP-001 not freezable as designed

**Re-verified.** Gates on exit codes; per-arm success, modelled cost and critical-miss rates
recomputed from `b1-variance.csv` match the worker's report. 220,000 B1 episodes, no failed or
excluded run, replay checks byte-identical.

**What the data say (exploration, not confirmation).**

- A periodic heuristic (`every` = 4) holds success 0.954 at every budget level for about 22k modelled
  ns per episode; `all_components` needs 522k for 0.965 and collapses to 0.16–0.29 when the budget
  binds. A strong simple baseline captures nearly all achievable quality at a few percent of the
  cost. This is the charter's baseline registry doing its job.
- The shared decision rule costs about 17.1k ns per episode, is charged at every step for every
  arm, and is 65–89% of the heuristic arms' cost. Against the tuned periodic pipeline, even a
  perfectly timed component selector can save at most about 10–18% (B4's estimate; arithmetic, not
  a run), below EXP-001's preregistered 20% margin. Quality headroom against that baseline is
  0.01–0.02.
- The one large oracle gap, JointlyDecisive (0.38), belongs to the shared rule's one-step probe
  choice; no component schedule can close it.
- Five of six stressors cannot fail any current arm: noise is separable for free by catalogue id,
  no arm has a salience mechanism, and the final declaration makes unbounded waiting invisible.
- Effective ambiguity in the ambiguous classes is 2.85 kinds, not 5; a prior-aware arm that never
  probes is right 41.6% of the time.

**Interpretation.** In the current small world, *which component runs* is not where the cost is.
The dominant computation is deliberation (the shared rule) and sensing (probes, which modelled cost
does not price). EXP-001 as designed would very likely return H0 for structural reasons of this
environment, not because selective activation fails in general. The charter anticipated the
mirror-image danger (a weak baseline making anything look good); here the strong baseline shows the
environment offers little to select.

**Instrument findings (not yet fixed).**

1. The rule is not schedulable: a selector cannot decide when to deliberate. Whether deliberation
   is part of what selective activation controls is a design decision.
2. Probes are not in the modelled cost, though the charter's `C` includes sensing; arms buy about
   two probes where one would do.
3. Stressors are toothless at default noise (see above).
4. A failed component can raise an arm's score at binding budgets (3 cells); suspected rule
   behaviour on an empty verifier output; not confirmed.
5. `b4.py` reads a truth table produced outside the repository; one exploration input is not
   regenerable.

**Decision required from the user** (not taken by the coordinator, because it shapes every later
experiment and freezing is irreversible):

- (a) Freeze EXP-001 in the current world and expect a bounded negative result.
- (b) Revise before freezing: make deliberation schedulable, price probes, and revise the world
  so that relevance is costly to determine (non-separable noise, specialist components with
  partial views), with the revision's properties fixed from the charter before any arm is run on
  it, and the generalist and periodic baselines kept.
- (c) Record (a) as an exploration finding only, and do (b).

Coordinator recommendation: (c). A preregistered experiment whose negative outcome is already
implied by exploration arithmetic has low information value, while the fixes in (b) are needed for
EXP-002 to EXP-004 anyway.

Independent of the decision, started then: A7b (ratio-interval calibration on B1's cost
distribution) and the two small exploration follow-ups, since merged (see the entry above).

## A8b counted operations — merged with a freeze gate

**Re-verified.** fmt, clippy (`--locked`), workspace tests with `--no-fail-fast`, oracle guard,
driver test, dump sha256 and the analysis suite on exit codes; reference checker functions
untouched.

**Accepted.** Counters follow the dominant loops of each component and the shared rule, are
deterministic, and cannot be seen or set by a policy (type-enforced, tested). Against hot-loop
minimum timings they fit with R² 0.98–0.99 on fit, held-out and real states.

**Scrutinized: in-situ rescaling.** With pure hot-loop weights the non-identical-arm check failed
(modelled ratio 15.66 against a wall-time median ratio of about 12.1). The worker found that a call
inside an episode costs 1.2–2.2× the same call in a loop, more so for arms that call components
sparsely, consistent with cache effects, and rescaled each weight by a per-component factor and
per-call constant fitted on other arms and seeds. The fit and the check share no arms or episodes,
and the worker reported the failure and offered rejection, so this is calibration, not tuning to
the test. Coordinator check on an unfitted sparse pattern (heuristic only against the verifier
every second step, seeds 200–219): modelled 5.25, wall-time median 5.40 [5.05, 5.85], inside.

**Residual risk.** The in-episode premium depends on the scheduling policy's call pattern, which is
exactly what EXP-001 varies. Fixed factors fitted on other arms priced the sparse arm about 5% off
in the worker's data and about 3% off in the coordinator's. The check is weak (intervals about 15%
wide).

**Carried forward to C1 (EXP-001 freeze gate).** Before freezing, run the cost check on the actual
EXP-001 arms (selective against the tuned periodic pipeline) on exploration seeds. If the modelled
ratio falls outside the wall-time interval, or the two disagree by more than a quarter of the
preregistered savings margin, the cost conclusion of EXP-001 is reported as unresolved, whatever
the modelled result.

**Host change.** The VM now reports a 2.10 GHz Xeon; earlier sessions reported 2.80 GHz. Modelled
cost is in calibration-host nanoseconds and does not change with the host; wall-time checks are
valid only on the host where they run. The manifest should record the CPU model and frequency, not
only flags (small follow-up).

## A8 interleaved arms — merged; A/A fails on an idle machine

**Re-verified.** Merged cleanly onto A6b; fmt, clippy (`--locked`), 293 workspace tests with
`--no-fail-fast`, the oracle guard, the driver test (62) and the analysis suite (212), on exit
codes.

**Worker's evidence.** Interleaving removes a large between-run effect: sequential A/A runs gave
12 of 20 intervals excluding 0 (sd of S 0.151), interleaved runs 0 of 21 (sd 0.037). The worker's
runs shared the machine with another worker.

**Coordinator's idle-machine A/A** (through the driver, cores 0–2, shell on core 3):

| Run | S | 90% interval | Contains 0 | Drift CV |
|---|---|---|---|---|
| heuristic only, run seed 1 | −0.7% | [−5.8%, +3.7%] | yes | 0.12 |
| heuristic only, run seed 2 | −2.9% | [−5.9%, +0.1%] | yes | 0.03 |
| heuristic only, run seed 3 | +5.4% | [+0.5%, +10.0%] | no | 0.27 |
| all components, run seed 1 | −6.9% | [−13.3%, −1.4%] | no | 0.11 |

The per-block minimum timing was stable to about 2% while the mean moved up to 80%; `/proc/stat`
shows non-zero steal time. In the all-components run five episodes carried 51% of the total
absolute difference, and S without them was +0.9%; the median per-episode log ratio was near 0 in
every run. Interference from the host arrives in bursts that hit one copy of an episode and not
the other, and a ratio of totals is sensitive to them.

**Decided.** The plan's A8 rule applies: counted operations are built before B1 (item A8b), and the
charter's cost `C` becomes deterministic modelled cost, with wall time as a secondary check. This
is a change of measurement made before any experiment is frozen.

## A6b final declaration — merged

**Re-verified.** All gates on exit codes; the `all_components` row at 60 µs reproduced exactly
(202 final declarations, 18 terminal, 38 successes, 40 critical misses).

**What it showed.** Under a binding budget, component-running arms exhaust their compute at
0.3–0.6 s of logical time, before any symptom has arrived, by re-running components on windows with
nothing new in them. At the final call they declare "no fault", which is right only on `NoFault`.
Nearly all of the success gained by the fix is `NoFault`; on faulted classes the failures are now
visible as wrong declarations and critical misses instead of undecided rows.

**Carried forward to C1 (EXP-001 preregistration).**

- An arm that does nothing and declares "no fault" scores `NoFault` for free. The primary outcome
  must be read per class with the critical-miss rate, or exclude `NoFault` from the success
  average and score it through false alarms; the preregistration chooses and states which.
- Whether the shared rule at its deadline should declare "no fault" on a window with no symptom,
  or abstain, is a rule choice that changes faulted-class scores. It is fixed before freezing and
  applies to every arm.
- The final call's declared cost is derived, not separately calibrated.

## A6 baselines — merged; headroom probe

**Re-verified.** fmt, clippy with `--locked`, the workspace tests, the oracle guard and the
driver shell test on the merged tree. Accepted the clock interpretation: a component's declared
Compute nanoseconds are its time for every component, and `Slow` multiplies them.

**Coordinator error, fixed forward.** The merge was pushed after a test summary showed one failure,
because the command chain gated on a parsed summary rather than cargo's exit code. The failure was
a random-count coverage floor in the A5b equivalence tests (about 2–3% flake rate; the checker
agreed with the reference on every case). Fixed with a deterministic sweep and a floor at about 3.9
standard deviations. Coordinator merges now gate on exit codes only.

**The worker's finding.** With the shared rule and default limits every arm is at or near the
ceiling (209–213 of 220; oracles 220). Taken alone, this would make EXP-001 trivially pass.

**Coordinator headroom probe** (20 seeds × 11 classes, cgroup-isolated, successes of 220):

| Arm | compute 20 ms (default) | 250 µs | 60 µs | 25 µs |
|---|---|---|---|---|
| heuristic only | 209 | 209 | 187 | 39 |
| estimator only | 210 | 206 | 20 | 20 |
| random p = 0.5 | 213 | 138 | 20 | 13 |
| verifier only | 210 | 78 | 19 | 10 |
| all components | 213 | 35 | 18 | 4 |
| oracles | 220 | | | |

Under a binding budget, quality depends strongly on scheduling. Two effects are mixed in this
table and must be separated:

1. *A shared-rule defect.* Nearly every failure is `budget_exhausted`: the rule waits for its
   patience deadline, runs out of affordable work first, and never declares although declaring is
   free. Plan item A6b gives every arm a final declaration.
2. *Real waste.* `all_components` ran 154 component calls per episode on windows that had mostly
   not changed (median decision at about 1.9 s, a step every 50 ms). Avoiding that waste is what
   selective activation claims to do, and a tuned periodic schedule is the charter's adversary.

**Decided.**

- No world revision for EXP-001. It is testable under a binding budget; B1 sweeps budget levels
  and the preregistration fixes the levels from B1.
- The saturation at default limits is a property of a budget that never binds, not evidence for
  or against any mechanism.

**Open, for the user.** One cheap component (the heuristic) is nearly sufficient alone: the
components are redundant generalists that all read the whole window with the full public rules,
not the specialists the charter's question is about. That limits the quality headroom available to
EXP-002 and EXP-003. Making components specialized (partial views, complementary evidence) is a
research-design choice that risks tailoring the environment to the hypothesis; it is raised to the
user rather than built.

## A5b checker — merged

**Re-verified.** fmt, clippy with `--locked` and all features, 220 workspace tests, oracle guard,
dump sha256 unchanged; the diff to `gordian-components` is cost constants only. The pinned
calibration recheck waits until no other worker is compiling.

**Accepted.** Root cause found by step counting before any change: `anchored(site)` rescanned the
informative prefix at every dependent alarm, about k·d steps. The fix carries the minimum anchor
instant per world; exact for unsorted instants. About 55,000 equivalence comparisons against the
kept reference across generated, adversarial and random inputs; mutation shows the tests can
fail. Late-anchor shape linear; 78× faster at n = 2048. 0–20% slower on shapes the reference
already handled linearly; not tuned, per spec.

**Correction to the A5 entry.** The "24× declared at n = 256" and "1.7 ms at n = 1025" figures
were not reproduced by the checker alone (0.9–1.0 ms at n = 1024 on the old build). The 24× was a
component-level declared-to-measured ratio from one session; read it as an order of magnitude,
not a constant.

**Decided: machine drift is a threat to every measured-cost comparison.** Wall time drifted up
to 30% within one session and 13–34% between sessions. No hardware counters exist in this VM.
Plan item A8: interleave arms per episode in randomized order, log a drift-control workload, test
for position effects, and pass an A/A check before B1.

## A4 run harness — merged; worker report lost

A container restart stopped the worker after it committed and before it reported. The commit
survived; the coordinator reviewed it without a report.

**Re-verified.** fmt, clippy with all features, 208 workspace tests, oracle guard. Read
`HARNESS.md` in full. Ran `heuristic_only` end to end through `scripts/run-driver.sh` under cgroup
v1 isolation: 550 episodes, 0 undecided, internal-to-external ratio 0.81 (first value; becomes the
tolerance starting point). A second run of the same manifest gave a byte-identical `results.csv`;
`measured.csv` differed, as designed. The events sample contained none of the hidden-state key
names. The episode class does not reach policies (`PublicInfo` has no class; the policy directory
guard bans `Episode`, `Simulator`, `gordian_eval`).

**Independent evidence from the smoke run.** `heuristic_only` never probes, and its success on
hidden-kind classes equals the generator's prior for the kind it guesses (JointlyDecisive 0.58
against a 0.595 prior; Ambiguous 0.44 against 0.40). A policy without hidden state should sit at
its prior; this is a second confirmation that the public stream does not leak the kind.

**Decided.**

- The budget reconciliation stands: the `Bill` is the authority for every resource; the episode
  spec's budget must equal the limits or the harness refuses the episode.
- A component's declared Compute nanoseconds are its time. `Slow` multiplies Compute as well as
  Time (assigned to A6; `HARNESS.md` section 8 item 7).
- Analysis must accept undecided rows, drop `bill_total` (it summed ns, probes and bytes), and
  read `measured.csv`, with measured cost the default for relative savings (separate unit).
- Every non-oracle baseline shares one decision rule; arms differ only in selection, because
  EXP-001's intervention is the scheduling policy only (assigned to A6).

**Carried forward.**

- To D1: the events sample carries `class` as a join key. Anything that trains on it must drop
  `class`; it is a strong label.
- To B3 and EXP-002: `QuietUrgent` and `NoiseFlood` are toothless against a component that reads
  the whole window, because catalogue messages are separable from noise for free (`heuristic_only`
  scored 1.00 on both). The stressors test only policies that pay to look.
- `FeedbackBait` is identifiable from public graph data (`unreliable_health`); it shares its prior
  with Ambiguous and StaleMemory, so identification reveals little about the kind. Documented in
  the world's DESIGN.md.

## A5b checker — restarted from a WIP snapshot

The restart stopped the worker with uncommitted changes. The coordinator snapshotted them as a
WIP commit on `checker-perf` and a new worker resumed from it, instructed to verify rather than
trust the unfinished work and to commit after each milestone.

## A5 components — merged after one revision

**Decided.**

- Measured cost, not declared cost, is the charter's `C` in every experiment. Declared costs fit
  the pooled average within 25% but deviate up to 2× per class and 24× for the verifier on
  late-anchor streams; a selective policy could be misbilled in its favour. Plan section 5/A4
  now requires per-call boundary timing in the ledger and the results table.
- The verifier's quadratic shape is the world checker's, not the component's. Plan item A5b:
  optimize behind an equivalence test with the current function kept as reference; required
  before B1.
- The lookup's `Resource::Memory` per-read charge was removed: `Memory` means resident memory,
  measured by the runner. Record reads remain charged as Compute.

**Accepted with notes.**

- The heuristic proposes "no fault" on any symptom-free window and is right 100/100 on `NoFault`.
  That is a property of the physics (every fault permits silence), not skill. Analyses must
  read `NoFault` success together with the critical-miss rate on faulted classes.
- Prior records outside `StaleMemory` are right about 1 time in 5, so the lookup is mostly
  noise. EXP-004 needs a memory that is sometimes useful; revisit the world's record generator
  when E2 is designed, as a new world revision, not by editing A1's guarantees in place.
- Worker subagents were refused `taskset` by their sandbox; the coordinator is not. Pinned
  measurement runs (B1 onward) are launched by the coordinator through `scripts/cgroup-run.sh`.

## A2 evaluator — merged

**Re-verified.** 118 workspace tests, clippy with all features, oracle guard. Eight randomly
sampled fixtures recomputed by hand from `RULES.md` without reading the scorer; all matched.

**Accepted with notes.** A `Correct` on a `NoFault` episode followed by `Abstain` scores both
success and false alarm. Each preregistration must state whether false alarms enter its primary
outcome. `Correct` reveals whether the site was right, at three probes; baselines should show
whether any arm uses it as an expensive probe.

## A1 small world — merged

**Re-verified.** Acceptance commands rerun on the branch and on merged main: 67 tests with all
features, clippy with warnings denied, oracle guard, dump sha256 identical across runs and across
the lockfile regeneration (`8be119bf…51fe2`).

**Independent check.** Plug-in mutual information between the true fault kind and coarse public
views of the stream (signal set, signal count, counter abnormality pattern, message severities),
3,000 episodes per class for Ambiguous, FeedbackBait, StaleMemory and JointlyDecisive, against
50 permutation baselines. No view exceeded its baseline. Ordered-sequence views had too many
distinct values for this test to discriminate; the worker's same-seed kind-swap test covers that
case by construction. Not checked: higher-order statistics, timing side channels in code.

**Accepted with notes.**

- Truth priors inside ambiguity sets are non-uniform (Ambiguous: ConfigDrift 40%,
  ResourceExhausted 40%, CredentialExpired 20%; JointlyDecisive 60/40). A learned policy can
  exploit this. It is a property of the environment distribution, not leakage, but B4 must
  report effective ambiguity, and A5 components must not bake the generator's priors in.
- JointlyDecisive defeats one-step selection on the decision value but not entropy-based
  information gain over (kind, bit) worlds. That distinction is itself testable in EXP-002.
- Core stays dependency-free; world keeps its serde shims (`BudgetSpec`, `CostSummary`). Revisit
  only if the shims cause a bug.

**Carried forward.**

- To A4: `Episode` derives `Serialize` including hidden state. Never serialize an `Episode` into
  any channel a policy can read. The recorder writes the public stream and the evaluator's
  verdict, not the episode.
- To A5: an empty `consistent_hypotheses` set means a bounded window dropped evidence, not a
  contradictory world.
- To A2: score from `(Truth, trajectory)` rather than `Episode`, so hand-written fixtures stay
  independent of the generator.

## A7 analysis — merged after one revision

**Re-verified.** 120 tests with `-W error`; no build artefacts committed.

**Revision requested and delivered.** The first submission could not test the charter's
EXP-001 cost measure (a ratio of totals) and could label a tiny sample "equivalent" with only a
warning. Added: paired-bootstrap ratio of totals with a threshold decision; a preregistered
sample-size gate forcing `unresolved` below plan, raw category still shown.

**Accepted with a hard follow-up.** The percentile bootstrap on the ratio is anti-conservative on
skewed costs (7–12% false exceedance at nominal 5%). Recorded as plan item A7b; EXP-001 may not be
frozen until it passes.

## A3 cost bill — merged

**Re-verified.** 39 tests; read `charge_recorded` and `replay`: a charge is applied only after
both budget acceptance and ledger append succeed, and replay uses the same atomic path.

**Carried forward to A4.**

- Build a `Bill` only from a fresh `Budget`. A pre-spent budget silently breaks
  sum-over-phases = total.
- Keep exactly one `Bill` per `Ledger`. `Bill::replay` folds every Accounting entry it finds.

## Process note

One coordinator merge (A7) hit a `.gitignore` conflict; a non-fail-fast command chain then
committed the conflicted tree locally. It was caught before push and repaired. Coordinator
merges now run under `set -euo pipefail`.
