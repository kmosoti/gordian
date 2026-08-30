# Admission Safety Mutation Suite

Graph node: `experiment:admission-mutation`
Intended class: `fault-injection` (see [`../../docs/testing/statistical-contract.md`](../../docs/testing/statistical-contract.md) section 2)
Owning issue: #19

Remove or negate conflict, freshness, verification, reconciliation, and authorization checks and require the executable test suite to detect every safety regression.

## Status

**Not pre-registered.** This directory exists so the graph node's `verification[].target` resolves
on disk and so `gordian-kg audit --strict` rule S4 can pass; it does not yet contain a protocol.

Registration means adding `protocol.json` here, validating against
[`../schema/experiment-protocol.schema.json`](../schema/experiment-protocol.schema.json), and
filling `analysis_plan` with the five required fields from the class row of the statistical
contract. Nothing in this directory may be written after the first run is recorded: the run's
`protocol_digest` is what makes the pre-registration checkable, and a protocol edited after a run
is a detectable post-hoc change, not a correction.
