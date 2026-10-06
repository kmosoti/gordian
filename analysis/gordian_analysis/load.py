"""Read run directories and pair two arms on (seed, class).

A run directory holds `results.csv` (declared and counted quantities, written by the harness)
and `measured.csv` (wall-clock timings). Both are required and are joined on (seed, class).

The charter's cost C is the modelled cost, `modelled_cost_ns` (work item A8b): the counted
operations of the components and of the shared decision rule, weighted by nanoseconds per
operation fitted to minimum timings. It is a deterministic column of `results.csv`, so a re-run
reproduces it and host interference cannot move it. The measured wall time, `measured_total_ns`,
is a secondary check on it: on this VM bursts of stolen CPU time make a ratio of wall-time totals
move by several percent with nothing changed. The `bill_*` columns are declared cost and are only
ever compared one resource at a time, because they do not share a unit.
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
    "ops_component", "ops_sched", "modelled_component_ns", "modelled_sched_ns",
]  # fmt: skip
MEASURED_COLUMNS = [
    "run_id", "seed", "class", "measured_component_ns", "measured_sched_ns",
    "measured_harness_ns",
]  # fmt: skip
# Kept for compatibility with breakdown.risk_coverage_curve. The harness does not write it yet;
# it is the one named optional column of results.csv, and any other extra column is still an
# error.
OPTIONAL_CONFIDENCE = "confidence"
# The last column of measured.csv since the interleaved runs of work item A8: the arm's position
# in the order its episode was played in (0 = first). Optional so that runs written before A8,
# including the fixtures, still load; a run that has it carries it as `arm_position` in
# `Run.results`.
OPTIONAL_ARM_POSITION = "arm_position"

BOOL_COLUMNS = ["success", "critical_miss", "false_alarm", "abstained", "undecided"]
BILL_COLUMNS = [
    "bill_compute", "bill_memory", "bill_time", "bill_probes", "bill_comm", "bill_storage",
]  # fmt: skip
MEASURED_VALUE_COLUMNS = ["measured_component_ns", "measured_sched_ns", "measured_harness_ns"]
# Counted operations (work item A8b). `ops_*` are sums of counts in units that differ between
# components, so they are sanity checks and never a cost; `modelled_*_ns` are the counts weighted
# by the calibrated nanoseconds per operation, and their sum is the charter's cost C.
OPS_COLUMNS = ["ops_component", "ops_sched"]
MODELLED_VALUE_COLUMNS = ["modelled_component_ns", "modelled_sched_ns"]
NUMERIC_COLUMNS = [
    "probes_used", "corrections", *BILL_COLUMNS, "components_run", "components_skipped",
    "directives_ignored", *OPS_COLUMNS, *MODELLED_VALUE_COLUMNS,
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
#
# `modelled_cost_ns` is the charter's cost C: the modelled cost of what a policy controls, which
# components ran and what the shared rule did with them. `measured_total_ns` also holds the
# harness's own work (generating the episode, the simulator, the ledger), which an arm does not
# choose, so the like-for-like wall time of `modelled_cost_ns` is `measured_policy_ns`, the
# component and scheduling wall times alone. Both measured totals are secondary checks.
MODELLED_TOTAL = "modelled_cost_ns"
MEASURED_TOTAL = "measured_total_ns"
MEASURED_POLICY = "measured_policy_ns"
DERIVED_METRICS = [MODELLED_TOTAL, MEASURED_TOTAL, MEASURED_POLICY]
METRICS = [
    *BOOL_COLUMNS, "probes_used", "corrections", DECISION_COLUMN, *BILL_COLUMNS, *OPS_COLUMNS,
    *MODELLED_VALUE_COLUMNS, *MEASURED_VALUE_COLUMNS, *DERIVED_METRICS,
]  # fmt: skip
# Metrics that were offered once and are now refused with an explanation, not "unknown".
REMOVED_METRICS = {
    "bill_total": (
        "bill_total was removed: it summed nanoseconds, probe counts and bytes, which have no "
        "common unit. Use modelled_cost_ns (the charter's cost C), measured_total_ns (a secondary "
        "check) or one bill_* column"
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

    `modelled_cost_ns` is the sum of the two modelled columns, nanoseconds of weighted counted
    operations. `measured_total_ns` is the sum of the three measured columns and
    `measured_policy_ns` the sum of the component and scheduling ones, all nanoseconds of wall
    time.
    """
    out = df.copy()
    out[MODELLED_TOTAL] = out[MODELLED_VALUE_COLUMNS].sum(axis=1)
    out[MEASURED_TOTAL] = out[MEASURED_VALUE_COLUMNS].sum(axis=1)
    out[MEASURED_POLICY] = out["measured_component_ns"] + out["measured_sched_ns"]
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
    raw = _read_csv(path, MEASURED_COLUMNS, optional=[OPTIONAL_ARM_POSITION])
    df = raw.copy()
    for c in MEASURED_VALUE_COLUMNS:
        df[c] = _parse_numeric(raw[c], c, where)
    if OPTIONAL_ARM_POSITION in df.columns:
        position = _parse_numeric(raw[OPTIONAL_ARM_POSITION], OPTIONAL_ARM_POSITION, where)
        if (position != np.floor(position)).any() or (position < 0).any():
            raise LoadError(f"{where}: column {OPTIONAL_ARM_POSITION!r} must be a non-negative integer")
        df[OPTIONAL_ARM_POSITION] = position.astype("int64")
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
    if OPTIONAL_ARM_POSITION in m.columns:
        out[OPTIONAL_ARM_POSITION] = m[OPTIONAL_ARM_POSITION].to_numpy()
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
            f"{path}: no measured.csv; the measured wall time is the secondary check on the "
            "charter's cost C and is required"
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


