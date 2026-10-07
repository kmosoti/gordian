"""Measured ns per operation of the dataflow engine, beside the declared prices (work item C1).

Usage:
    python3 -I scripts/c1_prices.py [--csv OUT.csv] CRITERION_DIR:STDERR_FILE [...]

Each argument pairs a criterion output directory (`target/criterion`, or a copy of it from an earlier
run) with the stderr of that run of `src/stream/arms/dataflow/bench.rs`, which carries the per-batch
operation counts of every workload (`counts <id> <probes> <writes> <scans> <fires> ...`, read from
the engine's own counters). For each run it prints the table of medians against the declared prices,
the per-kind estimates by differencing, and, over all runs together, a least-squares fit of prices
(relative residuals, with and without a per-batch intercept); it checks every measurement against the
band 0.7 to 1.4 of its modelled cost (M1b's band) at the declared prices, which are read from
`engine.rs`. Standard library only. `--csv` writes one row per run, workload and size.
"""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
KINDS = ["probes", "writes", "scans", "fires"]
WORKLOADS = ["probe", "write", "write_logged", "scan", "project", "project_moving", "mixed"]
SIZES = [10, 100, 1000]
BAND = (0.7, 1.4)


def declared():
    text = (ROOT / "crates/gordian-run/src/stream/arms/dataflow/engine.rs").read_text()
    block = re.search(r"pub const DECLARED: Prices = Prices \{(.*?)\};", text, re.S).group(1)
    got = dict(re.findall(r"(\w+)_ns: (\d+)", block))
    return {"probes": float(got["probe"]), "writes": float(got["write"]),
            "scans": float(got["scan"]), "fires": float(got["fire"])}


def read_counts(err):
    out = {}
    for line in Path(err).read_text().splitlines():
        if line.startswith("counts "):
            p = line.split()
            out[p[1]] = dict(zip(KINDS, map(int, p[2:6])))
    return out


def median(crit, wid):
    est = json.loads((Path(crit) / wid / "new/estimates.json").read_text())["median"]
    return est["point_estimate"], est["confidence_interval"]["lower_bound"], est["confidence_interval"]["upper_bound"]


def modelled(c, prices):
    return sum(c[k] * prices[k] for k in KINDS)


def solve(a, b):
    n = len(a[0])
    m = [[sum(r[i] * r[j] for r in a) for j in range(n)] for i in range(n)]
    v = [sum(r[i] * y for r, y in zip(a, b)) for i in range(n)]
    for i in range(n):
        p = max(range(i, n), key=lambda r: abs(m[r][i]))
        m[i], m[p], v[i], v[p] = m[p], m[i], v[p], v[i]
        for r in range(n):
            if r != i:
                f = m[r][i] / m[i][i]
                m[r] = [x - f * y for x, y in zip(m[r], m[i])]
                v[r] -= f * v[i]
    return [v[i] / m[i][i] for i in range(n)]


def main(args):
    csv = None
    if args[:1] == ["--csv"]:
        csv, args = args[1], args[2:]
    prices = declared()
    print(f"Declared prices (ns): {prices}\n")
    rows, out = [], []
    for k, pair in enumerate(args):
        crit, err = pair.split(":", 1)
        counts = read_counts(err)
        run = k + 1
        print(f"## run {run}: {crit}\n")
        print("| workload | n | probes | writes | scans | fires | median ns/batch [95% CI] | modelled ns | measured / modelled | ns per op |")
        print("|---|---|---|---|---|---|---|---|---|---|")
        med = {}
        for wl in WORKLOADS:
            for n in SIZES:
                wid = f"{wl}/{n}"
                if wid not in counts:
                    continue
                c = counts[wid]
                y, lo, hi = median(crit, wid)
                med[wid] = y
                ops = sum(c[x] for x in KINDS)
                md = modelled(c, prices)
                rows.append((run, wid, c, y))
                out.append({"run": run, "workload": wl, "n": n, **c, "median_ns": y, "ci_lo": lo,
                            "ci_hi": hi, "modelled_ns": md, "ratio": y / md, "ns_per_op": y / ops})
                print(f"| {wl} | {n} | {c['probes']} | {c['writes']} | {c['scans']} | {c['fires']} "
                      f"| {y:,.0f} [{lo:,.0f}, {hi:,.0f}] | {md:,.0f} | {y / md:.2f} | {y / ops:.1f} |")
        print("\n| n | probe | write (unlogged) | write (logged) | scan row | fire: (project - write_logged) / fires | declared |")
        print("|---|---|---|---|---|---|---|")
        for n in SIZES:
            try:
                p = med[f"probe/{n}"] / counts[f"probe/{n}"]["probes"]
                w = med[f"write/{n}"] / counts[f"write/{n}"]["writes"]
                wl = med[f"write_logged/{n}"] / counts[f"write_logged/{n}"]["writes"]
                s = med[f"scan/{n}"] / counts[f"scan/{n}"]["scans"]
                f = (med[f"project/{n}"] - med[f"write_logged/{n}"]) / counts[f"project/{n}"]["fires"]
            except KeyError:
                continue
            print(f"| {n} | {p:.1f} | {w:.1f} | {wl:.1f} | {s:.1f} | {f:.1f} | "
                  f"{prices['probes']:.0f} / {prices['writes']:.0f} / {prices['scans']:.0f} / {prices['fires']:.0f} |")
        print()
    print("## least-squares prices, all runs (relative residuals)\n")
    for intercept in (True, False):
        a = [([1.0] if intercept else []) + [float(c[x]) for x in KINDS] for _, _, c, _ in rows]
        a = [[x / y for x in r] for r, (_, _, _, y) in zip(a, rows)]
        coef = solve(a, [1.0] * len(a))
        names = (["per batch"] if intercept else []) + KINDS
        worst = max(abs(sum(x * c for x, c in zip(r, coef)) - 1) for r in a)
        text = ", ".join(f"{n} {c:.1f}" for n, c in zip(names, coef))
        print(f"- {'with' if intercept else 'without'} intercept: {text} ns; worst relative residual {worst:.2f}")
    ratios = [(y / modelled(c, prices), run, wid) for run, wid, c, y in rows]
    lo, hi = min(ratios), max(ratios)
    outside = [r for r in ratios if not BAND[0] <= r[0] <= BAND[1]]
    print(f"\n## measured / modelled at the declared prices; band {BAND}\n")
    print(f"- {len(ratios)} measurements, {lo[0]:.2f} ({lo[2]}, run {lo[1]}) to {hi[0]:.2f} ({hi[2]}, run {hi[1]}); "
          f"outside the band: " + (", ".join(f"{w} run {r} {x:.2f}" for x, r, w in outside) if outside else "none"))
    head = [r for r in ratios if r[2].startswith("mixed/")]
    if head:
        print(f"- headline (mixed): {min(head)[0]:.2f} to {max(head)[0]:.2f}")
    if csv:
        keys = list(out[0])
        Path(csv).write_text(",".join(keys) + "\n" + "\n".join(",".join(str(r[k]) for k in keys) for r in out) + "\n")


if __name__ == "__main__":
    main(sys.argv[1:])
