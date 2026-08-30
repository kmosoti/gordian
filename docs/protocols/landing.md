# Landing Protocol

Status: **normative research specification**

Gordian's goal is execution by an autonomous agent with no human in the loop. The step where a
finished candidate reaches the shared remote was previously undefined: no document in the
repository contained `git push`, mentioned branch protection, or described a pull request as
Gordian's own landing mechanism, and the bootstrap document reserved publication to "an explicit
coordinator/human action". This document defines that step.

## 1. Authority

Publication is reserved to an actor holding the `move_accepted_frontier` capability — the
**coordinator role**, which may be a software agent. It is a capability check, not a human check.
A Worker MUST NOT hold it **by default** ([`../spec/invariants.md` `## Worker authority boundary`](../spec/invariants.md#worker-authority-boundary)).

**Bootstrap exception.** Until the coordinator of [`#38`](https://github.com/kmosoti/gordian/issues/38)
exists, one agent run MAY hold both roles for a single Atom, and only when it has run the full
[`../implementation/agent-runbook.md` section 6.6](../implementation/agent-runbook.md) verifier
list against the exact state it is about to publish, and recorded each result. The exception is
scoped to that one publication; it is not a standing grant, and it expires when `#38` closes.
This mirrors the `by default` qualifier the invariant already carries — it does not weaken the
capability check, it names who holds the capability during bootstrap.

The bootstrap script adds and fetches `origin` and does not push. Pushing is a coordinator
operation performed through the source adapter's `move_frontier`, never by an ad hoc shell
command issued from outside the adapter.

## 2. Landing sequence

For an `IntegrationCandidate` `I` admitted at frontier sequence `n + 1`:

0. **Verification happens before admission, on a non-frontier ref.** The coordinator calls
   `stage(I.exact_state_id, refs/gordian/staging/<batch-id>)` and then
   `verify(I.exact_state_id, integration_manifest(I))` through the adapter. Those results are the
   `Evidence` records `Verified(I)` reads. This step is what makes the sequence executable: the
   verifiers cannot run on the accepted frontier before the state is admitted, and the state
   cannot be admitted before the verifiers run.
1. The coordinator appends `CandidateAdmitted` under a `FrontierVersion` precondition and a
   `WitnessGuard`. That append **is** the accepted-frontier compare-and-swap.
2. `move_frontier(expected = t, new = I.exact_state_id)` moves the local accepted bookmark. It is
   idempotent and returns `Committed` or `AlreadyAtNew`.
3. `publish_frontier(expected = t, new = I.exact_state_id)` publishes it. The adapter owns the
   command; for the Jujutsu adapter it is `jj git push --bookmark main --allow-new`. The push is a
   projection of the log, exactly like the local bookmark move, and it is an adapter operation
   precisely so that **crash recovery re-drives it**. A rejected push means the remote has
   advanced and is handled by appending `FrontierDivergenceObserved { source: published_bookmark }`
   and running the divergence reconciliation of
   [`../algorithms/evidence-and-admission.md#frontier-divergence-reconciliation`](../algorithms/evidence-and-admission.md#frontier-divergence-reconciliation);
   it never re-runs the CAS.
4. `FrontierMoved`, the per-Atom `AtomSatisfied` events, and the `CandidateClaimReleased` events
   are appended as **one conditional transactional append**.
5. If step 2 or 3 fails permanently, or exceeds `MAX_REDRIVE_ATTEMPTS`, the coordinator calls
   `reset_frontier(to = t, scope = Both)` and only then appends `AdmissionAborted`, so admission
   is neither deadlocked by the standing intent nor by a bookmark left ahead of the log.

### 2.1 Bootstrap sequence (until #29, #30 and #19 land)

Section 2 is written in adapter and event-log operations. **None of them exist yet** — the
workspace has one crate, no `SourceAdapter`, no event log and no compare-and-swap. Until
[`#29`](https://github.com/kmosoti/gordian/issues/29) (adapter),
[`#30`](https://github.com/kmosoti/gordian/issues/30) (workspace/change lifecycle) and
[`#19`](https://github.com/kmosoti/gordian/issues/19) (admission/CAS) close, a coordinator
executes this literal sequence instead, from a workspace whose candidate is already frozen:

```bash
set -euo pipefail
CAND="${1:?exact_state_id (commit id) of the frozen candidate}"
BASE="$(jj log --no-graph -r 'trunk()' -T 'commit_id')"   # the t of section 2

# 0. verify on a non-frontier ref — the stage() of step 0
jj new "$CAND" -m "stage: verify $CAND"
bash scripts/verify-formal.sh && cargo test --locked --workspace   && cargo run --locked -p gordian-kg -- audit --strict   && for s in scripts/check-*.sh; do bash "$s"; done

# 1-2. the CAS stand-in: refuse if trunk moved under us, then move the local bookmark
test "$(jj log --no-graph -r 'trunk()' -T 'commit_id')" = "$BASE"   || { echo "frontier advanced; re-reconcile"; exit 75; }
jj bookmark set main -r "$CAND"

# 3. publish
jj git push --bookmark main || {
  echo "remote advanced; re-reconcile against the new trunk()"; jj git fetch; exit 75; }
```

**Which native steps are skipped, and what stands in:**

| Section 2 step | Bootstrap stand-in | Restored by |
| --- | --- | --- |
| 0 `stage` + `verify` | verify in a scratch change off the candidate | `#29` |
| 1 `CandidateAdmitted` under `FrontierVersion` | the `trunk()`-unchanged test above | `#19` |
| 2 `move_frontier` | `jj bookmark set main -r <candidate>` | `#29` |
| 3 `publish_frontier` | `jj git push --bookmark main` | `#29` |
| 4 transactional append | the closure record of runbook section 6.8 | `#12`, `#19` |
| 5 `reset_frontier` / `AdmissionAborted` | `jj bookmark set main -r <BASE>` then report | `#19` |

Exit 75 means *the frontier moved*: fetch, reconcile onto the new `trunk()`, re-verify, retry.
It is never a reason to force the push. The stand-in is deliberately weaker than the real CAS —
it is a check-then-act with a small race — which is exactly why it is scoped to the single-agent
bootstrap and why `#19` is in the minimal self-hosting set.

The only bookmark this sequence pushes is `main`. A worker's own bookmark is
`gordian/atom-<N>/<actor-slug>`
([`jujutsu-development-environment.md` `## 6. Development topology`](jujutsu-development-environment.md#6-development-topology)),
is a `stage` ref, and never becomes the frontier.

## 3. Verifiers and CI status

`project_integration_verifiers` for this Project during bootstrap names **adapter-executable
verifiers**, not GitHub Actions job names:

```text
verifier:rust-check        cargo fmt --check; cargo clippy -D warnings; cargo test --locked
verifier:kg-audit          cargo run --locked -p gordian-kg -- validate; ... -- audit --strict
verifier:formal            lake build; leanchecker; axiom-audit
verifier:python            ruff; python -m compileall; python -m unittest
verifier:spec-consistency  every scripts/check-*.sh
```

Each runs through `verify(state, manifest)` against `I.exact_state_id` in an isolated workspace at
step 0, before step 1, and each produces an `Evidence` record bound to that exact state. A
verifier that cannot be executed by the adapter cannot be a member of
`project_integration_verifiers`.

`.github/workflows/verify.yml` runs the **same commands** on `push` and `pull_request` as a
post-hoc projection: it is a second, human-visible execution of the same verifiers, useful for
review and for catching a divergence between the coordinator's environment and a clean one. Its
job-level check names are `Rust substrate`, `Formal models and proofs`,
`Thin Python orchestration`, and `Specification consistency`
(the workflow name `verify` is not a check name).
It is **not** the source of the evidence admission reads, and it cannot be: `verify.yml` triggers
on `push: [main]` and `pull_request: [main]`, so `I.exact_state_id` — a local integration state
that reaches the remote only at step 3 — is invisible to it until after the admission that step 3
follows. Requiring green CI jobs on that state before step 1 would require admitting before
verifying, or pushing before being permitted to admit. `stage` exists so that a deployment which
genuinely wants an external runner can fetch the staged ref and report back as an ordinary
external verifier, still before step 1.

`scripts/check-integration-verifiers.sh` asserts that every member of
`project_integration_verifiers` names an adapter-executable command and that no member is a
workflow job name.

## 4. Pull requests

A pull request is **optional** and is a review projection, not the admission path. When one is
opened it MUST NOT be the compare-and-swap: the log append is.
[`../../.github/PULL_REQUEST_TEMPLATE.md`](../../.github/PULL_REQUEST_TEMPLATE.md) carries the
closure-evidence checklist so a human reviewer sees the same evidence the coordinator checked.

A direct push by a coordinator holding `move_accepted_frontier` is permitted and is the default
autonomous path. Branch protection, if enabled, MUST admit that actor; a configuration that makes
the autonomous path impossible is a defect in the deployment, not in this protocol. Where a
deployment does gate `main` on a pull request, the merging actor is the same coordinator role and
the required checks are the four job-level names in section 3. The closure-record validator is
not a separate check: `scripts/check-closure-records.sh` runs inside the **Specification
consistency** job, so requiring it by name would name a check that never reports;
the merge is still a projection of the step-1 append, never a second decision point. The bootstrap
authority table that names the concrete actor or token for this repository is
[`../implementation/agent-runbook.md`](../implementation/agent-runbook.md).

## 5. Supersedes jujutsu-development-environment.md section 8

The bootstrap adds and fetches `origin` but never pushes. Publication is an operation of an
actor holding the `move_accepted_frontier` capability, performed through the source adapter's
`move_frontier` and the landing sequence above. That actor may be a software agent; no human
step is required.
