"""R9's rerun of R7's comparison at the reader's delta*: manifests, selection, criterion, points and
provenance, built on R7's machinery without changing it.

Usage:
  r9_rerun.py values                 print the values the rerun uses (from r9-fit-hard.json)
  r9_rerun.py tune                   write the tuning manifests (one per new value)
  r9_rerun.py select                 after the tuning runs: r9-tuning-points.csv and r9-selected.json
  r9_rerun.py heldout                after `select` is committed: write the held-out manifests
  r9_rerun.py criterion              after the held-out runs: r9-rerun-criterion.csv, r9-outcome.json
  r9_rerun.py points                 every arm of every held-out run (critical misses, plain accuracy ...)
  r9_rerun.py provenance             run index, sha256 of every result, checks (r9-run-index.csv ...)

What is R7's, unchanged (`r7_common`, `r7_manifests`, `r7_select`, `r7_criterion`, `r6_stats`): the
manifest writer (`r7_manifests.write`, called here with the run ids renamed `r9-...`, the only change,
made by replacing `r7_common.run_id`), the primary setting (b = 5, rho = 0.7), the selection oracle at
R6's delay (16 s, not re-tuned), every builder configuration of R6's grids on the 100 tuning streams
10000-10099, the carry rule and the choice of "the selected builder" (`r7_select.carry`), the 200
held-out streams 20000-20199, and the statistic G and its paired 90% cluster bootstrap
(`r7_criterion.g_row`, 10,000 resamples). New in R9, and only that: which deltas are run
(`r9_stats.rerun_values`), the run seeds (tuning 9100 + k, held-out 9200 + k, k the index of the run in
this script's list; they order the interleaved arms and nothing else) and the bootstrap seeds
(9710 + k; R7 used 9600 + k).

A value that is within 0.005 of 0 uses R6's own held-out files through R7's procedure
(`r7_criterion.delta0_reference`, not re-run); one within 0.005 of R7's grid reuses R7's tuning and
held-out runs from the main checkout's `artifacts/runs/r7` and R7's `r7-selected.json`. The
reasoner's penalty is clamped at 0 by the simulator, so a negative lower bound cannot be run.

Exploration (nothing here tests a hypothesis).
"""

import hashlib
import json
import os
import pathlib
import re
import sys

import pandas as pd

import r6_stats as S6
import r7_common as C
import r7_criterion as CRIT
import r7_manifests as M7
import r7_select as SEL
import r9_stats as S9
from gordian_analysis.load import load_stream_run

R7_RUNS = pathlib.Path(os.environ.get("R7_RUNS", "/home/user/gordian/artifacts/runs/r7"))
R6_RUNS = pathlib.Path(os.environ.get("R6_RUNS", "/home/user/gordian/artifacts/runs/r6"))
FIT = C.OUT / "r9-fit-hard.json"
TUNE_SEED0, HELDOUT_SEED0, BOOT_SEED0 = 9100, 9200, 9710
b_rho = C.PRIMARY


def r9_run_id(stage, b, rho, delta):
    return f"r9-{stage}-{C.setting_id(b, rho)}-d{C.dlabel(delta)}"


def values():
    fit = json.load(open(FIT))
    lo, hi = fit["delta_interval"]
    return S9.rerun_values(lo, fit["delta"], hi), fit


def new_deltas():
    rows, _ = values()
    out = []
    for r in rows:
        if r["source"] == "new run" and r["run_delta"] not in out:
            out.append(r["run_delta"])
    return out


def run_dir(stage, delta, source):
    """Where a run's directory is: this worktree's for a new run, R7's for a reused one."""
    b, rho = b_rho
    if source == "R7 run reused":
        return R7_RUNS / f"r7-{stage}-{C.setting_id(b, rho)}-d{C.dlabel(delta)}"
    return C.RUNS / r9_run_id(stage, b, rho, delta)


def tune():
    C.run_id = r9_run_id  # R7's writer names a run with `r7_common.run_id`; R9's runs are r9-...
    b, rho = b_rho
    d = C.sel_delay_s(b, rho)
    for k, delta in enumerate(new_deltas()):
        arms = [C.C6.sel_arm(key, d) for key, _, _ in C.C6.all_builders()]
        arms.append(C.C6.ctxonly_arm(d))
        arms.append(C.C6.reference_arms()[0])
        M7.write("tune", b, rho, delta, C.TUNING_SEEDS, arms, TUNE_SEED0 + k, "exploration-r9-tuning")


