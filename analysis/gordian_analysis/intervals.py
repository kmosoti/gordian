"""Paired bootstrap interval for the mean of per-episode differences."""

from __future__ import annotations

import math
from dataclasses import dataclass

import numpy as np
from scipy.stats import norm

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


# Interval methods for S = 1 - sum(B)/sum(A). All three read the same bootstrap resamples (the
# same episode indices for a given seed), so they differ only in how a set of resampled
# statistics becomes an interval. The default is fixed by the calibration of work item A7b
# (experiments/exploration/a7b-ratio-calibration.md).
RATIO_METHODS = ("percentile", "bca", "studentized")
DEFAULT_RATIO_METHOD = "studentized"
# A log-scale standard error below this is rounding noise around a zero (B proportional to A on
# every episode), not a measurement; no real cost data has a relative standard error this small.
_TINY_SE = 1e-10


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
    method: str = "percentile"


def _check_ratio_inputs(a: np.ndarray, b: np.ndarray, label: str) -> None:
    if len(a) < 2:
        raise ValueError("need at least 2 paired episodes")
    if np.any(a < 0) or np.any(b < 0):
        raise ValueError(f"metric {label!r} has negative values; ratio of totals is undefined")
    if float(a.sum()) == 0.0:
        raise ValueError(f"sum of {label!r} over arm A is zero; relative savings is undefined")


def _check_ratio_args(confidence: float, resamples: int, method: str) -> None:
    if not 0.0 < confidence < 1.0:
        raise ValueError("confidence must be in (0, 1)")
    if resamples < 1:
        raise ValueError("resamples must be positive")
    if method not in RATIO_METHODS:
        raise ValueError(f"method must be one of {RATIO_METHODS}, got {method!r}")


def _ratio_resamples(
    a: np.ndarray,
    b: np.ndarray,
    rng: np.random.Generator,
    resamples: int,
    *,
    with_se: bool,
    label: str,
) -> tuple[np.ndarray, np.ndarray | None]:
    """S* for each bootstrap resample of the paired episodes, and (with_se) the delta-method
    standard error of log(sum B*/sum A*) computed from that resample.

        se_log* = sd_i(B*_i - R* A*_i) / (sqrt(n) mean(B*)),     R* = sum B* / sum A*

    The factor is the linearisation of log R: var(R) ~ var(B - R A) / (n mean(A)^2), and
    mean(A) R = mean(B). Resamples are whole episodes, so a pair stays together. A resample with
    sum A* = 0 raises (S undefined), as does one with sum B* = 0 when the standard error is
    wanted (the log ratio is undefined).
    """
    n = len(a)
    s_star = np.empty(resamples)
    se_star = np.empty(resamples) if with_se else None
    if with_se:
        aa, bb, ab = a * a, b * b, a * b
    done = 0
    for idx in _resample_indices(rng, n, resamples):
        m = len(idx)
        sa = a[idx].sum(axis=1)
        if np.any(sa == 0.0):
            raise ValueError(
                f"a bootstrap resample of {label!r} has zero total in arm A; "
                "relative savings is undefined for this data"
            )
        sb = b[idx].sum(axis=1)
        s_star[done : done + m] = 1.0 - sb / sa
        if with_se:
            if np.any(sb == 0.0):
                raise ValueError(
                    f"a bootstrap resample of {label!r} has zero total in arm B; the log ratio "
                    "behind the studentized interval is undefined (percentile and bca still are)"
                )
            r = sb / sa
            szz = bb[idx].sum(axis=1) - 2.0 * r * ab[idx].sum(axis=1) + r * r * aa[idx].sum(axis=1)
            var_z = np.maximum(szz, 0.0) / (n - 1)  # sum z = 0 by the definition of r
            se_star[done : done + m] = np.sqrt(var_z / n) / (sb / n)
        done += m
    return s_star, se_star


def _order_quantile(x: np.ndarray, p: float) -> float:
    """The ceil((B+1) p)-th smallest of B values (Davison and Hinkley's bootstrap-t convention).

    An order statistic, not an interpolation, so infinite values (a resample whose standard
    error is zero) stay infinite instead of turning into NaN.
    """
    k = min(max(math.ceil((len(x) + 1) * p), 1), len(x))
    return float(np.partition(x, k - 1)[k - 1])


