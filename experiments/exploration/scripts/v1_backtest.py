"""V1's back-test: the four criterion specifications against the kept run directories, compared with
(A) the lab scripts' CSVs at full precision and (B) the numbers the review log prints, to the printed
precision.

Usage: v1_backtest.py [--out DIR]   runs scripts/criterion.py on each specification (the committed
                                    ones, unchanged), then writes experiments/exploration/
                                    v1-backtest.csv (B) and v1-backtest-labcsv.csv (A).

The kept runs are read from the chief's checkout (read only). Every number the log prints for the
four units, and that the specifications' measures can express, is an entry of `PRINTED` below with
the place in the log it comes from; those the measures cannot express are entries with the status
`not_covered` and the reason. An entry's status is:

  match      the computed value, rounded half up to the printed number of decimals, equals the print
  off_by_1   it differs by one unit in the last printed digit
  mismatch   it differs by more

This file was written after the specifications were committed (f3c407a) and after the first
back-test run, so the log's numbers were not known when the specifications were written.
"""

import argparse
import csv
import json
import pathlib
import subprocess
import sys
from decimal import ROUND_HALF_UP, Decimal

import numpy as np
import pandas as pd

ROOT = pathlib.Path(__file__).resolve().parents[3]
RUNS = pathlib.Path("/home/user/gordian/artifacts/runs")
EXPL = ROOT / "experiments" / "exploration"
PY = "/home/user/gordian/analysis/.venv/bin/python"

UNITS = {
    "m2": {"heldout": RUNS / "m2/m2-heldout-b5-rho0.7"},
    "b3": {"b3": RUNS / "b3/b3-heldout-b5-rho0.7", "m2": RUNS / "m2/m2-heldout-b5-rho0.7"},
    "l1": {"fresh": RUNS / "l1/l1-fresh-b5-rho0.7"},
    "m3": {"heldout": RUNS / "m3/m3-heldout-b5-rho0.7"},
}


def run_unit(unit: str, out: pathlib.Path) -> dict:
    """scripts/criterion.py on one specification; exit status 0 (holds) and 1 (does not) are both
    results, 2 is an error."""
    cmd = [PY, str(ROOT / "scripts/criterion.py"), str(ROOT / f"experiments/criteria/{unit}.json"),
           "--out", str(out / unit)]
    for role, d in UNITS[unit].items():
        cmd += ["--run", f"{role}={d}"]
    p = subprocess.run(cmd, capture_output=True, text=True, env={"PYTHONPATH": str(ROOT / "analysis")})
    if p.returncode not in (0, 1):
        sys.exit(f"{unit}: criterion.py failed ({p.returncode}): {p.stderr}")
    return json.loads((out / unit / "verdict.json").read_text())


class V:
    """A verdict.json indexed for lookup."""

    def __init__(self, data):
        self.data = data
        self.clauses = {c["id"]: c for c in data["clauses"]}
        self.cells = {(r["id"], c["row"], c["measure"]): c for r in data["reports"] for c in r["cells"]}

    def cell(self, report, row, measure):
        return self.cells[(report, row, measure)]

    def val(self, report, row, measure, field="point"):
        return self.cells[(report, row, measure)][field]

    def holds(self):
        return self.data["verdict"]["holds"]

    def node(self, name):
        d = self.data["verdict"]
        return d["nodes"][name] if name in d["nodes"] else d["observations"][name]


# ---------------------------------------------------------------------------------------------
# Printed precision
# ---------------------------------------------------------------------------------------------


def parse_printed(text: str) -> tuple[Decimal, int]:
    t = text.replace("−", "-").replace("+", "").strip()
    d = Decimal(t)
    return d, max(0, -d.as_tuple().exponent)


def round_half_up(x: float, decimals: int) -> Decimal:
    return Decimal(repr(float(x))).quantize(Decimal(1).scaleb(-decimals), rounding=ROUND_HALF_UP)


def compare(printed: str, x) -> tuple[str, str, int | None]:
    """(status, computed rounded to the printed precision as text, units off)."""
    p, dec = parse_printed(printed)
    r = round_half_up(x, dec)
    units = int((r - p) / Decimal(1).scaleb(-dec))
    status = "match" if units == 0 else ("off_by_1" if abs(units) == 1 else "mismatch")
    return status, format(r, "f"), units


ROWS: list[dict] = []


def put(unit, ref, quantity, printed, value, note=""):
    """One printed number against one computed number."""
    status, rounded, units = compare(printed, value)
    ROWS.append({"unit": unit, "log_entry": ref, "quantity": quantity, "printed": printed,
                 "computed": repr(float(value)), "computed_at_printed_precision": rounded,
                 "units_off": units, "status": status, "note": note})


