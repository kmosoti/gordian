<!-- Canonical location of the Mission goal. Previously an untracked GOAL.md in a single
     working copy: no history, no review, lost with the workspace. -->

# Mission goal — complete the Gordian Atom backlog

Execute every open Atom in `kmosoti/gordian` to a validating closure record, and fix forward.
This is GOAL v2: it supersedes the serial v1. What changed: **bounded parallel execution**,
**explicit tier delegation**, and a **binding harness-improvement contract**.

## Read first

1. `CODEX-PROMPT.md` — ground truth, constraints, verification block, reporting shape.
2. `docs/implementation/agent-runbook.md` — the controlling per-Atom loop (§6).
3. `~/.codex/AGENTS.md` — tiers, severity policy, parallelism rules, self-improvement contract.

## Where the previous session stopped (nothing was lost)

No Atom has landed on `main` yet (`main` = `780c6a9e`) and no closure record exists. Two
workspaces hold real work, both durable in the jj store:

| Atom | jj change | Workspace | State |
| --- | --- | --- | --- |
| #70 | `kmtznoky` | `/tmp/gordian-atom70.B0ecVq/workspace` | large normalization/registry build-out, pre-dates current `main` — **rebase onto `main` first**; where `main` already solved something, keep the version with the test and log what you dropped |
| #2 | `rvqoopxu` | `/tmp/gordian-atom2.v5iDx2/workspace` | toolchain checks + a formal-evidence writer in progress |

Resume both. Do not restart either from scratch.

## Credentials (unchanged, still mandatory first step)

```bash
export GH_TOKEN="$(XDG_CONFIG_HOME=/mnt/d/DevConfig gh auth token)"
gh auth status && gh project list --owner kmosoti --limit 1 >/dev/null && echo "credentials OK"
```

Never `gh auth refresh`. A transport failure is never a scope problem.

## Parallel execution model

The write tier is cheap and fast, so the write plane is wide: **up to 10 concurrent luna
(`write`-profile) workers on unblocked, decided work. Anything that is not both unblocked and
decided is queued — visibly, never silently dropped.** The graph already encodes independence:
any two *Ready* Atoms are causally independent by construction.

**Slot budget:**

| Plane | Cap | What runs there |
| --- | --- | --- |
| Write (luna, priority) | **10** | Decided work packets: implement, edit, scaffold, test-write |
| Initiative verify (`iverify`: sol xhigh, priority) | **1 per Initiative, ≤3 concurrent** | Managing that Initiative's luna completions: packet done-checks, bounded rework, Atom state coherence, handoff to landing |
| Deep (sol max) | 2 | Design, root-cause, packet decomposition for hard Atoms |
| Landing | **1 — always, a singleton** | Rebase onto fresh `trunk()`, FULL verifier block, push |

**Initiative verification singletons.** Every Initiative (= milestone) with an Atom in flight
gets exactly one `iverify` agent — enforce the singleton with
`flock -n /tmp/gordian-iverify-<milestone-slug>.lock`. It is the completion authority for its
Initiative's luna workers:

- validates each packet against the packet's named done-check (running the check itself, on the
  exact state — a worker's own claim of done is not completion);
- orders rework with a concrete defect statement, at most **2 rounds per packet**, then
  escalates the packet to `deep` and frees the luna slot;
- keeps Atom state coherent as packets land: claim facts still true, board Status honest,
  attempt records written on failures;
- when every packet of an Atom passes and targeted checks are green, hands the Atom to the
  landing queue with the exact commit id it verified — and only then;
- verifies its Initiative's already-closed Atoms still hold (closure record present,
  schema-valid, milestone consistent) each time it wakes.

It never lands, never runs `jj` mutations, never edits GOAL.md, and never touches another
Initiative's Atoms. Its verdicts always cite the exact commit id they were rendered on.

**Rules that keep 10-wide safe:**

1. **Luna executes decided packets only.** Before fan-out, the coordinator (or a `deep` pass for
   hard Atoms) decomposes a claimed Ready Atom into work packets with named files and a named
   done-check each. "Unblocked" is necessary but not sufficient — undecided work queues for a
   decide pass; it never goes to luna raw.
2. **One Atom = one jj workspace.** Several luna workers MAY share an Atom's workspace only on
   pairwise-disjoint file sets, and workers NEVER run `jj` — the coordinator owns all jj
   operations in that workspace. Cross-Atom, never share.
3. **Claim before work** (runbook §6.3: assignee + board Status + claim comment), machine-checked
   before any packet dispatches — Review 1's P1-PROCESS finding stands.
