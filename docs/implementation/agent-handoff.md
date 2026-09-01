# Gordian — autonomous implementation and orchestration

You are implementing **Gordian**, a Rust-first, evidence-governed engineering coordination
substrate, in `~/projects/project-management-tools/gordian` (GitHub: `kmosoti/gordian`).

Work **conservatively and accurately**. This repository's own doctrine is that unverified
claims are worse than unfinished work. Prefer doing less and proving it to doing more and
asserting it.

---

## 1. Read these first, in this order

1. `docs/implementation/agent-runbook.md` — **the operating loop. This is your controlling
   document.** Sections 6.1–6.10 define actor identity, readiness derivation, claiming, spec
   snapshot, workspace, verification, landing, closure, board update, knowledge-graph update.
2. `AGENTS.md` — the canonical agent contract and the Rust/Python/Lean boundaries.
3. `docs/implementation/execution-order.md` — the causal spine, gates, and the minimal
   self-hosting prerequisite set.
4. `docs/protocols/landing.md` and `docs/protocols/jujutsu-development-environment.md`.
5. Only then, the specs you need for your Atom: `docs/spec/{mission-graph,data-model,invariants}.md`,
   `docs/algorithms/*.md`.

Do not start work before you can state, in one sentence each: what an Atom is, what
`Satisfied` requires, and why board status is not evidence.

---

## 2. Ground truth, and what is *not* ground truth

| Question | Authoritative source |
|---|---|
| Which Atoms exist, what each requires | GitHub issues `kmosoti/gordian` (77 open) |
| What blocks what | GitHub **native `blocked by`** relationships (305 edges) |
| Which Initiative an Atom belongs to | GitHub **milestone** (14) |
| Whether an Atom is done | A **closure record** validating against `artifacts/schema/closure-record.schema.json`, plus its verifiers passing on the exact state |
| Semantics of anything | `docs/spec/`, `docs/algorithms/` |

**Not** ground truth: GitHub Project 9's `Status`, `Wave`, `Fan In`, `Fan Out`. Those are
**derived projections** — recompute them, never reason from them, never hand-edit them.
An issue being closed is bookkeeping, not evidence.

Current state as of handoff: **77 open Atoms, 305 dependency edges, acyclic, max wave 18.**
**Ready now (zero open blockers): `#1`, `#2`, `#70`.** `#9` is gated on exactly
`{#2, #3, #4, #8, #71, #72}`.

---

## 3. Version control: Jujutsu, not git

`jj` is the VCS. **Never run `git`.** The pinned version lives only in
`scripts/bootstrap-jj.sh` (`--install` to get it); no other document states a version.

**A second agent may be working in the default workspace.** Before editing anything:

```bash
# Resolve the exact base FIRST. jujutsu-agent-protocol.md requires an exact base commit;
# `main` is a moving name and may be stale.
jj git fetch
BASE="$(jj log --no-graph -r 'trunk()' -T 'commit_id')"

jj workspace add --name codex-atom-<N> --revision "$BASE" /path/to/scratch/ws-atom-<N>
cd /path/to/scratch/ws-atom-<N>
jj new "$BASE" -m "<Atom title>"

# Record BASE — it is the exact base your evidence binds to.
echo "$BASE" > "$GORDIAN_LOG_ROOT/atom-<N>/base.txt"
```

Work only in your own workspace. `jj op log` shows what other agents just did — check it if
files change under you. Release with `jj workspace forget <name>` when done (`cd` out first).

---

## 3.5 If your sandbox has no network

Codex's `workspace-write` sandbox denies outbound connections by default. That presents as
`gh api` failing to reach `api.github.com` while your credentials are perfectly valid — it is a
**transport** failure, not an auth or scope failure, and no amount of `gh auth refresh` fixes it.
Enable it once in `~/.codex/config.toml` (**restart Codex afterwards**):

```toml
[sandbox_workspace_write]
network_access = true
```

**You can work productively even with no network.** `artifacts/atoms/issues.json` is committed:
every open Atom's contract, milestone, labels and native blocked-by edges. Readiness derives
offline, with no credentials:

```bash
gordian-derive-status --snapshot artifacts/atoms/issues.json ready
```

Offline you can do everything except three steps: **claiming** (§6.3), **landing** (§6.7) and the
**board update** (§6.9). Derive readiness, read the contract, implement, run all five verifiers,
and write the closure record — then hand those three steps to a connected environment. Say plainly
in your report which steps are pending rather than reporting the Atom as done.

Never fake a device code, never bypass `gh`, and never treat a transport failure as a scope
problem. If `gh auth status` succeeds but `gh api user` cannot connect, it is the sandbox.

---

## 4. The loop

Follow `agent-runbook.md` §6 exactly. In outline:

1. **Derive readiness** (§6.2) from the native blocked-by graph — an Atom is Ready iff it is
   open with zero open blockers. Never infer readiness from the board.
2. **Claim** (§6.3): assignee + board `Status = In Progress` + a claim comment. All three.
3. **Snapshot the spec** (§6.4) — record the spec digest you are implementing against.
4. **Workspace** (§6.5) from an exact base.
5. **Verify** (§6.6) — run the named verifiers: `verifier:rust-check`, `verifier:kg-audit`,
   `verifier:formal`, `verifier:python`, `verifier:spec-consistency`.
6. **Land** (§6.7) per `docs/protocols/landing.md`.
7. **Closure record** (§6.8, §2) → `artifacts/atoms/<N>/closure/…`, validating against the
   schema. Required: `record_format, atom_id, spec_digest, actor, exact_state_id,
   logical_change_id, verifiers, benchmarks, knowledge_graph_node_ids, known_limitations,
   closed_at`.
8. **Board update** (§6.9) and **knowledge graph** (§6.10).
9. On failure, §7: record the attempt, release the claim in reverse, discard the workspace.

**Take one Atom at a time.** Do not batch. Do not start a second Atom before the first has a
closure record.

---

## 5. Hard constraints

- **Never land anything whose verifiers you have not run against the exact state being pushed.**
  CI green after the fact is not the gate.
- **Never invent semantics.** If a spec does not define a predicate you need, stop and say so —
  do not guess. The specs were audited precisely to remove guesswork; a gap you find is a real
  finding, not an invitation.
- **Rust owns production semantics.** Python is orchestration only — it must never decide ready,
  dispatchable, fresh, satisfied, authorized, or accepted. Lean models narrow obligations.
- **No `unsafe`** (workspace-forbidden). Rust 1.98, edition 2024.
- **Formal work must stay green**: `cd formal && lake build` with zero `sorry` and no new axioms.
- **Do not weaken a verifier or a check script to make something pass.** If a check is wrong,
  say so and stop.
- **Do not close an issue** without its closure record.
- Do not push to `main` if branch protection or a ruleset says otherwise; open a PR instead.

---

## 6. Verification you must run before claiming any Atom done

This is `agent-runbook.md` §6.6 verbatim. Run it as a script — **do not** hand-edit the commands,
and **never** write `|| echo`, which turns a failed check into a passing loop.

```bash
#!/usr/bin/env bash
set -euo pipefail
shopt -s nullglob          # an empty scripts/check-*.sh glob must yield nothing, not a literal
fails=0
run() { printf '\n=== %s\n' "$*"; "$@" || { echo "FAILED: $*" >&2; fails=$((fails+1)); }; }

# Preflight. A missing tool means the verifier did not run — which is NOT the same as passing.
# Exit 78 (configuration missing), the same code the credential probes use, so the two are
# never confused with a real verifier failure.
for t in cargo lake python3 ruff shellcheck; do
  command -v "$t" >/dev/null || { echo "MISSING TOOL: $t — see agent-runbook.md section 0" >&2; exit 78; }
done

# verifier:rust-check
run cargo fmt --all -- --check
run cargo clippy --locked --workspace --all-targets -- -D warnings
run cargo test --locked --workspace

# verifier:kg-audit
run cargo run --locked -p gordian-kg -- validate
run cargo run --locked -p gordian-kg -- audit --strict

# verifier:formal   (lake build + banned-token scan + axiom-closure audit)
run bash scripts/verify-formal.sh

# verifier:python
run ruff check orchestration
run python3 -m compileall -q orchestration/src
run env PYTHONPATH=orchestration/src python3 -m unittest discover -s orchestration/tests

# verifier:spec-consistency
checks=(scripts/check-*.sh)
(( ${#checks[@]} )) || { echo "FAILED: no scripts/check-*.sh found" >&2; fails=$((fails+1)); }
for s in "${checks[@]}"; do run bash "$s"; done
run shellcheck scripts/*.sh

(( fails == 0 )) || { echo "$fails verifier(s) FAILED" >&2; exit 1; }
echo "all verifiers passed"
```

`--locked` is mandatory (`AGENTS.md`): a verifier that silently updates the lockfile is not
verifying the state you are about to publish. Baseline on `main` at `ec459100`: 23 Rust tests,
68 Python tests, `verify-formal.sh` green over 49 theorems, KG 345 nodes / 778 edges with
`audit --strict` clean, 24/24 check scripts. If your numbers differ, something changed — find out
what before proceeding.

---

## 7. Where to start — this is the authorized scope

**Work Atoms, in this order. Nothing else is authorized without asking.**

1. **`#2`** — stabilize Rust, Lean, Python, CI. Highest fan-out; unblocks the most work.
2. **`#1`** — qualify and pin the Jujutsu baseline. The only thing gating the source plane.
3. **`#70`** — bootstrap bookkeeping; independent, do it whenever.
4. Then recompute readiness and take whatever is Ready, preferring higher `Fan Out`.

