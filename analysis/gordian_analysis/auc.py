"""The area under the ROC curve of a score between two classes, and its cluster bootstrap (work item B5).

Defined as the probability that a randomly drawn positive scores higher than a randomly drawn negative,
a tie counting one half (the Mann-Whitney U statistic over the number of pairs). An AUC of 0.5 is no
separation, 1 is perfect, and below 0.5 the score ranks the classes the other way round (the feature's
sign is reversed). It is a function of the ranks only: any strictly increasing transform of the score
leaves it unchanged, so the scale a feature is written on does not matter to it.

Rows of one stream are not independent (they share a service graph, a noise process and a set of
incidents), so the interval resamples whole streams. The resampled AUC is a ratio of two sums over
pairs of streams, so it is computed from one matrix of pair counts and each resample's multinomial
weights, not by recomputing ranks: with `C[a, b]` the number of (positive in stream a, negative in
stream b) pairs the positive wins, ties counting one half, `P[a]` the positives of stream a and
`N[b]` the negatives of stream b, and `w` the number of times each stream is drawn,

    AUC(w) = (w' C w) / ((w . P) (w . N)).

`pair_matrix` and `cluster_auc` compute exactly that; `auc` is the unclustered statistic and the
two agree for `w` of all ones.
"""

from __future__ import annotations

import numpy as np


def auc(positive, negative) -> float:
    """The probability that a positive outscores a negative, ties counting one half; `nan` if either
    class is empty."""
    p = np.asarray(positive, float)
    n = np.asarray(negative, float)
    if len(p) == 0 or len(n) == 0:
        return float("nan")
    n_sorted = np.sort(n)
    less = np.searchsorted(n_sorted, p, side="left")
    less_or_equal = np.searchsorted(n_sorted, p, side="right")
    wins = less.sum() + 0.5 * (less_or_equal - less).sum()
    return float(wins / (len(p) * len(n)))


def pair_matrix(values, positive, cluster):
    """`(C, P, N, clusters)` for the rows with a score in `values`, a boolean class `positive`, and a
    cluster label per row. `clusters` is the sorted array of labels; row and column `i` of `C` is
    `clusters[i]`."""
    values = np.asarray(values, float)
    positive = np.asarray(positive, bool)
    cluster = np.asarray(cluster)
    clusters = np.unique(cluster)
    pos = [np.sort(values[(cluster == c) & positive]) for c in clusters]
    neg = [np.sort(values[(cluster == c) & ~positive]) for c in clusters]
    k = len(clusters)
    c_mat = np.zeros((k, k))
    for a in range(k):
        if len(pos[a]) == 0:
            continue
        for b in range(k):
            if len(neg[b]) == 0:
                continue
            less = np.searchsorted(neg[b], pos[a], side="left")
            less_or_equal = np.searchsorted(neg[b], pos[a], side="right")
            c_mat[a, b] = less.sum() + 0.5 * (less_or_equal - less).sum()
    p_vec = np.array([len(x) for x in pos], float)
    n_vec = np.array([len(x) for x in neg], float)
    return c_mat, p_vec, n_vec, clusters


def auc_from_weights(c_mat, p_vec, n_vec, weights):
    """The AUC of the rows when cluster `i` is drawn `weights[..., i]` times (a vector or a matrix of
    resamples by clusters); `nan` where a resample holds no positive or no negative."""
    w = np.atleast_2d(np.asarray(weights, float))
    num = np.einsum("ra,ab,rb->r", w, c_mat, w)
    den = (w @ p_vec) * (w @ n_vec)
    with np.errstate(divide="ignore", invalid="ignore"):
        out = np.where(den > 0, num / den, np.nan)
    return out


def cluster_auc(values, positive, cluster, resamples=10_000, seed=0, chunk=1000, lo=0.05, hi=0.95):
    """`(point, lower, upper, positives, negatives)`: the AUC, its 90% (by default) equal-tailed
    percentile interval over resamples of whole clusters, and the class sizes. The interval is `nan`
    when a class is empty. Resamples without a positive or without a negative are dropped (they have no
    AUC); with enough clusters there are none."""
    values = np.asarray(values, float)
    positive = np.asarray(positive, bool)
    n_pos, n_neg = int(positive.sum()), int((~positive).sum())
    if n_pos == 0 or n_neg == 0:
        return float("nan"), float("nan"), float("nan"), n_pos, n_neg
    c_mat, p_vec, n_vec, clusters = pair_matrix(values, positive, cluster)
    k = len(clusters)
    point = float(auc_from_weights(c_mat, p_vec, n_vec, np.ones(k))[0])
    rng = np.random.default_rng(seed)
    draws = []
    done = 0
    while done < resamples:
        m = min(chunk, resamples - done)
        w = rng.multinomial(k, np.full(k, 1.0 / k), size=m).astype(float)
        draws.append(auc_from_weights(c_mat, p_vec, n_vec, w))
        done += m
    x = np.concatenate(draws)
    x = x[~np.isnan(x)]
    if len(x) == 0:
        return point, float("nan"), float("nan"), n_pos, n_neg
    return (point, float(np.quantile(x, lo, method="lower")),
            float(np.quantile(x, hi, method="higher")), n_pos, n_neg)