def heldout():
    C.run_id = r9_run_id
    b, rho = b_rho
    sel = json.load(open(C.OUT / "r9-selected.json"))
    for k, delta in enumerate(new_deltas()):
        s = sel[r9_run_id("tune", b, rho, delta)]
        d = s["selection_delay_s"]
        arms = [C.C6.sel_arm(key, d) for key in s["carried"]]
        arms.append(C.C6.ctxonly_arm(d))
        arms.append(C.C6.reference_arms()[0])
        M7.write("heldout", b, rho, delta, C.HELDOUT_SEEDS, arms, HELDOUT_SEED0 + k, "exploration-r9-heldout")


def select():
    """R7's selection (`r7_select.main`'s loop body) for R9's tuning runs: the same rule, other runs."""
    b, rho = b_rho
    sid = C.setting_id(b, rho)
    d = C.sel_delay_s(b, rho)
    rows, out = [], {}
    for delta in new_deltas():
        rid = r9_run_id("tune", b, rho, delta)
        run = load_stream_run(C.RUNS / rid)
        pts = []
        for key, kind, p in C.C6.all_builders():
            pt = S6.point(run.arms[C.sel_arm_name(key, d)])
            pt.update({"run_id": rid, "setting": sid, "delta": delta, "builder": key, "kind": kind,
                       **{f"p_{k}": v for k, v in p.items()}})
            pts.append(pt)
        df = pd.DataFrame(pts)
        df["frontier"] = False
        for kind in C.KINDS:
            g = df[df["kind"] == kind]
            keep = S6.frontier_keys(g.reset_index(drop=True))
            df.loc[list(g.index[keep]), "frontier"] = True
        carried, top_of, sel = SEL.carry(df, C.r6_frontier_configs(b, rho))
        df["carried"] = df["builder"].isin(carried)
        df["selected"] = df["builder"] == sel
        rows.append(df)
        for name in (C.ctxonly_name(d), C.ORACLE):
            pt = S6.point(run.arms[name])
            pt.update({"run_id": rid, "setting": sid, "delta": delta, "builder": name, "kind": "reference",
                       "frontier": False, "carried": False, "selected": False})
            rows.append(pd.DataFrame([pt]))
        out[rid] = {
            "setting": sid, "b": b, "rho": rho, "delta": delta, "selection_delay_s": d,
            "carried": carried, "top_of_kind": top_of, "selected": sel,
            "selected_tuning_quality": float(df.loc[df["builder"] == sel, "quality"].iloc[0]),
            "ceiling_tuning_quality": float(S6.point(run.arms[C.ctxonly_name(d)])["quality"]),
        }
        print(rid, "carried", len(carried), "selected", sel, round(out[rid]["selected_tuning_quality"], 3), "tops", top_of)
    pd.concat(rows, ignore_index=True).to_csv(C.OUT / "r9-tuning-points.csv", index=False, float_format="%.6g")
    (C.OUT / "r9-selected.json").write_text(json.dumps(out, indent=2) + "\n")


def selection_for(row):
    """`(selected builder key, its tuning quality)` of a value's run: R9's, or R7's for a reused run."""
    b, rho = b_rho
    if row["source"] == "R7 run reused":
        s = json.load(open(C.OUT / "r7-selected.json"))[f"r7-tune-{C.setting_id(b, rho)}-d{C.dlabel(row['run_delta'])}"]
    else:
        s = json.load(open(C.OUT / "r9-selected.json"))[r9_run_id("tune", b, rho, row["run_delta"])]
    return s["selected"], s["selected_tuning_quality"]


