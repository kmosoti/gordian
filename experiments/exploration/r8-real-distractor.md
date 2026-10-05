# R8: distractor sensitivity of a real small model

**Status: exploration.** Nothing here tests a hypothesis and nothing here may later be cited as
confirmation. Written by the R8 worker on branch `real-distractor`; the coordinator decides what
follows. The numbers in the tables are copied from the CSVs in this directory (`r8-1p5b-*.csv`,
`r8-stage1-*.json`, `r8-reference-*.csv`, `r8-run-index.csv`); prose numbers were typed by the
worker from them. An estimate for one small model is not a claim about real models in general, and
"not significant" is never "equivalent".

## 1. Result in brief

1. **Outcome as written: unidentifiable with these models.** The identifiability precondition
   (hard incidents: A(0) at least 0.35 and its 90% lower bound above the control's estimate plus
   0.15) fails for both models, on the whole question set of the stage that decides it:

   | Model | hard questions | A(0) [90% bootstrap] | control p0 [90%] | A(0) at least 0.35 | lower bound above p0 + 0.15 |
   |---|---|---|---|---|---|
   | Qwen2.5-1.5B-Instruct Q4_K_M | 28 | 0.500 [0.357, 0.643] (14 of 28) | 0.357 [0.214, 0.500] (10 of 28) | yes | **no** (0.357 against 0.507) |
   | Qwen2.5-3B-Instruct Q4_K_M | 16 (the first 16 of the 28) | 0.250 [0.063, 0.438] (4 of 16) | 0.250 [0.063, 0.438] (4 of 16) | **no** | **no** |

   No regime is claimed. The plan's fallback chain ran to its end (1.5B, then 3B), so R8 stops here as
   the plan says, and reports the plain-incident delta as a labelled proxy (below).
2. **The criterion's own numbers, reported because the plan asks for them and not interpretable.** On
   the 1.5B's 28 hard questions at all levels, delta-hat is 3.0 with a 90% interval of [-0.08, 3.0]:
   the fit sits at the edge of its grid (3.0) and 1,211 of 10,000 resamples sit there too. There is
   nothing to fit: A(m) at m = 50 to 400 is the same as the control (section 3), so the amplitude
   A(0) - p0 is 0.14 [-0.04, 0.32] and delta is not identified. The region is **not** R6, **not** R7;
   under the plan's rule it is "unidentifiable", not "unresolved".
3. **Why the precondition fails is mostly not distractors.** The 1.5B answers "Compound" for 23 of 28
   control questions (no decisive evidence, 50 distractors) and "Cascade" for most others, so its
   control is right 0.36 of the time, about the share of the Compound and some Cascade questions. At
   m = 0 it names Cascade 16 times and Compound 10 times, right for 10 of 12 Cascade, 4 of 9 Compound
   and 0 of 7 SplitBrain questions. It reads the evidence only a little better than it guesses.
   Even on plain incidents, whose evidence is a clear signature, its A(0) is 0.30 (3 of 10; it
   writes an evidence line that says "site free-form: yes" where there is none).
4. **The expected half-width was written before the main run** (`r8-pilot.md`, section 4): above 0.05 in
   every case simulated, 0.11 to 0.18 at best (N = 28, A(0) = 0.7), 0.31 to 0.54 at the pilot's A(0).
   Neither regime could have been claimed at N this small even from a favourable estimate. It is the
   cost of a question (3.5 minutes at all levels on a 1.5B model on three cores), not the criterion.
5. **A program that reads the same rules, with no model, loses accuracy with distractors too** (section
   6): on the same 28 hard questions it gets 0.93 at m = 0 and 0.71, 0.50, 0.43, 0.36 at m = 50 to
   400, delta-hat 0.44 [0.24, 0.88]. Distractors in this world include free-form messages and alarms
   that look like evidence, so a decline of any reader with these contexts is not all "distraction"
   in the sense of the simulated reasoner's law. Whatever delta a model that does read the evidence
   would show would include that part.

## 2. What was run