def _bca_interval(
    a: np.ndarray, b: np.ndarray, s_hat: float, s_star: np.ndarray, confidence: float
) -> tuple[float, float]:
    """BCa interval of S (Efron 1987) from resampled S* values.

    Bias correction z0 = Phi^-1(share of S* below S-hat, ties half); acceleration from the
    jackknife of S (leave one episode out). Adjusted levels
    Phi(z0 + (z0 + z_alpha) / (1 - acc (z0 + z_alpha))) are read off the S* quantiles with numpy's
    linear interpolation, as the percentile interval is. Raises when z0 is infinite (every
    resample on one side of S-hat) or an adjusted level is undefined (1 - acc (z0 + z) <= 0);
    no clamp is applied.
    """
    n_boot = len(s_star)
    if float(s_star.max() - s_star.min()) <= _TINY_SE:
        return s_hat, s_hat  # B proportional to A: every resample gives the same S
    below = (np.count_nonzero(s_star < s_hat) + 0.5 * np.count_nonzero(s_star == s_hat)) / n_boot
    if not 0.0 < below < 1.0:
        raise ValueError(
            "BCa is undefined: every bootstrap resample lies on one side of the estimate "
            "(too few episodes or resamples)"
        )
    z0 = float(norm.ppf(below))
    rest_a, rest_b = a.sum() - a, b.sum() - b
    if np.any(rest_a == 0.0):
        raise ValueError("BCa is undefined: leaving out one episode leaves arm A with zero total")
    jack = 1.0 - rest_b / rest_a
    dev = jack.mean() - jack
    denom = 6.0 * float((dev**2).sum()) ** 1.5
    acc = float((dev**3).sum()) / denom if denom > 0.0 else 0.0
    lo, hi = np.quantile(s_star, _bca_levels(z0, acc, (1.0 - confidence) / 2.0))
    return float(lo), float(hi)


def _bca_levels(z0: float, acc: float, tail: float) -> tuple[float, float]:
    """The adjusted quantile levels of a BCa interval; (tail, 1 - tail) when z0 = acc = 0."""
    levels = []
    for alpha in (tail, 1.0 - tail):
        z = z0 + float(norm.ppf(alpha))
        d = 1.0 - acc * z
        if d <= 0.0:
            raise ValueError("BCa is undefined: the acceleration makes an adjusted level undefined")
        levels.append(float(norm.cdf(z0 + z / d)))
    return levels[0], levels[1]


def _studentized_interval(
    a: np.ndarray,
    b: np.ndarray,
    s_hat: float,
    s_star: np.ndarray,
    se_star: np.ndarray,
    confidence: float,
) -> tuple[float, float]:
    """Bootstrap-t interval for log(sum B / sum A), mapped back to S = 1 - exp(.).

    t*_b = (theta*_b - theta-hat) / se*_b with theta = log R; the interval for theta is
    [theta-hat - se q_{1-tail}, theta-hat - se q_{tail}] where se is the delta-method standard
    error of the observed data and q the order-statistic quantiles of t*. Since S = 1 - e^theta
    is decreasing, S's lower limit comes from theta's upper one. A resample with se* = 0 has
    t* = +-infinity (or 0 when it also hits theta-hat), which widens the interval rather than
    being dropped. If the observed standard error is zero (B is proportional to A on every
    episode; below 1e-10 it is rounding noise) the interval is the point estimate.
    """
    n = len(a)
    sum_a, sum_b = float(a.sum()), float(b.sum())
    if sum_b == 0.0:
        raise ValueError(
            "the studentized interval is undefined: arm B has zero total (log ratio of 0)"
        )
    r_hat = sum_b / sum_a
    z = b - r_hat * a
    se_hat = float(np.sqrt((z**2).sum() / (n - 1) / n)) / (sum_b / n)
    if se_hat <= _TINY_SE:
        return s_hat, s_hat
    theta_hat = math.log(r_hat)
    theta_star = np.log1p(-s_star)  # log R* = log(1 - S*)
    diff = theta_star - theta_hat
    with np.errstate(divide="ignore", invalid="ignore"):
        t = diff / se_star
    zero_se = se_star <= _TINY_SE
    if zero_se.any():
        flat = np.where(np.abs(diff) <= _TINY_SE, 0.0, np.copysign(np.inf, diff))
        t = np.where(zero_se, flat, t)
    tail = (1.0 - confidence) / 2.0
    theta_hi = theta_hat - se_hat * _order_quantile(t, tail)
    theta_lo = theta_hat - se_hat * _order_quantile(t, 1.0 - tail)
    return 1.0 - math.exp(theta_hi), 1.0 - math.exp(theta_lo)