4. **Landing strictly serial, no exceptions at any width.** `main` is a single-writer frontier.
   On a push race: fetch, re-reconcile, retry once; never force.
5. **Verifier economics at width 10:** workers run *targeted* checks only (`cargo test -p
   <crate> --locked`, single scripts) — never `--workspace` builds, never `lake build`, never the
   full check-script loop. The FULL verifier block runs exactly once per landing, in the landing
   slot. If two workers' targeted builds thrash the machine, the coordinator halves the write
   cap and logs it (that is a legitimate Adaptive rule, not a gate change).
6. **Worker crash = attempt record + claim release** (runbook §7); re-queue at most twice, then
   park with a report.

**Anti-stall rules (added 2026-09-01 after a diagnosed stall).** Telemetry over one
1,716-event window found: 175 file changes with 6 delegated sub-runs (ratio 0.03), one checker
re-run 48 times, that checker flipping pass/fail 75 times across 152 runs, and 924 reasoning
events for 175 changes — four hours, zero closures. These rules exist so that shape is
impossible:

7. **Delegation is mandatory, not available.** A packet that changes files is dispatched to a
   `write` worker. The coordinator may edit files only for: a one-line fix inside a packet it is
   already verifying, the closure record, or the board. Coordinator `fileChange` beyond those is
   smell **S1** and a P1-PROCESS. The write plane existing but unused is the most expensive
   failure mode available to this Mission.
8. **Rework is capped at two.** After **two** failed runs of the same command or gate on the same
   packet, STOP. A third identical attempt is not work: escalate to `deep` for a different
   approach, or abandon the packet with an attempt record. Detector **S2** flags repetition; six
   is a P1-PROCESS.
9. **A gate that flips is not a gate.** If a checker's pass/fail changes more than twice within
   one Atom, freeze the packet and fix the *gate* first — its subject is unknowable while the
   instrument is unstable (**S5**).
10. **Never trust a green exit alone.** A step whose output contains `command not found`, `No
    such file or directory`, or a traceback has FAILED regardless of exit code (**S3**). Gate
    scripts carry `set -euo pipefail`; `scripts/check-script-safety.sh` enforces it and is in the
    landing verifier block.
11. **Write the slot line.** Every parallel-set change prints
    `slots: write x/10, deep y/2, landing z/1 | queue: [...]`. Its absence is an A4 finding — the
    reviewer cannot score throughput it cannot see.

**Queue discipline:** everything not in a slot sits in one ordered queue — Ready Atoms by
descending `Fan Out`, then decided packets FIFO within their Atom. Every slot-state change
prints: `slots: write x/10, deep y/2, landing z/1 | queue: [...]`. Blocked Atoms are not queued;
they are simply not Ready (`gordian-derive-status --snapshot artifacts/atoms/issues.json ready`;
refresh via `scripts/snapshot-atoms.sh` after any closure). Expect the write plane to sit far
below 10 until the foundation Atoms land and the Ready set widens — an empty slot with an empty
queue is correct, not idle-waste. #9 stays untouchable until `#2, #3, #4, #8, #71, #72` all have
closure records.

## Identity ownership — who owns what

One owner per identity. Anyone else reads it; nobody else writes it. A verdict, record, or
handoff that names an identity outside this table is a process defect (A5).

