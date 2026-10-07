"""Tests of `gordian_analysis.criterion` (work item V1) on small hand-written fixtures.

Every expected number is counted by hand in the comments. The fixtures are tiny run directories
written here, not the harness's writer, so that the measures are checked against the files'
documented columns and not against the code that normally reads them; one test compares them with
`stream.notice_points` on the shared fixtures as a second, independent implementation.
"""

from __future__ import annotations

import copy
import json
import math
import sys
from pathlib import Path

import numpy as np
import pandas as pd
import pytest

from gordian_analysis import criterion as C

ROOT = Path(__file__).resolve().parents[2]


# ---------------------------------------------------------------------------------------------
# Fixture writers
# ---------------------------------------------------------------------------------------------


def write_arm(path: Path, streams: dict[int, dict]) -> None:
    """One arm directory. `streams` maps seed -> {"incidents": [(tier, family, noticed, anchor)],
    "bg": notices on background, "cost": total cost}. notices.csv, results.csv and
    notice_incidents.csv are written with the columns the criterion reads."""
    path.mkdir(parents=True, exist_ok=True)
    inc_rows, not_rows, res_rows = [], [], []
    for seed, s in sorted(streams.items()):
        for k, (tier, family, noticed, anchor) in enumerate(s["incidents"]):
            inc_rows.append({"seed": seed, "incident": k, "tier": tier, "family": family,
                             "noticed": str(noticed).lower(), "anchor_correct": str(anchor).lower()})
        on_inc = sum(1 for i in s["incidents"] if i[2])
        not_rows.append({"seed": seed, "notices": on_inc + s["bg"], "notices_on_background": s["bg"]})
        res_rows.append({"seed": seed, "total_cost_ns": s.get("cost", 0)})
    pd.DataFrame(inc_rows).to_csv(path / "notice_incidents.csv", index=False)
    pd.DataFrame(not_rows).to_csv(path / "notices.csv", index=False)
    pd.DataFrame(res_rows).to_csv(path / "results.csv", index=False)


H = ("hard", "cascade")
L = ("hard", "slow_leak")
P = ("plain", "")

# Four streams, seeds 10..13. Arm A, then arm B (every stream differs by a known amount).
#                    hard non-leak (anchor)        leak            bg
# A: 10   H H(1 of 3 ... see below)
STREAMS_A = {
    10: {"incidents": [H + (True, True), H + (True, False), H + (False, False), L + (True, False), P + (True, True)], "bg": 2, "cost": 100},
    11: {"incidents": [H + (True, True), P + (True, True)], "bg": 4, "cost": 200},
    12: {"incidents": [L + (False, False), P + (False, False)], "bg": 0, "cost": 300},
    13: {"incidents": [H + (True, True), H + (True, True), L + (True, True)], "bg": 6, "cost": 400},
}
# hard non-leak incidents: stream 10: 3 (anchor-correct 1), 11: 1 (1), 12: 0 (0), 13: 2 (2)
#   pooled anchor-correct = 4/6; mean of per-stream ratios would be (1/3 + 1 + 2/2)/3 = 7/9: different.
# leak incidents: 10: 1 noticed 1 anchor 0; 11: 0; 12: 1 noticed 0; 13: 1 noticed 1 anchor 1.
#   leak noticed = 2/3.
# background per stream: (2 + 4 + 0 + 6) / 4 = 3.0
STREAMS_B = {
    10: {"incidents": [H + (True, True), H + (True, True), H + (True, True), L + (True, True), P + (True, True)], "bg": 1, "cost": 100},
    11: {"incidents": [H + (True, True), P + (True, True)], "bg": 1, "cost": 100},
    12: {"incidents": [L + (True, False), P + (False, False)], "bg": 1, "cost": 100},
    13: {"incidents": [H + (True, False), H + (True, False), L + (False, False)], "bg": 1, "cost": 100},
}
# B anchor-correct: 3/3, 1/1, 0, 0/2 -> 4/6 too (equal pooled) but a different distribution over streams.

BOOT = {"unit": "stream", "resamples": 400, "seed": 7, "percentiles": [5, 95], "interval": "lower_higher",
        "chunk": 100}

