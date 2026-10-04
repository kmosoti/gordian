# Coordinator review log

What the coordinator checked for each merged unit, what it decided, and what it carried forward
to later units. Newest first. Reports from workers are model output; this log records what was
independently verified.

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
