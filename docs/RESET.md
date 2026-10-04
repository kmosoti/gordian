# Completing the reset

This branch carries the new Gordian alongside the old tree because the automated session that
prepared it was not permitted to delete or overwrite tracked files. One command finishes the job.
Run it from the repository root on this branch, review `git status`, then commit.

```bash
set -euo pipefail

# 1. Remove everything from the previous project except the license and the new files.
git ls-files -z \
  | grep -zvE '^(LICENSE|rust-toolchain\.toml|README\.new\.md|AGENTS\.new\.md|Cargo\.new\.toml|\.gitignore\.new|docs/charter\.md|docs/plan-review\.md|docs/RESET\.md|experiments/TEMPLATE\.md|crates/gordian-core/.*|\.github/workflows/ci\.yml)$' \
  | xargs -0 git rm -q --
find . -mindepth 1 -maxdepth 1 ! -name .git ! -name LICENSE ! -name rust-toolchain.toml \
  ! -name 'README.new.md' ! -name 'AGENTS.new.md' ! -name 'Cargo.new.toml' ! -name '.gitignore.new' \
  ! -name docs ! -name experiments ! -name crates ! -name .github -exec rm -rf {} +
rm -rf .github/ISSUE_TEMPLATE .github/PULL_REQUEST_TEMPLATE.md .github/dependabot.yml .github/workflows/verify.yml

# 2. Move the replacements into place.
git mv -f README.new.md README.md
git mv -f AGENTS.new.md AGENTS.md
git mv -f Cargo.new.toml Cargo.toml
git mv -f .gitignore.new .gitignore
git rm -q --cached Cargo.lock 2>/dev/null || true
rm -f Cargo.lock

# 3. Regenerate the lockfile and verify.
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add -A
git status --short
```

After it runs, delete this file: `git rm docs/RESET.md`.
