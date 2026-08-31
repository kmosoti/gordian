# Agent Runbook

Status: **normative operating procedure**

This document is the loop an autonomous agent executes with no human in the loop. It is
deliberately short and mechanical; everything it references is defined elsewhere and checked by a
script.

Sections 1-5 are the **native** loop, stated against the Mission Graph substrate Gordian is
building. Sections 6-9 are the **bootstrap** loop an agent follows today, while GitHub issues,
milestones, and Project 9 stand in for that substrate. The bootstrap loop is the same loop with
GitHub primitives substituted for native records, and it is retired when #48 and #49 close.

## 0. Acquire the repository and toolchain

Section 0 has two blocks and they run in order. The first installs the two tools the second one
needs; the second is the **acquisition block** (G-510). Neither contains a personal absolute path.

Prerequisites on the machine itself: `curl`, `git`, and a C linker (`build-essential` on Ubuntu).
A GitHub Actions `ubuntu-latest` image carries all three.

```bash
# Toolchain prerequisites. rustup provides cargo; cargo is how jj is installed, and
# scripts/bootstrap-jj.sh refuses to run without it.
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
  | sh -s -- -y --profile minimal --no-modify-path
. "$HOME/.cargo/env"

# A jj good enough to clone with. No version literal appears here on purpose: the next
# block re-installs jj at the repository's pinned version.
cargo install --locked --bin jj jj-cli

# The two linters section 6.6 runs. Their versions are pinned to what CI installs, so a
# local verifier run and the CI job reach the same verdict; an unpinned ruff will disagree.
python3 -m pip install --disable-pip-version-check --user ruff==0.16.5
command -v shellcheck >/dev/null || sudo apt-get install -y shellcheck
```

`verifier:python` and `verifier:spec-consistency` invoke `ruff` and `shellcheck` by name. A
missing linter is a **configuration** failure, not a verifier failure: it means the verifier did
not run, which is not the same as passing. Section 6.6 preflights both and exits 78 — the same
"configuration missing" code as the credential probes in 6.1 — rather than reporting a check
result it never obtained.

Then the acquisition block:

```bash
jj git clone https://github.com/kmosoti/gordian "${GORDIAN_ROOT:-$HOME/gordian}"
cd "${GORDIAN_ROOT:-$HOME/gordian}"
bash scripts/bootstrap-jj.sh --install
```

`scripts/bootstrap-jj.sh` is the single source of the pinned Jujutsu baseline
(`DEFAULT_JJ_VERSION`); no document restates that version number, which is why the first block
installs an unpinned `jj` and the second corrects it. `--install` is idempotent: it re-runs
`cargo install --force` at the pin and then adds and fetches `origin`, tracks `main@origin`,
defines `trunk()`, and never pushes, rewrites, or promotes.
[`../protocols/jujutsu-development-environment.md`](../protocols/jujutsu-development-environment.md)
describes what it does and does not qualify.

After both blocks, `jj log -r 'trunk()' -n 1` must exit 0. That is the acceptance check for
section 0.

`cargo` invoked inside the clone reads `rust-toolchain.toml` and installs the pinned Rust channel
and its components on first use, so no Rust version literal belongs here either. The remaining
per-tool installs — Lean via `elan`, Python 3.14, and `gh` — have no single pinned command yet.
**G-528 is assigned to #2**, which owns `scripts/check-toolchain.sh` and the per-tool pinned
install commands; until it closes, an agent installs the versions named in
`formal/lean-toolchain` and `orchestration/pyproject.toml` by hand and records the observed
versions in the closure record's `known_limitations`.

**G-510's remaining conjunct is a CI job and is assigned to #1**: a job on a clean
`ubuntu-latest` runner with no pre-existing checkout that executes the two blocks above verbatim,
asserts exit 0, and then asserts `jj log -r 'trunk()' -n 1` exits 0.
`.github/workflows/verify.yml` does not carry that job today, so section 0 is verified by hand
until #1 lands it.

## 1. Loop

```text
1. pick             the highest-ranked Atom with Dispatchable(a) = true
                    (docs/spec/mission-graph.md "## Logical state predicates")
2. base             obtain an exact base satisfying PrerequisiteContaining
                    (docs/spec/mission-graph.md "## Stable snapshots")
3. lease            acquire write_exclusive on LeaseSubject::LogicalChange(...)
4. execute          in the isolated workspace only
5. freeze           record the Candidate, including its fencing_token
                    NOTHING in the workspace may be edited after this point
6. verify           run the required verifier manifest against the exact state
7. hand off         to the coordinator's admission queue
8. claim + batch    coordinator claims the candidate and assembles an IntegrationBatch
9. stage + verify   stage I on a non-frontier ref and run integration_manifest(I) on it
10. admit           ten-conjunct witness, FrontierVersion CAS, WitnessGuard
                    (docs/algorithms/evidence-and-admission.md)
11. land            docs/protocols/landing.md
12. closure record  the COORDINATOR writes artifacts/atoms/<N>/closure.json, after admission,
                    in its own bookkeeping change (artifacts/schema/closure-record.schema.json)
```

