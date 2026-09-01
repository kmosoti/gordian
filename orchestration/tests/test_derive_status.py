"""Unit tests for the derived Project 9 projection.

Every test here runs against a checked-in fixture or a stubbed GraphQL callable. No test
touches the network, so the graph math is falsifiable offline and CI needs no token.
"""

from __future__ import annotations

import hashlib
import io
import json
import subprocess
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest.mock import call, patch

from fixture_graph import BLOCKED_BY, EXPECTED_WAVE

from gordian_orchestration.closure_validation import (
    SourceBinding,
    closure_problems,
    evidence_header,
)
from gordian_orchestration.derive_status import (
    _BLOCKED_BY_PAGE_QUERY,
    _ISSUES_QUERY,
    BLOCKED,
    EX_CONFIG,
    READY,
    AcceptedRevision,
    Board,
    BoardItem,
    DependencyCycleError,
    GitHubConfigurationError,
    IssueRecord,
    ProjectField,
    accepted_revision,
    apply_change,
    bootstrap_satisfied,
    closure_record_valid,
    derive,
    derived_status,
    fan_in,
    fan_out,
    fetch_board,
    fetch_issues,
    load_snapshot,
    main,
    normalise_graph,
    plan_changes,
    ready_set,
    selection_key,
    selection_order,
    topological_order,
    transitive_blockers,
    waves,
)
from gordian_orchestration.provenance import Provenance


def _records(graph=BLOCKED_BY, *, closed=()) -> tuple[IssueRecord, ...]:
    return tuple(
        IssueRecord(
            number=number,
            title=f"Atom {number}",
            state="CLOSED" if number in closed else "OPEN",
            blocked_by=tuple(sorted(blockers)),
        )
        for number, blockers in sorted(graph.items())
    )


class GraphMathTests(unittest.TestCase):
    def test_waves_reproduce_the_recorded_longest_path_depths(self) -> None:
        self.assertEqual(waves(normalise_graph(BLOCKED_BY)), EXPECTED_WAVE)

    def test_wave_is_longest_path_not_shortest(self) -> None:
        # 3 blocks 4; 2 blocks both. The shortest path to 4 is 2 hops, the longest 3.
        graph = normalise_graph({1: [], 2: [1], 3: [2], 4: [2, 3]})
        self.assertEqual(waves(graph), {1: 0, 2: 1, 3: 2, 4: 3})

    def test_fan_out_counts_directly_blocked_issues(self) -> None:
        counts = fan_out(normalise_graph(BLOCKED_BY))
        self.assertEqual(counts[11], 6)
        self.assertEqual(counts[18], 11)
        self.assertEqual(counts[44], 2)

    def test_fan_in_counts_direct_blockers(self) -> None:
        counts = fan_in(normalise_graph(BLOCKED_BY))
        self.assertEqual(counts[1], 0)
        self.assertEqual(counts[2], 0)
        self.assertEqual(counts[70], 0)
        self.assertEqual(counts[9], 6)
        self.assertEqual(counts[68], 14)

    def test_roots_and_deepest_atom(self) -> None:
        depth = waves(normalise_graph(BLOCKED_BY))
        self.assertEqual(sorted(n for n, d in depth.items() if d == 0), [1, 2, 70])
        self.assertEqual(max(depth.values()), 18)
        self.assertEqual([n for n, d in depth.items() if d == 18], [69])

    def test_topological_order_places_blockers_first(self) -> None:
        graph = normalise_graph(BLOCKED_BY)
        position = {node: index for index, node in enumerate(topological_order(graph))}
        for node, blockers in graph.items():
            for blocker in blockers:
                self.assertLess(position[blocker], position[node], f"#{blocker} after #{node}")

    def test_normalise_drops_self_edges_and_closes_over_referenced_nodes(self) -> None:
        graph = normalise_graph({"#5": ["#5", 7, {"number": 7}]})
        self.assertEqual(graph, {5: (7,), 7: ()})

    def test_cycle_is_reported_rather_than_silently_ranked(self) -> None:
        with self.assertRaises(DependencyCycleError) as raised:
            waves(normalise_graph({1: [2], 2: [1], 3: []}))
        self.assertIn("[1, 2]", str(raised.exception))

    def test_transitive_blockers(self) -> None:
        graph = normalise_graph(BLOCKED_BY)
        self.assertEqual(transitive_blockers(graph, 1), ())
        self.assertIn(9, transitive_blockers(graph, 13))
        self.assertNotIn(13, transitive_blockers(graph, 9))


class StatusTests(unittest.TestCase):
    def test_open_atom_with_no_blockers_is_ready(self) -> None:
        record = IssueRecord(number=1, title="root", state="OPEN", blocked_by=())
        self.assertEqual(derived_status(record, frozenset(), None), READY)

    def test_assigned_open_atom_is_excluded_from_the_ready_set(self) -> None:
        record = IssueRecord(
            number=1,
            title="claimed root",
            state="OPEN",
            blocked_by=(),
            assignees=("worker",),
        )
        rows = derive((record,))
        self.assertIsNone(rows[0].status)
        self.assertEqual(ready_set(rows), ())

    def test_open_atom_with_an_unsatisfied_blocker_is_blocked(self) -> None:
        record = IssueRecord(number=2, title="child", state="OPEN", blocked_by=(1,))
        self.assertEqual(derived_status(record, frozenset(), None), BLOCKED)
        self.assertEqual(derived_status(record, frozenset({1}), None), READY)

    def test_human_owned_statuses_are_never_overwritten(self) -> None:
        record = IssueRecord(number=2, title="child", state="OPEN", blocked_by=(1,))
        for status in ("In Progress", "In Review", "Accepted"):
            self.assertIsNone(derived_status(record, frozenset(), status))

    def test_stale_derived_status_is_recomputed(self) -> None:
        record = IssueRecord(number=2, title="child", state="OPEN", blocked_by=(1,))
        self.assertEqual(derived_status(record, frozenset({1}), BLOCKED), READY)

    def test_closed_issue_status_is_left_to_a_human(self) -> None:
        record = IssueRecord(number=1, title="root", state="CLOSED", blocked_by=())
        self.assertIsNone(derived_status(record, frozenset(), None))

    def test_ready_set_over_the_real_graph_is_exactly_the_three_roots(self) -> None:
        rows = derive(_records())
        self.assertEqual([row.number for row in ready_set(rows)], [2, 1, 70])
        self.assertTrue(
            all(row.status == BLOCKED for row in rows if row.number not in (1, 2, 70))
        )

    def test_ready_set_equals_the_rows_whose_derived_status_is_ready(self) -> None:
        rows = derive(_records(closed={1}), satisfied={1})
        self.assertEqual(
            {row.number for row in ready_set(rows)},
            {row.number for row in rows if row.status == READY},
        )

    def test_satisfying_a_root_advances_the_ready_set(self) -> None:
        rows = derive(_records(closed={2}), satisfied={2})
        # Closing root #2 releases #3 and #8. Remaining roots #1 and #70 still lead
        # because selection orders by Wave before Fan Out.
        self.assertEqual([row.number for row in ready_set(rows)], [1, 70, 3, 8])

    def test_ready_set_is_printed_in_the_selection_order(self) -> None:
        """execution-order.md section 5: lowest Wave, then highest Fan Out, then number."""
        # Fixture graph: #1 and #2 are Wave 0 roots, #2 blocks three Atoms and #1 blocks
        # one, and #3 is a Wave 0 root that blocks nothing. #4 stays blocked.
        graph = {1: [], 2: [], 3: [], 4: [1], 5: [2], 6: [2], 7: [2], 8: [4]}
        rows = derive(_records(graph))
        self.assertEqual([row.number for row in ready_set(rows)], [2, 1, 3])

    def test_selection_order_puts_the_lower_wave_first_even_at_lower_fan_out(self) -> None:
        # #2 has Fan Out 3 but sits at Wave 1; #9, a root with Fan Out 0, precedes it.
        graph = {1: [], 2: [1], 3: [2], 4: [2], 5: [2], 9: []}
        rows = derive(_records(graph, closed={1}), satisfied={1})
        self.assertEqual([row.number for row in ready_set(rows)], [9, 2])

    def test_selection_order_breaks_a_wave_and_fan_out_tie_on_issue_number(self) -> None:
        graph = {5: [], 3: [], 7: [], 8: [3]}
        rows = derive(_records(graph))
        # #3 blocks one Atom so it leads; #5 and #7 tie on Wave 0 / Fan Out 0.
        self.assertEqual([row.number for row in ready_set(rows)], [3, 5, 7])

    def test_selection_key_is_wave_then_negated_fan_out_then_number(self) -> None:
        rows = {row.number: row for row in derive(_records())}
        self.assertEqual(selection_key(rows[18]), (EXPECTED_WAVE[18], -11, 18))

    def test_selection_order_is_total_over_the_real_graph(self) -> None:
        rows = derive(_records())
        keys = [selection_key(row) for row in selection_order(rows)]
        self.assertEqual(keys, sorted(keys))
        self.assertEqual(len(set(keys)), len(keys))

    def test_selection_order_does_not_filter(self) -> None:
        rows = derive(_records())
        self.assertEqual(
            {row.number for row in selection_order(rows)}, {row.number for row in rows}
        )

    def test_derive_keeps_ascending_issue_order_for_the_board_writer(self) -> None:
        numbers = [row.number for row in derive(_records())]
        self.assertEqual(numbers, sorted(numbers))

    def test_unsatisfied_blockers_are_reported_per_row(self) -> None:
        rows = {row.number: row for row in derive(_records(closed={1}), satisfied={1})}
        self.assertEqual(rows[9].unsatisfied_blockers, (2, 3, 4, 8, 71, 72))
        self.assertEqual(rows[2].unsatisfied_blockers, ())

    def test_derive_carries_the_graph_quantities_onto_each_row(self) -> None:
        rows = {row.number: row for row in derive(_records())}
        self.assertEqual(rows[18].wave, EXPECTED_WAVE[18])
        self.assertEqual(rows[18].fan_out, 11)
        self.assertEqual(rows[18].fan_in, 3)
        self.assertFalse(rows[18].ready)


