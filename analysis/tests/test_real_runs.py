"""The loader and CLI against what the harness actually writes.

`fixtures/real_a` and `fixtures/real_b` are unedited `gordian-run` output (3 seeds x 11 classes,
`heuristic_only`, both arms; provenance is in `analysis/README.md`). The other tests build the same schema
by hand with `write_run` to reach cases the real runs do not contain.
"""

import csv
import json
import math

import numpy as np
import pytest

from gordian_analysis import load as load_module
from gordian_analysis.breakdown import coverage_error_table, per_class_table, risk_coverage_curve
from gordian_analysis.cli import main
from gordian_analysis.load import METRICS, LoadError, load_pair, load_run, pair_runs

from conftest import DEFAULTS, write_run


def run_cli(capsys, *argv):
    rc = main(list(argv))
    out = capsys.readouterr()
    return rc, out.out, out.err


def csv_rows(path):
    with open(path, newline="") as fh:
        return list(csv.DictReader(fh))


# ---- real harness output ------------------------------------------------------------------


def test_real_fixtures_load_and_pair(fixtures_dir):
    a, b = load_run(fixtures_dir / "real_a"), load_run(fixtures_dir / "real_b")
    assert (a.run_id, b.run_id) == ("real-a", "real-b")
    assert len(a.results) == len(b.results) == 33  # 3 seeds x 11 classes
    assert a.results["class"].nunique() == 11
    paired = pair_runs(a, b)
    assert len(paired) == 33
    assert paired.a[["seed", "class"]].equals(paired.b[["seed", "class"]])
    # the manifest next to each fixture is the one that produced it
    for name, run in (("real_a", a), ("real_b", b)):
        m = json.loads((fixtures_dir / name / "manifest.json").read_text())
        assert m["run_id"] == run.run_id
        assert len(m["seeds"]) * len(m["episode_classes"]) == len(run.results)


def test_real_fixture_undecided_rows_load_with_missing_decision_time(fixtures_dir):
    a, b = load_run(fixtures_dir / "real_a"), load_run(fixtures_dir / "real_b")
    # Counted from the raw files, not from the loader.
    raw_b = csv_rows(fixtures_dir / "real_b" / "results.csv")
    n_undecided = sum(r["undecided"] == "true" for r in raw_b)
    assert n_undecided == 30 and all(r["decision_at_ns"] == "" for r in raw_b if r["undecided"] == "true")
    assert int(b.results["undecided"].sum()) == n_undecided
    assert b.results.loc[b.results["undecided"], "decision_at_ns"].isna().all()
    assert b.results.loc[~b.results["undecided"], "decision_at_ns"].notna().all()
    assert a.results["undecided"].sum() == 0 and a.results["decision_at_ns"].notna().all()
    # a property of this harness's output, not a loader rule: undecided means a non-terminal stop
    assert (b.results["undecided"] == (b.results["stop_reason"] != "terminal")).all()
    # corrections and directives_ignored are loaded as numbers
    assert (b.results["corrections"] >= 0).all() and (b.results["directives_ignored"] >= 0).all()


def test_real_fixture_measured_columns_joined_by_key_and_summed(fixtures_dir):
    run = load_run(fixtures_dir / "real_a")
    raw = {
        (int(r["seed"]), r["class"]): sum(
            int(r[c]) for c in ("measured_component_ns", "measured_sched_ns", "measured_harness_ns")
        )
        for r in csv_rows(fixtures_dir / "real_a" / "measured.csv")
    }
    for _, row in run.results.iterrows():
        assert row["measured_total_ns"] == raw[(row["seed"], row["class"])]
    assert (run.results["measured_total_ns"] > 0).all()


