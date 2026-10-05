# gordian-stream: hidden design record

**Experimenter knowledge. Do not read this file to write an arm.** It describes the hidden side of
the stream world: the tiers' rules, the hard families, recurrence, regime changes, the noise
composition, the simulated reasoner's law and the default parameters. Someone writing a policy,
baseline or substrate reads [`PUBLIC.md`](PUBLIC.md), which holds everything a policy may know.
Someone who reads this file states it in their report. This file was `DESIGN.md` until 2026-10-05.
The review log and exploration reports written before then cite it under that name; the section
numbers are unchanged.

Work item R1 of `docs/local-test-plan.md`. This file records every rule of the stream world,
how each tier is built, the simulated reasoner's law, the default parameters and why they are what
they are, every way a policy might infer hidden state and what was done about it, every place the
build departs from or fills a gap in the plan, and the known weaknesses. The code is
authoritative; this file explains it. Everything described here is built, and tested as stated in
section 15, unless a sentence says "planned".

Read it with `crates/gordian-world/DESIGN.md` beside it. The stream reuses the first world's
observation vocabulary, service graph, fault kinds, probe semantics and public physics, and adds
what the charter's section 5 asks for: a resource ladder, persistent streams, and relevance that
costs computation to judge.

**The rule for anyone writing an arm: this file is experimenter knowledge, not arm
knowledge.** Sections 4 and 5, for example, describe the hidden rules of the hard incidents, because a reviewer must
be able to check the construction. An arm (a baseline, the substrate, any policy) may encode only the
first world's public physics and what it learns from its own run history. A policy that hard-codes
the hidden rules (for example "two characteristic messages at one site means a compound fault") is a
cheap rung with the reasoner's knowledge injected, not a conventional baseline, and the coordinator
should read any such diff as a leak of this file into a policy. A policy that *learns* such rules from
the stream is a different matter and a legitimate research outcome. A knowledge-injected cheap rung
exists only as a labelled ablation (section 12), never as an arm of a confirmatory comparison.

## 1. What is built, and what is not

Built: `generate(&StreamParams) -> Stream`; the policy-facing `StreamSimulator` with `observe_until`
and `apply`; the actions `Probe`, `Escalate` and `Declare`; the simulated reasoner; the feature-gated
`oracle`; `scripts/check-no-oracle.sh` extended to the crate; the `dump` example; 79 tests (four more, the cheap-rung tests, moved to `gordian-run`; item 10 of section 13).

Not built, and not claimed: the stream evaluator (R2), the stream harness and baselines (R3), the
headroom check (R4), a real-model rung, a second domain. Nothing here shows that any policy does
well or badly. Four families of the charter's stress suite are not in this world (section 13).

## 2. How a stream is generated

One pure function of `StreamParams` (which `normalized()` clamps first). No wall clock, no
`HashMap`, no platform math: every generator is ChaCha8 keyed by a list of words
`[seed, domain, ...]` through splitmix64 (`rng.rs`), and the two transcendental functions the
stream needs (the logarithm for exponential gaps, the exponential for the reasoner's logistic) are
series in basic IEEE operations (`det_ln`, `det_exp`), because `f64::ln` and `f64::exp` may differ
in the last bit between platforms and a bit can flip a comparison between a draw and a
probability. Their bits are pinned in a test, and so is a digest of the whole public stream of
seed 7.

Order of construction:

1. **Graph** (`regime::gen_graph`): 8 to 12 services by default (6 to 12 allowed), each depending on
   one earlier service and, with probability 1/5 each, on every other earlier one; edges point to
   lower ids. Retried deterministically until at least two pairs of services are unconnected (a
   cascade needs one), at most 64 attempts. Every generated graph in the tests had such pairs.
2. **Regimes** (`regime::epochs`): the scheduled changes are resolved from the seed into *epochs*, a
   physics and a graph in force from an instant on (section 8).
3. **Arrivals**: a Poisson process (exponential gaps, mean 20 s) until `duration - tail`.
4. **For each arrival**, in order: draw everything unconditionally (below), decide whether it is a
   recurrence, choose tier, family and site among the services free at that instant, draw
   criticality, difficulty and deadline, and build the incident's observations (section 4).
5. **Noise**: six independent Poisson processes (section 9).
6. **Assembly**: clip to the duration, stable-sort by instant, number the observations in
   delivery order, and count each incident's decisive observations.

**Draw discipline.** This is the first world's lesson (review log, A1), carried over. Every draw
that fixes an incident's instance is taken unconditionally and in a fixed order from a generator
keyed by the arrival index, and the generators are separate by purpose (`SHAPE`, `MISC`, `BURST`,
`PULSE`, `PHASE2`), so the tier is a decision *about* draws and never changes how many words any
generator consumes. A test forces the tier of one incident after every draw is made and checks
that nothing public changes for its first 6 s (section 5).

An arrival is *skipped* when no suitable service is free (0.27 per stream at the defaults). The arrival
index still advances, so a skip does not shift any other incident's draws.

## 3. What is public

`StreamPublic` is everything a policy may know at the start: the time-zero graph, the duration,
the declared deadline distributions per tier and criticality, the declared cost of a reasoner
call, the context limit, and the hard limits. A test pins its JSON to exactly those six fields and
checks that none of the tier mix, the recurrence probability, the regime schedule or the reasoner's
`(a, b, c)` appears anywhere in it, and that it does not change when any of them does.
`StreamPublic::world_public_info()` gives the cheap components the first world's `PublicInfo`:
the time-zero graph and the prior records that are true of the first world's physics (each kind's
full signature and its characteristic message).

Everything else is hidden and reachable only through `oracle` (section 11).

## 4. The three tiers

An incident is built from up to four parts, each from its own generator:

- **Burst** (first 300 ms): the observations that the public physics reads.
- **Heartbeats**: one reading every 0.8 to 1.5 s at the site, until the incident's last effect,
  then ten benign readings. A reading of a live incident is abnormal with probability 3/4 and a
  *flap* (benign) with probability 1/4. For the leak the series is the site's saturation reading
  instead (below).
- **Phase-2 evidence** (hard incidents only): observations in `[6 s, 16 s)` after onset.
- **Recovery** (decoys only): from the resolution time the heartbeats are all benign.

Timing constants are fixed (`params::timing`), not parameters, because the indistinguishability
argument of section 5 depends on them together: phase 1 is the first `T0` = 6 s; phase-2 evidence
spans `PHASE2` = 10 s; a decoy resolves uniformly in `[T0, T0 + 18 s]`; there are 10 benign beats
after a resolution or a closure, of which the first 5 are decisive evidence of a resolution.

