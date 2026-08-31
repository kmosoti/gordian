from __future__ import annotations

import io
import json
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from dataclasses import replace
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from fixture_graph import BLOCKED_BY

from gordian_orchestration.atom_registry import (
    BENCHMARK_GATE_BEGIN,
    BENCHMARK_OWNER_BEGIN,
    SPINE_BEGIN,
    SPINE_END,
    TARGET_CRATE_BEGIN,
    TARGET_CRATE_END,
    NewAtomSpec,
    _add_native_edge,
    _compensate_new_atom,
    _ensure_native_edge,
    _find_operation_issue,
    _find_operation_issues,
    _mark_operation_recovery_required,
    _normalization_final_audit,
    _operation_id,
    _operation_identity_problems,
    _remove_native_edge,
    _render_new_atom_projection,
    _restore_edge_mutation,
    _retire_operation_duplicates,
    _write_dependency_mirror,
    _write_json,
    _write_operation_journal,
    add_crate_owner,
    append_knowledge_node,
    audit,
    audit_benchmark_obligations,
    audit_target_crate_contracts,
    dependencies_from_body,
    insert_phase_member,
    insert_project_plan_atom,
    maximum_path_spine,
    maximum_paths,
    parse_benchmark_obligations,
    parse_crate_map,
    parse_execution_spine,
    parse_project_plan,
    plan_add_edge,
    plan_new_atom,
    render_benchmark_bodies,
    render_issue_index,
    render_project_plan,
    render_spine_block,
    render_target_crate_bodies,
    replace_body_dependencies,
    replace_spine_block,
    snapshot_object,
)
from gordian_orchestration.atom_registry import main as registry_main
from gordian_orchestration.derive_status import IssueRecord, load_snapshot
from gordian_orchestration.provenance import Provenance

INVENTED_MIRROR_EDGES = frozenset(
    {(14, 9), (18, 12), (19, 23), (29, 11), (31, 23)}
)

ROOT = Path(__file__).resolve().parents[2]
STAMP = Provenance(
    generated_at="2026-08-31T00:00:00Z",
    source_change_id="abcdefghijkl",
    source_commit_id="123456789abc",
    tool_versions={"gh": "gh 2.46.0", "jj": "jj 0.44.0"},
)


def _body(blockers=()) -> str:
    dependencies = "None" if not blockers else "\n".join(f"- #{number}" for number in blockers)
    return f"## Objective\nFixture\n\n## Dependencies\n{dependencies}\n\n## Acceptance\n- exact\n"


def _record(
    number: int,
    blockers=(),
    *,
    title: str | None = None,
    milestone: str = "Fixture Initiative",
) -> IssueRecord:
    return IssueRecord(
        number=number,
        title=title or f"Atom {number}",
        state="OPEN",
        blocked_by=tuple(blockers),
        body=_body(blockers),
        labels=("type:atom",),
        milestone=milestone,
        url=f"https://github.com/kmosoti/gordian/issues/{number}",
    )


def _fixture_issues() -> tuple[IssueRecord, ...]:
    """Build the registry fixture without depending on the pending live snapshot.

    The checked-in graph fixture supplies authoritative dependency edges while the
    project-plan rows supply stable titles and initiative membership.  Individual
    tests add generated projections when they need a coherent registry.
    """
    project_plan = (ROOT / "docs/implementation/project-plan.md").read_text(encoding="utf-8")
    rows = parse_project_plan(project_plan)
    return tuple(
        _record(
            number,
            tuple(
                blocker
                for blocker in BLOCKED_BY[number]
                if (number, blocker) not in INVENTED_MIRROR_EDGES
            ),
            title=rows[number].title,
            milestone=rows[number].initiative,
        )
        for number in sorted(BLOCKED_BY)
    )


def _fixture_snapshot(destination: Path) -> None:
    _write_json(snapshot_object(_fixture_issues(), STAMP), destination)


class BodyDependencyTests(unittest.TestCase):
    def test_none_is_an_empty_mirror(self) -> None:
        self.assertEqual(dependencies_from_body(_body()), ())

    def test_references_are_deduplicated_and_sorted(self) -> None:
        body = "## Dependencies\n- #9\n- #2\n- #9\n\n## Acceptance\n- exact\n"
        self.assertEqual(dependencies_from_body(body), (2, 9))

    def test_missing_or_empty_section_is_an_error(self) -> None:
        with self.assertRaisesRegex(ValueError, "missing"):
            dependencies_from_body("## Acceptance\n- exact\n")
        with self.assertRaisesRegex(ValueError, "names neither"):
            dependencies_from_body("## Dependencies\n\n## Acceptance\n- exact\n")

    def test_rewrite_changes_only_the_dependency_section(self) -> None:
        body = "## Objective\nKeep me\n\n## Dependencies\nNone\n\n## Acceptance\n- exact\n"
        updated = replace_body_dependencies(body, (2, 9))
        self.assertEqual(dependencies_from_body(updated), (2, 9))
        self.assertIn("## Objective\nKeep me", updated)
        self.assertIn("## Acceptance\n- exact", updated)


class JsonWriterTests(unittest.TestCase):
    def test_replaces_destination_atomically_and_creates_parent(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory) / "nested" / "report.json"

            encoded = _write_json({"status": "clean"}, destination)

            self.assertEqual(destination.read_text(encoding="utf-8"), encoded)
            self.assertEqual(
                json.loads(destination.read_text(encoding="utf-8")), {"status": "clean"}
            )
            self.assertEqual(tuple(destination.parent.iterdir()), (destination,))


