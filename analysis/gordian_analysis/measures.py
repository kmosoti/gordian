"""Measures for the program's aim: sample efficiency and an energy proxy (work item W1).

`docs/charter-aim-proposal.md` (a proposal, not adopted) asks every experiment to report two
proxies beside quality and cost: *improvement per unit experience* and *joules per correct
decision*. These two functions compute them from one arm's per-stream results table (the
`results.csv` of a stream run: one row per stream, count columns). They read columns, not a
loader's types, so they work on whatever frame carries the columns named below.

What is measured, and what is not

- `sample_efficiency`: correct decisions per incident seen, cumulative over stream order. "Seen"
  means the incident occurred in a stream the arm has run, whether or not the arm noticed or
  escalated it; "correct" means a correct declaration by the incident's deadline, as the
  evaluator counts it. The curve is a pooled ratio of cumulative counts (never a mean of
  per-stream ratios; see `stream.py`). It is a description of one arm on one ordered list of
  streams. It is **not** a matched comparison with a conventional learner, and for an arm that does
  not learn (every arm in R4 to R10) the expected curve is flat: a flat curve is the baseline
  against which a learning arm's slope would be read.
- `energy_proxy`: modelled nanoseconds per correct decision, and the same converted to joules by
  a **placeholder** conversion. The nanoseconds are the evaluator's modelled cost (the rung's
  priced operations plus the reasoner's price at the manifest's exchange rate), not a measurement
  of the machine. The conversion to joules is two assumed powers, one for the cheap rung's
  operations and one for a reasoner call. Both are **assumptions, not measurements**, chosen only
  so that every later experiment reports the same figure; the charter revision is meant to fix
  them once, and until it does nothing computed from them is a finding about energy.
"""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np
import pandas as pd

# PLACEHOLDER CONVERSION, an assumption to be replaced by the charter revision.
#
# Joules are modelled nanoseconds times an assumed power. The cheap rung is priced as CPU work on
# one core, so 10 W is assumed for it (a laptop-class core under load; not measured here). The
# reasoner is priced as an inference call, and `docs/charter-aim-proposal.md` puts a conventional
# inference node at "kilowatts", so 1,000 W is assumed for it (the low end of that statement;
# itself unchecked against a primary source). Hence 1e-8 and 1e-6 joules per modelled nanosecond.
PLACEHOLDER_WATTS_CHEAP = 10.0
PLACEHOLDER_WATTS_REASONER = 1000.0
PLACEHOLDER_LABEL = (
    "PLACEHOLDER conversion (assumption, not a measurement): "
    f"{PLACEHOLDER_WATTS_CHEAP:g} W for cheap-rung operations, "
    f"{PLACEHOLDER_WATTS_REASONER:g} W for a reasoner call"
)

_JOULES_PER_NS_PER_WATT = 1e-9

TIERS = ("hard", "plain", "all")


def _require(results: pd.DataFrame, columns: list[str]) -> None:
    missing = [c for c in columns if c not in results.columns]
    if missing:
        raise ValueError(f"results lack the columns {missing}")
    for c in columns:
        col = results[c]
        if col.isna().any():
            raise ValueError(f"column {c!r} has missing values")
        if (col < 0).any():
            raise ValueError(f"column {c!r} has negative values")


def _counts(results: pd.DataFrame, tier: str) -> tuple[pd.Series, pd.Series]:
    """(incidents, correct) per stream for a tier of incident."""
    if tier not in TIERS:
        raise ValueError(f"tier must be one of {TIERS}, not {tier!r}")
    tiers = ["plain", "hard"] if tier == "all" else [tier]
    cols = [f"incidents_{t}" for t in tiers] + [f"correct_{t}" for t in tiers]
    _require(results, cols)
    incidents = sum(results[f"incidents_{t}"] for t in tiers)
    correct = sum(results[f"correct_{t}"] for t in tiers)
    if (correct > incidents).any():
        raise ValueError("a stream has more correct decisions than incidents")
    return incidents, correct