class BootstrapSatisfactionTests(unittest.TestCase):
    EXACT = "a" * 40
    LOGICAL = "b" * 32
    ACCEPTED = "c" * 40
    ACCEPTED_CHANGE = "d" * 32
    SCHEMA = {
        "required": ["atom_id", "logical_change_id", "exact_state_id"],
        "properties": {"logical_change_id": {"minLength": 1}},
    }

    def _accepted(
        self,
        payload: dict | None = None,
        *,
        schema: dict | None = None,
        extra_files: dict[str, bytes] | None = None,
    ) -> AcceptedRevision:
        root = Path("/accepted-revision")
        files = {
            "artifacts/schema/closure-record.schema.json": json.dumps(
                schema or self.SCHEMA
            ).encode(),
        }
        if payload is not None:
            files["artifacts/atoms/1/closure.json"] = json.dumps(payload).encode()
        files.update(extra_files or {})
        return AcceptedRevision(
            self.ACCEPTED,
            self.ACCEPTED_CHANGE,
            root,
            files.get,
            lambda exact, logical, atom: SourceBinding(
                self.EXACT, self.LOGICAL, files.get, True, True
            ),
        )

    def _record(self, root: Path, number: int, payload: dict) -> None:
        path = root / str(number) / "closure.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(payload), encoding="utf-8")

    def test_source_binding_rejects_noncanonical_ids_even_with_custom_schema(self) -> None:
        problems = closure_problems(
            {"atom_id": "1", "exact_state_id": "c", "logical_change_id": "k"},
            {"type": "object"},
            label="closure.json",
            expected_atom="1",
            resolve_source=lambda *_: None,
            source_binding_required=True,
        )
        self.assertTrue(any("could not be resolved" in problem for problem in problems))

    def test_source_binding_requires_verifier_subject_to_match_exact_state(self) -> None:
        exact = self.EXACT
        payload = {
            "atom_id": "1",
            "exact_state_id": exact,
            "logical_change_id": self.LOGICAL,
            "verifiers": [{"subject_exact_state_id": "f" * 40}],
        }
        problems = closure_problems(
            payload,
            {"type": "object"},
            label="closure.json",
            expected_atom="1",
            resolve_source=lambda *_: SourceBinding(
                exact, self.LOGICAL, lambda _: None, True, True
            ),
            source_binding_required=True,
        )
        self.assertTrue(any("subject_exact_state_id" in problem for problem in problems))

    def test_closure_alone_does_not_satisfy(self) -> None:
        issues = _records({1: [], 2: [1]}, closed={1})
        with tempfile.TemporaryDirectory():
            satisfied, unevidenced = bootstrap_satisfied(
                issues,
                closure_root=Path("/accepted-revision/artifacts/atoms"),
                accepted=self._accepted(),
            )
        self.assertEqual(satisfied, frozenset())
        self.assertEqual(unevidenced, (1,))

    def test_workspace_copy_does_not_satisfy_against_accepted_revision(self) -> None:
        issues = _records({1: [], 2: [1]}, closed={1})
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self._record(
                root,
                1,
                {"atom_id": "1", "logical_change_id": "k", "exact_state_id": "c"},
            )
            satisfied, unevidenced = bootstrap_satisfied(
                issues,
                closure_root=root,
                accepted=self._accepted(),
            )
        self.assertEqual(satisfied, frozenset())
        self.assertEqual(unevidenced, (1,))

    def test_wrong_atom_closure_is_rejected_even_when_landed(self) -> None:
        issues = _records({1: [], 2: [1]}, closed={1})
        accepted = self._accepted(
            {"atom_id": "2", "logical_change_id": "k", "exact_state_id": "c"}
        )
        satisfied, unevidenced = bootstrap_satisfied(
            issues,
            closure_root=Path("/accepted-revision/artifacts/atoms"),
            accepted=accepted,
        )
        self.assertEqual(satisfied, frozenset())
        self.assertEqual(unevidenced, (1,))

    def test_noncanonical_closure_path_is_not_satisfaction_evidence(self) -> None:
        issues = _records({1: []}, closed={1})
        accepted = self._accepted(
            {"atom_id": "1", "logical_change_id": "k", "exact_state_id": "c"}
        )
        satisfied, unevidenced = bootstrap_satisfied(
            issues,
            closure_root=Path("/accepted-revision/alternate-closures"),
            accepted=accepted,
        )
        self.assertEqual(satisfied, frozenset())
        self.assertEqual(unevidenced, (1,))

    def test_nested_closure_root_is_not_reinterpreted_as_the_canonical_root(self) -> None:
        issues = _records({1: []}, closed={1})
        accepted = self._accepted(
            {"atom_id": "1", "logical_change_id": "k", "exact_state_id": "c"}
        )
        satisfied, unevidenced = bootstrap_satisfied(
            issues,
            closure_root=Path("nested/artifacts/atoms"),
            accepted=accepted,
        )
        self.assertEqual(satisfied, frozenset())
        self.assertEqual(unevidenced, (1,))

    def test_verifier_artifact_digest_is_checked_at_accepted_revision(self) -> None:
        schema = {
            "required": ["atom_id", "logical_change_id", "exact_state_id", "verifiers"],
            "properties": {
                "logical_change_id": {"minLength": 1},
                "verifiers": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["artifact_path", "artifact_sha256"],
                        "properties": {
                            "artifact_path": {"type": "string"},
                            "artifact_sha256": {"type": "string"},
                        },
                    },
                },
            },
        }
        payload = {
            "atom_id": "1",
            "logical_change_id": self.LOGICAL,
            "exact_state_id": self.EXACT,
            "verifiers": [
                {"artifact_path": "artifacts/atoms/1/verifiers/check.log", "artifact_sha256": "bad"}
            ],
        }
        accepted = self._accepted(
            payload,
            schema=schema,
            extra_files={"artifacts/atoms/1/verifiers/check.log": b"actual"},
        )
        issues = _records({1: []}, closed={1})
        satisfied, unevidenced = bootstrap_satisfied(
            issues,
            closure_root=Path("/accepted-revision/artifacts/atoms"),
            accepted=accepted,
        )
        self.assertEqual(satisfied, frozenset())
        self.assertEqual(unevidenced, (1,))

    def test_exact_accepted_atom_closure_and_digest_satisfy(self) -> None:
        artifact = evidence_header(self.EXACT, "cargo test --locked") + b"actual\n"
        schema = {
            "required": ["atom_id", "logical_change_id", "exact_state_id", "verifiers"],
            "properties": {
                "logical_change_id": {"minLength": 1},
                "verifiers": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["artifact_path", "artifact_sha256"],
                        "properties": {
                            "artifact_path": {"type": "string"},
                            "artifact_sha256": {"type": "string"},
                        },
                    },
                },
            },
        }
        payload = {
            "atom_id": "1",
            "logical_change_id": self.LOGICAL,
            "exact_state_id": self.EXACT,
            "verifiers": [
                {
                    "artifact_path": "artifacts/atoms/1/verifiers/check.log",
                    "artifact_sha256": hashlib.sha256(artifact).hexdigest(),
                    "verifier_id": "check",
                    "command": "cargo test --locked",
                    "exit_code": 0,
                    "subject_exact_state_id": self.EXACT,
                }
            ],
        }
        accepted = self._accepted(
            payload,
            schema=schema,
            extra_files={"artifacts/atoms/1/verifiers/check.log": artifact},
        )
        issues = _records({1: []}, closed={1})
        satisfied, unevidenced = bootstrap_satisfied(
            issues,
            closure_root=Path("/accepted-revision/artifacts/atoms"),
            accepted=accepted,
        )
        self.assertEqual(satisfied, frozenset({1}))
        self.assertEqual(unevidenced, ())

    def test_adversarial_verifier_records_do_not_satisfy_readiness(self) -> None:
        artifact = evidence_header(self.EXACT, "cargo test --locked") + b"actual\n"
        schema = {
            "required": ["atom_id", "logical_change_id", "exact_state_id", "verifiers"],
            "properties": {
                "logical_change_id": {"minLength": 1},
                "verifiers": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["artifact_path", "artifact_sha256"],
                        "properties": {
                            "artifact_path": {"type": "string"},
                            "artifact_sha256": {"type": "string"},
                        },
                    },
                },
            },
        }
        verifier = {
            "verifier_id": "check",
            "command": "cargo test --locked",
            "exit_code": 0,
            "artifact_path": "artifacts/atoms/1/verifiers/check.log",
            "artifact_sha256": hashlib.sha256(artifact).hexdigest(),
        }
        cases = []
        for field, value in (
            ("verifier_id", ""),
            ("verifier_id", "../check"),
            ("artifact_path", "README.md"),
            ("artifact_path", "verifiers/check.log"),
            ("artifact_path", "artifacts/atoms/1/closure.json"),
            ("exit_code", 1),
            ("command", ""),
            ("artifact_sha256", ""),
        ):
            invalid = dict(verifier)
            invalid[field] = value
            cases.append([invalid])
        cases.append([dict(verifier), dict(verifier)])
        for verifiers in cases:
            with self.subTest(verifiers=verifiers):
                payload = {
                    "atom_id": "1",
                    "logical_change_id": self.LOGICAL,
                    "exact_state_id": self.EXACT,
                    "verifiers": verifiers,
                }
                accepted = self._accepted(
                    payload,
                    schema=schema,
                    extra_files={
                        "artifacts/atoms/1/verifiers/check.log": artifact,
                        "nested/artifacts/atoms/1/verifiers/check.log": b"nested",
                    },
                )
                satisfied, unevidenced = bootstrap_satisfied(
                    _records({1: []}, closed={1}),
                    closure_root=Path("artifacts/atoms"),
                    accepted=accepted,
                )
                self.assertEqual(satisfied, frozenset())
                self.assertEqual(unevidenced, (1,))


    def test_a_validating_closure_record_satisfies(self) -> None:
        issues = _records({1: [], 2: [1]}, closed={1})
        with tempfile.TemporaryDirectory():
            satisfied, unevidenced = bootstrap_satisfied(
                issues,
                closure_root=Path("/accepted-revision/artifacts/atoms"),
                accepted=self._accepted(
                    {
                        "atom_id": "1",
                        "logical_change_id": self.LOGICAL,
                        "exact_state_id": self.EXACT,
                    }
                ),
            )
        self.assertEqual(satisfied, frozenset({1}))
        self.assertEqual(unevidenced, ())

    def test_an_incomplete_closure_record_does_not_satisfy(self) -> None:
        issues = _records({1: [], 2: [1]}, closed={1})
        with tempfile.TemporaryDirectory():
            satisfied, unevidenced = bootstrap_satisfied(
                issues,
                closure_root=Path("/accepted-revision/artifacts/atoms"),
                accepted=self._accepted(
                    {"atom_id": "1", "logical_change_id": "", "exact_state_id": "c"}
                ),
            )
        self.assertEqual(satisfied, frozenset())
        self.assertEqual(unevidenced, (1,))

    def test_weaker_github_rule_is_available_but_explicit(self) -> None:
        issues = _records({1: [], 2: [1]}, closed={1})
        satisfied, unevidenced = bootstrap_satisfied(issues, closure_root=None)
        self.assertEqual(satisfied, frozenset({1}))
        self.assertEqual(unevidenced, ())

    def test_valid_record_fixture_matches_nested_schema(self) -> None:
        schema = {
            "type": "object",
            "additionalProperties": False,
            "required": ["record_format", "actor", "verifiers", "closed_at"],
            "properties": {
                "record_format": {"const": "gordian-closure-v1"},
                "actor": {
                    "type": "object",
                    "additionalProperties": False,
                    "required": ["id", "kind"],
                    "properties": {
                        "id": {"type": "string", "pattern": r"^agent/[a-z]+$"},
                        "kind": {"enum": ["agent", "human"]},
                    },
                },
                "verifiers": {
                    "type": "array",
                    "minItems": 1,
                    "items": {"type": "string"},
                },
                "closed_at": {"type": "string", "format": "date-time"},
            },
        }
        record = {
            "record_format": "gordian-closure-v1",
            "actor": {"id": "agent/rust", "kind": "agent"},
            "verifiers": ["cargo test"],
            "closed_at": "2026-08-31T12:00:00Z",
        }
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "closure.json"
            path.write_text(json.dumps(record), encoding="utf-8")
            self.assertTrue(closure_record_valid(path, schema))

    def test_invalid_nested_type_pattern_enum_extra_and_min_items_fail(self) -> None:
        schema = {
            "type": "object",
            "additionalProperties": False,
            "required": ["actor", "verifiers"],
            "properties": {
                "actor": {
                    "type": "object",
                    "required": ["id", "kind"],
                    "properties": {
                        "id": {"type": "string", "pattern": r"^agent/[a-z]+$"},
                        "kind": {"enum": ["agent", "human"]},
                    },
                },
                "verifiers": {"type": "array", "minItems": 1, "items": {"type": "string"}},
            },
        }
        cases = (
            {"actor": {"id": 7, "kind": "agent"}, "verifiers": ["ok"]},
            {"actor": {"id": "worker/rust", "kind": "agent"}, "verifiers": ["ok"]},
            {"actor": {"id": "agent/rust", "kind": "robot"}, "verifiers": ["ok"]},
            {"actor": {"id": "agent/rust", "kind": "agent"}, "verifiers": []},
            {"actor": {"id": "agent/rust", "kind": "agent"}, "verifiers": ["ok"], "extra": 1},
        )
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "closure.json"
            for record in cases:
                path.write_text(json.dumps(record), encoding="utf-8")
                self.assertFalse(closure_record_valid(path, schema), record)

    def test_invalid_date_time_and_missing_schema_fail_closed(self) -> None:
        schema = {
            "type": "object",
            "properties": {"closed_at": {"type": "string", "format": "date-time"}},
        }
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "closure.json"
            path.write_text(json.dumps({"closed_at": "2026-08-31 12:00:00"}), encoding="utf-8")
            self.assertFalse(closure_record_valid(path, schema))
            path.write_text(json.dumps({}), encoding="utf-8")
            self.assertFalse(closure_record_valid(path, None))
            self.assertFalse(closure_record_valid(path, {}))

    def test_required_empty_arrays_are_allowed_without_min_items(self) -> None:
        schema = {
            "type": "object",
            "required": ["benchmarks", "known_limitations"],
            "properties": {
                "benchmarks": {"type": "array"},
                "known_limitations": {"type": "array", "items": {"type": "string"}},
            },
        }
        record = {"benchmarks": [], "known_limitations": []}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "closure.json"
            path.write_text(json.dumps(record), encoding="utf-8")
            self.assertTrue(closure_record_valid(path, schema))
            schema["properties"]["benchmarks"]["minItems"] = 1
            self.assertFalse(closure_record_valid(path, schema))