class MutationRecoveryTests(unittest.TestCase):
    def _arguments(self, directory: str):
        return type("Arguments", (), {"repository": "kmosoti/gordian"})()

    def test_dependency_merge_preserves_concurrent_body_text(self) -> None:
        current = _record(1)
        current = replace(current, body=current.body + "\nConcurrent note.\n")
        arguments = self._arguments("")
        with patch(
            "gordian_orchestration.atom_registry._records",
            side_effect=[
                (current,),
                (replace(current, body=_body((2,)) + "\nConcurrent note.\n"),),
            ],
        ), patch("gordian_orchestration.atom_registry.run_gh") as run_gh:
            _write_dependency_mirror(arguments, issue=1, blockers=(2,))
        self.assertIn("Concurrent note.", run_gh.call_args.args[0][-1])

    def test_journal_issue_is_reused_without_marker_create(self) -> None:
        issue = replace(_record(80), body=_record(80).body + " marker")
        journal = {"outcomes": [{"effect": "issue", "number": 80}]}
        self.assertIs(_find_operation_issue((issue,), "marker", journal), issue)

    def test_journal_issue_reuse_fails_closed_without_proven_marker(self) -> None:
        issue = _record(80)
        with self.assertRaisesRegex(RuntimeError, "marker-owned"):
            _find_operation_issue(
                (issue,), "marker", {"outcomes": [{"effect": "issue", "number": 80}]}
            )
        with self.assertRaisesRegex(RuntimeError, "marker-owned"):
            _find_operation_issue(
                (), "marker", {"outcomes": [{"effect": "issue", "number": 80}]}
            )
        with self.assertRaisesRegex(RuntimeError, "conflicting"):
            _find_operation_issue(
                (issue,),
                "marker",
                {
                    "outcomes": [
                        {"effect": "issue", "number": 80},
                        {"effect": "issue", "number": 81},
                    ]
                },
            )

    def test_unique_marker_is_reused_and_ambiguous_marker_fails(self) -> None:
        first = replace(_record(80), body=_record(80).body + " marker")
        self.assertEqual(_find_operation_issue((first,), "marker", {"outcomes": []}), first)
        with self.assertRaisesRegex(RuntimeError, "multiple"):
            _find_operation_issue(
                (first, replace(_record(81), body=first.body)),
                "marker",
                {"outcomes": []},
            )

    def test_lost_edge_response_is_accepted_after_canonical_reread(self) -> None:
        arguments = self._arguments("")
        with (
            patch(
                "gordian_orchestration.atom_registry._add_native_edge",
                side_effect=RuntimeError("lost"),
            ),
            patch(
                "gordian_orchestration.atom_registry._records",
                return_value=(replace(_record(1), blocked_by=(2,)),),
            ),
        ):
            _ensure_native_edge(arguments, 1, 2)

    def test_partial_add_edge_is_durable_recovery_not_compensation(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "operation.json"
            intent = {"issue": 1, "blocked_by": 2}
            journal = _write_operation_journal(
                path,
                operation_id="a" * 64,
                action="add-edge",
                intent=intent,
            )
            _mark_operation_recovery_required(
                journal, path, error=RuntimeError("partial write / lost response")
            )
            stored = json.loads(path.read_text(encoding="utf-8"))
            self.assertEqual(stored["status"], "recovery-required")
            self.assertNotIn("compensated", json.dumps(stored, sort_keys=True))
            self.assertEqual(
                stored["recovery"]["strategy"],
                "fresh-read-idempotent-additive-reconcile",
            )

    def test_clean_normalization_audit_clears_stale_remaining_drift(self) -> None:
        arguments = SimpleNamespace(journal=Path("journal.json"))
        journal = {
            "status": "audit-failed",
            "remaining_drift": ["stale drift"],
        }
        with (
            patch(
                "gordian_orchestration.atom_registry._coherence_reports",
                return_value=(),
            ),
            patch("gordian_orchestration.atom_registry.write_journal") as write,
        ):
            clean, problems = _normalization_final_audit(arguments, (), journal)

        self.assertTrue(clean)
        self.assertEqual(problems, ())
        self.assertNotIn("remaining_drift", journal)
        write.assert_not_called()

    def test_concurrent_additive_edge_is_preserved_during_lost_response(self) -> None:
        arguments = self._arguments("")
        with (
            patch(
                "gordian_orchestration.atom_registry._add_native_edge",
                side_effect=RuntimeError("response lost after write"),
            ),
            patch(
                "gordian_orchestration.atom_registry._records",
                return_value=(replace(_record(1), blocked_by=(2, 3)),),
            ),
            patch("gordian_orchestration.atom_registry._remove_native_edge") as remove,
        ):
            _ensure_native_edge(arguments, 1, 2)
        remove.assert_not_called()

    def test_operation_ids_separate_semantically_different_specs(self) -> None:
        common = {"body_sha256": "a", "blocks": [69]}
        self.assertNotEqual(
            _operation_id("new-atom", actor="a", lease_id="l", inputs=common),
            _operation_id(
                "new-atom",
                actor="a",
                lease_id="l",
                inputs={**common, "type_label": "type:experiment"},
            ),
        )

    def test_operation_id_is_stable_across_actor_and_lease_renewal(self) -> None:
        intent = {
            "body_sha256": "a",
            "blocks": [69],
            "knowledge_node": {"b": 1, "a": 2},
        }
        first = _operation_id(
            "new-atom",
            repository="Example/Repo",
            intent=intent,
            actor="gordian-agent/one/run-a",
            lease_id="lease-a",
        )
        renewed = _operation_id(
            "new-atom",
            repository="example/repo",
            intent={
                "knowledge_node": {"a": 2, "b": 1},
                "body_sha256": "a",
                "blocks": [69],
            },
            actor="gordian-agent/two/run-b",
            lease_id="lease-renewed",
        )
        self.assertEqual(first, renewed)

    def test_marker_scan_includes_all_matches_after_one_outcome_was_recorded(self) -> None:
        marker = "<!-- gordian-operation: op -->"
        first = replace(_record(80), body=_record(80).body + f"\n{marker}\n")
        second = replace(_record(81), body=_record(81).body + f"\n{marker}\n")
        matches = _find_operation_issues(
            (second, first),
            marker,
            {"outcomes": [{"effect": "issue", "number": 80}]},
        )
        self.assertEqual(tuple(issue.number for issue in matches), (80, 81))

    def test_duplicate_recovery_keeps_lowest_issue_and_verifies_only_marker_extras(self) -> None:
        marker = "<!-- gordian-operation: op -->"
        canonical = replace(_record(80), body=_record(80).body + f"\n{marker}\n")
        extra = replace(_record(82), body=_record(82).body + f"\n{marker}\n")
        arguments = SimpleNamespace(repository="example/repo")
        with tempfile.TemporaryDirectory() as directory:
            journal_path = Path(directory) / "journal.json"
            journal = {"outcomes": []}
            _write_json(journal, journal_path)
            with (
                patch(
                    "gordian_orchestration.atom_registry._records",
                    return_value=(canonical, extra),
                ),
                patch("gordian_orchestration.atom_registry.run_gh") as run_gh,
                patch(
                    "gordian_orchestration.atom_registry.run_gh_json",
                    return_value={
                        "body": extra.body,
                        "state": "CLOSED",
                        "labels": [{"name": "duplicate"}],
                    },
                ),
            ):
                retired = _retire_operation_duplicates(
                    arguments,
                    canonical=canonical,
                    extras=(extra,),
                    marker=marker,
                    journal=journal,
                    journal_path=journal_path,
                )
        self.assertEqual(retired, (82,))
        edited_numbers = [call.args[0][2] for call in run_gh.call_args_list]
        self.assertEqual(edited_numbers, ["82", "82"])
        self.assertEqual(journal["outcomes"][0]["canonical"], 80)

    def test_operation_identity_reports_spec_mismatch_before_cleanup(self) -> None:
        marker = "<!-- gordian-operation: op -->"
        issue = replace(_record(80, title="wrong"), body=_record(80).body + f"\n{marker}\n")
        spec = NewAtomSpec(
            title="intended",
            body=_record(80).body,
            milestone="Fixture Initiative",
            type_label="type:atom",
            target_crate=None,
            phase=6,
            blocked_by=(),
            blocks=(1,),
            knowledge_node={},
        )
        problems = _operation_identity_problems(
            issue,
            spec=spec,
            marker=marker,
            intent={},
        )
        self.assertIn("title 'wrong' != intended 'intended'", problems)

    def test_edge_post_error_is_recovered_when_reread_observes_edge(self) -> None:
        with patch("gordian_orchestration.atom_registry._remove_native_edge") as remove:
            _restore_edge_mutation(
                SimpleNamespace(),
                plan=SimpleNamespace(issue=1, blocked_by=2),
                original_body="old",
                body_changed=True,
                edge_added=True,
                project_touched=True,
                original_issues=(),
            )
        remove.assert_not_called()

    def test_same_actor_retry_keeps_existing_additive_edge(self) -> None:
        with patch("gordian_orchestration.atom_registry._remove_native_edge") as remove:
            _restore_edge_mutation(
                SimpleNamespace(),
                plan=SimpleNamespace(issue=1, blocked_by=2),
                original_body="old",
                body_changed=False,
                edge_added=True,
                project_touched=False,
                original_issues=(),
            )
        remove.assert_not_called()

    def test_lost_create_marker_recovery_is_unique(self) -> None:
        with patch("gordian_orchestration.atom_registry.run_gh") as gh:
            _compensate_new_atom(
                SimpleNamespace(),
                number=80,
                native_edges=[(80, 2)],
                original_bodies={2: "old"},
                changed_bodies=[2],
                project_item_id="item",
                project_touched=True,
                original_issues=(),
            )
        gh.assert_not_called()

    def test_downstream_race_does_not_trigger_remote_cleanup(self) -> None:
        with patch("gordian_orchestration.atom_registry.run_gh") as gh:
            _compensate_new_atom(
                SimpleNamespace(),
                number=80,
                native_edges=[(2, 80)],
                original_bodies={2: "old"},
                changed_bodies=[2],
                project_item_id="item",
                project_touched=True,
                original_issues=(),
            )
        gh.assert_not_called()

    def test_replace_failure_preserves_existing_bytes_and_cleans_temp(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory) / "report.json"
            original = b'{"status":"old"}\n'
            destination.write_bytes(original)

            with (
                patch(
                    "gordian_orchestration.atom_registry.os.replace",
                    side_effect=OSError("nope"),
                ),
                self.assertRaisesRegex(OSError, "nope"),
            ):
                _write_json({"status": "new"}, destination)

            self.assertEqual(destination.read_bytes(), original)
            self.assertEqual(tuple(destination.parent.iterdir()), (destination,))


class ProjectionParserTests(unittest.TestCase):
    def test_project_plan_rows_are_read_without_acceptance_tables(self) -> None:
        text = """
| # | Acceptance item | Atoms |
| 1 | something | #1, #2 |
# Initiative: Fixture

| Atom | Title | Target crate | Blocked by |
| #1 | Root | — | — |
| #2 | Child | `gordian-core` | #1 |
"""
        self.assertEqual(
            parse_project_plan(text),
            {
                1: parse_project_plan(text)[1],
                2: parse_project_plan(text)[2],
            },
        )
        self.assertEqual(parse_project_plan(text)[2].blocked_by, (1,))
        self.assertEqual(parse_project_plan(text)[2].target_crate, "`gordian-core`")
        self.assertEqual(parse_project_plan(text)[2].initiative, "Fixture")

    def test_generated_spine_edges_are_read_from_markers(self) -> None:
        text = f"""
{SPINE_BEGIN}
```text
#1 -> #2 -> #4
#1 -> #3 -> #4
```
{SPINE_END}
"""
        self.assertEqual(parse_execution_spine(text)[0], (1, 2, 3, 4))
        self.assertEqual(
            parse_execution_spine(text)[1], ((1, 2), (1, 3), (2, 4), (3, 4))
        )

    def test_missing_spine_markers_are_an_error(self) -> None:
        with self.assertRaisesRegex(ValueError, "marker pair"):
            parse_execution_spine("#1 -> #2")

    def test_benchmark_rows_keep_stable_identity_owner_and_gate(self) -> None:
        text = """## 17. Critical performance suite

| Row | Obligation | Owner | In first qualification |
| --- | --- | --- | --- |
| `EO17-MG-1` | validation | #1 | yes |
| `EO17-KG-2` | traversal | #2 | no — later qualification. |

### Benchmark obligation sections
"""
        rows = parse_benchmark_obligations(text)
        self.assertEqual(tuple(row.row_id for row in rows), ("EO17-MG-1", "EO17-KG-2"))
        self.assertEqual(tuple(row.owner for row in rows), (1, 2))
        self.assertEqual(tuple(row.first_qualification for row in rows), (True, False))


class BenchmarkContractTests(unittest.TestCase):
    ORDER = """## 17. Critical performance suite

| Row | Obligation | Owner | In first qualification |
| --- | --- | --- | --- |
| `EO17-MG-1` | validation | #1 | yes |
| `EO17-KG-2` | traversal | #2 | no — later qualification. |

### Benchmark obligation sections
"""

    def test_renderer_makes_owner_and_qualification_joins_exact(self) -> None:
        issues = (_record(1), _record(2), _record(3, (1,)))
        updates = render_benchmark_bodies(issues, self.ORDER, qualification_atom=3)
        rendered = tuple(
            IssueRecord(
                number=issue.number,
                title=issue.title,
                state=issue.state,
                blocked_by=issue.blocked_by,
                body=updates.get(issue.number, issue.body),
                labels=issue.labels,
                milestone=issue.milestone,
                url=issue.url,
            )
            for issue in issues
        )
        report = audit_benchmark_obligations(rendered, self.ORDER, qualification_atom=3)
        self.assertTrue(report.clean, "\n".join(report.problems))
        self.assertIn(BENCHMARK_OWNER_BEGIN, updates[1])
        self.assertIn("`EO17-MG-1`", updates[1])
        self.assertIn(BENCHMARK_GATE_BEGIN, updates[3])
        self.assertIn("`EO17-MG-1`", updates[3])
        self.assertNotIn("EO17-KG-2", updates[3])

    def test_renderer_preserves_existing_benchmark_prose_and_is_idempotent(self) -> None:
        owner = _record(1)
        owner = IssueRecord(
            number=owner.number,
            title=owner.title,
            state=owner.state,
            blocked_by=owner.blocked_by,
            body=owner.body
            + "\n## Benchmark obligation\n\nMeasure adversarial graphs.\n",
            labels=owner.labels,
            milestone=owner.milestone,
            url=owner.url,
        )
        issues = (owner, _record(2), _record(3, (1,)))
        first = render_benchmark_bodies(issues, self.ORDER, qualification_atom=3)
        updated_issues = tuple(
            IssueRecord(
                number=issue.number,
                title=issue.title,
                state=issue.state,
                blocked_by=issue.blocked_by,
                body=first.get(issue.number, issue.body),
                labels=issue.labels,
                milestone=issue.milestone,
                url=issue.url,
            )
            for issue in issues
        )
        second = render_benchmark_bodies(updated_issues, self.ORDER, qualification_atom=3)
        self.assertIn("Measure adversarial graphs.", first[1])
        self.assertEqual(first, second)

    def test_renderer_removes_stale_owner_block_after_reassignment(self) -> None:
        owner = _record(1)
        owner = replace(
            owner,
            body=owner.body
            + "\n## Benchmark obligation\nHuman benchmark rationale.\n",
        )
        issues = (owner, _record(2), _record(3, (1, 2)))
        initial = render_benchmark_bodies(issues, self.ORDER, qualification_atom=3)
        initial_issues = tuple(
            replace(issue, body=initial.get(issue.number, issue.body)) for issue in issues
        )

        reassigned_order = self.ORDER.replace(
            "| `EO17-MG-1` | validation | #1 | yes |",
            "| `EO17-MG-1` | validation | #2 | yes |",
        )
        stale = audit_benchmark_obligations(
            initial_issues, reassigned_order, qualification_atom=3
        )
        self.assertFalse(stale.clean)
        self.assertIn(
            "#1: stale generated EO17 ownership block; issue owns no rows",
            stale.problems,
        )

        first = render_benchmark_bodies(
            initial_issues, reassigned_order, qualification_atom=3
        )
        updated = tuple(
            replace(issue, body=first.get(issue.number, issue.body))
            for issue in initial_issues
        )
        report = audit_benchmark_obligations(
            updated, reassigned_order, qualification_atom=3
        )
        self.assertTrue(report.clean, "\n".join(report.problems))
        self.assertIn("Human benchmark rationale.", updated[0].body)
        self.assertNotIn(BENCHMARK_OWNER_BEGIN, updated[0].body)
        self.assertIn(BENCHMARK_OWNER_BEGIN, updated[1].body)
        second = render_benchmark_bodies(updated, reassigned_order, qualification_atom=3)
        self.assertEqual(first, second)

    def test_audit_reports_missing_sections_duplicate_ids_and_closure_escape(self) -> None:
        duplicate = self.ORDER.replace(
            "### Benchmark obligation sections",
            "| `EO17-MG-1` | repeated | #2 | yes |\n\n### Benchmark obligation sections",
        )
        report = audit_benchmark_obligations(
            (_record(1), _record(2), _record(3, (1,))),
            duplicate,
            qualification_atom=3,
        )
        joined = "\n".join(report.problems)
        self.assertIn("occurs 2 times", joined)
        self.assertIn("outside closure", joined)
        self.assertIn("missing `## Benchmark obligation`", joined)
        self.assertIn("Performance acceptance omits", joined)


class RegistryAuditTests(unittest.TestCase):
    def test_audit_rejects_native_edges_to_absent_executable_atoms(self) -> None:
        issues = (_record(1), _record(2, (99,)))
        report = audit(
            issues,
            project_plan=(
                "# Initiative: Fixture Initiative\n\n"
                "| #1 | Atom 1 | — | — |\n"
                "| #2 | Atom 2 | — | #99 |\n"
            ),
            execution_order=f"{SPINE_BEGIN}\n#1 -> #2\n{SPINE_END}\n",
            target=2,
            stamp=STAMP,
        )

        self.assertFalse(report.clean)
        self.assertIn(
            "#2: native blocker #99 is absent from the executable Atom registry",
            report.problems,
        )

    def test_mutation_plans_refuse_preexisting_absent_native_edges(self) -> None:
        issues = (_record(1), _record(2, (99,)), _record(3))
        with self.assertRaisesRegex(ValueError, r"#2: native blocker #99 is absent"):
            plan_add_edge(issues, issue_number=3, blocker_number=1)

        spec = NewAtomSpec(
            title="[Fixture] New Atom",
            body=(
                "## Initiative\nFixture Initiative\n\n"
                "## Dependencies\nNone\n\n"
                "## Acceptance\n- exact\n\n"
                "## Verification\n- exact\n\n"
                "## Closure\n- exact\n"
            ),
            milestone="Fixture Initiative",
            type_label="type:atom",
            target_crate=None,
            phase=6,
            blocked_by=(),
            blocks=(3,),
            knowledge_node={
                "@id": "artifact:fixture-new-atom",
                "@type": ["ImplementationArtifact"],
                "name": "New Atom",
                "summary": "Fixture.",
            },
        )
        with self.assertRaisesRegex(ValueError, r"#2: native blocker #99 is absent"):
            plan_new_atom(issues, spec)

    def test_add_edge_plan_refuses_preexisting_focused_drift(self) -> None:
        stderr = io.StringIO()
        with tempfile.TemporaryDirectory() as directory:
            snapshot = Path(directory) / "issues.json"
            _fixture_snapshot(snapshot)
            with redirect_stderr(stderr):
                status = registry_main(
                    [
                        "--snapshot",
                        str(snapshot),
                        "add-edge",
                        "3",
                        "2",
                    ]
                )

        self.assertEqual(status, 2)
        self.assertIn("non-coherent registry", stderr.getvalue())
        self.assertIn("benchmark obligations:", stderr.getvalue())

    def test_maximum_path_spine_keeps_all_tied_paths(self) -> None:
        issues = (
            _record(1),
            _record(2, (1,)),
            _record(3, (1,)),
            _record(4, (2, 3)),
            _record(5, (1, 4)),
        )
        spine = maximum_path_spine(issues, 5)
        self.assertEqual(spine.length, 3)
        self.assertEqual(spine.nodes, (1, 2, 3, 4, 5))
        self.assertEqual(
            spine.edges, ((1, 2), (1, 3), (2, 4), (3, 4), (4, 5))
        )
        self.assertEqual(maximum_paths(issues, 5), ((1, 2, 4, 5), (1, 3, 4, 5)))

    def test_add_edge_plan_is_cycle_checked_and_idempotent(self) -> None:
        issues = (_record(1), _record(2, (1,)), _record(3))
        plan, updated = plan_add_edge(issues, issue_number=3, blocker_number=2)
        self.assertTrue(plan.changed)
        self.assertEqual(plan.proposed_blockers, (2,))
        self.assertEqual(updated[2].blocked_by, (2,))
        self.assertEqual(dependencies_from_body(updated[2].body), (2,))

        repeated, unchanged = plan_add_edge(updated, issue_number=3, blocker_number=2)
        self.assertFalse(repeated.changed)
        self.assertEqual(unchanged, updated)

        with self.assertRaisesRegex(RuntimeError, "cycle"):
            plan_add_edge(issues, issue_number=1, blocker_number=2)

    def test_spine_render_is_deterministic_and_replaces_only_the_marked_block(self) -> None:
        issues = (_record(1), _record(2, (1,)), _record(3, (1,)), _record(4, (2, 3)))
        rendered = render_spine_block(issues, 4)
        self.assertEqual(
            rendered,
            "\n".join(
                (
                    SPINE_BEGIN,
                    "",
                    "```text",
                    "#1 -> #2 -> #4",
                    "#1 -> #3 -> #4",
                    "```",
                    "",
                    SPINE_END,
                )
            ),
        )
        original = f"before\n{SPINE_BEGIN}\nstale\n{SPINE_END}\nafter\n"
        self.assertEqual(replace_spine_block(original, rendered), f"before\n{rendered}\nafter\n")

    def test_audit_reports_body_plan_metadata_and_spine_drift(self) -> None:
        issues = (
            _record(1),
            IssueRecord(
                number=2,
                title="Live child",
                state="OPEN",
                blocked_by=(1,),
                body=_body(()),
                labels=(),
                milestone=None,
            ),
        )
        plan = """
# Initiative: Fixture Initiative

| Atom | Title | Target crate | Blocked by |
| #1 | Atom 1 | — | — |
| #2 | Stale child | — | — |
"""
        order = f"{SPINE_BEGIN}\n#2 -> #1\n{SPINE_END}\n"
        report = audit(
            issues,
            project_plan=plan,
            execution_order=order,
            target=2,
            stamp=STAMP,
        )
        joined = "\n".join(report.problems)
        self.assertFalse(report.clean)
        self.assertIn("body dependencies", joined)
        self.assertIn("exactly one type label", joined)
        self.assertIn("no Initiative milestone", joined)
        self.assertIn("project-plan title", joined)
        self.assertIn("project-plan blockers", joined)
        self.assertIn("execution spine", joined)

    def test_checked_in_plan_and_spine_match_the_captured_fixture(self) -> None:
        project_plan = (ROOT / "docs/implementation/project-plan.md").read_text(encoding="utf-8")
        execution_order = (ROOT / "docs/implementation/execution-order.md").read_text(
            encoding="utf-8"
        )
        issues = _fixture_issues()
        report = audit(
            issues,
            project_plan=project_plan,
            execution_order=execution_order,
            stamp=STAMP,
        )
        self.assertTrue(report.clean, "\n".join(report.problems))
        self.assertEqual(report.issue_count, 77)
        self.assertEqual(report.maximum_path_length, 18)

    def test_issue_index_count_is_registered_not_open(self) -> None:
        issue_index = """**77 registered Atoms**
<!-- BEGIN GENERATED: INITIATIVE REGISTER -->
| Initiative | Atom identities | Purpose | Acceptance rule |
| --- | --- | --- | --- |
| [Fixture Initiative](https://example.test) | #1, #2 | Fixture | closure |
<!-- END GENERATED: INITIATIVE REGISTER -->
Fourteen Initiatives, 77 Atoms.
"""
        closed = replace(_record(1), state="CLOSED")
        rendered = render_issue_index(issue_index, (closed, _record(2)))
        self.assertIn("**2 registered Atoms**", rendered)
        self.assertNotIn("open Atoms", rendered)
        self.assertNotIn("77 Atoms", rendered)
        self.assertIn("Fourteen Initiatives, 2 Atoms.", rendered)

    def test_plan_render_preserves_target_crate_and_refreshes_derived_cells(self) -> None:
        plan = """# Initiative: Fixture Initiative

| Atom | Title | Target crate | Blocked by |
| --- | --- | --- | --- |
| #1 | Stale | `gordian-core` | #9 |
"""
        rendered = render_project_plan(plan, (_record(1, title="Current"),))
        self.assertIn("| #1 | Current | `gordian-core` | — |", rendered)

    def test_plan_render_refuses_implicit_initiative_moves(self) -> None:
        plan = """# Initiative: Old Initiative

| Atom | Title | Target crate | Blocked by |
| --- | --- | --- | --- |
| #1 | Atom 1 | — | — |
"""
        with self.assertRaisesRegex(ValueError, "move the row explicitly"):
            render_project_plan(plan, (_record(1, milestone="New Initiative"),))

    def test_snapshot_carries_complete_contract_fields_and_provenance(self) -> None:
        payload = snapshot_object((_record(1),), STAMP)
        self.assertEqual(payload["record_format"], "gordian-atom-registry-v1")
        self.assertEqual(payload["source_commit_id"], STAMP.source_commit_id)
        self.assertEqual(payload["repository"], "kmosoti/gordian")
        self.assertTrue(payload["coherent"])
        self.assertEqual(payload["issues"][0]["body"], _body())
        self.assertEqual(payload["issues"][0]["blockedBy"], [])
        self.assertEqual(payload["issues"][0]["assignees"], [])
        self.assertTrue(json.dumps(payload, sort_keys=True))
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "issues.json"
            path.write_text(json.dumps(payload), encoding="utf-8")
            self.assertEqual(load_snapshot(path), (_record(1),))


class RegistryCaptureTests(unittest.TestCase):
    def test_coherent_capture_reports_true(self) -> None:
        issues = _fixture_issues()
        plan = (ROOT / "docs/implementation/project-plan.md").read_text(encoding="utf-8")
        order = (ROOT / "docs/implementation/execution-order.md").read_text(encoding="utf-8")
        target_updates = render_target_crate_bodies(issues, plan)
        target_clean = tuple(
            replace(issue, body=target_updates.get(issue.number, issue.body))
            for issue in issues
        )
        benchmark_updates = render_benchmark_bodies(target_clean, order)
        coherent = tuple(
            replace(issue, body=benchmark_updates.get(issue.number, issue.body))
            for issue in target_clean
        )
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory) / "issues.json"
            with patch(
                "gordian_orchestration.atom_registry._records", return_value=coherent
            ):
                status = registry_main(["capture", "--output", str(destination)])

            self.assertEqual(status, 0)
            self.assertTrue(json.loads(destination.read_text(encoding="utf-8"))["coherent"])

    def test_capture_preserves_existing_snapshot_when_benchmark_audit_drifts(self) -> None:
        issues = _fixture_issues()
        benchmark_clean = tuple(
            replace(issue, body=render_benchmark_bodies(
                issues,
                (ROOT / "docs/implementation/execution-order.md").read_text(
                    encoding="utf-8"
                ),
            ).get(issue.number, issue.body))
            for issue in issues
        )
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory) / "issues.json"
            destination.write_text("prior snapshot\n", encoding="utf-8")
            stderr = io.StringIO()
            with (
                patch("gordian_orchestration.atom_registry._records", return_value=benchmark_clean),
                redirect_stderr(stderr),
            ):
                status = registry_main(["capture", "--output", str(destination)])

            self.assertEqual(status, 1)
            self.assertEqual(destination.read_text(encoding="utf-8"), "prior snapshot\n")
            self.assertIn("target-crate contracts:", stderr.getvalue())

    def test_capture_does_not_create_snapshot_when_target_crate_audit_drifts(self) -> None:
        issues = _fixture_issues()
        plan = (ROOT / "docs/implementation/project-plan.md").read_text(encoding="utf-8")
        target_updates = render_target_crate_bodies(issues, plan)
        target_clean = tuple(
            replace(issue, body=target_updates.get(issue.number, issue.body))
            for issue in issues
        )
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory) / "nested" / "issues.json"
            stderr = io.StringIO()
            with (
                patch("gordian_orchestration.atom_registry._records", return_value=target_clean),
                redirect_stderr(stderr),
            ):
                status = registry_main(["capture", "--output", str(destination)])

            self.assertEqual(status, 1)
            self.assertFalse(destination.exists())
            self.assertIn("benchmark obligations:", stderr.getvalue())


