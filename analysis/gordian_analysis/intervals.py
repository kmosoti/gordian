"""Paired bootstrap interval for the mean of per-episode differences."""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np

from .load import PairedRuns

DEFAULT_RESAMPLES = 10_000
_CHUNK_ELEMENTS = 4_000_000  # bounds the index matrix to ~32 MB


@dataclass(frozen=True)
class BootstrapResult:
    mean: float
    low: float
    high: float
    confidence: float
    n: int
    n_resamples: int
    seed: int


def percentile_interval(stats: np.ndarray, confidence: float) -> tuple[float, float]:
    """Equal-tailed percentile interval with linear interpolation (numpy default)."""
    tail = (1.0 - confidence) / 2.0
    lo, hi = np.quantile(stats, [tail, 1.0 - tail])
    return float(lo), float(hi)


def paired_bootstrap_ci(
    d: np.ndarray,
    *,
    seed: int,
    confidence: float = 0.90,
    n_resamples: int = DEFAULT_RESAMPLES,
) -> BootstrapResult:
    """Percentile bootstrap CI of mean(d).

    `d` holds one difference per paired episode. Each resample draws episodes with
    replacement; because d_i already combines both arms, an episode is resampled as a pair
    and the two arms are never resampled independently. `seed` is required and returned.
    The default confidence of 0.90 is 1 - 2*alpha for alpha = 0.05.
    """
    d = np.asarray(d, dtype=float)
    if d.ndim != 1 or len(d) < 2:
        raise ValueError("need a 1-D array of at least 2 paired differences")
    if not np.all(np.isfinite(d)):
        raise ValueError("differences must be finite")
    if not 0.0 < confidence < 1.0:
        raise ValueError("confidence must be in (0, 1)")
    if n_resamples < 1:
        raise ValueError("n_resamples must be positive")
    if isinstance(seed, bool) or int(seed) != seed:
        raise ValueError("seed must be an integer")
    seed = int(seed)

    n = len(d)
    rng = np.random.default_rng(seed)
    means = np.empty(n_resamples)
    done = 0
    for idx in _resample_indices(rng, n, n_resamples):
        means[done : done + len(idx)] = d[idx].mean(axis=1)
        done += len(idx)
    lo, hi = percentile_interval(means, confidence)
    return BootstrapResult(float(d.mean()), lo, hi, confidence, n, n_resamples, seed)


def _resample_indices(rng: np.random.Generator, n: int, n_resamples: int):
    """Yield (m, n) blocks of episode indices drawn with replacement; m*blocks = n_resamples."""
    chunk = max(1, _CHUNK_ELEMENTS // n)
    done = 0
    while done < n_resamples:
        m = min(chunk, n_resamples - done)
        yield rng.integers(0, n, size=(m, n))
        done += m


@dataclass(frozen=True)
class RatioResult:
    """Relative savings S = 1 - sum(B)/sum(A) with a paired bootstrap interval."""

    savings: float
    low: float
    high: float
    sum_a: float
    sum_b: float
    confidence: float
    n: int
    n_resamples: int
    seed: int


def ratio_of_totals_ci(
    paired: PairedRuns,
    metric: str,
    seed: int,
    resamples: int = DEFAULT_RESAMPLES,
    confidence: float = 0.90,
) -> RatioResult:
    """Relative savings of B over A for a cost-like metric, with a paired bootstrap interval.

        S = 1 - sum_i B_i / sum_i A_i          (ratio of totals, charter EXP-001)

    Each resample draws episode indices with replacement and applies the same indices to A
    and B (pairs stay together), then recomputes S from the resampled totals. The interval
    is the equal-tailed percentile interval at `confidence` (0.90 = 1 - 2*alpha for
    alpha = 0.05). Positive S means B costs less. No sign flip is applied: this function is
    only meaningful when lower values of `metric` are better.

    Raises ValueError for negative values, when sum(A) == 0, or when any resample has
    sum(A) == 0 (S undefined); none of these is silently skipped.
    """
    a, b = paired.values(metric)
    if len(a) < 2:
        raise ValueError("need at least 2 paired episodes")
    if np.any(a < 0) or np.any(b < 0):
        raise ValueError(f"metric {metric!r} has negative values; ratio of totals is undefined")
    sum_a, sum_b = float(a.sum()), float(b.sum())
    if sum_a == 0.0:
        raise ValueError(f"sum of {metric!r} over arm A is zero; relative savings is undefined")
    if not 0.0 < confidence < 1.0:
        raise ValueError("confidence must be in (0, 1)")
    if resamples < 1:
        raise ValueError("resamples must be positive")
    if isinstance(seed, bool) or int(seed) != seed:
        raise ValueError("seed must be an integer")
    seed = int(seed)

    n = len(a)
    rng = np.random.default_rng(seed)
    stats_ = np.empty(resamples)
    done = 0
    for idx in _resample_indices(rng, n, resamples):
        sa = a[idx].sum(axis=1)
        if np.any(sa == 0.0):
            raise ValueError(
                f"a bootstrap resample of {metric!r} has zero total in arm A; "
                "relative savings is undefined for this data"
            )
        stats_[done : done + len(idx)] = 1.0 - b[idx].sum(axis=1) / sa
        done += len(idx)
    lo, hi = percentile_interval(stats_, confidence)
    return RatioResult(1.0 - sum_b / sum_a, lo, hi, sum_a, sum_b, confidence, n, resamples, seed)


@dataclass(frozen=True)
class MedianRatioResult:
    """The median over episodes of B_i / A_i, with a paired bootstrap interval."""

    median: float
    low: float
    high: float
    n: int
    confidence: float
    n_resamples: int
    seed: int


def median_ratio_ci(
    paired: PairedRuns,
    metric: str,
    seed: int,
    resamples: int = DEFAULT_RESAMPLES,
    confidence: float = 0.90,
) -> MedianRatioResult:
    """Median of the per-episode ratios B_i / A_i, with a paired bootstrap interval.

    The median of episodes is the estimate that a few interrupted episodes cannot move, which is
    why it is the wall-time estimate the cost model's check uses (work item A8b): bursts of
    stolen CPU time hit single episodes and dominate a ratio of totals. Each resample draws
    episode indices with replacement and applies them to the per-episode ratios, so a pair stays
    together; the interval is the equal-tailed percentile interval at `confidence`.

    Raises ValueError for negative values or a zero in arm A (the ratio of that episode is
    undefined), never skipping an episode.
    """
    a, b = paired.values(metric)
    if len(a) < 2:
        raise ValueError("need at least 2 paired episodes")
    if np.any(a <= 0) or np.any(b < 0):
        raise ValueError(
            f"metric {metric!r} must be positive in arm A and non-negative in arm B for a "
            "per-episode ratio"
        )
    if not 0.0 < confidence < 1.0:
        raise ValueError("confidence must be in (0, 1)")
    if resamples < 1:
        raise ValueError("resamples must be positive")
    if isinstance(seed, bool) or int(seed) != seed:
        raise ValueError("seed must be an integer")
    seed = int(seed)
    ratios = b / a
    n = len(ratios)
    rng = np.random.default_rng(seed)
    stats_ = np.empty(resamples)
    done = 0
    for idx in _resample_indices(rng, n, resamples):
        stats_[done : done + len(idx)] = np.median(ratios[idx], axis=1)
        done += len(idx)
    lo, hi = percentile_interval(stats_, confidence)
    return MedianRatioResult(float(np.median(ratios)), lo, hi, n, confidence, resamples, seed)
