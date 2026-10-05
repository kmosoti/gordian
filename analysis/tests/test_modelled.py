"""Modelled cost as the charter's cost C, and its check against wall time (work item A8b).

Hand-built runs only: the arithmetic is in the comments. The real-output fixtures are exercised in
`test_real_runs.py`.
"""

import json

import numpy as np
import pytest

from gordian_analysis import load as load_module
from gordian_analysis.cli import (
    analyze_cost_check,
    analyze_relative_savings,
    cost_basis,
    main,
)
from gordian_analysis.intervals import median_ratio_ci
from gordian_analysis.load import (
    MEASURED_POLICY,
    MEASURED_TOTAL,
    METRICS,
    MODELLED_TOTAL,
    LoadError,
    load_pair,
    load_run,
)

from conftest import write_run


def run_cli(capsys, *argv):
    rc = main(list(argv))
    out = capsys.readouterr()
    return rc, out.out, out.err


def episodes(n, modelled, measured, *, harness=0):
    """`n` episodes with a given modelled and measured policy cost each (callables of the index)."""
    rows = []
    for i in range(n):
        m, w = modelled(i), measured(i)
        rows.append(
            {
                "seed": i,
                "class": "c",
                "modelled_component_ns": m,
                "modelled_sched_ns": 0,
                "measured_component_ns": w,
                "measured_sched_ns": 0,
                "measured_harness_ns": harness,
            }
        )
    return rows


# ---- the loader ----------------------------------------------------------------------------


def test_modelled_cost_is_the_sum_of_the_two_modelled_columns(tmp_path):
    p = write_run(
        tmp_path / "r",
        [{"seed": 1, "modelled_component_ns": 70, "modelled_sched_ns": 5,
          "measured_component_ns": 100, "measured_sched_ns": 10, "measured_harness_ns": 1000}],
    )  # fmt: skip
    df = load_run(p).results
    assert df[MODELLED_TOTAL].tolist() == [75.0]
    # Wall time of the same episode: what the modelled cost covers, and the whole episode.
    assert df[MEASURED_POLICY].tolist() == [110.0]
    assert df[MEASURED_TOTAL].tolist() == [1110.0]


def test_the_counted_columns_are_metrics_and_required_numbers(tmp_path):
    for m in (MODELLED_TOTAL, MEASURED_POLICY, "modelled_component_ns", "modelled_sched_ns",
              "ops_component", "ops_sched"):  # fmt: skip
        assert m in METRICS
    p = write_run(tmp_path / "r", [{"seed": 1}])
    text = (p / "results.csv").read_text().splitlines()
    cols = text[0].split(",")
    row = text[1].split(",")
    row[cols.index("ops_sched")] = "many"
    (p / "results.csv").write_text("\n".join([text[0], ",".join(row)]) + "\n")
    with pytest.raises(LoadError, match="ops_sched.*non-numeric"):
        load_run(p)


# ---- relative savings: the default cost is the modelled cost -----------------------------


def test_relative_savings_defaults_to_the_modelled_cost_and_says_so(tmp_path):
    # Modelled: A 100 per episode, B 60 -> S = 1 - 600/1000 = 0.4. Measured wall time of the same
    # episodes is deliberately different (A 50, B 45 -> 0.1), so the two cannot be mistaken.
    a = write_run(tmp_path / "a", episodes(10, lambda i: 100, lambda i: 50, harness=7))
    b = write_run(tmp_path / "b", episodes(10, lambda i: 60, lambda i: 45, harness=7))
    r = analyze_relative_savings(str(a), str(b), threshold=0.2, seed=1, n_resamples=200)
    assert r["metric"] == MODELLED_TOTAL and r["cost_basis"] == "modelled"
    assert "the charter's cost C" in r["cost_basis_text"] and "NOT" not in r["cost_basis_text"]
    assert (r["a"]["total"], r["b"]["total"]) == (1000.0, 600.0)
    assert r["savings"] == pytest.approx(0.4)
    # The secondary check: wall time of the policy part (A 500, B 450 -> 0.1) and of the whole
    # episode (the harness's 7 ns per episode is in both arms: A 570, B 520 -> 50/570).
    sec = r["secondary"]
    assert sec[MEASURED_POLICY]["savings"] == pytest.approx(0.1)
    assert sec[MEASURED_TOTAL]["savings"] == pytest.approx(50 / 570)
    assert sec[MEASURED_POLICY]["cost_basis"] == "measured"