**Dumper.** `crates/gordian-stream/src/questions.rs` and `examples/questions.rs` (feature
`reveal-hidden-state`, the crate is on the oracle guard's allowlist, a comment names the example).
Per plain or hard (slow leak excluded) incident: the focus (its first observation), all decisive
evidence, the truth, and the pool: every observation within 40 s of the focus that belongs to the
background or to another incident. Seven tests recompute the pool from the labels independently and
fail if the focus incident's own observations enter it (checked by mutation).

**Runtime.** llama.cpp tag `b11429` built from source, `llama-server` over HTTP, a client that uses
the standard library; models Qwen2.5-1.5B and 3B Instruct, Q4_K_M, sha256 in
`scripts/r8-runtime-README.md` (with the requirement, the simpler alternatives considered and the
cost). Greedy decoding (temperature 0, `top_k` 1, seed 1), at most 80 output tokens, prompt caching
on. **Caching was used:** the 5,183-token prefix (system prompt and 14 worked examples) is
evaluated once per server start and reused in every later call (`cache_n` 4,9xx to 5,2xx); the
first call of each run is cold. Every inference process ran under `scripts/cgroup-run.sh` on cores
0-2, 3 threads, 6 GB (peak 1.2 GB for the 1.5B and 2.0 GB for the 3B; no OOM kill), the client on
core 3, and nothing was built while one ran.

**Runs** (all calls recorded; `r8-run-index.csv`):

| Run | Model | What | Calls | Server wall |
|---|---|---|---|---|
| `r8-pilot-1` | 1.5B | first pilot, stopped by the worker | 3 of 20 designed | not reported |
| `r8-pilot-2` | 1.5B | final prompt: 10 questions at m = 0, 4 at m = 400 | 14 | not reported |
| `r8-main-1p5b-s1` | 1.5B | stage 1: 28 hard at m = 0 and control | 56 | 716 s |
| `r8-main-3b-s1` | 3B | stage 1: first 16 hard at m = 0 and control | 32 | 868 s |
| `r8-main-1p5b-plain` | 1.5B | 10 plain, every level and control | 60 | 2,320 s |
| `r8-main-1p5b-s2h` | 1.5B | the same 28 hard at m = 50 to 400 | 112 | 5,631 s |

Scored (main) calls: 260. Parse failures: 0 of 260 (and 0 of the 17 pilot calls). Transport errors
and timeouts: 0. Calls ended by the token cap: 0. Calls excluded: 0. Incidents excluded by the pool
rule (pool under 400): 1 hard incident (met before 28 were chosen) and 14 plain incidents (among the
streams visited for 10 plain questions). `r8-main-1p5b-s2` and `r8-main-3b-s2` were designed (stage 2
as first planned) and never run; the plain run and the hard-levels run replaced them after stage 1
(section 7). Inference wall time: 159 min in the main runs (summed server wall) plus about 77 min
before them (benchmarks, development servers, both pilots, summed from server uptimes), about 3 h 56
min in all, against the 4 hours allowed.

## 3. A(m) of the 1.5B, hard incidents (28 questions, each at every level)

Descriptive: the criterion's precondition failed, so none of this is the criterion's estimate. 90%
cluster bootstrap over incidents (10,000 resamples, seed 9800); `r8-1p5b-levels.csv` also holds a
Wilson interval beside it.

| Level | Correct | A(m) [90%] | Parse failures |
|---|---|---|---|
| 0 | 14 of 28 | 0.500 [0.357, 0.643] | 0 |
| 50 | 10 | 0.357 [0.214, 0.500] | 0 |
| 100 | 11 | 0.393 [0.250, 0.536] | 0 |
| 200 | 12 | 0.429 [0.286, 0.571] | 0 |
| 400 | 10 | 0.357 [0.214, 0.500] | 0 |
| control (q = 0, m = 50) | 10 | 0.357 [0.214, 0.500] | 0 |

- From m = 50 on the accuracy equals the control's; the level 0 point is 0.14 above it with a 90%
  interval of [-0.04, 0.32] for the difference. The data show **no detectable effect of the evidence at
  any level beyond 0, and no decline between 50 and 400**; they do not show that there is none.
- The exponential form: with an amplitude of 0.14 and flat levels it cannot be judged. The fit with a
  free amplitude also goes to the grid's edge (delta 3.0, amplitude 0.14). **The exponential is
  neither supported nor contradicted.** The nonparametric curve is the result.
- By family (`r8-1p5b-family.csv`): Cascade 0.83, 0.42, 0.92, 0.33, 0.42 at m = 0 to 400 (12 questions),
  Compound 0.44, 0.56, 0, 0.33, 0.56 (9), SplitBrain 0, 0, 0, 0.71, 0 (7): the answers move with the
  distractors without a pattern, as one expects when the model's answer is not a function of the
  evidence.
- **Stage 1 detail** (`r8-stage1-detail.csv`): 1.5B at m = 0 answers Cascade 16, Compound 10,
  CredentialExpired 1, SplitBrain 1; at the control Compound 23, Cascade 5. The 3B at m = 0 answers
  Cascade 4, SplitBrain 4, SlowLeak 3, NONE 3, ConfigDrift 1, CredentialExpired 1; right for 2 of 4
  Cascade, 0 of 7 Compound, 2 of 5 SplitBrain.

## 4. Plain incidents (secondary; 10 questions, 1.5B) and the labelled proxy

| Level | Correct of 10 | A(m) [90%] |
|---|---|---|
| 0 | 3 | 0.30 [0.10, 0.50] |
| 50 | 1 | 0.10 [0.00, 0.30] |
| 100 | 0 | 0.00 |
| 200 | 0 | 0.00 |
| 400 | 0 | 0.00 |
| control | 0 | 0.00 |

- **Proxy, labelled:** delta-hat on plain incidents is 2.50 [0.64, 3.0] (edge of the grid in 282 of
  10,000 resamples). It is not a regime claim, and the plain set itself fails the precondition's
  floor (A(0) 0.30). With 10 questions it says only that this model, as prompted, does not read a plain
  signature reliably and loses what it has by m = 100.
- The plain questions are one per stream by the rule `seed mod k`; it happened to pick 7 Intermittent,
  1 CredentialExpired, 1 ResourceExhausted and 1 ConfigDrift incident, so the set is not balanced
  across kinds. Duos were not excluded (none was picked).

## 5. Beside the criterion

- **Parse failures per level:** 0 at every level, both sets, both models (`r8-1p5b-counts.csv`).
- **Tokens per reference** (the server's tokenizer on the context block, 1.5B's calls): 11.3 to 11.5
  for distractor lines, 13.2 per line at m = 0 where lines are the (tagged) decisive messages; the
  stream declares 20 (`r8-1p5b-tokens.csv`). A line is about 57% of the declared cost.
- **Wall time per call** (`r8-1p5b-timing.csv`, 38 calls per level, three of which are cold starts at
  level 0): mean 10.8 s at m = 0 (6.5 s warm), 19.0 s at 50, 27.7 s at 100, 51.1 s at 200, 103.2 s at
  400, 16.0 s at the control. Latency = 3.8 s + 0.0207 s per evaluated prompt token (about 48 tokens
  per second) over the 228 calls; prompt evaluation falls from about 90 tokens per second for short
  prompts to 50 at a 9,800-token context; decoding is about 17 tokens per second and each answer is
  46 tokens (the evidence line is part of it).
- **Accuracy by the position of the first decisive reference** (`r8-1p5b-position.csv`): in this
  design the decisive evidence is 6 to 16 s after the focus and the window is 40 s either side, so
  the first decisive reference sits in the middle third in 102 of the 112 hard calls at m = 50 to 400
  and in the last third in 10; never in the first. Accuracy is 0.40 (41 of 102) in the middle and 0.20
  (2 of 10) in the last third. Position cannot be separated from the number of references here and
  nothing about position is concluded.

## 6. A reference that is not a model: what a program reading the same rules gets

`r8_rules.py` applies the prompt's own rules (the same evidence fields, the same decision list) with a
deterministic program; no model, no hidden state (it reads rendered observations and the public
graph). It is not an arm and not a baseline of any experiment. Same contexts, same cluster
bootstrap, `r8-reference-selected.csv` and `r8-reference-all-hard.csv`:

| Set | N | m = 0 | 50 | 100 | 200 | 400 | control | delta-hat [90%] |
|---|---|---|---|---|---|---|---|---|
| hard, the 28 questions of the run | 28 | 0.93 | 0.71 | 0.50 | 0.43 | 0.36 | 0.07 | 0.44 [0.24, 0.88] |
| hard, all with a large pool, streams 30000 to 30039 | 59 | 0.88 | 0.51 | 0.36 | 0.27 | 0.25 | 0.05 | 0.85 [0.50, 1.37] |
| plain, the 10 questions of the run | 10 | 1.00 | 0.80 | 0.70 | 0.60 | 0.30 | 0.10 | 0.36 [0.16, 0.82] |

- The questions are answerable in principle by reading the stated rules: 0.88 to 0.93 on hard
  incidents at m = 0 (the misses are SplitBrain questions, which the rules cannot tell from a Cascade when the second service is not connected to the site;
  that is an inference from the rules, not checked question by question) and 1.00 on plain.
- With distractors a program loses most of that, because free-form messages from the background use
  the same 48 ids as the hard kinds and sit at every service. The reader here is the naive one the
  prompt states; a better reader (counting messages per service against the background rate) would
  lose less, and the simulated reasoner's law has no such effect at all. So **part of any real
  model's decline with m in this world is information the context no longer holds, not a model
  flaw**. The plan's delta does not separate the two. Nothing here measures that separation.

## 7. Readings and deviations

Everything the plan left open, and everything done differently from it, with the reason. The scripts
carry the readings in their docstrings and were committed before the first scored call
(`8ffb3fa`, and the prompt and decisions in `d32cfc3`).

1. **Regimes off in the question streams** (the dumper's default; `--regimes on` restores them): an
   unannounced change of the public physics would make some questions unanswerable from the stated
   rules, which is a different property from distractor sensitivity. Before the first change the
   streams are identical (tested).
2. **The focus** is the incident's first observation. It is shown in the question, and it is in the
   context only when it is decisive (plain). The incident's own non-decisive observations (first
   moments, heartbeats, closure) are in neither the context nor the pool, as the plan's pool ("from
   background and other incidents") says. The simulated reasoner's `m` counts them as distractors; here
   they are absent.
3. **Nested levels:** one seeded permutation of the pool per question (sha256 of seed, incident and
   observation id); level m takes the first m. This is a draw without replacement and makes the levels
   a paired design. **The control** is the level-50 distractors without the decisive evidence.
4. **Rendering:** time in whole seconds relative to the focus, services `s<N>`, free-form ids as a
   4-hex-digit alias, severity kept, and **messages carry a tag** `(site)`, `(connected)` or
   `(unconnected)` of their service relative to the focus's service, computed from the public graph and
   identical on evidence and distractors. No graph block. The tag is a reading aid added after
   development rounds showed neither model applying the connectivity rules from a list; it goes beyond
   "the public physics and the hard families' rules".
5. **The prompt** states the public physics and the hard families' rules (HIDDEN-DESIGN.md sections 4
   and 4.2, below) and adds **a decision list, an evidence-line scaffold (the reply is two lines, the
   evidence line and the answer line) and 14 worked examples** from streams 28000 upward (no pilot or
   scored question), each checked against the rules' own reader. The plan says "one answer, in a fixed
   format"; here the answer line is the answer and the evidence line precedes it. Output cap 80 tokens.
