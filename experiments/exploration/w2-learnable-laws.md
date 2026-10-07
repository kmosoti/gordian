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

| seeds (hidden) | group | incidents | per stream | recurrences | recurrences per stream [90%] | share of incidents [90%] |
|---|---|---|---|---|---|---|
| 10000-10099 | all | 2643 | 26.43 | 550 | 5.500 [5.080, 5.930] | 0.208 [0.195, 0.222] |
| 10000-10099 | tier=plain | 2110 | 21.10 | 437 | 4.370 [3.970, 4.770] | 0.207 [0.192, 0.223] |
| 10000-10099 | tier=hard | 243 | 2.43 | 44 | 0.440 [0.300, 0.590] | 0.181 [0.138, 0.222] |
| 10000-10099 | tier=decoy | 290 | 2.90 | 69 | 0.690 [0.510, 0.880] | 0.238 [0.191, 0.283] |
| 10000-10099 | hard/compound | 72 | 0.72 | 16 | 0.160 [0.070, 0.260] | 0.222 [0.122, 0.302] |
| 10000-10099 | hard/cascade | 63 | 0.63 | 13 | 0.130 [0.060, 0.210] | 0.206 [0.119, 0.288] |
| 10000-10099 | hard/split_brain | 64 | 0.64 | 10 | 0.100 [0.050, 0.160] | 0.156 [0.088, 0.219] |
| 10000-10099 | hard/slow_leak | 44 | 0.44 | 5 | 0.050 [0.010, 0.100] | 0.114 [0.030, 0.191] |
| 40000-40199 | all | 5344 | 26.72 | 1095 | 5.475 [5.210, 5.740] | 0.205 [0.196, 0.214] |
| 40000-40199 | tier=plain | 4304 | 21.52 | 883 | 4.415 [4.160, 4.670] | 0.205 [0.195, 0.216] |
| 40000-40199 | tier=hard | 512 | 2.56 | 83 | 0.415 [0.340, 0.495] | 0.162 [0.138, 0.186] |
| 40000-40199 | tier=decoy | 528 | 2.64 | 129 | 0.645 [0.535, 0.765] | 0.244 [0.212, 0.275] |
| 40000-40199 | hard/compound | 117 | 0.58 | 21 | 0.105 [0.065, 0.145] | 0.179 [0.124, 0.233] |
| 40000-40199 | hard/cascade | 132 | 0.66 | 15 | 0.075 [0.040, 0.115] | 0.114 [0.065, 0.164] |
| 40000-40199 | hard/split_brain | 149 | 0.74 | 26 | 0.130 [0.090, 0.175] | 0.174 [0.129, 0.219] |
| 40000-40199 | hard/slow_leak | 114 | 0.57 | 21 | 0.105 [0.065, 0.150] | 0.184 [0.126, 0.240] |

By mode, plain kind and decoy family:

| seeds (hidden) | group | incidents | per stream | recurrences | recurrences per stream [90%] | share of incidents [90%] |
|---|---|---|---|---|---|---|
| 10000-10099 | hard/compound/mimic | 35 | 0.35 | 6 | 0.060 [0.020, 0.110] | 0.171 [0.069, 0.265] |
| 10000-10099 | hard/compound/contradict | 37 | 0.37 | 10 | 0.100 [0.040, 0.170] | 0.270 [0.143, 0.375] |
| 10000-10099 | hard/cascade/mimic | 29 | 0.29 | 6 | 0.060 [0.010, 0.120] | 0.207 [0.056, 0.325] |
| 10000-10099 | hard/cascade/contradict | 34 | 0.34 | 7 | 0.070 [0.030, 0.120] | 0.206 [0.098, 0.303] |
| 10000-10099 | hard/split_brain/mimic | 32 | 0.32 | 9 | 0.090 [0.040, 0.150] | 0.281 [0.167, 0.378] |
| 10000-10099 | hard/split_brain/contradict | 32 | 0.32 | 1 | 0.010 [0.000, 0.030] | 0.031 [0.000, 0.087] |
| 10000-10099 | plain/ResourceExhausted | 375 | 3.75 | 74 | 0.740 [0.580, 0.900] | 0.197 [0.164, 0.231] |
| 10000-10099 | plain/ConfigDrift | 402 | 4.02 | 70 | 0.700 [0.530, 0.880] | 0.174 [0.139, 0.208] |
| 10000-10099 | plain/DependencyDown | 434 | 4.34 | 81 | 0.810 [0.650, 0.980] | 0.187 [0.156, 0.216] |
| 10000-10099 | plain/CredentialExpired | 461 | 4.61 | 109 | 1.090 [0.880, 1.310] | 0.236 [0.203, 0.269] |
| 10000-10099 | plain/Intermittent | 438 | 4.38 | 103 | 1.030 [0.810, 1.260] | 0.235 [0.198, 0.271] |
| 10000-10099 | plain/duo | 198 | 1.98 | 36 | 0.360 [0.270, 0.460] | 0.182 [0.144, 0.217] |
| 10000-10099 | decoy/compound | 70 | 0.70 | 20 | 0.200 [0.100, 0.310] | 0.286 [0.172, 0.386] |
| 10000-10099 | decoy/cascade | 59 | 0.59 | 9 | 0.090 [0.020, 0.170] | 0.152 [0.043, 0.253] |
| 10000-10099 | decoy/split_brain | 77 | 0.77 | 15 | 0.150 [0.070, 0.240] | 0.195 [0.107, 0.277] |
| 10000-10099 | decoy/slow_leak | 84 | 0.84 | 25 | 0.250 [0.149, 0.370] | 0.298 [0.213, 0.366] |
| 40000-40199 | hard/compound/mimic | 56 | 0.28 | 11 | 0.055 [0.030, 0.085] | 0.196 [0.119, 0.266] |
| 40000-40199 | hard/compound/contradict | 61 | 0.30 | 10 | 0.050 [0.020, 0.085] | 0.164 [0.082, 0.242] |
| 40000-40199 | hard/cascade/mimic | 59 | 0.29 | 6 | 0.030 [0.010, 0.055] | 0.102 [0.036, 0.170] |
| 40000-40199 | hard/cascade/contradict | 73 | 0.36 | 9 | 0.045 [0.015, 0.080] | 0.123 [0.053, 0.197] |
| 40000-40199 | hard/split_brain/mimic | 86 | 0.43 | 15 | 0.075 [0.040, 0.110] | 0.174 [0.110, 0.239] |
| 40000-40199 | hard/split_brain/contradict | 63 | 0.32 | 11 | 0.055 [0.030, 0.085] | 0.175 [0.103, 0.239] |
| 40000-40199 | plain/ResourceExhausted | 878 | 4.39 | 189 | 0.945 [0.805, 1.095] | 0.215 [0.191, 0.239] |
| 40000-40199 | plain/ConfigDrift | 911 | 4.55 | 206 | 1.030 [0.895, 1.180] | 0.226 [0.203, 0.250] |
| 40000-40199 | plain/DependencyDown | 900 | 4.50 | 183 | 0.915 [0.785, 1.050] | 0.203 [0.180, 0.226] |
| 40000-40199 | plain/CredentialExpired | 815 | 4.08 | 155 | 0.775 [0.660, 0.895] | 0.190 [0.168, 0.212] |
| 40000-40199 | plain/Intermittent | 800 | 4.00 | 150 | 0.750 [0.640, 0.865] | 0.188 [0.166, 0.209] |
| 40000-40199 | plain/duo | 528 | 2.64 | 111 | 0.555 [0.445, 0.670] | 0.210 [0.179, 0.240] |
| 40000-40199 | decoy/compound | 140 | 0.70 | 49 | 0.245 [0.175, 0.320] | 0.350 [0.284, 0.408] |
| 40000-40199 | decoy/cascade | 109 | 0.55 | 23 | 0.115 [0.065, 0.170] | 0.211 [0.140, 0.276] |
| 40000-40199 | decoy/split_brain | 131 | 0.66 | 22 | 0.110 [0.065, 0.155] | 0.168 [0.113, 0.219] |
| 40000-40199 | decoy/slow_leak | 148 | 0.74 | 35 | 0.175 [0.115, 0.240] | 0.236 [0.178, 0.288] |

Reading. The recurrence share is 0.205 of incidents in both default ranges, as the generator's 0.25
per eligible arrival implies once the first incidents (no template yet) and the busy arrivals are
counted. The hard tier recurs less (0.16) than the plain, and the decoys more (0.24 held-out), because the
tier of a recurrence is its template's and the pool of templates is 80% plain; only 2.4 to 2.6 hard incidents
exist in a stream to be repeated. By family the held-out rates differ (cascade 0.11, slow leak 0.18) by
amounts the intervals do not separate; I read them as one rate. Eligibility:

| seeds (hidden) | incidents | with an eligible template | recurrences | recurrences / eligible | recurrences with none eligible | of which repeat a recurrence |
|---|---|---|---|---|---|---|
| 10000-10099 | 2643 | 2155 | 550 | 0.2552 | 0 | 79 |
| 40000-40199 | 5344 | 4351 | 1095 | 0.2517 | 0 | 148 |
| 50000-50099 | 2679 | 2065 | 1262 | 0.6111 | 0 | 327 |

