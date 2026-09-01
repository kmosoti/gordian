#!/usr/bin/env bash
# Assert that every workflow step which installs a tool is safe to re-run on a runner
# whose cache was restored.
#
# Why this exists: caching the compiled `jj` binary cut 55% off the pipeline, but a
# `cargo install` refuses to overwrite a binary that is already present —
# "binary `jj` already exists in destination", exit 101. The commit that introduced the
# cache passed CI, because on that run the cache was still cold and was only written at
# the end. The *next* commit was the first cache hit, and it failed. A cache makes the
# second run take a different path through the same YAML than the first, so a green run
# is not evidence that the step is re-runnable.
#
# The gate groups in verify-local.sh cannot see this class at all: these commands live in
# workflow setup steps, not in the verifiers. So this checker reads the workflow directly
# and enforces the invariant the rust job already follows for cargo-deny — a restored
# binary is either checked before installing, or the install is told to overwrite it.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

python3 - "$root" <<'PY'
import pathlib
import re
import sys

root = pathlib.Path(sys.argv[1])
workflows = sorted((root / ".github" / "workflows").glob("*.yml"))
if not workflows:
    print("FAILED: no workflow files found under .github/workflows", file=sys.stderr)
    raise SystemExit(1)

# A guard proves the step looked before it installed; --force proves it may overwrite.
GUARD = re.compile(r"command -v |\[ -x |\[\[ -x |--force")
INSTALL = re.compile(r"\bcargo install\b|\bnpm (?:install|i) -g\b|\bpipx install\b|\bgo install\b")

failures = []
checked = 0

for wf in workflows:
    lines = wf.read_text().splitlines()
    i = 0
    while i < len(lines):
        line = lines[i]
        stripped = line.lstrip()
        # A block scalar: `run: |` or `run: >`, possibly with a chomping indicator.
        if re.match(r"run:\s*[|>][-+]?\s*$", stripped):
            indent = len(line) - len(stripped)
            body, j = [], i + 1
            while j < len(lines):
                nxt = lines[j]
                if nxt.strip() and (len(nxt) - len(nxt.lstrip())) <= indent:
                    break
                body.append(nxt)
                j += 1
            block, start = "\n".join(body), i + 2
            i = j
        else:
            # A single-line `run: cargo install ...`.
            m = re.match(r"run:\s*(\S.*)$", stripped)
            if not m:
                i += 1
                continue
            block, start = m.group(1), i + 1
            i += 1

        if not INSTALL.search(block):
            continue
        checked += 1
        if not GUARD.search(block):
            offending = next(
                (ln.strip() for ln in block.splitlines() if INSTALL.search(ln)),
                block.strip(),
            )
            failures.append(
                f"{wf.relative_to(root)}:{start}: installs a tool with no cache guard and "
                f"no --force, so it fails once the cache is warm\n"
                f"             {offending}"
            )

for f in failures:
    print(f"FAIL: {f}", file=sys.stderr)
if failures:
    print(
        "\nEvery install step must survive a restored cache. Follow the pattern the rust "
        "job\nuses for cargo-deny: check for the binary (and its pinned version) first, "
        "and only\ninstall when it is absent or wrong.",
        file=sys.stderr,
    )
    raise SystemExit(1)

print(f"OK: {checked} workflow install step(s) are re-runnable against a warm cache.")
PY
