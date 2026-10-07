"""Tests of the schema constructs work item E1 added to `gordian_analysis.criterion`: a `not` node,
exclusive outcome categories, not-null filters, count-with-total clauses, a per-stream quantile
measure and a join of two arms by incident.

Every expected number is counted by hand in the comments, on small run directories written here
(not by the harness's writer). The quantile and the join are also checked against a second,
brute-force reading (`np.repeat` of the resampled streams; a pandas merge written in the test).
"""

from __future__ import annotations

import copy
from pathlib import Path

import numpy as np
import pandas as pd
import pytest

from gordian_analysis import criterion as C

BOOT = {"unit": "stream", "resamples": 300, "seed": 11, "percentiles": [5, 95], "interval": "lower_higher",
        "chunk": 100}

# Arm "mem" and arm "ctl" on four streams (seeds 20..23). One row per incident:
#   seed incident tier  recurrence_of  correct esc  unasked_correct unasked_wrong stale_wrong
MEM = [
    # stream 20: a hard recurrence unasked correct, a hard incident asked, a plain wrongly declared
    (20, 0, "hard", None, 1, 0, True, False, False),
    (20, 1, "hard", 0, 1, 1, False, False, False),
    (20, 2, "plain", None, 0, 0, False, True, True),
    # stream 21: two hard recurrences, one unasked correct, one wrong recall
    (21, 0, "hard", None, 1, 1, False, False, False),
    (21, 1, "hard", 0, 1, 0, True, False, False),
    (21, 2, "hard", 0, 0, 0, False, True, True),
    # stream 22: no hard incident, a decoy wrongly declared
    (22, 0, "plain", None, 1, 0, True, False, False),
    (22, 1, "decoy", None, 0, 0, False, True, True),
    # stream 23: one hard recurrence unasked correct
    (23, 0, "hard", None, 1, 1, False, False, False),
    (23, 1, "hard", 0, 1, 0, True, False, False),
]
CTL = [
    (20, 0, "hard", None, 1, 1, False, False, False),
    (20, 1, "hard", 0, 1, 1, False, False, False),
    (20, 2, "plain", None, 1, 0, True, False, False),
    (21, 0, "hard", None, 1, 1, False, False, False),
    (21, 1, "hard", 0, 1, 1, False, False, False),
    (21, 2, "hard", 0, 1, 1, False, False, False),
    (22, 0, "plain", None, 1, 0, True, False, False),
    (22, 1, "decoy", None, 0, 0, False, True, False),
    (23, 0, "hard", None, 1, 1, False, False, False),
    (23, 1, "hard", 0, 1, 1, False, False, False),
]
COLS = ["seed", "incident", "tier", "recurrence_of", "correct_declarations", "escalations",
        "unasked_correct", "unasked_wrong", "stale_wrong"]


def write_arm(path: Path, rows) -> None:
    path.mkdir(parents=True, exist_ok=True)
    df = pd.DataFrame(rows, columns=COLS)
    df.to_csv(path / "memory_incidents.csv", index=False)
    pd.DataFrame({"seed": sorted(df["seed"].unique()), "x": 0}).to_csv(path / "results.csv", index=False)


def hard_rec(extra=()):
    return [{"column": "tier", "op": "eq", "value": "hard"}, {"column": "recurrence_of", "op": "notnull"}, *extra]


def term(agg="count", where=None, columns=None, **kw):
    t = {"file": "memory_incidents.csv", "agg": agg}
    if where is not None:
        t["where"] = where
    if columns is not None:
        t["columns"] = columns
    t.update(kw)
    return t


