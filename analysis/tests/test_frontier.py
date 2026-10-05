"""Quality-cost points, frontiers and the cluster bootstrap of the headroom check (R4)."""

import json

import numpy as np
import pandas as pd
import pytest
from stream_fixtures import incident, stream, write_stream_run

from gordian_analysis.frontier import (
    HEADROOM,
    NO_HEADROOM,
    ClusterData,
    arm_params,
    arm_point,
    best_at_cost,
    cheapest_reaching,
    cluster_bootstrap,
    comparison_arms,
    frontier,
    headroom_verdict,
    pareto_mask,
    points_table,
    pooled,
    stream_table,
)
from gordian_analysis.load import load_stream_run


def hard(family, ok):
    return incident(
        "hard", family, correct_by_deadline=ok, correct_declarations=1 if ok else 0
    )


def load(tmp_path, arms, roles=None):
    write_stream_run(tmp_path / "run", arms, roles)
    return load_stream_run(tmp_path / "run")


# ---- quality and cost -------------------------------------------------------------------------


def test_quality_excludes_the_slow_leak_and_is_a_ratio_of_sums(tmp_path):
    arm = [
        stream(1, [hard("compound", True), hard("slow_leak", False)], substrate_ns=1_000),
        # three hard incidents in one stream: a mean of per-stream ratios would give 0.5, 1 / 4
        # of a ratio of sums would not equal 0.5.
        stream(
            2,
            [hard("cascade", False), hard("split_brain", False), hard("compound", False)],
            substrate_ns=3_000,
        ),
        stream(3, [hard("slow_leak", True)], substrate_ns=5_000),
        stream(4, [incident("plain", correct_by_deadline=True, correct_declarations=1)]),
    ]
    run = load(tmp_path, {"a": arm})
    t = stream_table(run.arms["a"])
    assert list(t["quality_num"]) == [1, 0, 0, 0]
    assert list(t["quality_den"]) == [1, 3, 0, 0]
    assert list(t["leak_num"]) == [0, 0, 1, 0]
    assert list(t["leak_den"]) == [1, 0, 1, 0]
    p = arm_point(run.arms["a"])
    assert p["quality"] == pytest.approx(1 / 4)
    assert p["quality_incidents"] == 4
    assert p["leak_rate"] == pytest.approx(1 / 2)
    # Cost is the mean of the per-stream total over all streams, streams without hard
    # incidents included.
    assert p["cost_ns"] == pytest.approx(np.mean(t["total_cost_ns"]))


def test_pooled_ratio_is_nan_for_no_incident_and_not_zero():
    assert np.isnan(pooled([0, 0], [0, 0]))
    assert pooled([1, 0], [1, 3]) == 0.25


def test_the_other_columns_sit_beside_quality_and_are_not_folded_into_it(tmp_path):
    arm = [
        stream(
            1,
            [
                hard("compound", True),
                incident("plain", correct_by_deadline=True, correct_declarations=1),
                incident("plain", critical=True, wrong_declarations=2),
                incident("decoy", wrong_declarations=1),
            ],
        ),
    ]
    p = arm_point(load(tmp_path, {"a": arm}).arms["a"])
    assert p["quality"] == 1.0
    assert p["plain_rate"] == 0.5
    assert p["critical_misses"] == 1
    assert p["critical_miss_rate"] == 1.0
    assert p["wrong_per_stream"] == 2.0
    assert p["false_alarms_per_stream"] == 1.0
    assert p["decoys_alarmed"] == 1 and p["decoys"] == 1


# ---- frontiers --------------------------------------------------------------------------------


def test_pareto_mask_keeps_the_points_nothing_beats():
    quality = [0.5, 0.7, 0.7, 0.9, 0.6, 0.9, np.nan]
    cost = [1.0, 2.0, 3.0, 5.0, 4.0, 5.0, 0.1]
    # (0.7, 3) is beaten by (0.7, 2); (0.6, 4) by (0.7, 2); the tie (0.9, 5) twice is kept; NaN out.
    assert list(pareto_mask(quality, cost)) == [True, True, False, True, False, True, False]


def test_frontier_is_per_family_or_over_everything():
    pts = pd.DataFrame(
        {
            "arm": ["a1", "a2", "a3", "b1", "b2"],
            "policy": ["a", "a", "a", "b", "b"],
            "quality": [0.2, 0.5, 0.4, 0.45, 0.9],
            "cost_ns": [1.0, 2.0, 3.0, 2.5, 9.0],
        }
    )
    per = frontier(pts, by="policy")
    # within a family a3 is beaten by a2; b1 and b2 do not beat each other
    assert sorted(per["arm"]) == ["a1", "a2", "b1", "b2"]
    assert set(per["frontier"]) == {"a", "b"}
    # over everything b1 (0.45 at 2.5) is also beaten by a2 (0.5 at 2.0)
    assert sorted(frontier(pts, by=None)["arm"]) == ["a1", "a2", "b2"]


