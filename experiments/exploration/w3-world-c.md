# W3: world C, the phase-2 collision floor, and A2's predictions scored from the hidden side

Status: exploration. Nothing here tests a hypothesis, nothing in W3 reaches an arm, and nothing may
later be cited as a confirmation. Lab 3, unit W3 (`docs/lab-queue.md`), branch `world-c` (based on `main`
at a87aca8). The PI read `crates/gordian-stream/HIDDEN-DESIGN.md` (experimenter-side work) and the
stream crate's source; the report says where a statement comes from that reading and was not
measured. This file is assembled by `experiments/exploration/scripts/w3_assemble.py` from
`w3-world-c.template.md` and the committed `w3-*.csv`; every table below is one of `w3_tables.py`'s.

Every statistic names its **seed range** and its **side**:

- *hidden*: from the stream's hidden truth (`crates/gordian-stream/examples/laws`), a fact about the
  world and not about any arm;
- *run, hidden*: an arm's evaluator files and ledger joined to hidden truth by (seed, incident index);
- *hidden joined to public*: a verdict computed from the hidden side about something public (A2's
  predictions, E1's recalls).

Ranges. **World A** is the defaults: **10000-10099** (tuning-like) and **40000-40199** (held-out), the
ranges of W2 (its hidden tables are reproduced byte for byte, section 1.1). **World C** is the defaults
with the tier mix changed from plain 800 / hard 100 / decoy 100 per mille to **plain 600 / hard 300 /
decoy 100** and nothing else: **60000-60099** (tuning-like) and **70000-70199** (held-out). A2's smoke
is world A, seeds **10000-10019**. Intervals are 90% percentile intervals of a cluster bootstrap
over whole streams (10,000 resamples, seed 9950, W2's constants; A2-side intervals resample the
streams that had predictions).

## 0. Answers

1. **World C's reachable events: 138 against 41** on the held-out range (70000-70199 against
   40000-40199), 3.4 times; 63 against 18 on the tuning-like ranges. "Reachable" is W2's item-6 set `R`:
   a hard incident that repeats an incident the selection oracle's arm declared correctly before the
   repeat's first notice, the most a site-keyed memory fed by that arm could save. Of all hard
   recurrences, 259 against 83 (3.1 times; 1.295 per stream against 0.415); the share of the bill `R` covers
   is 10.0% [8.6, 11.5] against 8.5% [6.4, 10.8], an interval about a third narrower. The family-keyed reach inside a
   stream (`F`) is 412 against 70; carried across streams it is 99.98% of the bill from the eleventh stream on
   (97.8% in world A). The brief expected about 120 reachable events for 40; the measured ratio is 3.4 for `R` and 3.1 for all hard recurrences.
2. **What else changed with the mix** (section 2.2): *Unchanged within the intervals:* incidents per stream
   (26.7), the number of other incidents live during a hard incident (5.1), overlap of incidents (98%
   overlap another; the arrival rate and the incident lifetimes did not change), the background share of the
   delivered stream (0.64 against 0.65), deadlines (critical share 0.32), and the share of hard incidents
   that recur (0.17 against 0.16), and the reasoner's share of the oracle arm's bill (0.9998). *Changed:* (a) the
   **family mix of the hard tier** (cascade 25.8% to 18.4%, slow leak 22.3% to 28.4%, compound 22.9% to
   26.8%), which I attribute to the generator's fall-through to the next family when a cascade has no free
   unconnected pair (read from the code, not measured per arrival); so hard cascades per stream rose 2.1 times, not 3;
   (b) the **decoy-to-hard ratio**, 1:1 to 1:2.7, which is a consequence of keeping decoys at 100 per mille
   and changes any collision count that mixes the two; (c) the share of hard incidents the oracle arm
   notices fell from 0.857 to 0.815 and is a **family-mix effect** (the slow leak is noticed 43-47% of the
   time in both worlds, per family the shares are unchanged); (d) the **share of streams with a hard incident altered
   by the signature shift** doubled (0.19 to 0.40), by the added edge 0.105 to 0.195; (e) arrivals skipped
   because no service was free rose from 0.34 to 0.57 per stream (intervals [0.24, 0.45] and [0.45, 0.70]
   barely meet). The oracle arm's **missed deadlines** (a correct declaration after the deadline) are 2.8% of
   hard incidents against 3.1% [section 2.4].
3. **The phase-2 collision floor** (section 3). Hidden side, from each incident's own observations (the best case for a key), a recall wrong when the recalled truth is not the target's, every source right; seeds 40000-40199 (A) and 70000-70199 (C), hard
teacher, sources usable after their answer. (a) **The simulator reproduces W2's phase-1 table exactly** in all four ranges (32 of 32 cells). (b) **Waiting for the rule-breaking evidence removes the plain collisions and leaves
the decoys.** Family-keyed, stream reset, key `K2` (kinds, the evidence outside the site, the public checker's verdict): 0.606 [0.547, 0.662] of recalls wrong at 6 s, **0.289 [0.224, 0.354] at 16 s** (A; 0.23 wrong per stream);
0.455 and **0.166 [0.140, 0.193]** (C; 0.60 per stream); wrong recalls on plain incidents 116 to 9 (A) and 308 to 14 (C); what remains is slow-leak decoys (28 of 46 in A) and contradict-mode decoys, which no abnormal
evidence separates from their hard family until they resolve (the benign streak that shows a decoy has resolved is there for 39% of decoys 16 s after the first alarm and 78% at 24 s). With the decoy's own evidence (five benign readings, `K4`) 0.114 [0.058, 0.175] at 16 s (A), 0.088 (C), 0.061 and 0.018 at 32 s.
(c) **Site-keyed, stream reset: 0.087 [0.040, 0.143] at 16 s (A), 0.054 (C)**, 71 of the 83 hard recurrences recalled right. (d) **Carried across streams, family-keyed: 0.43 to 0.87 (A), 0.28 to 0.66 (C)** under every key tried (E1's keys: 0.61 to 0.64 in A, 0.33 to 0.34 in C), unchanged by aggregation: never below 0.20. (e) **Against E1's held-out collisions:** the floor for E1's site-keyed `kinds` key with the reset is **0.115 [0.000, 0.233]** (3 of 26) against E1's **0.11** (1 of 9): E1 sits on
the floor. The floor for its family-keyed `timing` key carried across streams is **0.637 [0.582, 0.688]** against E1's **0.81**: the world accounts for 0.64, and the rest is evidence from outside the incident (328 of E1's
372 plain recalls were on plain incidents whose own evidence the public checker finds consistent). World C's floors for the same two keys are 0.125 and 0.344.
4. **A2's predictions scored from the hidden side** (section 4). On the 251 predictions of the learner
   (seeds 10000-10019): **21 (8.4%) name a pair that is a true hidden edge** (a cascade partner or an added edge,
   in the direction predicted) against 7.1% for a uniform draw among the services the learner could have named;
   the permutation distribution puts 17.7% of draws at or above 21 (no evidence of precision above chance).
   **None of the 251 is an event hit**: not one was made on an alarm that belongs to an incident for which the pair
   is the true partner relation and followed by that incident's partner alarm. **Coverage of true partner
   alarms is 0 of 29** (0 of 10 for those a trial was licensed on); the funnel is 29, 25 within the widest band,
   17 whose site alarm the learner counted (the explanation filter and "not a first alarm" cost 4 and 4),
   10 with the partner quiet, 5 with an edge held for the pair at some read, 0 predicted on the alarm. The
   filter costs 4 of 25 coverable alarms (16%); the licence of a quiet partner costs 7 of 17 more. Learned edges
   (distinct, per band): 7 of 127 on a true pair (5.5% against 6.7% chance). The control arm (`a2_raw`) is no
   better (49 of 775 predictions, 14 of 302 edges, 0 of 29 covered). Hidden-side follow rate 60 of 251 = A2's own
   reading, 60 of 251.
5. **Which bounds move** (section 5): (i) **The unasked-correct count** 12 of 83 (lower bound above 0.075) becomes **28 of 259**; at 0.15 the lower bound is 0.114 (was 0.081). (ii) **The paired margin's half-width** falls from 0.063 to **0.036**; the "record rung
captures the lever" threshold from 0.27 to **0.32**. (iii) **The cost clause's interval** (R, 8.5% [6.4, 10.8]) becomes **10.0% [8.6, 11.5]**: +/-2.2 to +/-1.5 points. (iv) **The collision clause**: the floor is
0.09 (site-keyed) and 0.29 (family-keyed) at 16 s in A, 0.05 and 0.17 in C, below 0.20 only for the site-keyed key and for the family-keyed key that uses the decoy's resolution; across streams no key tried reaches 0.20.
The per-stream count (0.25) is a statement about the mix: it is 9.8 per 100 hard incidents in A and 0.74 per stream in C. (v) **A2b**: the precondition is not met on the smoke (precision at chance), and world C
gives no power for it (events with two earlier sightings of the pair 0.065 per stream against 0.070).
6. **Least sure of** (section 7): the floor is computed from each incident's own evidence and so is a floor of
   the world, not a prediction of what an arm reaches (E1's actual recalls on plain incidents are three times the
   floor's: section 3.4); the choice of the mix (decoys fixed) is mine and changes the hard-versus-decoy ratio; the
   A2 verdict rests on 29 true partner alarms in 20 streams.

## 1. What was measured, what was assumed, and what was decided

### 1.1 Verified by running something

- **The example is unchanged where W2 used it.** `laws` was extended (world C, `--floor`, `--alarms`); its
  five default outputs for the 300 world-A streams (10000-10099, 40000-40199) are **byte-identical** to
  W2's (`cmp` on incidents, regimes, streams and vocab of both ranges, and the same for the first 20 and 5
  streams under the new flags). The rebuilt-incident check of `laws` (every incident equal to the stream's own)
  held on all four ranges of this unit (the example exits non-zero otherwise).
- **W2's functions reproduce W2's numbers.** `w3_laws.py` and `w3_ceiling.py` call W2's functions on world A
  and the rows equal W2's committed CSVs (`recurrence-summary`, `family-elsewhere`, `regime-effects`, `stale`,
  `recurrence-eligibility`, `ceiling`, `ceiling-bill`, `ceiling-unasked`, `ceiling-propagation`; checked by `equals`).
  The world-C rows are the same code on the world-C tables and runs.
- **World C's runs**: the oracle arm `sel_reanchor_privileged` (the selection oracle at 16 s, the rung's
  context, B2's re-anchor noticer, b = 5, rho = 0.7), 100 and 200 streams, under `scripts/run-driver.sh`
  and the cgroup runner, exit 0 each, the ledger kept (`events-sample.jsonl`, trace rate 1), the manifest
  differing from W2's only in the run id, the seeds, the experiment name and `stream_params.mix` (the
  manifest script asserts it). Calls: ledger and `results.csv` agree per stream, calls per incident equal
  `incidents.csv`'s `escalations`, the evaluator's tier and family equal the hidden tables' (W2's join checks,
  which stop the script).
- **The floor's simulator** reproduces W2's published phase-1 collision table exactly on both world-A ranges
  (`w3-floor-check-w2.csv`, 32 of 32 cells equal over the four ranges, world C included) and has 11 unit tests (`w3_floor_test.py`). The hidden-side key
  features agree with E1's own recalls where E1 recalled from a source in the same stream (11 of 11 for the
  site-keyed forms, 8 of 9 to 22 of 23 for the family forms: section 3.4).
- **The scoring script** has 8 tests on a synthetic stream with answers derived by hand (`w3_score_test.py`) and
  caught 14 of 14 hand-made mutants of itself (window bounds, relation direction, owner join, quiet condition,
  counted-alarm condition and others).
- **Determinism**: every table is a pure function of the hidden tables, the runs and the inputs named; reruns are
  byte-identical: after a second run of `w3_laws.py`, `w3_ceiling.sh`, `w3_bounds.py`, `w3_pairs.py`, `w3_e1join.py` and `w3_score.py` (both arms) 31 of 31 CSVs are `cmp`-equal to the first; `w3_floor.py`'s CSVs (a 20-minute run) were
  compared the same way (section 8).
- Gates, process record: section 8.

### 1.2 Read from the code or the design record and not measured

- That a hard family falls through to the next in declaration order when it is infeasible, and so that
  a threefold hard share lowers the cascade share (the family mix of section 2.2 is measured; the cause is read from
  `stream.rs`).
- The phase-1 classes of W2 (a mimic's burst is a plain burst; a decoy presents as its family): W2 read them
  from the code; section 3.5 tests them against data-derived classes.
- The anomaly attach rule of the rung (an observation at the site or at a dependent of the site belongs to the
  site's anomaly; an unconnected service starts its own): read from `noticer::attach_target`; it decides which
  evidence reaches E1's consistency gate and key.

### 1.3 Decisions and where the brief was underspecified

1. **The mix.** The tier mix has two free numbers (plain, hard; decoys take the rest) and hard 300 needs plain
   to give way. "Nothing else changed" is read as: hard 100 -> 300, decoys held at 100, plain 800 -> 600. The
   alternative (decoys scaled with plain) would have kept the hard:decoy ratio near 1:1 and is not run; the report
   states what the choice changes (section 2.2, 3).
2. **"Reachable events"** is W2's `R` (the arm-conditional site-keyed reach), the quantity A1b's power rests on;
   `R_world` (every hard recurrence) and `F` are reported beside it.
3. **Item 2's "key that includes the evidence classes that arrive after the first phase"** has no single
   definition in the brief. I built the key from the incident's own public observations (best case: no other
   incident's or background symptom in them) with the invariant features E1 and A1c built, read at a ladder of
   cutoffs after the first abnormal observation, and with E1's own snapshot rule (section 3.1). "The rule-breaking
   evidence as the public rules define it" is the public checker's verdict (`consistent_hypotheses`) on the
   evidence attached to the anomaly; the evidence classes of phase 2 enter through the kinds at services outside
   the site's dependents, the verdict, the bands, A1c's partner relation and, as a labelled extension, the benign
   streak that is the decoy's own evidence (the public decoy rule).
4. **Sources of the memory**: two teachers, never merged: hard incidents and decoys with their true labels (W2's
   section 9 reading: what a perfect teacher would leave) and hard incidents only (what the selection oracle
   asks: its table holds no plain incident and no decoy). The stored answer is always the truth of the source (a
   right source); the reasoner's own error is W2's section 7.6 and is not here.
5. **Two availability readings**: a source is usable by every later-arriving incident (W2) or only after its
   answer would arrive (19 s after the source's first alarm: 16 s selection delay, about 2 s of latency, about 1 s of
   notice) and no earlier than the target's key is read. An incident never recalls its own answer.
6. **A2's trace**: scored from `/home/user/gordian/artifacts/runs/a2/a2-smoke-b5-rho0.7/` (the chief's copy,
   read only): `predictions-<arm>.csv`, `first-alarms-<arm>.csv`, `edges-<arm>.csv`, for the learner and the control.
   Brief item 4 asked for the trace file; Lab 1's joined files carry the same columns.
7. **No hidden-side accessor was added to `src/`.** The additions are the `laws` example's (`floor.rs`, world C, two
   flags), all behind `reveal-hidden-state`; `scripts/check-no-oracle.sh` passes.

## 2. Item 1: world C (hidden; run, hidden)

### 2.1 Recurrence, same family elsewhere, regime changes (W2's items 1, 2, 5)

