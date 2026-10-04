# gordian-world: design record

Work item A1 of `docs/local-test-plan.md`. This file records the rules of the world, how each
episode class is built, every place the build departs from or fills a gap in the plan, and the
known weaknesses. The rules themselves are in code in `src/physics.rs`, whose module
documentation is the authoritative statement; this file explains choices.

## 1. Public rules, hidden instance

The *rules* are public: fault kinds, the symptoms each produces at its site and at its
dependents, what each probe reveals, each action's cost. They live in `physics` and are what a
component may encode. The *instance* is hidden: which fault, where, the two parity bits, the
drifted config hash, and which stream entries are signal.

Hidden state is held in private fields of `Episode`. The only accessor is
`oracle::reveal(&Episode) -> HiddenState`, compiled under feature `reveal-hidden-state` or
`cfg(test)`. Because cargo unifies features across a build the feature is a guard, not a proof;
`scripts/check-no-oracle.sh` is the second guard (see section 6).

A policy holds a `Simulator`, never an `Episode`. `Simulator` has no accessor for the spec, the
class, the faults, or whether a declaration was right.

### Fault kinds and symptoms

Five kinds: `ResourceExhausted`, `ConfigDrift`, `DependencyDown`, `CredentialExpired`,
`Intermittent`. Symptoms by role (site, or a transitive dependent of the site) are the table in
`physics`. Two structural rules matter for everything below:

1. **Rules permit, they do not require.** A hypothesis is contradicted by an observation it does
   not permit, never by an observation that has not arrived. The one exception is propagation:
   an abnormal observation at a *dependent* is permitted only if an `ErrorRate` counter at or
   above 50 at the site appears earlier in the evidence. This is what pins the site, and it is
   why the checker can return the empty set (see 5.4).
2. **No fault, no symptoms.** `None` permits no abnormal observation.

A consequence worth stating: a `NoFault` stream cannot be *positively* identified from the
stream, because every fault hypothesis is also consistent with silence. Only probes exclude
faults (a `HealthCheck` on every service does, and fits the default budget for 12 services).
This is tested.

### Probes and costs

| probe | reveals | cost (probes, time) |
|---|---|---|
| `HealthCheck` | anything wrong at target (decoy: inconclusive, suggests itself) | 1, 1 ms |
| `ResourceUsage` | `ResourceExhausted` at target | 1, 2 ms |
| `ConfigSnapshot` | config hash now (changed iff `ConfigDrift`) | 1, 3 ms |
| `CredentialCheck` | `CredentialExpired` at target | 1, 2 ms |
| `LatencySample` | bit 1, if fault is `DependencyDown` or `Intermittent` | 2, 8 ms |
| `ErrorSample` | bit 2, same restriction | 2, 8 ms |
| `Correct` (action) | whether a fault was at the site | 3, 50 ms |

Declare and Abstain are free. Default budget: 12 probes, 250 ms probe time. Probes measure the
underlying cause, which exists from time zero, so they are valid at any instant; `onset` is only
when passive symptoms start.

`DependencyDown` and `Intermittent` have no dedicated probe. This is deliberate and
forced by `JointlyDecisive` (section 2): a dedicated probe for either kind would be individually
decisive.

## 2. Episode classes: how each guarantee is built

All eleven classes are implemented. Every guarantee below is checked by a unit test that also
asserts the interesting branches were exercised (so it cannot hold vacuously).

| class | construction | checked by |
|---|---|---|
| `Ambiguous` | only the shared first symptom (`ErrorRate` at the site) is emitted. All five kinds fit at the true site. Truth is drawn from the three kinds with a dedicated probe. Exactly one probe kind at the site resolves | `classes::ambiguous_*`, `props::headroom_needs_exactly_*` |
| `DelayedConfigChange` | truth `ConfigDrift`. A changed `Snapshot` at the site, then at least `delay_k` irrelevant observations strictly between it and the first symptom, then the shared first symptom. Snapshot present: exactly one hypothesis. Snapshot removed: five kinds, and a `ConfigSnapshot` probe resolves | `classes::delayed_*`, for k in 0,1,5,20,200 |
| `NoiseFlood` | full signature of a random kind, plus noise. Noise count is raised until at least 80% of observations are free-form messages (`text_id >= 65536`, random 64-bit) | `classes::noise_flood_*` |
| `JointlyDecisive` | truth `DependencyDown` or `Intermittent`. Stream carries `MixedSignals` and symptoms both kinds permit, so the stream leaves exactly those two. Two hidden bits obey the parity rule (equal means `DependencyDown`). Each sample probe reveals one bit | `classes::jointly_*`: world counts per kind go (2,2) to (1,1) after one sample, to one hypothesis after two; no other single probe decides |
| `NoFault` | no fault; noise and benign counters (some just under the alarm threshold) | `classes::no_fault_*` |
| `CriticalFault` | critical fault; half the time the plain ambiguous signal, half the full signature | `classes::critical_*` |
| `QuietUrgent` | flood noise, and the only true observation is one `Low` severity catalogue message that fixes kind and site. Critical | `classes::quiet_urgent_*` |
| `Duplicates` | each true observation emitted 2 to 5 times, identical content, later instants | `classes::duplicates_*` |
| `FeedbackBait` | plain ambiguous signal, plus a decoy service (never the site) with `unreliable_health` and `CheckHealth` messages. A health check on the decoy returns `Inconclusive` suggesting the same check, forever; only the budget stops the loop | `classes::feedback_bait_*` |
| `StaleMemory` | plain ambiguous signal. `PublicInfo.prior_records` has a record whose signature equals the current symptoms and whose resolution is wrong | `classes::stale_memory_*` |
| `ComponentTimeout` | full signature, plus 1 to 3 harness directives (`Fail` or `Slow { factor >= 2 }`) on distinct component indices 0..8, via `Episode::harness_directives()` | `classes::component_timeout_*` |

