### 3.1 What is computed

W2's section 9 asked what a key read in the first seconds would recall wrongly if it fired on every match, with each source taken to be a *right* earlier diagnosis. This is
the same question for keys that wait for the evidence of phase 2. For every incident (seeds 10000-10099, 40000-40199, 60000-60099, 70000-70199) the `laws` example
(`--floor`, `crates/gordian-stream/examples/laws/floor.rs`) takes **the incident's own observations** (public bytes; which ones belong to the incident is the hidden label, so
no other incident's and no background symptom is in them: the best case for any key, a floor and not a prediction) and writes the invariant features at cutoffs 1 to
32 s after its first abnormal observation. "Abnormal" is the rung's own predicate (`rung::is_abnormal`). Two evidence sets:

- **attached**: the abnormal observations at the site and at its dependents in the time-zero public graph. This is what the rung's anomaly holds for a site
  (`noticer::attach_target`; read, section 1.2): a service the public graph does not connect to the site starts an anomaly of its own, so a cascade's partner and a split brain's
  peer are *not* in the evidence the rung's consistency checker sees. E1's gate and key are built from this set.
- **all**: every abnormal observation of the incident, partner and peer included.

The public rules' verdict is `gordian_world::physics::consistent_hypotheses` on the evidence; "rule-breaking" is an empty set (the first world's word for contradictory). The **gate**
is the first prefix whose verdict is empty. The keys:

| Key | Features (all invariant: no service id, no message id) |
|---|---|
| `K1` | the abnormal tags at the site (counters at or above `HIGH`, catalogue messages other than `CheckHealth`, a changed configuration snapshot) and the anchor's tags: E1's `kinds` without the site id |
| `K2` | `K1`, the tags at services outside the site and its dependents (a partner's, a peer's, the added edge's new dependent), how many such services (up to 3), and the public checker's verdict on all the evidence |
| `K3` | `K2`, the highest band of each abnormal counter at the site (E1's `bands`) and A1c's partner relation (the unconnected service whose first alarm is nearest the site's, within 10 s: before or after, in the bands 0.4 s, 2 s, 10 s, and its tags) |
| `K4` | `K3` and whether the site's last five counter readings are all benign: the public decoy rule (labelled an extension: it is the *decoy's* evidence, not rule-breaking evidence) |
| E1 `kinds`, `bands`, `timing` | `key_of` of `noticer_record.rs`: the site's tags and the anchor's; the bands; the band of the gate delay (under 1, 3, 6, 10 s, or more); read at the **snapshot**, one second after the gate opens on the attached evidence; only incidents with a gate have a key |

Site-keyed forms add the site's id and so cannot cross a stream boundary; family-keyed forms are the invariant ones. The memory is a table from a key to the truth of the most recent source bound
there. A source binds when its answer would arrive (19 s after its first alarm: 16 s of selection delay, about 2 s of latency, about 1 s of notice) and is not available to its own recall; a
lookup happens at the cutoff. "Reset" empties the table at each stream; "carried" keeps it across the streams of the range in seed order, starting empty at the first. A recall is *wrong* when the
recalled truth is not the target's: a plain incident, a decoy, or another hard family. The collision share is wrong recalls over recalls. The primary teacher is hard incidents only (what the
selection oracle asks, so what E1's table holds); the W2 teacher (hard incidents and decoys, an earlier reading) is in `w3-floor-ladder.csv`.

**Which cutoffs are realisable.** The oracle arm calls the reasoner 16 s after the notice; a recall that saves the call must come before it. Cutoffs of 16 s and later measure what a key could
know *after* the arm has already asked, so they are the floor of a key's information and not a saving. The cost of waiting (anchor-relative; the slow leak's anchor is its first threshold
crossing, about 10 s after onset, so its deadline is 10 s nearer in anchor time):

{{waiting}}

### 3.2 When the public checker finds the evidence contradictory (the gate)

{{gate}}

Reading. On a stream's own attached evidence the gate opens for 98% of the compounds (at once for a contradict presentation, at a median 10.9 s for a mimic, when the second kind's message arrives), for 14% to 37%
of cascades and split brains, **never for the slow leak**, and for 7.5% of plain incidents (66% of those the signature shift altered, 10% of those the added edge altered). A cascade's partner and a split brain's
peer are outside the attached evidence unless the peer happens to be a dependent of the site in the public graph: the cascade gates 14% and 15% of the time (mimic, contradict; A) on the attached evidence and 100% on
all of its own evidence (median 10.9 s and 0.06 s). So **E1's gate cannot see a leak at all and sees a cascade only when other evidence reaches its anomaly**; E1's `captured` counts were 5 of 132 cascades and 5 of 114 leaks, which can then
come only from evidence that is not the incident's own. Decoys open the gate too: 62% of compound decoys, 7% to 21% of the others (the table does not split the decoys by mode).

### 3.3 The ladder (hidden; hard teacher, sources after their answer)

