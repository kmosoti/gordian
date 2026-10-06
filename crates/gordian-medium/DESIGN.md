# gordian-medium: what was built, and where it departs from the design

Work item M1 (`docs/lab-queue.md`), built from `docs/medium-ports.md` (the design). This file
records every place the build departs from the design or fills a gap in it, and why. The tick's
total order is in `src/medium.rs`'s module documentation; the archetypes' parameters and state are
in the table in `src/archetype.rs`.

## Built

The crate depends on `gordian-core` and `serde`; `proptest`, `serde_json` and `criterion` are
dev-dependencies. No world adapter, no noticer, no learning.

| Design item | Where |
|---|---|
| Section 3 types | `src/types.rs` |
| Section 4 tick, in a total order | `Medium::step`, `src/medium.rs` |
| Section 5 archetypes | `src/archetype.rs` |
| Section 6 ports and trivial adapters | `src/ports.rs` |
| Section 7 counting, prices, hard limits, `Truncated` | `OpCounts`, `Prices`, `Limits`, `Truncation` |
| Section 9 `MediumSpec` and builder | `src/spec.rs` |
| Persistence: deterministic bytes | `src/persist.rs` |
| Section 10 tests | `tests/determinism.rs`, `tests/archetypes.rs`, `tests/properties.rs` |
| Cost micro-benchmark | `benches/tick.rs`, results below |

## Departures from the design

Each is a choice where the design was silent, underspecified, or (in two cases) wrong.

### Types (section 3)

1. **`EventRef` carries `offset_ns`.** Section 3 has `EventRef { tick, seq }`. The anchoring rule
   orders events by `(tick, offset_ns)`, and a consumer of a proposal must be able to apply that
   order to its references alone. The derived order is `(tick, offset_ns, seq)`. `(tick, seq)`
   still identifies the event: a tick whose events repeat a `seq` is refused.
2. **Cells carry a support.** Section 5's anchor is "the earliest event among those that
   contributed to the activation", but section 3's cell has nowhere to keep events (`state` is
   `[f32; 8]`). Each cell has `support: Vec<EventRef>`, ascending, at most `max_refs` (a new hard
   limit). Messages carry the sender's support (shared, `Arc<[EventRef]>`). When a support would
   exceed `max_refs`, the earliest are kept (the anchor survives) and the number dropped is
   reported per tick (`TickSummary::refs_dropped`), never silently.
3. **`last_active` is `Option<u64>`**, so "never ran" is not confused with "ran at tick 0".
4. **No `ArchetypeId(u32)`.** The seven archetypes are a closed enum with explicit, stable tags
   in the persisted encoding. A dense id would be an indirection with nothing behind it until
   archetypes are data.
5. **A sense cell's address pattern is a typed `Pattern`** (domain, node, channel, each optional,
   plus an optional required `Tag`), not part of `params`. Addresses are structure, not tunable
   scalars, and a `u32` tag is not exact in `f32`.
6. **Prices are in picoseconds** (`Prices::DECLARED` = 20 000, 5 000, 10 000, 2 000), as in
   `gordian-components`, so a calibrated price below 1 ns keeps its precision. Section 7 declares
   no price for a proposal; it is 0.

### The tick (section 4)

7. **Ticks are consecutive.** The clock names the tick, and the medium refuses any tick other
   than the one after the last (`StepError::TickOutOfOrder`). The driver runs every tick; a tick
   in which nothing has input costs no counted operations. The alternative (allow jumps, and run
   the skipped ticks that have pending work implicitly) needs a field for ticks nobody asked for
   and hides ticks from the ledger and the plasticity port.
8. **A map instead of a ring.** Delayed messages are kept in a `BTreeMap` keyed by the tick they
   are due, not a ring of 256 per-tick queues. Same semantics, no wrap-around arithmetic.