**Critical marking.** `critical = true` for `CriticalFault` and `QuietUrgent`; false for every
other class (checked for all classes). `NoFault` has no fault to mark.

**XOR and "uniform prior".** The claim "a single probe leaves the posterior over kinds
unchanged" is checked as: with a uniform prior over consistent *worlds* (hypothesis plus bits),
the per-kind world counts are equal before and after one sample. It is *not* true of a policy
that scores probes by information gain over worlds: such a policy sees positive gain from one
sample. The class defeats one-step greedy selection on the *kind* posterior only.

## 3. Deviations and gap-fills

Each is a place where the plan's sketch was underspecified or where a constraint forced a
choice.

1. **`Budget` is not in `EpisodeSpec`; `BudgetSpec` is.** `gordian_core::Budget` is not
   serializable and core is not this unit's to change. `BudgetSpec { probes, time_ns }` is
   serializable and builds the `Budget`. Only `Probes` and `Time` are charged. *If core adds
   serde support, this can revert.* Same reason for `CostSummary` in `Outcome` instead of
   `Vec<Charge>`: `Charge` is not serializable. Declared costs are still `Vec<Charge>`
   (`physics::probe_cost`, `physics::correct_cost`, `Action::cost`).
2. **`Instant` is serialized as plain `u64` nanoseconds** through private `serde(with)` adapters,
   again because core's `Instant` has no serde impls.
3. **`World` holds services only.** The plan's `World { services, edges, hidden }`: edges are
   derived (`World::edges`), hidden state is not in `World`.
4. **`Episode` has private fields and accessors** (`spec`, `world`, `stream`,
   `harness_directives`, `public_info`), per the hidden-state constraint. `Episode` is
   `Serialize`/`Deserialize` (so evaluator fixtures can build one from JSON), which means
   serializing an `Episode` serializes hidden state. Policies never hold one.
5. **`Observation` has two variants beyond the plan's three**: `Probed { probe, result }` and
   `Correction { site, resolved }`. The checker's signature takes `(Instant, Observation)`
   evidence, so probe results must be observations; they are delivered in `Outcome`, not in
   `observe_until`.
6. **`Probe` is a struct** `{ kind, target }` as in the plan; `Action::Probe` is struct-like with
   the same fields.
