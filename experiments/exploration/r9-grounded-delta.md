# R9: grounding the distractor penalty in the world

**Status: exploration.** Nothing here tests a hypothesis and nothing here may later be cited as
confirmation. Written by the R9 worker on branch `grounded-delta`; the coordinator decides what
follows. Numbers in tables are copied from the CSV and JSON files in this directory (`r9-*.csv`,
`r9-*.json`); prose numbers were typed by the worker from them. The delta below is the loss of **one
program**, the reader of section 3, on this world's own questions. It is not a property of real models,
and "not significant" is never "equivalent".

## 1. Result in brief

1. **Outcome as the criterion is written: unresolved.**
   - The reader precondition holds: A(0) = 0.958 on 312 hard evaluation questions, 90% lower bound
     0.939 (the floor is 0.80 with the bound above 0.70).
   - delta\* = 0.0135 per 100 non-decisive references, 90% interval [0.0055, 0.0225] (10,000
     cluster-bootstrap resamples over streams, seed 9900).
   - The reruns of R7's comparison (b = 5, rho = 0.7) give G = 0.032 [0.011, 0.054] at delta\*_lo, 0.073
     [0.042, 0.104] at delta\* and 0.089 [0.059, 0.119] at delta\*_hi.
   - The R6 regime needs the upper bound of G at delta\*_hi below 0.10 (it is 0.119). The R7 regime needs
     G at delta\*_lo at least 0.10 (it is 0.032). Neither holds. Section 8.
2. **The reader loses very little to distractors, and what it loses is not an exponential.** Its
   accuracy on hard questions is 0.958, 0.939, 0.904, 0.913, 0.933 at m = 0, 50, 100, 200, 400 (control,
   no decisive evidence, 0.045). The curve dips by 0.054 at m = 100 and recovers. The exponential fit
   misses by 0.042 at m = 100 (largest absolute residual over the five levels). delta\* is a summary of that
   curve, not a law. Section 4.
3. **The loss is mostly information the context no longer holds, not distraction by look-alikes.** At
   every level above 0 the wrong answers are repaired when the focus incident's own burst, heartbeats and
   closure are added to the context (100% at m = 50 and 100, 96% at 200, 76% at 400), and at m = 400
   removing only the look-alike free-form messages repairs 17 of 21 wrong answers (81%). Section 5.
4. **A count-based delta does not transfer to a context that leaves services out.** Against the
   fitted curve at each arm's mean references per call, the reader on R6's real contexts is within 0.05
   for the window, cooccur and neighbourhood builders (differences -0.046 to +0.012) and 0.294 below it for
   the rung's own context, which excludes the services where a cascade's partner sits. Section 7.