class AcceptedRevisionResolutionTests(unittest.TestCase):
    def test_root_is_resolved_once_from_invocation_and_trunk_uses_that_root(self) -> None:
        invocation = Path("/workspace/repository/nested").resolve()
        root = Path("/workspace/repository").resolve()
        root_result = subprocess.CompletedProcess(
            ["jj", "root", "--ignore-working-copy"], 0, "/workspace/repository\n", ""
        )
        revision_result = subprocess.CompletedProcess(
            ["jj", "log"], 0, "commit-accepted\nchange-accepted\n", ""
        )
        with patch(
            "gordian_orchestration.derive_status.subprocess.run",
            side_effect=[root_result, revision_result],
        ) as run:
            accepted = accepted_revision(invocation)

        self.assertEqual(accepted.repository_root, root)
        self.assertEqual(accepted.commit_id, "commit-accepted")
        self.assertEqual(accepted.change_id, "change-accepted")
        self.assertEqual(
            run.call_args_list[0],
            call(
                ["jj", "root", "--ignore-working-copy"],
                cwd=invocation,
                check=False,
                capture_output=True,
                text=True,
                timeout=30,
            ),
        )
        self.assertEqual(run.call_args_list[1].kwargs["cwd"], root)
        self.assertEqual(run.call_args_list[1].args[0][0:4], ["jj", "log", "-r", "trunk()"])
        self.assertIn("--ignore-working-copy", run.call_args_list[1].args[0])

    def test_accepted_revision_reads_are_bound_to_fixed_root_and_commit(self) -> None:
        root = Path("/workspace/repository").resolve()
        accepted = AcceptedRevision("commit-accepted", "change-accepted", root)
        result = subprocess.CompletedProcess(
            ["jj", "file", "show"], 0, b"canonical schema", b""
        )
        with patch(
            "gordian_orchestration.derive_status.subprocess.run", return_value=result
        ) as run:
            self.assertEqual(
                accepted.read("artifacts/schema/closure-record.schema.json"), b"canonical schema"
            )
        self.assertEqual(run.call_args.kwargs["cwd"], root)
        self.assertEqual(
            run.call_args.args[0],
            [
                "jj",
                "file",
                "show",
                "-r",
                "commit-accepted",
                "--",
                "artifacts/schema/closure-record.schema.json",
            ],
        )


