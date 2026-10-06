# B2: notice precision, a notice-relative selection delay, and a public later re-anchor

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation). Lab 2, unit
B2 of `docs/lab-queue.md`, branch `reanchor`. The table makes no claim about which noticer is better:
it is B1's table extended, and the comparator for M2 whichever way the chief re-fixes it. Scripts:
`scripts/b2_*.py` and `scripts/b2_run.sh` (B1's scripts in `experiments/exploration/scripts/` are
imported read-only). Files: `b2-*.csv`, `b2-selected.json`, `b2-noticers-table.md` (generated).

## The result in one paragraph

On the 200 held-out streams at b = 5, rho = 0.7, the public later re-anchor (`ReanchorNoticer`: the
anchor of a notice moves onto the first burst that begins after an isolated anchor; site reading,
gap 20 ms, burst of at least 2 observations, threshold z = 2, all chosen on the tuning streams) reaches
**anchor-correct 0.952 [0.932, 0.970]** on the 372 hard non-leak incidents, against the queue's bar
of 0.944, at **6.82 [6.47, 7.18] background notices per stream** (budget 9.03). It is also 0.946
[0.926, 0.966] anchor-and-site-correct (one notice both), 0.978 [0.966, 0.990] noticed, and its quality
with the selection oracle is 0.551 [0.507, 0.597] with the rung's 6 s retirement and 0.591 [0.548,
0.636] with the hold at its tuned delay, which is R10's injected-notice ceiling (0.586) to within noise.
The point estimate clears the bar; the interval's lower end (0.932) does not, and the sample cannot say
more. The same mechanism at the rung's own threshold (z = 3) gives 0.938 at 2.30 background notices per
stream (the rung: 0.890 at 4.00; the rung at z = 2, B1's comparator: 0.914 at 9.03). It does nothing for
the slow leak (noticed 0.460, anchor-correct 0.000, as every public noticer here). Per the acceptance
clause the chief re-fixes M2's comparator; that is the chief's act and this report only supplies the
numbers (section "What the numbers say").

## What was built

- **Evaluator (rules N13 to N16, `crates/gordian-stream-eval`).** The notice record carries each
  notice's site; an incident's site is the first service it occupies (N13); a notice is *site-correct*
  when it is about an incident and its site is that incident's (N14); *anchor-and-site-correct* is one
  notice that is both (N15, one notice, not one of each); and *notice precision* is notices anchored on
  an incident of any tier over all notices, per tier and strict (anchor-and-site-correct over all) (N16).
  The harness writes three more columns per notice file and the analysis loader reads and checks them.
- **The selection oracle's option `hold_until_asked`** (`oracle.rs`, `spec.rs`, `arms/mod.rs`): off by
  default and then not written; on, an anomaly the oracle will ask about stays live until asked.
- **`ReanchorNoticer`** (`arms/noticer_reanchor.rs`, id `reanchor`): the rung's noticer, with one more
  step at the moment of notice. Its readings are in the module's documentation (committed before any
  tuning run, `df996bc`) and repeated below.
- The B1 scripts' B2 counterparts (`b2_common`, `b2_manifests`, `b2_select`, `b2_stats`, `b2_table`,
  `b2_diagnostics`, `b2_provenance`, `b2_hand_mutants`, `b2_run.sh`), `analysis` loader and measure
  changes, `HARNESS.md`, `RULES.md`.

## Byte identity: the gate

R6's held-out manifest at b = 5, rho = 0.7 (main checkout, `source_revision` replaced, run id kept,
written to a separate directory) replays byte-identical for **all 62 arms** against
`r6-results-sha256.csv`, for `results.csv` and `incidents.csv`: 62 of 62 and 62 of 62
(`b2-regression.csv`), with the binary built at `df996bc`, which holds the evaluator, the oracle option
and the noticer; the Rust sources have not changed since (the diff to HEAD in `crates/` is
`HARNESS.md` only). GATE_FINAL_PLACEHOLDER

## The evaluator: rules, fixtures, mutation

