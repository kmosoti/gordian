"""M4's frontier on the tuning streams: anchor-correct against strict precision, with leak noticed as
a third axis, for every configuration of every M4 tuning stage, per tick length.

Usage: m4_frontier.py   reads experiments/exploration/m4-tuning-points.csv (m4_select.py writes it);
                        writes experiments/exploration/m4-frontier.csv (every configuration with
                        its device switches and whether it is on the frontier) and prints the
                        frontier per tick length.

A configuration is on the frontier (Pareto-optimal) when no other configuration at the same tick
length, within the background bound, is at least as good on all three of anchor-correct, strict
precision and leak noticed and better on one. Nothing is chosen from this table: the frozen medium
is the tuning rule's (m4_select.py). It is analysis of the tuning streams, written after the rule.
"""

import pandas as pd

import m4_common as C
import m4_select as SEL

AXES = ["anchor_ok", "strict_prec", "leak_noticed"]


def switches(n):
    merge = n.get("merge", True) and n.get("merge_window_ns", 0) > 0
    event = n.get("confirm_in_event_time", True) and n.get("confirm_window_ns", 0) > 0
    if not n.get("ramp_inhibit", False):
        inhibit = "off"
    else:
        inhibit = n.get("ramp_inhibit_form", "any")
    rr = n.get("ramp_refractory_ns")
    return {
        "merge_ms": n.get("merge_window_ns", 0) // C.MS if merge else 0,
        "confirm": (f"event {n.get('burst_confirm')} {n.get('confirm_window_ns', 0) // C.MS} ms"
                    if event else
                    (f"latch {n.get('burst_confirm')}" if n.get("burst_confirm", "none") != "none"
                     else "none")),
        "inhibit": inhibit,
        "ramp_refractory_s": (rr if rr is not None else n["refractory_ns"]) / C.NS,
        "hold_s": n["hold_ns"] / C.NS,
    }


def pareto(g):
    on = []
    vals = g[AXES].to_numpy()
    for i in range(len(g)):
        dominated = ((vals >= vals[i]).all(axis=1) & (vals > vals[i]).any(axis=1)).any()
        on.append(not dominated)
    return on


def main():
    pts = pd.read_csv(C.OUT / "m4-tuning-points.csv")
    rows = []
    for _, r in pts.iterrows():
        row = {"stage": r.stage, "arm": r.arm, "tick_ms": r.tick_ms}
        row.update(switches(SEL.noticer_of(r.stage, r.arm)))
        for k in AXES + ["background", "ac_lo", "ln_lo", "leak_anchor_ok", "per_incident",
                         "cost_s", "calls", "conjunction", "graded"]:
            row[k] = r[k]
        rows.append(row)
    df = pd.DataFrame(rows)
    df["within_background"] = df.background <= C.BACKGROUND_BOUND
    df["frontier"] = False
    for tick in C.TICKS_MS:
        idx = df.index[(df.tick_ms == tick) & df.within_background]
        df.loc[idx, "frontier"] = pareto(df.loc[idx])
    df = df.sort_values(["tick_ms", "strict_prec"], ascending=[True, False])
    df.to_csv(C.OUT / "m4-frontier.csv", index=False)
    cols = ["stage", "arm", "merge_ms", "confirm", "inhibit", "ramp_refractory_s", "hold_s",
            "anchor_ok", "strict_prec", "leak_noticed", "background", "leak_anchor_ok",
            "cost_s", "conjunction"]
    with pd.option_context("display.width", 250, "display.max_rows", 500):
        for tick in C.TICKS_MS:
            f = df[(df.tick_ms == tick) & df.frontier]
            print(f"--- {tick} ms: {len(f)} on the frontier of "
                  f"{int(((df.tick_ms == tick) & df.within_background).sum())}")
            print(f[cols].to_string(index=False, float_format=lambda x: f"{x:.3f}"))


if __name__ == "__main__":
    main()