# ---------------------------------------------------------------------------------------------
# Stream runs (work items R3 and R3b)
#
# A stream run is the output of `gordian-run` on a stream manifest (crates/gordian-run/src/
# stream/): one directory per arm holding `results.csv` (one row per stream, keyed by `seed`),
# `incidents.csv` (one row per incident, keyed by `(seed, incident)`) and `measured.csv`, and the
# run directory's own `manifest.json` and `drift.csv`. The unit of replication is the stream, not
# the incident: a stream holds about 27 incidents that share a graph, a noise process and
# recurrences, so counts are pooled over streams and ratios are ratios of pooled counts
# (`gordian_analysis.stream`). This loader is separate from the episode loader above, which it
# does not touch: the files have another key (`seed` alone), other columns and another meaning.
#
# `tier`, `family`, `critical` and the verdict columns are hidden-side facts written by the
# evaluator for analysis. They are never a policy input and never training data
# (crates/gordian-run/HARNESS.md, section 11).
# ---------------------------------------------------------------------------------------------

# The exact column sets the stream harness writes (crates/gordian-run/src/stream/results.rs:
# RESULTS_HEADER, INCIDENTS_HEADER, MEASURED_HEADER). A test parses those constants and compares.
STREAM_RESULTS_COLUMNS = [
    "run_id", "arm_role", "seed", "duration_ns", "observations", "anomalies_noticed",
    "probes_used", "declarations", "declared_incident", "declared_dismissal",
    "incidents_plain", "incidents_hard", "incidents_decoy", "critical_incidents",
    "correct_plain", "correct_hard", "missed_plain", "missed_hard",
    "critical_missed_plain", "critical_missed_hard", "wrong_declarations",
    "decoys_dismissed", "decoys_alarmed", "decoys_silent", "false_alarms",
    "false_alarms_on_background",
    "escalations_needed", "escalations_unneeded", "escalations_background",
    "hard_incidents_escalated", "other_incidents_escalated", "calls_informed", "calls_correct",
    "reasoner_calls", "reasoner_refs", "reasoner_tokens", "reasoner_modelled_ns",
    "reasoner_latency_ns", "cheap_declarations", "reasoner_declarations",
    "escalations_refused", "probes_refused", "calls_unanswered", "reasoner_cost_ns",
    "bill_compute", "bill_probes", "bill_time", "bill_comm",
    "components_run", "components_skipped", "rule_skipped", "steps", "stop_reason",
    "ops_component", "ops_sched", "modelled_component_ns", "modelled_sched_ns", "substrate_ns",
    "total_cost_ns",
]  # fmt: skip
STREAM_INCIDENTS_COLUMNS = [
    "run_id", "arm_role", "seed", "incident", "tier", "family", "critical",
    "correct_declarations", "wrong_declarations", "first_correct_at_ns",
    "time_to_first_correct_ns", "correct_by_deadline", "missed", "critical_miss",
    "escalations", "informed_escalations", "correct_escalations",
]  # fmt: skip
STREAM_MEASURED_COLUMNS = [
    "run_id", "seed", "measured_component_ns", "measured_sched_ns", "measured_harness_ns",
    "arm_position",
]  # fmt: skip