class SnapshotStagingTests(unittest.TestCase):
    def _base_and_clean_variants(self) -> tuple[tuple[IssueRecord, ...], ...]:
        issues = _fixture_issues()
        plan = (ROOT / "docs/implementation/project-plan.md").read_text(encoding="utf-8")
        order = (ROOT / "docs/implementation/execution-order.md").read_text(encoding="utf-8")
        target_updates = render_target_crate_bodies(issues, plan)
        target_clean = tuple(
            replace(
                issue,
                body=target_updates.get(issue.number, issue.body),
            )
            for issue in issues
        )
        benchmark_updates = render_benchmark_bodies(issues, order)
        benchmark_clean = tuple(
            replace(issue, body=benchmark_updates.get(issue.number, issue.body))
            for issue in issues
        )
        benchmark_after_target = render_benchmark_bodies(target_clean, order)
        both_clean = tuple(
            replace(issue, body=benchmark_after_target.get(issue.number, issue.body))
            for issue in target_clean
        )
        return issues, benchmark_clean, target_clean, both_clean

    def test_benchmark_sync_leaves_successful_body_edits_when_target_drift_remains(self) -> None:
        issues, benchmark_clean, _, _ = self._base_and_clean_variants()
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory) / "issues.json"
            destination.write_text("prior snapshot\n", encoding="utf-8")
            stdout = io.StringIO()
            with (
                patch(
                    "gordian_orchestration.atom_registry._records",
                    side_effect=[issues, benchmark_clean],
                ),
                patch(
                    "gordian_orchestration.atom_registry.preflight",
                    return_value=SimpleNamespace(login="tester"),
                ),
                patch("gordian_orchestration.atom_registry.require_live_claim"),
                patch("gordian_orchestration.atom_registry.run_gh", return_value="") as run_gh,
                patch("gordian_orchestration.atom_registry._restore_issue_bodies") as restore,
                redirect_stdout(stdout),
            ):
                status = registry_main(
                    ["sync-benchmarks", "--apply", "--output", str(destination)]
                )

            self.assertEqual(status, 0)
            self.assertGreater(run_gh.call_count, 0)
            restore.assert_not_called()
            self.assertEqual(destination.read_text(encoding="utf-8"), "prior snapshot\n")
            payload = json.loads(stdout.getvalue())
            self.assertFalse(payload["coherent"])
            self.assertTrue(payload["snapshot_skipped"])
            self.assertTrue(payload["remaining_drift"])
            self.assertTrue(
                all(
                    item.startswith("target-crate contracts:")
                    for item in payload["remaining_drift"]
                )
            )

    def test_target_crate_sync_leaves_successful_body_edits_when_benchmark_drift_remains(
        self,
    ) -> None:
        issues, _, target_clean, _ = self._base_and_clean_variants()
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory) / "issues.json"
            destination.write_text("prior snapshot\n", encoding="utf-8")
            stdout = io.StringIO()
            with (
                patch(
                    "gordian_orchestration.atom_registry._records",
                    side_effect=[issues, target_clean],
                ),
                patch(
                    "gordian_orchestration.atom_registry.preflight",
                    return_value=SimpleNamespace(login="tester"),
                ),
                patch("gordian_orchestration.atom_registry.require_live_claim"),
                patch("gordian_orchestration.atom_registry.run_gh", return_value="") as run_gh,
                patch("gordian_orchestration.atom_registry._restore_issue_bodies") as restore,
                redirect_stdout(stdout),
            ):
                status = registry_main(
                    ["sync-target-crates", "--apply", "--output", str(destination)]
                )

            self.assertEqual(status, 0)
            self.assertGreater(run_gh.call_count, 0)
            restore.assert_not_called()
            self.assertEqual(destination.read_text(encoding="utf-8"), "prior snapshot\n")
            payload = json.loads(stdout.getvalue())
            self.assertFalse(payload["coherent"])
            self.assertTrue(payload["snapshot_skipped"])
            self.assertTrue(payload["remaining_drift"])
            self.assertTrue(
                all(
                    item.startswith("benchmark obligations:")
                    for item in payload["remaining_drift"]
                )
            )

    def test_zero_change_benchmark_sync_reports_staged_drift_without_snapshot(self) -> None:
        _, benchmark_clean, _, _ = self._base_and_clean_variants()
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory) / "issues.json"
            destination.write_text("prior snapshot\n", encoding="utf-8")
            stdout = io.StringIO()
            with (
                patch(
                    "gordian_orchestration.atom_registry._records",
                    side_effect=[benchmark_clean, benchmark_clean],
                ),
                patch(
                    "gordian_orchestration.atom_registry.preflight",
                    return_value=SimpleNamespace(login="tester"),
                ),
                patch("gordian_orchestration.atom_registry.require_live_claim"),
                redirect_stdout(stdout),
            ):
                status = registry_main(
                    ["sync-benchmarks", "--apply", "--output", str(destination)]
                )

            self.assertEqual(status, 0)
            self.assertEqual(destination.read_text(encoding="utf-8"), "prior snapshot\n")
            payload = json.loads(stdout.getvalue())
            self.assertFalse(payload["coherent"])
            self.assertTrue(payload["snapshot_skipped"])

    def test_zero_change_benchmark_sync_refreshes_a_coherent_snapshot(self) -> None:
        _, _, _, both_clean = self._base_and_clean_variants()
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory) / "issues.json"
            stdout = io.StringIO()
            with (
                patch(
                    "gordian_orchestration.atom_registry._records",
                    side_effect=[both_clean, both_clean],
                ),
                patch(
                    "gordian_orchestration.atom_registry.preflight",
                    return_value=SimpleNamespace(login="tester"),
                ),
                patch("gordian_orchestration.atom_registry.require_live_claim"),
                redirect_stdout(stdout),
            ):
                status = registry_main(
                    ["sync-benchmarks", "--apply", "--output", str(destination)]
                )

            self.assertEqual(status, 0)
            self.assertTrue(json.loads(destination.read_text(encoding="utf-8"))["coherent"])
            payload = json.loads(stdout.getvalue())
            self.assertTrue(payload["coherent"])
            self.assertEqual(payload["snapshot"], str(destination))


