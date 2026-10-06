# B1: the `Noticer` seam, notice measures, public noticing baselines

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation). Lab 2,
unit B1 of `docs/lab-queue.md`, branch `noticer-seam`. The table makes no claim about which noticer
is better: it is the comparator for M2. Scripts: `experiments/exploration/scripts/b1_*.py` and
`b1_run.sh`. Files: `b1-*.csv`, `b1-selected.json`, `b1-noticers-table.md` (generated).

## What was built

- **The seam.** `Noticer` (`crates/gordian-run/src/stream/arms/noticer.rs`): `observe` (one delivered
  observation with the public rules' verdict, returns the anomaly it joined), `notice(now, store)`
  (the anomalies noticed at this step: anchor, site, attached observations), `anomalies`, `score`,
  `refresh`, `retirable(now)` and `retire(id)`. `RungNoticer` is the default and is the rung's own
  noticing moved out of `rung.rs` unchanged (candidates, the graph-based attach rule, the z-score,
  the re-anchoring on the densest burst, forgetting stale candidates). The rung keeps, per anomaly the
  noticer tracks, everything after the notice (`Down`: working state, reviews, probes, escalations,
  declarations); a noticer cannot touch it. The noticer is chosen in the manifest: `rung.noticer`
  for every arm, `noticers` (arm name to noticer) per arm. Neither is written when it is the default.
- **The record.** Every notice and retirement is logged by the rung and scored by the evaluator.
  Three new files per arm, beside the three that are byte for byte as before: `notices.csv` (per
  stream), `notice_incidents.csv` (per incident), `notice_events.csv` (every notice and retirement).
  The analysis loader reads them with a schema guard against the harness's header constants
  (`load_stream_arm`; `notice_per_stream`, `notice_points`, `notice_latency` in `stream.py`).
- **The measures** (evaluator, `crates/gordian-stream-eval/src/notice.rs`, rules N1 to N12 in
  `RULES.md`), as implemented:
  - a notice is *about* an incident when its **anchor's label** names the incident (N1); the notice's
    attached evidence is not read;
  - *noticed*: at least one notice about the incident (N2); *notice latency*: the earliest such
    notice's instant minus the instant of the incident's **first observation** (lowest-id observation
    labelled with it) (N3, N4);
  - *anchor-correct*: a notice about the incident whose anchor's emission instant is within 1 s
    (inclusive, absolute difference) of the incident's first observation (N5);
  - per stream: notices on background, on plain incidents, on hard incidents, on decoys (N8); *notices
    per incident* = notices anchored on incidents over incidents of every tier, pooled (an analysis
    ratio); retirements (N10). Ratios are pooled from counts over streams, never taken per stream.
  - refused records (no verdict): instants not one per label, a notice before its anchor, notices or
    retirements going back in time, an anomaly noticed twice, a retirement of an anomaly not live (N11).
- **Three baselines**, public information only: `RungNoticer` at any `notice_z`; `ChangeTriggered`
  (a notice on the first abnormal observation at a node after `q` without one there, anchored there);
  `EarliestAnchor` (the rung's noticer, anchor moved to the earliest abnormal observation at the
  anomaly's site within `l` before the rung's anchor).

## Byte identity: the gate

R6's held-out manifest at b = 5, rho = 0.7 (main checkout, `source_revision` replaced, run id kept,
written to a separate directory) replays byte-identical for **all 62 arms** against
`r6-results-sha256.csv`, for `results.csv` and `incidents.csv`: 62 of 62 and 62 of 62
(`b1-regression.csv`). The replay was made with the binary of the seam commit `e4c556a` (before the
harness recorded notices) and again with the binary of the final tree (`b1-regression-final.csv`, 62
of 62 again). R10's held-out run at the same setting, which has the notice oracle R6's has not,
replays 6 of 6 (`b1-regression-r10.csv`). The new records are written to files of their own; the
headers of `results.csv` and `incidents.csv` are pinned by a test (`PRE_SEAM_*_HEADER`). R10's `sel_rung_privileged` arms at
z = 3, 2, 1 and 0.5 are, modulo the `run_id` column, this run's `sel_rung_*` arms (`b1-vs-r10.csv`),
so quality rows are R10's own numbers (0.489 at the default, 0.527 at z = 2, R10's +0.038).