def criterion():
    rows, fit = values()
    b, rho = b_rho
    d = C.sel_delay_s(b, rho)
    out = []
    for k, r in enumerate(rows):
        row = {"which": r["which"], "delta_star_value": r["raw"], "run_delta": r["run_delta"],
               "source": r["source"], "same_run_as": r.get("same_as", "")}
        if r["source"] == "R6 files (delta 0)":
            g = CRIT.delta0_reference()
        else:
            key, tq = selection_for(r)
            hid = run_dir("heldout", r["run_delta"], r["source"])
            g = CRIT.g_row(hid, C.ctxonly_name(d), C.sel_arm_name(key, d), BOOT_SEED0 + k)
            g.update({"selected": key, "selected_tuning_quality": tq, "run_id": hid.name})
        row.update(g)
        out.append(row)
        print(f"{r['which']:5s} delta {r['run_delta']:g} ({r['source']}): selected {row['selected']} "
              f"G {row['G']:.3f} [{row['lo']:.3f}, {row['hi']:.3f}]")
    pd.DataFrame(out).to_csv(C.OUT / "r9-rerun-criterion.csv", index=False, float_format="%.6g")
    by = {o["which"]: o for o in out}
    pre = fit["precondition"]["holds"]
    res = {
        "outcome": S9.outcome(pre, by["lo"], by["hi"]),
        "reader_precondition_holds": pre,
        "delta_star": {"lo": rows[0]["raw"], "point": rows[1]["raw"], "hi": rows[2]["raw"]},
        "run_delta": {r["which"]: r["run_delta"] for r in rows},
        "G": {w: {"G": by[w]["G"], "lo": by[w]["lo"], "hi": by[w]["hi"], "selected": by[w]["selected"]}
              for w in ("lo", "point", "hi")},
        "r6_regime_condition": {"upper_bound_of_G_at_delta_star_hi_below_0.10": bool(by["hi"]["hi"] < S9.R6_G_UPPER)},
        "r7_regime_condition": {"G_at_delta_star_lo_at_least_0.10_with_lower_bound_above_0.05":
                                bool(by["lo"]["G"] >= S9.R7_G_AT_LEAST and by["lo"]["lo"] > S9.R7_G_LOWER)},
    }
    (C.OUT / "r9-outcome.json").write_text(json.dumps(res, indent=2) + "\n")
    print("OUTCOME:", res["outcome"])


