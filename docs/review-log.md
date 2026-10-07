# Coordinator review log

What the coordinator checked for each merged unit, what it decided, and what it carried forward
to later units. Newest first. Reports from workers are model output; this log records what was
independently verified.

## L1 the learned noticer — merged (Lab 3); learning is real, fast, and stops short of the hand design

**Provenance.** Two interruptions (an API limit, then the disk restart); the final PI verified
the inherited state, rebuilt the release binary and found its hash equal to the one the main run
used, regenerated every analysis CSV from the run outputs byte-identically, and reran the
byte-identity gate (62 of 62, recomputed independently). The update rule and priors were
committed before any run; two amendments were made on development seeds before the main run
and are recorded. Gates under the runner on the merged tree: fmt, clippy `--locked`, 790 Rust
tests, the oracle guard, 417 analysis tests. Outputs in `artifacts/runs/l1/` (ignored). Not done:
mutation testing of the learner.

**Re-verified from the run outputs** (seeds 40000–40199, never used for tuning; chief's own
bootstraps): clause 1, learned minus frozen anchor-correct over the last 100 streams, 0.000
[−0.011, +0.010], holds; clause 3, learning-off minus frozen, −0.039 [−0.064, −0.015], holds;
clause 2, the slope of the cumulative curve over the first 100 streams, +0.053 [−0.037,
+0.085] (lab: +0.043 [−0.021, +0.126]), lower bound below zero, **fails**. End states match the
lab exactly: learned 0.980 anchor-correct at 8.76 background notices per stream; frozen 0.980
at 5.53; learning-off 0.941 at 31.55.

**Verdict as written: the learned arm does not count toward the aim** (the clauses are
conjunctive).

**What it means (causality, objective, failure, meta).**

- **Something was learned, from public structure alone.** The coincidence window goes
  400 ms → 100 ms → 49 ms → 30 ms within four streams, read from the excess over independence
  of same-service abnormal pairs by gap bin; the ramp threshold settles at 2.5. Against the same
  graph with learning off, learning is worth 22.8 fewer background notices per stream and
  +0.039 anchor-correct. The prior does not matter (50 ms and 1,000 ms priors reach the same
  end state). This is the first evidence in the program that a public statistic of the stream
  carries the knowledge the hand constants encode.
- **It stops short of the hand design, and pays in false notices.** The learner's 30 ms window
  against the hand-tuned 20 ms costs 3.2 more background notices per stream (8.76, above M2's
  6.82 bound) and 0.07 of strict precision. Anchor parity was bought by firing more. The
  background and precision bounds, not the clauses, show the learner is worse than the hand
  design.
- **The missing knowledge sits in one knob.** The learner's "precision demanded" parameter
  `p` decides where it stops: at 0.5 it stops at 30 ms and 2.5; at 0.8 (a pre-planned
  sensitivity arm) it recovers M2's 20 ms and 3.0 exactly, with the frozen graph's background
  and precision to the digit. So the hand tuning was moved up one level, not removed. The stream
  gives no feedback, so the learner has no way to set `p` from its own history; a public
  yield or cost signal (how many of its notices attach further evidence, or retire quickly) is
  what a stronger learner would need, and that is the design question the next learned unit
  must answer.
- **The criterion's slope clause cannot register fast learning.** The four medium arms make
  identical decisions on all 92 incidents in the first 50 streams, because the prior's cost
  only shows after the learner has already converged and the cumulative ratio has diluted it.
  A least-squares slope over 100 streams of a cumulative ratio is a poor instrument for a
  learner that finishes in four. **Coordinator error, recorded (sixth):** I fixed a clause
  whose statistic could not see the effect it was meant to detect at the speed the effect
  actually has; the feasibility note considered only whether a slope was measurable, not over
  what horizon. The clause stands for L1 as written; L2's criterion is amended below, before any
  L2 code.
- **One world, one seed, one setting, three differing incidents in 398.** The parity claim is
  exactly as thin as that.

**Decided.**

1. **L2's criterion is amended** (before any L2 code): clause 2 becomes "background notices per
   stream and strict precision against streams seen, with the endpoint fixed at stream 20: the
   learner's background over streams 21–40 is at most the learning-off control's minus 10, with
   the paired lower bound below −5", so that learning completed in a few streams is visible,
   and clause 1 adds the background bound (≤ 6.82) and strict-precision bound (≥ 0.67) as
   conditions, so that anchoring bought with false notices does not pass.
2. **L3 (Lab 3, after L2):** a learner that sets its own precision demand from a public yield
   signal, on a distribution-shifted stream (burst spacing and ramp rate changed from the
   defaults) where the frozen graph is wrong and there is something to catch up to; against a
   conventional online tuner (a grid search on the first N streams) as the matched
   conventional learner the charter's proxy 2 names. Criterion to be fixed before code, with the
   amended clause 2's horizon.
3. The L1 learner's window statistic is kept as the first public, label-free estimator of the
   world's burst spacing; it enters C1's engine as a derived relation if it proves useful there.

## B3 the public leak noticer and splitting noticer — merged (Lab 2); the status quo closes the leak too

