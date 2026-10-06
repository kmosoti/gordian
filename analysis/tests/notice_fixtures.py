"""Write the notice files of an arm for the tests, in the format the harness writes (work item B1,
`crates/gordian-run/src/stream/results.rs`: NOTICES_HEADER, NOTICE_INCIDENTS_HEADER,
NOTICE_EVENTS_HEADER), beside the three files `stream_fixtures.write_stream_arm` writes.

The values are made up. A stream carries its notices as `notice_rows` (one dict per incident, the
same order as `incidents`, made by `noticed`) and `notice_bg` (notices anchored on background);
`with_notices` sets the stream's `anomalies_noticed` to the total, which the loader checks.
"""

from __future__ import annotations

import csv
from pathlib import Path

from gordian_analysis.load import (
    STREAM_NOTICE_EVENTS_COLUMNS,
    STREAM_NOTICE_INCIDENTS_COLUMNS,
    STREAM_NOTICES_COLUMNS,
)

FIRST_OBSERVATION_NS = 10_000_000_000


def noticed(notices=1, correct=True, latency_s=2.0) -> dict:
    """What the evaluator says about one incident: `notices` notices about it, an anchor-correct one
    among them when `correct`, the first `latency_s` seconds after its first observation."""
    return {"notices": notices, "correct": correct and notices > 0, "latency_s": latency_s}


def not_noticed() -> dict:
    return {"notices": 0, "correct": False, "latency_s": None}


def with_notices(stream: dict, rows: list[dict], bg: int = 0, retire: int = 0) -> dict:
    """`stream` (from `stream_fixtures.stream`) noticing as `rows` say, one per incident."""
    assert len(rows) == len(stream["incidents"])
    total = sum(r["notices"] for r in rows) + bg
    return {**stream, "notice_rows": rows, "notice_bg": bg, "notice_retire": retire,
            "anomalies_noticed": total}


def _b(x: bool) -> str:
    return "true" if x else "false"


def write_notice_files(path: Path, streams: list[dict], run_id="r.arm", role="comparison",
                       noticer="rung") -> None:
    """Write `notices.csv`, `notice_incidents.csv` and `notice_events.csv` of the arm at `path`."""
    streams_rows, incident_rows, event_rows = [], [], []
    for s in streams:
        rows = s["notice_rows"]
        bg = s["notice_bg"]
        retire = s["notice_retire"]
        by_tier = {"plain": [0, 0, 0], "hard": [0, 0, 0], "decoy": [0, 0, 0]}  # notices, noticed, correct
        anomaly = 0
        t_ns = 20_000_000_000
        for k, (inc, r) in enumerate(zip(s["incidents"], rows)):
            tier = inc["tier"]
            by_tier[tier][0] += r["notices"]
            by_tier[tier][1] += int(r["notices"] > 0)
            by_tier[tier][2] += int(r["correct"])
            lat = r["latency_s"]
            first_notice = ""
            latency_ns = ""
            if r["notices"] > 0:
                latency_ns = round(lat * 1e9)
                first_notice = FIRST_OBSERVATION_NS + latency_ns
            incident_rows.append({
                "run_id": run_id, "arm_role": role, "seed": s["seed"], "incident": k, "tier": tier,
                "family": inc["family"], "first_observation_at_ns": FIRST_OBSERVATION_NS,
                "notices": r["notices"], "noticed": _b(r["notices"] > 0),
                "first_notice_at_ns": first_notice, "notice_latency_ns": latency_ns,
                "anchor_correct": _b(r["correct"]),
            })
            for j in range(r["notices"]):
                good = r["correct"] and j == 0
                event_rows.append({
                    "run_id": run_id, "arm_role": role, "seed": s["seed"], "noticer": noticer,
                    "event": "notice", "anomaly": anomaly, "anchor": 100 + anomaly, "site": 3,
                    "anchor_at_ns": FIRST_OBSERVATION_NS + (0 if good else 5_000_000_000),
                    "at_ns": t_ns + anomaly, "incident": k,
                    "anchor_offset_ns": 0 if good else 5_000_000_000, "anchor_correct": _b(good),
                })
                anomaly += 1
        for j in range(bg):
            event_rows.append({
                "run_id": run_id, "arm_role": role, "seed": s["seed"], "noticer": noticer,
                "event": "notice", "anomaly": anomaly, "anchor": 100 + anomaly, "site": 4,
                "anchor_at_ns": 1_000_000_000, "at_ns": t_ns + anomaly, "incident": "",
                "anchor_offset_ns": "", "anchor_correct": "false",
            })
            anomaly += 1
        for j in range(retire):
            event_rows.append({
                "run_id": run_id, "arm_role": role, "seed": s["seed"], "noticer": noticer,
                "event": "retire", "anomaly": j, "anchor": 100 + j, "site": 3,
                "anchor_at_ns": 1_000_000_000, "at_ns": t_ns + 10_000_000_000 + j, "incident": "",
                "anchor_offset_ns": "", "anchor_correct": "",
            })
        streams_rows.append({
            "run_id": run_id, "arm_role": role, "seed": s["seed"], "noticer": noticer,
            "notices": anomaly, "notices_on_background": bg,
            "notices_on_plain": by_tier["plain"][0], "notices_on_hard": by_tier["hard"][0],
            "notices_on_decoy": by_tier["decoy"][0], "retirements": retire,
            "noticed_plain": by_tier["plain"][1], "noticed_hard": by_tier["hard"][1],
            "noticed_decoy": by_tier["decoy"][1], "anchor_correct_plain": by_tier["plain"][2],
            "anchor_correct_hard": by_tier["hard"][2], "anchor_correct_decoy": by_tier["decoy"][2],
        })
    for name, columns, rows in (
        ("notices.csv", STREAM_NOTICES_COLUMNS, streams_rows),
        ("notice_incidents.csv", STREAM_NOTICE_INCIDENTS_COLUMNS, incident_rows),
        ("notice_events.csv", STREAM_NOTICE_EVENTS_COLUMNS, event_rows),
    ):
        with open(path / name, "w", newline="") as fh:
            w = csv.DictWriter(fh, fieldnames=columns, lineterminator="\n")
            w.writeheader()
            w.writerows(rows)


def noticing_streams() -> list[dict]:
    """Three streams (the shapes of `stream_fixtures.mixed_streams`) with notices, a hand count in
    `test_notices.py`."""
    from stream_fixtures import incident, stream

    return [
        with_notices(
            stream(1, [incident("plain"), incident("plain"), incident("decoy")]),
            [noticed(1, True, 1.0), not_noticed(), noticed(1, True, 3.0)],
            bg=2, retire=1,
        ),
        with_notices(
            stream(2, [incident("plain"), incident("hard", "compound"), incident("decoy")]),
            [noticed(2, True, 0.5), noticed(2, False, 4.0), not_noticed()],
            bg=1, retire=2,
        ),
        with_notices(
            stream(
                3,
                [
                    incident("hard", "slow_leak"),
                    incident("hard", "cascade"),
                    incident("hard", "split_brain"),
                    incident("plain"),
                ],
            ),
            [noticed(1, False, 18.0), not_noticed(), noticed(1, True, 2.0), noticed(3, True, 1.5)],
            bg=0, retire=0,
        ),
    ]