### 4.1 Plain

Obeys the first world's public physics exactly (`physics::counters`, `messages`,
`characteristic_message`, `probe_result`, the propagation rule). Two presentations:

- **Identified** (about 90%): the first world's `Identified` profile: an `ErrorRate` anchor at the
  site, the kind's other site counters, its characteristic message, and its counters and messages
  at every dependent. The public rules leave exactly one hypothesis.
- **Duo** (about 10%, kinds `ResourceExhausted` and `DependencyDown` only, needs a dependent): the
  anchor, `Latency` at the site and `ErrorRate` at one dependent. The public rules leave exactly
  those two kinds at the site open, and one `ResourceUsage` probe settles it. The first world has no
  presentation that leaves two kinds open, one probe resolves, and keeps the physics unchanged;
  this is the construction that fits.

All of a plain incident's burst is decisive evidence (the signature). The guarantee, and how it is
tested, is in section 15: from the incident's own evidence the shared rule declares the true
hypothesis in every case (324 of 324, with exactly one probe for each of the 35 duos and none for
the rest); handed everything at the site and its dependents for three seconds, with the noise and
any other incident, it does so for 245 of 261.

### 4.2 Hard

A hard incident is one of four families: one is drawn uniformly, and if it is not feasible at that
instant (a cascade needs a free unconnected pair; a split brain needs two free services) the next
in declaration order is taken. The
site is the root cause, and the truth is `(Hard(kind), site)`, which no first-world component can
output. Each family presents in one of two modes, chosen with equal probability: **Mimic**, where
phase 1 looks like a plain incident of a known kind and the evidence that breaks the first world's
rules arrives in phase 2; and **Contradict**, where phase 1 already breaks them.

| Family | Hidden rule | Phase 1, Mimic | Phase 1, Contradict | Phase 2 (hard only, all decisive) | What the cheap rung makes of it |
|---|---|---|---|---|---|
| **Compound** | Two known kinds `(a, b)` from `{ResourceExhausted, ConfigDrift, CredentialExpired}` at one site | `a`'s full signature | the union of `a`'s and `b`'s signatures | Mimic: `b`'s characteristic message and counters at the site. Both modes: 3 to 5 messages from the hidden vocabulary | Mimic: confidently `a` at the site until phase 2, then contradiction. Contradict: contradiction from the start |
| **Cascade** | A known kind `a` (`ResourceExhausted` or `DependencyDown`) at the root also alarms a service that the public graph does not connect to it, over a hidden edge | `a`'s full signature at the root | that, plus alarms at the partner (`ErrorRate`, `Latency`, and `ErrorRate` at up to three of its dependents) 20 to 230 ms after the first alarm | Mimic: the partner's alarms. Both: hidden-vocabulary messages, alternating root and partner | an alarm at a service that is neither the site nor its dependent is permitted by no hypothesis: contradiction |
| **SplitBrain** | Two services both act as primary. The site is the one that alarmed first | the first world's `Mixed` profile at the site (`MixedSignals`, `Latency`): leaves `DependencyDown` and `Intermittent` open | that, plus `ErrorRate`, `Latency` and `MixedSignals` at the peer | Mimic: the peer's alarm. Both: hidden-vocabulary messages, alternating site and peer | `MixedSignals` at two services: contradiction. The parity probes read `DependencyDown` at the site |
| **SlowLeak** | The site's saturation climbs from 15 to 25 by 2.5 to 4 per second and crosses the alarm threshold after about 6 to 14 s | *no burst*: a series of benign `Saturation` readings, one per beat | (same) | 3 to 5 messages from the hidden vocabulary | Before it crosses: benign counters, which the public rules ignore, so silence. After: `ResourceExhausted` at the site, with nothing contradicting it, and a confirming probe |

Why each of the cheap rung's failures is a failure of *knowledge* and not of effort:

- The cheap checker returns the empty set when no single `(kind, site)` explains the evidence.
  That is what a compound, a cascade and a split brain do once their rule-breaking evidence is in.
  It is the first world's own definition of "contradictory" and the empty set is, as the first
  world's DESIGN.md says, a statement about the window and not about the world, which a cheap
  policy cannot tell from a dropped anchor.
- A cheap rung that sees only the Mimic phase 1 is *confidently* wrong, and probes confirm it (a
  `ResourceUsage` probe on a compound with a `ResourceExhausted` half answers positive).
- The leak never contradicts anything. It is explained as resource exhaustion, wrongly, and late.
- The decisive evidence of every family includes messages whose ids come from a hidden vocabulary
  (3 to 5 per hard incident). Their ids are outside the catalogue range, so the first world's
  filter drops them, and they are drawn from the same 48-id pool as 70% of the background's
  free-form messages (section 9), so an id does not mark them.

**Probes** answer by rules that are not in `physics` (`probe.rs`, the table at its head): a compound
answers each of its two kinds' dedicated probes positive; a cascade answers as plain kind `a` at the
root and `HealthCheck` positive at the partner; a split brain answers `HealthCheck` and both parity
samples positive at both services, which the first world's parity rule reads as `DependencyDown`;
a leak answers `ResourceUsage` positive from its first threshold crossing. Exhaustive probing of
every kind at every service, after everything has arrived, still leaves the public rules with no
hypothesis for 137 of 137 non-leak hard incidents (a test), and with only `ResourceExhausted` for
the leak.

The scoring contract this implies: a declaration is correct only if kind **and** site equal the
truth. A cheap rung that names the imitated known kind at the right site is wrong, and the
evaluator must say so (section 10).

### 4.3 Decoy

A decoy draws a hard family (the *pretended* family) by the same draws a hard incident uses, so its
phase 1 is the same function of the same draws. It then *resolves*: at a time uniform in
`[6 s, 24 s]` after onset (mean 15.4 s) its heartbeats turn benign, its probes read healthy, and
nothing more happens. Its decisive evidence is the first five benign heartbeats after resolution.
It has no deadline, is never critical, and its truth is `None`.