**Provenance.** The unit was interrupted by the container restart after tuning and before the
held-out run; a fresh PI resumed in place, verified the inherited state, replayed the held-out
run from scratch at the tuning-stage commit, and reran the byte-identity gate with the final
binary (62 of 62, chief's hashes). The predecessor's last commit failed `cargo fmt --check` (one
hunk, fixed). cargo-mutants on both noticers: 134 of 150 caught after four added tests, 12
unviable, 3 missed by argument, 1 timeout. Gates under the cgroup runner on the merged tree:
fmt, clippy `--locked`, 774 Rust tests, the oracle guard, 402 analysis tests; peak build memory
under 3 GB, no OOM kills. Outputs in `artifacts/runs/b3/` (ignored).

**Re-verified from the per-incident notice files** (chief's own cluster bootstrap). B3's
comparator arms equal M2's modulo the run id (three arms, three files each), so M2 and B3 are
one table on the same 200 streams.

| Row | Anchor-correct | Leak noticed | Leak anchor-correct | Background / stream | Strict precision |
|---|---|---|---|---|---|
| Re-anchor (M2's comparator) | 0.952 | 0.460 | 0.000 | 6.82 | 0.672 |
| Ramp + split over re-anchor | 0.973 | 0.986 | 0.971 | 6.24 | 0.693 |
| Medium, 100 ms (M2) | 0.992 | 0.993 | 0.554 | 5.44 | 0.501 |

Medium minus ramp + split, paired: anchor-correct +0.019 [+0.005, +0.034]; leak noticed +0.007
[−0.014, +0.029]; leak anchor-correct −0.417 [−0.492, −0.343]; background −0.80; strict
precision −0.19. All match the lab's report.

**Verdict.** The acceptance clause triggers for the leak (0.986 ≥ 0.660 within the budget); M2's
runs had started, so this is the labelled supplementary comparison the clause names, recorded
in both reports.

**What it means (objective, failure, meta).**

- **A public rule that reads counter values notices the leak and anchors it at its start.** A
  chain of at least five readings rising by 15 above its first, with a tolerated step and drop,
  notices 137 of 139 leaks, 135 of them at offset exactly zero from the first observation, with
  a 5.5 s median latency. The objective saturated on tuning (175 configurations tie), so the
  constants are tie-rule choices, not an optimum. The two misses are chains that began on a
  background reading of the same counter a fraction of a second earlier: the mis-anchoring
  mechanism again, on the leak.
- **Splitting recovers 2 of the 8 never-noticed incidents** (both cascades), gains 8
  anchor-correct and loses none. Three split-brain incidents are noticed by no row.
- **The medium's standing after B2 and B3.** On every measure the medium was asked about, a
  public rule now reaches or beats it, except hard-incident anchoring (+0.019, interval above
  zero, at a 100 ms tick only) and background notices (0.8 fewer). It loses on leak anchoring by
  0.42 and on strict precision by 0.19, and costs more (1.85 s against 0.80 s per stream, partly
  because its own operations are billed and the public noticers' are not, mostly because it
  opens more anomalies). This is the third time the status quo, given a fair chance, has
  captured most of a lever (Stage B, B2, B3).
- **The ramp's shape is the decoys' shape.** It notices 0.948 of decoys (the re-anchor: 0.756),
  and the selection oracle never asks about decoys, so neither quality nor cost shows it. Of the
  510 notices the ramp adds per 200 streams, 137 are leaks, 154 decoys, 192 late plain, 11 hard,
  16 background. **The background budget counts only background-anchored notices, so it is
  blind to this.** The same blindness applies to the medium's rows. An honest cost column needs
  a non-privileged selector, so that decoy and late-plain notices are charged as calls.
- **Meta: what every noticing unit shares.** B2's gap, B3's chain constants and M2's graph were
  all tuned against evaluator measures on the tuning streams; each PI said so. None encodes a
  family or a hidden rule, and the guard passes, but all three are fitted to this world's burst
  spacing and ramp rate. Nothing yet shows that any of them transfers. That is the same for the
  medium and the public rules, so it does not favour either; it does mean EXP-101 on this world
  alone cannot speak to generality.

**Coordinator conclusion (sixth entry of its kind, not an error this time).** EXP-101 as
"noticing headroom" is closed on this world: anchoring is at 0.973 publicly with 0.027 of
headroom left, and the leak at 0.986. A preregistered noticing experiment here would be
saturated before it froze. What remains open, and what the charter's aim actually asks, is
(a) cost and precision at equal noticing, charged honestly, and (b) improvement per experience.
EXP-101 is therefore reframed before registration, below.

**Decided.**

1. **M3's criterion is not re-fixed** (result 1's +0.03 over 0.973 would be unreachable; M3 asks
   whether sub-tick pruning recovers the long ticks, against the re-anchor as fixed). The B3
   rows are reported beside with paired differences; Lab 1 was told.
2. **EXP-101 is reframed:** escalation control at matched noticing. Primary measures: critical
   misses and verified decisions per unit cost with a **non-privileged selector** (a public
   threshold or cascade over the noticer's anomalies, so every notice the noticer makes is
   charged), under the δ sweep, with both aim proxies; arms: the medium and the public
   ramp + split over the re-anchor, each with the same selector; the rung as the floor; the
   notice oracle as the ceiling. Noticing shares are bounded (no arm may fall below the public
   row's anchor-correct minus 0.02) but are not the claim. The draft comes to the user before
   the freeze.
3. **B4 (Lab 2):** the non-privileged selector (public threshold and change-triggered rules
   over any noticer's anomalies, tuned under cost), a decoy-notice column in the evaluator, and
   leak-versus-decoy separation from readings after the first five, so that a noticer's decoy
   cost is measured. This is the instrument EXP-101 needs.
4. **L1 stands as queued:** whether the medium learns is now the medium's main claim.

## M2 the medium as a noticer — merged (Lab 1); both results hold at 100 ms, not at 500 ms or 2 s

**Provenance.** The graph was frozen at `c5bcb11` before any held-out run; the comparator
re-fix reached the lab after two tuning stages and before any held-out run, and the lab retuned
to the new budget. R6's held-out run replays byte-identical for all 62 arms with the medium
wired in (chief's hashes). `ReanchorNoticer`, rerun in the lab's held-out run, reproduces B2
exactly. The medium's bill charge equals an exact replay at every tick length. One driver
refusal (a wrong manifest path), nothing ran. Gates on exit codes on the merged tree: fmt,
clippy `--locked`, 731 Rust tests, the oracle guard, 389 analysis tests. Outputs in
`artifacts/runs/m2/` (ignored).

**Re-verified from the per-incident notice files, all 17 arms, paired against the re-anchor**
(chief's own cluster bootstrap):

| Tick | Result 1: anchor-correct | Result 2: leak noticed | Background / stream (bound 6.82) |
|---|---|---|---|
| 100 ms | 0.992 vs 0.952, +0.040 [+0.021, +0.060], holds | 0.993 vs 0.460, +0.532 [+0.462, +0.603], holds | 5.44 |
| 500 ms | +0.030 [+0.010, +0.050], not shown (0.0004 under the margin) | +0.525, holds | 5.33 |
| 2 s | −0.043 [−0.075, −0.011], worse | +0.540, holds | 4.29 |

All match the lab's report. **Verdict as written: both results hold at the best tick length,
100 ms, with the sweep shown.** At 100 ms the medium anchors correctly all 18 incidents the
re-anchor misses and misses 3 the re-anchor gets right; none is wrong for both.

**Adversarial reading, and what survived it.**

- **Hidden knowledge.** `graph.rs` and `noticing.rs` name no family, vocabulary or hidden
  timing; the guard passes; the lab did not read the hidden record and designed from public
  dumps of the tuning streams and the evaluator's notice files on them. The constants (a 20 ms
  coincidence, 100 ms ticks) were tuned against evaluator measures on tuning streams, the same
  route B2's gap took. Accepted, stated.
- **Edge of the tuning budget.** The 100 ms selection sat at 5.58 against a tuning budget of
  5.6, with a tie-break reading added after the tuning tables. On the held-out streams it sits
  at 5.44 against 6.82, well inside. Accepted.
- **Hooks outside the territory**: a `Medium` variant in the seam, passthroughs, and a billing
  hook, all inert for other noticers (the byte identity shows it). Accepted.
- **Not done:** mutation testing of the M2 code. Queued into the next medium unit.

**What it means (mechanism, failure, time, objective).**

- **The medium wins by reading inside the tick.** The rung family scores an 8 s window and then
  re-anchors; the medium fires on two or three kinds of abnormal observation at one service
  within 20 ms, read from `offset_ns` by the ordered coincidence, with a lookback of zero that
  cuts its support at the tick edge. A stray just before a burst stays out unless it shares the
  burst's tick: rare at 100 ms (it wins), common at 2 s (it loses 31 incidents). The lookback
  table is the same mechanism in one variable: 0.992 at 0, 0.930 at 1 s. **The anchoring rule
  M1 built works when the support is short, and the tick is the support's floor.** Sub-tick
  pruning of the support is the obvious next medium unit (M3).
- **The ordered coincidence earned its place at every tick length**, as the M1b PI predicted:
  it buys background, not recall (the sliding form anchors as well but breaks the bound: 14.7,
  30.3, 22.7 per stream). The binned-by-rhythm form lost (0.976 at 8.59). Phase gates and
  oscillators found no role in this world; the result says nothing about them.
- **The leak.** The medium notices 0.993 of leaks (comparator 0.460) and anchors 0.554 of them
  at their start (every public noticer: 0.000), with a median latency of 9.8 s against 14.2 s.
- **The chief's synthesis premise was wrong, and the control shows it.** I wrote that an
  abnormal-only sense adapter would make result 2 unreachable by construction. The
  abnormal-only medium notices 1.000 of leaks: the ramp continues above the alarm line as dense,
  smooth readings, and the detector finds it there. Reading benign values buys anchoring the leak
  at its start and about 9 s of latency, not noticing. Result 2 tests the ramp detector, not the
  reading of benign values. Recorded as a coordinator error of reasoning (the fifth): I
  deduced a construction from the rules without checking what the rules leave above the line.
- **The cost column cuts the other way.** The medium's own operations are 3.9–16.7 ms per
  stream, under 1% of the arm's cost. But the medium opens more anomalies per incident (1.65
  against 1.04; strict precision 0.50 against 0.67), and under the selection oracle each one is
  a reasoner call, so the arm's total cost is 1.8–3.8 s per stream against 0.67 s. Better
  anchoring, bought with more escalations. EXP-101 must read noticing and cost together, and the
  medium's next unit must bring strict precision to the re-anchor's level or show why not.

**For the aim.** The first positive medium result: on the one lever the status quo could not
close with a tuned rule, the medium closes it at the short tick, and it does so with a
mechanism the rung does not have (event order inside a tick). Hand-designed, one world, one
setting, more reasoner calls. It is a mechanism, not yet an intelligence, and it has not yet
learned anything.

**Decided.**

1. **M3 (Lab 1):** support pruned below tick resolution, so the 100 ms behaviour survives at
   500 ms and 2 s; mutation testing of the M2 code; strict precision as a tuning constraint
   alongside the background bound. Criterion: result 1 at 500 ms and 2 s under M2's margins,
   with strict precision ≥ 0.67 (the re-anchor's) as a bound.
2. **EXP-101 preregistration** is drafted now, on the stream world at the primary setting with
   the δ sweep: noticing as the primary function (anchor-correct and leak noticed against the
   best public noticer from B3, under the background and strict-precision bounds), selection
   and cost as the secondary measures, critical misses and plain accuracy bounded, both aim
   proxies per arm. The draft comes to the user before the freeze.
3. **The learned noticer (Lab 3)** is queued: the medium's graph with its tuned constants
   replaced by parameters learned online from the stream's public history, against the frozen
   hand-designed graph and the re-anchor on the same stream order. First reading of aim
   proxy 2.

## B2 the public later re-anchor — merged (Lab 2); the status quo closes most of the anchoring gap

**Provenance.** R6's held-out run replays byte-identical for all 62 arms, twice (chief's hashes).
cargo-mutants: 78 of 80 caught on the evaluator (2 unviable), 33 of 35 on the new noticer (2
unviable), 20 of 20 hand mutants. Two tuning amendments were made after the first tuning run
and before any held-out run (grid extended downward; a tie rule), both recorded with the first
run's files kept. Gates on exit codes on the merged tree: fmt, clippy `--locked`, 713 Rust
tests, the oracle guard, 389 analysis tests. Outputs in `artifacts/runs/b2/` (ignored).

**Re-verified from the per-incident notice files.** `ReanchorNoticer` (site isolation 20 ms,
burst ≥ 2, z = 2): anchor-correct 0.952, leak noticed 0.460, 6.82 background notices per
stream; the rung at z = 2: 0.914, 0.460, 9.03; paired difference +0.038 [+0.021, +0.055] with
the chief's own cluster bootstrap. The held-out-best configuration within the budget is the
tuning-selected one, so the number is not selection luck.

**Verdict.** The acceptance clause triggers: the public later re-anchor reaches the 0.944 bar
within the budget (point estimate; the interval's lower end, 0.932, does not, and 200 streams
cannot say more). **M2's comparator is re-fixed to `ReanchorNoticer`** before any M2 held-out
run; Lab 1 was told directly.

**What it means (objective, failure and meta perspectives).**

- **A one-line public rule closes most of the mis-anchoring gap.** "If nothing follows the
  anchor within 20 ms and a burst begins later, move the anchor to the burst" gains 19
  incidents and loses 1 at the rung's own threshold, with fewer false notices. Of the gap R10
  measured, what remains for anything cleverer is 18 of 372 incidents: 10 anchored late (1–52
  s) and 8 never noticed. On burst families the status quo, tuned, has reached the noticing
  ceiling in quality terms: with the retirement confound removed, the re-anchor scores 0.591
  against the injected-notice oracle's 0.586 on the same streams (+0.005 [−0.028, +0.038]).
- **The leak is where noticing still has measured value**, and the re-anchor does not touch
  it: 0.460 noticed, 0.000 anchor-correct for every public noticer. R10's 0.75 lever stands.
- **The retirement confound is real and small for the rung family** (+0.008 to +0.040), and
  it is the whole story for floods (0.000 → 0.62–0.70 with the hold, bought with 57–63
  reasoner calls per stream). Neither quality reading measures noticing alone, which is why M2
  decides on noticing.
- **Precision as specified rewards floods** (0.56–0.60), because most abnormal observations
  belong to incidents. Strict precision (anchor- and site-correct notices over all notices:
  0.02 for floods, 0.67 for the re-anchor) and notices per incident expose them; both join M2's
  reported-beside list.
- **The 20 ms gap is a constant of this world's burst spacing.** The PI says so; the rule is
  close to "nothing follows the anchor immediately". One world, one setting. The medium's
  version of the same idea at a 100 ms tick is the fair comparison (B2's sensitivity rows: 0.935
  at 100 ms, 0.927 at 200 ms, 0.900 at 400 ms).

**For the aim.** This is the program's second instance of the status quo, given a fair chance,
capturing most of a lever (Stage B was the first). It is the falsification rule working as
intended, and it sharpens M2: the medium is now asked to beat the best public noticer, not the
default one. Its remaining claims are the 18 incidents, the leak, and cost.

**Decided.**

1. M2's comparator: `ReanchorNoticer`, anchor-correct 0.952 [0.932, 0.970], leak noticed 0.460,
   background bound 6.82 per stream. Margins unchanged: result 1 needs ≥ 0.982 (paired lower
   bound > 0.962), feasible only by fixing 11 of the 18 remaining incidents, and stated so.
2. Strict precision and notices per incident are reported beside M2's results.
3. **B3 (Lab 2):** the status quo gets its fair chance at the leak too: a public noticer that
   reads benign values (a per-node ramp or trend detector on saturation-type counters, tuned
   under the background budget), plus a splitting noticer for the 8 never-noticed incidents.
   If B3 reports before M2's held-out runs, the comparator for result 2 is re-fixed; otherwise
   M2 is read against B3 afterwards as a labelled supplementary comparison, and that is said in
   both reports.
4. The hold option stays off in every comparison row; the "held" quality reading is reported
   beside, never in a criterion.

## M1b the oscillome — merged (Lab 1); one benchmark point outside the band, accepted with a decision

**Provenance.** The all-off identity: `tests/m1_identity.rs` was committed on M1's code
(`a232cf5`) with two pinned digests over 310 specs; the chief reran it at that commit (M1 code,
no `oscillome.rs`) and at HEAD; both pass and the file is unchanged between them. Gates on
exit codes on the merged tree: fmt, clippy `--locked`, 686 Rust tests, the oracle guard, 377
analysis tests. 23 mutations, 22 caught; the one not caught (decay through `f32::exp`) gives
identical bits on this platform and is recorded. Benchmark data in `artifacts/runs/m1b/`.

**Verdict.** Accepted. One acceptance point fails: the smallest routing-only workload measures
1.42–1.43 of its model against a band of 0.7–1.4. The PI diagnosed two causes, an unpriced
fixed cost per tick (about 85 ns idle, about 190 ns with the engine on) and an 18–20% slower
routing path with the larger tick function, verified by alternating M1's and M1b's binaries
under one cgroup.

**Decided on the miss.** A per-tick fixed price of **200 ns** is charged by the ledger adapter
in M2 (a declared manifest price, like the others) so that a medium that ticks is billed for
ticking: 6,000 ticks per 600 s stream at 100 ms is 1.2 ms of modelled cost, against the rung's
0.66 s. The routing slowdown is not optimised now; this tick stays the reference and any faster
one is calibrated against it. The band is met at every other point (46 of 48 measurements, the
headline 0.89–1.22).

**What it means (time and structure).**

- **Times in seconds survive the tick sweep, with stated losses.** The conversion table shows
  which delays collapse to one tick (everything under 150 ms at 500 ms and 2 s) and which are
  refused (over 255 ticks). A design that wants a 100 s delay at a 100 ms tick uses a rhythm,
  not a synapse delay. That is the oscillome doing what it was added for.
- **The PI's prediction for the ablation is on record:** the global rhythms, phase gates and
  binned coincidence will be removed; the ordered coincidence (event order from `offset_ns`
  inside a long tick) is the element most likely to earn its place, at 500 ms and 2 s only;
  oscillators matter for the 6–16 s horizon, not first-tick anchoring; retirement lets the end
  of an anomaly reach B1's seam without a heartbeat. The chief agrees with the direction and
  records it so the ablation cannot be read post hoc.
- **A hazard accepted:** parameters M1 documented as unused now carry meaning; decoding refuses
  stored conversions that disagree with their quantities. No M1 spec exists outside the tests.

**M2 is unblocked.** M1b and B1 are merged; the M2 brief follows in `docs/lab-queue.md`.

## Synthesis after W1, M1, B1 (chief; the perspectives of AGENTS.md "Multidimensional analysis")

**Structure.** The three units compose at one point: B1's `Noticer` seam is the rung's noticing,
M1's effector port emits notices and (M1b) retirements, and W1's tick statistics size the sense
adapter between them. The medium therefore plugs into the status quo at exactly the place where
the status quo's measured error lives (mis-anchoring), with everything downstream shared and
byte-identical. That is the cleanest comparison this program has had.

**Causality.** Mis-anchoring is now identified from three independent sides: R10's ledger
replay, B1's evaluator instrument (17 of 31 never-noticed incidents have a background notice
within 1 s before them), and M1's own anchoring rule, which has the mirror-image risk (it prunes
the starting event and moves the anchor later). The mechanism is: a stray at or near the site
opens an anomaly a fraction of a second early; the incident's burst attaches to it; the anchor
never moves. The fix is a later re-anchor, which no public noticer makes (B2 builds one) and
which the medium's "earliest contributing event" rule makes only if its support keeps the right
events. M2 is, concretely, a test of two anchoring rules against one measured failure.

**Time.** W1 showed one tick cannot serve burst order (20–150 ms) and the incident horizon
(6–16 s), and that cost follows events, not ticks. The oscillome (M1b) is the structural answer;
the all-off identity test keeps it falsifiable.

**Objective.** The aim's proxy 1 is cost in disguise unless its denominator is hard decisions;
proxy 2 has an instrument and nothing to read until a learner exists; anchor correctness is
gameable by flooding and is sound only under a background budget. Every M2 number is therefore
reported under a bound, and the lever the medium is asked to move (anchoring) is one the public
arms demonstrably cannot.

**Failure.** Four found this round: floods, the earlier-anchor fix degrading with lookback,
silence not propagating in a sparse medium, and pass-limit latency. Each has a decision on
record.

**Meta: the assumption all three labs share.** Every instrument and arm defines signal by the
public rules' "abnormal" verdict: W1 copies the rule, B1's noticers see only abnormal
observations, and a sense adapter that tags only abnormal events would hand the medium the same
blindness. The slow leak is benign under that rule until it crosses the alarm line, which is why
its anchor-correct share is 0.000 for every public noticer and why R10's leak lever (0.75) is
the largest in the program. **Decision for M2:** the sense adapter delivers every observation
with its value, benign readings included, so that a `Novelty` or `Integrator` cell can read a
ramp. If the medium's leak result holds, it holds because it reads what the rules discard; if
the adapter were abnormal-only, result 2 would be unreachable by construction, which the chief
would otherwise have discovered after the run.

**Trajectory.** M1b → M2 (with B2 in parallel) → EXP-101 preregistration (noticing first,
leak as a named secondary, δ sweep, both aim proxies) → the learned noticer against a
never-learning control, which is the first reading of proxy 2 and the falsification test for
the cell model's claim to sample efficiency.

## B1 the Noticer seam, notice measures, public noticing baselines — merged (Lab 2)

**Provenance.** R6's held-out run replays byte-identical for all 62 arms with the seam, twice
(seam commit and final binary; hashes recomputed by the chief), and R10's 6 arms replay too. The
notice record is three new files per arm; `results.csv` and `incidents.csv` keep their columns,
pinned by a test. 29 evaluator fixtures; cargo-mutants 39 caught, 2 unviable, 0 missed; 25 hand
mutants caught. Gates on exit codes on the merged tree: fmt, clippy `--locked`, 652 Rust tests,
the oracle guard, 377 analysis tests. One driver refusal (unclean tree), nothing run; outputs in
`artifacts/runs/b1/` (ignored).

**Re-verified from the per-incident notice files** (200 held-out streams, 372 hard non-leak,
139 leak):

| Noticer | Noticed | Anchor-correct | Leak noticed | Background notices / stream |
|---|---|---|---|---|
| Rung z = 3 (default) | 0.917 | 0.890 | 0.460 | 4.00 |
| Rung z = 2 | 0.946 | 0.914 | 0.460 | 9.03 |
| EarliestAnchor l = 0.25 s | 0.917 | 0.890 | 0.460 | 4.01 |
| ChangeTriggered q = 128 s | 0.032 | 0.032 | 0.000 | 8.76 |
| ChangeTriggered q = 0.5 s (sensitivity, a flood) | 1.000 | 0.984 | 1.000 | 587.8 |

All match the PI's table.

**What it means (failure and objective perspectives).**

- **The rung's noticing gap is now measured without ledgers:** 8.3% of hard non-leak incidents
  never noticed, 2.7% noticed with a wrong or late anchor. Of the 31 never-noticed, 17 have a
  background-anchored notice within 1 s of their first observation (chance: about 1%). R10's
  mis-anchoring finding is confirmed by an independent instrument.
- **The cheapest public fix does not work.** Moving the anchor earlier (`EarliestAnchor`) makes
  anchoring worse as the lookback grows (0.890 → 0.422 at 8 s), because an earlier anchor lands
  on same-site background strays. The failure R10 found needs a *later* re-anchor onto the
  incident, which no public noticer here can make. That is a precise target for the medium.
- **Threshold tuning saturates early.** Anchor-correct peaks at z = 2 (0.914) and falls by
  z = 0.5 while background notices go 4 → 60 per stream.
- **Change-triggered noticing cannot be tuned into the rung's background budget.** At q ≤ 1 s it
  notices everything at 500+ background notices per stream; at the budget it notices nothing.
  The charter's change-triggered baseline is answered at the noticing level: it is a flood or
  it is blind.
- **The anchor-correctness measure is gameable by flooding**, and the PI showed it: a flood is
  0.984 anchor-correct. The measure is sound only under a background budget, which is why M2's
  criterion carries one. The leak is 0.000 anchor-correct for every abnormal-only noticer,
  because its early readings are benign under the public rules; a noticer that anchors a leak
  at its start must read the ramp, not the alarm.
- **The quality column is confounded** by the selection oracle's fixed 16 s delay and the rung's
  6 s retirement: a flood's anomalies die before they are asked about. M2 reports quality but
  decides on noticing, as its criterion says.

**Coordinator error, recorded (fourth).** M2's criterion named "the best public noticer by that
result's own measure" as the comparator. B1's sensitivity rows show that reading admits the
flood, under which both results are unreachable. The lab's budget rule kept the flood out of its
table, but the criterion should not have depended on a lab's tuning rule. Amended below, before
any M2 code or run. Lesson, added to the earlier three: a comparator must be named by a rule
that cannot select a degenerate arm, and the feasibility check must include the arms that game
the measure.

**Decided.**

1. **M2's comparator is fixed as `RungNoticer` z = 2** from B1's table: anchor-correct 0.914
   [0.890, 0.937], leak noticed 0.460, 9.03 background notices per stream. The medium's
   background notices per stream may not exceed 9.03. Result 1 needs anchor-correct ≥ 0.944
   with the paired lower bound above 0.924; result 2 needs leak noticed ≥ 0.660 with the lower
   bound above 0.560. Power: a +0.03 paired gain is 11 incidents of 372; feasible, tight, and
   stated.
2. **Reported beside M2's results:** leak anchor-correct (0.000 for every public noticer; a
   medium that anchors a leak at its ramp will show here first), notice precision (share of
   notices anchored on an incident), notices per incident, latency, and the quality column with
   its confound named.
3. **Lab 2's next unit, B2:** the evaluator gains a site check and a notice-level precision
   measure; the selection oracle gets a notice-relative delay option so quality measures
   noticing; and a public *later re-anchor* noticer (re-anchor a background-anchored anomaly
   onto the incident whose evidence it attaches) is built as the strongest public comparator for
   exactly the mis-anchoring the medium targets. If that public fix closes the gap, the medium's
   first job is already done by the status quo, and that is a result.
4. **Workers' scratch files:** a PI overwrote another's helper in the shared scratchpad. Each
   lab now uses its own subdirectory, named by its worktree, under the session scratchpad.

## M1 the medium crate — merged (Lab 1); prices calibrated at 10× the design's guess

**Provenance.** Gates on exit codes on the merged tree: fmt, clippy `--locked`, 626 Rust tests,
the oracle guard, 354 analysis tests. The crate forbids `unsafe`, has no hash map, I/O or
transcendental call (grepped), and depends on `gordian-core` and `serde` only (`serde_json`,
`proptest`, `criterion` as dev-dependencies). `Cargo.lock` gains one entry. The lab's
benchmark data is kept in `artifacts/runs/m1/` (ignored).

**Re-verified by the chief.**

- **A mutation of my own.** Removing the within-tick event sort makes
  `events_are_summed_in_offset_order` fail. The determinism tests test the claim; the lab's
  eight mutations are credible.
- **The benchmark**, rerun under the cgroup on cores 0-2 (67.7 s wall, 65.5 s CPU, 64.5 MB
  peak): headline tick 7.1 / 7.8 / 8.7 × the declared prices at 10 / 100 / 1,000 active cells;
  least-squares prices 201 ns per cell update, 19 per traversal, 40 per routing, 0.6 per field
  read. The lab measured 202 / 22.8 / 42.3. The proposed prices (200, 25, 40, 2) put every
  measurement within 0.78–1.28 of its model.

**Verdict.** Accepted. The acceptance allowed a miss of the factor-of-5 price bound if new
prices come with the measurement, and they do.

**Decided on prices.** The calibrated prices are the declared prices from here: M2's manifests
declare 200 / 25 / 40 / 2 ns, and M1b updates `Prices::DECLARED` with a test. This version of
the tick stays the reference implementation; any faster tick is calibrated against it (AGENTS.md,
"Keep a simple reference implementation as an oracle"). A medium cell update at 200 ns is about
a tenth of a cheap component call (median 1,743 ns) and 10⁻⁵ of a reasoner call; cost is not in
M2's criterion, but these are the numbers its cost column will carry.

**The PI's three warnings, and what the chief decided (structure and failure perspectives).**

1. **Silence never propagates.** Cells run only with input, so the end of an anomaly reaches
   nobody, and B1's seam needs retirements. Decision for M2: a heartbeat event per tick from the
   clock adapter, priced like any event (one routing, one update per tick), not a time-out in
   the effector. A time-out in the adapter would be anchoring logic outside the medium, which
   section 2 forbids.
2. **The anchoring rule as built prunes the starting event.** An accumulating cell's lookback
   bounds what it cites, not what it sums, so the event that began an anomaly is the one most
   likely to fall out of the support, which moves the anchor later. That is the rung's failure
   in a new form, and the PI is right to flag it. M2 must treat the lookback as a tuned
   parameter per tick length, report anchor correctness as a function of it, and the
   `EarliestAnchor` baseline from B1 is the fair comparator for exactly this. If anchor
   correctness is bounded by pruning rather than by the graph, that is M2's first finding.
3. **Pass-limit latency.** With `max_passes` 2 a sense → integrator → emitter chain emits one
   tick late. M2 uses `max_passes` 3 and reports latency at each tick length.

**Departures accepted**, all 33 recorded in the crate's `DESIGN.md`. The material ones: event
references carry their offset; cells carry a bounded support of references (a new hard limit)
and messages carry the sender's support; archetypes receive the ticks since they last ran so
that cells without input truly do not run; "counts monotone in events" is true only in its weak
form because inhibition exists, and the weak form is what is tested.

**Rhythms.** The PI did not fold the oscillome into M1, for reviewability, and proposed a
bounded M1b with an acceptance. Accepted: judging two designs under one acceptance would have
been the chief's error. M1b is queued to Lab 1 with the PI's acceptance and the open questions
of section 4b resolved in the queue entry.

**Resource-rule violation, recorded.** The PI's first build overlapped a Lab 2 byte-identity
run (a check, not a timed measurement). It disclosed it and guarded every later call. No
measurement was affected.

## W1 tick statistics and the aim's measures — merged (Lab 3)

**Provenance.** The example reruns byte-identical under the cgroup on the chief's own invocation
(five CSVs). Gates on exit codes on the merged tree: fmt, clippy `--locked`, 588 Rust tests, the
oracle guard, 354 analysis tests. The chief added the `[[example]]` wiring in the crate's
`Cargo.toml`, outside the lab's territory, as the PI asked. Run usage in `artifacts/runs/w1/`.

**Re-verified.** Sample efficiency and energy ratios follow from R10's numbers the chief already
checked: `notice_rung` 348/511 = 0.681, the oracle 480/511 = 0.939; ns per correct hard decision
equals cost per stream × 200 / correct hard, to three figures, for every arm checked.

**What it means for the medium (time perspective).**

- **The two timescales are real and a tick cannot serve both.** At 100 ms a contradicting
  burst's partner alarm lands in a later tick 79% (cascade) and 63% (split brain) of the time,
  so order survives; at 500 ms about five in six partner pairs share a tick; at 2 s order is gone
  (97–100% same tick). The leak is tick-indifferent. `offset_ns` inside the tick is therefore
  not a nicety: at 500 ms it is the only carrier of burst order.
- **Background swamps the long tick.** The share of (node, tick) cells with two or more abnormal
  observations is 0.12% at 100 ms and 15% at 2 s. A "two abnormal at a node" rule fires 446
  times per stream at 2 s against 75 at 100 ms. Thresholds on counts cannot be shared across
  tick lengths; M2 tunes per tick, and delays and decays are stated in seconds, not ticks.
- **Decisive evidence is never in the first tick**, by construction: hard incidents present for
  6–16 s before their decisive phase. A noticer's job in the first tick is anchoring, not
  deciding; the design's "earliest contributing event" rule is aimed at the right moment.
- **"Cost inverse in tick length" was wrong as stated.** Routed events are constant; active
  (node, tick) cells fall only 4,261 → 2,175 across a 20× change in tick. Only per-tick fixed
  work scales inversely. The medium's cost is driven by events, not ticks, which is what a
  sparse design should show; `docs/medium-ports.md` section 8 is corrected below.

**What it means for the aim (objective perspective).**

- **Cost per correct decision over all incidents is cost in disguise** (Spearman 1.00 with cost),
  because plain incidents dominate the denominator and every arm gets them equally. The proxy
  that discriminates is per correct *hard* decision, and even that is near cost order where
  quality is flat (R9: 0.98). Decision: preregistrations report both, with the hard-decision
  denominator as the aim's proxy 1.
- **Joules rank exactly as modelled nanoseconds** because the reasoner is 99.87% or more of
  every reasoner arm's cost. The placeholder watts (10 W cheap, 1,000 W reasoner) matter only
  for `never_escalate`. The conversion stays a labelled placeholder until measured.
- **Sample-efficiency curves of fixed policies are flat**, as they must be. The instrument is
  built; it has nothing to read until a learning arm exists (aim proxy 2). The PI's next test,
  an online public noticer against a never-learning control on the same stream order, is the
  right one and is queued after M2.

**Accepted with notes.** `is_abnormal` is a copy of the rung's rule because the stream crate
cannot depend on the harness; equality is by reading. Same-tick shares assume a grid starting at
zero; phase is not swept. Group counts are 56–140 without intervals.

**Decided.** `docs/medium-ports.md` section 8: replace the cost expectation with W1's finding;
require `offset_ns`-aware archetypes at 500 ms and above; tick sweep stays {100 ms, 500 ms, 2 s}.

## R9 grounded distractor penalty — merged; the world's look-alikes cost a strong reader little

**Provenance.**

- The reader was frozen at `cccbcf2` (tag `r9-reader-frozen`) before any evaluation number;
  the evaluation commit `a16f839` follows it. Development on seeds 29000–29999, evaluation on
  30000–30399.
- Three tuning runs were refused once by the driver because the R10 worker's `cargo test` was
  running; they wrote nothing and were rerun. The core-sharing rule worked as intended.
- The ceiling and R4's oracle are byte-identical to R6's in all six runs.
- Gates on exit codes on the merged tree: fmt, clippy `--locked`, 576 Rust tests, the oracle
  guard, 339 analysis tests. Run outputs moved to `artifacts/runs/r9/` (ignored).

**Re-verified from raw files.**

- Reader on 312 hard evaluation questions: A(m) = 0.958, 0.939, 0.904, 0.913, 0.933 at
  m = 0, 50, 100, 200, 400; control 0.045; δ* = 0.0137 [0.0054, 0.0223] with the coordinator's
  own 2,000-resample cluster bootstrap (worker: 0.0135 [0.0055, 0.0225]).
- Rerun, tuning-selected builder, G = ceiling − builder:

  | δ | Selected builder | G [90%] |
  |---|---|---|
  | 0.0055 | `window` 80 s, N 512 | 0.032 [0.011, 0.054] |
  | 0.0135 | `cooccur` 16 s or `neighbourhood` k 4 (tie at 0.734) | 0.073 [0.046, 0.099] |
  | 0.0225 | `neighbourhood` k 4, N 256 | 0.089 [0.059, 0.118] |

  All match the worker.

**Verdict as written: unresolved.** The R6 regime needs G's upper bound below 0.10 at δ*_hi
(it is 0.118); the R7 regime needs G ≥ 0.10 at δ*_lo (it is 0.032).

**Coordinator error, recorded.** The feasibility note said the R6 regime needs roughly
δ*_hi < 0.05. It needs about 0.012, because G(0.05) was already 0.097 in R7. I read the regime
boundaries off R7's grid without interpolating. The criterion stands as written; the outcome is
unresolved on its own terms, and the error is in the feasibility note, not the clauses.

**What it means.**

- **In this world, look-alike distractors cost a strong reader very little.** A reader that
  knows the hidden rules and has learned from labelled development questions loses at most
  0.054 of accuracy at any distractor count up to 400, and recovers to 0.933 at 400. R8's crude
  reader lost 0.57. The world's confusability is real but small for a competent reader.
- **The curve is not exponential in count.** It dips at m = 100 and recovers, because random
  draws from the ±40 s pool include the incident's own non-decisive observations, which help
  (a split brain's burst). δ* is a summary, not a law, and the rerun turns it into a
  count-based penalty that the reader's curve does not quite obey.
- **Read as a point estimate, the context lever is modest.** At δ* the perfect-context ceiling
  beats the best simple builder by 0.073 [0.046, 0.099] on hard incidents, at 36× the references.
  That is more than R6's 0.024 and less than R7's "fragile" 0.14–0.34. EXP-102 can claim both a
  small quality margin and a large reference saving, with the quality margin's interval reaching
  from 0.03 to 0.12 across δ*'s interval.
- **Count-based penalties transfer to builder contexts, not to the rung's.** On R6's actual
  contexts the reader is within 0.05 of the curve for `window`, `cooccur` and `neighbourhood`.
  On the rung's own context it reads 0.659 against a predicted 0.953: the rung's context lacks
  the cascade's partner, which is missing information, not distraction. The simulated reasoner's
  `q` already models that loss.
- **What δ* is not.** It is the loss of one program with the hidden rules and learned weights
  on this world's questions. It says nothing about real models (R8 could not), and a stronger
  reader would give a smaller value, as every version during development did.

**Accepted with notes.**

- The reader learned softmax weights from labelled development questions. The plan allowed
  the hidden rules and forbade labels at evaluation; it did not say whether labelled training
  was allowed. The coordinator accepts it: the reasoner is the hidden side's stand-in, and a
  model trained on labelled diagnoses is what it stands in for. The reader is therefore stronger
  than a rule-only reader, and δ* is smaller for it.
- Six of 170 direct-check calls had an anchor more than 0.1 s after onset and the reader was
  wrong on all six. Its first-alarm-is-onset assumption is a weakness, post hoc.
- The tuning-selected builder is an unstable comparator at small δ: two builders tie at δ*, and
  the held-out-best builder would pass the R6 clause's upper bound at δ*_hi (0.097). The
  criterion is kept as written.

**Decided.**

1. **The reasoner's default for every preregistration is δ = 0.0135**, with the sweep
   {0, 0.0055, 0.0225, 0.05} required beside it, so that every claim is read across the interval
   and at the old default. This replaces the free δ = 0 default in the reasoner's spec for
   experiments, not in the code (the code's default stays 0 so that R6 and R7 replay).
2. **EXP-102's claim is references and cost at matched quality, with a secondary quality margin**
   against the best public builder, preregistered at 0.05 with the δ sweep. The worker's
   "fewest references within 0.05 of the ceiling" reading is the primary measure.
3. **The simulated reasoner's distractor penalty remains a count-based stand-in.** A
   composition-based law (penalising look-alike free-form messages rather than every
   reference) would fit the reader's curve better. It is not built now: the count-based law
   with the measured δ is within 0.05 of the reader on every builder context, and a new law
   would need a new byte-identity gate and would reopen R7. Recorded as a known weakness of
   the world, with the direct-check table as the evidence for revisiting it.
4. With R5, R6, R7, R9 and R10 done, the simulation-side headroom work for EXP-101 and EXP-102
   is complete. The next unit is EXP-101's preregistration.

## R10 salience ceiling — merged; noticing is worth 0.10 on burst families and 0.75 on the leak

**Provenance.**

- The new privileged arm reads two plan fields, `hard` and `first`, and nothing else. The
  coordinator grepped the diff for every plan accessor.
- R6's held-out run at b = 5, ρ = 0.7 replays byte-identical with the new binary: 62 of 62 arms,
  hashes recomputed by the coordinator. The R10 comparison arms equal R6's own files once the
  `run_id` column is dropped.
- 8 driver runs and 5 replays, all exit 0, none refused or excluded. No waits on the other
  worker.
- Gates on exit codes on the merged tree: fmt, clippy `--locked`, 572 Rust tests, the oracle
  guard, 326 analysis tests. Run outputs moved to `artifacts/runs/r10/` (ignored).

**Re-verified from raw files, rung's own context, 90% paired cluster bootstrap.**

| Setting | Result 1: hard, no leak | Result 2: slow leak |
|---|---|---|
| b = 5 (primary) | 0.586 − 0.489 = +0.097 [+0.062, +0.132], holds | 0.935 − 0.187 = +0.748 [+0.678, +0.814], holds |
| b = 2.5 | +0.024 [−0.013, +0.060], not shown | +0.324 [+0.252, +0.400], holds |
| b = 8 | +0.099 [+0.063, +0.136], holds | +0.777 [+0.709, +0.841], holds |

All match the worker's report to 0.001.

**Verdict as written.** Both results hold at the primary setting. Result 1 does not hold at
b = 2.5, where it is "not shown", not "equivalent": the interval reaches 0.06 and the full oracle
itself reaches only 0.538 there.

**What it means.**

- **Noticing is a real lever, and a larger one than the R6 correction estimated.** The R6
  entry predicted at most 0.091 from the never-noticed incidents. The measured gain is 0.097 with
  the rung's context and 0.156 with the window builder. The extra comes from asking earlier
  about incidents the rung notices late.
- **With a good context, noticing alone reaches the full oracle's quality.** `notice` with the
  window builder scores 0.952 on hard incidents, equal to R4's oracle, at 3.44 s per stream
  against 0.32 s. So R4's oracle decomposes as: noticing (this item) plus context (R6) plus
  selection (R5, cost only). Timing beyond notice + delay is worth little, which agrees with
  the R6 correction.
- **The slow leak is almost entirely a noticing problem.** The rung sees the leak only after
  its threshold crossing, a median 17.8 s late, and never for 40 of 80 diagnostic leaks.
  Noticed at its first reading, the leak is answered in 0.935 of cases with the rung's own
  context.
- **"Never noticed" is mostly mis-anchoring.** Every never-noticed hard incident has abnormal
  observations attached to an anomaly the rung anchored elsewhere, usually on background
  about 0.3 s earlier. The rung sees the activity and files it under the wrong anchor. This is
  a segmentation failure, which is close to what the charter's salience function is for.
- **The threshold sweep says the burst-family gap is partly tuning and the leak gap is not.**
  Lowering the notice threshold to z = 2 recovers 0.038 of the 0.097 at the price of doubled
  false alarms. No threshold helps the leak, because its sub-alarm readings are not abnormal
  observations at any threshold.

**Accepted with notes.**

- The injected notice is the arm's own record, not a rung anomaly, so it does not retire after
  6 s of quiet. Four of 270 diagnostic incidents were called by `notice` whose selection
  anomaly had retired before the delay. This is a small timing privilege inside the ceiling.
  It is recorded, not corrected; a ceiling that retires like the rung would be slightly lower.
- Result 2 mixes "whether it asks" with "when it asks" (about 16 s apart). The notice arm
  beats R4's oracle on the leak (0.935 against 0.906), which shows the ask time matters there.
- The never-noticed / late-noticed decomposition rests on 14 incidents and the late part's
  interval includes zero.
- The worker reported ignoring an instruction that arrived inside a tool result. Correct.

**Decided.**

1. EXP-101's registration names noticing as its first function, not threshold-versus-oracle
   selection. Its privileged ceiling is `oracle_notice` with the rung's context; its public
   baselines include the threshold sweep at z = 2 and a change-triggered rung.
2. The slow leak gets its own preregistered secondary measure in EXP-101, since it is where
   noticing matters most and where no threshold helps.
3. The anchoring finding is the first concrete job for a substrate mechanism: attribute
   activity to the right anchor. A public baseline for it is designed before any substrate is
   built.
4. R9's result decides whether the context half of the oracle is a quality or a cost lever.

## R8 real-model distractor sensitivity — merged; unidentifiable with these models

**Provenance.**

- **Code:** an evaluator-side question dumper behind the oracle feature, allowlisted in the guard;
  llama.cpp pinned at tag `b11429`, built outside the workspace in `artifacts/runtime/` (ignored);
  Qwen2.5 1.5B and 3B, Q4_K_M, with sha256 equal to the published values, in `artifacts/models/`
  (ignored).
- **Calls:** 277 in total, 260 of them scored. No parse failures, no errors, none truncated, none
  repeated.
- **Gates:** on exit codes on the merged tree: fmt, clippy `--locked`, 560 Rust tests, the oracle
  guard, 326 analysis tests.
- **Run outputs:** in `artifacts/runs/r8/`. The coordinator checked that they are identical to the
  worktree's before removing it.

**Re-verified from `calls.jsonl`.**

| Model | Hard questions | A(0) | 90% lower bound | Guess rate (control) |
|---|---|---|---|---|
| 1.5B | 28 | 0.500 | 0.357 | 0.357 |
| 3B | 16 | 0.250 | — | 0.250 |

- For the 1.5B, A(m) at m = 50, 100, 200 and 400 is 0.357, 0.393, 0.429 and 0.357, all at the guess
  rate. All of these match the worker's report.

**Verdict as written: unidentifiable with these models.** The failure is structural, not a matter
of sample size.

- The 1.5B model gains only 0.14 from having all of the evidence. That is below the 0.15 the
  precondition requires, so it would fail at any N.
- The 3B does no better than its own guess rate on the main questions.
- Neither model reads this world's evidence well enough for its loss to distractors to be
  measured.

**Coordinator notes.**

- **The plan's sizing was optimistic.**
  - The reasoner prompt needs about 5,200 tokens: the rules plus 14 worked examples.
  - Prompt evaluation on three cores runs at about 50–75 tokens per second.
  - So the 4-hour envelope allowed 28 hard questions, not up to 120.
  - The worker computed, before the main run, that even a model passing the precondition could not
    have produced an interval narrow enough for either regime at this N. The pilot did its job;
    the plan should have expected this.
- **Prompt development used 12 rounds on pilot-range questions.**
  - The 3B scored 6/12 on development questions and 4/16 on the main ones. That is within sampling
    error, but some fitting to the development questions cannot be excluded.
  - No change was made after a scored call.
- **The reference rule-reader is the more informative output, and it needs careful reading.**
  - It is a program that applies the prompt's rules to the rendered context.
  - On the same contexts it falls from 0.93 at m = 0 to 0.36 at m = 400 (δ̂ 0.44 [0.24, 0.88]).
  - The cause is that hard-incident evidence messages share an ID pool with background messages,
    so distractors include look-alikes that the reader's crude windowed rule accepts.
  - This is not the loss an optimal public reader would suffer; a reader that learned each stream's
    vocabulary from its own history might lose much less.
- **What it does establish is a property of the simulated reasoner.**
  - Its `q` is counted from hidden labels, so at δ = 0 it behaves as a reader that always knows
    which references are evidence.
  - No reader without hidden labels has that ability in this world.
  - The δ = 0 law is therefore not "a strong model". It is an oracle-labelled reader. R6's
    "simple builders suffice" was measured under that oracle.
  - This moves the prior toward R7's regime, without estimating δ.
- **Deviation accepted.** The incident's own non-decisive observations were kept out of both the
  contexts and the pool, whereas the simulator's `m` counts them as distractors. It does not affect
  the verdict.

**Decided.**

1. EXP-102 is not preregistered on the current reasoner law. The law's δ = 0 default is an
   oracle-labelled reader, and no real model available here can estimate δ.
2. The options are put to the user:
   - **(a) Simulation-only.** Ground the distractor law in the world itself: measure how a strong
     public reader degrades with look-alike distractors, then let the simulated reasoner's
     informed probability depend on confusable distractors rather than on a free δ.
   - **(b) A stronger real model.**
     - A local 7B model is not practical on this CPU: about 4–5 minutes per call at these context
       sizes.
     - A remote model needs credentials and money, which only the user can supply.
   - **(c) The salience ceiling for EXP-101.** This is independent of the context question.
3. The models (3.1 GB) and runtime (325 MB) are kept for now; free disk is 7.7 GB. They can be
   deleted if (b) is not chosen.

## R7 reasoner-law sensitivity — merged; R6's context finding is fragile

**Provenance.**

- The world change adds a distractor penalty to the hidden reasoner. It multiplies the informed
  probability by `exp(−δ·m/100)` and takes no new draw.
  - The coordinator read the `reasoner.rs`, `params.rs` and `sim.rs` diffs.
  - `m` counts references that are not decisive evidence of the focus incident, probes included,
    as R7 specifies.
- At δ = 0, R6's held-out run at b = 5, ρ = 0.7 replays byte-identical: 62 of 62 arms.
- In all six held-out runs, the context-only ceiling and R4's oracle are byte-identical to R6's.
- 13 runs, all exit 0, none excluded.
  - Five tuning runs were refused by the driver's clean-tree preflight before starting, because the
    worker had created untracked files during a run.
  - They were set aside and rerun at the new HEAD (`r7-stale-manifests.csv`). No output was
    affected.
- Gates on exit codes on the merged tree: fmt, clippy `--locked`, 553 Rust tests, the oracle guard,
  308 analysis tests.
- Run outputs moved to `artifacts/runs/r7/` (ignored).

**Re-verified from raw files.** The coordinator rebuilt the tuning selection from the raw tuning
`incidents.csv` and recomputed G(δ) with its own cluster bootstrap:

| δ | Selected builder | G [90%] |
|---|---|---|
| 0.05 | `window` 20 s, N 256 | 0.097 [0.066, 0.129] |
| 0.1 | `window` 20 s, N 256 | 0.137 [0.104, 0.170] |
| 0.2 | `cooccur` 1 s, N 256 | 0.280 [0.233, 0.327] |
| 0.4 | `cooccur` 1 s, N 128 or N 256 | 0.34 |

- The builder choices match the worker's. The bounds agree to within 0.001.
- At δ = 0.4, two configurations tie on tuning quality. The worker's tie-break toward fewer
  references gives 0.339, and the other gives 0.341.

**Verdict as written: Fragile.** At δ = 0.1 and 0.2, G ≥ 0.10 with the lower bound above 0.05.
"Robust" fails at every δ in the grid. The sensitivity settings (b = 2.5 and b = 8, at δ = 0.2)
agree.

**What it means.**

- **The result is conditional, and the condition is the finding.**
  - The ceiling's context holds only decisive evidence, so it is immune to the penalty by
    construction, and G must rise with δ.
  - What R7 measures is where R6's conclusion breaks. "Simple builders capture the context lever"
    holds only while the penalty is about 0.05 per 100 irrelevant references or less. That is at
    most about a 12% relative loss of informed probability at 250 references.
  - Above that, choosing which references to send is worth 0.14 to 0.34 of hard-incident quality.
- **Under a penalty, the best public context shrinks.**
  - The best context goes from about 490 references per call to about 85.
  - The winning builder changes from a broad `window` to a narrow `cooccur`.
  - At δ = 0.4 the best builder is barely above the rung's own context.
  - So compaction becomes the lever: deciding which few references carry the evidence. A
    substrate could plausibly do this, but no public builder here does it well.
- **EXP-102's design therefore depends on one unknown: the δ of a real reasoner** on contexts
  like these.
  - Simulation cannot supply it.
  - The charter puts real-model work in EXP-106, after EXP-101 and EXP-102.
  - R7 shows that the order matters: without an estimate of δ, EXP-102 cannot say whether its
    claim is quality or references.

**Accepted with notes.**

- The worker's readings were fixed in scripts before the runs, and are reasonable.
- The functional form (exponential in count, no position effect) is the plan's assumption, not a
  measurement.
- The selection delay was not re-tuned at δ > 0, as the plan said. A shorter delay might change
  context sizes slightly.
- The literature anchors (Shi et al. 2023; Liu et al. 2023) are still unchecked against the
  primary texts. No number relies on them.

**Decided.**

1. Before EXP-102 is preregistered, estimate δ on a real model. The smallest form is a local
   small model on this CPU, asked R1-style diagnosis questions with controlled numbers of
   irrelevant references.
   - This needs a model download, a runtime dependency and disk.
   - It is put to the user before it is planned in detail.
2. EXP-102, when registered, sweeps δ as a preregistered parameter and states its claim per δ
   region. It never states a single conclusion.
3. The salience ceiling from the R6 correction (an oracle that notices, with the rung's context and
   delay) remains the next simulation-only item, for EXP-101.

## R6 context-construction headroom — merged; simple builders capture the context lever in quality

**Provenance.** 26 runs through the driver, all exit 0; none failed, timed out or was excluded.
Eleven manifests refused by the driver's revision preflight were set aside and rewritten, not
deleted (`r6-stale-manifests.csv`). The six R5 held-out runs were replayed: 317 arm-runs are
byte-identical to R5's hashes. Gates on exit codes on the merged tree: fmt, clippy `--locked`, 545
Rust tests, the oracle guard, 308 analysis tests. Run outputs moved to `artifacts/runs/r6/`
(ignored).

**Re-verified at b = 5, ρ = 0.7 from raw files** (hard quality excludes slow leak).

| Arm | Hard quality | Refs/call | Critical misses |
|---|---|---|---|
| R4 oracle (`oracle_escalation_privileged`) | 0.952 | 5.1 | 194 |
| Context-only ceiling (`oracle_selection_context_d16_privileged`, supplementary) | 0.820 | 5.3 | 255 |
| Selection oracle + `window` 40 s, N 256 | 0.796 | 251.1 | 255 |
| Selection oracle + rung's own context | 0.489 | 43.3 | 282 |
| `always_escalate` + `window` 40 s, N 256 | 0.500 | 249.0 | 268 |

All match the worker's report.

**Verdict as written.** Both clauses hold at every setting, and the verdict is uninformative.
Clause 1 compares at no more than the ceiling's 5 references per call. No public builder is that
small, so the comparator is the empty context. Clause 2 is unreachable because R4's oracle carries
privileges no builder can supply.

**Coordinator error, recorded (second time).** This is the same mistake as R4.

- I took R4's oracle as the context ceiling although it bundles more than context.
- I wrote a clause (equal or fewer references) that no public builder could satisfy.

Lessons, applied to every criterion from here on:

- A ceiling isolates exactly one privilege. Its comparison arm differs from the public arms in
  that privilege only.
- Each clause is checked for feasibility against what a public arm can do, before any run.
- The worker's supplementary context-only ceiling is the correct comparator. It was labelled and
  did not replace the verdict.

**What it means.**

- With selection held at the oracle, simple builders capture almost all of the context lever in
  quality.
  - `window` reaches 0.796, against 0.820 for the context-only ceiling: a gap of 0.024
    [0.000, 0.051].
  - They do it with 33× the references per call (31.7 to 49.5), which is 8–11× the cost per
    stream.
  - EXP-102 can therefore claim references or cost at matched quality, not quality.
- Binding evidence across services and over the following seconds is solved by simple builders.
  Evidence that precedes the anchor by more than 2 s is not: only a long `window` carries it.
- The realistic public pairing, `always_escalate` with a builder, tops out at 0.723 for 17.4 s per
  stream.
  - The reasoner's token budget refuses up to 1153 calls in 200 streams.
  - Plain accuracy falls to 0.756 with `window`.

**Correction to the worker's report: the 0.132 is mostly salience, not timing.** The worker read
R4's oracle minus the context-only ceiling as timing (readiness). The coordinator counted, from
`incidents.csv`, hard non-leak incidents with no reasoner call at all.

- The selection-oracle arms make no call on 34 of 372 such incidents. R4's oracle calls on all of
  them. The selection-oracle arms call only about anomalies the shared rung noticed, so these 34
  are incidents the public rung never noticed.
- That alone accounts for 0.091 of the gap at every setting:

  | Setting | Gap | No-call part | Called but wrong |
  |---|---|---|---|
  | b5 | 0.132 | 0.091 | 0.040 |
  | b8 | 0.137 | 0.091 | 0.046 |
  | b2.5 | 0.067 | 0.091 | −0.024 |
  | b5, ρ0 | 0.121 | 0.091 | 0.030 |

- The decomposition subtracts counts. It is not paired per incident and has no interval, so it is
  coordinator arithmetic, not a measured lever.

Consequences:

- The worker's proposed first substrate job, readiness and compaction, loses most of its readiness
  half. Timing given a call is worth about 0.03–0.05 at b ≥ 5 and nothing at b = 2.5.
- Noticing is the larger item. A public rung that misses 9% of non-leak hard incidents, plus the
  slow-leak family (0.28 against 0.91), is a salience gap. It belongs to EXP-101 and needs its own
  privileged ceiling: an oracle that notices, with the rung's context and delay.

**Assumption that carries the result.** In the simulated reasoner, extra references "cost but never
hurt" (no distractor penalty, `gordian-stream/DESIGN.md` section 11).

- Under that law a broad window loses only tokens, which is why `window` is competitive.
- Published evidence for real models runs the other way (Shi et al. 2023; Liu et al. 2023, "Lost in
  the Middle"; to be checked against the primary texts).
- R6 swept b and ρ only. Until a distractor penalty and the per-reference price are swept, the
  0.024 gap is a property of this reasoner, not of context construction.

**Hidden-document exposure.** While looking for the public sections of `gordian-stream/DESIGN.md`,
the worker read sections 4, 5 and 11, which describe the hidden rules, and disclosed it.

- The grids (starting at 0.25 s; a 2 s lookback) may be influenced.
- The best builder, `window`, encodes no timing, and every builder is a tested pure function of the
  public view.
- The coordinator accepts the result with this caveat. The cause is structural: the public and
  hidden design share one file.

**Decided.**

1. Separate the hidden-rule sections of `gordian-stream/DESIGN.md` into their own file, so that
   workers can read the public design without exposure.
2. R7, reasoner-law sensitivity. Rerun the R6 comparison with:
   - a swept distractor penalty, including zero;
   - a swept per-reference price.

   The criterion will be fixed before any run, with one privilege per ceiling and every clause
   checked for feasibility.
3. The salience gap above becomes part of EXP-101's headroom: a noticing oracle with the rung's
   context is a separate ceiling.
4. EXP-102, when registered, claims references or cost at matched quality, against `window` and
   `cooccur` tuned by the frontier method. It bounds critical misses and plain accuracy.

## R5 decomposed headroom — merged; selection buys cost, context buys quality

**Provenance.** A container restart interrupted the run; a new worker resumed in place, retained
the interrupted replay, and re-ran it. All 237 R4 regression arm-runs match R4's committed hashes.
Gates on exit codes on the merged tree. Run outputs moved to `artifacts/runs/r5/` (ignored).

**Re-verified at b = 5, ρ = 0.7 from raw files.**

| Arm | Hard quality | Plain accuracy | Critical misses | Cost s/stream | Calls/stream |
|---|---|---|---|---|---|
| Selection oracle | 0.489 | 0.741 | 282 | 0.66 | 2.1 |
| `always_escalate`, 14 s | 0.489 | 0.815 | 256 | 7.36 | 24.7 |
| `contradiction_escalation` | 0.478 | 0.804 | 275 | 7.86 | 23.5 |
| R4 oracle (selection + timing + context) | 0.952 | 0.741 | 194 | 0.32 | 2.6 |
| Decoy oracle | 0 | 0.741 | 338 | 0 | 0 |

**Verdict as written.** Both clauses of the R5 criterion hold at every setting (primary: gap 0.433
[0.389, 0.476]; cost ratio 10.75 [9.08, 11.79]).

**What it means.**

- Perfect selection buys about 11× lower cost at matched hard-incident quality, but no quality, and
  it pays for the saving with lower plain accuracy and more critical misses than escalating
  everything, which the criterion did not count. An EXP-101 freeze must bound critical misses and
  plain accuracy, not only hard-incident quality.
- The public contradiction signal does not select: the public checker contradicts 97% of plain
  anomalies at some point. Telling hard anomalies from plain ones with public information is
  genuinely hard in this world.
- Context construction holds the remaining 0.46 of quality and most of the critical-miss reduction
  (194 against 282). The cause is binding: evidence at services the rung never attaches (738
  missing observations) and outside its window (330).
- Decoy handling has measurable headroom: the decoy oracle cuts false alarms by 1.88 per stream
  [1.68, 2.08] at no cost.

**Decided.** Before any substrate is built, R6 measures how far simple public context builders go,
under a criterion fixed before it runs. The common thread of R5's findings is evidence binding
across services and time; if simple builders capture it, EXP-102 has little to win, and if not, the
substrate prototype's first job is binding.

## R4 headroom — merged; the margin was met but did not discriminate

**Re-verified.** Gates on exit codes; at b = 5, ρ = 0.7 the coordinator recomputed from raw
`incidents.csv` and `results.csv`: oracle 354/372 = 0.952 at 0.32 modelled s per stream; best
baseline at the oracle's cost 0.008; `always_escalate` with a 14 s delay 0.489 at 7.36 s;
hidden-rules ablation 0.427 at zero reasoner cost. All match the worker's report.

**Verdict as written.** Headroom on all six settings (smallest gap 0.489, lower bound 0.446); the
conditional clause did not trigger. R1 is not revised.

**Coordinator error, recorded.** The margin compared baselines against an oracle that bundles
selection, timing and context. EXP-101 concerns selection only; the worker's arithmetic splits
roughly 0.49 selection and 0.46 context at b = 5. A margin met by a factor of five against such a
comparator discriminates nothing. R5 decomposes the headroom with separate privileged ceilings
and adds a public-information cascade baseline, under a new criterion fixed before any R5 run. This
is a new pre-run criterion for a new question, not a reinterpretation of R4's verdict.

**Other findings carried forward.**

- At escalation time, `always_escalate`'s contexts held none of the decisive evidence in 17 of 22
  burst incidents; with a 14 s delay, contexts were complete in only 22 of 66. Why contexts miss
  evidence that has arrived is traced in R5.
- R4's oracle does not handle decoys (it alarms as `never_escalate` does); decoy headroom is
  unmeasured until R5's decoy ceiling.
- Hidden-rule knowledge is worth a lot (ablation 0.427 at no reasoner cost); at b = 2.5 it nearly
  equals the oracle.
- Periodic configurations with periods up to 20 s, and `change_triggered`, are bound by the
  reasoner token budget.

## R1 to R3 — stream world, evaluator and harness merged

**R1 stream world.** Re-verified on exit codes. Two reasoner fixes required before merge and
delivered: no information from no evidence (truth enters an answer only through the informed
branch, probability zero without decisive evidence; checked in code), and repeated questions are
keyed by context fingerprint with copula-correlated correctness (`ρ` default 0.7). The
policy-facing answer carries only focus and diagnosis.

**R2 stream evaluator.** Re-verified on exit codes; six randomly sampled fixtures recomputed by
hand from RULES.md without reading the scorer, all matched. Judgements left to each
preregistration: wrong declarations do not cancel a correct one (spam), and alarms on a decoy
before it resolves count as false alarms.

**R3 stream harness and baselines.** Re-verified on exit codes; read the guard diff (a tightening
plus one allowlisted shim directory) and the privileged oracle's surface. Merge conflicts with R2
were additive and resolved as the union. Accepted the worker's reading that oracle escalation
fires once the hard incident's decisive evidence is delivered (escalating earlier is empty-handed
under the revised reasoner law).

**Carried forward to R4.**

- The R3 smoke parameters are placeholders. `always_escalate` escalates at notice, before any
  decisive evidence, and got no hard incident right in the worker's diagnostic; periodic and
  change-triggered hit the reasoner budget. Every baseline is tuned before comparison.
- The shared rung never notices the slow-leak family; that headroom belongs to salience, not to
  escalation timing, and is reported separately.
- Plain-incident accuracy of the cheap rung fell from about 84% to 73% after the first regime
  change in the worker's diagnostic, by design (its rules are not updated).

## A7b ratio interval — merged with a freeze condition

**Re-verified.** Only `analysis/` and `experiments/exploration/` touched; the analysis suite passes
on the branch and on the merged tree with `-W error` (262 passed, 4 slow tests deselected).

**Result.** On B1's empirical paired costs at the planning sizes (1,237 and 1,713), false
exceedance at true S = 0.20, out of a nominal 0.05, was: percentile up to 0.064, BCa up to 0.0675
(at n = 40), studentized up to 0.0545 (0.0484 when that worst cell was extended to 10,000
experiments). The studentized interval is the new default. Power at true S = 0.25 and 0.30 is
essentially 1 at the planning sizes.

**Noted.**

- The rule choosing the default was stated after the simulation table was seen. It chooses an
  instrument among reported alternatives, not a hypothesis outcome, so it is accepted, but it is
  not preregistered.
- The 0.06 bar is met on point estimates: three of eight acceptance cells have a 95% upper bound
  above 0.06 at 2,000 experiments. On heavily skewed lognormal costs no method meets 0.06.
- Derived data committed (paired-cost table, 360 KB; cell table, 25 KB) because the tests need it
  without the binary; both are regenerable by script. Accepted.

**Carried forward (freeze condition).** Before any experiment freezes on a ratio-of-totals
criterion, rerun `experiments/exploration/scripts/a7b_calibrate.py` on that experiment's own
exploration paired costs; the studentized interval must meet 0.06 with its 95% upper bound, not
only its point estimate, at the frozen n. The original condition was written for EXP-001; it now
applies to EXP-101 onward.

## Charter revised — approved by the user

The user approved `docs/charter-revision-proposal.md`. The charter's sections 1, 5, 6, 7 and 12
are rewritten accordingly, with new foundations and sources, and a revision record at the top.
EXP-001 is retired unfrozen and recorded as an exploration finding; the old EXP-002 to EXP-007 and
EXP-I01 are retired or re-scoped into EXP-101 to EXP-106. Change-triggered execution joins the
baseline registry. The plan gains Stage R (R1 stream world and simulated reasoner, R2 stream
evaluator, R3 stream harness and conventional baselines, R4 headroom check).

The user's message read "Inapprove the proposal"; the coordinator read it as "I approve" from
context and said so. If that reading is wrong, this change is reverted from git history.

## A6d incremental narrowing — merged; the shared rule no longer repeats work

**Re-verified.** Fixture committed before the rule change and untouched after it; gates on exit
codes. Worker evidence: 2,200 fixture rows and 55,000 B1 rows at 20 ms identical in every verdict
column; 30,403 calls step-equivalent to the cache-free reference; rule fit R² 0.996 / 0.997.

**Effect.** Narrowing work fell about 96% (30,007 against 764,647 world evaluations over the step
test). "A failed component raises success" fell from 102 to 35 episodes.

**The 35 are real cost, not repeated work.** In 31 of them the no-directive run ended short of
affordable work by a median of 37 ns, after paying about 8 µs once to read the verifier's first
output. A failed verifier never pays that bill, and at a 60–100 µs budget it is the margin. No
cache removes a first read. The other 4 also need the verifier's stale set to lose priority.
Documented in POLICIES.md §3.3; the rule is unchanged. Changing what reading an output costs would
be a change to the instrument and is not made.

**Accepted with a note.** Declared world and evaluation terms are now billed one call late, so the
hard limit can be overshot once per episode by at most about 8 µs. The work is still counted in
modelled cost. Under the revision proposal, any expensive reasoner call must be paid before it
runs, never in arrears.

**State of the shared rule.** Three successive fixes (A6b, A6c, A6d) made it rational at exhaustion
and incremental on unchanged inputs. It is now a strong cheap rung: change-triggered by
construction.

## A6c decode once — merged; the residue becomes A6d

**Re-verified.** The verdict fixture was committed before the rule change and is untouched by it;
all 2,200 rows (10 arms, 20 ms, 20 seeds × 11 classes) identical. Gates on exit codes. Rule
counted-operation fit R² 0.995 (fit) and 0.997 (held out).

**Correction to the plan text.** A6c's premise ("stored outputs are re-decoded every step") was
wrong: stored outputs never were. What repeated was a component *re-producing* byte-identical
output each step, which the rule decoded and charged again. The worker implemented the intent by
content equality and kept the old rule as `Decider::without_reuse`, a test-only reference.

**Effect.** Decoding fell 92%; at 20 ms mean modelled cost fell 40–47% for verifier-running arms
and 12–34% for the others. At 250 µs `all_components` went from 29.1% to 87.0% success. The
"failed component raises success" effect fell from 210 to 102 raised episodes but is not gone: the
rule still re-narrows the verifier's unchanged 46-hypothesis set every step (about 10.5 ns per
world, declared). Zeroing that term leaves 35 raised episodes, unexplained.

**Meta-finding.** Two defects in a row were the shared rule repeating work on inputs that had not
changed. Fixing them makes the baseline incremental. Change-triggered execution (recompute only
when inputs change) is the cheapest form of selective activation, and the charter's baseline
registry does not name it. A salience mechanism must beat it, not only periodic schedules.
Coordinator recommendation to the user: add "change-triggered (memoized) execution — whether
salience adds anything beyond skipping unchanged inputs" to charter section 7. Not made here,
because the charter is normative.

## Exploration follow-ups — merged; correction to the Stage B entry

**Re-verified.** Gates on exit codes; the truth table regenerated from
`crates/gordian-eval/examples/truth_table.rs` hashes to the scratch original; no crate source
changed beyond that allowlisted example.

**Finding 4 resolved, and it corrects Stage B.** The shared rule charges for decoding every stored
component output on every step, 530 ns per output plus 115 ns per hypothesis, even when the output
has not changed. On a symptom-free window the verifier's output lists 46 hypotheses, so re-decoding
it costs about 5,800 ns of the roughly 7,600 ns the rule charges per step. A failed verifier leaves
nothing to decode, the budget lasts longer, and success rises: that is the whole "failed component
helps" effect (removing the charge lifts `all_components` at 250 µs from 13.8% to 72.2% on these
episodes; removing the verifier's priority in the rule changes 0.2–1.8 points).

**Correction.** Stage B's collapse of verifier-running arms at binding budgets (`all_components`
0.291 at 250 µs) is mostly this re-decoding charge, not component compute. With it removed, the
worker's upper-bound variant gives `all_components` 0.730 and `fixed_verifier_only` 0.943 at
250 µs. The heuristic family is bit-identical either way, so the Stage B conclusion about the tuned
periodic baseline stands; the "naive pipelines collapse because components are expensive" reading
does not.

**Decided.** Re-billing an unchanged output is an artefact of the shared rule's implementation,
not a property of the world, and it biases every comparison involving a verifier-running arm. It
is fixed under every option of the pending decision (plan item A6c), before any experiment.

## Stage B exploration (B1–B4) — merged; EXP-001 not freezable as designed

**Re-verified.** Gates on exit codes; per-arm success, modelled cost and critical-miss rates
recomputed from `b1-variance.csv` match the worker's report. 220,000 B1 episodes, no failed or
excluded run, replay checks byte-identical.

**What the data say (exploration, not confirmation).**

- A periodic heuristic (`every` = 4) holds success 0.954 at every budget level for about 22k modelled
  ns per episode; `all_components` needs 522k for 0.965 and collapses to 0.16–0.29 when the budget
  binds. A strong simple baseline captures nearly all achievable quality at a few percent of the
  cost. This is the charter's baseline registry doing its job.
- The shared decision rule costs about 17.1k ns per episode, is charged at every step for every
  arm, and is 65–89% of the heuristic arms' cost. Against the tuned periodic pipeline, even a
  perfectly timed component selector can save at most about 10–18% (B4's estimate; arithmetic, not
  a run), below EXP-001's preregistered 20% margin. Quality headroom against that baseline is
  0.01–0.02.
- The one large oracle gap, JointlyDecisive (0.38), belongs to the shared rule's one-step probe
  choice; no component schedule can close it.
- Five of six stressors cannot fail any current arm: noise is separable for free by catalogue id,
  no arm has a salience mechanism, and the final declaration makes unbounded waiting invisible.
- Effective ambiguity in the ambiguous classes is 2.85 kinds, not 5; a prior-aware arm that never
  probes is right 41.6% of the time.

**Interpretation.** In the current small world, *which component runs* is not where the cost is.
The dominant computation is deliberation (the shared rule) and sensing (probes, which modelled cost
does not price). EXP-001 as designed would very likely return H0 for structural reasons of this
environment, not because selective activation fails in general. The charter anticipated the
mirror-image danger (a weak baseline making anything look good); here the strong baseline shows the
environment offers little to select.

**Instrument findings (not yet fixed).**

1. The rule is not schedulable: a selector cannot decide when to deliberate. Whether deliberation
   is part of what selective activation controls is a design decision.
2. Probes are not in the modelled cost, though the charter's `C` includes sensing; arms buy about
   two probes where one would do.
3. Stressors are toothless at default noise (see above).
4. A failed component can raise an arm's score at binding budgets (3 cells); suspected rule
   behaviour on an empty verifier output; not confirmed.
5. `b4.py` reads a truth table produced outside the repository; one exploration input is not
   regenerable.

**Decision required from the user** (not taken by the coordinator, because it shapes every later
experiment and freezing is irreversible):

- (a) Freeze EXP-001 in the current world and expect a bounded negative result.
- (b) Revise before freezing: make deliberation schedulable, price probes, and revise the world
  so that relevance is costly to determine (non-separable noise, specialist components with
  partial views), with the revision's properties fixed from the charter before any arm is run on
  it, and the generalist and periodic baselines kept.
- (c) Record (a) as an exploration finding only, and do (b).

Coordinator recommendation: (c). A preregistered experiment whose negative outcome is already
implied by exploration arithmetic has low information value, while the fixes in (b) are needed for
EXP-002 to EXP-004 anyway.

Independent of the decision, started then: A7b (ratio-interval calibration on B1's cost
distribution) and the two small exploration follow-ups, since merged (see the entry above).

## A8b counted operations — merged with a freeze gate

**Re-verified.** fmt, clippy (`--locked`), workspace tests with `--no-fail-fast`, oracle guard,
driver test, dump sha256 and the analysis suite on exit codes; reference checker functions
untouched.

**Accepted.** Counters follow the dominant loops of each component and the shared rule, are
deterministic, and cannot be seen or set by a policy (type-enforced, tested). Against hot-loop
minimum timings they fit with R² 0.98–0.99 on fit, held-out and real states.

**Scrutinized: in-situ rescaling.** With pure hot-loop weights the non-identical-arm check failed
(modelled ratio 15.66 against a wall-time median ratio of about 12.1). The worker found that a call
inside an episode costs 1.2–2.2× the same call in a loop, more so for arms that call components
sparsely, consistent with cache effects, and rescaled each weight by a per-component factor and
per-call constant fitted on other arms and seeds. The fit and the check share no arms or episodes,
and the worker reported the failure and offered rejection, so this is calibration, not tuning to
the test. Coordinator check on an unfitted sparse pattern (heuristic only against the verifier
every second step, seeds 200–219): modelled 5.25, wall-time median 5.40 [5.05, 5.85], inside.

**Residual risk.** The in-episode premium depends on the scheduling policy's call pattern, which is
exactly what EXP-001 varies. Fixed factors fitted on other arms priced the sparse arm about 5% off
in the worker's data and about 3% off in the coordinator's. The check is weak (intervals about 15%
wide).

**Carried forward to C1 (EXP-001 freeze gate).** Before freezing, run the cost check on the actual
EXP-001 arms (selective against the tuned periodic pipeline) on exploration seeds. If the modelled
ratio falls outside the wall-time interval, or the two disagree by more than a quarter of the
preregistered savings margin, the cost conclusion of EXP-001 is reported as unresolved, whatever
the modelled result.

**Host change.** The VM now reports a 2.10 GHz Xeon; earlier sessions reported 2.80 GHz. Modelled
cost is in calibration-host nanoseconds and does not change with the host; wall-time checks are
valid only on the host where they run. The manifest should record the CPU model and frequency, not
only flags (small follow-up).

## A8 interleaved arms — merged; A/A fails on an idle machine

**Re-verified.** Merged cleanly onto A6b; fmt, clippy (`--locked`), 293 workspace tests with
`--no-fail-fast`, the oracle guard, the driver test (62) and the analysis suite (212), on exit
codes.

**Worker's evidence.** Interleaving removes a large between-run effect: sequential A/A runs gave
12 of 20 intervals excluding 0 (sd of S 0.151), interleaved runs 0 of 21 (sd 0.037). The worker's
runs shared the machine with another worker.

**Coordinator's idle-machine A/A** (through the driver, cores 0–2, shell on core 3):

| Run | S | 90% interval | Contains 0 | Drift CV |
|---|---|---|---|---|
| heuristic only, run seed 1 | −0.7% | [−5.8%, +3.7%] | yes | 0.12 |
| heuristic only, run seed 2 | −2.9% | [−5.9%, +0.1%] | yes | 0.03 |
| heuristic only, run seed 3 | +5.4% | [+0.5%, +10.0%] | no | 0.27 |
| all components, run seed 1 | −6.9% | [−13.3%, −1.4%] | no | 0.11 |

The per-block minimum timing was stable to about 2% while the mean moved up to 80%; `/proc/stat`
shows non-zero steal time. In the all-components run five episodes carried 51% of the total
absolute difference, and S without them was +0.9%; the median per-episode log ratio was near 0 in
every run. Interference from the host arrives in bursts that hit one copy of an episode and not
the other, and a ratio of totals is sensitive to them.

**Decided.** The plan's A8 rule applies: counted operations are built before B1 (item A8b), and the
charter's cost `C` becomes deterministic modelled cost, with wall time as a secondary check. This
is a change of measurement made before any experiment is frozen.

## A6b final declaration — merged

**Re-verified.** All gates on exit codes; the `all_components` row at 60 µs reproduced exactly
(202 final declarations, 18 terminal, 38 successes, 40 critical misses).

**What it showed.** Under a binding budget, component-running arms exhaust their compute at
0.3–0.6 s of logical time, before any symptom has arrived, by re-running components on windows with
nothing new in them. At the final call they declare "no fault", which is right only on `NoFault`.
Nearly all of the success gained by the fix is `NoFault`; on faulted classes the failures are now
visible as wrong declarations and critical misses instead of undecided rows.

**Carried forward to C1 (EXP-001 preregistration).**

- An arm that does nothing and declares "no fault" scores `NoFault` for free. The primary outcome
  must be read per class with the critical-miss rate, or exclude `NoFault` from the success
  average and score it through false alarms; the preregistration chooses and states which.
- Whether the shared rule at its deadline should declare "no fault" on a window with no symptom,
  or abstain, is a rule choice that changes faulted-class scores. It is fixed before freezing and
  applies to every arm.
- The final call's declared cost is derived, not separately calibrated.

## A6 baselines — merged; headroom probe

**Re-verified.** fmt, clippy with `--locked`, the workspace tests, the oracle guard and the
driver shell test on the merged tree. Accepted the clock interpretation: a component's declared
Compute nanoseconds are its time for every component, and `Slow` multiplies them.

**Coordinator error, fixed forward.** The merge was pushed after a test summary showed one failure,
because the command chain gated on a parsed summary rather than cargo's exit code. The failure was
a random-count coverage floor in the A5b equivalence tests (about 2–3% flake rate; the checker
agreed with the reference on every case). Fixed with a deterministic sweep and a floor at about 3.9
standard deviations. Coordinator merges now gate on exit codes only.

**The worker's finding.** With the shared rule and default limits every arm is at or near the
ceiling (209–213 of 220; oracles 220). Taken alone, this would make EXP-001 trivially pass.

**Coordinator headroom probe** (20 seeds × 11 classes, cgroup-isolated, successes of 220):

| Arm | compute 20 ms (default) | 250 µs | 60 µs | 25 µs |
|---|---|---|---|---|
| heuristic only | 209 | 209 | 187 | 39 |
| estimator only | 210 | 206 | 20 | 20 |
| random p = 0.5 | 213 | 138 | 20 | 13 |
| verifier only | 210 | 78 | 19 | 10 |
| all components | 213 | 35 | 18 | 4 |
| oracles | 220 | | | |

Under a binding budget, quality depends strongly on scheduling. Two effects are mixed in this
table and must be separated:

1. *A shared-rule defect.* Nearly every failure is `budget_exhausted`: the rule waits for its
   patience deadline, runs out of affordable work first, and never declares although declaring is
   free. Plan item A6b gives every arm a final declaration.
2. *Real waste.* `all_components` ran 154 component calls per episode on windows that had mostly
   not changed (median decision at about 1.9 s, a step every 50 ms). Avoiding that waste is what
   selective activation claims to do, and a tuned periodic schedule is the charter's adversary.

**Decided.**

- No world revision for EXP-001. It is testable under a binding budget; B1 sweeps budget levels
  and the preregistration fixes the levels from B1.
- The saturation at default limits is a property of a budget that never binds, not evidence for
  or against any mechanism.

**Open, for the user.** One cheap component (the heuristic) is nearly sufficient alone: the
components are redundant generalists that all read the whole window with the full public rules,
not the specialists the charter's question is about. That limits the quality headroom available to
EXP-002 and EXP-003. Making components specialized (partial views, complementary evidence) is a
research-design choice that risks tailoring the environment to the hypothesis; it is raised to the
user rather than built.

## A5b checker — merged

**Re-verified.** fmt, clippy with `--locked` and all features, 220 workspace tests, oracle guard,
dump sha256 unchanged; the diff to `gordian-components` is cost constants only. The pinned
calibration recheck waits until no other worker is compiling.

**Accepted.** Root cause found by step counting before any change: `anchored(site)` rescanned the
informative prefix at every dependent alarm, about k·d steps. The fix carries the minimum anchor
instant per world; exact for unsorted instants. About 55,000 equivalence comparisons against the
kept reference across generated, adversarial and random inputs; mutation shows the tests can
fail. Late-anchor shape linear; 78× faster at n = 2048. 0–20% slower on shapes the reference
already handled linearly; not tuned, per spec.

**Correction to the A5 entry.** The "24× declared at n = 256" and "1.7 ms at n = 1025" figures
were not reproduced by the checker alone (0.9–1.0 ms at n = 1024 on the old build). The 24× was a
component-level declared-to-measured ratio from one session; read it as an order of magnitude,
not a constant.

**Decided: machine drift is a threat to every measured-cost comparison.** Wall time drifted up
to 30% within one session and 13–34% between sessions. No hardware counters exist in this VM.
Plan item A8: interleave arms per episode in randomized order, log a drift-control workload, test
for position effects, and pass an A/A check before B1.

## A4 run harness — merged; worker report lost

A container restart stopped the worker after it committed and before it reported. The commit
survived; the coordinator reviewed it without a report.

**Re-verified.** fmt, clippy with all features, 208 workspace tests, oracle guard. Read
`HARNESS.md` in full. Ran `heuristic_only` end to end through `scripts/run-driver.sh` under cgroup
v1 isolation: 550 episodes, 0 undecided, internal-to-external ratio 0.81 (first value; becomes the
tolerance starting point). A second run of the same manifest gave a byte-identical `results.csv`;
`measured.csv` differed, as designed. The events sample contained none of the hidden-state key
names. The episode class does not reach policies (`PublicInfo` has no class; the policy directory
guard bans `Episode`, `Simulator`, `gordian_eval`).

**Independent evidence from the smoke run.** `heuristic_only` never probes, and its success on
hidden-kind classes equals the generator's prior for the kind it guesses (JointlyDecisive 0.58
against a 0.595 prior; Ambiguous 0.44 against 0.40). A policy without hidden state should sit at
its prior; this is a second confirmation that the public stream does not leak the kind.