### 2.2 The gap to the incident repeated

For each recurrence the index of the incident it repeats, the gap in incidents and in seconds, and its staleness are in
`w2-recurrences.csv` (one row per recurrence, 2,907 rows over the three ranges); per stream counts by tier and family
are in `w2-perstream.csv`. The gaps:

| seeds (hidden) | recurrences | gap | n | mean | min | p10 | p50 | p90 | max |
|---|---|---|---|---|---|---|---|---|---|
| 10000-10099 | all | gap_incidents | 550 | 9.4 | 1.0 | 3.0 | 8.0 | 18.0 | 32.0 |
| 10000-10099 | all | gap_seconds | 550 | 182.7 | 36.1 | 73.5 | 152.2 | 352.4 | 518.7 |
| 10000-10099 | all | gap_since_template_ended_seconds | 550 | 136.5 | 23.1 | 29.7 | 110.9 | 301.0 | 489.5 |
| 10000-10099 | hard | gap_incidents | 44 | 9.1 | 2.0 | 4.0 | 8.0 | 16.7 | 22.0 |
| 10000-10099 | hard | gap_seconds | 44 | 176.1 | 65.7 | 79.8 | 151.1 | 337.1 | 404.9 |
| 10000-10099 | hard | gap_since_template_ended_seconds | 44 | 115.7 | 23.3 | 29.2 | 95.0 | 277.6 | 342.2 |
| 10000-10099 | hard | gap_hard_incidents | 44 | 2.2 | 1.0 | 1.0 | 2.0 | 4.0 | 5.0 |
| 40000-40199 | all | gap_incidents | 1095 | 9.6 | 1.0 | 4.0 | 8.0 | 18.0 | 32.0 |
| 40000-40199 | all | gap_seconds | 1095 | 186.8 | 32.9 | 76.9 | 157.5 | 352.0 | 526.5 |
| 40000-40199 | all | gap_since_template_ended_seconds | 1095 | 140.2 | 23.0 | 32.6 | 106.5 | 307.1 | 505.4 |
| 40000-40199 | hard | gap_incidents | 83 | 10.1 | 1.0 | 4.0 | 8.0 | 19.8 | 24.0 |
| 40000-40199 | hard | gap_seconds | 83 | 202.9 | 58.6 | 82.7 | 158.2 | 393.6 | 478.9 |
| 40000-40199 | hard | gap_since_template_ended_seconds | 83 | 144.8 | 23.7 | 30.5 | 99.7 | 353.4 | 430.5 |
| 40000-40199 | hard | gap_hard_incidents | 83 | 1.8 | 1.0 | 1.0 | 1.0 | 3.0 | 5.0 |
| 50000-50099 | all | gap_incidents | 1262 | 9.5 | 1.0 | 4.0 | 8.0 | 17.0 | 31.0 |
| 50000-50099 | all | gap_seconds | 1262 | 182.8 | 33.4 | 77.0 | 155.8 | 336.4 | 535.0 |
| 50000-50099 | all | gap_since_template_ended_seconds | 1262 | 135.6 | 23.0 | 31.9 | 108.2 | 292.1 | 486.3 |
| 50000-50099 | hard | gap_incidents | 134 | 9.6 | 2.0 | 3.3 | 8.0 | 18.0 | 29.0 |
| 50000-50099 | hard | gap_seconds | 134 | 190.7 | 55.9 | 78.2 | 166.1 | 344.5 | 514.4 |
| 50000-50099 | hard | gap_since_template_ended_seconds | 134 | 126.7 | 23.3 | 27.0 | 91.2 | 297.7 | 474.5 |
| 50000-50099 | hard | gap_hard_incidents | 134 | 2.1 | 1.0 | 1.0 | 2.0 | 4.0 | 8.0 |