class TargetCrateContractTests(unittest.TestCase):
    PLAN = """# Initiative: Fixture Initiative

| Atom | Title | Target crate | Blocked by |
| --- | --- | --- | --- |
| #1 | Owner | `gordian-core` | — |
| #2 | No crate | — | — |
| #29 | Source adapter | `gordian-source` (trait), `gordian-jj` | — |
"""

    def test_renderer_writes_the_exact_target_crate_path_and_audit_accepts_it(self) -> None:
        updates = render_target_crate_bodies(
            (_record(1), _record(2), _record(29)), self.PLAN
        )

        self.assertEqual(
            updates[1],
            _body()
            + "\n"
            + f"## Target crate\n\n{TARGET_CRATE_BEGIN}\n"
            "Rust target: `crates/gordian-core`.\n"
            f"{TARGET_CRATE_END}\n\n",
        )
        rendered = tuple(
            replace(issue, body=updates.get(issue.number, issue.body))
            for issue in (_record(1), _record(2), _record(29))
        )
        report = audit_target_crate_contracts(rendered, self.PLAN)
        self.assertEqual(report.owner_count, 2)
        self.assertTrue(report.clean, "\n".join(report.problems))
        self.assertIn(
            "Rust targets: `crates/gordian-jj` and `crates/gordian-source`.",
            updates[29],
        )

    def test_multi_crate_contract_requires_exact_set(self) -> None:
        issue = replace(
            _record(29),
            body=_body()
            + "\n## Target crate\n"
            "Production Rust targets: `crates/gordian-source`.\n",
        )

        owner = _record(1, title="Owner")
        owner = replace(
            owner,
            body=owner.body
            + "\n## Target crate\nProduction Rust target: `crates/gordian-core`.\n",
        )
        report = audit_target_crate_contracts((owner, issue), self.PLAN)

        self.assertFalse(report.clean)
        self.assertIn(
            "expected ['crates/gordian-jj', 'crates/gordian-source']",
            "\n".join(report.problems),
        )

    def test_malformed_target_crate_cells_are_rejected(self) -> None:
        for cell in ("gordian-source", "`not-gordian`", "`gordian-source"):
            plan = self.PLAN.replace(
                "`gordian-source` (trait), `gordian-jj`", cell
            )
            with self.subTest(cell=cell), self.assertRaisesRegex(ValueError, "unreadable"):
                audit_target_crate_contracts((_record(1), _record(29)), plan)

    def test_audit_reports_missing_target_crate_section(self) -> None:
        report = audit_target_crate_contracts((_record(1), _record(2)), self.PLAN)

        self.assertFalse(report.clean)
        self.assertIn("#1: missing `## Target crate` section", report.problems)
        self.assertIn("expected ['crates/gordian-core']", report.problems[1])

    def test_audit_reports_wrong_and_extra_crate_paths(self) -> None:
        body = (
            _body()
            + "\n## Target crate\n"
            "Production Rust target: `crates/gordian-wrong` and `crates/gordian-extra`.\n"
        )
        issue = replace(_record(1), body=body)

        report = audit_target_crate_contracts((issue, _record(2)), self.PLAN)

        self.assertFalse(report.clean)
        self.assertIn(
            "names ['crates/gordian-extra', 'crates/gordian-wrong'], "
            "expected ['crates/gordian-core']",
            report.problems[0],
        )

    def test_renderer_preserves_prose_and_is_idempotent(self) -> None:
        body = _body() + "\nKeep this owner note.\n\n## Target crate\n\nOld prose.\n"
        issue = replace(_record(1), body=body)
        first = render_target_crate_bodies(
            (issue, _record(2), _record(29)), self.PLAN
        )
        updated = (
            replace(issue, body=first[1]),
            _record(2),
            replace(_record(29), body=first[29]),
        )
        second = render_target_crate_bodies(updated, self.PLAN)

        self.assertIn("Keep this owner note.", first[1])
        self.assertIn("Old prose.", first[1])
        self.assertEqual(first, second)

    def test_renderer_removes_stale_target_block_for_empty_target_set(self) -> None:
        plan = self.PLAN.replace(
            "| #2 | No crate | — | — |",
            "| #2 | No crate | — | — |\n| #3 | Empty target | — | — |",
        )
        stale_block = (
            f"{TARGET_CRATE_BEGIN}\n"
            "Production Rust target: `crates/gordian-old`.\n"
            f"{TARGET_CRATE_END}"
        )
        with_human = replace(
            _record(2),
            body=_body()
            + "\n## Target crate\nHuman target rationale.\n\n"
            + stale_block
            + "\n\n## Acceptance note\nKeep this section.\n",
        )
        generated_only = replace(
            _record(3),
            body=_body() + "\n## Target crate\n\n" + stale_block + "\n",
        )
        issues = (_record(1), with_human, generated_only, _record(29))
        stale = audit_target_crate_contracts(issues, plan)
        self.assertFalse(stale.clean)
        self.assertIn(
            "#2: stale generated target-crate block; crate map has no target crate",
            stale.problems,
        )

        first = render_target_crate_bodies(issues, plan)
        updated = tuple(
            replace(issue, body=first.get(issue.number, issue.body)) for issue in issues
        )
        report = audit_target_crate_contracts(updated, plan)
        self.assertTrue(report.clean, "\n".join(report.problems))
        self.assertIn("Human target rationale.", updated[1].body)
        self.assertIn("## Target crate", updated[1].body)
        self.assertNotIn(TARGET_CRATE_BEGIN, updated[1].body)
        self.assertNotIn("## Target crate", updated[2].body)
        self.assertNotIn(TARGET_CRATE_BEGIN, updated[2].body)
        second = render_target_crate_bodies(updated, plan)
        self.assertEqual(first, second)


