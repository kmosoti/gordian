//! The record rung (work item E1, Lab 2): the re-anchor noticer plus a table from a key to the
//! diagnosis last obtained from the reasoner for that key, consulted before the arm escalates.
//! It is the public comparator for the engram of work item A1: what a careful engineer would build
//! to keep records, in two key forms, three confirmation policies and a reset at the boundary of a
//! stream.
//!
//! Everything in this documentation (the keys, the policies, the gate, the tuning rule) was
//! written and committed before any run of the arm. A change after a run is recorded in
//! `experiments/exploration/e1-memory-measures.md` with its reason, never edited in here silently.
//! The author of the arm did not read `crates/gordian-stream/HIDDEN-DESIGN.md`; the arm uses the
//! public rules (`PUBLIC.md`, the first world's rules in `gordian-world`) and what it learns from
//! its own answers, and nothing that exists only on the hidden side of a world.
//!
//! # What it is
//!
//! A [`noticer::Noticer`] that wraps a base noticer (the later re-anchor, by default the
//! configuration B2 selected) and delegates every noticing decision to it, so what is noticed and
//! where it is anchored is exactly the base's. It adds a memory behind the seam the engram of work
//! item A1a uses: the reasoner's answers reach it ([`noticer::Noticer::answered`]) and it yields
//! recalls ([`noticer::MemoryRecall`]) that the arm declares without escalating, or, when the
//! confirmation policy says so, asks about instead.
//!
//! # The gate: it waits for evidence the public rules cannot explain
//!
//! A recall may be made only for an anomaly on which the rung's own consistency checker has found
//! no hypothesis consistent with the evidence attached to it
//! ([`super::rung::AnomalyView::contradicted_since`], the first world's own word for
//! "contradictory"; the public meaning of rule-breaking evidence). The rung keeps that verdict
//! only when asked ([`noticer::Noticer::wants_consistency`], which this noticer answers yes), runs
//! the consistency verifier through the meter (charged and counted like every component call) and
//! hands the verdicts to [`noticer::Noticer::gated_recalls`] after each step's checks. An anomaly
//! the public rules can explain is never recalled for, which is why a recall displaces no
//! declaration the shared rule would have made on such an anomaly (a plain incident the public rules
//! settle), and why a key built from the first seconds of evidence alone, which a plain incident
//! and a decoy also show, never fires. What the gate does not exclude is a decoy or a plain
//! incident whose attached evidence another incident's symptoms made contradictory: those recalls
//! are collisions, and the evaluator counts them (`RULES.md` K6).
//!
//! The gate also decides what is learned. The key of an anomaly is read **once**, when the gate has
//! been open for `settle_ns` (the *snapshot*), and the answer the reasoner later gives for that
//! anomaly is bound to that same key. A key read at the time of the answer instead would hold
//! evidence that arrived after the time a recall can act (the arm asks 16 s after the notice, a
//! recall is made seconds after the first contradiction) and would not match the key at recall
//! time for the same family of incident. An answer about an anomaly that has no snapshot (the gate
//! never opened before it was asked, or it was retired and forgotten) is not bound; it is counted.
//!
//! # The key (fixed before any run)
//!
//! Read from the base noticer's tracked anomaly and the observations the rung holds, at the
//! snapshot, as a sorted list of 64-bit features. `s` is the anomaly's site and `t0` its anchor's
//! instant. Every feature is a function of public bytes the arm was delivered, the public graph
//! and the public alarm line `HIGH`; nothing else enters.
//!
//! The **family-keyed** key holds stream-invariant features only (no service id and no message id,
//! both of which are regenerated per stream); the **site-keyed** key holds the same features and
//! the site's id besides. The features come in three cumulative levels, `level` in the parameters:
//!
//! | Level | Features added |
//! |---|---|
//! | `kinds` | the set of abnormal kinds at `s` (which of the five counters read at or above `HIGH`, which catalogue messages other than `CheckHealth`, whether a configuration snapshot differs from the public graph's), and the kind of the anchor observation |
//! | `bands` | for each abnormal counter at `s`, the band of its highest reading in the evidence: below 2 `HIGH`, below 4 `HIGH`, or above |
//! | `timing` | the band of the gate delay (from `t0` to the first contradicted check): under 1 s, 3 s, 6 s, 10 s, or more |
//!
//! Evidence means the abnormal observations attached to the anomaly by the base noticer that are
//! about `s`, emitted by the snapshot instant. Observations at other services (dependents, a
//! hidden partner) are not in the key: a concurrent incident at an unconnected service would put a
//! partner in the key of one in three recurrences by chance (the stream has an incident about every
//! 22 s and the evidence window is seconds), and a key of exact features must not carry chance.
//! Free-form message ids are benign under the public rules and appear at a site as background
//! too; they enter a site-keyed key only when `msg_ids` is on (off by default, a labelled
//! sensitivity row): the free-form ids at `s` between `t0` and the snapshot. Probe answers do not
//! cross the noticing seam (the rung admits them to the anomaly that bought them) and are not in
//! the key.
//!
//! # The table, and what a recall says
//!
//! A table from key to entry. An entry holds the diagnosis last obtained for the key (the answer
//! as the reasoner gave it, with the site it named), the observation the answer was about (the
//! **source**, handed to the harness for every recall: [`noticer::Noticer::recall_source`]), how
//! many answers have been bound to the key, and whether the entry is *disputed*: set when an answer
//! differs from the one stored, cleared when one agrees. The newest answer replaces the stored one;
//! there is no vote, which is what "the diagnosis last obtained" means. At a recall the table is
//! read with the anomaly's snapshot key:
//!
//! - **Site-keyed**: the stored diagnosis is declared as it is.
//! - **Family-keyed**: the stored answer is bound only if it names no site other than the anomaly's
//!   (or says "not an incident"); a service number means nothing in another stream. The declaration
//!   is the stored kind at the anomaly's own site.
//!
//! A recall is offered at most once per anomaly and only to an anomaly with no escalation and no
//! answer; the arm then declares it at once (source `recall`) and never escalates that anomaly,
//! whatever the selection rule asks, unless the policy confirms it.
//!
//! # The confirmation policies (fixed before any run)
//!
//! `confirm` in the parameters, as the engram's:
//!
//! - `never`: a recall is declared.
//! - `every` with `k`: the recalls of the arm, counted in stream order across segments (the count
//!   is carried even by an arm that resets its table), confirm at the `k`-th, `2k`-th, ...; a
//!   confirmed recall is not declared, the arm asks the reasoner about the anomaly at once, and the
//!   answer is bound like every answer.
//! - `on_contradiction`: a recall is confirmed when its entry is disputed.
//!
//! # Reset at the boundary of a stream
//!
//! `reset` in the parameters. On, the table is empty at the start of every segment: nothing about
//! services, message ids or the graph survives into a stream where they are regenerated. Off, the
//! table is carried to the next segment in a process-wide store keyed by `state_key` (L1's
//! precedent, [`carry`]); a site-keyed table carried is wrong by construction (W2: a site number
//! means another service in the next stream), a labelled control and reported with and without the
//! reset, never tuned away.
//!
//! # Parameters and the tuning rule (fixed before any run)
//!
//! | Parameter | Value | Why |
//! |---|---|---|
//! | base | the later re-anchor, B2's selection | the memoryless comparator is the same noticer; what differs is the memory |
//! | `settle_ns` | 1 s, not tuned | two checks' worth (the rung's `review_ns` is 0.5 s): more of the first rule-breaking evidence is in the key, and a recall still has most of a deadline of at least 20 s |
//! | `level` | `kinds`, `bands`, `timing`, tuned | the only free parameter of the key |
//! | `k` of `every` | 2, 4, 8, tuned | the only free parameter of a policy |
//! | `msg_ids` | off | below the key |
//!
//! Tuned on seeds 10000 to 10099, in stream order, under W2's stale-error bound for the memory's
//! own errors: of the recalls whose stored answer was right for its own incident (a recall with a
//! right source, `RULES.md` K6), at most 0.20 are wrong, and at most 0.25 such wrong recalls per
//! stream on average. Stage 1: for each form and each reset setting, the three levels under the
//! policy `never`; the level kept is the one with the most hard incidents unasked correct among those
//! meeting the bound (ties to the richer level; a stage-1 configuration with no recall of a right
//! source meets the bound vacuously, and if the best satisfying configuration has no unasked
//! correct hard incident the level kept is `timing`, the finest, and the cell is reported as one
//! the bound leaves empty). Stage 2: at that level, `every` at `k` of 2, 4 and 8 and
//! `on_contradiction`; for `every`, the `k` kept is the one with the most hard incidents unasked
//! correct among those meeting the bound (ties to the smaller `k`), or the smallest collision share
//! when none does. The held-out table (seeds 40000 to 40199, in stream order) runs every cell once
//! with its tuned values, beside the memoryless re-anchor.
//!
//! # What this is not
//!
//! It is not told whether a recall was right: the stream never says. It learns only from answers it
//! asked for, so what it can remember depends on what its selection rule asks about; under the
//! selection oracle every answer is about a hard incident and the table holds nothing about plain
//! incidents or decoys, and a wrong recall on one is never corrected. Its table lookups and binds
//! are bookkeeping and are not priced in the modelled bill (no calibrated weights); the
//! consistency checks it asks the rung for are, as for every arm that monitors.
