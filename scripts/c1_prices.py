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
WORKLOADS = ["probe", "write", "write_logged", "replace_logged", "scan", "dispatch", "project", "project_moving", "mixed"]
SIZES = [10, 100, 1000]
PROGRAM = ["background", "bursts", "ramps", "splits", "mixed"]
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


def suggest(rows):
    """One scale factor on the primitives' weights, chosen so that the program workloads sit as
    close to 1 as one factor can put them (minimising the largest |log ratio|)."""
    import math
    per = {}
    for _, wid, c, y in rows:
        wl, n = wid.split("/")
        if wl in ("probe",):
            per.setdefault("probes", []).append(y / c["probes"])
        if wl in ("write", "write_logged"):
            per.setdefault("writes", []).append(y / c["writes"])
        if wl == "scan":
            per.setdefault("scans", []).append(y / c["scans"])
        if wl == "dispatch":
            per.setdefault("fires", []).append(y / c["fires"])
    base = {k: sum(v) / len(v) for k, v in per.items()}
    prog = [(c, y) for _, wid, c, y in rows if wid.startswith("program/")]
    if len(base) < 4 or not prog:
        return
    best = None
    for k in range(1, 4001):
        lam = k / 400
        worst = max(abs(math.log(y / modelled(c, {a: lam * b for a, b in base.items()}))) for c, y in prog)
        if best is None or worst < best[0]:
            best = (worst, lam)
    worst, lam = best
    print(f"\n## suggested prices: primitives' means {({k: round(v, 1) for k, v in base.items()})} ns "
          f"scaled by {lam:.2f} (largest |log ratio| over program/* {worst:.2f}): "
          f"{({k: round(lam * v, 1) for k, v in base.items()})}")


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
        for wl, n in [(w, n) for w in WORKLOADS for n in SIZES] + [("program", p) for p in PROGRAM] + [("noticer", p) for p in PROGRAM]:
            if True:
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
        print("\n| n | probe | write (unlogged) | write (logged) | scan row | replace (logged) | fire: (project - replace_logged) / fires | fire: dispatch / fires | declared |")
        print("|---|---|---|---|---|---|---|---|---|")
        for n in SIZES:
            try:
                p = med[f"probe/{n}"] / counts[f"probe/{n}"]["probes"]
                w = med[f"write/{n}"] / counts[f"write/{n}"]["writes"]
                wl = med[f"write_logged/{n}"] / counts[f"write_logged/{n}"]["writes"]
                s = med[f"scan/{n}"] / counts[f"scan/{n}"]["scans"]
                f = (med[f"project/{n}"] - med[f"replace_logged/{n}"]) / counts[f"project/{n}"]["fires"]
            except KeyError:
                continue
            rp = med[f"replace_logged/{n}"] / counts[f"replace_logged/{n}"]["writes"]
            dp = med[f"dispatch/{n}"] / counts[f"dispatch/{n}"]["fires"]
            print(f"| {n} | {p:.1f} | {w:.1f} | {wl:.1f} | {s:.1f} | {rp:.1f} | {f:.1f} | {dp:.1f} | "
                  f"{prices['probes']:.0f} / {prices['writes']:.0f} / {prices['scans']:.0f} / {prices['fires']:.0f} |")
        print()
    groups = [("the table primitives (probe, write, write_logged, scan, project, project_moving, mixed)",
               [r for r in rows if not r[1].startswith(("program/", "noticer/"))]),
              ("the program over generated scenarios (program/*)", [r for r in rows if r[1].startswith("program/")]),
              ("the noticer through the seam adapter (noticer/*)", [r for r in rows if r[1].startswith("noticer/")]),
              ("all of them", rows)]
    for label, sel in groups:
        if len(sel) < 4:
            continue
        print(f"## least-squares prices, {label} (relative residuals)\n")
        for intercept in (True, False):
            a = [([1.0] if intercept else []) + [float(c[x]) for x in KINDS] for _, _, c, _ in sel]
            a = [[x / y for x in r] for r, (_, _, _, y) in zip(a, sel)]
            try:
                coef = solve(a, [1.0] * len(a))
            except ZeroDivisionError:
                print(f"- {'with' if intercept else 'without'} intercept: singular")
                continue
            names = (["per batch"] if intercept else []) + KINDS
            worst = max(abs(sum(x * c for x, c in zip(r, coef)) - 1) for r in a)
            text = ", ".join(f"{n} {c:.1f}" for n, c in zip(names, coef))
            print(f"- {'with' if intercept else 'without'} intercept: {text} ns; worst relative residual {worst:.2f}")
        print()
    ratios = [(y / modelled(c, prices), run, wid) for run, wid, c, y in rows]
    print(f"## measured / modelled at the declared prices; band {BAND}\n")
    for label, keep in (("table primitives", lambda w: not w.startswith(("program/", "noticer/"))),
                        ("program over generated scenarios", lambda w: w.startswith("program/")),
                        ("noticer through the seam adapter", lambda w: w.startswith("noticer/"))):
        sel = [r for r in ratios if keep(r[2])]
        if not sel:
            continue
        lo, hi = min(sel), max(sel)
        outside = [r for r in sel if not BAND[0] <= r[0] <= BAND[1]]
        print(f"- {label}: {len(sel)} measurements, {lo[0]:.2f} ({lo[2]}, run {lo[1]}) to {hi[0]:.2f} "
              f"({hi[2]}, run {hi[1]}); outside the band: "
              + (", ".join(f"{w} run {r} {x:.2f}" for x, r, w in outside) if outside else "none"))
    suggest(rows)
    if csv:
        keys = list(out[0])
        Path(csv).write_text(",".join(keys) + "\n" + "\n".join(",".join(str(r[k]) for k in keys) for r in out) + "\n")


if __name__ == "__main__":
    main(sys.argv[1:])
