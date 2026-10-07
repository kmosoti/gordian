"""Sample efficiency and the energy proxy (W1), on hand-written rows.

Every expected value below is worked out in the comment beside it. The frames carry only the
columns the measures read; the real `results.csv` has those columns among others.
"""

import math

import numpy as np
import pandas as pd
import pytest

from gordian_analysis import measures
from gordian_analysis.measures import (
    PLACEHOLDER_LABEL,
    PLACEHOLDER_WATTS_CHEAP,
    PLACEHOLDER_WATTS_REASONER,
    energy_proxy,
    outcome_slope,
    sample_efficiency,
)


def frame(rows):
    return pd.DataFrame(rows)


def streams(*triples, tier="hard"):
    """(seed, incidents, correct) per stream."""
    return frame(
        [
            {"seed": s, f"incidents_{tier}": i, f"correct_{tier}": c}
            for s, i, c in triples
        ]
    )


# ---- sample efficiency --------------------------------------------------------------------


def test_cumulative_curve_by_hand_in_stream_order_not_row_order():
    # Rows given out of order; stream order is by seed: 1, 2, 3, 4.
    #   seed  incidents  correct   cum_inc  cum_cor  efficiency
    #   1     0          0         0        0        NaN (nothing seen yet)
    #   2     3          2         3        2        2/3
    #   3     2          1         5        3        3/5
    #   4     5          5         10       8        8/10
    r = sample_efficiency(streams((3, 2, 1), (1, 0, 0), (4, 5, 5), (2, 3, 2)))
    c = r.curve
    assert list(c["key"]) == [1, 2, 3, 4]
    assert list(c["stream"]) == [1, 2, 3, 4]
    assert list(c["cum_incidents"]) == [0, 3, 5, 10]
    assert list(c["cum_correct"]) == [0, 2, 3, 8]
    assert math.isnan(c["efficiency"].iloc[0])
    assert c["efficiency"].iloc[1:].tolist() == pytest.approx([2 / 3, 3 / 5, 8 / 10])
    assert (r.streams, r.incidents, r.correct) == (4, 10, 8)
    assert r.efficiency == pytest.approx(0.8)
    assert r.at(2) == pytest.approx(2 / 3)
    assert r.at(4) == pytest.approx(0.8)


def test_the_ratio_is_pooled_not_a_mean_of_per_stream_ratios():
    # Stream ratios are 1/1 and 1/9: a mean of ratios is 0.5556, the pooled ratio is 2/10.
    r = sample_efficiency(streams((1, 1, 1), (2, 9, 1)))
    assert r.efficiency == pytest.approx(0.2)
    assert r.efficiency != pytest.approx(np.mean([1.0, 1 / 9]))


def test_halves_are_the_first_and_last_n_over_two_streams():
    # Four streams: first half = streams 1-2 (1 of 4), second half = streams 3-4 (6 of 6).
    r = sample_efficiency(streams((1, 2, 0), (2, 2, 1), (3, 3, 3), (4, 3, 3)))
    assert r.first_half == pytest.approx(1 / 4)
    assert r.second_half == pytest.approx(1.0)
    # Five streams: the middle one is in neither half.
    r5 = sample_efficiency(streams((1, 1, 0), (2, 1, 0), (3, 100, 100), (4, 1, 1), (5, 1, 1)))
    assert r5.first_half == pytest.approx(0.0)
    assert r5.second_half == pytest.approx(1.0)
    # A half with no incident has no ratio.
    r0 = sample_efficiency(streams((1, 0, 0), (2, 0, 0), (3, 1, 1), (4, 1, 0)))
    assert math.isnan(r0.first_half)
    assert r0.second_half == pytest.approx(0.5)


def test_a_constant_rate_gives_a_flat_curve_and_equal_halves():
    r = sample_efficiency(streams(*[(s, 4, 1) for s in range(10)]))
    assert r.curve["efficiency"].tolist() == pytest.approx([0.25] * 10)
    assert r.first_half == pytest.approx(r.second_half)


