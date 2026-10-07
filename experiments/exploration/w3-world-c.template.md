# W3: world C, the phase-2 collision floor, and A2's predictions scored from the hidden side

Status: exploration. Nothing here tests a hypothesis, nothing in W3 reaches an arm, and nothing may
later be cited as a confirmation. Lab 3, unit W3 (`docs/lab-queue.md`), branch `world-c` (based on `main`
at a87aca8). The PI read `crates/gordian-stream/HIDDEN-DESIGN.md` (experimenter-side work) and the
stream crate's source; the report says where a statement comes from that reading and was not
measured. This file is assembled by `experiments/exploration/scripts/w3_assemble.py` from
`w3-world-c.template.md` and the committed `w3-*.csv`; every table below is one of `w3_tables.py`'s.

Every statistic names its **seed range** and its **side**:

- *hidden*: from the stream's hidden truth (`crates/gordian-stream/examples/laws`), a fact about the
  world and not about any arm;
- *run, hidden*: an arm's evaluator files and ledger joined to hidden truth by (seed, incident index);
- *hidden joined to public*: a verdict computed from the hidden side about something public (A2's
  predictions, E1's recalls).

Ranges. **World A** is the defaults: **10000-10099** (tuning-like) and **40000-40199** (held-out), the
ranges of W2 (its hidden tables are reproduced byte for byte, section 1.1). **World C** is the defaults
with the tier mix changed from plain 800 / hard 100 / decoy 100 per mille to **plain 600 / hard 300 /
decoy 100** and nothing else: **60000-60099** (tuning-like) and **70000-70199** (held-out). A2's smoke
is world A, seeds **10000-10019**. Intervals are 90% percentile intervals of a cluster bootstrap
over whole streams (10,000 resamples, seed 9950, W2's constants; A2-side intervals resample the
streams that had predictions).

## 0. Answers

1. **World C's reachable events: 138 against 41** on the held-out range (70000-70199 against
   40000-40199), 3.4 times; 63 against 18 on the tuning-like ranges. "Reachable" is W2's item-6 set `R`:
   a hard incident that repeats an incident the selection oracle's arm declared correctly before the
   repeat's first notice, the most a site-keyed memory fed by that arm could save. Of all hard
   recurrences, 259 against 83 (3.1 times; 1.295 per stream against 0.415); the share of the bill `R` covers
   is 10.0% [8.6, 11.5] against 8.5% [6.4, 10.8], an interval about a third narrower. The family-keyed reach inside a
   stream (`F`) is 412 against 70; carried across streams it is 99.98% of the bill from the eleventh stream on
   (97.8% in world A). The brief expected about 120 reachable events for 40; the measured ratio is 3.4 for `R` and 3.1 for all hard recurrences.
2. **What else changed with the mix** (section 2.2): *Unchanged within the intervals:* incidents per stream
   (26.7), the number of other incidents live during a hard incident (5.1), overlap of incidents (98%
   overlap another; the arrival rate and the incident lifetimes did not change), the background share of the
   delivered stream (0.64 against 0.65), deadlines (critical share 0.32), and the share of hard incidents
   that recur (0.17 against 0.16), and the reasoner's share of the oracle arm's bill (0.9998). *Changed:* (a) the
   **family mix of the hard tier** (cascade 25.8% to 18.4%, slow leak 22.3% to 28.4%, compound 22.9% to
   26.8%), which I attribute to the generator's fall-through to the next family when a cascade has no free
   unconnected pair (read from the code, not measured per arrival); so hard cascades per stream rose 2.1 times, not 3;
   (b) the **decoy-to-hard ratio**, 1:1 to 1:2.7, which is a consequence of keeping decoys at 100 per mille
   and changes any collision count that mixes the two; (c) the share of hard incidents the oracle arm
   notices fell from 0.857 to 0.815 and is a **family-mix effect** (the slow leak is noticed 43-47% of the
   time in both worlds, per family the shares are unchanged); (d) the **share of streams with a hard incident altered
   by the signature shift** doubled (0.19 to 0.40), by the added edge 0.105 to 0.195; (e) arrivals skipped
   because no service was free rose from 0.34 to 0.57 per stream (intervals [0.24, 0.45] and [0.45, 0.70]
   barely meet). The oracle arm's **missed deadlines** (a correct declaration after the deadline) are 2.8% of
   hard incidents against 3.1% [section 2.4].
