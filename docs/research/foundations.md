# Scientific and Engineering Foundations

Gordian is not intended to invent a new vocabulary for every solved subproblem. Its novelty, if any, should come from how established mechanisms are composed and from the parts that survive falsification.

This document maps the main architectural choices to mature foundations.

## 1. Hierarchical planning

### Source family

Hierarchical Task Network (HTN) planning models problem solving as decomposition of compound tasks into progressively more primitive tasks under methods, ordering constraints, and state conditions.

Useful overview:

- Georgievski & Aiello, *HTN planning: Overview, comparison, and beyond* (Artificial Intelligence, 2015): https://www.sciencedirect.com/science/article/pii/S0004370215000247

### Gordian adaptation

```text
Mission
  -> PlanRevision
     -> Initiative
        -> Atom
           -> Quark
```

This borrows the idea of hierarchical decomposition, but Gordian does **not** claim this vocabulary is standard HTN terminology or an optimal hierarchy.

The important inherited lesson is that a high-level objective and its decomposition method are different objects. This motivates keeping Mission identity stable while PlanRevision changes.

### Risk

HTN systems depend heavily on domain decomposition knowledge. A poor method library can constrain planning into bad strategies. Gordian should therefore version decompositions and treat them as revisable hypotheses.

## 2. Preconditions, postconditions, and proof obligations

### Source

C. A. R. Hoare, *An axiomatic basis for computer programming*, Communications of the ACM 12(10), 1969.

DOI: https://doi.org/10.1145/363235.363259

### Gordian adaptation

An Atom is represented as a bounded contract with explicit assumptions and acceptance obligations.

Conceptually:

```text
{ P } Atom { Q }
```

This is intentionally reminiscent of Hoare-style pre/post reasoning.

Gordian does **not** assume ordinary engineering work can all be reduced to total formal correctness proofs. The adaptation is structural: hidden assumptions should become named preconditions, and “done” should become observable postconditions/evidence.

## 3. Dependency DAGs and declared versus actual dependencies

### Source

Bazel's dependency model distinguishes the graph of **actual dependencies** from **declared dependencies** and requires the actual dependency graph to be contained within the declared dependency graph for correct builds.

Current documentation: https://bazel.build/concepts/dependencies

### Gordian adaptation

```text
D_observed subset-of D_declared
```

where dependency/resource observation can include more than source imports.

Potential semantic resources include APIs, types, schemas, configuration, artifacts, and services.

This gives Gordian a falsifiable hidden-dependency condition.

### Limitation

Bazel itself notes that dependency checking cannot be complete in all cases. Gordian inherits an even harder problem because semantic work dependencies are richer than build inputs. Observation is evidence, not omniscience.

## 4. Optimistic concurrency control

### Source

H. T. Kung and John T. Robinson, *On optimistic methods for concurrency control*, ACM Transactions on Database Systems 6(2), 1981.

DOI: https://doi.org/10.1145/319566.319567

### Relevant mechanism

Optimistic concurrency permits work to proceed without pessimistically locking every object, then validates compatibility/conflict before commitment.

### Gordian adaptation

A worker receives a stable source snapshot, works privately, freezes an exact candidate, and reconciles/integrates against the current accepted frontier before admission.

This is an **analogy and design borrowing**, not an assertion that software edits satisfy database transaction semantics.

The read/write conflict predicate is useful as a scheduling heuristic, but semantic software effects can violate assumptions that would be natural in a database model.

## 5. Workflow soundness

### Source family

Workflow nets use Petri-net structure to analyze workflow properties such as sound completion and problematic transitions.

A useful survey/classification:

- van der Aalst et al., *Soundness of workflow nets: classification, decidability, and analysis*, Formal Aspects of Computing 23, 2011: https://link.springer.com/article/10.1007/s00165-010-0161-4

### Gordian adaptation

Mission Graph v0 deliberately chooses a simpler hard-dependency DAG for causal prerequisites.

This provides tractability and straightforward topological scheduling, but gives up some expressive power of Petri nets for loops, choice, synchronization, and token semantics.

Therefore:

- hard prerequisites remain acyclic;
- retries live in ExecutionAttempt history;
- replanning creates PlanRevisions;
- richer control constructs should be added explicitly rather than smuggling loops into `depends_on`.

### Future research

If conditional joins, resource tokens, or long-running cyclic processes dominate real use cases, a workflow/Petri-net layer may be warranted above or beside the dependency DAG.

## 6. Provenance

### Source

W3C PROV-DM: https://www.w3.org/TR/prov-dm/

PROV separates:

```text
Entity
Activity
Agent
```

and expresses relationships including use, generation, derivation, and responsibility.

### Gordian adaptation