| seeds (hidden) | group | incidents | per stream | recurrences | recurrences per stream [90%] | share of incidents [90%] |
|---|---|---|---|---|---|---|
| 10000-10099 | all | 2643 | 26.43 | 550 | 5.500 [5.080, 5.930] | 0.208 [0.195, 0.222] |
| 10000-10099 | tier=plain | 2110 | 21.10 | 437 | 4.370 [3.970, 4.770] | 0.207 [0.192, 0.223] |
| 10000-10099 | tier=hard | 243 | 2.43 | 44 | 0.440 [0.300, 0.590] | 0.181 [0.138, 0.222] |
| 10000-10099 | tier=decoy | 290 | 2.90 | 69 | 0.690 [0.510, 0.880] | 0.238 [0.191, 0.283] |
| 10000-10099 | hard/compound | 72 | 0.72 | 16 | 0.160 [0.070, 0.260] | 0.222 [0.122, 0.302] |
| 10000-10099 | hard/cascade | 63 | 0.63 | 13 | 0.130 [0.060, 0.210] | 0.206 [0.119, 0.288] |
| 10000-10099 | hard/split_brain | 64 | 0.64 | 10 | 0.100 [0.050, 0.160] | 0.156 [0.088, 0.219] |
| 10000-10099 | hard/slow_leak | 44 | 0.44 | 5 | 0.050 [0.010, 0.100] | 0.114 [0.030, 0.191] |
| 40000-40199 | all | 5344 | 26.72 | 1095 | 5.475 [5.210, 5.740] | 0.205 [0.196, 0.214] |
| 40000-40199 | tier=plain | 4304 | 21.52 | 883 | 4.415 [4.160, 4.670] | 0.205 [0.195, 0.216] |
| 40000-40199 | tier=hard | 512 | 2.56 | 83 | 0.415 [0.340, 0.495] | 0.162 [0.138, 0.186] |
| 40000-40199 | tier=decoy | 528 | 2.64 | 129 | 0.645 [0.535, 0.765] | 0.244 [0.212, 0.275] |
| 40000-40199 | hard/compound | 117 | 0.58 | 21 | 0.105 [0.065, 0.145] | 0.179 [0.124, 0.233] |
| 40000-40199 | hard/cascade | 132 | 0.66 | 15 | 0.075 [0.040, 0.115] | 0.114 [0.065, 0.164] |
| 40000-40199 | hard/split_brain | 149 | 0.74 | 26 | 0.130 [0.090, 0.175] | 0.174 [0.129, 0.219] |
| 40000-40199 | hard/slow_leak | 114 | 0.57 | 21 | 0.105 [0.065, 0.150] | 0.184 [0.126, 0.240] |
| 60000-60099 | all | 2581 | 25.81 | 511 | 5.110 [4.720, 5.510] | 0.198 [0.184, 0.212] |
| 60000-60099 | tier=plain | 1519 | 15.19 | 312 | 3.120 [2.770, 3.480] | 0.205 [0.187, 0.224] |
| 60000-60099 | tier=hard | 786 | 7.86 | 145 | 1.450 [1.220, 1.690] | 0.184 [0.160, 0.209] |
| 60000-60099 | tier=decoy | 276 | 2.76 | 54 | 0.540 [0.380, 0.710] | 0.196 [0.149, 0.241] |
| 60000-60099 | hard/compound | 191 | 1.91 | 36 | 0.360 [0.240, 0.480] | 0.189 [0.139, 0.236] |
| 60000-60099 | hard/cascade | 152 | 1.52 | 16 | 0.160 [0.090, 0.230] | 0.105 [0.066, 0.146] |
| 60000-60099 | hard/split_brain | 221 | 2.21 | 39 | 0.390 [0.260, 0.530] | 0.176 [0.129, 0.223] |
| 60000-60099 | hard/slow_leak | 222 | 2.22 | 54 | 0.540 [0.380, 0.720] | 0.243 [0.195, 0.289] |
| 70000-70199 | all | 5341 | 26.70 | 1056 | 5.280 [5.010, 5.550] | 0.198 [0.189, 0.207] |
| 70000-70199 | tier=plain | 3273 | 16.36 | 674 | 3.370 [3.135, 3.610] | 0.206 [0.194, 0.218] |
| 70000-70199 | tier=hard | 1516 | 7.58 | 259 | 1.295 [1.150, 1.440] | 0.171 [0.155, 0.186] |
| 70000-70199 | tier=decoy | 552 | 2.76 | 123 | 0.615 [0.505, 0.725] | 0.223 [0.192, 0.252] |
| 70000-70199 | hard/compound | 407 | 2.04 | 84 | 0.420 [0.330, 0.515] | 0.206 [0.171, 0.241] |
| 70000-70199 | hard/cascade | 279 | 1.40 | 41 | 0.205 [0.150, 0.265] | 0.147 [0.112, 0.181] |
| 70000-70199 | hard/split_brain | 399 | 2.00 | 61 | 0.305 [0.240, 0.375] | 0.153 [0.125, 0.180] |
| 70000-70199 | hard/slow_leak | 431 | 2.15 | 73 | 0.365 [0.285, 0.450] | 0.169 [0.140, 0.197] |

