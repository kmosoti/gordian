"""E1's held-out table and the tuning tables, from the raw evaluator files of the kept runs.

Usage: e1_table.py [--run DIR] [--tune-run DIR] [--out PREFIX]

Reads the held-out run (seeds 40000-40199, `artifacts/runs/e1/e1-heldout-b5-rho0.7`) with
`gordian_analysis.memory`, every number a ratio of sums over whole streams with a 90% percentile
cluster bootstrap over streams (10,000 resamples, seed 9950, W2's constants), and writes beside it
W2's ceiling rows from `w2-ceiling.csv` (the same arm on the same streams: `sel_reanchor_privileged`,
identical to the control here, which this script checks). Files written (all in
experiments/exploration/):

  e1-heldout-table.csv   one row per arm and measure: point, lower, upper, num, den, streams
  e1-heldout-paired.csv  every arm minus the memoryless control on the same streams
  e1-heldout-excess.csv  the paired excess of unasked wrong declarations over the control, by tier
  e1-heldout-captured.csv  what each arm's unasked correct hard incidents were: by population, by
                         family, and the recalls by outcome, source and tier
  e1-heldout-curves.csv  the experience curves across streams (cumulative share of the hard
                         incidents seen that were unasked correct) and their slopes per 100 streams
  e1-heldout-within.csv  the within-stream curve: the n-th hard incident's mean cumulative unasked
                         correct, by arm
  e1-ceiling.csv         W2's ceiling rows (R, F, F_cross) with this run's control beside them
  e1-heldout-table.md    the readable table
"""

import argparse
import csv
import json
import pathlib

import numpy as np
import pandas as pd

import e1_common as C
from gordian_analysis import memory as M

EXPL = C.OUT
ARM_PREFIX, ARM_SUFFIX = "sel_", "_privileged"


def short(dirname):
    return dirname[len(ARM_PREFIX):-len(ARM_SUFFIX)] if dirname.startswith(ARM_PREFIX) else dirname


def label(name):
    """A readable row label of an arm name: form, level, policy, reset."""
    if name == "reanchor":
        return "memoryless re-anchor (control)"
    parts = name.split("_")
    form, level, pol, reset = parts[1], parts[2], parts[3], parts[4]
    extra = " +msg ids" if name.endswith("_msgids") else (" (sensitivity)" if name.endswith("_sens") else "")
    return f"{form}-keyed, {level}, confirm {pol}, {'reset' if reset == 'reset' else 'carried'}{extra}"


SHOWN = [
    "unasked_correct_hard_recurrences", "unasked_correct_hard_elsewhere", "unasked_correct_hard_reachable",
    "unasked_correct_hard", "collision_share", "collisions_per_stream", "inherited_per_stream",
    "inherited_share", "recalls_per_stream", "calls_per_correct", "calls_per_stream", "cost_s",
    "reasoner_cost_s", "unasked_wrong_plain_per_stream", "unasked_wrong_hard_per_stream",
    "unasked_wrong_decoy_per_stream", "stale_wrong_hard_per_stream", "stale_wrong_plain_per_stream",
    "stale_wrong_decoy_per_stream",
]


