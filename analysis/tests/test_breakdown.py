import math

import pytest

from gordian_analysis.breakdown import (
    coverage_error_table,
    paired_class_table,
    per_class_table,
    risk_coverage_curve,
)
from gordian_analysis.load import load_pair, load_run

from conftest import write_run


def test_per_class_table_by_hand(tmp_path):
    # class x: success 1,0,1 -> mean 2/3, sd = sqrt(((1/3)^2*2 + (2/3)^2)/2) = sqrt(1/3) = 0.57735
    # class y: success 1 only -> mean 1, sd NaN (n = 1)
    p = write_run(
        tmp_path / "r",
        [
            {"seed": 1, "class": "x", "success": 1},
            {"seed": 2, "class": "x", "success": 0},
            {"seed": 3, "class": "x", "success": 1},
            {"seed": 1, "class": "y", "success": 1},
        ],
    )
    t = per_class_table(load_run(p).results, ["success"]).set_index("class")
    assert t.loc["x", "n"] == 3
    assert t.loc["x", "mean"] == pytest.approx(2 / 3)
    assert t.loc["x", "sd"] == pytest.approx(math.sqrt(1 / 3))
    assert t.loc["y", "n"] == 1 and t.loc["y", "mean"] == 1.0 and math.isnan(t.loc["y", "sd"])


def test_coverage_and_error_among_answered(tmp_path):
    # class x: 4 episodes, 1 abstained -> coverage 3/4. Answered successes: 1,1,0 -> error 1/3.
    #   (the abstained episode has success=0 and must not count as an error)
    # class y: 2 episodes, both abstained -> coverage 0, error NaN.
    p = write_run(
        tmp_path / "r",
        [
            {"seed": 1, "class": "x", "success": 1},
            {"seed": 2, "class": "x", "success": 1},
            {"seed": 3, "class": "x", "success": 0},
            {"seed": 4, "class": "x", "success": 0, "abstained": 1},
            {"seed": 1, "class": "y", "success": 0, "abstained": 1},
            {"seed": 2, "class": "y", "success": 0, "abstained": "true"},
        ],
    )
    t = coverage_error_table(load_run(p).results).set_index("class")
    assert t.loc["x", "coverage"] == pytest.approx(0.75)
    assert t.loc["x", "error_rate_answered"] == pytest.approx(1 / 3)
    assert t.loc["y", "coverage"] == 0.0 and math.isnan(t.loc["y", "error_rate_answered"])
    # pooled: 6 episodes, 3 answered -> coverage 0.5, error 1/3
    assert t.loc["ALL", "coverage"] == pytest.approx(0.5)
    assert t.loc["ALL", "error_rate_answered"] == pytest.approx(1 / 3)


def test_risk_coverage_curve_by_hand(tmp_path):
    # confidence .9 (ok), .8 (err), .8 (ok), .5 (ok), .3 (abstained, not answered). N = 5.
    #   threshold .9: accepted 1 -> coverage 1/5, risk 0
    #   threshold .8: accepted 3 -> coverage 3/5, risk 1/3   (ties enter together)
    #   threshold .5: accepted 4 -> coverage 4/5, risk 1/4
    p = write_run(
        tmp_path / "r",
        [
            {"seed": 1, "success": 1, "confidence": 0.9},
            {"seed": 2, "success": 0, "confidence": 0.8},
            {"seed": 3, "success": 1, "confidence": 0.8},
            {"seed": 4, "success": 1, "confidence": 0.5},
            {"seed": 5, "success": 0, "confidence": 0.3, "abstained": 1},
        ],
        extra_cols=("confidence",),
    )
    c = risk_coverage_curve(load_run(p).results)
    assert c["threshold"].tolist() == [0.9, 0.8, 0.5]
    assert c["n_accepted"].tolist() == [1, 3, 4]
    assert c["coverage"].tolist() == pytest.approx([0.2, 0.6, 0.8])
    assert c["risk"].tolist() == pytest.approx([0.0, 1 / 3, 0.25])


def test_risk_coverage_requires_confidence_column(tmp_path):
    p = write_run(tmp_path / "r", [{"seed": 1}])
    with pytest.raises(KeyError):
        risk_coverage_curve(load_run(p).results)


def test_risk_coverage_order_independent(tmp_path):
    base = [{"seed": i, "success": i % 2, "confidence": 0.5 + 0.1 * (i % 3)} for i in range(1, 10)]
    a = risk_coverage_curve(
        load_run(write_run(tmp_path / "a", base, extra_cols=("confidence",))).results
    )
    b = risk_coverage_curve(
        load_run(write_run(tmp_path / "b", base[::-1], extra_cols=("confidence",))).results
    )
    assert a.equals(b)


def test_paired_class_table_by_hand(tmp_path):
    # cost lower-is-better. class x: A = 10, 20 ; B = 6, 15 -> mean_a 15, mean_b 10.5,
    # d = A - B = 4, 5 -> mean_d 4.5.
    a = write_run(
        tmp_path / "a",
        [{"seed": 1, "class": "x", "bill_compute": 10}, {"seed": 2, "class": "x", "bill_compute": 20}],
    )
    b = write_run(
        tmp_path / "b",
        [{"seed": 1, "class": "x", "bill_compute": 6}, {"seed": 2, "class": "x", "bill_compute": 15}],
    )
    t = paired_class_table(load_pair(a, b), "bill_compute", higher_is_better=False)
    row = t.iloc[0]
    assert (row["n"], row["mean_a"], row["mean_b"], row["mean_d"]) == (2, 15.0, 10.5, 4.5)
