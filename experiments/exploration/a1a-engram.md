# A1a, the engram: memory in the medium (build phase) — Lab 1 report

Exploration. A1a has no criterion beyond its deliverables (`docs/lab-queue.md`, "## A1"); nothing
here tests a hypothesis, and the smoke counts may not later be cited as confirmation. Nothing was
tuned against any measure. I did **not** read `crates/gordian-stream/HIDDEN-DESIGN.md`. Before
designing the key I dumped the public observations of tuning seed 10000 (`examples/dump`, no
labels) and counted message ids, to see whether free-form ids were frequent background; that is
all I looked at.

## Commits (branch `engram`)

| Commit | What |
|---|---|
| `a82c38c` | the engram's design in `crates/gordian-medium/DESIGN.md`, before any code |
| `55be9db` | the mechanism in `gordian-medium` (bind, recall, decay, persistence, `grow`, `set_params`) and its tests |
| `715a700` | the adapter; key definition and confirmation policy in its module documentation, before any run |
| `f4cf4a4` | gate and smoke scripts read with the standard library (no pandas in this container) |
| `4baddc0` | R6 replay at `f4cf4a4`: 62 of 62 |
| `efcaa89` | **Amendment W2**, before any run of the arm (below) |
| `65ddb7f` | R6 replay at `efcaa89`: 62 of 62; the smoke manifest's `source_revision` |
| (this report) | smoke counts and the report |

## Verified by running something

- **Mechanism tests** (`crates/gordian-medium/tests/engram.rs`, 15): an engram recalls on its whole
  key at one node, anchored on the earliest feature, in the tick of the last feature; not when a
  feature is missing, late, or split across nodes; a fixed site recalls only at its node; strength
  grows to the cap, the same key in another order is the same key; a disagreeing answer weakens
  every engram whose key the pattern holds and binds its own; decay at each boundary of the decay
  rhythm; generalisation by intersection, and it never narrows away the last marked (late)
  feature; too-short keys are not bound; the hard limits refuse a bind and leave the medium
  unchanged; same inputs give the same bytes; persist, restore, continue is identical; a restart
  keeps engrams and forgets activity; every single-bit flip of the persisted pair is refused or
  re-encodes to itself, every truncation is refused; plasticity work is counted; `grow` keeps
  existing state and `set_params` refuses a parameter set by a quantity in time.
- **Adapter tests** (`crates/gordian-run/tests/stream_medium_engram.rs`, 15): the key's features
  and late marks; outcome tags for all ten diagnoses; bind from an answer and recall of the same
  pattern at another service with the site substituted, only after the late feature; site-keyed
  recall only at its own service; nothing without bind or without the layer; nothing bound for an
  anomaly no longer held, for a key with no late feature, or for a family answer naming another
  site; family engrams carry to the next segment, site-keyed ones do not unless `carry_site`;
  every k-th recall confirmed; contradicted or disputed recalls confirmed; the layer's ticks,
  routings and bind work charged exactly; manifest round trip and every parameter check. At the
  arm (`always_escalate` asking 5 s after notice): a recall is declared with source `recall`, the
  recalled anomaly is never escalated, the shared rule does not declare for it, while the control
  without the layer escalates it; a confirmed recall is asked about at the recall and not
  declared; and a rule asking at notice (delay 0) is **not** preempted, because the recall waits
  for late evidence.
- **Existing identity tests unchanged and passing**: `m1_identity.rs`, `m1b_identity.rs`,
  `subtick.rs`, and the pinned M2/M3/M4 digests in `stream_medium.rs`.
- **Gates** (under `scripts/cgroup-run.sh --name engram-build --cpus 0-2 --memory 3G`, on
  `efcaa89`): `cargo fmt --all -- --check` clean; `cargo clippy --workspace --all-targets -- -D
  warnings` clean; `cargo test --workspace` 981 passed, 0 failed; `scripts/check-no-oracle.sh` ok.
