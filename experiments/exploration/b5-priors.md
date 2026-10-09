# B5 (resumed): priors, written before any feature-log or tuning run

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation). Lab 2, unit B5
resumed from main at d3dcd77, branch `budgeted-selector-2`. This file is committed before the first
feature-log run and before the tuning rule, so that the report can say which of these the data
contradicted. It is a record of what the PI expected, not a claim.

## What the PI had seen when writing it

- The earlier units' printed results: B4's held-out table (the comparator under always-escalate makes
  27.0 calls per stream; the oracle 2.6; hard quality 0.56; the public threshold rule 26.4 calls), R5's
  statement that the checker contradicts about 97% of plain anomalies, B4's AUC of 0.63 for leak against
  decoy on the follow-up statistic, A1d's 584 calls against 61 in 20 streams.
- None of this unit's feature data. Two rows of the stopped run's scratch feature log (not committed,
  not used) were displayed by a `head` command while locating files: two plain anomalies, no statistic
  computed from them.
- The stopped unit's selector and its readings (module documentation of `public_budgeted.rs`), which
  were read, not changed.

## Predictions (each with the range the PI would be surprised to leave)

**Feature AUCs at the ask instant, comparator row, tuning streams.** Hard (outside the slow-leak family)
against plain: `evidence` 0.55 to 0.75; `services` 0.55 to 0.75; `age` 0.40 to 0.65; `silence` 0.45 to
0.65; `contradiction` 0.45 to 0.55 (it is near-constant on plain anomalies). The best single feature in
the score at or below 0.80 (80% confident). The rung's `score_z` and `peak_z`, which are not in the
score, carry about what `evidence` carries (within 0.08 of it). Leak against decoy: `age` or `evidence`
is the best, 0.55 to 0.80; `contradiction` and `silence` at 0.5 within 0.05.

**Tuning.** At k = 2 and k = 4 a non-flat score beats the first-k baseline on verified decisions per
stream on the tuning streams by 0.10 to 0.50, and survives the cross-validation gate of the tuning rule
(probability 0.7); at k = 16 the gate leaves the flat score (probability 0.5). A budget of k = 16 spent
first-come leaves the second half of the stream unasked, so the gain from a threshold that reserves
budget may be as large at k = 16 as at k = 8; this is the prediction the PI is least sure of.

**The table (held-out).** Verified decisions per stream, comparator: never about N0, first-k at
k = 2, 4, 8, 16 about N0 + 0.2, + 0.4, + 0.9, + 1.7 (the calls are roughly uniform in time, and a call
adds about 0.07 on a plain incident and 0.5 on a hard one); always about N0 + 2.9. The tuned score
adds to first-k 0.1 to 0.5 at k = 2 and 4, less than 0.3 at k = 8 and 16. Paired differences of the
other rows against the comparator at the same k: within plus or minus 0.2 verified decisions per stream
for every row except the rungs' (z = 3 rows), which are lower; the 90% half-widths of the paired
differences 0.10 to 0.25 for verified decisions per stream, 0.04 to 0.08 for hard quality, 0.10 to 0.25
for critical misses per stream. The M2 medium at 100 ms minus the comparator at k = 8 includes zero.

**The delay sweep (held-out, selection oracle).** Hard quality of the comparator is flat across 8, 12,
16 and 20 s to within 0.05; the best delay is 12 or 16 s; the share of hard notices the rung retires
before the delay rises with the delay and is below 0.10 at 16 s. Calls per stream stay near 2.6 at every
delay. The medium at 100 ms and 500 ms follow the comparator's shape within 0.08. The spread of hard
quality across the four delays is smaller than the paired half-width of EXP-101's candidate margin
(0.05): the delay is not a large hidden selector on the oracle.

## What would change the PI's mind

A hard-against-plain AUC above 0.85 for a single feature (the public state would then carry more than
B4's rules used). A tuned score at k = 2 that does not beat first-k on the tuning streams (then the
score has no selectivity and B4's reading, that public selection on this world is always-escalate,
extends to a budget). A spread across delays above 0.10 (the 16 s delay would then be a selector of the
size of the effects EXP-101 measures).