def put_bool(unit, ref, quantity, printed, value, note=""):
    ROWS.append({"unit": unit, "log_entry": ref, "quantity": quantity, "printed": printed,
                 "computed": str(bool(value)), "computed_at_printed_precision": str(bool(value)),
                 "units_off": 0, "status": "match" if bool(value) == (printed == "True") else "mismatch",
                 "note": note})


def put_equal(unit, ref, quantity, a, b, note=""):
    """The log says two values agree 'to the digit': they must be equal."""
    ROWS.append({"unit": unit, "log_entry": ref, "quantity": quantity, "printed": "equal",
                 "computed": f"{a!r} vs {b!r}", "computed_at_printed_precision": "",
                 "units_off": "", "status": "match" if abs(a - b) < 1e-12 else "mismatch", "note": note})


def not_covered(unit, ref, quantity, printed, why):
    ROWS.append({"unit": unit, "log_entry": ref, "quantity": quantity, "printed": printed, "computed": "",
                 "computed_at_printed_precision": "", "units_off": "", "status": "not_covered", "note": why})


def interval_rows(unit, ref, quantity, printed_lo, printed_hi, est, negate=False):
    lo, hi = est["lower"], est["upper"]
    if negate:
        lo, hi = -hi, -lo
    put(unit, ref, quantity + " lower", printed_lo, lo)
    put(unit, ref, quantity + " upper", printed_hi, hi)


# ---------------------------------------------------------------------------------------------
# (B) The numbers the review log prints
# ---------------------------------------------------------------------------------------------


