"""Run A1d manifests one after another through `scripts/run-driver.sh`, logging each run (A1c's
`a1c_run.py` with A1d's directories and the trace directory).

Usage: a1d_run.py NAME MANIFEST   (NAME: the run directory's name under artifacts/runs/a1d/)

Needs a clean tree whose HEAD is the manifest's `source_revision` and the release binary built
from that tree's code. Before the run this waits, polling every 30 s, until no `gordian-run`,
`cargo` or `rustc` process exists, logging every wait. The driver does the isolation
(scripts/cgroup-run.sh, cores 0-2, itself on core 3). The environment variable
GORDIAN_ENGRAM_TRACE_DIR is set to <run dir>/_trace, where the engram arms with `trace` on write
their files (`engram-trace-<state_key>.csv`); the script refuses to start if that directory already
holds a file. The log is artifacts/runs/a1d/_logs/a1d-runs.log and each run's output
artifacts/runs/a1d/_logs/<name>.log. Nothing is deleted.
"""

import json
import os
import subprocess
import sys
import time

import a1d_common as C

LOGS = C.RUN_DIR / "_logs"


def busy():
    names = []
    for name in ("gordian-run", "cargo", "rustc"):
        if subprocess.run(["pgrep", "-x", name], capture_output=True).returncode == 0:
            names.append(name)
    return names


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    name, m = sys.argv[1], sys.argv[2]
    json.load(open(m))  # readable
    LOGS.mkdir(parents=True, exist_ok=True)
    out_dir = C.RUN_DIR / name
    trace = out_dir / C.TRACE_DIR
    if trace.exists() and any(trace.iterdir()):
        sys.exit(f"{trace} already holds trace files; a recorded run is never overwritten")
    log = open(LOGS / "a1d-runs.log", "a")
    waits = 0
    while (who := busy()):
        print(f"{name} waiting-for {' '.join(who)} {time.strftime('%FT%T')}", file=log, flush=True)
        waits += 1
        time.sleep(30)
    env = dict(os.environ, **{C.TRACE_ENV: str(trace)})
    start = time.time()
    with open(LOGS / f"{name}.log", "w") as out:
        status = subprocess.run(
            [str(C.ROOT / "scripts" / "run-driver.sh"), "--manifest", str(m), "--out", str(out_dir)],
            cwd=C.ROOT, stdout=out, stderr=subprocess.STDOUT, env=env,
        ).returncode
    wall = time.time() - start
    print(f"{name} exit={status} wall_s={wall:.0f} waits={waits}", file=log, flush=True)
    print(f"{name} exit={status} wall_s={wall:.0f} waits={waits}")


if __name__ == "__main__":
    main()
