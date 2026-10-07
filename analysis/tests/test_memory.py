"""Tests of the memory files' loader and of `gordian_analysis.memory` (work item E1), on small run
directories written here, every expected number counted by hand in the comments.

The streams: seeds 5, 6, 7. The arm `rec` has recalls; the arm `ctl` is the memoryless control that
played the same streams.
"""

from __future__ import annotations

import csv
from pathlib import Path

import numpy as np
import pandas as pd
import pytest

from gordian_analysis import load as L
from gordian_analysis import memory as M
from stream_fixtures import incident, write_stream_arm

CELLS = L.STREAM_RECALL_CELLS


def inc(tier, family="", *, rec=None, fam=False, correct=0, wrong=0, esc=0, cells=None):
    """An incident of a stream: its evaluator row, and what memory_incidents.csv says about it."""
    row = incident(tier, family, correct_declarations=correct, wrong_declarations=wrong, escalations=esc,
                   correct_by_deadline=(correct > 0 and tier != "decoy"))
    return {"row": row, "recurrence_of": rec, "same_family_earlier": fam, "cells": cells or {}}


def recall(seed, step, incident_id, tier, correct, cls, source_obs=7, source_seed=None, age=0, background=False):
    return {"seed": seed, "step": step, "incident": "" if background else incident_id, "tier": "" if background else tier,
            "correct": correct, "class": cls, "source_obs": "" if cls == "unknown" else source_obs,
            "source_age": "" if cls == "unknown" else age,
            "source_seed": "" if cls == "unknown" else (seed if source_seed is None else source_seed)}


# --- the arm with a memory ---------------------------------------------------------------------
REC = {
    5: [inc("hard", "cascade", correct=1, esc=1),
        inc("hard", "cascade", rec=0, fam=True, correct=1, cells={"correct_source_right": 1}),
        inc("plain", correct=1),
        inc("decoy", wrong=1, cells={"wrong_source_right": 1})],
    6: [inc("hard", "compound", wrong=1, cells={"wrong_source_wrong": 1}),
        inc("hard", "compound", fam=True, correct=1, cells={"correct_source_right": 1})],
    7: [inc("hard", "split_brain", correct=1, esc=1),
        inc("hard", "split_brain", rec=0, fam=True, correct=1, esc=1)],
}
RECALLS = [
    recall(5, 3, 1, "hard", True, "right"),
    recall(5, 6, 3, "decoy", False, "right"),
    recall(6, 2, 0, "hard", False, "wrong"),
    recall(6, 4, 1, "hard", True, "right", age=1, source_seed=5),
]
# --- the control: the same streams, no memory --------------------------------------------------
CTL = {
    5: [inc("hard", "cascade", correct=1, esc=1),
        inc("hard", "cascade", rec=0, fam=True, correct=1, esc=1),
        inc("plain", wrong=1),
        inc("decoy", wrong=1)],
    6: [inc("hard", "compound", wrong=1, correct=1, esc=1),
        inc("hard", "compound", fam=True, correct=1, esc=1)],
    7: REC[7],
}


