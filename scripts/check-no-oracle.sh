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
#   crates/gordian-stream/                     the stream crate, which defines its own accessor
#   crates/gordian-eval/                       the evaluator
#   crates/gordian-run/src/policy/oracle.rs    the privileged oracle baselines (module `privileged`)
#
# It is a grep, so it is conservative in one direction and blind in another: an unrelated
# `reveal(` in a comment elsewhere fails it, while a macro that builds the path from pieces is
# not caught. Review remains the third guard.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

allow_re='^(crates/gordian-world/|crates/gordian-stream/|crates/gordian-eval/|crates/gordian-run/src/policy/oracle\.rs$)'
pattern='oracle::|reveal\('

# Policies get a stricter check than the rest of the tree. A policy may not name the evaluator
# crate or its truth type, the generated-episode type (whose serde output includes hidden state),
# the simulator (which applies actions outside the harness's bill), or the hidden-state feature,
# as well as the pattern above. The stream crate adds the same kind of ban: a policy may not name
# the generated `Stream` (hidden tiers, labels and deadlines) or the `StreamParams` that hold the
# tier mix, the regime schedule and the reasoner's `(a, b, c)`; it may name `StreamSimulator`,
# `StreamPublic` and the action and event types. Word boundaries keep `EpisodeClass`,
# `EpisodeSpec` and `StreamSimulator` legal. Only
# `policy/oracle.rs` is exempt. It is declared as `#[path = "oracle.rs"] mod privileged;`, so the
# rest of the crate refers to it as `privileged::` and never writes `oracle::`.
policy_re='^crates/gordian-run/src/policy/'
policy_pattern='oracle::|reveal|gordian_eval|\bTruth\b|\bEpisode\b|\bSimulator\b|\bStream\b|StreamParams|StreamTruth'

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
    if [[ "$file" =~ $policy_re ]] && grep -nE "$policy_pattern" "$file" >/dev/null 2>&1; then
        echo "check-no-oracle: policy $file names something a policy may not see" >&2
        grep -nE "$policy_pattern" "$file" | sed 's/^/    /' >&2
        status=1
    fi
done < <(git ls-files --cached --others --exclude-standard -- '*.rs')

# A policy crate dependency on the hidden-state feature would show up in a manifest.
if grep -nE 'reveal-hidden-state' crates/gordian-run/Cargo.toml >/dev/null 2>&1; then
    echo "check-no-oracle: crates/gordian-run/Cargo.toml enables reveal-hidden-state" >&2
    status=1
fi

if [[ $status -eq 0 ]]; then
    echo "check-no-oracle: ok"
fi
exit $status