class SnapshotTests(unittest.TestCase):
    def _envelope(self, rows):
        return {
            "record_format": "gordian-atom-registry-v1",
            "generated_at": "2026-08-31T00:00:00Z",
            "source_change_id": "change-accepted",
            "source_commit_id": "commit-accepted",
            "tool_versions": {"gh": "gh test", "jj": "jj test"},
            "source": "GitHub issue bodies and native blockedBy connections",
            "repository": "kmosoti/gordian",
            "coherent": True,
            "issues": [
                {
                    "number": row["number"],
                    "title": row.get("title", ""),
                    "state": row.get("state", "OPEN"),
                    "body": row.get("body", ""),
                    "url": row.get("url", ""),
                    "milestone": row.get("milestone"),
                    "labels": row.get("labels", []),
                    "blockedBy": row.get("blockedBy", []),
                    "assignees": row.get("assignees", []),
                }
                for row in rows
            ],
        }

    def test_snapshot_edges_are_read_from_blocked_by_node_lists(self) -> None:
        payload = [
            {"number": 2, "title": "child", "state": "OPEN", "blockedBy": [1]},
            {"number": 1, "title": "root", "state": "CLOSED", "blockedBy": []},
        ]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "issues.json"
            path.write_text(json.dumps(self._envelope(payload)), encoding="utf-8")
            records = load_snapshot(path)
        self.assertEqual([record.number for record in records], [1, 2])
        self.assertEqual(records[1].blocked_by, (1,))
        self.assertFalse(records[0].is_open)
        self.assertEqual(records[0].assignees, ())

    def test_accepted_gordian_atoms_v1_snapshot_migrates_deterministically(self) -> None:
        legacy = {
            "snapshot_format": "gordian.atoms.v1",
            "generated_by": "scripts/snapshot-atoms.sh",
            "generated_at": "2026-08-31T15:06:21Z",
            "source": "https://github.com/kmosoti/gordian/issues",
            "source_commit": "0ef3b53b5d1b5c5f6c28487088c1f6fb4aa07d2a",
            "atom_count": 2,
            "edge_count": 1,
            "issues": [
                {
                    "number": 1,
                    "title": "root",
                    "state": "OPEN",
                    "milestone": "Initiative",
                    "labels": [],
                    "blocked_by": [],
                    "body": "body",
                },
                {
                    "number": 2,
                    "title": "child",
                    "state": "OPEN",
                    "milestone": "Initiative",
                    "labels": [],
                    "blocked_by": [1],
                    "body": "body",
                },
            ],
        }
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "issues.json"
            path.write_text(json.dumps(legacy), encoding="utf-8")
            first = load_snapshot(path)
            second = load_snapshot(path)
        self.assertEqual(first, second)
        self.assertEqual(first[1].blocked_by, (1,))
        self.assertEqual(first[0].url, "https://github.com/kmosoti/gordian/issues/1")

    def test_bare_list_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "issues.json"
            path.write_text(json.dumps([]), encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "strict registry snapshot envelope"):
                load_snapshot(path)

    def test_snapshot_requires_coherent_repository_source_and_complete_rows(self) -> None:
        row = {"number": 1, "title": "root", "state": "OPEN", "blockedBy": []}
        base = self._envelope([row])
        cases = (
            {**base, "coherent": False},
            {**base, "repository": "other/repo"},
            {**base, "source_commit_id": "unknown"},
            {**base, "issues": [{**base["issues"][0], "assignees": None}]},
            {**base, "issues": [{**base["issues"][0], "blockedBy": [{"number": 2}]}]},
            {**base, "issues": [base["issues"][0], base["issues"][0]]},
        )
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "issues.json"
            for payload in cases:
                path.write_text(json.dumps(payload), encoding="utf-8")
                with self.assertRaises(RuntimeError):
                    load_snapshot(path)

    def test_closed_duplicate_is_excluded_but_open_duplicate_fails(self) -> None:
        row = {"number": 1, "title": "duplicate", "state": "CLOSED", "labels": ["duplicate"]}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "issues.json"
            path.write_text(json.dumps(self._envelope([row])), encoding="utf-8")
            self.assertEqual(load_snapshot(path), ())
            row["state"] = "OPEN"
            path.write_text(json.dumps(self._envelope([row])), encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "duplicate-labeled"):
                load_snapshot(path)