def printed_m2(m2: V):
    U, R = "m2", "M2 entry, re-verified table"
    t = lambda arm, m, f="point": m2.val("table", arm, m, f)  # noqa: E731
    for tick, (anc, d, dlo, dhi, ldiff, llo, lhi, bg, lk) in {
        100: ("0.992", "+0.040", "+0.021", "+0.060", "+0.532", "+0.462", "+0.603", "5.44", "0.993"),
        500: (None, "+0.030", "+0.010", "+0.050", None, None, None, "5.33", None),
        2000: (None, "-0.043", "-0.075", "-0.011", None, None, None, "4.29", None),
    }.items():
        c1, c2 = m2.clauses[f"r1.t{tick}"], m2.clauses[f"r2.t{tick}"]
        if anc:
            put(U, R, f"{tick} ms medium anchor-correct", anc, t(f"medium_t{tick}", "hard_anchor_correct_share"))
            put(U, R, f"{tick} ms medium leak noticed", lk, t(f"medium_t{tick}", "leak_noticed_share"))
            put(U, R, f"{tick} ms comparator anchor-correct", "0.952", t("reanchor", "hard_anchor_correct_share"))
            put(U, R, f"{tick} ms comparator leak noticed", "0.460", t("reanchor", "leak_noticed_share"))
        put(U, R, f"{tick} ms result 1 difference", d, c1["point"])
        interval_rows(U, R, f"{tick} ms result 1", dlo, dhi, c1)
        if ldiff:
            put(U, R, f"{tick} ms result 2 difference", ldiff, c2["point"])
            interval_rows(U, R, f"{tick} ms result 2", llo, lhi, c2)
        put(U, R, f"{tick} ms medium background per stream", bg, m2.clauses[f"bg.t{tick}"]["point"])
    put(U, R, "500 ms result 2 difference", "+0.525", m2.clauses["r2.t500"]["point"])
    put(U, R, "2 s result 2 difference", "+0.540", m2.clauses["r2.t2000"]["point"])
    put(U, R, "500 ms shortfall under the result-1 margin (0.03 - difference)", "0.0004",
        0.03 - m2.clauses["r1.t500"]["point"])
    put(U, R, "comparator background per stream (bound)", "6.82", t("reanchor", "notices_on_background_per_stream"))
    # the clause outcomes the table states in words
    put_bool(U, R, "100 ms result 1 holds", "True", m2.node("result1_t100"))
    put_bool(U, R, "100 ms result 2 holds", "True", m2.node("result2_t100"))
    put_bool(U, R, "500 ms result 1 holds ('not shown')", "False", m2.node("result1_t500"))
    put_bool(U, R, "500 ms result 2 holds", "True", m2.node("result2_t500"))
    put_bool(U, R, "2 s result 1 holds ('worse')", "False", m2.node("result1_t2000"))
    put_bool(U, R, "2 s result 2 holds", "True", m2.node("result2_t2000"))
    put_bool(U, R, "M2 holds at the best tick length, 100 ms", "True", m2.holds())
    put_bool(U, R, "both results hold at 500 ms", "False", m2.node("holds_t500"))
    put_bool(U, R, "both results hold at 2 s", "False", m2.node("holds_t2000"))
    # arithmetic of the counts: 372 hard non-leak incidents, the comparator misses 18, the medium 3
    n = 372
    put(U, "M2 entry, 'all 18 incidents the re-anchor misses'", "comparator misses (count)", "18",
        (1 - t("reanchor", "hard_anchor_correct_share")) * n, "372 from the queue's M2 text")
    put(U, "M2 entry, 'misses 3 the re-anchor gets right'", "medium at 100 ms misses (count)", "3",
        (1 - t("medium_t100", "hard_anchor_correct_share")) * n,
        "consistent with the print only: the medium misses 3 in all, and 18 - 3 = 15 = the difference x 372")
    put(U, "M2 entry, 'all 18 ... misses 3'", "difference x 372 = 18 - 3", "15", m2.clauses["r1.t100"]["point"] * n)
    # prose numbers
    R = "M2 entry, 'What it means'"
    put(U, R, "leak anchor-correct, medium at 100 ms", "0.554", t("medium_t100", "leak_anchor_correct_share"))
    put(U, R, "leak noticed, medium at 100 ms", "0.993", t("medium_t100", "leak_noticed_share"))
    put(U, R, "anomalies per incident, medium at 100 ms", "1.65", t("medium_t100", "notices_per_incident"))
    put(U, R, "anomalies per incident, re-anchor", "1.04", t("reanchor", "notices_per_incident"))
    put(U, R, "strict precision, medium at 100 ms", "0.50", t("medium_t100", "strict_precision"))
    put(U, R, "strict precision, re-anchor", "0.67", t("reanchor", "strict_precision"))
    costs = [t(f"medium_t{k}", "cost_s_per_stream") for k in (100, 500, 2000)]
    put(U, R, "medium total cost per stream, lowest of three ticks", "1.8", min(costs))
    put(U, R, "medium total cost per stream, highest of three ticks", "3.8", max(costs))
    put(U, R, "re-anchor total cost per stream", "0.67", t("reanchor", "cost_s_per_stream"))
    for tick, pr in ((100, "14.7"), (500, "30.3"), (2000, "22.7")):
        put(U, R, f"sliding form, background per stream at {tick} ms", pr,
            t(f"medium_t{tick}_sliding", "notices_on_background_per_stream"))
    put(U, R, "binned form at 100 ms, anchor-correct", "0.976", t("medium_t100_binned", "hard_anchor_correct_share"))
    put(U, R, "binned form at 100 ms, background per stream", "8.59",
        t("medium_t100_binned", "notices_on_background_per_stream"))
    put(U, R, "emitter lookback 0 (the frozen medium), anchor-correct", "0.992", t("medium_t100", "hard_anchor_correct_share"))
    put(U, R, "emitter lookback 1 s, anchor-correct", "0.930", t("medium_t100_lb1000", "hard_anchor_correct_share"))
    put(U, R, "abnormal-only medium at 100 ms, leak noticed", "1.000", t("medium_t100_abnormal_only", "leak_noticed_share"))
    not_covered(U, R, "median notice latency, medium vs comparator", "9.8 s against 14.2 s",
                "the schema has no quantile measure (latency is a per-incident column, not a ratio of sums)")
    not_covered(U, R, "the medium's own operations per stream", "3.9-16.7 ms",
                "a column of the bill, not of the notice files; no measure is specified for it")
    not_covered(U, "M2 entry, 'all 18 incidents the re-anchor misses'", "which incidents the medium gets right",
                "all 18 / misses 3", "a join of two arms on (seed, incident); the schema's terms are per arm and per stream")
    not_covered(U, "M2 entry, 'lose 31 incidents' at 2 s", "incidents lost at 2 s", "31",
                "a count of discordant incidents between two arms; no join measure")