| Identity | Owner | Everyone else |
| --- | --- | --- |
| jj **change id** (an Atom's evolving implementation) | **Coordinator** — creates it with the workspace at claim time, describes it, abandons or hands it off | Workers edit files only; `iverify` and landing read it |
| jj **commit id** (exact immutable state) | Produced by the **coordinator's** jj snapshots; **`iverify` owns verdicts on it**; **landing owns the final landed one** | A verdict or closure record citing a commit id nobody snapshotted is fabrication |
| Workspace (one per Atom) | **Coordinator** — add, `jj` operations inside it, forget | Luna workers write files in it on disjoint sets, never run `jj` |
| Bookmark `main` (accepted frontier) | **Landing singleton only** | Nobody else moves or pushes it, ever |
| Staging refs `gordian/atom-<N>/…` | **Coordinator** | read-only to others |
| Claim (assignee + Status + comment) | **Coordinator** — makes it, releases it | `iverify` re-checks it; workers require it |
| Work packets + the queue | **Coordinator** (decomposition may be delegated to `deep`) | `iverify` marks packets done/rework |
| Packet completion verdicts | **`iverify`** (its Initiative only), citing the exact commit id | Workers' own "done" is a claim, not completion |
| Closure record | **Coordinator** writes it (`recorded_by`), `actor` = the executing actor string, bound to the exact commit id `iverify` verified and landing landed | `iverify` validates it against the schema |
| Board fields (Status/Wave/Fan In/Fan Out) | **Coordinator** — derived projections, recomputed | Never hand-edited; reviewer reads |
| GOAL.md *Adaptive rules* | **Mission coordinator** (append-only) | Reviewer proposes; humans everything |
| `orchestration-reviews.md` | **Retro reviewer** (append-only) | Mission reads; acts on P1-PROCESS |
| GitHub issue bodies / native edges | **Coordinator**, only per an Atom's contract or runbook §8 | — |

Change id vs commit id, one line: **workers and packets progress on a change id; every
verification, verdict, and closure binds to a commit id.** A change id names what is evolving;
a commit id names what was true when someone checked.

## Tier delegation (do this, not just know it)

| Phase | Profile | Why |
| --- | --- | --- |
| Coordination loop, claims, board, sequencing | `coord` (your session) | cheap, frequent |
| Atom design, root-cause on a failed verifier | `deep` | wrong here is expensive |
| Managing an Initiative's luna completions | `iverify` | singleton per Initiative; priority tier — it sits on the workers' critical path |
| Pre-landing review of a worker's diff | `verify` | never self-certify at the tier that wrote it |
| Worker implementing a decided change | `write` | high-volume, priority service tier |

Concretely: `codex exec --profile write -C <workspace> "<decided change>"`. Escalate to `deep`
on evidence — an irreversible decision, a verifier disagreeing with an expectation, or a second
consecutive failure on the same task. Log every escalation and what it cost.

## Quality bar (the 99% clause)

- **No claim without the command output that proves it.** "Tests pass" means you ran them, on
  the exact state, and the exit code was read directly — never through a pipe.
- **Grep matches are not findings.** Confirm a hit is semantic before acting on it (the
  Lean-toolchain filename `UnreachableTactic.lean` is not a network error).
- **Negative-test every checker you add** by injecting the defect it must catch.
- **Fail closed.** A tool you could not run is a verifier that did NOT pass (exit 78 discipline).
- **Closure records are the only doneness.** Schema-validated, verifiers bound to the exact
  state. Closing an issue is bookkeeping.

## Fix forward

P1 (wrong, breaking, blocking, misleading) → fix now, same pass. P2 (real, not
quality/delivery-affecting) → one line in `docs/backlog.md`, move on. Undefined semantics → stop
and present both readings; never invent.

## Harness self-improvement (binding, not aspirational)

After **every** Atom closure *and* every failed attempt, append to
`docs/implementation/orchestration-log.md`:

```
## <atom or attempt id> — <date>
tokens_est: <coordination / deep / write, rough>   wall: <min>   retries: <n>
verifier_failures_caught_late: <n and which>
hallucinated_reference: <API/file/flag that did not exist, or none>
tier_mismatch: <where a cheaper tier would have sufficed, or none>
rule: <ONE actionable rule this experience yields>
```

Promotion contract: when the same `rule:` appears **twice**, promote it — append it to the
**Adaptive rules** section below in this file, and follow it thereafter. You own that section;
you may not weaken verifiers, gates, the landing serialization, or the claim protocol. The
measure of success: by ten Atoms, retries/Atom and late-caught failures are visibly falling. If
they are not, that itself is a `deep`-tier root-cause task.

## Process smells — counted, not opined

Deterministic detectors S1–S5 (no-delegation, command loop, green-but-broken gate, reasoning
drag, gate churn) are computed from telemetry and appear in every review. They are findings, not
opinions. A P1-PROCESS smell is resolved in the current pass, before new packets dispatch.

## Meta-review input

An independent reviewer (`retro` tier: sol / xhigh / priority) appends a scored review of your
execution to `docs/implementation/orchestration-reviews.md` every 30 minutes, across five fixed
axes (token economy, failure yield, hallucination rate, throughput/parallelism, process
integrity). At each scheduling pass, read the newest review: act on any **P1-PROCESS** proposal
in the current pass; treat other proposals as candidate rules for the promotion contract above.
You do not edit that file; it is the reviewer's.

## Adaptive rules (agent-owned; append-only)

<!-- promoted rules land here -->

- Prove the accepted graph and every generated mirror agree before delegating any snapshot-format rewrite.
- Verify the exact tool interface and refresh compiled test artifacts before trusting a negative test.
- Before dispatch, compile the normative format, checked-in schema, generated record, golden manifest, and independent negative oracle into one field-level contract.

## Done

`bash scripts/check-mission-stop-condition.sh --gate` exits 0. Nothing else terminates the
Mission. Report per Atom in the CODEX-PROMPT.md §10 shape, plus a one-line slot report each time
the parallel set changes (started / landed / parked, and why).
