## Initiative
Foundation and Falsification

## Objective
Replace the reported local `jj 0.23.0` development assumption with a supported, pinned Jujutsu baseline whose required behavior is demonstrated by executable fixtures.

## Effect class
external_read

## Research basis
Gordian depends on exact commit/change identity separation, workspaces, sibling/parent/multi-parent topology, first-class conflicts, operation recovery, tags, and revision-scoped verification. Current Jujutsu research targets a release new enough for `jj run`, which was introduced well after 0.23.

Knowledge-graph nodes: `tool:jujutsu`, `source:jj-docs`, `experiment:jj-baseline`.

## Dependencies
None

## Dependency rationale
This Atom is a causal root of the implementation Mission: every other Atom that relies on source-plane behaviour depends on this qualification, and it depends on nothing itself.

## Acceptance
- Document the minimum supported Jujutsu release and why.
- Upgrade/bootstrap the Codex development environment reproducibly.
- Add contract fixtures for change-ID persistence across rewrites, exact commit IDs, isolated workspaces, sibling changes, causal ancestry, multi-parent integration, conflict representation/resolution, operation history/recovery, and tags.
- Confirm `jj run` is available on the pinned release and record its presence; contract tests for `jj run` semantics are owned by #33.
- Make unsupported behavior an explicit adapter constraint rather than an assumption.
- Record the exact Jujutsu binary/version (`jj --version` output) in the contract-fixture manifest produced by this Atom's Verification fixtures and committed to the repository.

## Verification

<!-- BEGIN GENERATED: ATOM ACCEPTANCE VERIFIER -->
verifier_id: `atom-1-acceptance`
<!-- END GENERATED: ATOM ACCEPTANCE VERIFIER -->
Executable CLI contract tests against a disposable fixture repository.

## Benchmark obligation

<!-- BEGIN GENERATED: EO17 OWNERSHIP -->
Owned critical-performance rows: `EO17-JJ-5`.
<!-- END GENERATED: EO17 OWNERSHIP -->
Owned elsewhere: workspace spawn is #30, revision materialization and verification are #33, integration is #32, and status/query latency is #29. This Atom carries no benchmark obligation of its own.

## Closure
Closure is the loop defined by [`docs/implementation/agent-runbook.md`](docs/implementation/agent-runbook.md) sections 1, 2, and 6.6.
The coordinator writes `artifacts/atoms/1/closure.json` after admission in its own bookkeeping change; the record must validate against `artifacts/schema/closure-record.schema.json`.
The required verifier set is the five project integration verifiers below, plus only the Atom-specific verifier IDs declared in this issue's `## Verification` section.

Required verifier logs:
- `verifier:rust-check` — `artifacts/atoms/1/verifiers/rust-check.log`
- `verifier:kg-audit` — `artifacts/atoms/1/verifiers/kg-audit.log`
- `verifier:formal` — `artifacts/atoms/1/verifiers/formal.log`
- `verifier:python` — `artifacts/atoms/1/verifiers/python.log`
- `verifier:spec-consistency` — `artifacts/atoms/1/verifiers/spec-consistency.log`
- `verifier:atom-1-acceptance` — `artifacts/atoms/1/verifiers/atom-1-acceptance.log`
No generic integration command is repeated in this section; verifier execution and Atom-specific commands are defined by the referenced contracts.
