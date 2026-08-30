#!/usr/bin/env bash
# check-experiment-manifests.sh: no registered protocol is "not yet"; a protocol whose
# baseline_condition or primary_metric does not resolve, or a run whose protocol_digest does not
# match, is "broken".
set -euo pipefail
# shellcheck source=scripts/tests/harness.sh
# shellcheck disable=SC1091
. "$(dirname "$0")/harness.sh"

checker="$REPO_ROOT/scripts/check-experiment-manifests.sh"
schemas=(experiments/schema/experiment-protocol.schema.json
         experiments/schema/experiment-run.schema.json
         experiments/README.md)

protocol() {
  cat > "$1" <<'JSON'
{
  "protocol_format": "gordian-experiment-protocol-v1",
  "experiment_id": "demo",
  "class": "benchmark",
  "hypothesis": { "h0": "no difference", "h1": "a difference" },
  "falsification": "the interval excludes the minimum relevant effect",
  "conditions": [
    { "id": "baseline", "description": "the current implementation" },
    { "id": "treatment", "description": "the proposed implementation" }
  ],
  "baseline_condition": "baseline",
  "population": { "workloads": ["w1"], "sampling": "exhaustive", "seeds": [1, 2, 3] },
  "metrics": [ { "id": "latency_p50", "unit": "ms", "direction": "lower_is_better" } ],
  "analysis_plan": {
    "primary_metric": "latency_p50",
    "effect_size": { "measure": "ratio", "minimum_relevant": 1.1 },
    "min_n": 5,
    "multiplicity": "holm",
    "stopping_rule": "stop at min_n per cell"
  },
  "exclusion_rule": "drop runs whose harness crashed before the first measurement",
  "environment": { "source_commit": "0000", "toolchains": {}, "hardware_class": "ci-standard" },
  "registered_at": "2026-08-30T00:00:00Z",
  "registered_by": "gordian-agent/claude-code/test"
}
JSON
}

digest_of() {
  python3 -c 'import hashlib,sys; d=open(sys.argv[1],"rb").read().replace(b"\r\n",b"\n").replace(b"\r",b"\n"); print(hashlib.sha256(d).hexdigest())' "$1"
}

run() {
  cat > "$1" <<JSON
{
  "run_format": "gordian-experiment-run-v1",
  "run_id": "r1",
  "experiment_id": "demo",
  "protocol_digest": "$2",
  "started_at": "2026-08-30T01:00:00Z",
  "finished_at": "2026-08-30T01:10:00Z",
  "condition": "baseline",
  "seed": 1,
  "environment": { "source_commit": "0000", "toolchains": {}, "hardware_class": "ci-standard" },
  "outcome": "completed",
  "raw_artifacts": [],
  "tuning_budget_per_arm": [
    { "condition": "baseline", "unit": "person_hours", "value": 0 },
    { "condition": "treatment", "unit": "person_hours", "value": 0 }
  ],
  "metric_tooling": [],
  "nondeterminism_controls": { "temperature": 0, "seed_list": [1], "repeats_per_cell": 5 }
}
JSON
}

empty="$(new_fixture "${schemas[@]}")"
expect_ok "no registered manifests exits 0" bash "$checker" "$empty"

silent="$(new_fixture "${schemas[@]}")"
: > "$silent/experiments/README.md"
expect_fail "no manifests and a gutted README fails rather than skipping" bash "$checker" "$silent"

good="$(new_fixture "${schemas[@]}")"
mkdir -p "$good/experiments/demo/runs/r1"
protocol "$good/experiments/demo/protocol.json"
run "$good/experiments/demo/runs/r1/run.json" "$(digest_of "$good/experiments/demo/protocol.json")"
expect_ok "a valid protocol and a matching run pass" bash "$checker" "$good"

posthoc="$(new_fixture "${schemas[@]}")"
mkdir -p "$posthoc/experiments/demo/runs/r1"
protocol "$posthoc/experiments/demo/protocol.json"
run "$posthoc/experiments/demo/runs/r1/run.json" "$(printf '0%.0s' $(seq 64))"
expect_fail "a run whose protocol_digest does not match fails" bash "$checker" "$posthoc"

unresolved="$(new_fixture "${schemas[@]}")"
mkdir -p "$unresolved/experiments/demo"
protocol "$unresolved/experiments/demo/protocol.json"
python3 - "$unresolved/experiments/demo/protocol.json" <<'PY'
import json, sys
path = sys.argv[1]
p = json.load(open(path))
p["baseline_condition"] = "nonexistent"
json.dump(p, open(path, "w"), indent=2)
PY
expect_fail "a baseline_condition that resolves to nothing fails" bash "$checker" "$unresolved"

badmetric="$(new_fixture "${schemas[@]}")"
mkdir -p "$badmetric/experiments/demo"
protocol "$badmetric/experiments/demo/protocol.json"
python3 - "$badmetric/experiments/demo/protocol.json" <<'PY'
import json, sys
path = sys.argv[1]
p = json.load(open(path))
p["analysis_plan"]["primary_metric"] = "not_a_metric"
json.dump(p, open(path, "w"), indent=2)
PY
expect_fail "a primary_metric that resolves to nothing fails" bash "$checker" "$badmetric"

malformed="$(new_fixture "${schemas[@]}")"
mkdir -p "$malformed/experiments/demo"
printf '{ nope' > "$malformed/experiments/demo/protocol.json"
expect_fail "an unparsable protocol fails" bash "$checker" "$malformed"
