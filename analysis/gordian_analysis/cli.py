"""gordian-analyze: paired comparison and power commands."""

from __future__ import annotations

import argparse
import json
import math
import sys
from dataclasses import asdict

import numpy as np
import pandas as pd

from . import equivalence as eq
from .breakdown import coverage_error_table, paired_class_table, risk_coverage_curve
from .drift import (
    DEFAULT_PERMUTATIONS,
    METHOD_PAIRED,
    drift_report,
    load_drift,
    margin_verdict,
    position_effect_paired,
    position_effect_stratified,
)
from .intervals import DEFAULT_RESAMPLES, paired_bootstrap_ci, ratio_of_totals_ci
from .load import (
    BILL_COLUMNS,
    MEASURED_TOTAL,
    MEASURED_VALUE_COLUMNS,
    METRICS,
    OPTIONAL_CONFIDENCE,
    LoadError,
    check_metric,
    load_pair,
    load_run,
)
from .power import equivalence_n, noninferiority_n

SMALL_N = 30

CATEGORY_TEXT = {
    eq.EQUIVALENT: "the whole interval lies strictly inside (-margin, +margin)",
    eq.BENEFICIAL: "the interval lies entirely above zero (and is not inside the margin)",
    eq.HARMFUL: "the interval lies entirely below zero (and is not inside the margin)",
    eq.UNRESOLVED: (
        "the interval is not inside the margin and does not exclude zero; "
        "this is NOT equivalence, and an underpowered comparison ends here"
    ),
}


def _clean(o):
    """Make a structure JSON-safe: non-finite floats become null, numpy scalars become Python."""
    if isinstance(o, dict):
        return {str(k): _clean(v) for k, v in o.items()}
    if isinstance(o, (list, tuple)):
        return [_clean(v) for v in o]
    if isinstance(o, (np.floating, float)):
        return float(o) if math.isfinite(o) else None
    if isinstance(o, np.integer):
        return int(o)
    if isinstance(o, np.bool_):
        return bool(o)
    return o


def _table_records(df: pd.DataFrame) -> list[dict]:
    return _clean(df.to_dict(orient="records"))


