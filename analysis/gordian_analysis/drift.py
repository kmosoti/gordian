"""Machine-drift and arm-position diagnostics for interleaved runs (work item A8).

Wall time on the VM drifts within a session and the first play of an episode in a process may
cost more than the second. Two diagnostics say how much, so that a measured-cost comparison
between arms can be read with them:

* `drift_report`: the spread of the fixed reference workload's timings in `drift.csv` (the
  coefficient of variation) and the ratio of the last block to the first.
* `position_effect_*`: whether measured cost depends on `arm_position`, for one arm, with an
  interval and a permutation p-value. Two estimators, chosen by what the run offers; see
  `analysis/README.md` for the choice, the assumptions, and what each does not show.

Nothing here sets a margin. A margin on a position effect, or a tolerance on the drift, is
preregistered by the experiment that uses the run; a function that takes one reports where the
interval lies against it and nothing more.
"""

from __future__ import annotations

import math
from dataclasses import dataclass
from pathlib import Path

import numpy as np
import pandas as pd

from .intervals import DEFAULT_RESAMPLES, paired_bootstrap_ci, percentile_interval
from .load import (
    KEY,
    MEASURED_TOTAL,
    MEASURED_VALUE_COLUMNS,
    OPTIONAL_ARM_POSITION,
    RESULTS_COLUMNS,
    LoadError,
    Run,
)

# The columns the harness writes to drift.csv (crates/gordian-run/src/drift.rs, DRIFT_HEADER).
DRIFT_COLUMNS = ["run_id", "block", "units_done", "reps", "ns", "min_ns"]
DEFAULT_PERMUTATIONS = 10_000

# What an estimate is of cost on, besides the total.
_COST_METRICS = [MEASURED_TOTAL, *MEASURED_VALUE_COLUMNS]


# ---- drift.csv --------------------------------------------------------------------------------


def load_drift(path: str | Path) -> pd.DataFrame:
    """Read `drift.csv` from a run directory (or the file itself) and check it.

    Fails (`LoadError`) on a missing file, other columns than the harness writes, a value that is
    not a non-negative integer, a non-positive timing, more than one run id, blocks that do not
    run 0, 1, 2, ... in order, or `units_done` that decreases.
    """
    path = Path(path)
    file = path / "drift.csv" if path.is_dir() else path
    if not file.is_file():
        raise LoadError(f"{path}: no drift.csv (a run written before A8 has none)")
    raw = pd.read_csv(file, dtype=str, keep_default_na=False)
    where = str(file)
    if list(raw.columns) != DRIFT_COLUMNS:
        raise LoadError(f"{where}: columns are {list(raw.columns)}, expected {DRIFT_COLUMNS}")
    if len(raw) == 0:
        raise LoadError(f"{where}: no rows")
    df = pd.DataFrame({"run_id": raw["run_id"].str.strip()})
    if df["run_id"].nunique() != 1:
        raise LoadError(f"{where}: expected exactly one run_id, found {sorted(df['run_id'].unique())}")
    for c in DRIFT_COLUMNS[1:]:
        values = pd.to_numeric(raw[c].str.strip(), errors="coerce").astype(float)
        bad = ~np.isfinite(values.to_numpy()) | (values.to_numpy() != np.floor(values.to_numpy()))
        if bad.any() or (values < 0).any():
            pos = int(np.argmax(bad | (values.to_numpy() < 0)))
            raise LoadError(
                f"{where}: column {c!r} has {raw[c].iloc[pos]!r} (row {pos + 2}); "
                "expected a non-negative integer"
            )
        df[c] = values.astype("int64")
    if not (df["block"].to_numpy() == np.arange(len(df))).all():
        raise LoadError(f"{where}: blocks must be 0, 1, 2, ... in order")
    if (np.diff(df["units_done"].to_numpy()) < 0).any():
        raise LoadError(f"{where}: units_done decreases")
    if (df["ns"] <= 0).any() or (df["min_ns"] <= 0).any() or (df["reps"] <= 0).any():
        raise LoadError(f"{where}: ns, min_ns and reps must be positive")
    return df


@dataclass(frozen=True)
class DriftSummary:
    """Spread and trend of one timing column across blocks."""

    first: float
    last: float
    mean: float
    sd: float
    cv: float
    ratio_last_first: float
    minimum: float
    maximum: float


@dataclass(frozen=True)
class DriftReport:
    """`ns` is the sum of a block's verifier runs; `min_ns` the shortest single run in it."""

    n_blocks: int
    reps_per_block: int
    ns: DriftSummary
    min_ns: DriftSummary
    blocks: list[dict]


