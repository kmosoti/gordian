#!/usr/bin/env python3
"""W3: hashes of every raw file the report's numbers come from.

Usage: w3_provenance.py [--out-dir DIR]

Writes `w3-provenance.csv`: path (relative to the repository root, or absolute for files this unit
only reads), bytes and sha256 of the hidden tables of the five ranges, the two world-C ceiling runs
(every arm file, including the kept ledgers), the manifests, the two binaries (the `laws` example
and `gordian-run`), and the read-only inputs: W2's world-A runs and hidden tables used for the
comparison, E1's held-out run (its `recalls.csv` files) and A2's smoke files. The git revision of the
tree is the first row. The chief compares them with a rerun.
"""

import argparse
import hashlib
import pathlib
import subprocess

import w2_common as C

W2 = pathlib.Path("/home/user/gordian/artifacts/runs/w2")
E1 = pathlib.Path("/home/user/gordian/artifacts/runs/e1/e1-heldout-b5-rho0.7")
A2 = pathlib.Path("/home/user/gordian/artifacts/runs/a2/a2-smoke-b5-rho0.7")


def sha(path: pathlib.Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def main(argv=None) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out-dir", type=pathlib.Path, default=C.OUT)
    args = ap.parse_args(argv)
    root = C.ROOT
    rev = subprocess.run(["git", "rev-parse", "HEAD"], cwd=root, capture_output=True, text=True, check=True).stdout.strip()
    w3 = root / "artifacts" / "runs" / "w3"
    files = []
    for sub in ("hidden", "w3-ctune-b5-rho0.7", "w3-cheldout-b5-rho0.7", "_manifests"):
        files += sorted(p for p in (w3 / sub).rglob("*") if p.is_file() and p.name != "usage.json")
    files += [root / "target" / "debug" / "examples" / "laws", root / "target" / "release" / "gordian-run"]
    files += sorted(p for p in (W2 / "hidden").rglob("*") if p.is_file() and p.name != "usage.json")
    for run in ("w2-tune-b5-rho0.7", "w2-heldout-b5-rho0.7"):
        files += sorted(p for p in (W2 / run).rglob("*") if p.is_file() and p.name != "usage.json")
    files += sorted(E1.glob("*/recalls.csv"))
    for name in ("predictions-a2_learn.csv", "predictions-a2_raw.csv", "first-alarms-a2_learn.csv", "first-alarms-a2_raw.csv",
                 "edges-a2_learn.csv", "edges-a2_raw.csv"):
        files.append(A2 / name)
    rows = [{"path": "(git revision of the tree when written)", "bytes": "", "sha256": rev}]
    for p in files:
        try:
            rel = str(p.relative_to(root))
        except ValueError:
            rel = str(p)
        rows.append({"path": rel, "bytes": p.stat().st_size, "sha256": sha(p)})
    C.write_csv(args.out_dir / "w3-provenance.csv", rows)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