HARD_NL = [{"column": "tier", "op": "eq", "value": "hard"}, {"column": "family", "op": "ne", "value": "slow_leak"}]
LEAK = [{"column": "tier", "op": "eq", "value": "hard"}, {"column": "family", "op": "eq", "value": "slow_leak"}]

MEASURES = {
    "anchor": {"kind": "ratio",
               "num": {"file": "notice_incidents.csv", "agg": "sum", "columns": {"anchor_correct": 1}, "where": HARD_NL},
               "den": {"file": "notice_incidents.csv", "agg": "count", "where": HARD_NL}},
    "leak_noticed": {"kind": "ratio",
                     "num": {"file": "notice_incidents.csv", "agg": "sum", "columns": {"noticed": 1}, "where": LEAK},
                     "den": {"file": "notice_incidents.csv", "agg": "count", "where": LEAK}},
    "bg": {"kind": "ratio", "num": {"file": "notices.csv", "agg": "sum", "columns": {"notices_on_background": 1}},
           "den": {"per_stream": 1}},
    "on_incidents_share": {"kind": "ratio",
                           "num": {"file": "notices.csv", "agg": "sum",
                                   "columns": {"notices": 1, "notices_on_background": -1}},
                           "den": {"file": "notices.csv", "agg": "sum", "columns": {"notices": 1}}},
    "cost_s": {"kind": "ratio", "num": {"file": "results.csv", "agg": "sum", "columns": {"total_cost_ns": 1}},
               "den": {"per_stream": 1}, "scale": 1e-9},
}


def make_spec(clauses=None, verdict=None, **over) -> dict:
    spec = {
        "schema": C.SCHEMA, "id": "t", "title": "test", "source": "test",
        "runs": {"r": {"streams": {"first_seed": 10, "count": 4}}},
        "arms": {"a": {"run": "r", "dir": "A"}, "b": {"run": "r", "dir": "B"}},
        "bootstrap": dict(BOOT), "measures": copy.deepcopy(MEASURES),
        "clauses": clauses or [], "verdict": verdict or {"all": []},
    }
    spec.update(over)
    return spec


@pytest.fixture()
def run_dir(tmp_path: Path) -> Path:
    write_arm(tmp_path / "A", STREAMS_A)
    write_arm(tmp_path / "B", STREAMS_B)
    (tmp_path / "manifest.json").write_text(json.dumps({"run_id": "fixture", "source_revision": "abc"}))
    return tmp_path


def runset(spec, run_dir) -> C.RunSet:
    return C.RunSet(spec, {"r": run_dir})


# ---------------------------------------------------------------------------------------------
# Measures: ratio of sums, filters, windows
# ---------------------------------------------------------------------------------------------


def test_share_is_a_ratio_of_sums_not_a_mean_of_ratios(run_dir):
    ev = C.Evaluator(make_spec(), runset(make_spec(), run_dir))
    v = ev.value("a", "anchor")
    assert v["point"] == pytest.approx(4 / 6)
    assert v["point"] != pytest.approx(7 / 9)  # the mean of per-stream ratios, with stream 12 left out
    assert v["streams"] == 4


def test_tier_and_family_filters_split_leak_from_hard_non_leak(run_dir):
    spec = make_spec()
    ev = C.Evaluator(spec, runset(spec, run_dir))
    assert ev.value("a", "leak_noticed")["point"] == pytest.approx(2 / 3)
    assert ev.value("b", "leak_noticed")["point"] == pytest.approx(2 / 3)
    # the per-stream vectors, by hand: hard non-leak numerators and denominators of arm A
    num, den, _ = ev.arms.nd("a", "anchor")
    assert num.tolist() == [1, 1, 0, 2] and den.tolist() == [3, 1, 0, 2]
    num, den, _ = ev.arms.nd("a", "leak_noticed")
    assert num.tolist() == [1, 0, 0, 1] and den.tolist() == [1, 0, 1, 1]