def test_frontier_over_everything_drops_a_point_a_cheaper_better_one_beats():
    pts = pd.DataFrame(
        {
            "arm": ["x", "y", "z"],
            "policy": ["p", "q", "q"],
            "quality": [0.6, 0.5, 0.9],
            "cost_ns": [1.0, 2.0, 3.0],
        }
    )
    assert sorted(frontier(pts, by=None)["arm"]) == ["x", "z"]
    assert sorted(frontier(pts, by="policy")["arm"]) == ["x", "y", "z"]


def test_best_at_cost_and_cheapest_reaching():
    pts = pd.DataFrame(
        {
            "arm": ["a", "b", "c", "d"],
            "quality": [0.3, 0.6, 0.6, 0.95],
            "cost_ns": [0.0, 2.0, 1.0, 10.0],
        }
    )
    assert best_at_cost(pts, 1.5)["arm"] == "c"  # b is not affordable; c ties b's quality
    assert best_at_cost(pts, 2.0)["arm"] == "c"  # the cheaper of two equal ones
    assert best_at_cost(pts, 0.0)["arm"] == "a"
    assert best_at_cost(pts.assign(cost_ns=pts.cost_ns + 1), 0.5) is None
    assert cheapest_reaching(pts, 0.6)["arm"] == "c"
    assert cheapest_reaching(pts, 0.99) is None


# ---- loading a run ----------------------------------------------------------------------------


def test_points_table_carries_each_arms_policy_and_parameters(tmp_path):
    arms = {
        "never": [stream(1, [hard("compound", False)])],
        "rand": [stream(1, [hard("compound", True)])],
        "orc": [stream(1, [hard("compound", True)])],
    }
    roles = {"orc": "privileged"}
    run = load(tmp_path, arms, roles)
    (tmp_path / "run" / "manifest.json").write_text(
        json.dumps(
            {
                "arms": [
                    {"arm": "never", "policy": "never_escalate"},
                    {
                        "arm": "rand",
                        "policy": {"policy": "random_escalation", "p": 0.3, "delay_ns": 4},
                    },
                    {"arm": "orc", "policy": "oracle_escalation"},
                ]
            }
        )
    )
    run = load_stream_run(tmp_path / "run")
    assert arm_params(run.manifest)["rand"] == {"policy": "random_escalation", "p": 0.3, "delay_ns": 4}
    pts = points_table(run).set_index("arm")
    assert pts.loc["rand", "p"] == 0.3 and pts.loc["rand", "delay_ns"] == 4
    assert pts.loc["never", "policy"] == "never_escalate" and np.isnan(pts.loc["never", "p"])
    assert pts.loc["orc", "role"] == "privileged"
    assert comparison_arms(run) == ["never", "rand"]


# ---- the cluster bootstrap --------------------------------------------------------------------


def arm_of(values, cost=1_000, per_stream=1):
    """Streams whose hard incidents (compound) are all correct or all wrong, per `values`."""
    return [
        stream(
            100 + k,
            [hard("compound", bool(v)) for _ in range(per_stream)],
            substrate_ns=cost,
        )
        for k, v in enumerate(values)
    ]


