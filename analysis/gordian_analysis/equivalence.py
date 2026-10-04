"""Paired TOST, the t interval, and the four-way classification."""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np
from scipy import stats

BENEFICIAL = "beneficial"
HARMFUL = "harmful"
EQUIVALENT = "equivalent"
UNRESOLVED = "unresolved"
CATEGORIES = (BENEFICIAL, HARMFUL, EQUIVALENT, UNRESOLVED)


def classify(ci_low: float, ci_high: float, margin: float) -> str:
    """Charter category from the (1 - 2*alpha) interval of mean(d); positive d is good.

    equivalent  if -margin < ci_low and ci_high < margin
    beneficial  else if ci_low > 0
    harmful     else if ci_high < 0
    unresolved  otherwise
    An interval that is not inside the margin and does not exclude zero is unresolved,
    never equivalent.
    """
    if margin <= 0:
        raise ValueError("margin must be positive")
    if ci_low > ci_high:
        raise ValueError("ci_low exceeds ci_high")
    if -margin < ci_low and ci_high < margin:
        return EQUIVALENT
    if ci_low > 0:
        return BENEFICIAL
    if ci_high < 0:
        return HARMFUL
    return UNRESOLVED


def noninferior(ci_low: float, margin: float) -> bool:
    """Non-inferiority: the lower limit of the (1 - 2*alpha) interval exceeds -margin."""
    if margin <= 0:
        raise ValueError("margin must be positive")
    return bool(ci_low > -margin)


def exceeds(ci_low: float, threshold: float) -> bool:
    """The lower limit of the (1 - 2*alpha) interval is strictly above `threshold`.

    A one-sided level-alpha decision that the quantity exceeds the threshold (for EXP-001
    relative savings, threshold 0.20).
    """
    return bool(ci_low > threshold)


GATE_REASON = "n below preregistered sample size"


def gated_category(raw: str, n: int, planned_n: int | None) -> str:
    """Apply the preregistered-sample-size gate to a raw category or decision label.

    If a planned sample size exists and n < planned_n, the result is UNRESOLVED whatever
    `raw` says (charter section 8 item 3: underpowered experiments stay unresolved).
    Otherwise `raw` is returned unchanged; `planned_n=None` means exploratory, no gate.
    `classify` itself stays a pure function of the interval.
    """
    if planned_n is not None and n < planned_n:
        return UNRESOLVED
    return raw


def _check(d: np.ndarray) -> np.ndarray:
    d = np.asarray(d, dtype=float)
    if d.ndim != 1 or len(d) < 2:
        raise ValueError("need a 1-D array of at least 2 paired differences")
    if not np.all(np.isfinite(d)):
        raise ValueError("differences must be finite")
    return d


def paired_t_statistic(d: np.ndarray, mu0: float = 0.0) -> tuple[float, int]:
    """t = (mean(d) - mu0) / (sd(d)/sqrt(n)), df = n - 1.

    With sd = 0 the statistic is +/-inf (or 0 when mean == mu0).
    """
    d = _check(d)
    n = len(d)
    se = float(d.std(ddof=1)) / np.sqrt(n)
    num = float(d.mean()) - mu0
    if se > 0:
        return num / se, n - 1
    return (0.0 if num == 0 else float(np.sign(num)) * np.inf), n - 1


def t_interval(d: np.ndarray, confidence: float = 0.90) -> tuple[float, float]:
    """mean(d) +/- t_{1-(1-confidence)/2, n-1} * sd(d)/sqrt(n)."""
    d = _check(d)
    n = len(d)
    se = float(d.std(ddof=1)) / np.sqrt(n)
    q = stats.t.ppf(1.0 - (1.0 - confidence) / 2.0, n - 1)
    m = float(d.mean())
    return m - q * se, m + q * se


@dataclass(frozen=True)
class TostResult:
    mean: float
    sd: float
    n: int
    margin: float
    t_lower: float  # H0: mu <= -margin
    t_upper: float  # H0: mu >= +margin
    p_lower: float
    p_upper: float
    p_tost: float  # max of the two one-sided p-values
    alpha: float

    @property
    def rejects(self) -> bool:
        """True when both one-sided nulls are rejected at alpha."""
        return self.p_tost < self.alpha


def tost_paired(d: np.ndarray, margin: float, alpha: float = 0.05) -> TostResult:
    """Two one-sided paired t-tests against -margin and +margin.

    lower: H0 mu <= -margin vs H1 mu > -margin, t = (mean + margin)/se, p = P(T_{n-1} > t)
    upper: H0 mu >=  margin vs H1 mu <  margin, t = (mean - margin)/se, p = P(T_{n-1} < t)
    p_tost = max(p_lower, p_upper). Rejecting both at alpha is the same decision as the
    (1 - 2*alpha) t interval lying inside (-margin, margin).
    """
    if margin <= 0:
        raise ValueError("margin must be positive")
    d = _check(d)
    t_lo, df = paired_t_statistic(d, -margin)
    t_hi, _ = paired_t_statistic(d, margin)
    p_lo = float(stats.t.sf(t_lo, df))
    p_hi = float(stats.t.cdf(t_hi, df))
    return TostResult(
        mean=float(d.mean()),
        sd=float(d.std(ddof=1)),
        n=len(d),
        margin=margin,
        t_lower=t_lo,
        t_upper=t_hi,
        p_lower=p_lo,
        p_upper=p_hi,
        p_tost=max(p_lo, p_hi),
        alpha=alpha,
    )