class FetchTests(unittest.TestCase):
    def _page(self, nodes, *, has_next=False, cursor="", total_count=-1):
        page = {"pageInfo": {"hasNextPage": has_next, "endCursor": cursor}, "nodes": nodes}
        page["totalCount"] = len(nodes) if total_count == -1 else total_count
        return page

    def _labels(self, names=()):
        return {
            "totalCount": len(names),
            "pageInfo": {"hasNextPage": False, "endCursor": None},
            "nodes": [{"name": name} for name in names],
        }

    def _assignees(self, names=()):
        return self._page([{"login": name} for name in names])

    def _issue(self, number, *, blocked=None, assignees=()):
        return {
            "number": number,
            "title": f"Atom {number}",
            "state": "OPEN",
            "labels": self._labels(("type:atom",)),
            "assignees": self._assignees(assignees),
            "blockedBy": blocked if blocked is not None else self._page([]),
        }

    def _project_page(self, *, nodes, total, has_next=False, cursor="", project_id="PVT_1"):
        return {
            "user": {
                "projectV2": {
                    "id": project_id,
                    "fields": {"nodes": []},
                    "items": {
                        "totalCount": total,
                        "nodes": nodes,
                        "pageInfo": {"hasNextPage": has_next, "endCursor": cursor},
                    },
                }
            }
        }

    def test_live_assignee_lock_is_loaded(self) -> None:
        response = {"repository": {"issues": self._page([self._issue(1, assignees=("worker",))])}}
        with patch("gordian_orchestration.derive_status.graphql", return_value=response):
            records = fetch_issues("kmosoti", "gordian")
        self.assertEqual(records[0].assignees, ("worker",))

    def test_issue_pagination_rejects_empty_continuation_cursor(self) -> None:
        response = {
            "repository": {
                "issues": self._page(
                    [self._issue(1)], has_next=True, cursor="", total_count=2
                )
            }
        }
        with (
            patch("gordian_orchestration.derive_status.graphql", return_value=response),
            self.assertRaisesRegex(RuntimeError, "repository issues.*cursor did not advance"),
        ):
            fetch_issues("kmosoti", "gordian")

    def test_issue_pagination_rejects_total_count_change(self) -> None:
        responses = [
            {
                "repository": {
                    "issues": self._page(
                        [self._issue(1)], has_next=True, cursor="next", total_count=2
                    )
                }
            },
            {"repository": {"issues": self._page([self._issue(2)], total_count=3)}},
        ]
        with (
            patch("gordian_orchestration.derive_status.graphql", side_effect=responses),
            self.assertRaisesRegex(RuntimeError, "issues totalCount changed"),
        ):
            fetch_issues("kmosoti", "gordian")

    def test_issue_pagination_rejects_incomplete_terminal_read(self) -> None:
        response = {
            "repository": {"issues": self._page([self._issue(1)], total_count=2)}
        }
        with (
            patch("gordian_orchestration.derive_status.graphql", return_value=response),
            self.assertRaisesRegex(RuntimeError, "retrieved 1 nodes.*reports 2"),
        ):
            fetch_issues("kmosoti", "gordian")

    def test_blocked_by_pagination_rejects_repeated_cursor(self) -> None:
        initial = self._page(
            [{"number": 1}], has_next=True, cursor="blocked-next", total_count=3
        )
        responses = [
            {"repository": {"issues": self._page([self._issue(3, blocked=initial)])}},
            {
                "repository": {
                    "issue": {
                        "blockedBy": self._page(
                            [{"number": 2}],
                            has_next=True,
                            cursor="blocked-next",
                            total_count=3,
                        )
                    }
                }
            },
        ]
        with (
            patch("gordian_orchestration.derive_status.graphql", side_effect=responses),
            self.assertRaisesRegex(RuntimeError, "#3 blockedBy.*cursor did not advance"),
        ):
            fetch_issues("kmosoti", "gordian")

    def test_blocked_by_pagination_rejects_total_count_change(self) -> None:
        initial = self._page(
            [{"number": 1}], has_next=True, cursor="blocked-next", total_count=2
        )
        responses = [
            {"repository": {"issues": self._page([self._issue(3, blocked=initial)])}},
            {
                "repository": {
                    "issue": {
                        "blockedBy": self._page([{"number": 2}], total_count=3)
                    }
                }
            },
        ]
        with (
            patch("gordian_orchestration.derive_status.graphql", side_effect=responses),
            self.assertRaisesRegex(RuntimeError, "blockedBy totalCount changed"),
        ):
            fetch_issues("kmosoti", "gordian")

    def test_incomplete_assignee_read_is_rejected(self) -> None:
        issue = self._issue(1)
        issue["assignees"] = self._page([{"login": "worker"}], total_count=2)
        response = {"repository": {"issues": self._page([issue])}}
        with (
            patch("gordian_orchestration.derive_status.graphql", return_value=response),
            self.assertRaisesRegex(RuntimeError, "retrieved 1 assignees.*reports 2"),
        ):
            fetch_issues("kmosoti", "gordian")

    def test_assignee_pagination_is_followed_to_completion(self) -> None:
        issue = self._issue(1)
        issue["assignees"] = self._page(
            [{"login": "worker-a"}],
            has_next=True,
            cursor="assignee-next",
            total_count=2,
        )
        responses = [
            {"repository": {"issues": self._page([issue])}},
            {
                "repository": {
                    "issue": {
                        "assignees": self._page(
                            [{"login": "worker-b"}], total_count=2
                        )
                    }
                }
            },
        ]
        with patch(
            "gordian_orchestration.derive_status.graphql", side_effect=responses
        ):
            records = fetch_issues("kmosoti", "gordian")
        self.assertEqual(records[0].assignees, ("worker-a", "worker-b"))

    def test_project_item_pagination_rejects_empty_cursor(self) -> None:
        response = self._project_page(nodes=[], total=1, has_next=True, cursor="")
        with (
            patch("gordian_orchestration.derive_status.graphql", return_value=response),
            self.assertRaisesRegex(RuntimeError, "Project items.*cursor did not advance"),
        ):
            fetch_board("kmosoti", 9)

    def test_project_item_pagination_rejects_total_count_change(self) -> None:
        responses = [
            self._project_page(nodes=[], total=1, has_next=True, cursor="next"),
            self._project_page(nodes=[], total=2),
        ]
        with (
            patch("gordian_orchestration.derive_status.graphql", side_effect=responses),
            self.assertRaisesRegex(RuntimeError, "Project item totalCount changed"),
        ):
            fetch_board("kmosoti", 9)

    def test_project_item_pagination_rejects_incomplete_terminal_read(self) -> None:
        response = self._project_page(nodes=[], total=1)
        with (
            patch("gordian_orchestration.derive_status.graphql", return_value=response),
            self.assertRaisesRegex(RuntimeError, "retrieved 0 nodes.*reports 1"),
        ):
            fetch_board("kmosoti", 9)

    def test_project_item_pagination_rejects_project_identity_change(self) -> None:
        responses = [
            self._project_page(nodes=[], total=0, has_next=True, cursor="next"),
            self._project_page(nodes=[], total=0, project_id="PVT_other"),
        ]
        with (
            patch("gordian_orchestration.derive_status.graphql", side_effect=responses),
            self.assertRaisesRegex(RuntimeError, "Project identity changed"),
        ):
            fetch_board("kmosoti", 9)

    def test_blocked_by_pagination_is_followed_to_completion(self) -> None:
        responses = [
            {
                "repository": {
                    "issues": self._page(
                        [
                            self._issue(1),
                            self._issue(2),
                            {
                                "number": 3,
                                "title": "wide",
                                "state": "OPEN",
                                "labels": self._labels(("type:atom",)),
                                "assignees": self._assignees(),
                                "blockedBy": self._page(
                                    [{"number": 1}],
                                    has_next=True,
                                    cursor="c1",
                                    total_count=2,
                                ),
                            }
                        ]
                    )
                }
            },
            {
                "repository": {
                    "issue": {"blockedBy": self._page([{"number": 2}], total_count=2)}
                }
            },
        ]
        with patch(
            "gordian_orchestration.derive_status.graphql", side_effect=responses
        ) as query:
            records = fetch_issues("kmosoti", "gordian")
        self.assertEqual(
            next(record for record in records if record.number == 3).blocked_by,
            (1, 2),
        )
        self.assertEqual(query.call_count, 2)

    def test_live_registry_rejects_unknown_blocker(self) -> None:
        response = {
            "repository": {
                "issues": self._page([self._issue(3, blocked=self._page([{"number": 99}]))])
            }
        }
        with (
            patch("gordian_orchestration.derive_status.graphql", return_value=response),
            self.assertRaisesRegex(RuntimeError, "absent from the executable Atom registry"),
        ):
            fetch_issues("kmosoti", "gordian")

    def test_live_registry_rejects_a_cycle(self) -> None:
        response = {
            "repository": {
                "issues": self._page(
                    [
                        self._issue(1, blocked=self._page([{"number": 2}])),
                        self._issue(2, blocked=self._page([{"number": 1}])),
                    ]
                )
            }
        }
        with (
            patch("gordian_orchestration.derive_status.graphql", return_value=response),
            self.assertRaisesRegex(RuntimeError, "blocked-by cycle"),
        ):
            fetch_issues("kmosoti", "gordian")

    def test_incomplete_pagination_is_an_error_not_a_silent_short_graph(self) -> None:
        response = {
            "repository": {
                "issues": self._page(
                    [
                        {
                            "number": 3,
                            "title": "wide",
                            "state": "OPEN",
                            "labels": self._labels(("type:atom",)),
                            "assignees": self._assignees(),
                            "blockedBy": self._page([{"number": 1}], total_count=2),
                        }
                    ]
                )
            }
        }
        with (
            patch("gordian_orchestration.derive_status.graphql", return_value=response),
            self.assertRaises(RuntimeError) as raised,
        ):
            fetch_issues("kmosoti", "gordian")
        self.assertIn("pagination is incomplete", str(raised.exception))

    def test_issue_dependencies_summary_is_never_requested(self) -> None:
        # The summary's blocking counter is wrong for #11, #18 and #44. Native
        # `blockedBy` nodes and their connection count are the entire edge source.
        self.assertNotIn("issueDependenciesSummary", _ISSUES_QUERY)
        self.assertIn("blockedBy(first:100)", _ISSUES_QUERY)
        self.assertIn("totalCount", _ISSUES_QUERY)
        self.assertNotIn("issueDependenciesSummary", _BLOCKED_BY_PAGE_QUERY)

    def test_closed_duplicate_is_excluded_from_the_atom_corpus(self) -> None:
        response = {
            "repository": {
                "issues": self._page(
                    [
                        {
                            "number": 78,
                            "title": "duplicate",
                            "state": "CLOSED",
                            "labels": self._labels(("type:atom", "duplicate")),
                            "assignees": self._assignees(),
                            "blockedBy": self._page([], total_count=0),
                        }
                    ]
                )
            }
        }
        with patch("gordian_orchestration.derive_status.graphql", return_value=response):
            self.assertEqual(fetch_issues("kmosoti", "gordian"), ())

    def test_open_duplicate_is_a_reconciliation_error(self) -> None:
        response = {
            "repository": {
                "issues": self._page(
                    [
                        {
                            "number": 78,
                            "title": "duplicate",
                            "state": "OPEN",
                            "labels": self._labels(("type:atom", "duplicate")),
                            "assignees": self._assignees(),
                            "blockedBy": self._page([], total_count=0),
                        }
                    ]
                )
            }
        }
        with (
            patch("gordian_orchestration.derive_status.graphql", return_value=response),
            self.assertRaisesRegex(RuntimeError, "duplicate-labeled issue is still open"),
        ):
            fetch_issues("kmosoti", "gordian")

    def test_duplicate_project_items_for_one_issue_are_rejected(self) -> None:
        response = {
            "user": {
                "projectV2": {
                    "id": "PVT_1",
                    "fields": {"nodes": []},
                    "items": {
                        "totalCount": 2,
                        "nodes": [
                            {
                                "id": "item-a",
                                "content": {"number": 7},
                                "fieldValues": {"nodes": []},
                            },
                            {
                                "id": "item-b",
                                "content": {"number": 7},
                                "fieldValues": {"nodes": []},
                            },
                        ],
                        "pageInfo": {"hasNextPage": False, "endCursor": ""},
                    },
                }
            }
        }
        with (
            patch("gordian_orchestration.derive_status.graphql", return_value=response),
            self.assertRaisesRegex(RuntimeError, "duplicate item for issue #7"),
        ):
            fetch_board("kmosoti", 9)

    def test_duplicate_project_field_definitions_are_rejected(self) -> None:
        response = {
            "user": {
                "projectV2": {
                    "id": "PVT_1",
                    "fields": {
                        "nodes": [
                            {"id": "field-a", "name": "Wave", "dataType": "NUMBER"},
                            {"id": "field-b", "name": "Wave", "dataType": "NUMBER"},
                        ]
                    },
                    "items": {
                        "totalCount": 0,
                        "nodes": [],
                        "pageInfo": {"hasNextPage": False, "endCursor": ""},
                    },
                }
            }
        }
        with (
            patch("gordian_orchestration.derive_status.graphql", return_value=response),
            self.assertRaisesRegex(RuntimeError, "conflicting field definition 'Wave'"),
        ):
            fetch_board("kmosoti", 9)

    def test_repeated_project_fields_across_item_pages_are_accepted(self) -> None:
        fields = {
            "nodes": [{"id": "field-wave", "name": "Wave", "dataType": "NUMBER"}]
        }
        responses = [
            {
                "user": {
                    "projectV2": {
                        "id": "PVT_1",
                        "fields": fields,
                        "items": {
                            "totalCount": 2,
                            "nodes": [
                                {
                                    "id": "item-a",
                                    "content": {"number": 7},
                                    "fieldValues": {"nodes": []},
                                }
                            ],
                            "pageInfo": {"hasNextPage": True, "endCursor": "next"},
                        },
                    }
                }
            },
            {
                "user": {
                    "projectV2": {
                        "id": "PVT_1",
                        "fields": fields,
                        "items": {
                            "totalCount": 2,
                            "nodes": [
                                {
                                    "id": "item-b",
                                    "content": {"number": 8},
                                    "fieldValues": {"nodes": []},
                                }
                            ],
                            "pageInfo": {"hasNextPage": False, "endCursor": ""},
                        },
                    }
                }
            },
        ]
        with patch(
            "gordian_orchestration.derive_status.graphql", side_effect=responses
        ):
            board = fetch_board("kmosoti", 9)
        self.assertEqual(set(board.items), {7, 8})
        self.assertEqual(board.fields["Wave"].field_id, "field-wave")

    def test_conflicting_project_field_definition_across_pages_is_rejected(self) -> None:
        responses = [
            {
                "user": {
                    "projectV2": {
                        "id": "PVT_1",
                        "fields": {
                            "nodes": [
                                {"id": "field-wave", "name": "Wave", "dataType": "NUMBER"}
                            ]
                        },
                        "items": {
                            "totalCount": 0,
                            "nodes": [],
                            "pageInfo": {"hasNextPage": True, "endCursor": "next"},
                        },
                    }
                }
            },
            {
                "user": {
                    "projectV2": {
                        "id": "PVT_1",
                        "fields": {
                            "nodes": [
                                {"id": "field-other", "name": "Wave", "dataType": "NUMBER"}
                            ]
                        },
                        "items": {
                            "totalCount": 0,
                            "nodes": [],
                            "pageInfo": {"hasNextPage": False, "endCursor": ""},
                        },
                    }
                }
            },
        ]
        with (
            patch(
                "gordian_orchestration.derive_status.graphql", side_effect=responses
            ),
            self.assertRaisesRegex(RuntimeError, "conflicting field definition 'Wave'"),
        ):
            fetch_board("kmosoti", 9)