Steps 1-7 are Worker authority. Steps 8-12 require `move_accepted_frontier` and are Coordinator
authority. The separation is a capability check, not a human check.

**Why the closure record is written last, and by the coordinator.** Writing it at step 7 would
edit the workspace after the freeze, and [`../spec/data-model.md`](../spec/data-model.md)
`## Candidate` states that editing the same logical change after freeze creates a **new**
`Candidate` because `exact_state_id` has changed — so the handed-off state would not be the
verified state, `EvidenceFresh` would fail at admission, and every Atom closed by following the
runbook would be unadmittable. It would also be self-referential: `closure.json` records
`exact_state_id`, and the file would live inside the tree that `exact_state_id` names, so no value
could be correct.

The record is therefore authored **outside the verified subject**: after `AtomSatisfied` is
appended, the coordinator writes `artifacts/atoms/<N>/closure.json` in a separate bookkeeping
change whose own candidate carries no Atom's work. `exact_state_id` and `logical_change_id` in the
record name the **admitted** state of the Atom's work, which is an ancestor of the bookkeeping
change, so the value is well-defined and the file never hashes itself. `artifact_path` entries
point at verifier artifacts written into the same bookkeeping change **before** `closure.json`, so
`artifact_sha256` is computable and stable, and `scripts/check-closure-records.sh` never verifies
a digest of the record over itself.

## 2. Closure record

Every closed Atom has exactly one closure record, written by the coordinator after admission, at:

```text
artifacts/atoms/<N>/closure.json
```

It is never written into the Atom's own verified candidate: see section 1.

The record validates against
[`../../artifacts/schema/closure-record.schema.json`](../../artifacts/schema/closure-record.schema.json).
The record MUST be linked from the closing issue comment. `scripts/check-closure-records.sh`
validates every `artifacts/atoms/*/closure.json` against that schema and recomputes every
`artifact_sha256`, and fails the build on any schema or digest error. It is not a job or step of
its own: the **Specification consistency** job of `.github/workflows/verify.yml` runs it through
its `for s in scripts/check-*.sh` loop, so the required check name is `Specification consistency`
and there is no check named `closure-records`.

That schema is the single normative list of what closure contains.
[`issue-index.md`](issue-index.md) and [`../../AGENTS.md`](../../AGENTS.md) point here rather than
restating it. Walking it produces five questions it cannot answer alone, and this section fixes
each one so two agents produce the same record.

**`actor` is the executing worker; `recorded_by` is the coordinator that wrote the record.** They
are two fields precisely because section 1 has different actors do the two jobs. `actor.id` is the
`gordian-agent/<harness>/<run-id>` string of section 6.1 — the same string the claim comment opens
with and the `Gordian-Actor:` trailer on the Atom's own changes carries — with
`actor.kind = "agent"`, or `human:<identity>` with `kind = "human"` when a person executed the
Atom. `recorded_by` is the coordinator that authored the bookkeeping change, and it is omitted
when the coordinator and the executor are the same actor, which is the bootstrap default where one
agent run performs both roles.