def test_mean_per_stream_linear_combination_and_scale(run_dir):
    spec = make_spec()
    ev = C.Evaluator(spec, runset(spec, run_dir))
    assert ev.value("a", "bg")["point"] == pytest.approx(3.0)  # (2 + 4 + 0 + 6) / 4
    assert ev.value("b", "bg")["point"] == pytest.approx(1.0)
    # notices - background over notices: A: on incidents 4+2+0+3 ... counted by hand below
    # stream 10: 4 noticed incidents (H, H, L, P) + bg 2 -> notices 6; 11: 2 + 4 = 6; 12: 0 + 0; 13: 3 + 6 = 9
    num, den, _ = ev.arms.nd("a", "on_incidents_share")
    assert num.tolist() == [4, 2, 0, 3] and den.tolist() == [6, 6, 0, 9]
    assert ev.value("a", "on_incidents_share")["point"] == pytest.approx(9 / 21)
    assert ev.value("a", "cost_s")["point"] == pytest.approx(250 * 1e-9)


def test_empty_family_never_equals_and_always_differs(run_dir):
    # plain incidents have an empty family: `ne slow_leak` keeps them, `eq slow_leak` drops them
    spec = make_spec()
    spec["measures"]["plain_only"] = {"kind": "ratio",
        "num": {"file": "notice_incidents.csv", "agg": "count",
                "where": [{"column": "family", "op": "ne", "value": "slow_leak"},
                          {"column": "tier", "op": "eq", "value": "plain"}]},
        "den": {"file": "notice_incidents.csv", "agg": "count", "where": [{"column": "tier", "op": "eq", "value": "plain"}]}}
    ev = C.Evaluator(spec, runset(spec, run_dir))
    assert ev.value("a", "plain_only")["point"] == 1.0


def test_in_and_not_in(run_dir):
    spec = make_spec()
    for op, expect in (("in", 5), ("not_in", 7)):  # hard incidents: cascade 5 ... see below
        spec["measures"][op] = {"kind": "ratio",
            "num": {"file": "notice_incidents.csv", "agg": "count",
                    "where": [{"column": "family", "op": op, "value": ["cascade", "slow_leak"]}]},
            "den": {"per_stream": 1}}
        ev = C.Evaluator(spec, runset(spec, run_dir))
        n = ev.arms.nd("a", op)[0].sum()
        # arm A: cascade 6 (3+1+0+2), slow_leak 3 -> in = 9; incidents 12, so not_in = 3 (plain)
        assert n == (9 if op == "in" else 3)


def test_window_selects_first_last_and_slice_with_no_off_by_one():
    assert C.window_bounds(10, None) == (0, 10)
    assert C.window_bounds(10, {"first": 3}) == (0, 3)
    assert C.window_bounds(10, {"last": 3}) == (7, 10)
    assert C.window_bounds(10, {"slice": [2, 5]}) == (2, 5)
    assert C.window_bounds(10, {"last": 10}) == (0, 10)
    with pytest.raises(C.SpecError):
        C.window_bounds(10, {"last": 11})
    with pytest.raises(C.SpecError):
        C.window_bounds(10, {"slice": [5, 5]})


def test_windowed_value_uses_only_the_window(run_dir):
    spec = make_spec()
    ev = C.Evaluator(spec, runset(spec, run_dir))
    # last 2 streams (12, 13): hard non-leak 0/0 and 2/2 -> 2/2
    assert ev.value("a", "anchor", {"last": 2})["point"] == pytest.approx(1.0)
    assert ev.value("a", "anchor", {"first": 2})["point"] == pytest.approx(2 / 4)
    assert ev.value("a", "anchor", {"last": 2})["streams"] == 2
    assert ev.value("a", "bg", {"last": 2})["point"] == pytest.approx(3.0)  # (0 + 6) / 2
    assert ev.value("a", "bg", {"first": 1})["point"] == pytest.approx(2.0)


# ---------------------------------------------------------------------------------------------
# The bootstrap
# ---------------------------------------------------------------------------------------------


def test_counts_are_multinomial_and_chunk_independent():
    w = C.draw_counts(5, 7, 1000, 100)
    assert w.shape == (1000, 7) and (w.sum(axis=1) == 7).all() and (w >= 0).all()
    assert np.array_equal(w, C.draw_counts(5, 7, 1000, 500))
    assert np.array_equal(w, C.draw_counts(5, 7, 1000, 1000))
    assert not np.array_equal(w, C.draw_counts(6, 7, 1000, 100))