What separates it from a hard incident, and when: from its resolution on, its site reads benign
every time, where a hard incident's reading flaps one beat in four and its phase-2 evidence arrives.
The simplest public rule that finds this (five benign readings in a row; for the leak, a reading
ten below the highest so far) fires for 313 of 313 decoys, with a median of 19.0 s after onset (the
resolution time plus about five beats), and wrongly on 4 of 318 hard incidents before their
deadline and on 3 of 313 decoys before they resolved. So the evidence exists in the public stream,
costs time, and is what a policy that waits gets. That is the value-of-computation problem of
charter section 4 in this world: the evidence that settles a decoy arrives at about the time the
shortest hard-critical deadlines expire.

## 5. Why a decoy and a hard incident are indistinguishable at first

The construction: for a given arrival, a hard incident and a decoy of the same family make the same
draws from the same generators, and the heartbeat series draws its gap, its flap coin, an abnormal
value, a benign value and a sub-millisecond offset at every beat whether or not the beat is used.
The hard incident's extra evidence comes from a generator (`PHASE2`) that the decoy never calls. A
beat is *live* while it precedes the incident's last effect, which is at least `T0` for a decoy and
at least 25 s for a hard incident, so every beat before `T0` is live for both and reads the same.
Probes at a live decoy answer as the hard family does.

Checked three ways (section 15):

1. **Construction.** For 40 seeds and every incident (1,054 in all, hard and decoy), regenerate with
   its tier swapped after every draw is made. The entire public stream before onset + 6 s (every
   incident, all noise) is identical, and so is the answer to every probe kind at every service the
   incident occupies, at the first and the last instant of phase 1. Afterwards the two streams
   differ for every one of them (the test has power).
2. **Statistics.** Plug-in mutual information between the tier and each of fifteen views of the
   incident's own first-6-s evidence, over 1,957 hard incidents and 1,964 decoys, against 100
   permutations of the tier. The views are: the set of abnormal tags; the counts of observations,
   of abnormal ones and of readings at the site; the first four beats' flap pattern; the number of
   distinct services; the number outside the site and its dependents; the message severities; the
   public checker's set size, emptiness and silence; the kinds it leaves open; the gap before the
   first beat; the site's resource and dependent count; every probe's result at the end of phase
   1; the last saturation reading; the highest reading at the site. None exceeds the maximum of its
   baseline; the nearest is the severities at 0.91 of it. These are the incident's *own* observations,
   perfectly segmented from noise and other incidents, which is the best case for a policy.
3. **Power.** A view of what happens after phase 1 (benign readings at the site between 6 s and
   30 s) has mutual information 0.591 against a baseline maximum of 0.006.

Not checked, and not claimed: higher-order statistics beyond these views, and any timing side
channel in the Rust code (the simulator is not constant-time).

## 6. Deadlines

Each plain and hard incident has a hidden deadline, uniform on a window of time after onset per
tier and criticality. The windows are public (`StreamPublic::deadlines`); the draw is not.

| | not critical | critical |
|---|---|---|
| plain | 30 to 60 s | 15 to 30 s |
| hard | 40 to 90 s | 20 to 40 s |
| decoy | none | never critical |

A correct declaration after the deadline is a miss; the instant is the `now` of the `Declare`
action. Checked against the declared windows over 150 streams (mean and lower-quartile share of
each of the four cells within 4.5 standard deviations of uniform). Critical shares: plain 15%, hard 30%
(checked).

Why these windows (defaults are justified in section 12): a plain incident needs one look and at
most one probe, so its deadline measures the policy's attention, not the incident's difficulty. A
non-critical hard incident has 40 s or more, so *waiting* for a decoy to show itself (median 19 s,
section 4.3) is affordable. A critical hard incident has 20 to 40 s, and a call takes 2 s to
answer, so deciding at 19 s after onset misses about 5% of them, at 22 s about 20%, and at 28 s
about half.

After its deadline an incident keeps alarming for another 5 to 15 s and then closes with ten benign
readings. Closure is therefore not evidence of a decoy for anyone who could still act on it.

## 7. Recurrence