class CrateMapAuthorityTests(unittest.TestCase):
    CRATE_MAP = """| Crate | Path | May depend on (complete list) | Owning Atoms |
| --- | --- | --- | --- |
| `gordian-core` | `crates/gordian-core` | (none) | #1 |
| `gordian-jj` | `crates/gordian-jj` | `gordian-core` | #2 |
| `gordian-git` | `crates/gordian-git` | `gordian-core` | #3 |
"""
    PLAN = """# Initiative: Fixture Initiative

| Atom | Title | Target crate | Blocked by |
| --- | --- | --- | --- |
| #1 | Owner | `gordian-core` | — |
| #2 | Adapter | `gordian-jj` | — |
| #3 | Git adapter | `gordian-git` | — |
| #7 | Conformance | — | — |
| #34 | Comparison | — | — |
"""

    def test_crate_map_reverses_one_atom_to_multiple_targets(self) -> None:
        self.assertEqual(
            parse_crate_map(self.CRATE_MAP),
            {
                1: ("gordian-core",),
                2: ("gordian-jj",),
                3: ("gordian-git",),
            },
        )

    def test_crate_map_rejects_malformed_duplicate_and_empty_mappings(self) -> None:
        cases = {
            "malformed crate": self.CRATE_MAP.replace(
                "| `gordian-core` |", "| gordian-core |", 1
            ),
            "duplicate crate": self.CRATE_MAP.replace(
                "| `gordian-git` |", "| `gordian-jj` |", 1
            ),
            "duplicate atom": self.CRATE_MAP.replace("| #1 |", "| #1, #1 |"),
            "empty owners": self.CRATE_MAP.replace("| #3 |", "| — |"),
            "path mismatch": self.CRATE_MAP.replace(
                "`crates/gordian-core`", "`crates/gordian-other`", 1
            ),
        }
        for label, malformed in cases.items():
            with self.subTest(label=label), self.assertRaises(ValueError):
                parse_crate_map(malformed)

    def test_plan_target_cells_must_match_crate_map_reverse_index(self) -> None:
        issues = tuple(_record(number) for number in (1, 2, 3, 7, 34))
        disagreement = self.PLAN.replace(
            "| #7 | Conformance | — |", "| #7 | Conformance | `gordian-jj` |"
        )
        report = audit_target_crate_contracts(issues, disagreement, self.CRATE_MAP)
        self.assertFalse(report.clean)
        self.assertIn("#7: project-plan target crates", "\n".join(report.problems))

    def test_renderer_uses_crate_map_and_is_idempotent_with_neutral_wording(self) -> None:
        issues = tuple(_record(number) for number in (1, 2, 3, 7, 34))
        first = render_target_crate_bodies(issues, self.PLAN, self.CRATE_MAP)
        updated = tuple(replace(issue, body=first[issue.number]) for issue in issues)
        second = render_target_crate_bodies(updated, self.PLAN, self.CRATE_MAP)
        self.assertEqual(first, second)
        self.assertIn("Rust target: `crates/gordian-git`.", first[3])
        self.assertNotIn("Production Rust", first[3])
        self.assertTrue(
            audit_target_crate_contracts(updated, self.PLAN, self.CRATE_MAP).clean
        )

    def test_issue_body_paths_and_packages_must_be_owned_or_explicit_shared_tests(self) -> None:
        issues = tuple(_record(number) for number in (1, 2, 3, 7, 34))
        rendered = render_target_crate_bodies(issues, self.PLAN, self.CRATE_MAP)
        clean = tuple(replace(issue, body=rendered[issue.number]) for issue in issues)
        shared = replace(
            clean[3],
            body=(
                clean[3].body
                + "\n`cargo test -p gordian-core --test conformance`\n"
                + "Test fixture: `crates/gordian-core/tests/conformance.rs`.\n"
            ),
        )
        shared = (*clean[:3], shared, *clean[4:])
        self.assertTrue(
            audit_target_crate_contracts(shared, self.PLAN, self.CRATE_MAP).clean
        )

        wrong = replace(
            clean[0],
            body=clean[0].body + "\n`cargo test -p gordian-jj`\ncrates/gordian-missing\n",
        )
        report = audit_target_crate_contracts(
            (wrong, *clean[1:]), self.PLAN, self.CRATE_MAP
        )
        joined = "\n".join(report.problems)
        self.assertIn("cargo package 'gordian-jj'", joined)
        self.assertIn("unknown path 'gordian-missing'", joined)

    def test_git_adapter_may_reference_source_only_in_shared_contract_tests(self) -> None:
        crate_map = self.CRATE_MAP.replace(
            "| `gordian-git` | `crates/gordian-git` | `gordian-core` | #3 |",
            "| `gordian-git` | `crates/gordian-git` | `gordian-core` | #3, #76 |",
        ) + "| `gordian-source` | `crates/gordian-source` | (none) | #29 |\n"
        plan = self.PLAN.replace(
            "| #34 | Comparison | — | — |",
            "| #34 | Comparison | — | — |\n| #29 | Source adapter | `gordian-source` | — |\n"
            "| #76 | Git adapter | `gordian-git` | — |",
        )
        issue = _record(76)
        source = _record(29)
        owners = tuple(_record(number) for number in (1, 2, 3))
        base_issues = (*owners, source, issue)
        rendered = render_target_crate_bodies(base_issues, plan, crate_map)[76]
        rendered_bodies = render_target_crate_bodies(base_issues, plan, crate_map)
        rendered_issues = tuple(
            replace(record, body=rendered_bodies[record.number])
            for record in base_issues
        )
        allowed = replace(
            issue,
            body=(
                rendered
                + "\n## Verification\nShared suite: "
                "`crates/gordian-source/tests/adapter_contract.rs`.\n"
            ),
        )
        self.assertTrue(
            audit_target_crate_contracts(
                (*rendered_issues[:-1], allowed), plan, crate_map
            ).clean
        )

        rejected = replace(
            allowed,
            body=(
                rendered
                + "\n## Objective\nAdapter implementation path: "
                "`crates/gordian-source/src/lib.rs`.\n"
            ),
        )
        report = audit_target_crate_contracts(
            (*rendered_issues[:-1], rejected), plan, crate_map
        )
        self.assertIn("issue body references path 'gordian-source'", "\n".join(report.problems))

    def test_generated_markers_outside_target_section_are_rejected(self) -> None:
        issues = tuple(_record(number) for number in (1, 2, 3, 7, 34))
        rendered = render_target_crate_bodies(issues, self.PLAN, self.CRATE_MAP)
        body = (
            rendered[1]
            + f"\n## Acceptance note\n{TARGET_CRATE_BEGIN}\nstray\n{TARGET_CRATE_END}\n"
        )
        bad = tuple(
            replace(issue, body=body)
            if issue.number == 1
            else replace(issue, body=rendered[issue.number])
            for issue in issues
        )
        report = audit_target_crate_contracts(bad, self.PLAN, self.CRATE_MAP)
        self.assertIn("marker is outside", "\n".join(report.problems))