A repeat is never sooner than 23 s after its template ended (the template's services stay reserved), the
median is 157 s after the template began (8 incidents later), and the 90th percentile 352 s. A hard
incident repeats a hard one that is usually one or two hard incidents back (median 1).

### 2.3 The experience curve

| seeds (hidden) | incidents counted | seen (n) | streams reaching n | mean cumulative recurrences [90%] | share of the n-th that is a recurrence |
|---|---|---|---|---|---|
| 10000-10099 | all | 2 | 100 | 0.02 [0.00, 0.05] | 0.020 |
| 10000-10099 | all | 4 | 100 | 0.21 [0.14, 0.28] | 0.120 |
| 10000-10099 | all | 6 | 100 | 0.50 [0.40, 0.60] | 0.130 |
| 10000-10099 | all | 8 | 100 | 0.87 [0.74, 1.00] | 0.180 |
| 10000-10099 | all | 10 | 100 | 1.36 [1.20, 1.52] | 0.210 |
| 10000-10099 | all | 15 | 99 | 2.58 [2.32, 2.83] | 0.303 |
| 10000-10099 | all | 20 | 94 | 3.86 [3.53, 4.19] | 0.277 |
| 10000-10099 | all | 25 | 64 | 5.23 [4.81, 5.67] | 0.266 |
| 10000-10099 | all | 30 | 27 | 6.48 [5.59, 7.37] | 0.296 |
| 10000-10099 | hard | 1 | 83 | 0.00 [0.00, 0.00] | 0.000 |
| 10000-10099 | hard | 2 | 59 | 0.12 [0.05, 0.19] | 0.119 |
| 10000-10099 | hard | 3 | 40 | 0.35 [0.20, 0.50] | 0.275 |
| 10000-10099 | hard | 4 | 27 | 0.85 [0.63, 1.07] | 0.407 |
| 40000-40199 | all | 2 | 200 | 0.01 [0.00, 0.03] | 0.010 |
| 40000-40199 | all | 4 | 200 | 0.11 [0.07, 0.15] | 0.050 |
| 40000-40199 | all | 6 | 200 | 0.43 [0.36, 0.51] | 0.165 |
| 40000-40199 | all | 8 | 200 | 0.86 [0.76, 0.97] | 0.230 |
| 40000-40199 | all | 10 | 200 | 1.35 [1.23, 1.48] | 0.265 |
| 40000-40199 | all | 15 | 199 | 2.59 [2.41, 2.77] | 0.251 |
| 40000-40199 | all | 20 | 189 | 3.75 [3.54, 3.97] | 0.175 |
| 40000-40199 | all | 25 | 133 | 4.91 [4.62, 5.20] | 0.278 |
| 40000-40199 | all | 30 | 61 | 5.67 [5.23, 6.10] | 0.246 |
| 40000-40199 | hard | 1 | 175 | 0.00 [0.00, 0.00] | 0.000 |
| 40000-40199 | hard | 2 | 144 | 0.22 [0.16, 0.27] | 0.215 |
| 40000-40199 | hard | 3 | 99 | 0.43 [0.34, 0.53] | 0.202 |
| 40000-40199 | hard | 4 | 52 | 0.73 [0.58, 0.88] | 0.327 |
| 40000-40199 | hard | 5 | 28 | 1.14 [0.89, 1.43] | 0.214 |
| 50000-50099 | all | 2 | 100 | 0.03 [0.01, 0.06] | 0.030 |
| 50000-50099 | all | 4 | 100 | 0.29 [0.21, 0.37] | 0.170 |
| 50000-50099 | all | 6 | 100 | 0.88 [0.75, 1.02] | 0.410 |
| 50000-50099 | all | 8 | 100 | 1.61 [1.43, 1.79] | 0.440 |
| 50000-50099 | all | 10 | 100 | 2.74 [2.52, 2.96] | 0.610 |
| 50000-50099 | all | 15 | 100 | 5.59 [5.33, 5.86] | 0.600 |
| 50000-50099 | all | 20 | 98 | 8.54 [8.19, 8.88] | 0.602 |
| 50000-50099 | all | 25 | 66 | 11.65 [11.15, 12.15] | 0.682 |
| 50000-50099 | all | 30 | 25 | 14.24 [13.24, 15.24] | 0.720 |
| 50000-50099 | hard | 1 | 81 | 0.00 [0.00, 0.00] | 0.000 |
| 50000-50099 | hard | 2 | 58 | 0.43 [0.33, 0.53] | 0.431 |
| 50000-50099 | hard | 3 | 45 | 1.11 [0.96, 1.27] | 0.711 |
| 50000-50099 | hard | 4 | 36 | 1.75 [1.56, 1.94] | 0.722 |
| 50000-50099 | hard | 5 | 24 | 2.38 [2.17, 2.58] | 0.792 |
| 50000-50099 | hard | 6 | 20 | 2.95 [2.60, 3.25] | 0.600 |

Cumulative recurrences against incidents seen, averaged over the streams that reach n incidents (at least 20
streams). It is the best curve a memory could follow if it recalled every recurrence. It rises slowly
(0.01 recurrences after two incidents) and at about one recurrence per four incidents from n = 10: the
rate is a property of the template pool (the more incidents seen, the more eligible templates) and is
reached inside a stream. Across streams the same quantity is a straight line:

| seeds (hidden) | streams seen | cumulative hard incidents | cumulative hard recurrences | share |
|---|---|---|---|---|
| 10000-10099 | 10 | 27 | 3 | 0.111 |
| 10000-10099 | 25 | 50 | 6 | 0.120 |
| 10000-10099 | 50 | 116 | 18 | 0.155 |
| 10000-10099 | 75 | 170 | 28 | 0.165 |
| 10000-10099 | 100 | 243 | 44 | 0.181 |
| 40000-40199 | 10 | 18 | 3 | 0.167 |
| 40000-40199 | 25 | 57 | 9 | 0.158 |
| 40000-40199 | 50 | 129 | 19 | 0.147 |
| 40000-40199 | 75 | 190 | 29 | 0.153 |
| 40000-40199 | 100 | 243 | 36 | 0.148 |
| 40000-40199 | 150 | 393 | 66 | 0.168 |
| 40000-40199 | 200 | 512 | 83 | 0.162 |
| 50000-50099 | 10 | 27 | 14 | 0.518 |
| 50000-50099 | 25 | 81 | 41 | 0.506 |
| 50000-50099 | 50 | 132 | 61 | 0.462 |
| 50000-50099 | 75 | 209 | 95 | 0.455 |
| 50000-50099 | 100 | 289 | 134 | 0.464 |

The last two columns of the file `w2-experience-stream-order.csv` give the share of hard incidents that
are recurrences in the first and the second half of each range (a-tune 0.155 and 0.205; a-heldout 0.148 and
0.175; b 0.462 and 0.465). I did not test the difference; with 20 to 50 recurrences in a half, a difference of
0.03 to 0.05 is the size of the sampling error, which is what a stationary rate would give.

### 2.4 Recurrences stale by construction

| seeds (hidden) | group | recurrences | template before a change, repeat after (share) | stale: signature shift | stale: added edge | stale (either) share of recurrences [90%] | stale (either) count |
|---|---|---|---|---|---|---|---|
| 10000-10099 | all | 550 | 276 (0.50) | 39 | 21 | 0.109 [0.086, 0.133] | 60 |
| 10000-10099 | tier=plain | 437 | 222 (0.51) | 31 | 16 | 0.108 [0.080, 0.137] | 47 |
| 10000-10099 | tier=hard | 44 | 20 (0.45) | 2 | 2 | 0.091 [0.025, 0.170] | 4 |
| 10000-10099 | tier=decoy | 69 | 34 (0.49) | 6 | 3 | 0.130 [0.046, 0.222] | 9 |
| 10000-10099 | hard/compound | 16 | 8 (0.50) | 1 | 1 | 0.125 [0.000, 0.286] | 2 |
| 10000-10099 | hard/cascade | 13 | 3 (0.23) | 1 | 1 | 0.154 [0.000, 0.333] | 2 |
| 10000-10099 | hard/split_brain | 10 | 6 (0.60) | 0 | 0 | 0.000 [0.000, 0.000] | 0 |
| 10000-10099 | hard/slow_leak | 5 | 3 (0.60) | 0 | 0 | 0.000 [0.000, 0.000] | 0 |
| 40000-40199 | all | 1095 | 573 (0.52) | 107 | 38 | 0.128 [0.110, 0.146] | 140 |
| 40000-40199 | tier=plain | 883 | 471 (0.53) | 88 | 34 | 0.134 [0.114, 0.154] | 118 |
| 40000-40199 | tier=hard | 83 | 48 (0.58) | 9 | 4 | 0.145 [0.084, 0.211] | 12 |
| 40000-40199 | tier=decoy | 129 | 54 (0.42) | 10 | 0 | 0.077 [0.037, 0.122] | 10 |
| 40000-40199 | hard/compound | 21 | 14 (0.67) | 6 | 1 | 0.333 [0.167, 0.524] | 7 |
| 40000-40199 | hard/cascade | 15 | 7 (0.47) | 3 | 2 | 0.267 [0.091, 0.455] | 4 |
| 40000-40199 | hard/split_brain | 26 | 16 (0.62) | 0 | 1 | 0.038 [0.000, 0.111] | 1 |
| 40000-40199 | hard/slow_leak | 21 | 11 (0.52) | 0 | 0 | 0.000 [0.000, 0.000] | 0 |
| 50000-50099 | all | 1262 | 645 (0.51) | 94 | 94 | 0.143 [0.122, 0.164] | 181 |
| 50000-50099 | tier=plain | 954 | 496 (0.52) | 78 | 79 | 0.159 [0.136, 0.182] | 152 |
| 50000-50099 | tier=hard | 134 | 68 (0.51) | 10 | 7 | 0.112 [0.061, 0.165] | 15 |
| 50000-50099 | tier=decoy | 174 | 81 (0.47) | 6 | 8 | 0.081 [0.033, 0.134] | 14 |
| 50000-50099 | hard/compound | 27 | 10 (0.37) | 5 | 2 | 0.222 [0.062, 0.400] | 6 |
| 50000-50099 | hard/cascade | 26 | 18 (0.69) | 5 | 3 | 0.269 [0.105, 0.433] | 7 |
| 50000-50099 | hard/split_brain | 54 | 29 (0.54) | 0 | 2 | 0.037 [0.000, 0.089] | 2 |
| 50000-50099 | hard/slow_leak | 27 | 11 (0.41) | 0 | 0 | 0.000 [0.000, 0.000] | 0 |

Half of all recurrences repeat a template that began before a change and begin after it. Of all
recurrences 12.8% [11.0, 14.6] were altered by a change that took effect after their template began (held-out;
tuning 10.9%); of the hard ones 14.5% [8.4, 21.1] (12 of 83; tuning 4 of 44). The signature shift is the larger
part: compounds and cascades with the shifted kind in their physics (33% and 27% of hard compound and cascade
recurrences held-out), never split brains or slow leaks. Staleness is a **miss**, not a wrong answer: a recurrence
has the template's truth, so an old record is right about the diagnosis and wrong about the evidence.

## 3. Item 2: same family, different site (hidden)

| seeds (hidden) | key | group | hard | recurrences | non-recurrence hard | same family-and-mode earlier at another site | per stream [90%] | share of non-recurrence hard [90%] | recurrence or elsewhere, share of hard [90%] |
|---|---|---|---|---|---|---|---|---|---|
| 10000-10099 | fam_mode | all hard | 243 | 44 | 199 | 33 | 0.330 [0.220, 0.440] | 0.166 [0.122, 0.208] | 0.317 [0.261, 0.368] |
| 10000-10099 | fam_mode | compound | 72 | 16 | 56 | 12 | 0.120 [0.060, 0.190] | 0.214 [0.122, 0.298] | 0.389 [0.281, 0.471] |
| 10000-10099 | fam_mode | cascade | 63 | 13 | 50 | 8 | 0.080 [0.030, 0.130] | 0.160 [0.078, 0.240] | 0.333 [0.222, 0.429] |
| 10000-10099 | fam_mode | split_brain | 64 | 10 | 54 | 4 | 0.040 [0.010, 0.080] | 0.074 [0.017, 0.143] | 0.219 [0.132, 0.297] |
| 10000-10099 | fam_mode | slow_leak | 44 | 5 | 39 | 9 | 0.090 [0.040, 0.150] | 0.231 [0.118, 0.327] | 0.318 [0.184, 0.422] |
| 10000-10099 | strict | all hard | 243 | 44 | 199 | 20 | 0.200 [0.110, 0.300] | 0.101 [0.060, 0.141] | 0.263 [0.209, 0.312] |
| 40000-40199 | fam_mode | all hard | 512 | 83 | 429 | 51 | 0.255 [0.200, 0.315] | 0.119 [0.095, 0.143] | 0.262 [0.233, 0.290] |
| 40000-40199 | fam_mode | compound | 117 | 21 | 96 | 7 | 0.035 [0.015, 0.060] | 0.073 [0.034, 0.114] | 0.239 [0.179, 0.295] |
| 40000-40199 | fam_mode | cascade | 132 | 15 | 117 | 15 | 0.075 [0.045, 0.105] | 0.128 [0.084, 0.171] | 0.227 [0.172, 0.279] |
| 40000-40199 | fam_mode | split_brain | 149 | 26 | 123 | 12 | 0.060 [0.030, 0.095] | 0.098 [0.052, 0.143] | 0.255 [0.202, 0.304] |
| 40000-40199 | fam_mode | slow_leak | 114 | 21 | 93 | 17 | 0.085 [0.050, 0.120] | 0.183 [0.124, 0.239] | 0.333 [0.260, 0.398] |
| 40000-40199 | strict | all hard | 512 | 83 | 429 | 37 | 0.185 [0.135, 0.235] | 0.086 [0.065, 0.108] | 0.234 [0.206, 0.262] |
| 50000-50099 | fam_mode | all hard | 289 | 134 | 155 | 15 | 0.150 [0.090, 0.220] | 0.097 [0.061, 0.130] | 0.516 [0.475, 0.550] |
| 50000-50099 | fam_mode | compound | 68 | 27 | 41 | 3 | 0.030 [0.010, 0.060] | 0.073 [0.019, 0.139] | 0.441 [0.340, 0.522] |
| 50000-50099 | fam_mode | cascade | 56 | 26 | 30 | 2 | 0.020 [0.000, 0.050] | 0.067 [0.000, 0.140] | 0.500 [0.405, 0.574] |
| 50000-50099 | fam_mode | split_brain | 101 | 54 | 47 | 7 | 0.070 [0.030, 0.110] | 0.149 [0.073, 0.217] | 0.604 [0.526, 0.664] |
| 50000-50099 | fam_mode | slow_leak | 64 | 27 | 37 | 3 | 0.030 [0.010, 0.060] | 0.081 [0.024, 0.150] | 0.469 [0.372, 0.545] |
| 50000-50099 | strict | all hard | 289 | 134 | 155 | 10 | 0.100 [0.050, 0.150] | 0.065 [0.036, 0.093] | 0.498 [0.456, 0.535] |

Held-out, 51 of 429 non-recurrence hard incidents (11.9% [9.5, 14.3]; 0.255 per stream) have an earlier hard incident of the same family and mode at
another site; with the strict key (same kinds) 37 (8.6%). Tuning: 33 of 199 (16.6%) and 20 (10.1%). The sets
intersect little: of a stream's hard incidents the share a family-keyed memory could reach by experience is
0.262 [0.233, 0.290] held-out and 0.317 [0.261, 0.368] tuning, against 0.162 and 0.181 for the site-keyed one.

## 4. Item 3: the vocabulary (public ids, hidden labels)

| seeds (public ids, hidden labels) | sample | streams with two or more families | messages per stream | label entropy (bits) | MI(id; family) (bits) | permutation null mean | null p95 | excess over null [90%] | share of ids at exactly one family | same, ids seen at least twice |
|---|---|---|---|---|---|---|---|---|---|---|
| 10000-10099 | incident-borne (hard incidents' out-of-catalogue messages) | 49 of 100 | 16.1 | 1.101 | 1.101 | 0.587 | 0.816 | 0.514 [0.462, 0.567] | 1.000 | 1.000 |
| 10000-10099 | background free-form, labelled by the hard family live anywhere | 48 of 100 | 324.5 | 1.045 | 0.483 | 0.492 | 0.540 | -0.009 [-0.015, -0.003] | 0.756 | 0.211 |
| 10000-10099 | background free-form, labelled by the hard family live at its service | 49 of 100 | 59.8 | 1.019 | 0.792 | 0.808 | 0.896 | -0.017 [-0.030, -0.003] | 0.874 | 0.409 |
| 40000-40199 | incident-borne (hard incidents' out-of-catalogue messages) | 116 of 200 | 14.6 | 1.175 | 1.175 | 0.650 | 0.919 | 0.525 [0.500, 0.550] | 1.000 | 1.000 |
| 40000-40199 | background free-form, labelled by the hard family live anywhere | 116 of 200 | 291.0 | 1.093 | 0.524 | 0.524 | 0.577 | -0.001 [-0.005, 0.004] | 0.751 | 0.227 |
| 40000-40199 | background free-form, labelled by the hard family live at its service | 116 of 200 | 52.7 | 1.104 | 0.904 | 0.905 | 0.992 | -0.001 [-0.008, 0.007] | 0.895 | 0.425 |

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

| seeds (public ids, hidden labels) | free-form messages whose id is one of the stream's hard vocabulary | incident-borne | background | share incident-borne [90%] | share of incident-borne kept |
|---|---|---|---|---|---|
| 10000-10099 | any free-form message with a vocabulary id | 973 | 5952 | 0.141 [0.130, 0.151] | 1.000 |
| 10000-10099 | ... with at least 1 abnormal reading at its service in the previous 10 s | 885 | 3330 | 0.210 [0.196, 0.225] | 0.910 |
| 10000-10099 | ... with at least 2 | 808 | 2092 | 0.279 [0.260, 0.299] | 0.830 |
| 10000-10099 | ... with 3 or more | 726 | 1626 | 0.309 [0.286, 0.332] | 0.746 |
| 40000-40199 | any free-form message with a vocabulary id | 2077 | 13557 | 0.133 [0.128, 0.138] | 1.000 |
| 40000-40199 | ... with at least 1 abnormal reading at its service in the previous 10 s | 1847 | 7778 | 0.192 [0.184, 0.200] | 0.889 |
| 40000-40199 | ... with at least 2 | 1690 | 4830 | 0.259 [0.249, 0.270] | 0.814 |
| 40000-40199 | ... with 3 or more | 1477 | 3770 | 0.281 [0.269, 0.294] | 0.711 |

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

| seeds (hidden) | tier | family | mode | partner is a dependent of the site | n | unit | mean | p10 | p50 | p90 |
|---|---|---|---|---|---|---|---|---|---|---|
| 10000-10099 | decoy | cascade | contradict | 0 | 34 | ms | 84.3 | 27.8 | 91.4 | 128.5 |
| 10000-10099 | decoy | split_brain | contradict | 0 | 21 | ms | 101.7 | 65.9 | 102.1 | 144.0 |
| 10000-10099 | decoy | split_brain | contradict | 1 | 22 | ms | 46.4 | 25.3 | 43.0 | 59.3 |
| 10000-10099 | decoy | split_brain | mimic | 1 | 4 | s | 0.0 | 0.0 | 0.0 | 0.1 |
| 10000-10099 | hard | cascade | contradict | 0 | 34 | ms | 89.8 | 48.2 | 94.3 | 144.1 |
| 10000-10099 | hard | cascade | mimic | 0 | 29 | s | 11.1 | 7.0 | 11.2 | 14.9 |
| 10000-10099 | hard | split_brain | contradict | 0 | 22 | ms | 86.0 | 35.6 | 92.7 | 136.8 |
| 10000-10099 | hard | split_brain | contradict | 1 | 10 | ms | 41.1 | 27.0 | 41.1 | 54.5 |
| 10000-10099 | hard | split_brain | mimic | 0 | 19 | s | 10.3 | 7.4 | 9.3 | 14.3 |
| 10000-10099 | hard | split_brain | mimic | 1 | 13 | s | 5.9 | 0.0 | 0.1 | 14.2 |
| 10000-10099 | decoy | cascade | mimic |  | 25 | no partner alarm |  |  |  |  |
| 10000-10099 | decoy | split_brain | mimic |  | 30 | no partner alarm |  |  |  |  |
| 40000-40199 | decoy | cascade | contradict | 0 | 43 | ms | 88.9 | 35.8 | 91.4 | 143.0 |
| 40000-40199 | decoy | split_brain | contradict | 0 | 46 | ms | 83.3 | 37.6 | 77.8 | 133.8 |
| 40000-40199 | decoy | split_brain | contradict | 1 | 28 | ms | 52.0 | 25.6 | 38.6 | 87.8 |
| 40000-40199 | decoy | split_brain | mimic | 1 | 12 | s | 0.0 | 0.0 | 0.0 | 0.1 |
| 40000-40199 | hard | cascade | contradict | 0 | 73 | ms | 84.4 | 27.7 | 83.9 | 143.8 |
| 40000-40199 | hard | cascade | mimic | 0 | 59 | s | 11.4 | 7.3 | 11.7 | 14.8 |
| 40000-40199 | hard | split_brain | contradict | 0 | 40 | ms | 88.2 | 33.5 | 92.9 | 132.4 |
| 40000-40199 | hard | split_brain | contradict | 1 | 23 | ms | 50.6 | 23.1 | 41.8 | 94.2 |
| 40000-40199 | hard | split_brain | mimic | 0 | 64 | s | 10.8 | 7.0 | 10.6 | 15.2 |
| 40000-40199 | hard | split_brain | mimic | 1 | 22 | s | 6.0 | 0.0 | 6.7 | 13.5 |
| 40000-40199 | decoy | cascade | mimic |  | 66 | no partner alarm |  |  |  |  |
| 40000-40199 | decoy | split_brain | mimic |  | 45 | no partner alarm |  |  |  |  |

A cascade's partner alarm comes 22-150 ms after the root's in Contradict mode (mean 84 ms) and 6.2-15.8 s in
Mimic mode, where it is phase-2 evidence; the decoys of a Mimic cascade never show it (66 held-out). A
split brain's peer that is also a dependent of the site alarms about 40-50 ms in in Contradict mode (mean 41 ms
tuning, 51 ms held-out; it is an ordinary dependent alarm, W1's finding): a rule that reads "an alarm at a service
the graph does not connect" would not fire for these. For a dependent peer in Mimic mode the first abnormal reading
at the peer is bimodal (the site's own burst alarms it within 60 ms; the phase-2 peer alarm comes 6-16 s in), so its
mean (6 s) describes neither, and the decoys of a Mimic split brain whose peer is a dependent show the first mode only.

The added edge:

| seeds (hidden) | incidents | edge added at (s) | streams with an edge | after it, per stream | newly downstream of the site or partner, per stream | alarm over the new edge, per stream [90%] | total | share of those after |
|---|---|---|---|---|---|---|---|---|
| 10000-10099 | all | 400 | 100 | 6.73 | 1.09 | 0.99 [0.83, 1.15] | 99 | 0.147 |
| 10000-10099 | plain | 400 | 100 | 5.36 | 0.84 | 0.81 [0.67, 0.96] | 81 | 0.151 |
| 10000-10099 | hard | 400 | 100 | 0.64 | 0.11 | 0.07 [0.03, 0.11] | 7 | 0.109 |
| 10000-10099 | decoy | 400 | 100 | 0.73 | 0.14 | 0.11 [0.06, 0.16] | 11 | 0.151 |
| 40000-40199 | all | 400 | 200 | 7.21 | 1.09 | 0.97 [0.87, 1.07] | 194 | 0.134 |
| 40000-40199 | plain | 400 | 200 | 5.90 | 0.84 | 0.79 [0.70, 0.89] | 158 | 0.134 |
| 40000-40199 | hard | 400 | 200 | 0.63 | 0.14 | 0.11 [0.07, 0.15] | 22 | 0.175 |
| 40000-40199 | decoy | 400 | 200 | 0.69 | 0.11 | 0.07 [0.04, 0.10] | 14 | 0.102 |

The edge is added at 400 s; 1.09 incidents per stream (held-out) have a service newly downstream of their site or of
their cascade's partner, and 0.97 per stream alarm over it (the others have a new dependent that their presentation does not alarm: some alarm only the first three
dependents or one chosen at random): 0.11 hard incidents per stream, 0.79 plain. This is the number of
events from which a policy could learn the new edge from co-alarm timing: about one per stream, mostly plain.

Pair recurrence:

| seeds (hidden) | family | tier | incidents | per stream | flagged recurrences | ordered pair seen earlier | unordered pair seen earlier | root seen earlier | pair repeats not flagged | incomparable pairs per graph |
|---|---|---|---|---|---|---|---|---|---|---|
| 10000-10099 | cascade | hard | 63 | 0.63 | 13 | 13 | 14 | 14 | 0 | 16.5 |
| 10000-10099 | cascade | decoy | 59 | 0.59 | 9 | 9 | 10 | 9 | 0 | 16.5 |
| 10000-10099 | split_brain | hard | 64 | 0.64 | 10 | 10 | 10 | 11 | 0 | 16.5 |
| 10000-10099 | split_brain | decoy | 77 | 0.77 | 15 | 15 | 15 | 16 | 0 | 16.5 |
| 40000-40199 | cascade | hard | 132 | 0.66 | 15 | 16 | 17 | 18 | 1 | 16.4 |
| 40000-40199 | cascade | decoy | 109 | 0.55 | 23 | 24 | 24 | 27 | 1 | 16.4 |
| 40000-40199 | split_brain | hard | 149 | 0.74 | 26 | 26 | 26 | 28 | 0 | 16.4 |
| 40000-40199 | split_brain | decoy | 131 | 0.66 | 22 | 22 | 24 | 24 | 0 | 16.4 |

A (root, partner) pair recurs almost only as a flagged recurrence: held-out, 16 pair repeats among 132 hard cascades,
15 of them recurrences; 26 of 149 hard split brains, all flagged. Pairs available per graph (incomparable pairs):
16.4. A hidden edge learned from the first occurrence is thus reusable 0.08 times per stream (hard cascades: one
repeat per 12.5 streams) and 0.13 (split brains), and an A2 that waits for a second sighting of a pair has one
sighting to learn from in 12 streams or so.

## 6. Item 5: regime changes (hidden)

| seeds (hidden) | change | group | incidents after it, per stream | affected | affected per stream [90%] | share of those after it [90%] |
|---|---|---|---|---|---|---|
| 10000-10099 | signature_shift at 200 s | all | 16.68 | 281 | 2.810 [2.540, 3.080] | 0.169 [0.153, 0.185] |
| 10000-10099 | signature_shift at 200 s | tier=plain | 13.38 | 236 | 2.360 [2.100, 2.630] | 0.176 [0.159, 0.195] |
| 10000-10099 | signature_shift at 200 s | tier=hard | 1.51 | 23 | 0.230 [0.140, 0.320] | 0.152 [0.100, 0.209] |
| 10000-10099 | signature_shift at 200 s | tier=decoy | 1.79 | 22 | 0.220 [0.130, 0.320] | 0.123 [0.075, 0.177] |
| 10000-10099 | signature_shift at 200 s | hard/compound | 0.49 | 18 | 0.180 [0.100, 0.270] | 0.367 [0.222, 0.521] |
| 10000-10099 | signature_shift at 200 s | hard/cascade | 0.27 | 5 | 0.050 [0.020, 0.090] | 0.185 [0.070, 0.323] |
| 10000-10099 | signature_shift at 200 s | hard/split_brain | 0.44 | 0 | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] |
| 10000-10099 | signature_shift at 200 s | hard/slow_leak | 0.31 | 0 | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] |
| 10000-10099 | signature_shift at 200 s | decoy/compound | 0.45 | 14 | 0.140 [0.060, 0.230] | 0.311 [0.162, 0.458] |
| 10000-10099 | signature_shift at 200 s | decoy/cascade | 0.37 | 8 | 0.080 [0.040, 0.130] | 0.216 [0.111, 0.333] |
| 10000-10099 | edge_add at 400 s | all | 6.73 | 99 | 0.990 [0.830, 1.150] | 0.147 [0.126, 0.169] |
| 10000-10099 | edge_add at 400 s | tier=plain | 5.36 | 81 | 0.810 [0.670, 0.960] | 0.151 [0.128, 0.175] |
| 10000-10099 | edge_add at 400 s | tier=hard | 0.64 | 7 | 0.070 [0.030, 0.110] | 0.109 [0.049, 0.179] |
| 10000-10099 | edge_add at 400 s | tier=decoy | 0.73 | 11 | 0.110 [0.060, 0.160] | 0.151 [0.084, 0.224] |
| 10000-10099 | edge_add at 400 s | hard/compound | 0.17 | 2 | 0.020 [0.000, 0.040] | 0.118 [0.000, 0.273] |
| 10000-10099 | edge_add at 400 s | hard/cascade | 0.10 | 3 | 0.030 [0.010, 0.060] | 0.300 [0.077, 0.556] |
| 10000-10099 | edge_add at 400 s | hard/split_brain | 0.19 | 2 | 0.020 [0.000, 0.040] | 0.105 [0.000, 0.250] |
| 10000-10099 | edge_add at 400 s | hard/slow_leak | 0.18 | 0 | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] |
| 10000-10099 | edge_add at 400 s | decoy/compound | 0.16 | 3 | 0.030 [0.000, 0.060] | 0.188 [0.000, 0.357] |
| 10000-10099 | edge_add at 400 s | decoy/cascade | 0.18 | 5 | 0.050 [0.020, 0.090] | 0.278 [0.111, 0.444] |
| 40000-40199 | signature_shift at 200 s | all | 16.98 | 572 | 2.860 [2.655, 3.070] | 0.168 [0.157, 0.180] |
| 40000-40199 | signature_shift at 200 s | tier=plain | 13.79 | 490 | 2.450 [2.250, 2.650] | 0.178 [0.165, 0.191] |
| 40000-40199 | signature_shift at 200 s | tier=hard | 1.58 | 47 | 0.235 [0.175, 0.295] | 0.149 [0.114, 0.185] |
| 40000-40199 | signature_shift at 200 s | tier=decoy | 1.61 | 35 | 0.175 [0.125, 0.230] | 0.108 [0.077, 0.142] |
| 40000-40199 | signature_shift at 200 s | hard/compound | 0.34 | 28 | 0.140 [0.095, 0.190] | 0.412 [0.297, 0.526] |
| 40000-40199 | signature_shift at 200 s | hard/cascade | 0.41 | 19 | 0.095 [0.055, 0.135] | 0.232 [0.147, 0.323] |
| 40000-40199 | signature_shift at 200 s | hard/split_brain | 0.48 | 0 | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] |
| 40000-40199 | signature_shift at 200 s | hard/slow_leak | 0.34 | 0 | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] |
| 40000-40199 | signature_shift at 200 s | decoy/compound | 0.45 | 27 | 0.135 [0.090, 0.185] | 0.300 [0.208, 0.398] |
| 40000-40199 | signature_shift at 200 s | decoy/cascade | 0.31 | 8 | 0.040 [0.015, 0.070] | 0.129 [0.056, 0.210] |
| 40000-40199 | edge_add at 400 s | all | 7.21 | 194 | 0.970 [0.870, 1.070] | 0.134 [0.121, 0.148] |
| 40000-40199 | edge_add at 400 s | tier=plain | 5.90 | 158 | 0.790 [0.700, 0.885] | 0.134 [0.119, 0.149] |
| 40000-40199 | edge_add at 400 s | tier=hard | 0.63 | 22 | 0.110 [0.075, 0.150] | 0.175 [0.120, 0.233] |
| 40000-40199 | edge_add at 400 s | tier=decoy | 0.69 | 14 | 0.070 [0.040, 0.105] | 0.102 [0.061, 0.149] |
| 40000-40199 | edge_add at 400 s | hard/compound | 0.12 | 5 | 0.025 [0.010, 0.045] | 0.200 [0.074, 0.333] |
| 40000-40199 | edge_add at 400 s | hard/cascade | 0.17 | 13 | 0.065 [0.035, 0.100] | 0.371 [0.231, 0.517] |
| 40000-40199 | edge_add at 400 s | hard/split_brain | 0.17 | 4 | 0.020 [0.005, 0.040] | 0.114 [0.030, 0.214] |
| 40000-40199 | edge_add at 400 s | hard/slow_leak | 0.15 | 0 | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] |
| 40000-40199 | edge_add at 400 s | decoy/compound | 0.20 | 5 | 0.025 [0.005, 0.050] | 0.125 [0.030, 0.229] |
| 40000-40199 | edge_add at 400 s | decoy/cascade | 0.13 | 5 | 0.025 [0.010, 0.045] | 0.192 [0.074, 0.333] |