STREAM_STRING_COLUMNS = ["run_id", "arm_role", "stop_reason"]
# Every other column of results.csv is a non-negative integer count, instant or duration.
STREAM_COUNT_COLUMNS = [c for c in STREAM_RESULTS_COLUMNS if c not in STREAM_STRING_COLUMNS]
STREAM_MEASURED_VALUE_COLUMNS = MEASURED_VALUE_COLUMNS
# The roles `arm_role` takes (`ArmRole::as_str`, crates/gordian-run/src/stream/arms/mod.rs):
# an analysis compares `comparison` arms; the other two are references and say so on every row.
STREAM_ARM_ROLES = ("comparison", "privileged", "ablation")
COMPARISON_ROLE = "comparison"
# The tiers (`tier_name`, results.rs) and the hard-fault families (`family_name`, score.rs). A
# family is a hard incident's and only a hard incident's; it is empty for the other tiers.
STREAM_TIERS = ("plain", "hard", "decoy")
STREAM_FAMILIES = ("compound", "cascade", "split_brain", "slow_leak")
STREAM_INCIDENT_COUNT_COLUMNS = [
    "incident", "correct_declarations", "wrong_declarations", "escalations",
    "informed_escalations", "correct_escalations",
]  # fmt: skip
STREAM_INCIDENT_BOOL_COLUMNS = ["critical", "correct_by_deadline", "missed", "critical_miss"]
STREAM_INCIDENT_OPTIONAL_COLUMNS = ["first_correct_at_ns", "time_to_first_correct_ns"]
STREAM_KEY = ["seed"]
STREAM_INCIDENT_KEY = ["seed", "incident"]

# The notice files of work item B1 (crates/gordian-run/src/stream/results.rs: NOTICES_HEADER,
# NOTICE_INCIDENTS_HEADER, NOTICE_EVENTS_HEADER), written beside the three files above, which they
# leave byte for byte as they were. All three are present or none is (a run made before the seam
# has none). The measures are the evaluator's (crates/gordian-stream-eval/RULES.md, N1 to N12).
STREAM_NOTICES_COLUMNS = [
    "run_id", "arm_role", "seed", "noticer", "notices", "notices_on_background",
    "notices_on_plain", "notices_on_hard", "notices_on_decoy", "retirements",
    "noticed_plain", "noticed_hard", "noticed_decoy",
    "anchor_correct_plain", "anchor_correct_hard", "anchor_correct_decoy",
]  # fmt: skip
STREAM_NOTICE_INCIDENTS_COLUMNS = [
    "run_id", "arm_role", "seed", "incident", "tier", "family", "first_observation_at_ns",
    "notices", "noticed", "first_notice_at_ns", "notice_latency_ns", "anchor_correct",
]  # fmt: skip
STREAM_NOTICE_EVENTS_COLUMNS = [
    "run_id", "arm_role", "seed", "noticer", "event", "anomaly", "anchor", "site",
    "anchor_at_ns", "at_ns", "incident", "anchor_offset_ns", "anchor_correct",
]  # fmt: skip
STREAM_NOTICES_COUNT_COLUMNS = [
    c for c in STREAM_NOTICES_COLUMNS
    if c not in ("run_id", "arm_role", "noticer")
]  # fmt: skip
STREAM_NOTICE_EVENT_KINDS = ("notice", "retire")
STREAM_NOTICE_FILES = ("notices.csv", "notice_incidents.csv", "notice_events.csv")


@dataclass
class StreamArm:
    """One arm of a stream run: its per-stream and per-incident tables, both keyed on `seed`.

    `notices`, `notice_incidents` and `notice_events` are the arm's notice files (work item B1),
    `None` for a run made before the notice seam: per stream (keyed on `seed`), per incident
    (keyed on `(seed, incident)`, with the same tier and family as `incidents`) and per notice or
    retirement, in the order recorded.
    """

    path: Path
    name: str
    run_id: str
    role: str
    results: pd.DataFrame
    incidents: pd.DataFrame
    notices: pd.DataFrame | None = None
    notice_incidents: pd.DataFrame | None = None
    notice_events: pd.DataFrame | None = None


@dataclass
class StreamRun:
    """A stream run directory: its arms in directory-name order, and what sits beside them."""

    path: Path
    arms: dict[str, StreamArm]
    manifest: dict | None
    usage: dict | None


def _parse_count(series: pd.Series, column: str, where: str) -> pd.Series:
    """A non-negative integer column, read exactly (counts reach 1e12, so never through float)."""
    text = series.str.strip()
    bad = ~text.str.fullmatch(r"[0-9]{1,18}")
    if bad.any():
        pos = int(np.argmax(bad.to_numpy()))
        raise LoadError(
            f"{where}: column {column!r} has {series.iloc[pos]!r} (row {pos + 2}); "
            "expected a non-negative integer"
        )
    return text.astype("int64")


