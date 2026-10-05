"""R8: the half-width of delta-hat's interval implied by N, before the main run.

Usage: r8_halfwidth.py --n N --a0 A0 --p0 P0 [--deltas 0,0.03,...] [--sims 100] [--b 1000]

The plan asks, before the main run, for the half-width of delta-hat's 90% interval implied by the
pilot's accuracy and the chosen N, and for it to be said in writing if that half-width exceeds 0.05
(neither regime could then be claimed even at a favourable point estimate).

Method (a simulation, not a result): for each assumed delta, A(m) = p0 + (A0 - p0) exp(-delta m / 100)
at the five levels and p0 at the control. N incidents are drawn with an independent Bernoulli at each
level, which ignores the pairing of levels within an incident; pairing makes the levels positively
correlated and so can only narrow the interval for a difference, so independence is conservative.
Each draw is analysed by `r8_stats.analyse` (same fit, same cluster bootstrap with fewer resamples
and a coarser grid for speed) and the interval's half-width, (upper - lower) / 2, is summarised over
the draws: median and the 10th to 90th percentile.

Exploration (nothing here tests a hypothesis).
"""

import argparse
import math

import numpy as np

import r8_stats as S


def simulate(n, a0, p0, delta, sims, b, grid, seed):
    rng = np.random.default_rng(seed)
    probs = np.array(
        [p0 + (a0 - p0) * math.exp(-delta * m / 100.0) for m in S.LEVELS] + [p0]
    )
    widths, los, his, outs = [], [], [], []
    for k in range(sims):
        mat = (rng.random((n, 6)) < probs).astype(float)
        res = S.analyse(mat, b=b, seed=S.SEED + k, grid=grid)
        lo, hi = res["delta_interval"]
        if math.isnan(lo):
            continue
        widths.append((hi - lo) / 2)
        los.append(lo)
        his.append(hi)
        outs.append(res["outcome"])
    return widths, los, his, outs


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, required=True)
    ap.add_argument("--a0", type=float, required=True)
    ap.add_argument("--p0", type=float, required=True)
    ap.add_argument("--deltas", default="0,0.03,0.05,0.075,0.1,0.15,0.2,0.4")
    ap.add_argument("--sims", type=int, default=100)
    ap.add_argument("--b", type=int, default=1000)
    args = ap.parse_args()
    grid = np.arange(-0.5, 3.0 + 1e-9, 0.005)
    print(f"N = {args.n}, A(0) = {args.a0}, p0 = {args.p0}, {args.sims} draws, {args.b} resamples each")
    print("delta  half-width median [p10, p90]   share of draws: R6 / R7 / unresolved")
    for d in [float(x) for x in args.deltas.split(",")]:
        w, lo, hi, outs = simulate(args.n, args.a0, args.p0, d, args.sims, args.b, grid, 7)
        r6 = sum(o == "R6 regime" for o in outs) / len(outs)
        r7 = sum(o == "R7 regime" for o in outs) / len(outs)
        print(
            f"{d:5.3f}  {np.median(w):.3f} [{np.quantile(w, 0.1):.3f}, {np.quantile(w, 0.9):.3f}]"
            f"          {r6:.2f} / {r7:.2f} / {1 - r6 - r7:.2f}"
        )


if __name__ == "__main__":
    main()