def test_a_learning_arm_has_a_rising_curve_and_a_larger_second_half():
    r = sample_efficiency(streams(*[(s, 4, 0 if s < 5 else 4) for s in range(10)]))
    assert r.first_half == pytest.approx(0.0)
    assert r.second_half == pytest.approx(1.0)
    assert r.curve["efficiency"].is_monotonic_increasing


def test_tier_selects_the_columns_and_all_adds_plain_and_hard():
    df = frame(
        [
            {"seed": 1, "incidents_hard": 2, "correct_hard": 1, "incidents_plain": 8, "correct_plain": 6},
            {"seed": 2, "incidents_hard": 3, "correct_hard": 3, "incidents_plain": 7, "correct_plain": 5},
        ]
    )  # fmt: skip
    assert sample_efficiency(df).efficiency == pytest.approx(4 / 5)
    assert sample_efficiency(df, "plain").efficiency == pytest.approx(11 / 15)
    assert sample_efficiency(df, "all").efficiency == pytest.approx(15 / 20)
    assert sample_efficiency(df, "all").curve["cum_incidents"].tolist() == [10, 20]


def test_order_can_be_another_column():
    df = streams((1, 2, 2), (2, 2, 0)).assign(block=[9, 3])
    r = sample_efficiency(df, order_by="block")
    assert list(r.curve["key"]) == [3, 9]
    assert list(r.curve["cum_correct"]) == [0, 2]


def test_invalid_inputs_are_refused_not_repaired():
    with pytest.raises(ValueError, match="more correct decisions than incidents"):
        sample_efficiency(streams((1, 1, 2)))
    with pytest.raises(ValueError, match="uniquely"):
        sample_efficiency(streams((1, 1, 1), (1, 1, 0)))
    with pytest.raises(ValueError, match="lack the columns"):
        sample_efficiency(streams((1, 1, 1)), tier="plain")
    with pytest.raises(ValueError, match="negative"):
        sample_efficiency(streams((1, 1, -1)))
    with pytest.raises(ValueError, match="missing"):
        sample_efficiency(streams((1, 1, float("nan"))))
    with pytest.raises(ValueError, match="tier must be"):
        sample_efficiency(streams((1, 1, 1)), tier="decoy")
    with pytest.raises(ValueError):
        sample_efficiency(streams((1, 1, 1))).at(0)
    with pytest.raises(ValueError):
        sample_efficiency(streams((1, 1, 1))).at(2)


# ---- energy proxy -------------------------------------------------------------------------


def cost_frame():
    # Two streams. Total cost: 1,000,000,500 and 2,000 ns; the reasoner's part: 1,000,000,000
    # and 0. So the cheap remainder is 500 + 2,000 = 2,500 ns and the reasoner's is 1e9 ns.
    # Correct: stream 1 has 1 hard and 2 plain, stream 2 has 0 hard and 3 plain (6 in all).
    return frame(
        [
            {"seed": 1, "total_cost_ns": 1_000_000_500, "reasoner_cost_ns": 1_000_000_000,
             "correct_hard": 1, "incidents_hard": 2, "correct_plain": 2, "incidents_plain": 4},
            {"seed": 2, "total_cost_ns": 2_000, "reasoner_cost_ns": 0,
             "correct_hard": 0, "incidents_hard": 1, "correct_plain": 3, "incidents_plain": 3},
        ]
    )  # fmt: skip


def test_nanoseconds_per_correct_decision_by_hand():
    e = energy_proxy(cost_frame())
    assert (e.ns_cheap, e.ns_reasoner, e.ns_total) == (2_500, 1_000_000_000, 1_000_002_500)
    assert e.correct == 6
    assert e.ns_per_correct == pytest.approx(1_000_002_500 / 6)
    # Hard only: one correct hard decision carries the whole cost, including what was spent on
    # the plain incidents.
    h = energy_proxy(cost_frame(), "hard")
    assert h.correct == 1
    assert h.ns_per_correct == pytest.approx(1_000_002_500)
    p = energy_proxy(cost_frame(), "plain")
    assert p.correct == 5
    assert p.ns_per_correct == pytest.approx(1_000_002_500 / 5)