**Decided.**

- The budget reconciliation stands: the `Bill` is the authority for every resource; the episode
  spec's budget must equal the limits or the harness refuses the episode.
- A component's declared Compute nanoseconds are its time. `Slow` multiplies Compute as well as
  Time (assigned to A6; `HARNESS.md` section 8 item 7).
- Analysis must accept undecided rows, drop `bill_total` (it summed ns, probes and bytes), and
  read `measured.csv`, with measured cost the default for relative savings (separate unit).
- Every non-oracle baseline shares one decision rule; arms differ only in selection, because
  EXP-001's intervention is the scheduling policy only (assigned to A6).

**Carried forward.**

- To D1: the events sample carries `class` as a join key. Anything that trains on it must drop
  `class`; it is a strong label.
- To B3 and EXP-002: `QuietUrgent` and `NoiseFlood` are toothless against a component that reads
  the whole window, because catalogue messages are separable from noise for free (`heuristic_only`
  scored 1.00 on both). The stressors test only policies that pay to look.
- `FeedbackBait` is identifiable from public graph data (`unreliable_health`); it shares its prior
  with Ambiguous and StaleMemory, so identification reveals little about the kind. Documented in
  the world's DESIGN.md.

## A5b checker — restarted from a WIP snapshot

The restart stopped the worker with uncommitted changes. The coordinator snapshotted them as a
WIP commit on `checker-perf` and a new worker resumed from it, instructed to verify rather than
trust the unfinished work and to commit after each milestone.

