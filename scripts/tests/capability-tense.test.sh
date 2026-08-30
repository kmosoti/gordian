#!/usr/bin/env bash
# check-capability-tense.sh: a capability in the README's "Planned, not built" table may not be
# claimed in the present tense until every Atom in its unblocking cell has a closure record.
set -euo pipefail
# shellcheck source=scripts/tests/harness.sh
# shellcheck disable=SC1091
. "$(dirname "$0")/harness.sh"

checker="$REPO_ROOT/scripts/check-capability-tense.sh"
docs=(README.md AGENTS.md)

expect_ok "the repository as it stands passes" bash "$checker" "$REPO_ROOT"

claimed="$(new_fixture "${docs[@]}")"
printf '\nThe scheduler dispatches Atoms across workers today.\n' >> "$claimed/AGENTS.md"
expect_fail "a present-tense claim in AGENTS.md with no closure record fails" \
  bash "$checker" "$claimed"

variant="$(new_fixture "${docs[@]}")"
printf '\nRust is the production substrate.\n' >> "$variant/README.md"
expect_fail "a near-synonym of a table label fails too" bash "$checker" "$variant"

notable="$(new_fixture "${docs[@]}")"
python3 - "$notable/README.md" <<'PY'
import re
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
open(path, "w", encoding="utf-8").write(re.sub(r"^#{2,4} +Planned, not built\s*$", "### Other",
                                               text, count=1, flags=re.M))
PY
expect_fail "a README with no 'Planned, not built' table fails" bash "$checker" "$notable"

unlinked="$(new_fixture "${docs[@]}")"
python3 - "$unlinked/AGENTS.md" <<'PY'
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
open(path, "w", encoding="utf-8").write(text.replace("scripts/check-capability-tense.sh", "a script"))
PY
expect_fail "AGENTS.md dropping the checker reference fails" bash "$checker" "$unlinked"

closed="$(new_fixture "${docs[@]}")"
printf '\nThe scheduler dispatches Atoms across workers.\n' >> "$closed/AGENTS.md"
for atom in 20 21 24; do
  mkdir -p "$closed/artifacts/atoms/$atom"
  printf '{"record_format":"gordian-closure-v1","atom_id":"%s"}\n' "$atom" \
    > "$closed/artifacts/atoms/$atom/closure.json"
done
expect_ok "the same claim passes once every unblocking Atom has a closure record" \
  bash "$checker" "$closed"
