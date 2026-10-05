"""Calibration of the relative-savings interval (work item A7b).

Asks one question of each interval method: when the true relative savings S is exactly the
threshold, how often does the interval's lower limit exceed it (a false exceedance)? EXP-001
accepts H1 when it does, so that rate is the size of the decision, nominal 0.05 for the 90%
two-sided interval.

The null is built, not assumed. A sampler draws `n` paired episodes (A_i, B_i) with replacement
from a finite population whose arm-B column has been rescaled so that sum(B)/sum(A) over the
population is exactly 1 - S, or from a lognormal model whose E[B]/E[A] is exactly 1 - S. A
simulated experiment draws a sample, computes every method's interval from one shared set of
bootstrap resamples, and records whether each lower limit exceeds the threshold. The same
machinery at S above the threshold gives power.

Every experiment has its own random stream, `SeedSequence(BASE_SEED, spawn_key=(population id,
S in thousandths, n, experiment index))`, used first for the sample and then for the bootstrap
resamples, so any one experiment can be replayed alone and a rerun reproduces a table exactly.
Nothing here reads a clock or the environment.
"""

from __future__ import annotations

import csv
import gzip
import math
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable

import numpy as np
from scipy.stats import norm

from .intervals import (
    DEFAULT_RESAMPLES,
    RATIO_METHODS,
    _bca_interval,
    _ratio_resamples,
    _studentized_interval,
    percentile_interval,
)

THRESHOLD = 0.20
CONFIDENCE = 0.90  # 1 - 2*alpha, alpha = 0.05: the interval gordian-analyze reports
BASE_SEED = 20_261_005
# 40, 160 and 400 probe the small-n regime; 1237 and 1713 are B2's planning sizes at a 2%
# discordant fraction for 80% and 90% power (experiments/exploration/b2-power.md, T2).
SIZES = (40, 160, 400, 1237, 1713)
PLANNED_SIZES = (1237, 1713)

Sampler = Callable[[np.random.Generator, int], "tuple[np.ndarray, np.ndarray]"]


@dataclass(frozen=True)
class Population:
    """A named source of paired episodes whose true relative savings is `savings`."""

    name: str
    pop_id: int  # enters the seed of every experiment drawn from it
    savings: float
    sampler: Sampler = field(repr=False, compare=False)
    description: str = ""


def rescale_to_savings(a: np.ndarray, b: np.ndarray, savings: float) -> np.ndarray:
    """Arm B multiplied by one constant so that sum(B)/sum(A) = 1 - savings exactly.

    The paired structure (each episode's cost relative to its partner, and its covariance with
    A) is untouched; only the level of B moves.
    """
    a = np.asarray(a, dtype=float)
    b = np.asarray(b, dtype=float)
    if a.sum() <= 0 or b.sum() <= 0:
        raise ValueError("both arms need a positive total to be rescaled")
    return b * ((1.0 - savings) * a.sum() / b.sum())


def empirical_population(
    name: str, pop_id: int, a: np.ndarray, b: np.ndarray, savings: float = THRESHOLD
) -> Population:
    """Resample paired episodes from (a, b) with B rescaled so the population's S = `savings`."""
    a = np.asarray(a, dtype=float)
    b = rescale_to_savings(a, b, savings)
    n_pop = len(a)

    def sample(rng: np.random.Generator, n: int):
        idx = rng.integers(0, n_pop, size=n)
        return a[idx], b[idx]

    return Population(
        name, pop_id, savings, sample, f"{n_pop} paired episodes resampled, B rescaled to S={savings}"
    )


def lognormal_population(
    name: str, pop_id: int, sigma_a: float, sigma_r: float, savings: float = THRESHOLD
) -> Population:
    """A_i lognormal(0, sigma_a); B_i = A_i * rho_i, rho_i = (1 - S) exp(sigma_r Z - sigma_r^2/2).

    rho is independent of A with mean 1 - S, so E[B]/E[A] = 1 - S: the true savings is exactly
    `savings`, whatever the two sigmas.
    """

    def sample(rng: np.random.Generator, n: int):
        a = np.exp(sigma_a * rng.standard_normal(n))
        rho = (1.0 - savings) * np.exp(sigma_r * rng.standard_normal(n) - 0.5 * sigma_r**2)
        return a, a * rho

    return Population(
        name,
        pop_id,
        savings,
        sample,
        f"lognormal costs, sigma_A={sigma_a}, sigma_ratio={sigma_r}, true S={savings}",
    )