def test_the_lab_drew_counts_this_way():
    # the lab scripts: rng.multinomial(n, np.full(n, 1/n), size=k) in chunks of 500 from one generator
    rng = np.random.default_rng(9950)
    first = rng.multinomial(200, np.full(200, 1.0 / 200), size=500).astype(float)
    assert np.array_equal(C.draw_counts(9950, 200, 1500, 500)[:500], first)


def test_interval_methods_and_percentiles():
    x = np.arange(100, dtype=float)  # 0..99
    assert C.interval(x, [5, 95], "lower_higher") == (4.0, 95.0)  # floor(.05*99)=4, ceil(.95*99)=95
    lo, hi = C.interval(x, [5, 95], "linear")
    assert lo == pytest.approx(4.95) and hi == pytest.approx(94.05)
    assert C.interval(x, [10, 90], "lower_higher") == (9.0, 90.0)  # a different percentile is a different answer
    nan = np.array([np.nan, 1.0, 2.0, 3.0])
    assert C.interval(nan, [0, 100], "linear") == (1.0, 3.0)  # undefined resamples are dropped
    assert all(math.isnan(v) for v in C.interval(np.array([np.nan]), [5, 95], "linear"))


def test_evaluation_is_deterministic_and_seed_dependent(run_dir):
    spec = make_spec()
    a = C.Evaluator(spec, runset(spec, run_dir)).paired("a", "b", "anchor")
    b = C.Evaluator(spec, runset(spec, run_dir)).paired("a", "b", "anchor")
    assert a == b
    other = make_spec(bootstrap={**BOOT, "seed": 8})
    c = C.Evaluator(other, runset(other, run_dir)).paired("a", "b", "anchor")
    assert (c["lower"], c["upper"]) != (a["lower"], a["upper"])
    assert c["point"] == a["point"]


def test_paired_difference_is_resampled_as_a_pair(run_dir, tmp_path):
    # Arms whose per-stream cost differs from each other by exactly 1 on every stream: the difference
    # of the means is 1 on every resample, so a paired interval is the point [1, 1]. Resampling the
    # arms independently would give an interval of positive width.
    sa = {s: {"incidents": [], "bg": 0, "cost": 10 * (s - 9)} for s in (10, 11, 12, 13)}
    sb = {s: {"incidents": [], "bg": 0, "cost": 10 * (s - 9) + 10**9} for s in (10, 11, 12, 13)}
    for name, st in (("A", sa), ("B", sb)):
        write_arm(tmp_path / name, st)
    spec = make_spec()
    ev = C.Evaluator(spec, runset(spec, tmp_path))
    d = ev.paired("b", "a", "cost_s")
    assert d["point"] == pytest.approx(1.0)
    assert d["lower"] == pytest.approx(1.0) and d["upper"] == pytest.approx(1.0)
    # each arm alone has a wide interval
    v = ev.value("a", "cost_s")
    assert v["upper"] - v["lower"] > 0


def test_paired_arms_must_be_on_the_same_streams(run_dir, tmp_path):
    write_arm(tmp_path / "A", STREAMS_A)
    write_arm(tmp_path / "B", {s + 1: v for s, v in STREAMS_B.items()})  # seeds 11..14
    spec = make_spec()
    ev = C.Evaluator(spec, runset(spec, tmp_path))
    with pytest.raises(C.SpecError, match="streams"):
        ev.paired("a", "b", "anchor")


def test_dropping_a_cluster_changes_the_interval_and_the_point(run_dir):
    spec = make_spec()
    ev = C.Evaluator(spec, runset(spec, run_dir))
    full = ev.value("a", "bg")
    assert full["streams"] == 4
    short = ev.value("a", "bg", {"first": 3})  # stream 13 (bg 6) dropped
    assert short["point"] == pytest.approx(2.0) and short["point"] != full["point"]


# ---------------------------------------------------------------------------------------------
# Curve slope
# ---------------------------------------------------------------------------------------------


