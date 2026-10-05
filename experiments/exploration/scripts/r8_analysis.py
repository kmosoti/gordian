"""R8: the analysis of a run's `calls.jsonl`, and nothing else is read for the scores.

Usage: r8_analysis.py CALLS.jsonl --design DESIGN.json --out-prefix experiments/exploration/r8-main-
                      [--label main] [--bootstrap 10000]
       r8_analysis.py selftest

Reads the call file (the design file only for the exclusions it recorded before any call), re-parses
every stored response with `r8_common.parse_answer` and checks it against the stored parse, and
writes, under the prefix:

* `levels.csv`: per set (hard, plain) and level (0, 50, 100, 200, 400, control): calls, correct,
  A(m) with a 90% cluster-bootstrap interval (and a Wilson interval beside it, calls treated as
  independent), parse failures, transport errors;
* `fit.csv` and `outcome.json`: p0, A(0), delta-hat with its 90% interval, the precondition and the
  outcome (see `r8_stats`), for the hard set; the plain set's fit is written labelled "proxy";
* `position.csv`: accuracy by the position of the first decisive reference (first, middle, last third
  of the context);
* `family.csv`: accuracy by hard family and level;
* `tokens.csv`: tokens per reference, against the stream's declared 20;
* `timing.csv`: wall time per call and the server's prompt and decode timings by level;
* `counts.csv`: calls, parse failures, errors and the design's exclusions.

Exploration (nothing here tests a hypothesis). An estimate for one small model is not a claim about
real models in general; "not significant" is never "equivalent".
"""

import argparse
import collections
import csv
import hashlib
import json
import math
import os
import sys

import numpy as np

import r8_common as C
import r8_stats as S

COLUMNS = (0, 50, 100, 200, 400, "control")
Z90 = 1.6448536269514722


def load_calls(paths):
    """The calls of one or more call files (the stages of one model's run), in order. Each call
    keeps its `call_id` and gains `source`, the file's name, so ids stay unique across files."""
    calls = []
    for path in [paths] if isinstance(paths, str) else paths:
        with open(path) as f:
            for line in f:
                if line.strip():
                    c = json.loads(line)
                    c["source"] = os.path.basename(os.path.dirname(os.path.abspath(path)))
                    calls.append(c)
    return calls


def reparse(calls):
    """Parse every stored response again and require it to agree with what the runner stored."""
    bad = []
    for c in calls:
        if c["error"] is not None:
            parsed = ("fail", "transport", None)
        else:
            parsed = C.parse_answer(c["response"], c["n_services"])
        stored = (
            c["parse_status"],
            c["parsed_kind"] if c["parse_status"] == "ok" else c["parse_failure_reason"],
            c["parsed_site"],
        )
        if parsed != stored:
            bad.append((c["call_id"], parsed, stored))
    return bad


def wilson(k, n, z=Z90):
    if n == 0:
        return (float("nan"), float("nan"))
    p = k / n
    den = 1 + z * z / n
    centre = (p + z * z / (2 * n)) / den
    half = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / den
    return (centre - half, centre + half)


def complete_matrix(calls, tier):
    """Incidents of `tier` that have a call at every level and the control: (keys, 0/1 matrix)."""
    by = collections.defaultdict(dict)
    for c in calls:
        if c["tier"] == tier:
            by[c["qkey"]][c["level"]] = int(c["correct"])
    keys = sorted(k for k, v in by.items() if all(col in v for col in COLUMNS))
    mat = np.array([[by[k][col] for col in COLUMNS] for k in keys], dtype=float)
    return keys, mat, len(by)


def levels_rows(calls, tier, mat, b, seed):
    rows = []
    idx = S.bootstrap_indices(len(mat), b, seed) if len(mat) else None
    boot = mat[idx].mean(axis=1) if idx is not None else None
    for j, col in enumerate(COLUMNS):
        cs = [c for c in calls if c["tier"] == tier and c["level"] == col]
        k = sum(1 for c in cs if c["correct"])
        lo, hi = S.percentile_interval(boot[:, j]) if boot is not None else (math.nan, math.nan)
        wl, wh = wilson(k, len(cs))
        rows.append(
            {
                "set": tier.lower(),
                "level": col,
                "calls": len(cs),
                "correct": k,
                "A": k / len(cs) if cs else math.nan,
                "A_complete_cases": float(mat[:, j].mean()) if len(mat) else math.nan,
                "boot_lo": lo,
                "boot_hi": hi,
                "wilson_lo": wl,
                "wilson_hi": wh,
                "parse_failures": sum(1 for c in cs if c["parse_status"] != "ok"),
                "transport_errors": sum(1 for c in cs if c["error"] is not None),
                "answers_none": sum(1 for c in cs if c["parsed_kind"] == "NONE"),
            }
        )
    return rows