class ProjectionCoherenceTests(unittest.TestCase):
    def test_new_atom_projection_generates_and_validates_target_crate_body(self) -> None:
        issue = _record(2, title="[Fixture] New owner")
        issue = replace(
            issue,
            body=(
                "## Initiative\nFixture Initiative\n\n"
                "## Dependencies\nNone\n\n"
                "## Acceptance\n- exact\n\n"
                "## Verification\n- exact\n\n"
                "## Closure\n- exact\n"
            ),
        )
        spec = NewAtomSpec(
            title=issue.title,
            body=issue.body + "\ncrates/gordian-core\n",
            milestone="Fixture Initiative",
            type_label="type:atom",
            target_crate="gordian-core",
            phase=6,
            blocked_by=(),
            blocks=(1,),
            knowledge_node={"@id": "artifact:fixture", "@type": ["Artifact"]},
        )
        project_plan = """# Initiative: Fixture Initiative

| Atom | Title | Target crate | Blocked by |
| --- | --- | --- | --- |
| #1 | Atom 1 | `gordian-core` | — |
"""
        execution_order = """## 6. Fixture phase

### Members

- #1 Atom 1;
"""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            plan_path = root / "project-plan.md"
            order_path = root / "execution-order.md"
            plan_path.write_text(project_plan, encoding="utf-8")
            order_path.write_text(execution_order, encoding="utf-8")
            (root / "issue-index.md").write_text("index", encoding="utf-8")
            (root / "crate-map.md").write_text(
                "| Crate | Path | May depend on (complete list) | Owning Atoms |\n"
                "| --- | --- | --- | --- |\n"
                "| `gordian-core` | `crates/gordian-core` | (none) | #1 |\n",
                encoding="utf-8",
            )
            (root / "graph.jsonld").write_text("{}", encoding="utf-8")
            arguments = SimpleNamespace(
                project_plan=plan_path,
                execution_order=order_path,
                issue_index=root / "issue-index.md",
                crate_map=root / "crate-map.md",
                knowledge_graph=root / "graph.jsonld",
                target=69,
            )
            owner = _record(1)
            owner = replace(
                owner,
                body=render_target_crate_bodies(
                    (owner,), project_plan, (root / "crate-map.md").read_text()
                )[1],
            )
            issues = (owner, issue)
            with (
                patch(
                    "gordian_orchestration.atom_registry.render_spine_block",
                    return_value="spine",
                ),
                patch(
                    "gordian_orchestration.atom_registry.render_selfhosting_block",
                    return_value="self-hosting",
                ),
                patch(
                    "gordian_orchestration.atom_registry.replace_spine_block",
                    side_effect=lambda text, replacement: text,
                ),
                patch(
                    "gordian_orchestration.atom_registry.replace_selfhosting_block",
                    side_effect=lambda text, replacement: text,
                ),
                patch(
                    "gordian_orchestration.atom_registry.render_issue_index",
                    return_value="index",
                ),
                patch(
                    "gordian_orchestration.atom_registry.append_knowledge_node",
                    return_value="graph",
                ),
                patch(
                    "gordian_orchestration.atom_registry.audit",
                    return_value=SimpleNamespace(clean=True, problems=()),
                ),
            ):
                first = _render_new_atom_projection(
                    issues,
                    number=2,
                    spec=spec,
                    arguments=arguments,
                )
                second = _render_new_atom_projection(
                    tuple(
                        replace(item, body=first["issue_body"])
                        if item.number == 2
                        else item
                        for item in issues
                    ),
                    number=2,
                    spec=replace(spec, body=first["issue_body"]),
                    arguments=arguments,
                )

        self.assertIn(TARGET_CRATE_BEGIN, first["issue_body"])
        self.assertIn("Rust target: `crates/gordian-core`.", first["issue_body"])
        self.assertEqual(first["issue_body"], second["issue_body"])
        projected = replace(issue, body=first["issue_body"])
        report = audit_target_crate_contracts(
            (owner, projected),
            project_plan.replace(
                "| #1 | Atom 1 | `gordian-core` | — |",
                "| #1 | Atom 1 | `gordian-core` | — |\n"
                "| #2 | [Fixture] New owner | `gordian-core` | — |",
            ),
        )
        self.assertTrue(report.clean, "\n".join(report.problems))


