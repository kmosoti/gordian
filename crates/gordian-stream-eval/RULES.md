# Stream evaluator rules

What `gordian_stream_eval::score_stream(truth, trajectory, calls)` does, one numbered rule per
row. The code cites these ids (`src/score.rs`, `src/error.rs`, `src/verdict.rs`); every case in
`fixtures/stream-cases.json` names the rules it pins in its `rules` field;
`tests/fixtures.rs` fails if a rule here is pinned by no case or a case names a rule that is not
here. Ids start with `S` so they cannot be confused with the first evaluator's `R` ids
(`crates/gordian-eval/RULES.md`) or the plan's work items.

`score_stream` is a pure function: same truth, same steps and same call summaries give the same
`Result`. It is tested as a property, not as a rule row. The derived ratios at the end are
methods on the totals, tested directly, not rows.

## Inputs

- `StreamTruth`, from `gordian_stream::oracle`: the duration, every incident (`IncidentTruth`:
  id, tier, criticality, onset, deadline, true diagnosis) and one label per observation.
  `truth_from_stream(&Stream)` builds one from `oracle::reveal`; fixtures build one by hand. The
  scorer reads only: `duration_ns`, `labels`, and per incident `id`, `tier`, `critical`,
  `onset_ns`, `deadline_ns`, `truth`.
- `StreamStep { at, action, outcome }`, the stream's `StreamAction` and `StreamOutcome`
  unchanged. The outcome says what happened; the action says what was tried.
- `CallSummary { at, ready_at, focus, refs, informed, correct }`: the hidden side of each
  accepted reasoner call, in the order accepted. `calls_from_sim(&StreamSimulator)` builds them
  from `oracle::calls`.

## Rules

A "declaration" or an "escalation" in a rule below always means an *accepted* one (S1, S3).

### Reading the trajectory

| Id | Rule |
|---|---|
| S1 | **Declaration.** A step whose action is `Declare` and whose outcome is `Declared`. It says what is wrong (`diagnosis`) with the incident its anchor belongs to; `None` says it is not an incident. |
| S2 | **The anchor names the incident.** A declaration is about `truth.incident_of(anchor)`: the incident the anchor's label names, or no incident (background) when the label is background. An escalation is about the incident its `focus` belongs to, read the same way. The hypothesis in a declaration never chooses the incident: a correct hypothesis for incident 0 anchored in incident 1 is a wrong declaration about incident 1 and nothing about incident 0. |
| S3 | **Refusals count for nothing.** A step whose outcome is `Refused(_)` is not a declaration, an escalation or a cost, whatever the reason. Its anchor, focus and instant (past the end, or in a context the stream refused) are not checked, except that the instant may not go backwards (S30). |

### Per incident

| Id | Rule |
|---|---|
| S4 | **Correct declaration.** A declaration is correct when its `diagnosis` equals the truth of the incident it is about (S2): `Some` of the same kind and the same site, or `None` for a decoy. This holds for every tier and at every instant; the deadline is S6. |
| S5 | **Kind and site both.** The right site with a wrong kind, or the right kind at a wrong site, is not correct. Naming the imitated known kind at a hard incident's site is therefore wrong. |
| S6 | **Correct by deadline** (plain and hard only). The incident has a correct declaration at an instant at or before its deadline. A declaration at exactly the deadline is on time. The earliest correct declaration decides: it is on time or none is. |
| S7 | **Late.** A correct declaration after the deadline does not make the incident correct by deadline (S6). It is still a correct declaration (S4), it still sets `first_correct_at` (S8), and it is not a wrong one (S11). |
| S8 | **Time to first correct declaration.** `first_correct_at` is the instant of the earliest correct declaration, whatever the deadline, and `time_to_first_correct_ns` is that instant minus the incident's onset; both `None` when there is none. For a decoy it is the first dismissal. |
| S9 | **Missed** (plain and hard only). Not correct by deadline (S6): nothing declared, only wrong declarations, or only late ones. Never true for a decoy. |
| S10 | **Critical miss.** The incident is critical and missed (S9). A decoy is never critical, so it is never a critical miss. |
| S11 | **Wrong declarations.** The number of declarations about the incident that are not correct (S4). Counted for every tier. A wrong declaration never cancels a correct one, and a correct one never cancels a wrong one: wrong then right in time is correct by deadline with one wrong declaration; right then wrong is correct with one wrong declaration. |
| S12 | **`None` about a plain or hard incident** is a wrong declaration (S11), as `None` is not its truth. It is not a false alarm: false alarms are only about decoys and background (S14, S18). |
| S13 | **Escalations about an incident.** The number of escalations whose focus belongs to it (S2). `informed_escalations` and `correct_escalations` count, among them, the calls whose summary has `informed` and `correct` set. The summary of a call is `calls[n]` for the outcome whose `call` is `n`. |