6. **Pilot:** `r8-pilot-1` was stopped after 3 calls because every answer was the same kind. The
   development rounds (`r8-pilot.md`, section 1) ran on the pilot range at m = 0 only. The formal pilot
   with the final prompt (`r8-pilot-2`) ran 10 questions at m = 0 but only 4 at m = 400, to save about
   10 minutes of the budget. The model rule ("under 0.2 switch") was applied to the final prompt's pilot
   (0.30, keep the 1.5B), not to the earlier prompts' rounds (0.08 to 0.12), and that is stated.
7. **Staging and N** (decided before any scored call, `r8-pilot.md`, section 3): stage 1 on the
   whole question set decides the precondition; stage 2 would run only on a model that passes. N was 28
   hard (1.5B), the first 16 of them (3B), 8 plain. After both failed, the plain questions were run
   first (10, not 8: the extra two come from the next two streams, decided before the run) and then the
   28 hard questions' other levels on the 1.5B, **descriptively** and to fill the plan's table;
   neither changes the outcome. Flash attention was switched on after the pilot.
8. **Analysis readings** (`r8_stats.py`, before the first scored call): the fit's A(0) is the observed
   level-0 share, so delta is the only free parameter; p0 is re-estimated in every resample (the
   variant with p0 held is in `r8-1p5b-fit.csv`: [-0.02, 3.0]); the grid is delta in [-0.5, 3.0], no
   non-negativity constraint; resamples with an amplitude not above 0.02 have no delta and are
   counted (3); the intervals are the 5th and 95th percentiles; the paired analysis uses incidents
   that have every level and the control; an unparsable, failed or timed-out call is wrong and stays
   in the denominator (none occurred).
