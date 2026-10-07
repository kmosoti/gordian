# W2: the learnable laws of the stream world, measured from the hidden side

Status: exploration. Nothing here tests a hypothesis, nothing in W2 reaches an arm, and nothing
may later be cited as a confirmation. Lab 3, unit W2 (`docs/lab-queue.md`), branch `world-laws`
(based on `main` at 8c7b356). The PI read `crates/gordian-stream/HIDDEN-DESIGN.md` (it is
experimenter-side work) and the stream crate's source; the report says where a statement comes from
that reading and was not measured. This file is assembled by `experiments/exploration/scripts/w2_assemble.py` from this
template and the committed `w2-*.csv`; every table below is one of `w2_tables.py`'s.

Every statistic names its **seed range** and its **side**:

- *hidden*: from the stream's hidden truth (`crates/gordian-stream/examples/laws`); a fact about
  the world, not about any arm;
- *public ids, hidden labels*: bytes a policy sees (message ids, the abnormal-counter cue) grouped by
  a hidden label (the incident's family); it bounds what a policy could learn, not what it does;
- *run, hidden*: an arm's evaluator files and ledger joined to hidden truth by (seed, incident index).

Ranges: **10000-10099** (the tuning streams) and **40000-40199** (the held-out streams) at the
default parameters, always reported separately; **50000-50099** world B (recurrence 0.6, regime
changes at 150 s and 300 s), items 1, 2 and 5 only. Intervals are 90% percentile intervals of a
cluster bootstrap over whole streams (10,000 resamples, seed 9950, R10's constants).

## 0. Answers

Held-out streams (40000-40199, hidden unless it says run) unless a range is named; tuning
streams (10000-10099) beside.

1. **Recurrence is rare in the quantity that matters and exists only inside one stream.** 20.5%
   of incidents recur (5.5 per stream of 26.7); but only **0.415 hard incidents per stream** (83 in
   200 streams; tuning 0.44, 44 in 100) repeat an earlier hard incident, because 80% of the
   incidents a template can be are plain. The best experience curve a memory could follow, the
   cumulative recurrences against incidents seen, is 0 at the first incident, 0.43 after six,
   1.35 after ten and 3.75 after twenty (all tiers; section 2.3); the share of the n-th incident
   that is a recurrence climbs from 0 to about 0.25 by n = 10 and stays. Recurrence never crosses
   a stream boundary (service numbers, the hard vocabulary's ids and the graph are regenerated per
   stream), so there is **no recurrence experience to accumulate across streams**: across streams the
   rate is flat. Of the recurrences, 12.8% [11.0, 14.6] are stale by construction (14.5% [8.4, 21.1] of
   the hard ones, 12 of 83).
2. **Same family at another site** adds **0.255 hard incidents per stream** (51 of 429
   non-recurrence hard incidents, 11.9% [9.5, 14.3]; tuning 16.6%): with the recurrences, a
   family-keyed memory could reach 26.2% [23.3, 29.0] of hard incidents by experience inside a
   stream, a site-keyed one 16.2%.
3. **Vocabulary.** Every hard incident's out-of-catalogue messages determine its family: within a
   stream the mutual information between id and family equals the family's entropy (1.18 bits) and
   100% of ids sit at exactly one family; the background floor, the same statistic over background
   free-form messages labelled by the hard family live at that instant, is no better than its
   permutation null (excess -0.001 bits [-0.005, 0.004], against +0.53 [0.50, 0.55] for the incident-borne
   messages). But the id alone is a weak marker in the stream: only 13.3% of free-form messages whose id
   is one of the stream's hard vocabulary are incident-borne, 19.2% with an abnormal reading at the
   service in the previous 10 s, 28.1% with three or more.
4. **Hidden edges.** A cascade's root-to-partner delay is 22-150 ms in a Contradict presentation
   (mean 84 ms) and 6.2-15.8 s in a Mimic one (mean 11.4 s). The same pair recurs only as a
   recurrence: 16 of 132 hard cascades repeat a (root, partner) pair, 15 of them flagged
   recurrences. At 400 s one edge is added; 0.97 incidents per stream alarm over it (0.11 of them hard).
5. **Regime changes.** 61.7% [58.5, 65.0] of hard incidents begin after the first change at 200 s;
   the signature shift alters 0.235 hard incidents per stream (14.9% of those after it; compounds
   41%, cascades 23%, split brains and leaks never) and the added edge 0.11 (17.5%).
6. **The perfect-memory ceiling** (run, hidden; the selection oracle at 16 s with the re-anchor noticer
   and the rung's context, b = 5, rho = 0.7). The reasoner is 99.93% of the arm's bill. Site-keyed reach (a hard
   incident that repeats one the arm declared correctly earlier): 41 incidents, 39 calls, 11.7 x 10^9
   modelled ns = **8.5% [6.4, 10.8] of the total bill** (tuning 9.0% [5.7, 12.5]). Family-keyed
   reach inside a stream: 13.2% [10.3, 16.1] (tuning 20.9% [15.6, 26.0]). Family-keyed memory carried
   across streams reaches 97.8% [96.2, 99.1] of the held-out bill: that, not recurrence, is where a
   memory's reach is large; it is also where confusion with plain incidents and decoys lives (section 9).
7. **A1b bounds proposed** (section 10, with reasons): an unasked-correct clause on **hard
   recurrences only** (plain recurrences are 85.5% unasked-correct with no memory at all), a floor of
   0.15 of hard recurrences with a lower bound above 0.075 (the signature-keyed ceiling is 0.37, the
   site-keyed 0.49); a stale-error clause split in two. **Collision and staleness**: of the recalls whose stored answer was right for its
   own incident, at most 0.20 wrong and at most 0.25 per stream, which a site-only key fails by 4 and 7.6 times and a perfect phase-1
   site-and-class key meets at 0.15 and 0.19. **Inherited error** (the stored answer was wrong; 40% of site-keyed sources held-out,
   54% tuning, because the reasoner is right 48% of the time): reported and compared with the record rung's, not given an absolute bound.

World B (50000-50099) is in section 8: 47.1% of incidents and 46.4% of hard incidents recur,
and 69.9% of hard incidents begin after the first change at 150 s.

## 1. What was measured, what was assumed, and what was decided

### 1.1 Verified by running something

- **Hidden tables** (`laws` example, `w2_hidden.sh`, under `scripts/cgroup-run.sh`, cores 0-2, 2 GB;
  exit 0 each, `usage.json` beside the tables): 100 + 200 + 100 streams, 10,666 incidents. The example
  rebuilds every incident from its own plan with the stream's physics and graph
  (`oracle::rebuilt_incident`) and compares it with the stream's own observations: all 10,666 equal,
  or the example exits non-zero. A second run of the 100-stream tuning range gave byte-identical files.
- **The generator's recurrence promise** (HIDDEN-DESIGN.md section 7) holds in every recurrence of the
  three ranges (1,095 + 550 + 1,262): same tier, family, mode, site, partner, criticality and kinds as the
  template; the template is earlier; its services were free again (`w2_laws.py` asserts it). No recurrence
  occurred without an eligible template. The recurrence share among incidents with an eligible
  template is 0.255, 0.252 and 0.611 at r = 0.25, 0.25 and 0.6.
- **The ceiling's run.** One arm, `sel_reanchor_privileged`, 100 tuning and 200 held-out streams,
  with the ledger kept for every stream. Its manifest differs from L1's only in the run id, the experiment name, the timeout, the trace
  rate (1 here, 0 there), the source revision and the lockfile hash (my tree is `main` at 8c7b356 plus these commits). The
  held-out run is **identical to L1's fresh run of the same arm** (seeds 40000-40199): `incidents.csv`,
  `results.csv`, `notice_incidents.csv` and `notices.csv` equal but for `run_id` and `arm_role`
  (section 7.4). The join to hidden truth is checked: every `Escalate` focus maps to an incident through
  the observation owner map, the calls per incident equal `incidents.csv`'s `escalations` for every incident,
  and the calls and modelled ns sum to `results.csv` per stream; the evaluator's tier and family agree
  with the hidden tables for every incident (`w2_ceiling.py` stops otherwise).
- **An independent check of "altered"**: `w2_laws.py` also predicts the signature-shift-altered set from the rule read off `present.rs` and `incident.rs` (a
  plain incident of the shifted kind that is not a duo; a compound with the kind as `a`, or as `b` in a Contradict presentation or a hard Mimic's phase 2; a
  cascade with the kind as `a`) and stops unless it equals the rebuilt set: 281, 572 and 299 incidents in the three ranges, no disagreement. The rebuild and the
  code reading are two routes to the same set. (For the added edge there is no such second route; its set rests on the rebuild, and `edge_exposed`, the
  semantic version, is reported beside it.)
- Determinism: `w2_laws.py` and `w2_ceiling.py` rerun to byte-identical CSVs (checked with `cmp`).
- Gates, before the final push: section 12.

### 1.2 Read from the code or the design record and not measured

- The **phase-1 classes** of section 9 (a mimic's burst is the plain burst of the imitated kind; a decoy
  presents as its pretended family) are read from `present.rs` and `incident.rs`, and the indistinguishability
  claims of HIDDEN-DESIGN.md section 5, which were tested there, not here.
- That a signature-keyed memory cannot match a recurrence the physics altered ("stale by construction")
  is a consequence of the definition below, not something tested with a key.
- The arm's public inputs: the cue of section 4 is computable from the public stream, but no arm computes
  it here.

### 1.3 Definitions fixed here (choices, stated before the numbers were read)

- **Recurrence**: an incident with `recurrence_of` set (the generator's flag, hidden). **Template**: the
  incident it names. Recurrences of recurrences are recurrences of the named template.
- **Altered by a regime change**: the incident's observations, rebuilt from its own plan under the first
  world's physics (undoing a signature shift) or on the time-zero graph (undoing an added edge), differ
  from the stream's. This is a property of the incident, not of a family table I wrote.
- **Stale by construction**: a recurrence altered by a change that took effect after its template began
  (`template onset < change instant`), so the record the template left describes physics that no
  longer holds. The truth of a recurrence is the template's truth whatever the regime: a stale recurrence
  is a **miss** for a signature-keyed memory and a possible error only if the key collides.
- **Family and mode**: hard kind and Mimic or Contradict (the slow leak has no mode): seven classes.
  **Strict** adds the compound's pair and the cascade's known kind.
- **Reach**, item 6 (see `w2_ceiling.py`): a hard incident `j` is *site-keyed reachable* (`R`) when it
  repeats an incident that the arm declared correctly (`first_correct_at_ns`) **before `j`'s first notice**
  (its onset when never noticed), the earliest instant an arm could recall; family-keyed (`F`) when an earlier hard
  incident of the same family and mode (any site) was so declared. `F_only` is `F` without `R`. The `_world`
  variants drop the condition on the arm.
- **Side of the vocabulary statistics**: see section 4.

### 1.4 Where the brief was underspecified and what I did

1. **Item 3, "from the public side only" and "the same statistic over background free-form messages as the floor".**
   Read as: the ids are the public ones (nothing is taken from the generator's pool); the family label is
   necessarily hidden, since the stream never says which family a message belongs to. The floor has no
   family for a background message, so I labelled each by the hard family live at that instant (anywhere, and
   at the message's service) and compared each statistic with a label-permutation null of the same sample
   (200 permutations per stream; the plug-in mutual information of a few dozen messages is biased upward,
   so the null, not zero, is the floor).
2. **Item 6, "R6's manifest ... or a fresh run".** A fresh run of the re-anchor arm with the selection oracle
   (the second option), one arm per range, which replays L1's arm on 40000-40199. R6's own manifest is a
   62-arm run under the rung's noticer; its selection oracle is a different arm from this one, so
   `R` here is conditional on the re-anchor arm's declarations (48.2% of held-out hard incidents declared correctly).
3. **"Declared correctly earlier"** read with the arm's clock: before the repeating incident's first notice.
   The looser readings (any earlier declaration; `R_world`) are reported beside it.
4. **Item 2's "earlier incident"** is an earlier *hard* incident (a decoy teaches a memory no diagnosis).
5. **World B**: the regime kinds are kept (signature shift, then added edge), their instants move to 150 s and
   300 s; the tier mix, noise and reasoner are the defaults.
6. **Additions beyond the seven items**, because the A1b bounds need them and the brief asks me to propose
   them: the site and phase-1 collision tables (section 9), the cross-stream reach (section 7.3), the baseline
   of unasked-correct declarations without any memory (section 7.5), and the cue levels of section 4. They are
   labelled as additions where they appear.
7. **Hidden-side additions to the stream crate**, all feature-gated and unreachable from an arm: the
   `oracle::rebuilt_incident` accessor (`crates/gordian-stream/src/oracle.rs`, tested in
   `src/tests/rebuild.rs`: it equals the stream's own observations on 25 seeds and, over 60 seeds, changes
   only incidents after the first change, and has power), and the `laws` example entry in
   `crates/gordian-stream/Cargo.toml`. `scripts/check-no-oracle.sh` passes.
8. `incidents.csv` carries no cost per incident (only per stream), so the cost per incident is read from the
   ledger's `Escalated` outcomes (trace rate 1), matched to their `Escalate` decisions.

## 2. Item 1: recurrence (hidden)

### 2.1 Per stream, by tier and family

{{rec_summary_a}}

By mode, plain kind and decoy family:

{{rec_summary_modes}}

Reading. The recurrence share is 0.205 of incidents in both default ranges, as the generator's 0.25
per eligible arrival implies once the first incidents (no template yet) and the busy arrivals are
counted. The hard tier recurs less (0.16) than the plain, and the decoys more (0.24 held-out), because the
tier of a recurrence is its template's and the pool of templates is 80% plain; only 2.4 to 2.6 hard incidents
exist in a stream to be repeated. By family the held-out rates differ (cascade 0.11, slow leak 0.18) by
amounts the intervals do not separate; I read them as one rate. Eligibility:

{{rec_eligibility}}

### 2.2 The gap to the incident repeated

For each recurrence the index of the incident it repeats, the gap in incidents and in seconds, and its staleness are in
`w2-recurrences.csv` (one row per recurrence, 2,907 rows over the three ranges); per stream counts by tier and family
are in `w2-perstream.csv`. The gaps:

{{rec_gaps}}

A repeat is never sooner than 23 s after its template ended (the template's services stay reserved), the
median is 157 s after the template began (8 incidents later), and the 90th percentile 352 s. A hard
incident repeats a hard one that is usually one or two hard incidents back (median 1).

### 2.3 The experience curve

{{curve}}

Cumulative recurrences against incidents seen, averaged over the streams that reach n incidents (at least 20
streams). It is the best curve a memory could follow if it recalled every recurrence. It rises slowly
(0.01 recurrences after two incidents) and at about one recurrence per four incidents from n = 10: the
rate is a property of the template pool (the more incidents seen, the more eligible templates) and is
reached inside a stream. Across streams the same quantity is a straight line:

{{stream_order}}

The last two columns of the file `w2-experience-stream-order.csv` give the share of hard incidents that
are recurrences in the first and the second half of each range (a-tune 0.155 and 0.205; a-heldout 0.148 and
0.175; b 0.462 and 0.465). I did not test the difference; with 20 to 50 recurrences in a half, a difference of
0.03 to 0.05 is the size of the sampling error, which is what a stationary rate would give.

### 2.4 Recurrences stale by construction

{{stale}}

Half of all recurrences repeat a template that began before a change and begin after it. Of all
recurrences 12.8% [11.0, 14.6] were altered by a change that took effect after their template began (held-out;
tuning 10.9%); of the hard ones 14.5% [8.4, 21.1] (12 of 83; tuning 4 of 44). The signature shift is the larger
part: compounds and cascades with the shifted kind in their physics (33% and 27% of hard compound and cascade
recurrences held-out), never split brains or slow leaks. Staleness is a **miss**, not a wrong answer: a recurrence
has the template's truth, so an old record is right about the diagnosis and wrong about the evidence.

## 3. Item 2: same family, different site (hidden)

{{elsewhere}}

Held-out, 51 of 429 non-recurrence hard incidents (11.9% [9.5, 14.3]; 0.255 per stream) have an earlier hard incident of the same family and mode at
another site; with the strict key (same kinds) 37 (8.6%). Tuning: 33 of 199 (16.6%) and 20 (10.1%). The sets
intersect little: of a stream's hard incidents the share a family-keyed memory could reach by experience is
0.262 [0.233, 0.290] held-out and 0.317 [0.261, 0.368] tuning, against 0.162 and 0.181 for the site-keyed one.

## 4. Item 3: the vocabulary (public ids, hidden labels)

{{vocab}}

- Incident-borne: every id of a hard incident's out-of-catalogue messages sits at exactly one family
  (share 1.000 in every stream with a hard family; HIDDEN-DESIGN.md section 7 says each kind draws from its
  own four ids), so the plug-in mutual information equals the label entropy. Because a stream has 15 such messages over
  7 distinct ids, the plug-in value is inflated by chance; the permutation null is 0.59-0.65 bits, and the excess
  over the null is 0.51 and 0.53 bits.
- The floor: background free-form messages labelled by the hard family live anywhere or at their service have a
  mutual information that equals the null (excess -0.009 and -0.017 tuning, -0.001 and -0.001 held-out); ids seen at least
  twice sit at one family in only 21-23% (live anywhere) of cases. Background messages that happen during a hard incident carry
  no information about it.
- 49 of 100 tuning streams and 116 of 200 held-out streams contain two or more hard families with vocabulary
  messages; the rest have too few for the statistic (it is zero by definition with one label). The per-stream values are in
  `w2-vocabulary-perstream.csv`.

What the id is worth in the stream a policy sees:

{{vocab_precision}}

The id is not a marker: a hard kind's four ids are 16 of the 48-id pool that background free-form messages
(70% of 1.5 per second) draw from, so seven of every eight messages with a vocabulary id are background. The cue is public (abnormal counter
readings at the message's service in the previous ten seconds, at most three counted) and raises precision
to 28% with three, at the price of keeping 71% of the incident-borne messages. A policy that learns the
vocabulary must therefore learn it **attributed to an incident** (the message arrived at an anomaly it is
about to ask the reasoner about), not from the id's frequency, and it must learn it within the stream:
the 48 ids are regenerated per stream.

## 5. Item 4: hidden edges (hidden)

Delay from the first abnormal reading at the root to the first at the partner (the cascade's partner, the
split brain's peer), in milliseconds for a Contradict presentation and seconds for a Mimic one (the `unit` column):

{{delays}}

A cascade's partner alarm comes 22-150 ms after the root's in Contradict mode (mean 84 ms) and 6.2-15.8 s in
Mimic mode, where it is phase-2 evidence; the decoys of a Mimic cascade never show it (66 held-out). A
split brain's peer that is also a dependent of the site alarms about 40-50 ms in in Contradict mode (mean 41 ms
tuning, 51 ms held-out; it is an ordinary dependent alarm, W1's finding): a rule that reads "an alarm at a service
the graph does not connect" would not fire for these. For a dependent peer in Mimic mode the first abnormal reading
at the peer is bimodal (the site's own burst alarms it within 60 ms; the phase-2 peer alarm comes 6-16 s in), so its
mean (6 s) describes neither, and the decoys of a Mimic split brain whose peer is a dependent show the first mode only.

The added edge:

{{edgeadd}}

The edge is added at 400 s; 1.09 incidents per stream (held-out) have a service newly downstream of their site or of
their cascade's partner, and 0.97 per stream alarm over it (the others have a new dependent that their presentation does not alarm: some alarm only the first three
dependents or one chosen at random): 0.11 hard incidents per stream, 0.79 plain. This is the number of
events from which a policy could learn the new edge from co-alarm timing: about one per stream, mostly plain.

Pair recurrence:

{{pairs}}

A (root, partner) pair recurs almost only as a flagged recurrence: held-out, 16 pair repeats among 132 hard cascades,
15 of them recurrences; 26 of 149 hard split brains, all flagged. Pairs available per graph (incomparable pairs):
16.4. A hidden edge learned from the first occurrence is thus reusable 0.08 times per stream (hard cascades: one
repeat per 12.5 streams) and 0.13 (split brains), and an A2 that waits for a second sighting of a pair has one
sighting to learn from in 12 streams or so.

## 6. Item 5: regime changes (hidden)

{{regime_a}}

{{hard_after}}

Per change and family, the incidents it altered, per stream. At 200 s the signature shift alters 2.86
incidents per stream (2.45 plain, 0.235 hard, 0.175 decoy): 14.9% of the hard incidents after it, 41% of
compounds, 23% of cascades, no split brain and no slow leak (their burst holds no characteristic message).
At 400 s the added edge alters 0.97 (0.79 plain, 0.11 hard): 37% of the hard cascades after it. **61.7% of hard incidents
begin after the first change** (tuning 62.1%), 24.6% after the last: most of what a memory is asked about happens
under the changed physics. In 19% of held-out streams (18% tuning) at least one hard incident is altered by the shift, in 10.5% (7%) by the edge.

## 7. Item 6: the perfect-memory ceiling (run, hidden)

### 7.1 The bill

{{ceiling_bill}}

The reasoner is 99.93% of the arm's bill in both ranges; every call is on a hard incident (the selection
oracle asks about those it is given), none on a plain incident, a decoy or the background. The mean call
costs 0.30 s of modelled reasoner time. Because the reasoner is the bill, a share of the bill and a share of the
reasoner cost are the same number here (both columns are in `w2-ceiling.csv`).

### 7.2 Site-keyed and family-keyed reach

{{ceiling}}

`R` is the item-6 answer for a site-keyed memory: 41 hard incidents held-out (39 calls, 11.7 x 10^9 modelled ns), 8.5% [6.4, 10.8] of the
bill; tuning 18 incidents, 9.0% [5.7, 12.5]. It is about half of the hard recurrences (41 of 83; 18 of 44) because
the arm declares 48% of the hard incidents correctly. Of the cost on `R`, 8.5 x 10^9 ns (73%) is on incidents the arm
answered correctly anyway: that is saving at no change in quality; on the rest a memory would also have corrected the
answer. `R_chain` (any earlier same-site same-family incident, not only the named template) adds a few
(44 held-out), `R_fresh` removes the stale ones (31, 6.3% [4.5, 8.1]). The family-keyed form inside a stream reaches
`F` = 13.2% [10.3, 16.1] held-out (tuning 20.9% [15.6, 26.0]; the intervals meet only between 15.6 and 16.1, and I would not
average the ranges), of which `F_only` (family, not the site) is 4.7% [3.1, 6.3] (tuning 11.9%). The `_world` rows show what the
declared-correct condition costs: all 83 hard recurrences would be 16.4% [13.6, 19.3].

### 7.3 Family-keyed memory carried across streams (addition)

{{ceiling_cross}}

Service ids and message ids are regenerated per stream, so a key that carries across streams must have neither;
the family-keyed form is the only one that does. Its reach is almost everything: after the first ten streams
97.8% [96.2, 99.1] of the held-out bill is on a hard incident whose family and mode the arm had already
declared correctly in an earlier stream (93.9% with the strict key). This is an upper bound on **reach, not on correct recall**: whether a key
built from public evidence can tell the family without being fooled is section 9.

### 7.4 Identity with L1's fresh run

{{identity}}

### 7.5 What the same run says about unasked-correct and stale-wrong declarations with no memory (addition)

{{unasked}}

Plain incidents are declared correctly with no call by the cheap rung alone: 85.5% of the plain recurrences held-out [83.1, 87.8].
A clause on "unasked-correct share of recurrences" over all recurrences is therefore dominated (80%) by plain ones that
need no memory. The hard incidents' unasked-correct is zero by construction here (the oracle arm asks about every
hard incident it notices; an arm that does not ask would show its cheap rung's confident error instead), so
the only hard unasked-correct decisions an A1b arm can make are memory's. The "wrong, no call" column counts incidents
with at least one wrong declaration and no call: 17% of plain incidents (the cheap rung's own errors), 73% of decoys
(the arm declares every decoy an incident): these are errors of the **noticer and rung**, present with no memory, and an
E1 `stale_wrong` that counts them all is not a measure of what memory does.

### 7.6 The error a memory fed by the reasoner inherits (addition)

{{propagation}}

A memory that stores the reasoner's answer repeats it. In this arm the hard declarations are right 48% of the time (b = 5, rho = 0.7, the rung's context),
so of the hard incidents whose source answer was delivered before they were noticed, 40% [28, 51] of the site-keyed sources (68 held-out) and
44% [35, 53] of the family-keyed sources (115) held a **wrong** answer, 54% [38, 68] of the site-keyed sources in the tuning range. A recall of those is wrong
for a reason that is neither staleness nor a key collision: the reasoner was wrong the first time. (For the family-keyed form I count a source answer
as right when it was right for its own incident; one right in kind and wrong in site would still serve a key that substitutes the site, so the family-keyed
wrong share is an upper estimate.) This error is the same for every key form and every arm fed by the same reasoner, which is why section 10.2 keeps it apart.

## 8. Item 7: world B (hidden; items 1, 2 and 5)

Recurrence 0.6, regime changes at 150 s (signature shift) and 300 s (added edge), seeds 50000-50099.

{{rec_summary_b}}

{{regime_b}}

Recurrence share of incidents 47.1% (hard 46.4%, decoy 56.1%); 12.6 recurrences per stream, 1.34 hard. Items 1 and 2
in the same tables as world A (sections 2.2 to 2.4 and 3 carry the b rows): the stale-by-construction share of hard recurrences is
11.2% [6.1, 16.5] (15 of 134), the family-elsewhere count 15 of 155 non-recurrence hard incidents (0.15 per stream;
recurrence or elsewhere reaches 51.6% [47.5, 55.0] of hard incidents), and the share of hard incidents after the first change 69.9%
[66.0, 73.8] (43.3% after the last). World B has 3.2 times the hard recurrences per stream (1.34 against 0.415); nothing is tuned on it. It is a transfer test: a memory whose benefit comes
only from rare recurrence in world A should gain in B in proportion to the recurrence rate, and an arm that tuned a
constant to world A's rate would show it.

## 9. What a weak key recalls wrongly (hidden; addition)

A1b's stale-error clause needs a floor that the world imposes, not one that a particular arm happens to reach.
Two bounds, both computed from the hidden tables with the memory's source taken to be a **correct** earlier
diagnosis (hard family, or "not an incident" for a decoy); neither depends on any arm. They count recalls
that would be wrong **if the memory fired on every match**; a confirmation policy or a stricter key lowers them.

**Site only** (the key is the site and nothing else), most recent hard incident at the site:

{{site_collisions}}

Within a stream a site-only memory recalls at 8.7% of incidents (held-out) and is wrong at 82% of those: 1.9 wrong recalls per
stream; for a plain incident or a decoy at a site where a hard incident happened, always. Carried across streams
(site ids are numbered per stream) 98.6% of incidents have a site in memory and 98.4% of those recalls are wrong (26 per stream):
**a site-keyed engram carried across stream boundaries is a stale-error generator by construction**, and
any A1b arm that persists site-keyed state across segments must reset it at the boundary or key without the site id.

**Phase 1** (the key is the site and the class of the first seconds, what the public rules see before phase 2),
most recent hard incident or decoy of the same key:

{{phase1_collisions}}

The class is read from the code (section 1.2): a hard Mimic incident's burst is the plain burst of the kind it imitates, and
a decoy presents as the family it pretends to be. A memory that fires on the first seconds' evidence and a site is wrong in 14.7% of its recalls
(held-out 38 of 259; tuning 22 of 142): 0.19 and 0.22 wrong per stream, mostly plain incidents at a site with an earlier Mimic of the same imitated kind and
hard-versus-decoy confusions. The family-keyed form (class only, the site taken from the new incident) is wrong in 57% of its recalls, 1.9 per stream, 1.4 of them
plain incidents that look like a mimic's burst. **A family-keyed memory can only be safe if its key waits for the phase-2 evidence or the
resolution** (6 to 16 s after onset; a decoy's resolution median 19 s), which is the hard-versus-decoy problem restated:
what separates them arrives about when the shortest hard deadlines expire.

## 10. What binds A1b, and the bounds I propose

### 10.1 Which items bound A1b's feasibility

- **Volume (items 1, 2, 6).** The held-out range has 83 hard recurrences, of which at most 41 are reachable by an arm whose earlier
  declaration was right, on 36 streams out of 200: **A1b's memory effect lives in about 40 incidents** (tuning: 18 reachable, 15 streams
  of 100). The record rung, which the charter calls the comparator that has captured every earlier lever, can reach most of them. If two arms
  disagree on 10 of the 41 incidents, the 90% half-width of the difference in their recall share is 0.127 (sqrt(10)/41 x 1.645), about 5
  incidents: a smaller difference is not resolvable, and the tuning range cannot rank more than a couple of configurations on 18 events.
- **Cost (item 6).** The most any site-keyed memory can save is 8.5% [6.4, 10.8] of the bill (family-keyed inside a stream 13.2%): the
  interval on the ceiling itself is +/-2 points of bill, so a cost clause of "cheaper than the record rung" cannot be resolved at a margin
  under about 3 points, and if the record rung captures most of `R` (it should: it is site and signature keyed), the room above it is
  smaller than the interval.
- **Experience across streams (items 1 and 6).** Site-keyed recurrence has no experience curve across streams; the rate is flat (section 2.3). Only
  inside a stream does the share of recurrences rise (0 to 0.25 by the tenth incident). Only the family-keyed form can improve with experience
  across streams (reach 97.8%), and only if its key survives section 9.
- **Stale errors (items 5, 9, 7.6).** 14.5% of hard recurrences are stale by construction (misses); a site-only key is wrong at 82% of its recalls
  and a carried one at 98%; a phase-1 site-and-class key is wrong at 14.7% when its sources are right; and 40% to 54% of the sources a memory fed by
  this reasoner would store are wrong, whatever the key.
- **Items 3 and 4** bound the key, not the clause: the vocabulary is stream-local and its id is a 13%-precision marker; a cascade's partner pair
  recurs 0.08 times per stream, so A2's evidence is thinner than A1b's.

None of the items makes A1b infeasible. It makes it small: the site-keyed lever is at most 8.5% of the bill and 0.2 incidents per stream.

### 10.2 Proposed bounds

**Unasked-correct clause.**

- *Population*: **hard recurrences only** (`recurrence_of` set, tier hard; 83 in 40000-40199, 44 in 10000-10099). Over all recurrences the
  clause is passed by the cheap rung with no memory (85.5% of plain recurrences, section 7.5), and the decoys' answer is "not an
  incident", which no engram diagnoses.
- *Ceilings that the number should be read against*: 0.49 [41/83] for a site-keyed memory fed by the re-anchor arm's own declarations
  (0.41 tuning, 18 of 44), **0.37 [31/83] for a key that cannot match the physics-altered recurrences** (0.36 tuning, 16 of 44).
  An arm with a better context would have a higher ceiling, up to every hard recurrence (`R_world`).
- *Bound*: the engram arm's unasked-correct count over hard recurrences is **at least 0.15 of them (12 of 83), with the 90% cluster lower
  bound above 0.075**, on the held-out range. 0.15 is 40% of the signature-keyed ceiling (31% of the site-keyed one); an arm below it recalls
  less than two reachable incidents in five. The interval clause, by the normal approximation: with 12 of 83 (0.145) the standard error is 0.039, the
  90% half-width 0.064 and the lower bound 0.081, above 0.075; with 11 of 83 the lower bound is 0.071. The clause cannot be met with fewer than 12.
- *The comparison with the record rung is a separate clause and the world cannot support a margin under 0.06*: with 83 hard recurrences and
  about 10 incidents on which two arms disagree the 90% half-width of a paired difference in share is 0.063 (sqrt(10)/83 x 1.645). I propose
  a margin of **+0.10 of hard recurrences (8 incidents net) for the engram over the record rung of the same key form, lower bound above 0**, and I would pre-accept that
  if the record rung's share is above 0.27 (about 73% of the signature-keyed ceiling), +0.10 is out of reach for any arm and the outcome is
  the charter's "the record rung captures the lever".
- *Not a clause*: the slope of unasked-correct decisions against incidents seen **across streams** for a site-keyed engram. It is flat by
  construction (section 2.3, 7.3); a positive slope there would be an artefact. A slope inside a stream (position of the incident) or across
  streams for the family-keyed form is meaningful.

**Stale-error clause.** Two kinds of wrong recall, never merged (charter 1.2: a memory-made wrong decision is its own kind of error):

- *(a) Collision and staleness*: the recall's stored answer was **right for its own incident** and wrong for the new one (the key matched a
  different truth, or the physics had changed). *Bound*: **at most 0.20 of such recalls are wrong, and at most 0.25 per stream**, on both ranges, with
  the upper 90% bound of the share no higher than 0.30. *Reasons*: the world's floor for a phase-1 site-and-class key that fires on every match, with right
  sources, is 0.147 of recalls (0.155 tuning) and 0.19 wrong per stream (0.22 tuning), so the bound admits a competent site-keyed arm with a small margin;
  a site-only key fails the share by a factor of four (0.82) and the count by 7.6 within a stream (1.9 per stream) and 100 across streams (26 per stream);
  a family-keyed key that fires in phase 1 fails the count by 7.7 (1.9 per stream), which is the point: it has to wait for evidence.
- *(b) Inherited*: the stored answer was **wrong for its own incident** (section 7.6). It is bounded by the reasoner, not by the memory: 40% [28, 51] of
  site-keyed sources held-out and 54% [38, 68] tuning. I propose **no absolute bound**; the clause is the paired comparison "no more inherited wrong
  recalls than the record rung of the same key form on the same streams", and its count is reported with the unasked-correct share, because an arm that
  recalls less inherits less. A confirmation policy is the lever against it; the clause tells whether it works.
- *Measurement*: the evaluator cannot tell (a) from (b) unless the arm's recall names its source (the observation the engram was bound at), from which the
  evaluator reads the source incident's truth. E1's `stale_wrong` as specified (a wrong declaration with no escalation) counts both and also the cheap
  rung's own errors (17% of plain incidents, 73% of decoys, section 7.5); it should be computed over declarations whose source is a recall, or as the paired
  excess over the memoryless arm, and E1 should record the source.
- *Carried state*: the clause is evaluated twice for any arm that persists site-keyed state across segments, with and without the reset at the stream
  boundary; the carried version fails by construction (98%), and that result is to be reported, not tuned away.
- *Staleness by regime*: recurrences stale by construction are misses; I propose reporting them as a separate row (14.5% of hard recurrences
  held-out) and counting a **miss on a stale recurrence** as no failure of either clause. Recovery after a change (the charter's EXP-104 measure) has 12 stale hard
  recurrences held-out to work with, so only an aggregate over streams, not a per-stream recovery time, is estimable.

## 11. Uncertainty, what would change the conclusions, and what the chief should examine

**Least sure of.**

1. **The ceiling is conditional on the arm.** `R` is 49% of the hard recurrences because this arm's hard declarations are right 48% of the time at b = 5,
   rho = 0.7, 16 s, rung context. A stronger reasoner or context raises it toward `R_world` (16.4% of the bill); a weaker lowers it. It is the
   ceiling for this arm, not a theorem about the world. A different noticer (R6's rung at z = 3) would change which incidents are asked.
2. **Timing rule.** "Before the repeating incident's first notice" is my reading; I did not count recalls that a faster-than-notice memory
   would make. The strict reading removes a few: `R_chain` and `R` differ by 3 incidents held-out and 4 tuning.
3. **Few events.** 83 hard recurrences, 41 reachable, 12 stale. Every ceiling interval is +/-2 points of bill, and tuning and held-out
   ranges disagree on the family-keyed ceiling (20.9% against 13.2%); I report both and do not average.
4. **The vocabulary floor** depends on my labelling of background messages by the live hard family and on a permutation null; a different
   floor (for example matched on service) would move the numbers. The qualitative result, that the id carries the family only
   when attributed to an incident, does not depend on it.
5. **The phase-1 classes** are from the code. A test that regenerates tier-swapped pairs would confirm the equivalence of a Mimic and a plain
   burst in the public stream; I did not run it.
6. **Inherited error** (7.6) is counted on the source's correctness for its own incident, not on the kind alone, and a source with calls of both kinds is
   `mixed` (4 of 183); the arm asks each hard incident once in 95% of cases, so the approximation is small, but I did not read the answers' kinds from the ledger.
7. **No arm was run against any of this.** The bounds are the world's side; whether the record rung or an engram approaches them is E1 and A1b.

**What the chief should examine most carefully.**

- The **join** in `w2_ceiling.py` (`join_run`): the focus-to-incident map is the owner file the `laws` example writes; the checks are asserts, and the
  held-out run is identical to L1's.
- The **`oracle::rebuilt_incident` accessor** and its test: it is new code in the stream crate (behind the hidden-state feature); everything about
  "altered" and "stale by construction" rests on it, and its rebuild of an incident whose only dependent came with the added edge falls back to the
  identified presentation, as the generator does.
- The **definition of reach** (`reach_flags`): `R <= R_chain <= F_strict <= F` and `R <= F` are asserted; the timing uses `first_notice_at_ns`.
- The **bounds' arithmetic** in 10.2, whether the chief wants the unasked-correct clause on hard recurrences only, and the split of the stale-error clause into
  (a) and (b): the first draft of my own bound (0.20 of all recalls) was unattainable, because 40-54% of a reasoner-fed memory's recalls inherit a wrong answer.
- The **cross-stream finding** (7.3, 9): it says that a carried site-keyed memory is wrong by construction and that the only lever with reach across streams is the
  family-keyed form. If A1a carries state across segments keyed by site (the L1 `state_key` store is the precedent), it is exposed.

**What would change my conclusions.** A different arm with a higher hard-incident quality would raise `R`; a key that uses the phase-2 evidence
would lower the section 9 exposures; an E1 record rung that recalls less than 0.27 of the hard recurrences would leave room for an engram; a
world in which recurrence crosses streams (a persistent service graph) would make site keys meaningful and the experience curve non-flat.

**The improved question.** Not "does an engram beat the record rung at recalling recurrences" (at most 8.5% of the bill and 0.2 incidents a stream, inside
the interval of the difference) but "what is the family-level law worth when carried across streams, and does a key built from public evidence recognise
the family before the deadline without being fooled by plain incidents and decoys" (reach 97.8%; confusable at phase 1; section 9).

**Smallest high-information next steps.** (i) E1's record rung on 10000-10099 under the clause above, in the site-keyed form with and without the stream
reset: it tests section 9 directly. (ii) A hidden-side collision table for a key that includes the phase-2 evidence classes, to give the family-keyed
form its real floor. (iii) A world C with three times the hard share, so that 40 reachable events become 120: the cheapest way to give A1b power.
(iv) An A2 pre-check on the pair recurrence in section 5 before A2 is built.

## 12. Reproduction and gates

`experiments/exploration/scripts/w2_run.sh` lists the order (`w2_hidden.sh`, `w2_manifests.py` and `scripts/run-driver.sh`, `w2_laws.py`, `w2_ceiling.py`,
`w2_provenance.py`, `w2_assemble.py`). The run directories are under `artifacts/runs/w2/` (git-ignored; kept until the chief has verified them):
`hidden/{a-tune,a-heldout,b}` (tables, `usage.json`, owner maps), `w2-tune-b5-rho0.7` and `w2-heldout-b5-rho0.7` (the ceiling's runs with the
ledger, `usage.json`), `_manifests/`. File hashes are in `w2-provenance.csv`. Preflight before every cargo command and every run: free disk 12 GB (above the
6 GB floor), `pgrep -x gordian-run` empty; no wait was needed, and Lab 1's processes were not seen at any check. Peak memory of the release build
0.81 GB; of the runs 77 MB.

{{gates}}