- **Byte identity**: R6's held-out manifest (b = 5, rho = 0.7) replayed through
  `scripts/run-driver.sh`, 62 of 62 arms with `results.csv` and `incidents.csv` identical to
  `r6-results-sha256.csv`, twice: at `f4cf4a4` (`a1a-regression.csv`) and on the final engram
  code at `efcaa89` (`a1a-regression-2.csv`). Directories kept:
  `artifacts/runs/a1a/a1a-xcheck-r6-heldout-b5-rho0.7`, `.../a1a-xcheck2-r6-heldout-b5-rho0.7`.
- **Smoke run** (below), `artifacts/runs/a1a/a1a-smoke-b5-rho0.7`, kept.

Resource record: no build or test ever found a `gordian-run` process running (no build waits).
The first identity replay waited 4 times (2 min) for another lab's `cargo`; the second and the
smoke none. Free disk was at least 6.4 GB at every check (lowest just after the first replay); I
deleted my own `target/debug/incremental` (1.4 GB) and built with `CARGO_INCREMENTAL=0` after it.

## What is built

**In `gordian-medium`** (`src/engram.rs`; `DESIGN.md`, "The engram", departures 58-65): a bind
operation on the plasticity port that builds an engram out of ordinary cells (shared presence
`Sense` cells per node and feature tag; one sliding `Coincidence` per node the key's site ranges
over; a `Latch`; an `Emit` behind a plastic synapse whose weight is the strength), strengthens an
exact match, weakens contradicted engrams, optionally generalises by intersection, decays at the
decay rhythm's boundaries, and persists as a fixed binary table beside the medium's bytes.
`Medium::grow` and `Medium::set_params` are the structural operations it needs, validated as one
spec against the hard limits. Prices are the calibrated ones; plasticity work is counted.

**In the medium arm** (`arms/medium/engram.rs`): an engram layer, a second medium beside the
noticing graph on the same tick, fed every abnormal observation and every message; answers bind;
recalls are resolved to noticed anomalies and handed to the arm.

### The key (as amended, fixed before any run of the arm)

Over the observations the noticer holds about the anomaly's site `s`, emitted in `[t0, t0 + 10 s]`
(`t0` its anchor): the **kind** of each abnormal observation, the **band** of each abnormal counter
(below 2·HIGH, below 4·HIGH, above), each **catalogue message id**, and, in a site-keyed key only,
each **free-form message id**. A feature is **late** if first seen more than 2 s after `t0`; a key
with no late feature is not bound; the recall needs every live feature of the key at one service
within 10 s, so it waits for the late evidence. Up to 8 features (the first late one kept if the
first eight hold none). Family-keyed: the site is a variable; site-keyed: fixed at `s`.

### The confirmation policy (fixed before any run)

`never`; `every k` (the k-th, 2k-th ... recall of the arm in stream order, counted across
segments); `on_contradiction` (the recalling engram has been contradicted, or another engram
recalled a different outcome for the anomaly at the same step). A confirmed recall is not
declared: the arm escalates at once and the answer, declared as the reasoner's, is bound.

### Strength (as amended)

A net vote: each agreeing answer adds 1, each disagreeing one subtracts 1, capped at 4, times
0.99 at each 100 s boundary; a recall needs 1.5 (two more agreeing than disagreeing answers).

## Amendment W2 (before any run of the arm)

The chief's four findings from W2 arrived after `715a700` and before any run of the arm (only the
R6 replay, which runs no engram, had run). How each was handled, in `efcaa89`:

1. **Ids recur only inside a stream.** Free-form message ids enter only site-keyed keys; a
   site-keyed layer starts empty at every segment; carrying it is the switch `carry_site`, a
   labelled control (`eng_site_carried`), never the default. A family-keyed answer whose diagnosed
   site differs from the anomaly's is not bound (a service number does not survive the stream).
2. **The family law is the main form, of invariant features only.** Family keys hold kinds,
   bands and catalogue message ids. Not built, and said so in the module documentation: features
   at another service (a cascade's partner, a split brain's second primary, the timing between a
   site's alarm and its partner's) need a coincidence across two services, which the engram's
   one-node key cells cannot express; probe answers do not cross the noticing seam.
