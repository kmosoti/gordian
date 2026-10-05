# B3 finding 4: why a failed component raised an arm's score

Status: development run, `exploration-followups` branch. Nothing here tests a hypothesis and
nothing here may later be cited as confirmation. **The shared decision rule, the harness and every
crate are unchanged**; the experiments below ran on scratch variants of a copy of `crates/`
(`scripts/finding4-diagnostic.patch`, never applied to the tree). Whether to change the rule is
the coordinator's decision.

## 1. Result in five lines

- The effect is real, and the paired experiment shows it comes from the `Fail` directives alone: on
  the same episodes, in the three cells `Fail` raises success in 130 episodes and lowers none
  (sections 3 and 4).
- The suspected cause is **not** what lifts the score. Reading the verifier's stale empty-window set
  first is what makes the *undirected* episodes fail (they end on a final call that declares "no
  fault"), but removing that priority changes success by at most 1.8 points (section 5).
- The cause is a charge: the shared rule bills the decoding of every component output it is given,
  at every step, in proportion to the number of hypotheses in it. On a window with no symptom yet
  the verifier's output lists 46 hypotheses, so that bill is 5.8 thousand of the roughly 7.6 thousand
  declared nanoseconds the rule costs per step. A `Fail` removes the output, so the rule stops
  paying for it and the budget outlasts the symptoms (section 5).
- This is the cost model working as written (`POLICIES.md` 2.1 and 3), not a mistake in the
  decision logic, but it is an instrument property with a large reach: it decides most of what
  happens to every verifier-running arm at a binding budget (section 7).
- B3's "3 cells" was a count against `Ambiguous` and understates the effect: `Slow` directives in
  the same episodes hide it in four more cells (section 3).

## 2. What B3 reported, and the cells

`b3-stress.md` section 3, item 5: of the 22 of 40 `ComponentTimeout` cells that differ from
`Ambiguous`, 3 are higher. The three cells (success %, `ComponentTimeout` against `Ambiguous`, 500
episodes each, the committed B1 data):

| arm | compute limit (ns) | ComponentTimeout | Ambiguous |
|---|---|---|---|
| `all_components` | 250,000 | 18.4 | 13.8 |
| `random p=0.5` | 100,000 | 7.8 | 3.6 |
| `random p=0.25` | 60,000 | 11.2 | 5.0 |

B3's comparison is between two different classes, so it could not say whether a directive or the
class made the difference. The paired design below can.

## 3. The paired experiment (same manifest, seed and class; only the directives differ)

Setup. `ComponentTimeout`, seeds 1000 to 1499, the B1 manifests' limits, rule patience, episode
parameters and arm definitions, seven public arms. Four conditions, selected by a scratch
environment variable read where the harness copies the episode's directives
(`GORDIAN_DIAG_DIRECTIVES`): `all` (as B1), `none` (no directive applied), `fail` (only `Fail`
directives), `slow` (only `Slow`). The patched binary with the variable unset wrote a
`results.csv` identical, on every row and every deterministic column, to the committed code's
B1 rerun, for all 7 arms at all 4 budgets (14,000 rows). Directives were listed per episode by a
scratch example that uses only the public `harness_directives()` accessor.

| arm | compute (ns) | all (= B3) | none | Fail only | Slow only | Ambiguous | raised / lowered by Fail only | by Slow only |
|---|---|---|---|---|---|---|---|---|
| `all_components` | 250,000 | 18.4 | **13.8** | 24.0 | 9.4 | 13.8 | 51 / 0 | 0 / 22 |
| `random p=0.5` | 100,000 | 7.8 | **4.0** | 10.6 | 2.0 | 3.6 | 33 / 0 | 0 / 10 |
| `random p=0.25` | 60,000 | 11.2 | **5.6** | 14.8 | 3.6 | 5.0 | 46 / 0 | 0 / 10 |

- With no directive `ComponentTimeout` scores what `Ambiguous` does (13.8, 4.0, 5.6 against 13.8,
  3.6, 5.0), so the class itself is not the source of the difference.
- `Fail` raised the outcome of 51, 33 and 46 episodes in the three cells and lowered none. `Slow`
  lowers it (0 raised, 22, 10 and 10 lowered). The three B3 cells are where `Fail`'s gain outweighs
  `Slow`'s loss.
- `Fail` alone raises success in four further cells that B3 could not see because `Slow` cancelled
  or outweighed it: `random p=0.5` at 250,000 (+23 episodes, none lowered), `random p=0.25` at
  100,000 (+50), `random p=0.5` at 60,000 (+6) and `all_components` at 100,000 (+1). In all, seven
  (arm, budget) cells.
- No `Fail` raises success at 20,000,000, where the budget never binds (it lowers one episode of
  `all_components`, `random p=0.25` and `random p=0.5` each, and 61 to 69 of the single-component
  arms, as B3 found).

## 4. Which episodes, and what changed in them

Of the 51 episodes `Fail` raised for `all_components` at 250,000, the failed real components were
the verifier in 47 (32 alone, 6 with the lookup, 5 with the heuristic, 4 with the estimator), the
estimator alone in 3 and the heuristic alone in 1; no episode where only the lookup failed was
raised (the rule never reads it, `POLICIES.md` 2.5). In **every** raised episode of the three cells
(130 of 130) the stop reason changed from `final_declaration` (compute exhausted) to `terminal` (the
rule declared on its own), and the mean decision instant moved from 1.21 to 1.74 s
(`all_components`), 0.97 to 1.49 s and 1.13 to 1.72 s.

## 5. The mechanism, traced in the ledger

Episode: seed 1007, `ComponentTimeout`, `all_components`, compute 250,000. Its directives include
one `Fail` on component 3, the verifier (and a `Slow` on component 6, which is not a real
component and is ignored). Runs: the committed binary (directive applied) and the patched binary
with directives off, both with `trace_sample_rate` 1, same manifest otherwise
(`finding4.py trace`, `finding4.py show`). The public stream's observations arrive at 0.29, 0.76,
0.78, 1.05, 1.32 and 1.82 s, and the last one completes the signature.

| | without the directive | with the verifier `Fail` |
|---|---|---|
| steps | 22 (the last has its charge refused) | 38 |
| rule's declared charge per step | 45 ns at step 0, then 7,642 to 8,290 | 45 ns at step 0, then 1,213 to 1,868 |
| components' declared charge per step (heuristic, estimator, lookup, verifier) | 3,885 ns at step 0, 3,922 at the last full step | 3,885 ns at step 0, the same step for step (4,013 at step 37) |
| verifier output in the ledger | 46 candidates, first-ranked "no fault", every step | `output: none`, every step |
| how it ended | final call at 1.05 s: `Declare {fault: null}` (wrong); `final_declaration` | `Declare ConfigDrift` at site 3 at 1.85 s, right; `terminal` |

What the ledger shows, step by step:

1. A `Fail` charges the component and discards its output (`HARNESS.md` section 5). The verifier's
   own charge (1,819 ns) is paid in both runs. What differs is the **rule's** charge, in the
   scheduling accounting entry.
2. The rule's declared cost carries the decoding of every output it was handed at the previous
   call, `530 ns` per output plus `115 ns` per hypothesis in it (`decide.rs`, `DECODE_OUTPUT_PS`
   and `DECODE_HYPOTHESIS_PS`; `POLICIES.md` section 3). The verifier's output on a window with no
   symptom lists every hypothesis consistent with silence, 46 here, and it is decoded again at
   every step although it does not change: `530 + 115 x 46 = 5,820` ns per step. The step charge
   differs by 6,429 ns between the runs; the 609 ns that remain were not itemised and are
   consistent with the formula's term for the worlds of the larger set.
3. With the verifier alive the rule alone spends about 162,000 of the 250,000 ns over 21 steps and
   the budget is gone at 1.05 s, before the signature is complete. The final call (`HARNESS.md`
   section 1) then acts on what the rule holds: the verifier's stored set from the silent window,
   whose first-ranked hypothesis is "no fault". With the verifier failed the rule pays 1.2 to
   1.9 thousand ns a step, the budget lasts past 1.82 s, and the rule declares the fault itself.
4. The same arithmetic holds for a failed estimator: seed 1135 (estimator `Fail`), the rule's
   step charge falls from 4,400 to 3,295 ns, exactly `530 + 115 x 5` for its five-hypothesis
   output.

**Separating the two candidate mechanisms** (`GORDIAN_DIAG_VERIFIER_FREE_DECODE` and
`GORDIAN_DIAG_VERIFIER_UNUSED`, scratch variants; no directives applied; `ComponentTimeout`, 500
episodes, success %):

| arm | compute (ns) | rule as committed | verifier decoding not charged (cost only) | verifier's set never read, every charge unchanged (priority only) | at 20,000,000 |
|---|---|---|---|---|---|
| `all_components` | 250,000 | 13.8 | **72.2** | 14.0 | 100.0 |
| `random p=0.5` | 250,000 | 70.4 | **100.0** | 70.8 | 100.0 |
| `random p=0.5` | 100,000 | 4.0 | **33.2** | 4.2 | 100.0 |
| `random p=0.25` | 100,000 | 36.2 | **82.2** | 38.0 | 99.6 |
| `random p=0.25` | 60,000 | 5.6 | **31.0** | 7.0 | 99.6 |

Charging nothing for the verifier's decoding reproduces and exceeds the gain; ignoring the
verifier's stored set while paying every charge as before gives 0.2 to 1.8 points. In a
non-directive run with a budget of 600,000 ns, seeds 1007, 1135 and 1323 all succeed (they fail at
250,000). So the budget, through the rule's charge, is the cause. The priority of the verifier's
stale set is real (seed 1135 without the directive: at 1.70 s the heuristic and the estimator hold
the unique right answer, the verifier's call is refused for want of budget, and the rule still
declares "no fault" from the verifier's stale set) but small in aggregate.

## 6. Assessment: defect or legitimate behaviour

- **Not a defect in the decision logic.** The source order and the final call are as documented
  (`POLICIES.md` 2, 2.6), and the verifier is not "wrongly" read: removing its priority does not
  help.
- **Legitimate under the written cost model.** `POLICIES.md` 2.1 says an arm pays "the
  probe-evaluation and decoding terms from what it holds", so an arm that holds a large verifier
  output pays for it. A selector that skips the verifier until symptoms arrive would profit from
  exactly this, which is the kind of thing EXP-001 is meant to detect.
- **But it is an instrument property the documents do not state, with three consequences.**
  (i) It bills the decoding of an output that has not changed since the previous step, every
  step; on a silent window that is the largest output the verifier can emit. (ii) At a binding
  budget this makes ComponentTimeout non-monotone, so its success rate is not a clean measure of
  robustness there, and a "failed component can help" reading is correct, not a bug in the
  directives. (iii) It governs most of the binding-budget results of every arm that runs the
  verifier (next section). I would treat (iii) as the reason to decide it before freezing, not the
  three cells.
- Confidence: the paired comparison, the stop-reason shift and the cost-only variant are measured
  on the committed code and on seeds 1000 to 1499; the attribution to decoding is by ledger
  arithmetic and by the variants. What I did not check is whether the declared decode constants
  are right against real decode time (they were fitted in A6, `POLICIES.md` section 3; A8b's
  counted weights are separate).

## 7. What a fix would change, for every arm

The only fix I tried is not a proposal: a scratch variant that stops charging the verifier's
decoding at all (an upper bound on "decode an unchanged output once"). Pooled success over the 11
classes, B1 grid, directives as committed (`finding4.py full`), rule as committed then variant:

| arm | 20,000,000 | 250,000 | 100,000 | 60,000 |
|---|---|---|---|---|
| `heuristic_only` | 95.4 = | 95.4 = | 94.8 = | 94.4 = |
| `fixed_heuristic_every2`, `every4` | 95.4 = | 95.4 = | 95.3, 95.4 = | 95.0, 95.4 = |
| `fixed_estimator_only` | 95.4 = | 94.8 = | 67.2 = | 26.7 = |
| `all_components` | 96.5, same | 29.1 to **73.0** | 18.2 to 18.6 | 16.2 to 18.2 |
| `random p=0.25` | 96.2, same | 95.0 to 95.9 | 46.0 to **81.4** | 22.8 to **41.5** |
| `random p=0.5` | 96.5, same | 72.2 to **95.3** | 21.4 to **43.1** | 18.1 to 20.3 |
| `fixed_verifier_only` | 95.3, same | 53.9 to **94.3** | 18.5 to **72.3** | 17.9 to **34.7** |

`=` means the arm's success, stop reason, decision instant, compute bill and probes are identical
on every episode (as expected: those arms never run the verifier). On `Ambiguous` at 250,000
`all_components` goes from 13.8 to 73.0.

- A fix of this kind leaves the arms without a verifier exactly as they are (if the full fix also
  stopped re-decoding the heuristic's and estimator's unchanged output, their bills would fall a
  little; that I did not run).
- It moves the verifier-running arms a lot at binding budgets: the collapse of `all_components`
  to 0.16 to 0.29 in B1 is largely this charge, not component compute (still true at 100,000 and
  60,000, where the components' own cost dominates and the variant barely moves it). B4's "headroom
  against the naive pipeline, 0.66 to 0.79" would shrink to about 0.2 at 250,000. B4's headroom
  against the tuned periodic baseline, which has no verifier, would not change.
- At the non-binding level success is unchanged; the modelled cost of verifier arms would fall if
  the counted units (`decoded_outputs`, `decoded_ranked`) stopped counting a repeated decode,
  which changes C, and so S, for any comparison against `all_components` (not measured).
- The `Fail`-raises-success effect disappears (the cost that makes the verifier a liability is
  gone), and `ComponentTimeout` becomes monotone in its directives.

Options for the coordinator, no choice made here: (1) leave the rule, record this note against
finding 4, and compare stressors against the same class without directives rather than against
`Ambiguous`; (2) change the rule so an unchanged output is not decoded and charged again, then
recalibrate and rerun B1 (233 s of wall time for the whole sweep) before freezing. EXP-001 is not
frozen, so (2) needs no new experiment id.

## 8. What I am least sure of

- Everything is one generator and seeds 1000 to 1499, `ComponentTimeout` for the paired runs (the
  all-class table in section 7 is one run of each arm).
- The variants are scratch patches that read environment variables, which the reference core must
  not do; they exist to separate causes and must not be applied to the tree.
- The 609 ns residue in step 2 of section 5 is inferred, not itemised.
- "`Fail` lowers none" is a statement about the three B3 cells. Elsewhere `Fail` does lower success,
  as B3 found (the single-component arms at every level, 61 to 69 episodes at 20,000,000; the
  verifier-only arm at 250,000 in 33 episodes, for the obvious reason that nothing is left to run).
- The three-seed ledger traces (1007, 1135, 1323) are illustrations; the aggregate evidence is the
  paired runs and the variants.

## 9. Reproduction

`experiments/exploration/scripts/finding4.py` (docstring has the exact setup) and
`finding4-diagnostic.patch`. Run directories under `artifacts/runs/` are ignored by version control
and not committed. The B1 runs themselves were regenerated here by `scripts/run-driver.sh`, and
`b4.py` reproduced B4 from them exactly (`b1-variance.md`).