def test_the_text_report_labels_the_basis_and_the_secondary_check(capsys, tmp_path):
    a = write_run(tmp_path / "a", episodes(10, lambda i: 100, lambda i: 50))
    b = write_run(tmp_path / "b", episodes(10, lambda i: 60, lambda i: 45))
    rc, out, err = run_cli(capsys, "compare", "--a", str(a), "--b", str(b), "--relative-savings",
                           "--threshold", "0.2", "--seed", "1", "--resamples", "200")  # fmt: skip
    assert rc == 0 and err == ""
    assert "Relative savings on metric: modelled_cost_ns" in out
    assert "Cost basis: modelled cost, the charter's cost C" in out
    assert "Secondary check, MEASURED wall time" in out and "not the charter's cost C" in out
    assert "measured_policy_ns" in out and "measured_total_ns" in out


def test_a_measured_cost_is_labelled_a_secondary_check_and_reports_no_second_one(tmp_path):
    a = write_run(tmp_path / "a", episodes(10, lambda i: 100, lambda i: 50))
    b = write_run(tmp_path / "b", episodes(10, lambda i: 60, lambda i: 45))
    for metric in (MEASURED_TOTAL, MEASURED_POLICY):
        r = analyze_relative_savings(str(a), str(b), metric=metric, threshold=0.0, seed=1,
                                     n_resamples=100)  # fmt: skip
        assert r["cost_basis"] == "measured"
        assert "NOT the charter's cost C" in r["cost_basis_text"]
        assert "modelled_cost_ns is C" in r["cost_basis_text"]
        assert "secondary" not in r
    assert cost_basis("modelled_component_ns", False) == "modelled_partial"
    assert cost_basis("measured_sched_ns", False) == "measured_partial"
    with pytest.raises(ValueError, match="not a cost metric"):
        cost_basis("success", False)
    with pytest.raises(ValueError, match="declared cost, not the charter's cost C"):
        cost_basis("bill_compute", False)


def test_an_a_a_comparison_on_the_modelled_cost_is_exactly_zero(tmp_path):
    # Two copies of one arm: the modelled cost is deterministic, so S = 0 and the interval is a
    # point, however different their wall times are (here, wildly).
    rng = np.random.default_rng(3)
    modelled = rng.integers(500, 5000, size=60)
    a = write_run(tmp_path / "a", episodes(60, lambda i: int(modelled[i]), lambda i: int(modelled[i]) * 3))
    b = write_run(tmp_path / "b", episodes(60, lambda i: int(modelled[i]), lambda i: int(modelled[i]) + int(rng.integers(0, 4000))))  # fmt: skip
    r = analyze_relative_savings(str(a), str(b), threshold=0.0, seed=1, n_resamples=500)
    assert r["savings"] == 0.0
    assert (r["bootstrap"]["low"], r["bootstrap"]["high"]) == (0.0, 0.0)
    # ...while the wall time of the same two copies is not zero.
    assert r["secondary"][MEASURED_POLICY]["savings"] != 0.0


def test_an_arm_that_counts_nothing_cannot_be_a_modelled_baseline(tmp_path):
    a = write_run(tmp_path / "a", episodes(5, lambda i: 0, lambda i: 10))
    b = write_run(tmp_path / "b", episodes(5, lambda i: 4, lambda i: 10))
    with pytest.raises(ValueError, match="zero"):
        analyze_relative_savings(str(a), str(b), threshold=0.0, seed=1, n_resamples=50)
    with pytest.raises(ValueError, match="no modelled cost"):
        analyze_cost_check(str(a), str(b), seed=1, n_resamples=50)


