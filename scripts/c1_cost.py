"""C1's cost columns: each noticer's own cost per stream, modelled and measured.

Usage: c1_cost.py REPLAY_CSV [RUN_COST_CSV]

REPLAY_CSV is the output of the ignored test `replay_costs` of `tests/stream_dataflow.rs` on the
held-out streams: each noticer replayed on the public observations as the rung's steps deliver them,
timed outside the noticer (the least of three replays, less the driver loop's own, a noticer that does
nothing). RUN_COST_CSV (default `experiments/exploration/c1-cost-run.csv`, from `c1_analyze.py`) is the
run's own record: the bill's compute per arm per stream and the arm's measured bookkeeping time.

Writes experiments/exploration/c1-cost.csv (one row per noticer and measure: mean per stream, 90%
percentile cluster bootstrap over streams, B2's resamples) and c1-costs-per-stream.csv (the replay
file). Modelled cost: the dataflow noticer's counted operations at `Prices::DECLARED` (20 / 45 / 5 / 5
ns for probe / write / scan / fire), the medium's at its declared prices (200 / 25 / 40 / 2 ns) plus
200 ns per tick; the hand-written noticers are not billed and have no modelled cost. Measured: wall
time of the noticer's calls on the replay, and (in run) the arm's `measured_sched_ns`, in which the
noticer's time is inside the arm's other bookkeeping.
"""
import sys

import numpy as np
import pandas as pd

import c1_common as C


def boot_mean(x, seed=C.BOOT_SEED, resamples=C.N_RESAMPLES, chunk=500):
    x = np.asarray(x, float)
    n = len(x)
    rng = np.random.default_rng(seed)
    out, done = [], 0
    while done < resamples:
        k = min(chunk, resamples - done)
        w = rng.multinomial(n, np.full(n, 1.0 / n), size=k).astype(float)
        out.append(w @ x / n)
        done += k
    d = np.concatenate(out)
    return float(x.mean()), float(np.percentile(d, 5)), float(np.percentile(d, 95))


def boot_diff(a, b, seed=C.BOOT_SEED, resamples=C.N_RESAMPLES):
    return boot_mean(np.asarray(a, float) - np.asarray(b, float), seed, resamples)


def main(replay, run_cost):
    d = pd.read_csv(replay)
    d.to_csv(C.OUT / "c1-costs-per-stream.csv", index=False)
    streams = len(d)
    wall = {k: (d[f"{k}_ns"] - d["null_ns"]) for k in ("b3", "dataflow", "medium")}
    rows = []

    def add(noticer, measure, x, unit):
        m, lo, hi = boot_mean(x)
        rows.append({"noticer": noticer, "measure": measure, "unit": unit, "mean": m, "lo": lo,
                     "hi": hi, "streams": streams})

    for k in ("b3", "dataflow", "medium"):
        add(k, "wall_ns_per_stream (least of 3 replays, less the driver loop)", wall[k], "ns")
    add("driver loop", "wall_ns_per_stream (a noticer that does nothing)", d["null_ns"], "ns")
    add("dataflow", "modelled_ns_per_stream (counted operations at the declared prices)", d["df_modelled_ns"], "ns")
    add("dataflow", "billed_ns_per_stream (the replay's charges, as the bill takes them)", d["df_billed_ns"], "ns")
    for kind in ("probes", "writes", "scans", "fires"):
        add("dataflow", f"counted {kind} per stream", d[f"df_{kind}"], "operations")
    add("medium", "modelled_ns_per_stream (counted operations at 200/25/40/2 ns plus 200 ns per tick)", d["med_modelled_ns"], "ns")
    for k, label in (("ticks", "ticks"), ("cell_updates", "cell updates"), ("synapse_traversals", "synapse traversals"),
                     ("event_routings", "event routings"), ("field_reads", "field reads")):
        add("medium", f"counted {label} per stream", d[f"med_{k}"], "operations")
    add("all", "observations per stream", d["observations"], "observations")
    add("all", "abnormal observations per stream", d["abnormal"], "observations")
    for a, b, label in (("dataflow", "b3", "dataflow minus hand-written B3"), ("medium", "dataflow", "medium minus dataflow"),
                        ("medium", "b3", "medium minus hand-written B3")):
        m, lo, hi = boot_diff(wall[a], wall[b])
        rows.append({"noticer": label, "measure": "paired wall_ns_per_stream", "unit": "ns", "mean": m,
                     "lo": lo, "hi": hi, "streams": streams})
    ratio_df = d["df_modelled_ns"].sum() / wall["dataflow"].sum()
    ratio_med = d["med_modelled_ns"].sum() / wall["medium"].sum()
    rows.append({"noticer": "dataflow", "measure": "measured wall over modelled (sums over streams)",
                 "unit": "ratio", "mean": wall["dataflow"].sum() / d["df_modelled_ns"].sum(), "lo": np.nan, "hi": np.nan,
                 "streams": streams})
    rows.append({"noticer": "medium", "measure": "measured wall over modelled (sums over streams)",
                 "unit": "ratio", "mean": wall["medium"].sum() / d["med_modelled_ns"].sum(), "lo": np.nan, "hi": np.nan,
                 "streams": streams})
    if run_cost:
        r = pd.read_csv(run_cost)
        piv = r.pivot(index="seed", columns="arm", values="measured_sched_ns")
        bill = r.pivot(index="seed", columns="arm", values="bill_compute_ns")
        own = bill[C.DATAFLOW] - bill[C.DATAFLOW_FREE]
        m, lo, hi = boot_mean(own)
        rows.append({"noticer": "dataflow", "measure": "in-run charge per stream (bill_compute, billed arm minus unbilled control)",
                     "unit": "ns", "mean": m, "lo": lo, "hi": hi, "streams": len(own)})
        for arm in piv.columns:
            m, lo, hi = boot_mean(piv[arm])
            rows.append({"noticer": arm, "measure": "in-run measured_sched_ns per stream (the arm's bookkeeping, noticer included)",
                         "unit": "ns", "mean": m, "lo": lo, "hi": hi, "streams": len(piv)})
        for a, b in ((C.DATAFLOW, C.B3_ROW), (C.DATAFLOW_FREE, C.B3_ROW), (C.MEDIUM, C.B3_ROW)):
            m, lo, hi = boot_diff(piv[a], piv[b])
            rows.append({"noticer": f"{a} minus {b}", "measure": "paired in-run measured_sched_ns per stream",
                         "unit": "ns", "mean": m, "lo": lo, "hi": hi, "streams": len(piv)})
    out = pd.DataFrame(rows)
    out.to_csv(C.OUT / "c1-cost.csv", index=False)
    pd.set_option("display.width", 250)
    pd.set_option("display.max_colwidth", 95)
    print(out.to_string(index=False, float_format=lambda x: f"{x:,.1f}"))


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2] if len(sys.argv) > 2 else str(C.OUT / "c1-cost-run.csv"))