@dataclass(frozen=True)
class SampleEfficiency:
    """The result of `sample_efficiency`.

    `curve` has one row per stream in stream order: `stream` (1-based position), `key` (the value
    of the ordering column, the seed by default), `incidents` and `correct` (this stream's counts), `cum_incidents`, `cum_correct`, and
    `efficiency` (`cum_correct / cum_incidents`, NaN while no incident has been seen).
    """

    tier: str
    streams: int
    incidents: int
    correct: int
    efficiency: float
    first_half: float
    second_half: float
    curve: pd.DataFrame

    def at(self, streams: int) -> float:
        """The cumulative efficiency after the first `streams` streams."""
        if not 1 <= streams <= self.streams:
            raise ValueError(f"streams must be in 1..{self.streams}")
        return float(self.curve["efficiency"].iloc[streams - 1])


def sample_efficiency(
    results: pd.DataFrame, tier: str = "hard", order_by: str = "seed"
) -> SampleEfficiency:
    """Correct decisions per incident seen, cumulative over stream order.

    `results` is one arm's per-stream table with `incidents_<tier>` and `correct_<tier>` columns
    (`tier` is `"hard"`, `"plain"` or `"all"`, the last meaning plain and hard together) and the
    column named by `order_by` (default `seed`), which fixes the stream order and must be unique.

    `efficiency` is the pooled ratio of totals, `sum(correct) / sum(incidents)`, over the whole
    list; `first_half` and `second_half` are the same ratio over the first and the last
    `streams // 2` streams, so that a difference between them is a crude read of improvement with
    experience (with an odd count the middle stream is in neither). They are point estimates
    with no interval: about 2.5 hard incidents per stream means a half is a few hundred.
    """
    incidents, correct = _counts(results, tier)
    _require(results, [order_by])
    if results[order_by].duplicated().any():
        raise ValueError(f"{order_by!r} must identify a stream uniquely")
    order = np.argsort(results[order_by].to_numpy(), kind="stable")
    ordered = results.iloc[order]
    inc = incidents.iloc[order].to_numpy(dtype=np.int64)
    cor = correct.iloc[order].to_numpy(dtype=np.int64)
    cum_inc = np.cumsum(inc)
    cum_cor = np.cumsum(cor)
    with np.errstate(invalid="ignore", divide="ignore"):
        eff = np.where(cum_inc > 0, cum_cor / np.maximum(cum_inc, 1), np.nan)
    curve = pd.DataFrame(
        {
            "stream": np.arange(1, len(inc) + 1),
            "key": ordered[order_by].to_numpy(),
            "incidents": inc,
            "correct": cor,
            "cum_incidents": cum_inc,
            "cum_correct": cum_cor,
            "efficiency": eff,
        }
    )
    n = len(inc)
    h = n // 2

    def ratio(a: int, b: int) -> float:
        d = int(inc[a:b].sum())
        return float(cor[a:b].sum()) / d if d > 0 else float("nan")

    return SampleEfficiency(
        tier=tier,
        streams=n,
        incidents=int(cum_inc[-1]) if n else 0,
        correct=int(cum_cor[-1]) if n else 0,
        efficiency=float(eff[-1]) if n else float("nan"),
        first_half=ratio(0, h),
        second_half=ratio(n - h, n),
        curve=curve,
    )


@dataclass(frozen=True)
class EnergyProxy:
    """The result of `energy_proxy`. Every `joules_*` field is computed from the assumed
    conversion in `assumptions`, which is a placeholder; the `ns_*` fields are modelled cost."""

    tier: str
    streams: int
    correct: int
    ns_cheap: int
    ns_reasoner: int
    ns_total: int
    ns_per_correct: float
    joules_cheap: float
    joules_reasoner: float
    joules_total: float
    joules_per_correct: float
    assumptions: str


