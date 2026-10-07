"""Writes `experiments/criteria/a1b.draft.json`, the draft of A1b's specification in the schema E1
extended, from W2's proposed bounds (`w2-learnable-laws.md`, section 10.2) and the review log's
decisions after W2 and A1a. A draft: the chief fixes the numbers and the final file
`experiments/criteria/a1b.json` before any A1b run; every number below that is a proposal says so in
its description.

The arms are roles: `engram` (the family-keyed engram carried across streams), `record` (E1's
record rung, family-keyed, carried, the same confirmation policy), `control` (the memoryless
re-anchor under the same selection oracle), and `record_site` / `engram_site` for the within-stream
rows. The directories are placeholders for A1b's own arm names.

`--dry-run RUNDIR` evaluates the draft on E1's held-out run with the record rung standing in for
the engram (a stand-in to show that every clause evaluates, never a result about an engram).
"""

import argparse
import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "analysis"))
from gordian_analysis import criterion as C  # noqa: E402

MI = "memory_incidents.csv"
MEM = "memory.csv"
HARD = {"column": "tier", "op": "eq", "value": "hard"}
REC = {"column": "recurrence_of", "op": "notnull"}


def term(file, agg, where=None, columns=None, join=None):
    t = {"file": file, "agg": agg}
    if where:
        t["where"] = where
    if columns:
        t["columns"] = columns
    if join:
        t["join"] = join
    return t


def ratio(num, den, desc, scale=1.0):
    m = {"kind": "ratio", "description": desc, "num": num, "den": den}
    if scale != 1.0:
        m["scale"] = scale
    return m


PER = {"per_stream": 1}

MEASURES = {
    "recurrence_unasked_correct": ratio(
        term(MI, "count", [HARD, REC, {"column": "unasked_correct", "op": "eq", "value": True}]),
        term(MI, "count", [HARD, REC]),
        "hard incidents that repeat an earlier one (K1), declared correctly with no escalation about them (K3), over those incidents"),
    "reachable_unasked_correct": ratio(
        term(MEM, "sum", columns={"hard_reachable_unasked_correct": 1}), term(MEM, "sum", columns={"hard_reachable": 1}),
        "the same over the hard incidents a family-keyed memory can reach: a recurrence, or an earlier same-family-and-mode incident at another site"),
    "hard_unasked_correct": ratio(
        term(MEM, "sum", columns={"unasked_correct_hard": 1}), term("results.csv", "sum", columns={"incidents_hard": 1}),
        "hard incidents declared correctly with no escalation, over all hard incidents"),
    "collision_share": ratio(
        term(MEM, "sum", columns={"recalls_wrong_source_right": 1}),
        term(MEM, "sum", columns={"recalls_correct_source_right": 1, "recalls_wrong_source_right": 1}),
        "of the recalls whose stored answer was right for its own incident, the share that were wrong: collision or staleness (K6)"),
    "collisions_per_stream": ratio(term(MEM, "sum", columns={"recalls_wrong_source_right": 1}), PER,
                                   "wrong recalls whose stored answer was right, per stream"),
    "collisions_q90": {"kind": "quantile", "q": 0.9, "description": "the 90th percentile over streams of the collisions in a stream",
                       "num": term(MEM, "sum", columns={"recalls_wrong_source_right": 1}), "den": PER},
    "inherited_per_stream": ratio(term(MEM, "sum", columns={"recalls_wrong_source_wrong": 1}), PER,
                                  "wrong recalls whose stored answer was wrong for its own incident, per stream"),
    "plain_unasked_wrong_per_stream": ratio(
        term(MEM, "sum", columns={"unasked_wrong_plain": 1}), PER,
        "plain incidents with a wrong declaration and no escalation (every origin), per stream; read as the paired excess over the control"),
    "plain_displaced_per_stream": ratio(
        term(MI, "count", [{"column": "tier", "op": "eq", "value": "plain"}, {"column": "unasked_correct@control", "op": "eq", "value": True},
                           {"column": "unasked_correct", "op": "eq", "value": False}], join={"arm": "control"}), PER,
        "plain incidents the control declared correctly with no escalation and this arm did not: correct declarations a recall displaced, per stream"),
    "calls_per_stream": ratio(term("results.csv", "sum", columns={"reasoner_calls": 1}), PER, "reasoner calls per stream"),
    "cost_s": ratio(term("results.csv", "sum", columns={"total_cost_ns": 1, "noticer_ns": 1}), PER,
                    "modelled seconds per stream: total_cost_ns plus the noticer's charge", 1e-9),
    "hard_recurrences_n": ratio(term(MI, "count", [HARD, REC]), PER, "hard recurrences (the population of the floor and the margin)"),
    "reachable_n": ratio(term(MEM, "sum", columns={"hard_reachable": 1}), PER, "hard incidents reachable by a family-keyed memory"),
}