def test_joules_follow_the_declared_conversion_and_carry_its_label():
    e = energy_proxy(cost_frame())
    # 2,500 ns * 1e-9 s/ns * 10 W = 2.5e-5 J; 1e9 ns * 1e-9 * 1,000 W = 1,000 J.
    assert e.joules_cheap == pytest.approx(2.5e-5)
    assert e.joules_reasoner == pytest.approx(1000.0)
    assert e.joules_total == pytest.approx(1000.000025)
    assert e.joules_per_correct == pytest.approx(1000.000025 / 6)
    assert (PLACEHOLDER_WATTS_CHEAP, PLACEHOLDER_WATTS_REASONER) == (10.0, 1000.0)
    assert e.assumptions == PLACEHOLDER_LABEL
    assert "PLACEHOLDER" in e.assumptions and "assumption" in e.assumptions
    assert "PLACEHOLDER" in measures.__doc__ or "placeholder" in measures.__doc__


def test_the_conversion_is_a_parameter_and_changes_only_the_joules():
    base = energy_proxy(cost_frame())
    e = energy_proxy(cost_frame(), watts_cheap=20.0, watts_reasoner=500.0)
    assert e.joules_cheap == pytest.approx(2 * base.joules_cheap)
    assert e.joules_reasoner == pytest.approx(base.joules_reasoner / 2)
    assert e.ns_per_correct == base.ns_per_correct
    assert "20 W" in e.assumptions and "500 W" in e.assumptions
    zero = energy_proxy(cost_frame(), watts_cheap=0.0, watts_reasoner=0.0)
    assert zero.joules_total == 0.0 and zero.ns_total == base.ns_total


# ---- improvement per unit experience: the slope of an outcome against incidents seen -----------


def seen(*per_stream):
    """Incidents in the order seen, from the outcomes of each stream (a list per stream)."""
    rows = [
        {"stream": s, "y": float(y)} for s, ys in enumerate(per_stream, start=1) for y in ys
    ]
    return pd.DataFrame(rows)


def test_slope_by_hand_four_incidents():
    # positions 1..4, y = 0, 0, 1, 1: mean x 2.5, mean y 0.5, sum (x - 2.5)(y - 0.5) = 2.0,
    # sum (x - 2.5)^2 = 5.0, so the slope is 0.4 per incident, 40 per 100.
    r = outcome_slope(seen([0, 0], [1, 1]), [1, 2], resamples=200)
    assert r.incidents == 4
    assert r.slope_per_100 == pytest.approx(40.0)


def test_slope_sign_follows_the_order():
    up = outcome_slope(seen([0, 0], [1, 1]), [1, 2], resamples=50)
    down = outcome_slope(seen([1, 1], [0, 0]), [1, 2], resamples=50)
    assert up.slope_per_100 == pytest.approx(-down.slope_per_100)
    assert down.slope_per_100 < 0


def test_a_constant_outcome_has_slope_zero_and_a_degenerate_interval():
    r = outcome_slope(seen([1, 1, 1], [1, 1], [1, 1, 1, 1]), [1, 2, 3], resamples=200)
    assert r.slope_per_100 == pytest.approx(0.0, abs=1e-9)
    assert r.lower == pytest.approx(0.0, abs=1e-9)
    assert r.higher == pytest.approx(0.0, abs=1e-9)


def test_the_window_is_the_first_streams_and_positions_are_the_runs_own():
    # Streams 1 and 2 are all wrong, stream 3 all right. Over streams 1 and 2 the slope is zero;
    # over all three it is the slope of y = 0 0 0 0 1 1 at positions 1..6.
    data = seen([0, 0], [0, 0], [1, 1])
    first_two = outcome_slope(data, [1, 2], resamples=50)
    assert first_two.incidents == 4
    assert first_two.slope_per_100 == pytest.approx(0.0, abs=1e-9)
    all_three = outcome_slope(data, [1, 2, 3], resamples=50)
    xs = np.arange(1, 7.0)
    ys = np.array([0, 0, 0, 0, 1, 1.0])
    want = ((xs - xs.mean()) * (ys - ys.mean())).sum() / ((xs - xs.mean()) ** 2).sum()
    assert all_three.incidents == 6
    assert all_three.slope_per_100 == pytest.approx(100 * want)


