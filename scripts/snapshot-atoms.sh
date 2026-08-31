#!/usr/bin/env bash
# Mirror the executable Atom contracts into the repository (G-502).
#
# GitHub is the authoritative substrate, but an agent whose sandbox has no outbound
# network — or a reviewer reading the repository alone — cannot see the contracts at all.
# This writes artifacts/atoms/issues.json: every open Atom's number, title, state,
# milestone, labels, native blocked-by edges and body, plus provenance (G-609).
#
# ORDERING CONSTRAINT (G-502). Do not run this while the GitHub change plan is unapplied.
# Snapshotting a stale graph makes scripts/check-selfhosting-closure.sh compute a closure
# of 45 with 10 orphans instead of 43 with none, which fails verifier:spec-consistency for
# EVERY Atom. This script therefore refuses to write a snapshot that does not close.
#
# Consumers read `number` and `blocked_by`; the remaining fields exist so the contracts are
# legible offline. The snapshot is a projection: regenerate it, never hand-edit it.
set -euo pipefail
cd "$(dirname "$0")/.."

out=artifacts/atoms/issues.json
mkdir -p "$(dirname "$out")"

command -v gh >/dev/null || { echo "gh not found — this script needs network and credentials" >&2; exit 78; }
gh auth status >/dev/null 2>&1 || { echo "gh cannot authenticate; see agent-runbook.md section 6.1" >&2; exit 78; }

echo "fetching open Atoms and their native blocked-by edges…"
python3 - "$out" <<'PY'
import json, subprocess, sys, datetime

out = sys.argv[1]

def gql(query, variables):
    r = subprocess.run(["gh", "api", "graphql", "--input", "-"],
                       input=json.dumps({"query": query, "variables": variables}),
                       capture_output=True, text=True)
    payload = json.loads(r.stdout) if r.stdout.strip() else {}
    if r.returncode or "errors" in payload:
        raise SystemExit(f"graphql failed: {r.stderr.strip() or json.dumps(payload.get('errors'))[:300]}")
    return payload["data"]

Q = """query($o:String!,$r:String!,$c:String){repository(owner:$o,name:$r){
  issues(first:50,after:$c,states:OPEN){
    pageInfo{hasNextPage endCursor}
    nodes{number title state body
      milestone{title}
      labels(first:20){nodes{name}}
      blockedBy(first:50){nodes{number}}}}}}"""

issues, cursor = [], None
while True:
    page = gql(Q, {"o": "kmosoti", "r": "gordian", "c": cursor})["repository"]["issues"]
    for n in page["nodes"]:
        issues.append({
            "number": n["number"],
            "title": n["title"],
            "state": n["state"],
            "milestone": (n["milestone"] or {}).get("title"),
            "labels": sorted(x["name"] for x in n["labels"]["nodes"]),
            "blocked_by": sorted(x["number"] for x in n["blockedBy"]["nodes"]),
            "body": n["body"],
        })
    if not page["pageInfo"]["hasNextPage"]:
        break
    cursor = page["pageInfo"]["endCursor"]

issues.sort(key=lambda i: i["number"])

# Refuse to emit a cyclic graph: every consumer assumes a DAG.
deps = {i["number"]: set(i["blocked_by"]) for i in issues}
colour, cycles = {}, []
def visit(u, stack):
    colour[u] = 1
    stack.append(u)
    for v in sorted(deps.get(u, ())):
        if colour.get(v) == 1:
            cycles.append(stack[stack.index(v):] + [v])
        elif colour.get(v, 0) == 0:
            visit(v, stack)
    stack.pop()
    colour[u] = 2
for n in sorted(deps):
    if colour.get(n, 0) == 0:
        visit(n, [])
if cycles:
    raise SystemExit(f"refusing to snapshot a cyclic graph: {cycles[:3]}")

head = subprocess.run(["gh", "api", "repos/kmosoti/gordian/commits/main", "--jq", ".sha"],
                      capture_output=True, text=True).stdout.strip()

document = {
    "snapshot_format": "gordian.atoms.v1",
    "generated_by": "scripts/snapshot-atoms.sh",
    "generated_at": datetime.datetime.now(datetime.UTC).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "source": "https://github.com/kmosoti/gordian/issues",
    "source_commit": head,
    "atom_count": len(issues),
    "edge_count": sum(len(i["blocked_by"]) for i in issues),
    # key MUST be "issues": that is what check-selfhosting-closure.sh and
    # check-mission-acceptance.sh read out of a dict-shaped snapshot.
    "issues": issues,
}
with open(out, "w", encoding="utf-8") as handle:
    json.dump(document, handle, indent=1, sort_keys=False)
    handle.write("\n")
print(f"wrote {out}: {len(issues)} Atoms, {document['edge_count']} edges @ {head[:8]}")
PY

# The snapshot must close, or it breaks verifier:spec-consistency for every Atom.
if ! bash scripts/check-selfhosting-closure.sh; then
  echo "snapshot does not close — reverting so it cannot poison verifier:spec-consistency" >&2
  rm -f "$out"
  exit 1
fi
echo "snapshot written and closes."
