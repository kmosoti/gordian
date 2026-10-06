# The medium: an event-driven, tick-based substrate, and its ports

Status: design, approved by the user on 2026-10-06 as the starting point. The crate is
`crates/gordian-medium` (work item M1 in `docs/lab-queue.md`). Nothing here is built until M1's
report says so. Where this file and the charter disagree, the charter wins.

## 1. What the medium is

A sparse, persistent collection of **cells** joined by **synapses**, bathed in a **field** of a few
broadcast scalars, that advances in **ticks**. Events enter through one port, proposals leave
through one port, and everything the medium computes is counted and billed. It is the candidate
mechanism for the charter's four functions (salience, routing, memory, context construction). Its
first job, chosen by the evidence in `docs/review-log.md` (R10), is noticing and anchoring.

The medium is world-agnostic. It knows addresses, tags and scalars, never services, incidents,
faults or tiers. Those meanings live in the adapters.

## 2. Principles

1. **Nothing hidden crosses a port.** The sense adapter is the only place a world touches the
   medium, and it carries public observations only. The medium is an arm under
   `scripts/check-no-oracle.sh` and AGENTS.md's rule on arm knowledge.
2. **Time is logical.** A tick is an integer. The clock adapter derives it from the world's logical
   clock. No wall clock anywhere in the medium.
3. **Every port charges at the boundary.** Cell updates, synapse traversals, event routings and
   field reads are counted per tick and priced in the bill like a component call (the A8b
   convention). A hard limit on operations per tick and on cells and synapses is always on.
4. **Determinism.** Cells and synapses are stored and iterated in id order. No hash maps, no I/O,
   no randomness inside the crate. Scalars are `f32`; the crate uses only `+ − × ÷`, comparison,
   `min`, `max` and `abs`. Any transcendental function is a pinned series in basic operations, as
   in `gordian-stream/src/rng.rs`. Same inputs give the same bytes, on this machine.
5. **Ports are traits in the medium crate; adapters live outside it.** The crate depends on
   `gordian-core` only.
6. **Anchoring and routing are not ports.** They are what the medium does. Exposing either as a
   port would let an adapter supply the answer.

## 3. Types

```text
Address  { domain: u16, node: u16, channel: u16 }      // public structure of the world
Tag      u32                                            // a public symbol
EventRef { tick: u64, seq: u32 }                        // stable handle to an event
Event    { tick: u64, offset_ns: u32, source: Address, tags: Vec<Tag>, value: f32, seq: u32 }

CellId, SynapseId, ArchetypeId                          // dense u32 ids
Cell     { id, archetype, params: [f32; P], state: [f32; S], activation: f32, last_active: u64 }
Synapse  { id, from: CellId, to: CellId, weight: f32, delay_ticks: u8, gate: Gate, plastic: bool }
Gate     None | Cell(CellId) | Field(u8)                // the synapse carries only when the gate is active
Field    { scalars: [f32; F] }                          // broadcast, read-only within a tick

Proposal { kind: u16, anchor: EventRef, refs: Vec<EventRef>, strength: f32, cell: CellId }
OpCounts { cell_updates, synapse_traversals, event_routings, field_reads, proposals }
```

`P`, `S` and `F` are small compile-time constants (first values: 8, 8, 4). An event's `offset_ns`
is its position inside the tick, so a short-tick world can keep fine timing under a long tick.

## 4. The tick

Synchronous update, one tick:

1. The clock adapter names the tick. The field adapter supplies the field.
2. The sense adapter supplies this tick's events. The medium sorts them by `(offset_ns, source,
   seq)` and routes each to the sense cells whose address pattern matches (an index built once
   from the cells, in id order).
3. Messages delayed from earlier ticks are delivered from a ring of per-tick queues.
4. Every cell with input this tick runs its archetype function:
   `(params, state, inputs, field) → (state', activation)`, in id order. Cells without input do
   not run and are not counted. That is the sparsity.
5. Each active cell's outgoing synapses carry `weight × activation` to their targets, delayed by
   `delay_ticks`, if their gate is active. Zero-delay synapses deliver within the tick, in one
   extra pass, at most `max_passes` times (first value 2); anything left is carried to the next
   tick.
6. Emitter cells produce proposals. The effector adapter consumes them.
7. The plasticity adapter runs once at tick end with a summary of the tick.
8. Operation counts go to the ledger adapter. The trace adapter receives a sample.

Within-tick order is total and documented, so replay is exact.

## 4b. The oscillome: ticks and rhythms as nested oscillations (added 2026-10-06, user direction)

**Naming.** The set of nested oscillations the medium keeps is its **oscillome**; the base tick
is the oscillome's fastest oscillation, and "tick" remains the word for one step of it. The
component that computes phases, applies phase gates and fires the schedules below is the
**Oscillome Engine**. Per AGENTS.md ("Language"), the name is a label for a mechanism, not a
claim about what it achieves; the experiments establish that. In code: module `oscillome`, types
`Oscillome` (the spec of oscillations) and `OscillomeEngine`.

W1 found three timescales in the world that one tick cannot serve: burst order at 20–150 ms,
the incident horizon at 6–16 s, retention and regime change at 100–200 s. The oscillome
therefore holds **nested oscillations** beside the base tick, in the spirit of brain
oscillations, with the analogy's limits stated at the end of this section.

- **Global oscillations** (rhythms) are periods in seconds in the `MediumSpec`'s `Oscillome`
  (first set: 0.1 s, 10 s, 100 s; the fastest is the base tick). Each rhythm's **phase** in `[0, 1)` is a pure function of the
  tick index and the tick length, and all phases are broadcast in the field every tick (the
  `Field` gains `phases: [f32; R]`, `R` = 3 first). A rhythm boundary is the tick where its
  phase wraps.
- **Delays, decays and lookbacks are specified in seconds** in the spec and converted to ticks
  at build time, so a change of base tick does not change the program.
- **Phase gates.** `Gate::Phase { rhythm, from, to }`: the synapse carries only while that
  rhythm's phase is in `[from, to)`. This is oscillatory gating: sample in one window,
  integrate in another, emit in a third.
- **Binding by phase.** Two events are co-occurrence candidates at the fast scale when they
  share a fast cycle, and at the slow scale when they share a slow phase bin. The `Coincidence`
  archetype takes the rhythm it binds on as a parameter.
- **Local oscillators.** The `Latch` archetype gains a phase-reset form (`Oscillator`): an event
  resets its phase to 0 and it advances by its own period until it decays. A global rhythm cannot
  time an incident's own horizon, because incidents arrive by a Poisson process and a global
  phase is unrelated to onset; per-incident timing (notice, then wait for the decisive phase)
  belongs to local oscillators.
- **Schedules.** The plasticity adapter runs at the boundaries of the rhythm named in the spec
  (first: the 10 s rhythm), not every tick. Trace sampling happens at the 100 s boundaries.
  Memory decay is counted in slow cycles. Per-cycle work costs nothing per tick, which keeps
  the medium sparse; its operations are still counted when it runs.

**Where the analogy breaks, stated now.** In a brain the rhythms emerge from the dynamics; here
they are imposed clocks, a human prior like the archetypes. Each rhythm, the phase gate and
binding by phase must each earn its place by ablation (charter section 3): a medium with the
rhythms removed is an arm in M2's follow-up. Nothing in the world is periodic, so the rhythms
serve multi-scale integration and consolidation, not resonance with the world.

## 5. Archetypes (first set, hand-written)

One function, many instances, parameters per cell. These are the first set, enough for a
hand-designed noticer; the experiment decides which earn their place.

| Archetype | What it does | First use |
|---|---|---|
| `Sense` | receives external events matching an address pattern; activation = value or 1 | one per (node, channel) |
| `Integrator` | leaky accumulation of input; fires when the level crosses a threshold; resets or decays | abnormal rate per node |
| `Novelty` | keeps a running estimate of its input; fires on deviation | background rate per node |
| `Coincidence` | fires when at least `n` distinct sources arrive within `w` ticks | activity at a node and its neighbours |
| `Gate` | passes input only while a gate signal is active | budget- and deadline-conditioned routing |
| `Latch` | holds activation for `h` ticks after firing | an anomaly that stays open |
| `Emit` | turns activation above a threshold into a proposal, citing the events that drove it | notice |

An emitter's **anchor** is the earliest event, by `(tick, offset_ns)`, among those that
contributed to the activation that crossed its threshold; its `refs` are all contributing events
within its lookback. This is the medium's anchoring rule in its first form and the thing R10 says
the rung gets wrong.

## 6. Ports

| Port | Trait | Direction | Carries | First adapter |
|---|---|---|---|---|
| Clock | `Clock` | in | tick index for a logical instant; tick length ns | harness step loop over `StreamSimulator` |
| Sense | `Sense` | in | this tick's `Vec<Event>` | `StreamEvent::Observed` → `Event` |
| Field | `FieldSource` | in | the `Field` for this tick | budget pressure from the bill; deadline pressure from `StreamPublic::deadlines`; two scalars unused |
| Effector | `Effector` | out | `Proposal`s | proposals of kind `notice` → the harness's noticed anomaly |
| Resource | `Resource` | out, then in | a typed call with declared cost; the answer returns as an `Event` | not in M1 or M2 |
| Ledger | `Ledger` | out | `OpCounts` per tick | `gordian-core` bill |
| Persistence | `Persist` | both | deterministic bytes of the whole medium | in-memory |
| Trace | `Trace` | out | sampled per-tick activity | file, sampled |
| Plasticity | `Plasticity` | in | `fn end_of_tick(&mut Medium, &TickSummary)` | `NoOp` |

**Outcome** is deliberately not a field scalar in the first adapters. The stream never reports
correctness, and an outcome signal exists only in training, charged to evaluation (charter
section 9). When it is added it is a separate adapter, named as privileged where it is.

## 7. Resource accounting

Declared prices, in modelled nanoseconds per operation, in the manifest: a first guess of 20 ns per
cell update, 5 ns per synapse traversal, 10 ns per event routing, 2 ns per field read, to be
calibrated against measured time the way the components were (A7b, A8b). Hard limits in the
manifest: cells, synapses, operations per tick, proposals per tick. A tick that would exceed its
operation limit stops delivering and records a `Truncated` entry; it never silently drops.

## 8. The tick-length question

The world has two timescales: a cascade's partner alarms 20–230 ms after the first alarm; a leak
ramps over 6–14 s. The tick length is a manifest parameter and is swept at 100 ms, 500 ms and 2 s
in every medium experiment until the sweep shows it does not matter. W1 measured
(`experiments/exploration/w1-tick-and-measures.md`): bursts need the short tick (at 500 ms five
in six partner alarms share a tick with the first alarm; at 2 s nearly all), the leak does not
care, and cost is driven by routed events, which are constant across tick lengths, not by ticks.
So `offset_ns` must be read by any archetype that depends on order at 500 ms or longer, and
thresholds on counts are tuned per tick length. Delays are in ticks, so changing the tick length
changes the program graph; a design that depends on the tick must say so, and delays and decays
are specified in seconds and converted.

## 9. What M1 builds, and what it does not

**M1 builds:** the crate; the types above; the tick procedure; the seven archetypes; the nine port
traits with the trivial adapters (in-memory persistence, no-op plasticity, a sampling trace sink, a
counting ledger adapter); operation counting and hard limits; a builder for constructing a medium
from a description (`MediumSpec`, serializable, for manifests); the determinism tests in section
10. No world adapter, no noticer, no learning.

**M2 builds (after M1 and B1):** the stream sense and clock adapters; the effector adapter into
the harness's `Noticer` seam (B1); a hand-designed noticing graph; the comparison in
`docs/lab-queue.md`.

## 10. Tests M1 must carry

- Same `MediumSpec` and same event sequence twice: identical persisted bytes after every tick.
- Snapshot after tick `k`, restore, continue: identical to the uninterrupted run.
- Events supplied in a different order within a tick: identical result.
- A cell with no input is not updated and not counted.
- Operation counts equal an independent recount from the trace.
- The hard limit on operations per tick truncates and records; nothing panics.
- Each archetype against a hand-computed example.
- Property tests (`proptest`, already a workspace dev-dependency): no panic on any spec within the
  limits; counts are monotone in events delivered.
- `cargo clippy` clean, no `unsafe`, no dependency beyond `gordian-core`, `serde` and `proptest`
  (dev).

## 11. Known weaknesses of this design, stated now

- Synchronous ticks make every cell's output one tick late at least. A fully asynchronous
  event-driven medium would not. This is a deliberate first simplification; the tick sweep bounds
  its cost.
- Hand-written archetypes are a human prior. The ablation that matters later is whether learned
  parameters within these archetypes, and then learned wiring, beat the hand design.
- `f32` with fixed order is deterministic on one CPU, not across platforms. The plan already
  claims only "same CPU, same flags".
- The anchoring rule in section 5 is one rule. If it is right, it should beat the rung on the
  mis-anchoring R10 found; if it is wrong, the first medium experiment will say so, which is the
  point.