def test_real_fixtures_acceptance_compare_success(capsys, fixtures_dir, tmp_path):
    j = tmp_path / "o.json"
    rc, out, err = run_cli(
        capsys, "compare", "--a", str(fixtures_dir / "real_a"), "--b", str(fixtures_dir / "real_b"),
        "--metric", "success", "--margin", "0.05", "--higher-is-better", "--seed", "1",
        "--json", str(j),
    )  # fmt: skip
    assert rc == 0 and err == ""
    r = json.loads(j.read_text())
    ra = csv_rows(fixtures_dir / "real_a" / "results.csv")
    rb = csv_rows(fixtures_dir / "real_b" / "results.csv")
    assert r["a"]["mean"] == pytest.approx(sum(x["success"] == "true" for x in ra) / 33)
    assert r["b"]["mean"] == pytest.approx(sum(x["success"] == "true" for x in rb) / 33)
    assert r["n_pairs"] == 33
    # B undecided on 30 of 33 episodes, and those count as failures
    assert r["category"] == "harmful"
    undec = {row["class"]: row["undecided_rate"] for row in r["coverage_error"]["b"]}
    assert undec["ALL"] == pytest.approx(30 / 33)


def test_real_fixtures_relative_savings_defaults_to_measured_total(capsys, fixtures_dir, tmp_path):
    j = tmp_path / "o.json"
    rc, out, err = run_cli(
        capsys, "compare", "--a", str(fixtures_dir / "real_a"), "--b", str(fixtures_dir / "real_b"),
        "--relative-savings", "--threshold", "0.20", "--seed", "1", "--json", str(j),
    )  # fmt: skip
    assert rc == 0 and err == ""
    r = json.loads(j.read_text())

    def total(name):
        return sum(
            int(x["measured_component_ns"]) + int(x["measured_sched_ns"]) + int(x["measured_harness_ns"])
            for x in csv_rows(fixtures_dir / name / "measured.csv")
        )

    assert r["metric"] == "measured_total_ns" and r["cost_basis"] == "measured"
    assert r["a"]["total"] == total("real_a") and r["b"]["total"] == total("real_b")
    assert r["savings"] == pytest.approx(1 - total("real_b") / total("real_a"))
    assert "Relative savings on metric: measured_total_ns" in out
    assert "Cost basis: measured wall time, the charter's cost C" in out
    assert "DECLARED" not in out


# ---- undecided rows -----------------------------------------------------------------------


def undecided_row(seed, **over):
    return {"seed": seed, "success": 0, "undecided": 1, "decision_at_ns": "",
            "stop_reason": "budget_exhausted", **over}  # fmt: skip


def test_undecided_row_loads_with_nan_decision_time(tmp_path):
    p = write_run(tmp_path / "r", [{"seed": 1}, undecided_row(2), {"seed": 3, "undecided": "false"}])
    res = load_run(p).results
    assert res["undecided"].tolist() == [False, True, False]
    assert res["decision_at_ns"].isna().tolist() == [False, True, False]
    assert res["stop_reason"].tolist() == ["terminal", "budget_exhausted", "terminal"]


def test_decided_row_with_empty_decision_time_rejected(tmp_path):
    p = write_run(tmp_path / "r", [{"seed": 1}, {"seed": 2, "decision_at_ns": ""}])
    with pytest.raises(LoadError, match=r"decision_at_ns.*empty on a decided episode.*row 3"):
        load_run(p)


def test_undecided_row_with_a_decision_time_rejected(tmp_path):
    p = write_run(tmp_path / "r", [undecided_row(1, decision_at_ns=500)])
    with pytest.raises(LoadError, match="undecided episode"):
        load_run(p)


def test_empty_is_missing_only_for_decision_time(tmp_path):
    for col in ("bill_compute", "probes_used", "corrections", "directives_ignored", "stop_reason",
                "undecided"):  # fmt: skip
        p = write_run(tmp_path / col, [undecided_row(1, **{col: ""})])
        with pytest.raises(LoadError, match=col):
            load_run(p)
    p = write_run(tmp_path / "m", [undecided_row(1, measured_harness_ns="")])
    with pytest.raises(LoadError, match="measured_harness_ns"):
        load_run(p)


def test_nonfinite_decision_time_rejected_on_decided_row(tmp_path):
    p = write_run(tmp_path / "r", [{"seed": 1, "decision_at_ns": "nan"}])
    with pytest.raises(LoadError, match="decision_at_ns"):
        load_run(p)