| seeds (hidden) | measure | value | n |
|---|---|---|---|
| 10000-10099 | streams with at least one hard incident affected by signature_shift | 0.1800 | 100 |
| 10000-10099 | streams with at least one hard incident affected by edge_add | 0.0700 | 100 |
| 10000-10099 | share of hard incidents with onset at or after the first change | 0.6214 | 243 |
| 10000-10099 |   90% interval (first change) | [0.5730, 0.6716] |  |
| 10000-10099 | share of hard incidents with onset at or after the last change | 0.2634 | 243 |
| 10000-10099 |   90% interval (last change) | [0.2227, 0.3041] |  |
| 40000-40199 | streams with at least one hard incident affected by signature_shift | 0.1900 | 200 |
| 40000-40199 | streams with at least one hard incident affected by edge_add | 0.1050 | 200 |
| 40000-40199 | share of hard incidents with onset at or after the first change | 0.6172 | 512 |
| 40000-40199 |   90% interval (first change) | [0.5846, 0.6496] |  |
| 40000-40199 | share of hard incidents with onset at or after the last change | 0.2461 | 512 |
| 40000-40199 |   90% interval (last change) | [0.2179, 0.2744] |  |
| 50000-50099 | streams with at least one hard incident affected by signature_shift | 0.1900 | 100 |
| 50000-50099 | streams with at least one hard incident affected by edge_add | 0.1700 | 100 |
| 50000-50099 | share of hard incidents with onset at or after the first change | 0.6990 | 289 |
| 50000-50099 |   90% interval (first change) | [0.6597, 0.7384] |  |
| 50000-50099 | share of hard incidents with onset at or after the last change | 0.4325 | 289 |
| 50000-50099 |   90% interval (last change) | [0.3920, 0.4764] |  |

