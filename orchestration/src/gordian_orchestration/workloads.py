"""Deterministic benchmark workload generation and validation.

This module intentionally contains no repository or Mission Graph integration.  A repository
derived workload is made only from the closed source mapping supplied by its caller.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

WORKLOAD_SCHEMA_VERSION = "gordian.workload.v1"
SOURCE_SCHEMA_VERSION = "gordian.repository-workload-source.v1"
GENERATOR_NAME = "gordian-workload-generator"
GENERATOR_VERSION = "1.0.0"
SEED_MATRIX = (1, 2, 3, 5, 8, 13, 21, 34)

_DISTRIBUTIONS: dict[str, dict[str, tuple[int, int]]] = {
    "rep-small": {
        "nodes": (50, 200),
        "width": (4, 12),
        "critical_path": (250_000, 400_000),
        "semantic_overlap": (50_000, 150_000),
        "resource_contention": (50_000, 200_000),
    },
    "rep-wide": {
        "nodes": (500, 2_000),
        "width": (40, 120),
        "critical_path": (50_000, 150_000),
        "semantic_overlap": (50_000, 200_000),
        "resource_contention": (50_000, 250_000),
    },
    "adv-deep": {
        "nodes": (200, 800),
        "width": (2, 4),
        "critical_path": (600_000, 900_000),
        "semantic_overlap": (0, 100_000),
        "resource_contention": (0, 200_000),
    },
    "adv-contended": {
        "nodes": (300, 1_000),
        "width": (20, 60),
        "critical_path": (150_000, 300_000),
        "semantic_overlap": (550_000, 850_000),
        "resource_contention": (550_000, 850_000),
    },
}

_TOP_KEYS = {
    "schema_version",
    "workload_id",
    "distribution_id",
    "seed",
    "generator",
    "origin",
    "dimensions",
    "nodes",
    "edges",
    "workers",
    "events",
    "evidence",
    "resources",
}
_DIMENSION_KEYS = {
    "node_count",
    "edge_count",
    "density_numerator",
    "density_denominator",
    "density_ppm",
    "dag_width",
    "dag_depth",
    "critical_path_nodes",
    "critical_path_duration_ticks",
    "total_duration_ticks",
    "critical_path_ratio_ppm",
    "max_fan_in",
    "max_fan_out",
    "semantic_overlap_numerator",
    "semantic_overlap_denominator",
    "semantic_claim_overlap_ppm",
    "resource_contention_numerator",
    "resource_contention_denominator",
    "resource_contention_ppm",
    "worker_count",
    "capability_count",
    "event_count",
    "evidence_count",
    "min_worker_cost_units_per_second",
    "max_worker_cost_units_per_second",
    "min_worker_duration_scale_ppm",
    "max_worker_duration_scale_ppm",
    "fan_in_histogram",
    "fan_out_histogram",
}
_NODE_KEYS = {
    "id",
    "rank",
    "reference_duration_ticks",
    "cost_units",
    "required_capabilities",
    "semantic_claims",
    "resource_claims",
    "fan_in",
    "fan_out",
}
_EDGE_KEYS = {"from", "to"}
_WORKER_KEYS = {"id", "capabilities", "cost_units_per_second", "duration_scale_ppm"}
_EVENT_KEYS = {"id", "sequence", "node_id", "kind"}
_EVIDENCE_KEYS = {"id", "sequence", "node_id", "kind", "subject"}
_RESOURCE_KEYS = {"id", "capacity_units"}


class WorkloadError(ValueError):
    """Raised when a workload or repository source violates its public contract."""


class _SourceMapping(dict[str, Any]):
    """Closed source mapping carrying a non-serialised acquisition digest."""

    def __init__(self, value: Mapping[str, Any], file_digest: str | None = None) -> None:
        super().__init__(value)
        self.file_digest = file_digest


class _Rng:
    """Small fixed-width PRNG whose behavior is independent of ``random`` implementation."""

    def __init__(self, seed: int) -> None:
        self.state = seed & ((1 << 64) - 1)

    def next(self) -> int:
        self.state = (self.state + 0x9E3779B97F4A7C15) & ((1 << 64) - 1)
        value = self.state
        value = ((value ^ (value >> 30)) * 0xBF58476D1CE4E5B9) & ((1 << 64) - 1)
        value = ((value ^ (value >> 27)) * 0x94D049BB133111EB) & ((1 << 64) - 1)
        return value ^ (value >> 31)

    def pick(self, low: int, high: int) -> int:
        return low + self.next() % (high - low + 1)

    def shuffle(self, values: list[int]) -> None:
        for index in range(len(values) - 1, 0, -1):
            other = self.next() % (index + 1)
            values[index], values[other] = values[other], values[index]


def _is_int(value: Any) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


def _require_mapping(value: Any, label: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise WorkloadError(f"{label} must be a mapping")
    return value


def _keys(value: Mapping[str, Any], expected: set[str], label: str) -> None:
    actual = set(value)
    if actual != expected:
        missing = ", ".join(sorted(expected - actual))
        extra = ", ".join(sorted(actual - expected))
        detail = []
        if missing:
            detail.append(f"missing {missing}")
        if extra:
            detail.append(f"unexpected {extra}")
        raise WorkloadError(f"{label} has invalid keys ({'; '.join(detail)})")


def _text(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value:
        raise WorkloadError(f"{label} must be a non-empty string")
    return value


def _uint(value: Any, label: str, maximum: int = (1 << 64) - 1) -> int:
    if not _is_int(value) or value < 0 or value > maximum:
        raise WorkloadError(f"{label} must be an unsigned integer")
    return value


def _list(value: Any, label: str) -> list[Any]:
    if not isinstance(value, list):
        raise WorkloadError(f"{label} must be a list")
    return value


def _sorted_unique_text(value: Any, label: str) -> list[str]:
    items = _list(value, label)
    if any(not isinstance(item, str) or not item for item in items):
        raise WorkloadError(f"{label} must contain non-empty strings")
    if len(set(items)) != len(items) or items != sorted(items):
        raise WorkloadError(f"{label} must be sorted and duplicate-free")
    return items


def canonical_json(payload: Mapping[str, Any]) -> bytes:
    """Encode compact canonical UTF-8 JSON with exactly one final LF."""
    if not isinstance(payload, Mapping):
        raise WorkloadError("payload must be a mapping")
    try:
        encoded = json.dumps(
            payload, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False
        ).encode("utf-8")
        return encoded.rstrip(b"\n") + b"\n"
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise WorkloadError(f"payload is not canonical JSON: {exc}") from exc


def _identity_payload(payload: Mapping[str, Any]) -> dict[str, Any]:
    result = dict(payload)
    result.pop("workload_id", None)
    return result


def workload_digest(payload: Mapping[str, Any]) -> str:
    """Return the SHA-256 digest of framed canonical payload bytes excluding ``workload_id``."""
    return hashlib.sha256(canonical_json(_identity_payload(payload))).hexdigest()


def _ppm_count(low_high: tuple[int, int], count: int) -> int:
    low, high = low_high
    minimum = (low * count + 999_999) // 1_000_000
    maximum = high * count // 1_000_000
    if minimum > maximum:
        raise WorkloadError("no integer node count satisfies requested parts-per-million envelope")
    return minimum


def _pair_count(low_high: tuple[int, int], denominator: int) -> int:
    if denominator == 0:
        if low_high[0] == 0:
            return 0
        raise WorkloadError("shape has no incomparable pairs for a nonzero overlap envelope")
    low, high = low_high
    minimum = (low * denominator + 999_999) // 1_000_000
    maximum = high * denominator // 1_000_000
    if minimum > maximum:
        raise WorkloadError("no integer pair count satisfies requested parts-per-million envelope")
    return minimum


def _ratio_in(numerator: int, denominator: int, bounds: tuple[int, int]) -> bool:
    """Check a ppm range by exact cross multiplication, never by the displayed floor."""
    if denominator <= 0:
        return False
    low, high = bounds
    return low * denominator <= numerator * 1_000_000 <= high * denominator


def _choose_shape(distribution_id: str, seed: int, node_count: int | None) -> tuple[int, int, int]:
    spec = _DISTRIBUTIONS.get(distribution_id)
    if spec is None:
        raise WorkloadError(f"unknown distribution_id: {distribution_id}")
    rng = _Rng(seed ^ int.from_bytes(hashlib.sha256(distribution_id.encode()).digest()[:8], "big"))
    n = node_count if node_count is not None else rng.pick(*spec["nodes"])
    if not _is_int(n) or n < spec["nodes"][0] or n > spec["nodes"][1]:
        raise WorkloadError(f"node_count {n!r} is outside {distribution_id} envelope")
    choices: list[tuple[int, int]] = []
    for depth in range(1, n + 1):
        if not _ratio_in(depth, n, spec["critical_path"]):
            continue
        for width in range(spec["width"][0], spec["width"][1] + 1):
            if depth <= n <= depth * width:
                choices.append((width, depth))
    if not choices:
        raise WorkloadError(f"node_count {n} cannot satisfy {distribution_id} shape envelope")
    width, depth = choices[rng.next() % len(choices)]
    return n, width, depth


def _layers(node_count: int, width: int, depth: int, rng: _Rng) -> list[int]:
    sizes = [1] * depth
    peak = rng.next() % depth
    sizes[peak] = width
    remaining = node_count - depth - (width - 1)
    order = list(range(depth))
    rng.shuffle(order)
    for index in order:
        if index == peak:
            continue
        add = min(width - 1, remaining)
        sizes[index] += add
        remaining -= add
        if not remaining:
            break
    if remaining:
        raise WorkloadError("unable to construct requested exact layer shape")
    return sizes


def _canonical_source(source: Mapping[str, Any]) -> dict[str, Any]:
    value = _require_mapping(source, "source")
    _keys(value, {"schema_version", "repository", "revision", "nodes", "edges"}, "source")
    if value["schema_version"] != SOURCE_SCHEMA_VERSION:
        raise WorkloadError(f"source schema_version must be {SOURCE_SCHEMA_VERSION}")
    repository = _text(value["repository"], "source.repository")
    revision = _text(value["revision"], "source.revision")
    raw_nodes = _list(value["nodes"], "source.nodes")
    raw_edges = _list(value["edges"], "source.edges")
    nodes: list[dict[str, Any]] = []
    seen: set[str] = set()
    for index, raw in enumerate(raw_nodes):
        item = _require_mapping(raw, f"source.nodes[{index}]")
        required = {
            "id",
            "reference_duration_ticks",
            "cost_units",
            "required_capabilities",
            "semantic_claims",
            "resource_claims",
        }
        allowed = required | {"event_count", "evidence_count"}
        if set(item) - allowed or not required <= set(item):
            raise WorkloadError(f"source.nodes[{index}] has invalid keys")
        node_id = _text(item["id"], f"source.nodes[{index}].id")
        if node_id in seen:
            raise WorkloadError(f"duplicate source node id: {node_id}")
        seen.add(node_id)
        duration = _uint(
            item["reference_duration_ticks"],
            f"source.nodes[{index}].reference_duration_ticks",
            10_000_000_000,
        )
        if duration == 0:
            raise WorkloadError("source node reference_duration_ticks must be positive")
        cost = _uint(item["cost_units"], f"source.nodes[{index}].cost_units", 10_000_000_000)
        caps = _sorted_unique_text(
            item["required_capabilities"], f"source.nodes[{index}].required_capabilities"
        )
        claims = _sorted_unique_text(
            item["semantic_claims"], f"source.nodes[{index}].semantic_claims"
        )
        resources = _sorted_unique_text(
            item["resource_claims"], f"source.nodes[{index}].resource_claims"
        )
        event_count = _uint(
            item.get("event_count", 1), f"source.nodes[{index}].event_count", 1_000_000
        )
        evidence_count = _uint(
            item.get("evidence_count", 1), f"source.nodes[{index}].evidence_count", 1_000_000
        )
        nodes.append(
            {
                "id": node_id,
                "reference_duration_ticks": duration,
                "cost_units": cost,
                "required_capabilities": caps,
                "semantic_claims": claims,
                "resource_claims": resources,
                "event_count": event_count,
                "evidence_count": evidence_count,
            }
        )
    nodes.sort(key=lambda item: item["id"])
    raw_edge_values: list[tuple[str, str]] = []
    edge_seen: set[tuple[str, str]] = set()
    for index, raw in enumerate(raw_edges):
        item = _require_mapping(raw, f"source.edges[{index}]")
        _keys(item, _EDGE_KEYS, f"source.edges[{index}]")
        start = _text(item["from"], f"source.edges[{index}].from")
        end = _text(item["to"], f"source.edges[{index}].to")
        pair = (start, end)
        if start == end:
            raise WorkloadError(f"source edge is a self-loop: {start}")
        if start not in seen or end not in seen:
            raise WorkloadError(f"source edge references an unknown node: {start}->{end}")
        if pair in edge_seen:
            raise WorkloadError(f"duplicate source edge: {start}->{end}")
        edge_seen.add(pair)
        raw_edge_values.append(pair)
    raw_edge_values.sort()
    indegree = {node["id"]: 0 for node in nodes}
    outgoing: dict[str, list[str]] = {node["id"]: [] for node in nodes}
    for start, end in raw_edge_values:
        indegree[end] += 1
        outgoing[start].append(end)
    queue = sorted(node_id for node_id, degree in indegree.items() if degree == 0)
    topo: list[str] = []
    while queue:
        current = queue.pop(0)
        topo.append(current)
        for target in outgoing[current]:
            indegree[target] -= 1
            if indegree[target] == 0:
                queue.append(target)
                queue.sort()
    if len(topo) != len(nodes):
        raise WorkloadError("source graph contains a cycle")
    return {
        "schema_version": SOURCE_SCHEMA_VERSION,
        "repository": repository,
        "revision": revision,
        "nodes": nodes,
        "edges": [{"from": start, "to": end} for start, end in raw_edge_values],
    }


def _ranks(
    nodes: Sequence[Mapping[str, Any]], edges: Sequence[Mapping[str, Any]]
) -> dict[str, int]:
    predecessors: dict[str, list[str]] = {str(node["id"]): [] for node in nodes}
    outgoing: dict[str, list[str]] = {str(node["id"]): [] for node in nodes}
    for edge in edges:
        predecessors[str(edge["to"])].append(str(edge["from"]))
        outgoing[str(edge["from"])].append(str(edge["to"]))
    indegree = {node_id: len(value) for node_id, value in predecessors.items()}
    queue = sorted(node_id for node_id, degree in indegree.items() if degree == 0)
    ranks = {node_id: 0 for node_id in predecessors}
    visited = 0
    while queue:
        current = queue.pop(0)
        visited += 1
        for target in sorted(outgoing[current]):
            ranks[target] = max(ranks[target], ranks[current] + 1)
            indegree[target] -= 1
            if indegree[target] == 0:
                queue.append(target)
                queue.sort()
    if visited != len(nodes):
        raise WorkloadError("graph contains a cycle")
    return ranks


def _pair_metrics(
    nodes: Sequence[Mapping[str, Any]], edges: Sequence[Mapping[str, Any]], field: str
) -> tuple[int, int]:
    """Return (incomparable pairs sharing a claim, all incomparable pairs)."""
    ids = [str(node["id"]) for node in nodes]
    outgoing = {node_id: [] for node_id in ids}
    for edge in edges:
        outgoing[str(edge["from"])].append(str(edge["to"]))
    reaches: dict[str, set[str]] = {node_id: set() for node_id in ids}
    # Edges are acyclic (and _ranks has already checked this at each call site).  Reverse
    # identifier order is sufficient for the generated graph, while the stack makes this
    # independent of source edge ordering.
    ranks = {str(node["id"]): int(node["rank"]) for node in nodes}
    for node_id in sorted(ids, key=lambda item: (ranks[item], item), reverse=True):
        for target in outgoing[node_id]:
            reaches[node_id].add(target)
            reaches[node_id].update(reaches[target])
    numerator = 0
    denominator = 0
    claim_sets = {str(node["id"]): set(node[field]) for node in nodes}
    for index, left in enumerate(ids):
        for right in ids[index + 1 :]:
            if right in reaches[left] or left in reaches[right]:
                continue
            denominator += 1
            if claim_sets[left] & claim_sets[right]:
                numerator += 1
    return numerator, denominator


def _histogram(values: Sequence[int] | Any) -> dict[str, int]:
    result: dict[str, int] = {}
    for value in values:
        key = str(value)
        result[key] = result.get(key, 0) + 1
    return {key: result[key] for key in sorted(result, key=lambda item: int(item))}


def _maximum_antichain(
    nodes: Sequence[Mapping[str, Any]], edges: Sequence[Mapping[str, Any]]
) -> int:
    """Compute the exact width of a DAG's reachability poset via Dilworth's theorem."""
    ids = [str(node["id"]) for node in nodes]
    outgoing = {node_id: [] for node_id in ids}
    for edge in edges:
        outgoing[str(edge["from"])].append(str(edge["to"]))
    reaches = {node_id: set() for node_id in ids}
    # Ranks strictly increase along an edge, so this reverse topological sweep is deterministic.
    rank_map = {str(node["id"]): int(node["rank"]) for node in nodes}
    for node_id in sorted(ids, key=lambda item: (rank_map[item], item), reverse=True):
        for target in outgoing[node_id]:
            reaches[node_id].add(target)
            reaches[node_id].update(reaches[target])
    matched: dict[str, str] = {}

    def visit(left: str, seen: set[str]) -> bool:
        for right in sorted(reaches[left]):
            if right in seen:
                continue
            seen.add(right)
            if right not in matched or visit(matched[right], seen):
                matched[right] = left
                return True
        return False

    matching = sum(1 for node_id in ids if visit(node_id, set()))
    return len(ids) - matching


def _records(
    nodes: Sequence[Mapping[str, Any]], rng: _Rng
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    events: list[dict[str, Any]] = []
    evidence: list[dict[str, Any]] = []
    sequence = 0
    evidence_sequence = 0
    for node in nodes:
        for _ in range(int(node.get("event_count", 1))):
            events.append(
                {
                    "id": f"event-{sequence:08d}",
                    "sequence": sequence,
                    "node_id": node["id"],
                    "kind": "node-observed",
                }
            )
            sequence += 1
        for _ in range(int(node.get("evidence_count", 1))):
            evidence.append(
                {
                    "id": f"evidence-{evidence_sequence:08d}",
                    "sequence": evidence_sequence,
                    "node_id": node["id"],
                    "kind": "node-evidence",
                    "subject": node["id"],
                }
            )
            evidence_sequence += 1
    return events, evidence


def _workers(capabilities: Sequence[str], seed: int, count: int = 4) -> list[dict[str, Any]]:
    caps = list(capabilities)
    if not caps:
        caps = ["capability:general"]
    count = max(1, min(8, count))
    result: list[dict[str, Any]] = []
    for index in range(count):
        # Worker zero is the universal fallback. Other workers expose distinct subsets, while
        # the extra general capability distinguishes the one-capability case.
        if index == 0:
            worker_capabilities = sorted(caps)
        elif len(caps) == 1:
            worker_capabilities = sorted((*caps, "capability:general"))
        else:
            worker_capabilities = [caps[index % len(caps)]]
        result.append(
            {
                "id": f"worker-{index:03d}",
                "capabilities": worker_capabilities,
                "cost_units_per_second": 80 + ((seed + index * 37) % 121),
                "duration_scale_ppm": 500_000 + ((seed * 17 + index * 31) % 500_001),
            }
        )
    return result


def _assemble(
    distribution_id: str,
    seed: int,
    nodes: list[dict[str, Any]],
    edges: list[dict[str, Any]],
    origin: dict[str, Any],
    parameters: dict[str, Any],
    rng: _Rng,
) -> dict[str, Any]:
    nodes.sort(key=lambda item: item["id"])
    edges.sort(key=lambda item: (item["from"], item["to"]))
    capabilities = sorted({cap for node in nodes for cap in node["required_capabilities"]})
    workers = _workers(capabilities, seed, int(parameters.get("worker_count", 4)))
    events, evidence = _records(nodes, rng)
    ranks = _ranks(nodes, edges)
    for node in nodes:
        node["rank"] = ranks[node["id"]]
    node_count = len(nodes)
    depth = max(ranks.values(), default=-1) + 1
    indegree = {node["id"]: 0 for node in nodes}
    outdegree = {node["id"]: 0 for node in nodes}
    for edge in edges:
        indegree[edge["to"]] += 1
        outdegree[edge["from"]] += 1
    for node in nodes:
        node["fan_in"] = indegree[node["id"]]
        node["fan_out"] = outdegree[node["id"]]
        node.pop("event_count", None)
        node.pop("evidence_count", None)
    width = _maximum_antichain(nodes, edges)
    semantic_numerator, semantic_denominator = _pair_metrics(nodes, edges, "semantic_claims")
    resource_numerator, resource_denominator = _pair_metrics(nodes, edges, "resource_claims")
    total_duration = sum(node["reference_duration_ticks"] for node in nodes)
    order = sorted(nodes, key=lambda item: (item["rank"], item["id"]))
    longest = {node["id"]: node["reference_duration_ticks"] for node in nodes}
    path_nodes = {node["id"]: 1 for node in nodes}
    for node in order:
        predecessors = [edge["from"] for edge in edges if edge["to"] == node["id"]]
        if predecessors:
            predecessor = max(
                predecessors, key=lambda item: (longest[item], path_nodes[item], item)
            )
            longest[node["id"]] = node["reference_duration_ticks"] + longest[predecessor]
            path_nodes[node["id"]] = path_nodes[predecessor] + 1
    critical_duration = max(longest.values(), default=0)
    critical_end = max(
        longest, key=lambda item: (longest[item], path_nodes[item], item), default=""
    )
    critical_nodes = path_nodes.get(critical_end, 0)
    resources = sorted({resource for node in nodes for resource in node["resource_claims"]})
    dimensions = {
        "node_count": node_count,
        "edge_count": len(edges),
        "density_numerator": len(edges),
        "density_denominator": node_count * (node_count - 1) // 2 if node_count > 1 else 1,
        "density_ppm": (len(edges) * 1_000_000 // (node_count * (node_count - 1) // 2))
        if node_count > 1
        else 0,
        "dag_width": width,
        "dag_depth": depth,
        "critical_path_nodes": critical_nodes,
        "critical_path_duration_ticks": critical_duration,
        "total_duration_ticks": total_duration,
        "critical_path_ratio_ppm": critical_duration * 1_000_000 // total_duration,
        "max_fan_in": max(indegree.values(), default=0),
        "max_fan_out": max(outdegree.values(), default=0),
        "semantic_overlap_numerator": semantic_numerator,
        "semantic_overlap_denominator": semantic_denominator,
        "semantic_claim_overlap_ppm": semantic_numerator * 1_000_000 // semantic_denominator
        if semantic_denominator
        else 0,
        "resource_contention_numerator": resource_numerator,
        "resource_contention_denominator": resource_denominator,
        "resource_contention_ppm": resource_numerator * 1_000_000 // resource_denominator
        if resource_denominator
        else 0,
        "worker_count": len(workers),
        "capability_count": len(capabilities),
        "event_count": len(events),
        "evidence_count": len(evidence),
        "min_worker_cost_units_per_second": min(
            worker["cost_units_per_second"] for worker in workers
        ),
        "max_worker_cost_units_per_second": max(
            worker["cost_units_per_second"] for worker in workers
        ),
        "min_worker_duration_scale_ppm": min(worker["duration_scale_ppm"] for worker in workers),
        "max_worker_duration_scale_ppm": max(worker["duration_scale_ppm"] for worker in workers),
        "fan_in_histogram": _histogram(indegree.values()),
        "fan_out_histogram": _histogram(outdegree.values()),
    }
    parameters.update(
        {
            "node_count": node_count,
            "dag_width": width,
            "dag_depth": depth,
            "semantic_overlap_numerator": semantic_numerator,
            "resource_contention_numerator": resource_numerator,
            "event_count": len(events),
            "evidence_count": len(evidence),
            "capability_count": len(capabilities),
        }
    )
    payload: dict[str, Any] = {
        "schema_version": WORKLOAD_SCHEMA_VERSION,
        "workload_id": "",
        "distribution_id": distribution_id,
        "seed": seed,
        "generator": {
            "name": GENERATOR_NAME,
            "version": GENERATOR_VERSION,
            "parameters": parameters,
        },
        "origin": origin,
        "dimensions": dimensions,
        "nodes": nodes,
        "edges": edges,
        "resources": [{"id": resource, "capacity_units": 1} for resource in resources],
        "workers": workers,
        "events": events,
        "evidence": evidence,
    }
    payload["workload_id"] = "sha256:" + workload_digest(payload)
    return payload


def generate_synthetic(
    distribution_id: str, seed: int, *, node_count: int | None = None
) -> dict[str, Any]:
    """Generate one deterministic synthetic workload inside a named envelope."""
    if not isinstance(distribution_id, str) or distribution_id not in _DISTRIBUTIONS:
        raise WorkloadError(f"unknown distribution_id: {distribution_id!r}")
    seed = _uint(seed, "seed")
    n, width, depth = _choose_shape(distribution_id, seed, node_count)
    rng = _Rng(seed ^ 0xD6E8FEB86659FD93)
    sizes = _layers(n, width, depth, rng)
    nodes: list[dict[str, Any]] = []
    node_ids: list[list[str]] = []
    counter = 0
    for rank, size in enumerate(sizes):
        level_ids: list[str] = []
        for _ in range(size):
            node_id = f"node-{counter:06d}"
            level_ids.append(node_id)
            capability = f"capability:{rng.pick(0, 3)}"
            nodes.append(
                {
                    "id": node_id,
                    "rank": rank,
                    "reference_duration_ticks": 1,
                    "cost_units": rng.pick(1, 100),
                    "required_capabilities": [capability],
                    "semantic_claims": [],
                    "resource_claims": [],
                    "event_count": 1,
                    "evidence_count": 1,
                }
            )
            counter += 1
        node_ids.append(level_ids)
    spec = _DISTRIBUTIONS[distribution_id]
    incomparable_pairs = [
        (left, right)
        for layer in node_ids
        for left_index, left in enumerate(layer)
        for right in layer[left_index + 1 :]
    ]
    rng.shuffle(incomparable_pairs)
    semantic_count = _pair_count(spec["semantic_overlap"], len(incomparable_pairs))
    resource_count = _pair_count(spec["resource_contention"], len(incomparable_pairs))
    for index, (left, right) in enumerate(incomparable_pairs[:semantic_count]):
        claim = f"claim:shared:{index:08d}"
        nodes[int(left[5:])]["semantic_claims"].append(claim)
        nodes[int(right[5:])]["semantic_claims"].append(claim)
    for node in nodes:
        if not node["semantic_claims"]:
            node["semantic_claims"] = [f"claim:{node['id']}"]
        node["semantic_claims"].sort()
    for index, (left, right) in enumerate(incomparable_pairs[:resource_count]):
        resource = f"resource:shared:{index:08d}"
        nodes[int(left[5:])]["resource_claims"].append(resource)
        nodes[int(right[5:])]["resource_claims"].append(resource)
    for node in nodes:
        if not node["resource_claims"]:
            node["resource_claims"] = [f"resource:{node['id']}"]
        node["resource_claims"].sort()
    edges = [
        {"from": start, "to": end}
        for previous, current in zip(node_ids, node_ids[1:], strict=False)
        for start in previous
        for end in current
    ]
    parameters = {
        "distribution_id": distribution_id,
        "requested_node_count": node_count,
        "layer_width": width,
        "depth": depth,
        "worker_count": 4,
        "semantic_overlap_range_ppm": list(spec["semantic_overlap"]),
        "resource_contention_range_ppm": list(spec["resource_contention"]),
        "edge_rule": "complete-adjacent-layer",
    }
    payload = _assemble(distribution_id, seed, nodes, edges, {"kind": "synthetic"}, parameters, rng)
    validate_workload(payload)
    return payload


def generate_repository_derived(
    source: Mapping[str, Any], distribution_id: str, seed: int
) -> dict[str, Any]:
    """Turn a closed source description into a workload without reading the repository."""
    if not isinstance(distribution_id, str) or distribution_id not in _DISTRIBUTIONS:
        raise WorkloadError(f"unknown distribution_id: {distribution_id!r}")
    seed = _uint(seed, "seed")
    normalized = _canonical_source(source)
    nodes = normalized["nodes"]
    edges = normalized["edges"]
    ranks = _ranks(nodes, edges)
    output_nodes = [
        {
            "id": node["id"],
            "rank": ranks[node["id"]],
            "reference_duration_ticks": node["reference_duration_ticks"],
            "cost_units": node["cost_units"],
            "required_capabilities": node["required_capabilities"],
            "semantic_claims": node["semantic_claims"],
            "resource_claims": node["resource_claims"],
            "event_count": node["event_count"],
            "evidence_count": node["evidence_count"],
        }
        for node in nodes
    ]
    source_digest = getattr(source, "file_digest", None)
    if source_digest is None:
        source_digest = "sha256:" + hashlib.sha256(canonical_json(source)).hexdigest()
    parameters = {
        "distribution_id": distribution_id,
        "source_node_count": len(nodes),
        "source_edge_count": len(edges),
        "worker_count": 4,
        "edge_rule": "source-edges",
        "source_schema_version": SOURCE_SCHEMA_VERSION,
    }
    rng = _Rng(seed ^ 0xA5A5A5A5A5A5A5A5)
    payload = _assemble(
        distribution_id,
        seed,
        output_nodes,
        edges,
        {
            "kind": "repository-derived",
            "repository": normalized["repository"],
            "revision": normalized["revision"],
            "source_sha256": source_digest,
        },
        parameters,
        rng,
    )
    spec = _DISTRIBUTIONS[distribution_id]
    dimensions = payload["dimensions"]
    if not spec["nodes"][0] <= dimensions["node_count"] <= spec["nodes"][1]:
        raise WorkloadError(
            f"derived node_count={dimensions['node_count']} is outside {distribution_id} envelope"
        )
    if not spec["width"][0] <= dimensions["dag_width"] <= spec["width"][1]:
        raise WorkloadError(
            f"derived dag_width={dimensions['dag_width']} is outside {distribution_id} envelope"
        )
    if not _ratio_in(
        dimensions["critical_path_duration_ticks"],
        dimensions["total_duration_ticks"],
        spec["critical_path"],
    ):
        raise WorkloadError(f"derived critical path ratio is outside {distribution_id} envelope")
    if not _ratio_in(
        dimensions["semantic_overlap_numerator"],
        dimensions["semantic_overlap_denominator"],
        spec["semantic_overlap"],
    ):
        raise WorkloadError(f"derived semantic overlap is outside {distribution_id} envelope")
    validate_workload(payload)
    return payload


def _excluded_repository_path(relative: Path) -> bool:
    parts = set(relative.parts)
    if parts & {
        ".git",
        ".jj",
        ".hg",
        ".svn",
        "target",
        ".venv",
        "__pycache__",
        ".ruff_cache",
        ".mypy_cache",
        ".pytest_cache",
        ".cache",
        "cache",
        "artifacts",
        "generated",
        "dist",
        "build",
        "output",
        "outputs",
    }:
        return True
    if relative.parts[:3] == ("experiments", "workloads", "golden"):
        return True
    return (
        relative.suffix in {".pyc", ".pyo"}
        or relative.name.startswith("workload-output")
        or relative.name in {"workload.json", "workload-output.json"}
    )


def _source_files_digest(root: Path) -> str:
    files: list[tuple[bytes, bytes]] = []
    for path in sorted(root.rglob("*"), key=lambda item: item.relative_to(root).as_posix()):
        relative = path.relative_to(root)
        if _excluded_repository_path(relative):
            continue
        if path.is_symlink():
            raise WorkloadError(f"repository contains unsupported symlink: {relative}")
        if not path.is_file():
            if path.exists():
                raise WorkloadError(f"repository contains unsupported special path: {relative}")
            raise WorkloadError(f"repository path disappeared during extraction: {relative}")
        try:
            files.append((relative.as_posix().encode("utf-8"), path.read_bytes()))
        except OSError as exc:
            raise WorkloadError(f"cannot read repository file {relative}: {exc}") from exc
    digest = hashlib.sha256(b"gordian-source-digest-v1\0")
    digest.update(len(files).to_bytes(8, "big"))
    for relative, content in files:
        digest.update(len(relative).to_bytes(8, "big"))
        digest.update(relative)
        digest.update(len(content).to_bytes(8, "big"))
        digest.update(content)
    return "sha256:" + digest.hexdigest()


def extract_repository_source(
    root: Path, *, revision: str, manifest_relative_path: str = "gordian-workload-source.json"
) -> dict[str, Any]:
    """Extract a normalized source manifest and an identity digest for regular repository files."""
    if not isinstance(root, Path):
        raise WorkloadError("root must be a pathlib.Path")
    root = root.resolve()
    if not root.is_dir():
        raise WorkloadError(f"repository root is not a directory: {root}")
    relative_manifest = Path(manifest_relative_path)
    if relative_manifest.is_absolute() or ".." in relative_manifest.parts:
        raise WorkloadError("manifest_relative_path must stay inside repository root")
    manifest_path = root / relative_manifest
    try:
        manifest = _read_json(str(manifest_path))
    except WorkloadError:
        raise
    manifest_map = _require_mapping(manifest, "repository manifest")
    _keys(
        manifest_map,
        {"schema_version", "repository", "revision", "nodes", "edges"},
        "repository manifest",
    )
    if manifest_map["schema_version"] != SOURCE_SCHEMA_VERSION:
        raise WorkloadError(f"repository manifest schema_version must be {SOURCE_SCHEMA_VERSION}")
    if _text(manifest_map["revision"], "manifest.revision") != _text(revision, "revision"):
        raise WorkloadError("repository manifest revision does not match requested revision")
    source = _SourceMapping(
        {
            "schema_version": SOURCE_SCHEMA_VERSION,
            "repository": _text(manifest_map["repository"], "manifest.repository"),
            "revision": manifest_map["revision"],
            "nodes": manifest_map["nodes"],
            "edges": manifest_map["edges"],
        }
    )
    # Validate before hashing so malformed graph input cannot acquire an identity.
    normalized = _canonical_source(source)
    return _SourceMapping(normalized, _source_files_digest(root))


def generate_repository(
    root: Path,
    *,
    revision: str,
    seed: int,
    manifest_relative_path: str = "gordian-workload-source.json",
) -> dict[str, Any]:
    """Extract a repository workload and infer the sole matching named distribution."""
    source = extract_repository_source(
        root, revision=revision, manifest_relative_path=manifest_relative_path
    )
    candidates: list[tuple[str, dict[str, Any]]] = []
    for distribution_id in _DISTRIBUTIONS:
        try:
            candidates.append(
                (distribution_id, generate_repository_derived(source, distribution_id, seed))
            )
        except WorkloadError:
            continue
    if len(candidates) != 1:
        labels = ", ".join(item[0] for item in candidates) or "none"
        raise WorkloadError(
            f"repository shape must match exactly one distribution (matched {labels})"
        )
    return candidates[0][1]


def _manifest_root(manifest_path: Path) -> Path:
    resolved = manifest_path.resolve()
    for candidate in (resolved.parent, *resolved.parents):
        if (candidate / "orchestration").is_dir() and (candidate / "experiments").is_dir():
            return candidate
    return resolved.parent


def _json_without_duplicate_keys(raw: bytes, label: str) -> Any:
    def pairs(items: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, item in items:
            if key in result:
                raise WorkloadError(f"{label} contains duplicate JSON key: {key}")
            result[key] = item
        return result

    try:
        return json.loads(raw.decode("utf-8"), object_pairs_hook=pairs)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise WorkloadError(f"cannot parse {label}: {exc}") from exc


def _relative_manifest_path(value: Any, label: str) -> str:
    path = _text(value, label)
    parsed = Path(path)
    if parsed.is_absolute() or ".." in parsed.parts or "." in parsed.parts or "\\" in path:
        raise WorkloadError(f"{label} must be repository-relative POSIX path")
    return path


def validate_golden_manifest(path: Path, *, repository_root: Path | None = None) -> None:
    """Read, validate, regenerate, and byte-check a golden workload manifest without writing."""
    if not isinstance(path, Path):
        raise WorkloadError("manifest path must be a pathlib.Path")
    manifest_path = path.resolve()
    try:
        raw_manifest = manifest_path.read_bytes()
    except OSError as exc:
        raise WorkloadError(f"cannot read golden manifest: {exc}") from exc
    manifest = _json_without_duplicate_keys(raw_manifest, "golden manifest")
    manifest_map = _require_mapping(manifest, "golden manifest")
    _keys(manifest_map, {"schema_version", "entries"}, "golden manifest")
    if manifest_map["schema_version"] != "gordian.workload-golden-manifest.v1":
        raise WorkloadError("golden manifest has an invalid schema_version")
    entries = _list(manifest_map["entries"], "golden manifest.entries")
    if len(entries) != 5:
        raise WorkloadError("golden manifest.entries must contain exactly five entries")
    root = _manifest_root(manifest_path) if repository_root is None else repository_root.resolve()
    if not root.is_dir():
        raise WorkloadError(f"repository root is not a directory: {root}")
    golden_dir = manifest_path.parent.resolve()
    previous_path = ""
    seen_paths: set[str] = set()
    synthetic_distributions: set[str] = set()
    repository_distributions: set[str] = set()
    for index, raw_entry in enumerate(entries):
        entry = _require_mapping(raw_entry, f"golden manifest.entries[{index}]")
        common = {"path", "sha256", "kind", "seed", "distribution_id"}
        kind = entry.get("kind")
        if kind == "synthetic":
            _keys(entry, common | {"node_count"}, f"golden manifest.entries[{index}]")
        elif kind == "repository-derived":
            _keys(
                entry,
                common | {"repository_path", "revision", "manifest_relative_path"},
                f"golden manifest.entries[{index}]",
            )
        else:
            raise WorkloadError(f"golden manifest.entries[{index}].kind is invalid")
        workload_path = _relative_manifest_path(
            entry["path"], f"golden manifest.entries[{index}].path"
        )
        if workload_path <= previous_path or workload_path in seen_paths:
            raise WorkloadError("golden manifest entries must have unique sorted paths")
        previous_path = workload_path
        seen_paths.add(workload_path)
        digest = _text(entry["sha256"], f"golden manifest.entries[{index}].sha256")
        if len(digest) != 64 or any(character not in "0123456789abcdef" for character in digest):
            raise WorkloadError(
                "golden manifest sha256 must be 64 lowercase hexadecimal characters"
            )
        seed = _uint(entry["seed"], f"golden manifest.entries[{index}].seed")
        distribution_id = _text(
            entry["distribution_id"], f"golden manifest.entries[{index}].distribution_id"
        )
        if distribution_id not in _DISTRIBUTIONS:
            raise WorkloadError(f"unknown golden distribution_id: {distribution_id}")
        if kind == "synthetic":
            if distribution_id in synthetic_distributions:
                raise WorkloadError(
                    f"duplicate synthetic golden distribution_id: {distribution_id}"
                )
            synthetic_distributions.add(distribution_id)
        else:
            if distribution_id != "rep-small":
                raise WorkloadError("repository-derived golden distribution_id must be rep-small")
            if distribution_id in repository_distributions:
                raise WorkloadError(
                    f"duplicate repository-derived golden distribution_id: {distribution_id}"
                )
            repository_distributions.add(distribution_id)
        full_path = (root / workload_path).resolve()
        try:
            full_path.relative_to(golden_dir)
            artifact = full_path.read_bytes()
        except (OSError, ValueError) as exc:
            raise WorkloadError(
                "golden workload path must be contained in manifest golden directory: "
                f"{workload_path}"
            ) from exc
        if hashlib.sha256(artifact).hexdigest() != digest:
            raise WorkloadError(f"golden workload digest mismatch: {workload_path}")
        workload = _json_without_duplicate_keys(artifact, workload_path)
        validate_workload(workload)
        if workload["distribution_id"] != distribution_id or workload["seed"] != seed:
            raise WorkloadError(f"golden metadata mismatch: {workload_path}")
        if canonical_json(workload) != artifact:
            raise WorkloadError(f"golden workload is not canonical: {workload_path}")
        if kind == "synthetic":
            node_count = entry["node_count"]
            if node_count is not None and (not _is_int(node_count) or node_count < 0):
                raise WorkloadError("synthetic node_count must be an integer or null")
            regenerated = generate_synthetic(distribution_id, seed, node_count=node_count)
        else:
            repository_path = _relative_manifest_path(
                entry["repository_path"], f"golden manifest.entries[{index}].repository_path"
            )
            manifest_relative_path = _relative_manifest_path(
                entry["manifest_relative_path"],
                f"golden manifest.entries[{index}].manifest_relative_path",
            )
            repository_root_path = (root / repository_path).resolve()
            try:
                repository_root_path.relative_to(root)
            except ValueError as exc:
                raise WorkloadError(
                    f"golden manifest repository_path escapes repository root: {repository_path}"
                ) from exc
            regenerated = generate_repository(
                repository_root_path,
                revision=_text(entry["revision"], f"golden manifest.entries[{index}].revision"),
                seed=seed,
                manifest_relative_path=manifest_relative_path,
            )
        if canonical_json(regenerated) != artifact:
            raise WorkloadError(f"golden regeneration mismatch: {workload_path}")
    if synthetic_distributions != set(_DISTRIBUTIONS):
        raise WorkloadError(
            "golden manifest must contain one synthetic entry for each distribution"
        )
    if repository_distributions != {"rep-small"}:
        raise WorkloadError("golden manifest must contain one repository-derived rep-small entry")


def _validate_records(payload: Mapping[str, Any], nodes: Mapping[str, Mapping[str, Any]]) -> None:
    events = _list(payload["events"], "events")
    evidence = _list(payload["evidence"], "evidence")
    previous = -1
    for index, raw in enumerate(events):
        item = _require_mapping(raw, f"events[{index}]")
        _keys(item, _EVENT_KEYS, f"events[{index}]")
        _text(item["id"], f"events[{index}].id")
        sequence = _uint(item["sequence"], f"events[{index}].sequence")
        if sequence != index or sequence <= previous or item["node_id"] not in nodes:
            raise WorkloadError("events must have contiguous ordered sequences and valid node ids")
        _text(item["kind"], f"events[{index}].kind")
        previous = sequence
    previous = -1
    for index, raw in enumerate(evidence):
        item = _require_mapping(raw, f"evidence[{index}]")
        _keys(item, _EVIDENCE_KEYS, f"evidence[{index}]")
        _text(item["id"], f"evidence[{index}].id")
        sequence = _uint(item["sequence"], f"evidence[{index}].sequence")
        if sequence != index or sequence <= previous or item["node_id"] not in nodes:
            raise WorkloadError(
                "evidence must have contiguous ordered sequences and valid node ids"
            )
        _text(item["kind"], f"evidence[{index}].kind")
        subject = _text(item["subject"], f"evidence[{index}].subject")
        if subject not in nodes:
            raise WorkloadError(f"evidence[{index}].subject references an unknown node")
        previous = sequence


def validate_workload(payload: Mapping[str, Any]) -> None:
    """Validate every structural, topological, identity, and dimensional workload invariant."""
    value = _require_mapping(payload, "payload")
    _keys(value, _TOP_KEYS, "payload")
    if value["schema_version"] != WORKLOAD_SCHEMA_VERSION:
        raise WorkloadError(f"schema_version must be {WORKLOAD_SCHEMA_VERSION}")
    distribution_id = _text(value["distribution_id"], "distribution_id")
    if distribution_id not in _DISTRIBUTIONS:
        raise WorkloadError(f"unknown distribution_id: {distribution_id}")
    _uint(value["seed"], "seed")
    generator = _require_mapping(value["generator"], "generator")
    _keys(generator, {"name", "version", "parameters"}, "generator")
    if generator["name"] != GENERATOR_NAME or generator["version"] != GENERATOR_VERSION:
        raise WorkloadError("generator identity does not match this implementation")
    parameters = _require_mapping(generator["parameters"], "generator.parameters")
    if any(not isinstance(key, str) for key in parameters):
        raise WorkloadError("generator.parameters keys must be strings")
    origin = _require_mapping(value["origin"], "origin")
    kind = origin.get("kind")
    if kind == "synthetic":
        _keys(origin, {"kind"}, "origin")
    elif kind == "repository-derived":
        _keys(origin, {"kind", "repository", "revision", "source_sha256"}, "origin")
        _text(origin["repository"], "origin.repository")
        _text(origin["revision"], "origin.revision")
        digest = _text(origin["source_sha256"], "origin.source_sha256")
        if not digest.startswith("sha256:") or len(digest) != 71:
            raise WorkloadError("origin.source_sha256 must be sha256:<64 hex digits>")
        try:
            int(digest[7:], 16)
        except ValueError as exc:
            raise WorkloadError("origin.source_sha256 is not hexadecimal") from exc
    else:
        raise WorkloadError("origin.kind must be synthetic or repository-derived")
    dimensions = _require_mapping(value["dimensions"], "dimensions")
    _keys(dimensions, _DIMENSION_KEYS, "dimensions")
    for key, item in dimensions.items():
        if key in {"fan_in_histogram", "fan_out_histogram"}:
            if not isinstance(item, Mapping):
                raise WorkloadError(f"dimensions.{key} must be a mapping")
            for histogram_key, histogram_value in item.items():
                if (
                    not isinstance(histogram_key, str)
                    or not histogram_key
                    or not all("0" <= character <= "9" for character in histogram_key)
                ):
                    raise WorkloadError(f"dimensions.{key} has invalid bucket")
                _uint(histogram_value, f"dimensions.{key}[{histogram_key}]")
        else:
            _uint(item, f"dimensions.{key}", 10_000_000_000)
    raw_nodes = _list(value["nodes"], "nodes")
    raw_edges = _list(value["edges"], "edges")
    raw_workers = _list(value["workers"], "workers")
    nodes: dict[str, Mapping[str, Any]] = {}
    ranks: dict[str, int] = {}
    for index, raw in enumerate(raw_nodes):
        node = _require_mapping(raw, f"nodes[{index}]")
        _keys(node, _NODE_KEYS, f"nodes[{index}]")
        node_id = _text(node["id"], f"nodes[{index}].id")
        if node_id in nodes:
            raise WorkloadError(f"duplicate node id: {node_id}")
        nodes[node_id] = node
        ranks[node_id] = _uint(node["rank"], f"nodes[{index}].rank", 10_000_000_000)
        duration = _uint(
            node["reference_duration_ticks"],
            f"nodes[{index}].reference_duration_ticks",
            10_000_000_000,
        )
        if duration == 0:
            raise WorkloadError("reference_duration_ticks must be positive")
        _uint(node["cost_units"], f"nodes[{index}].cost_units", 10_000_000_000)
        _uint(node["fan_in"], f"nodes[{index}].fan_in", 10_000_000_000)
        _uint(node["fan_out"], f"nodes[{index}].fan_out", 10_000_000_000)
        _sorted_unique_text(node["required_capabilities"], f"nodes[{index}].required_capabilities")
        _sorted_unique_text(node["semantic_claims"], f"nodes[{index}].semantic_claims")
        _sorted_unique_text(node["resource_claims"], f"nodes[{index}].resource_claims")
    if list(nodes) != sorted(nodes):
        raise WorkloadError("nodes must be sorted by id")
    edge_pairs: list[tuple[str, str]] = []
    for index, raw in enumerate(raw_edges):
        edge = _require_mapping(raw, f"edges[{index}]")
        _keys(edge, _EDGE_KEYS, f"edges[{index}]")
        start = _text(edge["from"], f"edges[{index}].from")
        end = _text(edge["to"], f"edges[{index}].to")
        if start not in nodes or end not in nodes:
            raise WorkloadError("edge references an unknown node")
        if start == end:
            raise WorkloadError("edge self-loop")
        edge_pairs.append((start, end))
    if len(set(edge_pairs)) != len(edge_pairs) or edge_pairs != sorted(edge_pairs):
        raise WorkloadError("edges must be sorted and duplicate-free")
    edge_maps = [{"from": start, "to": end} for start, end in edge_pairs]
    calculated_ranks = _ranks(list(nodes.values()), edge_maps)
    if ranks != calculated_ranks:
        raise WorkloadError("node ranks do not match longest-path ranks")
    workers: dict[str, Mapping[str, Any]] = {}
    for index, raw in enumerate(raw_workers):
        worker = _require_mapping(raw, f"workers[{index}]")
        _keys(worker, _WORKER_KEYS, f"workers[{index}]")
        worker_id = _text(worker["id"], f"workers[{index}].id")
        if worker_id in workers:
            raise WorkloadError(f"duplicate worker id: {worker_id}")
        workers[worker_id] = worker
        _sorted_unique_text(worker["capabilities"], f"workers[{index}].capabilities")
        _uint(
            worker["cost_units_per_second"],
            f"workers[{index}].cost_units_per_second",
            10_000_000_000,
        )
        scale = _uint(
            worker["duration_scale_ppm"], f"workers[{index}].duration_scale_ppm", 1_000_000
        )
        if scale == 0:
            raise WorkloadError("worker duration_scale_ppm must be positive")
    if list(workers) != sorted(workers):
        raise WorkloadError("workers must be sorted by id")
    if not workers:
        raise WorkloadError("workload must have a worker")
    resources: dict[str, Mapping[str, Any]] = {}
    for index, raw in enumerate(_list(value["resources"], "resources")):
        resource = _require_mapping(raw, f"resources[{index}]")
        _keys(resource, _RESOURCE_KEYS, f"resources[{index}]")
        resource_id = _text(resource["id"], f"resources[{index}].id")
        if resource_id in resources:
            raise WorkloadError(f"duplicate resource id: {resource_id}")
        if _uint(resource["capacity_units"], f"resources[{index}].capacity_units") != 1:
            raise WorkloadError("workload resources must have capacity_units=1")
        resources[resource_id] = resource
    if list(resources) != sorted(resources):
        raise WorkloadError("resources must be sorted by id")
    if any(
        resource not in resources for node in nodes.values() for resource in node["resource_claims"]
    ):
        raise WorkloadError("node references an undeclared resource")
    worker_caps = [set(worker["capabilities"]) for worker in workers.values()]
    for node in nodes.values():
        required = set(node["required_capabilities"])
        if not any(required <= caps for caps in worker_caps):
            raise WorkloadError(f"no compatible worker for node {node['id']}")
    _validate_records(value, nodes)
    node_count = len(nodes)
    depth = max(ranks.values(), default=-1) + 1
    indegree = {node_id: 0 for node_id in nodes}
    outdegree = {node_id: 0 for node_id in nodes}
    for start, end in edge_pairs:
        indegree[end] += 1
        outdegree[start] += 1
    if any(
        node["fan_in"] != indegree[node_id] or node["fan_out"] != outdegree[node_id]
        for node_id, node in nodes.items()
    ):
        raise WorkloadError("per-node fan-in/fan-out does not match edges")
    semantic_numerator, semantic_denominator = _pair_metrics(
        list(nodes.values()), edge_maps, "semantic_claims"
    )
    resource_numerator, resource_denominator = _pair_metrics(
        list(nodes.values()), edge_maps, "resource_claims"
    )
    total_duration = sum(node["reference_duration_ticks"] for node in nodes.values())
    order = sorted(nodes.values(), key=lambda item: (item["rank"], item["id"]))
    longest = {node["id"]: node["reference_duration_ticks"] for node in order}
    path_nodes = {node["id"]: 1 for node in order}
    for node in order:
        predecessors = [edge["from"] for edge in edge_maps if edge["to"] == node["id"]]
        if predecessors:
            predecessor = max(
                predecessors, key=lambda item: (longest[item], path_nodes[item], item)
            )
            longest[node["id"]] = node["reference_duration_ticks"] + longest[predecessor]
            path_nodes[node["id"]] = path_nodes[predecessor] + 1
    critical_duration = max(longest.values(), default=0)
    critical_end = max(
        longest, key=lambda item: (longest[item], path_nodes[item], item), default=""
    )
    critical_nodes = path_nodes.get(critical_end, 0)
    fan_in_histogram = _histogram(indegree.values())
    fan_out_histogram = _histogram(outdegree.values())
    expected_dimensions = {
        "node_count": node_count,
        "edge_count": len(edge_pairs),
        "density_numerator": len(edge_pairs),
        "density_denominator": node_count * (node_count - 1) // 2 if node_count > 1 else 1,
        "density_ppm": len(edge_pairs) * 1_000_000 // (node_count * (node_count - 1) // 2)
        if node_count > 1
        else 0,
        "dag_width": _maximum_antichain(list(nodes.values()), edge_maps),
        "dag_depth": depth,
        "critical_path_nodes": critical_nodes,
        "critical_path_duration_ticks": critical_duration,
        "total_duration_ticks": total_duration,
        "critical_path_ratio_ppm": critical_duration * 1_000_000 // total_duration,
        "max_fan_in": max(indegree.values(), default=0),
        "max_fan_out": max(outdegree.values(), default=0),
        "semantic_overlap_numerator": semantic_numerator,
        "semantic_overlap_denominator": semantic_denominator,
        "semantic_claim_overlap_ppm": semantic_numerator * 1_000_000 // semantic_denominator
        if semantic_denominator
        else 0,
        "resource_contention_numerator": resource_numerator,
        "resource_contention_denominator": resource_denominator,
        "resource_contention_ppm": resource_numerator * 1_000_000 // resource_denominator
        if resource_denominator
        else 0,
        "worker_count": len(workers),
        "capability_count": len(
            {cap for node in nodes.values() for cap in node["required_capabilities"]}
        ),
        "event_count": len(value["events"]),
        "evidence_count": len(value["evidence"]),
        "min_worker_cost_units_per_second": min(
            worker["cost_units_per_second"] for worker in workers.values()
        ),
        "max_worker_cost_units_per_second": max(
            worker["cost_units_per_second"] for worker in workers.values()
        ),
        "min_worker_duration_scale_ppm": min(
            worker["duration_scale_ppm"] for worker in workers.values()
        ),
        "max_worker_duration_scale_ppm": max(
            worker["duration_scale_ppm"] for worker in workers.values()
        ),
        "fan_in_histogram": fan_in_histogram,
        "fan_out_histogram": fan_out_histogram,
    }
    if dict(dimensions) != expected_dimensions:
        raise WorkloadError("dimensions do not match workload contents")
    spec = _DISTRIBUTIONS[distribution_id]
    for name, bounds in (
        ("node_count", spec["nodes"]),
        ("dag_width", spec["width"]),
        ("critical_path_ratio_ppm", spec["critical_path"]),
        ("semantic_claim_overlap_ppm", spec["semantic_overlap"]),
    ):
        if name == "node_count" or name == "dag_width":
            matches = bounds[0] <= dimensions[name] <= bounds[1]
        elif name == "critical_path_ratio_ppm":
            matches = _ratio_in(
                dimensions["critical_path_duration_ticks"],
                dimensions["total_duration_ticks"],
                bounds,
            )
        else:
            matches = _ratio_in(
                dimensions["semantic_overlap_numerator"],
                dimensions["semantic_overlap_denominator"],
                bounds,
            )
        if not matches:
            raise WorkloadError(f"{name}={dimensions[name]} is outside {distribution_id} envelope")
    workload_id = _text(value["workload_id"], "workload_id")
    if workload_id != "sha256:" + workload_digest(value) or len(workload_id) != 71:
        raise WorkloadError("workload_id does not match canonical workload contents")


def _read_json(path: str, *, require_canonical: bool = False) -> Any:
    """Read strict JSON, optionally requiring exact canonical workload bytes."""
    try:
        raw = sys.stdin.buffer.read() if path == "-" else Path(path).read_bytes()
        payload = _json_without_duplicate_keys(raw, path)
        if require_canonical and canonical_json(payload) != raw:
            raise WorkloadError(f"workload JSON is not canonical: {path}")
        return payload
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise WorkloadError(f"cannot read JSON {path}: {exc}") from exc


def _write_json(payload: Mapping[str, Any], path: str) -> None:
    encoded = canonical_json(payload)
    try:
        if path == "-":
            sys.stdout.buffer.write(encoded)
        else:
            Path(path).write_bytes(encoded)
    except OSError as exc:
        raise WorkloadError(f"cannot write JSON {path}: {exc}") from exc


def main(argv: Sequence[str] | None = None) -> int:
    """CLI entry point for synthetic generation, source derivation, and validation."""
    parser = argparse.ArgumentParser(prog="gordian-workload")
    subparsers = parser.add_subparsers(dest="command", required=True)
    synthetic = subparsers.add_parser("synthetic")
    synthetic.add_argument("--distribution", required=True)
    synthetic.add_argument("--seed", required=True, type=int)
    synthetic.add_argument("--node-count", type=int)
    synthetic.add_argument("--output", default="-")
    derive = subparsers.add_parser("derive")
    derive.add_argument("--source")
    derive.add_argument("--repository")
    derive.add_argument("--revision")
    derive.add_argument("--manifest-relative-path", default="gordian-workload-source.json")
    derive.add_argument("--distribution", required=True)
    derive.add_argument("--seed", required=True, type=int)
    derive.add_argument("--output", default="-")
    validate = subparsers.add_parser("validate")
    validate.add_argument("--manifest", type=Path)
    validate.add_argument("paths", nargs="*")
    try:
        args = parser.parse_args(argv)
        if args.command == "synthetic":
            _write_json(
                generate_synthetic(args.distribution, args.seed, node_count=args.node_count),
                args.output,
            )
        elif args.command == "derive":
            if bool(args.source) == bool(args.repository):
                parser.error("derive requires exactly one of --source or --repository")
            if args.repository:
                if not args.revision:
                    parser.error("--revision is required with --repository")
                result = generate_repository(
                    Path(args.repository),
                    revision=args.revision,
                    seed=args.seed,
                    manifest_relative_path=args.manifest_relative_path,
                )
            else:
                if args.revision:
                    parser.error("--revision is only valid with --repository")
                result = generate_repository_derived(
                    _read_json(args.source), args.distribution, args.seed
                )
            _write_json(result, args.output)
        else:
            if (args.manifest is None) == (not args.paths):
                parser.error("validate requires exactly one of --manifest or workload paths")
            if args.manifest is not None:
                validate_golden_manifest(args.manifest)
                sys.stdout.write(f"{args.manifest}: valid\n")
            else:
                for path in args.paths:
                    validate_workload(_read_json(path, require_canonical=True))
                    if path != "-":
                        sys.stdout.write(f"{path}: valid\n")
        return 0
    except WorkloadError as exc:
        parser.error(str(exc))
    except (BrokenPipeError, OSError) as exc:
        parser.error(str(exc))
    return 2


__all__ = [
    "GENERATOR_NAME",
    "GENERATOR_VERSION",
    "SEED_MATRIX",
    "SOURCE_SCHEMA_VERSION",
    "WORKLOAD_SCHEMA_VERSION",
    "WorkloadError",
    "canonical_json",
    "extract_repository_source",
    "generate_repository",
    "generate_repository_derived",
    "generate_synthetic",
    "main",
    "validate_golden_manifest",
    "validate_workload",
    "workload_digest",
]


if __name__ == "__main__":
    raise SystemExit(main())