5. **A stronger reader gives a smaller delta; this one is not the strongest possible.** On the
   development set, as the reader was strengthened its point delta fell from 0.34 (R8's rule reader) to
   0.036, 0.0245, 0.0095 and 0.0085 (section 3). The program was built in bounded effort.
6. **The plan's feasibility note was optimistic.** It guessed that the R6 regime "needs roughly delta\*_hi
   below 0.05". At 0.0225 the selected builder is already 0.089 below the ceiling (upper bound 0.119): G
   rises at once from R6's 0.027 at delta 0, and the interval of delta\* at this N is about 0.017 wide, so
   the R6 clause would need delta\*_hi at about 0.012 or below (G's upper bound is 0.054 at 0.0055 and 0.104
   at 0.0135), a point estimate of about 0.004.

## 2. What was built and run

**Dumper option** (`crates/gordian-stream/src/questions.rs`, `examples/questions.rs`). `--pool own`
puts the focus incident's own non-decisive observations (heartbeats, flaps, closure, presentation) in the
pool, which then holds every observation within 40 s of the focus except the focus and the decisive
evidence, and adds a `pool_roles` field (`background:<kind>`, `own:<role>`, `other:<role>`) for the
evaluator's analysis. The default (`--pool others`) is R8's pool and its output is unchanged
(`pool_roles` is not written). Four new tests: default equals the old pool and has no roles key; the own
pool equals an independent recomputation from the labels; own minus default is exactly the incident's own
non-decisive observations; the record's keys. A mutation (excluding `Closure` instead of `Decisive`) is
caught by two of them. The pool excludes the focus itself because a question shows it as the first alarm.

**Questions** (`r9_common.py`, `r9_select.py`; R8's levels, nested seeded permutation, control and
correctness reused through `r8_common`). One hard incident per stream at position `seed mod k` among the
stream's eligible ones (not a slow leak, pool of at least 400); plain likewise until 100.

| Set | Seeds | Streams | Hard incidents | Excluded (pool under 400) | Streams with no eligible hard | Hard questions | Plain questions |
|---|---|---|---|---|---|---|---|
| Development | 29000 to 29999 | 1,000 | 1,900 | 63 | 205 | 795 (Compound 271, Cascade 249, SplitBrain 275) | 300 |
| Evaluation | 30000 to 30399 | 400 | 758 | 19 | 88 | 312 (Compound 108, Cascade 97, SplitBrain 107; mimic 152, contradict 160) | 100 |

(The development set was dumped in two parts, 29000 to 29399 and 29400 to 29999; `r9-dev-history.csv`
has what each reader version scored on it. The evaluation dump had sha256 `7e7ab0db...1f4f1f03` and was
deleted after selection to keep disk free; it is a pure function of the dumper's arguments.)

**Runs** (every run through `scripts/run-driver.sh`; release build from commit `38ee5e0`, and
`git diff 38ee5e0 HEAD -- crates Cargo.toml Cargo.lock scripts .cargo` is empty, so every later commit
changes scripts and experiments only):

| Run | Exit | Wall s |
|---|---|---|
| `r9-tune-b5-rho0.7-d0.0055`, `-d0.0135`, `-d0.0225` (first attempt) | 11 each | 0 (refused: another worker's `cargo test` was running) |
| `r9-tune-b5-rho0.7-d0.0055` | 0 | 106 |
| `r9-tune-b5-rho0.7-d0.0135` | 0 | 104 |
| `r9-tune-b5-rho0.7-d0.0225` | 0 | 107 |
| `r9-heldout-b5-rho0.7-d0.0055` | 0 | 40 |
| `r9-heldout-b5-rho0.7-d0.0135` | 0 | 41 |
| `r9-heldout-b5-rho0.7-d0.0225` | 0 | 40 |

Nothing failed, timed out or was excluded. The three refusals wrote nothing and the manifests were
rerun unchanged at the same revision (`r9-driver-log.csv`). Each run's internal/external ratio is in
`r9-run-index.csv` (0.945 for the first). The Python of the evaluation and the direct check ran under
`scripts/cgroup-run.sh` on cores 0-2 (`artifacts/runs/r9/final/*usage.json`).

## 3. The reader and its development

**What it is** (`r9_reader.py`, weights in `r9_reader_weights.json`). A deterministic program. `read(services,
focus, context)` returns `(kind, site, trace)`. It is shown the public graph, the focus record and the
context's observation records at full time resolution, and nothing else (`r9_dev.py` strips the question to
exactly that; a test checks that the reader's source names no hidden field and ignores extra fields).
Features, each computed from the stated rules:

- the burst (first 300 ms) and phase 2 (6 to 16 s after the first alarm);
- free-form messages at the site and at the best candidate partner in each of three groups (unconnected,
  dependent, upstream), as tail probabilities against the background rate read from the context itself
  over the time the context covers;
- repeated ids within the context (at the site, and shared between the site and a candidate);
- alarm clusters (`ErrorRate` with `Latency` within 50 ms, no characteristic message near) and abnormal
  counters at candidates, `MixedSignals` at the site and at others and whether it comes with an alarm at the
  same service, characteristic messages and non-`ErrorRate` counters at the site in phase 2, whether the
  site's burst fits one known kind, readings that follow a candidate's first alarm (heartbeats belong to a
  site, not to a partner).

A softmax over four classes (not hard, Compound, Cascade, SplitBrain) combines them, with weights fitted on
development questions only (`r9_train.py`, 5-fold cross-validation over streams reports the fit's
optimism). "Not hard" is answered by the first world's public rules read from the burst. The two
populations are weighted equally in the fit and each hard family equally within the hard half, so a reader
cannot gain by naming a hard kind whatever it sees. The weights carry the development labels; the evaluation
seeds are disjoint.

**What it does not use:** hidden labels, tiers, roles, the pool's per-stream vocabulary assignment, the
background's hidden rates, stream state outside the context. It does use that phase 2 spans 6 to 16 s, that
a burst lasts 300 ms, that a partner alarm is `ErrorRate` and `Latency`, that heartbeats are at the site: all
from `HIDDEN-DESIGN.md` sections 4 and 4.2.

**Development history** on 795 hard and 300 plain development questions (`r9-dev-history.csv`; the
point delta is `r8_stats.fit_delta` on the row's A(m) with p0 = 0.03, or 0.06 for the baseline, which
are assumptions where the row has no control):

| Version | What changed | hard A at m = 0, 50, 100, 200, 400 | point delta |
|---|---|---|---|
| baseline | R8's rule reader (the prompt's stated rules, windowed) on R9's pool | 0.883, 0.522, 0.453, 0.457, 0.519 | 0.34 |
| v1 | rule features plus softmax (5-fold CV) | 0.945, 0.892, 0.860, 0.840, 0.864 | 0.036 |
| v2 | partner-centric features (best unconnected, best connected candidate) | 0.946, 0.913, 0.887, 0.868, 0.891 | 0.0245 |
| v3 | adds candidate repeated ids, a density feature, absence-times-density | no gain (on 320 questions 0.956, 0.912, 0.881, 0.853, 0.884 against v2's 0.950, 0.922, 0.891, 0.853, 0.878) | not kept |
| v4 | three candidate groups, `MixedSignals` tied to an alarm at the same service | 0.946, 0.918, 0.916, 0.907, 0.932 | 0.0095 |
| v5 | adds the site-plus-candidate pair count to v4 | 0.945, 0.917, 0.917, 0.907, 0.930 | not kept |
| v6 (frozen) | v4 with the background rate read over the context's own time coverage | 0.943, 0.918, 0.919, 0.907, 0.931 | 0.0085 |
| L2 weight 0.1, 0.3, 3, 10 | on v4 | within 0.013 of v4 at every level | not kept |
| MLP 8 and 16 hidden units | on v6 features | at most +0.9 points at one level, within noise | not kept |

Dropped each time because it did not help beyond the noise of a cross-validation on this set. The one
check that came from the likely use, a robustness check on development data of a window context (the dumper's
pool in time order, the most recent N of the window up to a call at +16 s; `r9_dev_window.py`): accuracy
0.777 (W 20 s, N 128), 0.962 (20 s, 256), 0.957 (40 s, 256), 0.953 (80 s, 512). It prompted the change in v6.

**Freeze.** Commit `cccbcf2` (tag `r9-reader-frozen`), before any number on seeds 30000 and above.
`r9-reader-freeze.json` holds the sha256 of the reader (`8bb362b0...062068cb`) and the weights
(`fc08706b...cddd73`). The criterion's readings (`r9_stats.py`) were committed in it. The evaluation
(`a16f839`) and the direct check followed; the reader was not changed after them.

## 4. A(m), the fit and the misfit (hard evaluation questions, 312)

`r9-levels-hard.csv`, `r9-fit-hard.json`. 90% intervals are cluster bootstrap over streams (one question each).

| Level | A(m) [90%] |
|---|---|
| 0 | 0.958 [0.939, 0.974] |
| 50 | 0.939 [0.917, 0.962] |
| 100 | 0.904 [0.875, 0.929] |
| 200 | 0.913 [0.885, 0.939] |
| 400 | 0.933 [0.910, 0.955] |
| control (q = 0, m = 50), p0 | 0.045 [0.026, 0.064] |

- **delta\*** = 0.0135 [0.0055, 0.0225] (p0 held at the sample's value: [0.0050, 0.0225]; no resample was
  unidentified or at the grid's edge).
- **Misfit:** residuals `A(m) - fit` at m = 0, 50, 100, 200, 400: 0, -0.013, -0.042, -0.021, +0.022; the largest
  absolute is **0.042**. A fit with a free amplitude gives delta 0.0045 and misfit 0.028. The curve is not an
  exponential in m.
- **Precondition:** A(0) 0.958, lower bound 0.939: holds.

By family and mode (`r9-groups-hard.csv`; accuracy at m = 0, 50, 100, 200, 400, control):

| Group | n | 0 | 50 | 100 | 200 | 400 | control |
|---|---|---|---|---|---|---|---|
| Compound / contradict | 49 | 1.000 | 0.980 | 0.939 | 0.918 | 0.939 | 0.000 |
| Compound / mimic | 59 | 1.000 | 1.000 | 0.983 | 0.966 | 0.949 | 0.000 |
| Cascade / contradict | 58 | 1.000 | 0.897 | 0.776 | 0.879 | 0.879 | 0.034 |
| Cascade / mimic | 39 | 1.000 | 1.000 | 1.000 | 1.000 | 0.949 | 0.000 |
| SplitBrain / contradict | 53 | 0.755 | 0.774 | 0.755 | 0.736 | 0.906 | 0.075 |
| SplitBrain / mimic | 54 | 1.000 | 1.000 | 1.000 | 1.000 | 0.981 | 0.148 |
| mode = contradict | 160 | 0.919 | 0.881 | 0.819 | 0.844 | 0.906 | 0.037 |
| mode = mimic | 152 | 1.000 | 1.000 | 0.993 | 0.987 | 0.961 | 0.053 |

- **Mimic** incidents carry their rule-breaking evidence in phase 2, which is decisive, so the reader keeps
  it at every level and loses only slowly (0.961 at m = 400).
- **Contradict** incidents carry it in the burst, which is not decisive: at m = 0 it is absent, and as m
  grows more of it is sampled into the context. That is why SplitBrain / contradict *rises* with m (0.755 to 0.906: at m = 0 a split brain with an unconnected peer is indistinguishable from a cascade by the public
  graph) and Cascade / contradict dips at m = 100 (0.776) before recovering.
- The worst level for the whole set, m = 100, is the middle: enough distractors to bury a partial burst,
  not yet enough of the burst sampled in.

## 5. What the wrong answers rest on (the fooled-by analysis)

The wrong answers above m = 0 are 19, 30, 27 and 21 at m = 50, 100, 200, 400 (`r9-fooled-hard.csv`,
`r9-fooled-summary.csv`). Defined before the evaluation (`r9_evaluate.py`):

- **trace cites a look-alike:** the reader's trace (the free-form messages at the site and at the best
  candidate of each group, which its features were computed from) contains at least one message that is not
  decisive evidence of the focus incident;
- **look-alike FF repairs:** the answer becomes correct when every free-form message that is not decisive
  evidence (background, other incidents, the incident's own) is removed from the context;
- **only own repairs:** the answer becomes correct when only the decisive evidence and the focus incident's
  own observations stay (everything of the background and other incidents removed);
- **full own repairs:** the answer becomes correct when all of the focus incident's own observations in
  the pool are added.

| Level | Wrong | Trace cites a look-alike | Look-alike FF repairs | Only own repairs | Full own repairs |
|---|---|---|---|---|---|
| 50 | 19 | 11 (0.58) | 8 (0.42) | 8 (0.42) | 19 (1.00) |
| 100 | 30 | 19 (0.63) | 18 (0.60) | 21 (0.70) | 30 (1.00) |
| 200 | 27 | 25 (0.93) | 19 (0.70) | 19 (0.70) | 26 (0.96) |
| 400 | 21 | 20 (0.95) | 17 (0.81) | 20 (0.95) | 16 (0.76) |

- At m = 400, **81% of the reader's wrong answers are explained by look-alike free-form messages** in the
  counterfactual sense (95% in the looser trace sense). But 76% are also repaired by supplying the
  incident's own missing observations, and at m = 50 and 100 every wrong answer is. The look-alikes
  matter at high m; before that, what is missing is the incident's own evidence that the random draw left
  out. The two readings overlap (an error can be repaired either way).
- The wrong answer is almost always another hard family: the reader names "not hard" for at most 6 of 312
  questions at any level. It nearly always knows that the incident is a hard one and loses which.

## 6. Plain incidents, a labelled proxy

100 questions, one per stream (`r9-levels-plain.csv`, `r9-fit-plain.json`). **A proxy, not a regime
claim**: the plan asks for it beside the criterion.

| Level | 0 | 50 | 100 | 200 | 400 | control |
|---|---|---|---|---|---|---|
| A(m) | 0.970 | 0.970 | 0.960 | 0.940 | 0.910 | 0.230 |

delta on plain incidents is 0.0205 [0.0075, 0.0365], misfit 0.008. The control is high (0.23) because the
reader answers ResourceExhausted when it sees nothing, which is right for one plain question in five. The
only family with a clear loss is DependencyDown (0.81 to 0.63): the duo presentation leaves two kinds open
and the reader names the first listed.

## 7. The direct check on R6's diagnostic contexts

`r9_direct.py` (`r9-direct.csv`, `r9-direct-calls.csv`): the frozen reader on the contexts the builders of R6's
diagnostic run (`r6-diag-b5-rho0.7`, 100 streams, every ledger kept) actually sent. 199 calls about hard
incidents per arm, 29 of them slow-leak (excluded: R9's questions exclude the leak and the reader never names
it); 170 counted per arm (Compound 71, Cascade 32, SplitBrain 67). Predicted: the fitted curve of section 4 at the
arm's mean references per call. The reader's accuracy is strict (kind and site), 90% cluster bootstrap over
streams (seed 9901).

| Arm | Mean refs per call | Mean m | Reader accuracy [90%] | Predicted (exp, at refs) | Predicted (interpolated A(m)) | Accuracy minus exp |
|---|---|---|---|---|---|---|
| ceiling `oracle_selection_context` (reference, x = 0) | 5.4 | 0.0 | 0.900 [0.857, 0.940] | 0.958 | 0.958 | -0.058 |
| `sel_rung` | 43.0 | 39.6 | 0.659 [0.597, 0.720] | 0.953 | 0.942 | -0.294 |
| `sel_coc_d08000_n256` | 178.4 | 173.4 | 0.924 [0.891, 0.956] | 0.935 | 0.911 | -0.013 |
| `sel_nbh_k3_n256` | 185.8 | 180.7 | 0.894 [0.853, 0.933] | 0.936 | 0.912 | -0.042 |
| `sel_coc_d16000_n256` | 189.0 | 183.8 | 0.906 [0.871, 0.940] | 0.935 | 0.912 | -0.029 |
| `sel_nbh_k4_n256` | 190.5 | 185.3 | 0.906 [0.871, 0.940] | 0.935 | 0.913 | -0.029 |
| `sel_win_w40_n256` | 250.0 | 244.8 | 0.882 [0.839, 0.925] | 0.928 | 0.918 | -0.046 |
| `sel_win_w80_n512` | 488.2 | 482.8 | 0.912 [0.879, 0.944] | 0.900 | 0.933 | +0.012 |

- **For the window, cooccur and neighbourhood builders the curve is within 0.05** of the reader (and the
  intervals cover the prediction for four of the six). The count of references predicts the reader's
  accuracy there about as well as the sample variation allows.
- **For the rung's own context it does not transfer**: 0.659 against 0.953. The rung admits only the site
  and its dependents, so the cascade's partner (unconnected) is never in the context: Cascade accuracy is
  0.00 (of 32) for this arm and 0.91 to 0.97 for the others (`r9-direct-calls.csv`). Where the rung's
  context holds all the evidence that had arrived (41% of its calls) the reader is 0.986 right. The count
  of references says nothing about that loss, which is information absent, not distraction.
- **Beside** (`r9_direct_anchor.py`, `r9-direct-anchor.csv`, post hoc, not in the criterion): in 6 of 170
  calls per arm the call's anchor is more than 0.1 s after the incident's onset, and the reader, which
  takes the first alarm as onset, is right on none of them (0 of 6 for every builder). Without them the
  ceiling reads 0.945, and the builders 0.927 to 0.970.
- The ceiling's 0.900 is below A(0) = 0.958 for those reasons (six late anchors, and the family mix of this
  run: the SplitBrain share, whose m = 0 accuracy is 0.88, is 39% here against 34%).

## 8. The rerun of R7's comparison

R7's machinery unchanged (`r9_rerun.py`; section 9 lists the three things that are new): b = 5, rho = 0.7,
selection oracle at R6's 16 s delay, R6's grids on the 100 tuning streams, R6's 200 held-out streams, the
selected builder the carried configuration of highest tuning quality. None of the three values is within
0.005 of R7's grid or of 0, so no run of R7 or R6 was reused: three tuning and three held-out runs.

| Value | delta | Selected builder (tuning quality) | Ceiling quality @ refs per call | Selected quality @ refs per call | G [90%] | R6 condition: upper bound below 0.10 | R7 condition: G at least 0.10, lower above 0.05 |
|---|---|---|---|---|---|---|---|
| delta\*_lo | 0.0055 | `window` W 80 s, N 512 (0.759) | 0.820 @ 5.3 | 0.788 @ 485.8 | 0.032 [0.011, 0.054] | yes (not the one the criterion uses) | no |
| delta\* | 0.0135 | `cooccur` delta 16 s, N 256 (0.734) | 0.820 @ 5.3 | 0.747 @ 187.9 | 0.073 [0.042, 0.104] | no | no |
| delta\*_hi | 0.0225 | `neighbourhood` k 4, N 256 (0.719) | 0.820 @ 5.3 | 0.731 @ 189.2 | 0.089 [0.059, 0.119] | **no (0.119)** | no |

(`r9-rerun-criterion.csv`, `r9-outcome.json`; bootstrap seeds 9710 to 9712, 372 hard incidents.) The R6
condition is read at delta\*_hi: its upper bound is 0.119, so it fails by 0.019. The R7 condition is read at
delta\*_lo: G is 0.032, far below. **Outcome: unresolved.** For reference, from R7 and R6: G(0) = 0.027
[0.005, 0.049] and G(0.05) = 0.097 [0.066, 0.129].

Beside the criterion (`r9-rerun-points.csv`), never folded in. Hard-incident quality (slow leak
excluded), references per call, critical misses (as `r6_stats` counts them), plain accuracy and calls
refused, held-out:

| delta | Row | Quality | Refs per call | Critical misses | Plain accuracy | Refused |
|---|---|---|---|---|---|---|
| 0.0055 | ceiling | 0.820 | 5.3 | 255 | 0.741 | 0 |
| 0.0055 | selected `window` 80 s, N 512 | 0.788 | 485.8 | 258 | 0.741 | 0 |
| 0.0055 | held-out best `window` 40 s, N 256 (gap 0.030 [0.005, 0.055]) | 0.790 | 251.1 | 257 | 0.741 | 0 |
| 0.0135 | ceiling | 0.820 | 5.3 | 255 | 0.741 | 0 |
| 0.0135 | selected `cooccur` 16 s, N 256 | 0.747 | 187.9 | 262 | 0.741 | 0 |
| 0.0135 | held-out best `window` 40 s, N 512 (gap 0.046 [0.019, 0.074]) | 0.774 | 323.2 | 257 | 0.741 | 0 |
| 0.0225 | ceiling | 0.820 | 5.3 | 255 | 0.741 | 0 |
| 0.0225 | selected `neighbourhood` k 4, N 256 | 0.731 | 189.2 | 265 | 0.741 | 0 |
| 0.0225 | held-out best `window` 40 s, N 512 (gap 0.067 [0.039, 0.097]) | 0.753 | 323.2 | 258 | 0.741 | 0 |

- The plain accuracy is 0.741 in every selection-oracle row (the selection oracle escalates hard
  anomalies only), as in R7. R4's oracle is 0.952 at 5.1 references per call and the rung's own context
  0.484 to 0.487 at 43.3, in all three runs.
- **The held-out-best builder, which the plan says flatters the builders, would satisfy the R6
  condition's upper bound at delta\*_hi (gap 0.067, upper bound 0.097).** The criterion uses the
  tuning-selected one, so this is beside it and decides nothing. The two differ because the tuning
  streams chose a smaller context than the held-out streams prefer.
- The selected builder changes with delta (`window` 80 s, then `cooccur` 16 s, then `neighbourhood` 4
  hops), all between 188 and 486 references per call; at these small deltas the penalty, at about 2% to 4% of
  a 190-reference context's informed probability, still decides which of several near-equal builders ranks
  first on 100 tuning streams.
- Ceiling and R4's oracle are byte-identical to R6's results at every value (`r9-ceiling-vs-r6.csv`,
  6 of 6), the held-out incidents are R6's in every run, every manifest records its penalty
  (`r9_rerun.py provenance`).

## 9. Readings and deviations, each with its reason

Fixed in the scripts before the evaluation number they bear on (the commit that fixed each is in the git log).

1. **Pool.** The focus is not in the pool (a question shows it). The pool's own observations include the
   burst, so a reader at higher m sees more of it; the plan's `m` counts them as distractors, and the
   reader's curve includes what they are worth to it.
2. **One hard question per stream** at `seed mod k`; the plan says "at least 200 questions, each from a
   distinct stream segment where possible; cluster by stream": 312, a cluster is a question.
3. **Evaluation seeds** 30000 to 30399 (the first 400 streams), chosen before any reader number; the
   development set is all of 29000 to 29999 (795 hard questions).
4. **Weighting of the reader's training:** plain and hard questions equal, hard families equal. A reading
   of "the strongest reader that has the reasoner's knowledge" that stops it winning by always naming a hard
   kind. The control's p0 is then 0.045 because with no evidence the reader says "not hard".
5. **A(0) in the fit** is the observed level-0 share and p0 is re-estimated in every resample: R8's readings
   (`r9_stats.py`). The misfit is computed from the point estimates, at the five levels.
6. **The rerun values:** a delta within 0.005 of 0 uses R6's files, within 0.005 of R7's grid reuses R7's
   runs, a negative value is run as 0 (the simulator's clamp). None applied here. The values are the
   interval's grid points (0.0005 step), run at four decimals.
7. **Three new things in the rerun, nothing else:** which deltas, the run seeds (9100 + k, 9200 + k; they
   order interleaved arms only) and the bootstrap seeds (9710 + k; R7 used 9600 + k). R7's manifest writer
   is called with the run id function replaced so runs are named `r9-...`.
8. **Direct check:** accuracy is strict (kind and site, the site being the service of the call's anchor);
   the kind-only reading is in the CSV. Slow-leak calls are excluded and counted. The prediction is the
   exponential at the arm's mean references per call (the plan's words), with the interpolated A(m) and the
   m-based variant beside it. Ceiling arms are a labelled reference at x = 0.
9. **The fooled-by analysis** has two definitions (the trace's and the counterfactual's), both written
   before the evaluation; the plan names only "from the reader's own trace". The counterfactuals read the
   labels in the evaluator.
10. **The anchor analysis of section 7 and the window check on development data were not in the plan**; both
    are post hoc and not in the criterion.
11. **Deviation: a pgrep that matched itself.** The first core-sharing checks (`pgrep -f` on a command
    line containing the pattern) matched the shell's own command line, so one build (the first debug
    test of the dumper) started without a valid check. The release build and everything after
    used a pattern that cannot match itself and a wait for builds as well as runs. Waits recorded: 30 s
    (false positive), 60 s before writing the tuning manifests and 150 s before the first tuning run; the
    driver refused the three tuning runs once (exit 11, a build by the other worker was running).
12. **The release binary** was built before the reader existed from commit `38ee5e0`; nothing under
    `crates/` changed afterwards.

**Hidden record read:** `HIDDEN-DESIGN.md` sections 3, 4 (4.1, 4.2, 4.3), 5, 6, 7, 9 and 11, and the module
header and type definitions of `incident.rs` (first 60 lines). No generator code. The reader uses sections 4
and 4.2 and the facts in 9 and 11 about the background's free-form messages only to know that look-alikes
exist; it reads no rate from them. While developing the reader I looked at labelled development questions
(with roles) to see how the families present; no evaluation question was looked at before the freeze.

## 10. What was verified by running something, and what was assumed

**Verified by running:** the dumper's pool in both modes against an independent recomputation (4 tests, one
mutation caught by two); the whole gate set on the final tree (`cargo fmt --all -- --check`; `cargo clippy
--locked --workspace --all-targets -- -D warnings`; `cargo test --locked --workspace --no-fail-fast`: 564
tests, 0 failed; `scripts/check-no-oracle.sh`; `pytest -W error analysis`: 339 passed, 4 deselected); the
reader's blindness (a test on its source and on extra fields); the statistics (a known delta recovered at 0,
0.05 and 0.2, the precondition failing for a weak reader, every outcome reachable: `r9_stats.py selftest`); that
the ceiling and R4's oracle are byte-identical to R6's in the rerun; that the held-out incidents are R6's.

**Assumed, not shown:** that the reader's feature definitions use only the rules (reviewed by reading, not
mechanically enforced beyond the source test); that a reader with the same knowledge could be built by a
model (nothing here says any model reads this way); that the 5-fold cross-validation on development streams
estimates the evaluation accuracy well (it did within about 1.5 points); that the dumper's pool, with the
random draw of `m`, resembles what a builder carries (section 7 tests this and finds it holds for the
window, cooccur and neighbourhood builders and fails for the rung).

