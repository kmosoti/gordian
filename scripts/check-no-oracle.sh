#!/usr/bin/env bash
# Fail if any Rust source outside the allowlist mentions `oracle::` or `reveal(`.
#
# Hidden simulator state must never reach a policy's inputs. The cargo feature
# `reveal-hidden-state` is a guard, not a proof, because cargo unifies features across a build.
# This script is the second guard: a textual check that only the places allowed to read hidden
# state mention the accessor at all.
#
# Allowlist (path prefixes relative to the repository root):
#   crates/gordian-world/                      the crate that defines the accessor
#   crates/gordian-eval/                       the evaluator
#   crates/gordian-run/src/policy/oracle.rs    the privileged oracle baseline (future path)
#
# It is a grep, so it is conservative in one direction and blind in another: an unrelated
# `reveal(` in a comment elsewhere fails it, while a macro that builds the path from pieces is
# not caught. Review remains the third guard.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

allow_re='^(crates/gordian-world/|crates/gordian-eval/|crates/gordian-run/src/policy/oracle\.rs$)'
pattern='oracle::|reveal\('

status=0
# Tracked and untracked-but-not-ignored Rust files, so the check works before the first commit.
while IFS= read -r file; do
    if [[ "$file" =~ $allow_re ]]; then
        continue
    fi
    if grep -nE "$pattern" "$file" >/dev/null 2>&1; then
        echo "check-no-oracle: forbidden reference in $file" >&2
        grep -nE "$pattern" "$file" | sed 's/^/    /' >&2
        status=1
    fi
done < <(git ls-files --cached --others --exclude-standard -- '*.rs')

if [[ $status -eq 0 ]]; then
    echo "check-no-oracle: ok"
fi
exit $status