With probability `r` (default 0.25), *given that an earlier incident is available to repeat*, an
arriving incident repeats an earlier one: same tier, same family (kind, partner, pair, mode), same
site, same criticality, same difficulty. Different noise: every observation is drawn afresh, so the
evidence is never the old evidence replayed (a test checks that not one of 631 recurrences
reproduces its original's observations, relative to onset), and the deadline is drawn afresh. An
earlier incident is *available* when every service it occupied is free again (its busy window has
ended) and, for a cascade, its pair is still unconnected in the graph in force. A recurrence of a
decoy is a decoy. A recurrence takes the physics of its own instant, not its original's, so a
record kept from before a regime change describes a signature that no longer exists.

Measured: 837 of 3,488 eligible arrivals recurred at `r = 0.25` (0.2400; sd 0.0073), 758 of 1,293 at
`r = 0.6` (0.5862; sd 0.0136); no arrival without an available template ever recurred. The
conditional definition is a deviation from the plan, which says "with probability `r`": an
unconditional `r` is impossible when the only earlier incidents are still live, and the first
incident cannot repeat anything. At the defaults about 20% of incidents recur (5.5 per stream).

Hard kinds also share a *vocabulary*: within a stream, each hard kind draws its 3 to 5 decisive
messages from its own 4 ids of the 48-id pool, and no two kinds share an id, so a recurrence of a hard
kind reuses ids (a test checks at most 4 per kind and no overlap). The pool is regenerated per
stream, so the vocabulary cannot be learned once and kept.

## 8. Regime changes

At scheduled instants (default 200 s and 400 s) part of the public physics changes, announced
nowhere. Two kinds are built; their targets are drawn from the seed when the stream is generated.

- **`SignatureShift { kind, now_emits }`**: from the change on, incidents of `kind` emit the
  characteristic message of `now_emits` instead of their own. Counters, propagation and probe
  results do not change.
- **`EdgeAdd { dependent, dependency }`**: from the change on, `dependent` depends on `dependency`
  in the *true* graph, an edge the public graph does not have, chosen so that it adds a dependent
  that was not one already. Faults at `dependency` and at everything it depends on now alarm
  `dependent` and its dependents. Probe results do not change.

What the cheap rung can and cannot know after a change. Its knowledge is the first world's compiled
tables and checker, the time-zero graph in `StreamPublic`, and the prior records of
`world_public_info()`. None of it is updated by anything in the stream.

- It cannot know that a change happened. There is no announcement, the public information does not
  change (a test), and the public stream before the first change is identical to the same stream
  with no regimes (a test).
- It can know what the change does *not* touch. The checker stays sound on every incident that
  neither change affects: every plain incident before the first change (over 14 streams) and every
  later one the changes leave alone.
- After a shift, every plain Identified incident of the shifted kind has evidence that excludes its
  true hypothesis (checked on every such incident in 200 streams), because that kind's characteristic
  message is permitted only by another kind. The heuristic's table and the prior records, which
  carry the old message, mislead in the same direction. A duo, which has no characteristic message,
  is unaffected.
- After an edge, every incident that alarms the new dependent has evidence that excludes its true
  hypothesis (checked on every such incident), because the stale graph does not make that service a
  dependent of the site. Nothing else changes: the rules permit and never require a symptom, so a
  dependent that stopped alarming would be invisible, and an edge removal is not built for that
  reason.
- Probes are never affected, so a cheap rung that probes can tell that its rules and its
  evidence disagree. A test pins plain probe answers to the first world's after a change.

Not built: a `ConfigRebase` change (a service's public configuration hash changing legitimately, so
that its unchanged snapshots read as drift). It would be a third, independent way for stale
knowledge to mislead, and is listed in section 13.

## 9. Noise

Six independent Poisson processes. None of them belongs to an incident; the rates are parameters
(`NoiseSpec`, milli-hertz):

| process | default rate | what it is |
|---|---|---|
| benign counters | 2.0 per s | random service and counter, value below the alarm threshold |
| free-form messages | 1.5 per s | random service and severity; 70% take an id from the 48-id pool, 30% a fresh random id |
| catalogue strays | 0.8 per s | one of the eight catalogue messages (including `CheckHealth`) at a random service |
| blips | 0.4 per s | an abnormal counter reading at a random service |
| mini-bursts | 0.03 per s | an `ErrorRate` anchor and a characteristic message at one service within 30 ms, as the first moments of a plain incident, which then stop (no heartbeat ever follows) |
| snapshots | 0.1 per s | an unchanged configuration snapshot |

**The first world's filter fails here.** The first world separates signal from noise by the
catalogue: keep a counter at or above the threshold, a catalogue message other than `CheckHealth`,
a changed snapshot. Tested on the first world's `NoiseFlood` (40 seeds) it keeps every signal and
nothing else. On 20 streams here it keeps 57.7% incident evidence and 42.3% background (8,584
strays, 4,915 blips, 732 mini-burst observations; no free-form message gets through); of the
catalogue-range messages only 5.6% belong to an incident; and it keeps 23.8% of the decisive
evidence of hard incidents (88.1% of all decisive evidence, because plain incidents' is
catalogue-range). Three independent ways to fail, each tested.

**No cheap grouping.** Every observation of an incident is at an offset of a whole number of
milliseconds from its onset; without further care all of an incident's observations would share a
residue modulo one millisecond, which no background observation does, and a policy could segment the
stream by it for free. Every offset therefore carries an independent sub-millisecond draw. A test
checks that no two of an incident's first ten observations share a residue and that the residues of
incident and background observations are uniform at three resolutions; it fails if the jitter is
removed (checked by mutation).

**Noise has a local price too.** A cheap rung that windows by the true site (the best case) and
sees the noise and other incidents in that window identifies 245 of 261 plain incidents (93.9%), so
noise costs it about 6% there, and more in the choice of window, which this test is handed.

## 10. Interface

```text
StreamSimulator::new(stream)        public_info() -> StreamPublic        remaining()
observe_until(now) -> Vec<StreamEvent>           apply(action, now) -> StreamOutcome
StreamAction: Probe { kind, target }
            | Escalate { context: Vec<ObsRef>, question: Question::Diagnose { focus: ObsId } }
            | Declare { anchor: ObsId, diagnosis: Option<StreamHypothesis> }
StreamEvent:  Observed { id, at, obs }  |  Answered { call, at, answer }
```

A policy never holds a `Stream`; the harness holds the simulator and hands the policy events and
`StreamPublic`. The simulator is `Clone` (the harness can fork a state; charter section 9).

- **Observation ids** are the position in the time-sorted stream. A policy that has observed
  everything up to an instant holds exactly `0..n`, so an id carries no information beyond order.
  A reference to an observation not yet delivered is refused.
- **Probes** are the first world's probes with the first world's costs; they return immediately with
  `ready_at`, as in the first world. The result is an observation (`ObsRef::Probe(n)`), and a
  measurement.
- **Escalate** asks the simulated reasoner (section 11). The question names the incident by one
  observation the policy holds, `focus`; the context is a list of references, each held, none
  repeated, at most `max_context`. The cost is charged first (and refused without trace if it does
  not fit), then the answer is produced and queued, and `observe_until` delivers it once its latency
  has passed. The answer is a `StreamEvent::Answered`, whose `entry_kind()` is `Hypothesis`;
  observations are `Measurement`. A test pins both.
- **Declare** says which incident `anchor` belongs to and what is wrong with it; `None` says it is
  not an incident. It costs nothing and changes nothing in the stream, and its correctness is
  never reported.

**Why a declaration is anchored on an observation.** The plan asks for a time window or an incident
handle, and for the choice to be justified. A handle issued by the stream when an incident starts
would tell the policy that something is starting, which is the relevance judgement this world
exists to make costly, and for decoys too. A time window is ambiguous whenever incidents overlap
(about three are live at once at the defaults) and can be widened to cover everything. An anchor is
an incident handle that the policy builds from something it can only have obtained by observing:
every observation belongs to exactly one incident or to none, so it resolves without ambiguity, and
it is the same thing the policy already names to say which incident it is escalating about. It also
makes mistakes expensive in the right way: anchoring on a mini-burst, a stray or the wrong incident
is a declaration about nothing or about something else.

**What the evaluator (R2) needs, and gets.** From `oracle::reveal(&Stream)`: for each incident its
tier, criticality, onset, deadline, truth, difficulty, structure, every observation and every
decisive observation; for each observation its label; `StreamTruth::incident_of(ObsId)`. From the
harness's recorded trajectory: each declaration's instant, anchor and diagnosis, and each
escalation's instant, focus and context. From `oracle::calls(&sim)`: for each call its incident,
`q`, `d`, `p` and whether it was right. With those it can score, per incident, whether and when a
correct declaration was made (correct means `diagnosis == truth`, kind and site, and the declaration's
instant at or before the deadline), critical misses, wrong declarations, decoys declared as
incidents (a `Some` anchored on a decoy), correct dismissals (a `None` anchored on a decoy), and
escalations by tier. The evaluator never needs the generator: `StreamTruth` is data and fixtures can
be written by hand.

## 11. The simulated reasoner

It lives on the hidden side. A policy reaches it only through `Escalate`.

**The invariant: the answer depends on the truth only through the decisive evidence in the context.** A
real model's broad knowledge helps it interpret evidence; it cannot conjure the answer from none. The
first version of this reasoner broke that (its accuracy at `q = 0` was 12 to 27%, more than a guess from
the public context achieves, so it answered from hidden truth with no evidence). The law is now built so
that the invariant holds by construction.

**Law.** A call is *informed* with probability

```text
h(q, d) = (sigma(a + b q - c d) - sigma(a - c d)) / (1 - sigma(a - c d))
```

where `q` is the fraction of the focus incident's decisive evidence in the context (counted from hidden
labels over the whole incident, including evidence that has not arrived yet), `d` is the incident's
difficulty, and `(a, b, c)` are the swept parameters. `h(0, d) = 0` and `h` rises with `q` to the value the
old law `sigma(a + b q - c d)` would have given with no chance of luck. An informed call answers the truth.
An uninformed call answers a *guess*, drawn uniformly from `guess_distribution`, which is a function of the
context, the focus's service and the public rules and of nothing else (it takes no truth, no tier, no
label). The accuracy of a call is