### Decoys and background

| Id | Rule |
|---|---|
| S14 | **False alarm.** A `Some` declaration about a decoy. Each such declaration is one false alarm. It is the decoy's `wrong_declarations` (S11). |
| S15 | **Correct dismissal.** A `None` declaration about a decoy, at any instant: a decoy has no deadline. It is the decoy's `correct_declarations` (S4). |
| S16 | **Silence about a decoy** is neither a false alarm nor a dismissal. The decoy has no declaration, is `missed` false (S9), and counts only in `decoys_silent` (S21). |
| S17 | **Alarm and dismissal are not netted.** A decoy with both a `Some` and a `None` declaration has a false alarm and a dismissal; neither cancels the other. It counts in `decoys_alarmed` and in `decoys_dismissed`. |
| S18 | **Declarations about background.** A `Some` declaration anchored on an observation of no incident (a stray, a blip, a mini-burst) is a false alarm and counts in `false_alarms_on_background`. A `None` declaration anchored there is correct and counts for nothing. |

### Totals

| Id | Rule |
|---|---|
| S19 | **Incident counts.** `incidents` counts every incident of the truth by tier, and `critical_incidents` those with `critical` set, whether or not the trajectory touched them. A stream with no incident has all zeros. |
| S20 | **Plain and hard outcomes.** `correct` (S6), `missed` (S9) and `critical_missed` (S10) count plain and hard incidents separately; `wrong_declarations` sums S11 over plain and hard incidents. Decoys contribute to none of these. |
| S21 | **Decoy outcomes.** `decoys_dismissed` counts decoys with a dismissal (S15), `decoys_alarmed` those with a false alarm (S14), `decoys_silent` those with no declaration (S16). Each decoy counts once per category. |
| S22 | **False alarms.** `false_alarms` is the sum of decoys' false alarms (S14) and declarations about background (S18); `false_alarms_on_background` is the second part. It counts declarations, not decoys. |
| S23 | **Escalation classes.** Each escalation is counted once: `needed` if it is about a hard incident, `unneeded` if about a plain incident or a decoy, `background` if its focus belongs to no incident (S2). A repeat escalation about the same incident counts again. `needed + unneeded + background` equals `reasoner.calls`. |
| S24 | **Incidents escalated.** `hard_incidents_escalated` counts hard incidents with at least one escalation; `other_incidents_escalated` counts plain incidents and decoys with at least one. |
| S25 | **Informed and correct calls.** `escalations.informed` and `escalations.correct` count the calls whose summary has `informed` and `correct` set, over all calls including background. |
| S26 | **Reasoner cost, in its own units.** `reasoner.calls` is the number of escalations; `refs` is the sum of their context lengths; `tokens` and `modelled_ns` are the sums of the `cost` each outcome declared. A refused escalation costs nothing (S3). |

### Errors, not verdicts

An error replaces the verdict: there is no partial result. The error names the first offending
step where there is one.

