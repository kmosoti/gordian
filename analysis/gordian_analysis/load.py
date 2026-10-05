"""Read run directories and pair two arms on (seed, class).

A run directory holds `results.csv` (declared quantities, written by the harness) and
`measured.csv` (wall-clock timings). Both are required and are joined on (seed, class). The
charter's cost C is the measured cost, `measured_total_ns`; the `bill_*` columns are declared
cost and are only ever compared one resource at a time, because they do not share a unit.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path

import numpy as np
import pandas as pd

KEY = ["seed", "class"]

# The exact column sets the harness writes (crates/gordian-run/src/results.rs). Anything else is
# an error, so a change to the schema is noticed here rather than silently ignored.
RESULTS_COLUMNS = [
    "run_id", "seed", "class", "success", "critical_miss", "false_alarm", "abstained",
    "undecided", "probes_used", "corrections", "decision_at_ns", "bill_compute", "bill_memory",
    "bill_time", "bill_probes", "bill_comm", "bill_storage", "components_run",
    "components_skipped", "directives_ignored", "stop_reason",
]  # fmt: skip
MEASURED_COLUMNS = [
    "run_id", "seed", "class", "measured_component_ns", "measured_sched_ns",
    "measured_harness_ns",
]  # fmt: skip
# Kept for compatibility with breakdown.risk_coverage_curve. The harness does not write it yet;
# it is the one named optional column, and any other extra column is still an error.
OPTIONAL_CONFIDENCE = "confidence"

BOOL_COLUMNS = ["success", "critical_miss", "false_alarm", "abstained", "undecided"]
BILL_COLUMNS = [
    "bill_compute", "bill_memory", "bill_time", "bill_probes", "bill_comm", "bill_storage",
]  # fmt: skip
MEASURED_VALUE_COLUMNS = ["measured_component_ns", "measured_sched_ns", "measured_harness_ns"]
NUMERIC_COLUMNS = [
    "probes_used", "corrections", *BILL_COLUMNS, "components_run", "components_skipped",
    "directives_ignored",
]  # fmt: skip
# `decision_at_ns` is empty exactly when the episode is undecided (evaluator R9); it is parsed
# separately and is NaN for those rows.
DECISION_COLUMN = "decision_at_ns"
STRING_COLUMNS = ["stop_reason"]
# The values `stop_reason` takes (`StopReason::as_str` in crates/gordian-run/src/harness.rs), and
# whether each is a decided episode (the others are undecided, evaluator R9). `final_declaration`
# is a terminal step made at the harness's final call, after the arm ran out of affordable work or
# reached the horizon: scored like any other decision, and kept apart so that "answered while it
# had means" and "answered when it had none" can be told apart. The loader does not check values
# against this table (a new reason must not make a run unreadable); a test compares the table
# with the harness.
STOP_REASON_DECIDED = {
    "terminal": True, "final_declaration": True, "budget_exhausted": False, "horizon": False,
    "step_cap": False,
}  # fmt: skip
# Derived in code only; input files are never modified.
MEASURED_TOTAL = "measured_total_ns"
DERIVED_METRICS = [MEASURED_TOTAL]
METRICS = [
    *BOOL_COLUMNS, "probes_used", "corrections", DECISION_COLUMN, *BILL_COLUMNS,
    *MEASURED_VALUE_COLUMNS, *DERIVED_METRICS,
]  # fmt: skip
# Metrics that were offered once and are now refused with an explanation, not "unknown".
REMOVED_METRICS = {
    "bill_total": (
        "bill_total was removed: it summed nanoseconds, probe counts and bytes, which have no "
        "common unit. Use measured_total_ns (the charter's cost C) or one bill_* column"
    ),
}


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


def _parse_decision(series: pd.Series, undecided: pd.Series, where: str) -> pd.Series:
    """`decision_at_ns` is empty exactly when the episode is undecided (evaluator R9).

    Empty on a decided row, or a value on an undecided row, is an error. Undecided rows get
    NaN, which is never used silently: `PairedRuns.values` refuses it.
    """
    text = series.str.strip()
    empty = (text == "").to_numpy()
    und = undecided.to_numpy(dtype=bool)
    bad = empty & ~und
    if bad.any():
        pos = int(np.argmax(bad))
        raise LoadError(
            f"{where}: column {DECISION_COLUMN!r} is empty on a decided episode "
            f"(row {pos + 2}); only undecided episodes have no decision time"
        )
    bad = ~empty & und
    if bad.any():
        pos = int(np.argmax(bad))
        raise LoadError(
            f"{where}: column {DECISION_COLUMN!r} has value {series.iloc[pos]!r} on an "
            f"undecided episode (row {pos + 2}); an episode that never decided has no "
            "decision time"
        )
    # Parse the whole column (empty placeholders as 0) so error row numbers stay accurate.
    out = _parse_numeric(series.where(~empty, "0"), DECISION_COLUMN, where)
    out[empty] = np.nan
    return out


def with_derived(df: pd.DataFrame) -> pd.DataFrame:
    """Return df with derived metrics added (a copy; the input is untouched).

    `measured_total_ns` is the sum of the three measured columns, all nanoseconds of wall time.
    """
    out = df.copy()
    out[MEASURED_TOTAL] = out[MEASURED_VALUE_COLUMNS].sum(axis=1)
    return out


def _read_csv(path: Path, expected: list[str], optional: list[str] = ()) -> pd.DataFrame:
    """Read a CSV as strings and check its columns against the exact set the harness writes."""
    where = str(path)
    raw = pd.read_csv(path, dtype=str, keep_default_na=False)
    missing = [c for c in expected if c not in raw.columns]
    if missing:
        raise LoadError(f"{where}: missing columns {missing}")
    unknown = [c for c in raw.columns if c not in expected and c not in optional]
    if unknown:
        raise LoadError(
            f"{where}: unknown columns {unknown}; the schema is the one in "
            "crates/gordian-run/src/results.rs, and a change to it needs a change here"
        )
    if raw.columns.duplicated().any():
        raise LoadError(f"{where}: duplicate column names")
    if len(raw) == 0:
        raise LoadError(f"{where}: no rows")
    return raw


def _parse_key(raw: pd.DataFrame, df: pd.DataFrame, where: str) -> None:
    seed = _parse_numeric(raw["seed"], "seed", where)
    if (seed != np.floor(seed)).any():
        raise LoadError(f"{where}: column 'seed' has non-integer values")
    df["seed"] = seed.astype("int64")
    df["class"] = raw["class"].str.strip()
    if (df["class"] == "").any():
        raise LoadError(f"{where}: empty 'class' value")
    run_ids = sorted(raw["run_id"].unique())
    if len(run_ids) != 1:
        raise LoadError(f"{where}: expected exactly one run_id, found {run_ids}")
    dup = df.duplicated(KEY, keep=False)
    if dup.any():
        keys = df.loc[dup, KEY].drop_duplicates().head(5).values.tolist()
        raise LoadError(f"{where}: duplicate (seed, class) keys, e.g. {keys}")


def _load_results(path: Path) -> pd.DataFrame:
    where = str(path)
    raw = _read_csv(path, RESULTS_COLUMNS, optional=[OPTIONAL_CONFIDENCE])
    df = raw.copy()
    for c in BOOL_COLUMNS:
        df[c] = _parse_bool(raw[c], c, where)
    for c in NUMERIC_COLUMNS:
        df[c] = _parse_numeric(raw[c], c, where)
    if OPTIONAL_CONFIDENCE in df.columns:
        df[OPTIONAL_CONFIDENCE] = _parse_numeric(
            raw[OPTIONAL_CONFIDENCE], OPTIONAL_CONFIDENCE, where
        )
    df[DECISION_COLUMN] = _parse_decision(raw[DECISION_COLUMN], df["undecided"], where)
    for c in STRING_COLUMNS:
        df[c] = raw[c].str.strip()
        if (df[c] == "").any():
            raise LoadError(f"{where}: empty {c!r} value")
    _parse_key(raw, df, where)
    return df


def _load_measured(path: Path) -> pd.DataFrame:
    where = str(path)
    raw = _read_csv(path, MEASURED_COLUMNS)
    df = raw.copy()
    for c in MEASURED_VALUE_COLUMNS:
        df[c] = _parse_numeric(raw[c], c, where)
    _parse_key(raw, df, where)
    return df


def _join_measured(results: pd.DataFrame, measured: pd.DataFrame, where: Path) -> pd.DataFrame:
    """Attach the measured columns to results on (seed, class); any mismatch is an error."""
    kr = results.set_index(KEY).index
    km = measured.set_index(KEY).index
    only_r = kr.difference(km)
    only_m = km.difference(kr)
    if len(only_r) or len(only_m):
        raise LoadError(
            f"{where}: results.csv and measured.csv do not have the same (seed, class) keys: "
            f"{len(only_r)} only in results.csv (e.g. {list(only_r[:3])}), "
            f"{len(only_m)} only in measured.csv (e.g. {list(only_m[:3])})"
        )
    m = measured.set_index(KEY).loc[kr].reset_index()
    if not (m["run_id"].to_numpy() == results["run_id"].to_numpy()).all():
        raise LoadError(f"{where}: run_id differs between results.csv and measured.csv")
    out = results.copy()
    for c in MEASURED_VALUE_COLUMNS:
        out[c] = m[c].to_numpy()
    return out


def load_run(path: str | Path) -> Run:
    """Read results.csv and measured.csv (and usage.json if present) from a run directory."""
    path = Path(path)
    results_path = path / "results.csv"
    measured_path = path / "measured.csv"
    if not results_path.is_file():
        raise LoadError(f"{path}: no results.csv")
    results = _load_results(results_path)
    if not measured_path.is_file():
        raise LoadError(
            f"{path}: no measured.csv; measured cost is the charter's cost C and is required"
        )
    df = _join_measured(results, _load_measured(measured_path), path)
    run_ids = sorted(df["run_id"].unique())

    usage = None
    usage_path = path / "usage.json"
    if usage_path.is_file():
        try:
            usage = json.loads(usage_path.read_text())
        except json.JSONDecodeError as e:
            raise LoadError(f"{usage_path}: invalid JSON: {e}") from e

    return Run(path=path, run_id=run_ids[0], results=with_derived(df), usage=usage)


def check_metric(metric: str) -> None:
    """Raise LoadError for an unknown metric; a removed one gets its own explanation."""
    if metric in REMOVED_METRICS:
        raise LoadError(REMOVED_METRICS[metric])
    if metric not in METRICS:
        raise LoadError(f"unknown metric {metric!r}; choose from {METRICS}")


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
        check_metric(metric)
        a = self.a[metric].to_numpy(dtype=float)
        b = self.b[metric].to_numpy(dtype=float)
        if np.isnan(a).any() or np.isnan(b).any():
            # Only decision_at_ns can be missing. Dropping those pairs would condition the
            # comparison on the outcome (the arm that fails to decide would drop out), and
            # filling them in would invent a decision time, so neither is done here.
            raise LoadError(
                f"metric {metric!r} is undefined for undecided episodes "
                f"({int(np.isnan(a).sum())} in A, {int(np.isnan(b).sum())} in B); "
                "it is not computed over the decided episodes alone. Compare 'undecided' "
                "and 'success' instead"
            )
        return a, b

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