9. **The plain delta** is reported only as a labelled proxy and its `outcome` field says so.

**Hidden-record sections read:** all of `crates/gordian-stream/HIDDEN-DESIGN.md` (sections 1 to 16),
and the hidden generation code in `incident.rs`, `present.rs`, `stream.rs` and `labels.rs`, to build the
dumper and to state the families' rules in the prompt. The prompt uses sections 4 and 4.2 and the
public physics of the first world (`physics.rs`); no arm was built.

## 8. What was verified by running something, and what was assumed

**Verified by running:** the dumper's pool against an independent recomputation (7 tests, one
mutation caught); the whole gate set on the final tree (`cargo fmt --check`, `cargo clippy --locked
--workspace --all-targets -D warnings`, `cargo test --locked --workspace --no-fail-fast`: 560 tests,
0 failed; the oracle guard; `pytest -W error analysis`: 326 passed, 4 deselected); that the analysis
recovers a known delta from synthetic data (0, 0.05, 0.2 within the interval) and reaches every
outcome branch; that every stored response parses identically on re-parse (the analysis refuses
otherwise); throughput and token counts as above; that the precondition fails for both models.

**Assumed, not shown:** that the server is deterministic for a given prompt and cache state (no
call was replayed; a replay in a different order is not promised to be token-identical); that the
questions are well posed for a model that reads the evidence (a rule reader gets 0.88 to 0.93, so
they are in principle, not that a 3B model could); that the failure is the models' and not the
prompt's (see the next section).