```text
Artifact / Evidence / SpecRevision -> Entity
ExecutionAttempt / VerificationRun -> Activity
Human / worker process / coordinator -> Agent
```

This is a conceptual/interchange mapping. Gordian need not store its runtime state as RDF.

The provenance graph answers questions that the Mission Graph and Change Graph cannot:

- who produced this artifact?
- from which exact materials?
- under which activity?
- which evidence was derived from which attempt?

## 7. Attestation and software supply-chain integrity

### in-toto

Specification: https://github.com/in-toto/specification/blob/master/in-toto-spec.md

in-toto models authorized functionaries carrying out steps and recording signed metadata over materials, products, byproducts, commands, and environment.

### SLSA

Current specification family: https://slsa.dev/spec/v1.2/

Build provenance: https://slsa.dev/spec/v1.2/build-provenance

SLSA provenance separates the build definition from run details and captures resolved dependencies and output subjects.

### Gordian adaptation

Gordian generalizes the idea beyond a build step:

```text
Attestation {
  subject,
  predicate_type,
  actor,
  activity,
  materials,
  products,
  byproducts,
  resolved_dependencies,
  environment,
  identity
}
```

This motivates exact evidence binding and separation between a claim's authenticity and its truth.

A signed wrong assertion is still wrong.

## 8. Hermeticity and reproducibility

Build systems such as Bazel make hermeticity valuable by constraining computation to explicit inputs. SLSA similarly emphasizes identifying build parameters and dependencies.

Gordian adopts the direction, not an unrealistic universal assumption.

Some engineering work is naturally hermetic:

```text
compile exact source with exact toolchain
run deterministic static checker
transform one schema representation into another
```

Other work is effectful:

```text
query live cloud API
run LLM inference
change deployment state
measure latency against a live service
```

The correct response is to classify effects and capture observations, not falsely label the whole world hermetic.

## 9. Durable execution and deterministic replay

### Source family

Temporal documentation: https://docs.temporal.io/

Temporal's workflow model is a strong production example of separating deterministic replayable orchestration from effectful Activities whose results are recorded in event history.

### Gordian adaptation

```text
deterministic Mission/execution projector
+
recorded effect outcomes
```

An LLM call, network request, or external write should never happen merely because the coordinator is rebuilding state from history.

Replay consumes the recorded result. Retry explicitly creates a new attempt.

## 10. Capability separation and least authority

Gordian's Worker / Coordinator / DeploymentAuthority split follows a general security engineering principle: components should receive only the authority necessary for their role.

The exact implementation mechanism remains open, but likely enforcement layers include:

- process/user isolation;
- narrowly scoped credentials;
- remote repository permissions;
- sandbox policy;
- signed capability tokens;
- coordinator-mediated write APIs;
- deployment credentials inaccessible to worker execution contexts.

Formal role predicates do not substitute for these enforcement mechanisms.

## 11. Jujutsu as change-state infrastructure

Official documentation: https://docs.jj-vcs.dev/latest/

Particularly relevant properties:

- a logical change ID can survive rewrites while exact commit IDs change;
- multiple workspaces can share one repository;
- the operation log records repository-state operations;
- conflicts can exist as first-class repository states;
- `jj run` can execute commands against revisions in isolated temporary working copies;
- revsets provide declarative graph selection.

### Gordian mapping

```text
Atom                 logical work contract
jj change ID         evolving implementation identity
jj commit ID         exact implementation candidate
jj workspace         isolated execution state
jj operation log     VCS-level recovery/history
bookmark             external transport identity
main/trunk()         accepted-frontier projection
release tag          immutable release identity
```

Jujutsu is not made authoritative for Mission intent or evidence semantics. It is the Change Graph substrate.

## 12. Graph theory and formal certificates

The initial Lean DAG theorem uses a ranking certificate:

```text
edge A -> B implies rank(B) < rank(A)
```

A directed cycle would imply a natural number is strictly less than itself, contradiction.

This style is useful because the runtime can produce a topological rank during validation and the formal model can reason over the certificate without reimplementing a complex graph algorithm inside the proof kernel.

The runtime still needs to validate that every edge obeys the supplied rank.

## 13. Where Gordian is genuinely speculative

The following are **not established foundations**:

- the Mission/Initiative/Atom/Quark names and exact boundaries;
- semantic resource claims as a practical conflict predictor;
- the claim that Jujutsu materially simplifies autonomous coordination versus a careful Git implementation;
- derived-state project management as a superior human interface;
- the amount of formal proof that is cost-effective for an engineering coordinator;
- whether an integrated Mission Graph can outperform conventional issue/PR/project workflows on real teams.

Those must be tested rather than rhetorically promoted into laws.
