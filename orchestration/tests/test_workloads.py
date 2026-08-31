"""Acceptance tests for the deterministic workload public API.

The graph metrics in this file intentionally have no dependency on implementation helpers.  The
tests are a small, executable oracle for the workload contract and therefore should remain useful
when the generator's internal algorithm changes.
"""

from __future__ import annotations

import copy
import functools
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from gordian_orchestration.workloads import (
    SEED_MATRIX,
    SOURCE_SCHEMA_VERSION,
    WORKLOAD_SCHEMA_VERSION,
    WorkloadError,
    canonical_json,
    extract_repository_source,
    generate_repository,
    generate_repository_derived,
    generate_synthetic,
    validate_golden_manifest,
    validate_workload,
    workload_digest,
)

DISTRIBUTIONS = ("rep-small", "rep-wide", "adv-deep", "adv-contended")
U64_MAX = 2**64 - 1


def _schema_validate(value: object, schema: dict, root: dict, path: str = "$") -> None:
    """Validate the JSON-Schema subset used by the checked-in workload schema.

    The project intentionally has no runtime JSON-Schema dependency.  This deterministic test
    oracle covers the schema keywords present in the workload contract, including closed objects,
    refs, oneOf, arrays, numeric bounds, and string patterns.
    """
    if "$ref" in schema:
        reference = schema["$ref"]
        if not reference.startswith("#/$defs/"):
            raise AssertionError(f"unsupported schema reference at {path}: {reference}")
        schema = root["$defs"][reference.rsplit("/", 1)[-1]]
    if "const" in schema and value != schema["const"]:
        raise AssertionError(f"{path} is not {schema['const']!r}")
    if "enum" in schema and value not in schema["enum"]:
        raise AssertionError(f"{path} is outside enum")
    if "oneOf" in schema:
        matches = 0
        for option in schema["oneOf"]:
            try:
                _schema_validate(value, option, root, path)
            except AssertionError:
                continue
            matches += 1
        if matches != 1:
            raise AssertionError(f"{path} matched {matches} oneOf branches")
        return
    expected_types = schema.get("type")
    if expected_types is not None:
        if isinstance(expected_types, str):
            expected_types = [expected_types]
        type_matches = {
            "object": isinstance(value, dict),
            "array": isinstance(value, list),
            "string": isinstance(value, str),
            "integer": isinstance(value, int) and not isinstance(value, bool),
            "number": isinstance(value, (int, float)) and not isinstance(value, bool),
            "null": value is None,
            "boolean": isinstance(value, bool),
        }
        if not any(type_matches.get(item, False) for item in expected_types):
            raise AssertionError(f"{path} has wrong type")
    if isinstance(value, dict):
        required = set(schema.get("required", []))
        missing = required - set(value)
        if missing:
            raise AssertionError(f"{path} is missing {sorted(missing)}")
        properties = schema.get("properties", {})
        patterns = schema.get("patternProperties", {})
        if schema.get("additionalProperties") is False:
            import re

            unknown = [
                key
                for key in value
                if key not in properties
                and not any(re.search(pattern, key) for pattern in patterns)
            ]
            if unknown:
                raise AssertionError(f"{path} has unknown keys {unknown}")
        for key, child in value.items():
            if key in properties:
                _schema_validate(child, properties[key], root, f"{path}.{key}")
            for pattern, pattern_schema in patterns.items():
                import re

                if re.search(pattern, key):
                    _schema_validate(child, pattern_schema, root, f"{path}.{key}")
    elif isinstance(value, list):
        if len(value) < schema.get("minItems", 0) or len(value) > schema.get("maxItems", U64_MAX):
            raise AssertionError(f"{path} has invalid item count")
        if "items" in schema:
            for index, child in enumerate(value):
                _schema_validate(child, schema["items"], root, f"{path}[{index}]")
    elif isinstance(value, str):
        if len(value) < schema.get("minLength", 0):
            raise AssertionError(f"{path} is too short")
        if "pattern" in schema:
            import re

            if re.search(schema["pattern"], value) is None:
                raise AssertionError(f"{path} does not match pattern")
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        if "minimum" in schema and value < schema["minimum"]:
            raise AssertionError(f"{path} is below minimum")
        if "maximum" in schema and value > schema["maximum"]:
            raise AssertionError(f"{path} is above maximum")


def _valid(workload: dict) -> None:
    """Validate through the public validator, accepting either None or a true result."""
    result = validate_workload(workload)
    if result is False:
        raise AssertionError("validate_workload returned false for a generated workload")


def _rejected(workload: dict) -> None:
    try:
        result = validate_workload(workload)
    except WorkloadError, ValueError, TypeError, KeyError:
        return
    if result is not False:
        raise AssertionError("tampered workload was accepted")


def _ratio(workload: dict, name: str) -> tuple[int, int, int]:
    prefixes = {
        "density": ("density_numerator", "density_denominator", "density_ppm"),
        "critical_path_ratio": (
            "critical_path_duration_ticks",
            "total_duration_ticks",
            "critical_path_ratio_ppm",
        ),
        "semantic_overlap": (
            "semantic_overlap_numerator",
            "semantic_overlap_denominator",
            "semantic_claim_overlap_ppm",
        ),
        "resource_contention": (
            "resource_contention_numerator",
            "resource_contention_denominator",
            "resource_contention_ppm",
        ),
    }
    keys = prefixes[name]
    return tuple(workload["dimensions"][key] for key in keys)  # type: ignore[return-value]


