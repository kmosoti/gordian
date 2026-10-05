# Gordian

Gordian is a research program in **resource-bounded cognition**: deciding what to compute, what
information to retain, and when further computation is no longer worth its cost.

The question it exists to answer:

> **Can a sparse, persistent, event-driven substrate decide what deserves expensive reasoning,
> what to remember, and which reasoning resource to use, and so achieve better verified decisions
> per unit of total cost than an LLM-centred agent using conventional memory and routing?**

The answer may be no. The repository is built so that it can be.

## Read first

- [`docs/charter.md`](docs/charter.md): the research charter and experimental protocol. It holds
  the question, the four functions the substrate is judged on, the foundations borrowed and their
  limits, the small world, experiments EXP-101 to EXP-106, the baseline registry, the statistical
  procedure, the stress suite, the reproducibility claims, and the build order.
- [`docs/review-log.md`](docs/review-log.md): what was independently verified for each merged
  unit, every decision taken, and why the charter was revised after the first exploration.
- [`docs/local-test-plan.md`](docs/local-test-plan.md): what this machine can settle, the work
  items with their acceptance commands, and resource governance.
- [`experiments/TEMPLATE.md`](experiments/TEMPLATE.md): the preregistration form every experiment
  uses.

## What is built

| Path | Contents |
|---|---|
| `crates/gordian-core` | Deterministic core: explicit clock, budgets, append-only ledger keeping measurements apart from hypotheses, phase-attributed cost bill |
| `crates/gordian-world` | The first small world: software-diagnosis episodes with public rules and hidden instances, and a consistency checker kept with its reference |
| `crates/gordian-eval` | The evaluator: scores a trajectory against hidden truth; the only crate allowed to read it |
| `crates/gordian-components` | Four fixed components with counted-operation cost |
| `crates/gordian-run` | Episode loop, shared decision rule, baseline policies, privileged oracles, interleaved multi-arm runs, recorder and driver |
| `analysis/` | Paired statistics, equivalence and non-inferiority, power, drift and position diagnostics |
| `experiments/exploration/` | Stage B exploration results on the first world |
| `scripts/` | Isolated launcher (`cgroup-run.sh`), run driver, hidden-state guard |

Not built yet: the revised world (resource ladder, simulated reasoner, persistent streams), the
conventional escalation baselines, and the substrate itself. The build order is charter section 12.

## Working on it

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --no-fail-fast --workspace --all-features
bash scripts/check-no-oracle.sh
cd analysis && python -W error -m pytest -q
```

Rules for contributors, human or automated, are in [`AGENTS.md`](AGENTS.md).

Licensed under the Apache License 2.0.
