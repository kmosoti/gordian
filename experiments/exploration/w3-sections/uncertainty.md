**Least sure of.**

1. **The floor is the world's, computed from each incident's own evidence; it is not a prediction of what an arm reaches.** E1's family-keyed carried arm recalled 372 plain incidents; the floor has 125, and 328 of E1's
   372 are on plain incidents whose own evidence the public checker finds consistent (section 3.4). The gap is evidence from elsewhere in the anomaly. I measured the count, not the cause: I did not inspect the arm's
   anomaly evidence, so "contamination by other incidents' and background symptoms" is the reading, untested. The collision clause as A1b states it will be read on arms, so it is the arm's number that
   matters; the floor says which part of it the world already accounts for.
2. **The key families are mine.** `K1` to `K4` and E1's keys are three or four of many invariant keys; a richer key (counts, orders, inter-arrival gaps) could separate decoys earlier or recall less. I claim
   a floor for these keys, not over all invariant keys. The one structural statement I make beyond them (no abnormal evidence in phase 2 of a contradict presentation or of the leak) is read from
   `HIDDEN-DESIGN.md` section 4.2 and is consistent with the breakdown of the wrong recalls (section 3.3), not independently tested.
3. **The gate's time.** The gate is the delivery instant of the observation that completes a contradictory prefix; E1's rung checks at most once per 0.5 s after evidence arrives and reads the key one second after
   its first contradictory check, so an incident near a band boundary of the gate delay (1, 3, 6, 10 s) may fall in another band in the arm. The key agreement with E1's own same-stream recalls (11 of 11, 22 of 23,
   8 of 9, 4 of 5, small numbers) is the only check.
4. **The mix choice.** Decoys held at 100 per mille turns the hard-to-decoy ratio from 1:1 to 3:1 and lowers every collision *share* that involves decoys. With decoys scaled with plain the shares would sit between
   the two worlds'. Not run. The family mix of the hard tier also moved (section 2.2), by a mechanism I read from the code and did not count.
5. **Right sources only.** The floor takes every stored answer to be the truth; the reasoner is right about half the time on hard incidents (W2 section 7.6: 36-54% of sources wrong), so an arm's wrong recalls include
   inherited errors that are in neither this floor nor the collision clause as split by W2.
6. **A2's verdict rests on 20 streams, 251 predictions and 29 true partner alarms.** Zero covered and zero event hits are robust to my definitions (a looser reading, any prediction on the pair whose window holds
   the partner alarm, is also 0 of 29), but "precision at chance" has an interval of [3.6, 14.3]% and a handful of pairs behind it. The smoke is world A at seeds 10000-10019, not tuned.
7. **The permutation chance** conditions on a candidate being unconnected to the predicting service and quiet for 2 s, and counts any abnormal observation at the candidate as a follow; A2's own `permutation_chance` counts
   only counted first alarms, so the two follow-chance numbers differ (A2's report: 0.052, 0.222, 0.533 by band; here 0.100, 0.255, 0.618).
8. **`R` (reachable) is conditional on the arm** (W2's least-sure item 1): the arm declares about half the hard incidents correctly in both worlds.
9. **Seeds and streams.** One tuning-like and one held-out range per world; no run on world C with another selection delay or reasoner; the order of streams in the carried tables is seed order.

**What would change the conclusions.** A hard-to-decoy ratio of 1:1 in world C (the other reading of the mix): the family-keyed shares would rise toward world A's. An invariant key that holds a feature I do
not (the temporal pattern of heartbeat flaps, say) and separates a decoy from its hard family before it resolves: the K2 floor would fall. Evidence that E1's excess plain recalls come from something other than
attached symptoms of other incidents (for example a defect in the arm's attach rule): the floor-to-arm gap would be an artefact of the instrument and not of the world. A trace of A2 on a world with more
pair repeats: the coverage would be testable.

**What the chief should examine most carefully.**

- `floor.rs`: the attach rule I assumed (the site and its public dependents; an unconnected service starts its own anomaly), the gate, and that `key_of`'s features are E1's. The check against E1's recalls is
  `w3-floor-e1-join.csv`.
- `simulate` in `w3_floor.py`: an incident's own answer is excluded from its own recall (for cutoffs after 19 s every key would otherwise recall itself: my first run had this defect, 243 of 243 hard incidents
  "recalled right" at 20 s, found by reading the table, fixed before any number was used; the unit tests pin it).
- The join in `w3_score.py`: the predicting alarm is checked to be the stream's alarm at that instant and service (an assertion that stops the script), owner incidents come from the `alarms.csv` the example wrote,
  and the verdicts "true hidden edge" and "event hit" are the definitions of section 4. The tests (8, derived by hand) and the 14 caught mutants are the evidence it works as specified.
- The choice of the mix (section 1.3) and of the oracle arm (the arm of W2's item 6).
- Whether the brief's "about 120 reachable events" is `R` (138) or `R_world` (259) or something else; I used `R`.

**The improved question.** A1b's within-stream question is no longer short of power, and its collision clause has a floor: a site-keyed key from the rule-breaking evidence is clean (0.09), a family-keyed one is not (0.29) unless it
waits for the decoy to resolve (0.11 at 16 s) and nothing invariant is clean across streams (0.28 to 0.87). The question that follows is whether the arm that would carry the family law across streams can learn, from
its own history, what the world's decoys and altered plain incidents look like (a teacher that includes non-hard answers), since a table of hard answers alone cannot (aggregation did not move the floor). A2's question
is whether any evidence of one pair per stream can teach an edge (0.07 events per stream with two earlier sightings, in either world) before it asks whether the edge is used.

**Smallest high-information next steps.** (i) E1's record rung on world C's held-out range under the same arm and keys, to read the arm's collisions against this floor where hard recurrences are three times as many.
(ii) The same floor with a teacher that includes decoys and plain incidents as the memory's own outcomes (what a memory that records every notice and its eventual diagnosis would hold): the vote result suggests the
floor then moves; this unit did not run it. (iii) A1b's criterion fixed with the collision clause per hard incident and the K2/K4 floors above, before any A1b run. (iv) For A2, a world where the same pair recurs
(a persistent hidden edge), since this one cannot teach one.
