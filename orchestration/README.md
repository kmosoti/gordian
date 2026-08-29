# Python Orchestration

Python is a **thin research and process orchestration layer** around the Rust Gordian substrate.

It is intentionally not a second implementation of Mission Graph semantics.

## Appropriate responsibilities

Python may:

- generate benchmark/experiment datasets;
- launch Rust binaries, Jujutsu commands, verifiers, and agent processes;
- coordinate repeated trials and seeded experiment matrices;
- collect JSON/JSONL/Parquet result artifacts;
- perform statistical/result analysis;
- produce plots/reports from measured data;
- orchestrate external model APIs during experiments;
- compare experimental conditions.

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

Those decisions belong to Rust. Python calls a stable Rust CLI/IPC/library boundary and treats its result as substrate semantics.

## Expected layout

```text
orchestration/
  pyproject.toml
  src/gordian_orchestration/
    __init__.py
    runner.py
  experiments/
    ...
```

The first implementation should remain standard-library-heavy. Add scientific/data dependencies only when an experiment actually requires them.

## Reproducibility

Every experiment runner should record:

```text
repository commit
Mission/experiment identity
Rust binary identity
Jujutsu version
agent/model identities and parameters
random seeds
machine/environment identity
budgets/timeouts
raw event/evidence artifacts
analysis configuration
```

Python output is evidence/analysis data, not canonical Mission Graph state.