def analyze_compare(
    dir_a: str,
    dir_b: str,
    *,
    metric: str,
    margin: float,
    higher_is_better: bool,
    seed: int,
    alpha: float = 0.05,
    interval: str = "bootstrap",
    n_resamples: int = DEFAULT_RESAMPLES,
    planned_n: int | None = None,
) -> dict:
    if interval not in ("bootstrap", "t"):
        raise ValueError("interval must be 'bootstrap' or 't'")
    if not margin > 0:
        raise ValueError("margin must be positive")
    if not 0.0 < alpha < 0.5:
        raise ValueError("alpha must be in (0, 0.5)")
    _check_planned_n(planned_n)
    paired = load_pair(dir_a, dir_b)
    d = paired.differences(metric, higher_is_better)
    conf = 1.0 - 2.0 * alpha

    boot = paired_bootstrap_ci(d, seed=seed, confidence=conf, n_resamples=n_resamples)
    t_lo, t_hi = eq.t_interval(d, conf)
    tost = eq.tost_paired(d, margin, alpha)
    chosen = (boot.low, boot.high) if interval == "bootstrap" else (t_lo, t_hi)
    raw_category = eq.classify(chosen[0], chosen[1], margin)
    category = eq.gated_category(raw_category, len(d), planned_n)
    gate_applied = planned_n is not None and len(d) < planned_n
    cat_boot = eq.classify(boot.low, boot.high, margin)
    cat_t = eq.classify(t_lo, t_hi, margin)

    warnings = []
    if len(d) < SMALL_N:
        warnings.append(
            f"only {len(d)} paired episodes; both intervals are unreliable at this size "
            "(percentile bootstrap undercovers, t assumes near-normal differences)"
        )
    if float(d.std(ddof=1)) == 0.0:
        warnings.append("every paired difference is identical; intervals have zero width")
    if cat_boot != cat_t:
        warnings.append(
            f"bootstrap category ({cat_boot}) and t-interval category ({cat_t}) disagree; "
            "do not read the more favourable one"
        )
    if raw_category == eq.EQUIVALENT and not tost.rejects:
        warnings.append(
            f"category is equivalent but TOST max p = {tost.p_tost:.4g} >= alpha = {alpha}; "
            "treat as unresolved until the disagreement is explained"
        )

    a_vals, b_vals = paired.values(metric)
    out = {
        "metric": metric,
        "sign_convention": (
            "d_i = metric(B)_i - metric(A)_i; "
            + (
                "higher is better, so positive d means B is better"
                if higher_is_better
                else "lower is better, so d is negated and positive d means B is better"
            )
        ),
        "higher_is_better": higher_is_better,
        "a": {"path": str(paired.run_a.path), "run_id": paired.run_a.run_id, "mean": float(a_vals.mean())},
        "b": {"path": str(paired.run_b.path), "run_id": paired.run_b.run_id, "mean": float(b_vals.mean())},
        "usage_present": {"a": paired.run_a.usage is not None, "b": paired.run_b.usage is not None},
        "n_pairs": len(d),
        "margin": margin,
        "alpha": alpha,
        "interval_confidence": conf,
        "interval_used": interval,
        "mean_d": float(d.mean()),
        "sd_d": float(d.std(ddof=1)),
        "bootstrap": asdict(boot),
        "t_interval": {"low": t_lo, "high": t_hi, "confidence": conf},
        "tost": asdict(tost) | {"rejects": tost.rejects},
        "exploratory": planned_n is None,
        "planned_n": planned_n,
        "gate_applied": gate_applied,
        "gate_reason": eq.GATE_REASON if gate_applied else None,
        "category": category,
        "category_raw": raw_category,
        "category_bootstrap": cat_boot,
        "category_t": cat_t,
        "noninferior": eq.noninferior(chosen[0], margin),  # raw; see noninferior_reportable
        "noninferior_reportable": not gate_applied,
        "per_class": _table_records(paired_class_table(paired, metric, higher_is_better)),
        "coverage_error": {
            "a": _table_records(coverage_error_table(paired.a)),
            "b": _table_records(coverage_error_table(paired.b)),
        },
        "warnings": warnings,
    }
    for arm, frame in (("a", paired.a), ("b", paired.b)):
        if OPTIONAL_CONFIDENCE in frame.columns:
            out.setdefault("risk_coverage", {})[arm] = _table_records(risk_coverage_curve(frame))
    return _clean(out)


def _check_planned_n(planned_n: int | None) -> None:
    if planned_n is not None and (isinstance(planned_n, bool) or planned_n < 2):
        raise ValueError("planned n must be an integer of at least 2")


def _sample_size_header(r: dict) -> list[str]:
    if r["planned_n"] is None:
        return ["EXPLORATORY: no preregistered sample size"]
    return [
        f"Preregistered sample size: {r['planned_n']} pairs; observed: {r['n_pairs']} pairs "
        + ("(BELOW plan)" if r["gate_applied"] else "(plan met)")
    ]


COST_BASIS_TEXT = {
    "measured": "measured wall time, the charter's cost C",
    "measured_partial": (
        "measured wall time of one part of the episode; NOT the charter's cost C "
        f"({MEASURED_TOTAL} is C)"
    ),
    "declared": (
        "DECLARED cost (a bill column), NOT the charter's cost C; it is what the policy was "
        "charged, not what the work took, and it counts one resource only"
    ),
}