## 11. What I am least sure of

- **The reader is one program, built in bounded effort.** A better one gives a smaller delta (section 3
  shows that direction on development data), but the interval's width, about 0.017, would still keep the R6
  clause out of reach unless the estimate falls near 0: at this N the criterion cannot return the R6
  regime for a reader whose delta\* is above about 0.004 (delta\*_hi above about 0.012).
- **Whether the exponential is the right object.** The misfit is 0.042 and the curve has a dip and a
  recovery for a reason (the own burst becomes available as m grows), so the penalty the plan writes in
  count of references does not describe this reader's loss. The reader's accuracy loses 0.054 at m = 100,
  and the rerun converts it to a count-based penalty.
- **The look-alike share.** 81% of m = 400's errors are repaired by removing look-alike free-form messages,
  but the counterfactual removes every non-decisive free-form message at once and the reader's features rely
  on free-form messages heavily, so it overstates what a smaller change would do.
- **The direct check's six late anchors** show the reader's assumption that the first alarm is onset is a
  weakness outside this question design.
- **Whether the tuning-selected builder is the right comparator** at deltas this small (section 8: the
  held-out-best builder would pass the upper-bound test where the selected one does not).

## 12. Where things are

- Run outputs (git-ignored), in this worktree: `/home/user/gordian/.claude/worktrees/agent-a15196f7a969278d7/artifacts/runs/`:
  `r9-tune-*`, `r9-heldout-*` (the six runs), `r9/final/` (evaluation questions, usage files),
  `r9/dev/` (development questions, caches, dry-run outputs), `_manifests/`, `_logs/r9-runs.log`.
- Committed: `experiments/exploration/r9-grounded-delta.md` (this file), `r9-*.csv` and `r9-*.json`,
  `r9-dev-history.csv`, `r9-reader-freeze.json`, and `scripts/r9_*.py`, `r9_run.sh`, `r9_reader_weights.json`;
  `analysis/tests/test_r9_scripts.py`; the dumper option in `crates/gordian-stream`.
- Branch `grounded-delta`; the freeze is `cccbcf2` (tag `r9-reader-frozen`).