| Id | Rule (full text in `RULES.md`) |
|---|---|
| N13 | the incident's site is the first service it occupies (any tier); a notice's site is what the record says; a record may omit it |
| N14 | site-correct: about an incident, a recorded site, an incident site, equal; the site is read against the incident the *anchor* belongs to; an incident is site-correct if some notice about it is |
| N15 | anchor-and-site-correct: one notice that is anchor-correct (N5) and site-correct; for an incident, one single notice, not an anchor-correct one and a site-correct one |
| N16 | precision: notices on an incident of any tier over notices; per tier; strict (N15 notices over notices); pooled across streams from counts by the analysis |

- **Fixtures.** `fixtures/notice-cases.json` has 39 cases: B1's 29, unchanged (a test pins that they
  record no site and cite no rule above N12), and ten new hand-worked cases (site right and wrong,
  late anchor with the right site, one notice versus one of each, missing site on either side, a
  notice on background, the first occupied service, every tier with a decoy, the precision stream
  worked by hand, an earlier site-correct notice not undone by a later wrong one). A test checks that
  scoring with the site removed or changed does not move any measure of N1 to N12.
- **Generated streams.** `tests/notice_generated.rs` checks, on 30 generated streams, that the first
  occupied service is the site of the true diagnosis for every plain and hard incident (N13's
  judgement is therefore a checked fact for those, not an assumption), and scores one notice per
  incident, a flood, and a shifted site.
- **Mutation.** cargo-mutants on `notice.rs` (`--in-place`, cores 0-2): **80 mutants, 78 caught, 2
  unviable, 0 missed** (the two unviable are B1's: `score_notices -> Ok(Default::default())` and a
  let-chain `&&`, `b2-mutants.md`). Twenty hand-applied mutants of N13 to N16 (`b2_hand_mutants.py`,
  `b2-hand-mutants.csv`): **20 of 20 caught**. The first run of the hand mutants had one survivor
  ("an incident that occupies nothing has site 0"): the fixture's unsited incident was paired with a
  notice about service 3, which that mutant also rejects. A fixture with the notice about service 0
  kills it, and the whole list was rerun after the fixture changed.
- **Loader.** The new columns are in the loader's schema guard, which a test compares with the
  harness's header constants; 13 new tests refuse each way the three files can disagree about them.

## `hold_until_asked`

An anomaly the oracle will ask about (its anchor belongs to a hard incident, no escalation proposed for
it yet) is not given the rule's final call and not retired. It retires when asked, as any does. The
hook is `EscalationRule::keeps`, consulted only when the rule's `may_keep` is true, so the step does
nothing different for any other arm (the gate above is the check, the test `with_the_option_off_the_arm_
is_the_arm_it_was` is the small one). **A reading stated:** a live anomaly stays in the noticer's set,
and the rung's last-resort attach rule can then take a later observation at its site, so the notice
record of a run with the hold is that run's own. Measured: over 200 streams the hold changes the
record of each table noticer by at most 0.2% of its notices and changes no noticed or anchor-correct
count (`b2-hold-effect.csv`). The table's noticing columns are read from the arms without the hold
(B1's arms); the hold arms supply `quality_held` only.

The delay is tuned per noticer on the tuning seeds with the hold by R5's rule (the highest quality,
the cheapest of ties): 16 s for the rung at z = 3 and z = 2, EarliestAnchor and `reanchor`, 30 s for
ChangeTriggered q = 128 s (a row of 24 hard-anchored notices on 200 streams; its tuned delay is
noise-driven, `b2-tuning-delays.csv`).

## `ReanchorNoticer`: readings, tuning, choice

**The failure it answers.** B1: of 31 never-noticed incidents 17 had a background notice within 1 s
before them; the rung's own re-anchor (the observation with the most attached observations in the next
0.4 s) keeps a stray that comes before a burst, because the stray's window holds the whole burst.

