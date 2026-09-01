# Gordian Agent Contract

This is the repository's sole coding-agent instruction file.

## Mission

Build Gordian as a research-driven coordination substrate for human and autonomous software engineering. The system must keep intent, source state, execution, evidence, provenance, and authority distinct, then reconcile them through explicit contracts rather than mutable project-status folklore.

## Required reading

Before substantial implementation, read the documents relevant to the Atom, beginning with:

1. [`docs/implementation/agent-runbook.md`](docs/implementation/agent-runbook.md) — the loop you execute, start to finish
2. [`README.md`](README.md)
3. [`docs/architecture.md`](docs/architecture.md)
4. [`docs/spec/mission-graph.md`](docs/spec/mission-graph.md)
5. [`docs/spec/invariants.md`](docs/spec/invariants.md)
6. [`docs/implementation/execution-order.md`](docs/implementation/execution-order.md)
7. [`docs/implementation/crate-map.md`](docs/implementation/crate-map.md) — before writing any Rust
8. [`knowledge/ontology.md`](knowledge/ontology.md)
9. [`knowledge/acquisition.md`](knowledge/acquisition.md)
10. [`docs/research/verification-strategy.md`](docs/research/verification-strategy.md)
11. [`docs/formal/proof-boundary.md`](docs/formal/proof-boundary.md)
12. [`docs/protocols/source-adapter-contract.md`](docs/protocols/source-adapter-contract.md)
13. [`docs/protocols/jujutsu-agent-protocol.md`](docs/protocols/jujutsu-agent-protocol.md)
14. [`docs/protocols/landing.md`](docs/protocols/landing.md)

Do not rely on README summaries when a normative specification or Atom contract is more precise.

## Language architecture

### Rust is the substrate

Production semantics belong in Rust. Today exactly one crate exists, `crates/gordian-kg`, and it
owns the research knowledge schema, validation, indexes, and queries. Everything else below is
**planned**; [`docs/implementation/crate-map.md`](docs/implementation/crate-map.md) decides which
crate it lands in and what that crate may depend on, and
[`docs/implementation/execution-order.md`](docs/implementation/execution-order.md) decides when.

Rust will own (planned):

- Mission Graph domain types and invariants;
- graph algorithms and indexing;
- scheduling and concurrency control;
- event and state projection;
- evidence and provenance semantics;
- the source adapter, of which Jujutsu integration is one realization;
- persistence adapters;
- authorization and capability enforcement;
- protocol parsing and validation;
- CLI, server, and runtime hot paths.

Do not describe any of these in the present tense until the owning Atom has a validating closure
record; `scripts/check-capability-tense.sh` fails CI on the ones listed in `README.md` under
"Planned, not built".

Prefer safe Rust. `unsafe` requires a documented invariant, a demonstrated performance or interoperability need, focused tests, Miri or sanitizer coverage where applicable, and benchmark evidence showing why the safe design is insufficient.

Production crates should deny unsafe code until a reviewed Atom explicitly introduces a narrow exception.

### Python is a thin orchestration layer

Python may coordinate experiments, launch tools and processes, stage source acquisition, prepare datasets, aggregate benchmark results, perform statistical analysis, and reconcile temporary external projections.

Python MUST NOT become a second implementation of Mission Graph semantics, scheduler safety, evidence freshness, authorization, lease arbitration, accepted-frontier transitions, or research ontology semantics.

If Python needs a domain decision, call the Rust implementation through a stable CLI, IPC, or binding instead of duplicating logic.

### Lean is a development and formal dependency

All Lean source, toolchains, Lake configuration, formal fixtures, checker configuration, and proof-only dependencies live under `formal/`.

Lean MUST NOT be required to build or run the production Gordian binary.

Formal development follows verification-guided development. The current Lean modules are
proposition-level models with no executable oracle and no Rust bridge; the pipeline is **planned**
and #7 owns it, with the vector format in
[`docs/formal/conformance-vectors.md`](docs/formal/conformance-vectors.md):

