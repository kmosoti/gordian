"""R8: an index of every run directory, for the report.

Usage: r8_provenance.py OUT_PREFIX [RUNS_DIR]

Writes `OUT_PREFIX run-index.csv` (one row per `artifacts/runs/r8-*` directory: model label, phase,
calls made and designed, parse failures, transport errors, the client's wall time from the call
latencies, the server's wall time and peak memory from `usage.json` when it exists, and whether the
run ended early) and `OUT_PREFIX calls-sha256.csv` (the sha256 of every `calls.jsonl`, so the
analysis outputs can be tied to the files they read). Every run is listed, including interrupted
ones; nothing is left out.
"""

import csv
import hashlib
import json
import os
import sys


def main():
    prefix = sys.argv[1]
    runs = sys.argv[2] if len(sys.argv) > 2 else "artifacts/runs"
    rows, hashes = [], []
    for name in sorted(os.listdir(runs)):
        d = os.path.join(runs, name)
        if not (name.startswith("r8-") and os.path.isdir(d)):
            continue
        calls_path = os.path.join(d, "calls.jsonl")
        calls = []
        if os.path.exists(calls_path):
            with open(calls_path) as f:
                calls = [json.loads(line) for line in f if line.strip()]
            hashes.append(
                {"run": name, "sha256": hashlib.sha256(open(calls_path, "rb").read()).hexdigest()}
            )
        manifest = {}
        if os.path.exists(os.path.join(d, "manifest.json")):
            manifest = json.load(open(os.path.join(d, "manifest.json")))
        usage = {}
        if os.path.exists(os.path.join(d, "usage.json")):
            usage = json.load(open(os.path.join(d, "usage.json")))
        design_n = ""
        if os.path.exists(os.path.join(d, "design.json")):
            design_n = len(json.load(open(os.path.join(d, "design.json")))["calls"])
        rows.append(
            {
                "run": name,
                "model": manifest.get("model_label", ""),
                "phase": manifest.get("phase", ""),
                "calls_designed": design_n,
                "calls_recorded": len(calls),
                "parse_failures": sum(1 for c in calls if c["parse_status"] != "ok"),
                "transport_errors": sum(1 for c in calls if c["error"] is not None),
                "client_seconds": round(sum(c["latency_s"] for c in calls), 1),
                "server_wall_s": round(usage["wall_ns"] / 1e9, 1) if usage else "",
                "server_cpu_s": round(usage["cpu_ns"] / 1e9, 1) if usage else "",
                "peak_memory_mb": round(usage["peak_memory_bytes"] / 2**20) if usage else "",
                "oom_kills": usage.get("oom_kills", "") if usage else "",
                "complete": (len(calls) == design_n) if design_n != "" else "",
            }
        )
    for suffix, data in (("run-index.csv", rows), ("calls-sha256.csv", hashes)):
        if not data:
            continue
        with open(prefix + suffix, "w", newline="") as f:
            w = csv.DictWriter(f, fieldnames=list(data[0].keys()))
            w.writeheader()
            w.writerows(data)


if __name__ == "__main__":
    main()