Per change and family, the incidents it altered, per stream. At 200 s the signature shift alters 2.86
incidents per stream (2.45 plain, 0.235 hard, 0.175 decoy): 14.9% of the hard incidents after it, 41% of
compounds, 23% of cascades, no split brain and no slow leak (their burst holds no characteristic message).
At 400 s the added edge alters 0.97 (0.79 plain, 0.11 hard): 37% of the hard cascades after it. **61.7% of hard incidents
begin after the first change** (tuning 62.1%), 24.6% after the last: most of what a memory is asked about happens
under the changed physics. In 19% of held-out streams (18% tuning) at least one hard incident is altered by the shift, in 10.5% (7%) by the edge.

## 7. Item 6: the perfect-memory ceiling (run, hidden)

### 7.1 The bill

| seeds (run) | streams | total bill (modelled ns) | reasoner (modelled ns) | reasoner share of the bill | calls | on hard incidents | on plain or decoy | on background | mean ns per call | hard incidents | noticed | asked | declared correctly |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 10000-10099 | 100 | 67,332,800,599 | 67,285,000,000 | 0.99929 | 220 | 220 | 0 | 0 | 305,840,909 | 243 | 213 | 211 | 131 |
| 40000-40199 | 200 | 137,974,165,524 | 137,880,000,000 | 0.99932 | 459 | 459 | 0 | 0 | 300,392,157 | 512 | 439 | 435 | 247 |