MEASURES = {
    # hard recurrences unasked correct, over hard recurrences
    "rec_share": {"kind": "ratio", "num": term(where=hard_rec([{"column": "unasked_correct", "op": "eq", "value": True}])),
                  "den": term(where=hard_rec())},
    "hard_n": {"kind": "ratio", "num": term(where=[{"column": "tier", "op": "eq", "value": "hard"}]),
               "den": {"per_stream": 1}},
    # the unasked wrong declarations of the arm that the control did not make (a join by incident)
    "wrong_added": {"kind": "ratio",
                    "num": term(where=[{"column": "unasked_wrong", "op": "eq", "value": True},
                                       {"column": "unasked_wrong@ctl", "op": "eq", "value": False}],
                                join={"arm": "ctl"}),
                    "den": {"per_stream": 1}},
    "uw": {"kind": "ratio", "num": term("sum", columns={"unasked_wrong": 1}), "den": {"per_stream": 1}},
    "stale_per_stream_q90": {"kind": "quantile", "q": 0.9, "num": term("sum", columns={"stale_wrong": 1}),
                             "den": {"per_stream": 1}},
    "recshare_q50": {"kind": "quantile", "q": 0.5,
                     "num": term(where=hard_rec([{"column": "unasked_correct", "op": "eq", "value": True}])),
                     "den": term(where=hard_rec())},
}


def make_spec(clauses=None, verdict=None, **over) -> dict:
    spec = {
        "schema": C.SCHEMA, "id": "t", "title": "test", "source": "test",
        "runs": {"r": {"streams": {"first_seed": 20, "count": 4}}},
        "arms": {"mem": {"run": "r", "dir": "MEM"}, "ctl": {"run": "r", "dir": "CTL"}},
        "bootstrap": dict(BOOT), "measures": copy.deepcopy(MEASURES),
        "clauses": two_value_clauses() if clauses is None else clauses,
        "verdict": verdict or {"all": ["yes"]},
    }
    spec.update(over)
    return spec


@pytest.fixture()
def run_dir(tmp_path: Path) -> Path:
    write_arm(tmp_path / "MEM", MEM)
    write_arm(tmp_path / "CTL", CTL)
    return tmp_path


def evaluator(spec, run_dir) -> C.Evaluator:
    return C.Evaluator(spec, C.RunSet(spec, {"r": run_dir}))


# ---------------------------------------------------------------------------------------------
# Not-null filters
# ---------------------------------------------------------------------------------------------


def test_notnull_and_isnull_select_by_an_empty_cell(run_dir):
    spec = make_spec()
    runs = C.RunSet(spec, {"r": run_dir})
    notnull = {"file": "memory_incidents.csv", "agg": "count", "where": [{"column": "recurrence_of", "op": "notnull"}]}
    isnull = {"file": "memory_incidents.csv", "agg": "count", "where": [{"column": "recurrence_of", "op": "isnull"}]}
    # recurrence_of is set for incidents (20,1), (21,1), (21,2), (23,1): one, two, none, one per stream.
    assert C.term_vector(runs, "mem", notnull).tolist() == [1, 2, 0, 1]
    # and empty for the other six: two, one, two, one.
    assert C.term_vector(runs, "mem", isnull).tolist() == [2, 1, 2, 1]


def test_notnull_needs_no_value_and_the_other_ops_still_do():
    C.validate_spec(make_spec())
    bad = make_spec()
    bad["measures"]["hard_n"]["num"]["where"] = [{"column": "tier", "op": "eq"}]
    with pytest.raises(C.SpecError, match="where item"):
        C.validate_spec(bad)


# ---------------------------------------------------------------------------------------------
# Count-with-total clauses
# ---------------------------------------------------------------------------------------------


def count_clause(tests, **over):
    return {"id": "power", "kind": "count", "arm": "mem", "measure": "rec_share", "tests": tests, **over}


def test_a_count_clause_reports_the_numerator_and_the_denominator(run_dir):
    # hard recurrences: 20: (20,1) 1; 21: (21,1) (21,2) 2; 22: 0; 23: (23,1) 1  -> 4 in all
    # unasked correct among them: (20,1) no, (21,1) yes, (21,2) no, (23,1) yes -> 2.
    spec = make_spec([count_clause([{"on": "total", "op": ">=", "value": 4}, {"on": "count", "op": ">=", "value": 2}])])
    ev = evaluator(spec, run_dir)
    out = ev.clause(spec["clauses"][0])
    assert (out["count"], out["total"], out["share"], out["streams"]) == (2.0, 4.0, 0.5, 4)
    assert out["pass"] is True
    assert [t["pass"] for t in out["tests"]] == [True, True]