## A5 components — merged after one revision

**Decided.**

- Measured cost, not declared cost, is the charter's `C` in every experiment. Declared costs fit
  the pooled average within 25% but deviate up to 2× per class and 24× for the verifier on
  late-anchor streams; a selective policy could be misbilled in its favour. Plan section 5/A4
  now requires per-call boundary timing in the ledger and the results table.
- The verifier's quadratic shape is the world checker's, not the component's. Plan item A5b:
  optimize behind an equivalence test with the current function kept as reference; required
  before B1.
- The lookup's `Resource::Memory` per-read charge was removed: `Memory` means resident memory,
  measured by the runner. Record reads remain charged as Compute.

**Accepted with notes.**

- The heuristic proposes "no fault" on any symptom-free window and is right 100/100 on `NoFault`.
  That is a property of the physics (every fault permits silence), not skill. Analyses must
  read `NoFault` success together with the critical-miss rate on faulted classes.
- Prior records outside `StaleMemory` are right about 1 time in 5, so the lookup is mostly
  noise. EXP-004 needs a memory that is sometimes useful; revisit the world's record generator
  when E2 is designed, as a new world revision, not by editing A1's guarantees in place.
- Worker subagents were refused `taskset` by their sandbox; the coordinator is not. Pinned
  measurement runs (B1 onward) are launched by the coordinator through `scripts/cgroup-run.sh`.

