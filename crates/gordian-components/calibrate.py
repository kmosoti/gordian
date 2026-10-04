#!/usr/bin/env python3
"""Fit the declared-cost constants to the criterion medians.

Input is `target/cost-calibration.json`, written by the (ignored) calibration test after
`cargo bench -p gordian-components`. Each row is one benchmark id: the criterion median for one
pass over a pool of windows, and the features of that pool summed over its windows:

    windows   number of windows in the pool
    sum_n     total observations in the windows
    sum_s     total services
    sum_ns    total of (observations x services), per window
    sum_r     total prior records

For each component this fits `median = sum over windows of (a + b*n + c*s [+ d*n*s] [+ e*r])`
by least squares on *relative* error (each row weighted by 1/median), because the acceptance
criterion is relative. Constants are printed as the code stores them: `a` in nanoseconds, slopes
in picoseconds per unit. The fit is a procedure, not an authority: read the residuals, and run
the calibration test on the rounded constants.

Rows whose group ends in `_h` are held out of the fit and only scored.

Usage: python3 calibrate.py [path/to/cost-calibration.json]
"""
import json
import sys

# Feature columns used by each component's declared cost.
MODELS = {
    "heuristic": ["windows", "sum_n"],
    "estimator": ["windows", "sum_n", "sum_s"],
    "memory": ["windows", "sum_n", "sum_r"],
    "verifier": ["windows", "sum_n", "sum_s"],
}
EXTRA = {
    # Alternative shapes to compare when the first one leaves residuals.
    "verifier": [["windows", "sum_n", "sum_s", "sum_ns"]],
    "memory": [["windows", "sum_n"]],
}
UNIT = {"windows": 1.0, "sum_n": 1000.0, "sum_s": 1000.0, "sum_ns": 1000.0, "sum_r": 1000.0}


def solve(a, b):
    """Solve the small dense system a x = b by Gaussian elimination with pivoting."""
    n = len(b)
    m = [row[:] + [b[i]] for i, row in enumerate(a)]
    for i in range(n):
        p = max(range(i, n), key=lambda r: abs(m[r][i]))
        m[i], m[p] = m[p], m[i]
        for r in range(i + 1, n):
            f = m[r][i] / m[i][i]
            for c in range(i, n + 1):
                m[r][c] -= f * m[i][c]
    x = [0.0] * n
    for i in reversed(range(n)):
        x[i] = (m[i][n] - sum(m[i][c] * x[c] for c in range(i + 1, n))) / m[i][i]
    return x


def fit(rows, cols):
    """Weighted least squares on relative error: minimize sum ((x.c - y) / y)^2."""
    xs = [[r[c] / r["median_ns"] for c in cols] for r in rows]
    ys = [1.0 for _ in rows]
    k = len(cols)
    ata = [[sum(x[i] * x[j] for x in xs) for j in range(k)] for i in range(k)]
    atb = [sum(x[i] * y for x, y in zip(xs, ys)) for i in range(k)]
    coef = solve(ata, atb)
    ratio = [sum(c * r[col] for c, col in zip(coef, cols)) / r["median_ns"] for r in rows]
    return coef, ratio


def report(name, rows, cols):
    coef, ratio = fit(rows, cols)
    consts = ", ".join(
        f"{c}={v:.1f} ns" if c == "windows" else f"{c}={v * UNIT[c]:.0f} ps"
        for c, v in zip(cols, coef)
    )
    print(f"{name:10s} [{' + '.join(cols)}]  {consts}")
    print(f"           predicted/median: min {min(ratio):.3f}  max {max(ratio):.3f}")
    return coef, ratio


def main():
    path = sys.argv[1] if len(sys.argv) > 1 else "target/cost-calibration.json"
    rows = json.load(open(path))
    for name, cols in MODELS.items():
        # Rows in groups ending `_h` are held out: the fit never sees them, they are only scored.
        mine = [r for r in rows if r["component"] == name and not r["group"].endswith("_h")]
        held = [r for r in rows if r["component"] == name and r["group"].endswith("_h")]
        coef, _ = report(name, mine, cols)
        for alt in EXTRA.get(name, []):
            report(name + " (alt)", mine, alt)
        _, ratio = fit(mine, cols)
        for r, q in zip(mine, ratio):
            flag = "  <-- outside 0.8..1.2" if abs(q - 1) > 0.2 else ""
            print(f"    {r['group']}/{r['id']:<5} {q:.3f}{flag}")
        for r in held:
            q = sum(c * r[col] for c, col in zip(coef, cols)) / r["median_ns"]
            flag = "  <-- outside 0.8..1.2" if abs(q - 1) > 0.2 else ""
            print(f"    {r['group']}/{r['id']:<5} {q:.3f}  (held out){flag}")


if __name__ == "__main__":
    main()