def third(c):
    n = c["context_lines"]
    p = c["first_decisive_position"]
    if p is None or n == 0:
        return None
    x = (p + 0.5) / n
    return "first" if x < 1 / 3 else ("middle" if x < 2 / 3 else "last")


def position_rows(calls):
    rows = []
    for tier in ("Hard", "Plain"):
        for label, pred in (
            ("levels 50-400 pooled", lambda c: c["level"] in (50, 100, 200, 400)),
            ("level 50", lambda c: c["level"] == 50),
            ("level 100", lambda c: c["level"] == 100),
            ("level 200", lambda c: c["level"] == 200),
            ("level 400", lambda c: c["level"] == 400),
        ):
            for where in ("first", "middle", "last"):
                cs = [
                    c
                    for c in calls
                    if c["tier"] == tier and c["control"] is False and pred(c) and third(c) == where
                ]
                k = sum(1 for c in cs if c["correct"])
                wl, wh = wilson(k, len(cs))
                rows.append(
                    {
                        "set": tier.lower(),
                        "levels": label,
                        "third": where,
                        "calls": len(cs),
                        "correct": k,
                        "A": k / len(cs) if cs else math.nan,
                        "wilson_lo": wl,
                        "wilson_hi": wh,
                    }
                )
    return rows


def family_rows(calls):
    rows = []
    fams = sorted({c["family"] for c in calls if c["tier"] == "Hard"})
    for fam in fams:
        for col in COLUMNS:
            cs = [c for c in calls if c["tier"] == "Hard" and c["family"] == fam and c["level"] == col]
            k = sum(1 for c in cs if c["correct"])
            rows.append(
                {
                    "family": fam,
                    "level": col,
                    "calls": len(cs),
                    "correct": k,
                    "A": k / len(cs) if cs else math.nan,
                }
            )
    return rows


def token_rows(calls):
    rows = []
    xs, ys = [], []
    for c in calls:
        if c["context_tokens"] is not None and c["context_lines"] > 0:
            xs.append(c["context_lines"])
            ys.append(c["context_tokens"])
    slope = icpt = math.nan
    if len(set(xs)) > 1:
        slope, icpt = np.polyfit(np.array(xs, float), np.array(ys, float), 1)
    for tier in ("Hard", "Plain"):
        for col in COLUMNS:
            cs = [
                c
                for c in calls
                if c["tier"] == tier and c["level"] == col and c["context_tokens"] is not None
            ]
            lines = sum(c["context_lines"] for c in cs)
            toks = sum(c["context_tokens"] for c in cs)
            rows.append(
                {
                    "set": tier.lower(),
                    "level": col,
                    "calls": len(cs),
                    "mean_context_lines": lines / len(cs) if cs else math.nan,
                    "mean_context_tokens": toks / len(cs) if cs else math.nan,
                    "tokens_per_reference": toks / lines if lines else math.nan,
                    "declared_tokens_per_reference": 20,
                }
            )
    rows.append(
        {
            "set": "all",
            "level": "slope",
            "calls": len(xs),
            "mean_context_lines": math.nan,
            "mean_context_tokens": icpt,
            "tokens_per_reference": slope,
            "declared_tokens_per_reference": 20,
        }
    )
    return rows


def timing_rows(calls):
    rows = []
    xs, ys = [], []
    for col in COLUMNS:
        cs = [c for c in calls if c["level"] == col and c["timings"]]
        if not cs:
            continue

        def mean(key):
            return sum(c["timings"][key] for c in cs) / len(cs)

        rows.append(
            {
                "level": col,
                "calls": len(cs),
                "mean_latency_s": sum(c["latency_s"] for c in cs) / len(cs),
                "mean_prompt_tokens": sum(c["usage"]["prompt_tokens"] for c in cs) / len(cs),
                "mean_prompt_tokens_evaluated": mean("prompt_n"),
                "mean_prompt_tokens_cached": mean("cache_n"),
                "mean_prompt_ms": mean("prompt_ms"),
                "mean_predicted_tokens": mean("predicted_n"),
                "mean_predicted_ms": mean("predicted_ms"),
            }
        )
    for c in calls:
        if c["timings"]:
            xs.append(c["usage"]["prompt_tokens"] - c["timings"]["cache_n"])
            ys.append(c["latency_s"])
    if len(set(xs)) > 1:
        slope, icpt = np.polyfit(np.array(xs, float), np.array(ys, float), 1)
        rows.append(
            {
                "level": "fit: latency_s = a + b * evaluated prompt tokens",
                "calls": len(xs),
                "mean_latency_s": icpt,
                "mean_prompt_tokens": math.nan,
                "mean_prompt_tokens_evaluated": slope,
                "mean_prompt_tokens_cached": math.nan,
                "mean_prompt_ms": math.nan,
                "mean_predicted_tokens": math.nan,
                "mean_predicted_ms": math.nan,
            }
        )
    return rows