**`exact_state_id` is the state the verifiers ran on, and the landed state must equal it.** The
coordinator lands by moving `main` to the verified candidate — a fast-forward `move_frontier` and
`publish_frontier`, [`../protocols/landing.md`](../protocols/landing.md) section 2 steps 2-3 — so
the admitted state id is the verified state id. If a deployment instead lands through a merge or
squash, the resulting commit is a **different exact state**: the coordinator MUST re-run the
verifier manifest against that state and record *that* state id and *those* artifacts. Recording
a verified id that no longer names the landed tree breaks `EvidenceFresh` and
`RequiredVerificationPasses`
([`../algorithms/evidence-and-admission.md`](../algorithms/evidence-and-admission.md#the-admission-conjuncts-defined)).

**`verifiers[].artifact_path` has one convention.** Each verifier's log is written to

```text
artifacts/atoms/<N>/verifiers/<verifier_id>.log
```

repository-relative, in the bookkeeping change, before `closure.json`. `verifier_id` is the
[`../protocols/landing.md`](../protocols/landing.md) section 3 identity without its `verifier:`
prefix — `rust-check`, `kg-audit`, `formal`, `python`, `spec-consistency` — plus one id per
Atom-specific verifier. `scripts/check-closure-records.sh` resolves `artifact_path` from the
repository root or from the record's own directory and recomputes its SHA-256, so an invented path
or a stale digest is a build failure rather than a silent one.

**`benchmarks` is required and an empty array is only correct with a stated reason.** An Atom
whose issue body carries a `## Benchmark obligation` section MUST either carry one entry per
obligation, with `experiment_id`, `run_id`, and `artifact_sha256` drawn from the experiment ledger
(#37, #75), or write `"benchmarks": []` **together with** a `known_limitations` entry of the exact
form

```text
benchmark obligation <EO17-id> deferred: experiment ledger (#37, #75) not yet available
```

naming each [`execution-order.md`](execution-order.md#17-critical-performance-suite) row id it
defers. An Atom with no benchmark obligation writes `[]` and no entry. An empty array with no
entry and an obligation on the issue is a defect, not a pass.

**`knowledge_graph_node_ids` lists the nodes the Atom created or changed**, in the form
`concept:…`, `algorithm:…`, `theorem:…`, `experiment:…`, and it is produced by section 6.10. An
Atom that genuinely touches no node writes `[]` and says so in `known_limitations`.

Closing a GitHub issue without a validating closure record is not closure.

## 3. Stop condition

The Mission loop terminates when artifacts/atoms/69/closure.json validates against the closure
schema and every row of the Mission acceptance table resolves to a validating closure record.

That is the whole stop condition. The Mission acceptance table is in
[`project-plan.md`](project-plan.md#mission-acceptance), and the agent asks the repository whether
it is done with:

```bash
bash scripts/check-mission-stop-condition.sh --gate
```

**`--gate` is the agent-facing invocation.** It prints the unsatisfied rows and exits non-zero
while any row is unsatisfied, so `if bash scripts/check-mission-stop-condition.sh --gate; then
stop; fi` is correct. The bare form is the CI and reporting mode: it prints the same rows and
exits 0 by design, so an incomplete Mission does not fail every build. An agent that tests the
exit status of the bare form concludes the Mission is complete on its first iteration.

No other signal terminates the loop: not "all 77 issues closed", because issues are a temporary
external projection; not "the milestone is closed", because a milestone is bookkeeping.

#### Human-judgment metrics are waived out of first qualification

Some acceptance rows name a metric only a person can produce — `#50`'s operator-comprehension
rating is the clearest case. A row whose metric is human-only would make the stop condition
unreachable by an unattended agent, so those rows carry:

```text
unresolved_human_metric: <one line naming the metric and why no machine substitute exists>
```

A row carrying `unresolved_human_metric` is **out of first qualification**: the stop condition
does not wait on it, and `check-mission-stop-condition.sh --gate` does not count it as
unsatisfied. In exchange the waiver is loud — `#69`'s evidence bundle MUST list every waived row
verbatim under *known limitations*, so a qualification that skipped human judgement can never be
mistaken for one that obtained it. Adding a waiver to a row whose metric a machine *could*
produce is a contract defect, not a shortcut.

## 4. Order of work

Ready work is whatever the readiness predicates report as `Dispatchable`. During bootstrap it is
the output of section 6.2, restricted to the "Minimal self-hosting prerequisite set" of
[`execution-order.md`](execution-order.md#minimal-self-hosting-prerequisite-set) until #49 closes,
**plus the bootstrap-tooling Atoms named below**.

The restriction governs Mission work. It does not govern the tooling the bootstrap loop itself
runs on, or the loop could never build the commands it depends on. The carve-out is exactly one
Atom:

```text
#70   the temporary GitHub bootstrap projection: the readiness command (6.2), the claim
      subcommands (6.3), the board recompute (6.9), and the new-atom/add-edge/check-drift
      checklist automation (section 8)  -- G-502, G-504, G-507, G-516, G-527, G-445, G-530, G-609
```

#70 is dispatchable whenever its own blockers are satisfied, even though
[`execution-order.md`](execution-order.md#minimal-self-hosting-prerequisite-set) section 15 lists
it under "not in the set": that list states what #49 requires, not what an agent may claim. Every
other Atom this runbook depends on — #1 for section 0 and #2 for the toolchain and the local
formal invocations of 6.6 — is already inside the minimal set and needs no exception.

Among the ready set, the selection order and the concurrency cap are stated once, in
[`execution-order.md`](execution-order.md) section 5 under **Selecting among ready Atoms**: lowest
`Wave`, then highest `Fan Out`, then lowest issue number. Take the first unclaimed row of that
order. Do not invent a second ranking. `gordian-derive-status ready` prints its rows in issue-number
order today, so apply the three keys to its output; sorting inside the command is **G-530, assigned
to #70**.

## 5. When blocked

An agent that cannot satisfy an Atom writes the closure record's `known_limitations`, appends a
repair Atom rather than weakening the acceptance contract, and does not close the issue. Lowering
a verifier's strength to make an Atom pass is a specification change and requires a new
`PlanRevision`.

Section 7 states the concrete bootstrap procedure for a failed attempt.

## 6. Bootstrap loop

Until #48 and #49 close, the native records of sections 1-5 do not exist. The substituted loop is:

```text
0.  acquire     section 0
1.  readiness   gordian-derive-status ready                                  (6.2)
2.  claim       assignee + board Status + claim comment                      (6.3)
3.  snapshot    capture the Atom's spec revision                             (6.4)
4.  base        resolve and record the exact base state                      (6.5)
5.  workspace   jj workspace add from that base                              (6.5)
6.  execute     in that workspace only
7.  verify      the Atom's verifier manifest                                 (6.6)
8.  land        docs/protocols/landing.md                                    (6.7)
9.  closure     write artifacts/atoms/<N>/closure.json, link it, close       (6.8)
10. board       gordian-derive-status derive --apply                         (6.9)
11. graph       record the knowledge-graph nodes the Atom touched            (6.10)
```

Throughout, `$N` is the Atom's issue number and `$GORDIAN_ACTOR` is the string of 6.1.

### 6.1 Actor identity

An agent's actor string has the exact form:

```text
gordian-agent/<harness>/<run-id>
```

`<harness>` is a lowercase name for the harness (`claude-code`, `codex`, …); `<run-id>` is that
harness's session or run identifier. The string is used in five places, and they must agree
(G-511):

- the change author identity for every change the agent creates;
- a trailer on every commit the agent creates, matching
  `^Gordian-Actor: gordian-agent/[a-z0-9-]+/[A-Za-z0-9_-]+$`;
- the closure record's `actor.id`, with `actor.kind = "agent"` — that field names the actor that
  **executed** the Atom, and `recorded_by` names the coordinator that wrote the record (section 2);
- the attempt record's `actor.id`, with the same values;
- the first line of the issue comment that claims the Atom.

```bash
export GORDIAN_ACTOR="gordian-agent/${GORDIAN_HARNESS:?}/${GORDIAN_RUN_ID:?}"
export GORDIAN_ACTOR_SLUG="$(printf '%s' "$GORDIAN_ACTOR" | tr '/' '-')"
jj config set --repo user.name  "$GORDIAN_ACTOR"
jj config set --repo user.email "agents@gordian.invalid"
```

`GORDIAN_ACTOR_SLUG` is the same string with `/` replaced by `-`; it names workspaces and
bookmarks in 6.5 and 6.7, and it is defined here so those two sections do not each invent one.

#### Credentials (non-interactive, required before the loop starts)

The loop MUST NOT call an interactive command. `gh auth login` and `gh auth refresh` are
forbidden inside the loop: they block on a browser and cannot complete unattended.

```bash
# One token, supplied by the environment. Scopes required: repo, project, workflow.
: "${GORDIAN_GH_TOKEN:?set GORDIAN_GH_TOKEN before starting the loop}"
export GH_TOKEN="$GORDIAN_GH_TOKEN"     # gh and gh api graphql read this
export GORDIAN_LOG_ROOT="${GORDIAN_LOG_ROOT:-${TMPDIR:-/tmp}/gordian-logs}"

# Push credential. The same token authenticates the https remote; no separate secret exists.
git_askpass="$(mktemp)"; printf '#!/bin/sh\necho "$GORDIAN_GH_TOKEN"\n' > "$git_askpass"
chmod +x "$git_askpass"; export GIT_ASKPASS="$git_askpass"
```

Preflight, which fails closed rather than prompting:

```bash
gh auth status >/dev/null 2>&1 || { echo "GORDIAN_GH_TOKEN invalid or unset"; exit 78; }
gh api user -q .login >/dev/null      || { echo "token cannot read the API";   exit 78; }
gh api repos/kmosoti/gordian -q .name >/dev/null \
                                      || { echo "token lacks repo scope";      exit 78; }
gh project list --owner kmosoti --limit 1 >/dev/null \
                                      || { echo "token lacks project scope";   exit 78; }
```

Exit code 78 means *configuration missing* and is the one failure an agent reports without
recording an attempt: no Atom was claimed, so there is nothing to release. If the token lacks a
scope, stop and say which one. Never attempt to widen it yourself — `gh auth refresh` is the
interactive command this section exists to avoid.

### 6.2 Deriving readiness

The **native GitHub blocked-by graph is authoritative** for dependencies. The `## Dependencies`
prose in an issue body is a human-readable mirror of it and is never read by a tool.

The bootstrap satisfaction rule, which is the bootstrap analogue of Satisfied-as-admitted
(D2, [`../spec/mission-graph.md`](../spec/mission-graph.md#logical-state-predicates)), is:

> An Atom `N` counts as **satisfied for readiness** if and only if issue `N` is closed **and**
> `artifacts/atoms/<N>/closure.json` exists on `main` at the commit under evaluation **and** that
> record validates against
> [`../../artifacts/schema/closure-record.schema.json`](../../artifacts/schema/closure-record.schema.json).

An Atom is **ready** when every issue in its native `blocked by` list is satisfied under that
rule. A closed issue with no validating closure record is not satisfied, and the command below
exits non-zero when it finds one.

**`gordian-derive-status ready` is the only sanctioned way to pick the next Atom** (G-504, G-516).
It reads edges from `blockedByIssues` node lists — never from `issueDependenciesSummary.blocking`,
which is wrong for #11, #18 and #44 — applies the satisfaction rule above against
`artifacts/atoms/*/closure.json`, and prints each ready Atom with its `Wave`, `Fan In`, `Fan Out`
and derived `Status`:

```bash
python3.14 -m pip install -e './orchestration[dev]'    # once; see 6.6

GH_TOKEN="$GORDIAN_GH_TOKEN" gordian-derive-status ready
GH_TOKEN="$GORDIAN_GH_TOKEN" gordian-derive-status ready --json
GH_TOKEN="$GORDIAN_GH_TOKEN" gordian-derive-status ready --all      # every open Atom, not only ready
```

`GH_TOKEN` carries a **classic** personal access token with the `repo` and `project` scopes; the
interactive `gh auth refresh -s project` flow is not available to an unattended agent. A non-zero
exit means either an authentication failure or a closed issue with no validating closure record —
in the second case the readiness printed rests on unevidenced closure, and the fix is the missing
closure record, not a retry.

`--snapshot artifacts/atoms/issues.json` reads the committed edge snapshot instead of calling
`gh`, for an offline or rate-limited run. That snapshot is not committed yet: **G-502 is assigned
to #70**, which produces and refreshes it. Until it exists, run without `--snapshot`.

Do not hand-roll a `gh` query for the ready set. Hand-rolled readiness silently drops the
satisfaction rule, which is the whole point of the command.

### 6.3 Claiming an Atom

A claim is three facts, all idempotent, and it expires (G-507):

1. **Assignee.** The agent's GitHub identity is assigned to the issue. Assignment is the lock: an
   issue with an assignee is claimed, an issue with none is not.
2. **Board Status.** Project 9's `Status` field moves to `In Progress`. `In Progress` is a
   claim-owned value, not a derived one — 6.9 gives the split — so writing it here contradicts
   nothing.
3. **Claim comment.** A comment carrying the actor string, an ISO-8601 UTC timestamp, and the
   expiry, so a crashed agent's claim is visibly stale rather than permanent.

**Claim expiry: 240 minutes** from the comment's timestamp — the same 4 hours as the per-Atom
wall-clock cap of section 7. A claim whose timestamp plus 240 minutes is in the past is **stale**:
any agent may release it with the section 7 procedure and then claim the Atom itself. A live claim
by a different actor is never taken over.

```bash
claimed_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

# 1. assignee
gh issue edit "$N" --repo kmosoti/gordian --add-assignee "@me"

# 2. board Status = In Progress
project_id="$(gh project view 9 --owner kmosoti --format json --jq .id)"
status_field="$(gh project field-list 9 --owner kmosoti --format json \
  --jq '.fields[] | select(.name=="Status")')"
field_id="$(jq -r .id <<<"$status_field")"
option_id="$(jq -r '.options[] | select(.name=="In Progress") | .id' <<<"$status_field")"
item_id="$(gh project item-list 9 --owner kmosoti --limit 200 --format json \
  --jq ".items[] | select(.content.number==$N) | .id")"
gh project item-edit --project-id "$project_id" --id "$item_id" \
  --field-id "$field_id" --single-select-option-id "$option_id"

# 3. claim comment
gh issue comment "$N" --repo kmosoti/gordian --body "$(cat <<EOF
$GORDIAN_ACTOR
claimed_at: $claimed_at
expires_after_minutes: 240
EOF
)"
```

The comment's first line is the actor string of 6.1 and nothing else, so the claim's owner is
readable by a `head -1`. Two agents that race resolve it by assignee: the later one runs the
section 7 release and takes the next row of the selection order.

The `claim`, `release`, and `claims` subcommands that make this a command rather than a checklist
— where `claims` prints each Atom's claim actor and whether the claim is still live, and `claim`
exits non-zero against a live claim by another actor — are **G-507, assigned to #70**. A lease over
a `LeaseSubject::LogicalChange` replaces all of it when #23 closes.

### 6.4 Spec revision snapshot

Evidence binds to an exact specification revision. During bootstrap the Atom's specification is
its issue body, so capture it at claim time and commit it (G-512):

```bash
mkdir -p "artifacts/atoms/$N"
gh issue view "$N" --repo kmosoti/gordian --json body --jq .body \
  | tr -d '\r' > "artifacts/atoms/$N/spec.md"
sha256sum "artifacts/atoms/$N/spec.md"
```

That digest — SHA-256, lowercase hex, over the file with every CR byte removed — is the Atom's
bootstrap spec revision, and it is the value the closure record carries in `spec_digest`.
`scripts/check-spec-digests.sh` recomputes it from the committed snapshot and fails on
disagreement, so an issue body edited after the claim is a visible event rather than a silent one.

### 6.5 Base and workspace

**The base is conditional.** `trunk()` is the alias for the remote bookmark `main@origin`
([`../protocols/jujutsu-development-environment.md`](../protocols/jujutsu-development-environment.md#5-resulting-repository-configuration)),
which moves; it is the base **only when the Atom has no unsatisfied hard dependency**. When it has
one, the coordinator supplies an exact `PrerequisiteContaining` base
([`../protocols/jujutsu-agent-protocol.md`](../protocols/jujutsu-agent-protocol.md#5-snapshot-rule))
and that state id is substituted for `trunk()` below. Native step 2 of section 1 requires this;
the bootstrap loop does not get to skip it.

**Resolve the base to a commit id and record it before creating the workspace.** A moving bookmark
resolved at some unrecorded moment is not an exact base:

```bash
export GORDIAN_BASE="$(jj log -r 'trunk()' -T commit_id --no-graph)"   # or the supplied base
mkdir -p "artifacts/atoms/$N/attempts"
printf '%s\n' "$GORDIAN_BASE" > "artifacts/atoms/$N/attempts/base_exact_state_id"
```

`$GORDIAN_BASE` is the `base_exact_state_id` of the candidate handoff
([`../protocols/jujutsu-development-environment.md`](../protocols/jujutsu-development-environment.md#candidate-handoff)).
Neither schema carries a base field and both set `additionalProperties: false`, so the plain file
above is where it survives; the closure record names it in `known_limitations` — for example
`base_exact_state_id recorded at artifacts/atoms/<N>/attempts/base_exact_state_id` — until a
schema gains the field.

**The workspace name, root variable, and commands are stated once**, in
[`../protocols/jujutsu-development-environment.md`](../protocols/jujutsu-development-environment.md#one-atom-attempt-one-workspace-one-normal-path-writer)
section 6, and this runbook does not restate them. Set `GORDIAN_ATOM="$N"` and run that block,
substituting `$GORDIAN_BASE` for `trunk()`. In summary, and normative there rather than here:

- the workspace is `$GORDIAN_WORKSPACE_ROOT/atom-$GORDIAN_ATOM-$GORDIAN_ACTOR_SLUG`, where
  `GORDIAN_WORKSPACE_ROOT` defaults to `$PWD/../gordian-workspaces`;
- the actor slug is in the name, so two harnesses on one Atom do not collide, and section 7's
  `jj workspace forget` names the same string;
- **never edit the default workspace**: another agent or a human holds it.

[`../protocols/jujutsu-agent-protocol.md`](../protocols/jujutsu-agent-protocol.md) is normative for
the change and candidate lifecycle inside the workspace.

### 6.6 Verification

Run the whole list against the workspace state and write each verifier's log to
`$GORDIAN_LOG_ROOT/atom-$N/<verifier_id>.log`, where `GORDIAN_LOG_ROOT` defaults to
`${TMPDIR:-/tmp}/gordian-logs` and MUST resolve OUTSIDE the workspace. Writing a verifier log
inside the workspace would be auto-snapshotted by Jujutsu and change `exact_state_id`, breaking
the section 1 step 5 freeze the verifiers are evidence for. Copy the logs into the bookkeeping
change at section 6.8, after the candidate is frozen, at
`artifacts/atoms/$N/verifiers/<verifier_id>.log` (section 2).

The list is the five members of `project_integration_verifiers` named in
[`../protocols/landing.md`](../protocols/landing.md) section 3, so a bootstrap Atom's evidence and
an admitted integration's evidence are produced by the same verifiers. `.github/workflows/verify.yml`
runs the same commands; where the two could drift, landing.md section 3 and the workflow are the
authorities and this block is the copy.

The Python verifier needs the package installed first — the tests import `gordian_orchestration`
from a src layout, so `python -m unittest discover` fails with `ModuleNotFoundError` without it:

```bash
python3.14 -m pip install -e './orchestration[dev]'
```

That is the single bootstrap string used by [`../../AGENTS.md`](../../AGENTS.md) and
[`../index.md`](../index.md), and the `dev` extra carries the pinned `ruff`. On an interpreter
older than 3.14 the editable install is refused and the fallback of
[`issue-index.md`](issue-index.md#temporary-project-9-reconciliation) applies:
`PYTHONPATH=orchestration/src python3 -m unittest discover -s orchestration/tests`.

```bash
# verifier:rust-check
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace

# verifier:kg-audit
cargo run --locked -p gordian-kg -- validate
cargo run --locked -p gordian-kg -- audit --strict

# verifier:formal
bash scripts/verify-formal.sh

# verifier:python
ruff check orchestration
python -m compileall -q orchestration/src
python -m unittest discover -s orchestration/tests

# verifier:spec-consistency
for s in scripts/check-*.sh; do bash "$s"; done
shellcheck scripts/*.sh
bash -n scripts/*.sh
python3 -m compileall -q scripts/
```

`verifier:formal` is `lake build; leanchecker; axiom-audit` in landing.md section 3, and the
workflow's formal job runs all three (`leanchecker: true`, `axiom-audit: true`). The independent
checker and the axiom audit — the check that rejects `sorryAx` and any non-allowlisted axiom —
have **no local invocation yet**: they arrive with `formal/Gordian/Audit.lean` under **G-258 and
G-239, assigned to #2**. Until then a bootstrap Atom's `verifier:formal` evidence is `lake build`
only, and the closure record records that in `known_limitations`.

The `check-*.sh` loop is that job's first step and the last three lines are its second step.
Omitting either lets an Atom pass this list and still fail CI.

An Atom with stronger acceptance predicates runs those too, with its own `verifier_id`. Passing
this list is necessary and is not automatically sufficient.

### 6.7 Landing

Landing is [`../protocols/landing.md`](../protocols/landing.md). Its section 1 is the authority
rule and its section 2 is the ordered sequence; this runbook does not restate either.

Bootstrap authority, until #18 and #19 make it a capability check (G-509):

| Actor | May push `main` | Condition |
| --- | --- | --- |
| coordinator role (agent or human) holding the repository's push credential | yes | every member of `project_integration_verifiers` ran through the adapter against the exact state being pushed, at landing step 0, and produced a passing `Evidence` record bound to that state |
| worker role (any agent executing steps 1-7) | no | pushes a `gordian/` bookmark and opens a pull request instead |

The condition is deliberately **not** "the checks on `.github/workflows/verify.yml` are green on
the pushed state": that workflow triggers on `push` and `pull_request` to `main`, so a state that
reaches the remote only at landing step 3 is invisible to it until after the admission step 3
follows — requiring it would mean admitting before verifying, or pushing before being permitted to
admit ([`../protocols/landing.md`](../protocols/landing.md) section 3). The condition is also not
"`artifacts/atoms/<N>/closure.json` validates": section 1 and 6.8 require that record to be
written **after** admission, in a bookkeeping change, so it cannot exist before the push that
admission drives. Validating it is a **post-admission obligation**, discharged by 6.8 and enforced
by `scripts/check-closure-records.sh` on the next run of the **Specification consistency** job.

The worker's bookmark form is the one
[`../protocols/jujutsu-development-environment.md`](../protocols/jujutsu-development-environment.md#one-atom-attempt-one-workspace-one-normal-path-writer)
section 6 states normatively and
[`../protocols/landing.md`](../protocols/landing.md) section 2 recognises as a `stage` ref. Every
bookmark an agent creates lives under `gordian/`; `main` never does:

```bash
jj bookmark create "gordian/atom-$N/$GORDIAN_ACTOR_SLUG" -r @
jj git push --bookmark "gordian/atom-$N/$GORDIAN_ACTOR_SLUG" --allow-new
gh pr create --fill --base main --head "gordian/atom-$N/$GORDIAN_ACTOR_SLUG"
```

A pull request is a review projection, not the admission gate
([`../protocols/landing.md`](../protocols/landing.md) section 4).
`.github/PULL_REQUEST_TEMPLATE.md` carries the closure-evidence checklist. If the pull request is
merged with a merge or squash commit, the landed state is not the verified state and section 2's
re-verification rule applies.

### 6.8 Closure

Write `artifacts/atoms/<N>/closure.json` per section 2, in a change that is **not** the Atom's
verified candidate, with the verifier logs of 6.6 written into that same change first. Link it
from the closing issue comment, and close the issue. Closing without a validating record is not
closure and `scripts/check-closure-records.sh` fails the build.

### 6.9 Board update

Project 9's `Wave`, `Fan In`, `Fan Out`, and the `Blocked`/`Ready` half of `Status` are **derived
projections** of the native blocked-by graph and the on-disk closure records. Recompute them after
every closure rather than editing a cell:

```bash
GH_TOKEN="$GORDIAN_GH_TOKEN" gordian-derive-status derive --compare-board   # dry run
GH_TOKEN="$GORDIAN_GH_TOKEN" gordian-derive-status derive --apply
```

`Wave` is longest-path depth, `Fan In` and `Fan Out` are in-degree and out-degree, and none of the
three is ever written by hand.

`Status` has one definition, shared by this section, 6.3,
[`issue-index.md`](issue-index.md#initiative-milestones-and-native-dependencies), and
`gordian_orchestration.derive_status`, and it has five values:

```text
Blocked      some blocker is not satisfied under the bootstrap satisfaction rule   derived
Ready        every blocker is satisfied and the issue has no assignee              derived
In Progress  every blocker is satisfied and the issue has an assignee              claim-owned
In Review    a pull request referencing the issue is open                          claim-owned
Accepted     the issue is closed AND artifacts/atoms/<N>/closure.json validates    claim-owned
```

The split is what makes 6.3 and this section consistent. `Blocked` and `Ready` are computed from
the graph and the closure records, and `derive --apply` writes them. `In Progress`, `In Review`
and `Accepted` are facts about a claim and its landing that the loop asserts — 6.3 sets
`In Progress`, 6.7 sets `In Review` if the deployment uses pull requests, 6.8 sets `Accepted` —
and `derive --apply` never overwrites them. No cell is both computed and asserted, so "derived
projection, never an input" and "the claim step sets `Status`" are not in conflict: nothing reads
`Status` back as authority. The authority for a claim is the assignee; the authority for
acceptance is the closure record.

A milestone is bookkeeping, and closing one is the last step of an Initiative, not a gate:
**the same coordinator role that lands the Initiative's final Atom closes the milestone**, and only
when every member Atom of that milestone has a validating closure record — the `Acceptance rule`
column of [`issue-index.md`](issue-index.md#initiative-register). No agent closes a milestone as a
side effect of closing an issue (G-523).

### 6.10 Knowledge graph

The knowledge graph is part of the substrate, not documentation about it, so an Atom that
introduces or falsifies a material concept updates it in the same landing change (G-501):

1. Decide whether the Atom touches the graph, using the **Completeness rule** of
   [`../../knowledge/ontology.md`](../../knowledge/ontology.md#completeness-rule): a new
   architectural noun, an algorithm, a theorem, an experiment result, a retained or rejected
   alternative, or a newly exposed assumption all require a node.
2. Edit the owning shard under `knowledge/graph/`. The authoritative shard list and what each one
   holds is [`../../knowledge/ontology.md`](../../knowledge/ontology.md#shards);
   [`../knowledge-graph.md`](../knowledge-graph.md#corpus-structure) is the reader's overview of
   the same corpus. Do **not** hand-edit
   `knowledge/graph/95-backlog.jsonld`: it is the generated **Backlog mirror** of the native GitHub
   graph, and its generator and regenerate-and-diff CI step are **#72's** work.
3. Re-run the graph verifier of 6.6:

```bash
cargo run --locked -p gordian-kg -- validate
cargo run --locked -p gordian-kg -- audit --strict
```

4. List every node id created or changed in the closure record's `knowledge_graph_node_ids`
   (section 2). `.github/PULL_REQUEST_TEMPLATE.md` asks the reviewer for the same list.

An Atom that touches no node writes `[]` and records that judgement in `known_limitations`, so the
absence is a decision rather than an omission.

## 7. Failure handling

Concrete values, so an agent does not have to decide them (G-520):

- **Verifier retry limit: 2.** A verifier that fails twice on the same exact state is a failure,
  not a flake. A third run is only permitted after the state changes.
- **Per-Atom wall-clock cap: 4 hours** of agent execution, excluding queue time. This is the same
  240 minutes as the 6.3 claim expiry, and deliberately so: a claim outlives its agent by nothing.
- **Per-Atom cost cap: USD 25** of model spend.

On any of `verifier_failed`, `timed_out`, `budget_exceeded`, or a deliberate `abandoned` — and on
releasing another actor's **stale** claim under 6.3 — execute this ordered procedure:

```bash
# 1. record the attempt
mkdir -p "artifacts/atoms/$N/attempts"
# write artifacts/atoms/$N/attempts/<attempt_id>.json against
# artifacts/schema/attempt-record.schema.json, carrying the outcome below, a reason
# string, actor = $GORDIAN_ACTOR, and the observed budget.

# 2. release the claim: all three facts of 6.3, in reverse
gh issue edit "$N" --repo kmosoti/gordian --remove-assignee "@me"
gh project item-edit --project-id "$project_id" --id "$item_id" \
  --field-id "$field_id" --single-select-option-id "$ready_or_blocked_option_id"
gh issue comment "$N" --repo kmosoti/gordian --body "$GORDIAN_ACTOR released this claim at \
$(date -u +%Y-%m-%dT%H:%M:%SZ): <reason>"

# 3. discard the workspace (the 6.5 name, from jujutsu-development-environment.md section 6)
jj workspace forget "atom-$N-$GORDIAN_ACTOR_SLUG"
```

Step 2's board write resets the cell the claim set. Rather than choosing `Ready` or `Blocked` by
hand, run `gordian-derive-status derive --apply` after the assignee is cleared: with no assignee
the derived value is correct by construction, which is why the release must clear the assignee
first.

The attempt record carries an `outcome` drawn from the closed set

```text
verified | verifier_failed | timed_out | budget_exceeded | abandoned | contract_defect
```

plus a `reason` string, and it is validated in CI by the same validator as the closure record:
`scripts/check-closure-records.sh` validates every `artifacts/atoms/*/attempts/*.json` against
[`../../artifacts/schema/attempt-record.schema.json`](../../artifacts/schema/attempt-record.schema.json).
That schema is the single normative list of what an attempt record contains; the values this
section fixes are the ones it cannot supply. Note three of its rules:

- `attempt_id` is `<started_at as a compact UTC timestamp>-<harness>`, for example
  `20260830T142530Z-claude-code`, and the file name MUST be `<attempt_id>.json`, so two attempts
  never overwrite each other and attempts sort chronologically;
- `contract_defect_issue` is required when `outcome` is `contract_defect` and forbidden otherwise,
  so "the contract is wrong" cannot be asserted without a repair Atom to answer it;
- `budget.wall_clock_seconds` and `budget.cost_usd` are where the observed spend against the caps
  above is recorded.

On `contract_defect` — the Atom's acceptance predicates are wrong, unreachable, or contradictory —
the agent MUST open a new issue linked to Atom `N` describing the defect and MUST NOT edit Atom
`N`'s acceptance bullets. Weakening an acceptance contract to make an attempt pass is a
specification change; see section 5.

## 8. Creating or splitting an Atom

A new Atom must appear consistently in the issue body, the native blocked-by graph, the milestone,
the `type:*` label, and Project 9. The procedure and its drift check are the subject of **G-527,
assigned to #70** (`new-atom`, `add-edge`, and `check-drift` subcommands under `orchestration/`).
Until they exist, follow the manual checklist in
[`issue-index.md`](issue-index.md#adding-or-splitting-an-atom) and run every regeneration step it
names. Section 4's carve-out is what allows #70 itself to be claimed and closed.

## 9. Experiment execution policy

Experiment Atoms additionally need an authorized provider and model list, the environment-variable
names carrying each credential, and per-run and per-experiment cost and wall-clock caps. That
policy is **G-526, assigned to #37**, and is written into
[`../testing/statistical-contract.md`](../testing/statistical-contract.md) and
`orchestration/README.md` when #37 lands. Until then an experiment Atom records the provider,
model id, and observed spend in its `ExperimentRun` manifest and does not exceed the section 7
caps.