def test_curve_slope_matches_polyfit_and_leaves_out_points_before_the_first_denominator():
    num = np.array([0.0, 1.0, 1.0, 0.0, 2.0, 1.0])
    den = np.array([0.0, 2.0, 1.0, 1.0, 2.0, 2.0])  # stream 1 has no incident yet
    cn, cd = np.cumsum(num), np.cumsum(den)
    ok = cd > 0
    x = np.arange(1, 7)[ok]
    expect = np.polyfit(x, (cn / np.where(ok, cd, 1))[ok], 1)[0]
    got = C.curve_slopes(num, den, np.ones((1, 6)))[0]
    assert got == pytest.approx(expect)
    # a resample of weights equal to the stream counts, in place
    w = np.array([[2.0, 0, 1, 1, 1, 1]])
    cn, cd = np.cumsum(w[0] * num), np.cumsum(w[0] * den)
    ok = cd > 0
    expect = np.polyfit(np.arange(1, 7)[ok], (cn / np.where(ok, cd, 1))[ok], 1)[0]
    assert C.curve_slopes(num, den, w)[0] == pytest.approx(expect)


def test_curve_slope_nan_with_fewer_than_two_points():
    assert math.isnan(C.curve_slopes(np.array([0.0, 1.0]), np.array([0.0, 1.0]), np.ones((1, 2)))[0])


def test_curve_slope_agrees_with_w1_sample_efficiency(run_dir):
    from gordian_analysis.measures import sample_efficiency

    spec = make_spec()
    ev = C.Evaluator(spec, runset(spec, run_dir))
    est = ev.curve_slope("a", "anchor", 4, 100.0)
    num, den, _ = ev.arms.nd("a", "anchor")
    cur = sample_efficiency(pd.DataFrame({"seed": [10, 11, 12, 13], "incidents_hard": den.astype(int),
                                          "correct_hard": num.astype(int)}), tier="hard").curve
    cur = cur.dropna(subset=["efficiency"])
    assert est["point"] == pytest.approx(np.polyfit(cur["stream"], cur["efficiency"], 1)[0] * 100)


def test_curve_slope_difference_is_the_difference_of_the_slopes(run_dir):
    spec = make_spec()
    ev = C.Evaluator(spec, runset(spec, run_dir))
    a = ev.curve_slope("a", "anchor", 4, 1.0)["point"]
    b = ev.curve_slope("b", "anchor", 4, 1.0)["point"]
    assert ev.curve_slope("a", "anchor", 4, 1.0, minus="b")["point"] == pytest.approx(a - b)


# ---------------------------------------------------------------------------------------------
# Clauses, tests, the verdict tree
# ---------------------------------------------------------------------------------------------


def test_tests_are_strict_where_the_text_is_and_nan_fails(run_dir):
    spec = make_spec(clauses=[
        {"id": "ge", "kind": "value", "arm": "a", "measure": "bg", "tests": [{"on": "point", "op": ">=", "value": 3.0}]},
        {"id": "gt", "kind": "value", "arm": "a", "measure": "bg", "tests": [{"on": "point", "op": ">", "value": 3.0}]},
        {"id": "le", "kind": "value", "arm": "a", "measure": "bg", "tests": [{"on": "point", "op": "<=", "value": 3.0}]},
        {"id": "lt", "kind": "value", "arm": "a", "measure": "bg", "tests": [{"on": "point", "op": "<", "value": 3.0}]},
        {"id": "both", "kind": "value", "arm": "a", "measure": "bg",
         "tests": [{"on": "point", "op": ">=", "value": 3.0}, {"on": "lower", "op": ">", "value": 100.0}]},
    ], verdict={"all": ["ge", "le"]})
    out = C.evaluate(spec, {"r": run_dir})
    got = {c["id"]: c["pass"] for c in out["clauses"]}
    assert got == {"ge": True, "gt": False, "le": True, "lt": False, "both": False}
    assert out["verdict"]["holds"] is True
    nan_spec = make_spec(clauses=[{"id": "n", "kind": "value", "arm": "a", "measure": "leak_noticed",
                                   "window": {"slice": [1, 2]},  # stream 11 has no leak: 0/0
                                   "tests": [{"on": "point", "op": ">=", "value": -1.0}]}],
                         verdict={"all": ["n"]})
    out = C.evaluate(nan_spec, {"r": run_dir})
    assert out["clauses"][0]["point"] is None and out["clauses"][0]["pass"] is False
    assert out["verdict"]["holds"] is False