The reasoner is 99.93% of the arm's bill in both ranges; every call is on a hard incident (the selection
oracle asks about those it is given), none on a plain incident, a decoy or the background. The mean call
costs 0.30 s of modelled reasoner time. Because the reasoner is the bill, a share of the bill and a share of the
reasoner cost are the same number here (both columns are in `w2-ceiling.csv`).

### 7.2 Site-keyed and family-keyed reach

| seeds (run, hidden) | set | hard incidents | per stream | streams with at least one | calls | modelled ns | share of the total bill [90%] |
|---|---|---|---|---|---|---|---|
| 10000-10099 | all hard incidents | 243 | 2.430 | 83 | 220 | 67,285,000,000 | 0.999 [0.999, 0.999] |
| 10000-10099 | R  site-keyed: recurrence of an incident the arm declared correctly earlier | 18 | 0.180 | 15 | 19 | 6,065,000,000 | 0.090 [0.057, 0.125] |
| 10000-10099 | R_chain  site-keyed, any earlier same-site same-family incident declared correctly | 22 | 0.220 | 18 | 22 | 6,975,000,000 | 0.104 [0.070, 0.138] |
| 10000-10099 | R_fresh  R without the recurrences stale by construction | 16 | 0.160 | 14 | 17 | 5,445,000,000 | 0.081 [0.049, 0.114] |
| 10000-10099 | F_only  family-keyed beyond R: same family and mode, earlier correct, not in R | 24 | 0.240 | 16 | 25 | 8,035,000,000 | 0.119 [0.074, 0.166] |
| 10000-10099 | F  family-keyed: same family and mode (any site) earlier declared correctly | 42 | 0.420 | 26 | 44 | 14,100,000,000 | 0.209 [0.156, 0.260] |
| 10000-10099 | F_strict  as F with the same kinds (compound pair, cascade kind) | 30 | 0.300 | 22 | 31 | 9,840,000,000 | 0.146 [0.104, 0.188] |
| 10000-10099 | R_world  every hard recurrence (no condition on the arm) | 44 | 0.440 | 27 | 44 | 13,730,000,000 | 0.204 [0.152, 0.253] |
| 10000-10099 | F_world  every hard incident whose family and mode occurred earlier (no condition on the arm) | 78 | 0.780 | 35 | 75 | 23,380,000,000 | 0.347 [0.282, 0.406] |
| 40000-40199 | all hard incidents | 512 | 2.560 | 175 | 459 | 137,880,000,000 | 0.999 [0.999, 0.999] |
| 40000-40199 | R  site-keyed: recurrence of an incident the arm declared correctly earlier | 41 | 0.205 | 36 | 39 | 11,715,000,000 | 0.085 [0.064, 0.108] |
| 40000-40199 | R_chain  site-keyed, any earlier same-site same-family incident declared correctly | 44 | 0.220 | 37 | 42 | 12,570,000,000 | 0.091 [0.069, 0.115] |
| 40000-40199 | R_fresh  R without the recurrences stale by construction | 31 | 0.155 | 29 | 29 | 8,640,000,000 | 0.063 [0.045, 0.081] |
| 40000-40199 | F_only  family-keyed beyond R: same family and mode, earlier correct, not in R | 29 | 0.145 | 25 | 22 | 6,435,000,000 | 0.047 [0.031, 0.063] |
| 40000-40199 | F  family-keyed: same family and mode (any site) earlier declared correctly | 70 | 0.350 | 48 | 61 | 18,150,000,000 | 0.132 [0.103, 0.161] |
| 40000-40199 | F_strict  as F with the same kinds (compound pair, cascade kind) | 62 | 0.310 | 44 | 53 | 15,800,000,000 | 0.115 [0.088, 0.143] |
| 40000-40199 | R_world  every hard recurrence (no condition on the arm) | 83 | 0.415 | 66 | 75 | 22,645,000,000 | 0.164 [0.136, 0.193] |
| 40000-40199 | F_world  every hard incident whose family and mode occurred earlier (no condition on the arm) | 139 | 0.695 | 90 | 122 | 36,530,000,000 | 0.265 [0.232, 0.298] |

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

| seeds (run, hidden) | set | hard incidents | per stream | streams with at least one | calls | modelled ns | share of the total bill [90%] |
|---|---|---|---|---|---|---|---|
| 10010-10099 | all hard incidents [streams 11 onward] | 216 | 2.400 | 74 | 194 | 59,575,000,000 | 0.999 [0.999, 0.999] |
| 10010-10099 | R  site-keyed, within the stream [streams 11 onward] | 15 | 0.167 | 13 | 16 | 5,155,000,000 | 0.086 [0.051, 0.122] |
| 10010-10099 | F  family-keyed, within the stream [streams 11 onward] | 38 | 0.422 | 24 | 40 | 12,865,000,000 | 0.216 [0.159, 0.271] |
| 10010-10099 | F_cross  family-keyed, memory carried across streams in seed order [streams 11 onward] | 210 | 2.333 | 73 | 188 | 57,640,000,000 | 0.967 [0.934, 0.993] |
| 10010-10099 | F_strict_cross  as F_cross with the same kinds [streams 11 onward] | 199 | 2.211 | 72 | 177 | 54,380,000,000 | 0.912 [0.861, 0.957] |
| 40010-40199 | all hard incidents [streams 11 onward] | 494 | 2.600 | 167 | 444 | 133,995,000,000 | 0.999 [0.999, 0.999] |
| 40010-40199 | R  site-keyed, within the stream [streams 11 onward] | 39 | 0.205 | 35 | 37 | 11,175,000,000 | 0.083 [0.062, 0.106] |
| 40010-40199 | F  family-keyed, within the stream [streams 11 onward] | 68 | 0.358 | 47 | 59 | 17,610,000,000 | 0.131 [0.102, 0.160] |
| 40010-40199 | F_cross  family-keyed, memory carried across streams in seed order [streams 11 onward] | 486 | 2.558 | 166 | 435 | 131,080,000,000 | 0.978 [0.962, 0.991] |
| 40010-40199 | F_strict_cross  as F_cross with the same kinds [streams 11 onward] | 469 | 2.468 | 164 | 419 | 125,965,000,000 | 0.939 [0.915, 0.962] |

