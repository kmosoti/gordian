"""Run A1a manifests one after another through `scripts/run-driver.sh`, logging each run (L1's
`scripts/l1_run.py`, with A1a's directories).

Usage: a1a_run.py [--name NAME] MANIFEST...   (--name: the run directory's name, one manifest)

Needs a clean tree whose HEAD is each manifest's `source_revision` and the release binary built
from that tree. The machine is shared with other labs: before each run this waits, polling every
30 s, until no `gordian-run`, `cargo` or `rustc` process exists, logging every wait. The driver does
the isolation (scripts/cgroup-run.sh, cores 0-2, itself on core 3). The run directory is
artifacts/runs/a1a/<run_id>; the log is artifacts/runs/a1a/_logs/a1a-runs.log and each run's
output artifacts/runs/a1a/_logs/<run_id>.log. Nothing is deleted.
"""

import json
import subprocess
import sys
import time

import a1a_common as C

LOGS = C.SMOKE_DIR / "_logs"


def busy():
    names = []
    for name in ("gordian-run", "cargo", "rustc"):
        if subprocess.run(["pgrep", "-x", name], capture_output=True).returncode == 0:
            names.append(name)
    return names


def main():
    args = sys.argv[1:]
    out_name = None
    if args[:1] == ["--name"]:
        out_name, args = args[1], args[2:]
    if not args or (out_name and len(args) != 1):
        sys.exit(__doc__)
    LOGS.mkdir(parents=True, exist_ok=True)
    log = open(LOGS / "a1a-runs.log", "a")
    for m in args:
        rid = json.load(open(m))["run_id"]
        name = out_name or (rid if not rid.startswith("r6-") else f"a1a-xcheck-{rid}")
        waits = 0
        while (who := busy()):
            print(f"{name} waiting-for {' '.join(who)} {time.strftime('%FT%T')}", file=log, flush=True)
            waits += 1
            time.sleep(30)
        start = time.time()
        with open(LOGS / f"{name}.log", "w") as out:
            status = subprocess.run(
                [str(C.ROOT / "scripts" / "run-driver.sh"), "--manifest", str(m), "--out",
                 str(C.SMOKE_DIR / name)],
                cwd=C.ROOT, stdout=out, stderr=subprocess.STDOUT,
            ).returncode
        wall = time.time() - start
        print(f"{name} exit={status} wall_s={wall:.0f} waits={waits}", file=log, flush=True)
        print(f"{name} exit={status} wall_s={wall:.0f} waits={waits}")


if __name__ == "__main__":
    main()
