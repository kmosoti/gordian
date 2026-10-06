# M2: the medium as a noticer on the stream world

Exploration. Nothing here may later be cited as confirmation. Lab 1, unit M2 of `docs/lab-queue.md`,
branch `medium-noticer`. The freeze commit is `c5bcb11`. Scripts: `scripts/m2_*.py`. Outputs:
`experiments/exploration/m2-*`. Run directories are in this worktree's `artifacts/runs/` (git-ignored);
the tuning runs and the first replay are archived in `m2-tuning-and-xcheck.tar.xz`.

## The criterion, as it stood when the held-out run started

The chief re-fixed the comparator before any M2 held-out run, under B2's acceptance clause. Lab 1
was told directly while tuning, after two tuning stages. The comparator is now `ReanchorNoticer`
from B2's table: anchor-correct 0.952 [0.932, 0.970], leak noticed 0.460, 6.82 background notices
per stream. It replaces `RungNoticer` at z = 2 (0.914, 0.460, 9.03). The bound on the medium's
background notices is now 6.82. Margins, settings, seeds and the tick sweep are unchanged.

- **Result 1.** Anchor-correct share on hard non-leak incidents is at least the comparator's
  plus 0.03, and the paired lower bound of the difference is above 0.01.
- **Result 2.** Leak noticed share is at least the comparator's plus 0.20, and the paired lower
  bound is above 0.10.
- Both results need the medium's notices on background per stream to be at most 6.82.
- Both are tested at 100 ms, 500 ms and 2 s.

Readings fixed before the held-out run (`scripts/m2_common.py`):

- The "paired lower bound" is the 5th percentile of the paired 90% cluster bootstrap of (medium
  minus comparator). It uses 10,000 resamples of whole streams with the same counts for every
  arm. This is `b2_stats.Measures`, which is B1's arithmetic.
- The background bound applies to the point estimate.
- The comparator is the re-anchor arm of the same run. It reproduces B2 exactly: 0.952, 0.460,
  6.82.
- "Holds" for the experiment means it holds at the best tick length, where "best" is the tick
  length at which both results hold.

## Results (200 held-out streams, seeds 20000–20199; b = 5, ρ = 0.7; selection oracle at 16 s; the rung's context)

### Against the comparator (`m2-criterion.csv`)

| tick | Result 1: anchor-correct, medium − re-anchor | Result 2: leak noticed, medium − re-anchor | medium background / stream | verdict |
|---|---|---|---|---|
| **100 ms** | 0.992 − 0.952 = **+0.040 [+0.022, +0.060]** | 0.993 − 0.460 = **+0.532 [+0.462, +0.603]** | **5.44** [5.15, 5.74] | **both hold** |
| 500 ms | 0.981 − 0.952 = +0.0296 [+0.011, +0.049] | 0.986 − 0.460 = +0.525 [+0.455, +0.595] | 5.33 | result 1 **does not hold** (point 0.0296 < 0.03); result 2 holds |
| 2 s | 0.909 − 0.952 = −0.043 [−0.075, −0.011] | 1.000 − 0.460 = +0.540 [+0.467, +0.610] | 4.29 | result 1 does not hold; the medium is worse on anchoring; result 2 holds |

**Verdict as written.** Both results hold at the best tick length, 100 ms, with the sweep shown.
Result 2 holds at every tick length. Result 1 holds only at 100 ms:

- At 500 ms it misses the point margin by 0.0004, although its lower bound clears 0.01. That is
  "not shown", not equivalent.
- At 2 s the medium anchors worse than the comparator.

Beside it, against the first comparator, rung z = 2 (0.914, 0.460, 9.03):

| tick | Result 1 difference | Result 2 difference |
|---|---|---|
| 100 ms | +0.078 [+0.054, +0.102] | +0.532 |
| 500 ms | +0.067 [+0.044, +0.091] | +0.525 |
| 2 s | −0.005 [−0.038, +0.027] | +0.540 |

### Every row (`m2-heldout-table.csv`, 90% intervals there)

