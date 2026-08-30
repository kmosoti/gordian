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

Every GitHub call in this tree goes through `gh.run_gh`. An unattended agent authenticates by exporting a **classic** personal access token carrying the `repo` and `project` scopes (`G-522`):

```bash
export GH_TOKEN=<classic token with repo, project>
```

Fine-grained tokens do not carry the classic `project` scope that `gh project item-add` and the Project v2 field mutations require, so they will fail with `HTTP 403` no matter how their repository permissions are set. The token is never committed, never written to a report, and never echoed in an error message. `gh auth refresh -s project` is the interactive alternative and is available to a human only.

Mechanical check that the non-interactive path works:

```bash
GH_CONFIG_DIR=$(mktemp -d) GH_TOKEN=<token> gordian-project-sync --dry-run
```

## Temporary GitHub Project projection

GitHub issues and Project 9 are temporary external planning projections while Gordian builds its native Mission Graph substrate. Project status is not canonical Atom satisfaction.

### Membership: `gordian-project-sync`

The local reconciler lists repository issues and Project items once, reports missing or duplicate issue URLs, and adds missing issues idempotently. It owns membership only; it writes no derived field.

```bash
gordian-project-sync --dry-run     # preview
gordian-project-sync               # apply
```

Equivalent module form:

```bash
python -m gordian_orchestration.github_project --dry-run
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

Edges come from `blockedByIssues` node lists, paginated to completion. The `issueDependenciesSummary.blocking` counter is never read: it is wrong for #11, #18, and #44. The `blockedBy` counter is read for exactly one purpose, to assert that pagination retrieved every edge; a short read is an error, not a smaller graph.

```bash
gordian-derive-status ready                     # the ready set, in selection order
gordian-derive-status ready --all               # every open Atom, same order, with its wave
gordian-derive-status ready --json              # the same, machine-readable
gordian-derive-status derive                    # the full projection as JSON
gordian-derive-status derive --compare-board    # the pending board changes, unwritten
gordian-derive-status derive --apply            # write the four fields to Project 9
gordian-derive-status --snapshot artifacts/atoms/issues.json ready   # offline, no token
```

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
