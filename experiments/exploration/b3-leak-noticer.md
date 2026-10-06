# B3: a public leak noticer and a splitting noticer

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation). Lab 2, unit B3 of
`docs/lab-queue.md`, branch `leak-noticer`. The table makes no claim about which noticer is better: it is B2's
table extended, and it is the labelled supplementary comparison for M2 (M2's held-out runs had already been
played when this unit reported) and the comparator for M3 and EXP-101. Code: `noticer_ramp.rs`,
`noticer_split.rs` (`crates/gordian-run/src/stream/arms/`); scripts: `scripts/b3_*.py` and `scripts/b3_run.sh`
(B1's and B2's are imported, not edited). Files: `b3-*.csv`, `b3-selected.json`, `b3-noticers-table.md`
(generated). The unit was started by a predecessor whose container restarted; this report covers both halves
and says which is which.

## The result in one paragraph

On the 200 held-out streams at b = 5, rho = 0.7 (seeds 20000-20199; 372 hard non-leak incidents, 139 slow
leaks), **a public noticer that reads counter values notices the slow leak within the background budget**:
`RampNoticer` (gap 1.6 s, step <= 10, drop <= 4, >= 5 readings, rise >= 15, chosen on the tuning streams)
notices **0.986 [0.967, 1.000]** of the leaks (137 of 139; the re-anchor 0.460, a difference of +0.525 [+0.455,
+0.594]) and anchors **0.971 [0.945, 0.993]** of them at their first observation (135 of 139, 135 at an offset
of exactly zero; every abnormal-only noticer scores 0.000, M2's medium 0.554), with a median latency of 5.5 s
(the re-anchor 14.2 s, the medium 9.8 s), at 6.37 background notices per stream over the re-anchor (bound 6.82)
or 3.58 over the rung at z = 3. It changes no hard non-leak anchor-correct count (paired difference +0.000
[+0.000, +0.000]), costs +0.12 s per stream (+0.40 reasoner calls: the leaks' notices are asked about), and it
also notices the look-alike decoys: 0.948 of the 541 decoy incidents over the re-anchor (0.756 for the
re-anchor), because a decoy's counter ramps like a leak's. `SplitNoticer` (over the re-anchor: gap 3 s, burst
>= 3) raises hard non-leak anchor-correct from 0.952 to **0.973 [0.959, 0.986]** (+0.022 [+0.010, +0.034],
8 incidents gained, none lost) and **notices 2 of the 8 never-noticed incidents** (both anchor-correct, both
cascades; 6 stay never noticed, 3 of them split-brain incidents no row notices); over the rung at z = 3 it
gains 2 anchor-correct incidents (+0.005 [+0.000, +0.012]). The two pieces compose additively (interaction
0.000 on every measure), so ramp + split over the re-anchor is 0.973 anchor-correct, 0.986 leak noticed, 0.971
leak anchor-correct, 6.24 background, strict precision 0.693. Against M2's medium at 100 ms (same streams,
paired, medium minus ramp + split over the re-anchor) leak noticed is +0.007 [-0.013, +0.029]
(indistinguishable), leak anchor-correct -0.417 [-0.493, -0.345], hard anchor-correct +0.019 [+0.005, +0.033],
and strict precision 0.50 against 0.69. The acceptance clause (a public row at leak noticed >= 0.660 within the budget) is
triggered; M2's held-out runs had started, so this report is the supplementary comparison it names.

## What was built

Both noticers wrap a base noticer (the rung's or the later re-anchor) and add anomalies to the base's own
set, so that the attach rule, the score, the retirement and everything downstream treat them as the base's.
The spelling is `{"noticer": "composed", "base": {...}, "ramp"?: {...}, "split"?: {...}}`, one more entry of the
manifest's `noticers` (no schema change); `HARNESS.md` says it. No evaluator change: the evaluator's rules
N1 to N16 are B1's and B2's, so no new fixtures or mutation checks of the evaluator were due.

- **`RampNoticer`** (`noticer_ramp.rs`): chains of counter readings per (service, counter name), every reading
  whatever the public rules' verdict; a reading continues a chain within `gap_ns`, at most `max_step` above and
  `max_drop` below its level; a chain of `min_readings` readings whose level is `min_rise` above its first
  reading is a ramp, noticed once, anchored on the chain's first reading, about the service of the key. It
  reads no counter name, no label, no family.
- **`SplitNoticer`** (`noticer_split.rs`): an anomaly's attached observations form clusters (each at most the
  rung's 0.4 s after the one before); a later cluster with at least `min_burst` observations about services
  other than the anomaly's site, more than `gap_ns` after the end of the latest earlier burst, moves out as a
  new candidate anomaly anchored on its first foreign observation; the base decides whether it is noticed.
- Their readings of the brief's words are stated in each file's documentation before any tuning (commit
  `2a1854b`) and listed under "Readings and deviations".

## Byte identity: the gate

R6's held-out manifest (b = 5, rho = 0.7), replayed with this branch's binary under its own run id, writes
`results.csv` and `incidents.csv` with the SHA-256 recorded for R6 for all 62 arms: **62 of 62**
(`b3-regression.csv`, played by the predecessor at `99c48b4`; the run directory was deleted afterwards, as the
brief allowed). **Rerun at the end** (`xcheck4-r6-heldout-b5-rho0.7`, driver exit 0, 161 s, one poll waiting for another lab's
build): **62 of 62 again** (`b3-regression-final.csv`), with the same release binary, which I did not rebuild
(reading 9), so the rerun is the check by running that this binary reproduces R6's hashes.

## Readings, tuning and the choices

Tuning is on seeds 10000-10099 at b = 5, rho = 0.7 with the selection oracle at R5's 16 s and the rung's
retirement, as B1's and B2's; every rule was fixed in `b3_common.py` before the run it governs (commit
`c39ca73`); no amendment was made to any rule after any run (`b3_common.py`'s amendment list is empty). The
background budget (6.82) is applied literally to the whole noticer's notices on background on the tuning
streams. Nothing was chosen on any held-out stream.

1. **Stage R (ramp, 216 configurations over the rung at z = 3, `b3-tuning-ramp.csv`).** Objective: leak noticed
   share, within the budget; then leak anchor-correct, fewer background notices, and the more conservative
   parameter. **The objective saturated:** 176 of the 217 rows (216 + the rung) notice every one of the 44
   tuning leaks and 175 tie at the top within the budget, so the choice was made by the tie keys named in
   advance. Chosen **gap 1.6 s, step <= 10, drop <= 4, >= 5 readings, rise >= 15**: tuning leak noticed 1.000,
   leak anchor-correct 1.000, 3.70 background per stream (the rung 3.99). Held-out 0.986, 0.971, 3.58: no
   overfit visible, and the held-out sensitivity rows show how little the top plateau depends on any one value
   (below). The ramp's parameters are the same in every row that has a ramp: they were not re-tuned per base.
2. **Stage S (split, 15 configurations over the rung and over the re-anchor, `b3-tuning-split.csv`).** Over the
   rung z = 3 **all 15 configurations tie** at anchor-correct 0.854 (zero effect on the tuning streams; the
   tie rule chose gap 8 s, burst >= 4, the largest of each); over the re-anchor gap 3 s, burst >= 3 (tuning
   0.965 against the re-anchor's 0.950, 6.41 background per stream). Held-out the same choices give +0.022
   over the re-anchor and +0.005 over the rung. The held-out best of the grid over the re-anchor is burst >= 2
   (0.978), not the tuned burst >= 3 (0.973); the choice was not revised.
3. **Stage D (one hold delay per table row, `b3-tuning-delays.csv`).** R5's rule gives **16 s for every row**
   (the same delay R5 fixed), so the "tuned delay with hold" reading of quality uses 16 s throughout, and the
   two readings differ only by the hold.


## The table

All rows: 200 held-out streams, the selection oracle at R5's fixed 16 s delay with the rung's retirement for
every column but `quality held`, which is the arm with `hold_until_asked` at the row's own tuned delay (16 s).
Brackets are 90% percentile cluster-bootstrap intervals over whole streams (10,000 resamples, B2's seed).
"noticed" and "anchor-correct" are over the 372 hard non-leak incidents; leak columns over the 139 leaks;
"precision" is N16's, "strict precision" the anchor-and-site-correct notices over all notices, "background" the
notices anchored on background per stream (the budget's measure), "quality" is the share of hard non-leak
incidents the selection oracle's reasoner gets right (leak quality is in `b3-noticers-table.md`'s second
table). Cost is the harness's modelled cost per stream; **no noticer's own work is billed in any row**
(HARNESS.md), so the cost column is the components, the shared rule and the reasoner, and the ramp's own work
is counted separately below.

| row | noticed | anchor-correct | leak noticed | leak anchor-correct | background / stream | precision | strict precision | notices / incident | quality fixed | quality held | cost s / stream |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Rung z=3 | 0.917 [0.893, 0.939] | 0.890 [0.863, 0.916] | 0.460 [0.384, 0.536] | 0.000 [0.000, 0.000] | 4.00 [3.76, 4.24] | 0.855 [0.847, 0.863] | 0.748 [0.737, 0.759] | 0.89 [0.87, 0.90] | 0.489 [0.448, 0.532] | 0.497 [0.456, 0.539] | 0.664 [0.604, 0.728] |
| Rung z=2 | 0.946 [0.927, 0.964] | 0.914 [0.890, 0.937] | 0.460 [0.387, 0.536] | 0.000 [0.000, 0.000] | 9.03 [8.63, 9.45] | 0.740 [0.729, 0.751] | 0.613 [0.600, 0.624] | 0.97 [0.96, 0.98] | 0.527 [0.482, 0.572] | 0.559 [0.515, 0.603] | 0.646 [0.589, 0.706] |
| Re-anchor (z=2) | 0.978 [0.966, 0.990] | 0.952 [0.932, 0.970] | 0.460 [0.390, 0.533] | 0.000 [0.000, 0.000] | 6.82 [6.47, 7.18] | 0.802 [0.792, 0.812] | 0.672 [0.660, 0.683] | 1.04 [1.03, 1.05] | 0.551 [0.507, 0.597] | 0.591 [0.548, 0.636] | 0.670 [0.614, 0.730] |
| Ramp / rung z=3 | 0.925 [0.902, 0.946] | 0.890 [0.863, 0.916] | 0.986 [0.967, 1.000] | 0.971 [0.945, 0.993] | 3.58 [3.35, 3.81] | 0.877 [0.869, 0.884] | 0.758 [0.748, 0.768] | 0.96 [0.95, 0.97] | 0.489 [0.447, 0.532] | 0.500 [0.458, 0.542] | 0.786 [0.721, 0.852] |
| Split / rung z=3 | 0.919 [0.896, 0.942] | 0.895 [0.869, 0.920] | 0.460 [0.384, 0.536] | 0.000 [0.000, 0.000] | 3.91 [3.67, 4.16] | 0.858 [0.850, 0.866] | 0.754 [0.743, 0.764] | 0.89 [0.88, 0.90] | 0.492 [0.450, 0.535] | 0.500 [0.459, 0.542] | 0.666 [0.605, 0.729] |
| Ramp+split / rung z=3 | 0.927 [0.905, 0.948] | 0.895 [0.869, 0.920] | 0.986 [0.967, 1.000] | 0.971 [0.945, 0.993] | 3.50 [3.27, 3.73] | 0.880 [0.872, 0.887] | 0.763 [0.753, 0.773] | 0.96 [0.95, 0.97] | 0.492 [0.449, 0.535] | 0.503 [0.461, 0.545] | 0.787 [0.722, 0.853] |
| Ramp / re-anchor | 0.981 [0.969, 0.992] | 0.952 [0.932, 0.970] | 0.986 [0.967, 1.000] | 0.971 [0.945, 0.993] | 6.37 [6.02, 6.73] | 0.823 [0.813, 0.832] | 0.685 [0.674, 0.696] | 1.11 [1.10, 1.12] | 0.554 [0.509, 0.600] | 0.594 [0.550, 0.640] | 0.791 [0.728, 0.856] |
| Split / re-anchor | 0.984 [0.973, 0.994] | 0.973 [0.959, 0.986] | 0.460 [0.390, 0.533] | 0.000 [0.000, 0.000] | 6.70 [6.33, 7.07] | 0.806 [0.796, 0.816] | 0.680 [0.668, 0.692] | 1.05 [1.04, 1.06] | 0.559 [0.515, 0.605] | 0.599 [0.556, 0.644] | 0.674 [0.617, 0.733] |
| Ramp+split / re-anchor | 0.987 [0.976, 0.995] | 0.973 [0.959, 0.986] | 0.986 [0.967, 1.000] | 0.971 [0.945, 0.993] | 6.24 [5.88, 6.61] | 0.827 [0.816, 0.836] | 0.693 [0.682, 0.704] | 1.12 [1.11, 1.13] | 0.562 [0.517, 0.608] | 0.602 [0.558, 0.647] | 0.795 [0.731, 0.859] |
| Re-anchor z=3 | 0.957 [0.939, 0.974] | 0.938 [0.915, 0.960] | 0.460 [0.389, 0.530] | 0.000 [0.000, 0.000] | 2.29 [2.12, 2.48] | 0.917 [0.910, 0.923] | 0.809 [0.799, 0.819] | 0.95 [0.94, 0.96] | 0.516 [0.474, 0.558] | 0.524 [0.483, 0.565] | 0.683 [0.624, 0.745] |
| Ramp+split / re-anchor z=3 | 0.970 [0.954, 0.985] | 0.954 [0.934, 0.973] | 0.986 [0.967, 1.000] | 0.971 [0.945, 0.993] | 1.73 [1.57, 1.90] | 0.941 [0.935, 0.946] | 0.826 [0.817, 0.835] | 1.03 [1.02, 1.04] | 0.524 [0.481, 0.567] | 0.535 [0.493, 0.578] | 0.816 [0.749, 0.882] |


Row names: "X / base" is noticer X composed over that base ("rung z=3" is the rung's default threshold;
"re-anchor" is B2's row, z = 2; "re-anchor z=3" is the same re-anchor at the rung's default threshold, which
leaves 4.5 notices of budget for a composition). The first three rows are B2's rows rerun in this run: the
rung at z = 3 and z = 2 are R10's arms byte for byte and the re-anchor is B2's arm byte for byte
(`b3-vs-earlier.csv`, notice events included). The precise parameters of every row are in `b3-selected.json`
and `b3-noticers-table.md`. Every row but the rung at z = 2 is within the budget of 6.82; the rung at z = 2 is
B1's comparator row, shown for the record. Plain noticed, site-correct, notices by tier, leak quality and
calls are in `b3-noticers-table.md` ("Beside it"); the latency per row is `b3-noticers-latency.csv`.

### Paired differences against the re-anchor (same resamples)

| row minus the re-anchor | noticed | anchor-correct | leak noticed | leak anchor-correct | background / stream | strict precision | notices / incident | quality fixed | quality held | cost s / stream |
|---|---|---|---|---|---|---|---|---|---|---|
| Rung z=3 | -0.062 [-0.085, -0.040] | -0.062 [-0.086, -0.039] | +0.000 [-0.069, +0.068] | +0.000 [+0.000, +0.000] | -2.825 [-3.215, -2.445] | +0.076 [+0.065, +0.088] | -0.155 [-0.166, -0.144] | -0.062 [-0.096, -0.029] | -0.094 [-0.131, -0.059] | -0.006 [-0.031, +0.018] |
| Rung z=2 | -0.032 [-0.048, -0.017] | -0.038 [-0.055, -0.021] | +0.000 [-0.047, +0.046] | +0.000 [+0.000, +0.000] | +2.210 [+2.010, +2.415] | -0.059 [-0.065, -0.054] | -0.073 [-0.081, -0.066] | -0.024 [-0.040, -0.009] | -0.032 [-0.051, -0.014] | -0.024 [-0.041, -0.008] |
| Ramp / rung z=3 | -0.054 [-0.076, -0.032] | -0.062 [-0.086, -0.039] | +0.525 [+0.455, +0.594] | +0.971 [+0.945, +0.993] | -3.240 [-3.640, -2.855] | +0.086 [+0.074, +0.099] | -0.083 [-0.095, -0.070] | -0.062 [-0.097, -0.028] | -0.091 [-0.128, -0.056] | +0.115 [+0.084, +0.146] |
| Split / rung z=3 | -0.059 [-0.083, -0.037] | -0.056 [-0.082, -0.032] | +0.000 [-0.069, +0.068] | +0.000 [+0.000, +0.000] | -2.910 [-3.305, -2.530] | +0.082 [+0.070, +0.093] | -0.151 [-0.162, -0.139] | -0.059 [-0.093, -0.026] | -0.091 [-0.128, -0.056] | -0.004 [-0.030, +0.019] |
| Ramp+split / rung z=3 | -0.051 [-0.074, -0.029] | -0.056 [-0.082, -0.032] | +0.525 [+0.455, +0.594] | +0.971 [+0.945, +0.993] | -3.320 [-3.720, -2.935] | +0.092 [+0.079, +0.104] | -0.079 [-0.092, -0.066] | -0.059 [-0.094, -0.025] | -0.089 [-0.126, -0.053] | +0.117 [+0.085, +0.148] |
| Ramp / re-anchor | +0.003 [+0.000, +0.008] | +0.000 [+0.000, +0.000] | +0.525 [+0.455, +0.594] | +0.971 [+0.945, +0.993] | -0.450 [-0.545, -0.360] | +0.013 [+0.009, +0.018] | +0.070 [+0.064, +0.077] | +0.003 [+0.000, +0.008] | +0.003 [+0.000, +0.008] | +0.121 [+0.098, +0.146] |
| Split / re-anchor | +0.005 [+0.000, +0.012] | +0.022 [+0.010, +0.034] | +0.000 [+0.000, +0.000] | +0.000 [+0.000, +0.000] | -0.125 [-0.180, -0.075] | +0.008 [+0.006, +0.011] | +0.008 [+0.005, +0.010] | +0.008 [+0.000, +0.016] | +0.008 [+0.000, +0.016] | +0.003 [-0.000, +0.008] |
| Ramp+split / re-anchor | +0.008 [+0.002, +0.016] | +0.022 [+0.010, +0.034] | +0.525 [+0.455, +0.594] | +0.971 [+0.945, +0.993] | -0.580 [-0.690, -0.475] | +0.021 [+0.016, +0.026] | +0.078 [+0.071, +0.085] | +0.011 [+0.003, +0.020] | +0.011 [+0.003, +0.020] | +0.124 [+0.101, +0.149] |
| Re-anchor z=3 | -0.022 [-0.038, -0.006] | -0.013 [-0.032, +0.003] | +0.000 [-0.056, +0.054] | +0.000 [+0.000, +0.000] | -4.525 [-4.875, -4.185] | +0.137 [+0.127, +0.148] | -0.092 [-0.101, -0.082] | -0.035 [-0.067, -0.003] | -0.067 [-0.103, -0.033] | +0.013 [-0.004, +0.029] |
| Ramp+split / re-anchor z=3 | -0.008 [-0.024, +0.008] | +0.003 [-0.018, +0.022] | +0.525 [+0.455, +0.594] | +0.971 [+0.945, +0.993] | -5.090 [-5.445, -4.740] | +0.154 [+0.143, +0.165] | -0.012 [-0.023, -0.001] | -0.027 [-0.060, +0.005] | -0.056 [-0.093, -0.022] | +0.145 [+0.118, +0.173] |


The leak columns for the rows without a ramp are exactly zero because the ramp is the only piece that touches
the leak (the split's moves never produce a notice anchored on a leak observation: `b3-leak-offsets.csv`).

### Each piece against its own base, and the composition

(`b3-paired-base.csv`; the pieces' effects without the base's.) Over the re-anchor: the ramp changes hard
non-leak noticed by +0.003 [+0.000, +0.008] and anchor-correct by +0.000 [+0.000, +0.000], leak noticed by
+0.525 [+0.455, +0.594], leak anchor-correct by +0.971 [+0.945, +0.993], background by -0.45 [-0.55, -0.36]
per stream, notices per incident by +0.070 [+0.064, +0.077], cost by +0.121 s [+0.098, +0.146] and reasoner
calls by +0.40 per stream; the split changes anchor-correct by +0.022 [+0.010, +0.034], noticed by +0.005
[+0.000, +0.012], background by -0.125 [-0.180, -0.075], and cost by +0.003 s [-0.000, +0.008]. The
interaction of the two (both minus ramp minus split plus base) is 0.000 on noticed, anchor-correct, leak
noticed, leak anchor-correct, strict precision, notices per incident, quality and cost, and -0.005 to -0.010
notices per stream (a rounding of 1 notice in 200 streams): over the rung the same. Over the rung at z = 3
the ramp's own effect is the same on the leak columns and +0.008 [+0.002, +0.016] on hard noticed. **Quality
(fixed or held) does not move for the ramp:** +0.000 [-0.006, +0.006] over the rung, +0.003 [+0.000, +0.008]
over the re-anchor, because the quality columns are over hard non-leak incidents; the ramp's effect on the
leaks is in leak quality (0.187 -> 0.820 over the rung, 0.223 -> 0.820 over the re-anchor, both readings).

### The supplementary comparison with M2's medium (same streams, paired)

M2's held-out run (`artifacts/runs/m2/m2-heldout-b5-rho0.7` of the main checkout, read only) plays the same 200
streams; its three comparator arms (rung z = 3, z = 2, re-anchor) have notice records and incidents identical
to this run's (`b3-vs-m2-identity.csv`: 3 of 3 on `notice_incidents`, `notice_events` and `incidents`), so the
two runs are one table. The medium is M2's frozen graph at a 100 ms tick, the tick at which M2's two results
held; the medium's cost includes its own operations, billed at the calibrated prices, and the public rows'
does not, so cost is not like for like.

| row | anchor-correct | leak noticed | leak anchor-correct | background / stream | strict precision | notices / incident | quality fixed | cost s / stream |
|---|---|---|---|---|---|---|---|---|
| Medium, 100 ms (M2) | 0.992 [0.984, 0.998] | 0.993 [0.979, 1.000] | 0.554 [0.481, 0.623] | 5.44 [5.14, 5.74] | 0.501 [0.487, 0.516] | 1.65 [1.60, 1.70] | 0.591 [0.550, 0.632] | 1.848 [1.627, 2.079] |
| Rung z=3 | 0.890 [0.863, 0.916] | 0.460 [0.384, 0.536] | 0.000 [0.000, 0.000] | 4.00 [3.76, 4.24] | 0.748 [0.737, 0.759] | 0.89 [0.87, 0.90] | 0.489 [0.448, 0.532] | 0.664 [0.604, 0.728] |
| Rung z=2 | 0.914 [0.890, 0.937] | 0.460 [0.387, 0.536] | 0.000 [0.000, 0.000] | 9.03 [8.63, 9.45] | 0.613 [0.600, 0.624] | 0.97 [0.96, 0.98] | 0.527 [0.482, 0.572] | 0.646 [0.589, 0.706] |
| Re-anchor | 0.952 [0.932, 0.970] | 0.460 [0.390, 0.533] | 0.000 [0.000, 0.000] | 6.82 [6.47, 7.18] | 0.672 [0.660, 0.683] | 1.04 [1.03, 1.05] | 0.551 [0.507, 0.597] | 0.670 [0.614, 0.730] |
| Ramp / re-anchor | 0.952 [0.932, 0.970] | 0.986 [0.967, 1.000] | 0.971 [0.945, 0.993] | 6.37 [6.02, 6.73] | 0.685 [0.674, 0.696] | 1.11 [1.10, 1.12] | 0.554 [0.509, 0.600] | 0.791 [0.728, 0.856] |
| Split / re-anchor | 0.973 [0.959, 0.986] | 0.460 [0.390, 0.533] | 0.000 [0.000, 0.000] | 6.70 [6.33, 7.07] | 0.680 [0.668, 0.692] | 1.05 [1.04, 1.06] | 0.559 [0.515, 0.605] | 0.674 [0.617, 0.733] |
| Ramp+split / re-anchor | 0.973 [0.959, 0.986] | 0.986 [0.967, 1.000] | 0.971 [0.945, 0.993] | 6.24 [5.88, 6.61] | 0.693 [0.682, 0.704] | 1.12 [1.11, 1.13] | 0.562 [0.517, 0.608] | 0.795 [0.731, 0.859] |


Medium minus row, paired over the same resamples (`b3-vs-m2-paired.csv`):

M2| row minus the re-anchor | noticed | anchor-correct | leak noticed | leak anchor-correct | background / stream | strict precision | notices / incident | quality fixed | quality held | cost s / stream |
|---|---|---|---|---|---|---|---|---|---|---|
| Rung z=3 | -0.062 [-0.085, -0.040] | -0.062 [-0.086, -0.039] | +0.000 [-0.069, +0.068] | +0.000 [+0.000, +0.000] | -2.825 [-3.215, -2.445] | +0.076 [+0.065, +0.088] | -0.155 [-0.166, -0.144] | -0.062 [-0.096, -0.029] | -0.094 [-0.131, -0.059] | -0.006 [-0.031, +0.018] |
| Rung z=2 | -0.032 [-0.048, -0.017] | -0.038 [-0.055, -0.021] | +0.000 [-0.047, +0.046] | +0.000 [+0.000, +0.000] | +2.210 [+2.010, +2.415] | -0.059 [-0.065, -0.054] | -0.073 [-0.081, -0.066] | -0.024 [-0.040, -0.009] | -0.032 [-0.051, -0.014] | -0.024 [-0.041, -0.008] |
| Ramp / rung z=3 | -0.054 [-0.076, -0.032] | -0.062 [-0.086, -0.039] | +0.525 [+0.455, +0.594] | +0.971 [+0.945, +0.993] | -3.240 [-3.640, -2.855] | +0.086 [+0.074, +0.099] | -0.083 [-0.095, -0.070] | -0.062 [-0.097, -0.028] | -0.091 [-0.128, -0.056] | +0.115 [+0.084, +0.146] |
| Split / rung z=3 | -0.059 [-0.083, -0.037] | -0.056 [-0.082, -0.032] | +0.000 [-0.069, +0.068] | +0.000 [+0.000, +0.000] | -2.910 [-3.305, -2.530] | +0.082 [+0.070, +0.093] | -0.151 [-0.162, -0.139] | -0.059 [-0.093, -0.026] | -0.091 [-0.128, -0.056] | -0.004 [-0.030, +0.019] |
| Ramp+split / rung z=3 | -0.051 [-0.074, -0.029] | -0.056 [-0.082, -0.032] | +0.525 [+0.455, +0.594] | +0.971 [+0.945, +0.993] | -3.320 [-3.720, -2.935] | +0.092 [+0.079, +0.104] | -0.079 [-0.092, -0.066] | -0.059 [-0.094, -0.025] | -0.089 [-0.126, -0.053] | +0.117 [+0.085, +0.148] |
| Ramp / re-anchor | +0.003 [+0.000, +0.008] | +0.000 [+0.000, +0.000] | +0.525 [+0.455, +0.594] | +0.971 [+0.945, +0.993] | -0.450 [-0.545, -0.360] | +0.013 [+0.009, +0.018] | +0.070 [+0.064, +0.077] | +0.003 [+0.000, +0.008] | +0.003 [+0.000, +0.008] | +0.121 [+0.098, +0.146] |
| Split / re-anchor | +0.005 [+0.000, +0.012] | +0.022 [+0.010, +0.034] | +0.000 [+0.000, +0.000] | +0.000 [+0.000, +0.000] | -0.125 [-0.180, -0.075] | +0.008 [+0.006, +0.011] | +0.008 [+0.005, +0.010] | +0.008 [+0.000, +0.016] | +0.008 [+0.000, +0.016] | +0.003 [-0.000, +0.008] |
| Ramp+split / re-anchor | +0.008 [+0.002, +0.016] | +0.022 [+0.010, +0.034] | +0.525 [+0.455, +0.594] | +0.971 [+0.945, +0.993] | -0.580 [-0.690, -0.475] | +0.021 [+0.016, +0.026] | +0.078 [+0.071, +0.085] | +0.011 [+0.003, +0.020] | +0.011 [+0.003, +0.020] | +0.124 [+0.101, +0.149] |
| Re-anchor z=3 | -0.022 [-0.038, -0.006] | -0.013 [-0.032, +0.003] | +0.000 [-0.056, +0.054] | +0.000 [+0.000, +0.000] | -4.525 [-4.875, -4.185] | +0.137 [+0.127, +0.148] | -0.092 [-0.101, -0.082] | -0.035 [-0.067, -0.003] | -0.067 [-0.103, -0.033] | +0.013 [-0.004, +0.029] |
| Ramp+split / re-anchor z=3 | -0.008 [-0.024, +0.008] | +0.003 [-0.018, +0.022] | +0.525 [+0.455, +0.594] | +0.971 [+0.945, +0.993] | -5.090 [-5.445, -4.740] | +0.154 [+0.143, +0.165] | -0.012 [-0.023, -0.001] | -0.027 [-0.060, +0.005] | -0.056 [-0.093, -0.022] | +0.145 [+0.118, +0.173] |


The medium's remaining margin over every B3 row is anchor-correct on hard non-leak incidents (0.992 against
0.973); its leak noticing is indistinguishable from the ramp's, its leak anchoring is 0.417 lower, its leak
latency longer (9.8 s against 5.5 s), its strict precision 0.50 against 0.69, its notices per incident 1.65
against 1.12, its cost 1.85 s per stream against 0.79 s.

### Sensitivity: nothing in it chose anything

(`b3-noticers-sensitivity.csv`, `b3-gaming.csv`: 40 arms in all, every other configuration the run played, at
the fixed delay.)

- **Ramp, one parameter at a time over the rung** (the chosen values elsewhere): leak noticed 0.950 to 0.986,
  leak anchor-correct 0.856 (`max_drop` 0: a dip of one reading ends a chain) to 0.978, background 3.56 to
  3.83 per stream, hard anchor-correct 0.887 to 0.890. `min_readings` 3 buys hard noticed 0.944 and costs
  strict precision (0.648 against 0.758) and notices per incident (1.13 against 0.96): the ramp then notices
  ordinary readings. Gap 3 s: leak noticed 0.950, strict precision 0.619.
- **Two looser ramps outside the grid** (gap 3 s, step <= 14, drop <= 4, >= 3 readings, rise >= 10; and gap 3
  s, step <= 20, drop <= 6, >= 2 readings, rise >= 5): 7.25 and 47.49 background notices per stream, strict
  precision 0.210 and 0.088, notices per incident 3.92 and 8.68, anchor-correct 0.849 and 0.806; leak noticed
  0.950 and 0.942, **lower than the chosen ramp's 0.986**. Flooding does not buy the leak here; it buys hard
  noticed (0.997, 1.000) and costs everything else.
- **Split over the re-anchor**, gap 1 to 8 s at burst >= 3: anchor-correct 0.970 to 0.973; burst >= 2 at 3 s
  0.978, burst >= 4 0.962. Background 6.65 to 6.74.


## What the numbers say

**1. A public value-reading noticer notices the leak within the budget.** Verified by running: 137 of 139 leaks
noticed over either base, at 6.37 (re-anchor) or 3.58 (rung) background notices per stream, against a bar of 0.660
within 6.82. The result does not depend on one parameter value: every one of the 10 one-at-a-time grid variants
over the rung notices at least 0.950, and 175 of 216 grid configurations tie at the top on the tuning streams.
It does not come from flooding: the two loose ramps notice less (0.950, 0.942) at 7 to 47 background notices
per stream. What the rule reads is public: counter values and instants, the key (service, counter name), the
public graph through the base; it reads no label and no counter name's meaning.

**2. It anchors the leak at its start, which no abnormal-only noticer does.** Verified from the per-incident
files (`b3-leak.csv`, `b3-leak-offsets.csv`): of 137 notices about leaks, **135 are anchored at an offset of
exactly 0** from the incident's first observation (not "within 1 s": the window of N5 is not what the number
rests on; leak readings are 1 to 1.5 s apart, so a one-reading slip would still have scored), one at 1.2 s and one
at 3.7 s. The 4 leaks that are not anchor-correct: **two are never noticed because the chain began on a
background reading of the same counter 0.46 s and 0.66 s before the leak's first reading** (verified in the
notice files: the arm notices at those two seeds, anchored on a background observation; the public readings
show the stray, 29 then 28 and 19 then 28); two are noticed with a late anchor (one after a glitch reading, one after a step of 11 against the cap of 10). This is the reading "anchored at the earliest observation of the rise" that the file documents: a stray
within the gap that the leak's first reading continues becomes the anchor. Median latency from the first
observation 5.5 s [p90 6.8 s] against 14.2 s for the re-anchor and 9.8 s for M2's medium: the rule needs five
readings, so latency has a floor of about four reading intervals, which no longer-window rule here avoids.

**3. The same shape is what a decoy looks like, and the ramp notices them.** Verified by running
(`b3-decoys.csv`): of 541 decoy incidents, the rung z = 3 notices 0.651, the re-anchor 0.756, the ramp over the
re-anchor **0.948**; notices anchored on decoys per stream go 2.40 -> 2.97 over the re-anchor (1.88 -> 2.48 over
the rung). Of the ramp's 510 notices that its base does not make over the re-anchor, 137 are on leaks (135 at the
first observation), **154 on decoys (143 at the decoy's first observation)**, 192 on plain incidents (191 of
them anchored more than 5 s after the incident's first observation, median 30 s), 11 on hard non-leak
incidents (late), 16 on background. Public readings of four decoys (`Saturation`, 24, 26, 31, 35, 42, 44 ...) have
the leak's shape. The selection oracle never asks about decoys (it asks about hard-anchored anomalies), so
neither quality nor cost shows what a decoy notice costs a selector that must decide; **both precision measures
count a decoy notice as correct** (N16 counts any incident, and a decoy has a site, so N15 counts it
anchor-and-site-correct). What separates a leak from its look-alike is not in the shape of the first five
readings; whether it is in anything public later is untested here.

**4. The ramp also spends notices where the budget does not look.** The budget counts notices anchored on
background. The ramp's 192 late plain notices and 11 hard ones are anchored on incident observations (a
counter that happens to rise over five readings inside a burst: public readings of `ErrorRate` 65, 67, 74, 82,
80, 81, 94), so they cost the budget nothing, raise notice precision (0.855 -> 0.877 over the rung) and do not
count as anchor-correct. Notices per stream rise by +1.50 [+1.35, +1.66] over the rung and +1.42 over the
re-anchor, notices per incident by +0.07, the background count falls (-0.45 [-0.55, -0.36]) because the ramp's
anomalies absorb 106 of the re-anchor's background notices (counts; consistent with the adoption rule, not
verified per notice). Strict precision (+0.013 [+0.009, +0.018]) and notices per incident are the measures that
would show a flood, and here they show none; the sensitivity rows show what one looks like (strict precision
0.21, 0.088).

**5. Splitting recovers some of the 8 never-noticed incidents, and more of the anchored-late ones.** Over the
re-anchor the split gains 8 anchor-correct incidents and loses none (compound 5, cascade 3; split-brain 0),
of which 2 were never noticed (seed 20121 incident 32, anchored correctly and noticed at 3.8 s; seed 20186
incident 1, noticed at 1.0 s; both cascades), and 6 were noticed late or on the wrong anchor. The other 6 stay
never noticed: three split-brain incidents (seeds 20002, 20123, 20174) that no row of the table notices, two
cascades (20076, 20185) that the rung and the re-anchor at z = 3 notice and the re-anchor at z = 2 does not,
and one split-brain (20126) that only the ramp's composition notices, at 53 s and not anchor-correct. The
brief's hypothesis, that another incident's anomaly absorbed their observations, is supported for those two
cascades (the split recovers them by taking a later burst at another site out of an earlier anomaly) and not
for the split-brain family, where it recovers nothing. Counts of 8 and 2 support no rate; the paired gain of
+0.022 [+0.010, +0.034] on 372 incidents is real at this sample and small.

**6. The pieces compose and the base matters less than the piece.** The interaction is zero on every table
measure (above). Over the re-anchor at z = 3, ramp + split reaches 0.954 anchor-correct, 0.986 leak noticed and
0.971 leak anchor-correct at **1.73 background notices per stream and strict precision 0.826**, the row with
the most budget left.

**7. Quality does not distinguish the rows that differ in noticing.** Hard non-leak quality moves by at most
+0.011 [+0.003, +0.020] over the re-anchor for any piece, and the two readings differ by 0.008 to 0.040 per row (the retirement confound B2 measured, largest for the rungs at z = 2 and the
re-anchor family; the hold changes the notice record by 2 to 10 notices per arm over 200 streams: `b3-hold-effect.csv`). The leak's quality
is where the ramp acts (0.187 -> 0.820), and the leak is outside the quality column by definition.

**8. Cost.** The ramp adds +0.12 s per stream (+17% over the rung's 0.664) and +0.40 reasoner calls, nearly all of
it the leaks' newly anchored hard notices that the oracle asks about; the harness bills no noticer's own work.
Counted separately on the held-out streams (`ramp_operations`, the chosen parameters): 4472 observations per
stream, 2970 counter readings fed and 1753 chain comparisons per stream, at most 80 chains live at once.

## Where the measures can still be gamed

- **The background budget charges only notices anchored on background.** A noticer's notices on incident
  observations, off the incident's start or on decoys, are free in it (point 4). Read it with notices per
  stream and strict precision, and read strict precision knowing that a decoy notice is strict-correct.
- **Leak noticed saturates.** 176 of 217 tuning configurations notice every tuning leak; the measure cannot
  rank ramps, and the primary objective alone chose nothing here (the tie keys did).
- **Notice precision rewards notices on incident observations of any tier**, decoys included (B2's point 3,
  now seen with a noticer that is not a flood).
- **Anchor-correct, leak included, has a 1 s window**; the ramp's 135 anchors at exactly 0 show it is not leaning
  on the window here, but a rule that anchors a second reading later would score the same.
- **"Noticed" counts a late notice.** The latency file is the check; the ramp's floor is five readings.
- **The quality columns are over hard non-leak incidents** and move for none of the new rows; a reader who takes
  quality as the verdict will see no effect of a noticer that changes the leak completely.
- **No noticer's own work is billed**, the ramp's, the rung's and the re-anchor's alike, while the medium's is.
- **The ramp's constants were chosen after reading public readings of the tuning leaks against the evaluator's
  first-observation instants** (the same route as B2's 20 ms and M2's graph); the shape is therefore known to
  this world's leaks and decoys, and the sensitivity rows (not a second world) are the only check on it.

## Readings and deviations (each with its reason)

My predecessor's, from the commits' documentation and the code (the commit messages are bare):

1. **Ramp: chains, not a window.** "Monotonically or by more than a slope threshold over a window" became chains
   of readings with a step cap, a tolerated drop and a gap (a reading continuing none starts its own chain, at
   most four chains per key, a chain over 256 readings dropped). Reason: the leak's readings are noisy, with
   strays of the same counter among them; a strict monotone window would break on every dip (`max_drop` 0 loses
   0.115 of leak anchor-correct in the sensitivity rows).
2. **Ramp anchor = the chain's first reading**, so a stray within the gap that the ramp's first reading continues
   is the anchor (the two never-noticed leaks above). Stated in the file before any tuning.
3. **Ramp anomalies are noticed at once** and joined to the base's set, adopt any un-noticed candidate whose
   observations are all among theirs, and keep attaching later readings of the chain while tracked.
4. **Split: "two bursts at different sites" read as** a later cluster of at least `min_burst` observations about
   services other than the anomaly's site, after a silence of more than `gap_ns` from the latest earlier burst;
   the moved observations become a **candidate**, not a notice (noticing it at once was tried on a diagnostic
   of the tuning streams and rejected: two stray observations make a burst of two and a false notice that then
   takes the real incident's burst).
5. **The ramp is composed over the rung at z = 3 for tuning and played unchanged over the other bases**, not
   re-tuned per base; the split is tuned per base (rung, re-anchor).
6. **Tuning rules, grids and tie keys fixed in `b3_common.py` before the runs; no amendments.** The 216-point grid
   brackets the leaks' public readings (step 0 to 9, readings 0.6 to 1.5 s apart) with one looser value each side.
7. **Added rows beyond the brief**: the re-anchor at z = 3 and ramp + split over it (to have a composition with
   budget to spare), the rung at z = 2 (B1's comparator), and 18 sensitivity arms (never used to choose).
8. **The byte-identity gate was played before tuning** and the run directories of the gate and the tuning
   stages deleted after their CSVs were recorded (the brief allowed it).

Mine:

9. **I did not rebuild the release binary.** The brief's check (`git diff <built-revision> HEAD -- crates
   Cargo.toml Cargo.lock` empty) is not empty: it shows `HARNESS.md` and a test file. The built revision is not
   recorded in the binary; the binary's mtime (18:08:02) lies between commit `2a1854b` (the noticers, 18:05:57)
   and the next commit; no source or manifest file is newer than it; cargo's own up-to-date check reports
   nothing to build. I treated those as showing the binary is HEAD's source, and the rerun of the gate at the
   end (above) is the check by running.
10. **The held-out run was played at `f007954`** (the manifest's revision), before any of my commits; every
    commit of mine came after it (analysis only, and one rustfmt hunk in a test).
11. **Supplementary comparison with M2's medium** (`b3_vs_m2.py`), which the queue names for the case that M2's
    runs had started; M2's outputs were read from the main checkout, read only, after the identity check.
12. **Mutation testing of the two noticers** was not in the brief; I ran it as a check on the predecessor's tests
    (below).
13. **`b3_run.sh` and the driver were used as the predecessor wrote them**; the manifest was not regenerated.
14. **The files named `b3-finding4.md` and `b3-stress.md` in the brief are not in the scratch directory:** they
    are the Stage B exploration notes in `experiments/exploration/` about an older "B3" item (the stress suite),
    unrelated to this unit; I read them and used nothing from them. The predecessor's actual scratch notes are
    probe outputs and helper scripts (`probe-split.txt`, `probe-reanchor.txt`, `survey*.py`, `select-*.out`).

## Verification

**By running.**
- The held-out run, the table, the paired differences, the diagnostics and the supplementary comparison (this
  report's numbers all come from `artifacts/runs/b3-heldout-b5-rho0.7`, regenerated by `b3_table.py`,
  `b3_diagnostics.py`, `b3_vs_m2.py`, `b3_provenance.py`).
- Provenance (`b3_provenance.py`, exit 0): byte-identity 62 of 62; tuning manifests on 10000-10099 and the
  held-out on 20000-20199; incidents identical in every arm; the three comparator rows are earlier runs' arms
  (`b3-vs-earlier.csv`); 0 step-capped segments in 40 arms x 200 streams.
- The two runs of M2 and B3 are one table: 3 of 3 shared arms identical on notice and incident records.
- Tests: 756 passed, 0 failed, 7 ignored in the Rust workspace (four of them added after the mutation run) (`ramp` 988 and `split` 556 lines of integration
  tests, the unit tests of both files, the probe); 402 passed in the analysis tests, 4 deselected, 13 of them
  the B3 scripts' and diagnostics' (the diagnostics' on hand-built frames).
- Mutation testing of the two noticers: cargo-mutants 27.1.0 on `noticer_ramp.rs` and `noticer_split.rs` (`--in-place`, with
  the tests `stream_ramp`, `stream_split`, `stream_noticer`): 150 mutants; the first run (27 min) 119 caught, 18
  missed, 12 unviable (compile failures), 1 timeout (an infinite loop, counted as detected). The misses: the
  detector's and the split's counters (nothing asserted them), the delegation of `score` and `refresh` to the
  base (as in B2), the uid increment that tells two chains at one key apart, and three more. I added 4 tests
  (`a45cb81`) and reran the 19 not caught with `--iterate` (4 min): 15 caught. **Final: 134 caught, 3 missed, 1
  timeout, 12 unviable.** The 3 misses: `split_picks` line 128 (`i + 1` -> `i * 1` for a singleton cluster: an
  equivalent mutant by argument, a singleton is never a burst because `min_burst` >= 2) and the two mutants of
  `i += 1` in the `None` arm of `split_pass` (unreachable by argument, since `split_picks` returns only
  indices `split_off` accepts; I did not prove it). Outputs in the scratch directory.
- Two diagnoses made by looking at public readings and notice files, not by a mechanism test: the two never-noticed
  leaks (checked in the notice files) and the shape of the decoys and of the late plain notices (public readings
  of four examples each).

Gates on exit codes, at the final tree: `cargo fmt --all -- --check` 0 (after one rustfmt hunk in
the predecessor's probe test, `1dbd2d5`, which the check at `f007954` rejects); `cargo clippy --locked
--workspace --all-targets -- -D warnings` 0; `cargo test --locked --workspace --no-fail-fast` 0 (756 passed, 0
failed, 7 ignored); `bash scripts/check-no-oracle.sh` 0; `PYTHONPATH=analysis
/home/user/gordian/analysis/.venv/bin/python -W error -m pytest -q analysis` 0 (402 passed, 4 deselected), with
`PYTHONPATH=analysis` for the reason B1 and B2 gave (the shared environment imports the main checkout's
package). All at `a45cb81`, the last commit that changed source; the later commits are this report and CSVs.

**Assumed.** That cargo's up-to-date report means the binary is HEAD's source (and the rerun of the gate is the
check by running); that percentile cluster-bootstrap intervals over 200 streams describe the sampling
uncertainty (the leaks are 139 incidents in at most 200 streams, so a leak share has fewer effective
clusters than 200); that the first occupied service is the diagnosed site (B2's assumption, unchanged); that the
cgroup limits applied as the driver reports (internal/external ratio 0.942, no tolerance declared); that the
decoys' public readings look like the leaks' beyond the four examples shown (the counts of decoys noticed are
measured; why they are noticed is not).

## Runs and exit statuses

(`b3-driver-log.csv`, `b3-run-index.csv`; outputs in `artifacts/runs/` of the worktree, git-ignored.)

| run | exit | wall s | who | what |
|---|---|---|---|---|
| `xcheck3-r6-heldout-b5-rho0.7` | 0 | 262 | predecessor | the gate |
| `b3-tuneramp-b5-rho0.7` | 13, then 0 | 0, then 494 | predecessor | stage R: 217 arms. The first attempt was refused by the driver with status 13 (the working tree was not clean), nothing ran |
| `b3-tunesplit-b5-rho0.7` | 0 | 57 | predecessor | stage S: 32 arms |
| `b3-tunedelay-b5-rho0.7` | 0 | 256 | predecessor | stage D: 11 rows x 13 delays |
| `b3-heldout-b5-rho0.7` | 0 | 115 | me | the held-out run: 40 arms, 200 streams; 0 step-capped; internal/external ratio 0.942 |
| `xcheck4-r6-heldout-b5-rho0.7` | 0 | 161 | me | the gate again, at `a45cb81`, one poll waiting for another lab's cargo |
| cargo-mutants, two runs | 3 (missed mutants found; expected), 3 | 27 min, 4 min | me | 150 mutants, then the 19 not caught, under the runner |
Excluded or refused: nothing was excluded and no run was discarded; the bill refused probes only in the two
loosest sensitivity arms (6 and 49 probes over 200 streams, `b3-refusals.csv`) and nothing in any table row.
**The restart's effect:** the log ended with the held-out run waiting for another process (one "waiting" line,
written before the restart); no output directory existed for it, `artifacts/runs/` held only the logs and the
manifests, so I played it from the start with the manifest as written (its revision was HEAD's), and it waited
for nothing (0 polls). The tuning outputs the predecessor recorded as CSVs are the only record of stages R, S
and D; their run directories were deleted as the brief allowed, so I could not re-derive them; I read the CSVs
and the selection JSON and did not rerun a stage. Core-sharing checks (every cargo call and driver run, `pgrep -x gordian-run`, `cargo`, `rustc`): I waited one
poll (30 s) before the first `cargo fmt` and six polls (3 min) before the `cargo fmt` after my new tests,
when other labs' builds (lab 1's M3 tests, lab 3's release build) were running; the final gate's driver run
waited one poll for the same reason (`waits=1`); the held-out run waited for nothing. The chief's rule
that every cargo call run under `scripts/cgroup-run.sh --name lab2-build --cpus 0-2 --memory 3G` reached me
partway through: the release up-to-date check and the operations probe ran before it (under `taskset` only);
`fmt`, `clippy`, the workspace tests and both mutation runs after it, under the runner: 14 invocations, peak
memory 0.82 GB, no OOM kill, no restart of the container.

## Hidden record

I did not read `crates/gordian-stream/HIDDEN-DESIGN.md`. I read the evaluator's rules (`RULES.md`), the review
log's entries (B2 and M2), the noticer code, and the public observation dumps (`examples/dump`, which prints
each observation's id, instant and public content) of a few held-out seeds, alongside the evaluator's incident
files for the seeds' first-observation instants. The ramp's constants were chosen by my predecessor on the
tuning streams the same way (public readings against the evaluator's first-observation instants); I flag it as a
route by which the hidden labels shaped a constant, as B2's gap and M2's graph were, and it is not a
knowledge-injected arm: no rule encodes a family, a counter's name, a hidden timing or a hidden fault rule.
`scripts/check-no-oracle.sh` passes with both files.

## What I am least sure of

- That the ramp's leak result is about leaks and not about this world's generator: leaks are the one family whose
  first readings are a smooth run of one counter, and the rule is a description of that. The decoys' result
  says the world has look-alikes of the same shape; whether a second world's leaks would be as clean is
  untested.
- The leak anchoring rests on 137 notices over 139 incidents, 135 at exactly 0; the interval ([0.945, 0.993]) is
  a cluster interval on 139 incidents, and the measure's maximum is 1.
- The split's effect is 8 incidents; the row's selection on the tuning streams was a tie rule over a zero effect
  for the rung base, and the held-out best of the grid differs from the choice.
- That the +0.40 reasoner calls and +0.12 s are all the leaks': derived from counts of added notices by tier,
  not from a per-call attribution.
- That the background fall from the ramp (-0.45) is adoption: counts only.
- The cost column's comparability with the medium's (own operations billed against not billed).

## What the chief should examine most carefully

1. **M2's result 2 and the comparator for M3 and EXP-101.** A public row ties the medium on leak noticed and
   beats it on leak anchor-correct and latency (the table above); the medium's remaining margin is hard
   anchor-correct (+0.019 [+0.005, +0.033] over ramp + split over the re-anchor, +0.040 over the re-anchor)
   against more notices per incident and lower strict precision. Which row EXP-101 names (the best public noticer
   "under the background and strict-precision bounds") is the chief's act; the table has ramp + split over the
   re-anchor (0.973, 0.986, 0.971, 6.24, 0.693) and over the re-anchor at z = 3 (0.954, 0.986, 0.971, 1.73, 0.826).
2. **Decoys**: whether noticing a decoy is a cost the EXP-101 measures must charge (the selection oracle never asks
   about one; both precision measures count one as correct), before a public row is called "precise".
3. **The ramp's notices off the incident's start** (192 plain, 11 hard) that the background budget does not charge.
4. **The tie-rule choices** (a saturated objective for the ramp, a zero effect for the split over the rung) and the
   sensitivity rows beside them.
5. **That the binary was not rebuilt** (reading 9) and the rerun of the gate.
6. **The M2 identity check** reads M2's outputs from the main checkout; the chief's own hashes are the check.

## What I would test next

- **Leak versus decoy**: whether anything public separates a leak from its look-alike later in its run (the
  readings after the first five), which decides whether a noticer can have the leak without the decoys.
- **The ramp at the medium's tick lengths and on a second world** (W1's statistics), to see whether the 5-reading
  floor and the shape are the world's.
- **Quality at b = 2.5 and 8**, and a selection rule that is not the privileged oracle, so that decoy and
  late-plain notices are asked about and charged.
- **Re-anchoring the ramp** against the stray that precedes a leak (two leaks lost to it), and the same for the
  split's two never-noticed split-brain incidents.
- **A medium graph that reads the ramp** (M3's target, if the chief keeps it): the public row now sets the bar at
  0.971 leak anchor-correct and 5.5 s.