def _read_stream_csv(path: Path, expected: list[str], allow_empty: bool = False) -> pd.DataFrame:
    """Read a stream CSV as strings; its columns must be exactly the ones the harness writes."""
    where = str(path)
    raw = pd.read_csv(path, dtype=str, keep_default_na=False)
    if list(raw.columns) != expected:
        missing = [c for c in expected if c not in raw.columns]
        unknown = [c for c in raw.columns if c not in expected]
        raise LoadError(
            f"{where}: columns are not the stream schema of crates/gordian-run/src/stream/"
            f"results.rs (missing {missing}, unknown {unknown}, or out of order); a change to "
            "that schema needs a change here"
        )
    if len(raw) == 0 and not allow_empty:
        raise LoadError(f"{where}: no rows")
    return raw


def _single_value(raw: pd.DataFrame, column: str, where: str) -> str:
    values = sorted(raw[column].str.strip().unique())
    if len(values) != 1 or values[0] == "":
        raise LoadError(f"{where}: expected exactly one {column}, found {values}")
    return values[0]


def _single_role(raw: pd.DataFrame, where: str) -> str:
    role = _single_value(raw, "arm_role", where)
    if role not in STREAM_ARM_ROLES:
        raise LoadError(f"{where}: arm_role {role!r} is not one of {list(STREAM_ARM_ROLES)}")
    return role


def _load_stream_results(path: Path) -> pd.DataFrame:
    where = str(path)
    raw = _read_stream_csv(path, STREAM_RESULTS_COLUMNS)
    df = pd.DataFrame({"run_id": raw["run_id"].str.strip()})
    _single_value(raw, "run_id", where)
    df["arm_role"] = _single_role(raw, where)
    for c in STREAM_COUNT_COLUMNS:
        df[c] = _parse_count(raw[c], c, where)
    df["stop_reason"] = raw["stop_reason"].str.strip()
    if (df["stop_reason"] == "").any():
        raise LoadError(f"{where}: empty 'stop_reason' value")
    if df.duplicated(STREAM_KEY).any():
        seeds = df.loc[df.duplicated(STREAM_KEY, keep=False), "seed"].unique()[:5].tolist()
        raise LoadError(f"{where}: duplicate seeds, e.g. {seeds}")
    # Identities the harness guarantees. A violation means the file is not the harness's.
    calls = df["escalations_needed"] + df["escalations_unneeded"] + df["escalations_background"]
    bad = calls != df["reasoner_calls"]
    if bad.any():
        pos = int(np.argmax(bad.to_numpy()))
        raise LoadError(
            f"{where}: row {pos + 2}: escalations_needed + escalations_unneeded + "
            "escalations_background is not reasoner_calls"
        )
    bad = df["substrate_ns"] + df["reasoner_cost_ns"] != df["total_cost_ns"]
    if bad.any():
        pos = int(np.argmax(bad.to_numpy()))
        raise LoadError(
            f"{where}: row {pos + 2}: substrate_ns + reasoner_cost_ns is not total_cost_ns"
        )
    return df[STREAM_RESULTS_COLUMNS]


def _load_stream_incidents(path: Path) -> pd.DataFrame:
    where = str(path)
    raw = _read_stream_csv(path, STREAM_INCIDENTS_COLUMNS)
    df = pd.DataFrame({"run_id": raw["run_id"].str.strip()})
    _single_value(raw, "run_id", where)
    df["arm_role"] = _single_role(raw, where)
    df["seed"] = _parse_count(raw["seed"], "seed", where)
    for c in STREAM_INCIDENT_COUNT_COLUMNS:
        df[c] = _parse_count(raw[c], c, where)
    df["tier"] = raw["tier"].str.strip()
    bad = ~df["tier"].isin(STREAM_TIERS)
    if bad.any():
        pos = int(np.argmax(bad.to_numpy()))
        raise LoadError(
            f"{where}: tier {raw['tier'].iloc[pos]!r} (row {pos + 2}) is not one of "
            f"{list(STREAM_TIERS)}"
        )
    df["family"] = raw["family"].str.strip()
    hard = df["tier"] == "hard"
    bad = hard != df["family"].isin(STREAM_FAMILIES)
    if bad.any():
        pos = int(np.argmax(bad.to_numpy()))
        raise LoadError(
            f"{where}: row {pos + 2}: a hard incident has one of {list(STREAM_FAMILIES)} as its "
            f"family and no other incident has one (tier {df['tier'].iloc[pos]!r}, family "
            f"{df['family'].iloc[pos]!r})"
        )
    for c in STREAM_INCIDENT_BOOL_COLUMNS:
        df[c] = _parse_bool(raw[c], c, where)
    # A first correct declaration exists exactly when there is a correct declaration (S8).
    none_correct = (df["correct_declarations"] == 0).to_numpy()
    for c in STREAM_INCIDENT_OPTIONAL_COLUMNS:
        text = raw[c].str.strip()
        empty = text == ""
        parsed = _parse_count(text.where(~empty, "0"), c, where).astype("Int64")
        df[c] = parsed.mask(empty)
        bad = empty.to_numpy() != none_correct
        if bad.any():
            pos = int(np.argmax(bad))
            raise LoadError(
                f"{where}: row {pos + 2}: {c!r} is empty exactly when the incident has no correct "
                "declaration"
            )
    if df.duplicated(STREAM_INCIDENT_KEY).any():
        keys = df.loc[df.duplicated(STREAM_INCIDENT_KEY, keep=False), STREAM_INCIDENT_KEY]
        raise LoadError(
            f"{where}: duplicate (seed, incident) keys, e.g. {keys.head(3).values.tolist()}"
        )
    return df[STREAM_INCIDENTS_COLUMNS]


