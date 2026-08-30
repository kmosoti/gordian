# Jujutsu Development Environment

Gordian uses Jujutsu as its preferred development and candidate-source substrate, subject to the Git comparison experiment in issue #34. The local Codex environment reported `jj 0.23.0`. That release predates `jj run` and several configuration/identity behaviors the Gordian protocol intends to exercise.

As of 2026-08-29, the latest released Jujutsu documented upstream is `0.44.0`; `jj run` arrived in `0.43.0`. Gordian therefore treats `0.44.0` as the current **candidate qualification baseline**, not an eternally supported version. Issue #1 must prove the required behaviors through disposable-repository contract tests before the adapter declares support.

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

## 2. Repository assumptions

Local repository location reported for Codex:

```text
~/projects/project-management-tools/gordian
```

Canonical remote:

```text
https://github.com/kmosoti/gordian
```

The bootstrap script assumes the local checkout is already a Jujutsu repository. It intentionally does not initialize or overwrite an unrelated directory.

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
9. verifies `jj run` and exact remote-trunk resolution;
10. never pushes, rewrites, rebases, abandons, or promotes code.

Run without `--install` for a read/configure-only check:

```bash
./scripts/bootstrap-jj.sh
```

Override the candidate release only for qualification work:

```bash
JJ_REQUIRED_VERSION=0.44.0 ./scripts/bootstrap-jj.sh --install
```

Any changed release must be recorded in contract-test evidence and the supported-baseline decision.

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

Conceptually:

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

`main` is the public accepted/releasable source frontier. It is not the worker scratchpad and not proof of what is deployed.

### One Atom attempt, one workspace, one normal-path writer

Conceptual command:

```bash
jj workspace add ../gordian-worker-<attempt> \
  --name worker-<attempt> \
  -r <exact-base-commit>
```

Each active attempt receives:

```text
Atom/spec revision
exact base commit
workspace identity
logical change ID
owner lease/capability envelope
```

Two speculative attempts for the same Atom receive distinct change IDs from the same base. Two workers do not intentionally rewrite one change ID.

### Causality, not chronology

Independent Atoms should remain sibling changes. A change becomes an ancestor of another only when a real source dependency exists. Finish order is not causality.

### Candidate handoff

A worker hands off:

```text
Atom identity
specification revision
attempt identity
base commit ID
Jujutsu change ID
exact candidate commit ID
```

Verification binds to the exact commit ID. Any rewrite creates a new candidate and invalidates candidate-bound evidence even when the logical change ID survives.

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

The bootstrap adds/fetches `origin`, but never pushes. Publication remains an explicit coordinator/human action.

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

Issue #1 should build disposable repositories and test:

1. initial clone/fetch/tracking;
2. logical change ID persistence after rewrite;
3. exact commit ID change after rewrite;
4. workspace isolation and stale-workspace recovery;
5. sibling and causal parent/child topology;
6. multi-parent integration;
7. conflict materialization, persistence, and repair;
8. operation-log recovery;
9. tag fetch/push behavior needed for releases;
10. `jj run` exact-revision isolation, bounded parallelism, failure behavior, and `--ignore-changes` semantics;
11. machine-readable template/revset outputs used by the Rust adapter;
12. migration from the reported `0.23.0` local repository without source loss.

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