**The rule, applied at the step that notices a candidate, after the rung's own re-anchor:** *if the
anchor is isolated and a burst begins after it, the anchor moves to that burst's first observation*,
with the site, region and burst timing following and the evidence before it dropped (the move the
rung's own re-anchor makes, `Tracked::move_anchor_to`). Readings, stated before any tuning:

- **Isolated, gap `g`:** no other attached observation falls strictly before the anchor's instant plus
  `g` (an observation exactly `g` after leaves the anchor isolated; one at the anchor's own instant does
  not). Which observations count is a parameter, `isolation`: `site` (the brief's wording: only
  observations about the anchor's own service) or `any`.
- **Burst:** an attached observation strictly later than the anchor with at least `min_burst`
  attached observations, itself included, in the rung's own 0.4 s window from it. The first such
  observation is where the anchor moves. No upper bound on the lag: the attach rule bounds it.
- **When:** at the notice, never after. A lone observation cannot cross the threshold, so an anchor
  that is a lone observation is noticed together with the burst that attached to it. A burst that begins
  only after the notice is not served (the log has one notice per anomaly and no re-anchor entry; a
  second notice would be a different noticer). This is a limit of the design.

**Tuning** (tuning seeds 10000-10099, b = 5, rho = 0.7, the selection oracle at R5's 16 s with the
rung's retirement, as B1's tuning; the rule was fixed in `b2_common.py` before each run):

1. *Stage 1* (24 configurations at z = 3: `site`/`any` x gap 50-400 ms x burst 2, 3): the highest
   anchor-correct share within the budget. The first run's best was the smallest gap of the grid (50
   ms) and the two readings tied at the top (0.930, 185 of 199 incidents each), so **two amendments
   were made after the first run and before any held-out run**: (1) the grid was extended downward (10,
   20, 30 ms; run `b2-tune1b`); (2) the tie rule, which had put `any` first, now prefers the brief's
   literal reading (`site`) after the larger gap and burst, since a tie rule that favours my own
   variant over the brief's wording is backwards. The first run's table and choice are kept
   (`b2-tuning-stage1-first-run.csv`, `b2-selected-stage1-first-run.json`: its pick was `any`, 50 ms,
   burst 2, 0.930). The extension's best was 0.935 at 20 and 30 ms for both readings (the share falls
   at 10 ms and the anchor-and-site-correct share falls with it); the rule chose **`site`, 20 ms,
   burst >= 2**: tuning 0.935 at 2.26 background notices per stream (the rung at z = 3: 0.854 at 3.99).
2. *Stage 2* (that configuration at z in {3, 2, 1.5, 1}): z = 2, tuning 0.950 at 6.55; z = 1.5 is 0.950
   at 13.69 (over the budget) and z = 1 is 0.945 at 27.6. **Chosen: z = 2.** Tuning 0.950, held-out
   0.952; the z = 3 configuration, tuning 0.935, held-out 0.938: no overfit visible at this resolution.
3. *Stage 3:* one delay per table noticer with the hold (above).

The held-out run played both reanchor grids and the threshold ladder as sensitivity, and the whole of
B1's grid (20 configurations), all at the fixed delay; **nothing was chosen from any held-out number.**

## The table

200 held-out streams (seeds 20000-20199), b = 5, rho = 0.7, the selection oracle and the rung's own
context, 372 hard non-leak incidents, 139 slow-leak incidents, 4,264 plain. 90% equal-tailed percentile
intervals from 10,000 resamples of whole streams (`numpy.random.default_rng(9950)`, the draws B1 used,
so B1's four rows reproduce B1's numbers and intervals exactly, which they do). **Intervals are not
paired differences**; paired ones are in the next table and `b2-paired.csv`.

