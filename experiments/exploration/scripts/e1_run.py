"""Run E1 manifests one after another through `scripts/run-driver.sh`, logging each run (A1a's
`a1a_run.py`, with E1's directories).

Usage: e1_run.py [--trace] [--name NAME] MANIFEST...   (--name: the run directory's name, one manifest)

Needs a clean tree whose HEAD is each manifest's `source_revision` and the release binary built
from that tree. The machine is shared with other labs: before each run this waits, polling every
30 s, until no `gordian-run`, `cargo` or `rustc` process exists, logging every wait. The driver does
the isolation (scripts/cgroup-run.sh, cores 0-2, itself on core 3). The run directory is
artifacts/runs/e1/<run_id>; the log is artifacts/runs/e1/_logs/e1-runs.log and each run's output
artifacts/runs/e1/_logs/<run_id>.log. Nothing is deleted.
"""

import json
import os
import subprocess
import sys
import time

import e1_common as C

LOGS = C.E1_DIR / "_logs"


def busy():
    names = []
    for name in ("gordian-run", "cargo", "rustc"):
        if subprocess.run(["pgrep", "-x", name], capture_output=True).returncode == 0:
            names.append(name)
    return names


def main():
    args = sys.argv[1:]
    out_name = None
    trace = False
    if args[:1] == ["--trace"]:  # A1d's engram arms write their counters under <out>/_trace
        trace, args = True, args[1:]
    if args[:1] == ["--name"]:
        out_name, args = args[1], args[2:]
    if not args or (out_name and len(args) != 1):
        sys.exit(__doc__)
    LOGS.mkdir(parents=True, exist_ok=True)
    log = open(LOGS / "e1-runs.log", "a")
    for m in args:
        rid = json.load(open(m))["run_id"]
        name = out_name or rid
        waits = 0
        while (who := busy()):
            print(f"{name} waiting-for {' '.join(who)} {time.strftime('%FT%T')}", file=log, flush=True)
            waits += 1
            time.sleep(30)
        start = time.time()
        env = dict(os.environ)
        if trace:
            env["GORDIAN_ENGRAM_TRACE_DIR"] = str(C.E1_DIR / name / "_trace")
        with open(LOGS / f"{name}.log", "w") as out:
            status = subprocess.run(
                [str(C.ROOT / "scripts" / "run-driver.sh"), "--manifest", str(m), "--out",
                 str(C.E1_DIR / name)],
                cwd=C.ROOT, stdout=out, stderr=subprocess.STDOUT, env=env,
            ).returncode
        wall = time.time() - start
        print(f"{name} exit={status} wall_s={wall:.0f} waits={waits}", file=log, flush=True)
        print(f"{name} exit={status} wall_s={wall:.0f} waits={waits}")


if __name__ == "__main__":
    main()
