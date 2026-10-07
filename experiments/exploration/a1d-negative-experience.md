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

---

*Everything below was written after the run.*

## The result in one paragraph

Under the primary public selector (B4's threshold rule, t = 1 s), the family-keyed memory with the
rule of item 2 makes 89 fewer reasoner calls than the memoryless arm in 20 streams (584 → 495,
−15%; reasoner cost −25.7 s, 90% stream-bootstrap interval [−31.1, −19.9] s with `bill_compute`
added) and loses 5 plain and 3 hard decisions by deadline (plain −5 [−9, −1]; hard −3 [−7, 0];
hard critical misses +1). **It gains no decision anywhere**: of 523 incidents, not one is correct
by its deadline under the memory and not under the memoryless arm, under any selector and any form,
except one hard incident under the oracle. The memory, fed mostly plain outcomes, speaks mostly on
plain anomalies, and when it adds a declaration it is almost always wrong (51 of 58 declared plain
recalls add a wrong declaration), because a recall that agrees with the cheap rung adds nothing and
a recall that disagrees with it, on a plain incident, is disagreeing with rules that are usually
right. Every lost decision is an incident the memoryless arm got right after asking, which the
memory left unasked. The two-site form does the same, three to eight times larger (plain −42, hard
−9, calls −238). The site-keyed form barely speaks (5 recalls admitted). This is first values, no
tuning, 20 streams; it is reported, not claimed.

## Commits (branch `negative-experience`)

| Commit | What |
|---|---|
| `b64c24d` | the A1d design in `crates/gordian-medium/DESIGN.md`, before any code |
| `b98492e` | the build: the `stale` gate, the rung's declaration and verdict instants, the trace port's marks and the arm's trace file, the hooks; tests; departures 71–77 |
| `1e91217` | scripts (`a1d_*.py`) and the prediction above, before any run |
| `43061d4` | identity: R6 62 of 62; A1c's 15 and A1a's 8 smoke arms reproduced; the smoke manifest names this revision |
| (this report) | the smoke's tables (`a1d-smoke*.csv`, `a1d-trace*.csv`, `a1d-asked.csv`) and this report |

## Verified by running something

- **Identity.**
  - R6's held-out manifest (b = 5, rho = 0.7) replayed through `scripts/run-driver.sh` with the
    release binary of `b98492e`'s code: all 62 arms have `results.csv` and `incidents.csv`
    byte-identical to `experiments/exploration/r6-results-sha256.csv` (`a1d_gate.py r6`, exit 0,
    `a1d-regression.csv`; recomputed a second time by a separate snippet, 62 of 62). Kept:
    `artifacts/runs/a1d/a1d-xcheck-r6-heldout-b5-rho0.7`.
  - A1c's kept smoke manifest replayed under a new run id (`a1d-a1creplay-b5-rho0.7`): its 15 arms
    equal A1c's kept run and its 8 A1a arms equal A1a's kept run, `incidents.csv` and `results.csv`
    row for row with `run_id` removed (`a1d_gate.py a1c`, exit 0, `a1d-reproduction.csv`).
  - In the smoke itself, `sel_m3_privileged` equals A1c's kept `sel_m3_privileged` (run id aside).
  - The existing identity tests pass unchanged (`m1_identity.rs`, `m1b_identity.rs`, `subtick.rs`,
    the pinned M2/M3/M4 digests in `stream_medium.rs`), as do A1a's and A1c's engram and gate
    tests, unedited.
- **The test of item 2** (`crates/gordian-run/tests/stream_medium_stale.rs`, 7 tests):
  - `memory_corrects_a_cheap_declaration_the_rules_have_since_contradicted`: the cheap rung
    declares on a consistent verdict at 20.5 s, an `AuthFailures` alarm makes the checker empty;
    the stale-gated recall is declared at 23.5 s beside the cheap declaration, at the step A1c's
    gate declares it, and the anomaly is not asked about.
  - `memory_never_adds_to_a_declaration_that_stands`: the contradicting alarm arrives before the
    cheap rung's first review, the shared rule declares by its fallback on evidence the checker has
    never explained; A1c's gate adds the recall at 23.5 s; under the stale gate **every action at
    every step equals the arm without memory's**.
  - the gate's reading; a standing declaration holds a recall for one review period and then
    drops it (counted `gated_standing`); the trace's answers by kind with bind results and
    instants, contradictions, recall made/offered/admitted/declared at one instant; no marks without
    the switch; the file's header and rows.
- **Gates** under `scripts/cgroup-run.sh --name negative-experience-build --cpus 0-2 --memory 3G`
  on `b98492e` (code unchanged since): `cargo fmt --all -- --check` clean; `cargo clippy --workspace
  --all-targets -- -D warnings` clean; `cargo test --workspace` 1008 passed, 0 failed (A1c's 1001
  plus these 7); `scripts/check-no-oracle.sh` ok.
- **The smoke** ran once: `artifacts/runs/a1d/a1d-smoke-b5-rho0.7`, 15 arms, exit 0, 52 s,
  internal/external ratio 1.004, peak 86 MB, no OOM kill; every segment stopped at the horizon,
  no escalation refused, no compute refusal (the largest `bill_compute` in any stream is 620 ms, the
  two-site form's, against the 2 s limit). The 12 trace files each hold 20 `segment_end` rows.
- **Binary and revision.** The release binary was built from `b98492e`; the identity manifests name
  `1e91217` and the smoke manifest `43061d4`; `git diff b98492e 43061d4 -- crates Cargo.toml
  Cargo.lock` is empty (scripts, CSVs and this report only). The driver checks HEAD, not the binary.

**Resource record.** Free disk was at least 7.2 GB at every check (9.6 GB at the start). No run
waited (0 waits in the log). **One error of mine:** the release build (`cargo build --release`,
08:27–08:29) started while another lab's `gordian-run` (pid 29138) was running: my command printed
the `pgrep` result and did not stop on it. The process had exited by 08:29. I do not know whose run
it was or whether it was a measurement; its owner should be told that a 3-core build overlapped
its start. Every later cargo command and run checked first and waited (none needed to).

## The exact rule of item 2 (the `stale` gate, `arms/medium/gate.rs`)

A recall of the engram layer is declared on an anomaly only if, at a step within one review period
of the rung (500 ms) after the recall was resolved:

1. the latest public consistency check of the anomaly found no consistent hypothesis
   (`AnomalyView::contradicted_since` set: A1c's gate), and
2. the anomaly carries no **standing** declaration: with `d` the instant of the latest declaration
   the rung made for it (set wherever `cheap_declared` is set: the cheap rung's, a recogniser's, a
   dismissal) and `c` the instant of its latest consistent verdict (a review's or a check's), a
   declaration stands when `d` exists and (`c` does not exist or `d > c`). Strict: a declaration
   made at the step of a consistent verdict (`d = c`, as every cheap declaration made on a
   consistent review is) is stale once the checker turns empty, and the recall may speak beside it.

Otherwise the recall waits; at the end of the period it is dropped and counted by the last reason
it was read with (`gated_consistent`, `gated_standing`). A recall whose anomaly was asked about,
answered or retired meanwhile is dropped (`overtaken`). Unchanged from A1a: an admitted recall
marks the anomaly recalled, so it is never asked about afterwards, **whether or not a declaration
results** (a recall naming what the cheap rung already declared adds nothing and still silences
the question).

**What the rule did on the smoke.** It dropped 43 recalls under `thr` (7% of those offered), 44
under `chg` and 42 under the oracle. Under the oracle it cut A1c's declaration-level count (plain
unasked wrong 180 under A1c's gate in A1c's run, 148 here) and changed no decision count against
A1c's gated form (plain 366, hard 25 against 24, calls 53 against 51). It did not touch the
mechanism that costs decisions (below), because that mechanism acts on anomalies with no
declaration yet, which the rule admits by construction.

## What the public selector asked about, by the arm's own counts

The arm sees the outcome kind the reasoner named, never the tier. Answers received (trace of the
bind-off control, which asks exactly what the memoryless arm asks), 20 streams:

| Selector | Answers | A known kind | Not an incident | A hard kind | Evaluator's calls by class (background / plain / hard / leak / decoy) |
|---|---|---|---|---|---|
| threshold (primary) | 584 | 484 (83%) | 69 (12%) | 31 (5%) | 28 / 446 / 51 / 7 / 52 |
| change | 504 | 448 (89%) | 34 (7%) | 22 (4%) | 11 / 419 / 35 / 6 / 33 |
| selection oracle | 61 | 23 (38%) | 6 (10%) | 32 (52%) | 0 / 0 / 54 / 7 / 0 |

So the teacher is plain under the public selectors, as the brief intended: 83–89% of what the arm
binds names a known kind. Two things the arm cannot see: under the oracle 38% of the answers it
binds name a known kind although every question was about a hard incident (the reasoner is wrong
about half the time on hard incidents, W2), and under the threshold rule only 31 of 58 calls about
hard incidents and leaks come back naming a hard kind. Of the family form's answers under the
threshold rule, 80% were bound (323 of 408 known, 56 of 60 not-an-incident, 16 of 27 hard); the
rest were "elsewhere" (55) or too short a key (45).

## The smoke table

Seeds 10000–10019 in stream order, b = 5, rho = 0.7, M3's frozen 100 ms medium, run seed 15000.
Counts over 20 streams (434 plain incidents, 40 hard, 49 decoys). "Wrong only": missed with a
wrong declaration. "Unasked wrong": A1c's declaration-level count (a wrong declaration and no
escalation), for continuity. Cost: `reasoner_cost_ns` (the chief's A1c cost column) and
`bill_compute` (the arm's modelled compute, which carries the noticer, the layer and the checks;
`total_cost_ns` omits the noticer's charge, `a1d-smoke.csv` has it).

| Selector | Arm | Plain correct by deadline | Plain missed | Plain critical miss | Plain wrong only | Plain unasked wrong | Hard correct by deadline | Hard critical miss | Calls | Reasoner cost s | bill_compute s |
|---|---|---|---|---|---|---|---|---|---|---|---|
| threshold | `m3` (memoryless) | 393 | 41 | 16 | 5 | 0 | 24 | 10 | 584 | 165.1 | 0.72 |
| threshold | `eng_off` (control) | 393 | 41 | 16 | 5 | 0 | 24 | 10 | 584 | 165.1 | 0.74 |
| threshold | **`eng_family`** | **388** | 46 | 16 | 10 | 46 | **21** | 11 | **495** | 139.3 | 0.88 |
| threshold | `eng_2site` | 351 | 83 | 23 | 46 | 149 | 15 | 11 | 346 | 99.2 | 7.10 |
| threshold | `eng_site` | 392 | 42 | 16 | 6 | 3 | 24 | 10 | 579 | 163.3 | 0.76 |
| change | `m3` | 394 | 40 | 16 | 4 | 1 | 20 | 8 | 504 | 146.4 | 0.70 |
| change | `eng_off` | 394 | 40 | 16 | 4 | 1 | 20 | 8 | 504 | 146.4 | 0.74 |
| change | `eng_family` | 384 | 50 | 17 | 14 | 65 | 16 | 10 | 412 | 119.4 | 0.87 |
| change | `eng_2site` | 358 | 76 | 21 | 39 | 138 | 10 | 9 | 295 | 87.1 | 6.43 |
| change | `eng_site` | 393 | 41 | 16 | 5 | 2 | 19 | 8 | 499 | 144.6 | 0.76 |
| oracle (ceiling) | `m3` | 375 | 59 | 19 | 22 | 53 | 26 | 9 | 61 | 16.4 | 0.69 |
| oracle (ceiling) | `eng_off` | 375 | 59 | 19 | 22 | 53 | 26 | 9 | 61 | 16.4 | 0.76 |
| oracle (ceiling) | `eng_family` | 366 | 68 | 20 | 31 | 148 | 25 | 8 | 53 | 14.1 | 0.83 |
| oracle (ceiling) | `eng_2site` | 367 | 67 | 19 | 30 | 119 | 24 | 9 | 58 | 15.5 | 2.60 |
| oracle (ceiling) | `eng_site` | 375 | 59 | 19 | 22 | 53 | 26 | 9 | 61 | 16.4 | 0.76 |

**Paired against `m3` under the same selector** (sums over 20 streams, 90% interval from 10,000
bootstrap resamples of whole streams; "displaced / gained": incidents correct by deadline in one
arm and not the other; `a1d-smoke-paired.csv`):

| Selector | Arm | Plain correct | Hard correct | Plain crit. | Hard crit. | Calls | Reasoner + bill s | Plain displaced / gained | Hard displaced / gained |
|---|---|---|---|---|---|---|---|---|---|
| threshold | **`eng_family`** | **−5 [−9, −1]** | **−3 [−7, 0]** | 0 | +1 [0, 3] | **−89 [−107, −70]** | −25.5 [−31.1, −19.9] | 5 / 0 | 3 / 0 |
| threshold | `eng_2site` | −42 [−54, −30] | −9 [−14, −4] | +7 [3, 11] | +1 | −238 [−272, −203] | −59.5 | 42 / 0 | 9 / 0 |
| threshold | `eng_site` | −1 [−3, 0] | 0 | 0 | 0 | −5 [−9, −1] | −1.7 | 1 / 0 | 0 / 0 |
| threshold | `eng_off` | 0 | 0 | 0 | 0 | 0 | +0.026 | 0 / 0 | 0 / 0 |
| change | `eng_family` | −10 [−15, −5] | −4 [−10, 0] | +1 | +2 | −92 [−106, −78] | −26.8 | 10 / 0 | 4 / 0 |
| change | `eng_2site` | −36 [−47, −26] | −10 [−16, −5] | +5 | +1 | −209 | −53.6 | 36 / 0 | 10 / 0 |
| change | `eng_site` | −1 | −1 | 0 | 0 | −5 | −1.8 | 1 / 0 | 1 / 0 |
| change | `eng_off` | 0 | 0 | 0 | 0 | 0 | +0.041 | 0 / 0 | 0 / 0 |
| oracle | `eng_family` | −9 [−13, −5] | −1 [−4, 2] | +1 | −1 | −8 [−12, −5] | −2.2 | 9 / 0 | 2 / 1 |
| oracle | `eng_2site` | −8 [−12, −5] | −2 [−4, 0] | 0 | 0 | −3 | +1.0 | 8 / 0 | 2 / 0 |
| oracle | `eng_site` | 0 | 0 | 0 | 0 | 0 | +0.071 | 0 / 0 | 0 / 0 |

Intervals over 20 streams describe these streams' variation, not a population; they are shown so
the chief can see which differences are smaller than stream-to-stream noise, nothing more.

### The table against the prediction

| Predicted | Observed | |
|---|---|---|
| `thr` `m3` plain 405 (395–415), hard 25 (21–29), calls 600 (520–680) | 393, 24, 584 | plain just below my range |
| family `thr`: plain −8 (−30 to +6) | −5 | in range |
| family `thr`: hard −2 (−7 to +1) | −3 | in range |
| family `thr`: plain critical +2 (−5 to +10); hard critical +1 (−2 to +4) | 0; +1 | in range |
| family `thr`: calls −180, −30% (−330 to −40); P(< −10%) = 0.8 | −89, −15% | in range, half the point |
| family `thr`: reasoner cost −50 s (−95 to −10) | −25.7 s | in range |
| family `thr`: `bill_compute` +0.4 s (+0.1 to +2) | +0.16 s | in range |
| family `thr`: A1c's count +60 (+15 to +150) | +46 | in range |
| two-site `thr`: calls −80 (−250 to 0); plain −4 (−20 to +4); hard −1 (−5 to +1); recalls less than one-site | −238; −42; −9; recalls 2.7 times more | **wrong in direction and size** |
| two-site: P(bind refused) = 0.4; P(compute refusal) = 0.15 | neither | |
| site `thr`: calls −10 (−40 to 0), decisions within ±3 | −5, within ±1 | right |
| `eng_off` identical decisions under all three selectors | identical | right |
| `chg` same directions, sizes within a factor two | yes (family plain −10 against −5) | right |
| oracle: `sel_m3` reproduces A1c's exactly; family plain 362–375, calls 48–58, hard 22–26; site = `m3` | yes; 366, 53, 25; yes | right |
| trace: known 60–80%, not an incident 8–20%, hard 8–20% of answers | 82%, 12%, 5% | hard share below |
| bound 75–95% | 80% | right |
| offered 300–1,500; admitted 30–70% of offered | 623; 14% | **admission far lower** (60% overtaken) |
| standing drops under 15% of offered | 7% | right |
| not declared 30–70% of admitted | 21% | below |
| > 70% of admitted on plain anomalies | 84% | right |
| declared 3–10 s after notice | median 9.25 s (plain) | right |
| surprises: plain gain > 6; calls within 10%; hard gain > 2 | none occurred | |

The model behind the prediction was right about who the memory speaks on and what it costs, and
wrong about the two-site form (I expected specificity; it was the most talkative form) and about the
gate's admission (most offered recalls arrive after the 16 s question has been asked).

## What the counters show (trace, `a1d-trace.csv`, `a1d-trace-class.csv`)

Family form under the threshold rule, 20 streams:

- **Binds.** 495 answers; 395 bound (20 created, 12 strengthened, **363 generalised**); 2,812
  contradiction-weakenings. The store ends with 20 engrams (6 after the first stream, 20 from the
  13th on): the plain teacher collapses the family memory into a few generalised engrams whose
  keys are shared across kinds, and the vote weakens them constantly.
- **Recalls.** 840 made; 623 offered (531 on plain anomalies by the evaluator's class, 47 hard, 30
  decoy, 14 background, 1 leak); 371 overtaken (the anomaly had already been asked about: the
  recall came after the 16 s question), 120 dropped on a consistent verdict, 43 on a standing
  declaration, 89 admitted (75 plain, 5 hard, 6 decoy, 3 background). Of the 89, 70 declared and 19
  not (the same declaration already made). Admissions per stream do not grow: 5, 6, 0, 2, 10, 6, 3,
  4, 8, 7, 6, 2, 7, 6, 2, 2, 5, 5, 1, 2.
- **What the declared recalls did** (joined to the evaluator's incidents after the run): of 58
  declared on plain incidents, 51 added a wrong declaration and none added a correct one; of 4 on
  hard incidents, 3 added a wrong one; of 5 on decoys, 4 alarmed. The declared recalls named known
  kinds (tags 1 to 5) in 83 of 89 admissions and "not an incident" in 6; never a hard kind.
- **Timing.** A declared recall on a plain anomaly came a median 9.25 s after the notice (hard
  7.25 s), before the selector's 16 s question. Under the oracle the median is 34.5 s: without
  questions to overtake them, recalls wait for the checker to turn empty much later.
- **How decisions were lost** (incidents correct in `m3`, not in the arm, by their columns): in
  all 5 plain cases the memoryless arm had escalated and was correct with no wrong declaration, and
  the memory arm did not escalate and holds only a wrong declaration (under `chg` the same holds
  for all 10). So on those plain incidents the memory's wrong declaration was made on an anomaly
  with **no declaration yet** (had the cheap rung declared correctly first, the correct declaration
  would be there), and its admission silenced the question. Of the 3 hard cases, 2 are the same
  shape (escalated and correct in `m3`, unasked and wrong in the arm; `m3` also carried one wrong
  declaration) and 1 was escalated in both arms. This is A1a's preemption channel, which the rule of item 2 leaves open by design (no
  declaration, nothing stands). Whether the correct declaration in `m3` came from the cheap rung or
  from the reasoner is not in the run outputs (no ledger sample was kept); inference from counts.

The two-site form makes 6,152 recalls (mostly unmatched or redundant), admits 243, holds 116
engrams at the end and still growing, and costs 0.32 s of modelled compute per stream (620 ms in
its worst stream): about a third of the 2 s compute limit after 20 streams.

## Analysis: what it means and what it does not

**Best current model (inference, from the counts).** The selector change did what the brief
intended for the teacher (the arm now binds plain outcomes and the vote acts: 2,812
contradiction-weakenings against 131 for the same form under the oracle in this run), and it exposed the next structural
fact: **on plain incidents the memory can only be useful where it disagrees with the public rules,
and on plain incidents the public rules are mostly right.** A recall that agrees with the cheap
rung adds no declaration (19 here) but still saves the question; a recall that disagrees is, on
these streams, almost always wrong (51 of 58), and it also saves the question. So the memory buys
calls (−15%) by trading away the reasoner's corrections on the anomalies it speaks on, and the
generalised plain engrams are not discriminating enough to be right where the cheap rung is wrong.
Nothing in 20 streams suggests improvement with experience (admissions flat, the store saturated
at 20 engrams by stream 13).

**What it does not show.**
- It does not show the memory cannot pay for itself: −89 calls for −8 decisions is a cost trade at
  about 3.2 s of reasoner cost per lost decision, and whether that trade is worth having is an
  objective question (charter 1.1: decisions per unit cost) that this smoke does not settle and is
  not meant to.
- It says nothing about held-out streams, other worlds, or tuned parameters (all first values).
- It does not show the rule of item 2 is useless: it held 42–44 recalls per selector off standing
  declarations; it is simply not where the decisions are lost.
- It does not separate "the recall named the wrong kind" from "the right kind at the wrong site"
  (the support site is the anchor's service); the trace has the tag, not the site.

**Rejected readings.**
- "The layer or the checks change decisions": `eng_off` equals `m3` on every decision count under
  all three selectors; only `bill_compute` differs (+26 to +68 ms over 20 streams).
- "A1d changed the arms it is compared with": R6 62 of 62, A1a's 8 and A1c's 15 reproduced, and
  `sel_m3_privileged` equals A1c's.
- "The two-site form is more specific": it recalls 7 times as often as the one-site form and admits
  2.7 times as many; its pair engrams keep being created (116, no refusal) instead of generalising.

**What I would test next (not done, no claim).**
1. Make an admitted recall that agrees with the cheap rung's declaration silence the question (as
   now) and one that disagrees ask it instead (the confirmation policy keyed on disagreement with
   the public rules, a public reading): the disagreeing recalls are the wrong ones here.
2. Keep the selector's question for a recalled anomaly with no declaration yet (the preemption
   channel), and measure what the memory then saves.
3. The family key with generalisation off under the public selector: whether specific keys stop
   colliding across kinds.

## Departures from `DESIGN.md` and the brief, with reasons

- **Departures 71–77** (in `DESIGN.md`): the gate's reading as a value; `bind` returning what it
  did; an answer's mark takes the next step's instant; `not_declared` also when the arm skips a
  recall; the file written by `Drop`, failures counted; the declaration instant is the rung's `now`
  wherever `cheap_declared` is set; B4's constants (tuned on M2's medium) applied unchanged to M3's
  medium.
- **The rule of item 2 is applied on top of A1c's gate**, as a new gate value `stale`. The brief's
  sentence alone ("no declaration made after the checker's last consistent verdict") would let a
  recall add to a declaration whose latest check is consistent; the brief's purpose ("corrects a
  declaration the rules have since contradicted") presupposes the contradiction. Strict `>` is my
  reading of "after" (the equal case is the cheap declaration made on a consistent review, which is
  exactly the one "the rules have since contradicted").
- **The trace file is not inside the arm's directory.** The recorder that writes the arm's
  directory is Lab 2's (and E1 is changing it), and the medium's parameters are `Copy` through Lab
  2's `NoticerSpec`, so a path cannot travel in them. The file is
  `artifacts/runs/a1d/<run>/_trace/engram-trace-<state_key>.csv`, the directory given by the
  environment variable `GORDIAN_ENGRAM_TRACE_DIR` that `a1d_run.py` sets; the manifest maps state
  keys to arms. This is file I/O in the adapter (`arms/medium/trace.rs`), output only; the crate
  has no I/O (the trace port gained a default `mark` method and a `MarkLog`, nothing else).
- **"Binds by outcome tier as the arm sees it"** is recorded as the outcome tag (0 not an incident,
  1–5 known, 6–9 hard), which is the kind; the report groups it by kind.
- **Two selectors, not one.** The brief says "B4's public selector (`public_threshold.rs` and
  `public_change.rs`)"; I ran both, named the threshold rule primary in `a1d_common.py` before the
  run, and report both.
