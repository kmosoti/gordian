#!/usr/bin/env python3
"""Evaluate a criterion specification on kept run directories, from the raw evaluator files.

Usage:
    scripts/criterion.py SPEC.json --run ROLE=DIR [--run ROLE=DIR ...] --out DIR

`SPEC.json` is one of `experiments/criteria/*.json` (schema in its README). Each `--run` binds one
of the specification's run roles to a run directory (the directory that holds `manifest.json` and
one subdirectory per arm). `--out` receives `verdict.json` and `verdict.md`.

The script does no arithmetic of its own: it parses the arguments, calls
`gordian_analysis.criterion.evaluate` and writes the result. The result is deterministic under the
specification's bootstrap seed. Exit status: 0 when the verdict holds, 1 when it does not, 2 on a
specification or input error.

The package is imported from this checkout's `analysis/`, ahead of any installed copy, so that a
worktree evaluates with its own measures.
"""

import argparse
import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "analysis"))

from gordian_analysis import criterion as C  # noqa: E402


def parse_runs(items: list[str]) -> dict[str, str]:
    runs: dict[str, str] = {}
    for item in items:
        role, sep, path = item.partition("=")
        if not sep or not role or not path:
            raise C.SpecError(f"--run wants ROLE=DIR, got {item!r}")
        if role in runs:
            raise C.SpecError(f"--run role {role!r} given twice")
        runs[role] = path
    return runs


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("spec", type=pathlib.Path)
    ap.add_argument("--run", action="append", default=[], metavar="ROLE=DIR")
    ap.add_argument("--out", type=pathlib.Path, required=True)
    args = ap.parse_args(argv)
    try:
        spec = C.load_spec(args.spec)
        result = C.evaluate(spec, parse_runs(args.run))
    except (C.SpecError, OSError, json.JSONDecodeError) as e:
        print(f"criterion: {e}", file=sys.stderr)
        return 2
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / "verdict.json").write_text(json.dumps(result, indent=2, sort_keys=False) + "\n")
    md = C.render_markdown(result)
    (args.out / "verdict.md").write_text(md)
    print(md.split("\n\n")[1])
    return 0 if result["verdict"]["holds"] else 1


if __name__ == "__main__":
    sys.exit(main())