def test_verdict_tree_all_any_and_named_nodes(run_dir):
    def v(cid, ok):
        return {"id": cid, "kind": "value", "arm": "a", "measure": "bg",
                "tests": [{"on": "point", "op": ">=" if ok else "<", "value": 3.0}]}
    spec = make_spec(clauses=[v("t1", True), v("t2", True), v("f1", False)],
                     verdict={"name": "top", "any": [{"name": "left", "all": ["t1", "f1"]},
                                                     {"name": "right", "all": ["t1", "t2"]}]},
                     observations={"obs": {"all": ["f1"]}})
    out = C.evaluate(spec, {"r": run_dir})
    assert out["verdict"]["holds"] is True
    assert out["verdict"]["nodes"] == {"left": False, "right": True, "top": True}
    assert out["verdict"]["observations"] == {"obs": False}
    assert {c["id"]: c["in_verdict"] for c in out["clauses"]} == {"t1": True, "t2": True, "f1": True}


def test_clause_in_the_tree_needs_tests_and_unknown_ids_are_refused(run_dir):
    bad = make_spec(clauses=[{"id": "x", "kind": "value", "arm": "a", "measure": "bg"}], verdict={"all": ["x"]})
    with pytest.raises(C.SpecError, match="no tests"):
        C.validate_spec(bad)
    bad = make_spec(clauses=[], verdict={"all": ["nope"]})
    with pytest.raises(C.SpecError, match="unknown clause"):
        C.validate_spec(bad)


def test_validation_refuses_what_it_would_have_to_guess():
    base = make_spec()
    for edit, msg in (
        (lambda s: s.update(schema="x"), "schema"),
        (lambda s: s["bootstrap"].update(interval="nearest"), "interval"),
        (lambda s: s["bootstrap"].update(seed=None), "seed"),
        (lambda s: s["bootstrap"].update(percentiles=[95, 5]), "percentiles"),
        (lambda s: s["bootstrap"].update(unit="incident"), "unit"),
        (lambda s: s["arms"]["a"].update(run="zz"), "arms.a"),
    ):
        s = copy.deepcopy(base)
        edit(s)
        with pytest.raises(C.SpecError, match=msg):
            C.validate_spec(s)


def test_spec_hash_ignores_key_order_and_sees_values():
    a = {"x": 1, "y": [1, 2]}
    assert C.spec_hash(a) == C.spec_hash({"y": [1, 2], "x": 1})
    assert C.spec_hash(a) != C.spec_hash({"x": 2, "y": [1, 2]})


# ---------------------------------------------------------------------------------------------
# Identity, input checks
# ---------------------------------------------------------------------------------------------


def test_identity_clause_ignores_named_columns_only(run_dir, tmp_path):
    other = tmp_path / "other"
    write_arm(other / "A", STREAMS_A)
    write_arm(other / "B", STREAMS_B)
    spec = make_spec(
        runs={"r": {"streams": {"first_seed": 10, "count": 4}}, "s": {"streams": {"first_seed": 10, "count": 4}}},
        arms={"a": {"run": "r", "dir": "A"}, "a2": {"run": "s", "dir": "A"}, "b2": {"run": "s", "dir": "B"}},
        clauses=[{"id": "same", "kind": "identity", "a": "a", "b": "a2", "files": ["notice_incidents.csv", "notices.csv"]},
                 {"id": "diff", "kind": "identity", "a": "a", "b": "b2", "files": ["notices.csv"]},
                 {"id": "diff_ignored", "kind": "identity", "a": "a", "b": "b2", "files": ["notices.csv"],
                  "ignore_columns": ["notices", "notices_on_background"]}],
        verdict={"all": ["same"]})
    out = C.evaluate(spec, {"r": run_dir, "s": other})
    got = {c["id"]: c["pass"] for c in out["clauses"]}
    assert got == {"same": True, "diff": False, "diff_ignored": True}


