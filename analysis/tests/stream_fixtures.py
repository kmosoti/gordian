"""Write stream run directories for the tests, in the format the stream harness writes.

The values are made up; the format is the harness's (`crates/gordian-run/src/stream/results.rs`):
booleans as `true`/`false`, the two optional instants empty when there is no correct declaration,
counts as plain integers. The per-stream totals are derived from the incident rows the way the
evaluator derives them (`crates/gordian-stream-eval/RULES.md`, S19 to S26), so a fixture is
consistent unless a test overrides a column on purpose.
"""

from __future__ import annotations

import csv
import json
from pathlib import Path

from gordian_analysis.load import (
    STREAM_INCIDENTS_COLUMNS,
    STREAM_MEASURED_COLUMNS,
    STREAM_RESULTS_COLUMNS,
)

TOKENS_PER_CALL = 100
REFS_PER_CALL = 3
PRICE_NS_PER_TOKEN = 250_000


def incident(tier="plain", family="", **over) -> dict:
    """One incident row; every field the evaluator reports has a default."""
    row = {
        "tier": tier,
        "family": family,
        "critical": False,
        "correct_declarations": 0,
        "wrong_declarations": 0,
        "first_correct_at_ns": "",
        "time_to_first_correct_ns": "",
        "correct_by_deadline": False,
        "missed": False,
        "critical_miss": False,
        "escalations": 0,
        "informed_escalations": 0,
        "correct_escalations": 0,
    }
    row.update(over)
    if tier == "hard" and not family:
        row["family"] = "slow_leak"
    if row["correct_declarations"] and row["first_correct_at_ns"] == "":
        row["first_correct_at_ns"] = 5_000_000_000
        row["time_to_first_correct_ns"] = 2_000_000_000
    if tier != "decoy":
        row["missed"] = not row["correct_by_deadline"]
        row["critical_miss"] = bool(row["critical"] and row["missed"])
    return row


def _b(x: bool) -> str:
    return "true" if x else "false"


def _totals(incidents: list[dict], extra: dict) -> dict:
    by = lambda tier: [i for i in incidents if i["tier"] == tier]  # noqa: E731
    hard, plain, decoy = by("hard"), by("plain"), by("decoy")
    needed = sum(i["escalations"] for i in hard)
    unneeded = sum(i["escalations"] for i in plain + decoy)
    background = extra.get("background_escalations", 0)
    calls = needed + unneeded + background
    tokens = calls * TOKENS_PER_CALL
    substrate = extra.get("substrate_ns", 1_000_000)
    reasoner_cost = tokens * PRICE_NS_PER_TOKEN
    count = lambda rows, key: sum(1 for i in rows if i[key])  # noqa: E731
    decoy_alarms = sum(i["wrong_declarations"] for i in decoy)
    bg_alarms = extra.get("background_false_alarms", 0)
    return {
        "incidents_plain": len(plain),
        "incidents_hard": len(hard),
        "incidents_decoy": len(decoy),
        "critical_incidents": count(incidents, "critical"),
        "correct_plain": count(plain, "correct_by_deadline"),
        "correct_hard": count(hard, "correct_by_deadline"),
        "missed_plain": count(plain, "missed"),
        "missed_hard": count(hard, "missed"),
        "critical_missed_plain": count(plain, "critical_miss"),
        "critical_missed_hard": count(hard, "critical_miss"),
        "wrong_declarations": sum(i["wrong_declarations"] for i in plain + hard),
        "decoys_dismissed": sum(1 for i in decoy if i["correct_declarations"] > 0),
        "decoys_alarmed": sum(1 for i in decoy if i["wrong_declarations"] > 0),
        "decoys_silent": sum(
            1 for i in decoy if i["correct_declarations"] == 0 and i["wrong_declarations"] == 0
        ),
        "false_alarms": decoy_alarms + bg_alarms,
        "false_alarms_on_background": bg_alarms,
        "escalations_needed": needed,
        "escalations_unneeded": unneeded,
        "escalations_background": background,
        "hard_incidents_escalated": sum(1 for i in hard if i["escalations"] > 0),
        "other_incidents_escalated": sum(1 for i in plain + decoy if i["escalations"] > 0),
        "calls_informed": sum(i["informed_escalations"] for i in incidents),
        "calls_correct": sum(i["correct_escalations"] for i in incidents),
        "reasoner_calls": calls,
        "reasoner_refs": calls * REFS_PER_CALL,
        "reasoner_tokens": tokens,
        "reasoner_modelled_ns": reasoner_cost,
        "reasoner_latency_ns": calls * 4_000_000_000,
        "bill_comm": tokens,
        "reasoner_cost_ns": reasoner_cost,
        "substrate_ns": substrate,
        "modelled_component_ns": substrate - substrate // 4,
        "modelled_sched_ns": substrate // 4,
        "total_cost_ns": substrate + reasoner_cost,
    }


