# R8 pilot and decisions, written before the main run

**Status: exploration.** Written by the R8 worker on branch `real-distractor` after the pilot and
before any scored call. Committed before the main run starts, so that the decisions below were
taken without any scored result. Everything about the final prompt is in
`experiments/exploration/scripts/r8_common.py` and `r8-demos.json`.

## 1. Prompt development (not scored), in the pilot seed range

The pilot range is streams 29000 upward (the plan's); the questions are the first hard incidents
(slow leak excluded, pool of at least 400) of that range, so the 10 pilot questions are the first 10
of the 24 used in development. No call here is scored and none is a main-run question. Every round is
listed; nothing is left out. "m = 0" means the decisive evidence alone.

| Round | Model | Prompt | Questions | Correct at m = 0 |
|---|---|---|---|---|
| `r8-pilot-1` (formal pilot, stopped by the worker after 3 calls) | 1.5B | V1: direct answer line, graph block, decimal seconds, rule list | 2 at m = 0, 1 at m = 400 | 0 of 2; 0 of 1 at m = 400; every answer "SlowLeak" |
| dev | 1.5B | V1 | 24 | 2 (0.08); 17 of 24 answers "SlowLeak" |
| dev | 1.5B | V2: adds an evidence line, whole seconds, decision list | 24 | 3 (0.12); 19 of 24 "Compound" |
| dev | 1.5B | V3: evidence names a "second service" | 24 | 2 (0.08) |
| dev | 3B | V3 | 24 (twice) | 1 (0.04) both times |
| dev | 3B | V4: no graph block, message lines tagged `(site)`, `(conn)`, `(far)`, evidence in tags | 24 | 1 (0.04) |
| dev | 3B | V5: 14 worked examples instead of 5 | 12 | 2 (0.17) |
| dev | 3B | V6: evidence as yes/no fields | 12 | 4 (0.33) |
| dev | 3B | V7: tags renamed `connected` and `unconnected` | 12 | 6 (0.50) |
| dev | 3B | V8: V7 plus two emphasising sentences | 12 | 5 (0.42) |
| dev | 1.5B | V7 | 12 | 3 (0.25) |
| `r8-pilot-2` (formal pilot, final prompt V7) | 1.5B | V7 | 10 at m = 0, 4 of them at m = 400 | **3 of 10 (0.30)** at m = 0; 1 of 4 at m = 400; 14 of 14 parsed |

V8 was reverted to V7 (the V8 sentences did not help on 12 questions, 0.42 against 0.50, which is
sampling noise; V7 is shorter). The frozen prompt is V7.

What the development rounds show, reported because they bear on how far the result can be read:

- Without the evidence-line scaffold and the service tags, neither model applied the stated rules
  (the 1.5B collapsed to one kind; the 3B was no better).
- With them, the 3B extracts the evidence correctly (its evidence lines matched the rule reader's in
  10 of 12 questions at V8) and still maps it to a kind wrongly about half the time. It never
  answered Compound correctly on those 12 questions.
- The prompt therefore includes **a decision list and a tagged rendering** that go beyond "the
  public physics and the hard families' rules": the tag `(site)`, `(connected)` or `(unconnected)`
  of a message's service relative to the focus's service is computed from the public graph and is
  the same on evidence and on distractors. It is a reading aid. It is a deviation from a bare
  rendering, listed in the report.

## 2. Pilot measurements (`r8-pilot-2`, final prompt, 1.5B, cores 0-2, 3 threads)

- **Parse rate:** 14 of 14 calls parsed (no failure at m = 0 or m = 400).
- **Accuracy:** m = 0, 3 of 10 (0.30); m = 400, 1 of 4.
- **Tokens per reference** (measured with the server's tokenizer on the context block): 11.0 to 12.0
  at m = 400 (4,429 to 4,827 tokens for 403 to 407 lines); 12 to 14 for the few decisive lines at
  m = 0. The stream declares 20.
- **Throughput:** the shared prefix (system prompt and 14 examples) is 5,183 tokens; the first call
  of a run evaluates it once (73.5 s) and every later call reuses it (`cache_n` 5,183). Prompt
  evaluation falls from about 90 tokens per second for short prompts to 50 at a 9,700-token context.
  An m = 0 call takes 6.3 to 7.4 s (about 2 s of prompt, 46 decoded tokens at about 17 per second); an
  m = 400 call 95 to 109 s.
- **Estimated cost of a question at all levels** (m = 0, 50, 100, 200, 400 and the control), 1.5B:
  about 7 + 16 + 29 + 45 + 96 + 16 = 210 s; the 3B about twice that (not measured at m = 400).

## 3. Decisions

1. **Model.** The plan's rule: if pilot accuracy at m = 0 is under 0.2, switch to the 3B model. The
   pilot with the final prompt gives 0.30, so **the 1.5B model is used** for the main run. The 3B
   model remains the fallback if the identifiability precondition fails (below). The rule applied
   to earlier prompts (V1 to V3, 0.08 to 0.12) would have switched; the prompt changed, and the
   rule is applied to the final prompt's pilot. This is stated, not hidden.
2. **N and the staging.** The plan fixes the model and N from the pilot and requires the whole run
   to fit in 4 hours of inference on cores 0-2. Inference used before the main run, summed from
   server uptimes (benchmarks, development servers, both pilots): about 1 h 20 min (76 min: 3 + 4 + 14 + 40 + 4 + 10 + 3, rounded up for server load times). About 2 h 35 min
   remain. A question costs about 210 s at all levels on the 1.5B and about 420 s on the 3B, so a
   single fixed N cannot serve both models and a possible fallback. The main run is therefore made in
   two stages from one question list, **chosen now**:
   - **Questions:** the first 28 hard incidents (slow leak excluded, pool of at least 400
     observations) of streams 30000 upward, and one plain incident from each of the first 8 streams
     (the one at position `seed mod k` among the stream's plain incidents with a large enough pool).
   - **Stage 1 (identifiability):** the 28 hard questions at m = 0 and at the control. The
     precondition depends on A(0) and the control only, so this is the plan's check, made on the whole
     question set before any other level is run.
   - If it holds for the 1.5B: **stage 2** on the 1.5B: the hard questions at m = 50, 100, 200, 400,
     and the 8 plain questions at every level and the control.
   - If it fails for the 1.5B: stage 1 again on the 3B for the **first 16 of the same 28**, then, if it
     holds, stage 2 on the 3B for those 16 (and the plain questions if time remains). If it also
     fails, R8 stops and reports unidentifiable with these models, and the plain questions are run to
     report their delta as a labelled proxy, with the model of the later stage 1, if time remains.
   - **N** is therefore 28 hard questions per level for the 1.5B, 16 for the 3B, 8 plain.
3. **Control.** q = 0 at m = 50 for every question of the main run, hard and plain.
4. **Server flags.** Flash attention on (`-fa on`) was added after the pilot (llama-bench pp2048:
   98.7 against 86.1 tokens per second). The pilot ran without it; it changes timing and, in the last
   bits, numerics, not the prompt.

## 4. Expected half-width of delta-hat's interval, before the main run

Simulation with `r8_halfwidth.py` (assumed curve p0 + (A0 - p0) exp(-delta m / 100), independent
questions at each level, the analysis's own fit and cluster bootstrap with 500 resamples, 60 draws;
independence overstates the variance of a difference of paired levels, so these are conservative):

| N | A(0) | p0 | assumed delta | median half-width of the 90% interval [10th, 90th percentile of draws] |
|---|---|---|---|---|
| 28 | 0.35 | 0.10 | 0, 0.05, 0.10, 0.20 | 0.31 [0.22, 1.5], 0.34, 0.39, 0.54 |
| 28 | 0.70 | 0.10 | 0, 0.05, 0.10, 0.20 | 0.11 [0.09, 0.14], 0.13, 0.14, 0.18 |
| 18 | 0.50 | 0.10 | 0, 0.05, 0.10, 0.20 | 0.24 [0.16, 0.61], 0.26, 0.30, 0.37 |

The pilot's A(0) is 0.30. **The half-width exceeds 0.05 in every row, including the most favourable
(N = 28, A(0) = 0.70: 0.11 to 0.18). Neither the R6 regime (upper bound below 0.05) nor the R7 regime
(lower bound above 0.10) can be claimed at a favourable point estimate with this N.** In the best
row the share of draws in which the criterion names a regime is 0.18 (R6) at delta 0 and 0.25 (R7) at
delta 0.2. The main run is made anyway, as the plan asks, and reports its outcome as written; "unresolved"
is the expected one. The limit is the cost of a question (about 3.5 minutes at all levels on a 1.5B
model on three cores), not the criterion.

## 5. A reference that is not a model

`r8_reference.py` applies the prompt's own rules with a deterministic program (`r8_rules.py`, no
model) to the same contexts. On the 59 hard questions of streams 30000 to 30039 with a large
enough pool it gets 0.88 at m = 0, 0.51, 0.36, 0.27, 0.25 at m = 50, 100, 200, 400 and 0.05 at
the control; its delta-hat is 0.85 [0.50, 1.36]. A reader with no model and exact rules loses most of its accuracy
as distractors are added, because distractors include free-form messages and alarms that look like
evidence. So the decline of any reader with these contexts is not all distraction in the sense of
the simulated reasoner's law; part of it is information that the context no longer holds.