7. **`Outcome` carries `ready_at`** (`now` plus the probe's time cost). The simulator does not own
   a clock; the driver passes `now`. Times must not go backwards, and `now` after the horizon is
   refused (`PastHorizon`).
8. **`EpisodeSpec` extras:** `min_services`, `max_services` (the "4 to 12" is a per-episode
   draw), `delay_k` (default 5). `generate` stores the *normalized* spec (clamps services to
   4..=12, horizon to at least 2 s, `noise_rate` to at most 50, `delay_k` to at most 200).
   `generate(s) == generate(s.normalized())` is tested.
9. **`HiddenState` is not `pub(crate)`** as the plan says: `oracle::reveal` must return a type
   its callers can name, so it is `pub` but exists only under the feature.
10. **`Correct` has an observable effect** (`Correction { resolved }`), because the plan names
    Correct as an action with a cost but gives it no semantics. The passive stream is
    pre-generated and does not react to a correction. This is a limitation, not a model of
    repair.
11. **Seed expansion.** The ChaCha8 seed is expanded from `spec.seed` with a local splitmix64,
    not `SeedableRng::seed_from_u64`, so that no library's expansion is a hidden input. ChaCha8
    output is fixed by algorithm.
12. **Tests live in `src/tests/`, not `tests/`.** `oracle` is compiled under `cfg(test)`, so the
    tests can read hidden truth without a self-referencing dev-dependency. Integration tests
    under `tests/` would not see it. `cargo test -p gordian-world` and
    `cargo test -p gordian-world --all-features` both run the full set.
13. **`noise_rate`** is irrelevant observations per true observation, with at least 8
    true-observation equivalents assumed so that an empty signal still gets noise. Flood classes
    raise it to meet 80%.
14. **Module named `builder`**, not `generate`, so it does not shadow the `generate` function.

## 4. Where the checker is exact, and where it is not

`physics::consistent_hypotheses` is the reference semantics of the public rules: the set of
hypotheses (including `None`) that no observation contradicts. `consistent_worlds` is the same
with hidden bits kept, and is what the joint-decisiveness test counts. The simulator and the
checker share one definition of probe semantics (`physics::probe_result`): the simulator calls
it with the hidden truth, the checker with each candidate. This makes soundness for probe
results true by construction; the soundness tests therefore chiefly constrain the *generator's*
passive stream, and `ConfigDrift`'s single drifted value.

Prior records are advisory and play no part in the checker (they can be stale).

## 5. Known weaknesses and artifacts

These are properties of the construction that a researcher should know about before reading a
result off this world. None is hidden by a test.

1. **Truth priors are not uniform within an ambiguity set.** `Ambiguous`, `FeedbackBait`,
   `StaleMemory`, `DelayedConfigChange` and the plain half of `CriticalFault` draw truth from
   `ResourceExhausted`, `ConfigDrift`, `CredentialExpired` only, although the stream fits all
   five kinds. Reason: the plan requires *exactly one* probe kind to resolve `Ambiguous`, and
   `DependencyDown`/`Intermittent` have no dedicated probe (forced by `JointlyDecisive`). A
   policy that learns the prior never spends probes on the other two kinds, so the effective
   ambiguity in those classes is three kinds, not five. An earlier draft had a second ambiguous
   profile (latency in addition) whose truth was always `ResourceExhausted`; that made the class
   trivially decidable by a learned policy and was removed.
2. **Catalogue ids make noise separable by anyone who reads the rules.** Noise is free-form
   (`text_id >= 65536`); signals are catalogue ids. A rule-aware component filters noise for
   free. `NoiseFlood` and `QuietUrgent` therefore measure whether *surprise- or severity-driven*
   attention is captured by noise, not whether a rule-aware component is. Severity is uniform
   over noise so it does not mark the signal; in `QuietUrgent` the signal is `Low`, and so is a
   quarter of the noise.
3. **`StaleMemory` leaks weakly by definition.** The matching record is wrong, so it says
   "truth is not this kind". Outside `StaleMemory` the same record is drawn uniformly, so it is
   right with probability about 1/5 (tested). A policy that knows the class distribution can
   extract a little; that is what a stale record is.
4. **The checker can return the empty set.** Empty means the evidence is contradictory, which
   for a component with bounded memory can mean it *dropped the anchor* (the early `ErrorRate`
   at the site), not that the world is inconsistent. A5's verifier should treat empty as "my
   evidence is damaged", not "no fault".
5. **`FeedbackBait` is identifiable as a class** (a service with `unreliable_health` is public
   graph data) and always names a decoy that is not the site.
6. **Headroom is claimed for the default budget only** (12 probes, 250 ms), and for the
   probe-only action set. A spec with a smaller budget can have none; `budget.probes = 0`
   obviously does.
7. **A correction does not change the stream.** See deviation 10.
8. **Variable draw count.** Unbiased rejection sampling consumes a variable number of
   random words with probability about n/2^64. It is deterministic given the seed and spec; it
   does not threaten equality of repeated generation.

## 6. Adversarial pass (what was tried before commit)

Recorded in the commit message and the report; summary here.

- *True hypothesis missing from the consistent set.* Soundness tested for every prefix of every
  class's stream over random specs; after every probe kind at the site (both samples first) for
  all classes and 60 seeds; after random mixes of probes and corrections; and with the proptest
  budget overridden to the default so that probes are not refused. **Mutation check:** making the
  generator emit equal bits for `Intermittent` (breaking the parity rule) was caught by the
  deterministic soundness test and by the joint-decisive and headroom tests, but *not* by the
  random probe test on that run, which is why the deterministic one exists.
- *Guarantee holding vacuously.* Every class test asserts the branch coverage it depends on;
  `headroom_needs_exactly_the_probes_each_class_is_built_to_need` pins the minimum probe count
  per class (0 for decisive streams, 1 for ambiguous, 2 for jointly decisive), so a world that
  made everything decidable without probes would fail.
- *Inference from things that should be uninformative.* The strongest check regenerates the same
  seed with a different true kind from the same ambiguity set. The draws are unconditional and
  in fixed order, so the public stream, world, public info and harness directives must be
  byte-equal. **This found a real leak**: periodic snapshots were suppressed for the drifted
  site only when the truth was `ConfigDrift`, so their absence revealed the kind. Fixed by
  removing suppression everywhere except `DelayedConfigChange`, where the site is public anyway;
  a further test checks snapshots are not statistically avoided at the fault site. Also checked:
  probe cost, `ready_at` and refusal pattern are independent of truth; hidden bits are balanced
  within each kind; the volatile prior record is independent of truth outside `StaleMemory`.
- *Not checked.* Statistical dependence of stream shape on kind within `Identified` classes
  (kind is public there by design); any timing side channel in the Rust implementation itself
  (the simulator is not constant-time and is not claimed to be).

## 7. Scripts

`scripts/check-no-oracle.sh` fails if `oracle::` or `reveal(` appears in a tracked or untracked
(non-ignored) `.rs` file outside `crates/gordian-world/`, `crates/gordian-eval/`, and
`crates/gordian-run/src/policy/oracle.rs`. It passes now, and was verified to fail on a planted
violation. It is a text check: a macro assembling the path from pieces would evade it.
