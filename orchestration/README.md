# Python Orchestration

Python is a **thin research and process orchestration layer** around the Rust Gordian substrate.

It is intentionally not a second implementation of Mission Graph semantics.

## Appropriate responsibilities

Python may:

- generate benchmark and experiment datasets;
- launch Rust binaries, Jujutsu commands, verifiers, and agent processes;
- coordinate repeated trials and seeded experiment matrices;
- collect JSON, JSONL, or Parquet result artifacts;
- perform statistical and result analysis;
- produce plots and reports from measured data;
- orchestrate external model APIs during experiments;
- compare experimental conditions;
- reconcile temporary external planning projections such as GitHub Project 9.

## Forbidden duplication

Python must not independently decide:

```text
Atom readiness
hard dependency validity
evidence compatibility
candidate admission
capability authorization
lease correctness
accepted-frontier transitions
Mission satisfaction
```

Those decisions belong to Rust. Python calls a stable Rust CLI, IPC, or library boundary and treats its result as substrate semantics.

### The one bootstrap exception, and its expiry

`derive_status.py` computes Wave, Fan In, Fan Out, and a `Ready`/`Blocked` label for GitHub issues. That is not an exception to the rule above, because it decides nothing: every value is a **projection of GitHub's own `blockedBy` edges**, and the readiness it prints is the bootstrap rule stated in `docs/implementation/issue-index.md`, not `Ready` as defined in `docs/spec/mission-graph.md`. The module exists only because the bootstrap Mission has no native dependency store yet, and it is **deleted when #48 lands** (`G-504`). Nothing downstream of #48 may read it.

## Layout

```text
orchestration/
  pyproject.toml
  src/gordian_orchestration/
    __init__.py
    runner.py            external process execution
    gh.py                the single GitHub CLI entry point
    provenance.py        source and environment identity for every report
    normalization_journal.py  manifest-bound live Atom contract repair journal
    github_project.py    Project 9 membership reconciliation
    derive_status.py     Project 9 derived-field projection (deleted at #48)
  tests/
    fixture_graph.py     frozen blocked-by graph and its recorded depths
    test_derive_status.py
    test_gh.py
    test_github_project.py
    test_provenance.py
    test_runner.py
```

The implementation should remain standard-library-heavy. Add scientific or data dependencies only when an experiment actually requires them.

## Local installation

The interpreter is pinned to 3.14 by `.python-version` at the repository root and by `requires-python = ">=3.14"` here; the checked-in toolchain versions and their install commands live in `docs/implementation/toolchain.md`, and `scripts/check-toolchain.sh` verifies them. One bootstrap string is used everywhere in this repository:

```bash
python3.14 -m pip install -e './orchestration[dev]'
```

That extra pins `ruff==0.16.5`, which is the only pinned lint version in the repository. `pyproject.toml` is the single place the version is written; nothing else may spell `ruff==`.

If you prefer a virtual environment, create it first and then run the identical bootstrap string:

```bash
python3.14 -m venv .venv
source .venv/bin/activate
python3.14 -m pip install -e './orchestration[dev]'
```

`.venv/` and `.ruff_cache/` are ignored by the repository `.gitignore`, so a bootstrap leaves `jj status` unchanged. If it does not, the ignore list has drifted; fix `.gitignore` rather than working around it.

## Authenticating without a human

Every GitHub-backed entry point runs the same fail-closed preflight. An unattended agent must
provide the non-empty `GORDIAN_GH_TOKEN` environment variable:

```bash
export GORDIAN_GH_TOKEN="$CI_GH_TOKEN"
gordian-bootstrap preflight
```

The value is copied to `GH_TOKEN` for every `gh` subprocess, overriding any ambient `GH_TOKEN`, and
is never committed, written to a report, or printed. An installed `gh` credential-store login (or
any other ambient token variable) is not a fallback. Preflight checks the authenticated identity,
repository write permission, and Project 9 read/write API access using actual GitHub responses.
Unattended commands never invoke an interactive authentication flow; repair credentials outside the
unattended run.

Mechanical check that the non-interactive path works:

```bash
gordian-project-sync reconcile --check
```

This is read-only and does not add Project items. Mutating bootstrap commands run the same
preflight themselves and exit `78` when `GORDIAN_GH_TOKEN` is absent or lacks one of the required
capabilities.

## Temporary GitHub Project projection

GitHub issues and Project 9 are temporary external planning projections while Gordian builds its native Mission Graph substrate. Project status is not canonical Atom satisfaction.

### Membership: `gordian-project-sync`

The local reconciler lists repository issues and all Project items once, reports missing or duplicate issue URLs, and idempotently unarchives archive-only open issues or adds truly missing issues. It owns membership only; it writes no derived field.

```bash
gordian-project-sync reconcile --check  # read-only convergence gate
gordian-project-sync reconcile          # add missing items and verify
```