def write_csv(path, rows):
    if not rows:
        return
    with open(path, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0].keys()))
        w.writeheader()
        for r in rows:
            w.writerow({k: (f"{v:.6g}" if isinstance(v, float) else v) for k, v in r.items()})


def fit_row(label, res):
    lo, hi = res["delta_interval"]
    lo_f, hi_f = res["delta_interval_p0_fixed"]
    return {
        "set": label,
        "n_incidents": res["n_incidents"],
        "p0": res["p0"],
        "p0_lo": res["p0_interval"][0],
        "p0_hi": res["p0_interval"][1],
        "A0": res["A"][0],
        "A0_lo": res["A0_lower"],
        "A0_hi": res["A_interval"][0][1],
        "A0_minus_p0_lo": res["A0_minus_p0_interval"][0],
        "A0_minus_p0_hi": res["A0_minus_p0_interval"][1],
        "precondition_holds": res["precondition"]["holds"],
        "delta": res["delta"],
        "delta_lo": lo,
        "delta_hi": hi,
        "delta_unidentified_resamples": res["delta_unidentified_resamples"],
        "delta_edge_resamples": res["delta_edge_resamples"],
        "delta_lo_p0_fixed": lo_f,
        "delta_hi_p0_fixed": hi_f,
        "delta_free_amplitude": res["fit_free_amplitude"]["delta"],
        "amplitude_free_amplitude": res["fit_free_amplitude"]["amplitude"],
        "outcome": res["outcome"],
    }


def run(calls_paths, design_paths, prefix, label, b):
    calls = load_calls(calls_paths)
    bad = reparse(calls)
    if bad:
        raise SystemExit(f"stored parses disagree with the parser for {len(bad)} calls: {bad[:3]}")
    designs = [json.load(open(p)) for p in (design_paths or [])]
    design = {
        "exclusions": designs[0]["exclusions"] if designs else {},
        "calls": [c for d in designs for c in d["calls"]],
    }
    out = {}
    levels, fits = [], []
    for tier in ("Hard", "Plain"):
        keys, mat, n_seen = complete_matrix(calls, tier)
        if len(mat) == 0:
            continue
        levels.extend(levels_rows(calls, tier, mat, b, S.SEED))
        res = S.analyse(mat, b, S.SEED)
        res["incidents_seen"] = n_seen
        if tier == "Plain":
            # The plain set is secondary: its delta is a labelled proxy and claims no regime.
            res["regions_by_the_criterion_formula"] = res["outcome"]
            res["outcome"] = "plain proxy: no regime claimed"
        out[tier.lower()] = res
        fits.append(fit_row(tier.lower() + ("" if tier == "Hard" else " (labelled proxy)"), res))
    write_csv(prefix + "levels.csv", levels)
    write_csv(prefix + "fit.csv", fits)
    write_csv(prefix + "position.csv", position_rows(calls))
    write_csv(prefix + "family.csv", family_rows(calls))
    write_csv(prefix + "tokens.csv", token_rows(calls))
    write_csv(prefix + "timing.csv", timing_rows(calls))
    counts = []
    for tier in ("Hard", "Plain"):
        for col in COLUMNS:
            cs = [c for c in calls if c["tier"] == tier and c["level"] == col]
            counts.append(
                {
                    "set": tier.lower(),
                    "level": col,
                    "calls": len(cs),
                    "parse_failures": sum(1 for c in cs if c["parse_status"] != "ok"),
                    "transport_errors": sum(1 for c in cs if c["error"] is not None),
                    "finish_length": sum(1 for c in cs if c["finish_reason"] == "length"),
                }
            )
    counts.append(
        {
            "set": "design",
            "level": "exclusions " + json.dumps(design.get("exclusions", {})),
            "calls": len(design.get("calls", [])),
            "parse_failures": "",
            "transport_errors": "",
            "finish_length": "",
        }
    )
    write_csv(prefix + "counts.csv", counts)
    meta = {
        "label": label,
        "calls_files": {
            p: hashlib.sha256(open(p, "rb").read()).hexdigest() for p in calls_paths
        },
        "calls": len(calls),
        "results": out,
    }
    with open(prefix + "outcome.json", "w") as f:
        json.dump(meta, f, indent=1)
        f.write("\n")
    return meta


