"""Measured ns per operation of the medium's tick, beside the declared prices (work item M1).

Usage:
    python3 -I crates/gordian-medium/bench_prices.py CRITERION_DIR:STDERR_FILE [...]

Each argument pairs a criterion output directory (`target/criterion`, or a copy of it from an
earlier run) with the stderr of that run of `benches/tick.rs`, which carries the per-tick operation
counts of every workload. For each run it prints the table of medians against the declared prices
and the per-kind estimates by differencing; over all runs together it prints a least-squares fit
of prices (relative residuals, with and without a per-tick intercept) and checks the proposed
prices against every measurement. Standard library only.
"""
import json
import sys
from pathlib import Path

DECLARED = {"update": 20.0, "traversal": 5.0, "routing": 10.0, "field": 2.0}
PROPOSED = {"update": 200.0, "traversal": 25.0, "routing": 40.0, "field": 2.0}
KINDS = ["update", "traversal", "routing", "field"]
WORKLOADS = ["mixed", "unmatched", "sense", "fanout", "fieldgate"]
SIZES = [10, 100, 1000]


def read_counts(err):
    out = {}
    for line in Path(err).read_text().splitlines():
        if line.startswith("counts "):
            p = line.split()
            out[p[1]] = dict(zip(KINDS, map(int, p[2:6])), active=int(p[7]))
    return out


def median(crit, wid):
    est = json.loads((Path(crit) / wid / "new/estimates.json").read_text())
    m = est["median"]
    return m["point_estimate"], m["confidence_interval"]["lower_bound"], m["confidence_interval"]["upper_bound"]


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


def main(pairs):
    rows = []
    for k, pair in enumerate(pairs):
        crit, err = pair.split(":", 1)
        counts = read_counts(err)
        print(f"## run {k + 1}: {crit}\n")
        print("| workload | active | upd | trav | rout | field | median ns/tick [95% CI] | modelled ns, declared | measured / modelled | ns per op |")
        print("|---|---|---|---|---|---|---|---|---|---|")
        med = {}
        for wl in WORKLOADS:
            for n in SIZES:
                wid = f"{wl}/{n}"
                c = counts[wid]
                y, lo, hi = median(crit, wid)
                med[wid] = y
                rows.append((k + 1, wid, c, y))
                ops = sum(c[x] for x in KINDS)
                md = modelled(c, DECLARED)
                print(f"| {wl} | {c['active']} | {c['update']} | {c['traversal']} | {c['routing']} | {c['field']} "
                      f"| {y:,.0f} [{lo:,.0f}, {hi:,.0f}] | {md:,.0f} | {y / md:.2f} | {y / ops:.1f} |")
        print("\n| n | routing: unmatched / n | update: (sense - unmatched) / n | traversal: (fanout - sense) / 4n | field read: (fieldgate - fanout) / 4n |")
        print("|---|---|---|---|---|")
        for n in SIZES:
            r = med[f"unmatched/{n}"] / n
            u = (med[f"sense/{n}"] - med[f"unmatched/{n}"]) / n
            t = (med[f"fanout/{n}"] - med[f"sense/{n}"]) / (4 * n)
            f = (med[f"fieldgate/{n}"] - med[f"fanout/{n}"]) / (4 * n)
            print(f"| {n} | {r:.1f} | {u:.1f} | {t:.1f} | {f:.1f} |")
        print("| declared | 10 | 20 | 5 | 2 |\n")

    print("## least-squares prices over all runs (relative residuals)\n")
    for intercept in (True, False):
        a = [([1.0] if intercept else []) + [float(c[x]) for x in KINDS] for _, _, c, _ in rows]
        a = [[x / y for x in r] for r, (_, _, _, y) in zip(a, rows)]
        coef = solve(a, [1.0] * len(a))
        names = (["per tick"] if intercept else []) + KINDS
        worst = max(abs(sum(x * c for x, c in zip(r, coef)) - 1) for r in a)
        fit = ", ".join(f"{n} {c:.1f}" for n, c in zip(names, coef))
        print(f"- {'with' if intercept else 'without'} intercept: {fit} ns; worst relative residual {worst:.2f}")
    ratios = [(y / modelled(c, PROPOSED), run, wid) for run, wid, c, y in rows]
    head = [r for r in ratios if r[2].startswith("mixed/")]
    print(f"\n## proposed prices {PROPOSED} (ns)\n")
    print(f"- measured / modelled over all {len(ratios)} measurements: "
          f"{min(ratios)[0]:.2f} to {max(ratios)[0]:.2f}")
    print(f"- on the headline workload (mixed): {min(head)[0]:.2f} to {max(head)[0]:.2f}")


if __name__ == "__main__":
    main(sys.argv[1:])
