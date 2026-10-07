# Coordinator review log

What the coordinator checked for each merged unit, what it decided, and what it carried forward
to later units. Newest first. Reports from workers are model output; this log records what was
independently verified.

## E1 memory measures and the public record rung — merged (Lab 2); the status quo finds the same shape the engram did

**Provenance.** Eighteen commits on `memory-measures`, then a merge of main (A1c, A1d) resolved
by the lab with the gates rerun (merge commit 106ff76, head ccbeda2: E1 and A1c had built the gated-recall seam twice; the lab kept Lab 1's implementation and its step order, kept its own `recall_source`, and added `gated_recalls_in` with a default that preserves the engram's override; the lab replayed R6 (62 of 62), A1a's 8, A1c's 15 and A1d's 15 arms and its own held-out run after the merge, all byte-identical on the logical files; the chief's own gates on main follow in the commit after this entry). The chief recomputed from the held-out run's
`memory_incidents.csv`: 83 hard recurrences on 40000–40199 (W2's count, from the evaluator's
own `recurrence_of`); unasked-correct among them 0 (re-anchor), 7 (site-keyed, kinds, reset),
17 (family-keyed, timing, carried), 4 (family, reset); `stale_wrong` 0 / 6 / 539 / 8; plain
unasked-wrong 732 against 1019 for the carried family form (+1.435 per stream). R6's held-out
replay 62 of 62 under the stripped-column reading (chief's hashes on the fourth replay, after
the lab reran the one that overlapped Lab 1's build). cargo-mutants on `memory.rs` 104 of 108
caught, 4 unviable; the V1 back-test byte-identical after the schema extensions. Runs kept in
`artifacts/runs/e1/`, including the failed first tuning attempt (a harness defect in judging a
carried source, fixed, kept and labelled). Two process breaches disclosed by the lab (an early
`cargo check` during Lab 1's run; an identity replay overlapped by Lab 1's build, rerun).

**Verdict: E1 passes its criterion** (measures K1–K10 with fixtures and mutation, the harness
source record and the two new columns, the record rung in both forms with three policies, the
schema extensions, identity, the held-out table with W2's ceiling beside it).

**What the record rung shows (objective, environment).**

| Form (held-out, 200 streams) | Hard recurrences unasked-correct (of 83; ceiling 41) | Collision share of recalls | Stale errors plain / hard / decoy | Cost against the re-anchor |
|---|---|---|---|---|
| Site-keyed, reset per stream | 7 = 0.084 [0.038, 0.138] | 0.11 (1 in 200 streams) | 2 / 3 / 1 | about equal |
| Family-keyed, carried across streams | 17 = 0.205 [0.135, 0.278] | 0.81 (1.52 per stream) | 372 / 47 / 120 | −27.6% |
| Family-keyed, reset | 4 | — | 6 / 2 / 0 | — |

- **The status quo finds exactly the shape the engram found.** Within a stream, memory is
  clean and small (7 of 41 reachable, one collision in 200 streams); across streams, a key
  made of invariant features recalls more and is wrong four times in five, on plain incidents
  and decoys, from sources in earlier streams (654 of 661 recalls). This is the world's
  structure, reached now by the comparator from the public side, and by Lab 1 four times.
- **No cell meets W2's stale-error bound and captures a useful share.** The chief's A1b draft
  specification, evaluated with the record rung standing in for the engram, returns the
  outcome "stale errors exceed the bound". The comparator fails the clause that would have
  been asked of the medium.
- **The gate finding is the same as A1c's**: the public checker's verdict admits recalls on
  plain incidents.
- **Why only 7 of 41** site-reachable recurrences are recalled is undiagnosed (no arm-internal
  counters); the lab says so. The confirm-every-second policy is worse than never confirming
  and is undiagnosed. Both are the first things an A1b on this form would have to measure.

**Decided.**

1. The within-stream memory question on this world is answered to within the power the world
   offers: a clean lever of at most 8.5% of the bill, of which a simple public table takes
   about a sixth. The chief withdraws the recommendation that A1b be the within-stream unit
   unless the user wants the engram measured against the 7-of-41 row for completeness; the
   value is low and the draft specification would be fixed from this table.
2. The cross-stream learning line (option 2 of the synthesis: the reasoner's law) is the
   chief's recommendation, pending the user.
3. E1's measures and the record rung stay as instruments for any memory claim on any later
   world; `recall_source` with age is a requirement on any arm that claims memory.

## A1d the engram under a non-privileged selector — merged (Lab 1); negative experience does not help, and the chief's causal claim was wrong

**Provenance.** Five commits in the required order (design b64c24d, code b98492e, scripts and
prediction 1e91217, identity 43061d4, report 7f2e061); the prediction text has no removed
lines after its commit (chief's diff). The chief recomputed: R6's held-out replay 62 of 62;
decision columns per arm from `incidents.csv` and `results.csv` match the lab's table (family
form under the threshold selector against the memoryless arm: plain correct by deadline 388
against 393, hard 21 against 24, calls 495 against 584, reasoner cost −25.8 s). A1c's fifteen
and A1a's eight smoke arms reproduce their kept runs (lab's check). Gates on the merged tree:
see the commit. Runs kept in `artifacts/runs/a1d/`. **Process breach, recorded:** the PI's
release build at 08:27 started while Lab 2's run was live and did not wait; Lab 2 told.

**Verdict: A1d delivers; the table is a clean negative.** Under both public selectors and the
oracle, in every form, the memory gained no decision (one hard incident under the oracle
excepted) and lost a few; of 58 recalls on plain incidents under the threshold rule, 51 added
a wrong declaration and none a correct one. The family form's own cost is small (+0.16 s of
bill per 20 streams); the two-site form's is not (+6.4 s) and it loses the most decisions.
Every prediction the PI committed fell inside its 80% range except the two-site form's.

**The chief's causal claim in the A1c entry was wrong, and this is the test that showed it.**
I wrote that the one-sided teacher was "the causal root". With plain outcomes bound and
contradictions available, the memory recalls wrongly on plain incidents at the same rate. The
root is not the teacher; it is the key. Recorded as the chief's wrong lead (not a procedural
error; the claim was tested by the next unit as it should be).

## Synthesis after W2, A1a, A1c, A1d and V1 — what the engram line has shown, and where the purpose now points

**Representation.** The engram's key is a set of stream-invariant public features at one
service (abnormal kinds, counter bands, catalogue message ids), optionally a second service
and a timing band. Four units, every key form, two selectors, two gates: it never
distinguishes a hard family from a plain incident well enough to gain a decision. The
experimenter-side reason is in the world's hidden record (sections 4 and 5): each hard family
presents, half the time, as exactly a plain incident at the single-site invariant level, by
design; the evidence that separates them is the stream-local vocabulary (regenerated per
stream, so no invariant key can hold it) or multi-site structure (cascade partner, split-brain
peer), which is rare enough that 20 streams cannot teach it. The chief may say this because the
chief reads the hidden side; no lab encoded it, and the result was reached from the public
side four times.

**Objective.** The family law carried across streams, W2's 97.8% ceiling, is not reachable on
this world by any single-site invariant key. That is a property of the world, not of the
medium or of memory in general, and it was the world's stated design (hard and decoy
indistinguishable at first). What remains reachable by memory on this world is the within-
stream vocabulary law (ids determine the family with full mutual information inside a stream;
reach at most 26% of hard incidents and 13% of the bill) and structure (hidden edges, A2).

**Causality, inverted.** The law that does transfer across streams on this world is the
reasoner's: an answer is right with probability h only when the decisive evidence is in the
context, and that law is the same in every stream. A learner that learns when an answer can
be trusted and what to include learns something invariant; that is the aim's proxy 2 applied
to EXP-101 and EXP-102, and it has not been tried as learning.

**Environment.** The non-privileged selector costs ten times the oracle (584 calls against 61
in 20 streams) for +18 plain decisions and −2 hard ones; B4's result restated on decisions.
Any learning claim under it is a claim about a far more expensive regime.

**Failure and meta.** Three coordinator errors were found by the verifier this round and one
causal claim by the next unit. The program's correction loop is working; the chief's priors
about where the lever is have been wrong twice in a day (recurrence; the teacher). The honest
reading is that on this world memory over public patterns is a small lever and the medium has
not yet shown a mechanism the status quo lacks for it.

**Options for the user (the chief recommends 1 and 2; 3 proceeds as already approved).**

1. **A1b becomes the within-stream memory unit:** keys on the stream's own ids (site- and
   family-keyed inside a stream, reset at the boundary), under both selectors, against E1's
   record rung with the same keys, criterion as code, with W2's within-stream reach as the
   ceiling. Small, bounded, and it closes the memory question on this world honestly either
   way.
2. **The cross-stream learning line moves to the reasoner's law:** a learned selector and
   context builder (what to ask about, what to include, when to trust an answer and declare),
   under hard limits, measured by decisions per unit cost and by improvement per incident
   seen against B4's tuned public selector and the rung's context, in the medium and as a
   conventional learner. This is EXP-101 and EXP-102 as learning experiments, and the first
   place on this world where a law learned in one stream applies in the next.
3. **A2, anticipation of hidden edges**, proceeds now (Lab 1), with Lab 3 scoring predictions
   from the hidden side; W3 gives it power.
4. **A labelled learnability world** (the vocabulary persisting across streams) would test the
   engram mechanism on a world built to be learnable; it could not be claimed as this world's
   law and the chief does not recommend it before 1 and 2.

## A1c the recall gate and the two-site key — merged (Lab 1); the public checker does not separate plain from hard, and the chief's acceptance measure was the wrong one

**Provenance.** Five commits in order (design 33a31e8, code 43e0648, scripts with the PI's
prediction before any run ceb4a99, identity b25468c, report feae28d). The chief recomputed:
R6's held-out replay 62 of 62 on both files; the smoke counts from `incidents.csv` match the
lab's table (gated family 180 plain unasked-wrong, 1 hard unasked-correct; A1a's eight arms
unchanged; the gated site form recalls nothing). Gates on the merged tree under the runner: see
the commit. Runs kept in `artifacts/runs/a1c/`. Not done, recorded: gate counters and recall
instants in the run output (E1's territory); mutation tests; no held-out run (none briefed).

**Verdict: A1c delivers; its acceptance clause triggers the negative branch** (180 exceeds
53 + 5), so A1b's design is reconsidered here before its criterion is fixed.

**Coordinator error (thirteenth).** The acceptance measure I fixed, "plain incidents with a
wrong unasked declaration", counts a wrong declaration made beside a correct one, which the
decision scoring ignores. The chief's recomputation on the decision columns:

| Arm (20 tuning streams) | Plain correct by deadline (of 434) | Plain wrong only | Hard correct by deadline (of 40) | Calls | Cost |
|---|---|---|---|---|---|
| M3, no memory | 375 | 22 | 26 | 61 | 16.4 s |
| A1a family form, ungated | 333 | 63 | 20 | 45 | 12.0 s |
| A1c gated family form | 366 | 31 | 24 | 51 | 13.5 s |
| A1c gated two-site, late on | 369 | 29 | 24 | 55 | 14.9 s |

On decisions, the gate cut the memory's damage from 42 displaced plain decisions to 9 and the
hard loss from 6 to 2, at 18% less cost; that is the result the measure hid. The measure was
not wrong to exist (E1 keeps it as a declaration-level count), but a bound on it is not a bound
on the purpose's quantity. Lesson, the same as the tenth: the quantity bounded must be the one
the charter names (decisions, by deadline), and A1b's specification bounds displaced correct
decisions, paired, not declarations.

**What it shows (mechanism, failure, causality).**

- **The public rules' verdict does not separate plain from hard.** R5 measured that the
  checker contradicts 97% of plain anomalies at some point; so a gate on it admits a recall on
  a plain incident as soon as stray evidence arrives, typically after the cheap rung has already
  declared correctly. The lab inferred the order from counts and said so; the chief accepts the
  inference as the best model and not as observed.
- **The memory learns from a one-sided teacher.** Under the selection oracle the reasoner is
  asked about hard incidents only, so every bind is a hard outcome and no plain pattern is ever
  bound or contradicted. A learner that is never shown what plain looks like cannot learn the
  difference; the vote mechanism built in A1a has nothing to vote against. This is the causal
  root, and it is the selector, not the engram. A1a's PI noted it, A1c's PI listed it third; the
  chief now treats it as the design decision.
- **The two-site key** recalled nothing on cascades and split brains in 20 streams, as power
  predicts (about two binds per stream); its fewer plain errors may be specificity. Unknown.

**Decided.**

1. **A1b runs under a non-privileged selector, not the selection oracle.** The arm asks the
   reasoner through B4's public selector (tuned under cost, as B4 built it), so that plain
   anomalies are sometimes asked about, plain outcomes are bound, and a wrong plain recall can
   be contradicted. The record rung (E1) runs under the same selector. The selection-oracle
   rows are kept as labelled ceilings. This also makes A1b the first unit whose cost column is
   honest in the sense B3's review asked for.
2. **A1d (Lab 1), before A1b:** the engram arm under B4's public selector; bind of every
   answer including plain kinds; a recall never speaks on an anomaly that already carries a
   declaration made after the checker's last consistent verdict (so memory corrects a stale
   cheap declaration and does not add to a standing correct one); gate counters and recall
   instants through the medium's trace port into its own file; identity; the same smoke on the
   decision columns. Brief in the queue.
3. **A1b's specification** will bound, first, displaced correct plain decisions as a paired
   excess over the memoryless arm under the same selector; the A1c declaration-level count is
   reported, not bounded.
4. **W3 stays queued** behind the two-lab rule; it is the power lever and runs as soon as a lab
   is free.

## V1 criteria as code — merged (Lab 2); the verifier exists, and its first act was to catch the chief

