# Orchestration log

## Attempt #70-design-1 — 2026-08-31
- tier: deep; tokens_est: 364k; wall: 30 min; retries: 1
- late failures: accepted snapshot rejection; unreclaimable stale lease; false add-edge compensation
- hallucinated_reference: none
- tier_mismatch: deep review continued after the P1 set stabilized
- dropped duplicate implementation: retained the accepted-main G-502 snapshot/selector and G-609 projection behavior and layered only the #70 migration and recovery gaps
- actionable rule: once a P1 repair is behaviorally stable, switch to targeted evidence collection and stop redesign review before it creates duplicate implementations

## Attempt #70-snapshot-packet-1 — 2026-08-31
tokens_est: coordination ~6k / deep 0 / write ~195k   wall: ~7 min   retries: 1
verifier_failures_caught_late: 1 — the accepted 305-edge snapshot contradicted five normalized dependency mirrors
hallucinated_reference: none
tier_mismatch: write was dispatched before accepted graph and normalized-mirror compatibility was proven
rule: Prove the accepted graph and every generated mirror agree before delegating any snapshot-format rewrite.

## Atom #70
- tier used / tier that should have been used: coordination for decisions, write workers for bounded edits, and one iverify singleton; this allocation was appropriate, but packet prompts should have been narrower.
- reasoning spent that did not change the outcome: the snapshot rework worker repeatedly re-read the same diff and spent far beyond the stable 305-edge decision; the coordinator also inspected a failed CLI invocation whose only defect was global-option placement.
- hallucinated API, file, flag, or command, and what would have caught it earlier: the first snapshot packet treated the rejected 310-edge artifact as authority, the coordinator initially transcribed an unobserved base exact-state id, and the manifest retained the rejected plan digest; a pre-dispatch live native-edge count plus machine-computed base and plan digests would have caught all three.
- one rule to apply next time: before dispatch, freeze and print the authoritative graph count, exact base id, and manifest plan digest; stop any write worker once those facts conflict with its premise.

## Attempt #2-design-1 — 2026-08-31
tokens_est: coordination ~4k / deep ~231k / write ~35k   wall: ~35 min   retries: 1
verifier_failures_caught_late: 3 — foreign-namespace axiom bypass, caller-forged evidence subject, and missing `ruff` in the worker PATH
hallucinated_reference: `jj diff --check` (unsupported flag; exit 2)
tier_mismatch: deep review continued after the scoped P1 findings had stabilized
rule: Verify the exact tool interface and refresh compiled test artifacts before trusting a negative test.
