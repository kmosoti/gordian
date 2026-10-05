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
    # a property of this harness's output, not a loader rule: undecided means a stop that is not
    # a decision (`terminal` and `final_declaration` are the decided ones)
    decided = b.results["stop_reason"].map(load_module.STOP_REASON_DECIDED)
    assert decided.notna().all()
    assert (b.results["undecided"] == ~decided.astype(bool)).all()
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


def test_real_fixtures_relative_savings_defaults_to_the_modelled_cost(capsys, fixtures_dir, tmp_path):
    j = tmp_path / "o.json"
    rc, out, err = run_cli(
        capsys, "compare", "--a", str(fixtures_dir / "real_a"), "--b", str(fixtures_dir / "real_b"),
        "--relative-savings", "--threshold", "0.20", "--seed", "1", "--json", str(j),
    )  # fmt: skip
    assert rc == 0 and err == ""
    r = json.loads(j.read_text())

    def modelled(name):
        return sum(
            int(x["modelled_component_ns"]) + int(x["modelled_sched_ns"])
            for x in csv_rows(fixtures_dir / name / "results.csv")
        )

    def wall(name, columns):
        return sum(
            sum(int(x[c]) for c in columns) for x in csv_rows(fixtures_dir / name / "measured.csv")
        )

    assert r["metric"] == "modelled_cost_ns" and r["cost_basis"] == "modelled"
    assert r["a"]["total"] == modelled("real_a") and r["b"]["total"] == modelled("real_b")
    assert modelled("real_a") > 0 and modelled("real_b") > 0
    assert r["savings"] == pytest.approx(1 - modelled("real_b") / modelled("real_a"))
    # The measured wall time of the same pairs, recomputed from the raw files, is the secondary
    # check: the policy part and the whole episode.
    policy = ("measured_component_ns", "measured_sched_ns")
    everything = (*policy, "measured_harness_ns")
    sec = r["secondary"]
    assert sec["measured_policy_ns"]["sum_a"] == wall("real_a", policy)
    assert sec["measured_policy_ns"]["sum_b"] == wall("real_b", policy)
    assert sec["measured_total_ns"]["sum_a"] == wall("real_a", everything)
    assert sec["measured_total_ns"]["savings"] == pytest.approx(
        1 - wall("real_b", everything) / wall("real_a", everything)
    )
    assert "Relative savings on metric: modelled_cost_ns" in out
    assert "Cost basis: modelled cost, the charter's cost C" in out
    assert "Secondary check, MEASURED wall time" in out
    assert "DECLARED" not in out


def test_real_fixture_counts_are_present_deterministic_columns(fixtures_dir):
    # Counted operations (A8b): present in every real row, positive when a component ran, and the
    # modelled columns are what the harness's weights give. Not checked against expected values:
    # they are this revision's constants.
    run = load_run(fixtures_dir / "real_a").results
    assert (run["ops_component"] > 0).all() and (run["ops_sched"] > 0).all()
    assert (run["modelled_component_ns"] > 0).all() and (run["modelled_sched_ns"] > 0).all()
    assert (run["modelled_cost_ns"] == run["modelled_component_ns"] + run["modelled_sched_ns"]).all()
    # heuristic_only runs one component, so its counted cost is the same order as its wall time.
    ratio = run["modelled_cost_ns"].sum() / run["measured_policy_ns"].sum()
    assert 0.3 < ratio < 3.0, ratio


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
    # arm_position (work item A8) is the harness's last measured column; the loader requires the
    # others and accepts it, so that runs written before A8 still load.
    assert [*load_module.MEASURED_COLUMNS, load_module.OPTIONAL_ARM_POSITION] == measured


def test_stop_reasons_are_the_harness_stop_reasons():
    # harness.rs: `StopReason::as_str` names every value of the stop_reason column, and
    # `StopReason::is_decided` says which of them close an episode with a decision. The loader's
    # table must say the same, so that a new reason is noticed here and not misread as undecided.
    import re
    from pathlib import Path

    path = Path(__file__).parents[2] / "crates/gordian-run/src/harness.rs"
    if not path.is_file():
        pytest.skip("harness source not present")
    src = path.read_text()
    as_str = src[src.index("pub fn as_str(self)"):]
    as_str = as_str[: as_str.index("\n    }\n")]
    names = dict(re.findall(r'StopReason::(\w+) => "(\w+)"', as_str))
    decided_fn = src[src.index("pub fn is_decided(self)"):]
    decided_fn = decided_fn[: decided_fn.index("\n    }\n")]
    decided = set(re.findall(r"StopReason::(\w+)", decided_fn))
    assert len(names) >= 5 and decided <= set(names)
    assert load_module.STOP_REASON_DECIDED == {
        text: variant in decided for variant, text in names.items()
    }
    assert load_module.STOP_REASON_DECIDED["final_declaration"] is True