def clause(cid, kind, arm, measure, desc, tests=None, **kw):
    c = {"id": cid, "kind": kind, "arm": arm, "measure": measure, "description": desc}
    if tests is not None:
        c["tests"] = [{"on": o, "op": op, "value": v} for o, op, v in tests]
    c.update(kw)
    return c


CLAUSES = [
    clause("power_recurrences", "count", "control", "recurrence_unasked_correct",
           "a precondition: the held-out range holds enough hard recurrences to resolve a paired margin of 0.10 (PROPOSAL: at least 70 of 83; W3's world C would raise it)",
           [("total", ">=", 70)]),
    clause("power_reachable", "count", "control", "reachable_unasked_correct",
           "a precondition: enough reachable hard incidents (PROPOSAL: at least 120; the held-out range holds 134)", [("total", ">=", 120)]),
    clause("floor_recurrences", "value", "engram", "recurrence_unasked_correct",
           "W2 10.2, the floor on hard recurrences: unasked correct at least 0.15 of them with the 90% lower bound above 0.075 (PROPOSAL, W2's)",
           [("point", ">=", 0.15), ("lower", ">", 0.075)]),
    clause("margin_over_record", "paired", "engram", "recurrence_unasked_correct",
           "W2 10.2, the margin over the record rung of the same key form: +0.10 of hard recurrences with the lower bound above 0 (PROPOSAL, W2's)",
           [("point", ">=", 0.10), ("lower", ">", 0)], minus="record"),
    clause("reachable_over_record", "paired", "engram", "reachable_unasked_correct",
           "the same margin on the reachable hard incidents, where the family form's cross-stream law lives (PROPOSAL: reported with the same bounds; the chief may fix another)",
           [("point", ">=", 0.10), ("lower", ">", 0)], minus="record"),
    clause("collision_share_bound", "value", "engram", "collision_share",
           "W2 10.2(a): of the recalls whose stored answer was right for its own incident, at most 0.20 wrong, with the 90% upper bound at most 0.30 (PROPOSAL, W2's)",
           [("point", "<=", 0.20), ("upper", "<=", 0.30)]),
    clause("collisions_per_stream_bound", "value", "engram", "collisions_per_stream",
           "W2 10.2(a): at most 0.25 such wrong recalls per stream (PROPOSAL, W2's)", [("point", "<=", 0.25)]),
    clause("collisions_q90", "value", "engram", "collisions_q90",
           "the tail: no more than one collision in the stream at the 90th percentile (PROPOSAL, a reading of 'per stream' that a mean can hide)",
           [("point", "<=", 1.0)]),
    clause("inherited_not_above_record", "paired", "engram", "inherited_per_stream",
           "W2 10.2(b): no more inherited wrong recalls per stream than the record rung of the same key form (no absolute bound)",
           [("point", "<=", 0.0)], minus="record"),
    clause("plain_excess", "paired", "engram", "plain_unasked_wrong_per_stream",
           "the review log's decision after A1a: the paired excess of plain incidents with a wrong unasked declaration over the memoryless control, per stream, at most 0.25 (A1c's acceptance, 5 in 20 streams) on the 90% upper bound (PROPOSAL)",
           [("upper", "<=", 0.25)], minus="control"),
    clause("plain_displaced", "value", "engram", "plain_displaced_per_stream",
           "correct plain declarations a recall displaced, per stream: none (PROPOSAL)", [("point", "<=", 0.0)]),
    clause("cost_under_record", "paired", "engram", "cost_s",
           "the engram costs no more per stream than the record rung, the noticer's charge included (PROPOSAL; W2's interval on the ceiling is about 3 points of the bill)",
           [("point", "<=", 0.0)], minus="record"),
    clause("record_share", "value", "record", "recurrence_unasked_correct",
           "W2 10.2's pre-accepted outcome: the record rung's share of hard recurrences above 0.27 puts +0.10 out of reach for any arm (PROPOSAL, W2's)",
           [("point", ">", 0.27)]),
    clause("record_collision_bound", "value", "record", "collision_share",
           "beside, never in the verdict: whether the record rung meets the collision bound it is the comparator for", [("point", "<=", 0.20)]),
]