def test_decision_time_metric_refused_while_any_episode_is_undecided(tmp_path):
    a = write_run(tmp_path / "a", [{"seed": 1}, {"seed": 2}])
    b = write_run(tmp_path / "b", [{"seed": 1}, undecided_row(2)])
    paired = load_pair(a, b)
    with pytest.raises(LoadError, match=r"undefined for undecided episodes \(0 in A, 1 in B\)"):
        paired.values("decision_at_ns")
    with pytest.raises(LoadError, match="undecided"):
        paired.differences("decision_at_ns", False)
    # success and undecided are defined for every episode, undecided ones included
    assert paired.values("success")[1].tolist() == [1.0, 0.0]
    assert paired.values("undecided")[1].tolist() == [0.0, 1.0]
    # with nothing undecided the metric works as before
    c = write_run(tmp_path / "c", [{"seed": 1, "decision_at_ns": 300}, {"seed": 2, "decision_at_ns": 500}])
    va, vb = load_pair(a, c).values("decision_at_ns")
    assert va.tolist() == [100.0, 100.0] and vb.tolist() == [300.0, 500.0]


def test_decision_time_refusal_reaches_the_cli_as_exit_2(capsys, fixtures_dir):
    rc, out, err = run_cli(
        capsys, "compare", "--a", str(fixtures_dir / "real_a"), "--b", str(fixtures_dir / "real_b"),
        "--metric", "decision_at_ns", "--margin", "1", "--lower-is-better", "--seed", "1",
    )  # fmt: skip
    assert rc == 2 and out == "" and "undefined for undecided episodes" in err


def test_per_class_table_reports_missing_decision_times(tmp_path):
    p = write_run(
        tmp_path / "r",
        [{"seed": 1, "decision_at_ns": 10}, {"seed": 2, "decision_at_ns": 30}, undecided_row(3)],
    )
    t = per_class_table(load_run(p).results, ["decision_at_ns", "success"]).set_index("metric")
    assert t.loc["decision_at_ns", "n"] == 2 and t.loc["decision_at_ns", "n_missing"] == 1
    assert t.loc["decision_at_ns", "mean"] == pytest.approx(20.0)
    assert t.loc["success", "n"] == 3 and t.loc["success", "n_missing"] == 0
    # a class in which nothing decided has no mean, rather than a made-up one
    q = write_run(tmp_path / "q", [undecided_row(1)])
    t = per_class_table(load_run(q).results, ["decision_at_ns"])
    assert t.loc[0, "n"] == 0 and math.isnan(t.loc[0, "mean"])


def test_undecided_is_neither_coverage_nor_an_answered_error(tmp_path):
    # 4 episodes in one class: 1 answered correctly, 1 answered wrongly, 1 abstained, 1 undecided.
    # coverage = 2/4; error among answered = 1/2; undecided_rate = 1/4.
    p = write_run(
        tmp_path / "r",
        [
            {"seed": 1, "success": 1},
            {"seed": 2, "success": 0},
            {"seed": 3, "success": 0, "abstained": 1},
            undecided_row(4),
        ],
    )
    t = coverage_error_table(load_run(p).results).set_index("class")
    assert t.loc["c", "n_answered"] == 2 and t.loc["c", "coverage"] == pytest.approx(0.5)
    assert t.loc["c", "error_rate_answered"] == pytest.approx(0.5)
    assert t.loc["c", "undecided_rate"] == pytest.approx(0.25)
    # same exclusion in the risk-coverage curve; coverage still divides by every episode
    q = write_run(
        tmp_path / "q",
        [
            {"seed": 1, "success": 1, "confidence": 0.9},
            undecided_row(2, confidence=0.99),
        ],
        extra_cols=("confidence",),
    )
    curve = risk_coverage_curve(load_run(q).results)
    assert curve["threshold"].tolist() == [0.9]  # the undecided episode's confidence is not used
    assert curve["coverage"].tolist() == [0.5]


# ---- the exact column set -----------------------------------------------------------------