# ---- the median of episode ratios ----------------------------------------------------------


def test_median_ratio_by_hand_and_the_interval_brackets_it(tmp_path):
    # A = 10 each; B = 5, 10, 20 -> ratios 0.5, 1, 2 -> median 1.
    a = write_run(tmp_path / "a", episodes(3, lambda i: 1, lambda i: 10))
    b = write_run(tmp_path / "b", episodes(3, lambda i: 1, lambda i: [5, 10, 20][i]))
    res = median_ratio_ci(load_pair(a, b), MEASURED_POLICY, seed=4, resamples=2000)
    assert res.median == 1.0 and res.n == 3
    assert res.low <= 1.0 <= res.high
    assert 0.5 <= res.low and res.high <= 2.0  # a resample's median is one of the ratios
    again = median_ratio_ci(load_pair(a, b), MEASURED_POLICY, seed=4, resamples=2000)
    assert (again.low, again.high) == (res.low, res.high)
    other = median_ratio_ci(load_pair(a, b), MEASURED_POLICY, seed=5, resamples=2000)
    assert other.median == res.median


def test_median_ratio_is_not_moved_by_a_few_interrupted_episodes(tmp_path):
    # 40 episodes with a true wall ratio of 0.5; three of A's copies were hit by steal time (x20).
    a = write_run(tmp_path / "a", episodes(40, lambda i: 1, lambda i: 1000 * (20 if i in (3, 11, 25) else 1)))
    b = write_run(tmp_path / "b", episodes(40, lambda i: 1, lambda i: 500))
    paired = load_pair(a, b)
    res = median_ratio_ci(paired, MEASURED_POLICY, seed=1, resamples=1000)
    assert res.median == pytest.approx(0.5)
    # The ratio of totals moved a long way: sum(A) = 1000*37 + 20000*3 = 97000, sum(B) = 20000.
    assert 500 * 40 / (1000 * 37 + 20000 * 3) == pytest.approx(0.2062, abs=1e-3)  # not 0.5


def test_median_ratio_refuses_a_zero_baseline_and_bad_arguments(tmp_path):
    a = write_run(tmp_path / "a", episodes(4, lambda i: 1, lambda i: 0 if i == 2 else 10))
    b = write_run(tmp_path / "b", episodes(4, lambda i: 1, lambda i: 5))
    with pytest.raises(ValueError, match="positive in arm A"):
        median_ratio_ci(load_pair(a, b), MEASURED_POLICY, seed=1, resamples=10)
    ok = write_run(tmp_path / "ok", episodes(4, lambda i: 1, lambda i: 10))
    with pytest.raises(ValueError, match="confidence"):
        median_ratio_ci(load_pair(ok, b), MEASURED_POLICY, seed=1, resamples=10, confidence=1.0)
    with pytest.raises(ValueError, match="seed"):
        median_ratio_ci(load_pair(ok, b), MEASURED_POLICY, seed=True, resamples=10)


# ---- the non-identical-arm check -----------------------------------------------------------