**Family-keyed, stream reset** (the memory that could be A1b's within-stream form without site ids). Seeds 40000-40199 (A) and 70000-70199 (C); tuning-like ranges in `w3-floor-ladder.csv`:

{{ladder_family_reset_hard}}

**Site-keyed, stream reset** (the form that cannot cross streams):

{{ladder_site_reset}}

**Family-keyed, carried across streams** (the form that could learn a law; the table starts empty at the first stream of the range and fills as it goes):

{{ladder_family_carried}}

Who recalls wrongly (hidden):

{{who}}

Reading, the in-stream rows.

- **The first seconds' key is W2's key.** At 6 s the family-keyed K2 key is wrong in 0.606 [0.547, 0.662] of its recalls (A held-out; 0.83 wrong recalls per stream) and 0.455 [0.418, 0.491] in C
  (2.24 per stream). W2's class-only table, on code-derived classes and its own teacher, said 0.575; the simulator reproduces W2's table cell for cell on W2's classes (section 3.5), and the data-derived keys give
  a number of the same size.
- **The phase-2 evidence removes the plain collisions and leaves the decoys.** Between 6 s and 16 s the family-keyed K2 key falls to 0.289 [0.224, 0.354] (A; 0.23 per stream) and 0.166 [0.140, 0.193]
  (C; 0.60 per stream). Wrong recalls on plain incidents fall from 116 to 9 (A), from 308 to 14 (C): once the key holds the verdict of the rules and the evidence outside the site, a plain incident rarely
  looks like a hard one. What remains is **decoys**: slow-leak decoys (28 of 46 wrong recalls in A, 59 of 120 in C) and contradict-mode decoys (9 in A, 45 in C). Per `HIDDEN-DESIGN.md` section 4.2 phase 2 holds no *abnormal*
  evidence for the leak or for a contradict presentation (already contradictory in phase 1): only hidden-vocabulary messages, which the public rules ignore. The breakdown above is consistent with that (the wrong recalls at 16 s are the
  leak and contradict decoys, not mimic ones), so the rules' own evidence cannot separate these decoys from their hard family until they resolve.
- **The decoy's resolution is the rest, and it is late.** K3's extra features (bands, partner) buy little: 0.262 [0.169, 0.359] at 16 s (A). K4's benign streak is the decoy's own evidence; the share of decoys that
  have shown it is 0.11 at 10 s, **0.39 at 16 s**, 0.78 at 24 s and 1.00 at 32 s after the first alarm (slow-leak decoys 0.44, 0.75, 1.00, 1.00; hard incidents 0.00 to 0.03 throughout):

{{streak}}

  With it K4 is 0.114 [0.058, 0.175] at 16 s (A) and 0.088 [0.064, 0.114] (C); at 20 s 0.043 [0.011, 0.086] (A) and 0.043 [0.025, 0.065] (C) (`w3-floor-ladder.csv`); at 32 s 0.061 [0.015, 0.119] (A) and
  0.018 [0.003, 0.038] (C). At 16 s the arm has already asked.
- **What waiting costs in reach.** K2 at 16 s recalls right 113 of the 512 hard incidents (A; 22%) and 72 of the 83 hard recurrences; K4 at 16 s 62 and 40; at 32 s 61 and 38. In C 603 of 1516 (235 of 259
  recurrences) at K2/16 s and 352 (154) at K4/16 s. The hard incidents never recalled are those with no earlier source at the same key.
- **The site-keyed form** is cleaner: 0.087 [0.040, 0.143] at K2/16 s (A; 73 hard recalled right, 71 of 83 recurrences), 0.054 [0.033, 0.077] (C), 0 wrong in 41 recalls (A) at K3/32 s; its per-stream
  count is 0.04 (A) and 0.08 (C). The first-seconds site key is 0.248 [0.174, 0.326] (A, K1 at 6 s); W2's phase-1 site-and-class floor of 0.147 was for classes, not these features.
- **Across streams nothing meets 0.20.** The carried family-keyed floor is 0.87 at 6 s, 0.63 [0.60, 0.66] at K2/16 s and 0.43 [0.39, 0.46] at K4/32 s (A); 0.66, 0.38 and 0.28 (C). In A the 299 wrong recalls at K4/32 s are
  172 plain incidents (138 of them altered by no regime change) and 127 decoys (121 slow-leak decoys, 6 contradict cascades; `w3-floor-who.csv`, carried rows). Two inspections of `floor-rows.csv` that are not tables:
  203 plain resource-exhaustion incidents share one K4 key with **two hard compounds** (a signature shift made their second kind's message the first kind's, so they present as plain resource exhaustion), and 116 of the 122 slow-leak
  decoys carry a key that **one** hard slow leak in the range also carries (a leak whose last readings were benign). A table of hard answers cannot learn that plain incidents and decoys share these keys: aggregation changes nothing (below),
  and a key that one or two sources hold is a key a vote cannot repair.

### 3.4 The key forms E1 built (hidden; against E1's measured collisions)

E1's keys at their own snapshot, hard teacher, sources after their answer (tuning-like ranges follow the held-out ones; `w3-floor-e1.csv` has both availability readings: they differ by at most 2 recalls in
any cell):