def test_wrong_streams_are_refused(run_dir):
    spec = make_spec(runs={"r": {"streams": {"first_seed": 10, "count": 5}}})
    with pytest.raises(C.SpecError, match="specification declares"):
        C.Evaluator(spec, runset(spec, run_dir)).value("a", "anchor")
    spec = make_spec(runs={"r": {"streams": {"first_seed": 11, "count": 4}}})
    with pytest.raises(C.SpecError, match="specification declares"):
        C.Evaluator(spec, runset(spec, run_dir)).value("a", "anchor")


def test_a_per_stream_file_missing_a_stream_is_refused(run_dir):
    df = pd.read_csv(run_dir / "A" / "notices.csv")
    df[df["seed"] != 12].to_csv(run_dir / "A" / "notices.csv", index=False)
    spec = make_spec()
    with pytest.raises(C.SpecError, match="one row per seed"):
        C.Evaluator(spec, runset(spec, run_dir)).value("a", "bg")


def test_seeds_outside_the_declared_streams_are_refused(run_dir):
    df = pd.read_csv(run_dir / "A" / "notice_incidents.csv")
    extra = df.iloc[[0]].copy()
    extra["seed"] = 99
    pd.concat([df, extra]).to_csv(run_dir / "A" / "notice_incidents.csv", index=False)
    spec = make_spec()
    with pytest.raises(C.SpecError, match="outside"):
        C.Evaluator(spec, runset(spec, run_dir)).value("a", "anchor")


def test_missing_role_extra_role_and_missing_column(run_dir):
    spec = make_spec()
    with pytest.raises(C.SpecError, match="no --run"):
        C.RunSet(spec, {})
    with pytest.raises(C.SpecError, match="does not declare"):
        C.RunSet(spec, {"r": run_dir, "z": run_dir})
    spec["measures"]["anchor"]["num"]["columns"] = {"no_such": 1}
    with pytest.raises(C.SpecError, match="no column"):
        C.Evaluator(spec, runset(spec, run_dir)).value("a", "anchor")


def test_identities_record_every_file_read_by_hash(run_dir):
    spec = make_spec(clauses=[{"id": "x", "kind": "value", "arm": "a", "measure": "anchor",
                               "tests": [{"on": "point", "op": ">=", "value": 0}]}], verdict={"all": ["x"]})
    out = C.evaluate(spec, {"r": run_dir})
    r = out["runs"]["r"]
    assert r["run_id"] == "fixture" and r["source_revision"] == "abc"
    assert set(r["files_read"]) == {"A/notice_incidents.csv", "A/results.csv"}
    assert all(len(h) == 64 for h in r["files_read"].values())
    assert out["spec"]["sha256"] == C.spec_hash(spec)


# ---------------------------------------------------------------------------------------------
# A second implementation: stream.notice_points on the shared fixtures
# ---------------------------------------------------------------------------------------------


