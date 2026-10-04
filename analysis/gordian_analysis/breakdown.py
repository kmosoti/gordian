"""Per-class tables, abstention behaviour, and the risk-coverage curve.

All rows are episodes. Classes with few episodes are reported, not dropped.
"""

from __future__ import annotations

import numpy as np
import pandas as pd

from .load import METRICS, OPTIONAL_CONFIDENCE, PairedRuns


def per_class_table(df: pd.DataFrame, metrics: list[str] | None = None) -> pd.DataFrame:
    """Long table: class, metric, n, mean, sd (sd with ddof=1; NaN when n = 1)."""
    metrics = list(METRICS if metrics is None else metrics)
    rows = []
    for cls, g in df.groupby("class", sort=True):
        for m in metrics:
            x = g[m].to_numpy(dtype=float)
            rows.append(
                {
                    "class": cls,
                    "metric": m,
                    "n": len(x),
                    "mean": float(x.mean()),
                    "sd": float(x.std(ddof=1)) if len(x) > 1 else float("nan"),
                }
            )
    return pd.DataFrame(rows, columns=["class", "metric", "n", "mean", "sd"])


def coverage_error_table(df: pd.DataFrame) -> pd.DataFrame:
    """Per class: coverage = 1 - mean(abstained); error_rate = 1 - mean(success) among
    answered (non-abstained) episodes, NaN when none were answered. A final 'ALL' row pools
    episodes.
    """
    rows = []
    groups = [(c, g) for c, g in df.groupby("class", sort=True)] + [("ALL", df)]
    for cls, g in groups:
        abst = g["abstained"].to_numpy(dtype=bool)
        ans = g.loc[~abst]
        n_ans = len(ans)
        rows.append(
            {
                "class": cls,
                "n": len(g),
                "n_answered": n_ans,
                "coverage": 1.0 - float(abst.mean()),
                "error_rate_answered": (
                    1.0 - float(ans["success"].to_numpy(dtype=bool).mean())
                    if n_ans
                    else float("nan")
                ),
            }
        )
    return pd.DataFrame(rows)


def risk_coverage_curve(df: pd.DataFrame) -> pd.DataFrame:
    """Risk-coverage curve over answered episodes, ranked by `confidence` (descending).

    For each distinct confidence value c (tied episodes enter together, so the result does
    not depend on row order): accepted = answered episodes with confidence >= c,
    coverage = |accepted| / N (N counts every episode, abstained included, so the last
    point's coverage equals the policy's actual coverage), risk = fraction of accepted
    episodes that are not successes.
    """
    if OPTIONAL_CONFIDENCE not in df.columns:
        raise KeyError("no 'confidence' column")
    n_all = len(df)
    ans = df.loc[~df["abstained"].to_numpy(dtype=bool)]
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
