# A1c, the recall gate and the two-site key — Lab 1 report

Exploration. A1c's acceptance (`docs/lab-queue.md`, "## A1c", fixed by the chief before the unit)
is deliverables, identity, the displacement test and the smoke table; it makes no claim either way,
and nothing here may be cited as confirmation. Nothing was tuned: every engram parameter is A1a's
first value, the A1c switches were fixed in the code's documentation before any run, and nothing
was changed after the smoke. I did **not** read `crates/gordian-stream/HIDDEN-DESIGN.md`. The
evaluator's `tier` and `family` columns were read only to report, after the run.

**The first number: the gated family form's plain unasked-wrong count is 180 in 20 streams,
against the control's 53.** That exceeds the control by 127, far more than 5. Under the acceptance
this is reported as such, and A1b's design is to be reconsidered before its criterion is fixed. My
prediction, committed before any run (`a1c_common.py`, `ceb4a99`), was that the count would exceed
58 (probability about 0.6). The gate removes part of A1a's excess, not most of it.

## Commits (branch `recall-gate`)

| Commit | What |
|---|---|
| `33a31e8` | the two-site key and the gate, designed in `crates/gordian-medium/DESIGN.md` before any code |
| `43e0648` | the build: pair keys in the crate; the gate (its public reading in `arms/medium/gate.rs`), the late switch and the two-site key in the arm; hooks; tests; departures 66 to 70 |
| `ceb4a99` | scripts (`a1c_*.py`) and the PI's prediction, before any run |
| `b25468c` | identity: R6's held-out replay, 62 of 62 (`a1c-regression.csv`); the smoke manifest names this revision |
| (this report) | smoke counts (`a1c-smoke*.csv`) and this report |

## Verified by running something

- **Identity.** I replayed R6's held-out manifest (b = 5, rho = 0.7) through `scripts/run-driver.sh`
  with the release binary built at `ceb4a99`. All 62 arms have `results.csv` and `incidents.csv`
  byte-identical to `experiments/exploration/r6-results-sha256.csv` (`a1c_gate.py check`, exit 0;
  `a1c-regression.csv`). The directory is kept at
  `artifacts/runs/a1c/a1c-xcheck-r6-heldout-b5-rho0.7`.
- **A1a's eight arms reproduce A1a's kept smoke exactly.** I put A1a's eight arm definitions into
  my smoke unchanged (`a1a_common.arms()`, imported). Their `incidents.csv` and `results.csv` are
  identical row for row to A1a's kept run once the `run_id` column is removed
  (`a1c-smoke-a1a-reproduction.csv`, 8 of 8). So every A1c change is inert for an ungated layer,
  on the arm A1c is compared against and not only on R6.
- **Monitoring alone changes no declaration.** `eng_off_gated` has bind off and the gate on, so the
  rung keeps the checker's verdicts but nothing is ever recalled. Against `eng_off`, its
  `incidents.csv` differs only in `first_correct_at_ns` and `time_to_first_correct_ns`, by a few
  microseconds: the checks' modelled compute advances logical time. Every count is equal. Its
  `results.csv` differs only in cost columns.
- **Existing identity tests pass unchanged:** `m1_identity.rs`, `m1b_identity.rs`, `subtick.rs`,
  the pinned M2, M3 and M4 digests in `stream_medium.rs`, and A1a's 15 crate and 15 adapter engram
  tests, all unedited.
- **New tests.** `crates/gordian-medium/tests/engram_pair.rs` has 7 tests: edge nodes; a key cell
  per ordered pair wired by role; a recall needs the site, the partner and their edge, and names
  the pair; no recall without the relation, with another pair's relation, with the roles swapped,
  or with both features at one node; comparison by role; generalisation keeps the span; persistence
  and single-byte corruption. `crates/gordian-run/tests/stream_medium_gate.rs` has 10 tests,
  including the **displacement test** below, the gate's wait, the late switch's defaults, A1a's
  configuration text read and written as before, and the two-site key from relation events to a
  recall at the pair's first node (and none without the partner, or with another gap band).
