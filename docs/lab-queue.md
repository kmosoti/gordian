# Labs and the experiment queue

Three labs. Each has a principal investigator (PI), a worker agent that owns one queued unit at a
time in its own worktree and branch, and reports as AGENTS.md's "Coordinator and workers" says.
The chief researcher designs the units, fixes each criterion before the unit runs, queues units to
labs, verifies every report independently, merges what survives, and analyses results across
units and perspectives (`docs/review-log.md`).

## Lab assignments

| Lab | Focus | PI model | File territory (no other lab edits these) |
|---|---|---|---|
| Lab 1, Substrate | the medium and its adapters | opus | `crates/gordian-medium/`, later `crates/gordian-run/src/stream/arms/medium*.rs` |
| Lab 2, Instruments and baselines | the harness seam, evaluator measures, public baselines | sonnet | `crates/gordian-run/src/stream/`, `crates/gordian-stream-eval/`, `analysis/gordian_analysis/` |
| Lab 3, World and measures | world statistics, tick-scale questions, resource and sample-efficiency measures | sonnet | `crates/gordian-stream/examples/`, `experiments/exploration/scripts/`, analysis reports |

Shared rules: `docs/local-test-plan.md` section 2 (resources) and "Sharing cores 0-2 between
concurrent workers". Workspace `Cargo.toml` is edited only by Lab 1 (adding its crate) in this
round.

## Queue

Status is one of `queued`, `running`, `reported`, `merged`, `rejected`.

| Id | Lab | Title | Depends on | Status |
|---|---|---|---|---|
| M1 | 1 | The medium crate: types, tick, archetypes, ports, determinism | — | queued |
| B1 | 2 | The `Noticer` seam, notice measures in the evaluator, public noticing baselines | — | queued |
| W1 | 3 | Event statistics per tick length; sample-efficiency and energy-proxy measures | — | queued |
| M2 | 1 | The medium as a noticer on the stream world, against the public baselines | M1, B1 | queued |

## M1 The medium crate (Lab 1)

Build `crates/gordian-medium` as `docs/medium-ports.md` sections 3 to 7, 9 and 10 specify. No
world adapter, no noticer, no learning. The deliverable is the crate, its tests, a short
`crates/gordian-medium/DESIGN.md` that records every place the build departed from
`docs/medium-ports.md` and why, and a cost micro-benchmark (criterion, already a workspace
dev-dependency) of one tick at 10, 100 and 1,000 active cells, run under `scripts/cgroup-run.sh`,
reported as measured ns per operation beside the declared prices.

**Acceptance (fixed 2026-10-06).** All section-10 tests pass; the workspace gates pass; the crate
depends on nothing beyond `gordian-core`, `serde` and `proptest`; the measured ns per operation
in the micro-benchmark is within a factor of 5 of the declared prices, or the report proposes new
prices with the measurement. A departure from the design is allowed when the report states it;
a silent departure is a rejection.

## B1 The `Noticer` seam, notice measures, public baselines (Lab 2)

The shared rung bundles noticing, attaching, concluding and declaring. Separate noticing so that
an arm can swap it, and make noticing measurable without replaying ledgers (R10 had to).

