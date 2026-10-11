### 1.3 Definitions fixed here, with the priors, before any number on the reported ranges was read

Written and committed before the hidden tables of the reported ranges (10000-10099, 40000-40199,
60000-60099, 70000-70199) were produced. The example was debugged on a disjoint smoke range
(**90000-90019**, world A), whose numbers are not reported anywhere. A correction made after the
reported numbers exist is a new commit that says what changed and why.

**D1. Ranges, setting, sides, intervals.** World A (the defaults): 10000-10099 ("tuning") and
40000-40199 ("held-out"), items 1-5. World C (W3's mix, plain 600 / hard 300 / decoy 100 per mille,
nothing else changed): 60000-60099 and 70000-70199, items 1 and 3 (item 6). Setting: `(a, b, c) =
(-1, 5, 2)`, `rho = 0.7` (a marginal accuracy does not depend on `rho`), `delta` in {0, 0.0135, 0.05}
where named (item 2; the other items use 0). Every statistic names its range and side:
*hidden* (the stream crate's own law and labels, through `laws --w4`), *run, hidden* (a kept run's
evaluator files joined to hidden truth), *public ids, hidden labels*. Intervals are 90% percentile
intervals of a cluster bootstrap over whole streams, 10,000 resamples, seed 9950, a fresh generator
per interval (W2's constants). A ratio is a ratio of sums over streams, never a mean of ratios.

**D2. The law is evaluated, not re-implemented.** `p`, `h`, `p0`, `q`, `m` (distractors), `d` are the
fields `oracle::calls` records for a real `StreamAction::Escalate` applied to a `StreamSimulator`
of the stream with the reasoner budget raised to 10^15 modelled ns (nothing is refused; the budget
is not a parameter of generation, and the example asserts that the raised-budget stream equals the
default one in every public byte and every hidden label). The accuracy of a call is its `p`
(expectation), not a draw. A stream at `delta > 0` is generated from the same seed with only
`distractor_penalty` changed and asserted equal to the `delta = 0` stream in every public byte and
label (the penalty takes no draw). Each query uses a fresh simulator (the answer to a question does
not depend on what else was asked: HIDDEN-DESIGN 11, tested there).

**D3. The anomaly, the notice, the ask instant.** The noticer is the B2 later re-anchor
(`sel_reanchor_privileged`: notice_z 2.0, gap 20 ms, min_burst 2, isolation site) at the kept
runs' stream parameters. Its notice records come from kept runs of this arm: 40000-40199 from E1's
held-out run (`e1-heldout-b5-rho0.7`), 10000-10099 from W2's tuning run (`w2-tune-b5-rho0.7`),
60000-60099 and 70000-70199 from W3's world-C runs. The brief allowed "run it yourself or derive a
proxy"; reading the same arm's kept run is a third route, and its justification (E1's and W2's
held-out notice files are byte-identical but for `run_id` and `arm_role`; asked and not run, so no
arm is run) is checked in `w4-checks.csv`. An *anomaly* is one `notice` event (its anchor
observation, its site = the anchor's service, its anchor instant, its notice instant `n`); its
retirement is the `retire` event (or the end of the stream). The incident's *anomaly* is the
earliest `notice` about it (N3: its anchor belongs to the incident; ties by lowest anomaly id) and
is the **primary**; incidents with no such notice are *not noticed* and have no call under any
policy. Every other anomaly (a second notice about an incident, a notice on background) is
*secondary*. Every anchor, site, anchor instant and incident of every notice is checked against the
regenerated stream (the checks must all pass or the example exits non-zero): the stream the run
saw is the stream regenerated here.

**D4. Contexts.**

- `rung`: the references the shared rung's `Rung::context` returns for the anomaly at ask instant
  `t` (read from `crates/gordian-run/src/stream/arms/rung.rs`, `context_around`, and checked
  against recorded contexts in section 1.1): every observation delivered at or before `t` and not
  older than 120 s (the store's retention) that is at or after `anchor_at - 2 s` and is about the
  anomaly's site, or about a **public** dependent of the site (the time-zero graph) and at or
  before `anchor_at + 400 ms`; in id order; if more than 128, the first 32 and the last 96. The
  question's focus is the anchor. No probe is referenced.
- `min`: the decisive observations of the incident (hidden labels) delivered at or before `t`, in
  id order, and nothing else; the same focus. `m = 0`.
- Cost of a context of `r` references: `400 + 20 r` tokens at 250,000 modelled ns each; latency
  `2 s + 2 ms r`.

**D5. Ask policies and what a call is worth.** For an incident with a primary anomaly (notice `n`,
retirement `r_a`, deadline `dl`, `None` for a decoy):

- `dD` for `D` in {4, 8, 12, 16, 20, 30} s: ask at `t = n + D`. A call is made if the anomaly is
  live at `t` (`t < r_a`) and `t` is within the stream; otherwise the incident has no call under
  `dD` (counted, never hidden).
- `opt`: ask at the candidate instant `t` that maximises `p(t) * feasible(t)`, ties to the earliest.
  Candidates: `n`, and each arrival `a` with `n < a`, `a < r_a` and `a + 2 s <= dl` of an observation
  that is in the context (for `rung`: the observations the context rule admits; for `min`: the
  incident's decisive observations). `p(t)` is constant between candidates (the context changes only
  at those arrivals), so the maximum over continuous `t` is attained at one. If no candidate is
  feasible the call is made at `n` and is worth 0 (`opt_skip` below).
- `feasible(t)`: `t + 2 s + 2 ms r(t) <= dl` (inclusive; the declaration is made when the answer is
  delivered, with no step-grid delay). A decoy has no deadline: always feasible.
- The value of a call is `p(t) * feasible(t)`: the expected number of correct decisions (right answer
  declared by the deadline). **Expected correct decisions per call** of a policy on a class of
  incidents is the sum of the values over the incidents with a call divided by the number of such
  incidents, pooled over streams (ratio of sums). `opt_skip` is `opt` without the calls that have
  no feasible candidate (it makes fewer calls).
- The classes: plain (all, and identified/duo), hard (all; compound, cascade, split brain by mode;
  slow leak), decoy. "Plain beside hard": every table has the plain rows.

**D6. The "when" lever (item 1).** The **best fixed delay** is chosen by a rule fixed here: on the
tuning range of a world, the `D` in {4..30} maximising expected correct decisions per call pooled
over all **hard** incidents (ties to the smaller `D`), under the `rung` context at `delta = 0`; the
same `D` is applied on the held-out range ("selected on tuning, read on held-out"). The best fixed
delay *per class* on each range itself (selected and read on the same data, so optimistic for the
fixed side and conservative for the lever) is reported beside. The lever is `opt` minus the best
fixed delay, in expected correct decisions per call, with a paired cluster interval (the same
resamples for both terms).

**D7. The "what" lever (item 2).** For each noticed incident `i`, `t_min(i)` is the `opt` instant of
the `min` context (D5, at the given `delta`). The lever **at the same instant** is
`p_min(t_min) - p_rung(t_min)` where the `rung` context is built at `t_min` (an arm that asks then
with its own context); the lever **at each context's own optimum** is `p_min(t_min) - p_rung(t_rung)`
with `t_rung` the `opt` instant of the `rung` context. `delta` in {0, 0.0135, 0.05}. The context cost
(tokens, modelled seconds, latency) of both is reported with the references and the distractors
`m`; the cost-adjusted value is expected correct decisions per modelled second of reasoner cost
(sum of values over sum of costs).

**D8. Verification power of probes (item 3).** The *scenario*: a call made as B5 would make it
(`rung` context, `d16`, `delta = 0`) for every primary and secondary anomaly (hard, plain, decoy and
background alike); and the *best* scenario (`min` context at `opt`) for the primary anomalies. The
answer is a distribution: it is the truth with probability `h`, and otherwise a uniform draw from
the guess distribution, which is re-implemented in the example from the public checker
(`guess_distribution`) and checked against the stream crate's `p0` for every call (exact equality)
and against the realised answer of every uninformed call (support). A probe set is a non-empty
subset of the six probes at the answered site (63 sets; the two parity samples are individually
uninformative); a seventh, `HC@other`, is the health check at the cascade's partner or the split
brain's peer, and is credited only as an oracle-located probe (an arm would have to find the
service from the stream; the number of candidates it would face is measured). The *observed*
result of a probe is the stream's own `StreamSimulator::apply(Probe)` at the instant the answer is
delivered (ask instant plus latency) at the answered site. The *predicted* result of an answer is
the first world's public table `probe_result(services, Some(answer), bits, hash, probe)` for a
known kind (over both values of the hidden parity bits for the entangled pair, and "hash differs
from the public hash" for `ConfigSnapshot`), all-healthy for "not an incident", and, for a hard
kind, the answer's own family pattern. An answer is *accepted* by a set when every probe in it is
consistent with the prediction (the two parity samples jointly when both are in the set). The
**false-accept rate** of a set for a stratum is the weighted share of wrong answers accepted; the
**false-reject rate** is the weighted share of right answers rejected. The **cheapest separating
set** for a stratum and a tolerance `alpha` is the set of least cost (probe units, then time) with
false-reject rate 0 (or the stated rate) and false-accept rate at most `alpha`, `alpha` in
{0, 0.05, 0.25}; costs are `probe_cost` (units, ms). **Call rates**: B5's selector asks at most
`k` questions per stream at its ready anomalies; the calls per stream at budget `k` are taken as
`min(k, ready anomalies at notice + 16 s)` per stream, computed from the re-anchor's notice and
retire events (an upper bound on any tuned threshold's rate; stated as an assumption). The
verification bill per stream is `k` times the cost of the set (or of the set on the answers that
need it: a hard-kind answer is never a guess), against the 150-unit and 1.5 s limits, with the
re-anchor arm's own probes per stream (mean and maximum, `results.csv`) shown beside.

**D9. The regime loss (item 4).** S1 = plain, non-duo incidents whose known kind is the stream's
shifted kind (the duo has no characteristic message and is unaffected); *before* = onset < 200 s,
*after* = onset >= 200 s. S2 = incidents whose observations differ when rebuilt on the time-zero
graph (`oracle::rebuilt_incident`), onset >= 400 s ("alarming over the added edge"); contrast:
unaltered incidents of the same tier after 400 s. Both at `d16`/`rung`/`delta 0` (what B5 does) and
at `opt`. A paired counterfactual on the incident's own phase-1 evidence (the first 6 s): the
public checker's open set and the guess share `p0` on the actual evidence against the evidence
rebuilt under the first world's physics (for S1) or the time-zero graph (for S2). The **EXP-104
ceiling** for a change is, per stream, the number of its altered incidents after the change
(i) in total and (ii) after the first altered incident of the stream (a learner that re-learns
the signature from one verified sighting recovers every later one and none before it), each
multiplied by the share of those incidents the cheap checker settles correctly on their own
evidence once the rule is restored (rebuilt evidence) and does not settle on the actual evidence.
The **public footprint**: the checker's verdict on the actual evidence (empty set or not), the
instant of the first altered incident after the change, and the share of streams with one.

**D10. What transfers (item 5).** A quantity is *constant across streams at the setting* when the
generator's code feeds it no per-stream draw (read from `params.rs`, `incident.rs`, `reasoner.rs`,
`probe.rs`, `regime.rs`) and the measured minimum and maximum over all streams of the range agree
with the declared value; it is *per stream* otherwise. Each constant carries its public footprint and
the items it bounds.

**D11. Bounds rule (fixed before the numbers).** A clause of P1 or S1 that claims an improvement of
size `x` over B5's tuned selector from a lever is **feasible only if `x` is at most the lower end of
that lever's 90% interval**, and the bound proposed is **half of that lower end**, rounded down to
0.01, so that a clause is satisfiable with margin; a lever whose lower end is below 0.02 is
reported as "not a clause". Clauses are written in the `experiments/criteria/README.md` schema
(`value`, `paired`, `count`, `identity`, `curve_slope` with `point`/`lower` tests).

**Priors** (written before any reported number; central value, then range I would be surprised to
leave). They are not tuned; the report compares them with the measurement.

| quantity (A held-out unless named) | prior |
|---|---|
| hard, `rung`, `d16`: expected correct decisions per call | 0.55 (0.45-0.65) |
| hard, `rung`, `opt` minus best fixed delay (the "when" lever) | 0.03 (0.00-0.07) |
| hard, `min`, `opt` minus `min`, `d16` | 0.05 (0.02-0.08) |
| best fixed delay on tuning, hard | 16 s (12-20) |
| plain, best fixed delay | 4 s; `opt` minus `d16`: 0.05 (0.02-0.09) |
| what lever, `delta 0`, same instant: `min` minus `rung` on hard | 0.33 (0.25-0.42); `delta 0.05`: 0.35 (0.26-0.44) |
| context cost: tokens per call `rung` : `min` | 1,300 : 560 (ratio 1.5-4) |
| compound mimic, wrong imitated kind confirmed by its own kind's probe | confirmed (false-accept 1 for that probe), separated by the other kinds' dedicated probes (2 units) |
| cascade, split brain (DD answer), slow leak (RE answer, after the crossing): false-accept with all six site probes | 1.0 each |
| false-accept, all six site probes, known-kind answers about hard incidents, pooled | 0.55 (0.40-0.75) |
| any wrong hard-kind answer | none (by the law: a guess never names a hard kind) |
| verification units at `k = 16` with a 3-probe set | 48 + 13 of the arm's own = 61 of 150 |
| plain shifted kind: `p` after minus before (`d16`, `rung`) | -0.03 (-0.06 to 0.00) |
| cheap checker settles shifted plain incidents correctly after 200 s | 0 (W2: the true hypothesis is excluded in every case) |
| altered incidents per stream: shift / edge (after the change) | 2.9 / 1.0 (W2) |
| streams with an altered plain incident after 200 s | 0.85 (0.75-0.95) |

**Validation checks** (run with the tables; a failing check stops the pipeline): (V1) streams at
`delta > 0` and at the raised budget equal the default stream; (V2) every notice's anchor instant,
site and incident equal the regenerated stream's; (V3) the example's own `q` and `m` for every call
equal the recorded ones; (V4) the re-implemented guess distribution gives the recorded `p0` for
every call and contains every realised uninformed answer; (V5) the contexts the example builds at
notice + 16 s equal the `Escalate` contexts recorded in W2's ledger (the selection oracle at 16 s,
the same arm) for every recorded call of the checked streams, and the recorded calls replayed
through a simulator reproduce the run's informed and correct counts per incident; (V6) a hard-kind
answer is never wrong, an informed call is always right; (V7) E1's and W2's held-out notice files
are equal but for `run_id` and `arm_role`; (V8) the probe simulator's answers at the incident's
site equal the table of `probe.rs` for each family.