## 9. What I am least sure of

- **Whether "unidentifiable" is a property of these models or of my prompt and rendering.** The 3B got
  6 of 12 right at m = 0 on development questions (V7) and 4 of 16 on the main questions; the
  difference is within sampling error (the standard error at N = 16 is about 0.11) but I cannot rule out
  that the prompt was fitted to the development questions. The plan's rule says the model, not the
  prompt, is what fails the precondition; a better-prompted or larger model could pass it.
- **That the exponential form fits:** not testable here (amplitude 0.14, flat curve). Its use in the
  simulation (R7) is not supported or contradicted.
- **The size of the effect of m on a model that does read the evidence**, which is the question R8 was
  for. This run does not answer it: it shows that two small quantized models do not read this world's
  evidence well enough at m = 0 for a delta to mean anything, and that distractors in this world are
  not irrelevant to even a perfect-rules reader.
- **The plain proxy:** 10 questions, 7 of one kind, A(0) 0.30.
- **Time:** the whole inference budget was used. N was bounded by the cost of a question
  (about 3.5 minutes at all levels on the 1.5B), so even an identifiable model could not have
  produced an interval narrower than about 0.1 in delta.

## 10. Where things are

- Run outputs (git-ignored): `/home/user/gordian/artifacts/runs/r8/r8-*/` (calls, designs, selected
  questions, server and client logs, `usage.json`), copied from this worktree's `artifacts/runs/r8-*`.
- Models: `/home/user/gordian/artifacts/models/qwen2.5-1.5b-instruct-q4_k_m.gguf`,
  `.../qwen2.5-3b-instruct-q4_k_m.gguf`.
- Runtime: `/home/user/gordian/artifacts/runtime/llama.cpp` (tag `b11429`) and `.../build`.
- Scripts: `experiments/exploration/scripts/r8_*.py`, `r8_run.sh`, `r8-runtime-README.md`.
