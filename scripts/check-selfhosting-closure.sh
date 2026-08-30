#!/usr/bin/env bash
# The minimal self-hosting prerequisite set published in execution-order.md section 15 must be
# the computed one, and no issue in 1..77 may be orphaned from Mission completion.
#
# When a committed snapshot of the native blocked-by graph exists (artifacts/atoms/issues.json),
# both closures are recomputed from it and compared against the published list. With no snapshot
# the published arithmetic is still checked against itself: the set's cardinality must match the
# stated count, the set and the "not in the set" list must be disjoint, and together with
# {49, 68, 69, 70} they must cover 1..77 exactly -- which is the orphan rule, stated over the
# document's own numbers. On failure the computed set and the symmetric difference are printed,
# so the published list is corrected from the computation rather than argued with.
set -euo pipefail
cd "$(dirname "$0")/.."

order=docs/implementation/execution-order.md
snapshot=artifacts/atoms/issues.json

if [ ! -f "$order" ] || ! grep -q '^### Minimal self-hosting prerequisite set' "$order"; then
  echo "SKIP: $order has no '### Minimal self-hosting prerequisite set' subsection yet."
  exit 0
fi

python3 - "$order" "$snapshot" <<'PY'
import json
import os
import re
import sys

order_path, snapshot_path = sys.argv[1], sys.argv[2]
text = open(order_path, encoding="utf-8").read()

HIGHEST = 77
EXCLUDED = {49, 68, 69, 70}
ROOT = 49

section = text.split("### Minimal self-hosting prerequisite set", 1)[1]

fence = re.search(r"```text\n(.*?)\n```", section, re.S)
if not fence:
    print("FAIL: the subsection carries no fenced list of issue numbers")
    raise SystemExit(1)
published = [int(token) for token in fence.group(1).split()]

problems = []

if len(published) != len(set(published)):
    problems.append("the published list repeats an issue number")
published_set = set(published)

stated = re.search(r"\*\*(\d+) Atoms\*\*", section)
if not stated:
    problems.append("the subsection does not state its own cardinality as '**N Atoms**'")
elif int(stated.group(1)) != len(published_set):
    problems.append(
        f"the subsection says {stated.group(1)} Atoms but lists {len(published_set)}"
    )

out_of_range = sorted(n for n in published_set if not 1 <= n <= HIGHEST)
if out_of_range:
    problems.append(f"issue numbers outside 1..{HIGHEST}: {out_of_range}")
if published_set & EXCLUDED:
    problems.append(
        f"the minimal set contains excluded issues: {sorted(published_set & EXCLUDED)}"
    )


def expand(paragraph):
    numbers = set()
    for lower, upper in re.findall(r"#(\d+)\s*-\s*#?(\d+)", paragraph):
        numbers.update(range(int(lower), int(upper) + 1))
    residue = re.sub(r"#\d+\s*-\s*#?\d+", " ", paragraph)
    numbers.update(int(token) for token in re.findall(r"#(\d+)", residue))
    return numbers


excluded_paragraph = re.search(
    r"Not in the set, and deliberately so:(.*?)\n\n", section, re.S
)
if not excluded_paragraph:
    problems.append("no 'Not in the set, and deliberately so:' paragraph")
    declared_out = set()
else:
    declared_out = expand(excluded_paragraph.group(1))

overlap = sorted(published_set & declared_out)
if overlap:
    problems.append(f"issues both in and out of the minimal set: {overlap}")

covered = published_set | declared_out | EXCLUDED
uncovered = sorted(set(range(1, HIGHEST + 1)) - covered)
if uncovered:
    problems.append(f"issues in 1..{HIGHEST} named by neither list (orphans): {uncovered}")

# When the native graph is committed, recompute rather than trust the prose.
if os.path.isfile(snapshot_path):
    with open(snapshot_path, encoding="utf-8") as handle:
        issues = json.load(handle)
    if isinstance(issues, dict):
        issues = issues.get("issues", [])
    blocked_by = {}
    for issue in issues:
        number = issue.get("number")
        if number is None:
            continue
        edges = issue.get("blocked_by") or issue.get("blockedBy") or []
        blocked_by[int(number)] = {
            int(edge if not isinstance(edge, dict) else edge.get("number"))
            for edge in edges
            if edge is not None
        }

    def closure(root):
        seen, stack = set(), [root]
        while stack:
            current = stack.pop()
            for parent in blocked_by.get(current, set()):
                if parent not in seen:
                    seen.add(parent)
                    stack.append(parent)
        return seen

    computed = closure(ROOT)
    if computed != published_set:
        missing = sorted(computed - published_set)
        extra = sorted(published_set - computed)
        problems.append(
            f"closure(#{ROOT}) computed as {sorted(computed)}; "
            f"published list omits {missing} and adds {extra}"
        )
    reachable = closure(68) | closure(69) | computed | EXCLUDED
    orphans = sorted(set(range(1, HIGHEST + 1)) - reachable)
    if orphans:
        problems.append(f"orphaned from closure(#68) and closure(#69): {orphans}")
    print(f"computed closure(#{ROOT}) = {sorted(computed)}")

if problems:
    for problem in problems:
        print(f"FAIL: {problem}")
    raise SystemExit(1)

print(
    f"OK: {len(published_set)} Atoms in the minimal self-hosting set; "
    f"1..{HIGHEST} covered with no orphan."
)
PY