**Provenance.** Six commits on `criteria`: the schema and the four specifications (f3c407a)
before the measures (99fba9a) and the back-test (efc513b); one reports-only amendment between
(d52a26b; the chief read its diff: report rows added, no clause, bound, bootstrap setting or
verdict changed). No Rust touched (chief's diff over `crates/` and the lockfile: empty). The
chief reran `scripts/criterion.py` on the M2, L1 and M3 specifications against the kept runs:
M2 holds at 100 ms only (+0.040 [+0.022, +0.060]; +0.532 [+0.462, +0.603]; background 5.44),
L1's clause 2 fails (+0.043 [−0.021, +0.126]) and the conjunction is false, M3 does not hold;
the clause values equal the committed back-test. Analysis suite 507 passed on the merged tree;
the guard passes. Cargo gates not rerun (nothing in Rust changed).

**Verdict: V1 passes its criterion.** 189 printed numbers and verdicts: 166 match to the
printed precision, 11 one unit off in the last digit (bootstrap Monte Carlo error; the script
reproduces the labs' fixed-seed CSVs to 1e-14 in 4,209 cells), 4 mismatch, 8 outside the
schema. Every verdict the log states is reproduced. Six hand mutants caught.

**The four mismatches are findings against this log, and three are the chief's errors.**

- **Coordinator error (tenth).** The L1 entry prints a slope of the chief's own, +0.053
  [−0.037, +0.085], with no recorded definition; no reading the lab tried reproduces it, the
  lab's own figure reproduces exactly, and the verdict is the same under every reading. The
  number stands in the log as unreproducible and is so marked here. Lesson: a number the chief
  prints must carry its definition, which is what a specification is.
- **Coordinator error (eleventh).** The B3 and M3 entries print 0.80 for the ramp + split cost
  per stream; the files give 0.7948, double-rounded. Immaterial, and wrong.
- **Coordinator error (twelfth).** The M3 entry says no per-stream column existed to recompute
  strict precision; `notices.csv` carries `notices_anchor_site_correct`, and the script gives
  0.77 / 0.81 / 0.84 as printed. The chief's tooling, not M3.
- The eleven one-digit differences in interval ends are the chief's bootstrap at another seed;
  each printed end lies within the range of the same end under 40 seeds. Not an error, and the
  cure is the specification's fixed seed. One clause in sixty is seed-sensitive at the bound
  (M2's 500 ms lower bound straddles 0.01 across seeds; its point test fails anyway).

**What it means (meta, objective).** Charter section 11's rule is now operational: the verdict
is the script's output, with the spec's hash and every input file's hash recorded. The lab
named what the chief could not have seen in its own log, which is the point of a verifier
that is neither explorer nor expert. The schema's gaps for A1b are named by the lab and
adopted below: a `not` node and exclusive outcome categories (for "the record rung captures
the lever"), not-null filters, count-with-total clauses so that power is a precondition, a
per-stream quantile, and a join of two arms by incident. Nothing enforces that a
specification was committed before its run except git history; the chief's verification
records both hashes and their commit order.

**Decided.**

1. **E1 (Lab 2) starts now** as amended; it also extends the schema with the five constructs
   above, with tests, so that `experiments/criteria/a1b.json` can be written when its numbers
   are fixed.
2. **A1b's specification is written by the chief, before any A1b run, in the schema**; the
   verdict comes from the script. No number the chief prints about A1b appears without its
   clause id.
3. **This log's L1 entry is annotated** (one line) as unreproducible on the chief's slope; the
   entry is otherwise unchanged, as the rule on frozen records requires.

## A1a the engram, build phase — merged (Lab 1); the mechanism exists, and its first values collide exactly as W2 predicted

**Provenance.** Eight commits on `engram` in the order the brief required: design into
`DESIGN.md` (a82c38c) before code (55be9db); key and confirmation policy fixed (715a700);
W2's four constraints applied before any arm ran (efcaa89); identity recorded twice; the smoke
run's manifest names `65ddb7f`. The chief recomputed: R6's held-out replay on the final engram
code matches all 62 arms' `results.csv` and `incidents.csv` hashes in
`experiments/exploration/r6-results-sha256.csv`; the smoke counts from `incidents.csv` (hard
unasked-correct 3 / 0 / 0, hard escalated 28 / 40 / 40, plain unasked-wrong 204 / 53 / 53 for
the main form, bind-off and M3) match the lab's table; no family or mode name appears in the
engram code. Hooks in Lab 2's territory (`noticer.rs` two default methods, `rung.rs` a
passthrough, `arms/mod.rs` a `Source::Recall` variant and a never-escalate filter) are inert for
every noticer without memory, which the identity shows; accepted, and Lab 2 told. Gates on the
merged tree under the runner: see the commit. Runs kept in `artifacts/runs/a1a/` (ignored).
Not done, recorded: mutation testing; `total_cost_ns` omits the noticer's charge for every
medium arm since M2 (the bill has it).

**Verdict: A1a delivers** (mechanism, adapter, both key forms, three confirmation policies,
identity, smoke, report). No criterion was set and none is claimed.

**What the smoke shows (failure, mechanism, objective).** First values, 20 tuning streams,
nothing adjusted:

| Arm | Hard unasked-correct | Hard escalated | Plain unasked-wrong |
|---|---|---|---|
| M3 (no layer) and bind off | 0 | 40 | 53 |
| Family form, generalising | 3 | 28 | 204 |
| Family form, exact keys | 0 | 40 | 57 |
| Site-keyed, reset per stream | 0 | 40 | 53 |
| Site-keyed, carried (control) | 0 | 40 | 61 |
| Family form, confirm every 4th | 3 | 34 | 127 |

- **The collision W2 predicted is real and immediate.** The generalising family form recalls
  on plain incidents 151 times more than the control in 20 streams and displaces 42 correct
  cheap declarations, and the count rises with streams. "Evidence after 2 s" is not the
  rule-breaking evidence; plain incidents produce late evidence too. Exact keys never recur;
  generalised keys do not discriminate. The PI's hypothesis (two or three surviving features,
  one early alarm kind and one late band, shared with plain incidents) is plausible and
  untested.
- **The within-stream site form recalls nothing** on 20 streams (W2: 0.44 hard recurrences
  per stream, reachable only after a correct answer), and the carried form only adds errors,
  as W2 said it would by construction.
- **The three correct recalls are all slow leaks**, the one hard family whose evidence sits at
  a single service. Cascade and split brain put decisive evidence at two services, and a
  one-node coincidence cannot key them. This is a structural limit of the engram as built.
- **Under the selection oracle nothing corrects a plain-incident recall**, because plain
  anomalies are never asked about, so no contradiction ever arrives. The learner's errors are
  invisible to it. That is a property of the privileged selector, and one more reason EXP-101's
  non-privileged selector matters for the learning claim too.

**Coordinator reading.** The purpose's danger case (charter 1.2, "stale errors grow with
experience") appeared in the first twenty streams, before any tuning, which is the right time
to see it. It says what the recall gate must be: not a time since the anchor but the public
rules' own verdict that the evidence cannot be explained (the rung's consistency checker,
`contradicted_since`), which is the public meaning of "rule-breaking". Gated so, a recall can
fire only where the cheap rung would have returned no hypothesis, so it displaces nothing. The
PI named this and left it out as scope; it is the next unit.

**Decided.**

1. **A1c (Lab 1), before any A1b run:** the recall gate on the public consistency checker; a
   two-site key (the anomaly's service and the service whose alarm the public graph cannot
   connect to it, with their timing) so that cascades and split brains are keyable; identity;
   the same smoke, reported against A1a's table. No tuning. Brief in the queue.
2. **A1b's criterion** will bound plain-incident unasked-wrong declarations as the paired excess
   over the memoryless arm (the measure the PI asked for) before any other clause, and will
   report recall timing against the selector's ask instant.
3. **E1 gains** the `Source::Recall` reading: a declaration whose source is a recall is counted
   on its own in `results.csv` (today it is folded into the cheap rung's count), so the
   memory's declarations are a column, not a ledger search.
4. **M-cost:** `total_cost_ns` for medium arms is a known omission since M2; A1b's cost column
   is read from the bill, and the fix is queued into E1 as a column, not a redefinition.

## W2 the learnable laws — merged (Lab 3); the recurrence lever is small, the family law is the large one

**Provenance.** Seven commits on `world-laws`; a hidden-side accessor (`oracle::rebuilt_incident`,
three tests) and a `laws` example, both behind `reveal-hidden-state`; the guard passes. The chief
reran `w2_laws.py` and `w2_ceiling.py` from the kept hidden tables and run directories: 26 of 26
generated CSVs byte-identical (the 27th is the provenance file). The chief recomputed from the
raw hidden tables and the run with its own code: recurrence share 0.2049, 5.48 per stream, 83
hard recurrences (0.415 per stream), 51 of 429 same-family-and-mode-elsewhere (the chief's first
count of 34 was a NaN-mode bug in the chief's code, not the lab's), 12 stale hard recurrences
under the template-based definition, site-keyed reach 41 incidents and 39 calls = 8.5% of calls,
hard-incident quality 0.482, reasoner 99.93% of the bill. All match. The ceiling run is identical
to L1's `sel_reanchor_privileged` modulo ids (lab's check; chief read the claim, did not rerun
it). Gates on the merged tree under the runner: see the commit. Outputs kept in
`artifacts/runs/w2/` (ignored) for A1b.

**Verdict: W2 passes its acceptance** (reproduction, seed ranges and sides named, bounds proposed
with reasons).

**What it means (environment, time, objective, failure).**

- **Recurrence is the wrong lever.** Only 0.415 hard incidents per stream repeat an earlier
  hard one; the most a site-keyed memory fed by this arm can save is 8.5% [6.4, 10.8] of the
  bill, within an interval of ±2 points; and recurrence never crosses a stream boundary, so a
  site-keyed memory has no experience curve across streams at all. The EXP-103 framing in the
  charter ("recurring incidents") describes a small effect on this world.
- **The family law is the large lever: 97.8% of the bill is reachable by a family-keyed memory
  carried across streams.** That is learning what a kind of fault looks like, from public
  evidence, and recognising it in a stream never seen. It is also exactly where the danger is:
  a key on first-phase evidence collides with plain incidents and decoys at 1.9 wrong recalls
  per stream, and 40–54% of the answers the reasoner gives are wrong, so a memory inherits
  error unless it aggregates and confirms. This is the purpose of charter 1.2 stated as a
  measurable problem: a law learned from a noisy oracle, held across worlds, applied only when
  the evidence is decisive.
- **The vocabulary is a within-stream fact.** Ids determine the family perfectly inside a
  stream and are regenerated per stream; alone they are a 13%-precision marker. A key for the
  cross-stream form cannot use them.
- **Power is the binding constraint for A1b as briefed.** About 41 reachable recurrences on 36
  streams; a paired margin under 0.06 is unresolvable. The lab's three levers (a world with
  three times the hard share; the family form across streams; a phase-2-keyed collision floor)
  are adopted below.
- **Lab 3's own instrument.** The rebuild accessor makes "altered by a regime change" a measured
  property of an incident rather than a table; the chief read its tests and its fallback (an
  incident whose only dependent came with the added edge rebuilds as identified), and accepts
  it with that caveat recorded.

**Decided.**

1. **A1b is reframed before any A1b run.** Its primary population is the family form carried
   across streams (the law), with within-stream recurrence as a secondary row. The lab's bound
   structure is adopted: unasked-correct on hard incidents reachable by the key form, a floor
   with a lower bound, a paired margin against the record rung of the same key form with a
   pre-accepted "the record rung captures the lever" outcome; the stale clause split into
   collision/staleness (absolute bound) and inherited error (paired, no absolute bound); every
   arm evaluated with and without the stream-boundary reset. The numbers are fixed in
   `experiments/criteria/a1b.json` after E1 reports, by the chief, before any A1b run.
2. **E1's brief is amended** (before any E1 code): the harness records a recall's source (the
   observation the memory was bound at), so the evaluator separates collision from inherited
   error; `stale_wrong` is computed over recall-sourced declarations and as the paired excess
   over the memoryless arm; the record rung is built in site-keyed and family-keyed forms with
   the reset as a switch, and its family key uses invariant public features only.
3. **Lab 1 was told** the four design constraints (reset at the boundary, invariant features for
   the cross-stream key, wait for the decisive evidence, aggregate over bindings because half
   the stored answers are wrong), as constraints on A1a's build, not as a criterion.
4. **W3 (Lab 3) is queued:** world C with three times the hard share for A1b's power, and the
   phase-2-keyed collision floor; brief in the queue.
5. **The charter's EXP-103 wording** ("recurring incidents") is not changed by this entry; the
   reframing is recorded here and will go into the charter with the A1b registration.

## Resumption, 2026-10-07 — three lessons applied; the program's centre moves to memory and anticipation

**The reference.** Alman and Vassilevska Williams, arXiv:2610.06783 (5 October 2026; existence
confirmed by the chief, content not read beyond the abstract-level coverage): truly subquadratic
3SUM and truly subcubic APSP, the core reported as found by an internal language model in one
unattended run of about 16 million tokens, then simplified and formalised by the authors.
Unreviewed; scaffolding undisclosed; relied on here for nothing mathematical.

**Lessons applied (what changed in the repository).**

1. **The gap is cost, not capability.** The status quo substrate produced a discovery-like
   result at a token cost the aim counts against it. Charter section 1.1 now says so, and makes
   proxy 1 primary.
2. **The verdict is computed, not read.** Nine of this log's recorded coordinator errors were
   criterion or reasoning errors; none was caught by a checker because the checker was the
   chief. Charter section 11 and AGENTS.md now require a committed machine-evaluable
   specification per criterion (`experiments/criteria/`), evaluated by `scripts/criterion.py`
   (unit V1) from raw files. Explorer, expert, verifier are three roles.
3. **A unit is one trajectory.** The labs' failures were usage limits, restarts and disk, never
   reasoning, and resumption from the last commit worked every time. AGENTS.md now states the
   checkpoint rule as the design rather than the accident.

**The purpose (charter section 1.2).** Noticing is settled on this world (B3, M2, M3). The
centre moves to what the aim asks: experience converted into correct decisions made without the
reasoner (memory) and before the decisive evidence (anticipation), per unit cost and per unit
experience, with stale errors scored on their own. The world offers the laws by design
(recurrence, vocabulary, hidden edges, regime changes; hidden record section 14). The
comparator is a public record-keeping rung, because the status quo has captured every lever so
far when given the chance, and the honest prediction is that it captures much of this one too;
the medium's claim, if it has one, is generalisation across sites, survival of regime changes,
and cost. EXP-103 and EXP-104 move ahead of EXP-101 and EXP-102.

**Queued and launched.** A1a (Lab 1, opus: the engram mechanism and adapter, identity, no
criterion beyond deliverables), W2 (Lab 3, sonnet: the learnable laws measured from the hidden
side, the perfect-memory ceiling, world B); V1 and E1 (Lab 2) follow under the two-lab disk
rule. A1b's criterion is fixed after W2 and E1 report and before any A1b run, as a committed
specification. M4, B5, L2 stay stopped and resumable; C2, M5 stay queued behind.

**Chief's own uncertainty.** Whether the record rung leaves the medium anything is the question,
and I would not bet on the medium. If the record rung captures the lever, the purpose stands
and the medium's role narrows to generalisation and regime survival, which A1b measures
directly. The anticipation half (A2) is unbriefed until A1b reports.

## Stop order, 2026-10-07 — M4, B5 and L2 stopped mid-unit; evidence merged, criteria undecided

**What happened.** The user ordered the labs stopped. Each unit's branch was committed as left
(the in-progress files committed unreviewed under a message saying so), pushed to origin, and
merged into main under the full gates. None of the three reached its held-out or main run, so
**none of the three criteria is decided**, and nothing below is a result against them.

| Unit | Reached | Not reached | Gates on the merge |
|---|---|---|---|
| M4 (Lab 1) | switchable precision devices; the ramp inhibit's sparing form; tuning stages a–c on seeds 10000–10099; the frontier table; the selected configuration (`m4-selected.json`) | pinning, freeze, byte identity, held-out run, report | fmt, clippy, 905 Rust tests, guard, 450 analysis |
| B5 (Lab 2) | the call-budgeted selector with its readings in the module docs; tests; byte identity 62 of 62; a hand-mutant script | mutation runs, AUCs, tuning, delay sweep, held-out table, report | fmt, clippy (one lint fixed by the chief in the mid-edit test), 923 Rust tests, guard, 470 analysis |
| L2 (Lab 3) | the echo state network noticer with update rule, priors and residual rule in the module docs; tests; byte identity 62 of 62 | tuning, merge, main run on seeds 40000–40199, analysis, report | fmt, clippy (index-loop lints in the mid-edit numerics allowed with a note for the PI, not rewritten), 951 Rust tests, guard, 476 analysis |

**Evidence recorded.** M4's tuning tables and frontier (`experiments/exploration/m4-*.csv`);
B5's and L2's byte-identity records; every run log and manifest under
`artifacts/runs/stopped-*` (ignored); the branches `precision-frontier`, `budgeted-selector` and
`reservoir-noticer` on origin at the stopped heads.

**What the chief did not do.** Verify any tuning number from raw files (the tuning directories
were deleted by the PIs under the retention rule, which keeps held-out runs only); review the
in-progress files for correctness beyond the gates. The three units resume from their branches
with a fresh PI reading the stop commit first; their criteria stand as fixed.

**State of the program at the stop.** Merged and verified: Stage A, Stage B, R1–R10, W1, M1,
M1b, M2, M3, B1–B4, L1, C1. Decided: EXP-101 is reframed to escalation control at matched
noticing under a call-budgeted selector (B5 was building the instrument). Open: whether the
medium reaches parity with the best public noticer on anchoring, leak and precision at once
(M4); whether a generic temporal learner reaches the hand design (L2); whether a public score
has any selectivity on this world (B5). The last cross-lab synthesis stands (after W1, M1, B1),
with the additions recorded in the M2, B3, L1, B4, M3 and C1 entries.

## C1 the incremental-dataflow noticer — merged (Lab 2); exact reproduction, and the medium is not the cheap one