- **Gates**, run under `scripts/cgroup-run.sh --name recall-gate-build --cpus 0-2 --memory 3G` on
  `43e0648`, whose code is unchanged since: `cargo fmt --all -- --check` clean; `cargo clippy
  --workspace --all-targets -- -D warnings` clean; `cargo test --workspace` 1001 passed, 0 failed;
  `scripts/check-no-oracle.sh` ok.
- **The smoke** ran once: `artifacts/runs/a1c/a1c-smoke-b5-rho0.7`, 15 arms, exit 0, 24 s.

**Resource record.** No build or test ever found a `gordian-run` process running, so no build
waited. The identity replay waited 6 times (3 min) for another lab's `cargo` and `cargo-mutants`;
the smoke waited none. Free disk was at least 7.4 GB at every check. I built with
`CARGO_INCREMENTAL=0`.

## The gate's exact public reading (`crates/gordian-run/src/stream/arms/medium/gate.rs`)

A recall is acted on only for an anomaly whose `AnomalyView::contradicted_since` is set. That is,
the latest check of the first world's consistency verifier found no hypothesis consistent with the
evidence attached to the anomaly (its abnormal observations and the probes bought for it). The
verifier reports this as an empty candidate set or damaged evidence (`rung::verdict_is_empty`).
Nothing else enters the gate.

- **Who keeps the verdict.** The rung keeps it for an arm whose noticer asks (`needs_verdicts`), in
  the same way it does for `contradiction_escalation` (R5). It checks a noticed anomaly that has
  received evidence since its last check and has had no escalation, at most once per `review_ns`
  (500 ms), and it also records the verdict of every review of the shared rule. The checks go
  through the meter, so they are charged.
- **When the verdict is read.** At each step, after that step's checks: I moved the checks before
  the recalls in `StreamArm::step`. A recall whose anomaly is not contradicted waits for at most
  one review period of the rung and is read again at each step. After that it is dropped. A recall
  is also dropped if its anomaly is escalated, answered or retired while it waits.
- **What a dropped recall leaves.** Nothing. The anomaly is not marked recalled, so a later recall
  of it is gated afresh.