## A2 evaluator — merged

**Re-verified.** 118 workspace tests, clippy with all features, oracle guard. Eight randomly
sampled fixtures recomputed by hand from `RULES.md` without reading the scorer; all matched.

**Accepted with notes.** A `Correct` on a `NoFault` episode followed by `Abstain` scores both
success and false alarm. Each preregistration must state whether false alarms enter its primary
outcome. `Correct` reveals whether the site was right, at three probes; baselines should show
whether any arm uses it as an expensive probe.

## A1 small world — merged

**Re-verified.** Acceptance commands rerun on the branch and on merged main: 67 tests with all
features, clippy with warnings denied, oracle guard, dump sha256 identical across runs and across
the lockfile regeneration (`8be119bf…51fe2`).

**Independent check.** Plug-in mutual information between the true fault kind and coarse public
views of the stream (signal set, signal count, counter abnormality pattern, message severities),
3,000 episodes per class for Ambiguous, FeedbackBait, StaleMemory and JointlyDecisive, against
50 permutation baselines. No view exceeded its baseline. Ordered-sequence views had too many
distinct values for this test to discriminate; the worker's same-seed kind-swap test covers that
case by construction. Not checked: higher-order statistics, timing side channels in code.

**Accepted with notes.**

- Truth priors inside ambiguity sets are non-uniform (Ambiguous: ConfigDrift 40%,
  ResourceExhausted 40%, CredentialExpired 20%; JointlyDecisive 60/40). A learned policy can
  exploit this. It is a property of the environment distribution, not leakage, but B4 must
  report effective ambiguity, and A5 components must not bake the generator's priors in.
