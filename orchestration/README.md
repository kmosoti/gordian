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

## Layout

```text
orchestration/
  pyproject.toml
  src/gordian_orchestration/
    __init__.py
    runner.py
    github_project.py
  tests/
```

The implementation should remain standard-library-heavy. Add scientific or data dependencies only when an experiment actually requires them.

## Local installation

```bash
python3.14 -m venv .venv
source .venv/bin/activate
python -m pip install -e ./orchestration
```

## Temporary GitHub Project projection

GitHub issues and Project 9 are temporary external planning projections while Gordian builds its native Mission Graph substrate. Project status is not canonical Atom satisfaction.

The local reconciler lists repository issues and Project items once, reports missing or duplicate issue URLs, and adds missing issues idempotently.

Grant the GitHub CLI the required project scope:

```bash
gh auth status
gh auth refresh -s project
```

Preview:

```bash
gordian-project-sync --dry-run
```

Apply:

```bash
gordian-project-sync
```

Equivalent module form:

```bash
python -m gordian_orchestration.github_project --dry-run
python -m gordian_orchestration.github_project
```

The command emits a machine-readable JSON report. It never interprets issue closure or Project status as Mission Graph evidence.

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

Python output is evidence or analysis data, not canonical Mission Graph state.

## Checks

```bash
ruff check orchestration
python -m compileall -q orchestration/src
python -m unittest discover -s orchestration/tests
```
