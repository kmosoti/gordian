#!/usr/bin/env bash
# Derived-edge completeness (G-317).
#
# For every published PlanRevision serialized under artifacts/plans/<id>/plan.json:
#   * exactly one ProviderBinding exists per (consumer_atom, requirement) pair drawn from the
#     members' required_interfaces and declared_inputs;
#   * every binding names a requirement its consumer actually declares, and a provider that is
#     either a plan member Atom or a registered ExternalProvision;
#   * re-running the materialization rule of docs/spec/mission-graph.md over those bindings
#     reproduces exactly the stored derived hard-dependency edge set.
#
# The subject does not exist yet: no gordian-core, so no plan is published. An absent subject
# exits 0 AFTER asserting that the rule text the checker enforces is still in the specification,
# so the checker is never vacuous and never skips silently. A subject that exists and is
# malformed always fails.
#
# Usage: check-derived-edges.sh [ROOT]
set -euo pipefail
root="${1:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$root"

plans=()
while IFS= read -r plan; do
  plans+=("$plan")
done < <(find artifacts/plans -mindepth 2 -maxdepth 2 -name 'plan.json' 2>/dev/null | sort)

if [ "${#plans[@]}" -eq 0 ]; then
  # Empty subject. Assert the normative rule is still stated where the checker claims to enforce
  # it; a rule that is deleted while its checker sleeps is exactly the drift this guards.
  fail=0
  for doc in docs/spec/mission-graph.md docs/spec/invariants.md; do
    if [ ! -f "$doc" ]; then
      echo "FAIL: $doc is missing"
      fail=1
      continue
    fi
    if ! grep -qF 'scripts/check-derived-edges.sh' "$doc"; then
      echo "FAIL: $doc no longer names scripts/check-derived-edges.sh as its enforcing checker"
      fail=1
    fi
  done
  if [ ! -f docs/spec/data-model.md ]; then
    echo "FAIL: docs/spec/data-model.md is missing"
    fail=1
  elif ! grep -qiE 'exactly one\*{0,2} +`?ProviderBinding' docs/spec/data-model.md; then
    echo "FAIL: docs/spec/data-model.md no longer states the exactly-one ProviderBinding rule"
    fail=1
  fi
  if [ "$fail" -ne 0 ]; then
    exit 1
  fi
  echo "OK: no published PlanRevision under artifacts/plans/*/plan.json yet; the derived-edge rule is stated and unchanged."
  exit 0
fi

python3 - "${plans[@]}" <<'PY'
"""Rebuild each published plan's derived hard-dependency edge set and compare."""

import json
import sys

problems = []

KIND_ORIGIN = {"required_interface": "derived_interface", "declared_input": "derived_artifact"}


def as_list(value, label, path):
    if value is None:
        return []
    if not isinstance(value, list):
        problems.append(f"{path}: {label} must be an array, found {type(value).__name__}")
        return []
    return value


for path in sys.argv[1:]:
    try:
        with open(path, encoding="utf-8") as handle:
            plan = json.load(handle)
    except (OSError, ValueError) as error:
        problems.append(f"{path}: unreadable or malformed JSON: {error}")
        continue

    if not isinstance(plan, dict):
        problems.append(f"{path}: top level must be an object")
        continue

    state = plan.get("lifecycle_state")
    if state != "published":
        # Only publication freezes the edge set; a draft under artifacts/plans/ is a mistake.
        problems.append(
            f"{path}: lifecycle_state is {state!r}; only published plan revisions are serialized here"
        )
        continue
    if not plan.get("published_digest"):
        problems.append(f"{path}: a published plan revision must carry published_digest")

    members = as_list(plan.get("members"), "members", path)
    if not members:
        problems.append(f"{path}: a published plan revision has at least one member")

    member_ids = set()
    requirements = set()
    for position, member in enumerate(members):
        if not isinstance(member, dict) or not member.get("atom_id"):
            problems.append(f"{path}.members[{position}]: needs an atom_id")
            continue
        atom = member["atom_id"]
        if atom in member_ids:
            problems.append(f"{path}: atom {atom!r} appears twice in members")
        member_ids.add(atom)
        for field, kind in (
            ("required_interfaces", "required_interface"),
            ("declared_inputs", "declared_input"),
        ):
            for item in as_list(member.get(field), f"members[{position}].{field}", path):
                requirements.add((atom, str(item), kind))

    external = set()
    for position, provision in enumerate(
        as_list(plan.get("external_provisions"), "external_provisions", path)
    ):
        if not isinstance(provision, dict) or not provision.get("id"):
            problems.append(f"{path}.external_provisions[{position}]: needs an id")
            continue
        external.add(provision["id"])

    bound = {}
    for position, binding in enumerate(
        as_list(plan.get("provider_bindings"), "provider_bindings", path)
    ):
        label = f"{path}.provider_bindings[{position}]"
        if not isinstance(binding, dict):
            problems.append(f"{label}: must be an object")
            continue
        missing = [f for f in ("consumer_atom", "requirement", "requirement_kind", "provider")
                   if not binding.get(f)]
        if missing:
            problems.append(f"{label}: missing {', '.join(missing)}")
            continue
        kind = binding["requirement_kind"]
        if kind not in KIND_ORIGIN:
            problems.append(f"{label}: requirement_kind {kind!r} is not one of {sorted(KIND_ORIGIN)}")
            continue
        key = (binding["consumer_atom"], str(binding["requirement"]), kind)
        if key in bound:
            problems.append(
                f"{label}: a second ProviderBinding for {key}; exactly one is required"
            )
            continue
        if key not in requirements:
            problems.append(
                f"{label}: {key} is not declared by any member's "
                "required_interfaces or declared_inputs"
            )
            continue
        provider = binding["provider"]
        if provider not in member_ids and provider not in external:
            problems.append(
                f"{label}: provider {provider!r} is neither a plan member nor an ExternalProvision"
            )
            continue
        bound[key] = provider

    for key in sorted(requirements - set(bound)):
        problems.append(f"{path}: no ProviderBinding for {key}")

    rebuilt = set()
    for (consumer, requirement, kind), provider in bound.items():
        if provider not in member_ids:
            # An ExternalProvision is outside the plan, so it materializes no edge.
            continue
        rebuilt.add((consumer, provider, KIND_ORIGIN[kind], requirement))

    stored = set()
    for position, edge in enumerate(
        as_list(plan.get("derived_hard_dependencies"), "derived_hard_dependencies", path)
    ):
        label = f"{path}.derived_hard_dependencies[{position}]"
        if not isinstance(edge, dict):
            problems.append(f"{label}: must be an object")
            continue
        missing = [f for f in ("depender", "prerequisite", "origin", "requirement")
                   if not edge.get(f)]
        if missing:
            problems.append(f"{label}: missing {', '.join(missing)}")
            continue
        if edge["origin"] not in set(KIND_ORIGIN.values()):
            problems.append(
                f"{label}: origin {edge['origin']!r} is not one of {sorted(set(KIND_ORIGIN.values()))}"
            )
            continue
        stored.add(
            (edge["depender"], edge["prerequisite"], edge["origin"], str(edge["requirement"]))
        )

    for edge in sorted(rebuilt - stored):
        problems.append(f"{path}: rebuild produced edge {edge} that the stored set omits")
    for edge in sorted(stored - rebuilt):
        problems.append(f"{path}: stored edge {edge} is not reproduced by rebuilding")

if problems:
    for problem in problems:
        print(f"FAIL: {problem}")
    raise SystemExit(1)

print(f"OK: {len(sys.argv) - 1} published plan revision(s); derived edge sets reproduce exactly.")
PY
