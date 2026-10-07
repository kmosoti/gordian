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
- The stream never reports whether a `Declare` or an answer was right (`HIDDEN-DESIGN.md` of
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
   (`gordian-stream/HIDDEN-DESIGN.md`, section 10).
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

## Notices (work items B1 and B2)

`score_notices(truth, obs_at, trace)` scores what a noticer noticed, not a trajectory. It is a
separate pure function with its own inputs: the truth, `obs_at` (the instant each passive
observation was emitted, parallel to `truth.labels`: public information the harness holds, since
the truth carries no per-observation instant, judgement 8 above) and a `NoticeTrace` (every
notice: anomaly id, anchor observation, instant of the step that yielded it; every retirement:
anomaly id, instant). It does not read the trajectory or the call records. Rule ids start with `N`;
`fixtures/notice-cases.json` pins each, and `tests/notice_fixtures.rs` fails if a rule here is
pinned by no case or a case names a rule that is not here.

| Id | Rule |
|---|---|
| N1 | **A notice is about an incident** when its anchor's label names the incident, read as S2 reads a declaration's anchor. An anchor labelled background makes a notice about no incident. Which observations the notice *attaches* is not read: only the anchor counts. |
| N2 | **Noticed.** An incident (of any tier) is noticed when at least one notice is about it. |
| N3 | **First notice.** `first_notice_at` is the earliest instant among the notices about the incident, `None` when it was not noticed. |
| N4 | **Notice latency.** The incident's *first observation* is the lowest-id observation labelled with it (`IncidentTruth::observations`, the first), at the instant `obs_at` gives it. `notice_latency_ns` is `first_notice_at` minus that instant; `None` when the incident was not noticed or has no observation. It is measured from the first observation, not from the onset and not from the first abnormal observation: for a family whose first observations are not abnormal by the public rules (the slow leak's early readings) it includes that stretch. |
| N5 | **Anchor-correct.** A notice is anchor-correct when its anchor belongs to an incident (N1) and the anchor's instant is within `ANCHOR_WINDOW_NS` (1 s, inclusive) of that incident's first observation, in absolute difference. An incident is anchor-correct when at least one notice about it is. A notice anchored exactly 1 s after the first observation is anchor-correct; one nanosecond later is not. Anchor-correct is a property of the anchor only: it does not say the notice was the first, the only, or a useful one. |
| N6 | **Notices per incident.** `notices` counts the notices about the incident, whatever their anchors' offsets. |
| N7 | **Per-notice score.** For each notice, in the order recorded: the incident it is about (N1) and its tier, the anchor's offset from the incident's first observation (`None` for background or an incident with no observation) and whether it is anchor-correct (N5). |
| N8 | **Notices by what they are anchored on.** `notices` is the number recorded. `on_background`, `on_plain`, `on_hard` and `on_decoy` count them by what the anchor belongs to (N1); the four sum to `notices`. |
| N9 | **Incident totals.** `noticed` (N2) and `anchor_correct` (N5) count incidents by tier. Ratios (the share of hard incidents noticed, the share anchor-correct, the leak's share, notices on background per stream, notices per incident) are pooled from counts across streams by the analysis, as the derived ratios above are; a per-stream ratio is not written. |
| N10 | **Retirements.** `retirements` is the number recorded. A retirement is not about an incident: it is the noticer's record of an anomaly it is finished with. |
| N11 | **Errors, not verdicts.** The record is refused, with no verdict, in this order: `obs_at` does not have one instant per label; an incident's id is not its position; a label names an unknown incident; then for each notice in order, the anchor is not an observation, the notice is earlier than its anchor's instant, the notice is earlier than the notice before it, the anomaly was noticed already; then for each retirement in order, it is earlier than the retirement before it, its anomaly is not live (never noticed, or retired already), it is earlier than its notice. |
| N12 | **Purity.** The verdict is a function of the three inputs; it reads no clock, no randomness, nothing else. Notices and retirements are not trajectory steps, are never charged and do not move any score of `score_stream`. |
| N13 | **The incident's site** (work item B2) is the first service the incident occupies (`IncidentTruth::occupies`, site first), whatever its tier: a decoy has no diagnosis and so no diagnosed site, but it occupies one. An incident that occupies nothing has no site (only a hand-built truth does). For a plain or hard incident the first occupied service is the site of its true diagnosis; `tests/generated.rs` checks it on generated streams. A **notice's site** is the service the record says it is about (`NoticeEntry::site`, the index of the `ServiceId`); a hand-written record may leave it out. |
| N14 | **Site-correct.** A notice is site-correct when it is about an incident (N1), the record gives it a site, the incident has a site (N13), and the two are equal. A notice anchored on background, a notice with no site, and a notice about an incident with no site are not site-correct. The site is compared with the site of the incident the *anchor* belongs to, not with any incident's. An incident is site-correct when at least one notice about it is. Like N5 it is a property of one field of the notice: it does not say the notice was anchored on the right observation. |
| N15 | **Anchor-and-site-correct.** A notice is anchor-and-site-correct when it is anchor-correct (N5) and site-correct (N14). An incident is anchor-and-site-correct when at least one *single* notice about it is both; a notice that is anchor-correct and another that is site-correct do not make it so. `notices_site_correct` and `notices_anchor_site_correct` count notices of every tier; `site_correct` and `anchor_site_correct` count incidents by tier, as N9's counts do. |
| N16 | **Notice precision.** The counts of N8 and N15 give three ratios over a stream's notices, pooled across streams by the analysis from the counts (N9), never averaged per stream: *precision* is the notices anchored on an incident of any tier (`on_plain + on_hard + on_decoy`) over all notices; *precision in a tier* is the notices anchored on incidents of that tier over all notices (so the tiers' precisions add to the precision); *strict precision* is the anchor-and-site-correct notices (N15) over all notices. Each is undefined (`None`) for a stream with no notice. `NoticeTotals::precision`, `precision_in` and `strict_precision` compute them for one stream. |