def ratio_intervals(
    a: np.ndarray,
    b: np.ndarray,
    rng: np.random.Generator,
    *,
    resamples: int = DEFAULT_RESAMPLES,
    confidence: float = 0.90,
    methods: tuple[str, ...] = RATIO_METHODS,
    label: str = "cost",
) -> dict[str, tuple[float, float]]:
    """Intervals of S = 1 - sum(B)/sum(A) by several methods from ONE set of resamples.

    Returns {method: (low, high)}. The calibration simulation uses this to compare methods on
    identical resamples; `ratio_of_totals_ci` calls it with one method.
    """
    a = np.asarray(a, dtype=float)
    b = np.asarray(b, dtype=float)
    for m in methods:
        _check_ratio_args(confidence, resamples, m)
    _check_ratio_inputs(a, b, label)
    s_hat = 1.0 - float(b.sum()) / float(a.sum())
    s_star, se_star = _ratio_resamples(
        a, b, rng, resamples, with_se="studentized" in methods, label=label
    )
    out = {}
    for m in methods:
        if m == "percentile":
            out[m] = percentile_interval(s_star, confidence)
        elif m == "bca":
            out[m] = _bca_interval(a, b, s_hat, s_star, confidence)
        else:
            out[m] = _studentized_interval(a, b, s_hat, s_star, se_star, confidence)
    return out


def ratio_of_totals_ci(
    paired: PairedRuns,
    metric: str,
    seed: int,
    resamples: int = DEFAULT_RESAMPLES,
    confidence: float = 0.90,
    method: str = DEFAULT_RATIO_METHOD,
) -> RatioResult:
    """Relative savings of B over A for a cost-like metric, with a paired bootstrap interval.

        S = 1 - sum_i B_i / sum_i A_i          (ratio of totals, charter EXP-001)

    Each resample draws episode indices with replacement and applies the same indices to A
    and B (pairs stay together), then recomputes S from the resampled totals. `method` turns the
    resampled statistics into an interval at `confidence` (0.90 = 1 - 2*alpha for alpha = 0.05):

    - "percentile": the equal-tailed percentile interval of the resampled S. Anti-conservative
      on skewed costs (docs/local-test-plan.md, A7b).
    - "bca": Efron's bias-corrected and accelerated interval of S.
    - "studentized": bootstrap-t on log(sum B / sum A), mapped back to S.

    Positive S means B costs less. No sign flip is applied: this function is only meaningful
    when lower values of `metric` are better.

    Raises ValueError for negative values, when sum(A) == 0, or when any resample has
    sum(A) == 0 (S undefined); none of these is silently skipped. "studentized" also raises
    for a zero total in arm B (log of 0), and "bca" when it is undefined (see `_bca_interval`).
    """
    a, b = paired.values(metric)
    _check_ratio_inputs(a, b, metric)
    _check_ratio_args(confidence, resamples, method)
    if isinstance(seed, bool) or int(seed) != seed:
        raise ValueError("seed must be an integer")
    seed = int(seed)
    sum_a, sum_b = float(a.sum()), float(b.sum())
    rng = np.random.default_rng(seed)
    ((lo, hi),) = ratio_intervals(
        a, b, rng, resamples=resamples, confidence=confidence, methods=(method,), label=metric
    ).values()
    return RatioResult(
        1.0 - sum_b / sum_a, lo, hi, sum_a, sum_b, confidence, len(a), resamples, seed, method
    )


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
