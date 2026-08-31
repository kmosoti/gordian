# Experiments

Every `Experiment` node in `knowledge/graph/70-experiments.jsonld` names a `verification[].target`.
Before this tree existed all 24 of those paths were absent from disk, there was no manifest schema,
no runner input format, and no path from a completed run back to the graph — so no experiment in
this repository could be executed or recorded. This tree is that path. It closes G-519.

## Layout

```text
experiments/
  schema/
    experiment-protocol.schema.json
    experiment-run.schema.json
  <experiment-id>/
    protocol.json                  one per experiment; pre-registered
    runs/
      <run-id>/
        run.json                   manifest of one execution
        raw/                       raw artifacts, content-addressed
        analysis/                  generated; reproducible from raw/ by analysis code
```

`<experiment-id>` is the graph node id with its `experiment:` prefix removed — `atom-granularity`,
`snapshot-vs-rebase`, and so on — so a node's target path is derivable from its id and the
`gordian-kg` target-existence check (audit rule S4) is mechanical rather than a lookup table.

Three of the 24 targets deliberately live outside this tree: `benches/foundation` and
`benches/knowledge-graph` are criterion-style benchmark harnesses, and `formal/conformance` is the
Lean/Rust conformance suite. Those three paths are created by the Atoms that own them, not here.

## Registration

A directory holding only a `README.md` is **not pre-registered**: it exists so its graph node's
target resolves, and it names the class and owning issue it will be registered under. Registration
means writing `protocol.json` and validating it against
[`schema/experiment-protocol.schema.json`](schema/experiment-protocol.schema.json).

Every `analysis_plan` MUST carry the five fields the statistical contract requires —
`primary_metric`, `effect_size`, `min_n`, `multiplicity`, `stopping_rule` — with the values fixed
for the experiment's class in
[`../docs/testing/statistical-contract.md`](../docs/testing/statistical-contract.md). There is no
default and no inference from the class: the manifest is what the analysis code reads.

Two cross-field rules JSON Schema cannot express, and which the validator checks after schema
validation:

```text
baseline_condition            MUST be the id of a member of conditions[]
analysis_plan.primary_metric  MUST be the id of a member of metrics[]
```

`scripts/check-experiment-manifests.sh` runs both, plus the digest check below, in CI.

## Runs

`ExperimentRun.protocol_digest` is the digest of the `protocol.json` the run executed under. A run
whose digest does not match the protocol now on disk is a post-hoc run, and saying so is a
mechanical finding rather than an accusation. Nothing in an experiment directory may be edited
after its first run is recorded; a genuine change to the design is a new protocol with a new
digest, and the runs recorded under the superseded digest are reported separately.

Runs with `outcome != completed` are retained. The analysis code applies the pre-registered
`exclusion_rule` and reports how many runs each clause removed; no run is dropped by judgement and
none is dropped after the primary metric has been inspected.

Three fields carry the systematic controls of section 3 of the statistical contract and are
required on every run: `tuning_budget_per_arm`, `metric_tooling`, and `nondeterminism_controls`.
An arm with no tuning budget records an explicit zero. Omission is an error, never an implied zero.

## From a run to the graph

`cargo run -p gordian-experiments -- ingest <experiment-id>` maps completed runs into the knowledge
graph. The rule is stated normatively in `knowledge/ontology.md`; in outline:

```text
for each experiment with at least min_n completed runs per cell:
    create Result node   result:<experiment-id>-<analysis-digest>
        relations: measures -> experiment:<experiment-id>
                   supportedBy | qualifiedBy | challengedBy -> hypothesis:<...>
    create Decision node decision:<experiment-id>-<date>
        relations: decides -> hypothesis:<...>
        status: retain | revise | reject
```

`supportedBy` is emitted only when the observed primary effect exceeds
`analysis_plan.effect_size.minimum_relevant` in the hypothesized direction; when the interval spans
the minimum effect the edge is `qualifiedBy`; otherwise it is `challengedBy`. This is the only
place in the corpus where an evidential edge is created automatically, and it is why the sign and
the threshold are pre-registered rather than chosen once the numbers are in.

## Index

| Experiment id | Class | Issue | Registered |
| --- | --- | ---: | --- |
| `atom-granularity` | agent-trial | #51 | yes |
| `semantic-conflict-prediction` | classification | #52 | yes |
| `isolation-coordination-ablation` | agent-trial | #39 | yes |
| `snapshot-vs-rebase` | agent-trial | #53 | yes |
| `jj-vs-git` | agent-trial | #34 | yes |
| `derived-vs-mutable-state` | agent-trial | #54 | yes |
| `evidence-mutation` | fault-injection | #15, #16 | yes |
| `authorization-engine` | fault-injection | #67 | yes |
| `replay-faults` | fault-injection | #28 | yes |
| `compositional-verifier-inheritance` | classification | #75 | yes |
| `scheduler-benchmarks` | benchmark | #24 | no |
| `mission-ontology` | agent-trial | #50 | no |
| `jj-baseline` | benchmark | #1 | yes |
| `integration-composition` | fault-injection | #32 | no |
| `lease-faults` | fault-injection | #23 | no |
| `frontier-races` | fault-injection | #27 | no |
| `formal-value` | classification | #60 | no |
| `verification-stack` | classification | #6 | no |
| `artifact-store` | benchmark | #14 | no |
| `persistence-benchmarks` | benchmark | #25, #26 | no |
| `distributed-faults` | fault-injection | #42 | no |
| `admission-mutation` | fault-injection | #19 | no |
