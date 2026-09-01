## Initiative
Temporary GitHub Bootstrap

## Objective
Populate and continuously reconcile the temporary GitHub Project at `https://github.com/users/kmosoti/projects/9` with every open Gordian issue while GitHub remains the planning substrate.

## Effect class
idempotent_write

## Research basis
Knowledge-graph nodes: `concept:github-project-bootstrap`, `artifact:github-project-sync-script`, `artifact:issue-index`.

## Context
The connected GitHub integration used to create these issues does not expose GitHub Projects mutations. This Atom exists so the Codex/local environment can complete the board wiring through the authenticated GitHub CLI without making the board canonical Gordian state.

## Dependencies
None

## Dependency rationale
None. This Atom operates on the current repository issue corpus, which is an external substrate rather than a prerequisite Atom.

## Acceptance
- Verify `gh` authentication **non-interactively**: `GH_TOKEN=$GORDIAN_GH_TOKEN gh auth status` plus the
  four scope probes of [`agent-runbook.md` section 6.1 *Credentials*](https://github.com/kmosoti/gordian/blob/main/docs/implementation/agent-runbook.md).
  `gh auth login` and `gh auth refresh` are FORBIDDEN — they block on a browser and cannot complete
  unattended. A missing scope exits 78 and is reported, never self-repaired (closes **G-522**).
- Add every repository issue to user project 9 idempotently with `gh project item-add 9 --owner kmosoti --url <issue-url>`.
- Reconcile rather than blindly append: list project items, detect missing/duplicate/archive cases, and emit a machine-readable report.
- Preserve issue URLs/numbers as stable external identities.
- Do not infer Atom satisfaction from GitHub Project status.
- The Wave / Status / Fan In / Fan Out projection is the only readiness computation permitted in Python.
- The projection consumes GitHub's native `blockedBy` node lists, never `issueDependenciesSummary` and never the Markdown `## Dependencies` prose.
- Reconciling newly created Atoms and recomputing all four derived fields after any issue closes is part of acceptance.
- This Atom is retired when #48 is accepted; the projection is deleted and the board becomes a projection of native Mission Graph state.
- Add an optional repository script or workflow for repeated reconciliation, but do not store a personal token in the repository.
- Verify all current Atoms appear in Project 9 and record the resulting item count/evidence.

## Owned bootstrap gaps

The repository parks these gap ids on this Atom in seven separate documents. Until they are
listed here they point at nothing, so they are part of this Atom's contract and each must be
closed or explicitly re-assigned before it can close:

- **G-413** — project-plan.md Atom/Initiative counts must match the live board and issues.
- **G-433** — execution-order.md "Order" lists and the section 4 spine must not contradict the declared graph.
- **G-445** — define where a new Atom is registered; stop using number ranges as the register.
- **G-475** — every row of execution-order section 17's performance suite gets an owning Atom.
- **G-502** — mirror the executable Atom contracts into the repository, not GitHub alone.
- **G-507** — a claim/lease mechanism for bootstrap Atoms (superseded by #23 when it lands).
- **G-518** — one machine-readable dependency source generates the spine; delete the drifting prose orderings.
- **G-522** — no board or issue mutation may require an interactive `gh` re-auth.
- **G-527** — one procedure for creating an Atom or mutating an edge consistently across body, native graph and board.
- **G-609** — `artifacts/project-9-reconciliation.json` must carry commit, timestamp and tool version, or not be committed.

**G-502 ordering constraint.** Do not commit the `artifacts/atoms/` contract snapshot until the
GitHub change plan is applied. Snapshotting first makes `scripts/check-selfhosting-closure.sh`
compute a closure of 45 with 10 orphans instead of 43 with none, which fails
`verifier:spec-consistency` for **every** Atom, not just this one.

## Closure

```bash
bash scripts/check-selfhosting-closure.sh                 # exit:0 artifact:artifacts/atoms/70/selfhosting-closure.txt
PYTHONPATH=orchestration/src python3 -m gordian_orchestration.github_project --dry-run \
                                                          # exit:0 artifact:artifacts/atoms/70/project-reconciliation.json
for s in scripts/check-*.sh; do bash "$s" || exit 1; done # exit:0 artifact:artifacts/atoms/70/spec-consistency.txt
```

## Retirement

This Atom is retired when #48 is accepted: the projection is deleted and the board becomes a
projection of native Mission Graph state. Retirement requires a closure record like any other
Atom — closing the issue is not retirement.

## Verification
Dry-run output plus a post-run comparison between open repository issues and Project 9 item URLs.

## Performance
Batch/list once where possible; avoid an avoidable query-per-item synchronization loop, while retaining clear failure reporting and idempotence.

## Closure
Closure is the loop of [`docs/implementation/agent-runbook.md`](docs/implementation/agent-runbook.md) sections 1 and 2. This Atom is closed only when the coordinator has written `artifacts/atoms/70/closure.json` after admission, in its own bookkeeping change, and that record validates against `artifacts/schema/closure-record.schema.json`. The record MUST carry exactly these commands with the recorded exit code and artifact path:

```bash
cargo fmt --all -- --check                                        # exit:0 artifact:artifacts/atoms/70/cargo-fmt-all-check.txt
cargo clippy --all-targets --all-features -- -D warnings          # exit:0 artifact:artifacts/atoms/70/cargo-clippy-all-targets-all-features-d-warn.txt
python -m gordian_orchestration.github_project reconcile --check  # exit:0 artifact:artifacts/atoms/70/python-m-gordian-orchestration-github-projec.txt
```

Closing this issue without a validating closure record is not closure (`scripts/check-closure-records.sh`).


