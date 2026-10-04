# EXP-NNN — <one-line question>

Status: `exploration` | `frozen` | `executed` | `reported`

A confirmatory run may begin only when this file is `frozen` and its freeze commit is recorded
below. Edits after freezing create a new experiment id.

## Bounded hypothesis

- **H1:**
- **H0:**
- Margins (and why these values, from which exploration runs):

## Falsification and architecture-revision rule

What result deletes, conditions, or replaces which part of the design?

## Arms

| Arm | Role (treatment / baseline / control / alternative) | What it challenges |
|---|---|---|

## Workloads and sampling

- Environments, generators, held-out split (whole environments and trajectories, never adjacent events):
- Seeds:
- Budgets per arm (compute, memory, probes, time) and how they are matched:
- Stopping rule:

## Metrics

- **Primary (exactly one):**
- Secondary (exploratory, multiplicity-adjusted if used for any claim):
- Critical-miss criterion and bound:
- Coverage reporting:

## Cost accounting

- Resources counted and their declared utility exchange rate or hard-constraint treatment:
- Counterfactual fork rate and where its cost is charged:

## Analysis plan

- Test, equivalence/non-inferiority bounds, interval method:
- Power analysis against the margin (sample size, assumed variance, source of the estimate):
- Result categories: beneficial / harmful / practically equivalent within margin / unresolved

## Identity

- Source revision, toolchain, lockfile hash:
- Environment and hardware capabilities:
- Model hashes, scheduler policy, resource limits:

## Artifacts

- Run ledger destination:
- Raw artifact destination:
- Retention: all runs including failed, timed-out, excluded, and negative, with reasons.

## Threats to validity

## Freeze record

- Freeze commit:
- Frozen by:
- Date:

## Outcome

Category, interval, per-task behaviour, and every excluded run with its reason.
