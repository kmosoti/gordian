"""The AUC and its cluster bootstrap (work item B5): known values, ties, invariances, and that the
quadratic form the bootstrap uses is the AUC of the resampled rows."""

import numpy as np
import pytest

from gordian_analysis.auc import auc, auc_from_weights, cluster_auc, pair_matrix


def brute(pos, neg):
    """The definition, pair by pair."""
    wins = 0.0
    for p in pos:
        for n in neg:
            wins += 1.0 if p > n else 0.5 if p == n else 0.0
    return wins / (len(pos) * len(neg))


def test_known_values():
    assert auc([3, 4], [1, 2]) == 1.0
    assert auc([1, 2], [3, 4]) == 0.0
    assert auc([1, 2], [1, 2]) == pytest.approx(0.5)
    assert auc([2, 2, 2], [2, 2]) == 0.5
    # three of four pairs won, none tied
    assert auc([2, 4], [1, 3]) == pytest.approx(0.75)
    # a tie counts half: pairs (2,1) won, (2,2) tied
    assert auc([2], [1, 2]) == pytest.approx(0.75)


def test_empty_class_has_no_auc():
    assert np.isnan(auc([], [1.0]))
    assert np.isnan(auc([1.0], []))


def test_matches_the_definition_on_random_data_with_ties():
    rng = np.random.default_rng(3)
    for _ in range(20):
        pos = rng.integers(0, 6, size=rng.integers(1, 15)).astype(float)
        neg = rng.integers(0, 6, size=rng.integers(1, 15)).astype(float)
        assert auc(pos, neg) == pytest.approx(brute(pos, neg))


def test_only_the_ranks_matter_and_reversing_the_classes_complements_it():
    rng = np.random.default_rng(5)
    pos, neg = rng.normal(0.5, 1, 40), rng.normal(0, 1, 50)
    base = auc(pos, neg)
    assert auc(np.exp(pos), np.exp(neg)) == pytest.approx(base)
    assert auc(10 * pos - 3, 10 * neg - 3) == pytest.approx(base)
    assert auc(neg, pos) == pytest.approx(1 - base)
    assert auc(-pos, -neg) == pytest.approx(1 - base)


def test_the_quadratic_form_is_the_auc_of_the_resampled_rows():
    rng = np.random.default_rng(11)
    n = 60
    cluster = rng.integers(0, 8, size=n)
    positive = rng.random(n) < 0.4
    values = rng.integers(0, 5, size=n).astype(float)
    c_mat, p_vec, n_vec, clusters = pair_matrix(values, positive, cluster)
    assert list(clusters) == sorted(set(cluster))
    # all ones: the unclustered AUC
    ones = np.ones(len(clusters))
    assert auc_from_weights(c_mat, p_vec, n_vec, ones)[0] == pytest.approx(
        auc(values[positive], values[~positive])
    )
    # an arbitrary draw: rows of cluster i repeated w[i] times
    for _ in range(10):
        w = rng.multinomial(len(clusters), np.full(len(clusters), 1 / len(clusters)))
        rows = np.concatenate([np.flatnonzero(cluster == c).repeat(k) for c, k in zip(clusters, w)])
        want = brute(values[rows][positive[rows]], values[rows][~positive[rows]]) if (
            positive[rows].any() and (~positive[rows]).any()
        ) else float("nan")
        got = auc_from_weights(c_mat, p_vec, n_vec, w)[0]
        if np.isnan(want):
            assert np.isnan(got)
        else:
            assert got == pytest.approx(want)


def test_the_interval_brackets_the_point_and_is_reproducible():
    rng = np.random.default_rng(2)
    n = 400
    cluster = rng.integers(0, 40, size=n)
    positive = rng.random(n) < 0.3
    values = rng.normal(0, 1, n) + 0.8 * positive
    a = cluster_auc(values, positive, cluster, resamples=2000, seed=9)
    b = cluster_auc(values, positive, cluster, resamples=2000, seed=9)
    assert a == b
    point, lo, hi, n_pos, n_neg = a
    assert lo < point < hi
    assert 0.5 < lo and hi < 1.0
    assert (n_pos, n_neg) == (int(positive.sum()), int((~positive).sum()))
    assert point == pytest.approx(auc(values[positive], values[~positive]))


def test_a_score_with_no_information_has_an_interval_around_one_half():
    rng = np.random.default_rng(8)
    n = 600
    cluster = rng.integers(0, 60, size=n)
    positive = rng.random(n) < 0.3
    values = rng.normal(0, 1, n)
    _, lo, hi, _, _ = cluster_auc(values, positive, cluster, resamples=3000, seed=1)
    assert lo < 0.5 < hi


def test_an_empty_class_gives_nan_everywhere():
    out = cluster_auc([1.0, 2.0], [True, True], [0, 1], resamples=10)
    assert all(np.isnan(x) for x in out[:3])
    assert out[3:] == (2, 0)