def test_a_known_gap_with_identical_streams_has_a_degenerate_interval(tmp_path):
    n = 12
    run = load(
        tmp_path,
        {
            "orc": arm_of([1] * n, cost=1_000),
            "cheap": arm_of([0, 1] * (n // 2), cost=1_000),  # affordable, quality 0.5
            "dear_good": arm_of([1] * n, cost=8_000),  # reaches the oracle's quality at 8x
            "dear_better": arm_of([1] * n, cost=9_000),
            "unaffordable_mid": arm_of([1] * (n - 1) + [0], cost=2_000),  # 11/12, not affordable
        },
        {"orc": "privileged"},
    )
    data = ClusterData.from_arms(run.arms)
    est = cluster_bootstrap(
        data,
        "orc",
        ["cheap", "dear_good", "dear_better", "unaffordable_mid"],
        seed=1,
        n_resamples=300,
    )
    assert est.oracle_quality == 1.0 and est.best_arm == "cheap"
    assert est.gap == pytest.approx(0.5)
    # the oracle's quality is 1 in every resample, so the gap is 1 - the cheap arm's resampled
    # quality, which varies; it is centred near 0.5 and brackets it.
    assert est.gap_low < 0.5 < est.gap_high
    # the cheapest arm reaching quality 1.0 is dear_good (cost 8,000 + 0), in every resample where
    # the oracle's resampled quality is 1.0, which is all of them
    assert est.reach_arm == "dear_good"
    assert est.reach_cost_ns == pytest.approx(8_000)
    assert est.reach_share == 1.0
    assert est.no_affordable_share == 0.0
    assert est.n_streams == n and est.seed == 1


def test_the_interval_resamples_streams_not_incidents(tmp_path):
    # Forty streams of five hard incidents each, all right or all wrong together in the baseline:
    # a bootstrap over incidents would treat 200 independent trials; one over streams has 40.
    n, per = 40, 5
    rng = np.random.default_rng(3)
    base = (rng.random(n) < 0.5).astype(int)
    run = load(
        tmp_path,
        {
            "orc": arm_of([1] * n, per_stream=per),
            "base": arm_of(list(base), per_stream=per),
        },
        {"orc": "privileged"},
    )
    data = ClusterData.from_arms(run.arms)
    est = cluster_bootstrap(data, "orc", ["base"], seed=7, n_resamples=4000)
    p = base.mean()
    se_streams = np.sqrt(p * (1 - p) / n)
    width = est.gap_high - est.gap_low
    assert width == pytest.approx(2 * 1.645 * se_streams, rel=0.12)
    assert width > 2 * 1.645 * se_streams / np.sqrt(per) * 1.8  # far wider than the incident-level one
    assert est.gap == pytest.approx(1 - p)


def test_the_same_seed_gives_the_same_interval_and_another_does_not(tmp_path):
    run = load(
        tmp_path,
        {"orc": arm_of([1] * 30), "b": arm_of(list(np.random.default_rng(1).integers(0, 2, 30)))},
        {"orc": "privileged"},
    )
    data = ClusterData.from_arms(run.arms)
    a = cluster_bootstrap(data, "orc", ["b"], seed=5, n_resamples=500)
    b = cluster_bootstrap(data, "orc", ["b"], seed=5, n_resamples=500)
    c = cluster_bootstrap(data, "orc", ["b"], seed=6, n_resamples=500)
    assert repr(a) == repr(b)  # repr, because an interval that does not exist is NaN
    assert (a.gap_low, a.gap_high) != (c.gap_low, c.gap_high)


def test_affordability_is_judged_on_the_resampled_oracle_cost(tmp_path):
    # Streams alternate between a cheap and a dear oracle cost; the baseline costs 1,500 always.
    # It is affordable when the resampled mean oracle cost is at least 1,500 and not otherwise,
    # so over resamples the baseline is sometimes the only affordable arm and sometimes there is
    # only `never` (cost 0, quality 0).
    n = 20
    orc_streams = [
        stream(100 + k, [hard("compound", True)], substrate_ns=500 if k % 2 == 0 else 3_000)
        for k in range(n)
    ]
    run = load(
        tmp_path,
        {
            "orc": orc_streams,
            "never": arm_of([0] * n, cost=0),
            "mid": arm_of([1] * n, cost=1_500),
        },
        {"orc": "privileged"},
    )
    data = ClusterData.from_arms(run.arms)
    est = cluster_bootstrap(data, "orc", ["never", "mid"], seed=2, n_resamples=2000)
    assert est.oracle_cost_ns == pytest.approx(1_750)
    assert est.best_arm == "mid" and est.gap == 0.0
    share_mid = est.best_arm_share.get("mid", 0.0)
    assert 0.0 < share_mid < 1.0
    assert share_mid + est.best_arm_share.get("never", 0.0) == pytest.approx(1.0)
    assert est.gap_high == pytest.approx(1.0)  # when only `never` is affordable the gap is 1


def test_streams_must_match(tmp_path):
    run = load(tmp_path, {"a": arm_of([1, 0, 1]), "b": arm_of([1, 0, 1])})
    run.arms["b"].results = run.arms["b"].results.assign(seed=lambda d: d["seed"] + 1)
    run.arms["b"].incidents = run.arms["b"].incidents.assign(seed=lambda d: d["seed"] + 1)
    with pytest.raises(ValueError, match="other streams"):
        ClusterData.from_arms(run.arms)


# ---- the verdict ------------------------------------------------------------------------------


def test_the_verdict_applies_the_margin_and_the_lower_bound_it_is_given(tmp_path):
    n = 40
    run = load(
        tmp_path,
        {"orc": arm_of([1] * n), "b": arm_of([1] * 10 + [0] * 30), "c": arm_of([1] * 36 + [0] * 4)},
        {"orc": "privileged"},
    )
    data = ClusterData.from_arms(run.arms)
    wide = cluster_bootstrap(data, "orc", ["b"], seed=1, n_resamples=2000)  # gap 0.75
    assert headroom_verdict(wide) == HEADROOM
    assert headroom_verdict(wide, margin=0.80) == NO_HEADROOM
    assert headroom_verdict(wide, lower_bound=0.70) == NO_HEADROOM
    close = cluster_bootstrap(data, "orc", ["c"], seed=1, n_resamples=2000)  # gap 0.10
    assert close.gap == pytest.approx(0.10)
    assert headroom_verdict(close) == NO_HEADROOM  # the interval's lower limit is below 0.05
    assert headroom_verdict(close, margin=0.05, lower_bound=0.0) == HEADROOM