def cost_basis(metric: str, declared_cost: bool) -> str:
    """Classify a relative-savings metric, refusing a declared cost that was not asked for.

    The charter's C is measured cost (docs/local-test-plan.md, A4 and A5), so
    `measured_total_ns` is the default and a bill column needs `declared_cost=True`.
    """
    if metric in BILL_COLUMNS:
        if not declared_cost:
            raise ValueError(
                f"{metric} is declared cost, not the charter's cost C (measured cost, "
                f"{MEASURED_TOTAL}). To compute relative savings on a declared cost anyway, "
                "pass --declared-cost; the report will say so"
            )
        return "declared"
    if declared_cost:
        raise ValueError(
            f"--declared-cost applies only to a bill_* column, named with --metric; got metric "
            f"{metric!r}"
        )
    if metric == MEASURED_TOTAL:
        return "measured"
    if metric in MEASURED_VALUE_COLUMNS:
        return "measured_partial"
    check_metric(metric)
    raise ValueError(
        f"{metric!r} is not a cost metric; relative savings takes {MEASURED_TOTAL} (default), "
        f"another measured_* column, or with --declared-cost a bill_* column"
    )


def analyze_relative_savings(
    dir_a: str,
    dir_b: str,
    *,
    metric: str = MEASURED_TOTAL,
    declared_cost: bool = False,
    threshold: float,
    seed: int,
    alpha: float = 0.05,
    n_resamples: int = DEFAULT_RESAMPLES,
    planned_n: int | None = None,
) -> dict:
    """EXP-001 style cost measure: S = 1 - sum(B)/sum(A), decision S > threshold.

    The metric defaults to the measured total (the charter's C). A declared-cost bill column
    is accepted only with `declared_cost=True`, and the result then says so.
    """
    basis = cost_basis(metric, declared_cost)
    if not math.isfinite(threshold) or threshold >= 1.0:
        raise ValueError("threshold must be finite and below 1 (S cannot exceed 1)")
    if not 0.0 < alpha < 0.5:
        raise ValueError("alpha must be in (0, 0.5)")
    _check_planned_n(planned_n)
    paired = load_pair(dir_a, dir_b)
    conf = 1.0 - 2.0 * alpha
    res = ratio_of_totals_ci(paired, metric, seed, n_resamples, conf)
    raw = "exceeds" if eq.exceeds(res.low, threshold) else "does_not_exceed"
    gated = eq.gated_category(raw, res.n, planned_n)
    gate_applied = planned_n is not None and res.n < planned_n
    warnings = []
    if res.n < SMALL_N:
        warnings.append(
            f"only {res.n} paired episodes; the percentile bootstrap interval is unreliable "
            "at this size"
        )
    out = {
        "mode": "relative_savings",
        "metric": metric,
        "cost_basis": basis,
        "cost_basis_text": COST_BASIS_TEXT[basis],
        "definition": "S = 1 - sum(B)/sum(A) over paired episodes; positive S means B costs less",
        "a": {"path": str(paired.run_a.path), "run_id": paired.run_a.run_id, "total": res.sum_a},
        "b": {"path": str(paired.run_b.path), "run_id": paired.run_b.run_id, "total": res.sum_b},
        "usage_present": {"a": paired.run_a.usage is not None, "b": paired.run_b.usage is not None},
        "n_pairs": res.n,
        "alpha": alpha,
        "interval_confidence": conf,
        "threshold": threshold,
        "savings": res.savings,
        "bootstrap": asdict(res),
        "exploratory": planned_n is None,
        "planned_n": planned_n,
        "gate_applied": gate_applied,
        "gate_reason": eq.GATE_REASON if gate_applied else None,
        "decision": gated,
        "decision_raw": raw,
        "exceeds_raw": raw == "exceeds",
        "warnings": warnings,
    }
    return _clean(out)