def test_a_count_clause_fails_a_power_precondition_and_honours_a_window(run_dir):
    spec = make_spec([count_clause([{"on": "total", "op": ">", "value": 4}])])
    assert evaluator(spec, run_dir).clause(spec["clauses"][0])["pass"] is False
    spec = make_spec([count_clause([{"on": "total", "op": ">=", "value": 1}], window={"first": 2})])
    out = evaluator(spec, run_dir).clause(spec["clauses"][0])
    # the first two streams (20, 21): 3 recurrences, 1 unasked correct
    assert (out["count"], out["total"], out["streams"]) == (1.0, 3.0, 2)


def test_the_tests_of_a_count_clause_read_count_and_total_only(run_dir):
    with pytest.raises(C.SpecError, match="on is one of"):
        C.validate_spec(make_spec([count_clause([{"on": "point", "op": ">=", "value": 0.1}])]))
    with pytest.raises(C.SpecError, match="on is one of"):
        C.validate_spec(make_spec([{"id": "v", "kind": "value", "arm": "mem", "measure": "rec_share",
                                    "tests": [{"on": "count", "op": ">=", "value": 1}]}]))
    with pytest.raises(C.SpecError, match="not a quantile"):
        C.validate_spec(make_spec([count_clause([{"on": "count", "op": ">=", "value": 1}],
                                                measure="recshare_q50")]))


def test_a_count_clause_renders_in_the_table(run_dir):
    spec = make_spec([count_clause([{"on": "total", "op": ">=", "value": 4}])], verdict={"all": ["power"]})
    res = C.evaluate(spec, {"r": run_dir})
    assert "2 of 4 (0.5000)" in C.render_markdown(res)


# ---------------------------------------------------------------------------------------------
# The not node
# ---------------------------------------------------------------------------------------------


def two_value_clauses():
    return [
        {"id": "yes", "kind": "value", "arm": "mem", "measure": "rec_share", "tests": [{"on": "point", "op": ">=", "value": 0.5}]},
        {"id": "no", "kind": "value", "arm": "mem", "measure": "rec_share", "tests": [{"on": "point", "op": ">", "value": 0.5}]},
    ]


def test_not_negates_a_clause_and_a_node_and_is_recorded_by_name(run_dir):
    # rec_share point is exactly 0.5: `yes` holds, `no` does not.
    spec = make_spec(two_value_clauses(), verdict={"name": "v", "all": [
        "yes", {"name": "not_no", "not": "no"},
        {"name": "not_all", "not": {"name": "both", "all": ["yes", "no"]}}]})
    res = C.evaluate(spec, {"r": run_dir})
    assert res["verdict"]["holds"] is True
    assert res["verdict"]["nodes"] == {"not_no": True, "both": False, "not_all": True, "v": True}
    spec = make_spec(two_value_clauses(), verdict={"name": "v", "all": ["yes", {"not": "yes"}]})
    assert C.evaluate(spec, {"r": run_dir})["verdict"]["holds"] is False


def test_not_takes_one_item_and_a_node_has_one_kind():
    cl = two_value_clauses()
    with pytest.raises(C.SpecError, match="clause id or a node"):
        C.validate_spec(make_spec(cl, verdict={"not": ["yes", "no"]}))
    with pytest.raises(C.SpecError, match="exactly one of"):
        C.validate_spec(make_spec(cl, verdict={"all": ["yes"], "not": "no"}))
    with pytest.raises(C.SpecError, match="unknown clause"):
        C.validate_spec(make_spec(cl, verdict={"not": "ghost"}))


def test_a_clause_under_not_still_needs_tests():
    cl = [{"id": "c", "kind": "value", "arm": "mem", "measure": "rec_share"}]
    with pytest.raises(C.SpecError, match="no tests"):
        C.validate_spec(make_spec(cl, verdict={"not": "c"}))