Rules N13 to N16 are work item B2's. They add inputs and counts and change none of N1 to N12:
the fixtures of N1 to N12 are unchanged and still pass (a record without a site is allowed, N14).

### Where the notice rules are a judgement

1. **Only the anchor counts (N1).** A noticer that anchors on background but attaches an incident's
   observations has noticed nothing of that incident by this measure, though the rung may then work
   on the incident's evidence under the wrong anchor (R10 found this is how most never-noticed
   incidents arise). The measure is the one the queue fixed; "evidence attached to an incident" is
   a different measure and is not scored here.
2. **Any later notice makes an incident noticed (N2).** An incident noticed once, late, is
   noticed. Latency (N4) and anchor-correct (N5) say how well.
3. **Anchor-correct is gameable (N5).** It rewards an anchor on the incident's first observation
   wherever it is cheap to put one: a noticer that notices every abnormal observation is
   anchor-correct for every incident that begins with an abnormal observation. It must be read
   with notices on background per stream and notices per incident beside it, never alone.
4. **The 1 s window is applied to the anchor's emission instant**, not to the instant of the
   notice (N5): a noticer may notice late and still be anchor-correct, which latency (N4) shows.
5. **The incident's site is the first occupied service (N13).** The truth gives a diagnosed site only
   to plain and hard incidents; reading `occupies[0]` gives every tier one. If a family of incident
   were generated whose true site is not the first occupied service, N14 would call a correct notice
   wrong; the generated-stream test pins the agreement for plain and hard incidents and would fail.
6. **Site-correct is gameable in the other direction (N14).** A noticer that is about every service
   is site-correct for every incident it also anchors on. It is a check on one field, and is read with
   anchor-correct (N5), the single-notice conjunction (N15) and precision (N16), never alone.
7. **Precision counts notices, not incidents (N16).** A noticer that notices an incident ten times
   has ten notices on it. Precision is therefore the measure that a flood lowers, and it is also
   lowered by a noticer that re-notices a long incident; notices per incident (analysis) says which.

## Selection accounting (work item B4)

`score_selection(truth, trace)` says what a selector asked about and what each question cost, notice
by notice, and what a follow-up rule retired. It is a separate pure function with its own input, a
`SelectionTrace` (every notice, as `NoticeEntry`; every retirement, with whether a follow-up rule made
it; every accepted escalation, with the instant, the focus and the cost the stream declared for it),
and adds nothing to `score_stream` and nothing to `score_notices`: the notices' own scores (N1 to N16)
are unchanged. It exists because the selection oracle never asks about a notice anchored on a decoy or
on a late plain incident, both precision measures count such a notice as correct, and the background
budget does not charge it, so neither quality nor any notice measure shows what noticing it costs a
selector that must decide. Rule ids start with `E`; `fixtures/selection-cases.json` pins each, and
`tests/selection_fixtures.rs` fails if a rule here is pinned by no case or a case names a rule that is
not here.