def points():
    """Every arm of every held-out run: hard-incident quality, references per call, cost, critical
    misses, plain accuracy, calls refused (`r6_stats.point`), with the selected builder flagged and the
    held-out-best carried builder's gap to the ceiling beside it."""
    rows, _ = values()
    b, rho = b_rho
    d = C.sel_delay_s(b, rho)
    allrows, seen = [], set()
    for k, r in enumerate(rows):
        if r["source"] == "R6 files (delta 0)":
            continue
        key = (r["run_delta"], r["source"])
        if key in seen:
            continue
        seen.add(key)
        hdir = run_dir("heldout", r["run_delta"], r["source"])
        sel_key, _ = selection_for(r)
        run = load_stream_run(hdir)
        streams = S6.Streams(run)
        ceiling = C.ctxonly_name(d)
        sel_arms = [n for n in run.arms if n.startswith("sel_")]
        best = sorted(sel_arms, key=lambda n: (-S6.point(run.arms[n])["quality"], S6.point(run.arms[n])["refs_per_call"]))[0]
        gb, glo, ghi = S6.paired(streams, ceiling, best, BOOT_SEED0 + 50 + k)
        for name, arm in run.arms.items():
            pt = S6.point(arm)
            role = "ceiling" if name == ceiling else "R4 oracle (reference)" if name == C.ORACLE else (
                "selected builder" if name == C.sel_arm_name(sel_key, d) else "carried builder")
            pt.update({"run_id": hdir.name, "delta": r["run_delta"], "role": role,
                       "held_out_best": name == best, "held_out_best_gap": gb if name == best else "",
                       "held_out_best_gap_lo": glo if name == best else "",
                       "held_out_best_gap_hi": ghi if name == best else ""})
            allrows.append(pt)
    pd.DataFrame(allrows).to_csv(C.OUT / "r9-rerun-points.csv", index=False, float_format="%.6g")
    print(len(allrows), "arm rows")


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def provenance():
    log = C.RUNS / "_logs" / "r9-runs.log"
    status = []
    for line in open(log):
        m = re.match(r"(\S+) exit=(\d+) wall_s=(\d+)$", line.strip())
        if m:
            status.append((m[1], int(m[2]), int(m[3])))
    pd.DataFrame(status, columns=["run_id", "driver_exit", "wall_s"]).to_csv(C.OUT / "r9-driver-log.csv", index=False)
    index, shas = [], []
    dirs = sorted(p for p in C.RUNS.iterdir() if p.is_dir() and p.name.startswith("r9-") and (p / "manifest.json").exists()
                  and (p.name.startswith("r9-tune-") or p.name.startswith("r9-heldout-")))
    for dd in dirs:
        man = json.load(open(dd / "manifest.json"))
        usage = json.load(open(dd / "usage.json"))
        arms = sorted(a.name for a in dd.iterdir() if a.is_dir())
        for a in arms:
            shas.append({"dir": dd.name, "arm": a, "results_sha256": sha(dd / a / "results.csv"),
                         "incidents_sha256": sha(dd / a / "incidents.csv")})
        r = man["stream_params"]["reasoner"]
        index.append({
            "run_id": dd.name, "experiment": man["experiment"], "source_revision": man["source_revision"],
            "arms": len(arms), "streams": len(man["seeds"]), "seed_first": man["seeds"][0],
            "seed_last": man["seeds"][-1], "b": r["b"], "rho": r["rho"],
            "distractor_penalty": r.get("distractor_penalty", 0.0), "run_seed": man["run_seed"],
            "manifest_sha256": sha(dd / "manifest.json"), "usage_exit_code": usage.get("exit_code"),
            "wall_s": round(usage["wall_ns"] / 1e9, 1), "cpu_s": round(usage["cpu_ns"] / 1e9, 1),
            "peak_memory_mb": round(usage["peak_memory_bytes"] / 2**20, 1), "oom_kills": usage.get("oom_kills"),
            "internal_external_ratio": usage.get("internal_external_ratio"), "toolchain": man["toolchain"],
            "cpu_model": man.get("cpu_model"),
        })
    ix = pd.DataFrame(index)
    ix.to_csv(C.OUT / "r9-run-index.csv", index=False)
    pd.DataFrame(shas).to_csv(C.OUT / "r9-results-sha256.csv", index=False)
    deltas = new_deltas()
    for stage in ("tune", "heldout"):
        g = ix[ix.run_id.str.startswith(f"r9-{stage}-")]
        want = {r9_run_id(stage, *b_rho, dl) for dl in deltas}
        assert set(g.run_id) == want, (want ^ set(g.run_id))
        seeds = {(10000, 10099)} if stage == "tune" else {(20000, 20199)}
        assert set(zip(g.seed_first, g.seed_last)) == seeds, stage
        for _, r in g.iterrows():
            dl = float(re.search(r"-d([0-9.]+)$", r.run_id).group(1))
            assert r.distractor_penalty == dl, (r.run_id, r.distractor_penalty)
        print(f"check: {stage}: {len(g)} runs, R6's seeds, each manifest records the penalty it was asked to run")
    ref = None
    for rid in sorted(ix[ix.run_id.str.startswith("r9-heldout-")].run_id):
        arm = next(a for a in sorted((C.RUNS / rid).iterdir()) if a.is_dir() and a.name.startswith("sel_"))
        inc = pd.read_csv(arm / "incidents.csv")[["seed", "incident", "tier", "family", "critical"]]
        ref = inc if ref is None else ref
        assert inc.equals(ref), f"{rid}: incidents differ"
    r6 = pd.read_csv(R6_RUNS / "r6-heldout-b5-rho0.7" / C.C6.NEVER / "incidents.csv")[
        ["seed", "incident", "tier", "family", "critical"]]
    assert r6.equals(ref), "held-out incidents differ from R6's"
    print("check: the held-out incidents are identical in every run and equal R6's")
    b, rho = b_rho
    d = C.sel_delay_s(b, rho)
    rows = []
    for dl in deltas:
        hid = r9_run_id("heldout", b, rho, dl)
        for name in (C.ctxonly_name(d), C.ORACLE):
            row = {"run_id": hid, "delta": dl, "arm": name}
            for f in ("results", "incidents"):
                mine = pd.read_csv(C.RUNS / hid / name / f"{f}.csv").drop(columns=["run_id"])
                theirs = pd.read_csv(R6_RUNS / f"r6-heldout-{C.setting_id(b, rho)}" / name / f"{f}.csv").drop(columns=["run_id"])
                row[f"{f}_equal_to_r6_but_run_id"] = bool(mine.equals(theirs))
            rows.append(row)
    x = pd.DataFrame(rows)
    x.to_csv(C.OUT / "r9-ceiling-vs-r6.csv", index=False)
    print(x.to_string(index=False))


def main():
    cmd = sys.argv[1] if len(sys.argv) > 1 else ""
    fn = {"values": lambda: print(json.dumps(values()[0], indent=2)), "tune": tune, "select": select,
          "heldout": heldout, "criterion": criterion, "points": points, "provenance": provenance}.get(cmd)
    if fn is None:
        raise SystemExit(__doc__)
    fn()


if __name__ == "__main__":
    main()
