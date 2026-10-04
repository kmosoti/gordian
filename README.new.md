# Gordian

Gordian is a research program in **resource-bounded cognition**: deciding what to compute, what
information to retain, and when further computation is no longer worth its cost.

The question it exists to answer:

> **Can specialized components with private internal representations, coordinated through events
> and selective activation, produce better verified decisions under a fixed resource budget than
> strong simpler alternatives?**

The answer may be no. The repository is built so that it can be.

## Read first

- [`docs/charter.md`](docs/charter.md) — the research charter and experimental protocol: the
  question, the foundations borrowed and their limits, the three kinds of state, the salience
  objective, the small world, seven experiment definitions, the baseline registry, the
  statistical procedure, the stress suite, the reproducibility claims, and the build order.
- [`docs/plan-review.md`](docs/plan-review.md) — the review of the plan that produced the charter,
  and what the charter adds or changes.
- [`experiments/TEMPLATE.md`](experiments/TEMPLATE.md) — the preregistration form every
  experiment uses.

## What is here

| Path | Contents |
|---|---|
| `crates/gordian-core` | Deterministic reference core: explicit clock, resource budgets, append-only event ledger with the measurement/hypothesis distinction |
| `docs/` | Charter and reviews |
| `experiments/` | One directory per preregistered experiment, each starting from the template |

Everything else named in the charter is unbuilt. The build order is charter section 12; the
current stage is **measurement first**: a small world, an evaluator, fixed baselines, and the
explicit-clock core.

## Working on it

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Rules for contributors, human or automated, are in [`AGENTS.md`](AGENTS.md).

Licensed under the Apache License 2.0.
