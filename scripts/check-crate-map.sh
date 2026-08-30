#!/usr/bin/env bash
# docs/implementation/crate-map.md is the only declaration of cross-crate dependency direction
# (G-517). This asserts:
#   (a) every crates/ url in knowledge/graph/*.jsonld appears as a row;
#   (b) every Cargo.toml workspace member appears as a row;
#   (c) every [dependencies] entry naming a gordian-* crate appears in that crate's
#       `May depend on` column;
#   (d) the permitted-dependency relation is acyclic and each row is transitively closed, which is
#       what the column claims to be.
#
# Usage: check-crate-map.sh [ROOT]
set -euo pipefail
root="${1:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$root"

map=docs/implementation/crate-map.md

if [ ! -f "$map" ]; then
  echo "FAIL: $map is missing; cross-crate dependency direction is undeclared"
  exit 1
fi

python3 - "$map" <<'PY'
"""Parse the crate map table and check it against the graph, the workspace, and itself."""

import glob
import os
import re
import sys
import tomllib

map_path = sys.argv[1]
problems = []

with open(map_path, encoding="utf-8") as handle:
    map_lines = handle.read().splitlines()

ROW = re.compile(
    r"^\|\s*`(?P<crate>[a-z0-9-]+)`\s*\|\s*`(?P<path>crates/[a-z0-9-]+)`\s*\|"
    r"(?P<deps>[^|]*)\|(?P<atoms>[^|]*)\|\s*$"
)

rows = {}
for line in map_lines:
    match = ROW.match(line)
    if not match:
        continue
    crate = match.group("crate")
    path = match.group("path")
    cell = match.group("deps").strip()
    if crate in rows:
        problems.append(f"{map_path}: crate {crate!r} has two rows")
        continue
    if os.path.basename(path) != crate:
        problems.append(f"{map_path}: row {crate!r} declares path {path!r}")
    if cell in ("(none)", "none", ""):
        deps = set()
    else:
        deps = set(re.findall(r"`([a-z0-9-]+)`", cell))
        if not deps:
            problems.append(f"{map_path}: row {crate!r} has an unparsable `May depend on` cell: {cell!r}")
    rows[crate] = {"path": path, "deps": deps}

if not rows:
    problems.append(f"{map_path}: no crate rows found; the table is the declaration")

by_path = {row["path"]: crate for crate, row in rows.items()}

# (a) every crates/ url in the knowledge graph has a row.
for graph in sorted(glob.glob("knowledge/graph/*.jsonld")):
    with open(graph, encoding="utf-8") as handle:
        text = handle.read()
    for url in sorted(set(re.findall(r'"url"\s*:\s*"(crates/[a-z0-9-]+)"', text))):
        if url not in by_path:
            problems.append(f"{graph}: ImplementationArtifact url {url!r} has no row in {map_path}")

# (b) every workspace member has a row.
members = []
if os.path.isfile("Cargo.toml"):
    with open("Cargo.toml", "rb") as handle:
        workspace = tomllib.load(handle)
    members = workspace.get("workspace", {}).get("members", [])
    for member in members:
        if member not in by_path:
            problems.append(f"Cargo.toml: workspace member {member!r} has no row in {map_path}")
else:
    problems.append("Cargo.toml: absent; the workspace member list is the subject of check (b)")

# (c) every declared gordian-* dependency is permitted by the row.
for member in members:
    manifest = os.path.join(member, "Cargo.toml")
    if not os.path.isfile(manifest):
        problems.append(f"{manifest}: workspace member {member!r} has no manifest")
        continue
    with open(manifest, "rb") as handle:
        crate_manifest = tomllib.load(handle)
    crate = crate_manifest.get("package", {}).get("name", os.path.basename(member))
    row = rows.get(by_path.get(member, crate))
    permitted = row["deps"] if row else set()
    for dependency in sorted(crate_manifest.get("dependencies", {})):
        if not dependency.startswith("gordian-"):
            continue
        if dependency not in permitted:
            problems.append(
                f"{manifest}: {crate!r} depends on {dependency!r}, which is not in its "
                f"`May depend on` column {sorted(permitted) or ['(none)']}"
            )

# (d) the relation is acyclic and every row is transitively closed.
for crate, row in sorted(rows.items()):
    if crate in row["deps"]:
        problems.append(f"{map_path}: {crate!r} lists itself in `May depend on`")
    for dependency in sorted(row["deps"]):
        if dependency not in rows:
            problems.append(f"{map_path}: {crate!r} may depend on {dependency!r}, which has no row")
            continue
        missing = rows[dependency]["deps"] - row["deps"] - {crate}
        if missing:
            problems.append(
                f"{map_path}: {crate!r} may depend on {dependency!r} but omits its transitive "
                f"dependencies {sorted(missing)}; the column is declared transitively closed"
            )

state = {}
order = []


def visit(crate):
    state[crate] = "open"
    order.append(crate)
    for dependency in sorted(rows.get(crate, {}).get("deps", ())):
        if dependency not in rows:
            continue
        if state.get(dependency) == "open":
            cycle = order[order.index(dependency):] + [dependency]
            problems.append(f"{map_path}: dependency cycle {' -> '.join(cycle)}")
            continue
        if dependency not in state:
            visit(dependency)
    state[crate] = "done"
    order.pop()


for crate in sorted(rows):
    if crate not in state:
        visit(crate)

if problems:
    for problem in problems:
        print(f"FAIL: {problem}")
    raise SystemExit(1)

print(
    f"OK: {len(rows)} crate rows cover every graph url and every workspace member; "
    "every declared dependency is permitted; the relation is acyclic and transitively closed."
)
PY