| Id | Rule |
|---|---|
| S27 | **Error: incident ids.** An incident whose `id` is not its position in `truth.incidents` is `IncidentIdMismatch`. |
| S28 | **Error: tier contradicts truth.** A decoy with a true diagnosis, a deadline, or `critical` set, or a plain or hard incident with no true diagnosis or no deadline, is `TierContradictsTruth`. |
| S29 | **Error: label of unknown incident.** An observation labelled with an incident id that is not in the truth is `LabelOfUnknownIncident`. |
| S30 | **Error: time goes backwards.** A step whose `at` is strictly earlier than the previous step's, refused or not, is `TimeWentBackwards`. Equal instants are allowed. |
| S31 | **Error: outcome mismatch.** `Probe` may be answered only by `Probed` or a refusal, `Escalate` only by `Escalated` or a refusal, `Declare` only by `Declared` or a refusal. Anything else is `OutcomeMismatch`. |
| S32 | **Error: accepted after the end.** An accepted step (anything but `Refused`) whose `at` is after `truth.duration_ns` is `ActionAfterEnd`; the stream refuses every action past its duration (`PastDuration`). A step at exactly the duration is allowed. A refused step past the end is an ordinary refusal (S3). |
| S33 | **Error: unknown observation.** An accepted declaration's anchor, or an accepted escalation's focus, that is not an index into `truth.labels` is `UnknownObservation`. |
| S34 | **Error: observation not yet emitted.** An accepted declaration's anchor, or escalation's focus, that belongs to an incident whose `onset_ns` is after the step's `at` is `ObservationNotYetEmitted`: no observation of an incident precedes its onset. The truth holds no instant for individual observations, so a background observation, or an observation of an incident that has begun but has not yet been emitted, is not caught here (judgement 8). |
| S35 | **Error: call record mismatch.** An accepted escalation must carry `call == n` for the n-th accepted escalation of the trajectory (from zero), and `calls[n]` must exist and agree with the step: `at` equal to the step's, `ready_at` equal to the outcome's, `refs` equal to the context length, and `focus`, when present, equal to the question's. Otherwise `CallRecordMismatch`. |
| S36 | **Error: call count.** After the last step, a number of call summaries different from the number of accepted escalations is `CallCountMismatch`. |
| S37 | **Order of checks.** The truth is checked before any step: S27, then S28, then S29. For each step: S30, then S31, then S32, then S33 and S34 (S33 first), then S35. After the last step: S36. The first failing check on the first offending step is returned. |

## Derived ratios

Not rows; methods on `StreamTotals`, computed from the counts above. Ratios must be pooled from
counts across streams, not averaged per stream: a stream has about two hard incidents.

- `escalation_precision()`: `needed / (needed + unneeded + background)`, a call-level ratio.
  `None` when there was no escalation. Ten escalations about one hard incident score ten
  needed ones.
- `escalation_recall()`: `hard_incidents_escalated / incidents.hard`, an incident-level ratio.
  `None` when the stream has no hard incident.

The two are at different levels on purpose: precision is about where the reasoner's spend went,
recall is about which hard incidents it reached. Either can be changed by repeated calls on one
incident; read them with the counts (S23, S24) and the cost (S26).

## How the stream's types were read

- `StreamOutcome::Probed`, `Escalated` and `Declared` mean the stream accepted and carried out
  the action. `Refused` means it charged nothing and changed nothing.
- The stream never reports whether a `Declare` or an answer was right (`DESIGN.md` of
  `gordian-stream`, section 14, route 22), so correctness comes only from the truth and from the
  call summaries.
- Reasoner answers are `StreamEvent::Answered`, delivered by `observe_until`, not steps. The
  scorer does not read them. An escalation's answer matters to the score only if the policy then
  declares (S1).
- A declaration does not end an incident and may be repeated: the stream accepts any number of
  declarations about one incident. There is no retraction, so S11 is the only price of a wrong
  one.
- `StreamTruth::incident_of` is the only way an observation is attributed to an incident.

## Where the rules are a judgement