def test_unknown_results_column_rejected(tmp_path):
    p = write_run(tmp_path / "r", [{"seed": 1}], extra_cols=("brand_new_column",))
    with pytest.raises(LoadError, match=r"unknown columns \['brand_new_column'\]"):
        load_run(p)


def test_unknown_measured_column_rejected(tmp_path):
    p = write_run(tmp_path / "r", [{"seed": 1}])
    text = (p / "measured.csv").read_text().splitlines()
    (p / "measured.csv").write_text("\n".join([text[0] + ",measured_gpu_ns", text[1] + ",5"]) + "\n")
    with pytest.raises(LoadError, match=r"measured\.csv.*unknown columns \['measured_gpu_ns'\]"):
        load_run(p)


def test_every_documented_column_is_required(tmp_path):
    # drop each results column in turn (the loader names it), including the new ones
    for col in load_module.RESULTS_COLUMNS:
        p = write_run(tmp_path / col, [{"seed": 1}])
        lines = [ln.split(",") for ln in (p / "results.csv").read_text().splitlines()]
        i = lines[0].index(col)
        (p / "results.csv").write_text("\n".join(",".join(x for j, x in enumerate(ln) if j != i) for ln in lines) + "\n")
        with pytest.raises(LoadError, match=f"missing columns.*{col}"):
            load_run(p)


def test_confidence_stays_the_one_optional_column(tmp_path):
    p = write_run(tmp_path / "r", [{"seed": 1, "confidence": 0.7}], extra_cols=("confidence",))
    assert load_run(p).results["confidence"].tolist() == [0.7]


def test_real_schema_is_the_harness_header():
    # results.rs: RESULTS_HEADER / MEASURED_HEADER, the single source of the column names.
    import re
    from pathlib import Path

    path = Path(__file__).parents[2] / "crates/gordian-run/src/results.rs"
    if not path.is_file():
        pytest.skip("harness source not present")
    src = path.read_text()
    results = re.search(r'RESULTS_HEADER: &str = "([^"]+)"', src).group(1).split(",")
    measured = re.search(r'MEASURED_HEADER: &str =\s*"([^"]+)"', src).group(1).split(",")
    assert load_module.RESULTS_COLUMNS == results
    assert load_module.MEASURED_COLUMNS == measured


# ---- measured.csv and the join ------------------------------------------------------------


def test_missing_measured_csv_rejected(tmp_path):
    p = write_run(tmp_path / "r", [{"seed": 1}], measured=False)
    with pytest.raises(LoadError, match="no measured.csv"):
        load_run(p)


def mismatched(tmp_path, results_rows, measured_rows):
    p = write_run(tmp_path / "r", results_rows)
    write_run(tmp_path / "scratch", measured_rows)
    (p / "measured.csv").write_text((tmp_path / "scratch" / "measured.csv").read_text())
    return p


def test_measured_missing_a_results_key_rejected(tmp_path):
    p = mismatched(tmp_path, [{"seed": 1}, {"seed": 2}], [{"seed": 1}])
    with pytest.raises(LoadError, match=r"1 only in results\.csv"):
        load_run(p)


def test_measured_with_an_extra_key_rejected(tmp_path):
    p = mismatched(tmp_path, [{"seed": 1}], [{"seed": 1}, {"seed": 2}])
    with pytest.raises(LoadError, match=r"1 only in measured\.csv"):
        load_run(p)


def test_same_count_but_different_key_rejected(tmp_path):
    # equal row counts must not be mistaken for a match
    p = mismatched(tmp_path, [{"seed": 1, "class": "x"}], [{"seed": 1, "class": "y"}])
    with pytest.raises(LoadError, match="do not have the same"):
        load_run(p)
    p = mismatched(tmp_path / "s", [{"seed": 1}], [{"seed": 2}])
    with pytest.raises(LoadError, match="do not have the same"):
        load_run(p)


def test_duplicate_key_in_measured_rejected(tmp_path):
    p = mismatched(tmp_path, [{"seed": 1}, {"seed": 2}], [{"seed": 1}, {"seed": 1}])
    with pytest.raises(LoadError, match=r"measured\.csv.*duplicate"):
        load_run(p)


