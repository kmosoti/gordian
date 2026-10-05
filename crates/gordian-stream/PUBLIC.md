# gordian-stream: what a policy may know

This is the public description of the stream world, for anyone writing a policy, a baseline or a
substrate. An arm may encode what is here, the first world's public physics, and what it learns
from its own run history (AGENTS.md, "The rule that matters most"). Everything else about the stream
is hidden. The hidden design is in [`HIDDEN-DESIGN.md`](HIDDEN-DESIGN.md), which is experimenter
knowledge. Do not read it to write an arm; if you do read it, say so in your report.

The code is authoritative. `StreamPublic` and the action and event types are in `src/params.rs`
and `src/sim.rs`.

## 1. What the stream is, in public terms

A stream is a long run, 600 s by default, of observations from a service graph that uses the first
world's vocabulary: counters, messages, configuration snapshots, and probes. Incidents arrive over
time and overlap. Background observations that belong to no incident arrive throughout.

Incidents come in three tiers:

- **Plain.** The first world's public rules settle it.
- **Hard.** The public rules do not settle it. A correct declaration needs knowledge the cheap rung
  does not have; the reasoner can supply it, given the right evidence.
- **Decoy.** It looks like the start of a hard incident and then resolves. It is not an incident to
  declare.

Which tier an incident has, how many of each there are, and how they are built are hidden. So are
whether and when the world's physics changes during a stream, whether an incident repeats an earlier
one, and what the background is made of. A policy may find any of these out from the stream, at the
price of observing, probing and asking.

## 2. Public information at the start

`StreamPublic` has exactly six fields, and a test pins them:

| Field | What it is | Default |
|---|---|---|
| `services` | The service graph at time zero | 8 to 12 services |
| `duration_ns` | Logical length of the stream | 600 s |
| `deadlines` | Deadline windows per tier and criticality (below) | below |
| `reasoner_cost` | Declared cost of a reasoner call (below) | below |
| `max_context` | Most references one call may carry | 4,096 |
| `budget` | Hard limits over the whole stream | 150 probe units, 1.5 s of probe time, 2×10^10 modelled ns of reasoner cost |

`StreamPublic::world_public_info()` gives the cheap components the first world's `PublicInfo`. It
holds the time-zero graph and the prior records that are true of the first world's physics: each
known kind's full signature and its characteristic message.

Nothing in `StreamPublic` changes during a stream. It is the time-zero graph, not necessarily the
graph in force later.

## 3. Deadlines

Each plain and hard incident has a deadline drawn uniformly from a window after its onset. The
windows are public; the draw and whether an incident is critical are not.

| | Not critical | Critical |
|---|---|---|
| Plain | 30 to 60 s | 15 to 30 s |
| Hard | 40 to 90 s | 20 to 40 s |
| Decoy | none | never critical |

A correct declaration after the deadline is a miss. Critical misses are scored separately.

## 4. The reasoner's declared cost

A call costs `400 + 20 × references` tokens, priced at 250,000 modelled nanoseconds per token. That
is 100 ms of modelled compute for an empty context and 5 ms more per reference. One call costs about
5.7×10^4 times a typical component call.

The cost is charged when the call is made, before any answer exists. A call that does not fit the
budget is refused and costs nothing. The answer arrives after `2 s + 2 ms × references` of logical
time.

What the reasoner does with the context is hidden: how its accuracy depends on the evidence, on the
incident, and on repeated questions. A policy learns it, as it would learn a real model's
behaviour, from its own run history.

## 5. Interface

```text
StreamSimulator::new(stream)        public_info() -> StreamPublic        remaining()
observe_until(now) -> Vec<StreamEvent>           apply(action, now) -> StreamOutcome
StreamAction: Probe { kind, target }
            | Escalate { context: Vec<ObsRef>, question: Question::Diagnose { focus: ObsId } }
            | Declare { anchor: ObsId, diagnosis: Option<StreamHypothesis> }
StreamEvent:  Observed { id, at, obs }  |  Answered { call, at, answer }
```

A policy never holds a `Stream`. The harness holds the simulator and hands the policy events and
`StreamPublic`.

- **Observation ids** are positions in delivery order. A policy that has observed everything up to
  an instant holds exactly `0..n`, so an id carries no information beyond order. A reference to an
  observation not yet delivered is refused.
- **Probes** are the first world's probes, at the first world's costs. They return immediately
  with `ready_at`. The result is an observation (`ObsRef::Probe(n)`).
- **Escalate** asks the reasoner about the incident that the observation `focus` belongs to.
  - The context is a list of references, each held, none repeated, at most `max_context`.
  - The answer is a `StreamEvent::Answered`, delivered by `observe_until` once its latency has
    passed. Its ledger entry kind is `Hypothesis`; observations are `Measurement`.
- **Declare** says which incident `anchor` belongs to and what is wrong with it; `None` says it is
  not an incident.
  - A diagnosis is correct only if both kind and site are right.
  - It costs nothing and changes nothing in the stream.

The stream never reports whether a declaration or an answer was right. A malformed action is
refused without charge.

**Why a declaration is anchored on an observation.** Every observation belongs to exactly one
incident or to none. To declare, a policy names an observation it holds, the same thing it names
to escalate. No handle is issued when an incident starts.
