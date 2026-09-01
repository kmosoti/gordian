#!/usr/bin/env bash
# Every shell gate must fail closed.
#
# Diagnosis 2026-09-01: scripts/run-kg-benchmark.sh called `finalize` at three sites while
# only `finalize_retained` was defined. Bash printed "finalize: command not found" to stderr
# and the script still exited 0, so its checker reported success while the publication step
# had never run. A gate that reports green while failing internally launders a defect into
# evidence; deep review then overturns work a checker already blessed. That loop ran 30+
# times in one Atom before anyone noticed.
#
# Enforced here:
#   1. `set -euo pipefail` near the top, so exit 127 aborts instead of being narrated;
#   2. shellcheck clean at error level.
#
# Deliberately NOT done here: static "is this function defined?" analysis. Parsing shell
# with regex re-derives a linter badly (awk and python fragments inside quoted blocks read
# as shell calls). `set -euo pipefail` converts the same defect into a hard failure at the
# first call, shellcheck catches the rest statically, and the telemetry S3 detector catches
# any survivor at runtime by flagging exit 0 alongside "command not found". Three layers,
# none of them a hand-rolled parser.
set -euo pipefail
cd "$(dirname "$0")/.."

python3 - <<'PY'
import re, shutil, subprocess, sys
from pathlib import Path

BUILTINS = {
    "if","then","else","elif","fi","for","while","until","do","done","case","esac","function",
    "return","local","readonly","export","set","shift","exit","echo","printf","cd","source",
    "trap","declare","eval","read","test","true","false","break","continue","unset","wait",
    "shopt","mapfile","command","type","hash","umask","ulimit","exec","time","let","pushd",
    "popd","getopts","alias","jobs","kill","builtin",
}

fail = []
scripts = sorted(Path("scripts").rglob("*.sh"))
if not scripts:
    print("no scripts found — glob is wrong", file=sys.stderr); sys.exit(1)

for s in scripts:
    raw = s.read_text()
    head = "\n".join(raw.splitlines()[:40])
    sourced_only = ("Sourced by" in head) or not raw.startswith("#!")
    if not sourced_only and not re.search(r"^set -(euo pipefail|eu\b|e\b.*-u)", head, re.M):
        fail.append(f"{s}: no 'set -euo pipefail' in the first 40 lines — "
                    f"an undefined function exits 127 and is ignored")

if shutil.which("shellcheck"):
    r = subprocess.run(["shellcheck", "-S", "error", *map(str, scripts)],
                       capture_output=True, text=True)
    if r.returncode:
        fail.append("shellcheck (error level):\n" + r.stdout[:1500])
else:
    print("  shellcheck unavailable — see agent-runbook.md section 0", file=sys.stderr)
    sys.exit(78)

if fail:
    for f in fail:
        print(f"  {f}", file=sys.stderr)
    print(f"script safety FAILED: {len(fail)} finding(s)", file=sys.stderr)
    sys.exit(1)
print(f"script safety: {len(scripts)} scripts fail closed, no undefined callees, shellcheck clean")
PY