def format_relative(r: dict) -> str:
    b = r["bootstrap"]
    pct = f"{100 * r['interval_confidence']:g}%"
    lines = _sample_size_header(r) + [
        f"Relative savings on metric: {r['metric']}",
        f"Cost basis: {r['cost_basis_text']}",
        f"A (baseline):  {r['a']['path']}  run_id={r['a']['run_id']}  total={_f(r['a']['total'])}",
        f"B (treatment): {r['b']['path']}  run_id={r['b']['run_id']}  total={_f(r['b']['total'])}",
        f"Definition: {r['definition']}.",
        f"Paired episodes (unit of replication): {r['n_pairs']}",
        f"Threshold: S > {r['threshold']:g}   alpha: {r['alpha']:g}   interval: {pct} (1 - 2*alpha)",
        "",
        f"S = {_f(r['savings'])}",
        f"Bootstrap {pct} interval: [{_f(b['low'])}, {_f(b['high'])}]  "
        f"(resamples={b['n_resamples']}, seed={b['seed']}; whole episodes resampled as pairs)",
        "",
    ]
    raw_text = "S EXCEEDS the threshold" if r["exceeds_raw"] else "S does NOT exceed the threshold"
    if r["gate_applied"]:
        lines += [
            f"DECISION: UNRESOLVED  ({eq.GATE_REASON})",
            f"Raw decision (before the sample-size gate): {raw_text} "
            "(lower limit vs threshold)",
        ]
    else:
        lines += [f"DECISION: {raw_text} (lower limit of the interval vs threshold)"]
    if r["warnings"]:
        lines += ["", "WARNINGS:"] + [f"  - {w}" for w in r["warnings"]]
    return "\n".join(lines)


def _f(x, spec=".6g"):
    return "n/a" if x is None else format(x, spec)


def _frame_text(rows: list[dict], cols: list[str]) -> str:
    df = pd.DataFrame(rows, columns=cols)
    return df.to_string(index=False, float_format=lambda v: f"{v:.4f}", na_rep="n/a")


def format_compare(r: dict) -> str:
    b, t, tost = r["bootstrap"], r["t_interval"], r["tost"]
    pct = f"{100 * r['interval_confidence']:g}%"
    lines = [
        f"Paired comparison on metric: {r['metric']}",
        f"A (baseline):  {r['a']['path']}  run_id={r['a']['run_id']}  mean={_f(r['a']['mean'])}",
        f"B (treatment): {r['b']['path']}  run_id={r['b']['run_id']}  mean={_f(r['b']['mean'])}",
        f"Sign convention: {r['sign_convention']}.",
        f"Paired episodes (unit of replication): {r['n_pairs']}",
        f"Margin: {r['margin']:g}   alpha: {r['alpha']:g}   interval: {pct} (1 - 2*alpha)",
        "",
        f"Mean of d: {_f(r['mean_d'])}   sd of d: {_f(r['sd_d'])}",
        f"Bootstrap {pct} interval: [{_f(b['low'])}, {_f(b['high'])}]  "
        f"(resamples={b['n_resamples']}, seed={b['seed']})",
        f"t {pct} interval:         [{_f(t['low'])}, {_f(t['high'])}]",
        "",
        f"TOST against +/-{r['margin']:g}: p_lower={_f(tost['p_lower'])}  "
        f"p_upper={_f(tost['p_upper'])}  max p={_f(tost['p_tost'])}  "
        f"({'rejects both nulls' if tost['rejects'] else 'does not reject both nulls'} "
        f"at alpha={r['alpha']:g})",
        "",
        f"Interval used for the category: {r['interval_used']}",
    ]
    yn = "YES" if r["noninferior"] else "NO"
    if r["gate_applied"]:
        lines += [
            f"CATEGORY: UNRESOLVED  ({eq.GATE_REASON})",
            f"Raw category (before the sample-size gate): {r['category_raw'].upper()}  "
            f"({CATEGORY_TEXT[r['category_raw']]})",
            f"Non-inferior, raw (lower limit > -margin): {yn}  "
            f"[not reportable as a verdict: {eq.GATE_REASON}]",
        ]
    else:
        lines += [
            f"CATEGORY: {r['category'].upper()}  ({CATEGORY_TEXT[r['category']]})",
            f"Non-inferior (lower limit > -margin): {yn}",
        ]
    lines = _sample_size_header(r) + lines
    if r["warnings"]:
        lines += ["", "WARNINGS:"] + [f"  - {w}" for w in r["warnings"]]
    lines += [
        "",
        "Per class (mean_d uses the sign convention above):",
        _frame_text(r["per_class"], ["class", "n", "mean_a", "mean_b", "mean_d"]),
    ]
    for arm, name in (("a", "A"), ("b", "B")):
        lines += [
            "",
            f"Coverage and error among answered, arm {name}:",
            _frame_text(
                r["coverage_error"][arm],
                ["class", "n", "n_answered", "coverage", "error_rate_answered", "undecided_rate"],
            ),
        ]
        if arm in r.get("risk_coverage", {}):
            lines += [
                f"Risk-coverage curve, arm {name} (see --json for the full curve): "
                f"{len(r['risk_coverage'][arm])} points"
            ]
    return "\n".join(lines)