def test_run_id_must_agree_between_the_two_files(tmp_path):
    p = write_run(tmp_path / "r", [{"seed": 1}], run_id="one")
    write_run(tmp_path / "s", [{"seed": 1}], run_id="two")
    (p / "measured.csv").write_text((tmp_path / "s" / "measured.csv").read_text())
    with pytest.raises(LoadError, match="run_id differs"):
        load_run(p)


def test_join_follows_the_key_not_the_row_order(tmp_path):
    p = write_run(
        tmp_path / "r",
        [{"seed": 1, "measured_component_ns": 10}, {"seed": 2, "measured_component_ns": 20}],
    )
    lines = (p / "measured.csv").read_text().splitlines()
    (p / "measured.csv").write_text("\n".join([lines[0], lines[2], lines[1]]) + "\n")  # swap rows
    res = load_run(p).results
    assert res["measured_component_ns"].tolist() == [10.0, 20.0]


# ---- bill_total is gone -------------------------------------------------------------------


def test_bill_total_no_longer_exists(tmp_path):
    assert "bill_total" not in METRICS
    p = write_run(tmp_path / "r", [{"seed": 1, "bill_compute": 3, "bill_probes": 2}])
    assert "bill_total" not in load_run(p).results.columns
    paired = load_pair(p, write_run(tmp_path / "s", [{"seed": 1}]))
    with pytest.raises(LoadError, match=r"bill_total was removed.*no common unit"):
        paired.values("bill_total")
    # the per-resource bill columns are still available one at a time
    assert paired.values("bill_probes")[0].tolist() == [2.0]


def test_bill_total_is_a_clear_cli_error(capsys, fixtures_dir):
    base = ["compare", "--a", str(fixtures_dir / "run_a"), "--b", str(fixtures_dir / "run_b")]
    for extra in (
        ["--metric", "bill_total", "--margin", "1", "--lower-is-better", "--seed", "1"],
        ["--metric", "bill_total", "--relative-savings", "--threshold", "0.2", "--seed", "1"],
    ):
        with pytest.raises(SystemExit) as e:
            main([*base, *extra])
        assert e.value.code == 2
        err = capsys.readouterr().err
        assert "bill_total was removed" in err and "measured_total_ns" in err
    with pytest.raises(SystemExit):
        main([*base, "--metric", "nonsense", "--margin", "1", "--lower-is-better", "--seed", "1"])
    assert "unknown metric 'nonsense'" in capsys.readouterr().err


# ---- relative savings: which cost ---------------------------------------------------------


def rs(fx, *extra):
    return ("compare", "--a", str(fx / "run_a"), "--b", str(fx / "run_b"),
            "--relative-savings", "--threshold", "0.2", "--seed", "1", *extra)  # fmt: skip


def test_relative_savings_default_is_measured_total(capsys, fixtures_dir, tmp_path):
    j = tmp_path / "o.json"
    rc, out, _ = run_cli(capsys, *rs(fixtures_dir, "--json", str(j)))
    r = json.loads(j.read_text())
    assert rc == 0 and r["metric"] == "measured_total_ns" and r["cost_basis"] == "measured"
    assert (r["a"]["total"], r["b"]["total"]) == (360.0, 270.0)  # the same hand values as bill_total had
    assert "Cost basis: measured wall time, the charter's cost C" in out


def test_relative_savings_on_a_bill_column_needs_the_explicit_flag(capsys, fixtures_dir):
    rc, out, err = run_cli(capsys, *rs(fixtures_dir, "--metric", "bill_compute"))
    assert rc == 2 and out == ""
    assert "bill_compute is declared cost, not the charter's cost C" in err
    assert "--declared-cost" in err