```text
p = p0 + (1 - p0) h
```

where `p0` is the share of the guess distribution that falls on the truth: the accuracy of a
truth-independent guess from this context.

**The guess.** It reads the context's observations at the focus's service, upstream of it and downstream
of it through the first world's public checker, as the cheap rung would, and picks uniformly among the
hypotheses the checker leaves open that put the cause at the focus or upstream (and "not an incident"). When
the checker leaves none (the evidence contradicts every single fault) it picks "not an incident" or one
of the five known kinds at the focus. It never names a hard kind. So an uninformed answer about a mimic
of a plain incident is the imitated kind at the site (checked: 114 of 114 compound and cascade mimics,
with the regime changes off, because after a change the public rules are stale and the guess would be a
stale reading), and an uninformed answer about a contradiction is a random known kind. `p0` is not an
extra parameter: it is what the public evidence gets anyone. It is large exactly when the public rules
already settle the incident (a plain incident's full signature in the context gives `p0` near one), zero
for every hard incident, and near one in six for a decoy whose context is silent.

**What `q` counts.** Decisive evidence only (section 4). A probe reference counts for cost and never for `q`.
A question about an observation that belongs to no incident has `q = 1` and the background difficulty,
and its correct answer is `None`.

**Draws, and why repeating a question buys nothing.** ChaCha8 keyed by `(stream seed, subject,
fingerprint)`, where the subject is the incident (for background, the focus observation) and the
fingerprint hashes the focus and the context's references in sorted order. An identical question gets an
identical answer however often it is asked and however its references are ordered; a repeat is still
paid for. For different contexts about one incident, informed-ness is correlated by a Gaussian copula:

```text
z = sqrt(rho) z_incident + sqrt(1 - rho) z_context,    informed iff Phi(z) < h
```

with `z_incident` drawn once per incident and `z_context` once per fingerprint, so each call's marginal is
still `h`. `rho` is a stream parameter (default 0.7) swept like `(a, b, c)`. The guess is drawn independently
per fingerprint. A refused call consumes nothing. `Phi^-1` is Wichura's AS 241 in basic operations
(`rng::det_norm_inv`), so the draws replay bit for bit.

**Cost.** Declared as tokens (`400 + 20 per reference`) priced at 250,000 modelled nanoseconds each, so
that it adds to the components' declared `Resource::Compute` nanoseconds: 100 ms of modelled compute for
an empty context, 5 ms per reference more, so a context of 100 costs six bases. The median declared cost of
the four components over windows of 16, 64 and 256 observations is 1,743 ns, so one call costs 57,372 times a
typical component call (the plan requires at least 10^4; a test asserts it, and that one call exceeds 1,000
times the dearest component call at the largest window). The cost is charged before the answer exists, and a
refused call leaves no trace. The answer arrives after a declared latency (2 s plus 2 ms per reference).

**Checked** (section 15 has the tests):

- *No information from no evidence.* Over 1,534 incidents of every tier, the mutual information between the
  answer's category and the truth's, with no decisive evidence in the context, is 0.0139 for an empty context
  and 0.0184 for a context of eight random background observations, against permutation maxima of 0.0221 and
  0.0207; with all the decisive evidence it is 1.9205 against 0.0407 (the test has power). No uninformed
  answer names a hard kind.
- *Hard against decoy, when no public evidence separates them.* For 506 incidents, swapping the tier after
  every draw is made leaves the answer identical when the context is the incident's first six seconds
  (the swapped incident's public evidence, `q` and draws are the same). Over 1,943 unswapped hard and decoy
  incidents the mutual information between the answer and the tier is 0.0017 against a permutation maximum of
  0.0044; with the decisive evidence in the context it is 0.6343 against 0.0059.
- *Marginal accuracy follows the law.* 12,450 calls at the defaults and 12,750 to 13,440 calls at four other
  settings (including `rho` 0, 0.7 and 0.95), each with a different share of the decisive evidence and some
  noise. Correctness is judged by comparing each delivered answer with the truth, `h` is recomputed in the
  test with the standard library's `exp`, `p0` is the simulator's (and is itself checked: at `q = 0` the
  frequency of right answers is `p0`, with `h` identically zero). Calls about one incident are dependent, so the
  statistic is cluster-robust (one cluster per incident). Overall |z| <= 1.1 in all five runs and every one of
  nine subsets (by `q`, by `d`, by expected accuracy above and below one half) within |z| < 2. A question
  about background has `p0` equal to one over the size of the guess distribution, computed independently in the
  test from the public graph.
- *The same question gets the same answer*, with its references in any order, repeated; each repeat is charged;
  different contexts about one incident do give different answers; the answer to a question does not depend
  on what else was asked first.
- *Majority of three different contexts follows the copula and is worse than independence.* On 1,952 hard
  incidents, four triples each (`h` between 0.6 and 0.9, so single calls are right more than half the time):

  | `rho` | majority right | copula predicts | independence predicts |
  |---|---|---|---|
  | 0 | 0.8043 | 0.8056 (z -0.30) | 0.8056 (z -0.30) |
  | 0.7 | 0.7428 | 0.7393 (z 0.43) | 0.8056 (z -7.49) |
  | 0.95 | 0.7278 | 0.7243 (z 0.37) | 0.8056 (z -8.07) |

  Single-call accuracy z is -0.41, 0.88 and 0.57. (Positive correlation lowers a majority's accuracy only
  when single calls are right more than half the time; below one half it raises it.)

**What this assumes.** That better context gives better answers, monotonically, and that noise in the
context costs but never hurts: both are in `h` by construction and are tested against a real model in
EXP-106, not here. That errors on different contexts of one incident are correlated with one coefficient and
Gaussian dependence, which is a stand-in for whatever a real model does. That the guess reads the first
world's rules and nothing more, so a real model's broader priors about plausible incidents are not modelled.
That the reasoner knows the truth when informed and degrades by a logistic in `q` and `d`. A repeated
sample is no longer free (identical contexts agree, different ones are correlated), but how much a real
model's repeated samples are worth is for EXP-106.

## 12. Default parameters and why

| parameter | default | why |
|---|---|---|
| duration | 600 s | the plan's default |
| services | 8 to 12 | the first world used 4 to 12; a cascade and a split brain need room |
| mean gap between arrivals | 20 s | 27 incidents per stream (22.5 plain, 2.3 hard, 2.6 decoy); live for about 60 s on average, so about three overlap, which makes segmenting the stream part of the task, while services are still free; hard and decoy incidents are few per stream, so R4 needs on the order of 200 streams for 450 hard ones |
| tail | 60 s | at least 31.5 s, the latest a decisive observation can arrive |
| tier mix | 80% / 10% / 10% | the plan; drawn per incident, so counts vary |
| recurrence `r` | 0.25 | about 5 recurrences per stream, enough to measure a shortcut, few enough that most incidents are new |
| critical | 15% plain, 30% hard | critical misses are scored separately; hard incidents are where they are decided |
| deadlines | section 6 | see there |
| difficulty `d` | plain 0 to 0.3, decoy 0.2 to 0.8, hard 0.4 to 1, background 0.3 | harder incidents are the ones that need the expensive rung |
| `(a, b, c)` | `(-1, 5, 2)` | the informed probability `h`: at `q = 1`: plain 0.97, hard 0.92, decoy 0.95; at `q = 0.5` and `d = 0.7`, hard 0.48; at `q = 0`: zero; background 0.96. Accuracy is `p0 + (1 - p0) h` |
| `rho` | 0.7 | strongly correlated calls about one incident (the majority of three is worth about 6 points less than under independence), not so much that repeating different contexts is worthless |
| cost | 400 + 20 per ref tokens, 250 µs per token, latency 2 s + 2 ms per ref | section 11; per-reference cost matters from about 20 references |
| budgets | 150 probes, 1.5 s probe time, 2x10^10 modelled ns of reasoner (200 empty calls) | hard limits stay on for every arm; an always-escalate arm reaches the reasoner limit |
| regimes | `SignatureShift` at 200 s, `EdgeAdd` at 400 s | two changes inside 600 s, each leaving incidents before and after |
| noise | section 9 | local window pollution about 6% |

**Recommended sweep for experiments** (the plan requires `(a, b, c)` swept in every experiment; the
headroom check R4 sets the final grid). Three settings and three values of `rho`, with `h` at plain `q=1 d=.15` / hard
`q=1 d=.7` / hard `q=.5 d=.7` / hard `q=0` / decoy `q=1 d=.5` (accuracy is `p0 + (1 - p0) h`; `p0` is zero for every
hard incident):

| setting | `(a, b, c)` | `h` |
|---|---|---|
| weak | `(-2, 4, 3)` | 0.81 / 0.47 / 0.09 / 0 / 0.61 |
| default | `(-1, 5, 2)` | 0.97 / 0.92 / 0.48 / 0 / 0.95 |
| strong | `(0, 6, 1)` | 0.99 / 0.99 / 0.86 / 0 / 0.99 |

`rho` in `{0, 0.7, 0.95}`: independent, the default, and nearly one draw per incident.

Also sweep: the reasoner's cost per reference, the deadline windows, the noise rates and `r`.

**Measure what the hidden rules are worth.** Because a rule writer could hand-code the hard
families' visible patterns (section 4.2), R4 should include, as a labelled ablation and not as a
baseline, a cheap rung with those four recognizers injected. The gap between it and the reasoner is
what the vocabulary of hidden ids and the leak are worth; the gap between it and the plain cheap
rung is what the world's hard tier asks of knowledge alone.

## 13. Departures from the plan, gap-fills, and what is not built

1. **Declarations are anchored on an observation**, a third option beside the plan's window or
   handle (section 10).
2. **The reasoner has a declared latency, and its answer is delivered by `observe_until`** after it, not
   returned from `apply`. The plan gives a cost and no latency. A latency makes "escalate now" differ from
   "escalate later" in time and not only in evidence, and delivering the answer through the observation
   channel means a policy cannot act on it before it exists, without trusting the driver. Set the latency
   to zero in the parameters to remove it.
3. **Cost is declared in tokens and priced in modelled nanoseconds** (`ReasonerCost` carries
   calls, tokens, modelled nanoseconds and latency), because the charter reports reasoner cost in its own
   units and also through an exchange rate.
4. **The action set is `Probe`, `Escalate`, `Declare`.** The first world's `Correct` is not carried: it
   never changed the stream there either. Abstaining is not acting.
5. **Recurrence is conditional on an available earlier incident** (section 7).
6. **Plain "identifiable by the cheap rung from the stream, or with one cheap probe"** is read as: from the
   incident's own observations, identifiable; with the noise windowed by the true site, identifiable in 94%.
   Windowing is the policy's problem and the world prices it.
7. **Hard incidents are "not identifiable within the incident's budget"** is read against the first world's
   default budget (12 probes, 250 ms) and, stronger, against exhaustive probing.
8. **Arrivals are dropped when no suitable service is free**, 0.27 per stream at the defaults; two live incidents
   never share a service, so a probe's answer is unambiguous.
9. **Parameter clamps**: services 6 to 12; mean gap at least 100 ms (smaller makes the arrival loop run for
   as many iterations as there are nanoseconds); tail at least 31.5 s; probabilities per mille at most
   1000; `hard <= 1000 - plain`.
10. **The cheap-rung tests live in `gordian-run`, not here.** They need the shared decision rule
    (`Decider`) and `standard_components`, which `gordian-run` owns, and work item R3 made `gordian-run`
    depend on this crate, so a dev-dependency back would be a cycle. Four tests moved to
    `crates/gordian-run/tests/stream_cheap.rs` with their driver and assertions unchanged (plain incidents
    identified from their own evidence; the same in noise by the true site's window; hard incidents the
    cheap rung cannot identify even given everything; the reasoner's cost against a typical component
    call). This crate no longer dev-depends on `gordian-run`, or on `gordian-components`, which only those
    tests used. `src/tests/cheap.rs` keeps the one helper that needs neither (`probing_sim`).
11. **The law is not the plan's `sigma(a + b q - c d)`.** At the coordinator's direction it is
    `p0 + (1 - p0) h(q, d)` with a truth-independent guess, so that the answer depends on the truth only
    through decisive evidence, and draws are keyed by the question's fingerprint with a Gaussian copula
    instead of by call index (section 11). The plan's per-incident call index and its independent draws are
    gone.
12. **Tests live in `src/tests/`**, as in the first world, so that `oracle` is available under `cfg(test)`
    without a self-referencing dev-dependency. `gordian-world` is a dev-dependency with its own
    `reveal-hidden-state` feature, for one control (the first world's `NoiseFlood` labels).

Not built:

- **Four of the charter's stress families** (section 10 of the charter) are not in this world:
  no noise-flood episode (the noise rates can be raised but nothing floods on a schedule), no duplicated
  messages, no feedback loops (`FeedbackBait`), no component failure or timeout. The first world remains
  the regression environment for them. A quiet urgent event during background is approximated by a
  hard incident among strays, not tested as a class.
- **Configuration drift as a stream property** (a regime kind): see section 8.
- **A second question kind** for the reasoner; `Question` is an enum so that one can be added.
- **Repair.** A declaration does not end an incident, as `Correct` did not in the first world.

## 14. Every way a policy might infer hidden state

A leak is any route by which something hidden reaches a policy's *inputs*. The table lists each
route considered, what was done, and where it is tested. "By design" marks information that is
meant to be reachable, at a price.

| # | Route | What was done | Test |
|---|---|---|---|
| 1 | A handle issued at incident start | None issued; declarations anchor on an observation held | section 10 |
| 2 | Serializing or printing the stream | `Stream` and `StreamSimulator` are not `Serialize`; their `Debug` prints counts only; `StreamParams` (which holds the mix, regime schedule and `(a, b, c)`) is serializable for the manifest and banned from policy files | `the_generated_stream_and_the_simulator_are_not_serializable`, `debug_output_of_the_hidden_types_carries_no_hidden_state`, the guard |
| 3 | The public information | Six fields, none hidden, independent of every hidden parameter | `the_public_information_has_exactly_the_documented_fields`, `public_information_does_not_depend_on_any_hidden_parameter` |
| 4 | Observation ids | Position in delivery order, dense, assigned after sorting, delivered once | `observation_ids_are_dense_and_delivered_once` |
| 5 | Which draws a tier consumed | Unconditional draws, separate generators by purpose | tier swap |
| 6 | Statistics of the first 6 s separating hard from decoy | Same draws, same function; checked byte for byte and by mutual information with a positive control | section 5 |
| 7 | A probe's answer, cost or timing | Answers follow the pretended family while a decoy is live; cost and `ready_at` depend on the probe alone | tier swap (probe vectors), `probe_cost_and_ready_at_depend_on_the_probe_alone` |
| 8 | Time residue of an incident's observations | Sub-millisecond jitter on every offset | `an_incidents_observations_cannot_be_grouped_by_their_time_modulo_a_millisecond`, with a mutation check |
| 9 | Duo kind (`ResourceExhausted` or `DependencyDown`) from the stream | Same draws; the whole public stream is identical for the two kinds; only a probe parts them | `the_two_kinds_a_duo_leaves_open_are_the_same_in_the_whole_public_stream` |
| 10 | Value and severity distributions | Abnormal values, benign values and severities are drawn from the same distributions in incident and background evidence; ext messages have random severity | by construction, and the MI test's severity and value views |
| 11 | The reasoner's accuracy draw, `h`, `p0` or `q` | Not returned; an answer carries a diagnosis only. Latency and cost depend on the number of references only | `a_reasoner_answer_is_not_delivered_before_it_is_ready`, the law tests |
| 12 | The reasoner's answers themselves | **Informative about the truth only through the decisive evidence in the context.** Without it the answer is a guess from the context and the public rules (its mutual information with the truth is within its permutation baseline; hard and decoy are answered alike when nothing public separates them, identically under a tier swap). With it the answer is the truth with probability `h`, **by design**, at a price. Repeating a question buys nothing: an identical context gets an identical answer, and different contexts about one incident are correlated (copula, `rho` 0.7) | `without_decisive_evidence_the_answer_carries_no_information_about_the_truth`, `a_hard_incident_and_a_decoy_are_answered_alike_when_no_public_evidence_separates_them`, `the_same_question_gets_the_same_answer_...`, `majority_of_three_different_contexts_follows_the_copula_...` |
| 13 | A deadline or criticality | Hidden. Phase 1 is independent of both; the first public difference is closure, which follows the deadline | tier swap (which swaps criticality too), deadline tests |
| 14 | An incident's end | **By design**: heartbeats stop (a decoy at its resolution, the others after their deadline plus a grace); this is the evidence over time | section 4.3 |
| 15 | A regime change | Not announced; public information static; the stream before the first change is identical with and without regimes; effects are visible only as contradictions | `nothing_announces_a_regime_change`, section 8 tests |
| 16 | A recurrence | The flag is hidden; a policy that keeps records can notice a repeated site and kind. **By design**: that is what a recurrence is for | recurrence tests |
| 17 | The hard kinds' vocabulary | 48-id pool per stream, shared with 70% of background free-form messages; each hard kind uses 4 ids. A policy can learn which ids cluster at an alarming service. **By design** and learnable; not announced | `within_a_stream_each_hard_kind_has_a_small_vocabulary...`, noise tests |
| 18 | Where a new incident can start | Never on a service held by a live incident, so a service with an ongoing alarm series will not start another. Public and trivial | `incidents_arrive_before_the_tail_and_services_are_never_shared` |
| 19 | `CheckHealth` strays | Incidents never emit `CheckHealth`; strays do 1 in 8. A policy that reads the text can drop that eighth for free. Left in: the first world's rules already ignore it | documented |
| 20 | Snapshots | Only the background emits them, and the unchanged ones the physics ignores | documented |
| 21 | Refusals | Depend on public limits and on which observations have been delivered, never on hidden state | `actions_are_refused_without_charge_when_they_are_malformed` |
| 22 | Feedback on correctness | None. `Declare` and `Escalate` never report whether they were right | by construction |
| 23 | Evaluator-side descriptions | This file. See the warning at its head | review |
| 24 | Timing or other side channels in the Rust code | Not claimed. The simulator is not constant-time | not tested |

## 15. Tests

79 tests in the crate, `cargo test -p gordian-stream` (about 50 s in a debug build on two threads), and four
more in `crates/gordian-run/tests/stream_cheap.rs` (section 13, item 10; the claims they carry are listed
below under their old names). The ones that carry the claims:

- **Determinism** (`determinism.rs`): twice-generated streams are equal for 25 seeds and for odd
  parameters; different seeds differ; parameters round-trip through JSON; a fixed action script run twice gives
  the same transcript (and does probe, escalate, declare and receive an answer); a cloned simulator
  replays; a pinned digest of seed 7 (4,702 observations); 60 extreme and odd parameter sets generate
  without panic and keep every invariant; events are measurements and answers are hypotheses.
  `rng.rs` pins the series' bits.
- **Structure** (`structure.rs`): sorted, in range, real services; DAG with unconnected pairs; no arrival in
  the tail; no shared service between live incidents; the tier mix, criticality and hard families
  and modes as declared; ids; every refusal; budgets as hard limits; time may not go backwards; the
  non-serializability and redaction checks; the public information; regime non-announcement.
- **Tiers** (`tiers.rs`; the cheap-rung ones in `gordian-run/tests/stream_cheap.rs`): the cheap rung is
  `gordian_run::standard_components()` and the shared `Decider`, with probes answered by the stream. Plain: identified in 324 of 324 from its own evidence, one
  probe per duo (35 duos), none otherwise; 94% in noise by the true site's window.
  Hard: never explained by the public rules once everything has arrived, even with every probe at every
  service (137 of 137 non-leak incidents contradictory, the leak read as resource exhaustion); the shared
  rule declares the imitated kind at the right site for 182 of 203 and is contradicted from the start
  by a Contradict presentation; hard incidents carry 3 to 5 decisive messages outside the catalogue; the
  first moments give all four readings (unique, few, empty, silent) for hard and decoy at
  indistinguishable rates and plain is read by the rules in over 98%; decoys alarm, resolve, read
  healthy afterwards, and their resolution is readable from the stream at a median of 19 s; hard incidents
  alarm until their deadline; deadlines follow their windows; probe rules are as documented.
- **Indistinguishability** (`indistinguishable.rs`): section 5.
- **Soundness** (`soundness.rs`): the first world's checker never excludes the truth on any prefix of a plain
  incident's evidence (374 incidents, 22,867 prefixes), nor after each of its own probes, nor on the incidents
  a regime change leaves alone; and it excludes things.
- **Noise** (`noise.rs`): the first world's filter is perfect on `NoiseFlood` and fails here three ways; the
  vocabulary of hard incidents appears in the noise; noise has its declared rates; mini-bursts are believable
  incidents; no residue grouping.
- **Recurrence and regime** (`recurrence.rs`): section 7 and 8.
- **Reasoner** (`reasoner.rs`): section 11. The informed probability is zero at `q = 0` and increasing; the marginal law at five settings
  with cluster-robust statistics; no information from no evidence (mutual information, with a control); hard and decoy answered alike
  (tier swap and mutual information); the guess is the imitated kind for mimics; the same question gets the same answer; draws do not
  depend on history; the copula's majority-of-three at `rho` 0, 0.7 and 0.95; background; cost before answer, and the ratio to a component call.
  `rng.rs` also tests the inverse normal against an independent normal distribution function.

The oracle guard: `bash scripts/check-no-oracle.sh` passes, and was run against three planted violations
that it caught (a policy file naming `Stream`; one naming `StreamParams` and `StreamSimulator`; a file outside the
allowlist calling the stream's accessor) and two that it allowed (a policy file naming `StreamAction`,
`StreamEvent` and `StreamPublic`). The allowlist gained `crates/gordian-stream/`; the policy pattern
gained `\bStream\b`, `StreamParams`, `StreamTruth` and `StreamSimulator`, which narrows what a policy may
touch. The future evaluator crate for R2 will need its own allowlist entry; none was added for a crate
that does not exist.

## 16. Known weaknesses and what would change this record

1. **The hard families' visible rules are simple.** A human can write recognizers for three of the four from
   section 4.2 (and the leak's ramp from one more). The world's guarantee is about the *fixed* cheap rung. Section 12
   says how to measure what the hidden rules are worth.
2. **The reasoner knows the truth when informed**, and degrades by a logistic in `q` and `d`; that is an oracle's
   degradation, not any real model's. What it no longer does is answer from the truth without evidence: that
   was fixed (section 11), and the answers to a hard incident and a decoy are the same function of the public
   context until decisive evidence is in it.
3. **Repeated questions are correlated by one Gaussian-copula coefficient.** An identical context repeats its
   answer and different contexts share `rho` of an incident-level draw, so re-escalating is no longer free
   accuracy, but how much a real model's repeats are worth is for EXP-106. The default `rho` of 0.7 is a
   choice, not a measurement; sweep it.
3a. **The guess reads only the first world's rules**, so it is a weak stand-in for a real model's priors, and
   `p0` is high exactly where the cheap rung is already right.
4. **The pool vocabulary is learnable.** A policy that learns which ids cluster at an alarming service gets cheap
   relevance. It is per stream, so it must be re-learned from each stream's own history.
5. **Few hard incidents per stream** (about 2.3). Every claim about them needs many streams.
6. **The cheap-rung identification numbers depend on how the evidence is windowed**, which the tests grant
   (the true incident, or the true site) and a real policy must earn.
7. **Plain incidents' deadlines are long** relative to what they need, so a policy that ignores them for
   10 s loses little: plain misses will mostly be omissions.
8. **Stationary noise.** Poisson, without floods, duplicates or feedback.
9. **Regime changes are two of several possible**, and with the defaults affect few incidents each (about one to two
   plain incidents per change per stream); a test of adaptation will want a higher rate or more streams.
10. **The tests fix the first world's default probe budget** for the cheap rung; a world with a smaller budget has
    less headroom for it, and for any policy that probes.

What would change this record: a leak found by a statistic not in section 5; a policy that beats the cheap rung on
hard incidents using nothing the reasoner supplies; a headroom check (R4) that finds no gap on any setting.