def analyze_power(
    *,
    sd: float,
    margin: float,
    alpha: float,
    power: float,
    true_diff: float,
    equivalence: bool,
) -> dict:
    fn = equivalence_n if equivalence else noninferiority_n
    return _clean(asdict(fn(sd, margin, alpha, power, true_diff)))


def format_power(r: dict) -> str:
    kind = (
        "paired equivalence (TOST)" if r["design"] == "equivalence" else "paired non-inferiority"
    )
    if r["design"] == "equivalence":
        formula = "n = ((z_{1-alpha} + z_{1-beta/2}) * sd / (margin - |true_diff|))^2"
    else:
        formula = "n = ((z_{1-alpha} + z_{1-beta}) * sd / (margin + true_diff))^2"
    lines = [
        f"Sample size for {kind}, normal approximation",
        f"  {formula}, rounded up",
        f"  sd of paired differences: {r['sd']:g}",
        f"  margin: {r['margin']:g}",
        f"  alpha (one-sided, each test): {r['alpha']:g}",
        f"  target power: {r['power']:g}",
        f"  assumed true difference: {r['true_diff']:g}",
        f"  n (paired episodes) = {r['n']}   (unrounded {r['n_unrounded']:.4f})",
        f"  power at this n under the t test: {_f(r['achieved_power_t'], '.4f')}"
        + (" (lower bound)" if r["design"] == "equivalence" else ""),
    ]
    if r["achieved_power_t"] is not None and r["achieved_power_t"] < r["power"]:
        lines.append(
            "  note: the normal approximation understates n here; the t test at this n "
            "falls short of the target power"
        )
    return "\n".join(lines)


def analyze_drift(run_dir: str) -> dict:
    """The drift diagnostics of one run directory: CV and last/first ratio of `drift.csv`."""
    df = load_drift(run_dir)
    report = drift_report(df)
    return _clean(
        {
            "mode": "drift",
            "path": str(run_dir),
            "run_id": str(df["run_id"].iloc[0]),
            "n_blocks": report.n_blocks,
            "reps_per_block": report.reps_per_block,
            "ns": asdict(report.ns),
            "min_ns": asdict(report.min_ns),
            "blocks": report.blocks,
        }
    )


def format_drift(r: dict) -> str:
    def row(name, d):
        return (
            f"  {name:<7} first={_f(d['first'], '.6g')}  last={_f(d['last'], '.6g')}  "
            f"mean={_f(d['mean'], '.6g')}  sd={_f(d['sd'], '.6g')}  "
            f"CV={_f(d['cv'], '.4f')}  last/first={_f(d['ratio_last_first'], '.4f')}"
        )

    lines = [
        f"Drift control: {r['path']}  run_id={r['run_id']}",
        f"Blocks: {r['n_blocks']}, each the fixed verifier workload run {r['reps_per_block']} "
        "times (block 0 before the first episode, the last after the last episode)",
        "Timings in nanoseconds; CV = sample sd / mean across blocks.",
        row("ns", r["ns"]),
        row("min_ns", r["min_ns"]),
        "",
        _frame_text(r["blocks"], ["block", "units_done", "ns", "min_ns"]),
        "",
        "No tolerance is applied here: a tolerance on the drift is preregistered by the "
        "experiment that uses the run. The first block runs right after process start-up and "
        "may be cold; read last/first with the per-block table.",
    ]
    return "\n".join(lines)