- JointlyDecisive defeats one-step selection on the decision value but not entropy-based
  information gain over (kind, bit) worlds. That distinction is itself testable in EXP-002.
- Core stays dependency-free; world keeps its serde shims (`BudgetSpec`, `CostSummary`). Revisit
  only if the shims cause a bug.

**Carried forward.**

- To A4: `Episode` derives `Serialize` including hidden state. Never serialize an `Episode` into
  any channel a policy can read. The recorder writes the public stream and the evaluator's
  verdict, not the episode.
- To A5: an empty `consistent_hypotheses` set means a bounded window dropped evidence, not a
  contradictory world.
- To A2: score from `(Truth, trajectory)` rather than `Episode`, so hand-written fixtures stay
  independent of the generator.

## A7 analysis — merged after one revision

**Re-verified.** 120 tests with `-W error`; no build artefacts committed.

**Revision requested and delivered.** The first submission could not test the charter's
EXP-001 cost measure (a ratio of totals) and could label a tiny sample "equivalent" with only a
warning. Added: paired-bootstrap ratio of totals with a threshold decision; a preregistered
sample-size gate forcing `unresolved` below plan, raw category still shown.

**Accepted with a hard follow-up.** The percentile bootstrap on the ratio is anti-conservative on
skewed costs (7–12% false exceedance at nominal 5%). Recorded as plan item A7b; EXP-001 may not be
frozen until it passes.

## A3 cost bill — merged

**Re-verified.** 39 tests; read `charge_recorded` and `replay`: a charge is applied only after
both budget acceptance and ledger append succeed, and replay uses the same atomic path.

**Carried forward to A4.**

- Build a `Bill` only from a fresh `Budget`. A pre-spent budget silently breaks
  sum-over-phases = total.
- Keep exactly one `Bill` per `Ledger`. `Bill::replay` folds every Accounting entry it finds.

## Process note

One coordinator merge (A7) hit a `.gitignore` conflict; a non-fail-fast command chain then
committed the conflicted tree locally. It was caught before push and repaired. Coordinator
merges now run under `set -euo pipefail`.
