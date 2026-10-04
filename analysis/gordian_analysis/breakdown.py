"""Per-class tables, abstention behaviour, and the risk-coverage curve.

All rows are episodes. Classes with few episodes are reported, not dropped.
"""

from __future__ import annotations

import numpy as np
import pandas as pd

from .load import METRICS, OPTIONAL_CONFIDENCE, PairedRuns


def per_class_table(df: pd.DataFrame, metrics: list[str] | None = None) -> pd.DataFrame:
    """Long table: class, metric, n, n_missing, mean, sd (sd with ddof=1; NaN when n = 1).

    Only `decision_at_ns` can be missing (undecided episodes have no decision time). For it,
    mean and sd are over the episodes that have a value, `n` counts those, and `n_missing`
    says how many were left out. This is descriptive and conditions on having decided; the
    paired comparison refuses the metric instead.
    """
    metrics = list(METRICS if metrics is None else metrics)
    rows = []
    for cls, g in df.groupby("class", sort=True):
        for m in metrics:
            all_x = g[m].to_numpy(dtype=float)
            x = all_x[~np.isnan(all_x)]
            rows.append(
                {
                    "class": cls,
                    "metric": m,
                    "n": len(x),
                    "n_missing": len(all_x) - len(x),
                    "mean": float(x.mean()) if len(x) else float("nan"),
                    "sd": float(x.std(ddof=1)) if len(x) > 1 else float("nan"),
                }
            )
    return pd.DataFrame(rows, columns=["class", "metric", "n", "n_missing", "mean", "sd"])


def _answered(df: pd.DataFrame) -> np.ndarray:
    """True for episodes that gave an answer: not abstained and not undecided (evaluator R9)."""
    return ~df["abstained"].to_numpy(dtype=bool) & ~df["undecided"].to_numpy(dtype=bool)


def coverage_error_table(df: pd.DataFrame) -> pd.DataFrame:
    """Per class: coverage = answered / n; error_rate = 1 - mean(success) among answered.

    Answered means neither abstained nor undecided: an episode that ran out of budget or
    horizon never gave an answer, so it is not coverage and not an answered error. It is
    reported in `undecided_rate` instead, and it still counts as a failure in the `success`
    metric. NaN error when none were answered. A final 'ALL' row pools episodes.
    """
    rows = []
    groups = [(c, g) for c, g in df.groupby("class", sort=True)] + [("ALL", df)]
    for cls, g in groups:
        ans_mask = _answered(g)
        ans = g.loc[ans_mask]
        n_ans = len(ans)
        rows.append(
            {
                "class": cls,
                "n": len(g),
                "n_answered": n_ans,
                "coverage": n_ans / len(g),
                "error_rate_answered": (
                    1.0 - float(ans["success"].to_numpy(dtype=bool).mean())
                    if n_ans
                    else float("nan")
                ),
                "undecided_rate": float(g["undecided"].to_numpy(dtype=bool).mean()),
            }
        )
    return pd.DataFrame(rows)


def risk_coverage_curve(df: pd.DataFrame) -> pd.DataFrame:
    """Risk-coverage curve over answered episodes, ranked by `confidence` (descending).

    For each distinct confidence value c (tied episodes enter together, so the result does
    not depend on row order): accepted = answered episodes with confidence >= c,
    coverage = |accepted| / N (N counts every episode, abstained and undecided included, so
    the last point's coverage equals the policy's actual coverage), risk = fraction of
    accepted episodes that are not successes.
    """
    if OPTIONAL_CONFIDENCE not in df.columns:
        raise KeyError("no 'confidence' column")
    n_all = len(df)
    ans = df.loc[_answered(df)]
    if len(ans) == 0:
        return pd.DataFrame(columns=["threshold", "n_accepted", "coverage", "risk"])
    conf = ans[OPTIONAL_CONFIDENCE].to_numpy(dtype=float)
    err = (~ans["success"].to_numpy(dtype=bool)).astype(float)
    thresholds = np.sort(np.unique(conf))[::-1]
    rows = []
    for c in thresholds:
        take = conf >= c
        k = int(take.sum())
        rows.append(
            {
                "threshold": float(c),
                "n_accepted": k,
                "coverage": k / n_all,
                "risk": float(err[take].mean()),
            }
        )
    return pd.DataFrame(rows)


def paired_class_table(paired: PairedRuns, metric: str, higher_is_better: bool) -> pd.DataFrame:
    """Per class: n pairs, mean of A, mean of B, mean of d (sign convention applied once)."""
    a, b = paired.values(metric)
    d = paired.differences(metric, higher_is_better)
    frame = pd.DataFrame({"class": paired.a["class"].to_numpy(), "a": a, "b": b, "d": d})
    out = frame.groupby("class", sort=True).agg(
        n=("d", "size"), mean_a=("a", "mean"), mean_b=("b", "mean"), mean_d=("d", "mean")
    )
    return out.reset_index()