def test_cost_check_passes_when_the_model_tracks_the_wall_time_and_fails_when_it_does_not(tmp_path):
    rng = np.random.default_rng(11)
    n = 60
    base = rng.integers(2000, 9000, size=n)
    noise = rng.normal(1.0, 0.04, size=n)
    # Arm A: modelled = wall = base. Arm B costs 0.4 of it by the model and, in wall time, 0.4 x
    # noise; five of B's episodes were interrupted (x30), which moves a ratio of totals far.
    burst = np.ones(n)
    burst[[5, 17, 29, 41, 53]] = 30.0
    a = write_run(tmp_path / "a", episodes(n, lambda i: int(base[i]), lambda i: int(base[i])))
    b = write_run(
        tmp_path / "b",
        episodes(n, lambda i: int(0.4 * base[i]), lambda i: int(0.4 * base[i] * noise[i] * burst[i])),
    )
    r = analyze_cost_check(str(a), str(b), seed=1, n_resamples=2000)
    assert r["modelled"]["ratio_of_totals"] == pytest.approx(0.4, abs=1e-3)
    med = r["wall"]["median_episode_ratio"]
    assert med["median"] == pytest.approx(0.4, abs=0.02)
    assert r["modelled_ratio_inside_wall_interval"] is True
    assert r["wall"]["ratio_of_totals"] > 0.8  # the bursts moved the ratio of totals, not the median
    # A model that is wrong by a factor of two is outside the interval.
    wrong = write_run(tmp_path / "wrong", episodes(n, lambda i: int(0.8 * base[i]), lambda i: int(base[i])))
    r2 = analyze_cost_check(str(a), str(wrong), seed=1, n_resamples=2000)
    assert r2["wall"]["median_episode_ratio"]["median"] == 1.0  # the wall time did not move at all
    assert r2["modelled_ratio_inside_wall_interval"] is False
    off = write_run(tmp_path / "off", episodes(n, lambda i: int(0.8 * base[i]), lambda i: int(0.4 * base[i] * noise[i])))  # fmt: skip
    r3 = analyze_cost_check(str(a), str(off), seed=1, n_resamples=2000)
    assert r3["modelled_ratio_inside_wall_interval"] is False


def test_cost_check_cli_end_to_end(capsys, tmp_path):
    n = 30
    base = np.random.default_rng(2).integers(2000, 9000, size=n)
    a = write_run(tmp_path / "a", episodes(n, lambda i: int(base[i]), lambda i: int(base[i]), harness=500))
    b = write_run(tmp_path / "b", episodes(n, lambda i: int(0.5 * base[i]), lambda i: int(0.5 * base[i]), harness=500))
    j = tmp_path / "o.json"
    rc, out, err = run_cli(capsys, "cost-check", "--a", str(a), "--b", str(b), "--seed", "1",
                           "--resamples", "500", "--json", str(j))  # fmt: skip
    assert rc == 0 and err == ""
    r = json.loads(j.read_text())
    assert r["mode"] == "cost_check" and r["n_pairs"] == n
    assert r["wall"]["metric"] == MEASURED_POLICY
    assert r["wall"]["median_episode_ratio"]["median"] == pytest.approx(0.5, abs=1e-3)
    assert r["modelled_ratio_inside_wall_interval"] is True
    assert "lies INSIDE the wall-time interval" in out
    # The other wall reading includes the harness's own 500 ns per episode in both arms.
    rc, out, _ = run_cli(capsys, "cost-check", "--a", str(a), "--b", str(b), "--seed", "1",
                         "--resamples", "500", "--wall", MEASURED_TOTAL, "--json", str(j))  # fmt: skip
    r2 = json.loads(j.read_text())
    assert r2["wall"]["metric"] == MEASURED_TOTAL
    assert r2["wall"]["median_episode_ratio"]["median"] > 0.5  # diluted by the shared harness work
    # Errors are reported, not raised.
    rc, _, err = run_cli(capsys, "cost-check", "--a", str(a), "--b", str(tmp_path / "missing"),
                         "--seed", "1")  # fmt: skip
    assert rc == 2 and "gordian-analyze: error" in err


def test_bill_total_error_points_at_the_modelled_cost(tmp_path):
    p = write_run(tmp_path / "r", [{"seed": 1}])
    paired = load_pair(p, write_run(tmp_path / "s", [{"seed": 1}]))
    with pytest.raises(LoadError, match=r"bill_total was removed.*modelled_cost_ns"):
        paired.values("bill_total")


def test_schema_has_the_harness_columns_in_order():
    cols = load_module.RESULTS_COLUMNS
    assert cols[-4:] == ["ops_component", "ops_sched", "modelled_component_ns", "modelled_sched_ns"]
    assert cols.index("stop_reason") == len(cols) - 5