# ------------------------------------------------------------------ self test


def selftest():
    rng = np.random.default_rng(1)
    # 1. The parser: the cases that matter.
    p = C.parse_answer
    assert p("ANSWER: Compound s3", 10) == ("ok", "Compound", 3)
    assert p("ANSWER: NONE", 10) == ("ok", "NONE", None)
    assert p("Answer: Cascade s1", 10)[0] == "fail"  # the marker is case-sensitive
    assert p("ANSWER: cascade s1", 10) == ("ok", "Cascade", 1)
    assert p("ANSWER: Cascade", 10)[0] == "fail"
    assert p("ANSWER: Cascade s12", 10)[0] == "fail"
    assert p("ANSWER: Dragon s1", 10)[0] == "fail"
    assert p("I think it is Compound at s3", 10)[0] == "fail"
    assert p("reasoning\nANSWER: ConfigDrift s0\nANSWER: Compound s1", 10) == ("ok", "ConfigDrift", 0)
    # 2. Correctness is kind and site.
    t = {"kind": {"Hard": "Compound"}, "site": 3}
    assert C.is_correct(("ok", "Compound", 3), t)
    assert not C.is_correct(("ok", "Compound", 4), t)
    assert not C.is_correct(("ok", "Cascade", 3), t)
    assert not C.is_correct(("fail", "x", None), t)
    assert not C.is_correct(("ok", "NONE", None), t)
    # 3. The estimator recovers a known delta, and says so with an interval that covers it.
    n = 4000
    for delta in (0.0, 0.05, 0.2):
        a0, p0 = 0.8, 0.1
        probs = [p0 + (a0 - p0) * math.exp(-delta * m / 100) for m in S.LEVELS] + [p0]
        mat = (rng.random((n, 6)) < np.array(probs)).astype(float)
        res = S.analyse(mat, b=1000)
        lo, hi = res["delta_interval"]
        assert lo <= delta <= hi, (delta, lo, hi)
        assert abs(res["delta"] - delta) < 0.03, (delta, res["delta"])
    # 4. The outcome mapping, each branch reached.
    assert S.outcome(False, 0.0, 0.01).startswith("unidentifiable")
    assert S.outcome(True, -0.02, 0.04) == "R6 regime"
    assert S.outcome(True, 0.11, 0.2) == "R7 regime"
    assert S.outcome(True, 0.03, 0.08) == "unresolved"
    assert S.outcome(True, 0.02, 0.12) == "unresolved"
    assert S.outcome(True, 0.05, 0.05) == "unresolved"
    # 5. The precondition needs both parts.
    assert S.precondition(0.34, 0.30, 0.0)["holds"] is False
    assert S.precondition(0.60, 0.20, 0.10)["holds"] is False  # lower bound not above 0.25
    assert S.precondition(0.60, 0.30, 0.10)["holds"] is True
    # 6. A flat curve has no identified delta.
    d, st = S.fit_delta(np.array([0.1, 0.1, 0.1, 0.1, 0.1]), 0.1)
    assert np.isnan(d) and st == 1
    # 7. Context order: time, then position; nesting of the levels.
    q = {
        "seed": 1,
        "incident": 2,
        "decisive": [{"id": 5, "at_ns": 50, "obs": {}}],
        "pool": [{"id": i, "at_ns": 100 - i, "obs": {}} for i in range(600)],
    }
    ctx = C.contexts(q)
    for lo_m, hi_m in zip(C.LEVELS, C.LEVELS[1:]):
        a = {r["id"] for r in ctx[lo_m]}
        bset = {r["id"] for r in ctx[hi_m]}
        assert a <= bset
    assert [r["at_ns"] for r in ctx[400]] == sorted(r["at_ns"] for r in ctx[400])
    assert len(ctx[400]) == 401 and len(ctx["control"]) == 50
    assert all(r["id"] != 5 for r in ctx["control"])
    print("r8_analysis selftest: ok")


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "selftest":
        selftest()
        return
    ap = argparse.ArgumentParser()
    ap.add_argument("calls", nargs="+")
    ap.add_argument("--design", action="append")
    ap.add_argument("--out-prefix", required=True)
    ap.add_argument("--label", default="")
    ap.add_argument("--bootstrap", type=int, default=S.B)
    args = ap.parse_args()
    meta = run(args.calls, args.design, args.out_prefix, args.label, args.bootstrap)
    for tier, res in meta["results"].items():
        print(tier, json.dumps({k: res[k] for k in ("n_incidents", "A", "p0", "delta", "delta_interval", "precondition", "outcome")}))


if __name__ == "__main__":
    main()
