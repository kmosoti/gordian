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
from .intervals import DEFAULT_RESAMPLES, paired_bootstrap_ci
from .load import METRICS, OPTIONAL_CONFIDENCE, LoadError, load_pair
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
) -> dict:
    if interval not in ("bootstrap", "t"):
        raise ValueError("interval must be 'bootstrap' or 't'")
    if not margin > 0:
        raise ValueError("margin must be positive")
    if not 0.0 < alpha < 0.5:
        raise ValueError("alpha must be in (0, 0.5)")
    paired = load_pair(dir_a, dir_b)
    d = paired.differences(metric, higher_is_better)
    conf = 1.0 - 2.0 * alpha

    boot = paired_bootstrap_ci(d, seed=seed, confidence=conf, n_resamples=n_resamples)
    t_lo, t_hi = eq.t_interval(d, conf)
    tost = eq.tost_paired(d, margin, alpha)
    chosen = (boot.low, boot.high) if interval == "bootstrap" else (t_lo, t_hi)
    category = eq.classify(chosen[0], chosen[1], margin)
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
    if category == eq.EQUIVALENT and not tost.rejects:
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
        "category": category,
        "category_bootstrap": cat_boot,
        "category_t": cat_t,
        "noninferior": eq.noninferior(chosen[0], margin),
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
        f"CATEGORY: {r['category'].upper()}  ({CATEGORY_TEXT[r['category']]})",
        f"Non-inferior (lower limit > -margin): {'YES' if r['noninferior'] else 'NO'}",
    ]
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
                ["class", "n", "n_answered", "coverage", "error_rate_answered"],
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


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(prog="gordian-analyze", description=__doc__)
    sub = p.add_subparsers(dest="command", required=True)

    c = sub.add_parser("compare", help="paired comparison of two run directories")
    c.add_argument("--a", required=True, metavar="RUNDIR", help="baseline run directory")
    c.add_argument("--b", required=True, metavar="RUNDIR", help="treatment run directory")
    c.add_argument("--metric", required=True, choices=METRICS)
    c.add_argument("--margin", required=True, type=float, help="symmetric margin, in metric units")
    g = c.add_mutually_exclusive_group(required=True)
    g.add_argument("--higher-is-better", dest="higher", action="store_true")
    g.add_argument("--lower-is-better", dest="higher", action="store_false")
    c.add_argument("--seed", required=True, type=int, help="bootstrap RNG seed")
    c.add_argument("--alpha", type=float, default=0.05)
    c.add_argument("--interval", choices=["bootstrap", "t"], default="bootstrap")
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
    return p


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        if args.command == "compare":
            result = analyze_compare(
                args.a,
                args.b,
                metric=args.metric,
                margin=args.margin,
                higher_is_better=args.higher,
                seed=args.seed,
                alpha=args.alpha,
                interval=args.interval,
                n_resamples=args.resamples,
            )
            text = format_compare(result)
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