def load_paired_costs(path: str | Path) -> dict[tuple[str, int], tuple[np.ndarray, np.ndarray]]:
    """{(pair, compute_ns): (cost_a, cost_b)} from the table a7b_generate.py writes."""
    out: dict[tuple[str, int], list] = {}
    opener = gzip.open if str(path).endswith(".gz") else open
    with opener(path, "rt", newline="") as fh:
        for row in csv.DictReader(fh):
            key = (row["pair"], int(row["compute_ns"]))
            out.setdefault(key, ([], []))
            out[key][0].append(float(row["cost_a"]))
            out[key][1].append(float(row["cost_b"]))
    return {k: (np.array(v[0]), np.array(v[1])) for k, v in out.items()}


def experiment_rng(pop_id: int, savings: float, n: int, k: int) -> np.random.Generator:
    """The random stream of experiment `k` of one (population, S, n) cell."""
    key = (int(pop_id), int(round(savings * 1000)), int(n), int(k))
    return np.random.default_rng(np.random.SeedSequence(BASE_SEED, spawn_key=key))


@dataclass
class CellResult:
    """One (population, true S, n) cell: what each method did over `n_exp` experiments."""

    population: str
    savings: float
    n: int
    n_exp: int
    resamples: int
    threshold: float
    exceed: dict[str, int]  # lower limit > threshold
    failed: dict[str, int]  # the method raised (never counted as an exceedance)
    mean_low: dict[str, float]  # mean lower limit over the experiments the method completed
    mean_width: dict[str, float]  # mean (high - low), completed experiments with finite limits
    unbounded: dict[str, int]  # lower limit was -inf (counted as not exceeding)

    def rate(self, method: str) -> float:
        return self.exceed[method] / self.n_exp

    def mc_se(self, method: str) -> float:
        p = self.rate(method)
        return math.sqrt(p * (1.0 - p) / self.n_exp)


def simulate_cell(
    pop: Population,
    n: int,
    n_exp: int,
    *,
    resamples: int = DEFAULT_RESAMPLES,
    methods: tuple[str, ...] = RATIO_METHODS,
    threshold: float = THRESHOLD,
    confidence: float = CONFIDENCE,
    first: int = 0,
) -> CellResult:
    """Run experiments `first .. first + n_exp - 1` of one cell.

    Each experiment: draw n paired episodes, take `resamples` bootstrap resamples of them once,
    and build each requested method's interval from those same resamples. A method whose interval
    is undefined for that sample (it raises ValueError) is counted in `failed` and does not count
    as an exceedance; nothing is dropped from the denominator. An error in the resampling itself
    (a zero total, S undefined) propagates: the simulation has no policy for data where S does
    not exist, and none of the populations here produces it.
    """
    exceed = dict.fromkeys(methods, 0)
    failed = dict.fromkeys(methods, 0)
    unbounded = dict.fromkeys(methods, 0)
    low_sum = dict.fromkeys(methods, 0.0)
    width_sum = dict.fromkeys(methods, 0.0)
    done = dict.fromkeys(methods, 0)
    width_n = dict.fromkeys(methods, 0)
    want_se = "studentized" in methods
    for k in range(first, first + n_exp):
        rng = experiment_rng(pop.pop_id, pop.savings, n, k)
        a, b = pop.sampler(rng, n)
        s_hat = 1.0 - b.sum() / a.sum()
        s_star, se_star = _ratio_resamples(a, b, rng, resamples, with_se=want_se, label="cost")
        for m in methods:
            try:
                if m == "percentile":
                    lo, hi = percentile_interval(s_star, confidence)
                elif m == "bca":
                    lo, hi = _bca_interval(a, b, s_hat, s_star, confidence)
                else:
                    lo, hi = _studentized_interval(a, b, s_hat, s_star, se_star, confidence)
            except ValueError:
                failed[m] += 1
                continue
            done[m] += 1
            if lo > threshold:
                exceed[m] += 1
            if np.isfinite(lo) and np.isfinite(hi):
                low_sum[m] += lo
                width_sum[m] += hi - lo
                width_n[m] += 1
            elif lo == -math.inf:
                unbounded[m] += 1
    return CellResult(
        pop.name,
        pop.savings,
        n,
        n_exp,
        resamples,
        threshold,
        exceed,
        failed,
        {m: low_sum[m] / width_n[m] if width_n[m] else math.nan for m in methods},
        {m: width_sum[m] / width_n[m] if width_n[m] else math.nan for m in methods},
        unbounded,
    )


def wilson_interval(successes: int, trials: int, confidence: float = 0.95) -> tuple[float, float]:
    """Wilson score interval for a binomial proportion (the Monte Carlo uncertainty of a rate)."""
    z = float(norm.ppf(0.5 + confidence / 2.0))
    p = successes / trials
    denom = 1.0 + z * z / trials
    centre = (p + z * z / (2 * trials)) / denom
    half = z * math.sqrt(p * (1 - p) / trials + z * z / (4 * trials * trials)) / denom
    return centre - half, centre + half