| Id | Rule |
|---|---|
| E1 | **An escalation is about a notice** when the notice's anchor is the escalation's focus, the notice was made at or before the escalation's instant, and the notice's anomaly was not retired before the escalation's instant (a retirement at the escalation's own instant is after it: the rung asks before it retires within a step). When several notices qualify, the latest one recorded is the one. An escalation about no notice is *unattributed* (a call a rule makes directly about an observation no live anomaly is anchored on, or about an anchor that has since moved); it is still counted by the class of its focus (E2). |
| E2 | **Escalation classes and cost.** An escalation belongs to the class of the observation its focus names, read as S2 reads it: *background* (no incident), *plain*, *hard* (a hard incident outside the slow-leak family), *leak* (a hard incident of the slow-leak family), *decoy*. Per class the totals are the number of calls and the tokens and modelled nanoseconds the outcomes declared; the five classes' calls, tokens and modelled nanoseconds add to S26's `calls`, `tokens` and `modelled_ns`. |
| E3 | **Per notice.** One outcome per notice, in the order recorded: its anomaly, the incident its anchor belongs to and the class of that incident (N1, with E2's classes; an anchor of no incident is background), the number of escalations about it (E1), the instant of the earliest, the instant of its retirement if there is one, whether a follow-up rule retired it, and E4's flag. |
| E4 | **Retired before escalation.** A notice is retired before escalation when a retirement of its anomaly is recorded and no escalation is about it (E1). A notice never retired within the record is not; a notice asked about and then retired is not. The flag says that the retirement cost nothing in reasoner calls; it does not say the retirement was right. |
| E5 | **Retired by a follow-up rule.** A retirement carries whether a follow-up rule made it (as against the anomaly going quiet). `followup_retired` counts the notices so retired by the class of their anchor, and `followup_before_escalation` those that were also retired before escalation (E4). A follow-up retirement of a notice anchored on a leak is a leak wrongly retired; of one anchored on a decoy, a decoy notice retired; the evaluator does not call either right or wrong (judgement 2). |
| E6 | **Cost share.** The share of the reasoner's cost spent on a class is the class's tokens (or modelled nanoseconds) over all classes', pooled across streams by the analysis from the totals of E2, never averaged per stream. |
| E7 | **Totals by class.** `notices`, `escalated` (notices with at least one escalation about them), `retired_before_escalation`, `followup_retired` and `followup_before_escalation`, each by the class of the notice's anchor, are counts of notices over the stream. Ratios are pooled by the analysis (notices on decoys per decoy is the pooled count of notices on decoys over the pooled count of decoys, which the incident files hold). |
| E8 | **Errors, not verdicts.** The record is refused, with no verdict, in this order: a notice whose anchor is not an observation of the stream; a retirement of an anomaly never noticed, or retired already; an escalation whose focus is not an observation of the stream. The notices' times and the retirements' order are `score_notices`' checks (N11), which the harness runs on the same record first. |

### Where the selection rules are a judgement

1. **Attribution is by focus equal to anchor (E1).** A selector's call is about the observation it
   names; a notice's anchor is the observation the rung asks about. A rule that asks about another
   observation than an anomaly's anchor has made a call about no notice (unattributed), and the
   count says how many; for the rules built so far it is zero except for the privileged notice arm,
   which asks about anchors it injected itself.
2. **"Retired before escalation" is not "correctly retired" (E4, E5).** A decoy notice retired
   before it was asked about spared a call; a leak notice retired before it was asked about lost
   the incident its notice was the only anchor of (unless another notice is about it, which the
   per-incident files show). The evaluator counts; it does not judge.
3. **A retirement by quiet counts as retired before escalation too (E4).** A decoy anomaly that goes
   quiet before the rule would have asked about it is retired before escalation, with or without a
   follow-up rule. The comparison that isolates a follow-up rule is the same arm with and without
   it (E5's counts), not E4's alone.
4. **Escalation cost is the stream's declared cost (E2).** It is the reasoner's cost in its own
   units, as S26: it does not include the cheap components the rung runs to decide whether to ask
   (the contradiction checker a selector monitors with), which are in `results.csv`'s substrate
   columns.

## Memory (work item E1)

`score_memory(truth, trajectory, recalls)` says what an arm declared without asking, and which of
those declarations it made from memory and from where. It is a separate pure function with its own
inputs: the truth, the trajectory `score_stream` scores (the accepted `Declare` and `Escalate`
steps are read; a refused step is nothing, S3) and a record of **recalls**, one `RecallEntry` per
declaration the harness marked as made from memory (`Source::Recall`): the index of its step in
the trajectory and, when the memory says so, its **source**, the observation the stored answer was
about and the answer as stored. It adds nothing to `score_stream`'s verdict and moves none.
`fixtures/memory-cases.json` pins each rule (K1 to K10), and `tests/memory_fixtures.rs` fails if a rule here is
pinned by no case or a case names a rule that is not here. Rule ids start with `K`.

The brief's `unasked_correct` and `stale_wrong` are defined here as the amended brief (W2, after
the first draft) has them: an unasked wrong declaration counts every error an arm makes without
asking, the cheap rung's own included (W2 section 7.5: 17% of plain incidents and 73% of decoys at
no memory at all), so a memory's own errors are the declarations the harness marks as recalls,
classed by their source.