def test_final_declaration_loads_as_a_decided_row_with_a_decision_time(tmp_path):
    p = write_run(
        tmp_path / "r",
        [{"seed": 1}, {"seed": 2, "stop_reason": "final_declaration"}, undecided_row(3)],
    )
    res = load_run(p).results
    assert res["stop_reason"].tolist() == ["terminal", "final_declaration", "budget_exhausted"]
    assert res["undecided"].tolist() == [False, False, True]
    assert res["decision_at_ns"].notna().tolist() == [True, True, False]
    # a final declaration counts as an answer in the coverage tables, not as an undecided episode
    t = coverage_error_table(res).iloc[0]
    assert t["undecided_rate"] == pytest.approx(1 / 3)


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


def test_relative_savings_default_is_the_modelled_cost(capsys, fixtures_dir, tmp_path):
    j = tmp_path / "o.json"
    rc, out, _ = run_cli(capsys, *rs(fixtures_dir, "--json", str(j)))
    r = json.loads(j.read_text())
    assert rc == 0 and r["metric"] == "modelled_cost_ns" and r["cost_basis"] == "modelled"
    # The hand-built fixtures' modelled cost was invented equal to their measured total, so the
    # hand values that bill_total once had hold for both.
    assert (r["a"]["total"], r["b"]["total"]) == (360.0, 270.0)
    assert "Cost basis: modelled cost, the charter's cost C" in out


def test_relative_savings_on_the_measured_total_is_a_labelled_secondary_check(
    capsys, fixtures_dir, tmp_path
):
    j = tmp_path / "o.json"
    rc, out, _ = run_cli(capsys, *rs(fixtures_dir, "--metric", "measured_total_ns", "--json", str(j)))
    r = json.loads(j.read_text())
    assert rc == 0 and r["metric"] == "measured_total_ns" and r["cost_basis"] == "measured"
    assert (r["a"]["total"], r["b"]["total"]) == (360.0, 270.0)
    assert "Cost basis: MEASURED wall time, a secondary check, NOT the charter's cost C" in out
    assert "secondary" not in r


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
    # on a modelled or measured metric (named or defaulted)
    for extra in (["--declared-cost"], ["--metric", "measured_total_ns", "--declared-cost"],
                  ["--metric", "modelled_cost_ns", "--declared-cost"]):  # fmt: skip
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
    assert "NOT the charter's cost C (modelled_cost_ns is C)" in out


def test_metric_is_required_outside_relative_savings(capsys, fixtures_dir):
    with pytest.raises(SystemExit) as e:
        main(["compare", "--a", str(fixtures_dir / "run_a"), "--b", str(fixtures_dir / "run_b"),
              "--margin", "1", "--lower-is-better", "--seed", "1"])  # fmt: skip
    assert e.value.code == 2 and "--metric is required" in capsys.readouterr().err