Hard incidents by family (the share in brackets is of the range's hard incidents):

| seeds (hidden) | hard incidents | compound (share) | cascade | split brain | slow leak |
|---|---|---|---|---|---|
| A 10000-10099 | 243 | 72 (0.296) | 63 (0.259) | 64 (0.263) | 44 (0.181) |
| A 40000-40199 | 512 | 117 (0.229) | 132 (0.258) | 149 (0.291) | 114 (0.223) |
| C 60000-60099 | 786 | 191 (0.243) | 152 (0.193) | 221 (0.281) | 222 (0.282) |
| C 70000-70199 | 1516 | 407 (0.268) | 279 (0.184) | 399 (0.263) | 431 (0.284) |

Same family at another site (W2's item 2: a hard incident, not a recurrence, whose family and mode occurred earlier in the stream at another site):

| seeds (hidden) | key | hard incidents | recurrences | non-recurrence hard | same family at another site | per stream [90%] | share of non-recurrence [90%] | family-keyed reach by experience, share of hard [90%] |
|---|---|---|---|---|---|---|---|---|
| 10000-10099 | family and mode | 243 | 44 | 199 | 33 | 0.330 [0.220, 0.440] | 0.166 [0.122, 0.208] | 0.317 [0.261, 0.368] |
| 10000-10099 | strict (+ kinds) | 243 | 44 | 199 | 20 | 0.200 [0.110, 0.300] | 0.101 [0.060, 0.141] | 0.263 [0.209, 0.312] |
| 40000-40199 | family and mode | 512 | 83 | 429 | 51 | 0.255 [0.200, 0.315] | 0.119 [0.095, 0.143] | 0.262 [0.233, 0.290] |
| 40000-40199 | strict (+ kinds) | 512 | 83 | 429 | 37 | 0.185 [0.135, 0.235] | 0.086 [0.065, 0.108] | 0.234 [0.206, 0.262] |
| 60000-60099 | family and mode | 786 | 145 | 641 | 225 | 2.250 [1.980, 2.520] | 0.351 [0.323, 0.377] | 0.471 [0.442, 0.498] |
| 60000-60099 | strict (+ kinds) | 786 | 145 | 641 | 167 | 1.670 [1.420, 1.920] | 0.261 [0.231, 0.290] | 0.397 [0.365, 0.428] |
| 70000-70199 | family and mode | 1516 | 259 | 1257 | 435 | 2.175 [1.995, 2.355] | 0.346 [0.327, 0.365] | 0.458 [0.439, 0.475] |
| 70000-70199 | strict (+ kinds) | 1516 | 259 | 1257 | 328 | 1.640 [1.475, 1.805] | 0.261 [0.240, 0.281] | 0.387 [0.366, 0.407] |

Regime changes (W2's item 5):

| seeds (hidden) | hard after the first change | [90%] | hard after the last change | streams with a hard incident altered by the shift | by the added edge |
|---|---|---|---|---|---|
| 10000-10099 | 0.6214 | [0.5730, 0.6716] | 0.2634 | 0.1800 | 0.0700 |
| 40000-40199 | 0.6172 | [0.5846, 0.6496] | 0.2461 | 0.1900 | 0.1050 |
| 60000-60099 | 0.6514 | [0.6238, 0.6796] | 0.2519 | 0.4200 | 0.2500 |
| 70000-70199 | 0.6240 | [0.6060, 0.6424] | 0.2731 | 0.4000 | 0.1950 |

| seeds (hidden) | change | hard incidents after it, per stream | hard incidents altered | per stream [90%] | share of those after it [90%] |
|---|---|---|---|---|---|
| 10000-10099 | signature_shift | 1.51 | 23 | 0.230 [0.140, 0.320] | 0.152 [0.100, 0.209] |
| 10000-10099 | edge_add | 0.64 | 7 | 0.070 [0.030, 0.110] | 0.109 [0.049, 0.179] |
| 40000-40199 | signature_shift | 1.58 | 47 | 0.235 [0.175, 0.295] | 0.149 [0.114, 0.185] |
| 40000-40199 | edge_add | 0.63 | 22 | 0.110 [0.075, 0.150] | 0.175 [0.120, 0.233] |
| 60000-60099 | signature_shift | 5.12 | 67 | 0.670 [0.510, 0.850] | 0.131 [0.100, 0.164] |
| 60000-60099 | edge_add | 1.98 | 28 | 0.280 [0.200, 0.370] | 0.141 [0.102, 0.182] |
| 70000-70199 | signature_shift | 4.73 | 123 | 0.615 [0.505, 0.725] | 0.130 [0.108, 0.152] |
| 70000-70199 | edge_add | 2.07 | 45 | 0.225 [0.170, 0.280] | 0.109 [0.083, 0.136] |

Stale by construction (W2's definition: a recurrence altered by a change that took effect after its template began):

| seeds (hidden) | hard recurrences | whose template began before the first change they straddle | stale by construction | share [90%] |
|---|---|---|---|---|
| 10000-10099 | 44 | 20 | 4 | 0.091 [0.025, 0.170] |
| 40000-40199 | 83 | 48 | 12 | 0.145 [0.084, 0.211] |
| 60000-60099 | 145 | 73 | 14 | 0.097 [0.061, 0.134] |
| 70000-70199 | 259 | 134 | 30 | 0.116 [0.080, 0.154] |

Reading. Recurrence behaves the same per tier (the share of hard incidents that recur is 0.16 to 0.18 in all four
ranges), so the number of hard recurrences scales with the number of hard incidents (1.295 per stream against 0.415, 3.1 times).
The same-family-elsewhere count scales faster, 8.5 times (435 against 51), because it counts pairs of hard incidents
in a stream and the hard incidents are three times as many: family-keyed reach by experience inside a stream
is 45.8% [43.9, 47.5] of hard incidents against 26.2% [23.3, 29.0]. After the first change fall 62.4% of the hard incidents
in world C (61.7% in A): the schedule is unchanged. Streams with a hard incident altered by the signature shift go from 19% to
40%, by the added edge from 10.5% to 19.5%; the stale-by-construction hard recurrences are 30 of 259 (11.6% [8.0, 15.4]) against
12 of 83 (14.5%): 30 events against 12 for the recovery measure the charter asks (EXP-104), still too few for a per-stream time.

### 2.2 What else changed with the mix (hidden)

| measure (hidden; value [90% cluster interval]) | A 10000-10099 | A 40000-40199 | C 60000-60099 | C 70000-70199 |
|---|---|---|---|---|
| incidents per stream | 26.43 [25.63, 27.22] | 26.72 [26.16, 27.28] | 25.81 [24.90, 26.69] | 26.70 [26.13, 27.27] |
| plain incidents per stream | 21.10 [20.34, 21.84] | 21.52 [20.99, 22.05] | 15.19 [14.42, 15.95] | 16.36 [15.85, 16.88] |
| plain share of incidents | 0.798 [0.780, 0.816] | 0.805 [0.795, 0.815] | 0.589 [0.569, 0.608] | 0.613 [0.600, 0.625] |
| hard incidents per stream | 2.430 [2.100, 2.780] | 2.560 [2.365, 2.755] | 7.860 [7.370, 8.350] | 7.580 [7.265, 7.895] |
| hard share of incidents | 0.092 [0.080, 0.105] | 0.096 [0.089, 0.103] | 0.304 [0.288, 0.322] | 0.284 [0.273, 0.294] |
| decoy incidents per stream | 2.900 [2.530, 3.280] | 2.640 [2.420, 2.865] | 2.760 [2.430, 3.110] | 2.760 [2.520, 3.005] |
| decoy share of incidents | 0.110 [0.096, 0.123] | 0.099 [0.091, 0.107] | 0.107 [0.094, 0.120] | 0.103 [0.095, 0.112] |
| recurrences per stream | 5.500 [5.080, 5.930] | 5.475 [5.210, 5.740] | 5.110 [4.720, 5.510] | 5.280 [5.010, 5.550] |
| hard recurrences per stream | 0.440 [0.300, 0.590] | 0.415 [0.340, 0.495] | 1.450 [1.220, 1.690] | 1.295 [1.150, 1.440] |
| share of hard incidents that recur | 0.181 [0.138, 0.222] | 0.162 [0.138, 0.186] | 0.184 [0.160, 0.209] | 0.171 [0.155, 0.186] |
| skipped arrivals per stream (no service free) | 0.370 [0.210, 0.550] | 0.340 [0.240, 0.450] | 0.480 [0.280, 0.700] | 0.565 [0.445, 0.695] |
| share of arrivals skipped | 0.014 [0.008, 0.021] | 0.013 [0.009, 0.017] | 0.018 [0.011, 0.026] | 0.021 [0.016, 0.025] |
| incidents overlapping another incident's live interval, per stream | 26.00 [25.17, 26.81] | 26.25 [25.66, 26.84] | 25.25 [24.26, 26.21] | 26.34 [25.73, 26.93] |
| share of incidents overlapping another incident | 0.984 [0.979, 0.988] | 0.982 [0.979, 0.986] | 0.978 [0.972, 0.984] | 0.986 [0.984, 0.989] |
| hard incidents overlapping another incident, per stream | 2.410 [2.080, 2.760] | 2.525 [2.330, 2.720] | 7.770 [7.270, 8.270] | 7.525 [7.210, 7.835] |
| share of hard incidents overlapping another incident | 0.992 [0.981, 1.000] | 0.986 [0.978, 0.994] | 0.989 [0.981, 0.995] | 0.993 [0.989, 0.996] |
| other incidents live during a hard incident, mean per hard incident | 5.119 [4.833, 5.408] | 5.043 [4.818, 5.269] | 5.109 [4.882, 5.335] | 5.098 [4.951, 5.242] |
| incident-live seconds per stream (sum over incidents of live_end - onset) | 1280.51 [1239.56, 1320.36] | 1311.85 [1281.79, 1341.67] | 1326.53 [1275.45, 1377.07] | 1366.58 [1335.63, 1397.15] |
| mean live seconds per incident | 48.45 [47.78, 49.13] | 49.10 [48.63, 49.55] | 51.40 [50.65, 52.12] | 51.17 [50.69, 51.66] |
| mean busy seconds per incident (busy_until - onset) | 71.45 [70.78, 72.13] | 72.10 [71.63, 72.55] | 74.40 [73.65, 75.12] | 74.17 [73.69, 74.66] |
| observations per stream | 4471.70 [4424.50, 4517.40] | 4496.60 [4461.20, 4532.10] | 4506.70 [4447.40, 4565.20] | 4565.00 [4528.20, 4601.40] |
| background share of observations (not labelled with any incident) | 0.653 [0.646, 0.660] | 0.649 [0.644, 0.654] | 0.646 [0.637, 0.654] | 0.640 [0.634, 0.645] |
| incident-labelled observations per stream | 1551.10 [1503.10, 1597.70] | 1579.10 [1544.30, 1614.20] | 1596.40 [1537.20, 1654.90] | 1645.30 [1608.80, 1681.20] |
| free-form messages per stream | 910.00 [905.30, 914.70] | 909.10 [905.80, 912.50] | 934.50 [928.80, 940.10] | 929.80 [926.30, 933.30] |
| critical share of hard incidents | 0.292 [0.237, 0.346] | 0.326 [0.286, 0.367] | 0.316 [0.282, 0.351] | 0.319 [0.295, 0.343] |
| critical share of plain incidents | 0.166 [0.148, 0.185] | 0.158 [0.146, 0.170] | 0.158 [0.140, 0.176] | 0.152 [0.139, 0.165] |
| hard cascades per stream (a hidden partner) | 0.630 [0.470, 0.800] | 0.660 [0.560, 0.760] | 1.520 [1.310, 1.740] | 1.395 [1.240, 1.550] |
| decoy cascades per stream (partner alarms in phase 1 when they contradict early) | 0.590 [0.440, 0.750] | 0.545 [0.445, 0.655] | 0.600 [0.470, 0.750] | 0.585 [0.485, 0.685] |
| incidents altered by the added edge, per stream | 0.990 [0.830, 1.150] | 0.970 [0.870, 1.070] | 1.170 [1.000, 1.350] | 0.955 [0.855, 1.060] |
| incidents exposed to the added edge (a service newly downstream of their site), per stream | 1.090 [0.920, 1.270] | 1.090 [0.985, 1.200] | 1.380 [1.170, 1.610] | 1.135 [1.025, 1.245] |
| arrivals with an eligible template, per stream | 21.55 [20.75, 22.33] | 21.75 [21.23, 22.27] | 20.85 [20.07, 21.62] | 21.38 [20.86, 21.90] |
| share of hard incidents after the last regime change | 0.263 [0.223, 0.304] | 0.246 [0.218, 0.274] | 0.252 [0.226, 0.278] | 0.273 [0.255, 0.292] |

Reading. Nothing that sets the stream's clutter changed: incidents per stream, the mean number of other incidents live
during a hard incident (5.1 in all four ranges), the share of incidents that overlap another one (98%: with one arrival every 20 s and
incidents live for 50 s, nearly every incident overlaps another in every world here), the delivered stream's size and the share of it
that belongs to no incident (0.64 against 0.65). What changed is the composition: plain incidents fell from 21.5 to 16.4 per stream, hard rose
from 2.56 to 7.58, decoys stayed at 2.7; and, **because the generator takes the next family when a cascade or split brain is infeasible**,
the cascade share of the hard tier fell (and the leak's rose), so hard cascades (the incidents that carry a hidden partner) rose 2.1 times, not 3
(1.395 per stream against 0.66). The added edge alters about one incident per stream in both worlds (0.955 against 0.97): the mix does nothing for
the added-edge evidence A2 needs.

### 2.3 The ceiling (W2's item 6) on world C (run, hidden)

The oracle arm's bill and what it did:

| seeds (run) | streams | total bill, modelled s | reasoner share | calls | on hard | on plain or decoy | on background | hard incidents | noticed | asked | declared correctly | correct by the deadline |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 10000-10099 | 100 | 67.3 | 0.99929 | 220 | 220 | 0 | 0 | 243 | 213 | 211 | 131 | 124 |
| 40000-40199 | 200 | 138.0 | 0.99932 | 459 | 459 | 0 | 0 | 512 | 439 | 435 | 247 | 231 |
| 60000-60099 | 100 | 203.5 | 0.99977 | 671 | 671 | 0 | 0 | 786 | 644 | 642 | 363 | 340 |
| 70000-70199 | 200 | 391.7 | 0.99976 | 1288 | 1288 | 0 | 0 | 1516 | 1236 | 1228 | 759 | 716 |

Site-keyed and family-keyed reach (W2's sets; `R` is the item-6 answer):

| seeds (run, hidden) | set | hard incidents | per stream | streams with at least one | calls | modelled s | share of the total bill [90%] |
|---|---|---|---|---|---|---|---|
| 10000-10099 | all hard incidents | 243 | 2.430 | 83 | 220 | 67.3 | 0.999 [0.999, 0.999] |
| 10000-10099 | R | 18 | 0.180 | 15 | 19 | 6.1 | 0.090 [0.057, 0.125] |
| 10000-10099 | R_chain | 22 | 0.220 | 18 | 22 | 7.0 | 0.104 [0.070, 0.138] |
| 10000-10099 | R_fresh | 16 | 0.160 | 14 | 17 | 5.4 | 0.081 [0.049, 0.114] |
| 10000-10099 | F_only | 24 | 0.240 | 16 | 25 | 8.0 | 0.119 [0.074, 0.166] |
| 10000-10099 | F | 42 | 0.420 | 26 | 44 | 14.1 | 0.209 [0.156, 0.260] |
| 10000-10099 | F_strict | 30 | 0.300 | 22 | 31 | 9.8 | 0.146 [0.104, 0.188] |
| 10000-10099 | R_world | 44 | 0.440 | 27 | 44 | 13.7 | 0.204 [0.152, 0.253] |
| 10000-10099 | F_world | 78 | 0.780 | 35 | 75 | 23.4 | 0.347 [0.282, 0.406] |
| 40000-40199 | all hard incidents | 512 | 2.560 | 175 | 459 | 137.9 | 0.999 [0.999, 0.999] |
| 40000-40199 | R | 41 | 0.205 | 36 | 39 | 11.7 | 0.085 [0.064, 0.108] |
| 40000-40199 | R_chain | 44 | 0.220 | 37 | 42 | 12.6 | 0.091 [0.069, 0.115] |
| 40000-40199 | R_fresh | 31 | 0.155 | 29 | 29 | 8.6 | 0.063 [0.045, 0.081] |
| 40000-40199 | F_only | 29 | 0.145 | 25 | 22 | 6.4 | 0.047 [0.031, 0.063] |
| 40000-40199 | F | 70 | 0.350 | 48 | 61 | 18.1 | 0.132 [0.103, 0.161] |
| 40000-40199 | F_strict | 62 | 0.310 | 44 | 53 | 15.8 | 0.115 [0.088, 0.143] |
| 40000-40199 | R_world | 83 | 0.415 | 66 | 75 | 22.6 | 0.164 [0.136, 0.193] |
| 40000-40199 | F_world | 139 | 0.695 | 90 | 122 | 36.5 | 0.265 [0.232, 0.298] |
| 60000-60099 | all hard incidents | 786 | 7.860 | 100 | 671 | 203.4 | 1.000 [1.000, 1.000] |
| 60000-60099 | R | 63 | 0.630 | 41 | 57 | 17.8 | 0.087 [0.068, 0.107] |
| 60000-60099 | R_chain | 71 | 0.710 | 45 | 63 | 19.6 | 0.096 [0.076, 0.117] |
| 60000-60099 | R_fresh | 53 | 0.530 | 36 | 45 | 14.0 | 0.069 [0.051, 0.086] |
| 60000-60099 | F_only | 146 | 1.460 | 62 | 129 | 39.8 | 0.195 [0.167, 0.223] |
| 60000-60099 | F | 209 | 2.090 | 73 | 186 | 57.6 | 0.283 [0.249, 0.317] |
| 60000-60099 | F_strict | 167 | 1.670 | 67 | 140 | 42.6 | 0.209 [0.178, 0.241] |
| 60000-60099 | R_world | 145 | 1.450 | 66 | 119 | 36.0 | 0.177 [0.150, 0.204] |
| 60000-60099 | F_world | 382 | 3.820 | 92 | 306 | 93.5 | 0.460 [0.429, 0.488] |
| 70000-70199 | all hard incidents | 1516 | 7.580 | 199 | 1288 | 391.6 | 1.000 [1.000, 1.000] |
| 70000-70199 | R | 138 | 0.690 | 91 | 127 | 39.3 | 0.100 [0.086, 0.115] |
| 70000-70199 | R_chain | 159 | 0.795 | 94 | 148 | 45.5 | 0.116 [0.100, 0.133] |
| 70000-70199 | R_fresh | 115 | 0.575 | 82 | 104 | 31.9 | 0.082 [0.069, 0.095] |
| 70000-70199 | F_only | 274 | 1.370 | 140 | 238 | 72.9 | 0.186 [0.167, 0.205] |
| 70000-70199 | F | 412 | 2.060 | 155 | 365 | 112.3 | 0.287 [0.263, 0.310] |
| 70000-70199 | F_strict | 326 | 1.630 | 134 | 273 | 83.5 | 0.213 [0.192, 0.235] |
| 70000-70199 | R_world | 259 | 1.295 | 134 | 220 | 67.6 | 0.173 [0.154, 0.191] |
| 70000-70199 | F_world | 711 | 3.555 | 188 | 566 | 173.5 | 0.443 [0.422, 0.463] |

Family-keyed memory carried across streams, from the eleventh stream on:

| seeds (run, hidden) | set [streams 11 onward] | hard incidents | share of the total bill [90%] |
|---|---|---|---|
| 10010-10099 | all hard incidents [streams 11 onward] | 216 | 0.999 [0.999, 0.999] |
| 10010-10099 | R (site-keyed, within the stream) | 15 | 0.086 [0.051, 0.122] |
| 10010-10099 | F (family-keyed, within the stream) | 38 | 0.216 [0.159, 0.271] |
| 10010-10099 | F_cross (family-keyed, memory carried across streams in seed order) | 210 | 0.967 [0.934, 0.993] |
| 10010-10099 | F_strict_cross (as F_cross with the same kinds) | 199 | 0.912 [0.861, 0.957] |
| 40010-40199 | all hard incidents [streams 11 onward] | 494 | 0.999 [0.999, 0.999] |
| 40010-40199 | R (site-keyed, within the stream) | 39 | 0.083 [0.062, 0.106] |
| 40010-40199 | F (family-keyed, within the stream) | 68 | 0.131 [0.102, 0.160] |
| 40010-40199 | F_cross (family-keyed, memory carried across streams in seed order) | 486 | 0.978 [0.962, 0.991] |
| 40010-40199 | F_strict_cross (as F_cross with the same kinds) | 469 | 0.939 [0.915, 0.962] |
| 60010-60099 | all hard incidents [streams 11 onward] | 692 | 1.000 [1.000, 1.000] |
| 60010-60099 | R (site-keyed, within the stream) | 53 | 0.082 [0.062, 0.102] |
| 60010-60099 | F (family-keyed, within the stream) | 180 | 0.278 [0.241, 0.314] |
| 60010-60099 | F_cross (family-keyed, memory carried across streams in seed order) | 692 | 1.000 [1.000, 1.000] |
| 60010-60099 | F_strict_cross (as F_cross with the same kinds) | 683 | 0.983 [0.973, 0.992] |
| 70010-70199 | all hard incidents [streams 11 onward] | 1447 | 1.000 [1.000, 1.000] |
| 70010-70199 | R (site-keyed, within the stream) | 132 | 0.100 [0.084, 0.115] |
| 70010-70199 | F (family-keyed, within the stream) | 394 | 0.287 [0.262, 0.311] |
| 70010-70199 | F_cross (family-keyed, memory carried across streams in seed order) | 1447 | 1.000 [1.000, 1.000] |
| 70010-70199 | F_strict_cross (as F_cross with the same kinds) | 1435 | 0.988 [0.982, 0.994] |

`R` is the item-6 answer for a site-keyed memory: 138 hard incidents on the held-out range (127 calls, 39.3 x 10^9 modelled ns),
10.0% [8.6, 11.5] of the total bill; tuning-like 63 incidents, 8.75% [6.8, 10.7]. World A held-out: 41 incidents, 8.5% [6.4, 10.8]. The point
estimates are close and the interval narrows from +/-2.2 to +/-1.5 points. `R` is 53% of the hard recurrences in world C (49% in A):
the arm declares about half the hard incidents correctly in both worlds (0.50 and 0.48). The family-keyed form inside a stream reaches 28.7%
[26.3, 31.0] of the bill (13.2% in A), and carried across streams it reaches all of it (99.98%): in world C every family and mode has been seen
and declared correctly by the eleventh stream, so the carried ceiling is 1 in practice, and any difference between key forms is a difference in
what the key can recognise, not in what there was to recognise.

### 2.4 The oracle arm on the extra hard incidents (run, hidden)

| seeds (run, hidden) | tier | set | incidents | noticed [90%] | declared correctly by the deadline [90%] | correct but late (missed) | share missed [90%] |
|---|---|---|---|---|---|---|---|
| 10000-10099 | hard | all | 243 | 0.876 [0.839, 0.911] | 0.510 [0.451, 0.572] | 7 | 0.029 [0.007, 0.056] |
| 10000-10099 | hard | critical | 71 | 0.944 [0.893, 0.985] | 0.451 [0.350, 0.558] | 7 | 0.099 [0.023, 0.187] |
| 10000-10099 | plain | all | 2110 | 0.946 [0.938, 0.954] | 0.870 [0.855, 0.884] | 3 | 0.001 [0.001, 0.003] |
| 10000-10099 | plain | critical | 351 | 0.943 [0.923, 0.961] | 0.889 [0.859, 0.916] | 3 | 0.009 [0.003, 0.016] |
| 40000-40199 | hard | all | 512 | 0.857 [0.830, 0.884] | 0.451 [0.412, 0.489] | 16 | 0.031 [0.019, 0.045] |
| 40000-40199 | hard | critical | 167 | 0.820 [0.767, 0.871] | 0.311 [0.246, 0.380] | 16 | 0.096 [0.059, 0.137] |
| 40000-40199 | plain | all | 4304 | 0.948 [0.941, 0.954] | 0.858 [0.846, 0.869] | 4 | 0.001 [0.000, 0.002] |
| 40000-40199 | plain | critical | 679 | 0.947 [0.932, 0.961] | 0.876 [0.853, 0.898] | 3 | 0.004 [0.001, 0.009] |
| 60000-60099 | hard | all | 786 | 0.819 [0.791, 0.846] | 0.433 [0.401, 0.465] | 23 | 0.029 [0.018, 0.041] |
| 60000-60099 | hard | critical | 248 | 0.790 [0.732, 0.846] | 0.359 [0.305, 0.415] | 23 | 0.093 [0.060, 0.128] |
| 60000-60099 | plain | all | 1519 | 0.941 [0.931, 0.952] | 0.846 [0.826, 0.866] | 1 | 0.001 [0.000, 0.002] |
| 60000-60099 | plain | critical | 240 | 0.933 [0.900, 0.963] | 0.854 [0.814, 0.894] | 1 | 0.004 [0.000, 0.012] |
| 70000-70199 | hard | all | 1516 | 0.815 [0.798, 0.832] | 0.472 [0.450, 0.495] | 43 | 0.028 [0.022, 0.035] |
| 70000-70199 | hard | critical | 483 | 0.803 [0.769, 0.837] | 0.429 [0.389, 0.468] | 43 | 0.089 [0.069, 0.110] |
| 70000-70199 | plain | all | 3273 | 0.940 [0.933, 0.946] | 0.852 [0.841, 0.864] | 5 | 0.002 [0.001, 0.003] |
| 70000-70199 | plain | critical | 497 | 0.921 [0.901, 0.941] | 0.805 [0.774, 0.834] | 2 | 0.004 [0.000, 0.009] |

Reading. The share of hard incidents declared correctly by the deadline (0.472 against 0.451), the missed ones (correct after the deadline,
2.8% against 3.1%) and the critical misses are the same within the intervals; the oracle arm asks every hard incident it notices
(1288 calls, on 1228 of the 1516 hard incidents), none on a plain incident or a decoy, and the reasoner is 99.98% of its bill, as in world A. The hard
incidents noticed are 81.5% against 85.7%, a **family-mix effect**: per family the shares are cascade 0.986 / 0.985, compound 0.966 / 0.940,
split brain 0.965 / 0.980, slow leak 0.425 / 0.465 (world C held-out / world A held-out; `w3-ceiling-hard-incidents.csv`).

What the same run says about unasked-correct declarations with no memory:

| seeds (run, hidden) | tier | incidents | n | declared correctly with no call | share [90%] |
|---|---|---|---|---|---|
| 10000-10099 | plain | all incidents | 2110 | 1838 | 0.871 [0.857, 0.886] |
| 10000-10099 | plain | recurrences | 437 | 376 | 0.860 [0.824, 0.895] |
| 10000-10099 | hard | all incidents | 243 | 0 | 0.000 [0.000, 0.000] |
| 10000-10099 | hard | recurrences | 44 | 0 | 0.000 [0.000, 0.000] |
| 40000-40199 | plain | all incidents | 4304 | 3695 | 0.859 [0.847, 0.870] |
| 40000-40199 | plain | recurrences | 883 | 755 | 0.855 [0.831, 0.878] |
| 40000-40199 | hard | all incidents | 512 | 0 | 0.000 [0.000, 0.000] |
| 40000-40199 | hard | recurrences | 83 | 0 | 0.000 [0.000, 0.000] |
| 60000-60099 | plain | all incidents | 1519 | 1286 | 0.847 [0.827, 0.867] |
| 60000-60099 | plain | recurrences | 312 | 262 | 0.840 [0.803, 0.876] |
| 60000-60099 | hard | all incidents | 786 | 0 | 0.000 [0.000, 0.000] |
| 60000-60099 | hard | recurrences | 145 | 0 | 0.000 [0.000, 0.000] |
| 70000-70199 | plain | all incidents | 3273 | 2795 | 0.854 [0.842, 0.866] |
| 70000-70199 | plain | recurrences | 674 | 565 | 0.838 [0.813, 0.863] |
| 70000-70199 | hard | all incidents | 1516 | 0 | 0.000 [0.000, 0.000] |
| 70000-70199 | hard | recurrences | 259 | 0 | 0.000 [0.000, 0.000] |

Inherited error (the answer a memory fed by this reasoner would store is wrong), W2 section 7.6:

| seeds (run, hidden) | source of the recalled answer | hard incidents with an answered source | right | wrong | mixed | wrong share [90%] |
|---|---|---|---|---|---|---|
| 10000-10099 | site-keyed (the template) | 39 | 18 | 21 | 0 | 0.538 [0.385, 0.680] |
| 10000-10099 | family-keyed (most recent same family and mode) | 64 | 36 | 28 | 0 | 0.438 [0.343, 0.530] |
| 40000-40199 | site-keyed (the template) | 68 | 40 | 27 | 1 | 0.397 [0.281, 0.508] |
| 40000-40199 | family-keyed (most recent same family and mode) | 115 | 61 | 51 | 3 | 0.444 [0.354, 0.534] |
| 60000-60099 | site-keyed (the template) | 116 | 60 | 53 | 3 | 0.457 [0.368, 0.546] |
| 60000-60099 | family-keyed (most recent same family and mode) | 300 | 150 | 139 | 11 | 0.463 [0.403, 0.523] |
| 70000-70199 | site-keyed (the template) | 215 | 134 | 77 | 4 | 0.358 [0.302, 0.413] |
| 70000-70199 | family-keyed (most recent same family and mode) | 558 | 336 | 212 | 10 | 0.380 [0.343, 0.418] |

## 3. Item 2: the phase-2 collision floor (hidden side)

### 3.1 What is computed

W2's section 9 asked what a key read in the first seconds would recall wrongly if it fired on every match, with each source taken to be a *right* earlier diagnosis. This is
the same question for keys that wait for the evidence of phase 2. For every incident (seeds 10000-10099, 40000-40199, 60000-60099, 70000-70199) the `laws` example
(`--floor`, `crates/gordian-stream/examples/laws/floor.rs`) takes **the incident's own observations** (public bytes; which ones belong to the incident is the hidden label, so
no other incident's and no background symptom is in them: the best case for any key, a floor and not a prediction) and writes the invariant features at cutoffs 1 to
32 s after its first abnormal observation. "Abnormal" is the rung's own predicate (`rung::is_abnormal`). Two evidence sets:

- **attached**: the abnormal observations at the site and at its dependents in the time-zero public graph. This is what the rung's anomaly holds for a site
  (`noticer::attach_target`; read, section 1.2): a service the public graph does not connect to the site starts an anomaly of its own, so a cascade's partner and a split brain's
  peer are *not* in the evidence the rung's consistency checker sees. E1's gate and key are built from this set.
- **all**: every abnormal observation of the incident, partner and peer included.

The public rules' verdict is `gordian_world::physics::consistent_hypotheses` on the evidence; "rule-breaking" is an empty set (the first world's word for contradictory). The **gate**
is the first prefix whose verdict is empty. The keys:

| Key | Features (all invariant: no service id, no message id) |
|---|---|
| `K1` | the abnormal tags at the site (counters at or above `HIGH`, catalogue messages other than `CheckHealth`, a changed configuration snapshot) and the anchor's tags: E1's `kinds` without the site id |
| `K2` | `K1`, the tags at services outside the site and its dependents (a partner's, a peer's, the added edge's new dependent), how many such services (up to 3), and the public checker's verdict on all the evidence |
| `K3` | `K2`, the highest band of each abnormal counter at the site (E1's `bands`) and A1c's partner relation (the unconnected service whose first alarm is nearest the site's, within 10 s: before or after, in the bands 0.4 s, 2 s, 10 s, and its tags) |
| `K4` | `K3` and whether the site's last five counter readings are all benign: the public decoy rule (labelled an extension: it is the *decoy's* evidence, not rule-breaking evidence) |
| E1 `kinds`, `bands`, `timing` | `key_of` of `noticer_record.rs`: the site's tags and the anchor's; the bands; the band of the gate delay (under 1, 3, 6, 10 s, or more); read at the **snapshot**, one second after the gate opens on the attached evidence; only incidents with a gate have a key |

Site-keyed forms add the site's id and so cannot cross a stream boundary; family-keyed forms are the invariant ones. The memory is a table from a key to the truth of the most recent source bound
there. A source binds when its answer would arrive (19 s after its first alarm: 16 s of selection delay, about 2 s of latency, about 1 s of notice) and is not available to its own recall; a
lookup happens at the cutoff. "Reset" empties the table at each stream; "carried" keeps it across the streams of the range in seed order, starting empty at the first. A recall is *wrong* when the
recalled truth is not the target's: a plain incident, a decoy, or another hard family. The collision share is wrong recalls over recalls. The primary teacher is hard incidents only (what the
selection oracle asks, so what E1's table holds); the W2 teacher (hard incidents and decoys, an earlier reading) is in `w3-floor-ladder.csv`.

**Which cutoffs are realisable.** The oracle arm calls the reasoner 16 s after the notice; a recall that saves the call must come before it. Cutoffs of 16 s and later measure what a key could
know *after* the arm has already asked, so they are the floor of a key's information and not a saving. The cost of waiting (anchor-relative; the slow leak's anchor is its first threshold
crossing, about 10 s after onset, so its deadline is 10 s nearer in anchor time):

| cutoff, s after the first alarm | A: all hard | A: critical hard | C: all hard | C: critical hard |
|---|---|---|---|---|
| 4 | 0.000 | 0.000 | 0.000 | 0.000 |
| 6 | 0.000 | 0.000 | 0.000 | 0.000 |
| 10 | 0.002 | 0.006 | 0.005 | 0.017 |
| 16 | 0.027 | 0.084 | 0.030 | 0.093 |
| 20 | 0.051 | 0.156 | 0.052 | 0.164 |
| 24 | 0.139 | 0.425 | 0.109 | 0.342 |
| 32 | 0.256 | 0.784 | 0.242 | 0.727 |

### 3.2 When the public checker finds the evidence contradictory (the gate)

| group (hidden) | A 40000-40199: gate on the attached evidence | gate on all own evidence | C 70000-70199: attached | all own |
|---|---|---|---|---|
| plain | 323/4304 = 0.075, median 0.01 s | 0.106, median 0.01 s | 272/3273 = 0.083, median 0.01 s | 0.117, median 0.01 s |
| plain, altered by the signature shift | 323/490 = 0.659, median 0.01 s | 0.678, median 0.01 s | 272/372 = 0.731, median 0.01 s | 0.745, median 0.01 s |
| plain, altered by the added edge | 16/158 = 0.101, median 0.01 s | 0.937, median 0.02 s | 16/135 = 0.118, median 0.01 s | 0.933, median 0.02 s |
| decoy | 123/528 = 0.233, median 0.01 s | 0.409, median 0.03 s | 107/552 = 0.194, median 0.01 s | 0.391, median 0.05 s |
| hard | 179/512 = 0.350, median 0.07 s | 0.773, median 0.14 s | 559/1516 = 0.369, median 0.12 s | 0.711, median 0.13 s |
| hard/compound/mimic | 55/56 = 0.982, median 10.85 s | 0.982, median 10.46 s | 208/212 = 0.981, median 10.42 s | 0.981, median 10.23 s |
| hard/compound/contradict | 60/61 = 0.984, median 0.01 s | 0.984, median 0.01 s | 192/195 = 0.985, median 0.01 s | 0.985, median 0.01 s |
| hard/cascade/mimic | 8/59 = 0.136, median 0.01 s | 1.000, median 10.89 s | 10/135 = 0.074, median 0.01 s | 1.000, median 10.17 s |
| hard/cascade/contradict | 11/73 = 0.151, median 0.02 s | 1.000, median 0.06 s | 10/144 = 0.069, median 0.01 s | 1.000, median 0.08 s |
| hard/split_brain/mimic | 22/86 = 0.256, median 10.57 s | 1.000, median 10.53 s | 68/185 = 0.368, median 10.80 s | 1.000, median 10.61 s |
| hard/split_brain/contradict | 23/63 = 0.365, median 0.08 s | 1.000, median 0.09 s | 71/214 = 0.332, median 0.10 s | 1.000, median 0.09 s |
| hard/slow_leak | 0/114 = 0.000 | 0.000 | 0/431 = 0.000 | 0.000 |
| decoy/compound | 87/140 = 0.621, median 0.01 s | 0.643, median 0.01 s | 67/145 = 0.462, median 0.01 s | 0.503, median 0.01 s |
| decoy/cascade | 8/109 = 0.073, median 0.01 s | 0.459, median 0.08 s | 13/117 = 0.111, median 0.01 s | 0.573, median 0.07 s |
| decoy/split_brain | 28/131 = 0.214, median 0.08 s | 0.580, median 0.08 s | 27/141 = 0.192, median 0.10 s | 0.539, median 0.09 s |
| decoy/slow_leak | 0/148 = 0.000 | 0.000 | 0/149 = 0.000 | 0.000 |

Reading. On a stream's own attached evidence the gate opens for 98% of the compounds (at once for a contradict presentation, at a median 10.9 s for a mimic, when the second kind's message arrives), for 14% to 37%
of cascades and split brains, **never for the slow leak**, and for 7.5% of plain incidents (66% of those the signature shift altered, 10% of those the added edge altered). A cascade's partner and a split brain's
peer are outside the attached evidence unless the peer happens to be a dependent of the site in the public graph: the cascade gates 14% and 15% of the time (mimic, contradict; A) on the attached evidence and 100% on
all of its own evidence (median 10.9 s and 0.06 s). So **E1's gate cannot see a leak at all and sees a cascade only when other evidence reaches its anomaly**; E1's `captured` counts were 5 of 132 cascades and 5 of 114 leaks, which can then
come only from evidence that is not the incident's own. Decoys open the gate too: 62% of compound decoys, 7% to 21% of the others (the table does not split the decoys by mode).

### 3.3 The ladder (hidden; hard teacher, sources after their answer)

**Family-keyed, stream reset** (the memory that could be A1b's within-stream form without site ids). Seeds 40000-40199 (A) and 70000-70199 (C); tuning-like ranges in `w3-floor-ladder.csv`:

| seeds (hidden) | key read at (s) | key | recalls | on plain/decoy/hard | wrong | collision share [90%] | wrong per stream [90%] | hard recalled right / hard | hard recurrences recalled right / recurrences |
|---|---|---|---|---|---|---|---|---|---|
| 40000-40199 | 6 | K1 | 419 | 203/82/134 | 289 | 0.690 [0.644, 0.734] | 1.45 [1.24, 1.67] | 130/512 | 75/83 |
| 40000-40199 | 6 | K2 | 274 | 116/47/111 | 166 | 0.606 [0.547, 0.662] | 0.83 [0.68, 1.00] | 108/512 | 72/83 |
| 40000-40199 | 6 | K3 | 221 | 88/42/91 | 133 | 0.602 [0.535, 0.664] | 0.67 [0.53, 0.81] | 88/512 | 60/83 |
| 40000-40199 | 6 | K4 | 217 | 86/40/91 | 129 | 0.595 [0.526, 0.659] | 0.65 [0.51, 0.79] | 88/512 | 60/83 |
| 40000-40199 | 13 | K1 | 377 | 160/85/132 | 248 | 0.658 [0.607, 0.706] | 1.24 [1.04, 1.45] | 129/512 | 72/83 |
| 40000-40199 | 13 | K2 | 176 | 36/43/97 | 79 | 0.449 [0.379, 0.518] | 0.40 [0.30, 0.49] | 97/512 | 63/83 |
| 40000-40199 | 13 | K3 | 109 | 21/31/57 | 52 | 0.477 [0.392, 0.558] | 0.26 [0.18, 0.34] | 57/512 | 39/83 |
| 40000-40199 | 13 | K4 | 89 | 21/11/57 | 32 | 0.360 [0.258, 0.457] | 0.16 [0.10, 0.23] | 57/512 | 39/83 |
| 40000-40199 | 16 | K1 | 369 | 149/85/135 | 236 | 0.640 [0.586, 0.690] | 1.18 [0.98, 1.39] | 133/512 | 75/83 |
| 40000-40199 | 16 | K2 | 159 | 9/37/113 | 46 | 0.289 [0.224, 0.354] | 0.23 [0.17, 0.30] | 113/512 | 72/83 |
| 40000-40199 | 16 | K3 | 84 | 4/18/62 | 22 | 0.262 [0.169, 0.359] | 0.11 [0.07, 0.16] | 62/512 | 40/83 |
| 40000-40199 | 16 | K4 | 70 | 4/4/62 | 8 | 0.114 [0.058, 0.175] | 0.04 [0.02, 0.07] | 62/512 | 40/83 |
| 40000-40199 | 32 | K1 | 385 | 156/90/139 | 248 | 0.644 [0.591, 0.694] | 1.24 [1.04, 1.45] | 137/512 | 75/83 |
| 40000-40199 | 32 | K2 | 166 | 9/40/117 | 49 | 0.295 [0.229, 0.362] | 0.24 [0.17, 0.32] | 117/512 | 72/83 |
| 40000-40199 | 32 | K3 | 74 | 4/2/68 | 6 | 0.081 [0.032, 0.136] | 0.03 [0.01, 0.05] | 68/512 | 41/83 |
| 40000-40199 | 32 | K4 | 65 | 4/0/61 | 4 | 0.061 [0.015, 0.119] | 0.02 [0.01, 0.04] | 61/512 | 38/83 |
| 70000-70199 | 6 | K1 | 1326 | 451/183/692 | 667 | 0.503 [0.471, 0.534] | 3.33 [3.00, 3.69] | 659/1516 | 230/259 |
| 70000-70199 | 6 | K2 | 985 | 308/122/555 | 448 | 0.455 [0.418, 0.491] | 2.24 [1.97, 2.52] | 537/1516 | 232/259 |
| 70000-70199 | 6 | K3 | 822 | 238/103/481 | 352 | 0.428 [0.388, 0.468] | 1.76 [1.53, 2.00] | 470/1516 | 202/259 |
| 70000-70199 | 6 | K4 | 814 | 236/97/481 | 344 | 0.423 [0.382, 0.462] | 1.72 [1.50, 1.96] | 470/1516 | 202/259 |
| 70000-70199 | 13 | K1 | 1179 | 314/179/686 | 510 | 0.433 [0.397, 0.467] | 2.55 [2.24, 2.88] | 669/1516 | 219/259 |
| 70000-70199 | 13 | K2 | 780 | 126/112/542 | 240 | 0.308 [0.267, 0.349] | 1.20 [1.00, 1.42] | 540/1516 | 210/259 |
| 70000-70199 | 13 | K3 | 546 | 79/77/390 | 157 | 0.287 [0.245, 0.333] | 0.79 [0.64, 0.94] | 389/1516 | 159/259 |
| 70000-70199 | 13 | K4 | 525 | 79/56/390 | 136 | 0.259 [0.216, 0.304] | 0.68 [0.55, 0.83] | 389/1516 | 159/259 |
| 70000-70199 | 16 | K1 | 1134 | 246/181/707 | 431 | 0.380 [0.346, 0.414] | 2.15 [1.89, 2.44] | 703/1516 | 236/259 |
| 70000-70199 | 16 | K2 | 723 | 14/106/603 | 120 | 0.166 [0.140, 0.193] | 0.60 [0.49, 0.71] | 603/1516 | 235/259 |
| 70000-70199 | 16 | K3 | 417 | 7/57/353 | 64 | 0.153 [0.121, 0.188] | 0.32 [0.24, 0.41] | 353/1516 | 155/259 |
| 70000-70199 | 16 | K4 | 386 | 7/27/352 | 34 | 0.088 [0.064, 0.114] | 0.17 [0.12, 0.23] | 352/1516 | 154/259 |
| 70000-70199 | 32 | K1 | 1175 | 255/192/728 | 451 | 0.384 [0.351, 0.417] | 2.25 [1.99, 2.54] | 724/1516 | 236/259 |
| 70000-70199 | 32 | K2 | 743 | 15/112/616 | 127 | 0.171 [0.144, 0.199] | 0.64 [0.52, 0.76] | 616/1516 | 235/259 |
| 70000-70199 | 32 | K3 | 448 | 3/25/420 | 28 | 0.062 [0.042, 0.086] | 0.14 [0.09, 0.20] | 420/1516 | 149/259 |
| 70000-70199 | 32 | K4 | 392 | 3/4/385 | 7 | 0.018 [0.003, 0.038] | 0.04 [0.01, 0.07] | 385/1516 | 142/259 |

**Site-keyed, stream reset** (the form that cannot cross streams):

| seeds (hidden) | key read at (s) | key | recalls | on plain/decoy/hard | wrong | collision share [90%] | wrong per stream [90%] | hard recalled right / hard | hard recurrences recalled right / recurrences |
|---|---|---|---|---|---|---|---|---|---|
| 40000-40199 | 6 | K1 | 105 | 19/7/79 | 26 | 0.248 [0.174, 0.326] | 0.13 [0.09, 0.17] | 79/512 | 75/83 |
| 40000-40199 | 6 | K2 | 94 | 12/7/75 | 19 | 0.202 [0.130, 0.283] | 0.10 [0.06, 0.14] | 75/512 | 72/83 |
| 40000-40199 | 6 | K3 | 77 | 10/5/62 | 15 | 0.195 [0.118, 0.279] | 0.07 [0.04, 0.11] | 62/512 | 60/83 |
| 40000-40199 | 6 | K4 | 77 | 10/5/62 | 15 | 0.195 [0.118, 0.279] | 0.07 [0.04, 0.11] | 62/512 | 60/83 |
| 40000-40199 | 13 | K1 | 100 | 17/7/76 | 24 | 0.240 [0.165, 0.320] | 0.12 [0.08, 0.17] | 76/512 | 72/83 |
| 40000-40199 | 13 | K2 | 78 | 6/7/65 | 13 | 0.167 [0.097, 0.244] | 0.07 [0.04, 0.10] | 65/512 | 63/83 |
| 40000-40199 | 13 | K3 | 48 | 5/4/39 | 9 | 0.188 [0.098, 0.283] | 0.04 [0.02, 0.07] | 39/512 | 38/83 |
| 40000-40199 | 13 | K4 | 45 | 5/1/39 | 6 | 0.133 [0.054, 0.225] | 0.03 [0.01, 0.05] | 39/512 | 38/83 |
| 40000-40199 | 16 | K1 | 99 | 14/7/78 | 21 | 0.212 [0.141, 0.286] | 0.10 [0.07, 0.14] | 78/512 | 74/83 |
| 40000-40199 | 16 | K2 | 80 | 1/6/73 | 7 | 0.087 [0.040, 0.143] | 0.04 [0.01, 0.06] | 73/512 | 71/83 |
| 40000-40199 | 16 | K3 | 42 | 1/3/38 | 4 | 0.095 [0.025, 0.178] | 0.02 [0.01, 0.04] | 38/512 | 37/83 |
| 40000-40199 | 16 | K4 | 39 | 1/0/38 | 1 | 0.026 [0.000, 0.075] | 0.01 [0.00, 0.01] | 38/512 | 37/83 |
| 40000-40199 | 32 | K1 | 99 | 14/7/78 | 21 | 0.212 [0.141, 0.286] | 0.10 [0.07, 0.14] | 78/512 | 74/83 |
| 40000-40199 | 32 | K2 | 80 | 1/6/73 | 7 | 0.087 [0.040, 0.143] | 0.04 [0.01, 0.06] | 73/512 | 71/83 |
| 40000-40199 | 32 | K3 | 41 | 0/0/41 | 0 | 0.000 [0.000, 0.000] | 0.00 [0.00, 0.00] | 41/512 | 40/83 |
| 40000-40199 | 32 | K4 | 37 | 0/0/37 | 0 | 0.000 [0.000, 0.000] | 0.00 [0.00, 0.00] | 37/512 | 36/83 |
| 70000-70199 | 6 | K1 | 339 | 33/17/289 | 51 | 0.150 [0.117, 0.185] | 0.26 [0.20, 0.32] | 288/1516 | 239/259 |
| 70000-70199 | 6 | K2 | 306 | 24/12/270 | 37 | 0.121 [0.087, 0.157] | 0.18 [0.13, 0.24] | 269/1516 | 237/259 |
| 70000-70199 | 6 | K3 | 254 | 17/11/226 | 28 | 0.110 [0.074, 0.149] | 0.14 [0.09, 0.20] | 226/1516 | 198/259 |
| 70000-70199 | 6 | K4 | 254 | 17/11/226 | 28 | 0.110 [0.074, 0.149] | 0.14 [0.09, 0.20] | 226/1516 | 198/259 |
| 70000-70199 | 13 | K1 | 315 | 24/20/271 | 45 | 0.143 [0.108, 0.179] | 0.23 [0.17, 0.29] | 270/1516 | 217/259 |
| 70000-70199 | 13 | K2 | 270 | 12/18/240 | 31 | 0.115 [0.080, 0.152] | 0.15 [0.10, 0.21] | 239/1516 | 204/259 |
| 70000-70199 | 13 | K3 | 188 | 9/12/167 | 21 | 0.112 [0.071, 0.157] | 0.10 [0.07, 0.15] | 167/1516 | 145/259 |
| 70000-70199 | 13 | K4 | 185 | 9/9/167 | 18 | 0.097 [0.059, 0.140] | 0.09 [0.05, 0.14] | 167/1516 | 145/259 |
| 70000-70199 | 16 | K1 | 326 | 15/21/290 | 36 | 0.110 [0.080, 0.142] | 0.18 [0.12, 0.23] | 290/1516 | 236/259 |
| 70000-70199 | 16 | K2 | 296 | 0/16/280 | 16 | 0.054 [0.033, 0.077] | 0.08 [0.04, 0.12] | 280/1516 | 234/259 |
| 70000-70199 | 16 | K3 | 153 | 0/9/144 | 9 | 0.059 [0.030, 0.091] | 0.04 [0.03, 0.07] | 144/1516 | 130/259 |
| 70000-70199 | 16 | K4 | 150 | 0/7/143 | 7 | 0.047 [0.021, 0.076] | 0.04 [0.01, 0.06] | 143/1516 | 129/259 |
| 70000-70199 | 32 | K1 | 326 | 15/21/290 | 36 | 0.110 [0.080, 0.142] | 0.18 [0.12, 0.23] | 290/1516 | 236/259 |
| 70000-70199 | 32 | K2 | 296 | 0/16/280 | 16 | 0.054 [0.033, 0.077] | 0.08 [0.04, 0.12] | 280/1516 | 234/259 |
| 70000-70199 | 32 | K3 | 173 | 0/6/167 | 6 | 0.035 [0.013, 0.059] | 0.03 [0.01, 0.05] | 167/1516 | 142/259 |
| 70000-70199 | 32 | K4 | 154 | 0/0/154 | 0 | 0.000 [0.000, 0.000] | 0.00 [0.00, 0.00] | 154/1516 | 133/259 |

**Family-keyed, carried across streams** (the form that could learn a law; the table starts empty at the first stream of the range and fills as it goes):

| seeds (hidden) | key read at (s) | key | recalls | on plain/decoy/hard | wrong | collision share [90%] | wrong per stream [90%] | hard recalled right / hard | hard recurrences recalled right / recurrences |
|---|---|---|---|---|---|---|---|---|---|
| 40000-40199 | 6 | K1 | 3625 | 2675/469/481 | 3162 | 0.872 [0.863, 0.882] | 15.81 [15.26, 16.36] | 463/512 | 77/83 |
| 40000-40199 | 6 | K2 | 3463 | 2554/455/454 | 3022 | 0.873 [0.863, 0.882] | 15.11 [14.55, 15.66] | 441/512 | 74/83 |
| 40000-40199 | 6 | K3 | 3213 | 2356/433/424 | 2803 | 0.872 [0.862, 0.883] | 14.02 [13.43, 14.60] | 410/512 | 70/83 |
| 40000-40199 | 6 | K4 | 3201 | 2354/424/423 | 2792 | 0.872 [0.862, 0.883] | 13.96 [13.38, 14.54] | 409/512 | 70/83 |
| 40000-40199 | 13 | K1 | 3275 | 2324/468/483 | 2803 | 0.856 [0.845, 0.867] | 14.02 [13.37, 14.67] | 472/512 | 76/83 |
| 40000-40199 | 13 | K2 | 3036 | 2123/457/456 | 2583 | 0.851 [0.839, 0.862] | 12.91 [12.24, 13.57] | 453/512 | 74/83 |
| 40000-40199 | 13 | K3 | 2633 | 1827/409/397 | 2242 | 0.852 [0.839, 0.864] | 11.21 [10.54, 11.88] | 391/512 | 68/83 |
| 40000-40199 | 13 | K4 | 2525 | 1825/303/397 | 2134 | 0.845 [0.832, 0.858] | 10.67 [10.01, 11.33] | 391/512 | 68/83 |
| 40000-40199 | 16 | K1 | 2097 | 1170/442/485 | 1616 | 0.771 [0.754, 0.787] | 8.08 [7.68, 8.48] | 481/512 | 78/83 |
| 40000-40199 | 16 | K2 | 1241 | 432/347/462 | 779 | 0.628 [0.600, 0.655] | 3.90 [3.58, 4.22] | 462/512 | 76/83 |
| 40000-40199 | 16 | K3 | 948 | 236/306/406 | 542 | 0.572 [0.540, 0.603] | 2.71 [2.45, 2.98] | 406/512 | 71/83 |
| 40000-40199 | 16 | K4 | 803 | 236/161/406 | 397 | 0.494 [0.459, 0.529] | 1.99 [1.76, 2.22] | 406/512 | 71/83 |
| 40000-40199 | 32 | K1 | 2098 | 1170/442/486 | 1616 | 0.770 [0.754, 0.786] | 8.08 [7.68, 8.48] | 482/512 | 78/83 |
| 40000-40199 | 32 | K2 | 1242 | 432/347/463 | 779 | 0.627 [0.599, 0.655] | 3.90 [3.58, 4.22] | 463/512 | 76/83 |
| 40000-40199 | 32 | K3 | 872 | 176/292/404 | 468 | 0.537 [0.505, 0.568] | 2.34 [2.13, 2.56] | 404/512 | 68/83 |
| 40000-40199 | 32 | K4 | 701 | 172/127/402 | 299 | 0.426 [0.393, 0.460] | 1.50 [1.33, 1.67] | 402/512 | 66/83 |
| 70000-70199 | 6 | K1 | 4113 | 2127/513/1473 | 2731 | 0.664 [0.651, 0.677] | 13.65 [13.12, 14.19] | 1382/1516 | 239/259 |
| 70000-70199 | 6 | K2 | 3981 | 2036/499/1446 | 2602 | 0.654 [0.640, 0.667] | 13.01 [12.47, 13.54] | 1379/1516 | 240/259 |
| 70000-70199 | 6 | K3 | 3816 | 1936/484/1396 | 2480 | 0.650 [0.636, 0.664] | 12.40 [11.86, 12.94] | 1336/1516 | 233/259 |
| 70000-70199 | 6 | K4 | 3805 | 1933/476/1396 | 2469 | 0.649 [0.634, 0.663] | 12.35 [11.80, 12.88] | 1336/1516 | 233/259 |
| 70000-70199 | 13 | K1 | 3977 | 1992/508/1477 | 2543 | 0.639 [0.626, 0.653] | 12.71 [12.17, 13.26] | 1434/1516 | 244/259 |
| 70000-70199 | 13 | K2 | 3772 | 1836/492/1444 | 2345 | 0.622 [0.606, 0.637] | 11.72 [11.15, 12.30] | 1427/1516 | 245/259 |
| 70000-70199 | 13 | K3 | 3468 | 1651/463/1354 | 2130 | 0.614 [0.598, 0.630] | 10.65 [10.09, 11.21] | 1338/1516 | 228/259 |
| 70000-70199 | 13 | K4 | 3369 | 1649/367/1353 | 2032 | 0.603 [0.587, 0.620] | 10.16 [9.60, 10.71] | 1337/1516 | 228/259 |
| 70000-70199 | 16 | K1 | 3026 | 1085/460/1481 | 1554 | 0.513 [0.495, 0.532] | 7.77 [7.33, 8.21] | 1472/1516 | 250/259 |
| 70000-70199 | 16 | K2 | 2350 | 542/355/1453 | 897 | 0.382 [0.357, 0.406] | 4.49 [4.10, 4.88] | 1453/1516 | 247/259 |
| 70000-70199 | 16 | K3 | 2165 | 463/340/1362 | 803 | 0.371 [0.347, 0.395] | 4.01 [3.67, 4.37] | 1362/1516 | 231/259 |
| 70000-70199 | 16 | K4 | 2002 | 463/178/1361 | 641 | 0.320 [0.295, 0.345] | 3.21 [2.88, 3.54] | 1361/1516 | 231/259 |
| 70000-70199 | 32 | K1 | 3030 | 1088/460/1482 | 1557 | 0.514 [0.495, 0.533] | 7.79 [7.34, 8.22] | 1473/1516 | 250/259 |
| 70000-70199 | 32 | K2 | 2351 | 543/355/1453 | 898 | 0.382 [0.358, 0.406] | 4.49 [4.11, 4.88] | 1453/1516 | 247/259 |
| 70000-70199 | 32 | K3 | 2093 | 417/318/1358 | 735 | 0.351 [0.326, 0.376] | 3.67 [3.33, 4.03] | 1358/1516 | 231/259 |
| 70000-70199 | 32 | K4 | 1880 | 406/119/1355 | 525 | 0.279 [0.255, 0.304] | 2.62 [2.33, 2.93] | 1355/1516 | 230/259 |

Who recalls wrongly (hidden):

| seeds (hidden) | family-keyed, stream reset, hard teacher | wrong recalls by the incident that recalled (tier family mode, count) | of the plain ones: altered by the shift / by the added edge / unaltered |
|---|---|---|---|
| A 40000-40199 | ladder K2 at 6 s | plain 116, decoy slow_leak 25, decoy split_brain mimic 15, decoy compound mimic 2, decoy cascade mimic 2, hard cascade mimic 2, decoy compound contradict 2, decoy cascade contradict 1, hard compound mimic 1 | altered by the signature shift 15; altered by the added edge 4; unaltered 97 |
| A 40000-40199 | ladder K2 at 16 s | decoy slow_leak 28, plain 9, decoy compound contradict 4, decoy split_brain contradict 3, decoy cascade contradict 2 | altered by the signature shift 3; altered by the added edge 4; unaltered 2 |
| A 40000-40199 | ladder K4 at 16 s | plain 4, decoy slow_leak 3, decoy compound contradict 1 | altered by the signature shift 2; altered by the added edge 2; unaltered 0 |
| A 40000-40199 | ladder K4 at 32 s | plain 4 | altered by the signature shift 2; altered by the added edge 1; unaltered 1 |
| C 70000-70199 | ladder K2 at 6 s | plain 308, decoy slow_leak 57, decoy split_brain mimic 18, decoy split_brain contradict 14, decoy compound mimic 11, decoy cascade mimic 11, hard compound mimic 9, hard cascade mimic 8, decoy compound contradict 7, decoy cascade contradict 4, hard compound contradict 1 | altered by the signature shift 25; altered by the added edge 2; unaltered 281 |
| C 70000-70199 | ladder K2 at 16 s | decoy slow_leak 59, decoy split_brain contradict 23, plain 14, decoy compound contradict 13, decoy cascade contradict 9, decoy cascade mimic 1, decoy compound mimic 1 | altered by the signature shift 5; altered by the added edge 5; unaltered 4 |
| C 70000-70199 | ladder K4 at 16 s | decoy split_brain contradict 10, decoy slow_leak 9, plain 7, decoy compound contradict 6, decoy cascade contradict 1, decoy compound mimic 1 | altered by the signature shift 2; altered by the added edge 2; unaltered 3 |
| C 70000-70199 | ladder K4 at 32 s | decoy slow_leak 4, plain 3 | altered by the signature shift 2; altered by the added edge 0; unaltered 1 |

Reading, the in-stream rows.

- **The first seconds' key is W2's key.** At 6 s the family-keyed K2 key is wrong in 0.606 [0.547, 0.662] of its recalls (A held-out; 0.83 wrong recalls per stream) and 0.455 [0.418, 0.491] in C
  (2.24 per stream). W2's class-only table, on code-derived classes and its own teacher, said 0.575; the simulator reproduces W2's table cell for cell on W2's classes (section 3.5), and the data-derived keys give
  a number of the same size.
- **The phase-2 evidence removes the plain collisions and leaves the decoys.** Between 6 s and 16 s the family-keyed K2 key falls to 0.289 [0.224, 0.354] (A; 0.23 per stream) and 0.166 [0.140, 0.193]
  (C; 0.60 per stream). Wrong recalls on plain incidents fall from 116 to 9 (A), from 308 to 14 (C): once the key holds the verdict of the rules and the evidence outside the site, a plain incident rarely
  looks like a hard one. What remains is **decoys**: slow-leak decoys (28 of 46 wrong recalls in A, 59 of 120 in C) and contradict-mode decoys (9 in A, 45 in C). Per `HIDDEN-DESIGN.md` section 4.2 phase 2 holds no *abnormal*
  evidence for the leak or for a contradict presentation (already contradictory in phase 1): only hidden-vocabulary messages, which the public rules ignore. The breakdown above is consistent with that (the wrong recalls at 16 s are the
  leak and contradict decoys, not mimic ones), so the rules' own evidence cannot separate these decoys from their hard family until they resolve.
- **The decoy's resolution is the rest, and it is late.** K3's extra features (bands, partner) buy little: 0.262 [0.169, 0.359] at 16 s (A). K4's benign streak is the decoy's own evidence; the share of decoys that
  have shown it is 0.11 at 10 s, **0.39 at 16 s**, 0.78 at 24 s and 1.00 at 32 s after the first alarm (slow-leak decoys 0.44, 0.75, 1.00, 1.00; hard incidents 0.00 to 0.03 throughout):

| group (A 40000-40199, hidden) | 10 s | 16 s | 24 s | 32 s |
|---|---|---|---|---|
| decoy | 0.110 | 0.392 | 0.779 | 1.000 |
| decoy/slow_leak | 0.443 | 0.754 | 1.000 | 1.000 |
| decoy/other families | 0.003 | 0.276 | 0.708 | 1.000 |
| hard | 0.000 | 0.000 | 0.004 | 0.031 |
| hard/slow_leak | 0.000 | 0.000 | 0.018 | 0.114 |
| plain | 0.001 | 0.003 | 0.001 | 0.029 |

  With it K4 is 0.114 [0.058, 0.175] at 16 s (A) and 0.088 [0.064, 0.114] (C); at 20 s 0.043 [0.011, 0.086] (A) and 0.043 [0.025, 0.065] (C) (`w3-floor-ladder.csv`); at 32 s 0.061 [0.015, 0.119] (A) and
  0.018 [0.003, 0.038] (C). At 16 s the arm has already asked.
- **What waiting costs in reach.** K2 at 16 s recalls right 113 of the 512 hard incidents (A; 22%) and 72 of the 83 hard recurrences; K4 at 16 s 62 and 40; at 32 s 61 and 38. In C 603 of 1516 (235 of 259
  recurrences) at K2/16 s and 352 (154) at K4/16 s. The hard incidents never recalled are those with no earlier source at the same key.
- **The site-keyed form** is cleaner: 0.087 [0.040, 0.143] at K2/16 s (A; 73 hard recalled right, 71 of 83 recurrences), 0.054 [0.033, 0.077] (C), 0 wrong in 41 recalls (A) at K3/32 s; its per-stream
  count is 0.04 (A) and 0.08 (C). The first-seconds site key is 0.248 [0.174, 0.326] (A, K1 at 6 s); W2's phase-1 site-and-class floor of 0.147 was for classes, not these features.
- **Across streams nothing meets 0.20.** The carried family-keyed floor is 0.87 at 6 s, 0.63 [0.60, 0.66] at K2/16 s and 0.43 [0.39, 0.46] at K4/32 s (A); 0.66, 0.38 and 0.28 (C). In A the 299 wrong recalls at K4/32 s are
  172 plain incidents (138 of them altered by no regime change) and 127 decoys (121 slow-leak decoys, 6 contradict cascades; `w3-floor-who.csv`, carried rows). Two inspections of `floor-rows.csv` that are not tables:
  203 plain resource-exhaustion incidents share one K4 key with **two hard compounds** (a signature shift made their second kind's message the first kind's, so they present as plain resource exhaustion), and 116 of the 122 slow-leak
  decoys carry a key that **one** hard slow leak in the range also carries (a leak whose last readings were benign). A table of hard answers cannot learn that plain incidents and decoys share these keys: aggregation changes nothing (below),
  and a key that one or two sources hold is a key a vote cannot repair.

### 3.4 The key forms E1 built (hidden; against E1's measured collisions)

E1's keys at their own snapshot, hard teacher, sources after their answer (tuning-like ranges follow the held-out ones; `w3-floor-e1.csv` has both availability readings: they differ by at most 2 recalls in
any cell):

| seeds (hidden) | E1 level | form | stream | recalls | on plain/decoy/hard | wrong | collision share [90%] | wrong per stream [90%] | hard recalled right / hard | hard recurrences recalled right / recurrences |
|---|---|---|---|---|---|---|---|---|---|---|
| 40000-40199 | kinds | site | reset | 26 | 2/1/23 | 3 | 0.115 [0.000, 0.233] | 0.01 [0.00, 0.03] | 23/512 | 21/83 |
| 40000-40199 | timing | site | reset | 15 | 2/0/13 | 2 | 0.133 [0.000, 0.294] | 0.01 [0.00, 0.03] | 13/512 | 12/83 |
| 40000-40199 | kinds | family | reset | 55 | 16/7/32 | 23 | 0.418 [0.294, 0.537] | 0.12 [0.07, 0.17] | 32/512 | 21/83 |
| 40000-40199 | timing | family | reset | 34 | 16/1/17 | 17 | 0.500 [0.333, 0.646] | 0.09 [0.04, 0.14] | 17/512 | 12/83 |
| 40000-40199 | timing | family | carried | 344 | 125/93/126 | 219 | 0.637 [0.582, 0.688] | 1.09 [0.92, 1.28] | 125/512 | 20/83 |
| 40000-40199 | kinds | family | carried | 392 | 131/107/154 | 239 | 0.610 [0.558, 0.658] | 1.20 [1.00, 1.40] | 153/512 | 24/83 |
| 40000-40199 | timing | site | carried | 136 | 27/47/62 | 74 | 0.544 [0.471, 0.615] | 0.37 [0.29, 0.46] | 62/512 | 15/83 |
| 70000-70199 | kinds | site | reset | 104 | 2/11/91 | 13 | 0.125 [0.073, 0.182] | 0.07 [0.04, 0.10] | 91/1516 | 80/259 |
| 70000-70199 | timing | site | reset | 57 | 2/6/49 | 8 | 0.140 [0.070, 0.218] | 0.04 [0.02, 0.07] | 49/1516 | 48/259 |
| 70000-70199 | kinds | family | reset | 227 | 32/27/168 | 61 | 0.269 [0.208, 0.329] | 0.30 [0.22, 0.40] | 166/1516 | 79/259 |
| 70000-70199 | timing | family | reset | 122 | 30/13/79 | 44 | 0.361 [0.270, 0.444] | 0.22 [0.14, 0.30] | 78/1516 | 48/259 |
| 70000-70199 | timing | family | carried | 701 | 141/93/467 | 241 | 0.344 [0.306, 0.381] | 1.21 [1.02, 1.40] | 460/1516 | 80/259 |
| 70000-70199 | kinds | family | carried | 772 | 149/99/524 | 256 | 0.332 [0.296, 0.367] | 1.28 [1.09, 1.47] | 516/1516 | 92/259 |
| 70000-70199 | timing | site | carried | 376 | 22/59/295 | 84 | 0.223 [0.183, 0.265] | 0.42 [0.33, 0.52] | 292/1516 | 63/259 |
| 10000-10099 | kinds | site | reset | 19 | 0/0/19 | 0 | 0.000 [0.000, 0.000] | 0.00 [0.00, 0.00] | 19/243 | 18/44 |
| 10000-10099 | timing | site | reset | 12 | 0/0/12 | 0 | 0.000 [0.000, 0.000] | 0.00 [0.00, 0.00] | 12/243 | 12/44 |
| 10000-10099 | kinds | family | reset | 40 | 12/3/25 | 15 | 0.375 [0.229, 0.538] | 0.15 [0.08, 0.23] | 25/243 | 18/44 |
| 10000-10099 | timing | family | reset | 27 | 10/2/15 | 12 | 0.444 [0.258, 0.667] | 0.12 [0.06, 0.19] | 15/243 | 12/44 |
| 10000-10099 | timing | family | carried | 158 | 47/46/65 | 93 | 0.589 [0.491, 0.682] | 0.93 [0.68, 1.20] | 65/243 | 18/44 |
| 10000-10099 | kinds | family | carried | 184 | 50/53/81 | 103 | 0.560 [0.468, 0.647] | 1.03 [0.77, 1.31] | 81/243 | 19/44 |
| 10000-10099 | timing | site | carried | 44 | 6/11/27 | 17 | 0.386 [0.250, 0.525] | 0.17 [0.10, 0.25] | 27/243 | 13/44 |
| 60000-60099 | kinds | site | reset | 56 | 2/0/54 | 2 | 0.036 [0.000, 0.085] | 0.02 [0.00, 0.05] | 54/786 | 47/145 |
| 60000-60099 | timing | site | reset | 38 | 2/0/36 | 2 | 0.053 [0.000, 0.125] | 0.02 [0.00, 0.05] | 36/786 | 33/145 |
| 60000-60099 | kinds | family | reset | 121 | 17/10/94 | 33 | 0.273 [0.172, 0.378] | 0.33 [0.19, 0.50] | 88/786 | 47/145 |
| 60000-60099 | timing | family | reset | 77 | 15/5/57 | 24 | 0.312 [0.177, 0.446] | 0.24 [0.12, 0.38] | 53/786 | 33/145 |
| 60000-60099 | timing | family | carried | 310 | 38/45/227 | 90 | 0.290 [0.241, 0.340] | 0.90 [0.71, 1.11] | 220/786 | 44/145 |
| 60000-60099 | kinds | family | carried | 351 | 46/47/258 | 102 | 0.291 [0.239, 0.344] | 1.02 [0.80, 1.26] | 249/786 | 48/145 |
| 60000-60099 | timing | site | carried | 164 | 3/25/136 | 28 | 0.171 [0.124, 0.221] | 0.28 [0.20, 0.37] | 136/786 | 38/145 |

Against E1's held-out recalls (seeds 40000-40199, `recalls.csv` of each arm, read only), incident by incident, same key form, level and reset:

| E1 arm (seeds 40000-40199) | E1 recalls | on an incident with a hidden-side snapshot | E1 by tier plain/hard/decoy | E1 recalls with no hidden-side snapshot, by tier | floor recalls | floor by tier | in both | E1 only | floor only | same-stream recalls whose hidden keys of source and target are equal |
|---|---|---|---|---|---|---|---|---|---|---|
| site_kinds_never_reset | 15 | 11 | 2/12/1 | 2/2/0 | 26 | 2/23/1 | 11 | 4 | 15 | 11/11 |
| site_kinds_never_carry (sens) | 468 | 110 | 282/95/91 | 261/46/51 | 197 | 29/101/67 | 96 | 372 | 101 | 6/6 |
| site_bands_never_reset (sens) | 9 | 6 | 2/7/0 | 2/1/0 | 17 | 2/15/0 | 6 | 3 | 11 | 6/6 |
| site_bands_never_carry (sens) | 336 | 99 | 189/75/72 | 168/33/36 | 168 | 27/84/57 | 84 | 252 | 84 | 4/4 |
| site_timing_never_reset (sens) | 5 | 5 | 0/5/0 | 0/0/0 | 15 | 2/13/0 | 5 | 0 | 10 | 5/5 |
| site_timing_never_carry | 107 | 49 | 45/32/30 | 40/9/9 | 136 | 27/62/47 | 45 | 62 | 91 | 4/4 |
| family_kinds_never_reset (sens) | 46 | 23 | 28/15/3 | 20/3/0 | 55 | 16/32/7 | 20 | 26 | 35 | 22/23 |
| family_kinds_never_carry (sens) | 1788 | 278 | 1339/230/219 | 1248/126/136 | 392 | 131/154/107 | 260 | 1528 | 132 | 7/7 |
| family_bands_never_reset (sens) | 31 | 15 | 22/9/0 | 14/2/0 | 42 | 16/23/3 | 12 | 19 | 30 | 14/15 |
| family_bands_never_carry (sens) | 1373 | 253 | 978/198/197 | 897/105/118 | 359 | 125/140/94 | 229 | 1144 | 130 | 5/5 |
| family_timing_never_reset | 12 | 9 | 6/6/0 | 2/1/0 | 34 | 16/17/1 | 7 | 5 | 27 | 8/9 |
| family_timing_never_carry | 625 | 175 | 372/128/125 | 328/63/59 | 344 | 125/126/93 | 167 | 458 | 177 | 4/5 |

Reading.

- **E1's site-keyed collision of 0.11 sits on the floor.** E1 measured 1 wrong in 9 recalls with a right source (0.111 [0, 0.333]); the hidden-side floor for the same key, from each incident's own
  evidence, is 3 wrong in 26 recalls (0.115 [0.000, 0.233]; 0.01 per stream) on the same seeds, 0.125 [0.073, 0.182] in world C. A site-keyed key with `kinds` and the stream reset
  collides at about one recall in eight because of what the world presents, not because of the arm: the wrong recalls are 2 plain incidents and 1 decoy of 26 (A) and 2 plain incidents and 11 decoys of 104 (C).
- **E1's family-keyed, carried collision of 0.81 is 0.64 on the floor and the rest is the arm's evidence.** The hidden-side floor for `timing`, family-keyed, carried, from own evidence: 0.637 [0.582, 0.688] (A held-out),
  1.09 wrong per stream; world C 0.344 [0.306, 0.381]. E1's 625 recalls with an incident (661 with the background's) split as 175 on incidents that have a gate on their own evidence and **450 that do not**
  (328 plain, 63 hard, 59 decoy): of E1's 372 plain recalls, 328 were on plain incidents whose own evidence the public checker finds consistent. Their anomalies must have become contradictory through evidence that is not
  their own (other symptoms attached to them); this is the contamination the floor leaves out. I did not look at the arm's anomaly evidence, so *which* symptoms is untested; the count (328 of 372) is measured. The floor's own plain recalls are 125 against E1's 372, its hard
  126 against 128, its decoy 93 against 125: **what the arm adds over the floor is plain recalls**.
- **The instrument agrees with E1 where E1 recalled in the same stream:** the hidden keys of the source and the target are equal for 11 of 11 (site, kinds, reset), 6 of 6, 22 of 23 and 8 of 9 (family forms)
  recalls with both keys, so the features are E1's. Of the 15 recalls of the site-keyed `kinds` reset arm, 11 are on incidents with a hidden snapshot and all 11 are in the floor's set; the floor has 15 more.
- **Reach.** The hard recurrences recalled right under the floor are 21 of 83 (`kinds`, site, reset; 0.25) and 12 of 83 (`timing`), against E1's measured 7 (0.084): E1 reached a third of what the key allows in the
  best case (the others are the incidents whose gate or whose arm-side evidence differs; not diagnosed here). In C: 80 of 259 and 48 of 259.

Aggregation (the memory keeps every answer bound at a key and recalls the leader only when its share is above 0.5 or 0.8) does not lower the floor, because the collision is a non-hard incident sharing a key with
hard sources and a table of hard answers cannot know it:

| seeds (hidden) | key | form | stream | rule | recalls | on plain/decoy/hard | wrong | collision share [90%] | hard recalled right / hard |
|---|---|---|---|---|---|---|---|---|---|
| 40000-40199 | E1 kinds at the snapshot | family | carried | last answer | 392 | 131/107/154 | 239 | 0.610 [0.558, 0.658] | 153/512 |
| 40000-40199 | E1 kinds at the snapshot | family | carried | vote, leader above 0.5 | 380 | 121/105/154 | 227 | 0.597 [0.545, 0.647] | 153/512 |
| 40000-40199 | E1 kinds at the snapshot | family | carried | vote, leader above 0.8 | 380 | 121/105/154 | 227 | 0.597 [0.545, 0.647] | 153/512 |
| 40000-40199 | E1 timing at the snapshot | family | carried | last answer | 344 | 125/93/126 | 219 | 0.637 [0.582, 0.688] | 125/512 |
| 40000-40199 | E1 timing at the snapshot | family | carried | vote, leader above 0.5 | 334 | 116/92/126 | 209 | 0.626 [0.571, 0.677] | 125/512 |
| 40000-40199 | E1 timing at the snapshot | family | carried | vote, leader above 0.8 | 334 | 116/92/126 | 209 | 0.626 [0.571, 0.677] | 125/512 |
| 40000-40199 | E1 timing at the snapshot | family | reset | last answer | 34 | 16/1/17 | 17 | 0.500 [0.333, 0.646] | 17/512 |
| 40000-40199 | E1 timing at the snapshot | family | reset | vote, leader above 0.5 | 34 | 16/1/17 | 17 | 0.500 [0.333, 0.646] | 17/512 |
| 40000-40199 | E1 timing at the snapshot | family | reset | vote, leader above 0.8 | 34 | 16/1/17 | 17 | 0.500 [0.333, 0.646] | 17/512 |
| 40000-40199 | ladder K2 at 16 s | family | carried | last answer | 1241 | 432/347/462 | 779 | 0.628 [0.600, 0.655] | 462/512 |
| 40000-40199 | ladder K2 at 16 s | family | carried | vote, leader above 0.5 | 1241 | 432/347/462 | 779 | 0.628 [0.600, 0.655] | 462/512 |
| 40000-40199 | ladder K2 at 16 s | family | carried | vote, leader above 0.8 | 1241 | 432/347/462 | 779 | 0.628 [0.600, 0.655] | 462/512 |
| 40000-40199 | ladder K4 at 32 s | family | carried | last answer | 701 | 172/127/402 | 299 | 0.426 [0.393, 0.460] | 402/512 |
| 40000-40199 | ladder K4 at 32 s | family | carried | vote, leader above 0.5 | 701 | 172/127/402 | 299 | 0.426 [0.393, 0.460] | 402/512 |
| 40000-40199 | ladder K4 at 32 s | family | carried | vote, leader above 0.8 | 701 | 172/127/402 | 299 | 0.426 [0.393, 0.460] | 402/512 |
| 40000-40199 | ladder K4 at 16 s | family | reset | last answer | 70 | 4/4/62 | 8 | 0.114 [0.058, 0.175] | 62/512 |
| 40000-40199 | ladder K4 at 16 s | family | reset | vote, leader above 0.5 | 70 | 4/4/62 | 8 | 0.114 [0.058, 0.175] | 62/512 |
| 40000-40199 | ladder K4 at 16 s | family | reset | vote, leader above 0.8 | 70 | 4/4/62 | 8 | 0.114 [0.058, 0.175] | 62/512 |
| 70000-70199 | E1 kinds at the snapshot | family | carried | last answer | 772 | 149/99/524 | 256 | 0.332 [0.296, 0.367] | 516/1516 |
| 70000-70199 | E1 kinds at the snapshot | family | carried | vote, leader above 0.5 | 750 | 130/98/522 | 237 | 0.316 [0.280, 0.352] | 513/1516 |
| 70000-70199 | E1 kinds at the snapshot | family | carried | vote, leader above 0.8 | 718 | 109/95/514 | 208 | 0.290 [0.252, 0.328] | 510/1516 |
| 70000-70199 | E1 timing at the snapshot | family | carried | last answer | 701 | 141/93/467 | 241 | 0.344 [0.306, 0.381] | 460/1516 |
| 70000-70199 | E1 timing at the snapshot | family | carried | vote, leader above 0.5 | 684 | 127/92/465 | 227 | 0.332 [0.294, 0.370] | 457/1516 |
| 70000-70199 | E1 timing at the snapshot | family | carried | vote, leader above 0.8 | 661 | 111/90/460 | 205 | 0.310 [0.271, 0.349] | 456/1516 |
| 70000-70199 | E1 timing at the snapshot | family | reset | last answer | 122 | 30/13/79 | 44 | 0.361 [0.270, 0.444] | 78/1516 |
| 70000-70199 | E1 timing at the snapshot | family | reset | vote, leader above 0.5 | 120 | 30/12/78 | 42 | 0.350 [0.259, 0.434] | 78/1516 |
| 70000-70199 | E1 timing at the snapshot | family | reset | vote, leader above 0.8 | 120 | 30/12/78 | 42 | 0.350 [0.259, 0.434] | 78/1516 |
| 70000-70199 | ladder K2 at 16 s | family | carried | last answer | 2350 | 542/355/1453 | 897 | 0.382 [0.357, 0.406] | 1453/1516 |
| 70000-70199 | ladder K2 at 16 s | family | carried | vote, leader above 0.5 | 2350 | 542/355/1453 | 897 | 0.382 [0.357, 0.406] | 1453/1516 |
| 70000-70199 | ladder K2 at 16 s | family | carried | vote, leader above 0.8 | 2350 | 542/355/1453 | 897 | 0.382 [0.357, 0.406] | 1453/1516 |
| 70000-70199 | ladder K4 at 32 s | family | carried | last answer | 1880 | 406/119/1355 | 525 | 0.279 [0.255, 0.304] | 1355/1516 |
| 70000-70199 | ladder K4 at 32 s | family | carried | vote, leader above 0.5 | 1880 | 406/119/1355 | 525 | 0.279 [0.255, 0.304] | 1355/1516 |
| 70000-70199 | ladder K4 at 32 s | family | carried | vote, leader above 0.8 | 1880 | 406/119/1355 | 525 | 0.279 [0.255, 0.304] | 1355/1516 |
| 70000-70199 | ladder K4 at 16 s | family | reset | last answer | 386 | 7/27/352 | 34 | 0.088 [0.064, 0.114] | 352/1516 |
| 70000-70199 | ladder K4 at 16 s | family | reset | vote, leader above 0.5 | 386 | 7/27/352 | 34 | 0.088 [0.064, 0.114] | 352/1516 |
| 70000-70199 | ladder K4 at 16 s | family | reset | vote, leader above 0.8 | 386 | 7/27/352 | 34 | 0.088 [0.064, 0.114] | 352/1516 |

### 3.5 Checks on the instrument

W2's published phase-1 table against this script's simulator on W2's code-derived classes (every cell equal in every range, world C included):

| seeds (hidden) | form | tier | this script: incidents/recalled/right/wrong | W2's table | equal |
|---|---|---|---|---|---|
| 10000-10099 | site+class | all | 2643/142/120/22 | 2643/142/120/22 | True |
| 10000-10099 | site+class | plain | 2110/13/0/13 | 2110/13/0/13 | True |
| 10000-10099 | site+class | hard | 243/54/47/7 | 243/54/47/7 | True |
| 10000-10099 | site+class | decoy | 290/75/73/2 | 290/75/73/2 | True |
| 10000-10099 | class only | all | 2643/386/151/235 | 2643/386/151/235 | True |
| 10000-10099 | class only | plain | 2110/187/0/187 | 2110/187/0/187 | True |
| 10000-10099 | class only | hard | 243/89/63/26 | 243/89/63/26 | True |
| 10000-10099 | class only | decoy | 290/110/88/22 | 290/110/88/22 | True |
| 40000-40199 | site+class | all | 5344/259/221/38 | 5344/259/221/38 | True |
| 40000-40199 | site+class | plain | 4304/21/0/21 | 4304/21/0/21 | True |
| 40000-40199 | site+class | hard | 512/97/87/10 | 512/97/87/10 | True |
| 40000-40199 | site+class | decoy | 528/141/134/7 | 528/141/134/7 | True |
| 40000-40199 | class only | all | 5344/670/285/385 | 5344/670/285/385 | True |
| 40000-40199 | class only | plain | 4304/284/0/284 | 4304/284/0/284 | True |
| 40000-40199 | class only | hard | 512/162/111/51 | 512/162/111/51 | True |
| 40000-40199 | class only | decoy | 528/224/174/50 | 528/224/174/50 | True |
| 60000-60099 | site+class | all | 2581/270/224/46 | 2581/270/224/46 | True |
| 60000-60099 | site+class | plain | 1519/20/0/20 | 1519/20/0/20 | True |
| 60000-60099 | site+class | hard | 786/184/168/16 | 786/184/168/16 | True |
| 60000-60099 | site+class | decoy | 276/66/56/10 | 276/66/56/10 | True |
| 60000-60099 | class only | all | 2581/703/363/340 | 2581/703/363/340 | True |
| 60000-60099 | class only | plain | 1519/193/0/193 | 1519/193/0/193 | True |
| 60000-60099 | class only | hard | 786/376/297/79 | 786/376/297/79 | True |
| 60000-60099 | class only | decoy | 276/134/66/68 | 276/134/66/68 | True |
| 70000-70199 | site+class | all | 5341/479/416/63 | 5341/479/416/63 | True |
| 70000-70199 | site+class | plain | 3273/38/0/38 | 3273/38/0/38 | True |
| 70000-70199 | site+class | hard | 1516/301/289/12 | 1516/301/289/12 | True |
| 70000-70199 | site+class | decoy | 552/140/127/13 | 552/140/127/13 | True |
| 70000-70199 | class only | all | 5341/1490/706/784 | 5341/1490/706/784 | True |
| 70000-70199 | class only | plain | 3273/505/0/505 | 3273/505/0/505 | True |
| 70000-70199 | class only | hard | 1516/708/565/143 | 1516/708/565/143 | True |
| 70000-70199 | class only | decoy | 552/277/141/136 | 552/277/141/136 | True |

W2's code-derived classes against the data-derived keys at 6 s (`K2`). Where no regime change touched the incident, every data class holds incidents of a single W2 class (100% in all four ranges) and 90% to 95% of the
incidents of a W2 class sit in one data class; the data classes are finer (19 against 14). The code reading of W2 is therefore confirmed on the incidents the regime changes did not alter (W2's own least-sure item 5). Altered
incidents add many keys (a shifted message makes a new tag set), which is the "all incidents" row (81 to 106 data classes):

| seeds (hidden) | subset | incidents | W2 code-derived classes | data-derived classes (K2 at 6 s) | incidents whose W2 class holds one data class | incidents whose data class holds one W2 class |
|---|---|---|---|---|---|---|
| 10000-10099 | all incidents | 2620 | 14 | 81 | 105 | 2289 |
| 10000-10099 | not altered by a regime change | 2258 | 14 | 19 | 2131 | 2258 |
| 40000-40199 | all incidents | 5318 | 14 | 99 | 236 | 4549 |
| 40000-40199 | not altered by a regime change | 4584 | 14 | 19 | 4356 | 4584 |
| 60000-60099 | all incidents | 2565 | 14 | 90 | 289 | 2234 |
| 60000-60099 | not altered by a regime change | 2222 | 14 | 19 | 2004 | 2222 |
| 70000-70199 | all incidents | 5313 | 14 | 106 | 552 | 3941 |
| 70000-70199 | not altered by a regime change | 4621 | 14 | 19 | 4162 | 4621 |

### 3.6 What the floor says

- **A key that waits for the rule-breaking evidence is clean against plain incidents and not against decoys.** Within a stream, the phase-2 evidence takes the family-keyed collision from 0.61 (6 s) to 0.29 (16 s) in
  world A (0.46 to 0.17 in C); what is left is decoys that no abnormal evidence separates from their hard family before they resolve. That the bound of 0.20 is met only by a key that uses the decoy's resolution (K4: 0.114 at 16 s,
  0.043 at 20 s; both after the arm has asked) or by a site-keyed key (0.087) is the floor's statement; E1's measured 0.11 is the site-keyed floor.
- **Across streams the floor is above the bound under any key tried and under aggregation:** 0.28 to 0.87 (carried, 6 s to 32 s, K1 to K4, A and C; 0.34 to 0.64 for E1's keys), against 0.20.
- **The mix matters to the arithmetic, not to the finding.** Collision *shares* fall when the hard share rises (A 0.289 to C 0.166 at K2/16 s, family, reset) because hard targets are a larger share of recalls; the per-stream count
  rises (0.23 to 0.60). A bound written per stream is therefore a statement about the mix.

## 4. Item 4: A2's predictions scored from the hidden side (hidden joined to public)

A2's smoke (Lab 1: seeds 10000-10019, world A, the selection oracle, the frozen medium; arms `a2_learn`, the learner, and `a2_raw`,
a labelled control with the explanation filter off) wrote `predictions-<arm>.csv`. `w3_score.py` joins each prediction to the
hidden alarms (every abnormal observation with its owner incident or background), the time-zero graph and the added edge, and to the
hidden incidents. Definitions (one line each; the script's docstring is the full text):

- *True hidden edge pair*: the predicting service is a cascade's root and the predicted one its partner (`cascade`), or the predicted service
  depends on the predicting one in the true graph at the instant of the alarm and did not in the public graph (`added_edge`). The reverse
  relations and a split brain's peer are reported apart and are not counted as true.
- *Event hit*: the predicting alarm belongs to an incident for which the pair is the true partner relation, and the alarm that follows within the
  band belongs to the same incident.
- *Followed (hidden)*: any abnormal observation at the predicted service in `(t, t + band]`.
- *Chance*: for each prediction, the share of the services A2 could have named at that alarm (unconnected to the predicting one in the public
  graph, either way, and quiet for 2 s) that would have scored the same; 2000 uniform draws for the distribution.

### 4.1 Precision of the learner's predictions

| group (seeds 10000-10019; hidden joined to public) | predictions | on a true hidden-edge pair [90%] | chance (expected share) | event hits | followed, hidden [90%] | followed, A2's reading | chance of following | median lead from the alarm, s (followed) | median lead from the step, s |
|---|---|---|---|---|---|---|---|---|---|
| all predictions | 251 | 21 = 0.084 [0.036, 0.143] | 0.071 | 0 | 60 = 0.239 [0.193, 0.284] | 60 | 0.248 | 0.98 | 0.78 |
| band 0.4 s | 38 | 6 = 0.158 [0.028, 0.370] | 0.104 | 0 | 3 = 0.079 [0.033, 0.111] | 3 | 0.100 | 0.14 | 0.02 |
| band 2 s | 202 | 10 = 0.050 [0.015, 0.094] | 0.058 | 0 | 49 = 0.243 [0.188, 0.294] | 49 | 0.255 | 0.94 | 0.72 |
| band 10 s | 11 | 5 = 0.455 [0.000, 0.833] | 0.182 | 0 | 8 = 0.727 [0.300, 1.000] | 8 | 0.618 | 3.73 | 3.54 |
| pair relation cascade | 12 | 12 = 1.000 [1.000, 1.000] |  | 0 | 5 = 0.417 [0.000, 0.625] | 5 |  | 5.11 | 4.79 |
| pair relation added_edge | 9 | 9 = 1.000 [1.000, 1.000] |  | 0 | 0 = 0.000 [0.000, 0.000] | 0 |  |  |  |
| pair relation cascade_rev | 2 | 0 = 0.000 [0.000, 0.000] |  | 0 | 0 = 0.000 [0.000, 0.000] | 0 |  |  |  |
| pair relation peer | 5 | 0 = 0.000 [0.000, 0.000] |  | 0 | 2 = 0.400 [0.000, 0.500] | 2 |  | 1.14 | 0.86 |
| pair relation none | 223 | 0 = 0.000 [0.000, 0.000] |  | 0 | 53 = 0.238 [0.193, 0.283] | 53 |  | 0.94 | 0.72 |
| first 9 streams | 121 | 12 = 0.099 [0.021, 0.195] | 0.072 | 0 | 29 = 0.240 [0.180, 0.291] | 29 | 0.240 | 0.75 | 0.43 |
| last 9 streams | 130 | 9 = 0.069 [0.020, 0.145] | 0.070 | 0 | 31 = 0.238 [0.169, 0.315] | 31 | 0.255 | 1.20 | 0.94 |
| alarm at 0-200 s | 60 | 2 = 0.033 [0.000, 0.100] | 0.019 | 0 | 11 = 0.183 [0.119, 0.263] | 11 | 0.260 | 0.80 | 0.34 |
| alarm at 200-400 s | 108 | 7 = 0.065 [0.000, 0.169] | 0.074 | 0 | 32 = 0.296 [0.209, 0.362] | 32 | 0.242 | 0.96 | 0.79 |
| alarm at 400-601 s | 83 | 12 = 0.145 [0.062, 0.273] | 0.105 | 0 | 17 = 0.205 [0.146, 0.256] | 17 | 0.247 | 1.04 | 0.89 |

The permutation test (the learner, 251 predictions):

| verdict | observed | mean under permutation of the predicted service | 5th-95th percentile | share of permutations at or above the observed |
|---|---|---|---|---|
| true-edge pair precision | 21/251 | 0.0705 | [0.0518, 0.0916] | 0.177 |
| followed within the band (hidden) | 60/251 | 0.2476 | [0.2151, 0.2829] | 0.688 |

Reading. The predictions are at the chance level on every hidden-side verdict. 21 of 251 name a true hidden-edge pair against 7.1% expected,
and the permutation puts 17.7% of draws at or above 21. The 12 predictions on a `cascade` pair are on three pairs in three streams (7 on 7 -> 8 in seed 10003, a pair held after that
stream's cascade at 264 s; 3 on 3 -> 4 in seed 10010, **made before** that pair's decoy cascade at 403 s; 2 on 3 -> 6 in seed 10017), and the 9 on `added_edge` pairs are four pairs in four streams: the
pair-level verdict counts a pair that is a hidden edge at any instant of the stream, so it cannot tell a learned edge from a coincidence that preceded the cascade, and the
cluster bootstrap over streams ([0.036, 0.143]) is the honest width. None of the 21 was made on an alarm of an incident for which the pair is the partner relation (event hits 0).
The followed predictions have a median lead of 0.98 s from the alarm (0.78 s from the step that made the prediction); they are follows at chance (hidden-side follow rate
0.239, chance 0.248), and A2's own reading (60 of 251) and the hidden-side reading (60 of 251) agree. By stream position (the nine lowest and the nine highest seeds among those with predictions)
0.099 against 0.069 of predictions on a true pair; by time in the stream 0.033, 0.065 and 0.145 for alarms before 200 s, in 200-400 s and after 400 s (the added edge exists from 400 s; the intervals are wide and overlap, and I read
nothing into it).

### 4.2 Coverage of the true partner alarms, and what the explanation filter costs

A true partner alarm is the first alarm at a cascade's partner (hard or decoy: a decoy's early partner alarms are the same draws) or at the first service newly downstream of the
site of an incident the added edge altered. The funnel counts, among the 25 alarms whose lead from the site's first alarm is within the widest band, those that survive each condition of a
trial (the learner opens a trial only on a *counted* first alarm: one that no upstream burst in the previous 0.4 s explains; and a trial is licensed only for a *quiet* partner):

| true partner alarms (seeds 10000-10019) | all | lead within the widest band (10 s) | + the site's alarm was one the learner counted | of those lost: explained by an upstream burst / not a first alarm | + partner quiet (trial licensed) | an edge for the pair was held at some read of the stream | covered (predicted on this alarm, in the window) | covered, loose (any prediction on the pair whose window holds it) |
|---|---|---|---|---|---|---|---|---|
| all true partner alarms | 29 | 25 | 17 | 4 / 4 | 10 | 5 | 0 | 0 |
| added_edge plain | 17 | 17 | 11 | 3 / 3 | 7 | 3 | 0 | 0 |
| cascade decoy contradict | 3 | 3 | 1 | 1 / 1 | 0 | 1 | 0 | 0 |
| cascade hard contradict | 4 | 4 | 4 | 0 / 0 | 3 | 1 | 0 | 0 |
| cascade hard mimic | 5 | 1 | 1 | 0 / 0 | 0 | 0 | 0 | 0 |
| hard cascades and added-edge (no decoys) | 26 | 22 | 16 | 3 / 3 | 10 | 4 | 0 | 0 |

Reading. **Coverage is 0 of 29 over all true partner alarms, and 0 of the 10 for which a trial was licensed.** The lead of a true partner alarm is
short (median 43 ms; 33 to 78 ms for the hard contradict cascades, 4 to 57 ms for the added edge in this smoke) and nearly all are inside the 0.4 s band; the five mimic cascades' partner
alarms arrive 9 to 14 s after the root's (4 of 5 beyond the widest band): no band of A2 can cover them. The explanation filter, as specified,
removes **4 of the 25 coverable alarms** (a further 4 are not first alarms at all: the site was already alarming), so the filter's cost is 16% of the coverable
alarms and the larger loss is the licence: 7 of the 17 counted alarms have a partner that was not quiet. Five true partner alarms had an edge for the pair
held at some read of the stream and none was predicted on this alarm; in the one I traced (seed 10000, 4 -> 9) the edge was held at three reads at 402 to 430 s and was no longer held at the event, 491.9 s; the others I did not trace. The learned edges
themselves:

| learned edges (seed, a, b, band) | edges | on a true hidden-edge pair [90%] | chance (expected share) | cascade | added edge | cascade, reversed | added edge, reversed | split-brain peer | none |
|---|---|---|---|---|---|---|---|---|---|
| all learned edges (seed, a, b, band) | 127 | 7 = 0.055 [0.018, 0.097] | 0.067 | 5 | 2 | 4 | 0 | 6 | 110 |
| band index 0 | 24 | 2 = 0.083 [0.000, 0.182] | 0.098 | 1 | 1 | 2 | 0 | 1 | 19 |
| band index 1 | 97 | 4 = 0.041 [0.011, 0.074] | 0.060 | 3 | 1 | 2 | 0 | 5 | 86 |
| band index 2 | 6 | 1 = 0.167 [0.000, 0.500] | 0.056 | 1 | 0 | 0 | 0 | 0 | 5 |

### 4.3 What A2's learner could ever cover from one earlier sighting of a pair (hidden)

A learner whose evidence is the co-alarm timing of one pair, with nothing carrying across streams (W2), can predict a partner alarm only if that pair occurred earlier in the
same stream, and A2's threshold needs two follows beyond chance:

| seeds (hidden) | true partner events | events | per stream [90%] | with at least one earlier sighting of the pair in the stream | with at least two |
|---|---|---|---|---|---|
| A 10000-10099 | all | 213 | 2.13 [1.88, 2.39] | 35 (0.350 per stream) | 8 (0.080 per stream) |
| A 10000-10099 | cascade hard | 63 | 0.63 [0.47, 0.80] | 13 (0.130 per stream) | 3 (0.030 per stream) |
| A 10000-10099 | cascade decoy | 59 | 0.59 [0.44, 0.75] | 9 (0.090 per stream) | 4 (0.040 per stream) |
| A 10000-10099 | added edge | 91 | 0.91 [0.76, 1.06] | 13 (0.130 per stream) | 1 (0.010 per stream) |
| A 40000-40199 | all | 417 | 2.08 [1.92, 2.25] | 59 (0.295 per stream) | 14 (0.070 per stream) |
| A 40000-40199 | cascade hard | 132 | 0.66 [0.56, 0.76] | 18 (0.090 per stream) | 4 (0.020 per stream) |
| A 40000-40199 | cascade decoy | 109 | 0.55 [0.45, 0.66] | 24 (0.120 per stream) | 9 (0.045 per stream) |
| A 40000-40199 | added edge | 176 | 0.88 [0.79, 0.98] | 17 (0.085 per stream) | 1 (0.005 per stream) |
| C 60000-60099 | all | 314 | 3.14 [2.85, 3.44] | 48 (0.480 per stream) | 5 (0.050 per stream) |
| C 60000-60099 | cascade hard | 152 | 1.52 [1.31, 1.74] | 21 (0.210 per stream) | 2 (0.020 per stream) |
| C 60000-60099 | cascade decoy | 60 | 0.60 [0.47, 0.75] | 10 (0.100 per stream) | 1 (0.010 per stream) |
| C 60000-60099 | added edge | 102 | 1.02 [0.85, 1.20] | 17 (0.170 per stream) | 2 (0.020 per stream) |
| C 70000-70199 | all | 567 | 2.83 [2.64, 3.03] | 97 (0.485 per stream) | 13 (0.065 per stream) |
| C 70000-70199 | cascade hard | 279 | 1.40 [1.24, 1.55] | 46 (0.230 per stream) | 7 (0.035 per stream) |
| C 70000-70199 | cascade decoy | 117 | 0.58 [0.48, 0.69] | 22 (0.110 per stream) | 4 (0.020 per stream) |
| C 70000-70199 | added edge | 171 | 0.85 [0.76, 0.95] | 29 (0.145 per stream) | 2 (0.010 per stream) |
| A 10000-10019 (A2's smoke) | all | 40 | 2.00 [1.45, 2.60] | 9 (0.450 per stream) | 2 (0.100 per stream) |
| A 10000-10019 (A2's smoke) | cascade hard | 9 | 0.45 [0.20, 0.70] | 1 (0.050 per stream) | 0 (0.000 per stream) |
| A 10000-10019 (A2's smoke) | cascade decoy | 12 | 0.60 [0.35, 0.90] | 2 (0.100 per stream) | 1 (0.050 per stream) |
| A 10000-10019 (A2's smoke) | added edge | 19 | 0.95 [0.65, 1.30] | 6 (0.300 per stream) | 1 (0.050 per stream) |

Reading. In world A, 0.295 events per stream have an earlier sighting of the pair in the stream and 0.07 have two; in world C 0.485 and 0.065. **World C does not
raise the ceiling for a learner that needs two sightings (0.065 against 0.070 per stream) and raises the one-sighting ceiling 1.6 times**; the added-edge events do not
rise at all (0.855 against 0.88 per stream; pairs repeat 0.145 against 0.085 per stream). The brief's use of world C as the power source for A2b is therefore not supported by this table.

## 5. Which bounds move

The bounds are those W2 proposed in its section 10.2 and the chief adopted for A1b (`experiments/criteria/a1b.json` is to be fixed after E1; the numbers below are what W3 changes in the
arithmetic), and A2b's preconditions as Lab 1's report states them. Each row says which bound, what it was, what it becomes with world C's power and the new floor, and on what.

| seeds (run, hidden) | hard recurrences | site-keyed reach R (share) | signature-keyed reach R_fresh (share) | fewest unasked-correct with 90% lower bound above 0.075 (share) | half-width of a paired margin (10 of 83 disagree) | R, share of bill | half-width of R's interval | 0.4 x signature ceiling | 0.73 x signature ceiling |
|---|---|---|---|---|---|---|---|---|---|
| 10000-10099 | 44 | 18 (0.409) | 16 (0.364) | 8 (0.182) | 0.0861 | 0.0901 | 0.0337 | 0.145 | 0.265 |
| 40000-40199 | 83 | 41 (0.494) | 31 (0.373) | 12 (0.145) | 0.0627 | 0.0849 | 0.0221 | 0.149 | 0.273 |
| 60000-60099 | 145 | 63 (0.434) | 53 (0.366) | 18 (0.124) | 0.0474 | 0.0875 | 0.0195 | 0.146 | 0.267 |
| 70000-70199 | 259 | 138 (0.533) | 115 (0.444) | 28 (0.108) | 0.0355 | 0.1004 | 0.0147 | 0.178 | 0.324 |

| Bound (W2 section 10.2; Lab 1's A2 report) | Was | Moves to, and by how much | Because |
|---|---|---|---|
| **Unasked-correct clause**, the count: at least 0.15 of the hard recurrences, 90% lower bound above 0.075 | 12 of 83 (0.145) | **28 of 259 (0.108)** if the lower-bound rule is kept; if the 0.15 is kept (39 of 259) its lower bound is **0.114** instead of 0.081, so the lower bound can be fixed at about 0.11 | the counts triple; the normal-approximation half-width falls from 0.064 to 0.037 |
| The same, as a share of the ceilings | 0.15 = 40% of the signature-keyed ceiling 0.373 (31% of the site-keyed 0.494) | 0.15 is 34% of world C's signature-keyed ceiling **0.444** and 28% of the site-keyed **0.533**; 40% of 0.444 is **0.178** (+0.029) | the arm declares about half the hard incidents correctly in both worlds but more of the *recurrences'* templates were answered before the repeat in C (R is 53% of the recurrences against 49%) |
| **Paired margin against the record rung** (+0.10 of the hard recurrences, lower bound above 0) | half-width 0.063 if 10 of 83 recurrences are ones where two arms disagree; "unresolvable under 0.06" | half-width **0.036** at the same disagreement rate; a margin of +0.05 is now resolvable | 259 recurrences; the half-width scales as 1/sqrt(N) |
| The pre-accepted outcome "the record rung captures the lever" if its share exceeds 73% of the signature-keyed ceiling | 0.27 | **0.32** (+0.05) | the ceiling's share moves from 0.373 to 0.444 |
| **Cost clause** ("cheaper than the record rung"; the ceiling's interval) | the site-keyed ceiling 8.5% [6.4, 10.8] of the bill, +/-2.2 points; margins under about 3 points unresolvable | **10.0% [8.6, 11.5], +/-1.5 points**; margins above about 2 points resolvable (the interval scales by 1.5/2.2) | the same bill share on three times the hard incidents |
| **Collision clause (a)**, the share: of recalls with a right source, at most 0.20 wrong | W2's floor for a phase-1 site-and-class key 0.147 (so the bound admitted "a competent site-keyed arm with a small margin") | For keys that wait for the rule-breaking evidence (hidden side, own evidence, 16 s, stream reset): **site-keyed 0.087 [0.040, 0.143] (A), 0.054 [0.033, 0.077] (C): the bound is met with room. Family-keyed 0.289 [0.224, 0.354] (A), 0.166 [0.140, 0.193] (C): not met in A (lower end above 0.20), met in C; met in both once the decoy's resolution enters the key** (K4: 0.114, 0.088). **Carried across streams: 0.28 to 0.87 under every key tried (0.33 to 0.64 for E1's family-keyed forms) and under aggregation: no key tried meets the bound across streams in either world** | the decoys that no abnormal evidence separates (slow-leak and contradict-mode decoys); the share also depends on the hard-to-decoy ratio, which the mix changed |
| **Collision clause (a)**, the count: at most 0.25 wrong per stream | W2's floor 0.19 per stream (A) | A: 0.04 (site, K2/16 s), 0.23 (family, K2/16 s). **C: 0.08 and 0.60**: the same keys on three times the hard incidents. As a rate per hard incident the family, K2/16 s count is 9.0 per 100 hard incidents in A and 7.9 in C; **0.25 per stream corresponds to 9.8 per 100 hard incidents at world A's 2.56 per stream, and 0.74 per stream at world C's 7.58** | the per-stream count scales with the hard share; the clause should be stated per hard incident if it is to hold in both worlds |
| **Stale-error clause (b)**, inherited error: no absolute bound, paired against the record rung | the stored answer is wrong for 40% [28, 51] (site-keyed sources, A) | 36% [30, 41] (site-keyed, C, 215 sources) and 38% [34, 42] (family-keyed, 558 sources) | the same reasoner; the intervals halve |
| Staleness by regime: stale-by-construction hard recurrences, reported separately | 12 of 83 (14.5% [8.4, 21.1]) | **30 of 259 (11.6% [8.0, 15.4])** | enough for an aggregate over streams, still not for a per-stream recovery time |
| **A2b precondition** (Lab 1: learned-edge precision above W3's permutation level in the band A2b uses) | not yet measured | **A2's learner on the smoke: 7 of 127 learned edges on a true pair (5.5% [1.8, 9.7]) against 6.7% at chance; 21 of 251 predictions (8.4%) against 7.1%, permutation p 0.18; 0 event hits; 0 of 29 true partner alarms covered. The precondition is not met on this smoke in any band** | section 4 |
| **A2b power** (Lab 1: "hundreds of streams or W3's world C") | about one alarm over an added edge per stream | **World C does not buy A2b power**: events with two earlier sightings of the pair, the ceiling for a learner that needs two follows, are 0.070 per stream in A and 0.065 in C (15 streams per event in either); with one earlier sighting 0.295 and 0.485; added-edge events 0.88 and 0.855 per stream. About 1,500 streams of either world give 100 events with two earlier sightings | the mix raises hard cascades 2.1 times, not the added edge, and a pair repeats rarely |

What the new power does *not* move. The reach of the recurrence lever (R is 10.0% of the bill: 8.5% in A within the intervals), the staleness of the family law carried across streams (still 99.98% reachable
and not reachable by any invariant key the public side can build before the deadline without exceeding the bound), and the oracle arm's quality on hard incidents (0.50 against 0.48). The extra hard incidents
make A1b's within-stream question *resolvable*, not *larger*.

## 6. Reproduction

`experiments/exploration/scripts/w3_run.sh` lists the order (`w3_hidden.sh`, `w3_manifests.py` and `scripts/run-driver.sh`, `w3_laws.py`, `w3_ceiling.sh`,
`w3_bounds.py`, `w3_pairs.py`, `w3_floor.py`, `w3_e1join.py`, `w3_score.py`, `w3_provenance.py`, `w3_assemble.py`). Raw directories are under `artifacts/runs/w3/`
(git-ignored; kept until the chief has verified them): `hidden/{c-tune,c-heldout,a-tune,a-heldout,a-a2}` (tables, floor features, `usage.json`; `a-a2` also the
alarms and the graph for A2), `w3-ctune-b5-rho0.7` and `w3-cheldout-b5-rho0.7` (the ceiling's runs with the ledger, `usage.json`), `_manifests/`. File hashes are in
`w3-provenance.csv`. A2 is scored by `w3_score.py --predictions <A2 run>/predictions-a2_learn.csv --first-alarms <A2 run>/first-alarms-a2_learn.csv --edges <A2 run>/edges-a2_learn.csv --hidden artifacts/runs/w3/hidden/a-a2 --arm a2_learn` (the same with `a2_raw`). The hidden world-A tables are identical to W2's; W2's kept runs, E1's held-out run and A2's smoke are read, never written, from `/home/user/gordian/artifacts/runs/`.

## 7. Uncertainty, what would change the conclusions, and what the chief should examine

**Least sure of.**

1. **The floor is the world's, computed from each incident's own evidence; it is not a prediction of what an arm reaches.** E1's family-keyed carried arm recalled 372 plain incidents; the floor has 125, and 328 of E1's
   372 are on plain incidents whose own evidence the public checker finds consistent (section 3.4). The gap is evidence from elsewhere in the anomaly. I measured the count, not the cause: I did not inspect the arm's
   anomaly evidence, so "contamination by other incidents' and background symptoms" is the reading, untested. The collision clause as A1b states it will be read on arms, so it is the arm's number that
   matters; the floor says which part of it the world already accounts for.
2. **The key families are mine.** `K1` to `K4` and E1's keys are three or four of many invariant keys; a richer key (counts, orders, inter-arrival gaps) could separate decoys earlier or recall less. I claim
   a floor for these keys, not over all invariant keys. The one structural statement I make beyond them (no abnormal evidence in phase 2 of a contradict presentation or of the leak) is read from
   `HIDDEN-DESIGN.md` section 4.2 and is consistent with the breakdown of the wrong recalls (section 3.3), not independently tested.
3. **The gate's time.** The gate is the delivery instant of the observation that completes a contradictory prefix; E1's rung checks at most once per 0.5 s after evidence arrives and reads the key one second after
   its first contradictory check, so an incident near a band boundary of the gate delay (1, 3, 6, 10 s) may fall in another band in the arm. The key agreement with E1's own same-stream recalls (11 of 11, 22 of 23,
   8 of 9, 4 of 5, small numbers) is the only check.
4. **The mix choice.** Decoys held at 100 per mille turns the hard-to-decoy ratio from 1:1 to 3:1 and lowers every collision *share* that involves decoys. With decoys scaled with plain the shares would sit between
   the two worlds'. Not run. The family mix of the hard tier also moved (section 2.2), by a mechanism I read from the code and did not count.
5. **Right sources only.** The floor takes every stored answer to be the truth; the reasoner is right about half the time on hard incidents (W2 section 7.6: 36-54% of sources wrong), so an arm's wrong recalls include
   inherited errors that are in neither this floor nor the collision clause as split by W2.
6. **A2's verdict rests on 20 streams, 251 predictions and 29 true partner alarms.** Zero covered and zero event hits are robust to my definitions (a looser reading, any prediction on the pair whose window holds
   the partner alarm, is also 0 of 29), but "precision at chance" has an interval of [3.6, 14.3]% and a handful of pairs behind it. The smoke is world A at seeds 10000-10019, not tuned.
7. **The permutation chance** conditions on a candidate being unconnected to the predicting service and quiet for 2 s, and counts any abnormal observation at the candidate as a follow; A2's own `permutation_chance` counts
   only counted first alarms, so the two follow-chance numbers differ (A2's report: 0.052, 0.222, 0.533 by band; here 0.100, 0.255, 0.618).
8. **`R` (reachable) is conditional on the arm** (W2's least-sure item 1): the arm declares about half the hard incidents correctly in both worlds.
9. **Seeds and streams.** One tuning-like and one held-out range per world; no run on world C with another selection delay or reasoner; the order of streams in the carried tables is seed order.

**What would change the conclusions.** A hard-to-decoy ratio of 1:1 in world C (the other reading of the mix): the family-keyed shares would rise toward world A's. An invariant key that holds a feature I do
not (the temporal pattern of heartbeat flaps, say) and separates a decoy from its hard family before it resolves: the K2 floor would fall. Evidence that E1's excess plain recalls come from something other than
attached symptoms of other incidents (for example a defect in the arm's attach rule): the floor-to-arm gap would be an artefact of the instrument and not of the world. A trace of A2 on a world with more
pair repeats: the coverage would be testable.

**What the chief should examine most carefully.**

- `floor.rs`: the attach rule I assumed (the site and its public dependents; an unconnected service starts its own anomaly), the gate, and that `key_of`'s features are E1's. The check against E1's recalls is
  `w3-floor-e1-join.csv`.
- `simulate` in `w3_floor.py`: an incident's own answer is excluded from its own recall (for cutoffs after 19 s every key would otherwise recall itself: my first run had this defect, 243 of 243 hard incidents
  "recalled right" at 20 s, found by reading the table, fixed before any number was used; the unit tests pin it).
- The join in `w3_score.py`: the predicting alarm is checked to be the stream's alarm at that instant and service (an assertion that stops the script), owner incidents come from the `alarms.csv` the example wrote,
  and the verdicts "true hidden edge" and "event hit" are the definitions of section 4. The tests (8, derived by hand) and the 14 caught mutants are the evidence it works as specified.
- The choice of the mix (section 1.3) and of the oracle arm (the arm of W2's item 6).
- Whether the brief's "about 120 reachable events" is `R` (138) or `R_world` (259) or something else; I used `R`.

**The improved question.** A1b's within-stream question is no longer short of power, and its collision clause has a floor: a site-keyed key from the rule-breaking evidence is clean (0.09), a family-keyed one is not (0.29) unless it
waits for the decoy to resolve (0.11 at 16 s) and nothing invariant is clean across streams (0.28 to 0.87). The question that follows is whether the arm that would carry the family law across streams can learn, from
its own history, what the world's decoys and altered plain incidents look like (a teacher that includes non-hard answers), since a table of hard answers alone cannot (aggregation did not move the floor). A2's question
is whether any evidence of one pair per stream can teach an edge (0.07 events per stream with two earlier sightings, in either world) before it asks whether the edge is used.

**Smallest high-information next steps.** (i) E1's record rung on world C's held-out range under the same arm and keys, to read the arm's collisions against this floor where hard recurrences are three times as many.
(ii) The same floor with a teacher that includes decoys and plain incidents as the memory's own outcomes (what a memory that records every notice and its eventual diagnosis would hold): the vote result suggests the
floor then moves; this unit did not run it. (iii) A1b's criterion fixed with the collision clause per hard incident and the K2/K4 floors above, before any A1b run. (iv) For A2, a world where the same pair recurs
(a persistent hidden edge), since this one cannot teach one.

## 8. Gates and process record

Before every cargo command: `df -h /home/user` (9.3 GB free at the first build, 7.2 GB after the test build; the floor of 6 GB was never reached) and `pgrep -x gordian-run`.
**Waits and process record.** The first preflight (the first command of the unit) listed a `gordian-run` process (pid 11999); it was gone by the next check, a few minutes later, and no cargo command was issued
while it was listed (the first build started after a check that printed nothing). Every later check (before the release build, the run driver, the two clippy runs, the debug rebuild and the test build) found no
`gordian-run`; **no build or run waited**. The world-C runs were started by a script that polls for `cargo`, `rustc` and `gordian-run` every 30 s and records each wait: it waited zero times
(`runall.log`, scratch). Another process's `cargo test --workspace` was running when my release build finished; it had ended when the runs started. I never killed another lab's process and never passed
`--allow-unisolated`. The Python analyses (the floor takes 15 minutes) ran under the cgroup runner on cores 0-2 (the chief's gate builds may have run beside them; not recorded).

Gates, run under `scripts/cgroup-run.sh --name world-c-build --cpus 0-2 --memory 3G`, `CARGO_BUILD_JOBS=3 CARGO_PROFILE_DEV_DEBUG=0`, on the tree as committed (the Rust sources are unchanged since the
`cargo fmt` that followed the first commit; the outputs of the rebuilt example are byte-identical to the unformatted one's on the 20 streams of A2's range):
`cargo fmt --all -- --check` exit 0; `cargo clippy --workspace --all-targets -- -D warnings` exit 0 (and `cargo clippy -p gordian-stream --features reveal-hidden-state --all-targets -- -D warnings`,
which lints the `laws` example the workspace run skips for want of the feature, exit 0); `cargo test --workspace` exit 0, **1037 passed, 0 failed, 14 ignored** (269 s, peak 2.0 GB, no OOM kill);
`scripts/check-no-oracle.sh` prints `check-no-oracle: ok`. The Python tests (`w3_score_test.py`: 8; `w3_floor_test.py`: 11) pass; they are not part of the workspace gates.
No source under `crates/*/src` changed: nothing reaches an arm and the harness is the one W2 and E1 used.

**Reproducibility of the floor.** `w3_floor.py` (17.8 minutes under the runner, peak 0.25 GB, exit 0) was run in full twice for the ladder and the E1 forms, the second time after the memory gained its aggregating rule and its breakdowns:
`w3-floor-ladder.csv` (2304 rows) and `w3-floor-e1.csv` (96 rows) are `cmp`-identical across the two runs. The `streak` part was added after that run and ran alone (`--parts streak`).
