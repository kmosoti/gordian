import hashlib
import json

import numpy as np
import pytest

from gordian_analysis.load import LoadError, load_pair, load_run, pair_runs

from conftest import write_run


def rows(n=3, classes=("x",), **over):
    return [{"seed": s, "class": c, **over} for c in classes for s in range(1, n + 1)]


def test_loads_fixture_with_both_boolean_spellings_and_usage(fixtures_dir):
    a = load_run(fixtures_dir / "run_a")  # true/false
    b = load_run(fixtures_dir / "run_b")  # 0/1
    assert a.run_id == "run-a" and b.run_id == "run-b"
    assert len(a.results) == len(b.results) == 20
    assert a.results["success"].dtype == bool and b.results["success"].dtype == bool
    assert a.usage is None
    assert b.usage == {"cpu_ns": 123456, "internal_external_ratio": 1.02}
    assert "confidence" in b.results.columns and "confidence" not in a.results.columns


def test_boolean_parsing_variants(tmp_path):
    p = write_run(
        tmp_path / "r",
        [
            {"seed": 1, "success": "true"},
            {"seed": 2, "success": "False"},
            {"seed": 3, "success": "1"},
            {"seed": 4, "success": "0"},
        ],
    )
    assert load_run(p).results["success"].tolist() == [True, False, True, False]


def test_bad_boolean_and_bad_number_fail_loudly(tmp_path):
    p = write_run(tmp_path / "r", [{"seed": 1, "success": "maybe"}])
    with pytest.raises(LoadError, match="success"):
        load_run(p)
    p = write_run(tmp_path / "s", [{"seed": 1, "bill_compute": "abc"}])
    with pytest.raises(LoadError, match="bill_compute"):
        load_run(p)
    p = write_run(tmp_path / "t", [{"seed": 1, "bill_compute": "nan"}])
    with pytest.raises(LoadError, match="bill_compute"):
        load_run(p)
    p = write_run(tmp_path / "u", [{"seed": 1, "bill_compute": ""}])
    with pytest.raises(LoadError, match="bill_compute"):
        load_run(p)


def test_missing_file_and_missing_column(tmp_path):
    (tmp_path / "empty").mkdir()
    with pytest.raises(LoadError, match="no results.csv"):
        load_run(tmp_path / "empty")
    d = tmp_path / "nocol"
    d.mkdir()
    (d / "results.csv").write_text("run_id,seed,class\nr,1,x\n")
    with pytest.raises(LoadError, match="missing columns"):
        load_run(d)


def test_duplicate_seed_class_keys_rejected(tmp_path):
    p = write_run(tmp_path / "r", [{"seed": 1}, {"seed": 2}, {"seed": 1}])
    with pytest.raises(LoadError, match="duplicate"):
        load_run(p)


def test_same_seed_different_class_is_not_duplicate(tmp_path):
    p = write_run(tmp_path / "r", [{"seed": 1, "class": "x"}, {"seed": 1, "class": "y"}])
    assert len(load_run(p).results) == 2


def test_multiple_run_ids_rejected(tmp_path):
    p = write_run(tmp_path / "r", [{"seed": 1, "run_id": "a"}, {"seed": 2, "run_id": "b"}])
    with pytest.raises(LoadError, match="run_id"):
        load_run(p)


def test_unmatched_pairs_rejected_both_directions(tmp_path):
    a = write_run(tmp_path / "a", rows(3))
    b = write_run(tmp_path / "b", rows(2))  # seed 3 missing in B
    with pytest.raises(LoadError, match="unmatched"):
        load_pair(a, b)
    with pytest.raises(LoadError, match="unmatched"):
        load_pair(b, a)
    # same seeds, different class label
    c = write_run(tmp_path / "c", rows(3, classes=("y",)))
    with pytest.raises(LoadError, match="unmatched"):
        load_pair(a, c)


def test_pairing_aligns_rows_regardless_of_file_order(tmp_path):
    ra = [{"seed": s, "bill_compute": s} for s in (3, 1, 2)]
    rb = [{"seed": s, "bill_compute": 10 * s} for s in (2, 3, 1)]
    paired = load_pair(write_run(tmp_path / "a", ra), write_run(tmp_path / "b", rb))
    assert paired.a["seed"].tolist() == paired.b["seed"].tolist() == [1, 2, 3]
    # d = B - A = 10s - s = 9s ; bill_total = bill_compute here
    assert paired.differences("bill_compute", True).tolist() == [9.0, 18.0, 27.0]


def test_sign_convention_applied_once(tmp_path):
    a = write_run(tmp_path / "a", [{"seed": 1, "bill_compute": 10}, {"seed": 2, "bill_compute": 20}])
    b = write_run(tmp_path / "b", [{"seed": 1, "bill_compute": 6}, {"seed": 2, "bill_compute": 15}])
    paired = load_pair(a, b)
    # B - A = -4, -5. Higher-is-better keeps the raw sign; lower-is-better negates once.
    assert paired.differences("bill_compute", True).tolist() == [-4.0, -5.0]
    assert paired.differences("bill_compute", False).tolist() == [4.0, 5.0]


def test_bill_total_derived_in_code_and_input_not_mutated(tmp_path):
    p = write_run(
        tmp_path / "r",
        [{"seed": 1, "bill_compute": 1, "bill_memory": 2, "bill_time": 3,
          "bill_probes": 4, "bill_comm": 5, "bill_storage": 6}],
    )  # fmt: skip
    before = hashlib.sha256((p / "results.csv").read_bytes()).hexdigest()
    run = load_run(p)
    assert run.results["bill_total"].tolist() == [21.0]  # 1+2+3+4+5+6
    assert hashlib.sha256((p / "results.csv").read_bytes()).hexdigest() == before
    assert "bill_total" not in (p / "results.csv").read_text().splitlines()[0]


def test_unknown_metric_rejected(tmp_path):
    a = write_run(tmp_path / "a", rows(2))
    b = write_run(tmp_path / "b", rows(2))
    with pytest.raises(LoadError, match="unknown metric"):
        load_pair(a, b).differences("nope", True)


def test_bad_usage_json(tmp_path):
    p = write_run(tmp_path / "r", rows(2))
    (p / "usage.json").write_text("{not json")
    with pytest.raises(LoadError, match="usage.json"):
        load_run(p)


def test_usage_json_loaded(tmp_path):
    p = write_run(tmp_path / "r", rows(2))
    (p / "usage.json").write_text(json.dumps({"a": 1}))
    assert load_run(p).usage == {"a": 1}


def test_pair_runs_rejects_duplicates_even_if_load_run_was_bypassed(tmp_path):
    a = load_run(write_run(tmp_path / "a", rows(2)))
    b = load_run(write_run(tmp_path / "b", rows(2)))
    import pandas as pd

    b.results = pd.concat([b.results, b.results.iloc[[0]]], ignore_index=True)
    with pytest.raises(LoadError, match="duplicate"):
        pair_runs(a, b)


def test_pair_runs_values_are_numpy(tmp_path):
    a = load_run(write_run(tmp_path / "a", rows(2)))
    b = load_run(write_run(tmp_path / "b", rows(2)))
    va, vb = pair_runs(a, b).values("success")
    assert isinstance(va, np.ndarray) and va.dtype == float
