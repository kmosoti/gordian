## Initiative
Foundation and Falsification

## Objective
Create seed-reproducible synthetic and repository-derived workload generators before Gordian optimizes graph or scheduling algorithms.

## Effect class
hermetic

## Research basis
Knowledge-graph nodes: `foundation:dag`, `experiment:foundation-benchmarks`, `foundation:benchmarking`.

## Dependencies
- #2

## Required dimensions
- node/edge count and density
- DAG width/depth and fan-in/fan-out
- critical-path ratio
- resource contention
- heterogeneous worker capability/cost/duration distributions
- semantic-claim overlap
- event/evidence history volume

## Workload distributions
Every generator emits one of these named distribution ids. The ids are the vocabulary #6, #18, #24, #59 and #69 cite; a workload that names no id is not runnable (G-630).

- rep-small — representative: 50-200 nodes, DAG width 4-12, critical-path ratio 0.25-0.40, semantic-claim overlap 0.05-0.15.
- rep-wide — representative: 500-2000 nodes, DAG width 40-120, critical-path ratio 0.05-0.15, semantic-claim overlap 0.05-0.20.
- adv-deep — adversarial: 200-800 nodes, DAG width 2-4, critical-path ratio 0.60-0.90, semantic-claim overlap 0.00-0.10.
- adv-contended — adversarial: 300-1000 nodes, DAG width 20-60, critical-path ratio 0.15-0.30, semantic-claim overlap 0.55-0.85.

## Acceptance
- Every generated workload records seed and generator parameters.
- The four named distributions of `## Workload distributions` (rep-small, rep-wide, adv-deep, adv-contended) are all generatable, and each generated workload records which id it came from.
- Workloads can be reused by graph, scheduler, evidence, persistence, and agent experiments.
- Dataset format is stable enough for Rust benchmarks and thin Python experiment orchestration.

## Verification

<!-- BEGIN GENERATED: ATOM ACCEPTANCE VERIFIER -->
verifier_id: `atom-3-acceptance`
<!-- END GENERATED: ATOM ACCEPTANCE VERIFIER -->
Golden seeded fixtures plus property checks for requested graph/workload characteristics.

## Closure
Closure is the loop defined by [`docs/implementation/agent-runbook.md`](docs/implementation/agent-runbook.md) sections 1, 2, and 6.6.
The coordinator writes `artifacts/atoms/3/closure.json` after admission in its own bookkeeping change; the record must validate against `artifacts/schema/closure-record.schema.json`.
The required verifier set is the five project integration verifiers below, plus only the Atom-specific verifier IDs declared in this issue's `## Verification` section.

Required verifier logs:
- `verifier:rust-check` — `artifacts/atoms/3/verifiers/rust-check.log`
- `verifier:kg-audit` — `artifacts/atoms/3/verifiers/kg-audit.log`
- `verifier:formal` — `artifacts/atoms/3/verifiers/formal.log`
- `verifier:python` — `artifacts/atoms/3/verifiers/python.log`
- `verifier:spec-consistency` — `artifacts/atoms/3/verifiers/spec-consistency.log`
- `verifier:atom-3-acceptance` — `artifacts/atoms/3/verifiers/atom-3-acceptance.log`
No generic integration command is repeated in this section; verifier execution and Atom-specific commands are defined by the referenced contracts.