def test_undecided_episodes_stay_in_the_cost_totals(capsys, tmp_path):
    # Cost is spent whether or not the episode decided; dropping undecided rows would reward an
    # arm that burns its budget and fails. A: 3 x 100. B: two decided at 50 and one undecided at 90.
    a = write_run(tmp_path / "a", [{"seed": s, "modelled_component_ns": 100} for s in (1, 2, 3)])
    b = write_run(
        tmp_path / "b",
        [{"seed": 1, "modelled_component_ns": 50}, {"seed": 2, "modelled_component_ns": 50},
         undecided_row(3, modelled_component_ns=90)],
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


# ---- an interleaved A/A run: fixtures/real_aa (work item A8) --------------------------------
#
# Unedited `gordian-run` output: two copies of `heuristic_only` (arms a1 and a2), 3 seeds x 11
# classes, `--run-seed 1 --drift-block 10`. Provenance is in analysis/README.md. Its timings are
# one machine's and are never used as expected values here.


def test_real_aa_layout_and_manifests(fixtures_dir):
    root = fixtures_dir / "real_aa"
    top = json.loads((root / "manifest.json").read_text())
    assert [a["arm"] for a in top["arms"]] == ["a1", "a2"]
    assert "arm" not in top and "policy" not in top
    for name in ("a1", "a2"):
        arm = json.loads((root / name / "manifest.json").read_text())
        assert arm["arm"] == name and "arms" not in arm
        assert arm["run_id"] == f"real-aa.{name}"
        assert load_run(root / name).run_id == f"real-aa.{name}"


def test_real_aa_positions_are_complementary_and_the_copies_played_identically(fixtures_dir):
    from gordian_analysis.drift import check_same_episodes_same_play

    a = load_run(fixtures_dir / "real_aa" / "a1")
    b = load_run(fixtures_dir / "real_aa" / "a2")
    assert len(a.results) == len(b.results) == 33
    pa = a.results.sort_values(["seed", "class"])["arm_position"].to_numpy()
    pb = b.results.sort_values(["seed", "class"])["arm_position"].to_numpy()
    assert sorted(set(pa) | set(pb)) == [0, 1]
    assert (pa + pb == 1).all()  # each episode: one copy first, the other second
    # Both orders occur: with 33 fair coin flips the chance of all one way is 2 * 2^-33.
    assert 0 < pa.sum() < 33
    check_same_episodes_same_play(a, b)  # every non-timing column agrees


def test_real_aa_modelled_cost_is_identical_between_the_copies(capsys, fixtures_dir, tmp_path):
    # Work item A8b's acceptance for the A/A run: the modelled cost is deterministic, so two copies
    # of one arm have exactly the same modelled cost in every episode, whatever the host did to
    # their wall times, and S is exactly zero with a zero-width interval.
    root = fixtures_dir / "real_aa"
    a, b = load_run(root / "a1").results, load_run(root / "a2").results
    a, b = a.sort_values(["seed", "class"]), b.sort_values(["seed", "class"])
    assert (a["modelled_cost_ns"].to_numpy() == b["modelled_cost_ns"].to_numpy()).all()
    assert (a["ops_component"].to_numpy() == b["ops_component"].to_numpy()).all()
    assert (a["ops_sched"].to_numpy() == b["ops_sched"].to_numpy()).all()
    j = tmp_path / "o.json"
    rc, out, err = run_cli(
        capsys, "compare", "--a", str(root / "a1"), "--b", str(root / "a2"), "--relative-savings",
        "--threshold", "0", "--seed", "1", "--json", str(j),
    )  # fmt: skip
    assert rc == 0 and err == ""
    r = json.loads(j.read_text())
    assert r["metric"] == "modelled_cost_ns"
    assert r["savings"] == 0.0 and r["bootstrap"]["low"] == 0.0 and r["bootstrap"]["high"] == 0.0
    # The wall time of the same two copies is not identical: that is what this replaces.
    assert a["measured_total_ns"].to_numpy().tolist() != b["measured_total_ns"].to_numpy().tolist()


def test_real_aa_drift_and_position_diagnostics_run_on_real_output(capsys, fixtures_dir):
    from gordian_analysis.drift import drift_report, load_drift, position_effect_paired

    root = fixtures_dir / "real_aa"
    df = load_drift(root)
    assert df["block"].tolist() == [0, 1, 2, 3, 4]
    assert df["units_done"].tolist() == [0, 10, 20, 30, 33]  # every 10 episodes, then the close
    r = drift_report(df)
    assert r.n_blocks == 5 and r.ns.cv > 0 and r.ns.ratio_last_first > 0
    a, b = load_run(root / "a1"), load_run(root / "a2")
    e = position_effect_paired(a, b, seed=1, n_resamples=500, n_permutations=500)
    assert e.n_first == 33 and e.low < e.log_ratio < e.high
    for argv in (
        ["drift", "--run", str(root)],
        ["position", "--arm", str(root / "a1"), "--paired-with", str(root / "a2"), "--seed", "1"],
        ["position", "--arm", str(root / "a1"), "--seed", "1"],
        [
            "compare", "--a", str(root / "a1"), "--b", str(root / "a2"), "--relative-savings",
            "--threshold", "0", "--seed", "1",
        ],
    ):  # fmt: skip
        rc, out, err = run_cli(capsys, *argv)
        assert rc == 0 and err == "", (argv, err)