3. **The phase-2 collision floor** (section 3). FLOOR_ANSWER
4. **A2's predictions scored from the hidden side** (section 4). On the 251 predictions of the learner
   (seeds 10000-10019): **21 (8.4%) name a pair that is a true hidden edge** (a cascade partner or an added edge,
   in the direction predicted) against 7.1% for a uniform draw among the services the learner could have named;
   the permutation distribution puts 17.7% of draws at or above 21 (no evidence of precision above chance).
   **None of the 251 is an event hit**: not one was made on an alarm that belongs to an incident for which the pair
   is the true partner relation and followed by that incident's partner alarm. **Coverage of true partner
   alarms is 0 of 29** (0 of 10 for those a trial was licensed on); the funnel is 29, 25 within the widest band,
   17 whose site alarm the learner counted (the explanation filter and "not a first alarm" cost 4 and 4),
   10 with the partner quiet, 5 with an edge held for the pair at some read, 0 predicted on the alarm. The
   filter costs 4 of 25 coverable alarms (16%); the licence of a quiet partner costs 7 of 17 more. Learned edges
   (distinct, per band): 7 of 127 on a true pair (5.5% against 6.7% chance). The control arm (`a2_raw`) is no
   better (49 of 775 predictions, 14 of 302 edges, 0 of 29 covered). Hidden-side follow rate 60 of 251 = A2's own
   reading, 60 of 251.
5. **Which bounds move** (section 5): FLOOR_BOUNDS
6. **Least sure of** (section 7): the floor is computed from each incident's own evidence and so is a floor of
   the world, not a prediction of what an arm reaches (E1's actual recalls on plain incidents are three times the
   floor's: section 3.4); the choice of the mix (decoys fixed) is mine and changes the hard-versus-decoy ratio; the
   A2 verdict rests on 29 true partner alarms in 20 streams.

## 1. What was measured, what was assumed, and what was decided

### 1.1 Verified by running something

- **The example is unchanged where W2 used it.** `laws` was extended (world C, `--floor`, `--alarms`); its
  five default outputs for the 300 world-A streams (10000-10099, 40000-40199) are **byte-identical** to
  W2's (`cmp` on incidents, regimes, streams and vocab of both ranges, and the same for the first 20 and 5
  streams under the new flags). The rebuilt-incident check of `laws` (every incident equal to the stream's own)
  held on all four ranges of this unit (the example exits non-zero otherwise).
- **W2's functions reproduce W2's numbers.** `w3_laws.py` and `w3_ceiling.py` call W2's functions on world A
  and the rows equal W2's committed CSVs (`recurrence-summary`, `family-elsewhere`, `regime-effects`, `stale`,
  `recurrence-eligibility`, `ceiling`, `ceiling-bill`, `ceiling-unasked`, `ceiling-propagation`; checked by `equals`).
  The world-C rows are the same code on the world-C tables and runs.
- **World C's runs**: the oracle arm `sel_reanchor_privileged` (the selection oracle at 16 s, the rung's
  context, B2's re-anchor noticer, b = 5, rho = 0.7), 100 and 200 streams, under `scripts/run-driver.sh`
  and the cgroup runner, exit 0 each, the ledger kept (`events-sample.jsonl`, trace rate 1), the manifest
  differing from W2's only in the run id, the seeds, the experiment name and `stream_params.mix` (the
  manifest script asserts it). Calls: ledger and `results.csv` agree per stream, calls per incident equal
  `incidents.csv`'s `escalations`, the evaluator's tier and family equal the hidden tables' (W2's join checks,
  which stop the script).