Service ids and message ids are regenerated per stream, so a key that carries across streams must have neither;
the family-keyed form is the only one that does. Its reach is almost everything: after the first ten streams
97.8% [96.2, 99.1] of the held-out bill is on a hard incident whose family and mode the arm had already
declared correctly in an earlier stream (93.9% with the strict key). This is an upper bound on **reach, not on correct recall**: whether a key
built from public evidence can tell the family without being fooled is section 9.

### 7.4 Identity with L1's fresh run

| seeds | file | rows (this run) | rows (L1 fresh run) | identical but for run_id and arm_role |
|---|---|---|---|---|
| 40000-40199 | incidents.csv | 5344 | 5344 | True |
| 40000-40199 | results.csv | 200 | 200 | True |
| 40000-40199 | notice_incidents.csv | 5344 | 5344 | True |
| 40000-40199 | notices.csv | 200 | 200 | True |

### 7.5 What the same run says about unasked-correct and stale-wrong declarations with no memory (addition)

| seeds (run, hidden) | tier | which | n | declared correctly, no call | share [90%] | at least one wrong declaration, no call | share [90%] |
|---|---|---|---|---|---|---|---|
| 10000-10099 | plain | all incidents | 2110 | 1838 | 0.871 [0.857, 0.886] | 354 | 0.168 [0.150, 0.186] |
| 10000-10099 | plain | recurrences | 437 | 376 | 0.860 [0.824, 0.895] | 89 | 0.204 [0.165, 0.244] |
| 10000-10099 | hard | all incidents | 243 | 0 | 0.000 [0.000, 0.000] | 2 | 0.008 [0.000, 0.018] |
| 10000-10099 | hard | recurrences | 44 | 0 | 0.000 [0.000, 0.000] | 0 | 0.000 [0.000, 0.000] |
| 10000-10099 | decoy | all incidents | 290 | 0 | 0.000 [0.000, 0.000] | 215 | 0.741 [0.692, 0.791] |
| 10000-10099 | decoy | recurrences | 69 | 0 | 0.000 [0.000, 0.000] | 43 | 0.623 [0.500, 0.735] |
| 40000-40199 | plain | all incidents | 4304 | 3695 | 0.859 [0.847, 0.870] | 732 | 0.170 [0.159, 0.181] |
| 40000-40199 | plain | recurrences | 883 | 755 | 0.855 [0.831, 0.878] | 143 | 0.162 [0.140, 0.184] |
| 40000-40199 | hard | all incidents | 512 | 0 | 0.000 [0.000, 0.000] | 5 | 0.010 [0.004, 0.018] |
| 40000-40199 | hard | recurrences | 83 | 0 | 0.000 [0.000, 0.000] | 0 | 0.000 [0.000, 0.000] |
| 40000-40199 | decoy | all incidents | 528 | 0 | 0.000 [0.000, 0.000] | 388 | 0.735 [0.699, 0.771] |
| 40000-40199 | decoy | recurrences | 129 | 0 | 0.000 [0.000, 0.000] | 94 | 0.729 [0.652, 0.804] |

Plain incidents are declared correctly with no call by the cheap rung alone: 85.5% of the plain recurrences held-out [83.1, 87.8].
A clause on "unasked-correct share of recurrences" over all recurrences is therefore dominated (80%) by plain ones that
need no memory. The hard incidents' unasked-correct is zero by construction here (the oracle arm asks about every
hard incident it notices; an arm that does not ask would show its cheap rung's confident error instead), so
the only hard unasked-correct decisions an A1b arm can make are memory's. The "wrong, no call" column counts incidents
with at least one wrong declaration and no call: 17% of plain incidents (the cheap rung's own errors), 73% of decoys
(the arm declares every decoy an incident): these are errors of the **noticer and rung**, present with no memory, and an
E1 `stale_wrong` that counts them all is not a measure of what memory does.

### 7.6 The error a memory fed by the reasoner inherits (addition)

| seeds (run, hidden) | source of the recalled answer | hard incidents with an answered source before their notice | source answer right | wrong | mixed | wrong share [90%] |
|---|---|---|---|---|---|---|
| 10000-10099 | site-keyed (the template) | 39 | 18 | 21 | 0 | 0.538 [0.385, 0.680] |
| 10000-10099 | family-keyed (most recent same family and mode) | 64 | 36 | 28 | 0 | 0.438 [0.343, 0.530] |
| 40000-40199 | site-keyed (the template) | 68 | 40 | 27 | 1 | 0.397 [0.281, 0.508] |
| 40000-40199 | family-keyed (most recent same family and mode) | 115 | 61 | 51 | 3 | 0.444 [0.354, 0.534] |

A memory that stores the reasoner's answer repeats it. In this arm the hard declarations are right 48% of the time (b = 5, rho = 0.7, the rung's context),
so of the hard incidents whose source answer was delivered before they were noticed, 40% [28, 51] of the site-keyed sources (68 held-out) and
44% [35, 53] of the family-keyed sources (115) held a **wrong** answer, 54% [38, 68] of the site-keyed sources in the tuning range. A recall of those is wrong
for a reason that is neither staleness nor a key collision: the reasoner was wrong the first time. (For the family-keyed form I count a source answer
as right when it was right for its own incident; one right in kind and wrong in site would still serve a key that substitutes the site, so the family-keyed
wrong share is an upper estimate.) This error is the same for every key form and every arm fed by the same reasoner, which is why section 10.2 keeps it apart.

## 8. Item 7: world B (hidden; items 1, 2 and 5)

Recurrence 0.6, regime changes at 150 s (signature shift) and 300 s (added edge), seeds 50000-50099.

| seeds (hidden) | group | incidents | per stream | recurrences | recurrences per stream [90%] | share of incidents [90%] |
|---|---|---|---|---|---|---|
| 50000-50099 | all | 2679 | 26.79 | 1262 | 12.620 [12.060, 13.180] | 0.471 [0.456, 0.486] |
| 50000-50099 | tier=plain | 2080 | 20.80 | 954 | 9.540 [9.010, 10.070] | 0.459 [0.443, 0.474] |
| 50000-50099 | tier=hard | 289 | 2.89 | 134 | 1.340 [1.070, 1.630] | 0.464 [0.422, 0.500] |
| 50000-50099 | tier=decoy | 310 | 3.10 | 174 | 1.740 [1.410, 2.090] | 0.561 [0.511, 0.607] |
| 50000-50099 | hard/compound | 68 | 0.68 | 27 | 0.270 [0.150, 0.400] | 0.397 [0.290, 0.482] |
| 50000-50099 | hard/cascade | 56 | 0.56 | 26 | 0.260 [0.150, 0.380] | 0.464 [0.364, 0.542] |
| 50000-50099 | hard/split_brain | 101 | 1.01 | 54 | 0.540 [0.350, 0.740] | 0.535 [0.460, 0.595] |
| 50000-50099 | hard/slow_leak | 64 | 0.64 | 27 | 0.270 [0.160, 0.390] | 0.422 [0.321, 0.500] |
| 50000-50099 | hard/compound/mimic | 38 | 0.38 | 14 | 0.140 [0.060, 0.230] | 0.368 [0.219, 0.482] |
| 50000-50099 | hard/compound/contradict | 30 | 0.30 | 13 | 0.130 [0.050, 0.220] | 0.433 [0.258, 0.548] |
| 50000-50099 | hard/cascade/mimic | 25 | 0.25 | 9 | 0.090 [0.030, 0.160] | 0.360 [0.200, 0.472] |
| 50000-50099 | hard/cascade/contradict | 31 | 0.31 | 17 | 0.170 [0.080, 0.270] | 0.548 [0.414, 0.636] |
| 50000-50099 | hard/split_brain/mimic | 40 | 0.40 | 17 | 0.170 [0.090, 0.260] | 0.425 [0.308, 0.514] |
| 50000-50099 | hard/split_brain/contradict | 61 | 0.61 | 37 | 0.370 [0.200, 0.560] | 0.607 [0.510, 0.673] |

