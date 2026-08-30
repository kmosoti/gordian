# 2026 Multi-Agent Software Engineering Evidence

This document isolates the fast-moving empirical literature that most directly motivates Gordian's agent coordination design.

The studies below do **not** prove Gordian. They help constrain which architectural directions are plausible and which simplistic approaches already have evidence against them.

## 1. CAID — dependency-aware isolated delegation

**Paper:** Geng & Neubig, *Effective Strategies for Asynchronous Software Engineering Agents*  
**arXiv:** https://arxiv.org/abs/2603.21489v2 (retrieved 2026-08-30)

### Reported mechanism

Centralized Asynchronous Isolated Delegation (CAID) combines:

```text
centralized dependency-aware task planning
asynchronous execution
isolated workspaces
structured integration
executable test-based verification
```

### Current headline result

The submission history holds exactly two versions, and the current one is v2:

```text
arXiv:2603.21489v1  23 Mar 2026  +26.7 PaperBench  +14.3 Commit0   superseded
arXiv:2603.21489v2   8 Jul 2026  +25.6 PaperBench  +14.7 Commit0   current, retrieved 2026-08-30
```

The cited improvements over single-agent baselines are therefore **+25.6 absolute on PaperBench** and **+14.7 absolute on Commit0**, from v2.

This document previously presented the v1 figures as current and the v2 figures as an earlier research note, which inverted the very drift the record exists to capture; the correction is G-105. Because the public source changed, Gordian keeps both rows rather than silently treating them as identical: `source:caid` pins v2, and `sourcerev:caid-v1` / `sourcerev:caid-v2` hold the two revisions. The same two-row ledger appears in [`methodology.md`](methodology.md) section 4 and [`evidence-synthesis.md`](evidence-synthesis.md) section 2.1.

### Gordian inference

Strongest bounded inference:

> Dependency-aware task structure, isolated execution, and explicit integration/verification are credible ingredients for long-horizon multi-agent software work.

### Not established

- that branches are the optimal isolation primitive;
- that more agents always help;
- that CAID's benchmark gains generalize to arbitrary repositories;
- that Gordian's Atom or semantic-resource abstractions are validated.

## 2. STORM — explicit state management beyond worktree isolation

**Paper:** Liu et al., *Multi-agent Collaboration with State Management*  
**arXiv:** https://arxiv.org/abs/2605.20563

### Reported mechanism

STORM mediates interactions with shared code state rather than giving each agent an isolated view and deferring all conflict discovery to final merge.

### Reported comparison

Against a Git-worktree-based multi-agent baseline, the abstract reports:

```text
Commit0-Lite: +18.7
PaperBench:    +1.4
```

### Gordian inference

Isolation is useful but **isolation is not coordination**.

This supports introducing a semantic coordination plane that exposes claims, state, dependency readiness, and scope expansion while still keeping code mutation isolated.

### Tension with CAID

This does not directly falsify CAID. CAID shows structured isolated delegation can outperform solo baselines. STORM asks a different comparison and finds explicit state management improves over its isolated-worktree multi-agent baseline.

The synthesis is:

```text
isolated code state
+
explicit coordination/state
```

rather than choosing only one.

## 3. AgentRoom — coordination signal versus CRDT merge

**Paper:** Cho & Lee, *AgentRoom: Concurrent Multi-Agent Coding in a CRDT-Backed Shared Workspace*  
**arXiv:** https://arxiv.org/abs/2608.23740

### Why it matters

AgentRoom exposes file-level claim, status, and broadcast tools to agents in a CRDT-backed shared workspace.

The paper's ablation interpretation is particularly relevant: coordination rather than parallelism or CRDT merge appears to carry more of the observed benefit.

### Evidence strength

This is very fresh and small:

```text
four backend coding tasks
five frontier coding CLI models
```

It is therefore directional evidence, not a universal result.

### Gordian inference

Do not jump from “agents need awareness” to “agents should concurrently edit one shared filesystem.”

The more defensible mechanism-level inference is:

> Make intent, claims, state changes, and coordination signals explicit.

Gordian can test that while retaining Jujutsu workspace isolation.

## 4. AgenticFlict — post-hoc integration is not cheap

**Paper:** Ogenrwot & Businge, *AgenticFlict*  
**arXiv:** https://arxiv.org/abs/2604.03551

### Dataset

The paper reports:

```text
142K+ collected agent PRs
107K+ successfully processed through deterministic merge simulation
29K+ PRs exhibiting conflicts
336K+ conflict regions
27.67% textual conflict rate
```

### Gordian inference

