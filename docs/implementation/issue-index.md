# Gordian Atom Register

GitHub issues are the temporary execution substrate for Gordian's implementation. Each issue is an **Atom-sized contract**, not evidence of completion. Issue state and GitHub Project status remain external workflow metadata until Gordian imports the plan into its own Mission Graph.

The current implementation Mission contains **77 registered Atoms**, as captured from the live
native registry after excluding explicitly closed duplicates. This includes normally closed Atoms;
the open-issue search below is only an unfinished-work view.
The exact objective, dependencies, acceptance predicates, verification methods, benchmark
obligations, and falsification conditions live in each issue body. This register groups stable
issue identities by Initiative without duplicating every issue title and contract, which would
create another manually maintained source of drift.

The count and membership below are **derived** from the live milestones, not maintained by hand.
`gordian-atom-registry check-drift` regenerates the marked block and fails on any difference
(**G-445 and G-413, assigned to #70**).

## Canonical planning documents

- [`project-plan.md`](project-plan.md) carries the **normative Mission acceptance table** and a
  view of the Initiative structure. It is a view, not a contract store.
- [`execution-order.md`](execution-order.md) defines the causal spine, the kernel-start gate, the
  phases and their concurrency, the minimal self-hosting prerequisite set, and the experiment
  decision matrix.
- [`agent-runbook.md`](agent-runbook.md) is the loop an agent executes, including the claim
  protocol, the verifier command list, closure, and the Mission stop condition.
- [`crate-map.md`](crate-map.md) decides which crate each Rust Atom writes into and what it may
  depend on.
- **GitHub issue bodies are the executable Atom contracts.** No document here restates them.
- GitHub Project 9 is only a temporary visual projection.

## Initiative register

Issue numbers are listed explicitly rather than as ranges, so a new Atom cannot fall into a gap
between two dashes and go unregistered.

<!-- BEGIN GENERATED: INITIATIVE REGISTER -->

| Initiative | Atom identities | Purpose | Acceptance rule |
| --- | --- | --- | --- |
| [Foundation and Falsification](https://github.com/kmosoti/gordian/milestone/1) | #1, #2, #3, #4, #5, #6, #7, #8, #50, #51, #52, #53, #54, #59, #60, #61, #75 | Reproducible toolchains, source-substrate qualification, workload generators, reference algorithms, benchmark gates, verification qualification, formal/Rust conformance, architecture ablations, and experiment run discipline. | all member Atoms have validating closure records. |
| [Research Knowledge Substrate](https://github.com/kmosoti/gordian/milestone/2) | #71, #72, #73, #74 | Rich source-revision/claim/experiment/proof schema, ontology and repository coverage, epistemic traversal, acquisition refresh, and staleness propagation. | all member Atoms have validating closure records. |
| [Rust Mission Graph Kernel](https://github.com/kmosoti/gordian/milestone/3) | #9, #10, #11, #12, #13, #58 | Typed identities, immutable specifications, decomposition/dependency rules, attempts/candidates/effects, canonical events, derived state, and Project resources. | all member Atoms have validating closure records. |
| [Planning and Reconciliation](https://github.com/kmosoti/gordian/milestone/4) | #55, #56, #57 | Alternative immutable plans, untrusted planner proposals, and desired-versus-observed repair/replanning. | all member Atoms have validating closure records. |
| [Evidence, Provenance, and Authority](https://github.com/kmosoti/gordian/milestone/5) | #14, #15, #16, #17, #18, #19 | Content-addressed artifacts, exact evidence fingerprints, verifier manifests, provenance/attestations, capabilities, and accepted-frontier admission. | all member Atoms have validating closure records. |
| [Scheduling and Coordination](https://github.com/kmosoti/gordian/milestone/6) | #20, #21, #22, #23, #24 | Ready work, critical path, worker/resource matching, semantic claims, leases/fencing, and benchmark-selected scheduling policies. | all member Atoms have validating closure records. |
| [Durable Persistence and Replay](https://github.com/kmosoti/gordian/milestone/7) | #25, #26, #27, #28 | PostgreSQL canonical persistence, disposable projections, transactional transitions, crash/replay fault qualification. | all member Atoms have validating closure records. |
| [Jujutsu Change Plane](https://github.com/kmosoti/gordian/milestone/8) | #29, #30, #31, #32, #33, #34, #76 | The source-adapter trait, the Jujutsu realization, workspace/change lifecycle, exact candidate handoff, integration/conflict repair, exact-revision verification, the Git worktree adapter (#76), and the two-adapter comparison (#34). | all member Atoms have validating closure records. |
| [Agent Execution and Thin Python Orchestration](https://github.com/kmosoti/gordian/milestone/9) | #35, #36, #37, #38, #39, #62, #63, #77 | Worker protocol, sandboxing, secret brokerage, process/agent adapters, experiment manifest validation and run ledger (#37), worker launch and supervision (#77), local coordinator, and coordination ablation. | all member Atoms have validating closure records. |
| [Distributed Robustness](https://github.com/kmosoti/gordian/milestone/10) | #40, #41, #42, #43 | Idempotent remote protocol, distributed leases/frontier safety, deterministic fault simulation, and observability. | all member Atoms have validating closure records. |
| [Human and Programmatic Interface](https://github.com/kmosoti/gordian/milestone/11) | #44, #45, #46, #47 | Rust CLI, typed API/event stream, GitHub import, and Mission/evidence explorer. | all member Atoms have validating closure records. |
| [Self-Hosting and Acceptance](https://github.com/kmosoti/gordian/milestone/12) | #48, #49, #68, #69 | Native import, real multi-worker self-hosting Mission, architecture retention report, and release qualification evidence bundle. | all member Atoms have validating closure records. |
| [Release, Operations, and Acceptance](https://github.com/kmosoti/gordian/milestone/13) | #64, #65, #66, #67 | Immutable release/deployment state, reproducible signed artifacts, migration/recovery, and adversarial security qualification. | all member Atoms have validating closure records. |
| [Temporary GitHub Bootstrap](https://github.com/kmosoti/gordian/milestone/14) | #70 | Reconcile repository issues into user Project 9 without treating board status as canonical semantics. #70 is closed or archived once #48 lands and the native import replaces the board. | all member Atoms have validating closure records. |

<!-- END GENERATED: INITIATIVE REGISTER -->

Fourteen Initiatives, 77 Atoms. Each Atom belongs to exactly one milestone; the milestone is the
single source of truth for Initiative membership.

**The `Acceptance rule` cell is the milestone's contract** (G-523). Each of the 14 GitHub
milestone descriptions carries a line beginning with the literal prefix `Acceptance: ` whose
remaining text is byte-identical to that Initiative's cell above, so a milestone cannot acquire a
private completion rule. The coordinator role that lands an Initiative's final Atom is the actor
that closes the milestone, and only once the rule holds — see
[`agent-runbook.md`](agent-runbook.md) section 6.9. Closing a milestone is bookkeeping and
satisfies nothing on its own. Asserting the milestone descriptions and diffing them against this
column is part of `gordian-milestone-contracts check` (**G-527, assigned to #70**), alongside the
registry checks performed by `gordian-atom-registry check-drift`.

GitHub issue search is an authoritative view only of unfinished work in the temporary substrate;
the full executable registry above includes normally closed Atoms too:

```text
https://github.com/kmosoti/gordian/issues?q=is%3Aissue+is%3Aopen
```

## Research-first execution cut

The first implementation sequence is deliberately conservative:

1. **#2** stabilizes Rust, formal, Python, and CI independently.
2. **#3, #4** establish deterministic workloads and reference algorithms.
3. **#8, #71, #72** make research, proof, experiment, and implementation provenance mechanically
   inspectable.
4. **#9** begins the production Mission Graph kernel once, and only once, those six are satisfied.
5. **#1, #5, #6, #7, #73, #74, #75** run concurrently with the kernel and are consumed where
   [`execution-order.md`](execution-order.md) section 5 says they are consumed. They do not gate
   #9.

The pinned source-substrate baseline comes from `scripts/bootstrap-jj.sh`, and #1 is the
behavioral contract suite over it.

The first runtime implementation should not begin with PostgreSQL, a frontend, distributed workers, or an optimized scheduler. Those would concretize unresolved semantics before the project has reliable oracles and falsification machinery.

## Causal implementation spine

The spine is drawn once, in [`execution-order.md`](execution-order.md) section 4, as the set of
Atoms on a maximum-length blocker path to #69 in the native graph. This register does not carry a
second drawing of it. See that document for the phases, parallel work, safety gates, and
performance obligations.

## Temporary Project 9 reconciliation

The connected repository integration can create issues but does not expose GitHub Projects mutations. The thin local orchestrator completes the visual projection through the authenticated GitHub CLI.

```bash
python3.14 -m venv .venv
source .venv/bin/activate
scripts/install-toolchains.sh python-package
# add the printed virtual-environment bin directory to PATH

gordian-project-sync reconcile --check
gordian-project-sync reconcile --report artifacts/project-9-reconciliation.json
```

**Deterministic non-interactive authorization.** Supply `GORDIAN_GH_TOKEN` through the environment
and never commit it. Every orchestrator copies that value to `GH_TOKEN` for its `gh` subprocesses,
so neither possible `hosts.yml` location can silently select a different credential:

```bash
gordian-bootstrap preflight
gordian-project-sync reconcile --check
```

The preflight responses are authoritative for the authenticated identity, repository-write
permission, and Project read/write capability. **G-522 is assigned to #70**, which makes a
configuration failure name `GORDIAN_GH_TOKEN` and exit 78 rather than changing credentials inside
the loop.

The command:

- lists open repository issues once;
- lists Project items once before mutation;
- identifies missing and duplicate issue URLs;
- adds missing issues idempotently;
- lists Project items again after mutation;
- emits a machine-readable convergence report;
- does not infer readiness, satisfaction, evidence, or acceptance from GitHub fields.

`scripts/sync_github_project.py` is a compatibility entrypoint into the same packaged module, not a second implementation.

The package declares `requires-python = ">=3.14"`. On an interpreter older than 3.14 the
editable install is refused, but the module has no third-party dependencies and runs
directly:

```bash
PYTHONPATH=orchestration/src python3 -m gordian_orchestration.github_project reconcile --check
```

Known limitation: the post-mutation item listing is issued immediately after the last
`gh project item-add` with no retry, so a large first run can report a false
`converged: false` because of GitHub read-after-write lag. Re-running the command is
idempotent and settles the report.

## Initiative milestones and native dependencies

Initiative membership and causal prerequisites are stored in GitHub's own primitives, not
in parallel bookkeeping.

**Initiative = milestone.** Each of the 14 Initiatives above is a GitHub milestone, and
every Atom belongs to exactly one. The milestone is the single source of truth for
Initiative membership, and it gives Initiative-level progress for free. This mapping holds
for the current single implementation Mission; a second concurrent Mission would require
Mission-qualified milestone titles.

**Dependencies = native issue relationships.** Every edge declared in an issue's
`## Dependencies` section is also a native GitHub `blocked by` relationship, created with
the `addBlockedBy` mutation. The current native registry contains 305 edges; the five
previously proposed mirror-only additions are not part of that graph.

The native `blocked by` graph is **authoritative**. The `## Dependencies` prose is a
human-readable mirror with no authority, and the two must be re-checked for drift whenever a
dependency changes. Read the edges from the `blockedBy` node lists; `issueDependenciesSummary`'s
blocking counter has been observed wrong and is not a substitute.

**The kernel-start gate is the native `blocked by` list of #9** — today exactly #2, #3, #4, #8,
#71, #72 — and nothing else. The `gate:foundation` label marks those six issues, and the Project 9
"Foundation Gate" view filters on that label. No wave range defines the gate; a wave is a depth,
not a contract.

Project 9 carries four derived fields on top of GitHub's native ones.

| Field | Source of truth | Meaning |
| --- | --- | --- |
| Milestone | Native | The Initiative the Atom belongs to. |
| Assignee | Native | The claim. An Atom with an assignee is claimed. |
| Work Type | `type:*` label | `Atom` is a work contract. `Experiment` is a falsifiable study. |
| Wave | Derived | Longest-path depth over the native `blocked by` graph. |
| Fan In | Derived | In-degree: how many Atoms this one waits on directly. |
| Fan Out | Derived | Out-degree: how many Atoms this one directly unblocks. |
| Status | Derived | `Blocked`, `Ready`, `In Progress`, `In Review`, or `Accepted` — defined below. |

`Status` values are defined by facts, not by judgement, and each value is written by exactly one
writer:

```text
Blocked      some blocker is not satisfied under the bootstrap satisfaction rule   derived
Ready        every blocker is satisfied and the issue has no assignee              derived
In Progress  every blocker is satisfied and the issue has an assignee              claim-owned
In Review    a pull request referencing the issue is open                          claim-owned
Accepted     the issue is closed AND artifacts/atoms/<N>/closure.json validates    claim-owned
```

`Blocked` and `Ready` are computed from the graph plus the on-disk closure records and are written
only by the command below. `In Progress`, `In Review`, and `Accepted` are facts about a claim and
its landing, asserted by the agent executing
[`agent-runbook.md`](agent-runbook.md) sections 6.3, 6.7, and 6.8; the derivation never overwrites
them. No cell has two writers, and no cell is read back as authority: the authority for a claim is
the assignee and the authority for acceptance is the closure record.

`Wave`, `Fan In`, and `Fan Out` are projections, never inputs. They are recomputed from the graph,
never edited by hand, and carry no authority. The checked-in command that computes and writes all
four fields is `gordian-derive-status`, the `gordian_orchestration.derive_status` module under
`orchestration/src/gordian_orchestration/` (G-504, G-516):

```bash
GH_TOKEN="$GORDIAN_GH_TOKEN" gordian-derive-status ready              # the ready set
GH_TOKEN="$GORDIAN_GH_TOKEN" gordian-derive-status derive --compare-board
GH_TOKEN="$GORDIAN_GH_TOKEN" gordian-derive-status derive --apply     # write Project 9
```

It reads edges from the native `blockedBy` connection only and validates pagination against its
`totalCount` — `issueDependenciesSummary` is never read because its blocking counter has been
observed wrong for #11, #18, and #44 — computes `Wave` as longest-path depth and `Fan In`/`Fan Out`
as in- and out-degree, and applies the bootstrap satisfaction rule below.
`orchestration/README.md` documents its options. It is a projection of GitHub's own dependency
edges and is deleted when #48 lands.

A mistakenly created issue leaves the executable Atom corpus only after it is closed and receives
GitHub's `duplicate` label. The derivation rejects an open duplicate and continues to reject every
ordinary closed Atom without a validating closure record.

`gordian-atom-registry check` makes the live graph/body/metadata/plan/spine comparison executable.
After a coherent live change, `gordian-atom-registry capture --output
artifacts/atoms/issues.json` records the full issue contracts and native edges; capture refuses
while any mirror drifts. `scripts/check-atom-registry.sh` performs the offline CI comparison once
that snapshot exists.

`gordian-bootstrap preflight` enforces the unattended classic-token contract and exits 78 on
missing configuration. Its `claim`, `claims`, and `release` subcommands implement the temporary
assignee/Project-status/expiry lease with compensation for partial writes; #23 retires it when
native leases exist.

`--snapshot artifacts/atoms/issues.json` reads a committed registry capture instead of calling
`gh`, for inspection only. Snapshot-based readiness requires `ready --inspection`, is explicitly
non-dispatching, and cannot feed claim or apply. **G-502 is assigned to #70**, whose registry
capture records complete issue bodies, metadata, and native edges only after the live drift audit
is clean.

Sort the board by `Wave`, not by issue number. Issue numbers are arbitrary external
identities.

## Bootstrap satisfaction rule

Closing an issue is not satisfaction, so readiness cannot be computed from issue state alone. The
bootstrap analogue of Satisfied-as-admitted
([`../spec/mission-graph.md`](../spec/mission-graph.md#logical-state-predicates)) is normative:

> An Atom `N` counts as **satisfied for readiness** if and only if issue `N` is closed **and**
> `artifacts/atoms/<N>/closure.json` exists on the default branch `main` at the commit under
> evaluation **and** that record validates against
> [`../../artifacts/schema/closure-record.schema.json`](../../artifacts/schema/closure-record.schema.json).

An Atom is **ready** when every issue in its native `blocked by` list is satisfied under that
rule. A closed issue with no validating closure record is a defect: `gordian-derive-status ready`
exits non-zero when it finds one, rather than treating the Atom as done. Project 9's `Status` and
`Wave` are a projection of that command's output, and it is the only sanctioned way to pick the
next Atom ([`agent-runbook.md`](agent-runbook.md) section 6.2).

## Bootstrap specification-revision identity

Evidence binds to an exact specification revision, and during bootstrap an Atom's specification is
its issue body. The **bootstrap spec revision of Atom N** is the SHA-256 of the LF-normalized
snapshot at `artifacts/atoms/<N>/spec.md`, captured at claim time and committed:

```bash
gh issue view <N> --repo kmosoti/gordian --json body --jq .body \
  | tr -d '\r' > artifacts/atoms/<N>/spec.md
sha256sum artifacts/atoms/<N>/spec.md
```

`tr -d '\r'` is the LF normalization the definition requires, not an optional tidy-up: `gh issue
view` returns GitHub bodies with CRLF, and `sha256sum` over the raw capture is a digest nobody can
reproduce.

The closure record carries that digest in its `spec_digest` field.
`scripts/check-spec-digests.sh` recomputes the digest of every committed
`artifacts/atoms/*/spec.md`, rejects a snapshot that still contains a CR byte, and fails when the
digest disagrees with the paired closure record, so an issue body edited after the claim is a
visible event rather than a silent one.

## Adding or splitting an Atom

A new or split Atom is not registered until every one of these is true. Each row names the exact
edit or the exact command:

| Artifact | What must happen |
| --- | --- |
| GitHub issue | Created from [`.github/ISSUE_TEMPLATE/atom.yml`](../../.github/ISSUE_TEMPLATE/atom.yml) or `experiment.yml`, with the `[Initiative] Title` prefix convention. |
| Milestone | Set to exactly one of the 14 Initiatives. An Atom with no milestone has no Initiative. |
| `type:*` label | `type:atom` or `type:experiment`; this is the board's `Work Type`. |
| Native edges | `addBlockedBy` for every prerequisite, in **both** directions of the change (the new Atom's blockers and anything it now blocks). |
| `## Dependencies` prose | Rewritten on **both** endpoints of every edge added or removed, to mirror the native graph. |
| This register | The Initiative row gains the explicit issue number, and the registered-Atom count is re-derived. |
| [`project-plan.md`](project-plan.md) | The Initiative's Atom table gains a row, with its target crate from [`crate-map.md`](crate-map.md). |
| [`execution-order.md`](execution-order.md) | The Atom appears in exactly one phase's `Members` list (unordered); if it changes the longest path, section 4's spine is re-derived. |
| [`crate-map.md`](crate-map.md) | If the Atom writes Rust, it appears in one or more owning-Atoms crate rows as declared by the target-crate contract, and the issue body names those crate paths (G-517). |
| Benchmark obligation | If the Atom owns a row of [`execution-order.md`](execution-order.md#17-critical-performance-suite) section 17, its body carries a `## Benchmark obligation` section naming each `EO17-*` id it owns (G-475). |
| `knowledge/graph/90-project-plan.jsonld` | The Atom node and its relations are added. |
| Project 9 | The issue is added to the board and the derived fields are recomputed. |
| Closure check | `closure(#49)` and the orphan-coverage rule of `execution-order.md` section 15 are re-verified by `scripts/check-selfhosting-closure.sh`. |

**G-527 and G-445 are assigned to #70**, which owns the `new-atom`, `add-edge`, and `check-drift`
subcommands that make this table a command rather than a checklist, the generator that emits the
derived registers between markers, and the companion `gordian-milestone-contracts check` and
`gordian-milestone-contracts sync` commands for Initiative milestone descriptions. Every omission
is a drift defect.

## Atom completion rule

Closing a GitHub issue is bookkeeping. Closure is the record at
`artifacts/atoms/<N>/closure.json`, whose required contents are defined once by
[`../../artifacts/schema/closure-record.schema.json`](../../artifacts/schema/closure-record.schema.json)
and whose procedure is [`agent-runbook.md`](agent-runbook.md) section 2. This document previously
carried a prose list that differed from the one in `AGENTS.md`; both now point at the schema, so
there is one normative list and no second one.

A native Gordian Atom is satisfied only when its Candidate has been **admitted** into the accepted
frontier as part of an integration candidate whose verification discharged the Atom's manifest —
not merely when a verifier passed in a workspace
([`../spec/mission-graph.md` `## Logical state predicates`](../spec/mission-graph.md#logical-state-predicates)).