class NewAtomTests(unittest.TestCase):
    BODY = (
        "## Initiative\nTemporary GitHub Bootstrap\n\n"
        "## Dependencies\nNone\n\n"
        "## Acceptance\n- registered\n\n"
        "## Verification\n- drift check\n\n"
        "## Closure\n- exact evidence\n"
    )

    def test_plan_requires_and_establishes_a_mission_completion_path(self) -> None:
        issues = _fixture_issues()
        spec = NewAtomSpec(
            title="[Bootstrap] Fixture registration",
            body=self.BODY,
            milestone="Temporary GitHub Bootstrap",
            type_label="type:atom",
            target_crate=None,
            phase=13,
            blocked_by=(),
            blocks=(69,),
            knowledge_node={
                "@id": "artifact:fixture-registration",
                "@type": ["ImplementationArtifact"],
                "name": "Fixture registration",
                "summary": "Test-only new Atom registration fixture.",
            },
        )
        plan, proposed = plan_new_atom(issues, spec)
        self.assertEqual(plan.provisional_number, 78)
        records = {issue.number: issue for issue in proposed}
        self.assertIn(78, records[69].blocked_by)
        self.assertEqual(dependencies_from_body(records[69].body), records[69].blocked_by)

        with self.assertRaisesRegex(ValueError, "must --block"):
            plan_new_atom(issues, replace(spec, blocks=()))

    def test_new_atom_cli_dry_run_renders_every_repository_projection(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            body = Path(directory) / "body.md"
            node = Path(directory) / "node.json"
            snapshot = Path(directory) / "issues.json"
            _fixture_snapshot(snapshot)
            body.write_text(self.BODY, encoding="utf-8")
            node.write_text(
                json.dumps(
                    {
                        "@id": "artifact:dry-run-registration",
                        "@type": ["ImplementationArtifact"],
                        "name": "Dry-run registration",
                        "summary": "Test-only dry-run fixture.",
                    }
                ),
                encoding="utf-8",
            )
            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                status = registry_main(
                    [
                        "--snapshot",
                        str(snapshot),
                        "new-atom",
                        "--title",
                        "[Bootstrap] Dry-run registration",
                        "--body-file",
                        str(body),
                        "--milestone",
                        "Temporary GitHub Bootstrap",
                        "--type-label",
                        "type:atom",
                        "--target-crate",
                        "none",
                        "--phase",
                        "13",
                        "--blocks",
                        "69",
                        "--knowledge-node",
                        str(node),
                    ]
                )
        self.assertEqual(status, 2)
        self.assertEqual(stdout.getvalue(), "")
        self.assertIn("non-coherent registry", stderr.getvalue())

    def test_new_atom_apply_reuses_marker_owned_issue_without_create(self) -> None:
        recovered = replace(_record(78), body=self.BODY + "\n<!-- marker -->\n")
        with tempfile.TemporaryDirectory() as directory:
            body = Path(directory) / "body.md"
            node = Path(directory) / "node.json"
            journal = Path(directory) / "journal.json"
            body.write_text(self.BODY, encoding="utf-8")
            node.write_text(json.dumps({"@id": "artifact:recovered"}), encoding="utf-8")
            stdout = io.StringIO()
            lease = SimpleNamespace(actor="agent", lease_id="lease")
            with (
                patch(
                    "gordian_orchestration.atom_registry.preflight",
                    return_value=SimpleNamespace(login="tester"),
                ),
                patch("gordian_orchestration.atom_registry.require_live_claim", return_value=lease),
                patch("gordian_orchestration.atom_registry._records", return_value=(recovered,)),
                patch(
                    "gordian_orchestration.atom_registry._find_operation_issue",
                    return_value=recovered,
                ),
                patch("gordian_orchestration.atom_registry._coherence_reports", return_value=()),
                patch("gordian_orchestration.atom_registry._coherence_problems", return_value=()),
                patch("gordian_orchestration.atom_registry._create_issue") as create,
                redirect_stdout(stdout),
            ):
                status = registry_main([
                    "--repository", "example/repo", "new-atom", "--apply",
                    "--title", "[Bootstrap] Recovered", "--body-file", str(body),
                    "--milestone", "Temporary GitHub Bootstrap", "--type-label", "type:atom",
                    "--target-crate", "none", "--phase", "13", "--blocks", "69",
                    "--knowledge-node", str(node), "--journal", str(journal),
                ])
        self.assertEqual(status, 0)
        self.assertTrue(json.loads(stdout.getvalue())["already_complete"])
        create.assert_not_called()

    def test_repository_registration_transforms_are_narrow(self) -> None:
        issue = _record(80, title="[Fixture] New", milestone="Fixture Initiative")
        plan = """# Initiative: Fixture Initiative

| Atom | Title | Target crate | Blocked by |
| --- | --- | --- | --- |
| #1 | Existing | — | — |
"""
        updated_plan = insert_project_plan_atom(plan, issue, "gordian-core")
        self.assertIn("| #80 | [Fixture] New | `gordian-core` | — |", updated_plan)

        order = """## 6. Fixture phase

### Members

- #1 existing;

### Exit
"""
        updated_order = insert_phase_member(order, issue, 6)
        self.assertIn("- #80 [Fixture] New;", updated_order)

        crate_map = (
            "| Crate | Path | May depend on (complete list) | Owning Atoms |\n"
            "| --- | --- | --- | --- |\n"
            "| `gordian-core` | `crates/gordian-core` | (none) | #1, #2 |\n"
        )
        self.assertIn("| #1, #2, #80 |", add_crate_owner(crate_map, "gordian-core", 80))

    def test_knowledge_node_append_preserves_document_and_sets_issue_url(self) -> None:
        graph = '{\n  "@context": {},\n  "@graph": [\n    {"@id":"artifact:old"}\n  ]\n}\n'
        updated = append_knowledge_node(
            graph,
            {
                "@id": "artifact:new",
                "@type": ["ImplementationArtifact"],
                "name": "New",
                "summary": "New node",
            },
            "https://github.com/kmosoti/gordian/issues/80",
        )
        payload = json.loads(updated)
        self.assertEqual(len(payload["@graph"]), 2)
        self.assertEqual(
            payload["@graph"][1]["url"],
            "https://github.com/kmosoti/gordian/issues/80",
        )
        self.assertIn('{"@id":"artifact:old"}', updated)

    def test_native_edge_helpers_use_versioned_rest_and_database_id(self) -> None:
        with (
            patch(
                "gordian_orchestration.atom_registry.run_gh_json",
                return_value={"id": 9001},
            ),
            patch("gordian_orchestration.atom_registry.run_gh", return_value="") as run_gh,
        ):
            _add_native_edge("kmosoti/gordian", 80, 2)
            _remove_native_edge("kmosoti/gordian", 80, 2)
        add_argv, remove_argv = [entry.args[0] for entry in run_gh.call_args_list]
        self.assertEqual(add_argv[0:3], ["api", "--method", "POST"])
        self.assertTrue(
            any(value.endswith("issues/80/dependencies/blocked_by") for value in add_argv)
        )
        self.assertIn("issue_id=9001", add_argv)
        self.assertEqual(remove_argv[0:3], ["api", "--method", "DELETE"])
        self.assertTrue(
            any(
                value.endswith("issues/80/dependencies/blocked_by/9001")
                for value in remove_argv
            )
        )


if __name__ == "__main__":
    unittest.main()