- **A bind-off control under each selector** (`eng_off`), beyond the brief's arms, as A1c had one.
- **Hooks outside my territory, each a no-op for every other arm:**
  - `noticer.rs`: two default `Noticer` methods, `standing_declarations` and `recall_declared`.
  - `rung.rs`: two private fields in `Down` (`declared_at`, `consistent_at`); `record_verdict`
    records the consistent instant; `declared_at` set where `cheap_declared` is; `Rung::standing`;
    `take_gated_recalls` hands the standing ids to the noticer first; a `recall_declared`
    passthrough. `AnomalyView` is unchanged (its literal constructions in Lab 2's tests would
    break).
  - `arms/mod.rs`: `StreamArm::step` reports each recall's fate through `recall_declared` (three
    calls); the declaration is computed before it is pushed, unchanged in effect.

## What I am least sure of

1. **The mechanism of the lost decisions** is inferred from incident columns (memoryless arm
   escalated and correct with no wrong declaration; memory arm unasked with only a wrong one). The
   run kept no ledger sample, so I did not observe the order of cheap declaration, recall and
   question for those incidents.
2. **Why declared plain recalls are wrong** (wrong kind against wrong site, or a generalised
   cross-kind engram winning on strength): the trace has outcome tags, not sites or engram keys.
3. **The overlap of my release build with another lab's run** (above): I cannot say whether it
   disturbed a measurement.