class BoardPlanTests(unittest.TestCase):
    def _board(self, items: dict[int, BoardItem]) -> Board:
        return Board(
            project_id="PVT_1",
            fields={
                "Status": ProjectField(
                    field_id="f_status",
                    name="Status",
                    data_type="SINGLE_SELECT",
                    options=(("Blocked", "o_blocked"), ("Ready", "o_ready")),
                ),
                "Wave": ProjectField(field_id="f_wave", name="Wave", data_type="NUMBER"),
                "Fan In": ProjectField(field_id="f_in", name="Fan In", data_type="NUMBER"),
                "Fan Out": ProjectField(field_id="f_out", name="Fan Out", data_type="NUMBER"),
            },
            items=items,
        )

    def test_a_second_run_reports_zero_changes(self) -> None:
        rows = derive(_records({1: [], 2: [1]}))
        board = self._board(
            {
                1: BoardItem(item_id="i1", number=1),
                2: BoardItem(item_id="i2", number=2),
            }
        )
        changes, absent = plan_changes(rows, board)
        self.assertEqual(absent, ())
        self.assertTrue(changes)

        settled = dict(board.items)
        for change in changes:
            item = settled[change.number]
            settled[change.number] = BoardItem(
                item_id=item.item_id,
                number=item.number,
                status=change.desired if change.field_name == "Status" else item.status,
                wave=change.desired if change.field_name == "Wave" else item.wave,
                fan_in=change.desired if change.field_name == "Fan In" else item.fan_in,
                fan_out=change.desired if change.field_name == "Fan Out" else item.fan_out,
            )
        again, _ = plan_changes(rows, self._board(settled))
        self.assertEqual(again, ())

    def test_missing_derived_project_field_is_rejected_before_planning(self) -> None:
        rows = derive(_records({1: []}))
        board = self._board({1: BoardItem(item_id="i1", number=1)})
        board.fields.pop("Fan Out")
        with self.assertRaisesRegex(
            RuntimeError, "missing required derived field definitions.*Fan Out"
        ):
            plan_changes(rows, board)

    def test_a_human_status_produces_no_status_change(self) -> None:
        board = self._board({1: BoardItem(item_id="i1", number=1, status="In Progress")})
        rows = derive(
            _records({1: []}),
            board_status={1: "In Progress"},
        )
        changes, _ = plan_changes(rows, board)
        self.assertNotIn("Status", [change.field_name for change in changes])
        self.assertEqual(
            sorted(change.field_name for change in changes), ["Fan In", "Fan Out", "Wave"]
        )

    def test_issues_absent_from_the_board_are_reported_not_invented(self) -> None:
        rows = derive(_records({1: [], 2: [1]}))
        changes, absent = plan_changes(rows, self._board({1: BoardItem(item_id="i1", number=1)}))
        self.assertEqual(absent, (2,))
        self.assertTrue(all(change.number == 1 for change in changes))

    def test_apply_change_picks_the_mutation_matching_the_field_type(self) -> None:
        board = self._board({1: BoardItem(item_id="i1", number=1)})
        rows = derive(_records({1: []}))
        changes, _ = plan_changes(rows, board)
        with patch("gordian_orchestration.derive_status.graphql") as query:
            for change in changes:
                apply_change(board, change)
        documents = [call.args[0] for call in query.call_args_list]
        variables = [call.args[1] for call in query.call_args_list]
        self.assertTrue(any("singleSelectOptionId" in document for document in documents))
        self.assertTrue(any("value:{number:$value}" in document for document in documents))
        self.assertIn("o_ready", [payload.get("option") for payload in variables])