def printed_b3(b3: V):
    U = "b3"
    R = "B3 entry, re-verified table"
    rows = {
        "reanchor": ("0.952", "0.460", "0.000", "6.82", "0.672"),
        "ramp_split_over_re2": ("0.973", "0.986", "0.971", "6.24", "0.693"),
    }
    cols = ("hard_anchor_correct_share", "leak_noticed_share", "leak_anchor_correct_share",
            "notices_on_background_per_stream", "strict_precision")
    for arm, prints in rows.items():
        for col, pr in zip(cols, prints):
            put(U, R, f"{arm}: {col}", pr, b3.val("table", arm, col))
    med = ("0.992", "0.993", "0.554", "5.44", "0.501")
    for col, pr in zip(cols, med):
        put(U, R, f"medium_t100: {col}", pr, b3.val("supplementary_table", "medium_t100", col))
    # the same three rows from the supplementary table (one set of streams)
    for col, pr in zip(cols, rows["reanchor"]):
        put(U, R, f"reanchor (supplementary table): {col}", pr, b3.val("supplementary_table", "reanchor", col))
    for col, pr in zip(cols, rows["ramp_split_over_re2"]):
        put(U, R, f"ramp_split_over_re2 (supplementary table): {col}", pr,
            b3.val("supplementary_table", "ramp_split_over_re2", col))
    R = "B3 entry, 'Medium minus ramp + split, paired'"
    row = "medium_t100 - ramp_split_over_re2"
    c = b3.cell("supplementary_paired", row, "hard_anchor_correct_share")
    put(U, R, "anchor-correct difference", "+0.019", c["point"])
    interval_rows(U, R, "anchor-correct", "+0.005", "+0.034", c)
    c = b3.cell("supplementary_paired", row, "leak_noticed_share")
    put(U, R, "leak noticed difference", "+0.007", c["point"])
    interval_rows(U, R, "leak noticed", "-0.014", "+0.029", c)
    c = b3.cell("supplementary_paired", row, "leak_anchor_correct_share")
    put(U, R, "leak anchor-correct difference", "-0.417", c["point"])
    interval_rows(U, R, "leak anchor-correct", "-0.492", "-0.343", c)
    put(U, R, "background difference", "-0.80",
        b3.val("supplementary_paired", row, "notices_on_background_per_stream"))
    put(U, R, "strict precision difference", "-0.19", b3.val("supplementary_paired", row, "strict_precision"))
    R = "B3 entry, 'Verdict'"
    put_bool(U, R, "acceptance clause triggers: a public row reaches leak noticed >= 0.660 within the budget",
             "True", b3.node("a_public_row_reaches_the_leak_bar_within_the_budget"))
    put(U, R, "leak noticed of the triggering row (ramp + split over re-anchor)", "0.986",
        b3.val("table", "ramp_split_over_re2", "leak_noticed_share"))
    put_bool(U, "B3 entry, 'comparator arms equal M2's modulo the run id'",
             "three arms, three files each, identical", "True", b3.holds())
    R = "B3 entry, 'What it means'"
    put(U, R, "leaks noticed by the ramp (137 of 139), as a share", "0.986", 137 / 139,
        "the share the log's counts imply; compared with the computed share below")
    put(U, R, "leak noticed share computed from the files equals 137/139", "0.986",
        b3.val("table", "ramp_split_over_re2", "leak_noticed_share"))
    put(U, R, "leaks anchored at offset zero (135 of 139), as a share", "0.971",
        b3.val("table", "ramp_split_over_re2", "leak_anchor_correct_share"))
    put(U, R, "ramp over re-anchor, noticed share of decoys ('It notices 0.948 of decoys')", "0.948",
        b3.val("table", "ramp_over_re2", "decoy_noticed_share"),
        "'It' is the ramp noticer; the ramp + split row is 0.957")
    put(U, R, "re-anchor noticed share of decoys", "0.756", b3.val("table", "reanchor", "decoy_noticed_share"))
    put(U, R, "medium cost per stream", "1.85", b3.val("supplementary_table", "medium_t100", "cost_s_per_stream"))
    put(U, R, "ramp + split cost per stream", "0.80", b3.val("supplementary_table", "ramp_split_over_re2", "cost_s_per_stream"))
    not_covered(U, R, "latency 5.5 s median; 135 of 137 at offset zero; the 510 added notices by class; 8 never-noticed incidents",
                "137 of 139; 135; 5.5 s; 510 = 137+154+192+11+16; 2 of 8",
                "latency quantile, added-notice counts between two arms and the never-noticed incident set are not "
                "expressible: no quantile or arm-to-arm join measure")