| Id | Rule |
|---|---|
| K1 | **Recurrence.** `recurrence_of` is the truth's flag, the index of the incident this one repeats, for an incident of any tier; `None` when it repeats none. Hidden side; not an arm input. |
| K2 | **Same family earlier.** The *class* of a hard incident is its hard kind (`shape.hard_kind`) and its mode (`shape.contradicts_early`: `Some(true)` the evidence breaks the public rules at once, `Some(false)` it imitates a known kind first, `None` for the slow leak, which has no mode): seven classes, as W2 fixed them. An incident has `same_family_earlier` when it is hard, has a class and a site (K9), and an *earlier* hard incident (a lower id) of the same class at *another* site exists. A recurrence of an incident at the same site does not make it true; an incident that is both a recurrence and has an earlier same-class incident elsewhere is true. A decoy and a plain incident are never true, and a decoy does not count as the earlier incident (W2 item 2). |
| K3 | **Unasked correct.** An incident's declarations are the accepted declarations whose anchor belongs to it (S2), and its escalations the accepted escalations whose focus belongs to it (S13). It is `unasked_correct` when it has at least one correct declaration (S4, S5; for a decoy, a dismissal, S15) and no escalation, at any instant: a declaration made before an escalation does not count if an escalation about the incident follows. Defined for every tier; the population that carries the claim is the hard incidents. |
| K4 | **Unasked wrong.** An incident is `unasked_wrong` when it has at least one wrong declaration (S11; for a decoy, a false alarm, S14) and no escalation. It counts every wrong declaration whoever made it (the shared rule, a recogniser or a memory), so it is read as the **paired excess over the memoryless arm** on the same incidents (analysis, "Derived measures"), never as a memory's error rate. |
| K5 | **A recall, its outcome and its source.** A recall is *correct* when its declaration equals the truth of the incident its anchor belongs to (S4, S5; `None` is correct on background and on a decoy, S15, S18), and *wrong* otherwise. Its *source class* is *right* when the source's stored diagnosis equals the truth of the incident the source observation belongs to (`None`, "not an incident", for an observation of background and for a decoy's), *wrong* when it does not, and *unknown* when no source was recorded (the medium's engram does not record one; the record rung does). The source's incident is read as S2 reads an anchor; its instant is not read. |
| K6 | **The six cells and stale wrong.** Each recall falls in one of six cells: correct or wrong, crossed with source right, wrong or unknown. A **wrong recall with a right source** is a *collision or staleness*: the stored answer was right for its own incident and wrong for this one (the key matched another truth, or the physics had changed since). A **wrong recall with a wrong source** is *inherited*: the memory repeated the reasoner's error. The two are never merged. An incident is `stale_wrong` when it has at least one wrong recall and no escalation (K3's reading of "unasked"); its `recalls` cells count its recalls, over every recall anchored on it, escalated or not. |
| K7 | **Totals.** `recalls` is the six cells over every recall; `recalls_on_plain`, `_on_hard`, `_on_decoy` and `_on_background` split them by what the anchor belongs to and add to `recalls`. `unasked_correct`, `unasked_wrong` and `stale_wrong` count incidents by tier. `hard_recurrences` counts hard incidents with K1 set, `hard_elsewhere` hard incidents with no K1 and K2 set, `hard_reachable` hard incidents with either; each has an `_unasked_correct` count of those that are K3. The three populations are W2's: a site-keyed memory can reach the first, a family-keyed one all three. Ratios are pooled from counts across streams by the analysis, never averaged per stream (a stream has about 0.4 hard recurrences). |
| K8 | **Errors, not verdicts.** The record is refused, with no verdict, in this order: for each recall in order, its step does not exist (`StepOutOfRange`), is not an accepted declaration (`NotADeclaration`), or is named by an earlier recall (`StepRepeated`); then for each accepted step in order, a declaration's anchor or an escalation's focus is not an observation of the stream (`UnknownObservation`); then for each recall in order, its source observation is not an observation of the stream (`UnknownObservation`). The other checks of `score_stream` (S27 to S37) are `score_stream`'s, which the harness runs on the same trajectory first. |
| K9 | **A site** is the first service the incident occupies (N13). An incident that occupies nothing has no site, and K2 is false for it. |
| K10 | **Purity.** The verdict is a function of the three inputs; it reads no clock, no randomness, nothing else. Recalls are not steps, are never charged and move no score of `score_stream`. |