def fmt(p, lo, hi, nd=3):
    return f"{p:.{nd}f} [{lo:.{nd}f}, {hi:.{nd}f}]" if np.isfinite(p) else "n/a"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", default=str(C.E1_DIR / C.run_id("heldout")))
    ap.add_argument("--out", default="e1-heldout")
    args = ap.parse_args()
    boot = M.Boot()
    arms = M.load_arms(args.run)
    tabs = {short(k): M.streamtab(v) for k, v in arms.items()}
    control = tabs["reanchor"]
    n = len(control)

    rows, prow, erows, crow, wrows = [], [], [], [], []
    for name, t in tabs.items():
        for m in SHOWN:
            rows.append({"arm": name, **M.pooled(t, m, boot)})
        if name != "reanchor":
            for m in ("unasked_correct_hard", "unasked_correct_hard_reachable", "calls_per_stream", "cost_s",
                      "reasoner_cost_s", "calls_per_correct"):
                prow.append({"arm": name, **M.paired(t, control, m, boot)})
            for _, e in M.paired_excess(t, control, boot).iterrows():
                erows.append({"arm": name, **e.to_dict()})
        # What the unasked correct hard incidents were
        mi = arms[ARM_PREFIX + name + ARM_SUFFIX].memory_incidents
        hard = mi[mi["tier"] == "hard"]
        uc = hard[hard["unasked_correct"]]
        pop = np.where(uc["recurrence_of"].notna(), "recurrence", np.where(uc["same_family_earlier"], "elsewhere", "neither"))
        crow.append({"arm": name, "hard_incidents": len(hard), "unasked_correct": len(uc),
                     "of_recurrences": int((pop == "recurrence").sum()), "of_elsewhere": int((pop == "elsewhere").sum()),
                     "of_neither": int((pop == "neither").sum()),
                     **{f"family_{f}": int((uc["family"] == f).sum()) for f in ("compound", "cascade", "split_brain", "slow_leak")},
                     **{f"hard_{f}": int((hard["family"] == f).sum()) for f in ("compound", "cascade", "split_brain", "slow_leak")}})
        r = arms[ARM_PREFIX + name + ARM_SUFFIX].recalls
        if len(r):
            crow[-1].update({"recalls": len(r), "recalls_correct": int(r["correct"].sum()),
                             "recalls_on_hard": int((r["tier"] == "hard").sum()), "recalls_on_plain": int((r["tier"] == "plain").sum()),
                             "recalls_on_decoy": int((r["tier"] == "decoy").sum()), "recalls_on_background": int((r["tier"] == "").sum()),
                             "recalls_source_age_0": int((r["source_age"] == 0).sum()),
                             "recalls_source_age_pos": int((r["source_age"] > 0).sum())})
        else:
            crow[-1].update({"recalls": 0})
        # experience curves
        c = M.curve_across_streams(t, "unasked_correct_hard")
        for stream, share, cum in zip(c["stream"], c["share"], c["cum_event"]):
            if stream in (1, 10, 20, 50, 100, 150, 200):
                wrows.append({"arm": name, "kind": "across", "stream": int(stream), "cum_unasked_correct": cum, "share": share})
        for k in (50, 100, n):
            s = M.slope_across_streams(t.iloc[:k], boot, per=100.0)
            wrows.append({"arm": name, "kind": f"slope_per_100_streams_first_{k}", "stream": k, **s})
    # within-stream curves (all arms): the n-th hard incident's mean cumulative unasked correct
    within = []
    for name in tabs:
        cw = M.curve_within_streams(arms[ARM_PREFIX + name + ARM_SUFFIX].memory_incidents, "unasked_correct", "hard", 20)
        cw.insert(0, "arm", name)
        within.append(cw)
    pd.DataFrame(within and pd.concat(within)).to_csv(EXPL / f"{args.out}-within.csv", index=False)

    pd.DataFrame(rows).to_csv(EXPL / f"{args.out}-table.csv", index=False)
    pd.DataFrame(prow).to_csv(EXPL / f"{args.out}-paired.csv", index=False)
    pd.DataFrame(erows).to_csv(EXPL / f"{args.out}-excess.csv", index=False)
    pd.DataFrame(crow).to_csv(EXPL / f"{args.out}-captured.csv", index=False)
    pd.DataFrame(wrows).to_csv(EXPL / f"{args.out}-curves.csv", index=False)

    # W2's ceiling beside this run's control (the same arm on the same streams).
    w2 = list(csv.DictReader(open(EXPL / "w2-ceiling.csv")))
    held = [r for r in w2 if r["range"] == "a-heldout"]
    ctl_calls = int(control["reasoner_calls"].sum())
    ctl_hard = int(control["incidents_hard"].sum())
    ceil = []
    for r in held:
        ceil.append({"set": r["set"].split("  ")[0].strip(), "description": r["set"], "seeds": r["seeds"],
                     "hard_incidents": int(r["incidents"]), "calls": int(r["calls"]),
                     "share_of_total_bill": float(r["share_of_total_bill"]), "bill_lo90": float(r["bill_lo90"]),
                     "bill_hi90": float(r["bill_hi90"])})
    ceil.append({"set": "this run's control", "description": "sel_reanchor_privileged on 40000-40199 in this run",
                 "seeds": "40000-40199", "hard_incidents": ctl_hard, "calls": ctl_calls,
                 "share_of_total_bill": float("nan"), "bill_lo90": float("nan"), "bill_hi90": float("nan")})
    pd.DataFrame(ceil).to_csv(EXPL / "e1-ceiling.csv", index=False)

    # The readable table
    by = {(r["arm"], r["measure"]): r for r in rows}
    cells = {"all": M.pooled}  # noqa: F841
    order = ["reanchor"] + [a for a in tabs if a != "reanchor" and not a.endswith(("_sens", "_msgids"))] \
        + [a for a in tabs if a.endswith(("_sens", "_msgids"))]
    md = ["# E1: the record rung on the held-out streams (seeds 40000-40199, in stream order)", "",
          "Generated by `experiments/exploration/scripts/e1_table.py` from the raw files of "
          f"`{pathlib.Path(args.run).name}`. 90% percentile intervals of a cluster bootstrap over 200 whole streams.", ""]
    for title, ms in (
        ("Unasked-correct share (hard incidents declared correctly with no escalation about them)",
         ["unasked_correct_hard_recurrences", "unasked_correct_hard_elsewhere", "unasked_correct_hard_reachable", "unasked_correct_hard"]),
        ("What the memory got wrong (stale errors; a collision is a wrong recall whose stored answer was right for its own incident; inherited, whose stored answer was wrong)",
         ["recalls_per_stream", "collision_share", "collisions_per_stream", "inherited_per_stream", "inherited_share"]),
        ("What it cost (per stream; cost is total_cost_ns + noticer_ns in modelled seconds)",
         ["calls_per_stream", "calls_per_correct", "cost_s", "reasoner_cost_s"]),
        ("Unasked wrong declarations per stream (every origin; read as the paired excess over the control)",
         ["unasked_wrong_plain_per_stream", "unasked_wrong_hard_per_stream", "unasked_wrong_decoy_per_stream"]),
    ):
        md += [f"## {title}", "", "| arm | " + " | ".join(ms) + " |", "|---|" + "---|" * len(ms)]
        for a in order:
            cs = []
            for m in ms:
                r = by[(a, m)]
                share = m.startswith("unasked_correct") or m.endswith("_share")
                cs.append((fmt(r["point"], r["lower"], r["upper"]) + (f" ({int(r['num'])}/{int(r['den'])})" if share else "")))
            md.append(f"| {label(a)} | " + " | ".join(cs) + " |")
        md.append("")
    pe = pd.DataFrame(erows)
    md += ["## Paired excess of unasked wrong declarations over the memoryless control (incidents per stream)", "",
           "| arm | plain | hard | decoy | all |", "|---|---|---|---|---|"]
    for a in order[1:]:
        e = pe[pe["arm"] == a].set_index("tier")
        md.append(f"| {label(a)} | " + " | ".join(fmt(e.loc[t, "excess"], e.loc[t, "lower"], e.loc[t, "upper"], 2)
                                                 for t in ("plain", "hard", "decoy", "all")) + " |")
    md += ["", "## W2's ceiling beside it (hidden side, the same arm on the same streams)", "",
           "| set | hard incidents | calls | share of the control's bill [90%] |", "|---|---|---|---|"]
    for c in ceil[:-1]:
        md.append(f"| {c['description'].strip()} | {c['hard_incidents']} | {c['calls']} | "
                  f"{c['share_of_total_bill']:.3f} [{c['bill_lo90']:.3f}, {c['bill_hi90']:.3f}] |")
    (EXPL / f"{args.out}-table.md").write_text("\n".join(md) + "\n")
    print("\n".join(md[:60]))


if __name__ == "__main__":
    main()