def analyze_position(
    arm_dir: str,
    *,
    paired_with: str | None = None,
    metric: str = MEASURED_TOTAL,
    margin: float | None = None,
    seed: int,
    alpha: float = 0.05,
    n_resamples: int = DEFAULT_RESAMPLES,
    n_permutations: int = DEFAULT_PERMUTATIONS,
) -> dict:
    """Does measured cost depend on arm_position, for one arm?

    Without `paired_with` the arm is compared with itself across positions (stratified
    permutation test and bootstrap interval, because an episode is played once per arm). With
    `paired_with`, a second arm directory that played every episode identically (a copy of the
    same policy), the two copies' timings of each episode are paired.
    """
    if not 0.0 < alpha < 0.5:
        raise ValueError("alpha must be in (0, 0.5)")
    conf = 1.0 - 2.0 * alpha
    run = load_run(arm_dir)
    kwargs = dict(
        metric=metric, seed=seed, confidence=conf, n_resamples=n_resamples,
        n_permutations=n_permutations,
    )  # fmt: skip
    if paired_with is None:
        effect = position_effect_stratified(run, **kwargs)
        other = None
    else:
        other = load_run(paired_with)
        effect = position_effect_paired(run, other, **kwargs)
    verdict = None if margin is None else margin_verdict(effect, margin)
    return _clean(
        {
            "mode": "position",
            "arm": {"path": str(run.path), "run_id": run.run_id},
            "paired_with": (
                None if other is None else {"path": str(other.path), "run_id": other.run_id}
            ),
            "effect": asdict(effect),
            "alpha": alpha,
            "margin": margin,
            "verdict": verdict,
        }
    )


def format_position(r: dict) -> str:
    e = r["effect"]
    pct = f"{100 * e['confidence']:g}%"
    lines = [
        f"Position effect on {e['metric']} (log scale; cost at arm_position 0 over cost later)",
        f"Arm: {r['arm']['path']}  run_id={r['arm']['run_id']}",
    ]
    if e["method"] == METHOD_PAIRED:
        lines += [
            f"Method: paired copies; the same episodes played by {r['paired_with']['path']} "
            f"(run_id={r['paired_with']['run_id']}) in the other position; "
            f"{e['n_first']} episodes with one copy first.",
            f"Interval: paired bootstrap of the mean log ratio ({pct}, {e['n_resamples']} "
            f"resamples, seed {e['seed']}). p: sign-flip test, {e['n_permutations']} draws.",
        ]
    else:
        lines += [
            "Method: stratified permutation (an episode is played once per arm, so no episode "
            "is available in both positions); labels permuted within episode class; "
            f"{e['n_first']} episodes first, {e['n_later']} later, {e['n_strata']} classes.",
            f"Interval: bootstrap within class and position ({pct}, {e['n_resamples']} "
            f"resamples, seed {e['seed']}). p: {e['n_permutations']} permutations.",
        ]
    lines += [
        "",
        f"log ratio = {_f(e['log_ratio'], '.5f')}  ratio = {_f(e['ratio'], '.5f')}  "
        f"({_f(100 * (e['ratio'] - 1), '+.2f')}% for playing first)",
        f"{pct} interval for the log ratio: [{_f(e['low'], '.5f')}, {_f(e['high'], '.5f')}]  "
        f"ratio: [{_f(math.exp(e['low']), '.5f')}, {_f(math.exp(e['high']), '.5f')}]",
        f"two-sided p = {_f(e['p_value'], '.4f')}",
    ]
    if r["margin"] is None:
        lines += [
            "",
            "No margin given; none is applied. A p above alpha does not show that there is no "
            "position effect.",
        ]
    else:
        text = {
            "exceeds_margin": "the interval lies beyond the margin: a position effect larger "
            "than the margin is established, and the run's cost comparison is invalidated",
            "within_margin": "the whole interval is inside the margin",
            "unresolved": "the interval reaches beyond the margin and does not lie wholly "
            "beyond it; this is NOT 'no effect'",
        }[r["verdict"]]
        lines += [
            "",
            f"Margin: {100 * r['margin']:g}% relative cost  "
            f"VERDICT: {r['verdict'].upper()} ({text})",
        ]
    return "\n".join(lines)