A strategy of “let everyone work and merge at the end” has a material integration surface at agent scale.

This motivates:

- early scope/claim visibility;
- dependency-aware scheduling;
- integration as an explicit state;
- conflict repair as bounded work;
- integration-level verification.

### Important limitation

The reported 27.67% is **textual merge conflict**, not semantic conflict.

Textually clean merges can still break interfaces, behavior, schemas, or invariants. Gordian should not misuse the dataset as a direct estimate of total semantic conflict probability.

## 5. CodeTeam — architecture contracts before implementation

**Paper:** Wang et al., *CodeTeam: An LLM-Powered Multi-Agent Framework for Repository-Level Code Generation*  
**arXiv:** https://arxiv.org/abs/2606.22082

### Relevant design

CodeTeam separates planning, decision, and implementation. Its selected architecture is normalized into a machine-checkable contract specifying concepts including:

```text
file ownership
public interfaces
dependency constraints
```

before concurrent implementation.

### Gordian inference

This supports the general direction of making interface and dependency assumptions first-class coordination inputs rather than leaving architecture only in prose context.

Gordian goes further by proposing semantic read/write claims and evidence-bound satisfaction, but those extensions remain hypotheses.

## 6. Engineering Reliable Coding Agents — system reliability framing

**Review:** Stephanie Jarmak, *Engineering Reliable Coding Agents: Evaluating and Operating the System Around the Model*  
**arXiv:** https://arxiv.org/abs/2608.13867

### Relevant synthesis

The review argues that coding agents are deployed systems whose reliability depends on more than model capability, including:

```text
harness
execution state
retrieval
memory/state management
permissions
review interfaces
verification
observability
resource allocation
```

It synthesizes scholarly, practitioner, benchmark, and operated-system records, while explicitly noting evidence strength varies across topics.

### Gordian inference

The worker model should be treated as replaceable probabilistic machinery inside a reliability substrate.

Therefore safety-critical definitions should not depend on claims such as:

```text
"the agent is smart enough to know when it is done"
```

Instead use:

```text
explicit state
capability boundaries
recorded execution
verification
evidence
recovery
```

## 7. Combined mechanism map

| Observation | Strongest Gordian implication | Evidence limitation |
| --- | --- | --- |
| CAID improves over solo baselines with dependency-aware isolated delegation and integration | derive parallelism from dependency structure; isolate mutable execution | benchmark-specific preprint |
| STORM beats a worktree multi-agent baseline in reported experiments | isolation alone does not provide enough shared state | implementation/benchmark scope |
| AgentRoom ablation emphasizes coordination | expose coordination signals explicitly | only four backend tasks; very fresh |
| AgenticFlict finds 27.67% textual conflict in processed agent PRs | integration conflict deserves first-class handling | textual conflicts only; observational dataset |
| CodeTeam normalizes architecture contracts | expose interfaces/dependencies before concurrent implementation | repository-generation setting |
| reliability review emphasizes whole harness | make correctness a substrate property rather than model property | heterogeneous evidence quality |

## 8. The central synthesis

The current evidence supports this architecture **more strongly than** either extreme:

```text
Extreme A:
  one shared mutable filesystem, all agents edit freely

Extreme B:
  every agent isolated, no shared coordination until merge
```

Gordian's working hypothesis is:

```text
isolated code state
+
coordinated semantic state
+
dependency-aware scheduling
+
explicit integration
+
exact-artifact verification
+
capability-gated acceptance
```

The first five terms are motivated by overlapping evidence and mature engineering mechanisms. The exact Gordian realization remains unvalidated.

## 9. Experiments this literature forces us to run

### Isolation ablation

```text
solo
isolated parallel
isolated + shared status
isolated + semantic claims
```

### Conflict timing

Measure conflict discovery:

```text
write-time / coordination-time
candidate-time
integration-time
```

and total repair cost.

### Contract granularity

Compare:

```text
file ownership
module/package ownership
semantic resource claims
interface contracts
```

### Jujutsu versus Git

Hold planner/agents/verifiers constant and vary only source coordination substrate.

Measure:

- orchestration code complexity;
- candidate identity mistakes;
- recovery operations;
- integration failure rate;
- verification invalidation correctness;
- wall-clock completion.

If Jujutsu does not produce a meaningful advantage, Gordian should not turn it into ideology.

## 10. Update policy

These papers are young enough that Gordian should re-check their public versions before making benchmark numbers normative documentation.

The knowledge graph should eventually record paper revisions as distinct SourceRevision entities and connect claims to the exact version evaluated.
