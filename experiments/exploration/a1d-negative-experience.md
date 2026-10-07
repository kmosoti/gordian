# A1d, the engram under a non-privileged selector — Lab 1 report

Exploration. A1d's acceptance (`docs/lab-queue.md`, "## A1d", fixed by the chief before the unit)
is deliverables, identity, the test of item 2, the prediction committed before the run, and the
smoke table on decision columns. No claim, no tuning. I did **not** read
`crates/gordian-stream/HIDDEN-DESIGN.md`.

(The report proper is written after the run, below the prediction. The prediction section is
committed before any A1d run and is not edited afterwards.)

## Prediction (committed before any run)

Settings: `experiments/exploration/scripts/a1d_common.py`. Seeds 10000–10019, b = 5, rho = 0.7,
M3's frozen 100 ms medium under every arm; selectors `thr` (B4's public threshold, t = 1 s; the
primary), `chg` (B4's public change rule, k = 12) and `sel` (the selection oracle, labelled
ceiling); forms `m3` (memoryless), `eng_off` (layer, bind off, gate on), `eng_family`, `eng_2site`,
`eng_site`, every engram form with the `stale` gate and A1a's first values. Counts are totals over
the 20 streams; "diff" is arm minus `m3` under the same selector. Ranges are my 80% ranges;
probabilities are mine.

**The model behind the numbers.** Under the public selector about 30 anomalies per stream are
asked about (B4: medium, 30.3 calls per tuning stream), three quarters of them plain, so the family
memory is fed mostly plain outcomes, which the reasoner gives correctly most of the time. Plain
patterns that carry a known kind's signature will reach recall strength within a few streams.
Plain anomalies are contradicted by the checker at some point 97% of the time (R5), and the cheap
rung's declaration on them is usually made on a consistent verdict, so it is stale by the rule of
item 2 and a recall may speak. A recall comes within 10 s of the anchor, before the selector's 16 s
question, and **a recalled anomaly is never asked about** (A1a's rule), whether or not the recall
adds a declaration (most will name the same kind the cheap rung named, and then no declaration is
added). So I expect the memory to act mainly as a call-saver on plain anomalies, and to cost
decisions where the cheap rung was wrong and the question it silences would have corrected it
(the 0.88 → 0.94 plain-accuracy gain public selection buys, B4), partly offset where the recalled
kind is right and the cheap one wrong. Hard incidents: few hard binds, half of them wrong
answers (W2), and "not an incident" engrams from decoys and background resembling hard starts; I
expect no hard gain and a small loss. The memory's errors on the anomalies it recalls are never
contradicted by an answer about those anomalies (charter section 9, inside the arm).

**Primary selector (`thr`), absolute, memoryless `m3`:** plain correct by deadline 405 of about
434 (395–415); hard correct by deadline 25 of about 40 (21–29); calls 600 (520–680).

**Primary selector, family form minus `m3` (the brief's headline row):**

| Column | Point | 80% range | P(direction) |
|---|---|---|---|
| plain `correct_by_deadline` | −8 | −30 to +6 | P(< 0) = 0.65 |
| plain `critical_miss` | +2 | −5 to +10 | P(> 0) = 0.55 |
| hard `correct_by_deadline` | −2 | −7 to +1 | P(< 0) = 0.55 |
| hard `critical_miss` | +1 | −2 to +4 | |
| reasoner calls | −180 (−30%) | −330 to −40 | P(< −10%) = 0.8 |
| `reasoner_cost_ns` | −50 s | −95 s to −10 s | |
| `bill_compute` | +0.4 s | +0.1 s to +2 s | P(> 0) = 0.95 |
| A1c's count, plain unasked wrong | +60 | +15 to +150 | P(> 0) = 0.9 |

**Other rows.**

- `thr` two-site minus `m3`: calls −80 (−250 to 0); plain correct by deadline −4 (−20 to +4);
  hard −1 (−5 to +1). It recalls less than the one-site form. P(some bind refused by the cell
  limit, `bind_refused` > 0) = 0.4; P(the bill refuses the noticer's compute in some stream) = 0.15.
- `thr` site-keyed minus `m3`: calls −10 (−40 to 0); every decision column within ±3.
- `eng_off` minus `m3`: identical decision counts under `thr` (P = 0.9: both arms keep the
  verdicts, since the threshold rule monitors); under `chg` and `sel` identical counts (P = 0.85),
  with differences of microseconds in timing columns only; `bill_compute` higher by the layer and
  the checks.
- `chg`: the same directions as `thr`, sizes within a factor of two of them.
- `sel` (ceiling rows): `sel_m3_privileged` reproduces A1c's kept `sel_m3_privileged` exactly
  except `run_id` (P = 0.97). The stale family form near A1c's gated family form (366 plain correct
  by deadline, 51 calls): plain 362–375, calls 48–58, hard 22–26. Site-keyed equal to `m3`.

**Trace counters, `thr` family form.** Answers about 420 (equal to its calls); by the arm's own
kinds, known 60–80%, not an incident 8–20%, hard 8–20%; bound 75–95% of answers (the rest
elsewhere or unheld). Recalls offered 300–1,500 over 20 streams; admitted 30–70% of offered;
dropped for a standing declaration under 15% of offered; of the admitted, not declared (the same
declaration already made) 30–70%. By the evaluator's class, read after the run: more than 70% of
admitted recalls on plain anomalies. Declared recalls 3–10 s after notice (median), before the
16 s question.

**What would surprise me:** plain correct by deadline higher than `m3`'s by more than 6 (the
memory corrects more than it silences); calls within 10% of `m3`'s (the gate or the rule keeps the
memory quiet under public selection); any hard gain above 2.
