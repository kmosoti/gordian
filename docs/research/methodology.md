# Research and Evidence Methodology

Gordian is a research project as well as a software project. Its documentation therefore needs rules for how external evidence is converted into design decisions.

The goal is not to maximize the number of citations. The goal is to make every consequential inference inspectable and falsifiable.

## 1. Assertion classes

Every material architectural statement should belong to one of these classes.

### Verified formal proposition

A proposition checked by Lean for an explicit model and assumptions.

Required record:

```text
formal statement
assumptions
toolchain
proof source
CI revision
engineering interpretation
non-claims / scope boundary
```

### Standard / established foundation

A mechanism or model supported by mature research, a formal standard, or broad production engineering practice.

Examples:

- dependency DAGs;
- provenance Entity/Activity/Agent separation;
- hierarchical planning;
- content-bound artifact provenance;
- deterministic workflow replay boundaries.

Adopting an established mechanism does not prove Gordian's adaptation is superior.

### Empirical conclusion

A conclusion supported by measured study results.

Required record:

```text
study / version
tasks / benchmark
baselines
reported effect
limitations
what Gordian infers
what Gordian does NOT infer
```

### Engineering deduction

A conclusion derived from established mechanisms but not itself directly measured.

Example:

> Because Jujutsu preserves change identity across rewrites while exact commit identity changes, verification intended to describe exact source content should bind to the commit identity rather than only the change identity.

This is a strong engineering deduction, but its operational benefit still deserves implementation tests.

### Hypothesis

A Gordian-specific claim that must be experimentally challenged.

Every hypothesis should identify:

```text
independent variable
comparison/baseline
observable outcomes
failure threshold
confounders
cost metric
```

### Assumption

A premise required by an algorithm/proof.

Assumptions must be exposed because a correct deduction from a false or incomplete premise is still operationally wrong.

## 2. Evidence hierarchy is contextual

Gordian does not use a single universal source ranking.

Different questions require different evidence.

| Question | Strong evidence type |
| --- | --- |
| Does a theorem follow from definitions? | proof checker |
| Is a protocol field defined this way? | normative specification |
| How does Jujutsu currently behave? | current official docs + executable tests |
| Does an agent architecture improve benchmark outcomes? | replicated/controlled empirical study |
| Does a scheduler scale? | benchmark under representative graph/workload |
| Does a safety policy resist faults? | fault injection + model/state-machine tests |
| Does a UI help humans coordinate? | user study / operational observation |

A formal proof is not automatically stronger than experiment if the question is empirical.

## 3. Preprints and fresh results

Much of the 2026 agent literature is preprint-stage and fast moving.

Therefore:

- preserve the paper identifier and retrieved version/date where possible;
- record benchmark/task scope;
- do not generalize from a handful of tasks to universal agent behavior;
- prefer mechanism-level conclusions that are shared across multiple studies;
- revisit results when papers are revised or independently replicated.

For example, AgentRoom is relevant because its ablations point toward the importance of coordination, but its small four-task evaluation makes it directional evidence rather than a settled engineering law.

## 4. Version drift

Research and software documentation can change after Gordian records a result.

The initial research supplied to Gordian reported different CAID headline deltas than the current arXiv abstract. The current abstract reports +26.7 absolute on PaperBench and +14.3 on Commit0.

The correct response to this kind of drift is not to silently select whichever number is convenient.

Record:

```text
source identity
version/retrieval date
old value if historically material
new value
whether the design conclusion changes
```

The knowledge graph should eventually support explicit SourceRevision nodes.

## 5. Multi-source synthesis

A useful design conclusion should ideally survive more than one source or mechanism.

Current example:

```text
CAID:
  dependency-aware delegation + isolation + structured integration works better than its solo baseline.

STORM:
  isolation through worktrees alone is weaker than explicit shared state management in its evaluation.

AgentRoom:
  small recent study suggests coordination signals bear more benefit than CRDT merging or parallelism alone.
```