def test_the_notice_measures_agree_with_stream_py_on_the_shared_fixtures(tmp_path):
    sys.path.insert(0, str(Path(__file__).parent))
    from notice_fixtures import noticing_streams, write_notice_files
    from stream_fixtures import write_stream_arm

    from gordian_analysis.stream import notice_points

    arm_dir = tmp_path / "arm"
    streams = noticing_streams()
    write_stream_arm(arm_dir, streams)
    write_notice_files(arm_dir, streams)
    from gordian_analysis.load import load_stream_arm

    pts = notice_points(load_stream_arm(arm_dir))
    spec = {
        "schema": C.SCHEMA, "id": "x", "title": "x", "source": "x",
        "runs": {"r": {"streams": {"first_seed": 1, "count": 3}}},
        "arms": {"a": {"run": "r", "dir": "arm"}},
        "bootstrap": dict(BOOT), "clauses": [], "verdict": {"all": []},
        "measures": {
            "hard_noticed_share": {"kind": "ratio",
                "num": {"file": "notice_incidents.csv", "agg": "sum", "columns": {"noticed": 1}, "where": HARD_NL},
                "den": {"file": "notice_incidents.csv", "agg": "count", "where": HARD_NL}},
            "leak_anchor_correct_share": {"kind": "ratio",
                "num": {"file": "notice_incidents.csv", "agg": "sum", "columns": {"anchor_correct": 1}, "where": LEAK},
                "den": {"file": "notice_incidents.csv", "agg": "count", "where": LEAK}},
            "plain_noticed_share": {"kind": "ratio",
                "num": {"file": "notice_incidents.csv", "agg": "sum", "columns": {"noticed": 1},
                        "where": [{"column": "tier", "op": "eq", "value": "plain"}]},
                "den": {"file": "notice_incidents.csv", "agg": "count",
                        "where": [{"column": "tier", "op": "eq", "value": "plain"}]}},
            "notices_on_background_per_stream": {"kind": "ratio",
                "num": {"file": "notices.csv", "agg": "sum", "columns": {"notices_on_background": 1}}, "den": {"per_stream": 1}},
            "notice_precision": {"kind": "ratio",
                "num": {"file": "notices.csv", "agg": "sum", "columns": {"notices": 1, "notices_on_background": -1}},
                "den": {"file": "notices.csv", "agg": "sum", "columns": {"notices": 1}}},
            "strict_precision": {"kind": "ratio",
                "num": {"file": "notices.csv", "agg": "sum", "columns": {"notices_anchor_site_correct": 1}},
                "den": {"file": "notices.csv", "agg": "sum", "columns": {"notices": 1}}},
            "notices_per_incident": {"kind": "ratio",
                "num": {"file": "notices.csv", "agg": "sum", "columns": {"notices": 1, "notices_on_background": -1}},
                "den": {"file": "notice_incidents.csv", "agg": "count"}},
        },
    }
    ev = C.Evaluator(spec, C.RunSet(spec, {"r": tmp_path}))
    for name in spec["measures"]:
        got = ev.value("a", name)["point"]
        want = pts[name]
        assert got == pytest.approx(want, nan_ok=True), name


# ---------------------------------------------------------------------------------------------
# The script
# ---------------------------------------------------------------------------------------------


def _script():
    import importlib.util

    spec = importlib.util.spec_from_file_location("criterion_script", ROOT / "scripts" / "criterion.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def test_script_writes_deterministic_files_and_exit_codes(run_dir, tmp_path):
    spec = make_spec(clauses=[
        {"id": "yes", "kind": "paired", "arm": "a", "minus": "b", "measure": "bg", "description": "d",
         "tests": [{"on": "point", "op": ">=", "value": 1.0}]},
        {"id": "no", "kind": "value", "arm": "a", "measure": "bg", "tests": [{"on": "point", "op": ">", "value": 99}]}],
        verdict={"name": "v", "all": ["yes"]}, reports=[
            {"id": "t", "kind": "table", "arms": ["a", "b"], "measures": ["bg", "anchor"]},
            {"id": "p", "kind": "paired_table", "pairs": [["a", "b"]], "measures": ["bg"]},
            {"id": "s", "kind": "slope_table", "arms": ["a"], "measure": "anchor", "firsts": [3, 4], "per": 100}])
    sp = tmp_path / "spec.json"
    sp.write_text(json.dumps(spec))
    script = _script()
    out1, out2 = tmp_path / "o1", tmp_path / "o2"
    assert script.main([str(sp), "--run", f"r={run_dir}", "--out", str(out1)]) == 0
    assert script.main([str(sp), "--run", f"r={run_dir}", "--out", str(out2)]) == 0
    assert (out1 / "verdict.json").read_bytes() == (out2 / "verdict.json").read_bytes()
    assert (out1 / "verdict.md").read_text().startswith("# test")
    v = json.loads((out1 / "verdict.json").read_text())
    assert v["verdict"]["holds"] is True and v["spec"]["sha256"] == C.spec_hash(spec)
    md = (out1 / "verdict.md").read_text()
    assert "`yes`" in md and "`no` (beside)" in md and "Report `t`" in md
    spec["verdict"] = {"all": ["no"]}
    sp.write_text(json.dumps(spec))
    assert script.main([str(sp), "--run", f"r={run_dir}", "--out", str(out1)]) == 1
    assert script.main([str(sp), "--run", f"x={run_dir}", "--out", str(out1)]) == 2
    assert script.main([str(sp), "--run", "bad", "--out", str(out1)]) == 2