9. **`max_passes` counts every pass, the first included** (first value 2: one extra pass, as
   section 4 says). Zero-delay messages sent in the last pass go to the next tick, counted in
   `TickSummary::carried`. **Consequence for M2:** a chain of k zero-delay stages completes in
   ceil(k / max_passes) ticks; sense → integrator → emitter is three stages, so the emitter runs
   one tick after the evidence, which is up to 2 s of notice latency at a 2 s tick
   (`an_accumulating_cell_prunes_its_support_to_its_lookback` shows it).
10. **Inputs have a canonical order** before a cell runs: wakes, then events in routing order,
    then messages by `(sent tick, sent pass, synapse id)`. Without it, `f32` sums would depend on
    the order messages were collected (sender order). Two tests make the order observable
    through rounding (`1e8 + 1 - 1e8`).
11. **Propagation happens after the whole pass**, so a cell gate (`Gate::Cell`) reads the gate
    cell's activation as of the end of the source's pass: active iff the gate cell ran in this
    tick and its latest activation is above zero. A gate cell that runs in a later pass than the
    source is seen as inactive. `gates_on_synapses_and_the_pass_limit` shows both cases.
12. **A cell propagates only when its activation is not zero.** A cell that ran and stayed quiet
    wakes nobody. This is what keeps the medium sparse past the sense layer.
13. **Malformed input is refused, not repaired.** An event of another tick, a non-finite event
    value or field scalar, or a repeated `seq` makes `step` return a `StepError` and change
    nothing in the medium. The sense adapter has already handed its events over by then (the
    scripted adapter forgets them); retrying is the driver's decision.

### Limits and counting (sections 2 and 7)

14. **What counts.** One event routing per (event, matching cell) delivery, and one per event
    that matches no cell (the lookup was done). One synapse traversal per outgoing synapse of an
    active cell examined, whether or not its gate lets it carry. One field read per field-gated
    traversal and per run of a `Gate` cell. One cell update per run (a cell that runs in two
    passes counts twice; a woken latch counts). The per-tick operation limit applies to the sum
    of those four; proposals have their own limit.
15. **How truncation works.** The limit is checked before each unit of work (one routing; one
    cell run with its field reads; one traversal with its field read). The first unit that
    would exceed it is not done, and from then on nothing more is routed, run or sent in this
    tick. The `Truncation` record counts events not fully routed, cells with input that did not
    run, synapses not traversed, proposals dropped, and the pass where the limit was hit (0 for
    routing). Steps 6 to 8 (effector, plasticity, ledger, trace) still run. Undelivered input is
    dropped, not carried, so that one heavy tick cannot make the next one heavier.
16. **An emitter that fires with an empty support makes no proposal** (a proposal needs an
    anchor); the firing is counted in `TickSummary::unanchored`.

### Archetypes (section 5)