### Derived measures (the analysis's, from the rows)

Not rows; computed from `memory_incidents.csv`, `memory.csv`, `recalls.csv` and the other
evaluator files by `analysis/gordian_analysis/memory.py`:

- **Unasked-correct share** of a population: the K3 count over the population's size, pooled across
  streams (hard recurrences, same-family-elsewhere, reachable, all hard incidents).
- **Collision share:** wrong recalls with a right source over all recalls with a right source
  (`wrong_source_right / (correct_source_right + wrong_source_right)`), pooled; and per stream, the
  wrong-recall count with a right source. W2's bound for A1b is at most 0.20 of such recalls and
  at most 0.25 per stream.
- **Inherited errors:** wrong recalls with a wrong source, as a count per stream and as a share of
  recalls with a wrong source.
- **Paired excess over the memoryless arm:** for two arms that played the same streams, the sum over
  incidents of `unasked_wrong(arm) - unasked_wrong(control)`, joined by (seed, incident), also by
  tier. The cheap rung's own errors cancel; what is left is what the memory added or removed.
- **Calls per correct decision:** `reasoner_calls` over `correct_plain + correct_hard` (S6, S20) from
  `results.csv`, the correct decisions by their deadline whoever made them, pooled across streams.
  Not an evaluator output.
- **Experience curves:** the cumulative unasked-correct counts (K3), and the cumulative recalls by
  cell (K6), against the incidents seen in stream order (id order within a stream, seed order
  across streams), from the per-incident rows. Not an evaluator output.

### Where the memory rules are a judgement

1. **"Right for its own incident" compares the stored answer with the truth of the source's
   incident, kind and site (K5).** A family-keyed memory that substitutes the new anomaly's site
   into a stored kind declares the right kind at a new site; its stored answer is still compared at
   its own site, which is what the reasoner was asked about. A source whose answer was right in
   kind and wrong in site is wrong by this rule, and so is a recall it makes of that kind
   (W2 section 7.6 counts the family-keyed wrong share as an upper estimate for the same reason).
2. **The source is the memory's, not the world's (K5).** A recall whose source is right and which
   is wrong is called a collision or staleness, the two together; the evaluator does not separate a
   key that matched another family from a key whose physics had since changed (that needs W2's
   rebuilt incidents, a hidden-side script, not a rule here). The recall's time after a regime
   change is in `recalls.csv` for whoever wants to cut it.
3. **A recall that was followed by an escalation about its incident is still a recall (K6).** The
   arm's confirmation policies ask instead of declaring, so a recall that is declared is not
   escalated by the arms built so far; an arm that does both has its recall counted in the cells
   and not as `stale_wrong`.
4. **Unasked is a property of the incident, not of the declaration (K3).** A correct declaration
   made by the cheap rung on a plain incident the selector never asked about is unasked correct, as
   it should be: 85.5% of plain recurrences are, with no memory at all (W2 7.5), which is why the
   clause that carries the memory claim is on hard incidents.
5. **A decoy's "correct" declaration is a dismissal (K3, S15)**, and a recall that declares an
   incident on a decoy is wrong, as S14 says.