Equivalent module form:

```bash
python -m gordian_orchestration.github_project reconcile --check
```

The command emits a machine-readable JSON report carrying `generated_at`, `source_change_id`, `source_commit_id`, and `tool_versions`, so a report is never mistaken for a fresh one. It never interprets issue closure or Project status as Mission Graph evidence.

### Derived fields: `gordian-derive-status`

`gordian-derive-status` reads GitHub's native blocked-by graph and computes the four derived board fields. The native graph is authoritative for dependencies; `Wave`, `Fan In`, `Fan Out`, and `Status` are projections of it and are never hand-edited.

| Field | Definition |
| --- | --- |
| `Wave` | Longest-path depth in the blocked-by DAG: `0` with no blockers, otherwise `1 + max(Wave of blockers)`. Longest, not shortest — an Atom is not reachable earlier than its slowest prerequisite. |
| `Fan In` | In-degree: the number of direct blockers. |
| `Fan Out` | Out-degree: the number of issues this one directly blocks, counted over the edge lists. |
| `Status` | `Ready` iff the issue is open and every blocker is satisfied; `Blocked` otherwise. |

`In Progress`, `In Review`, and `Accepted` are set by a human against a claim, a pull request, and a closure record. The command never derives them and never writes over them.

Edges come from the native `blockedBy` connection, paginated to completion. Its `totalCount` proves that every edge was retrieved; a short read is an error, not a smaller graph. `issueDependenciesSummary` is never read because its blocking counter has been observed wrong for #11, #18, and #44.

A mistakenly created issue is removed from the executable Atom corpus only by closing it and applying GitHub's `duplicate` label. An open duplicate is a reconciliation error, and an ordinary closed Atom without a validating closure record remains a hard readiness error.

### Registry drift: `gordian-atom-registry`

The registry auditor closes the gap between the live native graph and its human-readable
mirrors. It checks every issue body's `## Dependencies` section, Initiative milestone, `type:*`
label, the Atom tables in `project-plan.md`, and the generated maximum-length spine in
`execution-order.md` against the native `blockedBy` connections:

```bash
gordian-atom-registry check
gordian-atom-registry check-drift
gordian-atom-registry check --json
gordian-atom-registry render-plan --write
gordian-atom-registry render-spine --write
```

No mirror becomes authoritative: disagreement fails and reports the exact drift. The registry is
coherent only when the core, EO17 benchmark, and target-crate audits all pass. Once those three
audits pass, #70 may capture the complete issue bodies, metadata, and native edges for offline
verification. Capture and rendering are deterministic for a given registry and repository inputs:

```bash
gordian-atom-registry capture --output artifacts/atoms/issues.json
gordian-atom-registry --snapshot artifacts/atoms/issues.json check
gordian-atom-registry --snapshot artifacts/atoms/issues.json render-plan
gordian-atom-registry --snapshot artifacts/atoms/issues.json render-spine
```

Capture refuses to write a drifting registry. Benchmark and target-crate synchronization may run
in either order: the first successful staged apply may report `coherent:false` and
`snapshot_skipped`, while preserving the prior snapshot; the second successful apply, or an
explicit capture, writes the coherent snapshot. `scripts/check-atom-registry.sh` reruns the core
audit offline after the snapshot exists.

### Benchmark obligations

The benchmark audit checks every EO17 row, owner, owner issue body, native transitive closure, and
first-qualification owner reachability. It is read-only and may run against live GitHub state or a
snapshot:

```bash
gordian-atom-registry check-benchmarks
gordian-atom-registry --snapshot artifacts/atoms/issues.json check-benchmarks
```

Target-crate contracts are audited and synchronized separately; the normative
`crate-map.md` ownership table is reversed to Atom → one-or-more Rust targets and
the project-plan target cells must match it exactly. An Atom may own one or
multiple targets, or explicitly own `none`. The check is read-only; the sync
without `--apply` prints a plan. Applying requires live GitHub state, preflight,
and a live #70 claim:

```bash
gordian-atom-registry check-target-crates
gordian-atom-registry --snapshot artifacts/atoms/issues.json check-target-crates
gordian-atom-registry sync-target-crates
gordian-atom-registry sync-target-crates --apply
```

The audit also checks every literal `crates/gordian-*` path and `cargo ... -p
gordian-*` package in an issue body. The only cross-owner references currently
allowed are explicit shared test/conformance references for Atoms #7 and #34;
their crate names must still be real rows in `crate-map.md`. Unknown packages
and paths are always errors, never aliases.

`sync-benchmarks` renders deterministic issue-body join keys. Without `--apply` it prints a plan
and does not mutate anything. Applying it requires live GitHub state, a successful preflight, and
a live claim held by the current actor for Atom #70:

```bash
gordian-atom-registry sync-benchmarks
gordian-atom-registry sync-benchmarks --apply
```

### Creating and connecting Atoms

### Bounded live contract normalization

`gordian-atom-registry normalize` reads the committed
[`atom-contract-normalization.json`](../docs/implementation/atom-contract-normalization.json)
manifest and emits a deterministic plan by default. `--apply` and `--recover` are live,
claim-gated operations that persist an atomically-written journal before any issue or edge
write. A fresh body read must match the journal's exact UTF-8 SHA-256 precondition before a
PATCH, and a fresh read must verify the proposed body afterward. Native dependency
entries and manifest-declared labels are additive intents and are never removed during
recovery or compensation. Label additions carry exact issue title/state preconditions and
verify labels through a single-issue read, including issues filtered from the registry after
becoming duplicates. Body
compensation restores an old body only when a fresh read still matches this journal's proposed
digest; an external edit is recorded as a conflict and is not overwritten.

```bash
gordian-atom-registry normalize
gordian-atom-registry normalize --apply --journal artifacts/atoms/normalization-journal.json
gordian-atom-registry normalize --recover --journal artifacts/atoms/normalization-journal.json
```

Snapshot capture is a separate final step after all live registry audits pass; a snapshot is
never used as an input to an apply or recovery operation.

`add-edge` plans one native `blockedBy` edge and all of its deterministic projections. `new-atom`
plans (or creates) a fully registered Atom, including its complete issue body, milestone,
`type:atom` or `type:experiment` label, one target crate (or `none`), execution phase, prerequisite
blockers, downstream `--blocks` links, and one JSON-LD knowledge node. Both commands require an
already coherent registry, never persist a drifting snapshot, and are dry-run by default;
`--apply` requires live GitHub state, a successful preflight, and a live #70 claim.

```bash
gordian-atom-registry add-edge ISSUE BLOCKER
gordian-atom-registry add-edge ISSUE BLOCKER --apply

gordian-atom-registry new-atom \
  --title "[Temporary GitHub Bootstrap] Atom title" \
  --body-file /path/to/complete-atom-body.md \
  --milestone "Initiative name" \
  --type-label type:atom \
  --target-crate gordian-core \
  --phase 13 \
  --blocked-by BLOCKER \
  --blocks DOWNSTREAM \
  --knowledge-node /path/to/knowledge-node.json
gordian-atom-registry new-atom ... --apply
```

`--blocked-by` and `--blocks` may be repeated. Use `--target-crate none` when no Rust crate owns
the Atom. Existing plan rows may name multiple crate owners, but `new-atom` accepts exactly one
target-crate value. The body file and knowledge-node JSON are required even for a dry plan; dry plans never
mutate GitHub or repository projections.

### Milestone contracts

`gordian-milestone-contracts check` is read-only: it returns `0` when clean, `1` for contract drift,
or `2` for an operational error. Configuration preflight for `sync --apply` returns `78` when
authentication is unavailable. The exact `Acceptance: ...`
line is derived from the generated Initiative register, so milestone descriptions do not define a
separate completion rule. `sync` is a deterministic dry plan; `sync --apply` requires noninteractive
preflight and the current actor's live #70 claim, and compensates partial milestone-description
writes if a later update or verification fails.

```bash
gordian-milestone-contracts check
gordian-milestone-contracts sync
gordian-milestone-contracts sync --apply
```

### Credentials and claims: `gordian-bootstrap`

The bootstrap loop never opens an interactive authentication flow. Its preflight requires
`GORDIAN_GH_TOKEN`, copies it to `GH_TOKEN` for each `gh` subprocess, and verifies the authenticated
login, repository write permission, and Project 9 read/write API access:

```bash
gordian-bootstrap preflight
```

Missing or insufficient configuration exits `78`; the command never calls `gh auth login` or
`gh auth refresh`. Repair credentials interactively outside an unattended run.
Claims use the sanctioned Ready ordering and first append an empty commit to the
`refs/heads/gordian-claim-log` ref through a create-or-fast-forward-only (`force=false`) CAS.
The commit message is strict canonical `gordian-bootstrap-claim-event-v1` JSON containing the
event and complete active state, capped at three leases. GitHub's commit `committer.date` is the
lease start and the response `Date` header is the expiry clock; missing headers fail closed. The
assignee, Project Status `In Progress`, and structured comment are projections written only after
the CAS succeeds. A losing race cannot write projections, and projection failure/closed-issue
recovery appends a CAS `abort` event. Historical comments, assignees, and Project status do not
arbitrate claims or readiness. Readers validate every event back to the accepted-main root,
including one-parent/invariant-tree/one-step-transition rules and the requirement that each active
`claim_commit` is an actual claim event on that ancestry. A historical root is accepted only when
GitHub compare reports the expected `status`, commit counts, `base_commit`, and `merge_base_commit`.
Same-actor retries are idempotent only for live leases; stale leases require an explicit release.