STALE = {"name": "stale errors within the bound", "all": ["collision_share_bound", "collisions_per_stream_bound", "collisions_q90"]}
SPEC = {
    "schema": C.SCHEMA, "id": "a1b",
    "title": "A1b DRAFT: the engram against the record rung, family-keyed and carried across streams",
    "source": "DRAFT written by Lab 2 in unit E1 from W2 section 10.2 and the review log entries after W2 and A1a; the chief fixes the numbers and writes experiments/criteria/a1b.json before any A1b run",
    "readings": [
        "The primary population is the family form carried across streams; the site form is a secondary row (the review log, after W2).",
        "'Unasked correct' is the evaluator's K3: a correct declaration and no escalation about the incident, whoever made it; on hard incidents only a memory makes one.",
        "Collision share is over the recalls whose stored answer was right for its own incident (K5, K6); inherited errors are compared with the record rung's and have no absolute bound.",
        "The paired excess of unasked wrong declarations is read on plain incidents, where the cheap rung's own errors cancel against the control's.",
        "The stale-error clauses are evaluated with and without the reset at the stream boundary by running the arms twice; the carried version of a site-keyed arm fails by construction and is reported, not tuned.",
        "Cost is total_cost_ns plus noticer_ns (results.csv); total_cost_ns alone omits the medium's noticer charge.",
    ],
    "runs": {"heldout": {"streams": {"first_seed": 40000, "count": 200}}},
    "arms": {
        "engram": {"run": "heldout", "dir": "sel_eng_family_carried_privileged"},
        "record": {"run": "heldout", "dir": "sel_rec_family_carried_privileged"},
        "control": {"run": "heldout", "dir": "sel_reanchor_privileged"},
    },
    "bootstrap": {"unit": "stream", "resamples": 10000, "seed": 9950, "percentiles": [5, 95], "interval": "lower_higher", "chunk": 500},
    "measures": MEASURES,
    "clauses": CLAUSES,
    "reports": [
        {"id": "shares", "kind": "table", "arms": ["engram", "record", "control"],
         "measures": ["recurrence_unasked_correct", "reachable_unasked_correct", "hard_unasked_correct"],
         "description": "unasked-correct shares by population"},
        {"id": "errors", "kind": "table", "arms": ["engram", "record"],
         "measures": ["collision_share", "collisions_per_stream", "inherited_per_stream", "plain_unasked_wrong_per_stream"],
         "description": "the two kinds of stale error and the plain-incident errors"},
        {"id": "cost", "kind": "paired_table", "pairs": [["engram", "record"], ["engram", "control"], ["record", "control"]],
         "measures": ["calls_per_stream", "cost_s"], "description": "calls and modelled seconds per stream, paired"},
    ],
    "verdict": {"name": "the memory holds: floor, margin, stale errors within the bound, no more inherited errors than the record rung, plain errors and cost bounded",
                "all": ["power_recurrences", "power_reachable", "floor_recurrences", "margin_over_record", STALE,
                        "inherited_not_above_record", "plain_excess", "plain_displaced", "cost_under_record"]},
    "observations": {"record_collision_within_bound": {"all": ["record_collision_bound"]},
                     "reachable_margin_holds": {"all": ["reachable_over_record"]}},
    "outcomes": {
        "name": "A1b's exclusive outcomes, in the order a reader should check them",
        "default": "no_gain_over_the_record_rung",
        "categories": [
            {"name": "underpowered", "when": {"not": {"all": ["power_recurrences", "power_reachable"]}}},
            {"name": "stale_errors_exceed_the_bound", "when": {"not": STALE | {"name": "stale errors within the bound (outcome)"}}},
            {"name": "memory_holds", "when": {"all": ["floor_recurrences", "margin_over_record", "inherited_not_above_record",
                                                       "plain_excess", "plain_displaced", "cost_under_record"]}},
            {"name": "the_record_rung_captures_the_lever", "when": {"all": ["record_share", {"not": "margin_over_record"}]}},
        ],
    },
}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dry-run", metavar="RUNDIR")
    ap.add_argument("--out", default=str(ROOT / "experiments/criteria/a1b.draft.json"))
    a = ap.parse_args()
    C.validate_spec(SPEC)
    if a.dry_run:
        spec = json.loads(json.dumps(SPEC))
        spec["arms"] = {"engram": {"run": "heldout", "dir": "sel_rec_family_timing_never_carry_privileged"},
                        "record": {"run": "heldout", "dir": "sel_rec_family_timing_k2_carry_privileged"},
                        "control": {"run": "heldout", "dir": "sel_reanchor_privileged"}}
        res = C.evaluate(spec, {"heldout": a.dry_run})
        print(C.render_markdown(res))
        return
    pathlib.Path(a.out).write_text(json.dumps(SPEC, indent=2) + "\n")
    print(a.out, C.spec_hash(SPEC))


if __name__ == "__main__":
    main()