def printed_l1(l1: V):
    U, R = "l1", "L1 entry, re-verified"
    c1, c2, c3 = (l1.clauses[k] for k in ("c1.end_state", "c2.slope", "c3.off_below_frozen"))
    put(U, R, "clause 1: learned minus frozen, last 100", "0.000", c1["point"])
    interval_rows(U, R, "clause 1", "-0.011", "+0.010", c1)
    put_bool(U, R, "clause 1 holds", "True", c1["pass"])
    put(U, R, "clause 3: learning-off minus frozen (the specification's frozen minus off, negated)", "-0.039",
        -c3["point"])
    interval_rows(U, R, "clause 3 (negated)", "-0.064", "-0.015", c3, negate=True)
    put_bool(U, R, "clause 3 holds", "True", c3["pass"])
    put(U, R, "clause 2: slope over the first 100 streams (the chief's own)", "+0.053", c2["point"],
        "the log gives a second figure, the lab's, which is compared next")
    interval_rows(U, R, "clause 2 (the chief's own)", "-0.037", "+0.085", c2)
    put(U, R, "clause 2: slope (the lab's, in parentheses in the log)", "+0.043", c2["point"])
    interval_rows(U, R, "clause 2 (the lab's)", "-0.021", "+0.126", c2)
    put_bool(U, R, "clause 2 holds (the lower bound is below zero: 'fails')", "False", c2["pass"])
    put_bool(U, R, "the learned arm counts toward the aim (the conjunction)", "False", l1.holds())
    t = lambda arm, m: l1.val("table_last100", arm, m)  # noqa: E731
    put(U, R, "end state: learned anchor-correct", "0.980", t("learned", "hard_anchor_correct_share"))
    put(U, R, "end state: learned background per stream", "8.76", t("learned", "notices_on_background_per_stream"))
    put(U, R, "end state: frozen anchor-correct", "0.980", t("frozen", "hard_anchor_correct_share"))
    put(U, R, "end state: frozen background per stream", "5.53", t("frozen", "notices_on_background_per_stream"))
    put(U, R, "end state: learning-off anchor-correct", "0.941", t("learned_off", "hard_anchor_correct_share"))
    put(U, R, "end state: learning-off background per stream", "31.55", t("learned_off", "notices_on_background_per_stream"))
    R = "L1 entry, 'What it means'"
    p = lambda row, m, f="point": l1.val("paired_last100", row, m, f)  # noqa: E731
    put(U, R, "learning vs learning-off: fewer background notices per stream", "-22.8",
        p("learned - learned_off", "notices_on_background_per_stream"))
    put(U, R, "learning vs learning-off: anchor-correct", "+0.039", p("learned - learned_off", "hard_anchor_correct_share"))
    put(U, R, "learner vs frozen: more background notices per stream", "+3.2",
        p("learned - frozen", "notices_on_background_per_stream"))
    put(U, R, "learner vs frozen: strict precision", "-0.07", p("learned - frozen", "strict_precision"))
    put_equal(U, R, "p = 0.8 arm: strict precision equals the frozen graph's", t("learned_p080", "strict_precision"),
              t("frozen", "strict_precision"))
    put_equal(U, R, "p = 0.8 arm: background per stream equals the frozen graph's",
              t("learned_p080", "notices_on_background_per_stream"), t("frozen", "notices_on_background_per_stream"))
    put(U, R, "p = 0.8 arm: anchor-correct equals the frozen graph's", "0.980", t("learned_p080", "hard_anchor_correct_share"))
    put(U, R, "50 ms prior: end-state anchor-correct equals the learned arm's", "0.980", t("learned_w050", "hard_anchor_correct_share"))
    put(U, R, "1,000 ms prior: end-state anchor-correct equals the learned arm's", "0.980", t("learned_w1000", "hard_anchor_correct_share"))
    not_covered(U, R, "learner state: window 400 ms to 100 ms to 49 ms to 30 ms; ramp threshold 2.5", "400/100/49/30 ms; 2.5",
                "state of the learner (the plasticity adapter's persisted constants), not an evaluator file column")
    not_covered(U, R, "identical decisions on all 92 incidents in the first 50 streams (four medium arms)", "92",
                "a join of arms by incident over a window; the schema's windows are over streams and its terms per arm")
    not_covered(U, R, "'three differing incidents in 398'", "3 of 398", "a join of two arms by incident")


