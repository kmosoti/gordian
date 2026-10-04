# Gordian Agent Contract

The sole instruction file for coding agents working in this repository.

## What Gordian is

A research program in resource-bounded cognition. The normative document is
[`docs/charter.md`](docs/charter.md). Read it before changing anything. When this file and the
charter disagree, the charter wins.

## The rule that matters most

An agent may implement alternatives, build instruments, and analyse results. An agent may **not**:

- change a metric, margin, split, budget, or stopping rule of an experiment whose status is
  `frozen` or later;
- weaken a null hypothesis;
- discard, hide, or relabel failed, timed-out, excluded, or negative runs;
- modify the hidden evaluator or simulator ground truth to make an implementation pass;
- let hidden simulator state reach a policy's inputs;
- report "not significant" as "equivalent."

A result that motivates a redesign motivates a **new** experiment id. The original keeps its pass
condition and its outcome.

## Workflow

```text
register the claim → implement the comparison → verify the evaluator → freeze → execute → report every outcome
```

Each experiment lives in `experiments/EXP-NNN-<slug>/` and starts from
[`experiments/TEMPLATE.md`](experiments/TEMPLATE.md). The status line in that file is
authoritative.

## Code

- Rust is the substrate. `unsafe` is forbidden at the workspace level.
- The reference core must be deterministic: no wall clock, randomness, or I/O inside it. Effects
  enter through explicit arguments and are recorded in the ledger at the boundary.
- Hard resource limits stay enabled in every experimental condition, including baselines.
- Measurements and hypotheses are different ledger entry kinds. Do not merge them.
- Keep a simple reference implementation as an oracle when optimizing anything on the hot path.
- Add a dependency only with a stated requirement, the simpler alternative considered, and its
  cost. The same standard applies to Gordian's own abstractions.

Before pushing:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Language

Say what is built and what is planned, and do not describe planned mechanisms in the present
tense. Do not call a component "attention" or "memory" as if the name established what it does;
the experiment does that. No maturity labels (`v0`, `M1`) for research concepts.

## Git

Work on a branch. Commit messages say what changed and why. Never rewrite history on a branch
someone else may hold. Pull requests are not required for every change but are the review path
for anything touching a frozen experiment.