1. **A late correct declaration is not a wrong one** (S7, S11). The deadline is a property of
   timeliness, not of correctness; the verdict keeps the two apart so that a consumer can count
   either. Whether late declarations enter an experiment's primary outcome must be stated in its
   preregistration.
2. **Wrong declarations do not cancel a correct one** (S11). An arm that declares every
   hypothesis for every incident gets `correct_by_deadline` and a large `wrong_declarations`. The
   per-incident success flag alone is not a measure of an arm; each preregistration must say
   whether `wrong_declarations` or `false_alarms` bounds the primary outcome. The evaluator does
   not collapse them into one number.
3. **A decoy can be alarmed and dismissed at once** (S17). A policy that alarms at 8 s and
   dismisses at 25 s has done both and the verdict says so.
4. **A dismissal has no deadline** (S15). A policy that dismisses a decoy after it has long
   resolved is scored as correct. This is the only reading of "decoys have no deadline"; whether
   a late dismissal is useful is a question about cost, not scoring.
5. **"Needed" is about the incident's tier, not about the call's timing or its information**
   (S23). An escalation about a hard incident after its deadline, or after it was already
   declared correctly, is still counted needed. An escalation about a hard incident with a
   context missing all its decisive evidence is needed too: `informed` (S25) says whether it
   could have helped.
6. **Escalation precision is call-level and recall is incident-level.** See Derived ratios.
7. **A `Some` declaration anchored on background is a false alarm** (S18), and a `None` anchored
   there is nothing. The plan names decoys, not background; a mini-burst is the stream's own
   believable non-incident, and an anchor on it is "a declaration about nothing"
   (`gordian-stream/DESIGN.md`, section 10).
8. **An anchor's time can only be checked against the incident's onset** (S34). The truth carries
   no per-observation instants, so a declaration anchored on an observation of an incident that
   exists but is not yet delivered at the step's instant is not caught here. The stream refuses
   it (`UnknownRef`), so in a real run it appears as a refused step (S3), not as an accepted one.
   A harness bug that bypassed the stream's refusal would pass the evaluator.

## Deviations from `docs/local-test-plan.md` section 5R, R2 and from the task's interface

| Specified | Here | Why |
|---|---|---|
| `CallSummary { at, ready_at, focus, refs, informed, correct }` | `focus` is `Option<ObsId>` | The evaluator was written when `gordian_stream::oracle::CallTrace` did not carry the focus, so a source might not know it. Work item R3b added `focus` to the trace and `calls_from_sim` now fills `Some(focus)`; the scorer still takes the focus from the trajectory's `Escalate` step, and a present focus must agree with it (S35). The `Option` stays so that a hand-written fixture or another source may leave it out. |
| Errors: time going backwards; outcome impossible for its action; an anchor naming no observation the policy could hold; a declaration after the stream's end | S30, S31, S33 and S34, S32; S27 to S29, S35 and S36 are additions | S32 covers every accepted step past the end, not only declarations: the stream refuses all of them with `PastDuration`. S34 is the part of "could hold" that the truth can decide (S34 and judgement 8). S35 and S36 stop a harness from attributing one call's `informed` flag to another call. S27 to S29 reject a hand-built truth that contradicts itself, as A2's R18 did. |
| Per incident: "escalations made about it, and whether any was informed" | counts `escalations`, `informed_escalations`, `correct_escalations` | A count says more than "any" and the summary also carries `correct`. |
| Totals: "escalation precision and recall against 'hard incident'" | counts, and two methods | Ratios must be pooled from counts across streams (Derived ratios). |
| Totals: "total reasoner cost in its own units (calls, references, tokens)" | adds `modelled_ns` | It is the quantity the stream's hard budget limits, and the charter's exchange rate (section 4) is applied to it by the harness. |
| Decoys: silence is "neither" | `decoys_silent` counts it | A count of the neutral case lets a consumer check that the three decoy outcomes cover every decoy. |