def printed_m3(m3: V):
    U, R = "m3", "M3 entry, re-verified table"
    t = lambda arm, m, f="point": m3.val("table", arm, m, f)  # noqa: E731
    for tick, (anc, d, lo, hi, leak, bg, ctrl, sp) in {
        100: ("0.981", "+0.030", "+0.011", "+0.049", "0.770", "2.04", "0.976", "0.77"),
        500: ("0.970", "+0.019", "-0.003", "+0.040", "0.892", "0.95", "0.911", "0.81"),
        2000: ("0.949", "-0.003", "-0.028", "+0.022", "0.712", "1.01", "0.780", "0.84"),
    }.items():
        c = m3.clauses[f"r1.t{tick}"]
        put(U, R, f"{tick} ms anchor-correct", anc, t(f"medium_t{tick}", "hard_anchor_correct_share"))
        put(U, R, f"{tick} ms difference against the re-anchor", d, c["point"])
        interval_rows(U, R, f"{tick} ms difference", lo, hi, c)
        put(U, R, f"{tick} ms leak noticed", leak, t(f"medium_t{tick}", "leak_noticed_share"))
        put(U, R, f"{tick} ms background per stream", bg, m3.clauses[f"bg.t{tick}"]["point"])
        put(U, R, f"{tick} ms sub-tick pruning off: anchor-correct", ctrl,
            t(f"medium_t{tick}_subtick_off", "hard_anchor_correct_share"))
        put(U, R, f"{tick} ms strict precision", sp, m3.clauses[f"sp.t{tick}"]["point"])
    put(U, R, "reference: the re-anchor's anchor-correct", "0.952", t("reanchor", "hard_anchor_correct_share"))
    put(U, R, "100 ms: shortfall under the result-1 margin (0.03 - difference)", "0.0004", 0.03 - m3.clauses["r1.t100"]["point"])
    for tick, pr in ((100, "+0.005"), (500, "+0.059"), (2000, "+0.169")):
        put(U, R, f"{tick} ms pruning is worth (frozen minus control, anchor-correct)", pr,
            m3.val("control", f"medium_t{tick} - medium_t{tick}_subtick_off", "hard_anchor_correct_share"))
    for tick in (100, 500, 2000):
        put_bool(U, R, f"result 2 holds at {tick} ms ('Result 2 holds at every tick')", "True",
                 m3.node(f"result2_t{tick}_continuity"))
    put_bool(U, R, "M3 holds ('Verdict as written: M3 does not hold')", "False", m3.holds())
    put_bool(U, R, "result 1 at 500 ms holds ('fails')", "False", m3.node("result1_t500"))
    put_bool(U, R, "result 1 at 2 s holds ('fails')", "False", m3.node("result1_t2000"))
    R = "M3 entry, 'What it means'"
    put(U, R, "2 s: anchor-correct with pruning off", "0.780", t("medium_t2000_subtick_off", "hard_anchor_correct_share"))
    put(U, R, "2 s: anchor-correct with pruning", "0.949", t("medium_t2000", "hard_anchor_correct_share"))
    put(U, R, "500 ms: anchor-correct with pruning off", "0.911", t("medium_t500_subtick_off", "hard_anchor_correct_share"))
    put(U, R, "500 ms: anchor-correct with pruning", "0.970", t("medium_t500", "hard_anchor_correct_share"))
    put(U, R, "best public row (ramp + split over the re-anchor) anchor-correct", "0.973",
        t("ramp_split_over_re2", "hard_anchor_correct_share"))
    q = lambda m, f="point": m3.val("paired_vs_ramp_split", "medium_t500 - ramp_split_over_re2", m, f)  # noqa: E731
    c = m3.cell("paired_vs_ramp_split", "medium_t500 - ramp_split_over_re2", "hard_anchor_correct_share")
    put(U, R, "500 ms vs ramp + split: anchor-correct difference", "-0.003", c["point"])
    interval_rows(U, R, "500 ms vs ramp + split anchor-correct", "-0.022", "+0.016", c)
    put(U, R, "500 ms vs ramp + split: false notices (background) per stream", "-5.3", q("notices_on_background_per_stream"))
    put(U, R, "500 ms vs ramp + split: strict precision", "+0.11", q("strict_precision"))
    put(U, R, "500 ms medium reasoner cost per stream", "0.81", t("medium_t500", "cost_s_per_stream"))
    put(U, R, "ramp + split reasoner cost per stream", "0.80", t("ramp_split_over_re2", "cost_s_per_stream"))
    put(U, R, "500 ms vs ramp + split: leak noticed", "-0.094", q("leak_noticed_share"))
    put(U, R, "500 ms vs ramp + split: leak anchor-correct", "-0.49", q("leak_anchor_correct_share"))
    put(U, "M3 entry, M4's criterion (fixed from B3's row)", "ramp + split strict precision", "0.693",
        t("ramp_split_over_re2", "strict_precision"))
    put(U, "M3 entry, M4's criterion (fixed from B3's row)", "ramp + split background per stream", "6.24",
        t("ramp_split_over_re2", "notices_on_background_per_stream"))
    put(U, "M3 entry, M4's criterion (fixed from B3's row)", "ramp + split leak noticed", "0.986",
        t("ramp_split_over_re2", "leak_noticed_share"))


# ---------------------------------------------------------------------------------------------
# (A) The lab scripts' CSVs at full precision
# ---------------------------------------------------------------------------------------------

LAB: list[dict] = []
TOL = 1e-9


def lab(unit, csv_name, pairs):
    """`pairs` yields (key, lab (point, lower, upper), mine (point, lower, upper) or None)."""
    n_cells = n_missing = n_bad = 0
    worst = 0.0
    bad = []
    for key, lv, mine in pairs:
        if mine is None:
            n_missing += 1
            continue
        for name, a, b in zip(("point", "lower", "upper"), lv, mine):
            if a is None or (isinstance(a, float) and np.isnan(a)):
                a = None
            if a is None and b is None:
                continue
            n_cells += 1
            d = float("inf") if (a is None or b is None) else abs(a - b)
            worst = max(worst, d if np.isfinite(d) else 1e9)
            if d > TOL:
                n_bad += 1
                bad.append((key, name, a, b))
    LAB.append({"unit": unit, "lab_csv": csv_name, "cells_compared": n_cells, "cells_not_in_specification": n_missing,
                "cells_differing": n_bad, "max_abs_difference": f"{worst:.3e}"})
    for key, name, a, b in bad[:20]:
        LAB.append({"unit": unit, "lab_csv": csv_name, "cells_compared": "", "cells_not_in_specification": "",
                    "cells_differing": f"DIFF {key} {name}: lab {a} mine {b}", "max_abs_difference": ""})