The bounded synthesis is:

> Isolation appears useful, but isolation alone is not sufficient; explicit coordination/state management is a credible additional requirement.

The unsupported synthesis would be:

> Gordian's semantic coordination protocol is proven optimal.

Nothing in those studies establishes that.

## 6. Negative and disconfirming evidence

The graph must have `challengedBy` and `qualifiedBy` relations, not only `supportedBy`.

For every major Gordian hypothesis, actively seek:

- cases where a simpler mechanism performs as well;
- cases where coordination overhead dominates;
- tasks where parallelism reduces quality;
- evidence that semantic claims are too expensive/inaccurate to maintain;
- evidence that Jujutsu-specific orchestration adds more complexity than it removes;
- formalization effort that fails to catch practical bugs.

A design that can only accumulate supporting citations is not doing science.

## 7. Falsification protocol

Before implementing a hypothesis deeply, define what would cause us to abandon or revise it.

Example: semantic claims.

### Hypothesis

Semantic resource declarations predict harmful concurrent work better than file-path overlap.

### Experiment

Run the same candidate workload through:

```text
A: file/path conflict predictor
B: semantic read/write predictor
C: combined predictor
```

Measure against observed integration outcomes.

### Outcomes

```text
precision
recall
false serialization rate
missed semantic conflict rate
coordination overhead
wall-clock completion
verification cost
```

### Revision trigger

If semantic claims do not materially improve useful predictive performance after accounting for declaration/observation cost, do not make them mandatory Gordian substrate complexity.

## 8. Reproducibility record

Experiments should capture:

```text
repository commit
Mission Graph revision
agent/model identities
prompts/policies if relevant
base commits
random seeds
machine/environment identity
time/resource budgets
verification manifest
raw event history
raw evidence
analysis code
```

A summarized score without these materials is insufficient for architecture-driving conclusions.

## 9. Statistical discipline

Agent evaluations can have large run-to-run variance.

Avoid:

- one-run comparisons;
- choosing the best run from one architecture against a mean from another;
- ignoring cost/token differences;
- changing verifier strength between conditions;
- using model-judge output as if it were ground truth without calibration;
- treating benchmark pass rate as a full measure of engineering usefulness.

Prefer paired comparisons where feasible, repeated trials, uncertainty intervals, and explicit ablations.

## 10. Specification gaming

Gordian itself is vulnerable to Goodhart effects.

Potential proxies:

```text
Atoms completed
parallel workers active
verification checks passed
Mission closure rate
mean execution latency
```

A system can optimize any of these while doing worse engineering.

For example, an agent could create trivial Atoms to maximize completion count or weaken acceptance predicates to increase Mission closure.

Therefore:

- Mission acceptance changes require provenance/review;
- success metrics should include externally meaningful outcomes;
- reward/policy layers cannot mutate safety predicates merely to improve scores;
- experiment analysis must inspect failure modes, not only aggregate success.

## 11. Research graph update rule

When new evidence arrives:

1. add/update the Source node;
2. create a SourceRevision when version changes materially;
3. connect it to explicit Claim/Hypothesis nodes;
4. mark relation semantics (`supportedBy`, `qualifiedBy`, `challengedBy`);
5. update the human research synthesis;
6. decide whether normative specification changes are justified;
7. if the specification changes, update formal obligations and implementation tests separately.

Research evidence should not mutate protocol semantics automatically.

## 12. Graduation criteria

A Gordian hypothesis can move toward a normative default when:

- it has a clear mechanism;
- alternatives have been tested;
- measurements show useful benefit on representative workloads;
- operational cost is understood;
- known failure modes have mitigations;
- the design does not rely on an obviously fragile proxy;
- safety-critical pieces have the strongest applicable formal/test coverage.

Even then, defaults should remain revisable. Mission Graph itself is a hypothesis until its operational utility is demonstrated.