def energy_proxy(
    results: pd.DataFrame,
    tier: str = "all",
    watts_cheap: float = PLACEHOLDER_WATTS_CHEAP,
    watts_reasoner: float = PLACEHOLDER_WATTS_REASONER,
) -> EnergyProxy:
    """Modelled nanoseconds, and placeholder joules, per correct decision, pooled over streams.

    The numerator is the arm's whole modelled cost over the streams (`total_cost_ns`), split into
    the reasoner's price (`reasoner_cost_ns`) and the rest (`total_cost_ns - reasoner_cost_ns`:
    the rung's priced operations, the substrate and the shared rule; this is a remainder, so
    that the two parts always sum to the total). The denominator is the number of correct
    decisions of `tier` (`"all"` plain and hard, the default; `"hard"`; `"plain"`), so a
    `"hard"` figure still carries the cost of everything the arm did on plain incidents and
    background. NaN when the arm made no correct decision of that tier.

    Joules are `ns * 1e-9 * watts`, with `watts_cheap` applied to the remainder and
    `watts_reasoner` to the reasoner's price. The defaults are the module's placeholders and are
    assumptions, not measurements; `assumptions` repeats that in the result so that a table made
    from it cannot lose the label.
    """
    if watts_cheap < 0 or watts_reasoner < 0:
        raise ValueError("watts must be non-negative")
    _incidents, correct = _counts(results, tier)
    _require(results, ["total_cost_ns", "reasoner_cost_ns"])
    total = int(results["total_cost_ns"].sum())
    reasoner = int(results["reasoner_cost_ns"].sum())
    if (results["reasoner_cost_ns"] > results["total_cost_ns"]).any():
        raise ValueError("a stream's reasoner cost exceeds its total cost")
    cheap = total - reasoner
    j_cheap = cheap * _JOULES_PER_NS_PER_WATT * watts_cheap
    j_reasoner = reasoner * _JOULES_PER_NS_PER_WATT * watts_reasoner
    n_correct = int(correct.sum())
    per = (lambda x: x / n_correct) if n_correct > 0 else (lambda x: float("nan"))
    return EnergyProxy(
        tier=tier,
        streams=len(results),
        correct=n_correct,
        ns_cheap=cheap,
        ns_reasoner=reasoner,
        ns_total=total,
        ns_per_correct=per(total),
        joules_cheap=j_cheap,
        joules_reasoner=j_reasoner,
        joules_total=j_cheap + j_reasoner,
        joules_per_correct=per(j_cheap + j_reasoner),
        assumptions=(
            "PLACEHOLDER conversion (assumption, not a measurement): "
            f"{watts_cheap:g} W for cheap-rung operations, {watts_reasoner:g} W for a reasoner call"
        ),
    )


@dataclass(frozen=True)
class OutcomeSlope:
    """The result of `outcome_slope`: the least-squares slope of an incident's outcome against its
    position in the sequence of incidents seen, in share per 100 incidents seen, with a 90%
    cluster-bootstrap interval (whole streams resampled), and the incidents it is over."""

    incidents: int
    slope_per_100: float
    lower: float
    higher: float


