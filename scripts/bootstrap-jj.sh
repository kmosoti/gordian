#!/usr/bin/env bash
set -euo pipefail

readonly DEFAULT_JJ_VERSION="0.44.0"
readonly DEFAULT_REMOTE_NAME="origin"
readonly DEFAULT_REMOTE_URL="https://github.com/kmosoti/gordian"

jj_version="${JJ_REQUIRED_VERSION:-$DEFAULT_JJ_VERSION}"
remote_name="${GORDIAN_REMOTE_NAME:-$DEFAULT_REMOTE_NAME}"
remote_url="${GORDIAN_REMOTE_URL:-$DEFAULT_REMOTE_URL}"
install=false
install_only=false

usage() {
  cat <<'EOF'
Usage: ./scripts/bootstrap-jj.sh [--install | --install-only]

Safely configure an existing Gordian Jujutsu repository.

  --install  Install the pinned candidate Jujutsu release with Cargo first.
  --install-only  Install/check Jujutsu without requiring a repository.

Environment overrides:
  JJ_REQUIRED_VERSION   Candidate Jujutsu release to qualify.
  GORDIAN_REMOTE_NAME   Git remote name. Defaults to origin.
  GORDIAN_REMOTE_URL    Canonical Git remote URL.

The script never initializes a repository, rewrites changes, pushes, moves main,
or creates releases.
EOF
}

for arg in "$@"; do
  case "$arg" in
    --install)
      install=true
      ;;
    --install-only)
      install=true
      install_only=true
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf 'unknown argument: %s\n' "$arg" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ "$install" == true ]]; then
  command -v cargo >/dev/null 2>&1 || {
    echo "cargo is required for --install" >&2
    exit 1
  }
  cargo install \
    --locked \
    --force \
    --version "$jj_version" \
    --bin jj \
    jj-cli
  hash -r
fi

command -v jj >/dev/null 2>&1 || {
  echo "jj is not installed; rerun with --install" >&2
  exit 1
}

actual_version="$(jj --version | awk 'NR == 1 { print $2 }')"
if [[ "$actual_version" != "$jj_version" ]]; then
  cat >&2 <<EOF
unsupported Jujutsu candidate: found $actual_version, expected $jj_version
rerun with --install or set JJ_REQUIRED_VERSION only for qualification work
EOF
  exit 1
fi

if [[ "$install_only" == true ]]; then
  printf 'Jujutsu %s is installed and matches the repository pin.\n' "$actual_version"
  exit 0
fi

if [[ ! -d .jj ]]; then
  cat >&2 <<EOF
current directory ($PWD) is not an existing Jujutsu repository (.jj is missing)
refusing to initialize automatically; cd to the Gordian repository root and rerun
EOF
  exit 1
fi

remote_line="$(jj git remote list | awk -v name="$remote_name" '$1 == name { print; exit }')"
if [[ -z "$remote_line" ]]; then
  jj git remote add "$remote_name" "$remote_url"
else
  actual_remote_url="$(awk '{ print $2 }' <<<"$remote_line")"
  if [[ "$actual_remote_url" != "$remote_url" ]]; then
    cat >&2 <<EOF
remote $remote_name points to $actual_remote_url, expected $remote_url
refusing to replace an existing remote automatically
EOF
    exit 1
  fi
fi

# An actor identity is required before Jujutsu will create a commit: without it,
# `jj git push` refuses with "no author and/or committer set", which is how CI's
# specification-consistency job failed while every local run passed. Only set it when
# absent, so a developer's own identity is never overwritten.
if ! jj config get user.name >/dev/null 2>&1; then
  jj config set --repo user.name "${GORDIAN_ACTOR:-gordian-agent/ci/unattended}"
fi
if ! jj config get user.email >/dev/null 2>&1; then
  jj config set --repo user.email "${GORDIAN_ACTOR_EMAIL:-agents@gordian.invalid}"
fi

# Current Jujutsu stores repository-scoped configuration outside tracked source.
jj config set --repo git.fetch "[\"$remote_name\"]"
jj config set --repo git.push "$remote_name"
jj git fetch
jj bookmark track main --remote="$remote_name"
jj config set --repo 'revset-aliases."trunk()"' "main@$remote_name"

# Capability checks are stronger than a version string alone, although issue #1
# still owns the complete disposable-repository contract qualification.
jj run --help >/dev/null
jj log -r "main@$remote_name" -n 1 --no-graph >/dev/null
jj log -r 'trunk()' -n 1 --no-graph >/dev/null

cat <<EOF
Gordian Jujutsu development configuration is ready.
  jj:      $(command -v jj) ($actual_version)
  remote:  $remote_name -> $remote_url
  trunk(): main@$remote_name

No changes were pushed and no source history was rewritten.
Next: execute issue #1's disposable-repository contract and performance suite.
EOF