def _nodes(workload: dict) -> dict[str, dict]:
    return {node["id"]: node for node in workload["nodes"]}


def _graph(workload: dict) -> tuple[dict[str, set[str]], dict[str, set[str]]]:
    nodes = _nodes(workload)
    outgoing = {node_id: set() for node_id in nodes}
    incoming = {node_id: set() for node_id in nodes}
    for edge in workload["edges"]:
        source, target = edge["from"], edge["to"]
        if source not in nodes or target not in nodes:
            raise AssertionError("edge references an unknown node")
        if source == target:
            raise AssertionError("self edge")
        if target in outgoing[source]:
            raise AssertionError("duplicate edge")
        outgoing[source].add(target)
        incoming[target].add(source)
    return outgoing, incoming


def _reachability(workload: dict) -> dict[str, set[str]]:
    outgoing, _ = _graph(workload)
    reach: dict[str, set[str]] = {node_id: set() for node_id in outgoing}
    for start in outgoing:
        stack = list(outgoing[start])
        while stack:
            target = stack.pop()
            if target in reach[start]:
                continue
            reach[start].add(target)
            stack.extend(outgoing[target])
    return reach


def _topological(workload: dict) -> list[str]:
    outgoing, incoming = _graph(workload)
    indegree = {node_id: len(parents) for node_id, parents in incoming.items()}
    ready = sorted(node_id for node_id, degree in indegree.items() if degree == 0)
    order = []
    while ready:
        node_id = ready.pop(0)
        order.append(node_id)
        for target in sorted(outgoing[node_id]):
            indegree[target] -= 1
            if indegree[target] == 0:
                ready.append(target)
                ready.sort()
    if len(order) != len(indegree):
        raise AssertionError("workload graph is cyclic")
    return order


def _max_antichain_bruteforce(workload: dict) -> int:
    """Exact width oracle for small fixtures; uses reachability, not recorded ranks."""
    node_ids = list(_nodes(workload))
    reach = _reachability(workload)
    best = 0
    for mask in range(1 << len(node_ids)):
        if mask.bit_count() <= best:
            continue
        chosen = [node_ids[index] for index in range(len(node_ids)) if mask & (1 << index)]
        if all(
            other not in reach[first] and first not in reach[other]
            for index, first in enumerate(chosen)
            for other in chosen[index + 1 :]
        ):
            best = len(chosen)
    return best


def _weighted_metrics(workload: dict) -> tuple[int, int]:
    nodes = _nodes(workload)
    outgoing, incoming = _graph(workload)
    longest: dict[str, int] = {}
    for node_id in _topological(workload):
        longest[node_id] = nodes[node_id]["reference_duration_ticks"] + max(
            (longest[parent] for parent in incoming[node_id]), default=0
        )
    return max(longest.values()), sum(node["reference_duration_ticks"] for node in nodes.values())


def _histogram(values: list[int]) -> dict[int, int]:
    result: dict[int, int] = {}
    for value in values:
        result[value] = result.get(value, 0) + 1
    return result


def _stored_histogram(workload: dict, name: str) -> dict[int, int]:
    return {int(key): value for key, value in workload["dimensions"][name].items()}


def _ppm(numerator: int, denominator: int) -> int:
    return numerator * 1_000_000 // denominator if denominator else 0


def _without_identity(workload: dict) -> dict:
    result = copy.deepcopy(workload)
    result.pop("workload_id", None)
    result.pop("seed", None)
    return result


def _refresh_workload_id(workload: dict) -> None:
    """Recompute identity after a mutation when testing a semantic invariant."""
    workload["workload_id"] = "sha256:" + workload_digest(workload)


@functools.cache
def _cached_synthetic(distribution: str, seed: int) -> dict:
    return generate_synthetic(distribution, seed=seed)


