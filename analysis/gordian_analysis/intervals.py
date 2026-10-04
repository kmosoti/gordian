"""Paired bootstrap interval for the mean of per-episode differences."""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np

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
    chunk = max(1, _CHUNK_ELEMENTS // n)
    means = np.empty(n_resamples)
    done = 0
    while done < n_resamples:
        m = min(chunk, n_resamples - done)
        idx = rng.integers(0, n, size=(m, n))
        means[done : done + m] = d[idx].mean(axis=1)
        done += m
    lo, hi = percentile_interval(means, confidence)
    return BootstrapResult(float(d.mean()), lo, hi, confidence, n, n_resamples, seed)
