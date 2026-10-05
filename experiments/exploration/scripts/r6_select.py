"""Choose, per reasoner setting, what R6 runs next, from tuning streams only.

Usage:
  r6_select.py frontier     after the tune1 runs: each builder's frontier of tuning quality against
                            references per call (selection oracle at its R5-tuned delay);
                            writes r6-tuning-selection.csv and r6-frontier.json
  r6_select.py selected     after the tune2 runs: for each frontier configuration, the delay of
                            `always_escalate` that the tuning streams pick; writes r6-tuning-always.csv
                            and r6-selected.json

Exploration (nothing here tests a hypothesis). The rules are fixed here, before any held-out run:

* The comparison of builders is on a frontier, not a single point: for each builder kind (`window`,
  `cooccur`, `neighbourhood`, and the single `rung` configuration), the configurations on that
  kind's frontier of hard-incident quality (slow leak excluded, pooled) against references per call
  (pooled sum of references over sum of calls), computed on the 100 tuning streams with the
  selection oracle at the delay R5 tuned for the setting (not re-tuned). Configurations that tie
  exactly on both axes built the same contexts and count once (the first in grid order).
  The rung is always carried, on or off a frontier, as R5's reference.
* `always_escalate`'s delay is tuned for every configuration that goes to the held-out run: the
  delay of highest tuning quality, the cheapest total cost of ties.
* For the re-trace (primary setting only): per kind, the configuration of highest tuning quality
  (fewest references of ties) and the cheapest configuration within 0.05 of it.

Nothing is chosen on any held-out stream.
"""

import json
import sys

import pandas as pd

import r6_common as C
import r6_stats as S
from gordian_analysis.load import load_stream_run


def r5_delay(sid):
    return C.load_json(C.OUT / "r5-selected.json")[sid]["selection_delay_s"]


def frontier():
    rows, out = [], {}
    for b, rho in C.SETTINGS:
        sid = C.setting_id(b, rho)
        run = load_stream_run(C.RUNS / C.run_id("tune1", b, rho))
        d = r5_delay(sid)
        pts = []
        for key, kind, p in C.all_builders():
            name = C.sel_arm(key, d)[0]
            pt = S.point(run.arms[name])
            pt.update({"setting": sid, "builder": key, "kind": kind, **{f"p_{k}": v for k, v in p.items()}})
            pts.append(pt)
        df = pd.DataFrame(pts)
        df["frontier"] = False
        df["chosen"] = False
        chosen = []
        for kind in C.KINDS:
            g = df[df["kind"] == kind]
            keep = S.frontier_keys(g.reset_index(drop=True))
            idx = list(g.index[keep])
            df.loc[idx, "frontier"] = True
            chosen += [df.loc[i, "builder"] for i in idx]
        if "rung" not in chosen:
            chosen.append("rung")
        df["chosen"] = df["builder"].isin(chosen)
        best = {}
        for kind in C.KINDS:
            g = df[df["kind"] == kind]
            top = g[g["quality"] == g["quality"].max()].sort_values(["refs_per_call", "builder"]).iloc[0]
            near = g[(g["quality"] >= top["quality"] - 0.05) & g["frontier"] | (g["builder"] == top["builder"])]
            knee = near.sort_values(["refs_per_call", "builder"]).iloc[0]
            best[kind] = {"best": top["builder"], "knee": knee["builder"]}
        out[sid] = {
            "selection_delay_s": d,
            "configs": chosen,
            "frontier_sizes": {k: int(df[(df["kind"] == k) & df["frontier"]].shape[0]) for k in C.KINDS},
            "best": best,
        }
        ref = [a[0] for a in C.reference_arms() if a[0] in run.arms] + [C.ctxonly_arm(d)[0]]
        for name in ref:
            pt = S.point(run.arms[name])
            pt.update({"setting": sid, "builder": name, "kind": "reference"})
            pts.append(pt)
        rows.append(pd.DataFrame(pts).assign(frontier=lambda x, df=df: x["builder"].map(
            dict(zip(df["builder"], df["frontier"]))).fillna(False)).assign(
            chosen=lambda x, df=df: x["builder"].map(dict(zip(df["builder"], df["chosen"]))).fillna(False)))
        print(sid, "delay", d, "chosen", len(chosen), out[sid]["frontier_sizes"])
    allpts = pd.concat(rows, ignore_index=True)
    allpts.to_csv(C.OUT / "r6-tuning-selection.csv", index=False, float_format="%.6g")
    (C.OUT / "r6-frontier.json").write_text(json.dumps(out, indent=2) + "\n")


def selected():
    fr = C.load_json(C.OUT / "r6-frontier.json")
    rows, out = [], {}
    for b, rho in C.SETTINGS:
        sid = C.setting_id(b, rho)
        run = load_stream_run(C.RUNS / C.run_id("tune2", b, rho))
        always = {}
        for key in fr[sid]["configs"]:
            pts = []
            for d in C.ALWAYS_DELAYS_S:
                name = C.always_arm(key, d)[0]
                pt = S.point(run.arms[name])
                pt.update({"setting": sid, "builder": key, "delay_s": d})
                pts.append(pt)
            df = pd.DataFrame(pts)
            top = df[df["quality"] == df["quality"].max()].sort_values(["cost_s", "delay_s"]).iloc[0]
            always[key] = int(top["delay_s"])
            df["chosen"] = df["delay_s"] == top["delay_s"]
            rows.append(df)
        f = fr[sid]
        diag = []
        for kind in ("rung", "window", "cooccur", "neighbourhood"):
            for k in ("best", "knee"):
                key = f["best"][kind][k]
                if key not in diag:
                    diag.append(key)
        out[sid] = {
            "selection_delay_s": f["selection_delay_s"],
            "sel_configs": f["configs"],
            "always_configs": always,
            "diag_sel_configs": diag,
            "best": f["best"],
        }
        print(sid, {k: v for k, v in always.items() if k in ("rung",)}, len(always))
    pd.concat(rows, ignore_index=True).to_csv(C.OUT / "r6-tuning-always.csv", index=False, float_format="%.6g")
    (C.OUT / "r6-selected.json").write_text(json.dumps(out, indent=2) + "\n")


if __name__ == "__main__":
    {"frontier": frontier, "selected": selected}[sys.argv[1]]()
