# Supported Jujutsu Baseline Qualification

Graph node: `experiment:jj-baseline`  
Class: `benchmark`  
Owning issue: #1  
Benchmark obligation: `EO17-JJ-5`

This benchmark qualifies scaling of machine-readable revision reads across disposable colocated
Jujutsu/Git fixtures. It declares five shapes: two linear change counts, wide siblings, a
multi-parent merge topology, and eight registered workspaces. Each shape has ten warmups and thirty
measured reads per condition in the full run. Fixture construction and workspace creation happen
before timing begins. Every raw observation, including failures and timeouts, remains in the JSON
report.

## Baseline rationale

The minimum supported release is `DEFAULT_JJ_VERSION` in
[`../../scripts/bootstrap-jj.sh`](../../scripts/bootstrap-jj.sh), the repository's single source
of the pin. Upstream release history shows that `jj run` preceded the required
`--ignore-changes` safety option used by exact-revision verification. This directory does not copy
the pin as an independent source; the runner records the installed binary's exact version output,
path, and SHA-256 digest.

## Contract and benchmark commands

Validate the registered protocol and all experiment manifests with:

```bash
bash scripts/check-experiment-manifests.sh
bash scripts/check-statistical-contract.sh
```

Run the bounded test mode:

```bash
python3 experiments/jj-baseline/benchmark.py \
  --smoke --output /tmp/jj-baseline-smoke.json
```

Run the registered benchmark (10 warmups and 30 measured iterations per shape and condition):

```bash
python3 experiments/jj-baseline/benchmark.py \
  --output experiments/jj-baseline/runs/<run-id>.json
```

The required output is one JSON report containing the protocol digest, source JJ/Git states,
Python/platform identity, JJ and Git binary digests, fixture manifests, every raw measurement, and
per-shape medians with deterministic 10,000-resample BCa 95% intervals for the paired `jj-log /
git-log` ratios. Smoke output can validate the harness but sets `registered_analysis_valid` to
false because it does not satisfy the fixed-n analysis.

## Constraints

The qualification does not promise migration from old 0.23 installations, an adapter parser
contract, or `jj-run` semantic qualification. The #33 Atom owns `jj run` semantics. The benchmark
times only machine-readable `git log` and `jj log` reads; it does not estimate workspace-add cost,
adapter completion rate, or semantic equivalence.