| noticer | hard non-leak noticed | anchor-correct | site-correct | anchor-and-site-correct | leak noticed | notices on background / stream | notice precision | strict precision | quality, fixed 16 s, retirement | quality, tuned delay, hold | cost s / stream |
|---|---|---|---|---|---|---|---|---|---|---|---|
| RungNoticer, z = 3 (default) | 0.917 [0.893, 0.939] | 0.890 [0.863, 0.916] | 0.917 [0.893, 0.939] | 0.887 [0.859, 0.914] | 0.460 [0.384, 0.536] | 4.00 [3.76, 4.24] | 0.855 [0.847, 0.863] | 0.748 [0.737, 0.759] | 0.489 [0.448, 0.532] | 0.497 [0.456, 0.539] (16 s) | 0.664 [0.604, 0.728] |
| RungNoticer, z = 2 | 0.946 [0.927, 0.964] | 0.914 [0.890, 0.937] | 0.927 [0.905, 0.949] | 0.895 [0.868, 0.921] | 0.460 [0.387, 0.536] | 9.03 [8.63, 9.45] | 0.740 [0.729, 0.751] | 0.613 [0.600, 0.624] | 0.527 [0.482, 0.572] | 0.559 [0.515, 0.603] (16 s) | 0.646 [0.589, 0.706] |
| ChangeTriggered, q = 128 s | 0.032 [0.018, 0.048] | 0.032 [0.018, 0.048] | 0.022 [0.010, 0.034] | 0.022 [0.010, 0.034] | 0.000 [0.000, 0.000] | 8.76 [8.50, 9.01] | 0.128 [0.106, 0.151] | 0.038 [0.032, 0.045] | 0.016 [0.006, 0.028] | 0.013 [0.005, 0.024] (30 s) | 0.013 [0.006, 0.021] |
| EarliestAnchor, l = 0.25 s | 0.917 [0.893, 0.939] | 0.890 [0.863, 0.916] | 0.917 [0.893, 0.939] | 0.887 [0.859, 0.914] | 0.460 [0.384, 0.536] | 4.01 [3.78, 4.25] | 0.854 [0.846, 0.862] | 0.747 [0.736, 0.758] | 0.492 [0.450, 0.535] | 0.500 [0.458, 0.542] (16 s) | 0.664 [0.604, 0.728] |
| **ReanchorNoticer** (site, g = 20 ms, burst >= 2, z = 2) | **0.978 [0.966, 0.990]** | **0.952 [0.932, 0.970]** | 0.968 [0.953, 0.982] | 0.946 [0.926, 0.966] | 0.460 [0.390, 0.533] | **6.82 [6.47, 7.18]** | 0.802 [0.792, 0.812] | 0.672 [0.660, 0.683] | 0.551 [0.507, 0.597] | 0.591 [0.548, 0.636] (16 s) | 0.670 [0.614, 0.730] |

Beside it (full columns in `b2-noticers-table.csv`): leak anchor-correct 0.000 in every row; the
slow leak's site-correct share equals its noticed share; notices per incident 0.89, 0.97, 0.05, 0.89,
1.04; notices per stream 27.6, 34.8, 10.0, 27.6, 34.5; median notice latency among the noticed hard
incidents 2.95 s (rung z = 3), 0.60 s (z = 2), 0.22 s (q = 128), 2.95 s, **0.51 s** (`reanchor`).

**Paired against the M2 comparator (the rung at z = 2), same resamples** (`b2-paired.csv`):

| measure | `reanchor` minus rung z = 2 |
|---|---|
| hard non-leak noticed | +0.032 [+0.017, +0.048] |
| anchor-correct | +0.038 [+0.021, +0.055] |
| site-correct | +0.040 [+0.021, +0.061] |
| anchor-and-site-correct | +0.051 [+0.031, +0.074] |
| leak noticed | 0.000 [-0.046, +0.047] |
| notices on background / stream | -2.21 [-2.41, -2.01] |
| notice precision / strict precision | +0.062 [+0.057, +0.068] / +0.059 [+0.054, +0.065] |
| quality, fixed / held | +0.024 [+0.009, +0.040] / +0.032 [+0.014, +0.051] |