```bash
gordian-bootstrap claim          # first unclaimed Ready Atom
gordian-bootstrap claim 2        # succeeds only if #2 is that Atom (or already held by this actor)
gordian-bootstrap claims         # actor and live/stale expiry for every assigned Atom
gordian-bootstrap release 2 --reason "verifier failed twice"
```

`release` requires the current canonical lease owned by the current actor, appends its CAS release
before touching projections, and then reconciles the assignee and derived `Ready`/`Blocked`
board value. A closed issue can still be canonically released; projection writes are skipped. Every
release projection mutation is bracketed by canonical rereads so a later winner—even one sharing
the login—keeps its assignee and `In Progress` status. `claims` reads the log and reports liveness
and projection drift; it does not infer an owner from comments or assignees. A lost response is
resolved by rereading the validated ancestry (including events followed by later appends), never
by guessing from a client timestamp.

```bash
gordian-derive-status ready                     # the ready set, in selection order
gordian-derive-status ready --all               # every open Atom, same order, with its wave
gordian-derive-status ready --json              # the same, machine-readable
gordian-derive-status derive                    # the full projection as JSON
gordian-derive-status derive --compare-board    # the pending board changes, unwritten
gordian-derive-status derive --apply            # write the four fields to Project 9
gordian-derive-status --snapshot artifacts/atoms/issues.json ready --inspection
                                                             # offline inspection; never dispatch
```

Snapshot readiness without `--inspection` fails closed. Inspection output carries
`dispatchable:false` and must never feed `claim`, selection, or a Project mutation; dispatch always
re-reads live GitHub state and accepted-revision closure evidence.

`--apply` is idempotent: it writes only where the board's stored value differs from the derived one, so a second run reports `"applied": 0` and an empty `changes` list. Issues absent from the board are reported under `absent_from_board` rather than created, and `--apply` or `--compare-board` then exits `1`: an Atom the board does not carry has no derived fields at all, and silence there would read as "everything is projected". `gordian-project-sync` adds the missing items.

### Picking the next Atom

`gordian-derive-status ready` is the only sanctioned way to choose the next Atom. During bootstrap an Atom counts as satisfied for readiness **iff**:

1. its issue is closed, **and**
2. `artifacts/atoms/<N>/closure.json` exists at the commit under evaluation, **and**
3. that record validates against `artifacts/schema/closure-record.schema.json`.

Rows are printed in the total selection order that [`../docs/implementation/execution-order.md`](../docs/implementation/execution-order.md) section 5 states once (`G-530`): lowest `Wave`, then highest `Fan Out`, then lowest issue number. The runbook's selection step is to take the first unclaimed row of that output, so the printed order is the order to work in; `--json` repeats the keys under `selection_keys`. `derive` keeps ascending issue order, because it feeds the board writer rather than an agent.

Closing an issue is bookkeeping, not evidence (`G-516`). This is the bootstrap analogue of Satisfied-as-admitted, and `ready` exits non-zero listing every issue closed without a validating closure record, because a ready set resting on an unevidenced closure is worse than no ready set. `--satisfaction closed` relaxes the rule to GitHub's own weaker one and exists for inspecting the raw graph, never for picking work.

## Experiment execution policy

Authorized providers and model ids, the environment-variable names carrying their credentials, the per-run and per-experiment cost and wall-clock caps, and the rule for a metric that requires a human are stated once, in the **Experiment execution policy** section of `docs/implementation/agent-runbook.md`. Any runner in this tree cites that section rather than restating it.

## Reproducibility

Every experiment runner should record:

```text
repository commit
Mission and experiment identity
Rust binary identity
Jujutsu version
agent and model identities and parameters
random seeds
machine and environment identity
budgets and timeouts
raw event and evidence artifacts
analysis configuration
```

`provenance.collect()` supplies the source and environment part of that list — `generated_at`, `source_change_id`, `source_commit_id`, and `tool_versions` — and every probe degrades to the literal string `unknown` rather than omitting a key, so a report is always self-describing about what it could not identify.

Python output is evidence or analysis data, not canonical Mission Graph state.

## Checks

```bash
ruff check orchestration
python3.14 -m compileall -q orchestration/src
python3.14 -m unittest discover -s orchestration/tests
```

`ruff format` is deliberately not used: the lint configuration in `pyproject.toml` selects rules, not a formatter, and no check in this repository invokes `ruff format --check`.

The unit tests never touch the network. Graph math runs against `tests/fixture_graph.py`, a frozen capture of the native blocked-by graph and the longest-path depth of every node; the `gh` wrappers run against a mocked `subprocess`. A test that needed a token would not run in CI, and a projection nobody can falsify offline is not one worth trusting.