def test_relative_savings_declared_cost_is_reported_as_such(capsys, fixtures_dir, tmp_path):
    j = tmp_path / "o.json"
    rc, out, err = run_cli(
        capsys, *rs(fixtures_dir, "--metric", "bill_compute", "--declared-cost", "--json", str(j))
    )
    r = json.loads(j.read_text())
    assert rc == 0 and err == ""
    # totals recomputed from the raw files, not from the loader
    a = sum(int(x["bill_compute"]) for x in csv_rows(fixtures_dir / "run_a" / "results.csv"))
    b = sum(int(x["bill_compute"]) for x in csv_rows(fixtures_dir / "run_b" / "results.csv"))
    assert (r["a"]["total"], r["b"]["total"]) == (a, b)
    assert r["savings"] == pytest.approx(1 - b / a)
    assert r["cost_basis"] == "declared" and "NOT the charter's cost C" in r["cost_basis_text"]
    assert "Cost basis: DECLARED cost (a bill column), NOT the charter's cost C" in out
    assert "Relative savings on metric: bill_compute" in out


def test_declared_cost_flag_is_refused_where_it_does_not_apply(capsys, fixtures_dir):
    # on a measured metric (named or defaulted)
    for extra in (["--declared-cost"], ["--metric", "measured_total_ns", "--declared-cost"]):
        rc, _, err = run_cli(capsys, *rs(fixtures_dir, *extra))
        assert rc == 2 and "--declared-cost applies only to a bill_* column" in err
    # outside --relative-savings
    with pytest.raises(SystemExit) as e:
        main(["compare", "--a", str(fixtures_dir / "run_a"), "--b", str(fixtures_dir / "run_b"),
              "--metric", "bill_compute", "--margin", "1", "--lower-is-better", "--seed", "1",
              "--declared-cost"])  # fmt: skip
    assert e.value.code == 2
    assert "--declared-cost applies only with --relative-savings" in capsys.readouterr().err


def test_relative_savings_refuses_metrics_that_are_not_costs(capsys, fixtures_dir):
    for m in ("success", "decision_at_ns", "probes_used"):
        rc, out, err = run_cli(capsys, *rs(fixtures_dir, "--metric", m))
        assert rc == 2 and "is not a cost metric" in err, m


def test_relative_savings_on_a_partial_measured_column_is_labelled_partial(capsys, fixtures_dir):
    rc, out, _ = run_cli(capsys, *rs(fixtures_dir, "--metric", "measured_component_ns"))
    assert rc == 0
    assert "NOT the charter's cost C (measured_total_ns is C)" in out


def test_metric_is_required_outside_relative_savings(capsys, fixtures_dir):
    with pytest.raises(SystemExit) as e:
        main(["compare", "--a", str(fixtures_dir / "run_a"), "--b", str(fixtures_dir / "run_b"),
              "--margin", "1", "--lower-is-better", "--seed", "1"])  # fmt: skip
    assert e.value.code == 2 and "--metric is required" in capsys.readouterr().err


def test_undecided_episodes_stay_in_the_cost_totals(capsys, tmp_path):
    # Cost is spent whether or not the episode decided; dropping undecided rows would reward an
    # arm that burns its budget and fails. A: 3 x 100. B: two decided at 50 and one undecided at 90.
    a = write_run(tmp_path / "a", [{"seed": s, "measured_component_ns": 100} for s in (1, 2, 3)])
    b = write_run(
        tmp_path / "b",
        [{"seed": 1, "measured_component_ns": 50}, {"seed": 2, "measured_component_ns": 50},
         undecided_row(3, measured_component_ns=90)],
    )  # fmt: skip
    j = tmp_path / "o.json"
    rc, _, _ = run_cli(capsys, "compare", "--a", str(a), "--b", str(b), "--relative-savings",
                       "--threshold", "0.2", "--seed", "1", "--json", str(j))  # fmt: skip
    r = json.loads(j.read_text())
    assert rc == 0 and r["b"]["total"] == 190.0 and r["savings"] == pytest.approx(1 - 190 / 300)
    assert r["n_pairs"] == 3
    assert np.isfinite(r["bootstrap"]["low"])


def test_defaults_row_matches_harness_column_count():
    assert list(DEFAULTS) == load_module.RESULTS_COLUMNS