def outcome_slope(
    outcomes: pd.DataFrame,
    streams: list,
    resamples: int = 10_000,
    seed: int = 0,
) -> OutcomeSlope:
    """Improvement per unit experience for a binary outcome: the slope of `y` against position.

    `outcomes` has one row per incident, in the order the incidents were seen (the row order is
    the position, 1-based), with `stream` (the cluster the incident belongs to) and `y` (1.0 if
    the incident's outcome was correct, else 0.0). `streams` lists the clusters the window is over,
    in order, including streams with no incident; incidents of other streams are ignored and the
    positions are the rows' own, so a window that starts at the first stream starts at position 1.

    The slope is `cov(x, y) / var(x)` over the incidents. The interval resamples whole streams
    with replacement (10,000 times by default, multinomial counts from
    `numpy.random.default_rng(seed)`) and keeps every incident's position; it is the 5th and 95th
    percentiles of the resampled slopes, NaN resamples (no spread in position) dropped. Slopes
    are NaN when fewer than two incidents have distinct positions.
    """
    if "stream" not in outcomes or "y" not in outcomes:
        raise ValueError("outcomes need the columns 'stream' and 'y'")
    if not outcomes["y"].isin([0.0, 1.0]).all():
        raise ValueError("y must be 0 or 1")
    t = outcomes.reset_index(drop=True)
    t["x"] = np.arange(1, len(t) + 1, dtype=float)
    t = t[t["stream"].isin(streams)].copy()
    t["y"] = t["y"].astype(float)
    t["xx"] = t["x"] * t["x"]
    t["xy"] = t["x"] * t["y"]
    g = t.groupby("stream")
    sums = np.stack(
        [
            g["x"].size().reindex(streams, fill_value=0).to_numpy(dtype=float),
            g["x"].sum().reindex(streams, fill_value=0.0).to_numpy(dtype=float),
            g["y"].sum().reindex(streams, fill_value=0.0).to_numpy(dtype=float),
            g["xx"].sum().reindex(streams, fill_value=0.0).to_numpy(dtype=float),
            g["xy"].sum().reindex(streams, fill_value=0.0).to_numpy(dtype=float),
        ]
    )

    def slopes(w: np.ndarray) -> np.ndarray:
        n, sx, sy, sxx, sxy = (w @ sums[i] for i in range(5))
        with np.errstate(divide="ignore", invalid="ignore"):
            var = sxx - sx * sx / n
            cov = sxy - sx * sy / n
            return np.where((n > 1) & (var > 0), cov / var, np.nan)

    k = len(streams)
    point = float(slopes(np.ones((1, k)))[0])
    rng = np.random.default_rng(seed)
    w = rng.multinomial(k, np.full(k, 1.0 / k), size=resamples).astype(float)
    draws = slopes(w)
    draws = draws[~np.isnan(draws)]
    lo, hi = (
        (float(np.percentile(draws, 5)), float(np.percentile(draws, 95)))
        if len(draws)
        else (float("nan"), float("nan"))
    )
    return OutcomeSlope(
        incidents=int(sums[0].sum()),
        slope_per_100=point * 100,
        lower=lo * 100,
        higher=hi * 100,
    )


# ---- a measure against streams seen (work item L2) -------------------------------------------
#
# The learned noticer of L1 settled within four streams, and a least-squares slope of a cumulative
# ratio over a hundred cannot see that (the review log's sixth coordinator error). L2's amended
# clause 2 reads a measure against streams seen in a window with a fixed endpoint instead: the
# background notices per stream over streams 21 to 40, learner against control, paired. These two
# functions are that reading and the curve it is read from. Both take per-stream columns (one
# arm's `results.csv`-like frame: one row per stream, the measure's count columns) and fix the
# stream order by a unique key (the seed by default), as `sample_efficiency` does.


def _ordered_counts(
    frame: pd.DataFrame, numerator: str, denominator: str | None, order_by: str
) -> tuple[np.ndarray, np.ndarray]:
    """(numerator, denominator) per stream in stream order; the denominator is one per stream
    when none is named (a mean per stream)."""
    cols = [order_by, numerator] + ([denominator] if denominator else [])
    _require(frame, cols)
    if frame[order_by].duplicated().any():
        raise ValueError(f"{order_by!r} must identify a stream uniquely")
    ordered = frame.iloc[np.argsort(frame[order_by].to_numpy(), kind="stable")]
    num = ordered[numerator].to_numpy(dtype=float)
    den = (
        ordered[denominator].to_numpy(dtype=float)
        if denominator
        else np.ones(len(ordered))
    )
    return num, den