1. **Seam.** A `Noticer` trait in `crates/gordian-run/src/stream/arms/`: given the public view
   (held observations with the public rules' verdicts, the public graph, the instant), it yields
   noticed anomalies (anchor, site, attached observations) and retirements. The rung's current
   noticing becomes `RungNoticer`, the default. Everything downstream (working state, components,
   the shared rule, declaring, context builders) is unchanged. **Gate:** R6's held-out run at
   b = 5, ρ = 0.7 replays byte-identical for all 62 arms against
   `experiments/exploration/r6-results-sha256.csv`.
2. **Measures.** The harness records every notice and retirement (anchor, site, instant, the
   noticer's id) in the run output, and the evaluator (`gordian-stream-eval`) scores per incident:
   noticed (any notice whose anchor belongs to the incident), notice latency from the incident's
   first observation, **anchor correctness** (the notice's anchor belongs to the incident and is
   within 1 s of its first observation), and per stream: notices on background, notices on plain
   incidents, notices per incident. Hand-written fixtures, mutation-checked as A2. The results
   schema gains these columns; `analysis/` loads them.
3. **Baselines**, each a `Noticer`, public information only:
   - `RungNoticer` at the default threshold and at z = 2 (R10's sweep).
   - `ChangeTriggered`: notices on the first abnormal observation at a node after a quiet period
     of `q` seconds, anchored there; the charter's change-triggered baseline, at the noticing
     level.
   - `EarliestAnchor`: the rung's noticer with its anchor moved to the earliest abnormal
     observation at the anomaly's site within `l` seconds before the rung's anchor. This is the
     cheapest public fix for the mis-anchoring R10 found.
   Each has one or two parameters tuned on R6's tuning streams (31000–31099 region, as the R6
   scripts do) and evaluated on the 200 held-out streams at b = 5, ρ = 0.7 with the selection
   oracle and the rung's context, so that quality rows are comparable with R10.

**Acceptance (fixed 2026-10-06).** The byte-identity gate; the evaluator fixtures with mutation
checks; a table of the four noticers on the held-out streams with: hard non-leak noticed share,
anchor-correct share, leak noticed share, notices on background per stream, hard-incident
quality with the selection oracle, cost per stream. No claim is made about which is better; the
table is the comparator for M2.

## W1 Event statistics per tick length; sample-efficiency and energy-proxy measures (Lab 3)

Two small studies, analysis-side, for the medium's design and the charter's aim.

1. **Tick statistics**, from a new feature-gated example in `crates/gordian-stream` (allowlisted
   like `dump`), over 200 streams at the defaults: for tick lengths 100 ms, 500 ms and 2 s, the
   distribution of events per tick (all, abnormal by the public rules, per node); for each hard
   family, how many ticks separate the incident's first observation from its first abnormal
   observation and from the partner's first alarm; what share of a hard incident's decisive
   evidence falls in the same tick as its first observation. Report as tables and a short
   reading: which tick length loses which structure.
2. **Measures for the aim.** From existing R10 and R9 outputs (`artifacts/runs/r10`,
   `artifacts/runs/r9`, read in place): for every arm, (a) **sample efficiency**: correct
   hard-incident decisions per hard incident seen, cumulative over stream order, as a curve; (b)
   **energy proxy**: modelled nanoseconds per correct decision, with a declared conversion of
   modelled ns to joules stated as an assumption (one figure for the cheap rung's operations, one
   for a reasoner call), so that every later experiment can report joules per correct decision
   beside cost. The conversion is a placeholder the charter revision will fix; the report says
   so. Add both as functions in `analysis/gordian_analysis/` with tests.

**Acceptance (fixed 2026-10-06).** The example and its test; the tables; the two analysis
functions with tests; a report `experiments/exploration/w1-tick-and-measures.md` that states what
is measured and what is assumed.

## M2 The medium as a noticer (Lab 1, after M1 and B1)

A hand-designed noticing graph on the medium, fed by the stream sense adapter, emitting notices
into B1's `Noticer` seam, compared with B1's public noticers on the same held-out streams with
the selection oracle and the rung's context.

**Criterion, fixed by the chief before any M2 code or run (2026-10-06).** Two separate results,
each named, at b = 5, ρ = 0.7, 200 held-out streams, paired 90% cluster bootstrap over streams,
at each tick length in {100 ms, 500 ms, 2 s}, with the best public noticer from B1's table as the
comparator for each result (the best by that result's own measure):

1. **Anchored noticing on hard incidents, slow leak excluded:** the medium's anchor-correct
   noticed share exceeds the best public noticer's by at least 0.03, with the lower bound above
   0.01, at no more notices on background per stream than that noticer.
2. **Noticing the slow leak:** the medium's leak noticed share exceeds the best public
   noticer's by at least 0.20, with the lower bound above 0.10, at no more notices on background
   per stream than that noticer.

A result that holds at one tick length and not another is reported as such; "holds" for the
experiment means at the best tick length, with the sweep shown. Modelled cost per stream for the
medium, including its own operations, is reported beside every row and bounded by the stream's
hard limits; it is not in the criterion, and a medium that costs more than the rung is still a
result. Feasibility: R10's sweep shows the rung at 0.917 noticed and 0.46 on the leak at 4.0
background notices per stream, and the noticing oracle at 1.0; anchor-correct shares are
unknown until B1 and may be lower. Every outcome is reachable.

The medium's graph is designed on the tuning streams and frozen (commit named) before any
held-out run. It uses public information only. The PI records every structural choice and what
it was for.
