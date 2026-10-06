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