class CommandTests(unittest.TestCase):
    """End-to-end CLI exercise over a snapshot file. No token, no network."""

    SNAPSHOT = [
        {"number": 1, "title": "root", "state": "OPEN", "blockedBy": []},
        {"number": 2, "title": "child", "state": "OPEN", "blockedBy": [1]},
        {"number": 3, "title": "done", "state": "CLOSED", "blockedBy": []},
    ]

    @staticmethod
    def _envelope(rows):
        return {
            "record_format": "gordian-atom-registry-v1",
            "generated_at": "2026-08-31T00:00:00Z",
            "source_change_id": "change-accepted",
            "source_commit_id": "commit-accepted",
            "tool_versions": {"gh": "gh test", "jj": "jj test"},
            "source": "GitHub issue bodies and native blockedBy connections",
            "repository": "kmosoti/gordian",
            "coherent": True,
            "issues": [
                {
                    "number": row["number"],
                    "title": row.get("title", ""),
                    "state": row.get("state", "OPEN"),
                    "body": row.get("body", ""),
                    "url": row.get("url", ""),
                    "milestone": row.get("milestone"),
                    "labels": row.get("labels", []),
                    "blockedBy": row.get("blockedBy", []),
                    "assignees": row.get("assignees", []),
                }
                for row in rows
            ],
        }

    def _run(
        self,
        argv: list[str],
        *,
        options: tuple[str, ...] = (),
        include_snapshot: bool = True,
    ) -> tuple[int, str, str]:
        out, err = io.StringIO(), io.StringIO()
        with (
            tempfile.TemporaryDirectory() as directory,
            redirect_stdout(out),
            redirect_stderr(err),
        ):
            prefix = ["--closure-root", str(Path(directory) / "atoms")]
            if include_snapshot:
                snapshot = Path(directory) / "issues.json"
                snapshot.write_text(
                    json.dumps(self._envelope(self.SNAPSHOT)),
                    encoding="utf-8",
                )
                prefix = ["--snapshot", str(snapshot), *prefix]
            code = main([*prefix, *options, *argv])
        return code, out.getvalue(), err.getvalue()

    def test_ready_exits_non_zero_on_an_unevidenced_closure(self) -> None:
        code, stdout, stderr = self._run(["ready", "--all", "--inspection"])
        self.assertEqual(code, 1)
        self.assertIn("#1", stdout)
        self.assertIn("#2", stdout)
        self.assertIn("closed without a validating closure record", stderr)

    def test_snapshot_ready_is_fail_closed_and_never_emits_dispatch_rows(self) -> None:
        code, stdout, stderr = self._run(["ready"])
        self.assertEqual(code, 2)
        self.assertNotIn("#1", stdout)
        self.assertIn("inspection-only", stderr)

    def test_snapshot_inspection_report_is_explicitly_non_dispatching(self) -> None:
        self.SNAPSHOT = [
            {"number": 1, "title": "root", "state": "OPEN", "blockedBy": []},
            {"number": 2, "title": "child", "state": "OPEN", "blockedBy": [1]},
        ]
        code, stdout, stderr = self._run(["ready", "--json", "--inspection"])
        payload = json.loads(stdout)
        self.assertEqual(code, 0)
        self.assertEqual(stderr, "")
        self.assertFalse(payload["dispatchable"])
        self.assertTrue(payload["inspection_only"])
        self.assertEqual([atom["number"] for atom in payload["atoms"]], [1])

    def test_claimed_snapshot_row_cannot_be_dispatch_output(self) -> None:
        self.SNAPSHOT = [
            {
                "number": 1,
                "title": "claimed",
                "state": "OPEN",
                "blockedBy": [],
                "assignees": ["worker"],
            }
        ]
        code, stdout, _ = self._run(["ready", "--json", "--all", "--inspection"])
        payload = json.loads(stdout)
        self.assertEqual(code, 0)
        self.assertFalse(payload["dispatchable"])
        self.assertEqual(payload["atoms"][0]["status"], None)

    def test_ready_all_lists_every_open_atom(self) -> None:
        _, stdout, _ = self._run(["ready", "--all", "--inspection"])
        self.assertIn("#1", stdout)
        self.assertIn("#2", stdout)
        self.assertIn("1 ready of 2 open", stdout)

    def test_ready_prints_rows_in_the_selection_order_not_by_issue_number(self) -> None:
        # #2 is a root that blocks two Atoms, #1 a root that blocks none, so the selection
        # order disagrees with ascending issue number and the ready rows lead the listing.
        self.SNAPSHOT = [
            {"number": 1, "title": "quiet root", "state": "OPEN", "blockedBy": []},
            {"number": 2, "title": "busy root", "state": "OPEN", "blockedBy": []},
            {"number": 3, "title": "child", "state": "OPEN", "blockedBy": [2]},
            {"number": 4, "title": "child", "state": "OPEN", "blockedBy": [2]},
        ]
        code, stdout, _ = self._run(["ready", "--all", "--inspection"])
        printed = [line.split()[0] for line in stdout.splitlines() if line.startswith("#")]
        self.assertEqual(code, 0)
        self.assertEqual(printed, ["#2", "#1", "#3", "#4"])

        _, ready_only, _ = self._run(["ready", "--inspection"])
        self.assertEqual(
            [line.split()[0] for line in ready_only.splitlines() if line.startswith("#")],
            ["#2", "#1"],
        )

    def test_ready_json_names_the_selection_keys(self) -> None:
        with patch(
            "gordian_orchestration.derive_status.provenance.collect",
            return_value=Provenance("t", "c", "x", {"gh": "v"}),
        ):
            _, stdout, _ = self._run(
                ["ready", "--json", "--inspection"], options=("--satisfaction", "closed")
            )
        self.assertEqual(
            json.loads(stdout)["selection_keys"], ["wave", "-fan_out", "number"]
        )

    def test_ready_json_emits_provenance_and_the_ready_set(self) -> None:
        with patch(
            "gordian_orchestration.derive_status.provenance.collect",
            return_value=Provenance("t", "c", "x", {"gh": "v"}),
        ):
            code, stdout, _ = self._run(
                ["ready", "--json", "--inspection"], options=("--satisfaction", "closed")
            )
        payload = json.loads(stdout)
        self.assertEqual(code, 0)
        self.assertEqual([atom["number"] for atom in payload["atoms"]], [1])
        self.assertEqual(payload["generated_at"], "t")
        self.assertEqual(payload["accepted_commit_id"], payload["accepted_source_commit_id"])
        self.assertEqual(payload["accepted_change_id"], payload["accepted_source_change_id"])

    def test_derive_without_apply_writes_nothing(self) -> None:
        with patch(
            "gordian_orchestration.derive_status._load_issues", return_value=()
        ) as load_issues:
            code, stdout, _ = self._run(["derive"], include_snapshot=False)
        payload = json.loads(stdout)
        self.assertEqual(code, 0)
        self.assertEqual(payload["applied"], 0)
        self.assertEqual(payload["changes"], [])
        load_issues.assert_called_once()

    def test_apply_rejects_snapshot_and_weaker_satisfaction_before_reads(self) -> None:
        with (
            patch("gordian_orchestration.derive_status._load_issues") as load_issues,
            patch("gordian_orchestration.derive_status.preflight") as check_config,
        ):
            code, _, stderr = self._run(
                ["derive", "--apply"], options=("--satisfaction", "closed")
            )
        self.assertEqual(code, 2)
        self.assertIn("derive does not support --snapshot", stderr)
        load_issues.assert_not_called()
        check_config.assert_not_called()

    def test_apply_rejects_weaker_satisfaction_before_live_reads(self) -> None:
        with (
            patch("gordian_orchestration.derive_status._load_issues") as load_issues,
            patch("gordian_orchestration.derive_status.preflight") as check_config,
        ):
            code, _, stderr = self._run(
                ["derive", "--apply"],
                options=("--satisfaction", "closed"),
                include_snapshot=False,
            )
        self.assertEqual(code, 2)
        self.assertIn("--apply requires --satisfaction closure-record", stderr)
        load_issues.assert_not_called()
        check_config.assert_not_called()

    def test_apply_preflights_before_loading_live_issues(self) -> None:
        events: list[str] = []

        def fail_board(*args, **kwargs):
            events.append("board")
            raise RuntimeError("stop after board read")

        with (
            patch(
                "gordian_orchestration.derive_status.preflight",
                side_effect=lambda **_: events.append("preflight"),
            ) as check_config,
            patch(
                "gordian_orchestration.derive_status._load_issues",
                side_effect=lambda _: (events.append("issues"), tuple())[1],
            ),
            patch(
                "gordian_orchestration.derive_status.fetch_board",
                side_effect=fail_board,
            ),
        ):
            code, _, _ = self._run(
                ["derive", "--apply"], options=(), include_snapshot=False
            )
        self.assertEqual(code, 2)
        self.assertEqual(events, ["preflight", "issues", "board"])
        check_config.assert_called_once_with(
            repository="kmosoti/gordian", project_owner="kmosoti", project_number=9
        )

    def test_apply_configuration_failure_returns_config_exit_code(self) -> None:
        with (
            patch(
                "gordian_orchestration.derive_status.preflight",
                side_effect=GitHubConfigurationError("missing token"),
            ),
            patch("gordian_orchestration.derive_status._load_issues") as load_issues,
        ):
            code, _, stderr = self._run(
                ["derive", "--apply"], options=(), include_snapshot=False
            )
        self.assertEqual(code, EX_CONFIG)
        self.assertIn("missing token", stderr)
        load_issues.assert_not_called()

    def test_derive_compare_board_rejects_snapshot_before_board_reads(self) -> None:
        board = Board(
            project_id="PVT_1",
            fields={
                "Wave": ProjectField(field_id="f_wave", name="Wave", data_type="NUMBER"),
                "Fan In": ProjectField(field_id="f_in", name="Fan In", data_type="NUMBER"),
                "Fan Out": ProjectField(
                    field_id="f_out", name="Fan Out", data_type="NUMBER"
                ),
                "Status": ProjectField(
                    field_id="f_status",
                    name="Status",
                    data_type="SINGLE_SELECT",
                    options=(("Ready", "o_ready"), ("Blocked", "o_blocked")),
                ),
            },
            items={1: BoardItem(item_id="i1", number=1, wave=0)},
        )
        with patch(
            "gordian_orchestration.derive_status.fetch_board", return_value=board
        ) as fetch:
            code, stdout, stderr = self._run(["derive", "--compare-board"])
        self.assertEqual(code, 2)
        self.assertEqual(stdout, "")
        self.assertIn("derive does not support --snapshot", stderr)
        fetch.assert_not_called()

    def test_a_cycle_is_reported_as_an_error_exit(self) -> None:
        out, err = io.StringIO(), io.StringIO()
        with (
            tempfile.TemporaryDirectory() as directory,
            redirect_stdout(out),
            redirect_stderr(err),
        ):
            snapshot = Path(directory) / "issues.json"
            snapshot.write_text(
                json.dumps(
                    CommandTests._envelope(
                        [
                            {"number": 1, "title": "a", "state": "OPEN", "blockedBy": [2]},
                            {"number": 2, "title": "b", "state": "OPEN", "blockedBy": [1]},
                        ]
                    )
                ),
                encoding="utf-8",
            )
            code = main(["--snapshot", str(snapshot), "ready"])
        self.assertEqual(code, 2)
        self.assertIn("blocked-by cycle", err.getvalue())


if __name__ == "__main__":
    unittest.main()