**The re-anchor's own effect** (`reanchor` at the rung's default threshold, site, 20 ms, burst >= 2,
against the rung at z = 3, `b2-paired-pure.csv`): anchor-correct 0.938 against 0.890 (+0.048 [+0.029,
+0.069]), background notices 2.30 against 4.00 (-1.70 [-1.90, -1.51]), noticed +0.040, anchor-and-site-
correct +0.051 [+0.031, +0.073], precision +0.062, quality +0.027 [+0.013, +0.042]. Of the 372 incidents,
330 were anchor-correct under the rung and still are, **19 are gained and 1 lost**, 22 are anchor-
correct under neither (`b2-transitions.csv`; gained by family: compound 4, cascade 9, split brain 6).

## What the numbers say

**1. Does the public later re-anchor close the mis-anchoring gap?** On the point estimate, yes:
0.952 against 0.944, within the budget (6.82 of 9.03), and the single-notice conjunction (0.946) also
clears the bar. The decisive evidence is the pure effect at the rung's own threshold: +19 gained, 1
lost, at fewer background notices (2.30, not 4.00). What it does not show: that the true value is above
0.944 (the interval is 0.932 to 0.970; the paired gain over the comparator is +0.038 [+0.021, +0.055],
which, read as M2's result 1 reads a candidate against the rung at z = 2, meets both the +0.03 margin and the paired lower bound above 0.01 and the background bound: the public status quo would pass the criterion built for the medium); that it holds at
other stream statistics (one setting; the 20 ms gap is a constant of this world's burst spacing, and the
stage 1 table shows the shortfall is gradual: 0.935 at 100 ms, 0.927 at 200 ms, 0.917 at 300 ms, 0.900
at 400 ms, against 0.938 at 20 ms, so a tick of 100 to 200 ms keeps most of it); or anything about the
leak. If it is the comparator, M2's result 1 would need anchor-correct >= 0.982 with the paired lower
bound above 0.962, against 18 incidents the re-anchor does not anchor correctly (below); the leak
result, whose comparator number is 0.460 for every non-flood noticer, is unchanged.

**What is left (18 of 372; `b2-residual.csv` and by hand).** Ten are noticed with an anchor 1 to 52 s
after the incident's first observation (compound 7, cascade 2, split brain 1), at latencies of 7 to 52
s: their first observation does not begin the burst that crosses the threshold, which is a *late-onset*
failure and the opposite of the stray problem (an earlier anchor is what B1's EarliestAnchor tried and
it lands on strays). Eight are never noticed (cascade 4, split brain 4); five have no background notice
within 5 s, so the stray explanation is not what holds them. I did not inspect where their observations
were attached (hypothesis: another incident's anomaly, a splitting problem). Two of 354 anchor-correct
incidents are not site-correct.

**2. What the two quality readings say about the confound.** The retirement confound is real and, for
the rung family, small: with the hold quality moves by +0.008 (z = 3), +0.032 (z = 2), +0.008
(EarliestAnchor), +0.040 (`reanchor`) and -0.003 for the degenerate row, and the order of the rows does
not change. It is the same size as the differences it was suspected of hiding (0.038 between z = 3 and
z = 2): the lifetimes show why (retired before the 16 s delay: 30 of 451 anomalies anchored on hard
incidents for z = 3, 81 of 513 for z = 2, 96 of 543 for `reanchor`; none with the hold,
`b2-lifetimes.csv`). Where retirement bites hardest it is the whole story: the supplementary run
(`b2-flood-hold.csv`, the hold at 16 s, not tuned) takes ChangeTriggered at q = 0.5 s from quality 0.000
to 0.621, q = 1 s from 0.000 to 0.667 and q = 2 s from 0.202 to 0.704. But those numbers are bought:
57 to 63 reasoner calls per stream (the rung: 2.2), 14.1 to 15.6 modelled seconds per stream (0.67), and
9,247 and 4,367 escalations the bill refused. **So neither reading alone measures noticing: a flood
scores 0.62 to 0.70 when its anomalies are kept live, because the selection oracle chooses the hard
ones for it.** Quality is only comparable at a stated cost. The other finding is a ceiling: with the
hold, `reanchor` scores 0.591, R10's injected-notice ceiling on the same streams and context is 0.586
(paired: +0.005 [-0.028, +0.038]; with the retirement -0.035 [-0.065, -0.005]), and the rung at z = 2
with the hold is -0.027 [-0.063, +0.010]. On hard non-leak quality the selection oracle's headroom for
noticing is, within noise, used up by the public re-anchor (`b2-vs-ceiling.csv`). Anchor-correct is
then the only measure in the table that still separates the rows, and the leak is the only place noticing
still has measured value.

**3. Where the new measures can still be gamed.**

- **Notice precision as specified (N16) is not flood-resistant.** Most abnormal observations in these
  streams belong to incidents, so a noticer that notices every one of them has precision 0.56 to 0.60
  (ChangeTriggered q = 0.5 s, 1 s; the rung at z = 2 is 0.74, `reanchor` 0.80). What exposes it is
  strict precision (0.019 to 0.020 for the floods, 0.67 for `reanchor`) and notices per incident (24 to
  33 against 1.04). I added strict precision to N15/N16 for this reason; M2's "reported beside" list
  names precision and notices per incident, and strict precision should be added.
- **Site-correct is 1.000 for the floods** (they are about every service) and is informative only with
  the conjunction and precision.
- **The 1 s anchor window is generous.** Moving an anchor to the second observation of a burst is
  invisible to anchor-correct; the site check sees only a change of service. The g = 10 ms
  configurations show the first signs (anchor-and-site-correct falls to 0.909 while anchor-correct
  stays 0.930). A noticer that anchors on the *last* observation of every burst within a second would be
  anchor-correct and useless as an anchor for a declaration.
- **The background budget counts notices by their anchor, so re-anchoring spends it.** `reanchor` makes
  about as many notices as the rung at z = 2 (34.5 against 34.8 per stream) and 2.2 fewer on background:
  the headroom is reclassification, not fewer notices. A lower threshold would use it (z = 1.5 is 13.8,
  over the budget, and not better: 0.957 against 0.952), so the budget is not what limits this row, but
  it is a different budget for a noticer that anchors better, and the chief should know the M2 bound is
  not a bound on how often a noticer fires.
- **A parameter tuned on 199 incidents** chose between configurations 1 incident apart; the held-out
  numbers agree (0.950/0.952, 0.935/0.938) and the surface is a plateau from 20 to 150 ms, so the choice
  is not the story, the family is.

## Readings and deviations (each with its reason)

1. **Tuning seeds are 10000-10099**, as B1 (not 31000-31099).
2. **`NoticeEntry::site` is optional** (`None` in hand-written records): so that B1's 29 fixtures stand
   unchanged and a test can say N13 to N16 change none of N1 to N12. The harness always gives one.
3. **Strict precision and the single-notice conjunction (N15, part of N16) go beyond the brief**,
   which asks for a site check and precision; they are two counts and a rule, and are what makes the
   flood visible.
4. **`isolation` (`site`/`any`) is my addition** to the brief's wording, which is the `site` reading;
   `any` is a stricter variant that the grid played. The table row is `site`.
5. **Two amendments to the tuning after the first stage 1 run and before any held-out run**, stated
   in `b2_common.py` and above. The first run's output is kept beside the amended one.
6. **The grid's z ladder and gaps are mine** (the brief leaves them); the budget is applied to the
   tuning streams literally (<= 9.03), and the held-out value is reported (6.82).
7. **`reanchor` re-anchors at notice only** (no re-anchor entry in the log; one notice per anomaly is
   what the loader's check and the evaluator's N11 assume). Stated as a limit.
8. **The hold keeps only hard-anchored anomalies** (the oracle's own privilege), so the hold arm's
   quality is not the quality of a noticer that was asked about everything.
9. **The table's noticing columns come from the arms without the hold**; the hold arms supply
   `quality_held` and, separately, `b2-hold-effect.csv`.
10. **The held-out run is larger than the table** (65 arms: B1's whole grid, both reanchor grids, the
    ladder, the table's `reanchor` arm which duplicates its grid arm byte for byte, and five hold arms);
    a **supplementary run** (`b2-supp`, seven flood-like noticers with the hold at 16 s) was added after
    I had seen the table, to measure the confound where it bites, not to choose anything.
11. **`StreamPolicySpec::from_parts` gained a parameter** and `OracleSelection` a field: nine
    test literals in `tests/stream_r5.rs`, `stream_context.rs`, `stream_r10.rs` and one call in
    `stream_noticer.rs` were updated (mechanical, inside the lab's territory).
12. **My scripts are in `scripts/`**, as the territory says; B1's are in `experiments/exploration/scripts/`
    and are imported, not edited.

## Verification

**By running.** The byte-identity gate (62 of 62). Evaluator: 39 fixtures, 3 new tests of the fixtures and 5
generated-stream tests (30 streams, and every stream the unit played); cargo-mutants 80 mutants (78 caught, 2 unviable, 0 missed) and 20 hand mutants
(20 caught). Noticer: 15 tests in `tests/stream_reanchor.rs` (the rule as a pure function with every
boundary, the readings, the failure through the rung, the identity with the rung when it never moves
over whole segments, the files and their replay). Hold: 4 tests in `tests/stream_hold.rs`, including
that with the hold no hard-anchored anomaly retires before it can be asked (over 24 streams, and that
without it some do, so the test is not vacuous). Loader and measures: 13 new tests in
`analysis/tests/test_notices.py`, 6 in `test_b2_scripts.py`. Provenance (`b2_provenance.py`): the
table's rung arms are R10's byte for byte (modulo run id), a lookback of 0 and a re-anchor with a gap
longer than the burst window and `any` are the rung's bytes, the table's `reanchor` arm equals its grid
arm, the incidents are the same in every arm of a run, no segment was step-capped. cargo-mutants on `noticer_reanchor.rs` (with the tests of `stream_reanchor.rs` and `stream_noticer.rs`):
35 mutants, first run 29 caught, 4 missed, 2 unviable; the four misses were the delegation of `score` and
`refresh` to the rung's (no test read them), a test was added, and the four rerun caught: **33 caught, 2
unviable, 0 missed** (`b2-mutants.md`).
GATES_PLACEHOLDER

**Runs** (`b2-driver-log.csv`, `b2-run-index.csv`; outputs in `artifacts/runs/` of the worktree,
git-ignored): every driver exit was 0; none was refused by the driver, none excluded or step-capped.

| run | exit | wall s | what |
|---|---|---|---|
| `xcheck-r6-heldout-b5-rho0.7` | 0 | 266 | the gate |
| `b2-tune1-b5-rho0.7` | 0 | 54 | 26 arms, 100 tuning streams: stage 1 |
| `b2-tune1b-b5-rho0.7` | 0 | 27 | 14 arms: the extension (amendment 1) |
| `b2-tune2-b5-rho0.7` | 0 | 12 | 6 arms: the threshold ladder |
| `b2-tunedelay-b5-rho0.7` | 0 | 124 | 65 arms: five noticers x 13 delays with the hold |
| `b2-heldout-b5-rho0.7` | 0 | 315 | 65 arms, 200 held-out streams |
| `b2-supp-b5-rho0.7` | 0 | 110 | 7 arms, 200 held-out streams: the flood-like rows with the hold |
| FINAL_RUN_ROW |

The driver waited for another worker's process 6 polls (the gate) and 9 polls (stage 1); my cargo
invocations never found a `gordian-run` to wait for. I checked, as instructed, for `gordian-run`
before cargo and also for `cargo` and `rustc` before runs; I did not check for another worker's
`cargo bench` before my own builds, so I cannot exclude that a build of mine overlapped another
lab's measurement. The bill refused probes for the ChangeTriggered flood rows as in B1 (181, 172, 80,
54, 4 over 200 streams) and, with the hold, 9,247 and 4,367 escalations for q = 0.5 s and 1 s; it
refused nothing for the five table noticers (`b2-refusals.csv`).

**Assumed.** That the cgroup limits applied as the driver reports (the internal/external ratios are
0.91 to 0.94 for these runs, no tolerance declared); that percentile bootstrap intervals over whole
streams describe the sampling uncertainty of 200 streams; that the first occupied service is the
diagnosed site for every incident of every held-out stream (checked on 30 generated streams and on every stream the tuning and held-out runs played, 300 streams, at the default parameters, `tests/notice_generated.rs`; the assumption inside it is that the manifests' streams are the defaults at those seeds, as the manifest changes only the reasoner's `b` and `rho`); that a hold does not change later noticing beyond what `b2-hold-effect.csv`
measures; that the held-out numbers generalize no further than this world, one setting, b = 5, rho = 0.7.

**Hidden record.** I did not read `crates/gordian-stream/HIDDEN-DESIGN.md`. To check N13 I read
`crates/gordian-stream/src/oracle.rs` (the truth the evaluator reads) and listed the lines of
`crates/gordian-stream/src` that mention `occupies`. I read the review log's descriptions of hidden
structure (mis-anchoring on background about 0.3 s earlier, the leak's sub-alarm readings) as
hypotheses; no rule of the noticer or the oracle option encodes them. `ReanchorNoticer` reads only
the public fields (the guard `scripts/check-no-oracle.sh` passes with its file). Its three
parameters were tuned against the evaluator's measures on the tuning streams, as every parameter of
every baseline is, which is a route by which a hidden label shaped a constant (the gap), and is stated
as such.

## What I am least sure of

- **That 0.952 is above 0.944.** It is a point estimate; the lower bound is 0.932.
- **The gap of 20 ms.** It sits at the burst's own spacing in this world, and the legitimate bursts are
  mostly "followed within 20 ms": the rule is close to "the anchor is a stray if nothing follows it
  immediately". If the world's burst spacing differed, or the medium's tick hid it, the number would
  move; the sensitivity table says how slowly (above), and I have not tested another world.
- **The residual explanations** (late-onset; splitting) are readings of 18 incidents, one of them
  checked by hand per incident only for the numbers listed.
- **The tie amendment.** It was made after seeing the tie, in the direction of the brief's wording; the
  two readings are indistinguishable on anchor-correct and the held-out grid shows both.

## What the chief should examine most carefully

1. **M2's comparator and bounds** if re-fixed to this row: anchor-correct 0.952 [0.932, 0.970], 6.82
   background notices, leak noticed 0.460, notices per incident 1.04; and whether the 9.03 bound should
   stay a bound on background-anchored notices given that re-anchoring spends it (above).
2. **Whether "noticed" and "anchor-correct" should be read for the medium at a tick of 100 ms to 2 s**
   with the gap this row needs (20 ms, `offset_ns`): the comparator uses observation instants the
   medium may not have at tick resolution; the sensitivity rows at 100 and 200 ms are the fair
   comparison for a 100 ms tick.
3. **N16's precision**, which as specified rewards floods; strict precision should be reported beside it.
4. **The two amendments** and the plateau they sit on (`b2-tuning-stage1*.csv`).
5. **The hold's privilege** and the flood-hold rows: quality is only comparable with calls and cost.

## What I would test next

- A noticer that **splits** an anomaly (a burst attached to another incident's anomaly) for the eight
  never-noticed, and one aimed at the **late onset** (an anchor on a lone early observation that is
  followed, seconds later, by the incident's burst), which is the opposite error and needs a way to tell
  such an observation from a stray that is not the observation's gap.
- A **parameter-free** re-anchor ("the anchor is the first attached observation that has a successor
  within `d`") to see whether the plateau is a gap or a property.
- The same rule and the same grid on **another stream world** (W1's statistics; a different burst
  spacing) and at the medium's tick lengths, before M2.
- The **leak**: a noticer that reads benign values, which the table shows no abnormal-only noticer can
  be anchor-correct on.
- Quality at **b = 2.5 and 8**, where the selection oracle's headroom differs.