3. **Phase-1 keys collide.** A key must hold a late feature (after 2 s, the rung's
   `burst_gap_ns` as the public reading of "first phase"); the recall waits for all of it within
   10 s (half the shortest public hard deadline); generalisation can never narrow an engram to a
   key without one of its late features (marked features in the crate's `Key`, departure 65).
4. **One answer is not a memory.** Strength is a net vote with threshold 1.5 (two net agreeing
   answers), and generalisation (on by default for the main form) is how binds of one pattern
   aggregate when background features make their keys differ.

The first values changed with it: key span 2 s → 10 s, onset 2 s (new), threshold 0.5 → 1.5,
decay 0.95 → 0.99 per 100 s, generalise off → on.

## Departures

From `docs/medium-ports.md`: the plasticity port gains structural power (section 6 and DESIGN
departure 30 limited it to weights), through the same validation as a spec; the engram is a
second medium held by the noticer rather than cells in the noticing graph (so the noticing graph is
byte for byte M3's, and only the engram layer carries across segments); bind runs between ticks
(an answer can arrive after the last tick of a segment); section 6's "outcome is privileged" does
not apply: the outcome is the reasoner's answer, which the arm paid for, never the truth. From
the DESIGN section written before the code: departures 58-65 in `DESIGN.md` (batch `set_params`,
four passes per tick, counts carried in the table, decode checks the wiring, refusals counted in
the table, variable sites range over services 0-11, dead features stay wired, marked features).

**Outside my territory, recorded:** two default methods on the `Noticer` seam (`answered`,
`recalls`) and `MemoryRecall` in `noticer.rs`; in `rung.rs` a call to `noticer.answered` in
`take_answer` and a `take_recalls` passthrough; in `arms/mod.rs` a `Source::Recall` (counted with
the cheap rung's in `results.csv`, so no column changes) and, in `StreamArm::step`, the recall
handling and a filter that keeps recalled anomalies out of the rule's targets. All inert for every
other noticer; the byte identity shows it. `rung.rs` and `arms/mod.rs` are Lab 2's files; the
brief allowed "the minimal hooks ... that wiring a new medium variant needs", and these are what
"a recall yields a `Declare` with no `Escalate`" needs; the chief should decide whether they stay.

## The smoke run

Seeds 10000-10019 in stream order, b = 5, rho = 0.7, the selection oracle at 16 s with the rung's
context (L1's conventions), M3's frozen 100 ms medium under every arm, one run of 8 arms, the first
values. Counts from `incidents.csv` only (`a1a-smoke.csv`): incidents with `correct_declarations`
≥ 1 and `escalations` = 0, by tier; beside it, as context from the same file, incidents escalated
and incidents with a wrong declaration and no escalation.

| Arm | Hard: correct, unasked | Hard: escalated | Hard: wrong, unasked | Plain: correct, unasked | Plain: wrong, unasked | Calls |
|---|---|---|---|---|---|---|
| `m3` (no layer) | 0 / 40 | 40 | 0 | 375 / 434 | 53 | 61 |
| `eng_off` (layer, bind off) | 0 | 40 | 0 | 375 | 53 | 61 |
| `eng_family` (main form) | **3** | 28 | 11 | 333 | 204 | 45 |
| `eng_family_exact` | 0 | 40 | 0 | 375 | 57 | 61 |
| `eng_site` (resets per stream) | 0 | 40 | 0 | 375 | 53 | 61 |
| `eng_site_carried` (control) | 0 | 40 | 0 | 370 | 61 | 61 |
| `eng_family_k4` | 3 | 34 | 4 | 316 | 127 | 100 |
| `eng_family_contra` | 3 | 27 | 12 | 332 | 186 | 55 |

**What the counts show (facts from the files):** recalls occur and declare without an escalation:
under the main form 12 hard incidents went unasked (40 → 28 escalated), 3 of them declared
correctly with no escalation (all three slow leaks), against 0 in both controls. The same arm's
memory declared wrongly on many plain incidents: plain incidents with a wrong unasked declaration
rose from 53 to 204, and 42 plain incidents lost the correct declaration the shared rule made in
the control (a recall preempts the rule). The newly wrong plain incidents per stream rise over the
20 streams (4-6 in the first three, 13-16 in several later ones). Exact family keys recalled almost
nothing (0 hard, 4 more wrong plain); within-stream site keys recalled nothing; the carried
site-keyed control only added wrong plain declarations, as W2 predicts. Confirming every fourth
recall cut the wrong plain incidents to 127 at 39 more calls. The layer's cost is on the bill
(`bill_compute` +1.31 ms per stream with bind off, +5.0 ms with the main form); it is not in
`total_cost_ns`, which for every medium arm since M2 omits the noticer's charge.

**Hypothesis (untested):** after generalisation the family engrams narrow to two or three
features, one early alarm kind and one "late" kind or band, and plain incidents that keep
alarming after 2 s carry the same pair. "Late" (after 2 s) is a public proxy for "rule-breaking"
evidence and on these counts a weak one: plain incidents produce later evidence too. The exact
keys are too specific to recur, the generalised ones too general to discriminate; these features
do not bridge the two. Under the selection oracle nothing corrects a recall on a plain incident
(plain anomalies are never asked about, so no contradiction arrives).

## What I am least sure of

1. That "evidence after 2 s" is a usable public stand-in for "the rule-breaking evidence"; the
   smoke suggests not. A public reading closer to the meaning is the rung's own consistency
   checker: evidence the public rules cannot explain (`AnomalyView::contradicted_since`). Gating
   recall on it lives in the arm, not the noticer, and I did not build it (scope).
2. That a recall should preempt the shared rule (`declare_recognized` blocks its later
   declaration). It is what "a recall yields a Declare" reads as, and it costs 42 correct plain
   declarations here.
3. Whether the hooks in Lab 2's files are acceptable as they are.
4. The coincidence of all features at one node within a window is all the engram can express;
   the hard families whose evidence is at two services (cascade, split brain) are out of reach by
   construction, and the smoke's correct recalls are all slow leaks.

## What A1b should measure that the brief does not name

- Recall precision by tier, and separately **plain incidents whose correct cheap declaration the
  recall displaced** (not only stale errors in general).
- **When** the recall fires against when the selector asks: a recall that waits for late evidence
  loses to a rule asking at notice (a test shows it), so the result depends on the selector's delay.
- What the arm can learn depends on what its selector asks: under the selection oracle every bind
  is a hard incident's answer and nothing is ever learned about plain incidents.
- Engram growth: engrams, cells and synapses per stream, the share of binds that strengthen, create
  or generalise, and the key size after generalisation; the layer's bill per stream as memory grows
  (every routed event feeds more key cells).
- Carry on against carry off for the family form, to separate across-stream memory from
  within-stream memory.
- `total_cost_ns` should include the noticer's charge, or A1b's cost column should be read from the
  bill.

## What the chief should examine most carefully

1. The smoke's plain-incident stale errors (204 against 53) and their growth with streams: by
   charter section 1.2, "stale errors grow with experience" counts against. A1b's criterion should
   bound them before anything else.
2. The amendment W2 was made after the first key definition was committed but before any arm ran;
   check that the order of commits shows it (`715a700` then `efcaa89`, smoke after).
3. The hooks in `rung.rs` and `arms/mod.rs`, and whether a recall's `Source::Recall` declaration
   should be counted with the cheap rung's in `results.csv`.
4. That the smoke ran the first values unchanged and nothing was adjusted after seeing it.

## Files

`crates/gordian-medium/src/engram.rs`, `crates/gordian-medium/src/medium.rs` (`grow`,
`set_params`), `crates/gordian-medium/DESIGN.md`, `crates/gordian-medium/tests/engram.rs`,
`crates/gordian-run/src/stream/arms/medium/engram.rs`, `.../medium/noticing.rs`,
`.../medium/graph.rs`, `crates/gordian-run/tests/stream_medium_engram.rs`,
`experiments/exploration/scripts/a1a_{common,manifests,gate,run,smoke}.py`,
`experiments/exploration/a1a-regression.csv`, `a1a-regression-2.csv`, `a1a-smoke.csv`.
