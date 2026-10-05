"""Write every manifest of the Stage B runs (pilot, B1, S1, S2, S3). Needs the release gordian-run binary and a clean tree at the commit to be run.

Stage B exploration script (development run; nothing here tests a hypothesis).
Working directory for outputs and manifests: $GORDIAN_WORK (default /tmp/gordian-exploration).
The repository root is found from this file's location.
"""
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
W = os.environ.get("GORDIAN_WORK", "/tmp/gordian-exploration")
os.makedirs(f"{W}/manifests", exist_ok=True)


def mk(run_id, c, run_seed, out, armset="b1", noise=None):
    cmd = [sys.executable, f"{HERE}/mkmanifest.py", run_id, str(c), "1000", "500", str(run_seed), "3600", out, armset]
    if noise is not None:
        cmd.append(str(noise))
    subprocess.run(cmd, check=True)


# the timing pilot: 100 seeds per class
subprocess.run([sys.executable, f"{HERE}/mkmanifest.py", "b1-pilot-c20000000-s1000-1099", "20000000", "1000", "100",
                "1", "3600", f"{W}/manifests/pilot.json"], check=True)
for c in (20000000, 250000, 100000, 60000):
    mk(f"b1-c{c}-s1000-1499", c, 1, f"{W}/manifests/c{c}.json")
for c in (20000000, 250000, 100000, 60000):
    mk(f"b1s-c{c}-s1000-1499", c, 2, f"{W}/manifests/s1-c{c}.json", "s1")
for c in (15000, 20000, 30000, 40000, 50000):
    mk(f"b1s2-c{c}-s1000-1499", c, 3, f"{W}/manifests/s2-c{c}.json", "s2")
for c in (20000000, 250000, 100000, 60000):
    mk(f"b3n50-c{c}-s1000-1499", c, 4, f"{W}/manifests/s3-c{c}.json", "b1", 50)