RESULT_DEFAULTS = {
    "duration_ns": 600_000_000_000,
    "observations": 900,
    "anomalies_noticed": 20,
    "probes_used": 3,
    "declarations": 20,
    "declared_incident": 18,
    "declared_dismissal": 2,
    "cheap_declarations": 20,
    "reasoner_declarations": 0,
    "escalations_refused": 0,
    "probes_refused": 0,
    "calls_unanswered": 0,
    "bill_compute": 900_000,
    "bill_probes": 3,
    "bill_time": 30_000_000,
    "components_run": 40,
    "components_skipped": 0,
    "rule_skipped": 0,
    "steps": 1200,
    "stop_reason": "horizon",
    "ops_component": 5000,
    "ops_sched": 700,
}


def stream(seed: int, incidents: list[dict], **over) -> dict:
    """One stream: its incident rows and any `results.csv` column to override."""
    return {"seed": seed, "incidents": incidents, **over}


def write_stream_arm(
    path: Path,
    streams: list[dict],
    run_id: str = "r.arm",
    role: str = "comparison",
    measured: bool = True,
    position: int = 0,
) -> Path:
    """Write `results.csv`, `incidents.csv` and (unless `measured=False`) `measured.csv`."""
    path.mkdir(parents=True, exist_ok=True)
    with open(path / "results.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=STREAM_RESULTS_COLUMNS, lineterminator="\n")
        w.writeheader()
        for s in streams:
            extra = {k: v for k, v in s.items() if k not in ("seed", "incidents")}
            row = {
                **RESULT_DEFAULTS,
                "run_id": run_id,
                "arm_role": role,
                "seed": s["seed"],
                **_totals(s["incidents"], extra),
            }
            row.update({k: v for k, v in extra.items() if k in STREAM_RESULTS_COLUMNS})
            w.writerow(row)
    with open(path / "incidents.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=STREAM_INCIDENTS_COLUMNS, lineterminator="\n")
        w.writeheader()
        for s in streams:
            for k, inc in enumerate(s["incidents"]):
                row = {"run_id": run_id, "arm_role": role, "seed": s["seed"], "incident": k, **inc}
                for key in ("critical", "correct_by_deadline", "missed", "critical_miss"):
                    row[key] = _b(row[key])
                w.writerow(row)
    if measured:
        with open(path / "measured.csv", "w", newline="") as fh:
            w = csv.DictWriter(fh, fieldnames=STREAM_MEASURED_COLUMNS, lineterminator="\n")
            w.writeheader()
            for s in streams:
                w.writerow(
                    {
                        "run_id": run_id,
                        "seed": s["seed"],
                        "measured_component_ns": 1_000,
                        "measured_sched_ns": 500,
                        "measured_harness_ns": 250,
                        "arm_position": position,
                    }
                )
    return path


def write_stream_run(path: Path, arms: dict[str, list[dict]], roles: dict | None = None) -> Path:
    """Write a run directory: one subdirectory per arm, and a `manifest.json`."""
    roles = roles or {}
    for position, (name, streams) in enumerate(arms.items()):
        write_stream_arm(
            path / name,
            streams,
            run_id=f"r.{name}",
            role=roles.get(name, "comparison"),
            position=position,
        )
    (path / "manifest.json").write_text(json.dumps({"run_id": "r", "arms": list(arms)}))
    return path


def mixed_streams() -> list[dict]:
    """Three streams with different shapes: no hard incident, one, and several."""
    return [
        stream(
            1,
            [
                incident("plain", correct_by_deadline=True, correct_declarations=1),
                incident("plain", critical=True),
                incident("decoy", correct_declarations=1),
            ],
        ),
        stream(
            2,
            [
                incident("plain", correct_by_deadline=True, correct_declarations=1, escalations=1),
                incident(
                    "hard",
                    "compound",
                    critical=True,
                    correct_by_deadline=True,
                    correct_declarations=1,
                    escalations=2,
                    informed_escalations=2,
                    correct_escalations=2,
                ),
                incident("decoy", wrong_declarations=1, escalations=1),
            ],
            background_escalations=1,
        ),
        stream(
            3,
            [
                incident("hard", "slow_leak", critical=True, wrong_declarations=2),
                incident("hard", "cascade", escalations=1, informed_escalations=1),
                incident("hard", "split_brain", correct_by_deadline=True, correct_declarations=1),
                incident("plain", correct_by_deadline=True, correct_declarations=1),
            ],
        ),
    ]