# ---------------------------------------------------------------------------------------------
# Exclusive outcome categories
# ---------------------------------------------------------------------------------------------


def outcomes(**over):
    o = {"name": "A1b-like", "default": "none_of_these", "categories": [
        {"name": "holds", "when": "yes"},
        {"name": "lever_captured", "when": {"all": ["yes", {"not": "no"}]}},
        {"name": "never", "when": {"all": ["no"]}},
    ]}
    o.update(over)
    return o


def test_the_outcome_is_the_first_category_that_holds_and_the_shadowed_are_reported(run_dir):
    spec = make_spec(two_value_clauses(), outcomes=outcomes())
    res = C.evaluate(spec, {"r": run_dir})
    o = res["verdict"]["outcome"]
    # yes holds, no does not: `holds` and `lever_captured` both hold; the order decides.
    assert (o["category"], o["held"], o["shadowed"], o["default"]) == (
        "holds", ["holds", "lever_captured"], ["lever_captured"], "none_of_these")
    assert "Outcome (A1b-like): **holds**" in C.render_markdown(res)


def test_the_default_category_is_the_outcome_when_none_holds(run_dir):
    spec = make_spec(two_value_clauses(), outcomes=outcomes(categories=[{"name": "never", "when": "no"}]))
    o = C.evaluate(spec, {"r": run_dir})["verdict"]["outcome"]
    assert (o["category"], o["held"], o["shadowed"]) == ("none_of_these", [], [])


def test_outcomes_are_validated_and_a_specification_without_them_has_no_outcome(run_dir):
    cl = two_value_clauses()
    with pytest.raises(C.SpecError, match="default"):
        C.validate_spec(make_spec(cl, outcomes={"categories": [{"name": "a", "when": "yes"}]}))
    with pytest.raises(C.SpecError, match="repeated"):
        C.validate_spec(make_spec(cl, outcomes=outcomes(categories=[{"name": "a", "when": "yes"}, {"name": "a", "when": "no"}])))
    with pytest.raises(C.SpecError, match="repeated"):
        C.validate_spec(make_spec(cl, outcomes=outcomes(categories=[{"name": "none_of_these", "when": "yes"}])))
    with pytest.raises(C.SpecError, match="unknown clause"):
        C.validate_spec(make_spec(cl, outcomes=outcomes(categories=[{"name": "a", "when": "ghost"}])))
    with pytest.raises(C.SpecError, match="non-empty"):
        C.validate_spec(make_spec(cl, outcomes=outcomes(categories=[])))
    res = C.evaluate(make_spec(cl, verdict={"all": ["yes"]}), {"r": run_dir})
    assert "outcome" not in res["verdict"]


# ---------------------------------------------------------------------------------------------
# The per-stream quantile measure
# ---------------------------------------------------------------------------------------------


def test_the_quantile_point_is_the_inverted_cdf_of_the_per_stream_values(run_dir):
    # stale wrong per stream (mem): 20: (20,2) 1; 21: (21,2) 1; 22: (22,1) 1; 23: 0 -> [1, 1, 1, 0]
    spec = make_spec([{"id": "q", "kind": "value", "arm": "mem", "measure": "stale_per_stream_q90"}])
    est = evaluator(spec, run_dir).value("mem", "stale_per_stream_q90")
    assert est["point"] == float(np.quantile([1, 1, 1, 0], 0.9, method="inverted_cdf")) == 1.0
    for q in (0.0, 0.25, 0.5, 0.75, 1.0):
        x = np.array([0.0, 1.0, 1.0, 1.0])
        got = C.quantile_draws(x, np.ones(4), 1.0, np.ones((1, 4)), q)[0]
        assert got == np.quantile(x, q, method="inverted_cdf"), q