def _summarize(x: np.ndarray) -> DriftSummary:
    mean = float(x.mean())
    sd = float(x.std(ddof=1))
    return DriftSummary(
        first=float(x[0]),
        last=float(x[-1]),
        mean=mean,
        sd=sd,
        cv=sd / mean,
        ratio_last_first=float(x[-1] / x[0]),
        minimum=float(x.min()),
        maximum=float(x.max()),
    )


def drift_report(df: pd.DataFrame) -> DriftReport:
    """The coefficient of variation (sample sd over mean) and last-over-first ratio of `drift.csv`.

    Both on `ns`, the block's total, and on `min_ns`, the fastest single run in the block, which
    a preemption does not move. Needs at least two blocks. The first block is the process's
    first work after start-up, so it is the one most likely to run cold; the ratio uses it as
    the specification says, and the per-block table is in the report for anyone who wants to read
    the others against each other.
    """
    if len(df) < 2:
        raise LoadError("the coefficient of variation needs at least two drift blocks")
    reps = sorted(df["reps"].unique())
    if len(reps) != 1:
        raise LoadError(f"blocks ran different numbers of repetitions: {reps}")
    return DriftReport(
        n_blocks=len(df),
        reps_per_block=int(reps[0]),
        ns=_summarize(df["ns"].to_numpy(dtype=float)),
        min_ns=_summarize(df["min_ns"].to_numpy(dtype=float)),
        blocks=df[["block", "units_done", "ns", "min_ns"]].to_dict(orient="records"),
    )


# ---- position effect --------------------------------------------------------------------------

METHOD_STRATIFIED = "stratified_permutation"
METHOD_PAIRED = "paired_copies"

WITHIN = "within_margin"
EXCEEDS = "exceeds_margin"
UNRESOLVED = "unresolved"


@dataclass(frozen=True)
class PositionEffect:
    """The effect of playing first on measured cost, on the log scale.

    `log_ratio` is log(cost at arm_position 0 / cost at a later position), a mean of logs, so
    `ratio = exp(log_ratio)` is a ratio of geometric means: above 1 means playing first costs
    more (a cold start), below 1 means it costs less. `low` and `high` bound `log_ratio` at
    `confidence`; `p_value` is two-sided, from permutations of the position labels (or of the
    signs, for the paired method), with the usual +1 so that it is never zero.
    """

    method: str
    metric: str
    n_first: int
    n_later: int
    n_strata: int
    log_ratio: float
    ratio: float
    low: float
    high: float
    confidence: float
    p_value: float
    n_resamples: int
    n_permutations: int
    seed: int


def _check_cost(df: pd.DataFrame, metric: str, where: str) -> np.ndarray:
    if metric not in _COST_METRICS:
        raise LoadError(f"{metric!r} is not a measured cost; choose from {_COST_METRICS}")
    y = df[metric].to_numpy(dtype=float)
    if not (y > 0).all():
        raise LoadError(f"{where}: {metric!r} has a zero or negative value; its log is undefined")
    return np.log(y)


def _check_resampling(seed: int, resamples: int, permutations: int, confidence: float) -> None:
    if isinstance(seed, bool) or int(seed) != seed:
        raise ValueError("seed must be an integer")
    if resamples < 1 or permutations < 1:
        raise ValueError("resamples and permutations must be positive")
    if not 0.0 < confidence < 1.0:
        raise ValueError("confidence must be in (0, 1)")


@dataclass
class _Stratum:
    first: np.ndarray  # log costs at position 0
    later: np.ndarray  # log costs at a later position

    @property
    def weight(self) -> float:
        n0, n1 = len(self.first), len(self.later)
        return n0 * n1 / (n0 + n1)

    @property
    def diff(self) -> float:
        return float(self.first.mean() - self.later.mean())


def _strata(classes: np.ndarray, position: np.ndarray, y: np.ndarray) -> list[_Stratum]:
    """One stratum per class that has episodes at position 0 and at a later position."""
    out = []
    for c in sorted(set(classes)):
        mask = classes == c
        first = y[mask & (position == 0)]
        later = y[mask & (position > 0)]
        if len(first) >= 1 and len(later) >= 1:
            out.append(_Stratum(first, later))
    return out


def _stratified_statistic(strata: list[_Stratum]) -> float:
    w = np.array([s.weight for s in strata])
    d = np.array([s.diff for s in strata])
    return float((w * d).sum() / w.sum())