def _check_incidents_against_results(results: pd.DataFrame, incidents: pd.DataFrame, where: Path):
    """The two files describe the same streams: same seeds, and the tier counts agree."""
    r = results.set_index("seed")
    unknown = sorted(set(incidents["seed"]) - set(r.index))
    if unknown:
        raise LoadError(f"{where}: incidents.csv has seeds {unknown[:3]} that results.csv lacks")
    tiers = incidents.groupby(["seed", "tier"]).size().unstack(fill_value=0)
    for tier in STREAM_TIERS:
        have = tiers[tier] if tier in tiers else pd.Series(0, index=tiers.index)
        have = have.reindex(r.index, fill_value=0)
        bad = have.to_numpy() != r[f"incidents_{tier}"].to_numpy()
        if bad.any():
            seed = int(r.index[int(np.argmax(bad))])
            raise LoadError(
                f"{where}: seed {seed}: results.csv counts {int(r.loc[seed, f'incidents_{tier}'])} "
                f"{tier} incidents, incidents.csv has {int(have.loc[seed])}"
            )
    for column in ("run_id", "arm_role"):
        if set(incidents[column]) != set(results[column]):
            raise LoadError(f"{where}: {column} differs between results.csv and incidents.csv")


def _optional_count(raw: pd.DataFrame, column: str, where: str) -> pd.Series:
    """A count column that is empty when there is nothing to count, as nullable integers."""
    text = raw[column].str.strip()
    empty = text == ""
    parsed = _parse_count(text.where(~empty, "0"), column, where).astype("Int64")
    return parsed.mask(empty)


def _load_stream_notices(path: Path, results: pd.DataFrame) -> pd.DataFrame:
    """`notices.csv`: one row per stream, the stream's counts of notices (N8 to N10)."""
    where = str(path)
    raw = _read_stream_csv(path, STREAM_NOTICES_COLUMNS)
    df = pd.DataFrame({"run_id": raw["run_id"].str.strip()})
    _single_value(raw, "run_id", where)
    df["arm_role"] = _single_role(raw, where)
    df["noticer"] = _single_value(raw, "noticer", where)
    for c in STREAM_NOTICES_COUNT_COLUMNS:
        df[c] = _parse_count(raw[c], c, where)
    if df.duplicated(STREAM_KEY).any():
        raise LoadError(f"{where}: duplicate seeds")
    if set(df["seed"]) != set(results["seed"]):
        raise LoadError(f"{path.parent}: results.csv and notices.csv do not have the same seeds")
    by_anchor = (
        df["notices_on_background"]
        + df["notices_on_plain"]
        + df["notices_on_hard"]
        + df["notices_on_decoy"]
    )
    bad = by_anchor != df["notices"]
    if bad.any():
        pos = int(np.argmax(bad.to_numpy()))
        raise LoadError(f"{where}: row {pos + 2}: the notices by anchor do not add up to notices")
    # The harness guarantees one notice per anomaly the arm's rung reports noticing.
    r = results.set_index("seed")["anomalies_noticed"]
    bad = df.set_index("seed")["notices"] != r.reindex(df["seed"]).to_numpy()
    if bad.any():
        seed = int(df["seed"].iloc[int(np.argmax(bad.to_numpy()))])
        raise LoadError(f"{where}: seed {seed}: notices is not results.csv's anomalies_noticed")
    for tier in STREAM_TIERS:
        bad = df[f"anchor_correct_{tier}"] > df[f"noticed_{tier}"]
        if bad.any():
            pos = int(np.argmax(bad.to_numpy()))
            raise LoadError(f"{where}: row {pos + 2}: more anchor-correct than noticed {tier}")
    return df[STREAM_NOTICES_COLUMNS]