{{e1forms}}

Against E1's held-out recalls (seeds 40000-40199, `recalls.csv` of each arm, read only), incident by incident, same key form, level and reset:

{{e1join}}

Reading.

- **E1's site-keyed collision of 0.11 sits on the floor.** E1 measured 1 wrong in 9 recalls with a right source (0.111 [0, 0.333]); the hidden-side floor for the same key, from each incident's own
  evidence, is 3 wrong in 26 recalls (0.115 [0.000, 0.233]; 0.01 per stream) on the same seeds, 0.125 [0.073, 0.182] in world C. A site-keyed key with `kinds` and the stream reset
  collides at about one recall in eight because of what the world presents, not because of the arm: the wrong recalls are 2 plain incidents and 1 decoy of 26 (A) and 2 plain incidents and 11 decoys of 104 (C).
- **E1's family-keyed, carried collision of 0.81 is 0.64 on the floor and the rest is the arm's evidence.** The hidden-side floor for `timing`, family-keyed, carried, from own evidence: 0.637 [0.582, 0.688] (A held-out),
  1.09 wrong per stream; world C 0.344 [0.306, 0.381]. E1's 625 recalls with an incident (661 with the background's) split as 175 on incidents that have a gate on their own evidence and **450 that do not**
  (328 plain, 63 hard, 59 decoy): of E1's 372 plain recalls, 328 were on plain incidents whose own evidence the public checker finds consistent. Their anomalies must have become contradictory through evidence that is not
  their own (other symptoms attached to them); this is the contamination the floor leaves out. I did not look at the arm's anomaly evidence, so *which* symptoms is untested; the count (328 of 372) is measured. The floor's own plain recalls are 125 against E1's 372, its hard
  126 against 128, its decoy 93 against 125: **what the arm adds over the floor is plain recalls**.
- **The instrument agrees with E1 where E1 recalled in the same stream:** the hidden keys of the source and the target are equal for 11 of 11 (site, kinds, reset), 6 of 6, 22 of 23 and 8 of 9 (family forms)
  recalls with both keys, so the features are E1's. Of the 15 recalls of the site-keyed `kinds` reset arm, 11 are on incidents with a hidden snapshot and all 11 are in the floor's set; the floor has 15 more.
- **Reach.** The hard recurrences recalled right under the floor are 21 of 83 (`kinds`, site, reset; 0.25) and 12 of 83 (`timing`), against E1's measured 7 (0.084): E1 reached a third of what the key allows in the
  best case (the others are the incidents whose gate or whose arm-side evidence differs; not diagnosed here). In C: 80 of 259 and 48 of 259.

Aggregation (the memory keeps every answer bound at a key and recalls the leader only when its share is above 0.5 or 0.8) does not lower the floor, because the collision is a non-hard incident sharing a key with
hard sources and a table of hard answers cannot know it:

{{vote}}

### 3.5 Checks on the instrument

W2's published phase-1 table against this script's simulator on W2's code-derived classes (every cell equal in every range, world C included):

{{check_w2}}

W2's code-derived classes against the data-derived keys at 6 s (`K2`). Where no regime change touched the incident, every data class holds incidents of a single W2 class (100% in all four ranges) and 90% to 95% of the
incidents of a W2 class sit in one data class; the data classes are finer (19 against 14). The code reading of W2 is therefore confirmed on the incidents the regime changes did not alter (W2's own least-sure item 5). Altered
incidents add many keys (a shifted message makes a new tag set), which is the "all incidents" row (81 to 106 data classes):

{{check_partition}}

### 3.6 What the floor says

- **A key that waits for the rule-breaking evidence is clean against plain incidents and not against decoys.** Within a stream, the phase-2 evidence takes the family-keyed collision from 0.61 (6 s) to 0.29 (16 s) in
  world A (0.46 to 0.17 in C); what is left is decoys that no abnormal evidence separates from their hard family before they resolve. That the bound of 0.20 is met only by a key that uses the decoy's resolution (K4: 0.114 at 16 s,
  0.043 at 20 s; both after the arm has asked) or by a site-keyed key (0.087) is the floor's statement; E1's measured 0.11 is the site-keyed floor.
- **Across streams the floor is above the bound under any key tried and under aggregation:** 0.28 to 0.87 (carried, 6 s to 32 s, K1 to K4, A and C; 0.34 to 0.64 for E1's keys), against 0.20.
- **The mix matters to the arithmetic, not to the finding.** Collision *shares* fall when the hard share rises (A 0.289 to C 0.166 at K2/16 s, family, reset) because hard targets are a larger share of recalls; the per-stream count
  rises (0.23 to 0.60). A bound written per stream is therefore a statement about the mix.