class WorkloadAcceptanceTests(unittest.TestCase):
    def make_workload(self, distribution: str, seed: int = 1) -> dict:
        return copy.deepcopy(_cached_synthetic(distribution, seed))

    def test_public_constants_and_distribution_vocabulary(self) -> None:
        self.assertEqual(WORKLOAD_SCHEMA_VERSION, "gordian.workload.v1")
        self.assertEqual(SOURCE_SCHEMA_VERSION, "gordian.repository-workload-source.v1")
        self.assertEqual(SEED_MATRIX, (1, 2, 3, 5, 8, 13, 21, 34))

    def test_every_distribution_has_metadata_and_is_valid(self) -> None:
        for distribution in DISTRIBUTIONS:
            with self.subTest(distribution=distribution):
                workload = self.make_workload(distribution, 1)
                _valid(workload)
                self.assertEqual(workload["schema_version"], WORKLOAD_SCHEMA_VERSION)
                self.assertRegex(workload["generator"]["version"], r"^\d+\.\d+\.\d+$")
                self.assertEqual(workload["distribution_id"], distribution)
                self.assertEqual(
                    workload["generator"]["parameters"]["distribution_id"], distribution
                )
                self.assertEqual(workload["origin"]["kind"], "synthetic")
                self.assertEqual(workload["seed"], 1)

    def test_seed_matrix_all_distributions(self) -> None:
        seeds = list(SEED_MATRIX)
        for distribution in DISTRIBUTIONS:
            for seed in seeds:
                with self.subTest(distribution=distribution, seed=seed):
                    _valid(self.make_workload(distribution, seed))

    def test_worker_duration_scale_schema_bounds(self) -> None:
        for distribution in DISTRIBUTIONS:
            for seed in SEED_MATRIX:
                with self.subTest(distribution=distribution, seed=seed):
                    workload = self.make_workload(distribution, seed)
                    scales = [worker["duration_scale_ppm"] for worker in workload["workers"]]
                    self.assertTrue(all(500_000 <= scale <= 1_000_000 for scale in scales))
                    candidate = copy.deepcopy(workload)
                    candidate["workers"][0]["duration_scale_ppm"] = 1_000_001
                    _refresh_workload_id(candidate)
                    _rejected(candidate)

    def test_seed_11765_and_all_matrix_records_match_json_schema(self) -> None:
        schema_path = Path(__file__).parents[2] / "experiments" / "schema" / "workload.schema.json"
        schema = json.loads(schema_path.read_text(encoding="utf-8"))
        for distribution in DISTRIBUTIONS:
            for seed in (*SEED_MATRIX, 11765):
                with self.subTest(distribution=distribution, seed=seed):
                    workload = self.make_workload(distribution, seed)
                    _schema_validate(workload, schema, schema)

    def test_envelope_bounds(self) -> None:
        envelopes = {
            "rep-small": (50, 200, 4, 12, 250_000, 400_000, 50_000, 150_000),
            "rep-wide": (500, 2000, 40, 120, 50_000, 150_000, 50_000, 200_000),
            "adv-deep": (200, 800, 2, 4, 600_000, 900_000, 0, 100_000),
            "adv-contended": (300, 1000, 20, 60, 150_000, 300_000, 550_000, 850_000),
        }
        for distribution, (nmin, nmax, wmin, wmax, cpmin, cpmax, omin, omax) in envelopes.items():
            workload = self.make_workload(distribution, 34)
            node_count = len(workload["nodes"])
            width = workload["dimensions"]["dag_width"]
            cp = _ratio(workload, "critical_path_ratio")
            overlap = _ratio(workload, "semantic_overlap")
            self.assertTrue(nmin <= node_count <= nmax)
            self.assertTrue(wmin <= width <= wmax)
            self.assertTrue(cpmin <= cp[2] <= cpmax)
            self.assertTrue(omin <= overlap[2] <= omax)

    def test_graph_is_dag_and_reachability_is_consistent(self) -> None:
        for distribution in DISTRIBUTIONS:
            with self.subTest(distribution=distribution):
                workload = self.make_workload(distribution, 34)
                _topological(workload)
                reach = _reachability(workload)
                self.assertTrue(all(node_id not in targets for node_id, targets in reach.items()))
                self.assertEqual(set(reach), {node["id"] for node in workload["nodes"]})

    def test_small_width_is_exact_antichain_and_complete_layers_cross_check(self) -> None:
        workload = self.make_workload("rep-small", 1)
        small = copy.deepcopy(workload)
        small["nodes"] = small["nodes"][:10]
        small_ids = {node["id"] for node in small["nodes"]}
        small["edges"] = [
            edge for edge in small["edges"] if edge["from"] in small_ids and edge["to"] in small_ids
        ]
        self.assertLessEqual(_max_antichain_bruteforce(small), workload["dimensions"]["dag_width"])
        ranks: dict[int, int] = {}
        for node in workload["nodes"]:
            ranks[node["rank"]] = ranks.get(node["rank"], 0) + 1
        self.assertEqual(workload["dimensions"]["dag_width"], max(ranks.values()))

    def test_weighted_critical_path_and_total_work_integer_oracle(self) -> None:
        workload = self.make_workload("rep-small", 34)
        critical, total = _weighted_metrics(workload)
        numerator, denominator, ppm = _ratio(workload, "critical_path_ratio")
        self.assertEqual((numerator, denominator), (critical, total))
        self.assertEqual(ppm, _ppm(critical, total))

    def test_density_fan_degrees_and_histograms(self) -> None:
        workload = self.make_workload("rep-small", 1)
        outgoing, incoming = _graph(workload)
        n = len(workload["nodes"])
        edge_count = sum(len(targets) for targets in outgoing.values())
        numerator, denominator, ppm = _ratio(workload, "density")
        self.assertEqual((numerator, denominator), (edge_count, n * (n - 1) // 2 if n > 1 else 1))
        self.assertEqual(ppm, _ppm(numerator, denominator))
        for node in workload["nodes"]:
            self.assertEqual(node["fan_in"], len(incoming[node["id"]]))
            self.assertEqual(node["fan_out"], len(outgoing[node["id"]]))
        self.assertEqual(
            _stored_histogram(workload, "fan_in_histogram"),
            _histogram(list(map(len, incoming.values()))),
        )
        self.assertEqual(
            _stored_histogram(workload, "fan_out_histogram"),
            _histogram(list(map(len, outgoing.values()))),
        )

    def test_semantic_overlap_uses_incomparable_pairs(self) -> None:
        workload = self.make_workload("rep-small", 34)
        node_ids = list(_nodes(workload))
        reach = _reachability(workload)
        incomparable = shared = 0
        claims = {node["id"]: set(node["semantic_claims"]) for node in workload["nodes"]}
        for index, first in enumerate(node_ids):
            for second in node_ids[index + 1 :]:
                if second not in reach[first] and first not in reach[second]:
                    incomparable += 1
                    shared += bool(claims[first] & claims[second])
        numerator, denominator, ppm = _ratio(workload, "semantic_overlap")
        self.assertEqual((numerator, denominator), (shared, incomparable))
        self.assertEqual(ppm, _ppm(shared, incomparable))

    def test_capacity_one_resource_contention_and_heterogeneous_workers(self) -> None:
        workload = self.make_workload("adv-contended", 34)
        nodes = workload["nodes"]
        pair_count = 0
        incomparable_count = 0
        resources = {node["id"]: set(node["resource_claims"]) for node in nodes}
        reach = _reachability(workload)
        for index, first in enumerate(nodes):
            for second in nodes[index + 1 :]:
                if (
                    second["id"] not in reach[first["id"]]
                    and first["id"] not in reach[second["id"]]
                ):
                    incomparable_count += 1
                    pair_count += bool(resources[first["id"]] & resources[second["id"]])
        numerator, denominator, ppm = _ratio(workload, "resource_contention")
        self.assertEqual((numerator, denominator), (pair_count, incomparable_count))
        self.assertEqual(ppm, _ppm(numerator, denominator))
        workers = workload["workers"]
        self.assertGreater(len({tuple(w["capabilities"]) for w in workers}), 1)
        self.assertGreater(len({w["cost_units_per_second"] for w in workers}), 1)
        self.assertGreater(len({w["duration_scale_ppm"] for w in workers}), 1)
        for node in nodes:
            self.assertTrue(
                any(
                    set(node["required_capabilities"]).issubset(set(w["capabilities"]))
                    for w in workers
                )
            )

    def test_history_references_and_nonempty_volume(self) -> None:
        workload = self.make_workload("rep-small", 1)
        node_ids = set(_nodes(workload))
        self.assertTrue(workload["events"])
        self.assertTrue(workload["evidence"])
        self.assertTrue(all(event["node_id"] in node_ids for event in workload["events"]))
        self.assertTrue(
            all(
                item["node_id"] in node_ids and item["subject"] in node_ids
                for item in workload["evidence"]
            )
        )
        self.assertEqual(len(workload["events"]), workload["dimensions"]["event_count"])
        self.assertEqual(len(workload["evidence"]), workload["dimensions"]["evidence_count"])

    def test_deterministic_canonical_bytes_and_digest(self) -> None:
        first = self.make_workload("rep-small", 1)
        second = self.make_workload("rep-small", 1)
        encoded = canonical_json(first)
        self.assertIsInstance(encoded, bytes)
        self.assertEqual(encoded, canonical_json(second))
        self.assertTrue(encoded.endswith(b"\n"))
        self.assertNotIn(b" ", encoded)
        self.assertNotIn(b"NaN", encoded)
        self.assertNotIn(b"Infinity", encoded)
        identity = copy.deepcopy(first)
        identity.pop("workload_id")
        self.assertEqual(
            workload_digest(first), hashlib.sha256(canonical_json(identity)).hexdigest()
        )
        self.assertEqual(first["workload_id"], "sha256:" + workload_digest(first))

    def test_canonical_roundtrip_accepts_lexicographic_histogram_keys(self) -> None:
        workload = generate_synthetic("rep-small", seed=1)
        roundtrip = json.loads(canonical_json(workload))
        buckets = set(roundtrip["dimensions"]["fan_in_histogram"]) | set(
            roundtrip["dimensions"]["fan_out_histogram"]
        )
        self.assertTrue(any(int(bucket) >= 10 for bucket in buckets))
        _valid(roundtrip)

    def test_different_seeds_change_payload_not_only_identity(self) -> None:
        first = self.make_workload("rep-small", 1)
        second = self.make_workload("rep-small", 34)
        self.assertNotEqual(_without_identity(first), _without_identity(second))

    def test_rejects_all_metadata_tampering(self) -> None:
        base = self.make_workload("rep-small", 1)
        mutations = {
            "seed": lambda w: w.__setitem__("seed", 34),
            "parameters": lambda w: w["generator"]["parameters"].__setitem__(
                "requested_node_count", 1
            ),
            "distribution": lambda w: w.__setitem__("distribution_id", "adv-deep"),
            "schema": lambda w: w.__setitem__("schema_version", "gordian-workload-v999"),
            "id": lambda w: w.__setitem__("workload_id", "0" * 64),
            "width": lambda w: w["dimensions"].__setitem__("dag_width", 1),
            "critical_path": lambda w: w["dimensions"].__setitem__("critical_path_ratio_ppm", 1),
            "overlap": lambda w: w["dimensions"].__setitem__("semantic_claim_overlap_ppm", 1),
            "contention": lambda w: w["dimensions"].__setitem__("resource_contention_ppm", 1),
            "worker_compatibility": lambda w: w["nodes"][0]["required_capabilities"].append(
                "unknown-capability"
            ),
            "history_ref": lambda w: (
                w["evidence"][0].__setitem__("subject", "missing"),
                _refresh_workload_id(w),
            ),
        }
        for label, mutate in mutations.items():
            with self.subTest(label=label):
                candidate = copy.deepcopy(base)
                mutate(candidate)
                _rejected(candidate)

    def test_rejects_evidence_subject_not_a_node_after_identity_recompute(self) -> None:
        candidate = self.make_workload("rep-small", 1)
        candidate["evidence"][0]["subject"] = "missing-node"
        _refresh_workload_id(candidate)
        _rejected(candidate)

    def test_cli_rejects_duplicate_and_noncanonical_workload_json(self) -> None:
        workload = self.make_workload("rep-small", 1)
        with tempfile.TemporaryDirectory(prefix="gordian-workload-json-") as directory:
            path = Path(directory) / "workload.json"
            path.write_bytes(
                canonical_json(workload).replace(
                    b'"schema_version":',
                    b'"schema_version":"gordian.workload.v1","schema_version":',
                    1,
                )
            )
            environment = dict(os.environ)
            environment["PYTHONPATH"] = str(Path.cwd() / "orchestration/src")
            duplicate = subprocess.run(
                [sys.executable, "-m", "gordian_orchestration.workloads", "validate", str(path)],
                capture_output=True,
                text=True,
                env=environment,
                check=False,
            )
            self.assertNotEqual(duplicate.returncode, 0)
            path.write_text(json.dumps(workload, sort_keys=True) + "\n", encoding="utf-8")
            noncanonical = subprocess.run(
                [sys.executable, "-m", "gordian_orchestration.workloads", "validate", str(path)],
                capture_output=True,
                text=True,
                env=environment,
                check=False,
            )
            self.assertNotEqual(noncanonical.returncode, 0)

    def test_rejects_dangling_cycle_and_impossible_parameters(self) -> None:
        base = self.make_workload("rep-small", 1)
        for label, mutate in (
            (
                "dangling",
                lambda w: w["edges"].append({"from": "missing", "to": w["nodes"][0]["id"]}),
            ),
            (
                "cycle",
                lambda w: w["edges"].extend(
                    [
                        {"from": w["nodes"][-1]["id"], "to": w["nodes"][0]["id"]},
                    ]
                ),
            ),
            ("duplicate", lambda w: w["edges"].append(copy.deepcopy(w["edges"][0]))),
            (
                "impossible",
                lambda w: w["generator"]["parameters"].__setitem__("requested_node_count", 0),
            ),
        ):
            with self.subTest(label=label):
                candidate = copy.deepcopy(base)
                mutate(candidate)
                _rejected(candidate)


class GoldenManifestAcceptanceTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tempdir = tempfile.TemporaryDirectory(prefix="gordian-workload-golden-")
        self.root = Path(self.tempdir.name)

    def tearDown(self) -> None:
        self.tempdir.cleanup()

    def manifest_fixture(self) -> tuple[Path, Path, dict]:
        entries = []
        minimum_counts = {
            "adv-contended": 300,
            "adv-deep": 200,
            "rep-small": 50,
            "rep-wide": 500,
        }
        for distribution_id, node_count in minimum_counts.items():
            workload = generate_synthetic(distribution_id, seed=1, node_count=node_count)
            filename = f"{distribution_id}.json"
            (self.root / filename).write_bytes(canonical_json(workload))
            entries.append(
                {
                    "path": filename,
                    "sha256": hashlib.sha256(canonical_json(workload)).hexdigest(),
                    "kind": "synthetic",
                    "seed": 1,
                    "distribution_id": distribution_id,
                    "node_count": len(workload["nodes"]),
                }
            )
        repository_root = self.root / "repository"
        repository_root.mkdir(exist_ok=True)
        layers = [[f"task-{layer:02d}-{item:02d}" for item in range(5)] for layer in range(10)]
        source_nodes = [
            {
                "id": node_id,
                "reference_duration_ticks": 2 if item == 0 else 1,
                "cost_units": 1,
                "required_capabilities": [f"capability:{item % 2}"],
                "semantic_claims": [
                    "claim:shared" if item < 2 else f"claim:{layer:02d}:{item:02d}"
                ],
                "resource_claims": [
                    "resource:shared" if item < 2 else f"resource:{layer:02d}:{item:02d}"
                ],
            }
            for layer, layer_nodes in enumerate(layers)
            for item, node_id in enumerate(layer_nodes)
        ]
        source_edges = [
            {"from": source, "to": target}
            for previous, current in zip(layers[:-1], layers[1:], strict=True)
            for source in previous
            for target in current
        ]
        source_manifest = {
            "schema_version": SOURCE_SCHEMA_VERSION,
            "repository": "fixture-repository",
            "revision": "fixture-revision-001",
            "nodes": source_nodes,
            "edges": source_edges,
        }
        (repository_root / "gordian-workload-source.json").write_text(
            json.dumps(source_manifest, sort_keys=True) + "\n", encoding="utf-8"
        )
        repository_workload = generate_repository(
            repository_root, revision="fixture-revision-001", seed=1
        )
        repository_filename = "repository-derived.json"
        (self.root / repository_filename).write_bytes(canonical_json(repository_workload))
        entries.append(
            {
                "path": repository_filename,
                "sha256": hashlib.sha256(canonical_json(repository_workload)).hexdigest(),
                "kind": "repository-derived",
                "seed": 1,
                "distribution_id": "rep-small",
                "repository_path": "repository",
                "revision": "fixture-revision-001",
                "manifest_relative_path": "gordian-workload-source.json",
            }
        )
        entries.sort(key=lambda entry: entry["path"])
        manifest = {
            "schema_version": "gordian.workload-golden-manifest.v1",
            "entries": entries,
        }
        manifest_path = self.root / "manifest.json"
        manifest_path.write_text(json.dumps(manifest, sort_keys=True) + "\n", encoding="utf-8")
        return manifest_path, self.root / entries[0]["path"], manifest

    def assert_manifest_valid(self, manifest_path: Path) -> None:
        result = validate_golden_manifest(manifest_path)
        if result is False:
            raise AssertionError("validate_golden_manifest returned false")

    def assert_manifest_rejected(self, manifest_path: Path) -> None:
        with self.assertRaises((WorkloadError, ValueError, TypeError, KeyError)):
            validate_golden_manifest(manifest_path)

    def write_manifest(self, path: Path, value: dict) -> None:
        path.write_text(json.dumps(value, sort_keys=True) + "\n", encoding="utf-8")

    def test_valid_manifest_and_cli_are_read_only(self) -> None:
        manifest_path, golden_path, manifest = self.manifest_fixture()
        before_manifest = manifest_path.read_bytes()
        before_golden = golden_path.read_bytes()
        self.assert_manifest_valid(manifest_path)
        environment = dict(os.environ)
        environment["PYTHONPATH"] = str(Path.cwd() / "orchestration/src")
        result = subprocess.run(
            [
                sys.executable,
                "-W",
                "error",
                "-m",
                "gordian_orchestration.workloads",
                "validate",
                "--manifest",
                str(manifest_path),
            ],
            capture_output=True,
            text=True,
            env=environment,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, "")
        self.assertEqual(manifest_path.read_bytes(), before_manifest)
        self.assertEqual(golden_path.read_bytes(), before_golden)
        self.assertEqual(len(manifest["entries"]), 5)

    def test_package_root_workload_exports_are_lazy_and_stable(self) -> None:
        import gordian_orchestration

        self.assertIs(gordian_orchestration.generate_synthetic, generate_synthetic)
        self.assertEqual(gordian_orchestration.__all__[0], "CommandResult")

    def test_generated_parameter_shapes_match_checked_in_schema(self) -> None:
        _, _, manifest = self.manifest_fixture()
        schema_path = Path(__file__).parents[2] / "experiments" / "schema" / "workload.schema.json"
        schema = json.loads(schema_path.read_text(encoding="utf-8"))
        parameter_schema = schema["$defs"]["generator"]["properties"]["parameters"]
        shapes = {
            reference["$ref"].rsplit("/", 1)[-1]: schema["$defs"][
                reference["$ref"].rsplit("/", 1)[-1]
            ]
            for reference in parameter_schema["oneOf"]
        }
        for entry in manifest["entries"]:
            workload = json.loads((self.root / entry["path"]).read_text(encoding="utf-8"))
            parameters = workload["generator"]["parameters"]
            shape_name = (
                "synthetic_generator_parameters"
                if entry["kind"] == "synthetic"
                else "repository_generator_parameters"
            )
            self.assertIn(shape_name, shapes)
            self.assertEqual(set(parameters), set(shapes[shape_name]["required"]))
            self.assertEqual(set(parameters), set(shapes[shape_name]["properties"]))

    def test_manifest_rejects_digest_and_metadata_mutations(self) -> None:
        manifest_path, _, manifest = self.manifest_fixture()
        mutations = {
            "golden_bytes": lambda value: (self.root / value["entries"][0]["path"]).write_bytes(
                b"{}\n"
            ),
            "wrong_sha256": lambda value: value["entries"][0].__setitem__("sha256", "0" * 64),
            "wrong_seed": lambda value: value["entries"][0].__setitem__("seed", 999),
            "wrong_distribution": lambda value: value["entries"][0].__setitem__(
                "distribution_id", "adv-deep"
            ),
            "wrong_node_count": lambda value: value["entries"][0].__setitem__("node_count", 1),
            "unknown_key": lambda value: value.__setitem__("unexpected", True),
            "entry_unknown_key": lambda value: value["entries"][0].__setitem__("unexpected", True),
        }
        for label, mutate in mutations.items():
            with self.subTest(label=label):
                fresh_path, _, fresh = self.manifest_fixture()
                mutate(fresh)
                self.write_manifest(fresh_path, fresh)
                self.assert_manifest_rejected(fresh_path)

    def test_manifest_rejects_path_order_duplicates_and_traversal(self) -> None:
        for label, mutate in (
            ("unsorted", lambda value: value["entries"].reverse()),
            (
                "duplicate",
                lambda value: value["entries"].__setitem__(1, copy.deepcopy(value["entries"][0])),
            ),
            (
                "absolute",
                lambda value: value["entries"][0].__setitem__("path", "/tmp/workload.json"),
            ),
            (
                "repository-root escape",
                lambda value: value["entries"][0].__setitem__("path", "README.md"),
            ),
            ("parent", lambda value: value["entries"][0].__setitem__("path", "../workload.json")),
        ):
            with self.subTest(label=label):
                manifest_path, _, manifest = self.manifest_fixture()
                mutate(manifest)
                self.write_manifest(manifest_path, manifest)
                self.assert_manifest_rejected(manifest_path)

    def test_repository_regeneration_mismatch_is_rejected(self) -> None:
        manifest_path, _, manifest = self.manifest_fixture()
        repository_entry = next(
            entry for entry in manifest["entries"] if entry["kind"] == "repository-derived"
        )
        repository_entry["revision"] = "wrong-revision"
        self.write_manifest(manifest_path, manifest)
        self.assert_manifest_rejected(manifest_path)

    def test_manifest_rejects_missing_and_duplicate_composition(self) -> None:
        manifest_path, _, manifest = self.manifest_fixture()
        missing = copy.deepcopy(manifest)
        missing["entries"].pop()
        self.write_manifest(manifest_path, missing)
        self.assert_manifest_rejected(manifest_path)

        duplicate = copy.deepcopy(manifest)
        duplicate["entries"][1]["distribution_id"] = duplicate["entries"][0]["distribution_id"]
        self.write_manifest(manifest_path, duplicate)
        self.assert_manifest_rejected(manifest_path)


class RepositoryWorkloadAcceptanceTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tempdirs: list[tempfile.TemporaryDirectory[str]] = []

    def tearDown(self) -> None:
        for tempdir in self.tempdirs:
            tempdir.cleanup()

    def fixture(self) -> tuple[Path, dict]:
        tempdir = tempfile.TemporaryDirectory(prefix="gordian-workload-repository-")
        self.tempdirs.append(tempdir)
        root = Path(tempdir.name)
        layers = [[f"task-{layer:02d}-{item:02d}" for item in range(5)] for layer in range(10)]
        nodes = [
            {
                "id": node_id,
                "reference_duration_ticks": 2 if item == 0 else 1,
                "cost_units": 1,
                "required_capabilities": [f"capability:{item % 2}"],
                "semantic_claims": [
                    "claim:shared" if item < 2 else f"claim-{layer:02d}-{item:02d}"
                ],
                "resource_claims": [
                    "resource:shared" if item < 2 else f"resource:{layer:02d}-{item:02d}"
                ],
            }
            for layer, layer_nodes in enumerate(layers)
            for item, node_id in enumerate(layer_nodes)
        ]
        edges = [
            {"from": source, "to": target}
            for previous, current in zip(layers[:-1], layers[1:], strict=True)
            for source in previous
            for target in current
        ]
        manifest = {
            "schema_version": SOURCE_SCHEMA_VERSION,
            "repository": "fixture-repository",
            "revision": "fixture-revision-001",
            "nodes": nodes,
            "edges": edges,
        }
        (root / "gordian-workload-source.json").write_text(
            json.dumps(manifest, sort_keys=True) + "\n", encoding="utf-8"
        )
        return root, manifest

    def test_extract_is_path_mtime_locale_and_hashseed_independent(self) -> None:
        root, _ = self.fixture()
        second = Path(tempfile.mkdtemp(prefix="gordian-workload-copy-"))
        self.addCleanup(shutil.rmtree, second)
        shutil.copytree(root, second / "repo")
        left = extract_repository_source(root, revision="fixture-revision-001")
        right = extract_repository_source(second / "repo", revision="fixture-revision-001")
        self.assertEqual(left, right)
        self.assertEqual(canonical_json(left), canonical_json(right))
        self.assertEqual(left.file_digest, right.file_digest)
        os.utime(second / "repo" / "gordian-workload-source.json", (1, 1))
        after_mtime = extract_repository_source(second / "repo", revision="fixture-revision-001")
        self.assertEqual(left, after_mtime)
        self.assertEqual(left.file_digest, after_mtime.file_digest)

    def test_in_scope_manifest_edit_changes_source_digest_and_dimension(self) -> None:
        root, _ = self.fixture()
        before = extract_repository_source(root, revision="fixture-revision-001")
        manifest_path = root / "gordian-workload-source.json"
        changed = json.loads(manifest_path.read_text(encoding="utf-8"))
        changed["nodes"][0]["semantic_claims"].append("new-claim")
        manifest_path.write_text(json.dumps(changed, sort_keys=True) + "\n", encoding="utf-8")
        after = extract_repository_source(root, revision="fixture-revision-001")
        self.assertNotEqual(
            getattr(before, "file_digest", None), getattr(after, "file_digest", None)
        )
        self.assertNotEqual(before, after)

    def test_out_of_scope_files_and_metadata_do_not_change_source(self) -> None:
        root, _ = self.fixture()
        before = extract_repository_source(root, revision="fixture-revision-001")
        (root / ".git").mkdir()
        (root / ".jj").mkdir()
        (root / "cache").mkdir()
        (root / ".git" / "ignored").write_text("x", encoding="utf-8")
        (root / ".jj" / "ignored").write_text("x", encoding="utf-8")
        (root / "cache" / "ignored").write_text("x", encoding="utf-8")
        os.utime(root, (2, 2))
        self.assertEqual(before, extract_repository_source(root, revision="fixture-revision-001"))

    def test_repository_generator_has_repository_origin_and_valid_graph(self) -> None:
        root, _ = self.fixture()
        source = extract_repository_source(root, revision="fixture-revision-001")
        derived = generate_repository_derived(source, "rep-small", 1)
        generated = generate_repository(root, revision="fixture-revision-001", seed=1)
        _valid(derived)
        _valid(generated)
        self.assertEqual(derived["origin"]["kind"], "repository-derived")
        self.assertEqual(derived["origin"]["source_sha256"], source.file_digest)
        self.assertEqual(derived, generated)
        self.assertEqual(len(derived["nodes"]), 50)

    def test_repository_weighted_path_can_be_shorter_than_graph_depth(self) -> None:
        root, manifest = self.fixture()
        layers = [[f"task-{layer:02d}-{item:02d}" for item in range(5)] for layer in range(10)]
        manifest["edges"] = [
            {"from": source, "to": target}
            for previous, current in zip(layers[:-1], layers[1:], strict=True)
            for source in previous
            if not source.endswith("-00")
            for target in current[1:]
        ]
        manifest["edges"].append({"from": layers[0][0], "to": layers[-1][0]})
        manifest["edges"].extend(
            {"from": layers[index][0], "to": layers[index + 1][0]}
            for index in range(len(layers) - 2)
        )
        manifest["edges"].extend({"from": source, "to": layers[-1][0]} for source in layers[-2][1:])
        for node in manifest["nodes"]:
            node["reference_duration_ticks"] = 1
            if node["id"] in {layers[0][0], layers[-1][0]}:
                node["reference_duration_ticks"] = 10
            layer, item = node["id"].split("-")[1:]
            if int(item) < 3:
                node["semantic_claims"] = [f"claim:weighted:{layer}"]
            else:
                node["semantic_claims"] = [f"claim:weighted:{layer}:{item}"]
        manifest_path = root / "gordian-workload-source.json"
        manifest_path.write_text(json.dumps(manifest, sort_keys=True) + "\n", encoding="utf-8")
        source = extract_repository_source(root, revision="fixture-revision-001")
        workload = generate_repository_derived(source, "rep-small", 1)
        _valid(workload)
        self.assertEqual(workload["dimensions"]["dag_depth"], 10)
        self.assertEqual(workload["dimensions"]["critical_path_duration_ticks"], 20)
        self.assertEqual(workload["dimensions"]["critical_path_nodes"], 2)
        self.assertLess(
            workload["dimensions"]["critical_path_nodes"],
            workload["dimensions"]["dag_depth"],
        )

    def test_repository_input_rejects_symlink_missing_identity_and_bad_edges(self) -> None:
        root, manifest = self.fixture()
        manifest_path = root / "gordian-workload-source.json"
        for label, mutate in (
            (
                "dangling edge",
                lambda value: value["edges"].append(
                    {"from": "missing", "to": value["nodes"][0]["id"]}
                ),
            ),
            (
                "self edge",
                lambda value: value["edges"].append(
                    {"from": value["nodes"][0]["id"], "to": value["nodes"][0]["id"]}
                ),
            ),
            (
                "cycle",
                lambda value: value["edges"].append(
                    {"from": value["nodes"][-1]["id"], "to": value["nodes"][0]["id"]}
                ),
            ),
        ):
            with self.subTest(label=label):
                candidate = copy.deepcopy(manifest)
                mutate(candidate)
                manifest_path.write_text(
                    json.dumps(candidate, sort_keys=True) + "\n", encoding="utf-8"
                )
                with self.assertRaises((WorkloadError, ValueError, TypeError, KeyError)):
                    extract_repository_source(root, revision="fixture-revision-001")
                manifest_path.write_text(
                    json.dumps(manifest, sort_keys=True) + "\n", encoding="utf-8"
                )
        with self.assertRaises((WorkloadError, ValueError, TypeError, KeyError)):
            extract_repository_source(root, revision="")
        symlink = root / "source-manifest-link.json"
        symlink.symlink_to(manifest_path)
        with self.assertRaises((WorkloadError, ValueError, OSError)):
            extract_repository_source(root, revision="fixture-revision-001")

    def test_repository_manifest_schema_and_keys_are_closed(self) -> None:
        root, manifest = self.fixture()
        manifest_path = root / "gordian-workload-source.json"
        mutations = {
            "unknown key": lambda value: value.__setitem__("unexpected", True),
            "missing schema": lambda value: value.pop("schema_version"),
            "wrong schema": lambda value: value.__setitem__("schema_version", "source.v999"),
            "missing revision": lambda value: value.pop("revision"),
            "wrong revision": lambda value: value.__setitem__("revision", "other-revision"),
        }
        for label, mutate in mutations.items():
            with self.subTest(label=label):
                candidate = copy.deepcopy(manifest)
                mutate(candidate)
                manifest_path.write_text(
                    json.dumps(candidate, sort_keys=True) + "\n", encoding="utf-8"
                )
                with self.assertRaises(WorkloadError):
                    extract_repository_source(root, revision="fixture-revision-001")
                manifest_path.write_text(
                    json.dumps(manifest, sort_keys=True) + "\n", encoding="utf-8"
                )


if __name__ == "__main__":
    unittest.main(verbosity=2)