def _metric_arg(value: str) -> str:
    try:
        check_metric(value)
    except LoadError as e:
        raise argparse.ArgumentTypeError(str(e)) from e
    return value


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(prog="gordian-analyze", description=__doc__)
    sub = p.add_subparsers(dest="command", required=True)

    c = sub.add_parser("compare", help="paired comparison of two run directories")
    c.add_argument("--a", required=True, metavar="RUNDIR", help="baseline run directory")
    c.add_argument("--b", required=True, metavar="RUNDIR", help="treatment run directory")
    c.add_argument("--metric", type=_metric_arg, metavar="METRIC",
                   help="one of: " + ", ".join(METRICS) + ". Required, except with "
                   f"--relative-savings, where it defaults to {MEASURED_TOTAL}")  # fmt: skip
    c.add_argument("--margin", type=float, help="symmetric margin, in metric units (required "
                   "unless --relative-savings)")  # fmt: skip
    g = c.add_mutually_exclusive_group()
    g.add_argument("--higher-is-better", dest="higher", action="store_true", default=None)
    g.add_argument("--lower-is-better", dest="higher", action="store_false", default=None)
    c.add_argument("--relative-savings", action="store_true",
                   help="report S = 1 - sum(B)/sum(A) for a cost metric against --threshold; "
                   "implies lower-is-better and refuses --higher-is-better")  # fmt: skip
    c.add_argument("--declared-cost", action="store_true",
                   help="with --relative-savings, allow a declared-cost bill_* column as the "
                   "metric. The report states that it is declared cost, not the charter's "
                   "cost C (measured)")  # fmt: skip
    c.add_argument("--threshold", type=float, help="S must exceed this (EXP-001: 0.20)")
    c.add_argument("--planned-n", type=int, help="preregistered number of paired episodes; "
                   "fewer observed pairs forces the category to unresolved")  # fmt: skip
    c.add_argument("--seed", required=True, type=int, help="bootstrap RNG seed")
    c.add_argument("--alpha", type=float, default=0.05)
    c.add_argument("--interval", choices=["bootstrap", "t"], default=None,
                   help="interval for the category (default bootstrap)")  # fmt: skip
    c.add_argument("--resamples", type=int, default=DEFAULT_RESAMPLES)
    c.add_argument("--json", metavar="FILE", help="also write the full result as JSON")

    w = sub.add_parser("power", help="sample size for a margin")
    w.add_argument("--sd", required=True, type=float, help="sd of paired differences")
    w.add_argument("--margin", required=True, type=float)
    w.add_argument("--alpha", type=float, default=0.05)
    w.add_argument("--power", type=float, default=0.8)
    w.add_argument("--true-diff", type=float, default=0.0)
    w.add_argument("--equivalence", action="store_true", help="TOST design instead of non-inferiority")
    w.add_argument("--json", metavar="FILE", help="also write the result as JSON")

    d = sub.add_parser("drift", help="spread and trend of the drift-control workload (drift.csv)")
    d.add_argument("--run", required=True, metavar="RUNDIR",
                   help="the run directory holding drift.csv (the interleaved run's root, "
                   "not an arm directory)")  # fmt: skip
    d.add_argument("--json", metavar="FILE", help="also write the result as JSON")

    q = sub.add_parser("position", help="does measured cost depend on arm_position, for one arm")
    q.add_argument("--arm", required=True, metavar="ARMDIR",
                   help="an arm directory of an interleaved run")  # fmt: skip
    q.add_argument("--paired-with", metavar="ARMDIR",
                   help="another arm directory that played every episode identically (a copy of "
                   "the same policy, as in an A/A run): pair the two copies' timings by "
                   "episode. Refused if the two did not play identically")  # fmt: skip
    q.add_argument("--metric", default=MEASURED_TOTAL,
                   choices=[MEASURED_TOTAL, *MEASURED_VALUE_COLUMNS],
                   help=f"measured cost to test (default {MEASURED_TOTAL})")  # fmt: skip
    q.add_argument("--margin", type=float,
                   help="relative cost, e.g. 0.05 for 5%%; report where the interval lies against "
                   "it. None is applied unless given: it is preregistered by the "
                   "experiment")  # fmt: skip
    q.add_argument("--seed", required=True, type=int, help="resampling RNG seed")
    q.add_argument("--alpha", type=float, default=0.05)
    q.add_argument("--resamples", type=int, default=DEFAULT_RESAMPLES)
    q.add_argument("--permutations", type=int, default=DEFAULT_PERMUTATIONS)
    q.add_argument("--json", metavar="FILE", help="also write the result as JSON")
    return p


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.command == "compare":
        if args.relative_savings:
            if args.higher is True:
                parser.error("--relative-savings is a cost measure (lower is better); "
                             "it refuses --higher-is-better")  # fmt: skip
            if args.threshold is None:
                parser.error("--relative-savings requires --threshold")
            if args.margin is not None or args.interval is not None:
                parser.error("--margin and --interval do not apply to --relative-savings")
        else:
            if args.threshold is not None:
                parser.error("--threshold applies only with --relative-savings")
            if args.declared_cost:
                parser.error("--declared-cost applies only with --relative-savings")
            if args.metric is None:
                parser.error("--metric is required")
            if args.margin is None:
                parser.error("--margin is required")
            if args.higher is None:
                parser.error("one of --higher-is-better / --lower-is-better is required")
    try:
        if args.command == "compare" and args.relative_savings:
            result = analyze_relative_savings(
                args.a,
                args.b,
                metric=args.metric or MEASURED_TOTAL,
                declared_cost=args.declared_cost,
                threshold=args.threshold,
                seed=args.seed,
                alpha=args.alpha,
                n_resamples=args.resamples,
                planned_n=args.planned_n,
            )
            text = format_relative(result)
        elif args.command == "compare":
            result = analyze_compare(
                args.a,
                args.b,
                metric=args.metric,
                margin=args.margin,
                higher_is_better=args.higher,
                seed=args.seed,
                alpha=args.alpha,
                interval=args.interval or "bootstrap",
                n_resamples=args.resamples,
                planned_n=args.planned_n,
            )
            text = format_compare(result)
        elif args.command == "drift":
            result = analyze_drift(args.run)
            text = format_drift(result)
        elif args.command == "position":
            result = analyze_position(
                args.arm,
                paired_with=args.paired_with,
                metric=args.metric,
                margin=args.margin,
                seed=args.seed,
                alpha=args.alpha,
                n_resamples=args.resamples,
                n_permutations=args.permutations,
            )
            text = format_position(result)
        else:
            result = analyze_power(
                sd=args.sd,
                margin=args.margin,
                alpha=args.alpha,
                power=args.power,
                true_diff=args.true_diff,
                equivalence=args.equivalence,
            )
            text = format_power(result)
    except (LoadError, ValueError) as e:
        print(f"gordian-analyze: error: {e}", file=sys.stderr)
        return 2
    print(text)
    if args.json:
        with open(args.json, "w") as fh:
            json.dump(result, fh, indent=2, allow_nan=False)
            fh.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