## The table

200 held-out streams (seeds 20000-20199), b = 5, rho = 0.7, `oracle_selection` at R5's 16 s delay,
the rung's own context, as R10's `sel_rung_privileged`. 90% equal-tailed percentile intervals from
10,000 resamples of whole streams (`r6_stats.interval`, multinomial counts from
`numpy.random.default_rng(9950)`). 372 hard non-leak incidents, 139 slow-leak incidents, 4,264 plain.
Intervals are not paired differences: **no comparison between rows is claimed**.

| noticer | hard non-leak noticed | anchor-correct | leak noticed | notices on background / stream | hard quality (selection oracle) | cost s / stream |
|---|---|---|---|---|---|---|
| RungNoticer, z = 3 (default) | 0.917 [0.893, 0.939] | 0.890 [0.863, 0.916] | 0.460 [0.384, 0.536] | 4.00 [3.76, 4.24] | 0.489 [0.448, 0.532] | 0.664 [0.604, 0.728] |
| RungNoticer, z = 2 | 0.946 [0.927, 0.964] | 0.914 [0.890, 0.937] | 0.460 [0.387, 0.536] | 9.03 [8.63, 9.45] | 0.527 [0.482, 0.572] | 0.646 [0.589, 0.706] |
| ChangeTriggered, q = 128 s | 0.032 [0.018, 0.048] | 0.032 [0.018, 0.048] | 0.000 [0.000, 0.000] | 8.76 [8.50, 9.01] | 0.016 [0.006, 0.028] | 0.013 [0.006, 0.021] |
| EarliestAnchor, l = 0.25 s | 0.917 [0.893, 0.939] | 0.890 [0.863, 0.916] | 0.460 [0.384, 0.536] | 4.01 [3.78, 4.25] | 0.492 [0.450, 0.535] | 0.664 [0.604, 0.728] |