def _load_stream_notice_incidents(path: Path, incidents: pd.DataFrame) -> pd.DataFrame:
    """`notice_incidents.csv`: one row per incident (N1 to N6), the same incidents as
    `incidents.csv`."""
    where = str(path)
    raw = _read_stream_csv(path, STREAM_NOTICE_INCIDENTS_COLUMNS)
    df = pd.DataFrame({"run_id": raw["run_id"].str.strip()})
    _single_value(raw, "run_id", where)
    df["arm_role"] = _single_role(raw, where)
    for c in ("seed", "incident", "notices"):
        df[c] = _parse_count(raw[c], c, where)
    df["tier"] = raw["tier"].str.strip()
    df["family"] = raw["family"].str.strip()
    for c in ("noticed", "anchor_correct"):
        df[c] = _parse_bool(raw[c], c, where)
    optional = ("first_observation_at_ns", "first_notice_at_ns", "notice_latency_ns")
    for c in optional:
        df[c] = _optional_count(raw, c, where)
    if df.duplicated(STREAM_INCIDENT_KEY).any():
        raise LoadError(f"{where}: duplicate (seed, incident) keys")
    mine = df.sort_values(STREAM_INCIDENT_KEY).reset_index(drop=True)
    theirs = incidents.sort_values(STREAM_INCIDENT_KEY).reset_index(drop=True)
    key = ["seed", "incident", "tier", "family"]
    if not mine[key].equals(theirs[key]):
        raise LoadError(
            f"{where}: not the incidents of incidents.csv (same seed, incident, tier and family "
            "on every row)"
        )
    # N2, N3, N4, N5: what holds together.
    bad = df["noticed"] != (df["notices"] > 0)
    if bad.any():
        raise LoadError(f"{where}: row {int(np.argmax(bad.to_numpy())) + 2}: noticed is not 'notices > 0'")
    bad = df["first_notice_at_ns"].isna() == df["noticed"]
    if bad.any():
        raise LoadError(
            f"{where}: row {int(np.argmax(bad.to_numpy())) + 2}: first_notice_at_ns is empty "
            "exactly when the incident was not noticed"
        )
    has_latency = df["notice_latency_ns"].notna()
    bad = has_latency != (df["noticed"] & df["first_observation_at_ns"].notna())
    if bad.any():
        raise LoadError(
            f"{where}: row {int(np.argmax(bad.to_numpy())) + 2}: notice_latency_ns is empty "
            "unless the incident was noticed and has a first observation"
        )
    bad = df["anchor_correct"] & ~df["noticed"]
    if bad.any():
        raise LoadError(f"{where}: row {int(np.argmax(bad.to_numpy())) + 2}: anchor-correct but not noticed")
    return df[STREAM_NOTICE_INCIDENTS_COLUMNS]


def _load_stream_notice_events(path: Path) -> pd.DataFrame:
    """`notice_events.csv`: every notice and retirement the noticer recorded, in order (N7)."""
    where = str(path)
    raw = _read_stream_csv(path, STREAM_NOTICE_EVENTS_COLUMNS, allow_empty=True)
    if len(raw) == 0:
        return pd.DataFrame(columns=STREAM_NOTICE_EVENTS_COLUMNS)
    df = pd.DataFrame({"run_id": raw["run_id"].str.strip()})
    _single_value(raw, "run_id", where)
    df["arm_role"] = _single_role(raw, where)
    df["noticer"] = _single_value(raw, "noticer", where)
    for c in ("seed", "anomaly", "anchor", "site", "anchor_at_ns", "at_ns"):
        df[c] = _parse_count(raw[c], c, where)
    df["event"] = raw["event"].str.strip()
    bad = ~df["event"].isin(STREAM_NOTICE_EVENT_KINDS)
    if bad.any():
        pos = int(np.argmax(bad.to_numpy()))
        raise LoadError(f"{where}: event {raw['event'].iloc[pos]!r} (row {pos + 2}) is not one of "
                        f"{list(STREAM_NOTICE_EVENT_KINDS)}")
    for c in ("incident", "anchor_offset_ns"):
        df[c] = _optional_count(raw, c, where)
    text = raw["anchor_correct"].str.strip()
    notice = df["event"] == "notice"
    # A retirement has no verdict; a notice has one, true or false.
    if ((text == "") != ~notice).any():
        raise LoadError(f"{where}: anchor_correct is empty exactly on retirements")
    parsed = _parse_bool(text.where(text != "", "false"), "anchor_correct", where)
    df["anchor_correct"] = parsed.astype("boolean").mask(~notice)
    bad = df["at_ns"] < df["anchor_at_ns"]
    if bad.any():
        raise LoadError(f"{where}: row {int(np.argmax(bad.to_numpy())) + 2}: an event before its anchor")
    return df[STREAM_NOTICE_EVENTS_COLUMNS]


