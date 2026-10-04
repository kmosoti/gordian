"""Read run directories and pair two arms on (seed, class)."""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path

import numpy as np
import pandas as pd

KEY = ["seed", "class"]

REQUIRED_COLUMNS = [
    "run_id", "seed", "class", "success", "critical_miss", "false_alarm", "abstained",
    "probes_used", "decision_at_ns", "bill_compute", "bill_memory", "bill_time",
    "bill_probes", "bill_comm", "bill_storage", "components_run", "components_skipped",
]  # fmt: skip

BOOL_COLUMNS = ["success", "critical_miss", "false_alarm", "abstained"]
BILL_COLUMNS = [
    "bill_compute", "bill_memory", "bill_time", "bill_probes", "bill_comm", "bill_storage",
]  # fmt: skip
NUMERIC_COLUMNS = ["probes_used", "decision_at_ns", *BILL_COLUMNS]
# Derived in code only; input files are never modified.
DERIVED_METRICS = ["bill_total"]
METRICS = [*BOOL_COLUMNS, "probes_used", "decision_at_ns", *BILL_COLUMNS, *DERIVED_METRICS]
OPTIONAL_CONFIDENCE = "confidence"


class LoadError(ValueError):
    """Raised for malformed, duplicated, or unmatched run data."""


@dataclass
class Run:
    path: Path
    run_id: str
    results: pd.DataFrame
    usage: dict | None


_TRUE = {"true", "1"}
_FALSE = {"false", "0"}


def _parse_bool(series: pd.Series, column: str, where: str) -> pd.Series:
    low = series.str.strip().str.lower()
    bad = ~low.isin(_TRUE | _FALSE)
    if bad.any():
        pos = int(np.argmax(bad.to_numpy()))
        raise LoadError(
            f"{where}: column {column!r} has non-boolean value {series.iloc[pos]!r} "
            f"(row {pos + 2}); expected true/false/0/1"
        )
    return low.isin(_TRUE)


def _parse_numeric(series: pd.Series, column: str, where: str) -> pd.Series:
    out = pd.to_numeric(series.str.strip(), errors="coerce").astype(float)
    bad = ~np.isfinite(out.to_numpy())
    if bad.any():
        pos = int(np.argmax(bad))
        raise LoadError(
            f"{where}: column {column!r} has non-numeric or non-finite value "
            f"{series.iloc[pos]!r} (row {pos + 2})"
        )
    return out


def with_derived(df: pd.DataFrame) -> pd.DataFrame:
    """Return df with derived metrics added (a copy; the input is untouched)."""
    out = df.copy()
    out["bill_total"] = out[BILL_COLUMNS].sum(axis=1)
    return out


def load_run(path: str | Path) -> Run:
    """Read results.csv (and usage.json if present) from a run directory."""
    path = Path(path)
    csv_path = path / "results.csv"
    if not csv_path.is_file():
        raise LoadError(f"{path}: no results.csv")
    where = str(csv_path)
    raw = pd.read_csv(csv_path, dtype=str, keep_default_na=False)
    missing = [c for c in REQUIRED_COLUMNS if c not in raw.columns]
    if missing:
        raise LoadError(f"{where}: missing columns {missing}")
    if len(raw) == 0:
        raise LoadError(f"{where}: no rows")

    df = raw.copy()
    for c in BOOL_COLUMNS:
        df[c] = _parse_bool(raw[c], c, where)
    for c in NUMERIC_COLUMNS:
        df[c] = _parse_numeric(raw[c], c, where)
    if OPTIONAL_CONFIDENCE in df.columns:
        df[OPTIONAL_CONFIDENCE] = _parse_numeric(
            raw[OPTIONAL_CONFIDENCE], OPTIONAL_CONFIDENCE, where
        )
    seed = _parse_numeric(raw["seed"], "seed", where)
    if (seed != np.floor(seed)).any():
        raise LoadError(f"{where}: column 'seed' has non-integer values")
    df["seed"] = seed.astype("int64")
    df["class"] = raw["class"].str.strip()
    if (df["class"] == "").any():
        raise LoadError(f"{where}: empty 'class' value")

    run_ids = sorted(df["run_id"].unique())
    if len(run_ids) != 1:
        raise LoadError(f"{where}: expected exactly one run_id, found {run_ids}")

    dup = df.duplicated(KEY, keep=False)
    if dup.any():
        keys = df.loc[dup, KEY].drop_duplicates().head(5).values.tolist()
        raise LoadError(f"{where}: duplicate (seed, class) keys, e.g. {keys}")

    usage = None
    usage_path = path / "usage.json"
    if usage_path.is_file():
        try:
            usage = json.loads(usage_path.read_text())
        except json.JSONDecodeError as e:
            raise LoadError(f"{usage_path}: invalid JSON: {e}") from e

    return Run(path=path, run_id=run_ids[0], results=with_derived(df), usage=usage)


@dataclass
class PairedRuns:
    """Two arms aligned row for row on (seed, class). `a` is baseline, `b` is treatment."""

    a: pd.DataFrame
    b: pd.DataFrame
    run_a: Run
    run_b: Run

    def __len__(self) -> int:
        return len(self.a)

    def values(self, metric: str) -> tuple[np.ndarray, np.ndarray]:
        if metric not in METRICS:
            raise LoadError(f"unknown metric {metric!r}; choose from {METRICS}")
        return (
            self.a[metric].to_numpy(dtype=float),
            self.b[metric].to_numpy(dtype=float),
        )

    def differences(self, metric: str, higher_is_better: bool) -> np.ndarray:
        """d_i = metric(B)_i - metric(A)_i; negated once if lower is better.

        Positive always means B (treatment) is better. This is the only place the sign
        convention is applied.
        """
        a, b = self.values(metric)
        d = b - a
        return d if higher_is_better else -d


def pair_runs(run_a: Run, run_b: Run) -> PairedRuns:
    """Join two arms on (seed, class). Fails on any key present in only one arm."""
    ka = run_a.results.set_index(KEY)
    kb = run_b.results.set_index(KEY)
    for name, k in (("A", ka), ("B", kb)):
        if not k.index.is_unique:
            raise LoadError(f"arm {name}: duplicate (seed, class) keys")
    only_a = ka.index.difference(kb.index)
    only_b = kb.index.difference(ka.index)
    if len(only_a) or len(only_b):
        raise LoadError(
            f"unmatched pairs: {len(only_a)} only in A (e.g. {list(only_a[:3])}), "
            f"{len(only_b)} only in B (e.g. {list(only_b[:3])})"
        )
    order = ka.index.sort_values()
    return PairedRuns(
        a=ka.loc[order].reset_index(),
        b=kb.loc[order].reset_index(),
        run_a=run_a,
        run_b=run_b,
    )


def load_pair(dir_a: str | Path, dir_b: str | Path) -> PairedRuns:
    return pair_runs(load_run(dir_a), load_run(dir_b))