def test_a_stream_with_no_denominator_is_left_out_of_the_quantile(run_dir):
    # hard recurrences per stream (den): [1, 2, 0, 1]; unasked correct among them: [0, 1, 0, 1].
    # the per-stream share is defined for three streams: 0/1, 1/2, 1/1 -> [0, 0.5, 1]; the median is 0.5.
    spec = make_spec([], verdict={"all": []})
    est = evaluator(spec, run_dir).value("mem", "recshare_q50")
    assert est["point"] == 0.5
    # the same median over the four streams with the empty one counted as 0 would be 0.25 .. 0.5.
    num = np.array([0, 1, 0, 1.0])
    den = np.array([1, 2, 0, 1.0])
    assert C.quantile_draws(num, den, 1.0, np.ones((1, 4)), 0.5)[0] == 0.5


def test_the_resampled_quantile_matches_a_brute_force_reading(run_dir):
    spec = make_spec([], verdict={"all": []})
    ev = evaluator(spec, run_dir)
    num, den, scale = ev.arms.nd("mem", "stale_per_stream_q90")
    w = ev.boot.counts(4)
    fast = C.quantile_draws(num, den, scale, w, 0.9)
    slow = []
    x = num / den
    for row in w:
        sample = np.repeat(x, row.astype(int))
        slow.append(np.quantile(sample, 0.9, method="inverted_cdf"))
    assert np.array_equal(fast, np.array(slow))
    est = ev.value("mem", "stale_per_stream_q90")
    lo, hi = ev.boot.interval(fast)
    assert (est["lower"], est["upper"]) == (lo, hi)


def test_a_paired_clause_takes_the_difference_of_two_quantiles_over_the_same_resamples(run_dir):
    spec = make_spec([], verdict={"all": []})
    ev = evaluator(spec, run_dir)
    est = ev.paired("mem", "ctl", "stale_per_stream_q90")
    # ctl stale_wrong is zero in every stream: its quantile is 0, the difference is mem's.
    assert est["point"] == 1.0
    w = ev.boot.counts(4)
    nm, dm, s = ev.arms.nd("mem", "stale_per_stream_q90")
    assert est["lower"] <= est["point"] <= est["upper"]
    assert np.array_equal(C.quantile_draws(nm, dm, s, w, 0.9) - 0.0,
                          C.measure_draws({"kind": "quantile", "q": 0.9}, nm, dm, s, w))


def test_the_quantile_measure_is_validated():
    bad = make_spec()
    bad["measures"]["stale_per_stream_q90"]["q"] = 1.5
    with pytest.raises(C.SpecError, match=r"q is a number in \[0, 1\]"):
        C.validate_spec(bad)
    bad = make_spec()
    bad["measures"]["stale_per_stream_q90"]["kind"] = "median"
    with pytest.raises(C.SpecError, match="kind is one of"):
        C.validate_spec(bad)
    bad = make_spec([{"id": "s", "kind": "curve_slope", "arm": "mem", "measure": "recshare_q50", "per": 1,
                      "window": {"first": 4}, "tests": [{"on": "point", "op": ">", "value": 0}]}])
    with pytest.raises(C.SpecError, match="not a quantile"):
        C.validate_spec(bad)


# ---------------------------------------------------------------------------------------------
# The join of two arms by incident
# ---------------------------------------------------------------------------------------------


def test_a_join_counts_the_incidents_where_two_arms_differ(run_dir):
    # unasked wrong in mem and not in ctl: (20,2) mem wrong, ctl right: yes; (21,2): yes; (22,1) wrong in both: no.
    spec = make_spec()
    runs = C.RunSet(spec, {"r": run_dir})
    vec = C.term_vector(runs, "mem", spec["measures"]["wrong_added"]["num"])
    assert vec.tolist() == [1, 1, 0, 0]
    # the join is symmetric in what it counts: swap the arms and the conditions.
    swapped = term(where=[{"column": "unasked_wrong", "op": "eq", "value": True},
                          {"column": "unasked_wrong@mem", "op": "eq", "value": False}], join={"arm": "mem"})
    assert C.term_vector(runs, "ctl", swapped).tolist() == [0, 0, 0, 0]