- **The floor's simulator** reproduces W2's published phase-1 collision table exactly on both world-A ranges
  (`w3-floor-check-w2.csv`, 32 of 32 cells equal over the four ranges, world C included) and has 11 unit tests (`w3_floor_test.py`). The hidden-side key
  features agree with E1's own recalls where E1 recalled from a source in the same stream (11 of 11 for the
  site-keyed forms, 8 of 9 to 22 of 23 for the family forms: section 3.4).
- **The scoring script** has 8 tests on a synthetic stream with answers derived by hand (`w3_score_test.py`) and
  caught 14 of 14 hand-made mutants of itself (window bounds, relation direction, owner join, quiet condition,
  counted-alarm condition and others).
- **Determinism**: every table is a pure function of the hidden tables, the runs and the inputs named; reruns are
  byte-identical: after a second run of `w3_laws.py`, `w3_ceiling.sh`, `w3_bounds.py`, `w3_pairs.py`, `w3_e1join.py` and `w3_score.py` (both arms) 31 of 31 CSVs are `cmp`-equal to the first; `w3_floor.py`'s CSVs (a 20-minute run) were
  compared the same way (section 8).
- Gates, process record: section 8.

### 1.2 Read from the code or the design record and not measured

- That a hard family falls through to the next in declaration order when it is infeasible, and so that
  a threefold hard share lowers the cascade share (the family mix of section 2.2 is measured; the cause is read from
  `stream.rs`).
- The phase-1 classes of W2 (a mimic's burst is a plain burst; a decoy presents as its family): W2 read them
  from the code; section 3.5 tests them against data-derived classes.
- The anomaly attach rule of the rung (an observation at the site or at a dependent of the site belongs to the
  site's anomaly; an unconnected service starts its own): read from `noticer::attach_target`; it decides which
  evidence reaches E1's consistency gate and key.

### 1.3 Decisions and where the brief was underspecified

1. **The mix.** The tier mix has two free numbers (plain, hard; decoys take the rest) and hard 300 needs plain
   to give way. "Nothing else changed" is read as: hard 100 -> 300, decoys held at 100, plain 800 -> 600. The
   alternative (decoys scaled with plain) would have kept the hard:decoy ratio near 1:1 and is not run; the report
   states what the choice changes (section 2.2, 3).
2. **"Reachable events"** is W2's `R` (the arm-conditional site-keyed reach), the quantity A1b's power rests on;
   `R_world` (every hard recurrence) and `F` are reported beside it.
3. **Item 2's "key that includes the evidence classes that arrive after the first phase"** has no single
   definition in the brief. I built the key from the incident's own public observations (best case: no other
   incident's or background symptom in them) with the invariant features E1 and A1c built, read at a ladder of
   cutoffs after the first abnormal observation, and with E1's own snapshot rule (section 3.1). "The rule-breaking
   evidence as the public rules define it" is the public checker's verdict (`consistent_hypotheses`) on the
   evidence attached to the anomaly; the evidence classes of phase 2 enter through the kinds at services outside
   the site's dependents, the verdict, the bands, A1c's partner relation and, as a labelled extension, the benign
   streak that is the decoy's own evidence (the public decoy rule).
4. **Sources of the memory**: two teachers, never merged: hard incidents and decoys with their true labels (W2's
   section 9 reading: what a perfect teacher would leave) and hard incidents only (what the selection oracle
   asks: its table holds no plain incident and no decoy). The stored answer is always the truth of the source (a
   right source); the reasoner's own error is W2's section 7.6 and is not here.
5. **Two availability readings**: a source is usable by every later-arriving incident (W2) or only after its
   answer would arrive (19 s after the source's first alarm: 16 s selection delay, about 2 s of latency, about 1 s of
   notice) and no earlier than the target's key is read. An incident never recalls its own answer.
6. **A2's trace**: scored from `/home/user/gordian/artifacts/runs/a2/a2-smoke-b5-rho0.7/` (the chief's copy,
   read only): `predictions-<arm>.csv`, `first-alarms-<arm>.csv`, `edges-<arm>.csv`, for the learner and the control.
   Brief item 4 asked for the trace file; Lab 1's joined files carry the same columns.
7. **No hidden-side accessor was added to `src/`.** The additions are the `laws` example's (`floor.rs`, world C, two
   flags), all behind `reveal-hidden-state`; `scripts/check-no-oracle.sh` passes.