def write_memory_arm(path: Path, streams: dict, recalls: list, noticer="record_family", ns=0, legacy=False):
    seqs = [{"seed": s, "incidents": [i["row"] for i in incs], "noticer_ns": ns,
             "recall_declarations": sum(1 for r in recalls if r["seed"] == s)}
            for s, incs in sorted(streams.items())]
    write_stream_arm(path, seqs, legacy=legacy)
    if legacy:
        return
    flag = lambda x: "true" if x else "false"  # noqa: E731
    with open(path / "memory_incidents.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, L.STREAM_MEMORY_INCIDENTS_COLUMNS, lineterminator="\n")
        w.writeheader()
        for s, incs in sorted(streams.items()):
            for k, i in enumerate(incs):
                r = i["row"]
                cells = {c: i["cells"].get(c, 0) for c in CELLS}
                un_c = r["correct_declarations"] > 0 and r["escalations"] == 0
                un_w = r["wrong_declarations"] > 0 and r["escalations"] == 0
                stale = (cells["wrong_source_right"] + cells["wrong_source_wrong"] + cells["wrong_source_unknown"]) > 0 \
                    and r["escalations"] == 0
                w.writerow({"run_id": "r.arm", "arm_role": "comparison", "seed": s, "incident": k, "tier": r["tier"],
                            "family": r["family"], "recurrence_of": "" if i["recurrence_of"] is None else i["recurrence_of"],
                            "same_family_earlier": flag(i["same_family_earlier"]),
                            "correct_declarations": r["correct_declarations"], "wrong_declarations": r["wrong_declarations"],
                            "escalations": r["escalations"], "unasked_correct": flag(un_c), "unasked_wrong": flag(un_w),
                            "stale_wrong": flag(stale), "recalls": sum(cells.values()),
                            **{f"recalls_{c}": v for c, v in cells.items()}})
    mem = pd.read_csv(path / "memory_incidents.csv", dtype={"recurrence_of": "Int64"})
    with open(path / "memory.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, L.STREAM_MEMORY_COLUMNS, lineterminator="\n")
        w.writeheader()
        for s, incs in sorted(streams.items()):
            m = mem[mem["seed"] == s]
            row = {c: 0 for c in L.STREAM_MEMORY_COLUMNS}
            row.update({"run_id": "r.arm", "arm_role": "comparison", "seed": s, "noticer": noticer})
            for c in CELLS:
                row[f"recalls_{c}"] = int(m[f"recalls_{c}"].sum())
            for t in ("plain", "hard", "decoy"):
                mt = m[m["tier"] == t]
                row[f"unasked_correct_{t}"] = int(mt["unasked_correct"].sum())
                row[f"unasked_wrong_{t}"] = int(mt["unasked_wrong"].sum())
                row[f"stale_wrong_{t}"] = int(mt["stale_wrong"].sum())
            # recalls by anchor: from the recall rows
            for r in recalls:
                if r["seed"] != s:
                    continue
                t = r["tier"] or "background"
                row[f"recalls_{t}_{'correct' if r['correct'] else 'wrong'}"] += 1
            row["recalls"] = sum(row[f"recalls_{c}"] for c in CELLS)
            hard = m[m["tier"] == "hard"]
            isrec, elsew = hard["recurrence_of"].notna(), hard["recurrence_of"].isna() & hard["same_family_earlier"]
            for name, sel in (("hard_recurrences", isrec), ("hard_elsewhere", elsew),
                              ("hard_reachable", isrec | hard["same_family_earlier"])):
                row[name] = int(sel.sum())
                row[f"{name}_unasked_correct"] = int((sel & hard["unasked_correct"]).sum())
            w.writerow(row)
    with open(path / "recalls.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, L.STREAM_RECALLS_COLUMNS, lineterminator="\n")
        w.writeheader()
        for r in recalls:
            w.writerow({"run_id": "r.arm", "arm_role": "comparison", "seed": r["seed"], "step": r["step"], "at_ns": 1,
                        "anchor": 2, "declared": "none", "source_obs": r["source_obs"], "stored": "none" if r["source_obs"] != "" else "",
                        "incident": r["incident"], "tier": r["tier"], "correct": flag(r["correct"]),
                        "source_class": r["class"], "source_incident": "", "source_age": r["source_age"],
                        "source_seed": r["source_seed"]})


@pytest.fixture()
def arms(tmp_path):
    write_memory_arm(tmp_path / "rec", REC, RECALLS)
    write_memory_arm(tmp_path / "ctl", CTL, [], noticer="reanchor")
    return M.load_arms(tmp_path)


# ---- the loader ---------------------------------------------------------------------------------


def test_the_memory_files_load_beside_the_older_ones(arms):
    rec = arms["rec"]
    assert rec.memory is not None and rec.memory_incidents is not None and rec.recalls is not None
    assert len(rec.memory) == 3 and len(rec.memory_incidents) == 8 and len(rec.recalls) == 4
    assert list(rec.results["recall_declarations"]) == [2, 2, 0]
    assert list(arms["ctl"].results["recall_declarations"]) == [0, 0, 0]
    assert rec.memory_incidents["recurrence_of"].isna().sum() == 6


def test_a_run_made_before_e1_loads_with_missing_values_not_zeros(tmp_path):
    write_memory_arm(tmp_path / "old", REC, [], legacy=True)
    arm = L.load_stream_arm(tmp_path / "old")
    assert arm.memory is None and arm.recalls is None
    assert arm.results["recall_declarations"].isna().all() and arm.results["noticer_ns"].isna().all()
    assert str(arm.results["recall_declarations"].dtype) == "Int64"


def test_the_loader_refuses_files_that_disagree(tmp_path):
    write_memory_arm(tmp_path / "rec", REC, RECALLS)
    p = tmp_path / "rec" / "recalls.csv"
    df = pd.read_csv(p, dtype=str, keep_default_na=False)
    df.iloc[0, df.columns.get_loc("source_class")] = "mostly"
    df.to_csv(p, index=False)
    with pytest.raises(L.LoadError, match="source_class"):
        L.load_stream_arm(tmp_path / "rec")
    write_memory_arm(tmp_path / "rec2", REC, RECALLS)
    p = tmp_path / "rec2" / "memory.csv"
    df = pd.read_csv(p)
    df.loc[0, "recalls_hard_correct"] += 1
    df.to_csv(p, index=False)
    with pytest.raises(L.LoadError, match="recalls by anchor|six cells"):
        L.load_stream_arm(tmp_path / "rec2")
    write_memory_arm(tmp_path / "rec3", REC, RECALLS)
    (tmp_path / "rec3" / "recalls.csv").unlink()
    with pytest.raises(L.LoadError, match="come together"):
        L.load_stream_arm(tmp_path / "rec3")


def test_the_source_columns_are_checked_against_each_other(tmp_path):
    bad = [dict(r) for r in RECALLS]
    bad[3]["source_age"] = 0  # age 0 but the source seed is another stream's
    write_memory_arm(tmp_path / "rec", REC, bad)
    with pytest.raises(L.LoadError, match="source_age is 0 exactly"):
        L.load_stream_arm(tmp_path / "rec")


def test_the_loaders_columns_are_the_harness_headers():
    root = Path(__file__).resolve().parents[2] / "crates/gordian-run/src/stream/results.rs"
    if not root.is_file():
        pytest.skip("harness source not present")
    import re
    src = root.read_text()
    for name, cols in (("MEMORY_HEADER", L.STREAM_MEMORY_COLUMNS), ("MEMORY_INCIDENTS_HEADER", L.STREAM_MEMORY_INCIDENTS_COLUMNS),
                       ("RECALLS_HEADER", L.STREAM_RECALLS_COLUMNS), ("RESULTS_HEADER", L.STREAM_RESULTS_COLUMNS)):
        m = re.search(rf'{name}: &str = "([^"]+)"', src)
        assert m and m.group(1).split(",") == cols, name


# ---- the measures ------------------------------------------------------------------------------


def tab(arms, name):
    return M.streamtab(arms[name])


BOOT = M.Boot(resamples=400, seed=3, chunk=100)


def test_the_populations_and_their_unasked_correct_shares(arms):
    t = tab(arms, "rec")
    # recurrences: (5,1) unasked correct; (7,1) asked: 1 of 2.  elsewhere: (6,1): 1 of 1.
    # reachable: (5,1) (6,1) (7,1): 2 of 3.  hard incidents: 6 in all, (5,1) and (6,1) unasked correct.
    got = {m: M.pooled(t, m, BOOT) for m in ("unasked_correct_hard_recurrences", "unasked_correct_hard_elsewhere",
                                             "unasked_correct_hard_reachable", "unasked_correct_hard")}
    assert [(g["num"], g["den"]) for g in got.values()] == [(1, 2), (1, 1), (2, 3), (2, 6)]
    assert got["unasked_correct_hard_recurrences"]["point"] == 0.5
    assert all(g["lower"] <= g["point"] <= g["upper"] for g in got.values())


def test_the_two_kinds_of_wrong_recall_are_counted_apart(arms):
    t = tab(arms, "rec")
    coll = M.pooled(t, "collision_share", BOOT)
    # recalls with a right source: stream 5: two, stream 6: one; one of the three is wrong.
    assert (coll["num"], coll["den"]) == (1, 3)
    assert M.pooled(t, "collisions_per_stream", BOOT)["point"] == pytest.approx(1 / 3)
    inh = M.pooled(t, "inherited_share", BOOT)
    assert (inh["num"], inh["den"]) == (1, 1)
    assert M.pooled(t, "inherited_per_stream", BOOT)["point"] == pytest.approx(1 / 3)
    assert M.pooled(t, "recalls_per_stream", BOOT)["point"] == pytest.approx(4 / 3)


def test_calls_per_correct_decision_and_cost_include_the_noticer(tmp_path):
    write_memory_arm(tmp_path / "rec", REC, RECALLS, ns=1_000)
    write_memory_arm(tmp_path / "ctl", CTL, [], noticer="reanchor")
    arms = M.load_arms(tmp_path)
    t = tab(arms, "rec")
    # calls: stream 5: 1 (incident 0), stream 6: 0, stream 7: 2 -> 3. correct plain + hard by deadline:
    # stream 5: 2 hard + 1 plain = 3; stream 6: 1 hard (the other has no correct declaration) = 1;
    # stream 7: 2 -> 6 correct decisions.
    ratio = M.pooled(t, "calls_per_correct", BOOT)
    assert (ratio["num"], ratio["den"]) == (3, 6)
    # the cost is total_cost_ns + noticer_ns (1,000 per stream), in seconds per stream.
    base = tab(arms, "ctl")
    extra = (t["cost_ns"] - t["total_cost_ns"]).tolist()
    assert extra == [1000.0, 1000.0, 1000.0]
    assert (base["cost_ns"] == base["total_cost_ns"]).all()


def test_the_paired_excess_over_the_memoryless_arm_cancels_the_cheap_rungs_own_errors(arms):
    rec, ctl = tab(arms, "rec"), tab(arms, "ctl")
    # unasked wrong, per tier. rec: plain 0, hard 1 (6,0), decoy 1 (5,3). ctl: plain 1 (5,2), hard 0
    # ((6,0) was asked), decoy 1 (5,3). Over 3 streams: the excess is -1/3, +1/3, 0, and 0 in all.
    ex = M.paired_excess(rec, ctl, BOOT).set_index("tier")
    assert ex.loc["plain", "excess"] == pytest.approx(-1 / 3)
    assert ex.loc["hard", "excess"] == pytest.approx(1 / 3)
    assert ex.loc["decoy", "excess"] == pytest.approx(0)
    assert ex.loc["all", "excess"] == pytest.approx(0)
    assert ex.loc["all", "arm_value"] == pytest.approx(2 / 3) and ex.loc["all", "control"] == pytest.approx(2 / 3)
    assert (ex["lower"] <= ex["excess"]).all() and (ex["excess"] <= ex["upper"]).all()


def test_a_paired_difference_needs_the_same_streams(arms):
    rec, ctl = tab(arms, "rec"), tab(arms, "ctl")
    d = M.paired(rec, ctl, "unasked_correct_hard", BOOT)
    # rec: 2 of 6; ctl: 0 of 6: the difference is 1/3.
    assert d["point"] == pytest.approx(2 / 6)
    with pytest.raises(ValueError, match="same streams"):
        M.paired(rec.iloc[:2], ctl, "unasked_correct_hard", BOOT)


def test_the_curves(arms):
    t = tab(arms, "rec")
    c = M.curve_across_streams(t, "unasked_correct_hard")
    # hard incidents seen: 2, 4, 6; unasked correct: 1, 2, 2.
    assert c["cum_seen"].tolist() == [2, 4, 6] and c["cum_event"].tolist() == [1, 2, 2]
    assert c["share"].tolist() == pytest.approx([0.5, 0.5, 1 / 3])
    s = M.slope_across_streams(t, BOOT, per=1.0)
    x = np.array([1, 2, 3.0])
    y = np.array([0.5, 0.5, 1 / 3])
    assert s["point"] == pytest.approx(np.polyfit(x, y, 1)[0])
    within = M.curve_within_streams(arms["rec"].memory_incidents, "unasked_correct", min_streams=2)
    # the n-th hard incident of a stream: n = 1: three streams reach it, cumulative 0, 0 (6: wrong), 0
    # (7: asked) -> mean 0; n = 2: three streams, cumulative 1, 1, 0 -> 2/3.
    assert within["n"].tolist() == [1, 2] and within["streams"].tolist() == [3, 3]
    assert within["mean_cumulative"].tolist() == pytest.approx([0.0, 2 / 3])