def test_a_stream_with_no_incident_is_a_cluster_that_can_be_resampled():
    data = seen([0, 0], [], [1, 1])
    r = outcome_slope(data, [1, 2, 3], resamples=300, seed=1)
    assert r.incidents == 4
    assert r.slope_per_100 == pytest.approx(40.0)


def test_the_interval_brackets_a_real_trend_and_straddles_zero_without_one():
    rng = np.random.default_rng(3)
    per = [list((rng.random(4) < 0.2 + 0.7 * s / 59).astype(int)) for s in range(60)]
    r = outcome_slope(seen(*per), list(range(1, 61)), resamples=2000, seed=5)
    assert r.lower < r.slope_per_100 < r.higher
    assert r.lower > 0, "a strong trend over 240 incidents is distinguishable from none"
    flat = [list((rng.random(4) < 0.5).astype(int)) for _ in range(60)]
    f = outcome_slope(seen(*flat), list(range(1, 61)), resamples=2000, seed=5)
    assert f.lower < 0 < f.higher


def test_the_interval_is_reproducible_by_seed():
    data = seen([0, 1, 0], [1], [1, 1, 0, 1], [0, 0])
    a = outcome_slope(data, [1, 2, 3, 4], resamples=500, seed=9)
    assert a == outcome_slope(data, [1, 2, 3, 4], resamples=500, seed=9)
    c = outcome_slope(data, [1, 2, 3, 4], resamples=500, seed=10)
    assert (a.lower, a.higher) != (c.lower, c.higher)


def test_outcome_slope_refuses_what_it_cannot_read():
    with pytest.raises(ValueError, match="columns"):
        outcome_slope(pd.DataFrame({"y": [1.0]}), [1])
    with pytest.raises(ValueError, match="0 or 1"):
        outcome_slope(pd.DataFrame({"stream": [1], "y": [2.0]}), [1])
    r = outcome_slope(seen([1]), [1], resamples=20)
    assert math.isnan(r.slope_per_100) and math.isnan(r.lower)


def test_an_arm_that_never_calls_the_reasoner_is_all_cheap():
    df = cost_frame().assign(reasoner_cost_ns=0)
    e = energy_proxy(df)
    assert e.ns_reasoner == 0 and e.ns_cheap == e.ns_total == 1_000_002_500
    assert e.joules_reasoner == 0.0
    assert e.joules_total == pytest.approx(1_000_002_500 * 1e-9 * 10.0)


def test_no_correct_decision_has_no_cost_per_decision():
    df = cost_frame().assign(correct_hard=0, correct_plain=0)
    e = energy_proxy(df)
    assert e.correct == 0
    assert math.isnan(e.ns_per_correct) and math.isnan(e.joules_per_correct)
    assert e.ns_total == 1_000_002_500  # the totals are still reported


def test_energy_proxy_refuses_inconsistent_rows():
    with pytest.raises(ValueError, match="exceeds its total"):
        energy_proxy(cost_frame().assign(reasoner_cost_ns=[2_000_000_000, 0]))
    with pytest.raises(ValueError, match="non-negative"):
        energy_proxy(cost_frame(), watts_cheap=-1.0)
    with pytest.raises(ValueError, match="lack the columns"):
        energy_proxy(cost_frame().drop(columns="reasoner_cost_ns"))
    with pytest.raises(ValueError, match="more correct decisions"):
        energy_proxy(cost_frame().assign(correct_hard=[9, 0]))


def test_two_arms_compare_on_cost_per_correct_decision_not_on_cost():
    # B is dearer in total but answers four times as many incidents: cheaper per decision.
    a = frame([{"total_cost_ns": 100, "reasoner_cost_ns": 0, "correct_hard": 1, "incidents_hard": 4}])
    b = frame([{"total_cost_ns": 200, "reasoner_cost_ns": 100, "correct_hard": 4, "incidents_hard": 4}])
    assert energy_proxy(a, "hard").ns_per_correct == pytest.approx(100.0)
    assert energy_proxy(b, "hard").ns_per_correct == pytest.approx(50.0)