def position_effect_stratified(
    run: Run,
    *,
    metric: str = MEASURED_TOTAL,
    seed: int,
    confidence: float = 0.90,
    n_resamples: int = DEFAULT_RESAMPLES,
    n_permutations: int = DEFAULT_PERMUTATIONS,
) -> PositionEffect:
    """Position effect for ONE arm, comparing episodes it played first with episodes it played later.

    Within one arm an episode is played once, at one position, so no episode is available in both
    positions and no pairing exists. The arm's position in each episode was drawn at random,
    independently of the episode, so under the sharp null that position does not change an
    episode's cost, the position labels are exchangeable among episodes, and a permutation of
    them is the exact randomization distribution of any statistic. Episode classes differ in cost
    by an order of magnitude, so labels are permuted *within each class* and the statistic is
    stratified by class:

        T = sum_c w_c * (mean(log cost | pos 0, class c) - mean(log cost | pos > 0, class c))
            / sum_c w_c,             w_c = n0_c * n1_c / (n0_c + n1_c)

    (the inverse-variance weights for equal variances). The interval is the percentile bootstrap
    of T, resampling episodes with replacement *within each (class, position-0-or-later) cell*,
    which keeps every cell's size and so every weight. The p-value is two-sided:
    `(1 + #{|T*| >= |T|}) / (1 + n_permutations)`.

    A class with episodes at one kind of position only contributes nothing. A cell of one
    episode contributes no resampling variance, so the interval is a little too narrow when many
    cells are tiny; with the A/A design (20 seeds per class, two arms) cells hold about ten.
    """
    where = str(run.path)
    _check_resampling(seed, n_resamples, n_permutations, confidence)
    df = run.results
    if OPTIONAL_ARM_POSITION not in df.columns:
        raise LoadError(f"{where}: measured.csv has no arm_position (a run written before A8)")
    position = df[OPTIONAL_ARM_POSITION].to_numpy()
    y = _check_cost(df, metric, where)
    classes = df["class"].to_numpy()
    strata = _strata(classes, position, y)
    if not strata:
        raise LoadError(
            f"{where}: no class has episodes at position 0 and at a later position; a one-arm "
            "run has no later position, so there is no position effect to estimate"
        )
    observed = _stratified_statistic(strata)
    rng = np.random.default_rng(seed)

    # Bootstrap: resample inside each cell.
    boots = np.empty(n_resamples)
    w = np.array([s.weight for s in strata])
    first_means = np.empty((len(strata), n_resamples))
    later_means = np.empty((len(strata), n_resamples))
    for i, s in enumerate(strata):
        first_means[i] = s.first[rng.integers(0, len(s.first), size=(n_resamples, len(s.first)))].mean(axis=1)
        later_means[i] = s.later[rng.integers(0, len(s.later), size=(n_resamples, len(s.later)))].mean(axis=1)
    boots[:] = (w[:, None] * (first_means - later_means)).sum(axis=0) / w.sum()
    low, high = percentile_interval(boots, confidence)

    # Permutations: shuffle the position-0 label among a class's episodes, keeping its count.
    perm = np.zeros(n_permutations)
    for s in strata:
        pooled = np.concatenate([s.first, s.later])
        n0 = len(s.first)
        # argsort of uniform noise is a uniform random permutation of each row.
        order = np.argsort(rng.random((n_permutations, len(pooled))), axis=1)
        shuffled = pooled[order]
        perm += s.weight * (shuffled[:, :n0].mean(axis=1) - shuffled[:, n0:].mean(axis=1))
    perm /= w.sum()
    p = (1 + int((np.abs(perm) >= abs(observed) - 1e-12).sum())) / (1 + n_permutations)

    return PositionEffect(
        method=METHOD_STRATIFIED,
        metric=metric,
        n_first=int(sum(len(s.first) for s in strata)),
        n_later=int(sum(len(s.later) for s in strata)),
        n_strata=len(strata),
        log_ratio=observed,
        ratio=math.exp(observed),
        low=low,
        high=high,
        confidence=confidence,
        p_value=float(p),
        n_resamples=n_resamples,
        n_permutations=n_permutations,
        seed=seed,
    )


def check_same_episodes_same_play(a: Run, b: Run) -> None:
    """Fail unless the two arms played every episode identically, apart from wall time.

    The paired estimator treats the two arms' timings of one episode as two measurements of the
    same work. That is true when the arms are copies of one policy: every column of results.csv
    except `run_id` (the bill, the probes, the outcome, the stop reason) then agrees on every
    episode, and the check is that it does. It is the empirical test that the two plays were the
    same work; it cannot be passed by arms that differ (a policy seeded by its arm name, for one,
    plays a different episode in each copy).
    """
    cols = [c for c in RESULTS_COLUMNS if c != "run_id" and c not in KEY]
    ka = a.results.set_index(KEY)
    kb = b.results.set_index(KEY)
    if not ka.index.sort_values().equals(kb.index.sort_values()):
        raise LoadError("the two arms do not hold the same (seed, class) episodes")
    kb = kb.loc[ka.index]
    # Series.equals counts a missing value as equal to a missing value, which an undecided
    # episode's decision_at_ns needs.
    differing = [c for c in cols if not ka[c].equals(kb[c])]
    if differing:
        raise LoadError(
            f"{a.path} and {b.path} did not play every episode identically (they differ in "
            f"{differing}), so their timings are not two measurements of the same work; use the "
            "stratified estimator for one arm instead"
        )