Columns: AC is hard non-leak anchor-correct, LN is leak noticed, leak AC is leak anchor-correct,
BG is background notices per stream, strict prec. is strict precision (B2's N16), per inc. is
notices per incident. Quality, the medium's own cost and latency are in the next two tables.

| arm | hard noticed | AC | LN | leak AC | BG | precision | strict prec. | per inc. |
|---|---|---|---|---|---|---|---|---|
| rung z = 3 | 0.917 | 0.890 | 0.460 | 0.000 | 3.99 | 0.855 | 0.748 | 0.89 |
| rung z = 2 | 0.946 | 0.914 | 0.460 | 0.000 | 9.03 | 0.740 | 0.613 | 0.97 |
| **re-anchor (comparator)** | 0.978 | 0.952 | 0.460 | 0.000 | 6.82 | 0.802 | 0.672 | 1.04 |
| **medium 100 ms** | 0.997 | 0.992 | 0.993 | 0.554 | 5.44 | 0.889 | 0.501 | 1.65 |
| medium 500 ms | 0.987 | 0.981 | 0.986 | 0.489 | 5.33 | 0.879 | 0.537 | 1.45 |
| medium 2 s | 1.000 | 0.909 | 1.000 | 0.273 | 4.29 | 0.966 | 0.163 | 4.56 |
| 100 ms, sliding instead of ordered (ablation) | 0.995 | 0.987 | 0.993 | 0.554 | **14.69** | 0.790 | 0.356 | 2.08 |
| 500 ms, sliding (ablation) | 0.992 | 0.981 | 1.000 | 0.489 | **30.31** | 0.654 | 0.270 | 2.15 |
| 2 s, sliding (ablation) | 1.000 | 0.925 | 1.000 | 0.273 | **22.74** | 0.857 | 0.134 | 5.12 |
| 100 ms, abnormal-only sense (control) | 0.997 | 0.992 | **1.000** | **0.000** | 5.38 | 0.892 | 0.480 | 1.68 |
| 500 ms, abnormal-only (control) | 0.989 | 0.981 | 1.000 | 0.000 | 5.23 | 0.883 | 0.515 | 1.48 |
| 2 s, abnormal-only (control) | 1.000 | 0.887 | 1.000 | 0.000 | 1.23 | 0.984 | 0.233 | 2.82 |
| 100 ms, binned on the 10 s rhythm (sensitivity) | 0.984 | 0.976 | 0.993 | 0.554 | 8.59 | 0.849 | 0.419 | 1.82 |

### Selection-oracle quality and cost

| arm | quality (hard, 16 s) | leak quality | cost s / stream | medium's own ns / stream |
|---|---|---|---|---|
| re-anchor | 0.551 | 0.223 | 0.670 | — |
| medium 100 ms | 0.591 | 0.755 | 1.848 | 16.7 ms [10.6, 23.2] |
| medium 500 ms | 0.589 | 0.669 | 1.786 | 7.2 ms |
| medium 2 s | 0.642 | 0.705 | 3.795 | 3.9 ms |

How to read these two columns:

- The quality column is confounded, as B1 warned, by the fixed 16 s delay and by retirement.
- The cost column in `results.csv` is the reasoner plus components and does not include the
  medium. The medium arms cost more because they open more anomalies per incident, and the
  oracle then asks about hard ones more often.
- The medium's own cost is replayed exactly from public observations (`m2-cost.csv`), and a test
  shows it equals the in-run charge on the bill. It is the operations at 200 / 25 / 40 / 2 ns
  plus 200 ns per tick, with no truncated tick on any stream.
- Per stream at 100 ms: 5,995 ticks, 72,000 cell updates, 27,800 traversals and 10,373 routings.
  That is 36 times the rung's substrate cost (0.46 ms) and 0.9% of the stream's total.

### Anchor correctness as a function of the lookback (100 ms; sensitivity rows, nothing chosen from them)

| burst-path lookback | 0 (frozen) | 100 ms | 200 ms | 500 ms | 1 s |
|---|---|---|---|---|---|
| anchor-correct | 0.992 | 0.992 | 0.987 | 0.970 | 0.930 |
| background / stream | 5.44 | 5.79 | 6.05 | 7.17 | 8.46 |

A longer lookback reaches back to a stray before the burst. Anchor-correct falls and background
rises together, because the notice is then anchored on the stray. This is the M1 PI's warning
about pruning, seen from the other side: the anchoring rule ("earliest event in the support") is
only as good as the support's lower edge.

### Latency (`m2-latency.csv`, first notice from the incident's first observation)

| | hard non-leak median (p90) | slow leak median (p90) |
|---|---|---|
| re-anchor | 0.51 s (5.0) | 14.2 s (17.2) |
| medium 100 ms | 0.28 s (0.49) | 9.8 s (18.7) |
| medium 500 ms | 0.32 s (0.65) | 10.5 s (23.4) |
| medium 2 s | 1.32 s (11.7) | 12.8 s (13.8) |

### Discordance with the comparator (hard non-leak incidents)

| tick | medium right, re-anchor wrong | re-anchor right, medium wrong | both wrong |
|---|---|---|---|
| 100 ms | 18 | 3 | 0 |
| 500 ms | 17 | 6 | 1 |
| 2 s | 15 | 31 | 3 |

At 100 ms the medium anchors correctly on every one of the comparator's 18 failures. It fails on
3 compound incidents that the re-anchor gets right.

## What was built

**Lab 1's directory, `crates/gordian-run/src/stream/arms/medium/`:**

- **`adapters.rs`: sense, clock and ledger.**
  - **Sense.** Every delivered observation becomes an event carrying its value, benign readings
    included. Its verdict is a tag. An abnormal observation also carries a tag for its kind (one
    of five counters, a message, a snapshot).
  - **Addresses.** The node is the service. The channel is counter, message, snapshot or probe;
    probe is reserved, because probes never reach a noticer through the seam.
  - **Message ids.** A catalogue id becomes `0x4000_0000 | id`, which is exact and never
    collides. A free-form id becomes `0x8000_0000 | ((lo ^ hi) & 0x7FFF_FFFF)`; two free-form ids
    collide when their folds are equal. No cell in M2 reads a message tag.
  - **Events.** `seq` is the observation id, so an `EventRef` names its observation.
  - **Clock.** A tick runs once it is complete at the harness's instant. Every tick runs, empty
    ones included.
  - **Ledger.** Counts at the declared prices, plus 200 ns per tick.
- **`noticing.rs`: `MediumNoticer`, the effector.**
  - A notice proposal becomes an anomaly: its anchor is the proposal's anchor, its site is the
    anchor's service, and its attached evidence is the anchor plus the proposal's refs.
  - A retire proposal from the latch of service `n` makes the live anomalies sited at `n`, or
    made by `n`'s emitters, ready to retire.
  - Later abnormal observations attach by the rung's own `attach_target`.
  - When the bill refuses, the medium stops for the rest of the segment.
- **`graph.rs`: the parametric graph and `MediumParams`.** The cell table is in the module
  documentation.
- **`mod.rs`.**
- **Tests: `crates/gordian-run/tests/stream_medium.rs`, 18 tests plus two tools** (a dump of
  public observations, and the cost replay). They cover:
  - the encoding and tags, and the fold rule;
  - the clock;
  - benign readings reaching the medium, and the control dropping them;
  - anchoring past a stray;
  - retirement after the hold;
  - a ramp noticed, with noise and the control not noticing one;
  - every coincidence form;
  - the burst window read from offsets at every tick length, with the same-tick stray limit
    stated;
  - confirmation, and the anchor staying at the service;
  - the manifest round trip;
  - the bill charge equalling the replay at every tick length;
  - refusal stopping the medium;
  - determinism;
  - a non-medium arm's five files unchanged beside a medium arm;
  - the frozen media and every held-out variant validating, with no rhythm, phase gate or
    oscillator in a frozen graph.

**Outside the directory.** The seam has no way to construct an outside noticer or to charge a
noticer's work, so the smallest additive hooks were needed. Each is inert for B1's and B2's
noticers. This is a deviation from the brief's file list:

- `arms/noticer.rs`:
  - the variant `NoticerSpec::Medium(MediumParams)` and its arms in `id`, `validate` and `build`;
  - two defaulted `Noticer` methods: `take_cost` returns `None`, and `refused` does nothing;
  - the struct `NoticerCost { component, compute_ns }`.
- `arms/rung.rs`: `take_noticer_cost` and `noticer_refused`, which pass through to the noticer.
- `arms/mod.rs`:
  - `pub mod medium;`;
  - in `StreamArm::step`, after `rung.notice(now)`, four lines that charge the noticer's cost
    through the meter and tell the noticer when it is refused.
- `meter.rs`: `Meter::charge_noticer`. It charges `Phase::Component(ComponentId(16))` and
  advances the clock on acceptance, like a component's busy time.
- `crates/gordian-run/Cargo.toml`: the `gordian-medium` dependency, with its justification.
  `Cargo.lock` gains one dependency line.

No field was added to `manifest.rs` or `spec.rs`. The medium is chosen through the existing
`noticers` map, spelled `{"noticer": "medium", ...}`.

**Merge.** Main was merged into the branch at `11a638f`, bringing B2 and `noticer_reanchor.rs`.
The one conflict was in `noticer.rs`, and both variants were kept.

## The frozen graph (per service; `m2-selected.json`)

**At 100 ms**, three paths and a retiring latch:

- **The burst.** Abnormal observations of two distinct kinds at the service within 20 ms. The
  four kinds are error rate, latency, a message, and any other counter or snapshot. They are
  combined in an ordered coincidence, which reads `offset_ns`, with lookback 0.
- **The confirmation.** The burst reaches the emitter two ticks later, through a relay, gated by
  a latch held 200 ms after an alarm at any other service. A gate carries no references, so the
  anchor stays at the service.
- **Three kinds within 20 ms** pass ungated.
- **The ramp.** For each counter at the service, an integrator (τ 4 s, threshold 3 readings,
  lookback 10 s) gains +1 per reading and loses 0.3 per unit of a step above 10 between
  consecutive readings. This finds readings that come often and move little.
- **The retiring latch.** Hold 6 s; it proposes retire. Refractory 1 s.

The other tick lengths differ as follows:

- **500 ms:** confirmation by dependents in the burst's own tick (delay 1); two kinds within 25 ms,
  three kinds within 30 ms.
- **2 s:** three kinds within 30 ms with no confirmation. The ramp has no jump penalty and a
  threshold of 2.45, because a 2 s tick sums two readings of a ramp.

The conversion table is in `m2-conversions.csv`. Every time constant and span converts; leaks
are 0.9753, 0.8825 and 0.6065 per tick at 4 s. There are no time-converted synapse delays, so no
delay collapses. The confirmation delay is given in ticks.

**Oscillome elements:**

| element | role in the frozen graph | tuning evidence |
|---|---|---|
| ordered coincidence | in, at every tick length | on the tuning streams it beat sliding (14–30 background notices) and binned (100 ms: 0.970 at 8.36, or 0.945 at 3.22) |
| rhythms | none | the binned form was the only element that uses them, and it lost |
| phase gates | none | no candidate role was found in a world whose incidents start at random |
| oscillators | none | no candidate role was found for noticing |

So "rhythms off", "phase gates off" and "oscillators off" are the frozen arms themselves, and were
not rerun under another name. Retirement uses M1b's retiring latch.

## Tuning record (seeds 10000–10099 only; tables `m2-tuning-tune-*.csv`, points `m2-tuning-points.csv`)

| stage | what was learned |
|---|---|
| a | The onset integrator with dependents was poor. A bug: a notice was suppressed when a live anomaly merely held its anchor, which re-imported the rung's mis-anchoring. Fixed before b (notices are now suppressed only on an equal anchor). The ramp was 1.000 on the leak at 0.0–0.8 background. |
| b | The re-fixed comparator was run from here on. The medium's misses (8) and the re-anchor's (10) were disjoint. Threshold 2 got 0.960 at 24 background. |
| c | The public observations show onsets as two or more kinds within ~20 ms at one service. The burst with a 20 ms window got 0.995 at 14 background. |
| d | Confirmation (dependents, or all others; hold) plus three kinds unconfirmed brought background to 2–6. |
| e | Windows, refractory and the binned form at 100 ms; confirmation delay and ramp variants at 500 ms and 2 s. |
| f | 2 s only. |

The rule was fixed before the first tuning run: maximise the minimum of the two normalised
margins over the comparator, within a background budget. The budget was 7.5 against the first
comparator and was reset to 5.6 at the same margin when the comparator was re-fixed. It chose:

- 100 ms: 0.980 / 1.000 / 5.58;
- 500 ms: 0.955 / 1.000 / 4.87;
- 2 s: 0.869 / 1.000 / 4.26.

At 2 s no configuration scored above the comparator. Two readings were added after the tables
were seen and before any held-out run; both are stated in `m2_select.py`:

- Stages b to f are the candidate set.
- Residual ties go to the earlier stage, then the arm name. Three tied at 100 ms, differing only
  in a window no stream exercised.

## Runs (all through `scripts/run-driver.sh`, release build, `artifacts/runs/_logs/m2-runs.log`)

| run | exit | wall | waits (30 s) |
|---|---|---|---|
| m2-xcheck-r6-heldout-b5-rho0.7 (byte identity, seam binary) | 0 | 254 s | 6 (another lab's `cargo test`) |
| m2-tune-a … m2-tune-e | 0 each | 75–248 s | 11, 0, 0, 0, 1 |
| m2-tune-f, first attempt | **3, refused by the driver**: I passed a manifest path relative to the wrong directory; nothing ran | 0 | 0 |
| m2-tune-f | 0 | 65 s | 0 |
| **m2-heldout-b5-rho0.7** (17 arms) | **0** | 150 s | 2 |
| m2-xcheck2-r6-heldout-b5-rho0.7 (byte identity, final binary, after the merge) | 0 | 213 s | 0 |

Nothing was excluded and nothing was step-capped. The cost replay ran under
`scripts/cgroup-run.sh` on cores 0-2 at 300% and 2 GB: exit 0, 32 s wall, 380 MB peak, no OOM.

## Verified by running versus assumed

**Verified by running:**

- **Byte identity.** R6's held-out run replays 62 of 62 arms (both `results.csv` and
  `incidents.csv`), twice: `m2-regression.csv` and `m2-regression-final.csv`.
- The bill charge equals the replayed counts at every tick length (a test).
- The comparator reproduces B2 exactly.
- **Gates on exit codes, final tree:** fmt 0; clippy `--locked --workspace --all-targets -D
  warnings` 0; `cargo test --locked --workspace --no-fail-fast` 0 (731 passed, 7 ignored);
  `check-no-oracle.sh` 0; `PYTHONPATH=analysis python -W error -m pytest -q analysis` 0 (389
  passed). The path variable is needed from a worktree, as B1 found.

**Assumed:**

- The cgroup limits held as the driver reports.
- The medium's wall time sits in `measured_sched_ns`, like other noticers' bookkeeping. It was
  not separately timed.
- A clock advance of a few microseconds per step for the medium's busy time does not matter to
  results. It applies to medium arms only.
- No mutation testing of the M2 code was done. The tests were checked by reasoning: for example,
  the control test fails if benign readings are not routed.

## Hidden record

I did not read `crates/gordian-stream/HIDDEN-DESIGN.md` or any generator source. I read W1, R6,
R10 and B1, whose text describes hidden structure (for example, the leak's sub-alarm readings and
a reading roughly every 0.8–1.5 s). I treated it as hypothesis.

The design was taken from three things:

- the tuning streams' public observations, dumped by a test that writes public fields only;
- the evaluator's notice files on the tuning streams;
- the public rules: verdicts, kinds and the public graph.

The graph treats all five counters alike. It does not single out the counter that ramped on the
tuning streams (saturation).

## PI's analysis

**What the medium's anchoring does differently.** The rung family scores a candidate over an 8 s
window, then re-anchors. The re-anchor moves an isolated anchor forward onto a later burst. The
medium never forms the candidate the rung forms. It fires on a sub-tick event: two or three kinds
of abnormal observation at one service within 20 ms, read from `offset_ns` by the ordered
coincidence. Its anchor is the earliest event of a support that a lookback of 0 cuts at the tick's
edge.

So a stray before the burst is outside the support unless it shares the burst's tick:

- At 100 ms that is rare. All 18 of the comparator's failures were fixed.
- At 2 s it is common, and anchoring loses 31 incidents to the comparator.

The lookback curve shows the same mechanism directly. The medium's three failures at 100 ms are
compound incidents whose onset is an error rate and a catalogue message 5–20 ms apart. That is the
same shape as the background's commonest coincidence (about 11 per stream). No public structure
within 300 ms separates them.

**Which oscillome elements earned their place.**

- **The ordered coincidence earned it**, at every tick length, and on background rather than
  anchoring. The sliding ablation keeps anchor-correct (0.987 / 0.981 / 0.925) but raises
  background to 14.7 / 30.3 / 22.7 per stream, far over the bound. The 20 ms coincidence is what
  makes the result legal.
- **The binned form on the 10 s rhythm** is the only rhythm use tried. It is worse on both
  measures, at 0.976 and 8.6 background.
- **Rhythms, phase gates and oscillators** have no role in the frozen graph. The M1b PI's
  prediction holds: they were removed, and the ordered coincidence stayed. It stayed at 100 ms
  too, not only at 500 ms and 2 s as predicted. The oscillator was never given a role, so this run
  says nothing about the 6–16 s horizon.

**What the abnormal-only control says.** It contradicts the synthesis decision's premise:

- With only abnormal observations the medium still notices 1.000 of leaks. The ramp continues
  above the alarm line as dense, smooth readings, and the density-and-smoothness detector finds
  it there.
- What the benign readings buy is **anchoring the leak at its start** (leak anchor-correct 0.554
  against 0.000) and about 9 s of latency (9.8 s against 18.6 s median).
- So result 2 is not a test of reading benign values. It is a test of a ramp detector against a
  z-scored candidate. The public rung family misses the leak because its abnormal readings are
  slow (about 1 per second) and its score does not reach the threshold, not because they are
  invisible.
- "If the adapter were abnormal-only, result 2 would be unreachable by construction" is false
  here. The leak-anchoring measure reported beside is where the synthesis decision shows.

**What it does not show.**

- One setting.
- A reasoner-free quality comparison: the quality column stays confounded, and the medium opens
  1.5–4.6 notices per incident against the comparator's 1.04.
- A comparison at matched precision. The medium's plain precision is higher, but its strict
  precision (0.50 against 0.67) is lower, because it notices plain incidents several times at
  dependent sites.
- Any learning. The graph is hand-designed and was tuned on 100 streams against a budget it sits
  close to (5.58 against 5.6 on tuning; 5.44 against 6.82 held-out).

**What I would test next.**

1. Strict-precision-matched rows: the medium with notices per incident capped by a per-incident
   latch inhibition, against the re-anchor. This tests whether the anchoring gain survives
   without extra notices.
2. A support pruned at sub-tick resolution, either an ordered coincidence whose support is cut to
   its window or a shorter tick. This would recover the 500 ms near-miss and the 2 s losses. It
   is a medium-crate change, so it belongs to a new unit.
3. The ramp detector as a public noticer, outside the medium. Result 2 should then be reachable by
   the status quo too; if so, it is a property of the detector, not of the substrate.
4. A second setting, and the held selection option from B2, for the quality column.

**What the chief should examine most carefully.**

1. **The hooks outside Lab 1's directory** (`noticer.rs`, `rung.rs`, `arms/mod.rs`, `meter.rs`)
   and the reason for them. Both byte-identity replays pass.
2. **The selection sitting at the edge of the tuning budget** (5.58 against 5.6), and the
   tie-break reading added after the tables.
3. **The ramp detector's design came from public tuning data** showing smooth ramps. Judge
   whether that is "learned from its own run history" or design knowledge.
4. **The 500 ms row.** +0.0296 against a margin of 0.03 is a miss, as written.
5. **The confirmation gate reads "any other service"** at 100 ms. This is a public-graph-free
   rule chosen over dependents (0.975 at 2.1 background on tuning) because it scored higher.
6. **The abnormal-only control's result**, which revises the synthesis entry's premise.