def _check_notice_files(
    path: Path,
    results: pd.DataFrame,
    notices: pd.DataFrame,
    notice_incidents: pd.DataFrame,
    notice_events: pd.DataFrame,
):
    """The three notice files describe the same streams and add up to one another (N8 to N10)."""
    for column in ("run_id", "arm_role"):
        if set(notice_incidents[column]) != set(results[column]) or set(notices[column]) != set(
            results[column]
        ):
            raise LoadError(f"{path}: {column} differs between results.csv and the notice files")
    n = notices.set_index("seed")
    g = notice_incidents.groupby("seed")
    on_incidents = g["notices"].sum().reindex(n.index, fill_value=0)
    expected = n["notices"] - n["notices_on_background"]
    bad = on_incidents.to_numpy() != expected.to_numpy()
    if bad.any():
        seed = int(n.index[int(np.argmax(bad))])
        raise LoadError(
            f"{path}: seed {seed}: notice_incidents.csv counts {int(on_incidents.loc[seed])} "
            f"notices on incidents, notices.csv {int(expected.loc[seed])}"
        )
    for tier in STREAM_TIERS:
        sub = notice_incidents[notice_incidents["tier"] == tier].groupby("seed")
        for flag, column in (("noticed", f"noticed_{tier}"), ("anchor_correct", f"anchor_correct_{tier}")):
            have = sub[flag].sum().reindex(n.index, fill_value=0)
            bad = have.to_numpy() != n[column].to_numpy()
            if bad.any():
                seed = int(n.index[int(np.argmax(bad))])
                raise LoadError(f"{path}: seed {seed}: {column} disagrees with notice_incidents.csv")
    if len(notice_events):
        events = notice_events.groupby(["seed", "event"]).size().unstack(fill_value=0)
        for kind, column in (("notice", "notices"), ("retire", "retirements")):
            have = (events[kind] if kind in events else pd.Series(0, index=events.index)).reindex(
                n.index, fill_value=0
            )
            bad = have.to_numpy() != n[column].to_numpy()
            if bad.any():
                seed = int(n.index[int(np.argmax(bad))])
                raise LoadError(f"{path}: seed {seed}: notice_events.csv disagrees with {column}")
        if set(notice_events["noticer"]) != set(notices["noticer"]):
            raise LoadError(f"{path}: the noticer differs between notices.csv and notice_events.csv")
    elif int(n["notices"].sum() + n["retirements"].sum()) != 0:
        raise LoadError(f"{path}: notice_events.csv is empty but notices.csv counts notices")


def _load_stream_measured(path: Path, results: pd.DataFrame) -> pd.DataFrame:
    """Attach the measured columns and `arm_position` to results on `seed`."""
    where = str(path)
    raw = _read_stream_csv(path, STREAM_MEASURED_COLUMNS)
    df = pd.DataFrame({"seed": _parse_count(raw["seed"], "seed", where)})
    for c in [*STREAM_MEASURED_VALUE_COLUMNS, "arm_position"]:
        df[c] = _parse_count(raw[c], c, where)
    if df.duplicated("seed").any():
        raise LoadError(f"{where}: duplicate seeds")
    if set(df["seed"]) != set(results["seed"]):
        raise LoadError(f"{path.parent}: results.csv and measured.csv do not have the same seeds")
    return results.merge(df, on="seed", how="left", validate="one_to_one")


def load_stream_arm(path: str | Path, name: str | None = None) -> StreamArm:
    """Read one arm directory of a stream run: `results.csv` and `incidents.csv` (both required)
    and `measured.csv` (joined on `seed` when present).

    Fails (`LoadError`) on other columns than the harness writes, a value that is not what its
    column holds, an incident file that disagrees with the results file about which streams
    exist or how many incidents of each tier they hold, or an identity the harness guarantees.
    """
    path = Path(path)
    results_path = path / "results.csv"
    incidents_path = path / "incidents.csv"
    for p in (results_path, incidents_path):
        if not p.is_file():
            raise LoadError(f"{path}: no {p.name}")
    results = _load_stream_results(results_path)
    incidents = _load_stream_incidents(incidents_path)
    _check_incidents_against_results(results, incidents, path)
    measured_path = path / "measured.csv"
    if measured_path.is_file():
        results = _load_stream_measured(measured_path, results)
    results = results.sort_values("seed").reset_index(drop=True)
    incidents = incidents.sort_values(STREAM_INCIDENT_KEY).reset_index(drop=True)
    notices = notice_incidents = notice_events = None
    present = [(path / f).is_file() for f in STREAM_NOTICE_FILES]
    if any(present) and not all(present):
        missing = [f for f, p in zip(STREAM_NOTICE_FILES, present) if not p]
        raise LoadError(f"{path}: the notice files come together; missing {missing}")
    if all(present):
        notices = _load_stream_notices(path / "notices.csv", results)
        notice_incidents = _load_stream_notice_incidents(path / "notice_incidents.csv", incidents)
        notice_events = _load_stream_notice_events(path / "notice_events.csv")
        _check_notice_files(path, results, notices, notice_incidents, notice_events)
        notices = notices.sort_values("seed").reset_index(drop=True)
        notice_incidents = notice_incidents.sort_values(STREAM_INCIDENT_KEY).reset_index(drop=True)
    return StreamArm(
        path=path,
        name=name or path.name,
        run_id=str(results["run_id"].iloc[0]),
        role=str(results["arm_role"].iloc[0]),
        results=results,
        incidents=incidents,
        notices=notices,
        notice_incidents=notice_incidents,
        notice_events=notice_events,
    )


