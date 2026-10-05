"""Choose, per R7 run, what is carried from the tuning streams to the held-out streams, and which
carried configuration is "the selected builder". Tuning streams only.

Usage:
  r7_select.py          after the six tuning runs; writes r7-tuning-points.csv (every arm of every
                        tuning run, with its quality, references per call and whether it is on its
                        builder's frontier at that delta) and r7-selected.json

The rules, fixed here before any R7 run (docs/local-test-plan.md, 5R, R7, "Tuning"):

* Per run (that is, per delta and setting), every grid configuration of R6 ran on R6's 100 tuning
  streams with the selection oracle at R6's delay.
* For each builder kind (`window`, `cooccur`, `neighbourhood`, and the single `rung` configuration),
  the configuration of highest tuning quality *at this delta* is carried to the held-out streams, and
  with it "the three next-best on R6's frontier". Readings:
    - "R6's frontier" is the set of configurations on R6's tuning frontier of quality against
      references per call at delta = 0 for the same setting (`r6-frontier.json`, `configs`).
    - "next-best" is by tuning quality at this delta, among the builder kind's configurations on that
      set other than the top one, and the three highest are taken (fewer if fewer exist).
    - ties in tuning quality break to the fewest references per call, then to grid order.
* "The selected builder" is the carried configuration of highest tuning quality across all builder
  kinds and the rung, with the same tie-break.

Quality is the pooled fraction of hard incidents outside the slow-leak family declared correctly by
their deadline; references per call is the pooled ratio of references to calls (`r6_stats.point`).

Exploration (nothing here tests a hypothesis).
"""

import json

import pandas as pd

import r6_stats as S
import r7_common as C
from gordian_analysis.frontier import pareto_mask  # noqa: F401  (frontier flags use r6_stats)
from gordian_analysis.load import load_stream_run


def rank_key(row):
    return (-row["quality"], row["refs_per_call"], C.GRID_ORDER[row["builder"]])


def carry(df, r6_configs):
    """The carried configuration keys and the selected one, from one run's tuning points `df` (one
    row per builder configuration: `builder`, `kind`, `quality`, `refs_per_call`)."""
    on_r6 = set(r6_configs)
    carried = []
    top_of = {}
    for kind in C.KINDS:
        g = df[df["kind"] == kind]
        rows = sorted((r for _, r in g.iterrows()), key=rank_key)
        top = rows[0]
        top_of[kind] = top["builder"]
        carried.append(top["builder"])
        if kind == "rung":
            continue
        nxt = [r for r in rows if r["builder"] != top["builder"] and r["builder"] in on_r6]
        carried += [r["builder"] for r in nxt[: C.CARRY_NEXT]]
    cdf = df[df["builder"].isin(carried)]
    sel = sorted((r for _, r in cdf.iterrows()), key=rank_key)[0]["builder"]
    return carried, top_of, sel


def main():
    rows, out = [], {}
    for b, rho, delta in C.RUNS_SPEC:
        rid = C.run_id("tune", b, rho, delta)
        sid = C.setting_id(b, rho)
        d = C.sel_delay_s(b, rho)
        run = load_stream_run(C.RUNS / rid)
        pts = []
        for key, kind, p in C.C6.all_builders():
            pt = S.point(run.arms[C.sel_arm_name(key, d)])
            pt.update({"run_id": rid, "setting": sid, "delta": delta, "builder": key, "kind": kind,
                       **{f"p_{k}": v for k, v in p.items()}})
            pts.append(pt)
        df = pd.DataFrame(pts)
        df["frontier"] = False
        for kind in C.KINDS:
            g = df[df["kind"] == kind]
            keep = S.frontier_keys(g.reset_index(drop=True))
            df.loc[list(g.index[keep]), "frontier"] = True
        carried, top_of, sel = carry(df, C.r6_frontier_configs(b, rho))
        df["carried"] = df["builder"].isin(carried)
        df["selected"] = df["builder"] == sel
        rows.append(df)
        for name in (C.ctxonly_name(d), C.ORACLE):
            pt = S.point(run.arms[name])
            pt.update({"run_id": rid, "setting": sid, "delta": delta, "builder": name, "kind": "reference",
                       "frontier": False, "carried": False, "selected": False})
            rows.append(pd.DataFrame([pt]))
        out[rid] = {
            "setting": sid, "b": b, "rho": rho, "delta": delta, "selection_delay_s": d,
            "carried": carried, "top_of_kind": top_of, "selected": sel,
            "selected_tuning_quality": float(df.loc[df["builder"] == sel, "quality"].iloc[0]),
            "ceiling_tuning_quality": float(S.point(run.arms[C.ctxonly_name(d)])["quality"]),
        }
        print(rid, "carried", len(carried), "selected", sel, round(out[rid]["selected_tuning_quality"], 3),
              "tops", top_of)
    pd.concat(rows, ignore_index=True).to_csv(C.OUT / "r7-tuning-points.csv", index=False, float_format="%.6g")
    (C.OUT / "r7-selected.json").write_text(json.dumps(out, indent=2) + "\n")


if __name__ == "__main__":
    main()
