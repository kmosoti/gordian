"""A null model of one pair's three pair cells, for A2's design checks and the PI's prediction.

Usage: a2_null_model.py   (prints a table; reads and writes nothing)

Written and run before any A2 run, on no data: services are independent (no edge at all), a
partner's counted first alarms begin at `h` per second of its quiet time, a pair gets `n` trials per
600 s stream (the predicting service's counted first alarms while the partner is quiet), and each
trial is followed in band `w` with probability `1 - exp(-h w)`. The layer's rule is applied as
written (decay `tau` = 150 s between trials, the threshold 2 read before each trial, the partner's
rate estimated with the rung's prior, 3 over 30 s, from roughly `0.6 t` of quiet time), with the
two weightings: `odds` (a follow 1, a miss `exp(r w) - 1`: the design before departure 83) and
`excess` (a follow `1 - q`, a miss `q`: as built). Evidence is entered at the trial, not at its
resolution (a simplification). 33 ordered pairs per stream, 20 streams, seed 1.

What it gives is what the rule does with no structure in the world: the edges a learner holds by
chance, which the smoke's counts are to be read against. It is not the world: the world's alarms
cluster (incidents, bursts) and have structure (hidden edges), both of which raise the counts.
"""

import math
import random

W = [0.4, 2.0, 10.0]
TAU = 150.0
THETA = 2.0


def run(h, trials, weighting, T=600.0, pairs=33, streams=20, seed=1):
    rnd = random.Random(seed)
    preds = [0, 0, 0]
    followed = [0, 0, 0]
    ever_any = 0
    ever = [0, 0, 0]
    held_reads = [0, 0, 0]
    reads = 0
    for _ in range(streams):
        for _ in range(pairs):
            lev = [0.0, 0.0, 0.0]
            last = 0.0
            t = 0.0
            eh = [False, False, False]
            while True:
                t += rnd.expovariate(trials / T)
                if t >= T:
                    break
                d = math.exp(-(t - last) / TAU)
                lev = [x * d for x in lev]
                last = t
                reads += 1
                held = [x >= THETA for x in lev]
                for k in range(3):
                    if held[k]:
                        held_reads[k] += 1
                        eh[k] = True
                gap = rnd.expovariate(h)
                if any(held):
                    k = held.index(True)
                    preds[k] += 1
                    followed[k] += gap <= W[k]
                quiet = 0.6 * t
                lam = (h * quiet + 3.0) / (quiet + 30.0)
                for k in range(3):
                    q = 1.0 - math.exp(-lam * W[k])
                    if weighting == "odds":
                        lev[k] += 1.0 if gap <= W[k] else -(math.exp(lam * W[k]) - 1.0)
                    else:
                        lev[k] += (1.0 - q) if gap <= W[k] else -q
            ever_any += any(eh)
            for k in range(3):
                ever[k] += eh[k]
    return {
        "edges_per_stream": round(ever_any / streams, 2),
        "edges_by_band": [round(e / streams, 2) for e in ever],
        "held_share_of_reads": [round(x / reads, 3) for x in held_reads],
        "predictions_per_stream": round(sum(preds) / streams, 1),
        "predictions_by_band": [round(p / streams, 1) for p in preds],
        "follow_rate_by_band": [round(f / p, 3) if p else None for f, p in zip(followed, preds)],
    }


def main():
    for weighting in ("odds", "excess"):
        for h, n in ((0.1, 20), (0.1, 40), (0.2, 40), (0.2, 60), (0.3, 60)):
            print(weighting, f"h={h}", f"trials={n}", run(h, n, weighting))


if __name__ == "__main__":
    main()
