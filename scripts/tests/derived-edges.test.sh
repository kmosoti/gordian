#!/usr/bin/env bash
# check-derived-edges.sh: an absent PlanRevision is "not yet"; a present one that does not rebuild
# is "broken". The two must not look alike.
set -euo pipefail
# shellcheck source=scripts/tests/harness.sh
# shellcheck disable=SC1091
. "$(dirname "$0")/harness.sh"

checker="$REPO_ROOT/scripts/check-derived-edges.sh"
docs=(docs/spec/mission-graph.md docs/spec/invariants.md docs/spec/data-model.md)

empty="$(new_fixture "${docs[@]}")"
expect_ok "empty subject exits 0" bash "$checker" "$empty"

silent="$(new_fixture "${docs[@]}")"
: > "$silent/docs/spec/mission-graph.md"
expect_fail "empty subject with the rule text deleted fails rather than skipping" \
  bash "$checker" "$silent"

broken="$(new_fixture "${docs[@]}")"
mkdir -p "$broken/artifacts/plans/p1"
printf '{ this is not json' > "$broken/artifacts/plans/p1/plan.json"
expect_fail "unparsable plan fails" bash "$checker" "$broken"

write_plan() {
  cat > "$1" <<'JSON'
{
  "id": "plan-1",
  "lifecycle_state": "published",
  "published_digest": "0000000000000000000000000000000000000000000000000000000000000000",
  "members": [
    { "atom_id": "atom-a", "provided_interfaces": ["interface://x"], "declared_outputs": [] },
    { "atom_id": "atom-b", "required_interfaces": ["interface://x"], "declared_inputs": [] }
  ],
  "external_provisions": [],
  "provider_bindings": [
    { "consumer_atom": "atom-b", "requirement": "interface://x",
      "requirement_kind": "required_interface", "provider": "atom-a" }
  ],
  "derived_hard_dependencies": [
    { "depender": "atom-b", "prerequisite": "atom-a", "origin": "derived_interface",
      "requirement": "interface://x" }
  ]
}
JSON
}

good="$(new_fixture "${docs[@]}")"
mkdir -p "$good/artifacts/plans/p1"
write_plan "$good/artifacts/plans/p1/plan.json"
expect_ok "a plan whose derived edges rebuild exactly passes" bash "$checker" "$good"

dropped="$(new_fixture "${docs[@]}")"
mkdir -p "$dropped/artifacts/plans/p1"
write_plan "$dropped/artifacts/plans/p1/plan.json"
python3 - "$dropped/artifacts/plans/p1/plan.json" <<'PY'
import json, sys
path = sys.argv[1]
plan = json.load(open(path))
plan["derived_hard_dependencies"] = []
json.dump(plan, open(path, "w"), indent=2)
PY
expect_fail "a stored edge set that omits a rebuilt edge fails" bash "$checker" "$dropped"

unbound="$(new_fixture "${docs[@]}")"
mkdir -p "$unbound/artifacts/plans/p1"
write_plan "$unbound/artifacts/plans/p1/plan.json"
python3 - "$unbound/artifacts/plans/p1/plan.json" <<'PY'
import json, sys
path = sys.argv[1]
plan = json.load(open(path))
plan["provider_bindings"].append(dict(plan["provider_bindings"][0]))
json.dump(plan, open(path, "w"), indent=2)
PY
expect_fail "two ProviderBindings for one requirement fail" bash "$checker" "$unbound"