def _read_json(path: Path) -> dict | None:
    if not path.is_file():
        return None
    try:
        return json.loads(path.read_text())
    except json.JSONDecodeError as e:
        raise LoadError(f"{path}: invalid JSON: {e}") from e


def load_stream_run(path: str | Path) -> StreamRun:
    """Read a stream run directory: every subdirectory holding `results.csv` is an arm.

    A directory that holds `results.csv` itself is read as a run of that one arm. All arms must
    have played the same seeds.
    """
    path = Path(path)
    if not path.is_dir():
        raise LoadError(f"{path}: not a directory")
    arm_dirs = sorted(p for p in path.iterdir() if p.is_dir() and (p / "results.csv").is_file())
    if not arm_dirs and (path / "results.csv").is_file():
        arm_dirs = [path]
    if not arm_dirs:
        raise LoadError(f"{path}: no arm directory with a results.csv")
    arms = {p.name: load_stream_arm(p) for p in arm_dirs}
    seeds = {name: tuple(arm.results["seed"]) for name, arm in arms.items()}
    first = next(iter(seeds.values()))
    for name, s in seeds.items():
        if s != first:
            raise LoadError(f"{path}: arm {name!r} played other seeds than the first arm")
    return StreamRun(
        path=path,
        arms=arms,
        manifest=_read_json(path / "manifest.json"),
        usage=_read_json(path / "usage.json"),
    )


@dataclass
class PairedStreams:
    """Two arms aligned stream for stream on `seed`. `a` is baseline, `b` is treatment.

    `results_a` and `results_b` have the same seeds in the same order; `incidents_a` and
    `incidents_b` the same `(seed, incident)` keys in the same order, and the same tier, family
    and criticality on every row, because the incidents of a stream do not depend on the arm.
    """

    a: StreamArm
    b: StreamArm
    results_a: pd.DataFrame
    results_b: pd.DataFrame
    incidents_a: pd.DataFrame
    incidents_b: pd.DataFrame

    def __len__(self) -> int:
        return len(self.results_a)


def pair_streams(a: StreamArm, b: StreamArm) -> PairedStreams:
    """Join two arms on `seed`. Fails on a seed present in one arm only, and on incidents that
    differ between the arms (two arms of one run have played the same streams; arms from runs
    with other stream parameters have not, and must not be paired)."""
    ka = a.results.set_index("seed")
    kb = b.results.set_index("seed")
    only_a = ka.index.difference(kb.index)
    only_b = kb.index.difference(ka.index)
    if len(only_a) or len(only_b):
        raise LoadError(
            f"unmatched streams: {len(only_a)} only in {a.name!r} (e.g. {list(only_a[:3])}), "
            f"{len(only_b)} only in {b.name!r} (e.g. {list(only_b[:3])})"
        )
    order = ka.index.sort_values()
    ia = a.incidents.set_index(STREAM_INCIDENT_KEY).sort_index()
    ib = b.incidents.set_index(STREAM_INCIDENT_KEY).sort_index()
    if not ia.index.equals(ib.index):
        raise LoadError(
            f"arms {a.name!r} and {b.name!r} have different incidents; they did not play the "
            "same streams"
        )
    for c in ("tier", "family", "critical"):
        if not (ia[c].to_numpy() == ib[c].to_numpy()).all():
            raise LoadError(
                f"arms {a.name!r} and {b.name!r} disagree about the {c} of an incident; they "
                "did not play the same streams"
            )
    return PairedStreams(
        a=a,
        b=b,
        results_a=ka.loc[order].reset_index(),
        results_b=kb.loc[order].reset_index(),
        incidents_a=ia.reset_index(),
        incidents_b=ib.reset_index(),
    )


def pair_arms(run: StreamRun, a: str, b: str) -> PairedStreams:
    """Pair two named arms of one run."""
    for name in (a, b):
        if name not in run.arms:
            raise LoadError(f"{run.path}: no arm {name!r}; the arms are {sorted(run.arms)}")
    return pair_streams(run.arms[a], run.arms[b])