## 2. Item 1: world C (hidden; run, hidden)

### 2.1 Recurrence, same family elsewhere, regime changes (W2's items 1, 2, 5)

{{rec_summary}}

Hard incidents by family (the share in brackets is of the range's hard incidents):

{{family_shares}}

Same family at another site (W2's item 2: a hard incident, not a recurrence, whose family and mode occurred earlier in the stream at another site):

{{elsewhere}}

Regime changes (W2's item 5):

{{regime}}

{{regime_effects}}

Stale by construction (W2's definition: a recurrence altered by a change that took effect after its template began):

{{stale}}

Reading. Recurrence behaves the same per tier (the share of hard incidents that recur is 0.16 to 0.18 in all four
ranges), so the number of hard recurrences scales with the number of hard incidents (1.295 per stream against 0.415, 3.1 times).
The same-family-elsewhere count scales faster, 8.5 times (435 against 51), because it counts pairs of hard incidents
in a stream and the hard incidents are three times as many: family-keyed reach by experience inside a stream
is 45.8% [43.9, 47.5] of hard incidents against 26.2% [23.3, 29.0]. After the first change fall 62.4% of the hard incidents
in world C (61.7% in A): the schedule is unchanged. Streams with a hard incident altered by the signature shift go from 19% to
40%, by the added edge from 10.5% to 19.5%; the stale-by-construction hard recurrences are 30 of 259 (11.6% [8.0, 15.4]) against
12 of 83 (14.5%): 30 events against 12 for the recovery measure the charter asks (EXP-104), still too few for a per-stream time.

### 2.2 What else changed with the mix (hidden)

{{world_compare}}

Reading. Nothing that sets the stream's clutter changed: incidents per stream, the mean number of other incidents live
during a hard incident (5.1 in all four ranges), the share of incidents that overlap another one (98%: with one arrival every 20 s and
incidents live for 50 s, nearly every incident overlaps another in every world here), the delivered stream's size and the share of it
that belongs to no incident (0.64 against 0.65). What changed is the composition: plain incidents fell from 21.5 to 16.4 per stream, hard rose
from 2.56 to 7.58, decoys stayed at 2.7; and, **because the generator takes the next family when a cascade or split brain is infeasible**,
the cascade share of the hard tier fell (and the leak's rose), so hard cascades (the incidents that carry a hidden partner) rose 2.1 times, not 3
(1.395 per stream against 0.66). The added edge alters about one incident per stream in both worlds (0.955 against 0.97): the mix does nothing for
the added-edge evidence A2 needs.

### 2.3 The ceiling (W2's item 6) on world C (run, hidden)

The oracle arm's bill and what it did:

{{bill}}

Site-keyed and family-keyed reach (W2's sets; `R` is the item-6 answer):

{{ceiling}}

Family-keyed memory carried across streams, from the eleventh stream on:

{{ceiling_cross}}

`R` is the item-6 answer for a site-keyed memory: 138 hard incidents on the held-out range (127 calls, 39.3 x 10^9 modelled ns),
10.0% [8.6, 11.5] of the total bill; tuning-like 63 incidents, 8.75% [6.8, 10.7]. World A held-out: 41 incidents, 8.5% [6.4, 10.8]. The point
estimates are close and the interval narrows from +/-2.2 to +/-1.5 points. `R` is 53% of the hard recurrences in world C (49% in A):
the arm declares about half the hard incidents correctly in both worlds (0.50 and 0.48). The family-keyed form inside a stream reaches 28.7%
[26.3, 31.0] of the bill (13.2% in A), and carried across streams it reaches all of it (99.98%): in world C every family and mode has been seen
and declared correctly by the eleventh stream, so the carried ceiling is 1 in practice, and any difference between key forms is a difference in
what the key can recognise, not in what there was to recognise.

### 2.4 The oracle arm on the extra hard incidents (run, hidden)

{{deadlines}}

Reading. The share of hard incidents declared correctly by the deadline (0.472 against 0.451), the missed ones (correct after the deadline,
2.8% against 3.1%) and the critical misses are the same within the intervals; the oracle arm asks every hard incident it notices
(1288 calls, on 1228 of the 1516 hard incidents), none on a plain incident or a decoy, and the reasoner is 99.98% of its bill, as in world A. The hard
incidents noticed are 81.5% against 85.7%, a **family-mix effect**: per family the shares are cascade 0.986 / 0.985, compound 0.966 / 0.940,
split brain 0.965 / 0.980, slow leak 0.425 / 0.465 (world C held-out / world A held-out; `w3-ceiling-hard-incidents.csv`).

What the same run says about unasked-correct declarations with no memory:

{{unasked}}

Inherited error (the answer a memory fed by this reasoner would store is wrong), W2 section 7.6:

{{propagation}}

## 3. Item 2: the phase-2 collision floor (hidden side)

FLOOR_SECTION

## 4. Item 4: A2's predictions scored from the hidden side (hidden joined to public)

A2's smoke (Lab 1: seeds 10000-10019, world A, the selection oracle, the frozen medium; arms `a2_learn`, the learner, and `a2_raw`,
a labelled control with the explanation filter off) wrote `predictions-<arm>.csv`. `w3_score.py` joins each prediction to the
hidden alarms (every abnormal observation with its owner incident or background), the time-zero graph and the added edge, and to the
hidden incidents. Definitions (one line each; the script's docstring is the full text):

- *True hidden edge pair*: the predicting service is a cascade's root and the predicted one its partner (`cascade`), or the predicted service
  depends on the predicting one in the true graph at the instant of the alarm and did not in the public graph (`added_edge`). The reverse
  relations and a split brain's peer are reported apart and are not counted as true.
- *Event hit*: the predicting alarm belongs to an incident for which the pair is the true partner relation, and the alarm that follows within the
  band belongs to the same incident.
- *Followed (hidden)*: any abnormal observation at the predicted service in `(t, t + band]`.
- *Chance*: for each prediction, the share of the services A2 could have named at that alarm (unconnected to the predicting one in the public
  graph, either way, and quiet for 2 s) that would have scored the same; 2000 uniform draws for the distribution.

### 4.1 Precision of the learner's predictions

{{a2_precision}}

The permutation test (the learner, 251 predictions):

{{a2_perm}}

Reading. The predictions are at the chance level on every hidden-side verdict. 21 of 251 name a true hidden-edge pair against 7.1% expected,
and the permutation puts 17.7% of draws at or above 21. The 12 predictions on a `cascade` pair are on three pairs in three streams (7 on 7 -> 8 in seed 10003, a pair held after that
stream's cascade at 264 s; 3 on 3 -> 4 in seed 10010, **made before** that pair's decoy cascade at 403 s; 2 on 3 -> 6 in seed 10017), and the 9 on `added_edge` pairs are four pairs in four streams: the
pair-level verdict counts a pair that is a hidden edge at any instant of the stream, so it cannot tell a learned edge from a coincidence that preceded the cascade, and the
cluster bootstrap over streams ([0.036, 0.143]) is the honest width. None of the 21 was made on an alarm of an incident for which the pair is the partner relation (event hits 0).
The followed predictions have a median lead of 0.98 s from the alarm (0.78 s from the step that made the prediction); they are follows at chance (hidden-side follow rate
0.239, chance 0.248), and A2's own reading (60 of 251) and the hidden-side reading (60 of 251) agree. By stream position (the nine lowest and the nine highest seeds among those with predictions)
0.099 against 0.069 of predictions on a true pair; by time in the stream 0.033, 0.065 and 0.145 for alarms before 200 s, in 200-400 s and after 400 s (the added edge exists from 400 s; the intervals are wide and overlap, and I read
nothing into it).

### 4.2 Coverage of the true partner alarms, and what the explanation filter costs

A true partner alarm is the first alarm at a cascade's partner (hard or decoy: a decoy's early partner alarms are the same draws) or at the first service newly downstream of the
site of an incident the added edge altered. The funnel counts, among the 25 alarms whose lead from the site's first alarm is within the widest band, those that survive each condition of a
trial (the learner opens a trial only on a *counted* first alarm: one that no upstream burst in the previous 0.4 s explains; and a trial is licensed only for a *quiet* partner):

{{a2_coverage}}

Reading. **Coverage is 0 of 29 over all true partner alarms, and 0 of the 10 for which a trial was licensed.** The lead of a true partner alarm is
short (median 43 ms; 33 to 78 ms for the hard contradict cascades, 4 to 57 ms for the added edge in this smoke) and nearly all are inside the 0.4 s band; the five mimic cascades' partner
alarms arrive 9 to 14 s after the root's (4 of 5 beyond the widest band): no band of A2 can cover them. The explanation filter, as specified,
removes **4 of the 25 coverable alarms** (a further 4 are not first alarms at all: the site was already alarming), so the filter's cost is 16% of the coverable
alarms and the larger loss is the licence: 7 of the 17 counted alarms have a partner that was not quiet. Five true partner alarms had an edge for the pair
held at some read of the stream and none was predicted on this alarm; in the one I traced (seed 10000, 4 -> 9) the edge was held at three reads at 402 to 430 s and was no longer held at the event, 491.9 s; the others I did not trace. The learned edges
themselves:

{{a2_edges}}

### 4.3 What A2's learner could ever cover from one earlier sighting of a pair (hidden)

A learner whose evidence is the co-alarm timing of one pair, with nothing carrying across streams (W2), can predict a partner alarm only if that pair occurred earlier in the
same stream, and A2's threshold needs two follows beyond chance:

{{pairs}}

Reading. In world A, 0.295 events per stream have an earlier sighting of the pair in the stream and 0.07 have two; in world C 0.485 and 0.065. **World C does not
raise the ceiling for a learner that needs two sightings (0.065 against 0.070 per stream) and raises the one-sighting ceiling 1.6 times**; the added-edge events do not
rise at all (0.855 against 0.88 per stream; pairs repeat 0.145 against 0.085 per stream). The brief's use of world C as the power source for A2b is therefore not supported by this table.

## 5. Which bounds move

BOUNDS_SECTION

## 6. Reproduction

`experiments/exploration/scripts/w3_run.sh` lists the order (`w3_hidden.sh`, `w3_manifests.py` and `scripts/run-driver.sh`, `w3_laws.py`, `w3_ceiling.sh`,
`w3_bounds.py`, `w3_pairs.py`, `w3_floor.py`, `w3_e1join.py`, `w3_score.py`, `w3_provenance.py`, `w3_assemble.py`). Raw directories are under `artifacts/runs/w3/`
(git-ignored; kept until the chief has verified them): `hidden/{c-tune,c-heldout,a-tune,a-heldout,a-a2}` (tables, floor features, `usage.json`; `a-a2` also the
alarms and the graph for A2), `w3-ctune-b5-rho0.7` and `w3-cheldout-b5-rho0.7` (the ceiling's runs with the ledger, `usage.json`), `_manifests/`. File hashes are in
`w3-provenance.csv`. A2 is scored by `w3_score.py --predictions <A2 run>/predictions-a2_learn.csv --first-alarms <A2 run>/first-alarms-a2_learn.csv --edges <A2 run>/edges-a2_learn.csv --hidden artifacts/runs/w3/hidden/a-a2 --arm a2_learn` (the same with `a2_raw`). The hidden world-A tables are identical to W2's; W2's kept runs, E1's held-out run and A2's smoke are read, never written, from `/home/user/gordian/artifacts/runs/`.

## 7. Uncertainty, what would change the conclusions, and what the chief should examine

UNCERTAINTY_SECTION

## 8. Gates and process record

GATES_SECTION