Beside it (same rows; the full columns are in `b1-noticers-table.csv`): plain incidents noticed
0.860, 0.887, 0.023, 0.860; notices on plain incidents per stream 19.4, 20.9, 1.1, 19.4; notices on
hard incidents per stream 2.25, 2.56, 0.12, 2.25; notices per incident 0.89, 0.97, 0.05, 0.89;
leak anchor-correct 0.000 in every row; slow-leak quality 0.187, 0.216, 0.000, 0.187; calls per
stream 2.10, 2.16, 0.04, 2.10. Cost is modelled seconds per stream (substrate and rule plus the
reasoner at the manifest's rate); the substrate part is 0.0002 to 0.0005 s of it in each of these four rows, so the
cost column is the reasoner's calls there (at q <= 1 s the substrate part is 0.019 to 0.022 s).

Median notice latency among the noticed, from the incident's first observation (`b1-noticers-latency.csv`):
hard non-leak 2.95 s (rung), 0.60 s (z = 2), 0.22 s (q = 128; 12 incidents), 2.95 s (l = 0.25); slow
leak 16.1 s, 14.3 s, none noticed, 16.1 s.

### How the four rows were chosen (the tuning rule)

The brief says each parameterised noticer's one parameter is tuned on R6's tuning streams and gives no
criterion, so I fixed one before the tuning run (`b1_common.py`, commit `2e56fb1`): on the 100 tuning
streams (10000-10099) at the primary setting, with the same selection oracle, **the parameter with the
highest anchor-correct share of hard non-leak incidents among those whose notices on background per
stream do not exceed the default `RungNoticer`'s on the same streams** (3.99); ties toward the rung's
own behaviour; if no parameter is within the budget, the one with the fewest background notices. The
budget is the one M2's criterion imposes (`no more notices on background per stream than that
noticer`), applied to the baselines so that each is the best it can be at the background the rung
spends. Results of the tuning (`b1-tuning-points.csv`, `b1-selected.json`):

- **ChangeTriggered: no q of the grid (0.5 to 128 s) is within the budget.** The lowest background rate
  is 8.6 notices per stream at q = 128 s; the rule's fallback chose it, and it notices 3.5% of hard
  non-leak incidents on the tuning streams. The 8.6 is a floor, not a tail: under the reading below
  (a node never seen is quiet) the first abnormal observation at each of the 8 to 12 nodes is a notice
  whatever q is (8.58 per stream fall in the first 30 s of the held-out streams at q = 128;
  `b1-flood-floor.csv`).
- **EarliestAnchor:** the rule as first written would have chosen l = 0, which is the rung exactly (a
  test and the held-out run both show the rows identical, `b1-vs-r10.csv`). **Amendment, made after
  the tuning run and before any held-out run:** l = 0 stays in the grid as a control and is not a
  candidate. Among l > 0 only l = 0.25 s is within the budget (3.97 against 3.99); it is the row above.
  Every larger l makes the anchor worse (below). So the EarliestAnchor row is, in effect, "the rung
  with a 0.25 s lookback", and says little more than that the rung's own row stands.

The held-out run played the whole grid (18 configurations) and, afterwards, the rung at z = 1 and
0.5 (R10's sweep values). **Nothing was chosen from any held-out number.**

### Sensitivity: every configuration, held-out (`b1-noticers-sensitivity.csv`)

| noticer | hard non-leak noticed | anchor-correct | leak noticed | notices on background / stream | hard quality | cost s / stream |
|---|---|---|---|---|---|---|
| RungNoticer, z = 3 (default) | 0.917 | 0.890 | 0.460 | 4.00 | 0.489 | 0.664 |
| RungNoticer, z = 2 | 0.946 | 0.914 | 0.460 | 9.03 | 0.527 | 0.646 |
| RungNoticer, z = 1 | 0.941 | 0.909 | 0.460 | 30.86 | 0.513 | 0.632 |
| RungNoticer, z = 0.5 | 0.927 | 0.874 | 0.432 | 60.33 | 0.478 | 0.591 |
| ChangeTriggered, q = 0.5 s | 1.000 | 0.984 | 1.000 | 587.75 | 0.000 | 0.022 |
| ChangeTriggered, q = 1 s | 1.000 | 0.962 | 1.000 | 510.95 | 0.000 | 0.019 |
| ChangeTriggered, q = 2 s | 1.000 | 0.927 | 0.705 | 422.44 | 0.202 | 0.736 |
| ChangeTriggered, q = 4 s | 0.911 | 0.847 | 0.540 | 318.95 | 0.376 | 0.578 |
| ChangeTriggered, q = 8 s | 0.704 | 0.624 | 0.374 | 192.25 | 0.250 | 0.327 |
| ChangeTriggered, q = 16 s | 0.325 | 0.306 | 0.144 | 74.21 | 0.105 | 0.129 |
| ChangeTriggered, q = 32 s | 0.089 | 0.083 | 0.043 | 17.38 | 0.043 | 0.037 |
| ChangeTriggered, q = 64 s | 0.032 | 0.032 | 0.000 | 8.96 | 0.016 | 0.013 |
| EarliestAnchor, l = 0 (the rung) | 0.917 | 0.890 | 0.460 | 4.00 | 0.489 | 0.664 |
| EarliestAnchor, l = 0.5 s | 0.914 | 0.884 | 0.453 | 4.29 | 0.489 | 0.664 |
| EarliestAnchor, l = 1 s | 0.860 | 0.823 | 0.576 | 5.01 | 0.449 | 0.650 |
| EarliestAnchor, l = 2 s | 0.801 | 0.766 | 0.626 | 6.91 | 0.422 | 0.627 |
| EarliestAnchor, l = 4 s | 0.645 | 0.613 | 0.561 | 11.26 | 0.344 | 0.521 |
| EarliestAnchor, l = 8 s | 0.465 | 0.422 | 0.496 | 15.56 | 0.220 | 0.404 |

(Intervals for every cell are in the CSV and in `b1-noticers-table.md`. ChangeTriggered q = 128 and
EarliestAnchor l = 0.25 are the rows of the table above.)

## What the table means, and what it does not show

**Established by the runs.**

1. The rung's own row reproduces R10: 0.917 noticed, 0.460 on the leak at 4.0 background notices per
   stream, 0.489 quality. Anchor-correct is 0.890, so of the 341 hard non-leak incidents the rung
   notices, 331 have an anchor within 1 s of their first observation. The rung's gap to a perfect
   noticer on non-leak hard incidents is therefore **8.3% never noticed (31 incidents) and 2.7% noticed
   with a late or wrong anchor (10)**, not an anchoring problem among the noticed.
2. The mis-anchoring R10 found in the ledgers is visible in the new records without ledgers: of the 31
   hard non-leak incidents the rung never notices, 17 have a notice anchored on *background* within 1 s
   of their first observation and 4 more within 5 s; 10 have none within 5 s (`b1-misanchor.csv`). A
   background notice lands within 1 s of a given instant by chance about 1% of the time (4 per stream
   over 600 s), so 17 of 31 is a real signature. The 10 others are not explained here (they may be
   attached to another incident's anomaly, or never reach the threshold).
3. **Lowering the rung's threshold stops helping early.** Anchor-correct is 0.890, 0.914, 0.909, 0.874
   at z = 3, 2, 1, 0.5 while background notices go 4.0, 9.0, 30.9, 60.3 per stream: the ~9% of
   incidents the rung misses are not a threshold matter (R10's reading, now with a measure for it).
4. **EarliestAnchor, as specified, moves the anchor the wrong way for the failure R10 found.** The
   anchor-correct share falls steadily with l (0.890, 0.884, 0.823, 0.766, 0.613, 0.422 at l = 0,
   0.5, 1, 2, 4, 8 s) while background notices rise. The decline matches what an earlier anchor on a
   stray at the same site would do if abnormal background arrives at roughly 0.1 per node per second
   (the public rate in `stream/mod.rs`): the share of incidents with no stray at the site in l seconds
   is exp(-0.1 l), which gives 0.905, 0.82, 0.67, 0.45 at l = 1, 2, 4, 8, against 0.92, 0.86, 0.69, 0.47
   observed as fractions of the rung's own 0.890. This is a consistent explanation, not a verified one
   (the strays' positions were not checked). It is why the 0.25 s row equals the rung's: at that reach
   almost nothing is moved. The mis-anchoring fix R10 points at is a *later* anchor (onto the burst
   rather than a stray that came just before it), which this noticer cannot make.
5. **ChangeTriggered cannot be tuned into the rung's background budget.** At q = 0.5 to 1 s it notices
   every hard non-leak and every leak incident and is 96 to 98% anchor-correct, at 511 to 588
   notices on background per stream and 24 to 33 notices per incident (of 173,386 notices on incidents
   at q = 0.5, 156,588 are not anchor-correct). At the rung's own background rate it notices almost
   nothing. Its curve never meets the rung's.

**Not shown, and why.**

- *Which noticer is better.* The rows differ in a trade between background notices and noticing that
  the table does not price, and the intervals are not paired differences.
- *Quality as a measure of noticing.* The hard-quality column is the selection oracle at a 16 s delay
  that R5 tuned for the rung's notice times. The oracle asks once, 16 s after the notice, and only about
  an anomaly still live (the rung retires an anomaly after 6 s with no abnormal observation). The
  lifetimes show why the ChangeTriggered quality is ~0 at small q: of 22,014 notices anchored on hard
  incidents at q = 0.5 s, 51 were still live after 16 s (median lifetime 6 s); for the rung, 421 of 451
  (`b1-lifetimes.csv`). The same arithmetic touches z = 2 (81 of 513 retired before the delay). So the
  quality column carries delay-and-retirement effects as well as noticing, and a noticer that notices
  earlier is penalised by a delay it was not tuned for. It is reported because the brief asks for it
  and because it makes the rows comparable with R10, not as the noticer's quality.
- *Hard limits stayed on.* No segment was step-capped; the bill refused no component or rule call. The
  bill refused probes on the ChangeTriggered arms with many notices (181, 172, 80, 54, 4 refused probes
  over 200 streams at q = 0.5, 1, 2, 4, 8 s; none for the four rows' other noticers): the stream's
  150 probe units are spent by a noticer that opens hundreds of anomalies, which is part of its cost.
- *The leak.* Every noticer here sees only abnormal observations; the slow leak's early readings are
  not abnormal under the public rules, so leak anchor-correct is 0.000 in every row and the leak's
  notice latency is 14 to 16 s for the rung family (9 s for the flooding noticer, which anchors on a
  later abnormal observation at the leak's node). "Leak noticed" credits a notice anchored anywhere on
  a leak observation, which a flood obtains (1.000 at q <= 1 s). The leak result M2 asks for should be
  read with this.
- *One setting.* b = 5, rho = 0.7 only; the noticing measures do not depend on the reasoner, but the
  selection-oracle rows do.

## Where a noticer can game anchor-correctness

- **Flooding.** Anchor-correct asks for one notice with an anchor within 1 s of the incident's first
  observation; it does not ask for the other notices to be few or right. ChangeTriggered at q = 0.5 s is
  0.984 anchor-correct and 1.000 noticed with 588 background notices per stream and 33 notices per
  incident, and leak noticed is 1.000. A comparator chosen by this measure alone is a flood. The
  measures to read with it are notices on background per stream and notices per incident, and neither
  is in M2's criterion for result 1 beyond "no more notices on background per stream than that
  noticer", which a flooding comparator satisfies trivially for any candidate.
- **The window is generous.** A burst lasts about 0.4 s and the window is 1 s, so an anchor on any
  early observation of the incident counts, including one that is not the observation the declaration
  should be about.
- **Only the anchor is read.** A noticer that anchors on the right observation and then attaches the
  wrong evidence, or one whose site (the node it reasons about) is not the incident's, is
  anchor-correct. The anchor is a label lookup, not a statement that the notice is useful.
- **First-observation timing is the noticer's friend for non-leak families and its enemy for the leak.**
  For the four non-leak-observation-first families a noticer that anchors on every abnormal observation
  is correct whenever the incident's first observation is abnormal (as it is for the three non-leak
  hard families here, 372 of 372 noticed at q <= 2 s); for the leak no abnormal-only noticer can be
  anchor-correct, however many notices it makes.

## Readings and deviations (each with its reason)

1. **Tuning seeds are 10000-10099, not 31000-31099.** The brief says "R6's tuning streams (31000-31099
   region, as the R6 scripts do)". The R6 scripts tune on 10000-10099 (`TUNING_SEEDS`, shared by R4 to
   R6) and use 31000-31099 for the ledger diagnostic. I read "tuning streams" as the tuning seeds. Both
   ranges are disjoint from the held-out 20000-20199; the choice affects which streams chose the
   parameters, nothing else.
2. **The notice record is three files, not new columns of `results.csv`.** The brief allows either;
   separate files keep the 62-arm byte gate on `results.csv` and `incidents.csv` literally true, and
   keep every earlier experiment's pinned hashes valid. `results.csv`'s columns are unchanged.
3. **The per-arm noticer is a manifest-level map (`noticers`), not a field of `StreamArmSpec`.**
   `crates/gordian-run/src/main.rs`, outside my territory, builds `StreamArmSpec` by a struct literal;
   a new field would not compile there. The map is not written when empty.
4. **The tuning rule is mine and was amended once** (above): fixed before the tuning run, one
   candidate excluded (the control l = 0) after seeing the tuning table and before any held-out run.
   For ChangeTriggered the rule gave a degenerate row (no feasible q); I kept it for the table rather
   than rewrite the rule to produce a more flattering row, and show the whole grid beside it. A reader
   who wants ChangeTriggered "at its best" has the q <= 2 s rows; those are floods.
5. **ChangeTriggered, readings stated in `noticer_change.rs`:** a node never seen is quiet (this is the
   8.6-per-stream floor); a quiet period of exactly q is quiet; every onset is its own anomaly even
   inside another anomaly's burst (a cascade makes one notice per quiet node); an abnormal observation
   that is not an onset and joins no anomaly is dropped; retirement uses the rung's quiet time, not q.
6. **EarliestAnchor, readings stated in `noticer_rung.rs`:** "earliest" is the smallest observation id
   among abnormal, same-site, earlier-than-the-rung's-anchor observations within l; the abnormal
   observations at the site between it and the rung's anchor are copied into the anomaly's evidence so
   that the anchor remains its first attached observation (a stray can then belong to two anomalies);
   the site, region and burst timing do not move.
7. **Anchor-correct uses the anchor's emission instant and the incident's first observation by id**,
   inclusive 1 s, absolute difference; instants come from the stream's public observation list, which
   the harness holds (the truth has none, evaluator judgement 8).
8. **Notices on `oracle_notice` arms are the rung's;** that arm's injected notices are not logged
   (it is not in the table).
9. **Selection delay is R5's 16 s for every noticer**, as the brief says (quality rows comparable with
   R10); see "Not shown".

## Verification

**By running.**

- The byte-identity gate: R6 held-out, 62 of 62 arms, twice (seam binary, final binary); R10 held-out,
  6 of 6.
- Evaluator: 29 hand-written cases (`fixtures/notice-cases.json`, worked out from the rules, covering
  every rule N1 to N12 and all ten error variants and the order of checks) and 9 tests; cargo-mutants
  on `notice.rs`: 41 mutants, 39 caught, 2 unviable (a first run had 44 mutants and 3 survivors, two of
  them equivalent under the ordering N11 enforces, so the code was simplified, and one on `Display`,
  so a test was added); 25 hand-applied mutants (window width and inclusivity, accumulation, which
  observation is first, tier counting, every check of N11), 25 of 25 caught (`b1-hand-mutants.csv`).
  Hand-applied mutant 7 ("latency from the anchor") is crude (it clamps the latency) and is caught for
  that reason as much as for the intended one.
- Noticers: 17 tests in `tests/stream_noticer.rs` (specs, manifest, the threshold, ChangeTriggered's
  onset rule and boundary, cascade, retirement, EarliestAnchor's reach and boundary, the control l = 0
  equal to the rung, the files' columns and sums, replay byte for byte, and that an arm with no
  `noticers` entry writes the same five files in a run where another arm has one).
- Analysis: 23 tests in `analysis/tests/test_notices.py` (schema guard against `results.rs`, loading,
  refusals, hand-counted measures).
- Gates on exit codes at the final tree: `cargo fmt --all -- --check` 0; `cargo clippy --locked
  --workspace --all-targets -- -D warnings` 0; `cargo test --locked --workspace --no-fail-fast` 0 (602
  tests); `bash scripts/check-no-oracle.sh` 0; `python -W error -m pytest -q analysis` 362 passed, 4
  deselected, **with `PYTHONPATH=analysis`**: the shared virtual environment installs the main
  checkout's `gordian_analysis` (editable), so from a worktree the command without it imports the
  main checkout's copy, which has no notice loader (it exits 2 at collection, as it did here before
  the path was set). On the merged tree in the main checkout the command is the plain one.
- All runs, exit status and wall time (`b1-driver-log.csv`, `b1-run-index.csv`; outputs under
  `artifacts/runs/` of the worktree, git-ignored):

  | run | exit | wall s | what |
  |---|---|---|---|
  | `xcheck-r6-heldout-b5-rho0.7` | 0 | 252 | the gate, seam binary |
  | `b1-tune-b5-rho0.7` | 0 | 60 | 18 arms, 100 tuning streams |
  | `b1-heldout-b5-rho0.7` (first attempt) | **13, refused by the driver** | 0 | the working tree was not clean: I edited documentation while the script was waiting for another worker's process. Nothing ran; the manifest was set aside as `.unrun-stale-revision` and rewritten for the new HEAD |
  | `b1-heldout-b5-rho0.7` | 0 | 121 | 18 arms, 200 held-out streams |
  | `xcheck2-r6-heldout-b5-rho0.7` | 0 | 257 | the gate, final binary |
  | `xcheck-r10-heldout-b5-rho0.7` | 0 | 23 | R10's held-out run replayed |
  | `b1-heldout-extra-b5-rho0.7` | 0 | 10 | the rung at z = 1 and 0.5 |

  None was excluded or step-capped. Waits for other workers' processes: the run script polled the
  other worker's build and benchmark 4 times before each held-out attempt (30 s each); builds and
  tests waited a few times for the same reason (`b1_cg.sh`). I killed nothing.
- **A shared-scratchpad hazard.** Another worker overwrote `cg.sh` in the shared scratchpad directory
  while I was working (it made the script `cd` into its own worktree). My gates and builds before that
  had run in this worktree (their logs name its paths); I then used a wrapper with a unique name and
  checked that the release binary was current for this tree (`cargo build` reported nothing to do).

**Assumed.** That the cgroup runner's limits were applied as the driver reports (usage files exit 0,
no OOM kills); that the public background rate of about 0.1 abnormal observations per node per second
(from `stream/mod.rs`) is the right order in point 4; that the noticers' own bookkeeping (not in the
modelled cost, as for the rung) does not matter at the cost resolution of the table; that
interleaving order does not affect results (the gate shows it for 62 arms; the new files are
byte-identical on replay in a test).

**Hidden record.** I did not read `crates/gordian-stream/HIDDEN-DESIGN.md`. I read the review log's R10
entry and R6's, which describe hidden structure (the mis-anchoring on background about 0.3 s earlier,
the leak's sub-alarm readings): I treated them as hypotheses. No noticer here uses them; the
evaluator reads the truth, as it is meant to. The `anchor_offset` and mis-anchoring diagnostics read
evaluator output.

## What I am least sure of

- **ChangeTriggered's reading that a never-seen node is quiet** sets a floor of ~8.6 background notices
  per stream and shapes where the tuning rule ends up. Under the other reading (the stream's start
  counts as a last observation at every node) the floor goes away and a q within the budget exists
  (around 32 to 64 s), but its anchor-correct share would still be a few percent, because a node that
  has been quiet for 30 s or more is rarely where an incident begins. I did not run that reading.
- **The tuning rule** (a background budget) is a design choice with consequences for M2. It excludes
  floods from the table by construction. Another rule (maximise anchor-correct share outright) would
  put the q = 0.5 s row in the table.
- **The selection-oracle quality column** is confounded as described; I did not re-tune the delay per
  noticer (the brief says R5's value).
- **The attribution in point 4** (strays) is a fit to a rate, not an inspection.
- **Held-out sample size.** 372 hard non-leak incidents and 139 leaks: the intervals are 4 to 8 points
  wide on the shares. A difference of 0.03, M2's margin, is inside them.

## What the chief should examine most carefully

1. **M2's comparator rule.** "The best public noticer by that result's own measure, at no more
   notices on background per stream than that noticer" picks the flood for anchor-correct and for leak
   noticed if the flood is a candidate, and then result 1 (needs 0.03 above 0.984) and result 2 (needs
   0.20 above 1.000) are unreachable. With only the four table rows the comparators are RungNoticer
   z = 2 (anchor-correct 0.914 at 9.03 background notices) and the rung rows (leak noticed 0.460 at
   4.00 or 9.03). I recommend the criterion compare the medium with the public frontier at the
   medium's own background rate (the sensitivity table is that frontier: anchor-correct 0.890 at 4.0,
   0.914 at 9.0, 0.909 at 30.9, 0.927 at 422), and that result 2 also report leak anchor-correct, which
   is 0.000 for every abnormal-only noticer. That changes M2's fixed criterion, which is yours to do
   or not; I have not touched it.
2. **That the seam moved the rung and nothing else**: the gate hashes cover `results.csv` and
   `incidents.csv` only. The diff of `rung.rs` (`Anomaly` split into `Tracked` and `Down`; the
   crediting of an answer to a *candidate* the noticer has not yet noticed is preserved on purpose: the
   R4 oracle's rows depend on it, and the gate covers it).
3. **The evaluator's N1 and N5**, and in particular that only the anchor counts and that the window is
   on the anchor's instant.
4. **The tuning-rule amendment** (the control l = 0) and the ChangeTriggered fallback row.
5. **The shared-scratchpad overwrite** and the `PYTHONPATH` note for the analysis gate.

## What I would test next

- The selection oracle with a delay re-tuned per noticer, or a notice-relative rule that does not
  depend on retirement, so that the quality column measures noticing.
- A noticer aimed at the failure found: the 17 of 31 never-noticed incidents with a background notice
  within 1 s. A *later* re-anchor (onto the densest burst among the attached observations, with the
  window widened, or splitting a candidate that holds a stray and a burst) is the public fix, and it
  needs no new information. The rung already re-anchors on the densest burst within 0.4 s; the stray
  within 0.4 s before the burst outweighs it. A test: the same measure on that noticer, and the
  10 never-noticed incidents with no nearby background notice, which this unit did not explain.
- The rung at z between 1 and 3 and ChangeTriggered with the other reading of a quiet node, to fill the
  frontier between 9 and 400 background notices per stream.
- Anchor-correct plus a site check (does the notice's site equal where the incident's evidence is),
  and a precision measure (the share of notices on incidents that are anchor-correct) in the
  evaluator, so that a flood is visible in one number.