def position_effect_paired(
    a: Run,
    b: Run,
    *,
    metric: str = MEASURED_TOTAL,
    seed: int,
    confidence: float = 0.90,
    n_resamples: int = DEFAULT_RESAMPLES,
    n_permutations: int = DEFAULT_PERMUTATIONS,
) -> PositionEffect:
    """Position effect from two copies of one policy, which play every episode in both positions.

    Arms `a` and `b` must have played every episode identically (`check_same_episodes_same_play`),
    so for an episode one copy was played first and the other later, and the two timings are
    repeat measurements of the same work. For each episode in which one of the copies was at
    position 0:

        d_e = log(cost of the copy at position 0) - log(cost of the copy at a later position)

    An episode in which neither copy was first (possible with three or more arms) has no
    position-0 timing among these two and is left out; which episodes those are was decided by
    the draw, not by the costs. The interval is the percentile bootstrap of mean(d) over
    episodes (`paired_bootstrap_ci`: whole episodes are resampled, so each pair stays
    together). The p-value is a sign-flip test: if position does nothing, which copy is first is
    a fair coin independent of the work, so d_e and -d_e are equally likely, and
    `(1 + #{|mean(s * d)| >= |mean(d)|}) / (1 + n_permutations)` over random sign vectors s is
    exact for that null.
    """
    _check_resampling(seed, n_resamples, n_permutations, confidence)
    for run in (a, b):
        if OPTIONAL_ARM_POSITION not in run.results.columns:
            raise LoadError(f"{run.path}: measured.csv has no arm_position (a run written before A8)")
    check_same_episodes_same_play(a, b)
    ra = a.results.set_index(KEY).sort_index()
    rb = b.results.set_index(KEY).sort_index()
    ya = _check_cost(ra, metric, str(a.path))
    yb = _check_cost(rb, metric, str(b.path))
    pa = ra[OPTIONAL_ARM_POSITION].to_numpy()
    pb = rb[OPTIONAL_ARM_POSITION].to_numpy()
    if (pa == pb).any():
        raise LoadError("the two arms were at the same position in some episode; they are not arms of one run")
    a_first = pa == 0
    b_first = pb == 0
    use = a_first | b_first
    if use.sum() < 2:
        raise LoadError("fewer than two episodes have one of the two arms at position 0")
    d = np.where(a_first, ya - yb, yb - ya)[use]
    boot = paired_bootstrap_ci(d, seed=seed, confidence=confidence, n_resamples=n_resamples)
    rng = np.random.default_rng(seed)
    signs = rng.choice([-1.0, 1.0], size=(n_permutations, len(d)))
    flipped = (signs * d).mean(axis=1)
    p = (1 + int((np.abs(flipped) >= abs(float(d.mean())) - 1e-12).sum())) / (1 + n_permutations)
    return PositionEffect(
        method=METHOD_PAIRED,
        metric=metric,
        n_first=int(use.sum()),
        n_later=int(use.sum()),
        n_strata=1,
        log_ratio=float(d.mean()),
        ratio=math.exp(float(d.mean())),
        low=boot.low,
        high=boot.high,
        confidence=confidence,
        p_value=float(p),
        n_resamples=n_resamples,
        n_permutations=n_permutations,
        seed=seed,
    )


def margin_verdict(effect: PositionEffect, margin: float) -> str:
    """Where the interval of a position effect lies against a relative margin `margin`.

    The margin is relative cost (0.05 is 5%) and symmetric on the log scale: `L = log(1 + margin)`.
    `exceeds_margin`: the whole interval is beyond +L or beyond -L, so an effect larger than the
    margin is established (this is what invalidates a run's cost comparison). `within_margin`:
    the whole interval is inside (-L, +L). `unresolved`: otherwise, and an interval that is
    merely wide is never read as within the margin.
    """
    if not (math.isfinite(margin) and margin > 0):
        raise ValueError("margin must be a positive relative cost, for example 0.05")
    limit = math.log1p(margin)
    if effect.low > limit or effect.high < -limit:
        return EXCEEDS
    if effect.low > -limit and effect.high < limit:
        return WITHIN
    return UNRESOLVED

