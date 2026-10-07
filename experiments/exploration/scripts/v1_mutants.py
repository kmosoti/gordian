"""V1's hand mutants of the measures: six single-line edits of analysis/gordian_analysis/criterion.py,
each evaluated on the four specifications and the kept runs, and compared with the unmutated
evaluation. A mutant is caught when a verdict, a clause outcome or a number changes.

Usage: v1_mutants.py --scratch DIR   writes experiments/exploration/v1-mutants.csv

The mutated copies live under DIR only; the module in the worktree is never edited.
"""
import argparse
import csv
import json
import pathlib
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
PY = "/home/user/gordian/analysis/.venv/bin/python"
sys.path.insert(0, str(pathlib.Path(__file__).parent))
import v1_backtest as BT  # noqa: E402  (the run directories)

MUTANTS = [
    ("M1", "wrong tier/family filter",
     "an `eq` condition of a where-clause selects the rows that differ (the hard non-leak share counts the plain incidents and the leaks)",
     "            hit = col == v\n", "            hit = col != v\n"),
    ("M2", "off-by-one in a window",
     "`last k` starts one stream late (k - 1 streams)",
     "        a, b = n - v, n\n", "        a, b = n - v + 1, n\n"),
    ("M3", "a dropped cluster",
     "the last stream is never drawn into any resample",
     "        parts.append(rng.multinomial(n, np.full(n, 1.0 / n), size=k).astype(float))\n",
     "        parts.append(rng.multinomial(n, np.full(n, 1.0 / n), size=k).astype(float))\n"
     "        parts[-1][:, -1] = 0.0\n"),
    ("M4", "a wrong pairing",
     "the subtrahend arm of a paired difference is evaluated on shifted resamples (independent draws)",
     "        d = ratio_draws(na[a:b], da[a:b], sca, w) - ratio_draws(nb[a:b], db[a:b], scb, w)\n",
     "        d = ratio_draws(na[a:b], da[a:b], sca, w) - ratio_draws(nb[a:b], db[a:b], scb, np.roll(w, 1, axis=0))\n"),
    ("M5", "a wrong percentile",
     "the interval is the 10th and 90th percentiles instead of the specification's 5th and 95th",
     "    lo, hi = percentiles[0] / 100.0, percentiles[1] / 100.0\n",
     "    lo, hi = (percentiles[0] + 5) / 100.0, (percentiles[1] - 5) / 100.0\n"),
    ("M6", "an unseeded bootstrap",
     "the generator is created without the specification's seed",
     "    rng = np.random.default_rng(seed)\n", "    rng = np.random.default_rng()\n"),
]

RUNNER = r'''
import json, sys
sys.path.insert(0, sys.argv[1])
from gordian_analysis import criterion as C
unit, out = sys.argv[2], sys.argv[3]
dirs = json.loads(sys.argv[4])
spec = C.load_spec(sys.argv[5])
res = C.evaluate(spec, dirs)
json.dump(res, open(out, "w"))
'''


def evaluate_all(pkg_parent: pathlib.Path, out: pathlib.Path, tag: str) -> dict:
    out.mkdir(parents=True, exist_ok=True)
    res = {}
    for unit, dirs in BT.UNITS.items():
        f = out / f"{tag}-{unit}.json"
        p = subprocess.run([PY, "-c", RUNNER, str(pkg_parent), unit, str(f),
                            json.dumps({k: str(v) for k, v in dirs.items()}),
                            str(ROOT / f"experiments/criteria/{unit}.json")], capture_output=True, text=True)
        if p.returncode:
            sys.exit(f"{tag} {unit}: {p.stderr[-800:]}")
        res[unit] = json.loads(f.read_text())
    return res


def numbers(res: dict) -> dict:
    d = {}
    for c in res["clauses"]:
        for k in ("point", "lower", "upper"):
            if k in c:
                d[("clause", c["id"], k)] = c[k]
        d[("clause", c["id"], "pass")] = c["pass"]
    for r in res["reports"]:
        for c in r["cells"]:
            for k in ("point", "lower", "upper"):
                d[(r["id"], c["row"], c["measure"], k)] = c[k]
    d[("verdict",)] = res["verdict"]["holds"]
    for k, v in res["verdict"]["nodes"].items():
        d[("node", k)] = v
    return d


def diff(a: dict, b: dict) -> dict:
    changed_num, max_abs, flips = 0, 0.0, []
    for k, x in a.items():
        y = b.get(k)
        if isinstance(x, bool) or isinstance(y, bool):
            if x != y:
                flips.append("/".join(map(str, k)))
            continue
        if x is None or y is None:
            if x != y:
                changed_num += 1
            continue
        if abs(x - y) > 1e-12:
            changed_num += 1
            max_abs = max(max_abs, abs(x - y))
    return {"numbers_changed": changed_num, "of": len(a), "max_abs_change": max_abs, "flips": flips}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--scratch", type=pathlib.Path, required=True)
    args = ap.parse_args()
    src = ROOT / "analysis" / "gordian_analysis" / "criterion.py"
    text = src.read_text()
    base = evaluate_all(ROOT / "analysis", args.scratch / "out", "base")
    rows = []
    for mid, name, what, old, new in MUTANTS:
        assert text.count(old) == 1, (mid, text.count(old))
        pkg = args.scratch / mid
        if pkg.exists():
            shutil.rmtree(pkg)
        shutil.copytree(ROOT / "analysis" / "gordian_analysis", pkg / "gordian_analysis")
        (pkg / "gordian_analysis" / "criterion.py").write_text(text.replace(old, new))
        res = evaluate_all(pkg, args.scratch / "out", mid)
        row = {"mutant": mid, "name": name, "edit": what}
        tot_changed = 0
        flips_all = []
        for unit in BT.UNITS:
            d = diff(numbers(base[unit]), numbers(res[unit]))
            row[f"{unit}_numbers_changed"] = f"{d['numbers_changed']}/{d['of']}"
            row[f"{unit}_max_abs_change"] = f"{d['max_abs_change']:.4g}"
            tot_changed += d["numbers_changed"]
            flips_all += [f"{unit}:{f}" for f in d["flips"]]
        row["verdict_flips"] = "; ".join(f for f in flips_all if f.endswith(":verdict") or ":verdict" in f) or ""
        row["clause_or_node_flips"] = len(flips_all)
        row["caught"] = bool(tot_changed or flips_all)
        if mid == "M6":
            res2 = evaluate_all(pkg, args.scratch / "out", mid + "b")
            again = sum(diff(numbers(res[u]), numbers(res2[u]))["numbers_changed"] for u in BT.UNITS)
            row["note"] = f"two runs of the mutant differ in {again} numbers; the unmutated script's two runs are identical"
        rows.append(row)
        print(row, flush=True)
    fields = list(rows[0])
    with open(ROOT / "experiments/exploration/v1-mutants.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=fields, extrasaction="ignore")
        w.writeheader()
        for r in rows:
            r.setdefault("note", "")
            w.writerow(r)


if __name__ == "__main__":
    main()
