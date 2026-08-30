"""Unit tests for the derived Project 9 projection.

Every test here runs against a checked-in fixture or a stubbed GraphQL callable. No test
touches the network, so the graph math is falsifiable offline and CI needs no token.
"""

from __future__ import annotations

import io
import json
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest.mock import patch

from fixture_graph import BLOCKED_BY, EXPECTED_WAVE

from gordian_orchestration.derive_status import (
    _BLOCKED_BY_PAGE_QUERY,
    _ISSUES_QUERY,
    BLOCKED,
    READY,
    Board,
    BoardItem,
    DependencyCycleError,
    IssueRecord,
    ProjectField,
    apply_change,
    bootstrap_satisfied,
    derive,
    derived_status,
    fan_in,
    fan_out,
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
        self.assertEqual(counts[70], 0)
        self.assertEqual(counts[9], 13)
        self.assertEqual(counts[68], 12)

    def test_roots_and_deepest_atom(self) -> None:
        depth = waves(normalise_graph(BLOCKED_BY))
        self.assertEqual(sorted(n for n, d in depth.items() if d == 0), [1, 70])
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

    def test_ready_set_over_the_real_graph_is_exactly_the_two_roots(self) -> None:
        rows = derive(_records())
        self.assertEqual([row.number for row in ready_set(rows)], [1, 70])
        self.assertTrue(all(row.status == BLOCKED for row in rows if row.number not in (1, 70)))

    def test_ready_set_equals_the_rows_whose_derived_status_is_ready(self) -> None:
        rows = derive(_records(closed={1}), satisfied={1})
        self.assertEqual(
            {row.number for row in ready_set(rows)},
            {row.number for row in rows if row.status == READY},
        )

    def test_satisfying_a_root_advances_the_ready_set(self) -> None:
        rows = derive(_records(closed={1}), satisfied={1})
        # #29 is blocked by #1 and #2; satisfying #1 alone does not release it. #70 is
        # printed before #2 because the selection order sorts on Wave first and #70 is a
        # root (Wave 0) while #2 sits at Wave 1.
        self.assertEqual([row.number for row in ready_set(rows)], [70, 2])
        self.assertEqual({row.number for row in ready_set(rows)}, {2, 70})

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
        self.assertEqual(rows[9].unsatisfied_blockers, (2, 3, 4, 5, 6, 7, 8, 71, 72, 73, 74, 75))
        self.assertEqual(rows[2].unsatisfied_blockers, ())

    def test_derive_carries_the_graph_quantities_onto_each_row(self) -> None:
        rows = {row.number: row for row in derive(_records())}
        self.assertEqual(rows[18].wave, EXPECTED_WAVE[18])
        self.assertEqual(rows[18].fan_out, 11)
        self.assertEqual(rows[18].fan_in, 2)
        self.assertFalse(rows[18].ready)


class BootstrapSatisfactionTests(unittest.TestCase):
    SCHEMA = {"required": ["atom", "logical_change_id", "exact_state_id"]}

    def _record(self, root: Path, number: int, payload: dict) -> None:
        path = root / str(number) / "closure.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(payload), encoding="utf-8")

    def test_closure_alone_does_not_satisfy(self) -> None:
        issues = _records({1: [], 2: [1]}, closed={1})
        with tempfile.TemporaryDirectory() as directory:
            satisfied, unevidenced = bootstrap_satisfied(
                issues, closure_root=Path(directory), schema=self.SCHEMA
            )
        self.assertEqual(satisfied, frozenset())
        self.assertEqual(unevidenced, (1,))

    def test_a_validating_closure_record_satisfies(self) -> None:
        issues = _records({1: [], 2: [1]}, closed={1})
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self._record(root, 1, {"atom": 1, "logical_change_id": "k", "exact_state_id": "c"})
            satisfied, unevidenced = bootstrap_satisfied(
                issues, closure_root=root, schema=self.SCHEMA
            )
        self.assertEqual(satisfied, frozenset({1}))
        self.assertEqual(unevidenced, ())

    def test_an_incomplete_closure_record_does_not_satisfy(self) -> None:
        issues = _records({1: [], 2: [1]}, closed={1})
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self._record(root, 1, {"atom": 1, "logical_change_id": "", "exact_state_id": "c"})
            satisfied, unevidenced = bootstrap_satisfied(
                issues, closure_root=root, schema=self.SCHEMA
            )
        self.assertEqual(satisfied, frozenset())
        self.assertEqual(unevidenced, (1,))

    def test_weaker_github_rule_is_available_but_explicit(self) -> None:
        issues = _records({1: [], 2: [1]}, closed={1})
        satisfied, unevidenced = bootstrap_satisfied(issues, closure_root=None)
        self.assertEqual(satisfied, frozenset({1}))
        self.assertEqual(unevidenced, ())


class SnapshotTests(unittest.TestCase):
    def test_snapshot_edges_are_read_from_blocked_by_node_lists(self) -> None:
        payload = [
            {"number": 2, "title": "child", "state": "OPEN", "blockedBy": [{"number": 1}]},
            {"number": 1, "title": "root", "state": "CLOSED", "blockedBy": []},
        ]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "issues.json"
            path.write_text(json.dumps(payload), encoding="utf-8")
            records = load_snapshot(path)
        self.assertEqual([record.number for record in records], [1, 2])
        self.assertEqual(records[1].blocked_by, (1,))
        self.assertFalse(records[0].is_open)


class FetchTests(unittest.TestCase):
    def _page(self, nodes, *, has_next=False, cursor=""):
        return {"pageInfo": {"hasNextPage": has_next, "endCursor": cursor}, "nodes": nodes}

    def test_blocked_by_pagination_is_followed_to_completion(self) -> None:
        responses = [
            {
                "repository": {
                    "issues": self._page(
                        [
                            {
                                "number": 3,
                                "title": "wide",
                                "state": "OPEN",
                                "issueDependenciesSummary": {"blockedBy": 2},
                                "blockedByIssues": self._page(
                                    [{"number": 1}], has_next=True, cursor="c1"
                                ),
                            }
                        ]
                    )
                }
            },
            {"repository": {"issue": {"blockedByIssues": self._page([{"number": 2}])}}},
        ]
        with patch(
            "gordian_orchestration.derive_status.graphql", side_effect=responses
        ) as query:
            records = fetch_issues("kmosoti", "gordian")
        self.assertEqual(records[0].blocked_by, (1, 2))
        self.assertEqual(query.call_count, 2)

    def test_incomplete_pagination_is_an_error_not_a_silent_short_graph(self) -> None:
        response = {
            "repository": {
                "issues": self._page(
                    [
                        {
                            "number": 3,
                            "title": "wide",
                            "state": "OPEN",
                            "issueDependenciesSummary": {"blockedBy": 2},
                            "blockedByIssues": self._page([{"number": 1}]),
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

    def test_the_wrong_blocking_counter_is_never_requested(self) -> None:
        # `issueDependenciesSummary.blocking` is wrong for #11, #18 and #44, so Fan Out is
        # counted over the edge lists and the counter is never even asked for.
        self.assertIn("issueDependenciesSummary{blockedBy}", _ISSUES_QUERY.replace(" ", ""))
        self.assertNotIn("blocking", _ISSUES_QUERY)
        self.assertNotIn("blocking", _BLOCKED_BY_PAGE_QUERY)


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

    def _run(self, argv: list[str], *, options: tuple[str, ...] = ()) -> tuple[int, str, str]:
        out, err = io.StringIO(), io.StringIO()
        with (
            tempfile.TemporaryDirectory() as directory,
            redirect_stdout(out),
            redirect_stderr(err),
        ):
            snapshot = Path(directory) / "issues.json"
            snapshot.write_text(json.dumps(self.SNAPSHOT), encoding="utf-8")
            code = main(
                [
                    "--snapshot",
                    str(snapshot),
                    "--closure-root",
                    str(Path(directory) / "atoms"),
                    *options,
                    *argv,
                ]
            )
        return code, out.getvalue(), err.getvalue()

    def test_ready_exits_non_zero_on_an_unevidenced_closure(self) -> None:
        code, stdout, stderr = self._run(["ready"])
        self.assertEqual(code, 1)
        self.assertIn("#1", stdout)
        self.assertNotIn("#2", stdout)
        self.assertIn("closed without a validating closure record: #3", stderr)

    def test_ready_all_lists_every_open_atom(self) -> None:
        _, stdout, _ = self._run(["ready", "--all"])
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
        code, stdout, _ = self._run(["ready", "--all"])
        printed = [line.split()[0] for line in stdout.splitlines() if line.startswith("#")]
        self.assertEqual(code, 0)
        self.assertEqual(printed, ["#2", "#1", "#3", "#4"])

        _, ready_only, _ = self._run(["ready"])
        self.assertEqual(
            [line.split()[0] for line in ready_only.splitlines() if line.startswith("#")],
            ["#2", "#1"],
        )

    def test_ready_json_names_the_selection_keys(self) -> None:
        with patch(
            "gordian_orchestration.derive_status.provenance.collect",
            return_value=Provenance("t", "c", "x", {"gh": "v"}),
        ):
            _, stdout, _ = self._run(["ready", "--json"], options=("--satisfaction", "closed"))
        self.assertEqual(
            json.loads(stdout)["selection_keys"], ["wave", "-fan_out", "number"]
        )

    def test_ready_json_emits_provenance_and_the_ready_set(self) -> None:
        with patch(
            "gordian_orchestration.derive_status.provenance.collect",
            return_value=Provenance("t", "c", "x", {"gh": "v"}),
        ):
            code, stdout, _ = self._run(
                ["ready", "--json"], options=("--satisfaction", "closed")
            )
        payload = json.loads(stdout)
        self.assertEqual(code, 0)
        self.assertEqual([atom["number"] for atom in payload["atoms"]], [1])
        self.assertEqual(payload["generated_at"], "t")

    def test_derive_without_apply_writes_nothing(self) -> None:
        with patch("gordian_orchestration.derive_status.graphql") as query:
            code, stdout, _ = self._run(["derive"])
        payload = json.loads(stdout)
        self.assertEqual(code, 0)
        self.assertEqual(payload["applied"], 0)
        self.assertEqual(payload["changes"], [])
        query.assert_not_called()

    def test_compare_board_flags_atoms_the_board_does_not_carry(self) -> None:
        board = Board(
            project_id="PVT_1",
            fields={"Wave": ProjectField(field_id="f_wave", name="Wave", data_type="NUMBER")},
            items={1: BoardItem(item_id="i1", number=1, wave=0)},
        )
        with patch("gordian_orchestration.derive_status.fetch_board", return_value=board):
            code, stdout, stderr = self._run(["derive", "--compare-board"])
        payload = json.loads(stdout)
        self.assertEqual(code, 1)
        self.assertEqual(payload["absent_from_board"], [2, 3])
        self.assertEqual(payload["applied"], 0)
        self.assertIn("gordian-project-sync", stderr)

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
                    [
                        {"number": 1, "title": "a", "state": "OPEN", "blockedBy": [2]},
                        {"number": 2, "title": "b", "state": "OPEN", "blockedBy": [1]},
                    ]
                ),
                encoding="utf-8",
            )
            code = main(["--snapshot", str(snapshot), "ready"])
        self.assertEqual(code, 2)
        self.assertIn("blocked-by cycle", err.getvalue())


if __name__ == "__main__":
    unittest.main()