1. write a small executable formal model for safety-critical semantics (planned, #7);
2. prove meaningful model properties under explicit assumptions;
3. implement optimized production semantics in Rust;
4. differentially test Rust against the executable model over generated inputs (planned, #7);
5. use property, mutation, fuzz, bounded verification, concurrency, integration, and fault tests outside the formal model.

A theorem about the model is never described as proof of an empirical performance claim or proof that Rust refines the model unless that refinement has actually been established.

## Architecture rules

1. **Mission is goal, PlanRevision is strategy.** Never fuse the identity of the objective with one implementation plan.
2. **Decomposition is not dependency.** Containment and causal prerequisites are separate graph relations.
3. **Atoms are the current global scheduling contract.** Quarks remain local execution structure unless experiment falsifies this boundary.
4. **Attempts are not specifications.** A failed attempt does not mutate the Atom contract.
5. **Status is a projection.** Derive readiness, blocking, activity, verification, and satisfaction from canonical facts wherever feasible.
6. **Evidence is exact-subject evidence.** Rewrites, relevant dependency changes, environment changes, or verifier changes invalidate incompatible evidence.
7. **Integration is a first-class candidate.** Passing sibling candidates do not imply their composition passes.
8. **Workers cannot redefine accepted reality.** Promotion, release creation, and deployment are distinct capabilities.
9. **Deterministic core, explicit effects.** Model calls, clocks, network reads, external writes, and other nondeterministic activities are recorded at the boundary and never repeated merely to replay state.
10. **Graph topology encodes causality, not completion order.** Do not serialize independent changes merely because one happened first.
11. **Reference semantics precede optimization.** Preserve a simple oracle or another explicit correctness bridge when practical.
12. **Negative evidence may delete architecture.** A failed Gordian hypothesis must simplify, condition, or replace the affected design.

## Jujutsu workflow

Jujutsu is the preferred local change substrate, subject to the Jujutsu-versus-Git experiment. GitHub remains an external collaboration and transport system.

[`scripts/bootstrap-jj.sh`](scripts/bootstrap-jj.sh) is the single source of the pinned Jujutsu baseline. Do not restate that version number anywhere, and do not design around an older binary.

Acquisition and bootstrap are one block, in [`docs/implementation/agent-runbook.md`](docs/implementation/agent-runbook.md) section 0. From an existing checkout:

```bash
bash scripts/bootstrap-jj.sh --install
jj --version
jj root
jj status
jj log -r '::@ | @::'
```

The script is not a behavioral contract test; #1 owns that suite.

Repository policy:

```text
main / trunk()      accepted source frontier
change ID           logical evolving implementation
commit ID           exact candidate identity
workspace           isolated worker execution state
bookmark            external transport identity
tag                 immutable release identity
deployment record   observed deployed release state
```

There is no permanent `develop` bookmark.

One active writer per logical change is the normal path. Speculative alternatives receive separate changes from the same exact base.

Never move `main`, create releases, deploy, or push canonical state from a worker flow. Publication is reserved to an actor holding `move_accepted_frontier`; who that is, and under exactly what condition, is the bootstrap authority table in [`docs/implementation/agent-runbook.md`](docs/implementation/agent-runbook.md) section 6.7, and the sequence is [`docs/protocols/landing.md`](docs/protocols/landing.md).

Verification applies to exact state ids. Rewriting a change after verification creates a new Candidate and invalidates state-bound evidence even when the `logical_change_id` remains stable.

**Never edit the default workspace.** Work happens in a workspace created with `jj workspace add -r 'trunk()'`, per the runbook section 6.5; another agent or a human may hold the default one.

## Black-box module rule

A module should expose contracts rather than internal structure.

Prefer:

```text
input contract -> deterministic or controlled transformation -> output contract
```

Internal representation may change without forcing callers to know about it.

Cross-crate dependencies must follow the declared architecture direction, which is
[`docs/implementation/crate-map.md`](docs/implementation/crate-map.md) and nothing else: a
dependency is permitted if and only if it appears in that crate's `May depend on` row.
`scripts/check-crate-map.sh` enforces it. Avoid convenience imports that create hidden coupling.

## Performance rule

Correct-but-greedy is a baseline, not the finish line.

For algorithms on the Mission Graph, scheduler, evidence index, replay engine, persistence layer, Jujutsu adapter, and knowledge graph:

1. implement the simplest correct reference algorithm when useful;
2. state time and space complexity;
3. construct representative and adversarial benchmark datasets;
4. benchmark time, allocation, memory, throughput, tail behavior, and scaling shape as applicable;
5. compare optimized implementations against the reference for semantic equivalence;
6. preserve the reference implementation as a test oracle when cheap enough;
7. measure heuristic regret against an exact solver on small instances when the optimization problem permits it;
8. separate substrate overhead from worker, verifier, network, or model latency.

Never optimize from intuition alone. Never retain an elegant optimization whose benefit disappears under benchmark or whose complexity cost exceeds the measured gain.

Important scheduling lower bounds and baselines include total work, critical-path length, topological or list scheduling, resource constraints, and exact small-instance solutions. Heterogeneous workers should be evaluated with HEFT-style or comparable list-scheduling baselines rather than only a FIFO queue.

A scheduler that maximizes concurrent workers while increasing conflict, verification, repair, or model cost is optimizing the wrong proxy.

## Verification ladder

Use the strongest applicable method for each claim. A tool belongs only when it protects a named risk.

### Required everyday checks

```bash
scripts/verify-local.sh all
```

`--strict` is not optional: a bare `audit` passes on warnings and is therefore not a gate.

`scripts/verify-local.sh` is the command source for both local runs and CI's four separately
reported jobs. The formal group builds with warnings denied, replays the compiled environment with
the pinned toolchain's `leanchecker`, audits the axiom closure through
`formal/Gordian/Audit.lean`, and negative-tests both controls. `leanchecker` is a separate pass
through Lean's kernel, not an independently implemented kernel. The Rust group runs the graph CLI
integration/audit-rule tests and `cargo deny check`. Supply-chain update policy is
`.github/dependabot.yml`; exact tool versions are checked by `scripts/check-toolchain.sh`.

### As applicable

- generated property and state-machine tests;
- differential randomized testing against executable Lean models (planned, #7);
- mutation testing with `cargo-mutants` or an equivalent method;
- fuzzing for parsers, protocols, event histories, JSON-LD, Jujutsu output, and command sequences;
- Miri and sanitizers for unsafe and memory-sensitive boundaries;
- Kani for bounded bit-precise safety or correctness harnesses;
- Loom for compact concurrent state spaces;
- Shuttle for broader deterministic randomized schedules;
- deterministic network and failure simulation such as Turmoil where appropriate;
- benchmark suites with Criterion or Divan and lower-noise instruction or allocation profiling where useful;
- process, storage, database, network, lease, stale-worker, duplicate-event, candidate-rewrite, and recovery fault injection.

Record configured bounds, seeds, toolchains, source commits, environment identity, and limitations. Bounded verification is not unbounded proof. A long stress test is not concurrency proof. A passing benchmark is not a complexity theorem.

## Research discipline

The knowledge graph is part of the engineering substrate.

Every material concept introduced by research, experiment, proof, specification, dependency, or failure must be represented in the canonical JSON-LD shards under `knowledge/graph/`, with inspectable relations to:

```text
source revisions and exact locators
claims and scope
assumptions and limitations
supporting, qualifying, challenging, or contradictory evidence
algorithms and complexity obligations
theorems and proof artifacts
experiments and exact runs
engineering decisions and alternatives
documents, issues, and implementation artifacts
```

Follow [`knowledge/acquisition.md`](knowledge/acquisition.md). A link in prose is not complete acquisition.

Distinguish:

```text
formal theorem
established scientific or engineering foundation
evidence-supported conclusion
engineering deduction
hypothesis
assumption
unresolved uncertainty
```

Actively record disconfirming and qualifying evidence. A graph that only accumulates support edges is a citation shrine, not research infrastructure.

When a preprint, standard, documentation page, repository, or dataset materially changes, create a source revision and propagate staleness or changed scope. Do not overwrite historical provenance.

## Experiment discipline

An experiment must state before execution:

- bounded hypothesis;
- falsification and architecture-revision rule;
- treatments, baselines, controls, and alternatives;
- workloads, sampling, seeds, budgets, and stopping rule;
- primary and secondary metrics;
- analysis plan and uncertainty method;
- exact source, tools, model/provider, environment, and hardware identity;
- raw artifact and run-ledger destinations;
- threats to validity.

Retain failed, timed-out, excluded, and negative runs with reasons. Do not keep only favorable trials. Conclusions remain bounded to the actual study scope.

Use [`.github/ISSUE_TEMPLATE/experiment.yml`](.github/ISSUE_TEMPLATE/experiment.yml) for experiment Atoms. The analysis design is fixed per experiment class by [`docs/testing/statistical-contract.md`](docs/testing/statistical-contract.md), not chosen per study. The authorized provider list, credential environment variables, and cost caps are **G-526, assigned to #37**; until it lands, record provider, model id, and observed spend in the run manifest and stay inside the per-Atom caps of the runbook section 7.

## Documentation style

Do not use maturity labels such as `v0`, `v1`, `v2`, `M0`, `M1`, or similar pseudo-roadmap version names for Gordian concepts, specifications, or phases.

Use semantic names:

```text
Mission Graph Specification
Foundation and Falsification Initiative
Reference Kernel Initiative
Distributed Coordination Initiative
```

Technical protocol, package, source, schema, and wire versions are allowed only when compatibility or reproducibility genuinely requires an explicit identifier.

Avoid status prose that will become stale. Prefer generated facts, experiment records, exact evidence, and links to canonical Atom identities.

## Atom contract

Use [`.github/ISSUE_TEMPLATE/atom.yml`](.github/ISSUE_TEMPLATE/atom.yml) for new implementation Atoms.

An Atom must define:

- Initiative and bounded objective;
- non-goals where scope could leak;
- causal dependencies and required interfaces;
- declared inputs, outputs, semantic reads/writes/provides/requires as applicable;
- effect class;
- acceptance predicates;
- verification and exact evidence subjects;
- benchmark obligation for performance-sensitive work, written as a `## Benchmark obligation`
  section naming each `EO17-*` row id the Atom owns in
  [`docs/implementation/execution-order.md`](docs/implementation/execution-order.md) section 17;
- falsification or simplification trigger for experimental architecture.

Before declaring an Atom complete, write `artifacts/atoms/<N>/closure.json` conforming to
[`artifacts/schema/closure-record.schema.json`](artifacts/schema/closure-record.schema.json) and
link it from the closing comment. That schema is the single normative list; do not restate it
here, and do not close an Atom whose record fails `scripts/check-closure-records.sh`. The
procedure is [`docs/implementation/agent-runbook.md`](docs/implementation/agent-runbook.md)
section 2, and the reviewer-facing copy of the same eight items is
[`.github/PULL_REQUEST_TEMPLATE.md`](.github/PULL_REQUEST_TEMPLATE.md).

Passing tests are necessary but not automatically sufficient when the Atom has stronger acceptance predicates. Closing a GitHub issue is bookkeeping, not native satisfaction evidence.

An attempt that does not reach closure is recorded too: the retry limits, budget caps, and the
ordered abandon procedure are [`docs/implementation/agent-runbook.md`](docs/implementation/agent-runbook.md)
section 7. Weakening an acceptance predicate to make an attempt pass is a specification change,
never an implementation choice.

### Actor identity

The executing worker carries the actor string `gordian-agent/<harness>/<run-id>` in the Atom claim
comment's first line, the Atom candidate's author identity and `Gordian-Actor:` trailer, and the
closure record's `actor.id`; those four executing-worker identities MUST agree. The closure
bookkeeping change has a separate author identity and matching `Gordian-Actor:` trailer: its actor
is the coordinator who recorded the closure, named by `closure.recorded_by.id`. `recorded_by` may
be omitted only when the executing worker also authored the bookkeeping change; in that case it
defaults to `actor`. `closure.actor` names the worker that executed the Atom and produced the
candidate, not the author of the bookkeeping record. See the runbook sections 2 and 6.1.

### Per-file license headers

Per-file license headers are not required; `LICENSE` and Cargo package metadata govern. Requiring
headers would add a mechanical edit and checker without improving license identification for this
single-license repository; `cargo deny check licenses` verifies dependency license policy instead.

## Temporary GitHub substrate

GitHub issues and Project 9 are temporary external projections while Gordian builds its native planning substrate. The **native `blocked by` graph is authoritative** for dependencies; the `## Dependencies` prose in an issue body is a mirror with no authority. The milestone is authoritative for Initiative membership. `Wave`, `Fan In`, `Fan Out`, `Status = Blocked` and `Status = Ready` are derived projections, never inputs, and are written only by `gordian-derive-status derive --apply`; `In Progress`, `In Review`, and `Accepted` are claim facts the runbook's loop asserts and the derivation never overwrites. No board cell is read back as authority — see [`docs/implementation/issue-index.md`](docs/implementation/issue-index.md) and [`docs/implementation/agent-runbook.md`](docs/implementation/agent-runbook.md) section 6.9.

Board and issue mutations use the process-injected `GH_TOKEN`, copied to `GH_TOKEN` for
every `gh` subprocess so ambient config files cannot select a different credential. Never commit
the token. The implemented preflight is authoritative for identity, repository-write permission,
and Project read/write capability; do not infer those facts from a token label or config file.

```bash
python3.14 -m pip install -e './orchestration[dev]'
gordian-bootstrap preflight
gordian-project-sync reconcile --check
gordian-project-sync reconcile --report artifacts/project-9-reconciliation.json
```

The reconciler may add missing issue URLs and report duplicates. It must not infer readiness, status, satisfaction, evidence, or acceptance from GitHub fields. #70 owns this projection and is closed or archived once #48 lands; **G-475, G-502, G-507, G-522, G-527, G-530, and G-609 are assigned to #70.** The readiness projection of G-504 and G-516 is already checked in as `gordian-derive-status` — [`docs/implementation/agent-runbook.md`](docs/implementation/agent-runbook.md) section 6.2 names it as the only sanctioned way to pick the next Atom — and what remains of those two ids is the committed edge snapshot (G-502) and the ordered `ready` output (G-530).

## No cargo-cult dependencies

Before adding a crate, service, database, workflow engine, agent framework, graph store, sandbox, or formal tool, state:

- which requirement it serves;
- what simpler alternative exists;
- performance and operational costs;
- failure, security, and maintenance surface;
- whether it becomes a production or development-only dependency;
- which benchmark, experiment, proof obligation, or threat model justifies retention.

Gordian exists partly to avoid reproducing accidental industry complexity. The same standard applies to Gordian itself.