def block_curve(
    frame: pd.DataFrame,
    numerator: str,
    denominator: str | None = None,
    block: int = 20,
    order_by: str = "seed",
) -> pd.DataFrame:
    """A measure against streams seen, in consecutive blocks of `block` streams in stream order.

    One row per block (the last may be shorter): `block` (1-based), `first` and `last` (1-based
    positions of the block's first and last stream), `streams`, `numerator` and `denominator`
    (their sums over the block) and `value`: the pooled ratio `sum(numerator) / sum(denominator)`,
    or the mean per stream of `numerator` when no `denominator` column is named (NaN when the
    block's denominator is zero). Unlike `sample_efficiency` it is not cumulative: a block
    shows what the arm does once it has seen that many streams, not an average over its whole
    history, so a change that completes in a few streams is visible as a step.
    """
    if block < 1:
        raise ValueError("block must be at least 1")
    num, den = _ordered_counts(frame, numerator, denominator, order_by)
    rows = []
    for b, lo in enumerate(range(0, len(num), block)):
        hi = min(lo + block, len(num))
        n, d = float(num[lo:hi].sum()), float(den[lo:hi].sum())
        rows.append(
            {
                "block": b + 1,
                "first": lo + 1,
                "last": hi,
                "streams": hi - lo,
                "numerator": n,
                "denominator": d,
                "value": n / d if d > 0 else float("nan"),
            }
        )
    return pd.DataFrame(rows)


@dataclass(frozen=True)
class WindowDifference:
    """The result of `paired_window_difference`: the measure of each arm over streams
    `first..last` (1-based, inclusive), their difference `a - b`, and the 5th and 95th percentiles
    of its paired cluster-bootstrap distribution (a 90% interval)."""

    first: int
    last: int
    a: float
    b: float
    point: float
    lower: float
    higher: float


def paired_window_difference(
    a: pd.DataFrame,
    b: pd.DataFrame,
    numerator: str,
    denominator: str | None = None,
    first: int = 1,
    last: int | None = None,
    order_by: str = "seed",
    resamples: int = 10_000,
    seed: int = 0,
) -> WindowDifference:
    """The difference of a measure between two arms over a window of streams, paired.

    `a` and `b` are per-stream frames of two arms over the same streams (the same `order_by`
    keys). The measure is the pooled ratio over the window, or the mean per stream with no
    `denominator`. The interval resamples the window's streams with replacement (10,000 times by
    default, multinomial counts from `numpy.random.default_rng(seed)`), the same streams for both
    arms, so that what the two arms share (a stream that is noisy for both) cancels; it is the 5th
    and 95th percentiles of the resampled differences, resamples with an empty denominator
    dropped.
    """
    na, da = _ordered_counts(a, numerator, denominator, order_by)
    nb, db = _ordered_counts(b, numerator, denominator, order_by)
    if not np.array_equal(
        np.sort(a[order_by].to_numpy()), np.sort(b[order_by].to_numpy())
    ):
        raise ValueError("the two arms are not over the same streams")
    last = len(na) if last is None else last
    if not 1 <= first <= last <= len(na):
        raise ValueError(f"the window {first}..{last} is not within 1..{len(na)}")
    sl = slice(first - 1, last)
    na, da, nb, db = na[sl], da[sl], nb[sl], db[sl]
    k = len(na)

    def ratio(n: np.ndarray, d: np.ndarray) -> float:
        return float(n.sum()) / float(d.sum()) if d.sum() > 0 else float("nan")

    point_a, point_b = ratio(na, da), ratio(nb, db)
    rng = np.random.default_rng(seed)
    w = rng.multinomial(k, np.full(k, 1.0 / k), size=resamples).astype(float)
    with np.errstate(divide="ignore", invalid="ignore"):
        diff = (w @ na) / (w @ da) - (w @ nb) / (w @ db)
    diff = diff[~np.isnan(diff)]
    lo, hi = (
        (float(np.percentile(diff, 5)), float(np.percentile(diff, 95)))
        if len(diff)
        else (float("nan"), float("nan"))
    )
    return WindowDifference(
        first=first,
        last=last,
        a=point_a,
        b=point_b,
        point=point_a - point_b,
        lower=lo,
        higher=hi,
    )
