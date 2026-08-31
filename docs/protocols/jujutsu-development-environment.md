# Jujutsu Development Environment

Gordian uses Jujutsu as its development baseline and candidate-source substrate, subject to the
Git comparison experiment in issue #34. This is a bounded tooling choice for the development
environment, not a claim that Jujutsu is superior to Git. Jujutsu is one implementation of the adapter-neutral
[`source-adapter-contract.md`](source-adapter-contract.md); the semantics it serves are stated
there and in [`jujutsu-agent-protocol.md`](jujutsu-agent-protocol.md), not here.

The pinned candidate release is `DEFAULT_JJ_VERSION` in
[`scripts/bootstrap-jj.sh`](../../scripts/bootstrap-jj.sh), which is the **single source** of the
baseline; this document deliberately does not restate a version number that could drift from it.
Upstream release history records that `jj run` arrived before its required read-only
`--ignore-changes` mode. The script remains the sole version authority; this explanatory history
does not define a second pin. It is a **candidate
qualification baseline**, not an eternally supported version: issue #1 must prove the required
behaviors through disposable-repository contract tests before the adapter declares support.

## 1. Why Gordian needs a qualified baseline

Required behaviors include:

- persistent logical change IDs across ordinary rewrites;
- exact commit IDs for immutable verification subjects;
- isolated workspaces;
- sibling changes for independent work;
- parent/child topology for causal source dependencies;
- multi-parent integration candidates;
- first-class intermediate conflict state;
- operation history and recovery;
- tracked remote bookmarks and tags;
- revision-scoped command execution through `jj run` or an equally strict fallback;
- read-only `jj run --ignore-changes` verification;
- machine-readable output stable enough for a bounded Rust adapter.

Gordian must not infer support merely from a version comparison. The version gate prevents obviously unsupported installations; the contract suite establishes behavioral support.

## 2. Obtaining a checkout

Canonical remote:

```text
https://github.com/kmosoti/gordian
```

A fresh agent with no existing checkout acquires one and configures it in a single sequence,
which contains no personal absolute path and no interactive step:

```bash
jj git clone https://github.com/kmosoti/gordian
cd gordian
bash scripts/bootstrap-jj.sh --install
```

`jj git clone` creates the `origin` remote with that URL, so the bootstrap's remote check passes
without adding anything. The end-to-end agent loop that this sequence is the first step of is
[`../implementation/agent-runbook.md`](../implementation/agent-runbook.md).

The bootstrap script assumes the current directory is already a Jujutsu repository. It
intentionally does not initialize or overwrite an unrelated directory, so it must be run from the
root of the clone.

## 3. Bootstrap

From the repository root:

```bash
./scripts/bootstrap-jj.sh --install
```

The script:

1. installs the pinned candidate Jujutsu release only when explicitly requested;
2. verifies the executable version;
3. verifies the current directory contains `.jj`;
4. adds `origin` only when absent and rejects a conflicting URL;
5. fetches the remote;
6. tracks `main@origin`;
7. configures fetch/push defaults;
8. defines `trunk()` as `main@origin` through supported `jj config set --repo` commands;
9. checks that the `jj run` subcommand is registered (not a behavioral contract test; see
   section 9 / issue #1) and resolves `trunk()`;
10. never pushes, rewrites, rebases, abandons, or promotes code.

Run without `--install` for a read/configure-only check:

```bash
./scripts/bootstrap-jj.sh
```

Override the candidate release only for qualification work, by naming the release under test:

```bash
JJ_REQUIRED_VERSION="$JJ_CANDIDATE" ./scripts/bootstrap-jj.sh --install
```

Any changed release must be recorded in contract-test evidence and the supported-baseline
decision, and `DEFAULT_JJ_VERSION` in the script is what changes when the baseline moves.

## 4. Identity configuration

Jujutsu requires user identity. Gordian does not commit personal identity values into repository configuration.

Inspect:

```bash
jj config get user.name
jj config get user.email
```

Set user-level identity when missing:

```bash
jj config set --user user.name "Your Name"
jj config set --user user.email "you@example.com"
```

## 5. Resulting repository configuration

The bootstrap sets repository-scoped values using the CLI rather than committing `.jj/repo/config.toml`. Current Jujutsu stores repository/workspace configuration outside the repository metadata location that older versions used.

The resulting repository-scoped values are:

```toml
[git]
fetch = ["origin"]
push = "origin"

[revset-aliases]
"trunk()" = "main@origin"
```

Do not create a permanent `develop` bookmark.

## 6. Development topology

### Accepted frontier

```text
trunk() == main@origin
```

is a **projection** of `project(H).accepted_frontier`, not the accepted frontier itself. When the
bookmark and the event log disagree, the log wins: the coordinator appends
`FrontierDivergenceObserved { expected, observed, source }` and re-drives the bookmark
([`../spec/invariants.md` `## Accepted-frontier linearization`](../spec/invariants.md#accepted-frontier-linearization)).

A local `trunk()` that has drifted is therefore an operational fact to be recorded and repaired,
never a reason to move the frontier. Three representations existed — the revset alias, the remote
bookmark, and the `accepted_frontier` projection — and no document said which was authoritative or
how they were reconciled at startup.

`main` is the public accepted/releasable source frontier. It is not the worker scratchpad and not
proof of what is deployed. Moving it is the landing sequence of [`landing.md`](landing.md), which
only an actor holding `move_accepted_frontier` may run.

### One Atom attempt, one workspace, one normal-path writer

An agent starting Atom `#N` runs exactly this, from the root of the clone. It is executable as
written once the four environment variables are set; the actor string is the
`gordian-agent/<harness>/<run-id>` identity of the runbook.

```bash
export GORDIAN_ATOM=42
export GORDIAN_ACTOR="gordian-agent/claude-code/run-882"
export GORDIAN_ACTOR_SLUG="$(printf '%s' "$GORDIAN_ACTOR" | tr '/' '-')"
export GORDIAN_WORKSPACE_ROOT="${GORDIAN_WORKSPACE_ROOT:-$PWD/../gordian-workspaces}"

mkdir -p "$GORDIAN_WORKSPACE_ROOT"
jj workspace add "$GORDIAN_WORKSPACE_ROOT/atom-$GORDIAN_ATOM-$GORDIAN_ACTOR_SLUG" \
  --name "atom-$GORDIAN_ATOM-$GORDIAN_ACTOR_SLUG" \
  -r 'trunk()'

cd "$GORDIAN_WORKSPACE_ROOT/atom-$GORDIAN_ATOM-$GORDIAN_ACTOR_SLUG"
jj describe -m "Atom: #$GORDIAN_ATOM

Gordian-Actor: $GORDIAN_ACTOR"

# ... implement and run the Atom's verifier manifest here ...

jj bookmark create "gordian/atom-$GORDIAN_ATOM/$GORDIAN_ACTOR_SLUG" -r @
jj git push --bookmark "gordian/atom-$GORDIAN_ATOM/$GORDIAN_ACTOR_SLUG" --allow-new
```

The bookmark naming rule is `gordian/atom-<N>/<actor-slug>`, where `<actor-slug>` is the actor
string with `/` replaced by `-`. Every bookmark an agent creates lives under the `gordian/`
namespace; `main` is never in it. Pushing such a bookmark is the adapter's `stage` operation
([`source-adapter-contract.md`](source-adapter-contract.md)): it makes the exact state fetchable
by an external verifier, it is garbage-collectable, and it implies nothing about admission.

Each agent works in its own added workspace and must **never edit the default workspace**, which
belongs to whoever cloned the repository and is not an execution container.

`-r 'trunk()'` is the base only when the Atom has no unsatisfied hard dependency. When it has
one, the coordinator supplies an exact `PrerequisiteContaining` base instead, per
[`jujutsu-agent-protocol.md` `## 5. Snapshot rule`](jujutsu-agent-protocol.md#5-snapshot-rule);
substitute that state id for `trunk()` in the `jj workspace add` invocation.

Each active attempt receives:

```text
Atom/spec revision
exact base state (exact_state_id)
workspace identity (workspace_id)
logical change identity (logical_change_id)
owner lease/capability envelope (LeaseSubject::LogicalChange, fencing_token)
```

Two speculative attempts for the same Atom receive distinct logical change identities from the
same base. Two workers do not intentionally rewrite one logical change.

### Causality, not chronology

Independent Atoms should remain sibling changes. A change becomes an ancestor of another only when a real source dependency exists. Finish order is not causality.

### Candidate handoff

A worker hands off:

```text
Atom identity
specification revision
attempt identity
base_exact_state_id
logical_change_id
exact_state_id
fencing_token
```

Jujutsu adapter: `logical_change_id` = change ID, `exact_state_id` = commit ID; a Git adapter
synthesizes the change identity and uses the commit SHA. The canonical record is
[`../spec/data-model.md` `## Candidate`](../spec/data-model.md#candidate).

Verification binds to the `exact_state_id`. Any rewrite creates a new candidate and invalidates
candidate-bound evidence even when the `logical_change_id` survives. The `fencing_token` is
carried so admission can check `LeaseValidAtFreeze`.

## 7. Read-only exact-revision verification

On the qualified baseline:

```bash
jj run \
  -r '<candidate-revset>' \
  -j <bounded-jobs> \
  --ignore-changes \
  -- ./tools/verify
```

`--ignore-changes` discards working-copy modifications and permits read-only checks of immutable revisions. The adapter must still record the exact revision, environment, verifier identity/configuration, exit behavior, and outputs.

Ignored files may persist in temporary working copies for incremental builds. Qualification must test reproducibility implications and cleaning policy rather than assuming every repeated run is hermetic.

## 8. Remote setup without implicit publication

The bootstrap adds and fetches `origin` but never pushes. Publication is an operation of an actor
holding the `move_accepted_frontier` capability, performed through the source adapter's
`move_frontier` and the landing sequence of [`landing.md`](landing.md). That actor may be a
software agent; no human step is required.

A worker pushing a `gordian/`-namespaced staging bookmark is not publication: it moves no
frontier. Only `main` is the published frontier projection.

Inspect:

```bash
jj git remote list
jj bookmark list --all-remotes
jj log -r 'trunk()' -n 1
```

A normal worker must not:

```text
move main/trunk()
push canonical remote state
bypass immutable revisions
create releases or tags
mutate another worker workspace
```

## 9. Contract qualification

Issue #1 builds disposable repositories and tests its bounded baseline contract:

1. logical change ID persistence and exact commit ID change after rewrite;
2. workspace isolation;
3. sibling, causal parent/child, and multi-parent topology;
4. conflict materialization, persistence, and repair;
5. operation-log recovery;
6. local tag identity;
7. machine-readable change, commit, and operation identities;
8. presence of `jj run` and its read-only flag, without claiming execution semantics.

Initial clone/fetch/tracking remains the bootstrap acquisition check. Atom #33 owns `jj run`
exact-revision isolation, bounded parallelism, failure behavior, and read-only semantics. Atom #29
owns the adapter parser and its broader machine-output contract. Migration from unsupported
repositories, including the historical 0.23 environment, is not supported: tooling fails closed
before mutation until a later Atom explicitly qualifies a migration path.

The suite records:

```text
jj version and binary digest
Git version
OS/filesystem
fixture source identity
commands/arguments
operation/change/commit identities
results and output artifacts
```

## 10. Performance qualification

The source adapter must benchmark:

- workspace creation/update/removal;
- exact revision resolution;
- status and revset queries;
- candidate commit materialization;
- `jj run` startup and parallel verification;
- sibling integration and rebase/reconciliation;
- conflict query/materialization;
- operation-log recovery;
- repository scaling by commits, changes, workspaces, changed files, and merge topology.

A fast development workflow is not enough. Gordian needs predictable orchestration cost and failure semantics under many workers and exact-revision verifier runs.

## 11. Upgrade policy

A future Jujutsu release is admitted only after:

1. release notes and breaking changes are reviewed;
2. the disposable contract suite passes;
3. performance and output changes are compared;
4. stored evidence identifies the new binary/version;
5. adapter assumptions and documentation are updated;
6. rollback/migration behavior is known.

Gordian uses Jujutsu deeply, but it should remain an adapter-backed dependency whose value is continuously falsifiable.
