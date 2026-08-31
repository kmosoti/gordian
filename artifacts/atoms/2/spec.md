## Initiative
Foundation and Falsification

## Objective
Make every development layer reproducible and independently verifiable before runtime implementation expands.

## Effect class
idempotent_write

## Research basis
Knowledge-graph nodes: `tool:rust`, `tool:lean4`, `artifact:runtime`.

## Dependencies
None

## Dependency rationale
#1 is deliberately not a blocker (G-417). The Rust, Lean, Python, and CI foundations are qualified against the toolchain files pinned in this repository; the Jujutsu baseline of #1 is consumed by #29 and #33, not by CI setup, so a soft edge here would serialize the whole backlog behind a Jujutsu upgrade.

## Acceptance
- Rust toolchain is pinned and `fmt`, `clippy -D warnings`, tests, graph validation, and graph audit pass.
- Lean builds only from `formal/`; independent checking rejects `sorry`; axiom audit passes.
- Python orchestration package is independently lintable/testable and contains no duplicate substrate semantics.
- CI exposes Rust, formal, Python, and later benchmark checks as separate failures.
- Developer commands are documented in `AGENTS.md` and README.

## Verification

<!-- BEGIN GENERATED: ATOM ACCEPTANCE VERIFIER -->
verifier_id: `atom-2-acceptance`
<!-- END GENERATED: ATOM ACCEPTANCE VERIFIER -->
A clean checkout passes all declared CI jobs with no setup-only failures.

## Closure
Closure is the loop defined by [`docs/implementation/agent-runbook.md`](docs/implementation/agent-runbook.md) sections 1, 2, and 6.6.
The coordinator writes `artifacts/atoms/2/closure.json` after admission in its own bookkeeping change; the record must validate against `artifacts/schema/closure-record.schema.json`.
The required verifier set is the five project integration verifiers below, plus only the Atom-specific verifier IDs declared in this issue's `## Verification` section.

Required verifier logs:
- `verifier:rust-check` — `artifacts/atoms/2/verifiers/rust-check.log`
- `verifier:kg-audit` — `artifacts/atoms/2/verifiers/kg-audit.log`
- `verifier:formal` — `artifacts/atoms/2/verifiers/formal.log`
- `verifier:python` — `artifacts/atoms/2/verifiers/python.log`
- `verifier:spec-consistency` — `artifacts/atoms/2/verifiers/spec-consistency.log`
- `verifier:atom-2-acceptance` — `artifacts/atoms/2/verifiers/atom-2-acceptance.log`
No generic integration command is repeated in this section; verifier execution and Atom-specific commands are defined by the referenced contracts.