Do **not** start `#9` (the Rust Mission Graph kernel) until all six of its blockers —
`#2, #3, #4, #8, #71, #72` — have closure records. That gate was deliberately narrowed; do not
widen or bypass it.

There is no separate "fix the gaps first" phase. Everything in §8 is either already closed or
carried inside an Atom's contract, so **an Atom's issue body is the full statement of what you are
authorized to do.** If work seems necessary but no Atom covers it, that is a finding — report it
under §9 rather than widening scope.

---

## 8. What was fixed, and what is still known-open

A seven-node adversarial review of this repository found eleven blocking items. All were closed on
2026-08-30 and are **on `main` at `ec459100` with CI green** — verify, do not redo:

- **The GitHub plan is applied**: 77 Atoms, 305 native edges, acyclic, 0 body↔graph mismatches,
  77 board items, 77/77 milestones. `check-selfhosting-closure.sh` reports 43 Atoms, no orphan.
- **Credentials are non-interactive** (`agent-runbook.md` §6.1 *Credentials*): `GH_TOKEN`,
  `GIT_ASKPASS`, and four scope probes that exit 78 rather than prompt. `gh auth login` and
  `gh auth refresh` are forbidden inside the loop.
- **Landing is executable** (`landing.md` §2.1): a bootstrap sequence of concrete `jj` commands
  for the period before the adapter, event log and CAS exist, with a table naming every
  stood-in step and the Atom that restores it.
- **Authority resolves**: `landing.md` §1 carries the `by default` qualifier the invariant always
  had, plus a bootstrap exception scoped to one Atom, expiring when `#38` closes. §4 no longer
  requires a "closure-record validator" check that is not a job.
- **`verifier:formal` is runnable**: `scripts/verify-formal.sh` — `lake build`, a banned-token
  scan, and an axiom-closure audit over all 49 theorems. Negative-tested against an injected
  `sorry`.
- **Verifier logs are written outside the workspace** (`$GORDIAN_LOG_ROOT/atom-$N/`); writing them
  inside was auto-snapshotted by jj, changing `exact_state_id` and breaking the freeze they attest
  to.
- **The stop condition is machine-reachable**: `project-plan.md` declares the two metrics with no
  machine substitute (`#50` operator-comprehension, `#54` manual-correction count) as
  `unresolved_human_metric`, and `check-mission-stop-condition.sh` honours them. `--gate` exits
  non-zero while real Atoms remain.
- **`#70` carries its ten parked gaps** (G-413, G-433, G-445, G-475, G-502, G-507, G-518, G-522,
  G-527, G-609) and a closure block that parses.
- **`#1`'s closure block runs**: it no longer calls `cargo test -p gordian-source`, a crate that
  does not exist and that the crate map assigns to `#29` — which `#1` blocks.
- **Branch protection exists**: ruleset `gordian-main` requires the four job-level checks
  (*Rust substrate*, *Formal models and proofs*, *Thin Python orchestration*, *Specification
  consistency*), blocks deletion and non-fast-forward, with the coordinator role as bypass actor.

**Still open, and honestly so:** `#70`'s G-502 has an ordering constraint — do not commit the
`artifacts/atoms/` contract snapshot before verifying the GitHub plan is applied, or
`check-selfhosting-closure.sh` computes 45 with 10 orphans and fails `verifier:spec-consistency`
for *every* Atom. Roughly 40 register gaps remain, each owned by an Atom's contract. Four
non-blocking defects are recorded in the gap register: the two closure-record conventions, `#1`'s
self-contradictory benchmark obligation, the readiness rule wording, and `#69`'s taste-laden
acceptance.

**Gordian is now internally consistent, mechanically checkable, and bootstrappable. It is not
finished** — that is what the 77 Atoms are for.

---

## 9. When you are blocked

Stop and report. Say precisely: which Atom, which step of §6, what you needed, what the
documents say, and what the two possible readings are. Do **not**:

- guess at semantics and continue,
- widen your Atom's scope to route around the blocker,
- mark something done that is not,
- or silently skip a verifier.

A clean stop with a precise question is a good outcome. A plausible-looking implementation built
on an invented definition is the failure mode this entire repository exists to prevent.

---

## 10. Reporting

After each Atom, report in this shape:

```
Atom:        #N — <title>
Spec digest: <digest you implemented against>
State:       <exact_state_id>
Verifiers:   <each, pass/fail, on which state>
Closure:     <path to closure record>
Board:       <what you set>
Limitations: <what you did not do and why>
Next Ready:  <recomputed from the native graph>
```

Report failures with the same rigor as successes, including the verbatim failure output.
