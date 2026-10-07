#!/usr/bin/env python3
"""W2: hashes of every raw file the report's numbers come from.

Usage: w2_provenance.py [--out-dir DIR]

Writes `w2-provenance.csv`: path (relative to the repository root), bytes and sha256 of the hidden
tables of the three ranges, the two ceiling runs (every arm file, including the kept ledgers), the
manifests, and the two binaries (the `laws` example and `gordian-run`), with the git revision of the
tree. The chief compares them with a rerun.
"""

import argparse
import hashlib
import pathlib
import subprocess

import w2_common as C


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
    files = []
    for sub in ("hidden", "w2-tune-b5-rho0.7", "w2-heldout-b5-rho0.7", "_manifests"):
        base = C.RUNS / sub
        files += sorted(p for p in base.rglob("*") if p.is_file() and p.name != "usage.json")
    files += [root / "target" / "debug" / "examples" / "laws", root / "target" / "release" / "gordian-run"]
    rows = [{"path": f"(git revision of the tree when written)", "bytes": "", "sha256": rev}]
    for p in files:
        rows.append({"path": str(p.relative_to(root)), "bytes": p.stat().st_size, "sha256": sha(p)})
    C.write_csv(args.out_dir / "w2-provenance.csv", rows)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