4. **Whether `d = c` should count as stale.** It is the reading that makes "corrects a declaration
   the rules have since contradicted" possible at all, but the chief should confirm it is what was
   meant; under `>=` the rule would block A1c's 118-also-correct channel and the `stale` gate would
   approach "speak only where nothing is declared".
5. **B4's constants on M3's medium**: tuned on M2's medium; applied as instructed, unverified for
   this noticer.

## What the chief should examine most carefully

1. **"Gained 0" across every form and selector** (one hard incident under the oracle aside): the
   memory never produced a decision the memoryless arm lacked. Recompute it from
   `incidents.csv` (`a1d_smoke.py`, displaced/gained columns).
2. **The rule of item 2 is not where the decisions are lost**: the losses are on anomalies with no
   declaration yet, which the rule admits; and a recalled anomaly is never asked about even when the
   recall adds nothing. Both are A1a's rule, unchanged by the brief.
3. **The hooks in `rung.rs` and `arms/mod.rs`** and the claim they are inert (R6 62 of 62 and the
   A1a/A1c reproduction are the evidence).
4. **The trace's I/O in an arm** and its location outside the arm's directory.

## What A1b's specification should bound, clause by clause

1. **Displaced correct decisions, paired, by tier** (the chief's decided first clause): plain and
   hard separately, with critical misses beside; here −5 plain and −3 hard under the primary
   selector, with **gained** decisions reported beside so that a trade (calls for decisions) is
   visible as one.
2. **The cost of the trade**: reasoner cost plus `bill_compute` per lost decision, or decisions per
   unit cost (charter 1.1), against the memoryless arm under the same selector; and a bound on the
   layer's own compute (`bill_compute`), since a two-site store grows by about 6 engrams per stream
   and costs a third of the compute limit after 20 streams.
3. **Questions silenced**: recalled anomalies that are never asked about, split into recalls that
   added a declaration and recalls that added none; the second kind saves calls invisibly to every
   declaration count.
4. **Recall precision against the public rules**: of declared recalls that disagree with a
   declaration already made, how many are right (here 0 of 51 on plain). A1b should not let this be
   folded into general accuracy (charter 1.2).
5. **Experience slope**: admitted and declared recalls, and decisions displaced and gained, per
   stream in stream order, so that "stale errors grow with experience" (charter 1.2) is read
   directly; here flat over 20 streams.
6. **The selector is part of the arm's definition**: name it (threshold with t = 1 s, or the
   call-budgeted selector of B5 when it exists), apply it to every arm including the record rung,
   and report what it asked about by the arm's own kinds beside the evaluator's classes (the arm
   sees 5% hard answers under the threshold rule; 38% of the oracle's answers name known kinds).
7. **The declaration-level count** (A1c's "unasked wrong"): reported, not bounded, as the chief
   decided.

## Not done, and why

- No mutation testing (not in the brief).
- No held-out run (not in the brief).
- No ledger sample in the smoke (`trace_sample_rate` 0, as A1a's and A1c's), so the order of events
  on the lost incidents is inferred, not observed.

## Files

- Crate: `crates/gordian-medium/src/ports.rs` (`Mark`, `MarkLog`, `Trace::mark`), `src/lib.rs`,
  `DESIGN.md` (section A1d, departures 71–77).
- Adapter: `crates/gordian-run/src/stream/arms/medium/gate.rs`, `engram.rs`, `noticing.rs`,
  `trace.rs` (new), `mod.rs`.
- Hooks: `crates/gordian-run/src/stream/arms/noticer.rs`, `rung.rs`, `mod.rs`.
- Tests: `crates/gordian-run/tests/stream_medium_stale.rs`.
- Scripts: `experiments/exploration/scripts/a1d_{common,manifests,run,gate,smoke}.py`.
- Outputs: `experiments/exploration/a1d-regression.csv`, `a1d-reproduction.csv`, `a1d-smoke.csv`,
  `a1d-smoke-paired.csv`, `a1d-trace.csv`, `a1d-trace-class.csv`, `a1d-asked.csv`.
- Kept runs (git-ignored): `artifacts/runs/a1d/a1d-xcheck-r6-heldout-b5-rho0.7`,
  `a1d-a1creplay-b5-rho0.7`, `a1d-smoke-b5-rho0.7` (with `_trace/`), logs in `_logs/`.