def est(c):
    return (c["point"], c["lower"], c["upper"]) if c else None


def lab_tables(spec_arms: dict, v: V, report: str, df: pd.DataFrame, arm_col: str, measures: list[str],
               rename=lambda x: x, window_col=None, window_map=None):
    by_dir = {}
    for n, a in spec_arms.items():
        by_dir.setdefault(a["dir"], n)  # the first arm of a directory name (b3 has two runs with the same names)
    for _, r in df.iterrows():
        arm = rename(r[arm_col])
        name = by_dir.get(arm, arm)
        rep = report if window_col is None else window_map.get(r[window_col])
        if rep is None:
            continue
        for m in measures:
            if m not in df.columns:
                continue
            mine = v.cells.get((rep, name, m))
            yield (name, m), (r[m], r.get(m + "_lo"), r.get(m + "_hi")), est(mine)


def part_a(V_):
    spec = {u: json.loads((ROOT / f"experiments/criteria/{u}.json").read_text()) for u in UNITS}
    m2, b3, l1, m3 = (V_[u] for u in ("m2", "b3", "l1", "m3"))
    # M2
    meas = list(spec["m2"]["measures"])
    df = pd.read_csv(EXPL / "m2-heldout-table.csv")
    lab("m2", "m2-heldout-table.csv", lab_tables(spec["m2"]["arms"], m2, "table", df, "arm", meas))
    df = pd.read_csv(EXPL / "m2-criterion.csv")
    pairs = []
    for _, r in df.iterrows():
        res = "r1" if r.result == "result1" else "r2"
        if r.versus.startswith("reanchor"):
            mine = est(m2.clauses.get(f"{res}.t{r.tick_ms}"))
        else:
            mine = est(m2.cells.get(("paired_vs_rung_z2", f"medium_t{r.tick_ms} - rung_z2", r.measure)))
        pairs.append(((r.tick_ms, r.result, r.versus), (r["difference"], r["lower"], r["higher"]), mine))
    lab("m2", "m2-criterion.csv", pairs)
    # B3
    meas = list(spec["b3"]["measures"])
    df = pd.read_csv(EXPL / "b3-noticers-table.csv")
    pairs = list(lab_tables(spec["b3"]["arms"], b3, "table", df, "arm", meas))
    for _, r in df.iterrows():
        for m in ("quality_held", "leak_quality_held"):
            base = m.replace("_held", "")
            mine = b3.cells.get(("table_held", f"hold_{r.config}", base))
            pairs.append(((r.config, m), (r[m], r[m + "_lo"], r[m + "_hi"]), est(mine)))
    lab("b3", "b3-noticers-table.csv", pairs)
    df = pd.read_csv(EXPL / "b3-paired.csv")
    pairs = []
    for _, r in df.iterrows():
        if r.measure.endswith("_held"):
            mine = b3.cells.get(("paired_held_vs_reanchor", f"hold_{r.config} - hold_reanchor", r.measure.replace("_held", "")))
        else:
            mine = b3.cells.get(("paired_vs_reanchor", f"{r.config} - reanchor", r.measure))
        pairs.append(((r.config, r.measure), (r["difference"], r["lower"], r["higher"]), est(mine)))
    lab("b3", "b3-paired.csv", pairs)
    df = pd.read_csv(EXPL / "b3-vs-m2.csv")
    pairs = []
    for _, r in df.iterrows():
        nm = r["arm"]
        for m in spec["b3"]["measures"]:
            if m in df.columns:
                mine = b3.cells.get(("supplementary_table", nm, m))
                pairs.append(((nm, m), (r[m], r[m + "_lo"], r[m + "_hi"]), est(mine)))
    lab("b3", "b3-vs-m2.csv", pairs)
    df = pd.read_csv(EXPL / "b3-vs-m2-paired.csv")
    pairs = [((r.medium_minus, r.measure), (r["difference"], r["lower"], r["higher"]),
              est(b3.cells.get(("supplementary_paired", f"medium_t100 - {r.medium_minus}", r.measure))))
             for _, r in df.iterrows()]
    lab("b3", "b3-vs-m2-paired.csv", pairs)
    # L1
    df = pd.read_csv(EXPL / "l1-criterion.csv")
    ids = {1: "c1.end_state", 2: "c2.slope", 3: "c3.off_below_frozen"}
    pairs = []
    for _, r in df.iterrows():
        if str(r.clause) in ("1", "2", "3"):
            c = l1.clauses[ids[int(r.clause)]]
            mine = est(c)
            if r.clause == 3:  # the lab's clause 3 is frozen minus off, as the specification's
                pass
            pairs.append((("clause", r.clause), (r["difference"], r["lower"], r["higher"]), mine))
    lab("l1", "l1-criterion.csv", pairs)
    df = pd.read_csv(EXPL / "l1-table.csv")
    meas = list(spec["l1"]["measures"])
    pairs = []
    for _, r in df.iterrows():
        rep = {"all": "table_all", "last100": "table_last100"}.get(r.window)
        if rep is None:
            continue
        for m in meas:
            if m in df.columns:
                pairs.append(((r.arm, r.window, m), (r[m], r[m + "_lo"], r[m + "_hi"]), est(l1.cells.get((rep, r.arm, m)))))
    lab("l1", "l1-table.csv", pairs)
    df = pd.read_csv(EXPL / "l1-paired.csv")
    pairs = []
    for _, r in df.iterrows():
        if r.window == "last100":
            mine = l1.cells.get(("paired_last100", f"learned - {r.learned_minus}", r.measure))
            pairs.append(((r.learned_minus, r.measure, r.window), (r["difference"], r["lower"], r["higher"]), est(mine)))
    lab("l1", "l1-paired.csv", pairs)
    df = pd.read_csv(EXPL / "l1-slopes.csv")
    pairs = []
    for _, r in df[df.reading.str.startswith("R3 curve")].iterrows():
        rep, m = ("slopes", "hard_anchor_correct_share") if r.outcome == "anchor" else ("slopes_leak", "leak_noticed_share")
        mine = l1.cells.get((rep, r.arm, f"{m} slope, first {r.streams}"))
        pairs.append(((r.arm, r.streams, r.outcome), (r["slope"], r["lower"], r["higher"]), est(mine)))
    lab("l1", "l1-slopes.csv", pairs)
    # M3
    df = pd.read_csv(EXPL / "m3-heldout-table.csv")
    meas = list(spec["m3"]["measures"])
    lab("m3", "m3-heldout-table.csv", lab_tables(spec["m3"]["arms"], m3, "table", df, "arm", meas))
    df = pd.read_csv(EXPL / "m3-criterion.csv")
    pairs = []
    for _, r in df.iterrows():
        arm = {v["dir"]: n for n, v in spec["m3"]["arms"].items()}[r.arm]
        mine = m3.cells.get(("paired_vs_reanchor", f"{arm} - reanchor", r.measure))
        pairs.append(((arm, r.measure), (r["difference"], r["lower"], r["higher"]), est(mine)))
    lab("m3", "m3-criterion.csv", pairs)
    df = pd.read_csv(EXPL / "m3-control.csv")
    pairs = [((r.tick_ms, r.measure), (r["frozen_minus_control"], r["lower"], r["higher"]),
              est(m3.cells.get(("control", f"medium_t{r.tick_ms} - medium_t{r.tick_ms}_subtick_off", r.measure))))
             for _, r in df.iterrows()]
    lab("m3", "m3-control.csv", pairs)
    df = pd.read_csv(EXPL / "m3-b3-paired.csv")
    by_dir = {a["dir"]: n for n, a in spec["m3"]["arms"].items()}
    pairs = []
    for _, r in df.iterrows():
        a, b = by_dir.get(r.arm), by_dir.get(r.reference)
        mine = m3.cells.get(("paired_vs_ramp_split", f"{a} - {b}", r.measure)) if a and b else None
        pairs.append(((r.arm, r.reference, r.measure), (r["difference"], r["lower"], r["higher"]), est(mine)))
    lab("m3", "m3-b3-paired.csv", pairs)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", type=pathlib.Path, required=True, help="where the four verdicts are written")
    args = ap.parse_args()
    verdicts = {u: V(run_unit(u, args.out)) for u in UNITS}
    printed_m2(verdicts["m2"])
    printed_b3(verdicts["b3"])
    printed_l1(verdicts["l1"])
    printed_m3(verdicts["m3"])
    part_a(verdicts)
    fields = ["unit", "log_entry", "quantity", "printed", "computed", "computed_at_printed_precision",
              "units_off", "status", "note"]
    with open(EXPL / "v1-backtest.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=fields)
        w.writeheader()
        w.writerows(ROWS)
    fields = ["unit", "lab_csv", "cells_compared", "cells_not_in_specification", "cells_differing", "max_abs_difference"]
    with open(EXPL / "v1-backtest-labcsv.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=fields)
        w.writeheader()
        w.writerows(LAB)
    df = pd.DataFrame(ROWS)
    print(df.groupby(["unit", "status"]).size().unstack(fill_value=0))
    print(pd.DataFrame(LAB).to_string(index=False))
    bad = df[df.status.isin(["off_by_1", "mismatch"])]
    print(bad[["unit", "quantity", "printed", "computed", "units_off", "status"]].to_string(index=False))


if __name__ == "__main__":
    main()
