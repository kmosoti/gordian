#!/usr/bin/env python3
"""Reproducible scaling benchmark for the pinned Jujutsu source substrate.

The fixture builder deliberately lives in this thin orchestration layer. It creates disposable
colocated JJ/Git repositories, then times only machine-readable read commands. The report is
fully self-contained: raw observations, source states, binary digests, and deterministic BCa ratio
analysis are all retained in one JSON document.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import platform
import random
import shutil
import subprocess
import sys
import tempfile
import time
from collections.abc import Sequence
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from statistics import NormalDist, median
from typing import Any

EXPERIMENT_ID = "jj-baseline"
BOOTSTRAP_RESAMPLES = 10_000
BOOTSTRAP_SEED = 20_260_831
FULL_WARMUPS = 10
FULL_MEASUREMENTS = 30
READ_TIMEOUT_SECONDS = 60.0


@dataclass(frozen=True)
class Shape:
    id: str
    description: str
    changes: int
    kind: str
    workspaces: int = 0


SHAPES: tuple[Shape, ...] = (
    Shape("linear-16", "linear history with 16 changes", 16, "linear"),
    Shape("linear-128", "linear history with 128 changes", 128, "linear"),
    Shape("wide-32", "32 sibling changes sharing one parent", 32, "wide"),
    Shape("merge-32", "two 16-change branches joined by one merge", 32, "merge"),
    Shape("workspaces-8", "one history with eight disposable workspaces", 16, "workspace", 8),
)


@dataclass(frozen=True)
class Condition:
    id: str
    description: str


CONDITIONS: tuple[Condition, ...] = (
    Condition("git-log", "Git machine-readable commit-id log"),
    Condition("jj-log", "Jujutsu machine-readable commit-id log"),
)


def utc_now() -> str:
    return datetime.now(UTC).isoformat().replace("+00:00", "Z")


def canonical_digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes().replace(b"\r\n", b"\n").replace(b"\r", b"\n")).hexdigest()


def file_digest(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def executable_identity(name: str) -> dict[str, str]:
    resolved = shutil.which(name)
    if resolved is None:
        raise RuntimeError(f"required executable is not on PATH: {name}")
    path = Path(resolved).resolve()
    version = subprocess.run([str(path), "--version"], capture_output=True, text=True, check=False)
    if version.returncode != 0:
        raise RuntimeError(f"{name} --version failed with exit {version.returncode}")
    return {
        "name": name,
        "path": str(path),
        "version_output": (version.stdout + version.stderr).strip(),
        "sha256": file_digest(path),
    }


def run_checked(command: Sequence[str], cwd: Path, *, timeout: float = READ_TIMEOUT_SECONDS) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(list(command), cwd=cwd, capture_output=True, text=True, timeout=timeout, check=False)
    if result.returncode != 0:
        detail = (result.stderr or result.stdout).strip()
        raise RuntimeError(f"command failed ({result.returncode}): {' '.join(command)}: {detail}")
    return result


def jj(repo: Path, *args: str, timeout: float = READ_TIMEOUT_SECONDS) -> subprocess.CompletedProcess[str]:
    return run_checked(["jj", *args], repo, timeout=timeout)


def commit_id(repo: Path, revision: str = "@-") -> str:
    return jj(repo, "log", "-r", revision, "--no-graph", "--ignore-working-copy", "-T", "commit_id").stdout.strip()


def write_and_commit(repo: Path, relative: str, content: str, message: str) -> str:
    target = repo / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content, encoding="utf-8")
    jj(repo, "commit", "-m", message)
    return commit_id(repo)


def initialize_fixture(repo: Path, shape: Shape) -> dict[str, Any]:
    repo.mkdir(parents=True, exist_ok=True)
    jj(repo, "git", "init", "--colocate")
    jj(repo, "config", "set", "--repo", "user.name", "Gordian benchmark")
    jj(repo, "config", "set", "--repo", "user.email", "benchmark@gordian.invalid")
    root = write_and_commit(repo, "fixture.txt", "root\n", "fixture root")
    commits: list[str] = [root]

    if shape.kind == "linear":
        for number in range(shape.changes):
            jj(repo, "new")
            commits.append(write_and_commit(repo, "linear.txt", f"change {number}\n", f"linear {number}"))
    elif shape.kind == "wide":
        for number in range(shape.changes):
            jj(repo, "new", root)
            commits.append(write_and_commit(repo, "wide.txt", f"sibling {number}\n", f"sibling {number}"))
    elif shape.kind == "merge":
        left = root
        for number in range(shape.changes // 2):
            jj(repo, "new", left)
            left = write_and_commit(repo, "left.txt", f"left {number}\n", f"left {number}")
        right = root
        for number in range(shape.changes // 2):
            jj(repo, "new", right)
            right = write_and_commit(repo, "right.txt", f"right {number}\n", f"right {number}")
        jj(repo, "new", left, right, "-m", "merge branches")
        commits.append(write_and_commit(repo, "merge.txt", "merged\n", "merge branches"))
    elif shape.kind == "workspace":
        for number in range(shape.changes):
            jj(repo, "new")
            commits.append(write_and_commit(repo, "workspace.txt", f"change {number}\n", f"workspace {number}"))
        base = commits[-1]
        for number in range(shape.workspaces):
            workspace = repo.parent / f"{repo.name}-workspace-{number}"
            jj(repo, "workspace", "add", str(workspace), "-r", base)
    else:
        raise ValueError(f"unknown fixture kind: {shape.kind}")

    benchmark_heads = commits[1:] if shape.kind == "wide" else [commits[-1]]
    union = " | ".join(benchmark_heads)
    jj_state = jj(
        repo,
        "log",
        "-r",
        f"ancestors({union}) ~ root()",
        "--no-graph",
        "--ignore-working-copy",
        "-T",
        'commit_id ++ "\\n"',
    ).stdout.splitlines()
    git_state = run_checked(["git", "rev-parse", "HEAD"], repo).stdout.strip()
    operation = jj(repo, "op", "log", "--no-graph", "-n", "1", "-T", 'id ++ "\\n"').stdout.strip()
    return {
        "shape": shape.id,
        "description": shape.description,
        "commit_count": len(jj_state),
        "declared_changes": shape.changes,
        "declared_workspaces": shape.workspaces,
        "jj_head": commit_id(repo),
        "git_head": git_state,
        "jj_commit_ids": jj_state,
        "jj_operation_id": operation,
        "benchmark_heads": benchmark_heads,
        "workspace_paths": [str(repo.parent / f"{repo.name}-workspace-{number}") for number in range(shape.workspaces)],
    }


def machine_read_command(condition: str, heads: Sequence[str]) -> list[str]:
    if not heads:
        raise ValueError("a benchmark fixture must expose at least one exact head")
    if condition == "git-log":
        return ["git", "rev-list", "--topo-order", *heads]
    if condition == "jj-log":
        union = " | ".join(heads)
        return [
            "jj",
            "log",
            "--no-graph",
            "--ignore-working-copy",
            "-r",
            f"ancestors({union}) ~ root()",
            "-T",
            'commit_id ++ "\\n"',
        ]
    raise ValueError(f"unknown benchmark condition: {condition}")


def timed_read(repo: Path, condition: str, heads: Sequence[str], *, iteration: int, warmup: bool) -> dict[str, Any]:
    command = machine_read_command(condition, heads)
    started = time.perf_counter_ns()
    try:
        result = subprocess.run(command, cwd=repo, capture_output=True, text=True, timeout=READ_TIMEOUT_SECONDS, check=False)
        outcome = "completed" if result.returncode == 0 and result.stdout.strip() else "failed"
        error = None
        if result.returncode != 0:
            error = f"exit {result.returncode}"
        elif not result.stdout.strip():
            error = "empty machine-readable stdout"
        stdout, stderr, returncode = result.stdout, result.stderr, result.returncode
    except subprocess.TimeoutExpired as exc:
        outcome = "timeout"
        error = f"timeout after {READ_TIMEOUT_SECONDS} seconds"
        stdout = exc.stdout if isinstance(exc.stdout, str) else ""
        stderr = exc.stderr if isinstance(exc.stderr, str) else ""
        returncode = None
    elapsed_ns = time.perf_counter_ns() - started
    return {
        "condition": condition,
        "iteration": iteration,
        "phase": "warmup" if warmup else "measured",
        "command": command,
        "elapsed_ns": elapsed_ns,
        "elapsed_seconds": elapsed_ns / 1_000_000_000,
        "returncode": returncode,
        "outcome": outcome,
        "error": error,
        "stdout": stdout,
        "stderr": stderr,
    }


def percentile(values: Sequence[float], probability: float) -> float:
    if not values:
        raise ValueError("cannot calculate a percentile of an empty sequence")
    ordered = sorted(values)
    position = (len(ordered) - 1) * probability
    lower, upper = math.floor(position), math.ceil(position)
    if lower == upper:
        return ordered[lower]
    return ordered[lower] + (ordered[upper] - ordered[lower]) * (position - lower)


def bca_ratio_interval(
    baseline: Sequence[float],
    treatment: Sequence[float],
    *,
    threshold: float,
    resamples: int = BOOTSTRAP_RESAMPLES,
    seed: int = BOOTSTRAP_SEED,
) -> tuple[float, float, float]:
    """Return a paired BCa interval and one-sided bootstrap probability."""
    if len(baseline) != len(treatment) or len(baseline) < 2:
        raise ValueError("BCa requires at least two paired observations")
    pairs = tuple((float(reference), float(candidate)) for reference, candidate in zip(baseline, treatment))

    def statistic(sample: Sequence[tuple[float, float]]) -> float:
        return float(
            median(candidate for _, candidate in sample)
            / median(reference for reference, _ in sample)
        )

    observed = statistic(pairs)
    rng = random.Random(seed)
    bootstrapped = [statistic(rng.choices(pairs, k=len(pairs))) for _ in range(resamples)]
    less = sum(value < observed for value in bootstrapped)
    equal = sum(value == observed for value in bootstrapped)
    bias_probability = (less + equal / 2) / resamples
    normal = NormalDist()
    z0 = normal.inv_cdf(min(1 - 1e-12, max(1e-12, bias_probability)))
    jackknife = [statistic(pairs[:index] + pairs[index + 1 :]) for index in range(len(pairs))]
    jack_mean = sum(jackknife) / len(jackknife)
    deviations = [jack_mean - value for value in jackknife]
    denominator = 6 * (sum(value * value for value in deviations) ** 1.5)
    acceleration = sum(value**3 for value in deviations) / denominator if denominator else 0.0
    adjusted: list[float] = []
    for tail in (0.025, 0.975):
        alpha_z = normal.inv_cdf(tail)
        numerator = z0 + alpha_z
        adjusted_probability = normal.cdf(z0 + numerator / (1 - acceleration * numerator))
        adjusted.append(min(1.0, max(0.0, adjusted_probability)))
    one_sided_probability = (1 + sum(value < threshold for value in bootstrapped)) / (
        resamples + 1
    )
    return (
        percentile(bootstrapped, adjusted[0]),
        percentile(bootstrapped, adjusted[1]),
        one_sided_probability,
    )


def apply_holm(summaries: list[dict[str, Any]], *, family_size: int) -> None:
    """Apply Holm-Bonferroni to the pre-registered shape family in place."""
    if len(summaries) > family_size:
        raise ValueError("observed benchmark family is larger than its registration")
    ranked = sorted(
        enumerate(summaries),
        key=lambda item: item[1]["regression_gate"]["one_sided_bootstrap_probability"],
    )
    still_rejecting = True
    cumulative_adjusted = 0.0
    for rank, (_, summary) in enumerate(ranked, start=1):
        gate = summary["regression_gate"]
        raw_probability = gate["one_sided_bootstrap_probability"]
        remaining = family_size - rank + 1
        critical = 0.05 / remaining
        cumulative_adjusted = max(cumulative_adjusted, min(1.0, remaining * raw_probability))
        rejected = still_rejecting and raw_probability <= critical
        if not rejected:
            still_rejecting = False
        interval_crossed = gate["unadjusted_lower_bound_crossed"]
        gate.update(
            {
                "holm_rank": rank,
                "holm_critical_probability": critical,
                "holm_adjusted_probability": cumulative_adjusted,
                "holm_rejected": rejected,
                "crossed": interval_crossed and rejected,
                "interpretation": (
                    "shape-specific Jujutsu read overhead is at least five percent after Holm correction"
                    if interval_crossed and rejected
                    else "five-percent overhead is not established after Holm correction"
                ),
            }
        )


def source_identity(repository_root: Path) -> dict[str, str]:
    exact_commit = commit_id(repository_root, "@")
    return {
        "jj_commit": exact_commit,
        "git_backend_commit": exact_commit,
    }


def environment(repository_root: Path, identities: dict[str, dict[str, str]]) -> dict[str, Any]:
    return {
        "source_state": source_identity(repository_root),
        "jj": identities["jj"],
        "git": identities["git"],
        "python": sys.version,
        "platform": platform.platform(),
        "machine": platform.machine(),
        "processor": platform.processor(),
        "cpu_count": os.cpu_count(),
    }


def analyze(measurements: Sequence[dict[str, Any]], shape: Shape, measured_count: int) -> dict[str, Any]:
    by_condition: dict[str, list[float]] = {condition.id: [] for condition in CONDITIONS}
    for measurement in measurements:
        if measurement["phase"] == "measured" and measurement["outcome"] == "completed":
            by_condition[measurement["condition"]].append(measurement["elapsed_seconds"])
    counts = {condition: len(values) for condition, values in by_condition.items()}
    if any(count != measured_count for count in counts.values()):
        raise RuntimeError(f"shape {shape.id} has invalid successful measured counts: {counts}")
    baseline, treatment = by_condition["git-log"], by_condition["jj-log"]
    ratio = median(treatment) / median(baseline)
    threshold = 1.05
    lower, upper, one_sided_probability = bca_ratio_interval(
        baseline, treatment, threshold=threshold
    )
    return {
        "shape": shape.id,
        "successful_measured_counts": counts,
        "condition_medians_seconds": {condition: median(values) for condition, values in by_condition.items()},
        "ratio_of_medians_jj_over_git": ratio,
        "ratio_bca_95_jj_over_git": {"lower": lower, "upper": upper},
        "regression_gate": {
            "threshold": threshold,
            "unadjusted_lower_bound_crossed": lower >= threshold,
            "one_sided_bootstrap_probability": one_sided_probability,
            "crossed": False,
            "interpretation": "Holm correction pending",
        },
        "bootstrap": {"resamples": BOOTSTRAP_RESAMPLES, "seed": BOOTSTRAP_SEED},
    }


def build_report(repository_root: Path, *, smoke: bool) -> dict[str, Any]:
    identities = {name: executable_identity(name) for name in ("jj", "git")}
    warmups, measured = (1, 2) if smoke else (FULL_WARMUPS, FULL_MEASUREMENTS)
    report: dict[str, Any] = {
        "report_format": "gordian-jj-baseline-report-v1",
        "experiment_id": EXPERIMENT_ID,
        "started_at": utc_now(),
        "finished_at": None,
        "mode": "smoke" if smoke else "full",
        "protocol_digest": canonical_digest(Path(__file__).with_name("protocol.json")),
        "protocol_path": str(Path(__file__).with_name("protocol.json")),
        "registered_counts": {"warmups": warmups, "measured": measured},
        "required_counts": {"warmups": FULL_WARMUPS, "measured": FULL_MEASUREMENTS},
        "environment": environment(repository_root, identities),
        "fixture_shapes": [],
        "measurements": [],
        "summaries": [],
        "failures": [],
    }
    shapes = SHAPES[:2] if smoke else SHAPES
    with tempfile.TemporaryDirectory(prefix="gordian-jj-baseline-") as temporary:
        fixture_root = Path(temporary)
        for shape in shapes:
            repo = fixture_root / shape.id
            try:
                fixture = initialize_fixture(repo, shape)
                report["fixture_shapes"].append(fixture)
                order = [condition.id for condition in CONDITIONS]
                random.Random(BOOTSTRAP_SEED + len(report["fixture_shapes"])).shuffle(order)
                shape_measurements: list[dict[str, Any]] = []
                heads = fixture["benchmark_heads"]
                for condition in order:
                    for iteration in range(warmups):
                        observation = timed_read(repo, condition, heads, iteration=iteration, warmup=True)
                        observation["shape"] = shape.id
                        shape_measurements.append(observation)
                    for iteration in range(measured):
                        observation = timed_read(repo, condition, heads, iteration=iteration, warmup=False)
                        observation["shape"] = shape.id
                        shape_measurements.append(observation)
                for phase in ("warmup", "measured"):
                    for iteration in range(warmups if phase == "warmup" else measured):
                        paired = [
                            item
                            for item in shape_measurements
                            if item["phase"] == phase and item["iteration"] == iteration
                        ]
                        distinct_revision_sets = {
                            frozenset(item["stdout"].splitlines()) for item in paired
                        }
                        if len(paired) != len(CONDITIONS) or len(distinct_revision_sets) != 1:
                            raise RuntimeError(
                                f"shape {shape.id} produced non-equivalent revision sets in {phase} iteration {iteration}"
                            )
                report["measurements"].extend(shape_measurements)
                summary = analyze(shape_measurements, shape, measured)
                summary["analysis_mode"] = "smoke" if smoke else "registered"
                report["summaries"].append(summary)
            except (OSError, RuntimeError, ValueError, subprocess.TimeoutExpired) as error:
                report["failures"].append({"shape": shape.id, "error": str(error)})
    report["finished_at"] = utc_now()
    apply_holm(report["summaries"], family_size=len(SHAPES))
    report["valid"] = not report["failures"] and (smoke or len(report["summaries"]) == len(SHAPES))
    report["registered_analysis_valid"] = report["valid"] and not smoke
    return report


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path, help="required JSON report path")
    parser.add_argument("--smoke", action="store_true", help="run two shapes with one warmup and two measurements")
    parser.add_argument("--repository-root", type=Path, default=Path(__file__).resolve().parents[2])
    args = parser.parse_args(argv)
    report = build_report(args.repository_root.resolve(), smoke=args.smoke)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output), "valid": report["valid"], "failures": len(report["failures"])}, sort_keys=True))
    return 0 if report["valid"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
