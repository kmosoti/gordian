"""Measured ns per operation of the medium's tick, beside the declared prices (work items M1, M1b).

Usage:
    python3 -I crates/gordian-medium/bench_prices.py CRITERION_DIR:STDERR_FILE [...]

Each argument pairs a criterion output directory (`target/criterion`, or a copy of it from an
earlier run) with the stderr of that run of `benches/tick.rs`, which carries the per-tick operation
counts of every workload. For each run it prints the table of medians against the declared prices
and the per-kind estimates by differencing; over all runs together it prints a least-squares fit
of prices (relative residuals, with and without a per-tick intercept) and checks every measurement
against the M1b acceptance band (0.7 to 1.4 of its modelled cost at the declared prices).

M1's five workloads are the acceptance; M1b's priced workloads (oscillome on) are reported beside
them; the two idle workloads have no counted operation and are reported in ns per tick, with the
difference between them (the oscillome engine's per-tick work, which the prices do not model).
Standard library only. Runs whose stderr lacks a workload (M1's runs) are read for what they have.
"""
import json
import sys
from pathlib import Path

DECLARED = {"update": 200.0, "traversal": 25.0, "routing": 40.0, "field": 2.0}
FIRST_GUESS = {"update": 20.0, "traversal": 5.0, "routing": 10.0, "field": 2.0}
KINDS = ["update", "traversal", "routing", "field"]
M1_WORKLOADS = ["mixed", "unmatched", "sense", "fanout", "fieldgate"]
OSC_WORKLOADS = ["mixed_osc", "phasegate", "oscillator"]
IDLE = ["idle", "idle_osc"]
SIZES = [10, 100, 1000]
BAND = (0.7, 1.4)


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


def table(crit, counts, workloads, rows, run):
    print("| workload | active | upd | trav | rout | field | median ns/tick [95% CI] | modelled ns | measured / modelled | ns per op |")
    print("|---|---|---|---|---|---|---|---|---|---|")
    med = {}
    for wl in workloads:
        for n in SIZES:
            wid = f"{wl}/{n}"
            if wid not in counts:
                continue
            c = counts[wid]
            y, lo, hi = median(crit, wid)
            med[wid] = y
            rows.append((run, wid, c, y))
            ops = sum(c[x] for x in KINDS)
            md = modelled(c, DECLARED)
            print(f"| {wl} | {c['active']} | {c['update']} | {c['traversal']} | {c['routing']} | {c['field']} "
                  f"| {y:,.0f} [{lo:,.0f}, {hi:,.0f}] | {md:,.0f} | {y / md:.2f} | {y / ops:.1f} |")
    return med


def fit(rows, label):
    print(f"\n## least-squares prices, {label} (relative residuals)\n")
    for intercept in (True, False):
        a = [([1.0] if intercept else []) + [float(c[x]) for x in KINDS] for _, _, c, _ in rows]
        a = [[x / y for x in r] for r, (_, _, _, y) in zip(a, rows)]
        coef = solve(a, [1.0] * len(a))
        names = (["per tick"] if intercept else []) + KINDS
        worst = max(abs(sum(x * c for x, c in zip(r, coef)) - 1) for r in a)
        text = ", ".join(f"{n} {c:.1f}" for n, c in zip(names, coef))
        print(f"- {'with' if intercept else 'without'} intercept: {text} ns; worst relative residual {worst:.2f}")


def band(rows, label):
    ratios = [(y / modelled(c, DECLARED), run, wid) for run, wid, c, y in rows]
    if not ratios:
        return
    lo, hi = min(ratios), max(ratios)
    outside = [r for r in ratios if not BAND[0] <= r[0] <= BAND[1]]
    print(f"- {label}: {len(ratios)} measurements, measured / modelled {lo[0]:.2f} ({lo[2]}, run {lo[1]}) "
          f"to {hi[0]:.2f} ({hi[2]}, run {hi[1]}); outside [{BAND[0]}, {BAND[1]}]: "
          + (", ".join(f"{w} run {r} {x:.2f}" for x, r, w in outside) if outside else "none"))


def main(pairs):
    m1_rows, osc_rows = [], []
    for k, pair in enumerate(pairs):
        crit, err = pair.split(":", 1)
        counts = read_counts(err)
        run = k + 1
        print(f"## run {run}: {crit}\n")
        print(f"Declared prices (ns): {DECLARED}\n")
        print("### M1's workloads (oscillome off)\n")
        med = table(crit, counts, M1_WORKLOADS, m1_rows, run)
        print("\n| n | routing: unmatched / n | update: (sense - unmatched) / n | traversal: (fanout - sense) / 4n | field read: (fieldgate - fanout) / 4n |")
        print("|---|---|---|---|---|")
        for n in SIZES:
            r = med[f"unmatched/{n}"] / n
            u = (med[f"sense/{n}"] - med[f"unmatched/{n}"]) / n
            t = (med[f"fanout/{n}"] - med[f"sense/{n}"]) / (4 * n)
            f = (med[f"fieldgate/{n}"] - med[f"fanout/{n}"]) / (4 * n)
            print(f"| {n} | {r:.1f} | {u:.1f} | {t:.1f} | {f:.1f} |")
        print("| declared | 40 | 200 | 25 | 2 |\n")
        if not any(f"{w}/10" in counts for w in OSC_WORKLOADS):
            continue
        print("### M1b's workloads (oscillome on: 100 ms tick, rhythms 10 s and 100 s, cycle summaries)\n")
        omed = table(crit, counts, OSC_WORKLOADS, osc_rows, run)
        print("\n| n | mixed_osc - mixed, ns per tick | phase read: (phasegate - fanout) / 4n | oscillator run: (oscillator / n) - traversal 25 | idle ns/tick | idle_osc ns/tick | engine per tick: idle_osc - idle |")
        print("|---|---|---|---|---|---|---|")
        for n in SIZES:
            d = omed[f"mixed_osc/{n}"] - med[f"mixed/{n}"]
            ph = (omed[f"phasegate/{n}"] - med[f"fanout/{n}"]) / (4 * n)
            osc = omed[f"oscillator/{n}"] / n - DECLARED["traversal"]
            i0, _, _ = median(crit, f"idle/{n}")
            i1, _, _ = median(crit, f"idle_osc/{n}")
            print(f"| {n} | {d:,.0f} | {ph:.1f} | {osc:.1f} | {i0:,.0f} | {i1:,.0f} | {i1 - i0:,.0f} |")
        print()

    fit(m1_rows, "M1's workloads")
    if osc_rows:
        fit(m1_rows + osc_rows, "M1's and M1b's priced workloads")
    print(f"\n## measured / modelled at the declared prices; acceptance band {BAND}\n")
    band(m1_rows, "M1's workloads (the acceptance)")
    band([r for r in m1_rows if r[1].startswith("mixed/")], "of which the headline (mixed)")
    band(osc_rows, "M1b's priced workloads")


if __name__ == "__main__":
    main(sys.argv[1:])