**Provenance.** The engine choice was recorded before any code: a hand-written incremental
relational core (206 code lines; ordered tables with counted probes, writes and scans; no
dependency, hash, thread or clock) rather than timely/differential dataflow, whose operators
cannot be counted from outside and which were not in the lockfile or registry cache; the unit
therefore measures this engine and makes no claim about those libraries. Reproduction checked
three ways: per-call differential tests against the hand-written noticers on the 100 tuning and
200 held-out streams (10,828 notices, 10,737 retirements, no difference), eight base/ramp/split
combinations and 240 randomized scenarios; and the harness's own files. The chief recomputed
from the kept held-out directory: the dataflow arm's `notice_incidents.csv` equals the B3 row's,
the notice files differ only in the noticer's id label, `results.csv` only in the bill, and
`incidents.csv` only in two clock columns by microseconds, because charging the noticer advances
the logical clock within a step; the unbilled control is identical to B3 in every result file.
Byte identity 62 of 62 (chief's hashes). 22 hand mutants caught after six boundary unit tests
were added; cargo-mutants not run. Gates under the runner on the merged tree: fmt, clippy
`--locked`, 851 Rust tests, the oracle guard, 450 analysis tests. The benchmark ran three times
under the cgroup. Outputs in `artifacts/runs/c1/` (ignored).

**Verdict as written: C1 passes** (exact reproduction, cost reported with calibrated prices).

**What it means (structure, environment, objective).**

- **The public rules are folds, not joins.** Attach reads the anomalies as the last observation
  left them; chain ties break by arrival order; a moved anchor resets a burst timestamp a
  rebuild would not. Reproducing them exactly on relations meant writing each quirk down as a
  reading and keeping the step's evaluation order; the engine contributed nothing to that. What
  it made easier is index upkeep (four projections maintained in one place) and counting.
- **Per stream, in wall time on the same streams: hand-written rules 1.95 ms, dataflow 5.6 ms,
  medium 10.5 ms.** The medium's own operations are 16 cell updates per observation, nearly all
  its bill; its ticks are 7%. The dataflow noticer's 46 counted operations per observation are
  mostly clock-driven scans of score windows and live anomalies, which an expiry-driven engine
  would make incremental. **On this world and at this scale the medium's sparsity buys no wall
  time over either alternative**; its larger operation count comes from the graph's redundancy,
  not from the substrate. None of this is the reasoner bill, which dominates every arm by three
  orders of magnitude.
- **Prices are a judgement within the band, not a fit.** The isolating workloads price only
  part of the program's time (7–9 ns per operation against 11–13 in program workloads), the
  fire operation is inlined away, and in situ the noticer runs at 2.0× its model. The medium's
  in-situ ratio is 0.6–0.7. Only the wall columns compare like with like, and the lab said so.
- **Billing changes the clock.** Any billed noticer advances the logical instant by its charge
  within a step; on this run it moved `first_correct_at_ns` by 0.7–30 µs on every row and
  changed no notice and no result. A declaration landing within microseconds of a step
  boundary could in principle flip. Recorded as a harness property for EXP-101's registration:
  every arm in a comparison is billed, or none is.
- **Code.** 1,441 lines for the dataflow noticer (engine 206, relations 795, rules 210, seam
  184) against 840 hand-written; the medium's graph and adapters 956 on a 3,207-line engine
  crate.

**For the aim.** C1 removes a claim the medium might have made: that event-driven sparsity is
cheaper than incremental rules. It is not, here. What the medium still has over both rule
substrates is what C1 did not test (ports, the oscillome, learning) and the operating point M3
found (precision at parity anchoring). The improved question the PI left is the right one:
which rules are folds and which are joins, and does sparsity pay on the joins.

**Decided.**

1. The dataflow noticer is a labelled row in EXP-101's table, billed like the medium, with the
   hand-written B3 row unbilled beside it so that the billing's clock effect is visible.
2. C2 (Lab 2, after B5): an expiry-driven score window in the engine, and one genuinely
   relational rule (a join over many services) on both substrates, to answer the folds-or-joins
   question. Queued, criterion to be fixed before code.
3. The medium's operation count per observation (16 updates) is a target for M5: a graph with
   less redundancy at the same anchoring and precision, measured in counted operations and wall
   time against C1's rows.

## M3 sub-tick support and strict precision — merged (Lab 1); the mechanism works, the criterion fails

**Provenance.** Two interruptions; the final PI verified the inherited state, committed the two
crash-left test files after reading them, finished the mutation tally with two fresh runs (294 +
94 mutants: 352 caught, 25 unviable, 3 timeouts that are detections, 8 missed and argued
equivalent one by one), froze, merged main (L1 and B4), and found that B4's new notice field
broke the pinned freeze digests; the fix strips exactly B4's two inert spellings, and the 40 of
40 per-arm reproduction against earlier runs is the independent check that the merge changed
nothing. Byte identity 62 of 62 (chief's hashes). The held-out and byte-identity directories
were kept and the chief recomputed every criterion row from them. Gates under the runner on
the merged tree: fmt, clippy `--locked`, 875 Rust tests, the oracle guard, 450 analysis tests.
Outputs in `artifacts/runs/m3/` (ignored).

**Re-verified from the held-out run** (paired against the re-anchor, 0.952):

| Tick | Anchor-correct | Difference [90%] | Leak noticed | Background / stream | Result 1 |
|---|---|---|---|---|---|
| 100 ms | 0.981 | +0.030 [+0.011, +0.049] | 0.770 | 2.04 | misses the margin by 0.0004 |
| 500 ms | 0.970 | +0.019 [−0.003, +0.040] | 0.892 | 0.95 | fails |
| 2 s | 0.949 | −0.003 [−0.028, +0.022] | 0.712 | 1.01 | fails |

Sub-tick pruning off (control), anchor-correct: 0.976 / 0.911 / 0.780 at 100 ms / 500 ms /
2 s, so pruning is worth +0.005 / +0.059 / +0.169 at the same graph. All match the lab. Strict
precision (0.77 / 0.81 / 0.84) is the lab's figure; the chief did not find a per-stream column
to recompute it and takes it as reported.

**Verdict as written: M3 does not hold.** Result 2 holds at every tick.

**What it means (mechanism, objective, failure).**

- **M2's diagnosis was right and the fix works.** The tick was the anchoring rule's floor;
  pruning the support below the tick lifts 2 s anchoring from 0.780 to 0.949 and 500 ms from
  0.911 to 0.970. The medium now anchors as well at 500 ms as the best public row does
  (0.970 against 0.973).
- **The criterion failed because the tuning rule bought something else with the gain.** Under
  the new strict-precision bound, the devices that raised precision from about 0.5 to about
  0.8 (cluster merge, confirmation in event time, a ramp inhibit) cost anchoring; at 500 ms ten
  configurations tied on tuning anchoring and the tie rule chose the one with fewest false
  notices. The frozen media sit at about 1 false notice per stream against a bound of 6.8 and
  precision 0.81 against 0.67: a different operating point from M2, not a failure of the
  mechanism. A chief who wanted anchoring should not have let a tie rule prefer background.
- **The leak loss is a side effect of a precision device.** The ramp inhibit, which suppresses
  the ramp detector while another anomaly is open, drops leak noticing from 0.99 to about 0.89
  at 500 ms and 0.71 at 2 s; the tuning score was bound by anchoring and ignored it.
- **The medium's first advantage outside anchoring.** At 500 ms, against ramp + split over the
  re-anchor: anchoring at parity (−0.003 [−0.022, +0.016]), 5.3 fewer false notices per
  stream, +0.11 strict precision, at the same reasoner cost (0.81 s against 0.80 s), and far
  worse on the leak (−0.094 noticed, −0.49 anchored at start). In B5's call-budgeted frame,
  precision is what decides which anomalies get asked about, so this is the row that matters.

**Decided.**

1. **M4 (Lab 1):** the anchoring-against-precision frontier, each precision device switched
   separately, with a ramp inhibit that does not suppress a ramp-noticed anomaly. Criterion
   (fixed now): a parity test on all three public measures at 500 ms against ramp + split over
   the re-anchor: anchor-correct ≥ 0.963 with the paired lower bound above −0.03, leak noticed
   ≥ 0.976 with the paired lower bound above −0.03, strict precision ≥ 0.693, background ≤ 6.24.
   Feasibility: M3's 500 ms row meets the anchoring and precision parts and misses the leak by
   0.09, and the leak loss has a named cause; both outcomes are reachable.
2. Tie rules in every future tuning rule prefer the criterion's own measure; background is a
   bound, never a tie-break.
3. Sub-tick pruning stays on in every medium arm from here.

## B4 public selectors and decoy accounting — merged (Lab 2); on this world, public selection is always-escalate

**Provenance.** Two interruptions (an API limit, then the disk restart); the final PI found one
mutated line left in the tree by a killed cargo-mutants run and reverted it, redid the mutation
tallies (48 of 48 hand mutants; cargo-mutants 38 of 44 and 58 of 66 caught, the rest unviable,
none missed), reran the byte-identity gate (62 of 62, recomputed), and verified the held-out
run's manifest against the frozen choices and its binary against a rebuild at HEAD, bit for bit.
**The chief could not verify the table from raw files:** the PI deleted the held-out run
directory after confirming its CSVs, following the chief's disk instruction. **Coordinator
error, recorded (seventh):** that instruction removed the evidence the chief's own review
depends on. The rule is fixed below. What the chief verified: the committed tables are
internally consistent with the report, and the merged tree passes every gate (fmt, clippy
`--locked`, 828 Rust tests, the oracle guard, 450 analysis tests). The merge's seam conflict
(B4's extended `Composed` arms against L1's `Learned` variant) was first resolved wrongly by
the chief (both arms kept, the stale one winning, two tests failing) and then correctly as B4's
file plus L1's four additions, with every gate rerun.

**What the table says** (200 held-out streams; the comparator is ramp + split over the
re-anchor; hard-incident quality with 90% intervals).

| Selector over the comparator's anomalies | Hard quality | Calls / stream | Cost s / stream |
|---|---|---|---|
| Selection oracle (labelled ceiling) | 0.562 [0.517, 0.608] | 2.6 | 0.79 |
| Public threshold rule | 0.570 [0.524, 0.616] | 26.4 | 7.95 |
| Public change rule | 0.556 | 25.5 | 7.56 |
| Always escalate | 0.562 | 27.0 | 7.81 |

Medium at 100 ms minus the comparator: under the oracle +0.030 [+0.000, +0.060] hard quality
for +1.05 s; under the threshold rule +0.027 [−0.003, +0.057] for +0.94 s [+0.79, +1.09]; under
the change rule −0.027 for −0.11 s. Leak quality is lower for the medium under every asking
selector (intervals exclude zero).

**What it means (objective, environment, failure).**

- **Public selection has no selectivity on this world.** The rung's checker contradicts about
  97% of plain anomalies (R5), so a rule that escalates "when the rung is contradicted or
  silent" escalates nearly everything: 90–98% of always-escalate's calls. The bill is 4–12× the
  oracle's and 79% of it is plain incidents, shared by every arm. A cost claim under these
  selectors would be a claim about the selector, not the noticer. This is R5's finding seen
  from the other side: selection is where the cost lives, and nothing public does it.
- **The selection oracle is not a quality ceiling.** Public selectors beat it on plain accuracy
  (+0.06 to +0.075) and critical misses, because it never asks about plain incidents. "Ceiling"
  holds for hard-incident quality only, and is labelled so from here.
- **The medium's extra anomalies are neither shown paid for nor unpaid for.** +0.027 of hard
  quality for +12% of cost, with the interval touching zero at n = 200.
- **Decoy and late-plain notices cost 4–6% of the bill** once charged; the ramp's own
  addition is about 2.4%. Leaks and decoys separate weakly after five readings (AUC 0.63 on
  the statistic tried), and the follow-up rule tuned to lose no leak on 100 tuning streams lost
  7 on held-out. The rule is not retained.
- **Feasible EXP-101 margins at n = 200:** hard quality about 0.05, cost about 5%, anchoring
  the public row minus 0.02. Not resolvable: 1% cost across noticer families, 0.02 quality, or
  the decoy cost itself.

**Decided.**

1. **Rule fix:** a lab keeps its held-out run directory and byte-identity directory until the
   chief has verified them; only tuning directories are deleted for disk. Recorded in
   `docs/local-test-plan.md`.
2. **EXP-101's cost axis is call-budgeted.** Arms are compared at a matched number of reasoner
   calls per stream (a public score ranks a noticer's anomalies; the top k per stream are
   asked), with k swept, so that what differs between arms is which anomalies they ask about,
   not how many. **B5 (Lab 2, after C1):** the call-budgeted selector, a delay sweep (the 16 s
   delay is a hidden selector), and the public score's feature AUCs for hard against plain at
   the ask instant.
3. EXP-101's primary measure is verified decisions (plain and hard) and critical misses per
   stream at a matched call budget, with hard quality and anchoring bounded; the draft comes to
   the user before the freeze, after B5's table sets the margins.

## L1 the learned noticer — merged (Lab 3); learning is real, fast, and stops short of the hand design

*Annotation 2026-10-07 (V1): the chief's own slope figure in this entry, +0.053 [−0.037, +0.085], has no recorded definition and was not reproduced by the criterion script; the lab's figure reproduces exactly and the clause verdict is the same under every reading. See the V1 entry.*

**Provenance.** Two interruptions (an API limit, then the disk restart); the final PI verified
the inherited state, rebuilt the release binary and found its hash equal to the one the main run
used, regenerated every analysis CSV from the run outputs byte-identically, and reran the
byte-identity gate (62 of 62, recomputed independently). The update rule and priors were
committed before any run; two amendments were made on development seeds before the main run
and are recorded. Gates under the runner on the merged tree: fmt, clippy `--locked`, 790 Rust
tests, the oracle guard, 417 analysis tests. Outputs in `artifacts/runs/l1/` (ignored). Not done:
mutation testing of the learner.

**Re-verified from the run outputs** (seeds 40000–40199, never used for tuning; chief's own
bootstraps): clause 1, learned minus frozen anchor-correct over the last 100 streams, 0.000
[−0.011, +0.010], holds; clause 3, learning-off minus frozen, −0.039 [−0.064, −0.015], holds;
clause 2, the slope of the cumulative curve over the first 100 streams, +0.053 [−0.037,
+0.085] (lab: +0.043 [−0.021, +0.126]), lower bound below zero, **fails**. End states match the
lab exactly: learned 0.980 anchor-correct at 8.76 background notices per stream; frozen 0.980
at 5.53; learning-off 0.941 at 31.55.

**Verdict as written: the learned arm does not count toward the aim** (the clauses are
conjunctive).

**What it means (causality, objective, failure, meta).**

- **Something was learned, from public structure alone.** The coincidence window goes
  400 ms → 100 ms → 49 ms → 30 ms within four streams, read from the excess over independence
  of same-service abnormal pairs by gap bin; the ramp threshold settles at 2.5. Against the same
  graph with learning off, learning is worth 22.8 fewer background notices per stream and
  +0.039 anchor-correct. The prior does not matter (50 ms and 1,000 ms priors reach the same
  end state). This is the first evidence in the program that a public statistic of the stream
  carries the knowledge the hand constants encode.
- **It stops short of the hand design, and pays in false notices.** The learner's 30 ms window
  against the hand-tuned 20 ms costs 3.2 more background notices per stream (8.76, above M2's
  6.82 bound) and 0.07 of strict precision. Anchor parity was bought by firing more. The
  background and precision bounds, not the clauses, show the learner is worse than the hand
  design.
- **The missing knowledge sits in one knob.** The learner's "precision demanded" parameter
  `p` decides where it stops: at 0.5 it stops at 30 ms and 2.5; at 0.8 (a pre-planned
  sensitivity arm) it recovers M2's 20 ms and 3.0 exactly, with the frozen graph's background
  and precision to the digit. So the hand tuning was moved up one level, not removed. The stream
  gives no feedback, so the learner has no way to set `p` from its own history; a public
  yield or cost signal (how many of its notices attach further evidence, or retire quickly) is
  what a stronger learner would need, and that is the design question the next learned unit
  must answer.
- **The criterion's slope clause cannot register fast learning.** The four medium arms make
  identical decisions on all 92 incidents in the first 50 streams, because the prior's cost
  only shows after the learner has already converged and the cumulative ratio has diluted it.
  A least-squares slope over 100 streams of a cumulative ratio is a poor instrument for a
  learner that finishes in four. **Coordinator error, recorded (sixth):** I fixed a clause
  whose statistic could not see the effect it was meant to detect at the speed the effect
  actually has; the feasibility note considered only whether a slope was measurable, not over
  what horizon. The clause stands for L1 as written; L2's criterion is amended below, before any
  L2 code.
- **One world, one seed, one setting, three differing incidents in 398.** The parity claim is
  exactly as thin as that.

**Decided.**

1. **L2's criterion is amended** (before any L2 code): clause 2 becomes "background notices per
   stream and strict precision against streams seen, with the endpoint fixed at stream 20: the
   learner's background over streams 21–40 is at most the learning-off control's minus 10, with
   the paired lower bound below −5", so that learning completed in a few streams is visible,
   and clause 1 adds the background bound (≤ 6.82) and strict-precision bound (≥ 0.67) as
   conditions, so that anchoring bought with false notices does not pass.
2. **L3 (Lab 3, after L2):** a learner that sets its own precision demand from a public yield
   signal, on a distribution-shifted stream (burst spacing and ramp rate changed from the
   defaults) where the frozen graph is wrong and there is something to catch up to; against a
   conventional online tuner (a grid search on the first N streams) as the matched
   conventional learner the charter's proxy 2 names. Criterion to be fixed before code, with the
   amended clause 2's horizon.
3. The L1 learner's window statistic is kept as the first public, label-free estimator of the
   world's burst spacing; it enters C1's engine as a derived relation if it proves useful there.

## B3 the public leak noticer and splitting noticer — merged (Lab 2); the status quo closes the leak too

**Provenance.** The unit was interrupted by the container restart after tuning and before the
held-out run; a fresh PI resumed in place, verified the inherited state, replayed the held-out
run from scratch at the tuning-stage commit, and reran the byte-identity gate with the final
binary (62 of 62, chief's hashes). The predecessor's last commit failed `cargo fmt --check` (one
hunk, fixed). cargo-mutants on both noticers: 134 of 150 caught after four added tests, 12
unviable, 3 missed by argument, 1 timeout. Gates under the cgroup runner on the merged tree:
fmt, clippy `--locked`, 774 Rust tests, the oracle guard, 402 analysis tests; peak build memory
under 3 GB, no OOM kills. Outputs in `artifacts/runs/b3/` (ignored).

**Re-verified from the per-incident notice files** (chief's own cluster bootstrap). B3's
comparator arms equal M2's modulo the run id (three arms, three files each), so M2 and B3 are
one table on the same 200 streams.

| Row | Anchor-correct | Leak noticed | Leak anchor-correct | Background / stream | Strict precision |
|---|---|---|---|---|---|
| Re-anchor (M2's comparator) | 0.952 | 0.460 | 0.000 | 6.82 | 0.672 |
| Ramp + split over re-anchor | 0.973 | 0.986 | 0.971 | 6.24 | 0.693 |
| Medium, 100 ms (M2) | 0.992 | 0.993 | 0.554 | 5.44 | 0.501 |

Medium minus ramp + split, paired: anchor-correct +0.019 [+0.005, +0.034]; leak noticed +0.007
[−0.014, +0.029]; leak anchor-correct −0.417 [−0.492, −0.343]; background −0.80; strict
precision −0.19. All match the lab's report.

**Verdict.** The acceptance clause triggers for the leak (0.986 ≥ 0.660 within the budget); M2's
runs had started, so this is the labelled supplementary comparison the clause names, recorded
in both reports.

**What it means (objective, failure, meta).**

- **A public rule that reads counter values notices the leak and anchors it at its start.** A
  chain of at least five readings rising by 15 above its first, with a tolerated step and drop,
  notices 137 of 139 leaks, 135 of them at offset exactly zero from the first observation, with
  a 5.5 s median latency. The objective saturated on tuning (175 configurations tie), so the
  constants are tie-rule choices, not an optimum. The two misses are chains that began on a
  background reading of the same counter a fraction of a second earlier: the mis-anchoring
  mechanism again, on the leak.
- **Splitting recovers 2 of the 8 never-noticed incidents** (both cascades), gains 8
  anchor-correct and loses none. Three split-brain incidents are noticed by no row.
- **The medium's standing after B2 and B3.** On every measure the medium was asked about, a
  public rule now reaches or beats it, except hard-incident anchoring (+0.019, interval above
  zero, at a 100 ms tick only) and background notices (0.8 fewer). It loses on leak anchoring by
  0.42 and on strict precision by 0.19, and costs more (1.85 s against 0.80 s per stream, partly
  because its own operations are billed and the public noticers' are not, mostly because it
  opens more anomalies). This is the third time the status quo, given a fair chance, has
  captured most of a lever (Stage B, B2, B3).
- **The ramp's shape is the decoys' shape.** It notices 0.948 of decoys (the re-anchor: 0.756),
  and the selection oracle never asks about decoys, so neither quality nor cost shows it. Of the
  510 notices the ramp adds per 200 streams, 137 are leaks, 154 decoys, 192 late plain, 11 hard,
  16 background. **The background budget counts only background-anchored notices, so it is
  blind to this.** The same blindness applies to the medium's rows. An honest cost column needs
  a non-privileged selector, so that decoy and late-plain notices are charged as calls.
- **Meta: what every noticing unit shares.** B2's gap, B3's chain constants and M2's graph were
  all tuned against evaluator measures on the tuning streams; each PI said so. None encodes a
  family or a hidden rule, and the guard passes, but all three are fitted to this world's burst
  spacing and ramp rate. Nothing yet shows that any of them transfers. That is the same for the
  medium and the public rules, so it does not favour either; it does mean EXP-101 on this world
  alone cannot speak to generality.

**Coordinator conclusion (sixth entry of its kind, not an error this time).** EXP-101 as
"noticing headroom" is closed on this world: anchoring is at 0.973 publicly with 0.027 of
headroom left, and the leak at 0.986. A preregistered noticing experiment here would be
saturated before it froze. What remains open, and what the charter's aim actually asks, is
(a) cost and precision at equal noticing, charged honestly, and (b) improvement per experience.
EXP-101 is therefore reframed before registration, below.

**Decided.**

1. **M3's criterion is not re-fixed** (result 1's +0.03 over 0.973 would be unreachable; M3 asks
   whether sub-tick pruning recovers the long ticks, against the re-anchor as fixed). The B3
   rows are reported beside with paired differences; Lab 1 was told.
2. **EXP-101 is reframed:** escalation control at matched noticing. Primary measures: critical
   misses and verified decisions per unit cost with a **non-privileged selector** (a public
   threshold or cascade over the noticer's anomalies, so every notice the noticer makes is
   charged), under the δ sweep, with both aim proxies; arms: the medium and the public
   ramp + split over the re-anchor, each with the same selector; the rung as the floor; the
   notice oracle as the ceiling. Noticing shares are bounded (no arm may fall below the public
   row's anchor-correct minus 0.02) but are not the claim. The draft comes to the user before
   the freeze.
3. **B4 (Lab 2):** the non-privileged selector (public threshold and change-triggered rules
   over any noticer's anomalies, tuned under cost), a decoy-notice column in the evaluator, and
   leak-versus-decoy separation from readings after the first five, so that a noticer's decoy
   cost is measured. This is the instrument EXP-101 needs.
4. **L1 stands as queued:** whether the medium learns is now the medium's main claim.

## M2 the medium as a noticer — merged (Lab 1); both results hold at 100 ms, not at 500 ms or 2 s

**Provenance.** The graph was frozen at `c5bcb11` before any held-out run; the comparator
re-fix reached the lab after two tuning stages and before any held-out run, and the lab retuned
to the new budget. R6's held-out run replays byte-identical for all 62 arms with the medium
wired in (chief's hashes). `ReanchorNoticer`, rerun in the lab's held-out run, reproduces B2
exactly. The medium's bill charge equals an exact replay at every tick length. One driver
refusal (a wrong manifest path), nothing ran. Gates on exit codes on the merged tree: fmt,
clippy `--locked`, 731 Rust tests, the oracle guard, 389 analysis tests. Outputs in
`artifacts/runs/m2/` (ignored).

**Re-verified from the per-incident notice files, all 17 arms, paired against the re-anchor**
(chief's own cluster bootstrap):

| Tick | Result 1: anchor-correct | Result 2: leak noticed | Background / stream (bound 6.82) |
|---|---|---|---|
| 100 ms | 0.992 vs 0.952, +0.040 [+0.021, +0.060], holds | 0.993 vs 0.460, +0.532 [+0.462, +0.603], holds | 5.44 |
| 500 ms | +0.030 [+0.010, +0.050], not shown (0.0004 under the margin) | +0.525, holds | 5.33 |
| 2 s | −0.043 [−0.075, −0.011], worse | +0.540, holds | 4.29 |

All match the lab's report. **Verdict as written: both results hold at the best tick length,
100 ms, with the sweep shown.** At 100 ms the medium anchors correctly all 18 incidents the
re-anchor misses and misses 3 the re-anchor gets right; none is wrong for both.

**Adversarial reading, and what survived it.**

- **Hidden knowledge.** `graph.rs` and `noticing.rs` name no family, vocabulary or hidden
  timing; the guard passes; the lab did not read the hidden record and designed from public
  dumps of the tuning streams and the evaluator's notice files on them. The constants (a 20 ms
  coincidence, 100 ms ticks) were tuned against evaluator measures on tuning streams, the same
  route B2's gap took. Accepted, stated.
- **Edge of the tuning budget.** The 100 ms selection sat at 5.58 against a tuning budget of
  5.6, with a tie-break reading added after the tuning tables. On the held-out streams it sits
  at 5.44 against 6.82, well inside. Accepted.
- **Hooks outside the territory**: a `Medium` variant in the seam, passthroughs, and a billing
  hook, all inert for other noticers (the byte identity shows it). Accepted.
- **Not done:** mutation testing of the M2 code. Queued into the next medium unit.

**What it means (mechanism, failure, time, objective).**

- **The medium wins by reading inside the tick.** The rung family scores an 8 s window and then
  re-anchors; the medium fires on two or three kinds of abnormal observation at one service
  within 20 ms, read from `offset_ns` by the ordered coincidence, with a lookback of zero that
  cuts its support at the tick edge. A stray just before a burst stays out unless it shares the
  burst's tick: rare at 100 ms (it wins), common at 2 s (it loses 31 incidents). The lookback
  table is the same mechanism in one variable: 0.992 at 0, 0.930 at 1 s. **The anchoring rule
  M1 built works when the support is short, and the tick is the support's floor.** Sub-tick
  pruning of the support is the obvious next medium unit (M3).
- **The ordered coincidence earned its place at every tick length**, as the M1b PI predicted:
  it buys background, not recall (the sliding form anchors as well but breaks the bound: 14.7,
  30.3, 22.7 per stream). The binned-by-rhythm form lost (0.976 at 8.59). Phase gates and
  oscillators found no role in this world; the result says nothing about them.
- **The leak.** The medium notices 0.993 of leaks (comparator 0.460) and anchors 0.554 of them
  at their start (every public noticer: 0.000), with a median latency of 9.8 s against 14.2 s.
- **The chief's synthesis premise was wrong, and the control shows it.** I wrote that an
  abnormal-only sense adapter would make result 2 unreachable by construction. The
  abnormal-only medium notices 1.000 of leaks: the ramp continues above the alarm line as dense,
  smooth readings, and the detector finds it there. Reading benign values buys anchoring the leak
  at its start and about 9 s of latency, not noticing. Result 2 tests the ramp detector, not the
  reading of benign values. Recorded as a coordinator error of reasoning (the fifth): I
  deduced a construction from the rules without checking what the rules leave above the line.
- **The cost column cuts the other way.** The medium's own operations are 3.9–16.7 ms per
  stream, under 1% of the arm's cost. But the medium opens more anomalies per incident (1.65
  against 1.04; strict precision 0.50 against 0.67), and under the selection oracle each one is
  a reasoner call, so the arm's total cost is 1.8–3.8 s per stream against 0.67 s. Better
  anchoring, bought with more escalations. EXP-101 must read noticing and cost together, and the
  medium's next unit must bring strict precision to the re-anchor's level or show why not.

**For the aim.** The first positive medium result: on the one lever the status quo could not
close with a tuned rule, the medium closes it at the short tick, and it does so with a
mechanism the rung does not have (event order inside a tick). Hand-designed, one world, one
setting, more reasoner calls. It is a mechanism, not yet an intelligence, and it has not yet
learned anything.

**Decided.**

1. **M3 (Lab 1):** support pruned below tick resolution, so the 100 ms behaviour survives at
   500 ms and 2 s; mutation testing of the M2 code; strict precision as a tuning constraint
   alongside the background bound. Criterion: result 1 at 500 ms and 2 s under M2's margins,
   with strict precision ≥ 0.67 (the re-anchor's) as a bound.
2. **EXP-101 preregistration** is drafted now, on the stream world at the primary setting with
   the δ sweep: noticing as the primary function (anchor-correct and leak noticed against the
   best public noticer from B3, under the background and strict-precision bounds), selection
   and cost as the secondary measures, critical misses and plain accuracy bounded, both aim
   proxies per arm. The draft comes to the user before the freeze.
3. **The learned noticer (Lab 3)** is queued: the medium's graph with its tuned constants
   replaced by parameters learned online from the stream's public history, against the frozen
   hand-designed graph and the re-anchor on the same stream order. First reading of aim
   proxy 2.

## B2 the public later re-anchor — merged (Lab 2); the status quo closes most of the anchoring gap

**Provenance.** R6's held-out run replays byte-identical for all 62 arms, twice (chief's hashes).
cargo-mutants: 78 of 80 caught on the evaluator (2 unviable), 33 of 35 on the new noticer (2
unviable), 20 of 20 hand mutants. Two tuning amendments were made after the first tuning run
and before any held-out run (grid extended downward; a tie rule), both recorded with the first
run's files kept. Gates on exit codes on the merged tree: fmt, clippy `--locked`, 713 Rust
tests, the oracle guard, 389 analysis tests. Outputs in `artifacts/runs/b2/` (ignored).

**Re-verified from the per-incident notice files.** `ReanchorNoticer` (site isolation 20 ms,
burst ≥ 2, z = 2): anchor-correct 0.952, leak noticed 0.460, 6.82 background notices per
stream; the rung at z = 2: 0.914, 0.460, 9.03; paired difference +0.038 [+0.021, +0.055] with
the chief's own cluster bootstrap. The held-out-best configuration within the budget is the
tuning-selected one, so the number is not selection luck.

**Verdict.** The acceptance clause triggers: the public later re-anchor reaches the 0.944 bar
within the budget (point estimate; the interval's lower end, 0.932, does not, and 200 streams
cannot say more). **M2's comparator is re-fixed to `ReanchorNoticer`** before any M2 held-out
run; Lab 1 was told directly.

**What it means (objective, failure and meta perspectives).**

- **A one-line public rule closes most of the mis-anchoring gap.** "If nothing follows the
  anchor within 20 ms and a burst begins later, move the anchor to the burst" gains 19
  incidents and loses 1 at the rung's own threshold, with fewer false notices. Of the gap R10
  measured, what remains for anything cleverer is 18 of 372 incidents: 10 anchored late (1–52
  s) and 8 never noticed. On burst families the status quo, tuned, has reached the noticing
  ceiling in quality terms: with the retirement confound removed, the re-anchor scores 0.591
  against the injected-notice oracle's 0.586 on the same streams (+0.005 [−0.028, +0.038]).
- **The leak is where noticing still has measured value**, and the re-anchor does not touch
  it: 0.460 noticed, 0.000 anchor-correct for every public noticer. R10's 0.75 lever stands.
- **The retirement confound is real and small for the rung family** (+0.008 to +0.040), and
  it is the whole story for floods (0.000 → 0.62–0.70 with the hold, bought with 57–63
  reasoner calls per stream). Neither quality reading measures noticing alone, which is why M2
  decides on noticing.
- **Precision as specified rewards floods** (0.56–0.60), because most abnormal observations
  belong to incidents. Strict precision (anchor- and site-correct notices over all notices:
  0.02 for floods, 0.67 for the re-anchor) and notices per incident expose them; both join M2's
  reported-beside list.
- **The 20 ms gap is a constant of this world's burst spacing.** The PI says so; the rule is
  close to "nothing follows the anchor immediately". One world, one setting. The medium's
  version of the same idea at a 100 ms tick is the fair comparison (B2's sensitivity rows: 0.935
  at 100 ms, 0.927 at 200 ms, 0.900 at 400 ms).

**For the aim.** This is the program's second instance of the status quo, given a fair chance,
capturing most of a lever (Stage B was the first). It is the falsification rule working as
intended, and it sharpens M2: the medium is now asked to beat the best public noticer, not the
default one. Its remaining claims are the 18 incidents, the leak, and cost.

**Decided.**

1. M2's comparator: `ReanchorNoticer`, anchor-correct 0.952 [0.932, 0.970], leak noticed 0.460,
   background bound 6.82 per stream. Margins unchanged: result 1 needs ≥ 0.982 (paired lower
   bound > 0.962), feasible only by fixing 11 of the 18 remaining incidents, and stated so.
2. Strict precision and notices per incident are reported beside M2's results.
3. **B3 (Lab 2):** the status quo gets its fair chance at the leak too: a public noticer that
   reads benign values (a per-node ramp or trend detector on saturation-type counters, tuned
   under the background budget), plus a splitting noticer for the 8 never-noticed incidents.
   If B3 reports before M2's held-out runs, the comparator for result 2 is re-fixed; otherwise
   M2 is read against B3 afterwards as a labelled supplementary comparison, and that is said in
   both reports.
4. The hold option stays off in every comparison row; the "held" quality reading is reported
   beside, never in a criterion.

## M1b the oscillome — merged (Lab 1); one benchmark point outside the band, accepted with a decision

**Provenance.** The all-off identity: `tests/m1_identity.rs` was committed on M1's code
(`a232cf5`) with two pinned digests over 310 specs; the chief reran it at that commit (M1 code,
no `oscillome.rs`) and at HEAD; both pass and the file is unchanged between them. Gates on
exit codes on the merged tree: fmt, clippy `--locked`, 686 Rust tests, the oracle guard, 377
analysis tests. 23 mutations, 22 caught; the one not caught (decay through `f32::exp`) gives
identical bits on this platform and is recorded. Benchmark data in `artifacts/runs/m1b/`.

**Verdict.** Accepted. One acceptance point fails: the smallest routing-only workload measures
1.42–1.43 of its model against a band of 0.7–1.4. The PI diagnosed two causes, an unpriced
fixed cost per tick (about 85 ns idle, about 190 ns with the engine on) and an 18–20% slower
routing path with the larger tick function, verified by alternating M1's and M1b's binaries
under one cgroup.

**Decided on the miss.** A per-tick fixed price of **200 ns** is charged by the ledger adapter
in M2 (a declared manifest price, like the others) so that a medium that ticks is billed for
ticking: 6,000 ticks per 600 s stream at 100 ms is 1.2 ms of modelled cost, against the rung's
0.66 s. The routing slowdown is not optimised now; this tick stays the reference and any faster
one is calibrated against it. The band is met at every other point (46 of 48 measurements, the
headline 0.89–1.22).

**What it means (time and structure).**

- **Times in seconds survive the tick sweep, with stated losses.** The conversion table shows
  which delays collapse to one tick (everything under 150 ms at 500 ms and 2 s) and which are
  refused (over 255 ticks). A design that wants a 100 s delay at a 100 ms tick uses a rhythm,
  not a synapse delay. That is the oscillome doing what it was added for.
- **The PI's prediction for the ablation is on record:** the global rhythms, phase gates and
  binned coincidence will be removed; the ordered coincidence (event order from `offset_ns`
  inside a long tick) is the element most likely to earn its place, at 500 ms and 2 s only;
  oscillators matter for the 6–16 s horizon, not first-tick anchoring; retirement lets the end
  of an anomaly reach B1's seam without a heartbeat. The chief agrees with the direction and
  records it so the ablation cannot be read post hoc.
- **A hazard accepted:** parameters M1 documented as unused now carry meaning; decoding refuses
  stored conversions that disagree with their quantities. No M1 spec exists outside the tests.

**M2 is unblocked.** M1b and B1 are merged; the M2 brief follows in `docs/lab-queue.md`.

## Synthesis after W1, M1, B1 (chief; the perspectives of AGENTS.md "Multidimensional analysis")

**Structure.** The three units compose at one point: B1's `Noticer` seam is the rung's noticing,
M1's effector port emits notices and (M1b) retirements, and W1's tick statistics size the sense
adapter between them. The medium therefore plugs into the status quo at exactly the place where
the status quo's measured error lives (mis-anchoring), with everything downstream shared and
byte-identical. That is the cleanest comparison this program has had.

**Causality.** Mis-anchoring is now identified from three independent sides: R10's ledger
replay, B1's evaluator instrument (17 of 31 never-noticed incidents have a background notice
within 1 s before them), and M1's own anchoring rule, which has the mirror-image risk (it prunes
the starting event and moves the anchor later). The mechanism is: a stray at or near the site
opens an anomaly a fraction of a second early; the incident's burst attaches to it; the anchor
never moves. The fix is a later re-anchor, which no public noticer makes (B2 builds one) and
which the medium's "earliest contributing event" rule makes only if its support keeps the right
events. M2 is, concretely, a test of two anchoring rules against one measured failure.

**Time.** W1 showed one tick cannot serve burst order (20–150 ms) and the incident horizon
(6–16 s), and that cost follows events, not ticks. The oscillome (M1b) is the structural answer;
the all-off identity test keeps it falsifiable.

**Objective.** The aim's proxy 1 is cost in disguise unless its denominator is hard decisions;
proxy 2 has an instrument and nothing to read until a learner exists; anchor correctness is
gameable by flooding and is sound only under a background budget. Every M2 number is therefore
reported under a bound, and the lever the medium is asked to move (anchoring) is one the public
arms demonstrably cannot.

**Failure.** Four found this round: floods, the earlier-anchor fix degrading with lookback,
silence not propagating in a sparse medium, and pass-limit latency. Each has a decision on
record.

**Meta: the assumption all three labs share.** Every instrument and arm defines signal by the
public rules' "abnormal" verdict: W1 copies the rule, B1's noticers see only abnormal
observations, and a sense adapter that tags only abnormal events would hand the medium the same
blindness. The slow leak is benign under that rule until it crosses the alarm line, which is why
its anchor-correct share is 0.000 for every public noticer and why R10's leak lever (0.75) is
the largest in the program. **Decision for M2:** the sense adapter delivers every observation
with its value, benign readings included, so that a `Novelty` or `Integrator` cell can read a
ramp. If the medium's leak result holds, it holds because it reads what the rules discard; if
the adapter were abnormal-only, result 2 would be unreachable by construction, which the chief
would otherwise have discovered after the run.

**Trajectory.** M1b → M2 (with B2 in parallel) → EXP-101 preregistration (noticing first,
leak as a named secondary, δ sweep, both aim proxies) → the learned noticer against a
never-learning control, which is the first reading of proxy 2 and the falsification test for
the cell model's claim to sample efficiency.

## B1 the Noticer seam, notice measures, public noticing baselines — merged (Lab 2)

**Provenance.** R6's held-out run replays byte-identical for all 62 arms with the seam, twice
(seam commit and final binary; hashes recomputed by the chief), and R10's 6 arms replay too. The
notice record is three new files per arm; `results.csv` and `incidents.csv` keep their columns,
pinned by a test. 29 evaluator fixtures; cargo-mutants 39 caught, 2 unviable, 0 missed; 25 hand
mutants caught. Gates on exit codes on the merged tree: fmt, clippy `--locked`, 652 Rust tests,
the oracle guard, 377 analysis tests. One driver refusal (unclean tree), nothing run; outputs in
`artifacts/runs/b1/` (ignored).

**Re-verified from the per-incident notice files** (200 held-out streams, 372 hard non-leak,
139 leak):

| Noticer | Noticed | Anchor-correct | Leak noticed | Background notices / stream |
|---|---|---|---|---|
| Rung z = 3 (default) | 0.917 | 0.890 | 0.460 | 4.00 |
| Rung z = 2 | 0.946 | 0.914 | 0.460 | 9.03 |
| EarliestAnchor l = 0.25 s | 0.917 | 0.890 | 0.460 | 4.01 |
| ChangeTriggered q = 128 s | 0.032 | 0.032 | 0.000 | 8.76 |
| ChangeTriggered q = 0.5 s (sensitivity, a flood) | 1.000 | 0.984 | 1.000 | 587.8 |

All match the PI's table.

**What it means (failure and objective perspectives).**

- **The rung's noticing gap is now measured without ledgers:** 8.3% of hard non-leak incidents
  never noticed, 2.7% noticed with a wrong or late anchor. Of the 31 never-noticed, 17 have a
  background-anchored notice within 1 s of their first observation (chance: about 1%). R10's
  mis-anchoring finding is confirmed by an independent instrument.
- **The cheapest public fix does not work.** Moving the anchor earlier (`EarliestAnchor`) makes
  anchoring worse as the lookback grows (0.890 → 0.422 at 8 s), because an earlier anchor lands
  on same-site background strays. The failure R10 found needs a *later* re-anchor onto the
  incident, which no public noticer here can make. That is a precise target for the medium.
- **Threshold tuning saturates early.** Anchor-correct peaks at z = 2 (0.914) and falls by
  z = 0.5 while background notices go 4 → 60 per stream.
- **Change-triggered noticing cannot be tuned into the rung's background budget.** At q ≤ 1 s it
  notices everything at 500+ background notices per stream; at the budget it notices nothing.
  The charter's change-triggered baseline is answered at the noticing level: it is a flood or
  it is blind.
- **The anchor-correctness measure is gameable by flooding**, and the PI showed it: a flood is
  0.984 anchor-correct. The measure is sound only under a background budget, which is why M2's
  criterion carries one. The leak is 0.000 anchor-correct for every abnormal-only noticer,
  because its early readings are benign under the public rules; a noticer that anchors a leak
  at its start must read the ramp, not the alarm.
- **The quality column is confounded** by the selection oracle's fixed 16 s delay and the rung's
  6 s retirement: a flood's anomalies die before they are asked about. M2 reports quality but
  decides on noticing, as its criterion says.

**Coordinator error, recorded (fourth).** M2's criterion named "the best public noticer by that
result's own measure" as the comparator. B1's sensitivity rows show that reading admits the
flood, under which both results are unreachable. The lab's budget rule kept the flood out of its
table, but the criterion should not have depended on a lab's tuning rule. Amended below, before
any M2 code or run. Lesson, added to the earlier three: a comparator must be named by a rule
that cannot select a degenerate arm, and the feasibility check must include the arms that game
the measure.

**Decided.**

1. **M2's comparator is fixed as `RungNoticer` z = 2** from B1's table: anchor-correct 0.914
   [0.890, 0.937], leak noticed 0.460, 9.03 background notices per stream. The medium's
   background notices per stream may not exceed 9.03. Result 1 needs anchor-correct ≥ 0.944
   with the paired lower bound above 0.924; result 2 needs leak noticed ≥ 0.660 with the lower
   bound above 0.560. Power: a +0.03 paired gain is 11 incidents of 372; feasible, tight, and
   stated.
2. **Reported beside M2's results:** leak anchor-correct (0.000 for every public noticer; a
   medium that anchors a leak at its ramp will show here first), notice precision (share of
   notices anchored on an incident), notices per incident, latency, and the quality column with
   its confound named.
3. **Lab 2's next unit, B2:** the evaluator gains a site check and a notice-level precision
   measure; the selection oracle gets a notice-relative delay option so quality measures
   noticing; and a public *later re-anchor* noticer (re-anchor a background-anchored anomaly
   onto the incident whose evidence it attaches) is built as the strongest public comparator for
   exactly the mis-anchoring the medium targets. If that public fix closes the gap, the medium's
   first job is already done by the status quo, and that is a result.
4. **Workers' scratch files:** a PI overwrote another's helper in the shared scratchpad. Each
   lab now uses its own subdirectory, named by its worktree, under the session scratchpad.

## M1 the medium crate — merged (Lab 1); prices calibrated at 10× the design's guess

**Provenance.** Gates on exit codes on the merged tree: fmt, clippy `--locked`, 626 Rust tests,
the oracle guard, 354 analysis tests. The crate forbids `unsafe`, has no hash map, I/O or
transcendental call (grepped), and depends on `gordian-core` and `serde` only (`serde_json`,
`proptest`, `criterion` as dev-dependencies). `Cargo.lock` gains one entry. The lab's
benchmark data is kept in `artifacts/runs/m1/` (ignored).

**Re-verified by the chief.**

- **A mutation of my own.** Removing the within-tick event sort makes
  `events_are_summed_in_offset_order` fail. The determinism tests test the claim; the lab's
  eight mutations are credible.
- **The benchmark**, rerun under the cgroup on cores 0-2 (67.7 s wall, 65.5 s CPU, 64.5 MB
  peak): headline tick 7.1 / 7.8 / 8.7 × the declared prices at 10 / 100 / 1,000 active cells;
  least-squares prices 201 ns per cell update, 19 per traversal, 40 per routing, 0.6 per field
  read. The lab measured 202 / 22.8 / 42.3. The proposed prices (200, 25, 40, 2) put every
  measurement within 0.78–1.28 of its model.

**Verdict.** Accepted. The acceptance allowed a miss of the factor-of-5 price bound if new
prices come with the measurement, and they do.

**Decided on prices.** The calibrated prices are the declared prices from here: M2's manifests
declare 200 / 25 / 40 / 2 ns, and M1b updates `Prices::DECLARED` with a test. This version of
the tick stays the reference implementation; any faster tick is calibrated against it (AGENTS.md,
"Keep a simple reference implementation as an oracle"). A medium cell update at 200 ns is about
a tenth of a cheap component call (median 1,743 ns) and 10⁻⁵ of a reasoner call; cost is not in
M2's criterion, but these are the numbers its cost column will carry.

**The PI's three warnings, and what the chief decided (structure and failure perspectives).**

1. **Silence never propagates.** Cells run only with input, so the end of an anomaly reaches
   nobody, and B1's seam needs retirements. Decision for M2: a heartbeat event per tick from the
   clock adapter, priced like any event (one routing, one update per tick), not a time-out in
   the effector. A time-out in the adapter would be anchoring logic outside the medium, which
   section 2 forbids.
2. **The anchoring rule as built prunes the starting event.** An accumulating cell's lookback
   bounds what it cites, not what it sums, so the event that began an anomaly is the one most
   likely to fall out of the support, which moves the anchor later. That is the rung's failure
   in a new form, and the PI is right to flag it. M2 must treat the lookback as a tuned
   parameter per tick length, report anchor correctness as a function of it, and the
   `EarliestAnchor` baseline from B1 is the fair comparator for exactly this. If anchor
   correctness is bounded by pruning rather than by the graph, that is M2's first finding.
3. **Pass-limit latency.** With `max_passes` 2 a sense → integrator → emitter chain emits one
   tick late. M2 uses `max_passes` 3 and reports latency at each tick length.

**Departures accepted**, all 33 recorded in the crate's `DESIGN.md`. The material ones: event
references carry their offset; cells carry a bounded support of references (a new hard limit)
and messages carry the sender's support; archetypes receive the ticks since they last ran so
that cells without input truly do not run; "counts monotone in events" is true only in its weak
form because inhibition exists, and the weak form is what is tested.

**Rhythms.** The PI did not fold the oscillome into M1, for reviewability, and proposed a
bounded M1b with an acceptance. Accepted: judging two designs under one acceptance would have
been the chief's error. M1b is queued to Lab 1 with the PI's acceptance and the open questions
of section 4b resolved in the queue entry.

**Resource-rule violation, recorded.** The PI's first build overlapped a Lab 2 byte-identity
run (a check, not a timed measurement). It disclosed it and guarded every later call. No
measurement was affected.

## W1 tick statistics and the aim's measures — merged (Lab 3)

**Provenance.** The example reruns byte-identical under the cgroup on the chief's own invocation
(five CSVs). Gates on exit codes on the merged tree: fmt, clippy `--locked`, 588 Rust tests, the
oracle guard, 354 analysis tests. The chief added the `[[example]]` wiring in the crate's
`Cargo.toml`, outside the lab's territory, as the PI asked. Run usage in `artifacts/runs/w1/`.

**Re-verified.** Sample efficiency and energy ratios follow from R10's numbers the chief already
checked: `notice_rung` 348/511 = 0.681, the oracle 480/511 = 0.939; ns per correct hard decision
equals cost per stream × 200 / correct hard, to three figures, for every arm checked.

**What it means for the medium (time perspective).**

- **The two timescales are real and a tick cannot serve both.** At 100 ms a contradicting
  burst's partner alarm lands in a later tick 79% (cascade) and 63% (split brain) of the time,
  so order survives; at 500 ms about five in six partner pairs share a tick; at 2 s order is gone
  (97–100% same tick). The leak is tick-indifferent. `offset_ns` inside the tick is therefore
  not a nicety: at 500 ms it is the only carrier of burst order.
- **Background swamps the long tick.** The share of (node, tick) cells with two or more abnormal
  observations is 0.12% at 100 ms and 15% at 2 s. A "two abnormal at a node" rule fires 446
  times per stream at 2 s against 75 at 100 ms. Thresholds on counts cannot be shared across
  tick lengths; M2 tunes per tick, and delays and decays are stated in seconds, not ticks.
- **Decisive evidence is never in the first tick**, by construction: hard incidents present for
  6–16 s before their decisive phase. A noticer's job in the first tick is anchoring, not
  deciding; the design's "earliest contributing event" rule is aimed at the right moment.
- **"Cost inverse in tick length" was wrong as stated.** Routed events are constant; active
  (node, tick) cells fall only 4,261 → 2,175 across a 20× change in tick. Only per-tick fixed
  work scales inversely. The medium's cost is driven by events, not ticks, which is what a
  sparse design should show; `docs/medium-ports.md` section 8 is corrected below.

**What it means for the aim (objective perspective).**

- **Cost per correct decision over all incidents is cost in disguise** (Spearman 1.00 with cost),
  because plain incidents dominate the denominator and every arm gets them equally. The proxy
  that discriminates is per correct *hard* decision, and even that is near cost order where
  quality is flat (R9: 0.98). Decision: preregistrations report both, with the hard-decision
  denominator as the aim's proxy 1.
- **Joules rank exactly as modelled nanoseconds** because the reasoner is 99.87% or more of
  every reasoner arm's cost. The placeholder watts (10 W cheap, 1,000 W reasoner) matter only
  for `never_escalate`. The conversion stays a labelled placeholder until measured.
- **Sample-efficiency curves of fixed policies are flat**, as they must be. The instrument is
  built; it has nothing to read until a learning arm exists (aim proxy 2). The PI's next test,
  an online public noticer against a never-learning control on the same stream order, is the
  right one and is queued after M2.

**Accepted with notes.** `is_abnormal` is a copy of the rung's rule because the stream crate
cannot depend on the harness; equality is by reading. Same-tick shares assume a grid starting at
zero; phase is not swept. Group counts are 56–140 without intervals.

**Decided.** `docs/medium-ports.md` section 8: replace the cost expectation with W1's finding;
require `offset_ns`-aware archetypes at 500 ms and above; tick sweep stays {100 ms, 500 ms, 2 s}.

## R9 grounded distractor penalty — merged; the world's look-alikes cost a strong reader little

**Provenance.**

- The reader was frozen at `cccbcf2` (tag `r9-reader-frozen`) before any evaluation number;
  the evaluation commit `a16f839` follows it. Development on seeds 29000–29999, evaluation on
  30000–30399.
- Three tuning runs were refused once by the driver because the R10 worker's `cargo test` was
  running; they wrote nothing and were rerun. The core-sharing rule worked as intended.
- The ceiling and R4's oracle are byte-identical to R6's in all six runs.
- Gates on exit codes on the merged tree: fmt, clippy `--locked`, 576 Rust tests, the oracle
  guard, 339 analysis tests. Run outputs moved to `artifacts/runs/r9/` (ignored).

**Re-verified from raw files.**

- Reader on 312 hard evaluation questions: A(m) = 0.958, 0.939, 0.904, 0.913, 0.933 at
  m = 0, 50, 100, 200, 400; control 0.045; δ* = 0.0137 [0.0054, 0.0223] with the coordinator's
  own 2,000-resample cluster bootstrap (worker: 0.0135 [0.0055, 0.0225]).
- Rerun, tuning-selected builder, G = ceiling − builder:

  | δ | Selected builder | G [90%] |
  |---|---|---|
  | 0.0055 | `window` 80 s, N 512 | 0.032 [0.011, 0.054] |
  | 0.0135 | `cooccur` 16 s or `neighbourhood` k 4 (tie at 0.734) | 0.073 [0.046, 0.099] |
  | 0.0225 | `neighbourhood` k 4, N 256 | 0.089 [0.059, 0.118] |

  All match the worker.

**Verdict as written: unresolved.** The R6 regime needs G's upper bound below 0.10 at δ*_hi
(it is 0.118); the R7 regime needs G ≥ 0.10 at δ*_lo (it is 0.032).

**Coordinator error, recorded.** The feasibility note said the R6 regime needs roughly
δ*_hi < 0.05. It needs about 0.012, because G(0.05) was already 0.097 in R7. I read the regime
boundaries off R7's grid without interpolating. The criterion stands as written; the outcome is
unresolved on its own terms, and the error is in the feasibility note, not the clauses.

**What it means.**

- **In this world, look-alike distractors cost a strong reader very little.** A reader that
  knows the hidden rules and has learned from labelled development questions loses at most
  0.054 of accuracy at any distractor count up to 400, and recovers to 0.933 at 400. R8's crude
  reader lost 0.57. The world's confusability is real but small for a competent reader.
- **The curve is not exponential in count.** It dips at m = 100 and recovers, because random
  draws from the ±40 s pool include the incident's own non-decisive observations, which help
  (a split brain's burst). δ* is a summary, not a law, and the rerun turns it into a
  count-based penalty that the reader's curve does not quite obey.
- **Read as a point estimate, the context lever is modest.** At δ* the perfect-context ceiling
  beats the best simple builder by 0.073 [0.046, 0.099] on hard incidents, at 36× the references.
  That is more than R6's 0.024 and less than R7's "fragile" 0.14–0.34. EXP-102 can claim both a
  small quality margin and a large reference saving, with the quality margin's interval reaching
  from 0.03 to 0.12 across δ*'s interval.
- **Count-based penalties transfer to builder contexts, not to the rung's.** On R6's actual
  contexts the reader is within 0.05 of the curve for `window`, `cooccur` and `neighbourhood`.
  On the rung's own context it reads 0.659 against a predicted 0.953: the rung's context lacks
  the cascade's partner, which is missing information, not distraction. The simulated reasoner's
  `q` already models that loss.
- **What δ* is not.** It is the loss of one program with the hidden rules and learned weights
  on this world's questions. It says nothing about real models (R8 could not), and a stronger
  reader would give a smaller value, as every version during development did.

**Accepted with notes.**

- The reader learned softmax weights from labelled development questions. The plan allowed
  the hidden rules and forbade labels at evaluation; it did not say whether labelled training
  was allowed. The coordinator accepts it: the reasoner is the hidden side's stand-in, and a
  model trained on labelled diagnoses is what it stands in for. The reader is therefore stronger
  than a rule-only reader, and δ* is smaller for it.
- Six of 170 direct-check calls had an anchor more than 0.1 s after onset and the reader was
  wrong on all six. Its first-alarm-is-onset assumption is a weakness, post hoc.
- The tuning-selected builder is an unstable comparator at small δ: two builders tie at δ*, and
  the held-out-best builder would pass the R6 clause's upper bound at δ*_hi (0.097). The
  criterion is kept as written.

**Decided.**

1. **The reasoner's default for every preregistration is δ = 0.0135**, with the sweep
   {0, 0.0055, 0.0225, 0.05} required beside it, so that every claim is read across the interval
   and at the old default. This replaces the free δ = 0 default in the reasoner's spec for
   experiments, not in the code (the code's default stays 0 so that R6 and R7 replay).
2. **EXP-102's claim is references and cost at matched quality, with a secondary quality margin**
   against the best public builder, preregistered at 0.05 with the δ sweep. The worker's
   "fewest references within 0.05 of the ceiling" reading is the primary measure.
3. **The simulated reasoner's distractor penalty remains a count-based stand-in.** A
   composition-based law (penalising look-alike free-form messages rather than every
   reference) would fit the reader's curve better. It is not built now: the count-based law
   with the measured δ is within 0.05 of the reader on every builder context, and a new law
   would need a new byte-identity gate and would reopen R7. Recorded as a known weakness of
   the world, with the direct-check table as the evidence for revisiting it.
4. With R5, R6, R7, R9 and R10 done, the simulation-side headroom work for EXP-101 and EXP-102
   is complete. The next unit is EXP-101's preregistration.

## R10 salience ceiling — merged; noticing is worth 0.10 on burst families and 0.75 on the leak

**Provenance.**

- The new privileged arm reads two plan fields, `hard` and `first`, and nothing else. The
  coordinator grepped the diff for every plan accessor.
- R6's held-out run at b = 5, ρ = 0.7 replays byte-identical with the new binary: 62 of 62 arms,
  hashes recomputed by the coordinator. The R10 comparison arms equal R6's own files once the
  `run_id` column is dropped.
- 8 driver runs and 5 replays, all exit 0, none refused or excluded. No waits on the other
  worker.
- Gates on exit codes on the merged tree: fmt, clippy `--locked`, 572 Rust tests, the oracle
  guard, 326 analysis tests. Run outputs moved to `artifacts/runs/r10/` (ignored).

**Re-verified from raw files, rung's own context, 90% paired cluster bootstrap.**

| Setting | Result 1: hard, no leak | Result 2: slow leak |
|---|---|---|
| b = 5 (primary) | 0.586 − 0.489 = +0.097 [+0.062, +0.132], holds | 0.935 − 0.187 = +0.748 [+0.678, +0.814], holds |
| b = 2.5 | +0.024 [−0.013, +0.060], not shown | +0.324 [+0.252, +0.400], holds |
| b = 8 | +0.099 [+0.063, +0.136], holds | +0.777 [+0.709, +0.841], holds |

All match the worker's report to 0.001.

**Verdict as written.** Both results hold at the primary setting. Result 1 does not hold at
b = 2.5, where it is "not shown", not "equivalent": the interval reaches 0.06 and the full oracle
itself reaches only 0.538 there.

**What it means.**

- **Noticing is a real lever, and a larger one than the R6 correction estimated.** The R6
  entry predicted at most 0.091 from the never-noticed incidents. The measured gain is 0.097 with
  the rung's context and 0.156 with the window builder. The extra comes from asking earlier
  about incidents the rung notices late.
- **With a good context, noticing alone reaches the full oracle's quality.** `notice` with the
  window builder scores 0.952 on hard incidents, equal to R4's oracle, at 3.44 s per stream
  against 0.32 s. So R4's oracle decomposes as: noticing (this item) plus context (R6) plus
  selection (R5, cost only). Timing beyond notice + delay is worth little, which agrees with
  the R6 correction.
- **The slow leak is almost entirely a noticing problem.** The rung sees the leak only after
  its threshold crossing, a median 17.8 s late, and never for 40 of 80 diagnostic leaks.
  Noticed at its first reading, the leak is answered in 0.935 of cases with the rung's own
  context.
- **"Never noticed" is mostly mis-anchoring.** Every never-noticed hard incident has abnormal
  observations attached to an anomaly the rung anchored elsewhere, usually on background
  about 0.3 s earlier. The rung sees the activity and files it under the wrong anchor. This is
  a segmentation failure, which is close to what the charter's salience function is for.
- **The threshold sweep says the burst-family gap is partly tuning and the leak gap is not.**
  Lowering the notice threshold to z = 2 recovers 0.038 of the 0.097 at the price of doubled
  false alarms. No threshold helps the leak, because its sub-alarm readings are not abnormal
  observations at any threshold.

**Accepted with notes.**

- The injected notice is the arm's own record, not a rung anomaly, so it does not retire after
  6 s of quiet. Four of 270 diagnostic incidents were called by `notice` whose selection
  anomaly had retired before the delay. This is a small timing privilege inside the ceiling.
  It is recorded, not corrected; a ceiling that retires like the rung would be slightly lower.
- Result 2 mixes "whether it asks" with "when it asks" (about 16 s apart). The notice arm
  beats R4's oracle on the leak (0.935 against 0.906), which shows the ask time matters there.
- The never-noticed / late-noticed decomposition rests on 14 incidents and the late part's
  interval includes zero.
- The worker reported ignoring an instruction that arrived inside a tool result. Correct.

**Decided.**

1. EXP-101's registration names noticing as its first function, not threshold-versus-oracle
   selection. Its privileged ceiling is `oracle_notice` with the rung's context; its public
   baselines include the threshold sweep at z = 2 and a change-triggered rung.
2. The slow leak gets its own preregistered secondary measure in EXP-101, since it is where
   noticing matters most and where no threshold helps.
3. The anchoring finding is the first concrete job for a substrate mechanism: attribute
   activity to the right anchor. A public baseline for it is designed before any substrate is
   built.
4. R9's result decides whether the context half of the oracle is a quality or a cost lever.

## R8 real-model distractor sensitivity — merged; unidentifiable with these models

**Provenance.**

- **Code:** an evaluator-side question dumper behind the oracle feature, allowlisted in the guard;
  llama.cpp pinned at tag `b11429`, built outside the workspace in `artifacts/runtime/` (ignored);
  Qwen2.5 1.5B and 3B, Q4_K_M, with sha256 equal to the published values, in `artifacts/models/`
  (ignored).
- **Calls:** 277 in total, 260 of them scored. No parse failures, no errors, none truncated, none
  repeated.
- **Gates:** on exit codes on the merged tree: fmt, clippy `--locked`, 560 Rust tests, the oracle
  guard, 326 analysis tests.
- **Run outputs:** in `artifacts/runs/r8/`. The coordinator checked that they are identical to the
  worktree's before removing it.

**Re-verified from `calls.jsonl`.**

| Model | Hard questions | A(0) | 90% lower bound | Guess rate (control) |
|---|---|---|---|---|
| 1.5B | 28 | 0.500 | 0.357 | 0.357 |
| 3B | 16 | 0.250 | — | 0.250 |

- For the 1.5B, A(m) at m = 50, 100, 200 and 400 is 0.357, 0.393, 0.429 and 0.357, all at the guess
  rate. All of these match the worker's report.

**Verdict as written: unidentifiable with these models.** The failure is structural, not a matter
of sample size.

- The 1.5B model gains only 0.14 from having all of the evidence. That is below the 0.15 the
  precondition requires, so it would fail at any N.
- The 3B does no better than its own guess rate on the main questions.
- Neither model reads this world's evidence well enough for its loss to distractors to be
  measured.

**Coordinator notes.**

- **The plan's sizing was optimistic.**
  - The reasoner prompt needs about 5,200 tokens: the rules plus 14 worked examples.
  - Prompt evaluation on three cores runs at about 50–75 tokens per second.
  - So the 4-hour envelope allowed 28 hard questions, not up to 120.
  - The worker computed, before the main run, that even a model passing the precondition could not
    have produced an interval narrow enough for either regime at this N. The pilot did its job;
    the plan should have expected this.
- **Prompt development used 12 rounds on pilot-range questions.**
  - The 3B scored 6/12 on development questions and 4/16 on the main ones. That is within sampling
    error, but some fitting to the development questions cannot be excluded.
  - No change was made after a scored call.
- **The reference rule-reader is the more informative output, and it needs careful reading.**
  - It is a program that applies the prompt's rules to the rendered context.
  - On the same contexts it falls from 0.93 at m = 0 to 0.36 at m = 400 (δ̂ 0.44 [0.24, 0.88]).
  - The cause is that hard-incident evidence messages share an ID pool with background messages,
    so distractors include look-alikes that the reader's crude windowed rule accepts.
  - This is not the loss an optimal public reader would suffer; a reader that learned each stream's
    vocabulary from its own history might lose much less.
- **What it does establish is a property of the simulated reasoner.**
  - Its `q` is counted from hidden labels, so at δ = 0 it behaves as a reader that always knows
    which references are evidence.
  - No reader without hidden labels has that ability in this world.
  - The δ = 0 law is therefore not "a strong model". It is an oracle-labelled reader. R6's
    "simple builders suffice" was measured under that oracle.
  - This moves the prior toward R7's regime, without estimating δ.
- **Deviation accepted.** The incident's own non-decisive observations were kept out of both the
  contexts and the pool, whereas the simulator's `m` counts them as distractors. It does not affect
  the verdict.

**Decided.**

1. EXP-102 is not preregistered on the current reasoner law. The law's δ = 0 default is an
   oracle-labelled reader, and no real model available here can estimate δ.
2. The options are put to the user:
   - **(a) Simulation-only.** Ground the distractor law in the world itself: measure how a strong
     public reader degrades with look-alike distractors, then let the simulated reasoner's
     informed probability depend on confusable distractors rather than on a free δ.
   - **(b) A stronger real model.**
     - A local 7B model is not practical on this CPU: about 4–5 minutes per call at these context
       sizes.
     - A remote model needs credentials and money, which only the user can supply.
   - **(c) The salience ceiling for EXP-101.** This is independent of the context question.
3. The models (3.1 GB) and runtime (325 MB) are kept for now; free disk is 7.7 GB. They can be
   deleted if (b) is not chosen.

## R7 reasoner-law sensitivity — merged; R6's context finding is fragile

**Provenance.**

- The world change adds a distractor penalty to the hidden reasoner. It multiplies the informed
  probability by `exp(−δ·m/100)` and takes no new draw.
  - The coordinator read the `reasoner.rs`, `params.rs` and `sim.rs` diffs.
  - `m` counts references that are not decisive evidence of the focus incident, probes included,
    as R7 specifies.
- At δ = 0, R6's held-out run at b = 5, ρ = 0.7 replays byte-identical: 62 of 62 arms.
- In all six held-out runs, the context-only ceiling and R4's oracle are byte-identical to R6's.
- 13 runs, all exit 0, none excluded.
  - Five tuning runs were refused by the driver's clean-tree preflight before starting, because the
    worker had created untracked files during a run.
  - They were set aside and rerun at the new HEAD (`r7-stale-manifests.csv`). No output was
    affected.
- Gates on exit codes on the merged tree: fmt, clippy `--locked`, 553 Rust tests, the oracle guard,
  308 analysis tests.
- Run outputs moved to `artifacts/runs/r7/` (ignored).

**Re-verified from raw files.** The coordinator rebuilt the tuning selection from the raw tuning
`incidents.csv` and recomputed G(δ) with its own cluster bootstrap:

| δ | Selected builder | G [90%] |
|---|---|---|
| 0.05 | `window` 20 s, N 256 | 0.097 [0.066, 0.129] |
| 0.1 | `window` 20 s, N 256 | 0.137 [0.104, 0.170] |
| 0.2 | `cooccur` 1 s, N 256 | 0.280 [0.233, 0.327] |
| 0.4 | `cooccur` 1 s, N 128 or N 256 | 0.34 |

- The builder choices match the worker's. The bounds agree to within 0.001.
- At δ = 0.4, two configurations tie on tuning quality. The worker's tie-break toward fewer
  references gives 0.339, and the other gives 0.341.

**Verdict as written: Fragile.** At δ = 0.1 and 0.2, G ≥ 0.10 with the lower bound above 0.05.
"Robust" fails at every δ in the grid. The sensitivity settings (b = 2.5 and b = 8, at δ = 0.2)
agree.

**What it means.**

- **The result is conditional, and the condition is the finding.**
  - The ceiling's context holds only decisive evidence, so it is immune to the penalty by
    construction, and G must rise with δ.
  - What R7 measures is where R6's conclusion breaks. "Simple builders capture the context lever"
    holds only while the penalty is about 0.05 per 100 irrelevant references or less. That is at
    most about a 12% relative loss of informed probability at 250 references.
  - Above that, choosing which references to send is worth 0.14 to 0.34 of hard-incident quality.
- **Under a penalty, the best public context shrinks.**
  - The best context goes from about 490 references per call to about 85.
  - The winning builder changes from a broad `window` to a narrow `cooccur`.
  - At δ = 0.4 the best builder is barely above the rung's own context.
  - So compaction becomes the lever: deciding which few references carry the evidence. A
    substrate could plausibly do this, but no public builder here does it well.
- **EXP-102's design therefore depends on one unknown: the δ of a real reasoner** on contexts
  like these.
  - Simulation cannot supply it.
  - The charter puts real-model work in EXP-106, after EXP-101 and EXP-102.
  - R7 shows that the order matters: without an estimate of δ, EXP-102 cannot say whether its
    claim is quality or references.

**Accepted with notes.**

- The worker's readings were fixed in scripts before the runs, and are reasonable.
- The functional form (exponential in count, no position effect) is the plan's assumption, not a
  measurement.
- The selection delay was not re-tuned at δ > 0, as the plan said. A shorter delay might change
  context sizes slightly.
- The literature anchors (Shi et al. 2023; Liu et al. 2023) are still unchecked against the
  primary texts. No number relies on them.

**Decided.**

1. Before EXP-102 is preregistered, estimate δ on a real model. The smallest form is a local
   small model on this CPU, asked R1-style diagnosis questions with controlled numbers of
   irrelevant references.
   - This needs a model download, a runtime dependency and disk.
   - It is put to the user before it is planned in detail.
2. EXP-102, when registered, sweeps δ as a preregistered parameter and states its claim per δ
   region. It never states a single conclusion.
3. The salience ceiling from the R6 correction (an oracle that notices, with the rung's context and
   delay) remains the next simulation-only item, for EXP-101.

## R6 context-construction headroom — merged; simple builders capture the context lever in quality

**Provenance.** 26 runs through the driver, all exit 0; none failed, timed out or was excluded.
Eleven manifests refused by the driver's revision preflight were set aside and rewritten, not
deleted (`r6-stale-manifests.csv`). The six R5 held-out runs were replayed: 317 arm-runs are
byte-identical to R5's hashes. Gates on exit codes on the merged tree: fmt, clippy `--locked`, 545
Rust tests, the oracle guard, 308 analysis tests. Run outputs moved to `artifacts/runs/r6/`
(ignored).

**Re-verified at b = 5, ρ = 0.7 from raw files** (hard quality excludes slow leak).

| Arm | Hard quality | Refs/call | Critical misses |
|---|---|---|---|
| R4 oracle (`oracle_escalation_privileged`) | 0.952 | 5.1 | 194 |
| Context-only ceiling (`oracle_selection_context_d16_privileged`, supplementary) | 0.820 | 5.3 | 255 |
| Selection oracle + `window` 40 s, N 256 | 0.796 | 251.1 | 255 |
| Selection oracle + rung's own context | 0.489 | 43.3 | 282 |
| `always_escalate` + `window` 40 s, N 256 | 0.500 | 249.0 | 268 |

All match the worker's report.

**Verdict as written.** Both clauses hold at every setting, and the verdict is uninformative.
Clause 1 compares at no more than the ceiling's 5 references per call. No public builder is that
small, so the comparator is the empty context. Clause 2 is unreachable because R4's oracle carries
privileges no builder can supply.

**Coordinator error, recorded (second time).** This is the same mistake as R4.

- I took R4's oracle as the context ceiling although it bundles more than context.
- I wrote a clause (equal or fewer references) that no public builder could satisfy.

Lessons, applied to every criterion from here on:

- A ceiling isolates exactly one privilege. Its comparison arm differs from the public arms in
  that privilege only.
- Each clause is checked for feasibility against what a public arm can do, before any run.
- The worker's supplementary context-only ceiling is the correct comparator. It was labelled and
  did not replace the verdict.

**What it means.**

- With selection held at the oracle, simple builders capture almost all of the context lever in
  quality.
  - `window` reaches 0.796, against 0.820 for the context-only ceiling: a gap of 0.024
    [0.000, 0.051].
  - They do it with 33× the references per call (31.7 to 49.5), which is 8–11× the cost per
    stream.
  - EXP-102 can therefore claim references or cost at matched quality, not quality.
- Binding evidence across services and over the following seconds is solved by simple builders.
  Evidence that precedes the anchor by more than 2 s is not: only a long `window` carries it.
- The realistic public pairing, `always_escalate` with a builder, tops out at 0.723 for 17.4 s per
  stream.
  - The reasoner's token budget refuses up to 1153 calls in 200 streams.
  - Plain accuracy falls to 0.756 with `window`.

**Correction to the worker's report: the 0.132 is mostly salience, not timing.** The worker read
R4's oracle minus the context-only ceiling as timing (readiness). The coordinator counted, from
`incidents.csv`, hard non-leak incidents with no reasoner call at all.

- The selection-oracle arms make no call on 34 of 372 such incidents. R4's oracle calls on all of
  them. The selection-oracle arms call only about anomalies the shared rung noticed, so these 34
  are incidents the public rung never noticed.
- That alone accounts for 0.091 of the gap at every setting:

  | Setting | Gap | No-call part | Called but wrong |
  |---|---|---|---|
  | b5 | 0.132 | 0.091 | 0.040 |
  | b8 | 0.137 | 0.091 | 0.046 |
  | b2.5 | 0.067 | 0.091 | −0.024 |
  | b5, ρ0 | 0.121 | 0.091 | 0.030 |

- The decomposition subtracts counts. It is not paired per incident and has no interval, so it is
  coordinator arithmetic, not a measured lever.

Consequences:

- The worker's proposed first substrate job, readiness and compaction, loses most of its readiness
  half. Timing given a call is worth about 0.03–0.05 at b ≥ 5 and nothing at b = 2.5.
- Noticing is the larger item. A public rung that misses 9% of non-leak hard incidents, plus the
  slow-leak family (0.28 against 0.91), is a salience gap. It belongs to EXP-101 and needs its own
  privileged ceiling: an oracle that notices, with the rung's context and delay.

**Assumption that carries the result.** In the simulated reasoner, extra references "cost but never
hurt" (no distractor penalty, `gordian-stream/DESIGN.md` section 11).

- Under that law a broad window loses only tokens, which is why `window` is competitive.
- Published evidence for real models runs the other way (Shi et al. 2023; Liu et al. 2023, "Lost in
  the Middle"; to be checked against the primary texts).
- R6 swept b and ρ only. Until a distractor penalty and the per-reference price are swept, the
  0.024 gap is a property of this reasoner, not of context construction.

**Hidden-document exposure.** While looking for the public sections of `gordian-stream/DESIGN.md`,
the worker read sections 4, 5 and 11, which describe the hidden rules, and disclosed it.

- The grids (starting at 0.25 s; a 2 s lookback) may be influenced.
- The best builder, `window`, encodes no timing, and every builder is a tested pure function of the
  public view.
- The coordinator accepts the result with this caveat. The cause is structural: the public and
  hidden design share one file.

**Decided.**

1. Separate the hidden-rule sections of `gordian-stream/DESIGN.md` into their own file, so that
   workers can read the public design without exposure.
2. R7, reasoner-law sensitivity. Rerun the R6 comparison with:
   - a swept distractor penalty, including zero;
   - a swept per-reference price.

   The criterion will be fixed before any run, with one privilege per ceiling and every clause
   checked for feasibility.
3. The salience gap above becomes part of EXP-101's headroom: a noticing oracle with the rung's
   context is a separate ceiling.
4. EXP-102, when registered, claims references or cost at matched quality, against `window` and
   `cooccur` tuned by the frontier method. It bounds critical misses and plain accuracy.

## R5 decomposed headroom — merged; selection buys cost, context buys quality

**Provenance.** A container restart interrupted the run; a new worker resumed in place, retained
the interrupted replay, and re-ran it. All 237 R4 regression arm-runs match R4's committed hashes.
Gates on exit codes on the merged tree. Run outputs moved to `artifacts/runs/r5/` (ignored).

**Re-verified at b = 5, ρ = 0.7 from raw files.**

| Arm | Hard quality | Plain accuracy | Critical misses | Cost s/stream | Calls/stream |
|---|---|---|---|---|---|
| Selection oracle | 0.489 | 0.741 | 282 | 0.66 | 2.1 |
| `always_escalate`, 14 s | 0.489 | 0.815 | 256 | 7.36 | 24.7 |
| `contradiction_escalation` | 0.478 | 0.804 | 275 | 7.86 | 23.5 |
| R4 oracle (selection + timing + context) | 0.952 | 0.741 | 194 | 0.32 | 2.6 |
| Decoy oracle | 0 | 0.741 | 338 | 0 | 0 |

**Verdict as written.** Both clauses of the R5 criterion hold at every setting (primary: gap 0.433
[0.389, 0.476]; cost ratio 10.75 [9.08, 11.79]).

**What it means.**

- Perfect selection buys about 11× lower cost at matched hard-incident quality, but no quality, and
  it pays for the saving with lower plain accuracy and more critical misses than escalating
  everything, which the criterion did not count. An EXP-101 freeze must bound critical misses and
  plain accuracy, not only hard-incident quality.
- The public contradiction signal does not select: the public checker contradicts 97% of plain
  anomalies at some point. Telling hard anomalies from plain ones with public information is
  genuinely hard in this world.
- Context construction holds the remaining 0.46 of quality and most of the critical-miss reduction
  (194 against 282). The cause is binding: evidence at services the rung never attaches (738
  missing observations) and outside its window (330).
- Decoy handling has measurable headroom: the decoy oracle cuts false alarms by 1.88 per stream
  [1.68, 2.08] at no cost.

**Decided.** Before any substrate is built, R6 measures how far simple public context builders go,
under a criterion fixed before it runs. The common thread of R5's findings is evidence binding
across services and time; if simple builders capture it, EXP-102 has little to win, and if not, the
substrate prototype's first job is binding.

## R4 headroom — merged; the margin was met but did not discriminate

**Re-verified.** Gates on exit codes; at b = 5, ρ = 0.7 the coordinator recomputed from raw
`incidents.csv` and `results.csv`: oracle 354/372 = 0.952 at 0.32 modelled s per stream; best
baseline at the oracle's cost 0.008; `always_escalate` with a 14 s delay 0.489 at 7.36 s;
hidden-rules ablation 0.427 at zero reasoner cost. All match the worker's report.

**Verdict as written.** Headroom on all six settings (smallest gap 0.489, lower bound 0.446); the
conditional clause did not trigger. R1 is not revised.

**Coordinator error, recorded.** The margin compared baselines against an oracle that bundles
selection, timing and context. EXP-101 concerns selection only; the worker's arithmetic splits
roughly 0.49 selection and 0.46 context at b = 5. A margin met by a factor of five against such a
comparator discriminates nothing. R5 decomposes the headroom with separate privileged ceilings
and adds a public-information cascade baseline, under a new criterion fixed before any R5 run. This
is a new pre-run criterion for a new question, not a reinterpretation of R4's verdict.

**Other findings carried forward.**

- At escalation time, `always_escalate`'s contexts held none of the decisive evidence in 17 of 22
  burst incidents; with a 14 s delay, contexts were complete in only 22 of 66. Why contexts miss
  evidence that has arrived is traced in R5.
- R4's oracle does not handle decoys (it alarms as `never_escalate` does); decoy headroom is
  unmeasured until R5's decoy ceiling.
- Hidden-rule knowledge is worth a lot (ablation 0.427 at no reasoner cost); at b = 2.5 it nearly
  equals the oracle.
- Periodic configurations with periods up to 20 s, and `change_triggered`, are bound by the
  reasoner token budget.

## R1 to R3 — stream world, evaluator and harness merged

**R1 stream world.** Re-verified on exit codes. Two reasoner fixes required before merge and
delivered: no information from no evidence (truth enters an answer only through the informed
branch, probability zero without decisive evidence; checked in code), and repeated questions are
keyed by context fingerprint with copula-correlated correctness (`ρ` default 0.7). The
policy-facing answer carries only focus and diagnosis.

**R2 stream evaluator.** Re-verified on exit codes; six randomly sampled fixtures recomputed by
hand from RULES.md without reading the scorer, all matched. Judgements left to each
preregistration: wrong declarations do not cancel a correct one (spam), and alarms on a decoy
before it resolves count as false alarms.

**R3 stream harness and baselines.** Re-verified on exit codes; read the guard diff (a tightening
plus one allowlisted shim directory) and the privileged oracle's surface. Merge conflicts with R2
were additive and resolved as the union. Accepted the worker's reading that oracle escalation
fires once the hard incident's decisive evidence is delivered (escalating earlier is empty-handed
under the revised reasoner law).

**Carried forward to R4.**

- The R3 smoke parameters are placeholders. `always_escalate` escalates at notice, before any
  decisive evidence, and got no hard incident right in the worker's diagnostic; periodic and
  change-triggered hit the reasoner budget. Every baseline is tuned before comparison.
- The shared rung never notices the slow-leak family; that headroom belongs to salience, not to
  escalation timing, and is reported separately.
- Plain-incident accuracy of the cheap rung fell from about 84% to 73% after the first regime
  change in the worker's diagnostic, by design (its rules are not updated).

## A7b ratio interval — merged with a freeze condition

**Re-verified.** Only `analysis/` and `experiments/exploration/` touched; the analysis suite passes
on the branch and on the merged tree with `-W error` (262 passed, 4 slow tests deselected).

**Result.** On B1's empirical paired costs at the planning sizes (1,237 and 1,713), false
exceedance at true S = 0.20, out of a nominal 0.05, was: percentile up to 0.064, BCa up to 0.0675
(at n = 40), studentized up to 0.0545 (0.0484 when that worst cell was extended to 10,000
experiments). The studentized interval is the new default. Power at true S = 0.25 and 0.30 is
essentially 1 at the planning sizes.

**Noted.**

- The rule choosing the default was stated after the simulation table was seen. It chooses an
  instrument among reported alternatives, not a hypothesis outcome, so it is accepted, but it is
  not preregistered.
- The 0.06 bar is met on point estimates: three of eight acceptance cells have a 95% upper bound
  above 0.06 at 2,000 experiments. On heavily skewed lognormal costs no method meets 0.06.
- Derived data committed (paired-cost table, 360 KB; cell table, 25 KB) because the tests need it
  without the binary; both are regenerable by script. Accepted.

**Carried forward (freeze condition).** Before any experiment freezes on a ratio-of-totals
criterion, rerun `experiments/exploration/scripts/a7b_calibrate.py` on that experiment's own
exploration paired costs; the studentized interval must meet 0.06 with its 95% upper bound, not
only its point estimate, at the frozen n. The original condition was written for EXP-001; it now
applies to EXP-101 onward.

## Charter revised — approved by the user

The user approved `docs/charter-revision-proposal.md`. The charter's sections 1, 5, 6, 7 and 12
are rewritten accordingly, with new foundations and sources, and a revision record at the top.
EXP-001 is retired unfrozen and recorded as an exploration finding; the old EXP-002 to EXP-007 and
EXP-I01 are retired or re-scoped into EXP-101 to EXP-106. Change-triggered execution joins the
baseline registry. The plan gains Stage R (R1 stream world and simulated reasoner, R2 stream
evaluator, R3 stream harness and conventional baselines, R4 headroom check).

The user's message read "Inapprove the proposal"; the coordinator read it as "I approve" from
context and said so. If that reading is wrong, this change is reverted from git history.

## A6d incremental narrowing — merged; the shared rule no longer repeats work

**Re-verified.** Fixture committed before the rule change and untouched after it; gates on exit
codes. Worker evidence: 2,200 fixture rows and 55,000 B1 rows at 20 ms identical in every verdict
column; 30,403 calls step-equivalent to the cache-free reference; rule fit R² 0.996 / 0.997.

**Effect.** Narrowing work fell about 96% (30,007 against 764,647 world evaluations over the step
test). "A failed component raises success" fell from 102 to 35 episodes.

**The 35 are real cost, not repeated work.** In 31 of them the no-directive run ended short of
affordable work by a median of 37 ns, after paying about 8 µs once to read the verifier's first
output. A failed verifier never pays that bill, and at a 60–100 µs budget it is the margin. No
cache removes a first read. The other 4 also need the verifier's stale set to lose priority.
Documented in POLICIES.md §3.3; the rule is unchanged. Changing what reading an output costs would
be a change to the instrument and is not made.

**Accepted with a note.** Declared world and evaluation terms are now billed one call late, so the
hard limit can be overshot once per episode by at most about 8 µs. The work is still counted in
modelled cost. Under the revision proposal, any expensive reasoner call must be paid before it
runs, never in arrears.

**State of the shared rule.** Three successive fixes (A6b, A6c, A6d) made it rational at exhaustion
and incremental on unchanged inputs. It is now a strong cheap rung: change-triggered by
construction.

## A6c decode once — merged; the residue becomes A6d

**Re-verified.** The verdict fixture was committed before the rule change and is untouched by it;
all 2,200 rows (10 arms, 20 ms, 20 seeds × 11 classes) identical. Gates on exit codes. Rule
counted-operation fit R² 0.995 (fit) and 0.997 (held out).

**Correction to the plan text.** A6c's premise ("stored outputs are re-decoded every step") was
wrong: stored outputs never were. What repeated was a component *re-producing* byte-identical
output each step, which the rule decoded and charged again. The worker implemented the intent by
content equality and kept the old rule as `Decider::without_reuse`, a test-only reference.

**Effect.** Decoding fell 92%; at 20 ms mean modelled cost fell 40–47% for verifier-running arms
and 12–34% for the others. At 250 µs `all_components` went from 29.1% to 87.0% success. The
"failed component raises success" effect fell from 210 to 102 raised episodes but is not gone: the
rule still re-narrows the verifier's unchanged 46-hypothesis set every step (about 10.5 ns per
world, declared). Zeroing that term leaves 35 raised episodes, unexplained.

**Meta-finding.** Two defects in a row were the shared rule repeating work on inputs that had not
changed. Fixing them makes the baseline incremental. Change-triggered execution (recompute only
when inputs change) is the cheapest form of selective activation, and the charter's baseline
registry does not name it. A salience mechanism must beat it, not only periodic schedules.
Coordinator recommendation to the user: add "change-triggered (memoized) execution — whether
salience adds anything beyond skipping unchanged inputs" to charter section 7. Not made here,
because the charter is normative.

## Exploration follow-ups — merged; correction to the Stage B entry

**Re-verified.** Gates on exit codes; the truth table regenerated from
`crates/gordian-eval/examples/truth_table.rs` hashes to the scratch original; no crate source
changed beyond that allowlisted example.

**Finding 4 resolved, and it corrects Stage B.** The shared rule charges for decoding every stored
component output on every step, 530 ns per output plus 115 ns per hypothesis, even when the output
has not changed. On a symptom-free window the verifier's output lists 46 hypotheses, so re-decoding
it costs about 5,800 ns of the roughly 7,600 ns the rule charges per step. A failed verifier leaves
nothing to decode, the budget lasts longer, and success rises: that is the whole "failed component
helps" effect (removing the charge lifts `all_components` at 250 µs from 13.8% to 72.2% on these
episodes; removing the verifier's priority in the rule changes 0.2–1.8 points).

**Correction.** Stage B's collapse of verifier-running arms at binding budgets (`all_components`
0.291 at 250 µs) is mostly this re-decoding charge, not component compute. With it removed, the
worker's upper-bound variant gives `all_components` 0.730 and `fixed_verifier_only` 0.943 at
250 µs. The heuristic family is bit-identical either way, so the Stage B conclusion about the tuned
periodic baseline stands; the "naive pipelines collapse because components are expensive" reading
does not.

**Decided.** Re-billing an unchanged output is an artefact of the shared rule's implementation,
not a property of the world, and it biases every comparison involving a verifier-running arm. It
is fixed under every option of the pending decision (plan item A6c), before any experiment.

## Stage B exploration (B1–B4) — merged; EXP-001 not freezable as designed

**Re-verified.** Gates on exit codes; per-arm success, modelled cost and critical-miss rates
recomputed from `b1-variance.csv` match the worker's report. 220,000 B1 episodes, no failed or
excluded run, replay checks byte-identical.

**What the data say (exploration, not confirmation).**

- A periodic heuristic (`every` = 4) holds success 0.954 at every budget level for about 22k modelled
  ns per episode; `all_components` needs 522k for 0.965 and collapses to 0.16–0.29 when the budget
  binds. A strong simple baseline captures nearly all achievable quality at a few percent of the
  cost. This is the charter's baseline registry doing its job.
- The shared decision rule costs about 17.1k ns per episode, is charged at every step for every
  arm, and is 65–89% of the heuristic arms' cost. Against the tuned periodic pipeline, even a
  perfectly timed component selector can save at most about 10–18% (B4's estimate; arithmetic, not
  a run), below EXP-001's preregistered 20% margin. Quality headroom against that baseline is
  0.01–0.02.
- The one large oracle gap, JointlyDecisive (0.38), belongs to the shared rule's one-step probe
  choice; no component schedule can close it.
- Five of six stressors cannot fail any current arm: noise is separable for free by catalogue id,
  no arm has a salience mechanism, and the final declaration makes unbounded waiting invisible.
- Effective ambiguity in the ambiguous classes is 2.85 kinds, not 5; a prior-aware arm that never
  probes is right 41.6% of the time.

**Interpretation.** In the current small world, *which component runs* is not where the cost is.
The dominant computation is deliberation (the shared rule) and sensing (probes, which modelled cost
does not price). EXP-001 as designed would very likely return H0 for structural reasons of this
environment, not because selective activation fails in general. The charter anticipated the
mirror-image danger (a weak baseline making anything look good); here the strong baseline shows the
environment offers little to select.

**Instrument findings (not yet fixed).**

1. The rule is not schedulable: a selector cannot decide when to deliberate. Whether deliberation
   is part of what selective activation controls is a design decision.
2. Probes are not in the modelled cost, though the charter's `C` includes sensing; arms buy about
   two probes where one would do.
3. Stressors are toothless at default noise (see above).
4. A failed component can raise an arm's score at binding budgets (3 cells); suspected rule
   behaviour on an empty verifier output; not confirmed.
5. `b4.py` reads a truth table produced outside the repository; one exploration input is not
   regenerable.

**Decision required from the user** (not taken by the coordinator, because it shapes every later
experiment and freezing is irreversible):

- (a) Freeze EXP-001 in the current world and expect a bounded negative result.
- (b) Revise before freezing: make deliberation schedulable, price probes, and revise the world
  so that relevance is costly to determine (non-separable noise, specialist components with
  partial views), with the revision's properties fixed from the charter before any arm is run on
  it, and the generalist and periodic baselines kept.
- (c) Record (a) as an exploration finding only, and do (b).

Coordinator recommendation: (c). A preregistered experiment whose negative outcome is already
implied by exploration arithmetic has low information value, while the fixes in (b) are needed for
EXP-002 to EXP-004 anyway.

Independent of the decision, started then: A7b (ratio-interval calibration on B1's cost
distribution) and the two small exploration follow-ups, since merged (see the entry above).

## A8b counted operations — merged with a freeze gate

**Re-verified.** fmt, clippy (`--locked`), workspace tests with `--no-fail-fast`, oracle guard,
driver test, dump sha256 and the analysis suite on exit codes; reference checker functions
untouched.

**Accepted.** Counters follow the dominant loops of each component and the shared rule, are
deterministic, and cannot be seen or set by a policy (type-enforced, tested). Against hot-loop
minimum timings they fit with R² 0.98–0.99 on fit, held-out and real states.

**Scrutinized: in-situ rescaling.** With pure hot-loop weights the non-identical-arm check failed
(modelled ratio 15.66 against a wall-time median ratio of about 12.1). The worker found that a call
inside an episode costs 1.2–2.2× the same call in a loop, more so for arms that call components
sparsely, consistent with cache effects, and rescaled each weight by a per-component factor and
per-call constant fitted on other arms and seeds. The fit and the check share no arms or episodes,
and the worker reported the failure and offered rejection, so this is calibration, not tuning to
the test. Coordinator check on an unfitted sparse pattern (heuristic only against the verifier
every second step, seeds 200–219): modelled 5.25, wall-time median 5.40 [5.05, 5.85], inside.

**Residual risk.** The in-episode premium depends on the scheduling policy's call pattern, which is
exactly what EXP-001 varies. Fixed factors fitted on other arms priced the sparse arm about 5% off
in the worker's data and about 3% off in the coordinator's. The check is weak (intervals about 15%
wide).

**Carried forward to C1 (EXP-001 freeze gate).** Before freezing, run the cost check on the actual
EXP-001 arms (selective against the tuned periodic pipeline) on exploration seeds. If the modelled
ratio falls outside the wall-time interval, or the two disagree by more than a quarter of the
preregistered savings margin, the cost conclusion of EXP-001 is reported as unresolved, whatever
the modelled result.

**Host change.** The VM now reports a 2.10 GHz Xeon; earlier sessions reported 2.80 GHz. Modelled
cost is in calibration-host nanoseconds and does not change with the host; wall-time checks are
valid only on the host where they run. The manifest should record the CPU model and frequency, not
only flags (small follow-up).

## A8 interleaved arms — merged; A/A fails on an idle machine

**Re-verified.** Merged cleanly onto A6b; fmt, clippy (`--locked`), 293 workspace tests with
`--no-fail-fast`, the oracle guard, the driver test (62) and the analysis suite (212), on exit
codes.

**Worker's evidence.** Interleaving removes a large between-run effect: sequential A/A runs gave
12 of 20 intervals excluding 0 (sd of S 0.151), interleaved runs 0 of 21 (sd 0.037). The worker's
runs shared the machine with another worker.

**Coordinator's idle-machine A/A** (through the driver, cores 0–2, shell on core 3):

| Run | S | 90% interval | Contains 0 | Drift CV |
|---|---|---|---|---|
| heuristic only, run seed 1 | −0.7% | [−5.8%, +3.7%] | yes | 0.12 |
| heuristic only, run seed 2 | −2.9% | [−5.9%, +0.1%] | yes | 0.03 |
| heuristic only, run seed 3 | +5.4% | [+0.5%, +10.0%] | no | 0.27 |
| all components, run seed 1 | −6.9% | [−13.3%, −1.4%] | no | 0.11 |

The per-block minimum timing was stable to about 2% while the mean moved up to 80%; `/proc/stat`
shows non-zero steal time. In the all-components run five episodes carried 51% of the total
absolute difference, and S without them was +0.9%; the median per-episode log ratio was near 0 in
every run. Interference from the host arrives in bursts that hit one copy of an episode and not
the other, and a ratio of totals is sensitive to them.

**Decided.** The plan's A8 rule applies: counted operations are built before B1 (item A8b), and the
charter's cost `C` becomes deterministic modelled cost, with wall time as a secondary check. This
is a change of measurement made before any experiment is frozen.

## A6b final declaration — merged

**Re-verified.** All gates on exit codes; the `all_components` row at 60 µs reproduced exactly
(202 final declarations, 18 terminal, 38 successes, 40 critical misses).

**What it showed.** Under a binding budget, component-running arms exhaust their compute at
0.3–0.6 s of logical time, before any symptom has arrived, by re-running components on windows with
nothing new in them. At the final call they declare "no fault", which is right only on `NoFault`.
Nearly all of the success gained by the fix is `NoFault`; on faulted classes the failures are now
visible as wrong declarations and critical misses instead of undecided rows.

**Carried forward to C1 (EXP-001 preregistration).**

- An arm that does nothing and declares "no fault" scores `NoFault` for free. The primary outcome
  must be read per class with the critical-miss rate, or exclude `NoFault` from the success
  average and score it through false alarms; the preregistration chooses and states which.
- Whether the shared rule at its deadline should declare "no fault" on a window with no symptom,
  or abstain, is a rule choice that changes faulted-class scores. It is fixed before freezing and
  applies to every arm.
- The final call's declared cost is derived, not separately calibrated.

## A6 baselines — merged; headroom probe

**Re-verified.** fmt, clippy with `--locked`, the workspace tests, the oracle guard and the
driver shell test on the merged tree. Accepted the clock interpretation: a component's declared
Compute nanoseconds are its time for every component, and `Slow` multiplies them.

**Coordinator error, fixed forward.** The merge was pushed after a test summary showed one failure,
because the command chain gated on a parsed summary rather than cargo's exit code. The failure was
a random-count coverage floor in the A5b equivalence tests (about 2–3% flake rate; the checker
agreed with the reference on every case). Fixed with a deterministic sweep and a floor at about 3.9
standard deviations. Coordinator merges now gate on exit codes only.

**The worker's finding.** With the shared rule and default limits every arm is at or near the
ceiling (209–213 of 220; oracles 220). Taken alone, this would make EXP-001 trivially pass.

**Coordinator headroom probe** (20 seeds × 11 classes, cgroup-isolated, successes of 220):

| Arm | compute 20 ms (default) | 250 µs | 60 µs | 25 µs |
|---|---|---|---|---|
| heuristic only | 209 | 209 | 187 | 39 |
| estimator only | 210 | 206 | 20 | 20 |
| random p = 0.5 | 213 | 138 | 20 | 13 |
| verifier only | 210 | 78 | 19 | 10 |
| all components | 213 | 35 | 18 | 4 |
| oracles | 220 | | | |

Under a binding budget, quality depends strongly on scheduling. Two effects are mixed in this
table and must be separated:

1. *A shared-rule defect.* Nearly every failure is `budget_exhausted`: the rule waits for its
   patience deadline, runs out of affordable work first, and never declares although declaring is
   free. Plan item A6b gives every arm a final declaration.
2. *Real waste.* `all_components` ran 154 component calls per episode on windows that had mostly
   not changed (median decision at about 1.9 s, a step every 50 ms). Avoiding that waste is what
   selective activation claims to do, and a tuned periodic schedule is the charter's adversary.

**Decided.**

- No world revision for EXP-001. It is testable under a binding budget; B1 sweeps budget levels
  and the preregistration fixes the levels from B1.
- The saturation at default limits is a property of a budget that never binds, not evidence for
  or against any mechanism.

**Open, for the user.** One cheap component (the heuristic) is nearly sufficient alone: the
components are redundant generalists that all read the whole window with the full public rules,
not the specialists the charter's question is about. That limits the quality headroom available to
EXP-002 and EXP-003. Making components specialized (partial views, complementary evidence) is a
research-design choice that risks tailoring the environment to the hypothesis; it is raised to the
user rather than built.

## A5b checker — merged

**Re-verified.** fmt, clippy with `--locked` and all features, 220 workspace tests, oracle guard,
dump sha256 unchanged; the diff to `gordian-components` is cost constants only. The pinned
calibration recheck waits until no other worker is compiling.

**Accepted.** Root cause found by step counting before any change: `anchored(site)` rescanned the
informative prefix at every dependent alarm, about k·d steps. The fix carries the minimum anchor
instant per world; exact for unsorted instants. About 55,000 equivalence comparisons against the
kept reference across generated, adversarial and random inputs; mutation shows the tests can
fail. Late-anchor shape linear; 78× faster at n = 2048. 0–20% slower on shapes the reference
already handled linearly; not tuned, per spec.

**Correction to the A5 entry.** The "24× declared at n = 256" and "1.7 ms at n = 1025" figures
were not reproduced by the checker alone (0.9–1.0 ms at n = 1024 on the old build). The 24× was a
component-level declared-to-measured ratio from one session; read it as an order of magnitude,
not a constant.

**Decided: machine drift is a threat to every measured-cost comparison.** Wall time drifted up
to 30% within one session and 13–34% between sessions. No hardware counters exist in this VM.
Plan item A8: interleave arms per episode in randomized order, log a drift-control workload, test
for position effects, and pass an A/A check before B1.

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
