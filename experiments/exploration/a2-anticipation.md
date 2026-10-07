# A2, anticipation of hidden edges — Lab 1 report

Exploration. A2's acceptance (`docs/lab-queue.md`, "## A2", fixed by the chief before the unit) is
deliverables, identity, the PI's prediction committed before the run, and the trace file of
predictions joinable by Lab 3. No claim, no tuning. I did **not** read
`crates/gordian-stream/HIDDEN-DESIGN.md`.

(The report proper is written after the run, below the prediction. The prediction section is
committed before any A2 run and is not edited afterwards.)

## Prediction (committed before any run)

Settings: `experiments/exploration/scripts/a2_common.py` (the same numbers are in `PREDICTION`
there). Seeds 10000–10019 in stream order, b = 5, rho = 0.7, the selection oracle at 16 s with the
rung's context, M3's frozen 100 ms medium under every arm. Arms: `m3` (no layer), `a2_learn` (the
anticipation layer at its first values, attach off, trace on: **the learner**), `a2_raw` (the
learner with `explained` off: every first alarm counts; a labelled control). Numbers are per
stream means over the 20 streams with my 80% ranges; probabilities are mine. "Edges learned" are
distinct ordered pairs `(a, b)` held at one or more reads in a stream (a pair is read at every
trial at `a`); "follow rate" is the public-side share of predictions followed by a first alarm at
the predicted service within the band.

**The model behind the numbers.** The rule counts follows beyond chance (departure 83), so with no
structure in the world a pair cell is a zero-mean walk and an edge is held only by its fluctuations
above 2. How often depends on two quantities I do not know and did not measure: `h`, the rate at
which a quiet service's counted (unexplained) bursts begin, and `n`, the trials per pair per
stream. From W1's published public statistics (an abnormal observation per service every 3.4 s on
average, 37.5% of 2 s cells holding one against 44% for a Poisson process of that rate, so mild
clustering), I put `h` at 0.1 to 0.3 per quiet second, centred at 0.2, and `n` at about 0.6 of
the predicting service's counted first alarms (`h` times its quiet time, roughly 360 s), so 20 to
60, centred at 40. The null model (`scripts/a2_null_model.py`, run before any A2 run, output
below) gives, for the rule as built:

| h, trials per pair | edges per stream (0.4 s, 2 s, 10 s) | predictions per stream (0.4 s, 2 s, 10 s) | follow rate (0.4 s, 2 s, 10 s) |
|---|---|---|---|
| 0.1, 20 | 1.7 (0.05, 1.2, 0.7) | 3.8 (0.1, 2.5, 1.2) | –, 0.34, 0.79 |
| 0.1, 40 | 10.4 (0.6, 5.6, 6.7) | 46 (1.1, 18.6, 26.6) | 0.00, 0.18, 0.65 |
| 0.2, 40 | 13.4 (2.5, 12.3, 0.9) | 63 (6.8, 54.0, 2.5) | 0.09, 0.30, 0.92 |
| 0.2, 60 | 23.3 (5.0, 20.6, 8.2) | 228 (23.4, 170, 34.6) | 0.08, 0.33, 0.84 |
| 0.3, 60 | 26.6 (9.4, 25.2, 0.6) | 328 (48, 279, 0.8) | 0.12, 0.46, 0.88 |

The world is not the null: incidents and background bursts cluster in time across services, which
gives every pair a small positive excess and raises the counts, and the hidden edges add a little
real structure, which W2 measured at about one alarm over an added edge per stream and a cascade
pair recurring once in twelve streams; no rule of this kind learns an edge from one sighting
without also learning noise, so I expect the true edges to be a small share of what is learned.
The 2 s band is where the null model's fluctuations most often cross the threshold (its chance is
near one half, where a zero-mean walk of bounded steps is widest); the 0.4 s band needs about two
follows close together (chance near 0.05); the 10 s band's chance is near 1, its steps toward the
threshold are small, and it crosses rarely.

**The learner (`a2_learn`), per stream:**

| Quantity | Central | 80% range |
|---|---|---|
| ordered pairs with pair cells | 33 | 25–40 |
| edges learned, any band | 12 | 3–28 |
| edges learned, 0.4 s band | 2 | 0–8 |
| edges learned, 2 s band | 10 | 2–25 |
| edges learned, 10 s band | 1 | 0–10 |
| predictions | 60 | 8–350 |
| share of predictions in the 0.4 s / 2 s / 10 s band | 0.12 / 0.80 / 0.04 | 0.03–0.30 / 0.55–0.95 / 0–0.15 |
| public follow rate, 0.4 s band | 0.10 | 0.03–0.30 |
| public follow rate, 2 s band | 0.33 | 0.18–0.50 |
| public follow rate, 10 s band | 0.85 | 0.60–0.97 |
| `bill_compute` added, ms per stream | 2.0 | 1.3–4.0 |

**Probabilities:**

- The follow rate of predictions in each band is within 0.10 of the follow rate of all trials in
  that band (every quiet partner of every counted alarm; the public chance level): **0.75**. That
  is, on the public side the predictions are at chance.
- The follow rate of all trials is at least the mean chance the layer assigned, in each band (the
  world's clustering gives a positive excess the Poisson chance does not include): **0.6**.
- Lab 3 will find fewer than 5% of the learner's predictions on a true hidden edge (cascade
  partner or added edge): **0.8**. (I cannot check this; it is stated so that W3's scoring tests
  it.)
- The learner's decision columns (`incidents.csv`: correct and wrong declarations, correct by
  deadline, missed, critical miss, escalations) equal the memoryless arm's for every incident:
  **0.9**. The timing columns may differ by microseconds (the layer's charges advance logical
  time, as A1c's monitoring did); a difference there is not a change of decision.
- No compute refusal, no truncated tick, every segment to the horizon: **0.95**.

**The control (`a2_raw`, `explained` off):** siblings under a common upstream co-alarm within
0.4 s on every incident there, and with the explanation off those alarms count. More 0.4 s-band
edges than the learner, at least twice as many: **0.7**; a higher 0.4 s-band follow rate than the
learner's: **0.7**; predictions per stream 150 (80%: 20–600).

**What would surprise me** (and what it would mean): a 0.4 s-band follow rate far above the all
trials' rate for the learner (structure the public graph does not explain, which is what the unit
is for); fewer than one edge per stream (the world quieter than W1's numbers suggest, or the
explanation removing most alarms); more than 300 predictions per stream (strong clustering, so the
Poisson chance is badly calibrated, which the second probability tests).
