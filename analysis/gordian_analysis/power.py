"""Sample size for paired non-inferiority and paired equivalence (TOST).

Normal approximation. It uses z quantiles where the analysis uses t quantiles with n - 1
degrees of freedom, so it understates n when n is small. `achieved_power_t` reports the power
the returned n actually has under the t analysis, so the shortfall is visible.
"""

from __future__ import annotations

import math
from dataclasses import dataclass

from scipy import stats

NONINFERIORITY = "noninferiority"
EQUIVALENCE = "equivalence"


@dataclass(frozen=True)
class PowerResult:
    design: str
    n: int  # number of paired episodes, rounded up
    n_unrounded: float
    sd: float  # sd of the per-episode differences
    margin: float
    alpha: float  # one-sided level of each test
    power: float
    true_diff: float
    achieved_power_t: float  # power at n under the t test (lower bound for equivalence)


def _validate(sd: float, margin: float, alpha: float, power: float) -> None:
    if not sd > 0:
        raise ValueError("sd must be positive")
    if not margin > 0:
        raise ValueError("margin must be positive")
    if not 0.0 < alpha < 0.5:
        raise ValueError("alpha must be in (0, 0.5)")
    if not 0.0 < power < 1.0:
        raise ValueError("power must be in (0, 1)")


def _ceil(x: float) -> int:
    # Guard against 25.000000000000004 style float noise before rounding up.
    return int(math.ceil(round(x, 9)))


def _achieved_power_ni(n: int, sd: float, margin: float, alpha: float, true_diff: float) -> float:
    if n < 2:
        return float("nan")
    df = n - 1
    c = stats.t.ppf(1.0 - alpha, df)
    ncp = (margin + true_diff) / (sd / math.sqrt(n))
    return float(stats.nct.sf(c, df, ncp))


def _achieved_power_eq(n: int, sd: float, margin: float, alpha: float, true_diff: float) -> float:
    if n < 2:
        return float("nan")
    df = n - 1
    c = stats.t.ppf(1.0 - alpha, df)
    se = sd / math.sqrt(n)
    p1 = stats.nct.sf(c, df, (true_diff + margin) / se)  # reject mu <= -margin
    p2 = stats.nct.cdf(-c, df, (true_diff - margin) / se)  # reject mu >= +margin
    # Bonferroni: P(A and B) >= P(A) + P(B) - 1, so this is a lower bound on TOST power.
    return float(max(0.0, p1 + p2 - 1.0))


def noninferiority_n(
    sd: float, margin: float, alpha: float = 0.05, power: float = 0.8, true_diff: float = 0.0
) -> PowerResult:
    """n = ((z_{1-alpha} + z_{1-beta}) * sd / (margin + true_diff))^2, rounded up.

    Positive true_diff means the treatment is truly better, which lowers n.
    """
    _validate(sd, margin, alpha, power)
    if margin + true_diff <= 0:
        raise ValueError("margin + true_diff must be positive (treatment truly inferior)")
    z = stats.norm.ppf(1.0 - alpha) + stats.norm.ppf(power)
    n_raw = (z * sd / (margin + true_diff)) ** 2
    n = _ceil(n_raw)
    return PowerResult(
        NONINFERIORITY, n, n_raw, sd, margin, alpha, power, true_diff,
        _achieved_power_ni(n, sd, margin, alpha, true_diff),
    )  # fmt: skip


def equivalence_n(
    sd: float, margin: float, alpha: float = 0.05, power: float = 0.8, true_diff: float = 0.0
) -> PowerResult:
    """n = ((z_{1-alpha} + z_{1-beta/2}) * sd / (margin - |true_diff|))^2, rounded up.

    The specified case is true_diff = 0. For true_diff != 0 the same expression is applied
    with margin replaced by margin - |true_diff| (conservative; the textbook formula for
    that case uses z_{1-beta} and would give a smaller n).
    """
    _validate(sd, margin, alpha, power)
    eff = margin - abs(true_diff)
    if eff <= 0:
        raise ValueError("|true_diff| must be smaller than margin for equivalence")
    beta = 1.0 - power
    z = stats.norm.ppf(1.0 - alpha) + stats.norm.ppf(1.0 - beta / 2.0)
    n_raw = (z * sd / eff) ** 2
    n = _ceil(n_raw)
    return PowerResult(
        EQUIVALENCE, n, n_raw, sd, margin, alpha, power, true_diff,
        _achieved_power_eq(n, sd, margin, alpha, true_diff),
    )  # fmt: skip