def test_a_join_agrees_with_a_pandas_merge_written_in_the_test(run_dir):
    spec = make_spec()
    runs = C.RunSet(spec, {"r": run_dir})
    t = term("sum", columns={"correct_declarations": 1, "correct_declarations@ctl": -1}, join={"arm": "ctl"})
    got = C.term_vector(runs, "mem", t)
    a = pd.DataFrame(MEM, columns=COLS)
    b = pd.DataFrame(CTL, columns=COLS)
    m = a.merge(b, on=["seed", "incident"], suffixes=("", "@ctl"))
    want = (m["correct_declarations"] - m["correct_declarations@ctl"]).groupby(m["seed"]).sum()
    assert got.tolist() == want.reindex([20, 21, 22, 23]).tolist() == [-1, -1, 0, 0]


def test_the_join_filter_of_the_other_arm_drops_rows_and_the_paired_excess_is_a_difference_of_sums(run_dir):
    spec = make_spec()
    runs = C.RunSet(spec, {"r": run_dir})
    only_hard_in_ctl = term(join={"arm": "ctl", "where": [{"column": "tier", "op": "eq", "value": "hard"}]})
    assert C.term_vector(runs, "mem", only_hard_in_ctl).tolist() == [2, 3, 0, 2]
    # Summed over incidents, the difference of unasked wrong is the paired difference of the stream
    # totals: mem 1, 1, 1, 0 against ctl 0, 0, 1, 0 per stream.
    ev = evaluator(spec, run_dir)
    paired = ev.paired("mem", "ctl", "uw")
    assert paired["point"] == pytest.approx((1 + 1 + 1 + 0) / 4 - (0 + 0 + 1 + 0) / 4)


def test_a_join_refuses_arms_that_do_not_hold_the_same_incidents(run_dir):
    pd.DataFrame(CTL[:-1], columns=COLS).to_csv(run_dir / "CTL" / "memory_incidents.csv", index=False)
    spec = make_spec()
    runs = C.RunSet(spec, {"r": run_dir})
    with pytest.raises(C.SpecError, match="do not hold the same rows"):
        C.term_vector(runs, "mem", spec["measures"]["wrong_added"]["num"])


def test_a_join_refuses_a_key_that_does_not_identify_a_row(run_dir):
    rows = CTL + [CTL[0]]
    pd.DataFrame(rows, columns=COLS).to_csv(run_dir / "CTL" / "memory_incidents.csv", index=False)
    spec = make_spec()
    runs = C.RunSet(spec, {"r": run_dir})
    with pytest.raises(C.SpecError, match="do not identify a row"):
        C.term_vector(runs, "mem", spec["measures"]["wrong_added"]["num"])


def test_a_join_validation_names_what_it_would_have_to_guess():
    bad = make_spec()
    bad["measures"]["wrong_added"]["num"]["join"] = {"arm": "ghost"}
    with pytest.raises(C.SpecError, match="join.arm"):
        C.validate_spec(bad)
    bad = make_spec()
    bad["measures"]["wrong_added"]["num"]["join"] = {"arm": "ctl", "on": ["incident"]}
    with pytest.raises(C.SpecError, match="includes seed"):
        C.validate_spec(bad)
    bad = make_spec()
    bad["measures"]["wrong_added"]["num"]["join"] = {"arm": "ctl", "how": "left"}
    with pytest.raises(C.SpecError, match="join keys"):
        C.validate_spec(bad)


def test_the_files_a_join_reads_are_hashed_for_both_arms(run_dir):
    spec = make_spec([{"id": "j", "kind": "value", "arm": "mem", "measure": "wrong_added",
                       "tests": [{"on": "point", "op": ">=", "value": 0.0}]}], verdict={"all": ["j"]})
    res = C.evaluate(spec, {"r": run_dir})
    files = res["runs"]["r"]["files_read"]
    assert "MEM/memory_incidents.csv" in files and "CTL/memory_incidents.csv" in files
    assert res["clauses"][0]["point"] == pytest.approx(0.5)  # (1 + 1 + 0 + 0) / 4 streams