| seeds (hidden) | change | group | incidents after it, per stream | affected | affected per stream [90%] | share of those after it [90%] |
|---|---|---|---|---|---|---|
| 50000-50099 | signature_shift at 150 s | all | 19.22 | 299 | 2.990 [2.620, 3.370] | 0.156 [0.136, 0.175] |
| 50000-50099 | signature_shift at 150 s | tier=plain | 15.19 | 244 | 2.440 [2.110, 2.780] | 0.161 [0.139, 0.183] |
| 50000-50099 | signature_shift at 150 s | tier=hard | 2.02 | 27 | 0.270 [0.170, 0.380] | 0.134 [0.087, 0.184] |
| 50000-50099 | signature_shift at 150 s | tier=decoy | 2.01 | 28 | 0.280 [0.160, 0.420] | 0.139 [0.080, 0.208] |
| 50000-50099 | signature_shift at 150 s | hard/compound | 0.52 | 19 | 0.190 [0.110, 0.280] | 0.365 [0.226, 0.532] |
| 50000-50099 | signature_shift at 150 s | hard/cascade | 0.34 | 8 | 0.080 [0.020, 0.150] | 0.235 [0.074, 0.415] |
| 50000-50099 | signature_shift at 150 s | hard/split_brain | 0.67 | 0 | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] |
| 50000-50099 | signature_shift at 150 s | hard/slow_leak | 0.49 | 0 | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] |
| 50000-50099 | signature_shift at 150 s | decoy/compound | 0.50 | 11 | 0.110 [0.030, 0.220] | 0.220 [0.071, 0.397] |
| 50000-50099 | signature_shift at 150 s | decoy/cascade | 0.50 | 17 | 0.170 [0.080, 0.270] | 0.340 [0.184, 0.500] |
| 50000-50099 | edge_add at 300 s | all | 11.59 | 187 | 1.870 [1.630, 2.130] | 0.161 [0.141, 0.183] |
| 50000-50099 | edge_add at 300 s | tier=plain | 9.12 | 154 | 1.540 [1.330, 1.760] | 0.169 [0.147, 0.192] |
| 50000-50099 | edge_add at 300 s | tier=hard | 1.25 | 17 | 0.170 [0.110, 0.230] | 0.136 [0.090, 0.184] |
| 50000-50099 | edge_add at 300 s | tier=decoy | 1.22 | 16 | 0.160 [0.080, 0.260] | 0.131 [0.066, 0.202] |
| 50000-50099 | edge_add at 300 s | hard/compound | 0.36 | 6 | 0.060 [0.020, 0.100] | 0.167 [0.069, 0.282] |
| 50000-50099 | edge_add at 300 s | hard/cascade | 0.19 | 5 | 0.050 [0.020, 0.090] | 0.263 [0.100, 0.462] |
| 50000-50099 | edge_add at 300 s | hard/split_brain | 0.39 | 6 | 0.060 [0.020, 0.100] | 0.154 [0.065, 0.259] |
| 50000-50099 | edge_add at 300 s | hard/slow_leak | 0.31 | 0 | 0.000 [0.000, 0.000] | 0.000 [0.000, 0.000] |
| 50000-50099 | edge_add at 300 s | decoy/compound | 0.35 | 4 | 0.040 [0.010, 0.080] | 0.114 [0.022, 0.231] |
| 50000-50099 | edge_add at 300 s | decoy/cascade | 0.23 | 2 | 0.020 [0.000, 0.050] | 0.087 [0.000, 0.208] |

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

| seeds (hidden) | memory | incident | incidents | site in memory | share | recalled right | recalled wrong | wrong share of recalls |
|---|---|---|---|---|---|---|---|---|
| 10000-10099 | site-only, most recent hard incident, same stream | all | 2643 | 193 | 0.073 | 45 | 148 | 0.767 |
| 10000-10099 | site-only, most recent hard incident, same stream | plain | 2110 | 112 | 0.053 | 0 | 112 | 1.000 |
| 10000-10099 | site-only, most recent hard incident, same stream | hard | 243 | 61 | 0.251 | 45 | 16 | 0.262 |
| 10000-10099 | site-only, most recent hard incident, same stream | decoy | 290 | 20 | 0.069 | 0 | 20 | 1.000 |
| 10000-10099 | site-only, most recent hard incident in earlier streams (after the first 10) | all | 2360 | 2330 | 0.987 | 31 | 2299 | 0.987 |
| 10000-10099 | site-only, most recent hard incident in earlier streams (after the first 10) | plain | 1884 | 1862 | 0.988 | 0 | 1862 | 1.000 |
| 10000-10099 | site-only, most recent hard incident in earlier streams (after the first 10) | hard | 216 | 214 | 0.991 | 31 | 183 | 0.855 |
| 10000-10099 | site-only, most recent hard incident in earlier streams (after the first 10) | decoy | 260 | 254 | 0.977 | 0 | 254 | 1.000 |
| 40000-40199 | site-only, most recent hard incident, same stream | all | 5344 | 466 | 0.087 | 86 | 380 | 0.816 |
| 40000-40199 | site-only, most recent hard incident, same stream | plain | 4304 | 321 | 0.075 | 0 | 321 | 1.000 |
| 40000-40199 | site-only, most recent hard incident, same stream | hard | 512 | 114 | 0.223 | 86 | 28 | 0.246 |
| 40000-40199 | site-only, most recent hard incident, same stream | decoy | 528 | 31 | 0.059 | 0 | 31 | 1.000 |
| 40000-40199 | site-only, most recent hard incident in earlier streams (after the first 10) | all | 5094 | 5020 | 0.986 | 80 | 4940 | 0.984 |
| 40000-40199 | site-only, most recent hard incident in earlier streams (after the first 10) | plain | 4105 | 4046 | 0.986 | 0 | 4046 | 1.000 |
| 40000-40199 | site-only, most recent hard incident in earlier streams (after the first 10) | hard | 494 | 486 | 0.984 | 80 | 406 | 0.835 |
| 40000-40199 | site-only, most recent hard incident in earlier streams (after the first 10) | decoy | 495 | 488 | 0.986 | 0 | 488 | 1.000 |

Within a stream a site-only memory recalls at 8.7% of incidents (held-out) and is wrong at 82% of those: 1.9 wrong recalls per
stream; for a plain incident or a decoy at a site where a hard incident happened, always. Carried across streams
(site ids are numbered per stream) 98.6% of incidents have a site in memory and 98.4% of those recalls are wrong (26 per stream):
**a site-keyed engram carried across stream boundaries is a stale-error generator by construction**, and
any A1b arm that persists site-keyed state across segments must reset it at the boundary or key without the site id.

**Phase 1** (the key is the site and the class of the first seconds, what the public rules see before phase 2),
most recent hard incident or decoy of the same key:

| seeds (hidden) | memory | incident | incidents | recalled | share | right | wrong | wrong share of recalls | wrong per stream |
|---|---|---|---|---|---|---|---|---|---|
| 10000-10099 | phase-1 site+class | all | 2643 | 142 | 0.054 | 120 | 22 | 0.155 | 0.220 |
| 10000-10099 | phase-1 site+class | plain | 2110 | 13 | 0.006 | 0 | 13 | 1.000 | 0.130 |
| 10000-10099 | phase-1 site+class | hard | 243 | 54 | 0.222 | 47 | 7 | 0.130 | 0.070 |
| 10000-10099 | phase-1 site+class | decoy | 290 | 75 | 0.259 | 73 | 2 | 0.027 | 0.020 |
| 10000-10099 | phase-1 class only | all | 2643 | 386 | 0.146 | 151 | 235 | 0.609 | 2.350 |
| 10000-10099 | phase-1 class only | plain | 2110 | 187 | 0.089 | 0 | 187 | 1.000 | 1.870 |
| 10000-10099 | phase-1 class only | hard | 243 | 89 | 0.366 | 63 | 26 | 0.292 | 0.260 |
| 10000-10099 | phase-1 class only | decoy | 290 | 110 | 0.379 | 88 | 22 | 0.200 | 0.220 |
| 40000-40199 | phase-1 site+class | all | 5344 | 259 | 0.049 | 221 | 38 | 0.147 | 0.190 |
| 40000-40199 | phase-1 site+class | plain | 4304 | 21 | 0.005 | 0 | 21 | 1.000 | 0.105 |
| 40000-40199 | phase-1 site+class | hard | 512 | 97 | 0.190 | 87 | 10 | 0.103 | 0.050 |
| 40000-40199 | phase-1 site+class | decoy | 528 | 141 | 0.267 | 134 | 7 | 0.050 | 0.035 |
| 40000-40199 | phase-1 class only | all | 5344 | 670 | 0.125 | 285 | 385 | 0.575 | 1.925 |
| 40000-40199 | phase-1 class only | plain | 4304 | 284 | 0.066 | 0 | 284 | 1.000 | 1.420 |
| 40000-40199 | phase-1 class only | hard | 512 | 162 | 0.316 | 111 | 51 | 0.315 | 0.255 |
| 40000-40199 | phase-1 class only | decoy | 528 | 224 | 0.424 | 174 | 50 | 0.223 | 0.250 |

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

Gates, run under `scripts/cgroup-run.sh --name world-laws-build --cpus 0-2 --memory 3G` (never `--allow-unisolated`),
`CARGO_BUILD_JOBS=3 CARGO_PROFILE_DEV_DEBUG=0`, on the tree as committed (the Rust sources are unchanged since commit 85a9719; every later commit touches scripts, CSVs and this report):
`cargo fmt --all -- --check` exit 0; `cargo clippy --workspace --all-targets -- -D warnings` exit 0;
`cargo test --workspace` exit 0, 954 passed, 0 failed, 13 ignored (262 s wall, peak 1.78 GB, no OOM kill; the new
`tests::rebuild` tests are among them); `scripts/check-no-oracle.sh` prints `check-no-oracle: ok`. Disk after the
test build: 6.4 GB free (12 GB before the first build).
