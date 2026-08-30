#!/usr/bin/env bash
# check-crate-map.sh: a crate url or workspace member with no row, a dependency the row does not
# permit, and a non-transitively-closed row must all fail.
set -euo pipefail
# shellcheck source=scripts/tests/harness.sh
# shellcheck disable=SC1091
. "$(dirname "$0")/harness.sh"

checker="$REPO_ROOT/scripts/check-crate-map.sh"

expect_ok "the repository as it stands passes" bash "$checker" "$REPO_ROOT"

base_fixture() {
  fixture="$(new_fixture docs/implementation/crate-map.md Cargo.toml crates/gordian-kg/Cargo.toml)"
  mkdir -p "$fixture/knowledge/graph"
  printf '{"@graph":[{"@id":"artifact:kg","url":"crates/gordian-kg"}]}\n' \
    > "$fixture/knowledge/graph/80-implementation.jsonld"
  printf '%s\n' "$fixture"
}

nomap="$(new_fixture Cargo.toml)"
expect_fail "an absent crate map fails" bash "$checker" "$nomap"

unrowed="$(base_fixture)"
printf '{"@graph":[{"@id":"artifact:new","url":"crates/gordian-invented"}]}\n' \
  > "$unrowed/knowledge/graph/80-implementation.jsonld"
expect_fail "a graph crates/ url with no row fails" bash "$checker" "$unrowed"

member="$(base_fixture)"
python3 - "$member/Cargo.toml" <<'PY'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
open(path, "w", encoding="utf-8").write(
    text.replace('members = ["crates/gordian-kg"]',
                 'members = ["crates/gordian-kg", "crates/gordian-unmapped"]')
)
PY
mkdir -p "$member/crates/gordian-unmapped"
printf '[package]\nname = "gordian-unmapped"\nversion = "0.1.0"\n' \
  > "$member/crates/gordian-unmapped/Cargo.toml"
expect_fail "a workspace member with no row fails" bash "$checker" "$member"

forbidden="$(base_fixture)"
printf '\n[dependencies]\ngordian-core = { path = "../gordian-core" }\n' \
  >> "$forbidden/crates/gordian-kg/Cargo.toml"
expect_fail "a dependency the row does not permit fails" bash "$checker" "$forbidden"

untransitive="$(base_fixture)"
python3 - "$untransitive/docs/implementation/crate-map.md" <<'PY'
import re
import sys
path = sys.argv[1]
lines = open(path, encoding="utf-8").read().splitlines()
for index, line in enumerate(lines):
    # gordian-coordination lists gordian-evidence, which itself lists gordian-core; dropping
    # gordian-core from the coordination row breaks the transitive-closure claim.
    if re.match(r"^\| `gordian-coordination`", line):
        lines[index] = line.replace("`gordian-core`, ", "", 1)
        break
else:
    raise SystemExit("fixture assumption broken: no gordian-coordination row")
open(path, "w", encoding="utf-8").write("\n".join(lines) + "\n")
PY
expect_fail "a row that is not transitively closed fails" bash "$checker" "$untransitive"

cyclic="$(base_fixture)"
python3 - "$cyclic/docs/implementation/crate-map.md" <<'PY'
import re
import sys
path = sys.argv[1]
lines = open(path, encoding="utf-8").read().splitlines()
for index, line in enumerate(lines):
    if re.match(r"^\| `gordian-core`", line):
        cells = line.split("|")
        cells[3] = " `gordian-evidence` "
        lines[index] = "|".join(cells)
        break
else:
    raise SystemExit("fixture assumption broken: no gordian-core row")
open(path, "w", encoding="utf-8").write("\n".join(lines) + "\n")
PY
expect_fail "a cycle in the permitted-dependency relation fails" bash "$checker" "$cyclic"
