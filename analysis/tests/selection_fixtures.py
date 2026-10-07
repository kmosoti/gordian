"""Write the selection files of an arm for the tests, in the format the harness writes (work item B4,
`crates/gordian-run/src/stream/results.rs`: SELECTION_HEADER, SELECTION_NOTICES_HEADER), beside the
five files `stream_fixtures.write_stream_arm` and `notice_fixtures.write_notice_files` write.

The values are made up, and consistent the way the harness's are: the classes' calls, tokens and
modelled nanoseconds add up to the stream's reasoner totals (E2, S26), the notices by class are the
notice files' (N8), every notice of the record has a row, and the retirements are the notice record's.
A stream says how many of its first notices were retired (its `notice_retire`, as the notice fixtures
do) and which of them by a follow-up rule (`followup`, indices into the notices in order); the calls of
an incident are put on its first notice and the stream's background calls on its first background
notice, and the rest are unattributed.
"""

from __future__ import annotations

import csv
from pathlib import Path

import pandas as pd
from stream_fixtures import PRICE_NS_PER_TOKEN, TOKENS_PER_CALL

from gordian_analysis.load import (
    STREAM_SELECTION_CLASSES,
    STREAM_SELECTION_COLUMNS,
    STREAM_SELECTION_FIELDS,
    STREAM_SELECTION_NOTICES_COLUMNS,
)

NS_PER_CALL = TOKENS_PER_CALL * PRICE_NS_PER_TOKEN


def class_of(incident: dict) -> str:
    if incident["tier"] == "hard":
        return "leak" if incident["family"] == "slow_leak" else "hard"
    return incident["tier"]


def _b(x: bool) -> str:
    return "true" if x else "false"


def selection_rows(path: Path, streams: list[dict], run_id="r.arm", role="comparison",
                   noticer="rung", followup=None):
    """The per-stream and per-notice rows of the selection files for the notice files at `path`.

    `followup`: seed -> set of indices (into the stream's notices, in order) retired by a follow-up
    rule; the others of the first `notice_retire` are retired as quiet."""
    followup = followup or {}
    events = pd.read_csv(path / "notice_events.csv", dtype=str, keep_default_na=False)
    stream_rows, notice_rows = [], []
    for s in streams:
        ev = events[(events["seed"] == str(s["seed"])) & (events["event"] == "notice")]
        incidents = s["incidents"]
        calls = {c: 0 for c in STREAM_SELECTION_CLASSES}
        for inc in incidents:
            calls[class_of(inc)] += inc["escalations"]
        calls["background"] += s.get("background_escalations", 0)
        left = dict(calls)
        counts = {f: {c: 0 for c in STREAM_SELECTION_CLASSES} for f in STREAM_SELECTION_FIELDS}
        seen_incident = set()
        about = 0
        retire = s["notice_retire"]
        for pos, (_, e) in enumerate(ev.iterrows()):
            inc_id = e["incident"]
            if inc_id == "":
                cls = "background"
            else:
                cls = class_of(incidents[int(inc_id)])
            esc = 0
            key = (cls, inc_id)
            if key not in seen_incident:
                seen_incident.add(key)
                esc = incidents[int(inc_id)]["escalations"] if inc_id != "" else left["background"]
            about += esc
            retired = pos < retire
            cause = ("followup" if pos in followup.get(s["seed"], ()) else "quiet") if retired else ""
            rbe = retired and esc == 0
            counts["notices"][cls] += 1
            counts["escalated"][cls] += int(esc > 0)
            counts["retired_before_escalation"][cls] += int(rbe)
            counts["followup_retired"][cls] += int(cause == "followup")
            counts["followup_before_escalation"][cls] += int(cause == "followup" and rbe)
            notice_rows.append({
                "run_id": run_id, "arm_role": role, "seed": s["seed"], "noticer": noticer,
                "anomaly": e["anomaly"], "incident": inc_id, "class": cls, "escalations": esc,
                "first_escalation_at_ns": 25_000_000_000 if esc else "",
                "retired_at_ns": 30_000_000_000 + pos if retired else "",
                "retire_cause": cause, "retired_before_escalation": _b(rbe),
            })
        for c in STREAM_SELECTION_CLASSES:
            counts["calls"][c] = calls[c]
            counts["tokens"][c] = calls[c] * TOKENS_PER_CALL
            counts["modelled_ns"][c] = calls[c] * NS_PER_CALL
        row = {"run_id": run_id, "arm_role": role, "seed": s["seed"], "noticer": noticer,
               "escalations_unattributed": sum(calls.values()) - about}
        for f in STREAM_SELECTION_FIELDS:
            for c in STREAM_SELECTION_CLASSES:
                row[f"{f}_{c}"] = counts[f][c]
        stream_rows.append(row)
    return stream_rows, notice_rows


def write_selection_files(path: Path, streams: list[dict], **kw) -> None:
    """Write `selection.csv` and `selection_notices.csv` of the arm at `path`, which holds the notice
    files of `streams` already."""
    stream_rows, notice_rows = selection_rows(path, streams, **kw)
    for name, columns, rows in (
        ("selection.csv", STREAM_SELECTION_COLUMNS, stream_rows),
        ("selection_notices.csv", STREAM_SELECTION_NOTICES_COLUMNS, notice_rows),
    ):
        with open(path / name, "w", newline="") as fh:
            w = csv.DictWriter(fh, fieldnames=columns, lineterminator="\n")
            w.writeheader()
            w.writerows(rows)


def selecting_streams() -> list[dict]:
    """Three streams with notices and calls, a hand count in `test_selection.py`."""
    from notice_fixtures import noticed, not_noticed, with_notices
    from stream_fixtures import incident, stream

    return [
        with_notices(
            stream(
                1,
                [
                    incident("plain", escalations=2),
                    incident("decoy", escalations=1),
                    incident("plain"),
                ],
                background_escalations=1,
            ),
            [noticed(2, True, 1.0), noticed(1, True, 3.0), not_noticed()],
            bg=2, retire=3,
        ),
        with_notices(
            stream(
                2,
                [
                    incident("hard", "slow_leak"),
                    incident("hard", "compound", escalations=1),
                    incident("decoy"),
                    incident("decoy", escalations=2),
                ],
            ),
            [noticed(1, True, 5.5), noticed(1, True, 2.0), noticed(1, False, 9.0), noticed(2, False, 4.0)],
            bg=0, retire=4,
        ),
        with_notices(
            stream(3, [incident("hard", "slow_leak"), incident("plain")]),
            [noticed(1, True, 5.5), not_noticed()],
            bg=1, retire=0,
        ),
    ]
