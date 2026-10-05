"""R9 development check: does the reader cope with a context that is a time window, not a random
sample of the pool?

Usage: r9_dev_window.py SET_DIR [--reader MODULE]

On the development questions only (seeds 29000-29999), build the context a `window` builder would
send for a call made `DELAY_S` seconds after the first alarm: every observation of the window's last
`W` seconds up to the call (the dumper's pool and the decisive evidence are every observation within
40 s of the focus), the most recent `N` of them, the first alarm included. Report the reader's
accuracy at a few (W, N). This is a robustness check of the reader's density estimate on development
data; it is not R9's direct check on R6's real contexts, which waits for the freeze.

Exploration (nothing here tests a hypothesis).
"""

import argparse
import importlib
from pathlib import Path

import r9_common as C

DELAY_S = 16
CONFIGS = [(20, 128), (20, 256), (40, 256), (80, 512)]


def window_context(q, w_s, n):
    t_call = q["focus"]["at_ns"] + DELAY_S * 10**9
    recs = [q["focus"]] + list(q["decisive"]) + list(q["pool"])
    keep = [r for r in recs if t_call - w_s * 10**9 <= r["at_ns"] <= t_call]
    keep.sort(key=lambda r: (r["at_ns"], r["id"]))
    return keep[-n:]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("set_dir")
    ap.add_argument("--reader", default="r9_reader")
    a = ap.parse_args()
    reader = importlib.import_module(a.reader)
    qs = list(C.iter_records(Path(a.set_dir) / "hard.jsonl"))
    for q in qs:
        assert C.DEV_SEEDS[0] <= q["seed"] <= C.DEV_SEEDS[1], "development seeds only"
    print(f"{len(qs)} hard development questions, call at +{DELAY_S} s")
    for w, n in CONFIGS:
        ok = 0
        refs = 0
        by = {}
        for q in qs:
            ctx = window_context(q, w, n)
            refs += len(ctx)
            kind, site, _ = reader.read(q["services"], q["focus"], ctx)
            good = C.correct((kind, site), q)
            ok += good
            by.setdefault(q["family"], []).append(good)
        fam = " ".join(f"{f}={sum(v) / len(v):.3f}" for f, v in sorted(by.items()))
        print(f"window W={w:2d} s N={n:3d}: accuracy {ok / len(qs):.3f}, refs/call {refs / len(qs):.0f}   {fam}")


if __name__ == "__main__":
    main()