17. **Signature.** Section 5's `(params, state, inputs, field) → (state', activation)` gains
    `dt`, the ticks since the cell last ran, so leak (`Integrator`), silent-tick estimates
    (`Novelty`), window ages (`Coincidence`), hold (`Latch`) and refractory (`Emit`) are applied
    lazily when the cell next has input. That is what lets a cell without input not run. The
    function also returns a wake request, a support rule (replace, merge, keep), whether to
    clear the support after this run, and whether to emit.
18. **`Latch` wakes itself.** Holding activation for h ticks needs the latch to run on those
    ticks with no input, or its targets would never see the held value. A latch asks to be
    woken at the next tick; a wake counts as input and the run is counted. The hold covers the
    firing tick and the h ticks after it.
19. **`Gate` gates on a field scalar only.** Gating on a cell is the synapse's `Gate::Cell`;
    a second mechanism for the same thing would need input ports, which section 3 does not have.
20. **`Coincidence` has at most eight sources**, one state slot per incoming synapse (by synapse
    id). "Distinct sources" is distinct incoming synapses whose message is positive. Ages are
    kept as `f32` ticks, which keeps the state inside `[f32; 8]`. A spec with more incoming
    synapses to a coincidence cell is refused.
21. **`Novelty` uses the mean absolute deviation, not a standard deviation**, because a square
    root is not one of the allowed operations. In the "gaps are zeros" mode the silent ticks
    since the last run enter in closed form (`mean_n = b^n mean`,
    `dev_n = b^n dev + n·rate·|mean|·b^(n-1)`, `b = 1 - rate`); the test checks it against the
    step-by-step recurrence.
22. **`Integrator` fires on the rising edge**: when the level before this run's input (after
    leak) is below the threshold and the level after is at or above it. Reset mode 0 returns to
    zero and clears the support; mode 1 keeps decaying.
23. **Thresholds compare with `>=`** throughout; `Novelty` compares `deviation > band`.
24. **Numerical guard.** Every activation, state scalar and message value is clamped to
    `[-1e30, 1e30]`, with NaN mapped to 0, so that one runaway cell cannot fill the medium with
    infinities or make the persisted bytes depend on NaN payloads. The only power is `pow_det`,
    repeated squaring in `f32` multiplications, with two values pinned to bits computed
    independently.

### The anchoring rule (section 5), as built

25. The anchor is the earliest reference in the emitter's support at the run that fires, where
    the support is the references carried by this run's inputs (after `max_refs`). The `refs`
    are the support filtered to the emitter's lookback, so **the anchor can lie outside `refs`**;
    that is the literal reading of section 5 ("anchor: the earliest ... among those that
    contributed"; "refs: all contributing events within its lookback").
26. **An accumulating cell's lookback bounds what it cites, not what it sums.** An integrator
    with leak 1 and lookback 1 still sums an event from three ticks ago into the level that
    crosses, but no longer cites it, so the anchor moves later
    (`an_accumulating_cell_prunes_its_support_to_its_lookback`). The level and the support have
    different memories. This is a real weakness of the first rule: the event that started an
    anomaly is the one most likely to be pruned. The alternative (no pruning) makes the anchor
    the first event since the last reset, however old, which is the rung's failure in a different
    form. M2 has to choose lookbacks with this in view.

### What this design cannot express, found while building it

- **Silence is invisible.** A cell runs only with input and sends only when its activation is
  not zero, so the end of activity reaches nobody: a latch that expires, or an integrator that
  leaks below its threshold, sends nothing. B1's `Noticer` seam yields retirements as well as
  notices; with this design, retiring a notice needs either a heartbeat event per tick from the
  clock or sense adapter (one routing and one update per tick, priced like any other) or a
  time-out in the effector adapter. Neither is in section 6. This is a consequence of the
  sparsity rule, not a bug, and M2 has to choose.
- **Values are `f32`.** A stream counter is a `u64` and a configuration hash does not fit at all;
  a message id is a `u64` and a `Tag` is a `u32`. The sense adapter must say what it maps to a
  value, what to a tag, and how it folds a `u64` id into a `u32` tag (and what collisions do).
- **A tick is at most about 4.29 s** for `offset_ns` to keep its resolution (`u32` nanoseconds);
  the sweep's longest tick is 2 s.

### Ports (section 6)

27. **The trace adapter keeps samples in memory** (`SamplingTrace`, capped, overflow counted).
    Section 6's first adapter is "file, sampled", but the crate does no I/O; writing the samples
    belongs to the harness.
28. **`Resource` is defined and never called.** No archetype issues a call in M1; the trait and
    a refusing adapter exist so that the port's shape is fixed.
29. **The ledger port receives counts and the truncation record**; the `CountingLedger` sums
    them. Putting the cost on a `gordian_core::Bill` (`OpCounts::compute_charge_ps`) is an M2
    adapter.
30. **`Plasticity` gets `&mut Medium`, which exposes only `set_weight` on synapses marked
    plastic.** It cannot add cells or synapses, so the structural limits cannot be bypassed.
31. **`Persist` is a fixed binary encoding** (`src/persist.rs` documents it), not serde, so the
    bytes do not depend on a library's formatting. Decoding validates everything and never
    panics (a property test and a byte-flip test).

### Tests (section 10)

32. **"Counts are monotone in events delivered" is tested in the form that is true**: from the
    same state, a superset of a tick's events never decreases that tick's routings or the cells
    run in its first pass; and cumulative counts never decrease along a run. The stronger form
    (more events, never fewer operations in the tick or the run) is false for this design, and
    should be, because inhibition exists: `whole_tick_counts_are_not_monotone_in_events` is a
    hand-worked counterexample (an inhibitory event silences an integrator and the latch behind
    it, 6 cell updates become 3).
33. **Dependencies.** Section 10 lists `gordian-core`, `serde` and `proptest`. The crate also uses
    `serde_json` (dev only, the spec's JSON round trip) and `criterion` (dev only, the benchmark
    M1 asks for). Neither adds anything to `Cargo.lock`.

## Rhythms (section 4b, added during M1): not absorbed; proposed as M1b

Section 4b (nested rhythms) arrived after this crate was built and its tests passed. M1 is
delivered as specified, and the rhythms are proposed as a bounded follow-up, M1b. Names for M1b,
as the user set them: the set of nested oscillations is the medium's **oscillome**; the base tick
is its fastest oscillation, and "tick" stays the word for one step of it; the component that
computes phases, applies phase gates and fires the schedules is the **Oscillome Engine**. In code:
module `oscillome`, types `Oscillome` (the spec of oscillations, periods in seconds) and
`OscillomeEngine`. The names label a mechanism; they claim nothing about what it does. Reasons for
deferring:

1. **Reviewability.** 4b changes the spec's units (seconds for delays, decays, lookbacks), the
   `Field` (phases), the `Gate` enum, one archetype's semantics (`Coincidence`), adds an archetype
   form (`Oscillator`), moves when the plasticity port runs, and changes the persisted encoding.
   Folding that into M1 would leave the chief verifying two designs against one acceptance
   criterion written for the first.
2. **4b has decisions to make first**, listed below; building them silently would make a design.
3. **Much of 4b is reachable from M1 through adapters**, so M2 is not blocked (next table).

| 4b item | Reachable with M1 as built? |
|---|---|
| Phases broadcast every tick | Partly: a field adapter can put window indicators (1 inside `[from, to)`, else 0) in field scalars, but `F` is 4 |
| `Gate::Phase { rhythm, from, to }` | Yes, as `Gate::Field(k)` on such an indicator; one field scalar per window |
| Plasticity at 10 s boundaries | Yes: the plasticity adapter receives every tick's summary and can act only at boundaries (but it sees one tick's summary, not the cycle's) |
| Trace sampling at 100 s boundaries | Yes: `SamplingTrace { every: 100 s / tick length }` |
| Seconds converted to ticks at build | In the builder caller, not in the spec |
| `Coincidence` binding by rhythm | No (M1 has a sliding window in ticks) |
| `Oscillator` (phase-reset latch) | No (a `Latch` holds; it does not cycle) |

**Decisions M1b has to make, with the problems found in 4b:**

- **The rhythm set and the tick sweep conflict.** 4b's fastest rhythm is the base tick (0.1 s),
  while section 8 sweeps the tick over 100 ms, 500 ms and 2 s. At a 2 s tick there is no 0.1 s
  rhythm: burst order (20–150 ms) lives only in `offset_ns`, which the medium uses for sorting and
  anchoring, never for dynamics. Either the fast rhythm moves with the sweep (and the claim
  becomes "the fastest rhythm is the tick"), or the sweep stops below the fastest rhythm.
- **"A change of base tick does not change the program" cannot hold exactly.** A delay of 0.15 s
  is 1 or 2 ticks at 100 ms (by rounding rule) and 0 ticks at 500 ms or 2 s, and 0 means "next
  pass", a different path through section 4's pass structure. M1b must state a rounding rule and
  report, per spec, which delays collapse at which tick length.
- **Decay in seconds needs a fractional power.** A per-tick leak from a time constant is
  `exp(-tick / tau)`, not a basic operation. It can be computed once at build time in `f64` with
  a pinned series (as `gordian-stream/src/rng.rs` does) and rounded to `f32`, which keeps the
  tick within the allowed operations. Pinned-bit tests follow.
- **Phase arithmetic.** A phase that is a pure function of the tick should be computed in integer
  nanoseconds, `(tick * tick_len_ns) mod period_ns`, then divided once in `f32`. When the period is
  not a multiple of the tick, boundaries fall unevenly (a 10 s rhythm on a 0.3 s tick wraps every
  33 or 34 ticks); M1b must either require whole multiples or define the boundary as the tick in
  which the cycle index changes.
- **Binning and windows behave differently.** "Share a fast cycle" bins time at cycle edges: two
  events 1 ms apart that straddle a boundary are not coincident, while two events almost a whole
  cycle apart are. M1's sliding window has no edges. The analogy favours bins and detection
  probably favours windows; both forms should exist so that the ablation can compare them.
- **Oscillator cost.** A phase-reset oscillator that wakes every tick costs one update per tick
  while it runs; one that schedules its own wake at the next phase of interest (a delayed
  self-message in the pending map) costs one per cycle. The second keeps 4b's "per-cycle work
  costs nothing per tick".
- **Plasticity at boundaries** needs a per-cycle summary (summed counts and activity since the
  last boundary), or plasticity sees only the boundary tick.

**Where the analogy breaks** (4b states the first two points; the rest were found here): the
rhythms are imposed clocks, a human prior; nothing in the world is periodic, so phase carries no
information about the world, only about when the medium does what; a global phase is unrelated to
an incident's onset (4b's reason for local oscillators); and with ticks of 500 ms or 2 s the
"fast" band is the tick itself, so the nesting that gives the analogy its content collapses
to two levels.

Proposed M1b acceptance: phases are a pure function of tick index and tick length (a property
test against an integer-arithmetic reference); identical bytes after a snapshot and restore at
a rhythm boundary and mid-cycle; each 4b element switchable off in the spec, so that "the medium
with rhythms removed" is the M1 tick with identical bytes (a test); the seconds-to-ticks
conversion table per tick length in its report; the benchmark rerun.

## Mutation checks

Eight mutations of the source, each run against the whole test suite. All are caught. Two were not
at first: removing the event sort and removing the input sort passed every test, because the
generated values were multiples of 0.5 (exact in any order) and a cell rarely got three or more
events in one tick (IEEE addition of two values is commutative). The generated values and the
event density were changed, and two hand-worked tests were added that make the order observable.

| Mutation | Caught by |
|---|---|
| no event sort | `events_are_summed_in_offset_order`, `events_in_a_different_order_within_a_tick_give_the_same_result` |
| no input sort | `messages_are_summed_in_synapse_id_order` |
| anchor = latest instead of earliest | `the_anchor_is_the_earliest...`, `refs_beyond_max_refs...` |
| wakes not persisted | `snapshot_restore_and_continue...`, the byte round trips |
| traversal counted twice | `operation_counts_equal_an_independent_recount_from_the_trace` and three others |
| limit off by one | `the_operation_limit_truncates...`, `no_panic_on_any_spec_within_the_limits` |
| `cells_not_run` not recorded | `the_operation_limit_truncates...` |
| no support pruning | `an_accumulating_cell_prunes_its_support_to_its_lookback` |

## Benchmark: measured cost per operation against the declared prices

`benches/tick.rs` (five steady-state workloads, its module documentation says what each does),
release profile (thin LTO, one codegen unit), run twice on 2026-10-06 through
`scripts/cgroup-run.sh --cpus 0-2 --cpu-quota 300 --memory 2G`, each time only after no `cargo`,
`rustc` or `gordian-run` process was left on the machine (the first run waited 450 s for another
lab's tests), and with none found when it ended. Criterion medians, 60 samples, 3 s measurement.
`bench_prices.py` turns the outputs into the tables below.

| run | cgroup | wall | CPU | peak memory | OOM kills |
|---|---|---|---|---|---|
| 1 | v1, cores 0-2, 300%, 2 GB | 66.2 s | 64.7 s | 48.5 MB | 0 |
| 2 | v1, cores 0-2, 300%, 2 GB | 70.4 s | 69.2 s | 48.1 MB | 0 |

**The headline: one tick at 10, 100 and 1,000 active cells** (`mixed`: per tick N/2 routings,
N cell updates, N traversals, N/2 field reads).

| active cells | median ns per tick, run 1 [95% CI] | run 2 | modelled at declared prices | measured / modelled | measured ns per operation (blended) | declared ns per operation (blended) |
|---|---|---|---|---|---|---|
| 10 | 2,126 [2,117, 2,135] | 2,363 [2,238, 2,540] | 310 | 6.9, 7.6 | 71, 79 | 10.3 |
| 100 | 24,746 [24,017, 25,119] | 25,145 [24,417, 25,529] | 3,100 | 8.0, 8.1 | 83, 84 | 10.3 |
| 1,000 | 273,795 [268,223, 290,197] | 279,079 [268,167, 290,510] | 31,000 | 8.8, 9.0 | 91, 93 | 10.3 |

**Per kind of operation**, by differencing the isolating workloads (run 1 / run 2, ns):

| n | routing | cell update | synapse traversal | field read |
|---|---|---|---|---|
| 10 | 45.0 / 46.6 | 163 / 147 | 16.7 / 20.4 | 1.4 / -1.0 |
| 100 | 41.1 / 43.9 | 234 / 216 | 17.9 / 31.9 | 0.6 / -13.5 |
| 1,000 | 38.7 / 37.5 | 276 / 258 | 27.9 / 40.3 | -1.7 / -11.9 |
| declared | 10 | 20 | 5 | 2 |

A least-squares fit over all 30 measurements (relative residuals) gives 202 ns per cell update,
22.8 per traversal, 42.3 per routing, -2.2 per field read and -23 per tick (the intercept is
negligible; without it the prices barely move); worst relative residual 0.27.

**Result against M1's acceptance.** The measured cost is not within a factor of 5 of the declared
prices: the headline workload costs 6.9 to 9.0 times the modelled cost, cell updates about 10
times their price, traversals 3 to 8 times, routings about 4 times. As the acceptance allows, the
proposal is **new prices: 200 ns per cell update, 25 ns per synapse traversal, 40 ns per event
routing, and 2 ns per field read kept** (field reads are not resolved by this benchmark: the
differences are within noise of zero, and negative because the control workload's cell gate reads
another cell's memory, which a field gate does not; that explanation is not tested). At those
prices every one of the 30 measurements is within 0.78 to 1.34 of its modelled cost, and the
headline within 0.86 to 1.13. `Prices::DECLARED` is left at section 7's values: changing the
declared prices is the chief's decision, not this unit's.

What these numbers are and are not:

- They price **this reference implementation**. Most of a cell update is probably allocation
  (each run takes its inbox and allocates its support; a firing cell allocates the shared `Arc`
  its messages carry; the inbox is reallocated every tick). That is a hypothesis: no profile was
  taken. An optimised tick, with this one kept as its oracle, would need its own calibration.
- The cost per cell update grows with the number of active cells (about 150 to 260 ns from 10 to
  1,000), so one price per update is a compromise; M2's sparse ticks will mostly sit at the lower
  end, where 200 ns overstates the cost a little.
- Criterion medians, not the per-block minimum that A8b's calibration uses; on this VM medians
  carry stolen time. The two runs agree within about 10% on the headline.
- The workloads have no `Novelty`, `Coincidence`, `Latch` or `Emit` cells, empty supports except
  single events, and no delayed messages beyond one tick. Archetypes that merge larger supports
  cost more per update than these.