- **The late-feature switch.** When absent from a manifest, it is off for a gated layer (the
  brief's default) and on for an ungated one, which keeps A1a's form so that A1a's manifests mean
  what they meant.

**The displacement test** is
`a_gated_recall_displaces_no_declaration_where_the_public_checker_explains_the_anomaly`. An engram
is bound to a hard diagnosis on a pattern. The same pattern then recurs at another service, with
evidence that the checker explains (`ResourceExhausted` at the site). The ungated memory recalls,
adds a wrong declaration and keeps the anomaly from being asked about. The gated arm's actions, at
every step, are **equal** to those of the arm without memory: the cheap rung's correct declaration
and the escalation. The companion test shows that where the checker cannot explain the evidence
(`OutOfResource` together with `AuthFailures`), the gated recall is declared at the same step as
the ungated one.

**The guarantee is narrower than the brief's sentence, and the smoke shows by how much.** "Fires
only where the cheap rung would have returned no hypothesis" holds for the cheap rung's *verifier*
at the recall's instant. It does not hold for the cheap rung's declarations, for three reasons:

1. The shared rule falls back to the estimator's and then the heuristic's candidates when the
   verifier's set is empty (`policy/decide.rs`), so it still declares there.
2. Under A1a's rule, which is unchanged, a recall made after the cheap rung has declared adds a
   second declaration.
3. A verdict can turn empty after a correct declaration, once later evidence arrives.

## The two-site key (adapter module documentation, "A1c"; crate design, `DESIGN.md`)

**In the crate.** Features have roles: `Site`, `Partner` and `Relation`. A pair key
(`KeySite::Pair`) has one sliding coincidence per ordered pair `(a, b)` of distinct nodes, which
makes 132 for twelve services. Its inputs are the site features at `a`, the partner features at
`b`, and the relation feature at the edge node `pair_node(a, b) = 0x8000 | a << 7 | b`. **The
coincidence fires only when every live feature has occurred at both nodes and at their edge within
the window.** `recall_in` names the key cell that fired, and so the pair. Generalisation never
narrows a pair engram to a key without its relation feature or without a partner feature.

**In the adapter, the two-site key works as follows:**

- **First alarms.** A *first alarm* is an abnormal observation that begins a burst at its service
  (the rung's 2 s burst-gap rule).
- **Unconnected services.** Two services are *unconnected* when neither is a transitive dependent
  of the other in the time-zero public graph.
- **Relation events.** At every first alarm, the layer senses relation events on both edges with
  every unconnected service that had a first alarm in the last 10 s. Each is tagged with:
  - **order:** whether the partner's alarm came strictly first;
  - **gap band:** under `burst_ns` (0.4 s), under `burst_gap_ns` (2 s), or up to 10 s.
- **The partner.** For an answer about an anomaly at `A`, the partner `B` is the unconnected
  service whose first alarm is nearest to `A`'s, within 10 s.
- **The key** is the relation tag, then `B`'s and `A`'s invariant family features (kinds, counter
  bands, catalogue ids) over the 10 s from the earlier of the two first alarms. It holds at most 8
  features, always keeping the relation and one partner feature.
- **What never enters the key:** no service id and no free-form id.
- **No partner.** An anomaly with no unconnected partner keeps A1a's one-site key.
- **Where a pair recall lands.** It is declared at the pair's first node.

## The smoke against A1a's table

The smoke used A1a's manifest: seeds 10000–10019 in stream order, b = 5, rho = 0.7, the selection
oracle at 16 s with the rung's context, M3's frozen 100 ms medium, run seed 15000. Counts are from
`incidents.csv` (A1a's measure, imported). "Unasked" means `escalations` = 0. "Plain displaced" and
"plain newly wrong" are paired against `m3` on the same incidents (`a1c-smoke-paired.csv`).
`bill_compute` is per stream, in ms of modelled compute. A1a's rows are reproduced exactly.

| Arm | Hard: correct, unasked | Hard: escalated | Plain: wrong, unasked | Plain: correct, unasked | Plain displaced | Plain newly wrong (also correct) | Calls | Checks run | bill_compute ms/stream |
|---|---|---|---|---|---|---|---|---|---|
| `m3` (no layer) | 0 / 40 | 40 | 53 | 375 / 434 | 0 | 0 | 61 | 2,943 | 34.7 |
| `eng_off` | 0 | 40 | 53 | 375 | 0 | 0 | 61 | 2,943 | 36.0 |
| `eng_family` (A1a main form) | 3 | 28 | 204 | 333 | 42 | 151 (111) | 45 | 2,685 | 39.7 |
| `eng_family_exact` | 0 | 40 | 57 | 375 | 0 | 4 (4) | 61 | 2,943 | 41.7 |
| `eng_site` | 0 | 40 | 53 | 375 | 0 | 0 | 61 | 2,943 | 36.1 |
| `eng_site_carried` (control) | 0 | 40 | 61 | 370 | 5 | 8 (3) | 61 | 2,925 | 37.3 |
| `eng_family_k4` | 3 | 34 | 127 | 316 | 18 | 79 (63) | 100 | 2,754 | 40.9 |
| `eng_family_contra` | 3 | 27 | 186 | 332 | 33 | 133 (102) | 55 | 2,709 | 39.8 |
| `eng_off_gated` (monitor only) | 0 | 40 | 53 | 375 | 0 | 0 | 61 | 22,294 | 38.1 |
| **`eng_family_gated`** (late off) | **1** | 36 | **180** | 366 | 9 | 127 (118) | 51 | 22,431 | 41.6 |
| `eng_family_gated_2site` (late off) | 0 | 38 | 130 | 367 | 8 | 77 (68) | 57 | 22,306 | 130.2 |
| `eng_site_gated` (late off) | 0 | 40 | 53 | 375 | 0 | 0 | 61 | 22,294 | 38.2 |
| `eng_family_gated_late` | 3 | 35 | 161 | 367 | 8 | 108 (100) | 52 | 22,456 | 41.9 |
| `eng_family_gated_2site_late` | 0 | 38 | 112 | 369 | 6 | 59 (52) | 55 | 22,322 | 137.3 |
| `eng_site_gated_late` | 0 | 40 | 53 | 375 | 0 | 0 | 61 | 22,294 | 38.2 |

"Checks run" is the `components_run` column, summed over the 20 streams. Decoy rows are in
`a1c-smoke.csv`; every gated form has 42 or 43 decoys with a wrong declaration and no escalation,
against the control's 42.

**The hard unasked-correct count against A1a's 3:**

- **Gated family, late off:** 1, a slow leak.
- **Gated family, late on:** 3, two slow leaks and one compound incident.
- **Both two-site forms:** 0 in 20 streams.

**What the counts show, as facts from the files:**

- **The gate cuts displacement, not error.** Against A1a's main form, which has the same late
  setting, the gated form `eng_family_gated_late` changes three counts:
  - correct cheap declarations displaced: 42 → 8;
  - newly wrong plain incidents: 151 → 108;
  - the main gated form, with late off, has 127 newly wrong plain incidents.
- **Of the gated form's 127 newly wrong plain incidents, 118 also carry a correct declaration.**
  The recall did not replace the cheap rung's answer; it added a wrong one beside it.
- **Turning the late requirement off adds errors.** With the gate on, late off gives 180 plain
  wrong where late on gives 161: dropping it lets more first-phase engrams form.
- **The two-site key makes fewer plain errors.** It has 130 (late off) and 112 (late on) plain
  wrong, against 180 and 161 for the one-site gated forms. It makes no correct hard recall, and it
  escalates 38 hard incidents.
- **The site-keyed gated forms recall nothing**, as A1a's `eng_site` did. Without a gate,
  `eng_site` recalls nothing either (W2: almost no within-stream recurrence).
- **Per-stream newly wrong plain incidents under the main gated form do not keep rising.** These
  are the paired counts against `m3`. They are 0 in the first two streams and then fluctuate
  between 1 and 18, with 63 in the first ten streams and 64 in the last ten. A1a's main form shows
  the same shape: 74 and 77. Absolute per-stream counts are in `a1c-smoke-streams.csv`. Twenty
  streams cannot separate growth from noise.
- **Cost.** The gate's checks multiply component runs by about 7.6 (2,943 → 22,294) and add about
  2.1 ms of modelled compute per stream (`eng_off_gated` against `eng_off`). The two-site layer
  adds about 89 ms per stream on top of the one-site gated layer. A reasoner call with a
  128-reference context is about 0.74 s, so the two-site layer costs roughly an eighth of one call
  per stream. Gated forms save 9 to 10 calls in 20 streams (61 → 51 or 52).

## Analysis: what the result means and what it does not

**Best current model (inference).** On this world, under the selection oracle, the public checker's
verdict at the recall's instant does not separate plain incidents from hard ones much better than
"evidence after 2 s" did. Both readings mostly say the same thing: more evidence kept arriving at
the site. R5 had already measured that the checker contradicts 97% of plain anomalies at some
point.

The 118-of-127 count points to a mechanism. The cheap rung declares correctly early, about 3 s
after notice. Later evidence attached to the anomaly makes the checker empty. Then the engram's
coincidence completes and the gate admits it. **I infer that order from the counts and did not
observe it directly:** the run outputs carry no recall instant and no gate counters (see "Not
done").

If the order is right, the brief's premise fails *on timing*. Gating on the verifier's verdict at
the recall cannot know that the cheap rung had already returned a consistent hypothesis before the
contradiction arrived.

**What it does not show.**

- It does not show the gate is useless. It cuts displaced correct declarations by a factor of
  about 5 (42 → 8 or 9), which is the property the displacement test guards.
- It does not show the two-site key fails on cascades and split brains. In 20 streams under the
  selection oracle there are about 2 binds per stream, and 0 correct pair recalls is not evidence
  either way (W2: power).
- It says nothing about held-out streams.

**Rejected readings.**

- "The gate is not running": `eng_off_gated` shows the checks run, 22,294 against 2,943, and the
  displacement drops.
- "A1c changed the arms it is compared with": A1a's eight arms reproduce exactly, and R6 is 62 of
  62.

**Credible alternatives the chief could test next** (none tested here, no claim):

1. Gate on the verdict *and* on the cheap rung having no declaration yet. This would remove the
   118 added errors, but probably most hard recalls too, since the cheap rung declares on hard
   incidents within about 3 s.
2. Make an admitted recall a question rather than a declaration: the gate becomes a public
   selector.
3. Use a selector that also asks about plain anomalies, so a wrong plain recall can be
   contradicted. Under the selection oracle nothing ever corrects one (A1a's finding, still true).

## Departures from `DESIGN.md` and the brief, with reasons

- **DESIGN departures 66–70** (in `DESIGN.md`):
  - 66: `recall_in` is added beside `recall`, which is unchanged.
  - 67: the adapter resolves recalls after every tick. The order is the same, and a one-site recall
    keeps A1a's anchor-site rule.
  - 68: relation events use channel 4 and a separate sequence space from `0x8000_0000`.
  - 69: generalisation of a pair engram keeps one relation feature and one partner feature, by
    role.
  - 70: the gate's wait is checked before the verdict, so the wait is exactly one review period.
    The gate's wait test found this before any run.
- **`Engrams::new` now refuses nodes of 128 or more**, so that every ordered pair has an edge
  node. Nothing used such nodes.
- **No fixed (site-keyed) two-site key.** The brief's gated site-keyed form is the one-site
  site-keyed key, gated. A carried site-keyed pair would be wrong by construction (W2).
- **A two-site arm keeps the one-site key when no unconnected partner exists.** This is my
  resolution of what the brief leaves open, so that the two arms differ only where a partner
  exists.
- **The gate keeps a recall for one review period** rather than reading the verdict only at the
  recall's step, because the checker runs at most once per period. **A dropped recall does not
  consume the anomaly.** Both choices are mine, written before any run, and stated in `gate.rs`.
- **Arms beyond the brief's three gated forms.**
  - `eng_off_gated` isolates the cost of monitoring and its effect.
  - The late-on twins are there because the brief asks for the late switch to be reported both
    ways.
- **The paired breakdown was added to `a1c_smoke.py` after the run.** It is reporting only; the
  arms, parameters and measure were not touched.
- **Outside my territory (Lab 2's files), minimal, each inert without a gate:**
  - `noticer.rs`: two default `Noticer` methods (`needs_verdicts`, `gated_recalls`) and an import
    of `AnomalyView`.
  - `rung.rs`: two passthroughs (`noticer_needs_verdicts`, `take_gated_recalls`).
  - `arms/mod.rs` (`StreamArm`), three changes:
    - the monitor is set when the rule *or* the noticer asks;
    - the consistency checks are moved before the recall block;
    - a gated noticer's recalls are taken through `take_gated_recalls`.

  For any arm without a gate the checks list is empty, so their order changes nothing; R6's 62
  arms and A1a's 8 confirm it.

## What I am least sure of

1. **The timing inference behind the 118 of 127.** The run outputs contain no recall instant, no
   gate counters and no contradiction onset, so it is an inference from counts. A per-incident
   recall source and instant (E1's planned `Source::Recall` column) would settle it.
2. **Whether contradictions on plain anomalies come from another incident's evidence attached by
   the rung's burst rules**, or from something inherent. This is a hypothesis; R5's "evidence at
   services the rung never attaches" is the nearest evidence.
3. **Why the two-site form makes fewer plain errors.** It may be specificity (fewer recalls of any
   kind) rather than discrimination. The partner is the nearest unconnected first alarm within
   10 s, and on a busy stream that is often a coincidental background alarm.
4. **The one-review-period wait** is reasoned from the checker's cadence, not measured.
5. **The binary's revision.** The smoke's manifest names `b25468c`, but the binary was built at
   `ceb4a99`. Their code is identical; `b25468c` adds only `a1c-regression.csv`. The driver checks
   HEAD, not the binary.

## What the chief should examine most carefully

1. **The 118 "also correct" errors** against the brief's premise that a gated recall "displaces
   nothing". The gate keeps the cheap rung's correct declarations (displaced 42 → 9) but adds wrong
   ones beside them. Whether such an incident counts as an error under A1b is a scoring decision,
   not a mechanism one.
2. **Moving the consistency checks before the recalls in `StreamArm::step`**, which is Lab 2's
   file, and the claim that it is inert without a gate.
3. **The displacement test's exact claim**: action equality on an anomaly the checker explains
   throughout. It does not cover the fallback or added-declaration channels.
4. **The reproduction evidence.** A1a's 8 rows are reproduced, `eng_off_gated` equals `eng_off`
   except for timing and cost, and R6 is 62 of 62.

## What A1b's criterion should bound that the brief does not name

- **Plain incidents that end with both a correct and a wrong declaration, separately from those
  whose correct declaration was displaced.** They are different failures: an added declaration
  against a lost one. The evaluator's convention for a later contradicting declaration should also
  be fixed before the criterion.
- **Recall timing relative to the cheap declaration and to the onset of contradiction**, per tier.
  This needs the recall instant in the output.
- **The gate's admission rate by tier** (offered, admitted, closed, overtaken). The layer counts
  these, but no output carries them.
- **The cost of the gate's checks** (about 2 ms per stream here) and of a two-site layer (about
  89 ms per stream), in the cost column. Also the two-site layer's cell growth against the 65,536
  cell limit, which holds about 450 pair engrams at twelve services; A1b's 300 streams could
  approach it.
- **The selector.** Under the selection oracle no plain recall is ever contradicted. The criterion
  should say which selector A1b learns under, because that decides whether the memory can
  correct its plain errors at all.

## Not done, and why

- **Gate counters and recall instants are not in the run outputs.** Adding columns is E1's
  territory, and a diagnostic run would have been an extra run after seeing results.
- **Mutation testing** was not done (not in the brief).
- **No held-out run** (not in the brief).

## Files

- Crate: `crates/gordian-medium/src/engram.rs`, `src/lib.rs`, `DESIGN.md`, `tests/engram_pair.rs`.
- Adapter: `crates/gordian-run/src/stream/arms/medium/gate.rs` (new), `engram.rs`, `noticing.rs`,
  `mod.rs`.
- Hooks: `crates/gordian-run/src/stream/arms/noticer.rs`, `rung.rs`, `mod.rs`.
- Tests: `crates/gordian-run/tests/stream_medium_gate.rs`.
- Scripts: `experiments/exploration/scripts/a1c_{common,manifests,gate,run,smoke}.py`.
- Outputs: `experiments/exploration/a1c-regression.csv`, `a1c-smoke.csv`,
  `a1c-smoke-paired.csv`, `a1c-smoke-streams.csv`, `a1c-smoke-a1a-reproduction.csv`.
- Kept runs: `artifacts/runs/a1c/` (git-ignored).
