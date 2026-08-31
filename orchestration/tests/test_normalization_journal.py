from __future__ import annotations

import json
import re
import tempfile
import unittest
from copy import deepcopy
from dataclasses import replace
from pathlib import Path

from gordian_orchestration.atom_registry import parse_crate_map
from gordian_orchestration.bootstrap_claims import ClaimLease
from gordian_orchestration.derive_status import IssueRecord
from gordian_orchestration.normalization_journal import (
    CANONICAL_ATOM_NUMBERS,
    CANONICAL_CLOSURE_TEMPLATE,
    DEFAULT_MANIFEST,
    NormalizationConflict,
    NormalizationError,
    _dependencies,
    advance_journal,
    body_digest,
    canonical_closure_intent,
    compensate_bodies,
    initial_journal,
    journal_complete,
    load_manifest,
    operation_plan_digest,
    parse_manifest,
    plan_from_json,
    plan_normalization,
    read_journal,
    recover_journal,
    write_journal,
)

SEMANTIC_REPLACEMENT_ATOMS = frozenset(
    {4, 10, 11, 12, 13, 15, 18, 19, 22, 23, 26, 31, 32, 41, 49, 54, 67, 69, 70, 76}
)


def _body(*, wording: str = "old", blockers: tuple[int, ...] = ()) -> str:
    dependencies = "None" if not blockers else "\n".join(f"- #{number}" for number in blockers)
    return (
        f"## Objective\n{wording}\n\n"
        f"## Dependencies\n{dependencies}\n\n"
        "## Closure\nold closure\n"
    )


def _issue(number: int, *, body: str | None = None, blockers: tuple[int, ...] = ()) -> IssueRecord:
    return IssueRecord(
        number=number,
        title=f"Atom {number}",
        state="OPEN",
        blocked_by=blockers,
        body=body if body is not None else _body(blockers=blockers),
        labels=("type:atom",),
        milestone="Fixture",
        url=f"https://github.com/example/repo/issues/{number}",
    )


def _manifest(*transforms: dict) -> object:
    # Fixture manifests use the same external source-input anchor as production
    # manifests, computed from the fixture issues at plan construction time.
    # Callers may override this below when their issue set differs.
    numbers = {
        int(transform["issue"])
        for transform in transforms
        if "issue" in transform
    }
    for transform in transforms:
        numbers.update(int(value) for value in transform.get("add_blocked_by", ()))
    expected = {
        int(transform["issue"]): transform
        for transform in transforms
        if "issue" in transform
    }
    fixture_issues = tuple(
        replace(
            _issue(number),
            title=str(expected.get(number, {}).get("expected_title", f"Atom {number}")),
            state=str(expected.get(number, {}).get("expected_state", "OPEN")),
        )
        for number in sorted(numbers)
    )
    payload = {
        "format": "gordian-atom-contract-normalization-v1",
        "atom": 70,
        "repository": "example/repo",
        "original_plan_sha256": "0" * 64,
        "transforms": list(transforms),
    }
    provisional = parse_manifest(payload, repository="example/repo")
    payload["original_plan_sha256"] = operation_plan_digest(
        plan_normalization(fixture_issues, provisional, repository="example/repo").as_json_object()
    )
    return parse_manifest(payload, repository="example/repo")


class FakeLive:
    def __init__(self, issues: tuple[IssueRecord, ...]):
        self.issues = {issue.number: issue for issue in issues}
        self.patch_calls: list[tuple[str, int, str]] = []
        self.edge_calls: list[tuple[str, int, int]] = []
        self.label_calls: list[tuple[str, int, str]] = []
        self.remove_calls: list[tuple[int, int]] = []
        self.patch_hook = None
        self.edge_hook = None
        self.label_hook = None

    def fetch(self) -> tuple[IssueRecord, ...]:
        return tuple(self.issues[number] for number in sorted(self.issues))

    def patch(self, repository: str, number: int, body: str) -> None:
        self.patch_calls.append((repository, number, body))
        self.issues[number] = replace(self.issues[number], body=body)
        if self.patch_hook is not None:
            self.patch_hook(repository, number, body)

    def add_edge(self, repository: str, issue: int, blocker: int) -> None:
        self.edge_calls.append((repository, issue, blocker))
        current = self.issues[issue]
        self.issues[issue] = replace(
            current,
            blocked_by=tuple(sorted((*current.blocked_by, blocker))),
        )
        if self.edge_hook is not None:
            self.edge_hook(repository, issue, blocker)

    def add_label(self, repository: str, issue: int, label: str) -> None:
        self.label_calls.append((repository, issue, label))
        current = self.issues[issue]
        self.issues[issue] = replace(
            current,
            labels=tuple(sorted((*current.labels, label))),
        )
        if self.label_hook is not None:
            self.label_hook(repository, issue, label)


class ManifestTests(unittest.TestCase):
    def test_body_digest_is_sensitive_to_line_endings(self) -> None:
        self.assertNotEqual(body_digest("line\n"), body_digest("line\r\n"))

    def test_plan_round_trip_preserves_exact_old_body_bytes(self) -> None:
        old_body = _body().replace("\n", "\r\n")
        plan = plan_normalization(
            (_issue(1, body=old_body),),
            _manifest({"issue": 1}),
            repository="example/repo",
        )
        restored = plan_from_json(plan.as_json_object())
        operation = restored.bodies[0]
        self.assertEqual(operation.old_body, old_body)
        self.assertEqual(operation.old_digest, body_digest(old_body))

    def test_manifest_canonicalizes_transforms_and_rejects_executable_fields(self) -> None:
        manifest = _manifest(
            {
                "issue": 1,
                "body": _body(wording="new"),
                "closure_wording": "accepted closure",
                "add_blocked_by": [2],
            }
        )
        self.assertEqual(manifest.transforms[0].issue, 1)
        self.assertEqual(manifest.transforms[0].add_blocked_by, (2,))
        with self.assertRaises(ValueError):
            _manifest({"issue": 1, "shell": "gh issue edit"})

    def test_plan_is_deterministic_and_contains_exact_body_preconditions(self) -> None:
        issues = (_issue(1), _issue(2))
        manifest = _manifest({"issue": 1, "body": _body(wording="new"), "add_blocked_by": [2]})
        first = plan_normalization(issues, manifest, repository="example/repo")
        second = plan_normalization(issues, manifest, repository="example/repo")
        self.assertEqual(first.as_json_object(), second.as_json_object())
        operation = first.bodies[0]
        self.assertEqual(operation.old_digest, body_digest(operation.old_body))
        self.assertEqual(operation.proposed_digest, body_digest(operation.proposed_body))

    def test_plan_repairs_body_to_native_edges_without_inventing_edge_operations(self) -> None:
        issue = _issue(1, body=_body(blockers=(9,)), blockers=())
        plan = plan_normalization(
            (issue,),
            _manifest({"issue": 1}),
            repository="example/repo",
        )

        self.assertEqual(plan.edges, ())
        self.assertEqual(_dependencies(plan.bodies[0].proposed_body), ())

    def test_committed_manifest_covers_atoms_targets_edges_and_duplicate_labels(self) -> None:
        manifest = load_manifest(DEFAULT_MANIFEST, repository="kmosoti/gordian")
        transforms = {transform.issue: transform for transform in manifest.transforms}
        self.assertEqual(
            set(transforms), set(CANONICAL_ATOM_NUMBERS) | {78, 79}
        )

        crate_map = parse_crate_map(Path("docs/implementation/crate-map.md").read_text())
        for number in CANONICAL_ATOM_NUMBERS:
            transform = transforms[number]
            self.assertTrue(transform.target_crates_set)
            self.assertEqual(transform.target_crates or (), crate_map.get(number, ()))
            self.assertEqual(transform.closure_template, CANONICAL_CLOSURE_TEMPLATE)
            self.assertEqual(
                transform.closure_verifier_ids,
                (f"atom-{number}-acceptance",),
            )
            self.assertEqual(
                transform.as_json_object()["closure_wording"],
                canonical_closure_intent((f"atom-{number}-acceptance",)),
            )
            if number in SEMANTIC_REPLACEMENT_ATOMS:
                self.assertTrue(transform.replacements)
            else:
                self.assertEqual(transform.replacements, ())

        self.assertEqual(
            [
                (replacement.old, replacement.new, replacement.count)
                for replacement in transforms[26].replacements
                if "mandatory projections" in replacement.old
            ],
            [
                (
                    "covering every one of the eleven mandatory projections named in Acceptance",
                    "covering every one of the sixteen mandatory projections named in Acceptance",
                    1,
                )
            ],
        )
        self.assertEqual(
            [
                (replacement.old, replacement.new, replacement.count)
                for replacement in transforms[49].replacements
                if replacement.old == "the PlanRevision #48 imported"
            ],
            [
                (
                    "the PlanRevision #48 imported",
                    "the selected PlanRevision of the separate runtime Mission",
                    1,
                )
            ],
        )
        # The generated payload must not silently retain the stale projection
        # count or the old self-hosting-plan wording in the replacement output.
        self.assertNotIn(
            "eleven mandatory projections",
            "\n".join(replacement.new for replacement in transforms[26].replacements),
        )
        self.assertNotIn(
            "PlanRevision #48 imported",
            "\n".join(replacement.new for replacement in transforms[49].replacements),
        )

        self.assertEqual(
            {
                (transform.issue, blocker)
                for transform in manifest.transforms
                for blocker in transform.add_blocked_by
            },
            set(),
        )
        self.assertEqual(transforms[78].add_labels, ("duplicate",))
        self.assertEqual(transforms[79].add_labels, ("duplicate",))
        self.assertEqual(transforms[78].expected_state, "CLOSED")
        self.assertEqual(transforms[79].expected_state, "CLOSED")
        self.assertEqual(
            transforms[78].expected_title,
            "[Jujutsu Change Plane] Implement Git worktree source adapter behind "
            "the source-adapter trait",
        )
        self.assertEqual(
            transforms[79].expected_title,
            "[Agent Execution] Launch and supervise experiment workers",
        )

    def test_canonical_plan_injects_verifier_and_preserves_verification_prose(self) -> None:
        body = (
            "## Objective\nkeep objective\n\n"
            "## Verification\nHuman verification prose survives.\n\n"
            "## Closure\nlegacy closure\n"
        )
        issue = _issue(1, body=body)
        manifest = _manifest(
            {
                "issue": 1,
                "target_crates": [],
                "closure_wording": canonical_closure_intent(("atom-1-acceptance",)),
            }
        )
        operation = plan_normalization((issue,), manifest, repository="example/repo").bodies[0]
        self.assertIn("Human verification prose survives.", operation.proposed_body)
        self.assertIn("<!-- BEGIN GENERATED: ATOM ACCEPTANCE VERIFIER -->", operation.proposed_body)
        self.assertIn("verifier_id: `atom-1-acceptance`", operation.proposed_body)
        self.assertEqual(
            len(re.findall(r"^## Verification[ \t]*$", operation.proposed_body, re.MULTILINE)),
            1,
        )
        self.assertEqual(
            len(re.findall(r"^## Closure[ \t]*$", operation.proposed_body, re.MULTILINE)),
            1,
        )
        self.assertIn("artifacts/atoms/1/closure.json", operation.proposed_body)
        self.assertIn(
            "artifacts/atoms/1/verifiers/atom-1-acceptance.log",
            operation.proposed_body,
        )

    def test_canonical_plan_creates_missing_verification_section(self) -> None:
        issue = _issue(
            7,
            body="## Objective\nkeep objective\n\n## Closure\nlegacy closure\n",
        )
        manifest = _manifest(
            {
                "issue": 7,
                "target_crates": [],
                "closure_wording": canonical_closure_intent(("atom-7-acceptance",)),
            }
        )
        operation = plan_normalization((issue,), manifest, repository="example/repo").bodies[0]
        self.assertIn("## Objective\nkeep objective", operation.proposed_body)
        self.assertEqual(
            len(re.findall(r"^## Verification[ \t]*$", operation.proposed_body, re.MULTILINE)),
            1,
        )
        self.assertIn("verifier_id: `atom-7-acceptance`", operation.proposed_body)

    def test_canonical_plan_collapses_duplicate_closure_sections(self) -> None:
        issue = _issue(
            70,
            body=(
                "## Objective\nkeep objective\n\n"
                "## Closure\nfirst legacy closure\n\n"
                "## Retirement\nretirement prose survives\n\n"
                "## Verification\nverification prose survives\n\n"
                "## Closure\nsecond legacy closure\n"
            ),
        )
        manifest = _manifest(
            {
                "issue": 70,
                "target_crates": [],
                "closure_wording": canonical_closure_intent(("atom-70-acceptance",)),
            }
        )
        operation = plan_normalization((issue,), manifest, repository="example/repo").bodies[0]
        self.assertEqual(
            len(re.findall(r"^## Closure[ \t]*$", operation.proposed_body, re.MULTILINE)),
            1,
        )
        self.assertIn("retirement prose survives", operation.proposed_body)
        self.assertIn("verification prose survives", operation.proposed_body)
        self.assertIn("verifier_id: `atom-70-acceptance`", operation.proposed_body)


class JournalTests(unittest.TestCase):
    def _plan(self, issue: IssueRecord | None = None, *, edge: bool = False):
        issue = issue or _issue(1)
        issues = (issue, _issue(2)) if edge else (issue,)
        transform = {"issue": 1, "body": _body(wording="new")}
        if edge:
            transform["add_blocked_by"] = [2]
        return plan_normalization(
            issues,
            _manifest(transform),
            repository="example/repo",
        )

    def _label_plan(self, issue: IssueRecord | None = None):
        issue = issue or replace(_issue(78), state="CLOSED")
        manifest = _manifest(
            {
                "issue": issue.number,
                "add_labels": ["duplicate"],
                "expected_title": issue.title,
                "expected_state": issue.state,
            }
        )
        return plan_normalization((issue,), manifest, repository="example/repo")

    def test_plan_from_json_rejects_coherent_operation_tampering(self) -> None:
        plan = self._plan(edge=True)
        original = plan.as_json_object()
        cases: list[tuple[str, callable]] = []

        def body(payload):
            operation = payload["body_operations"][0]
            operation["proposed_body"] = _body(wording="tampered")
            operation["proposed_body_sha256"] = body_digest(operation["proposed_body"])

        def edge(payload):
            payload["edge_operations"][0]["blocked_by"] = 1

        def label(payload):
            payload["label_operations"] = [{
                "issue": 1, "label": "type:atom", "expected_title": "tampered",
                "expected_state": "OPEN", "was_present": True,
            }]

        cases.extend([("body", body), ("edge", edge), ("label", label)])
        for name, mutate in cases:
            with self.subTest(name=name):
                payload = deepcopy(original)
                mutate(payload)
                with self.assertRaises(NormalizationError):
                    plan_from_json(payload)

        for name, mutate in (
            (
                "add",
                lambda payload: payload["body_operations"].append(
                    deepcopy(payload["body_operations"][0])
                ),
            ),
            ("delete", lambda payload: payload["body_operations"].pop()),
            (
                "duplicate",
                lambda payload: payload["edge_operations"].append(
                    deepcopy(payload["edge_operations"][0])
                ),
            ),
        ):
            with self.subTest(name=name):
                payload = deepcopy(original)
                mutate(payload)
                with self.assertRaises(NormalizationError):
                    plan_from_json(payload)

    def test_stale_label_title_or_state_refuses_without_add(self) -> None:
        for field in ("title", "state"):
            with self.subTest(field=field):
                live = FakeLive((replace(_issue(78), state="CLOSED"),))
                plan = self._label_plan(live.issues[78])
                changed = {"title": "renamed", "state": "OPEN"}
                live.issues[78] = replace(live.issues[78], **{field: changed[field]})
                with tempfile.TemporaryDirectory() as directory:
                    journal_path = Path(directory) / "journal.json"
                    with self.assertRaises(NormalizationConflict):
                        advance_journal(
                            plan,
                            journal_path,
                            writer="agent",
                            fetch_records=live.fetch,
                            add_label=live.add_label,
                        )
                    journal = read_journal(journal_path)
                self.assertEqual(live.label_calls, [])
                self.assertEqual(journal["operations"][0]["state"], "conflict")
                self.assertEqual(
                    journal["operations"][0]["conflict_reason"],
                    "stale-label-title-state",
                )

    def test_ambiguous_label_response_recovers_from_fresh_label_read(self) -> None:
        live = FakeLive((_issue(78),))
        plan = self._label_plan(live.issues[78])

        def fail_after_add(_repository: str, _issue: int, _label: str) -> None:
            raise RuntimeError("transport lost")

        live.label_hook = fail_after_add
        with tempfile.TemporaryDirectory() as directory:
            journal_path = Path(directory) / "journal.json"
            with self.assertRaises(RuntimeError):
                advance_journal(
                    plan,
                    journal_path,
                    writer="agent",
                    fetch_records=live.fetch,
                    add_label=live.add_label,
                )
            self.assertEqual(read_journal(journal_path)["operations"][0]["state"], "ambiguous")
            live.label_hook = None
            recovered = recover_journal(
                journal_path,
                writer="agent",
                fetch_records=live.fetch,
                add_label=live.add_label,
                manifest=_manifest(
                    {"issue": 78, "add_labels": ["duplicate"],
                     "expected_title": "Atom 78", "expected_state": "OPEN"}
                ),
            )
            compensated = compensate_bodies(
                journal_path,
                writer="agent",
                fetch_records=live.fetch,
                patch_body=live.patch,
            )
        self.assertTrue(journal_complete(recovered))
        self.assertTrue(journal_complete(compensated))
        self.assertEqual(len(live.label_calls), 1)
        self.assertEqual(live.remove_calls, [])
        self.assertIn("duplicate", live.issues[78].labels)

    def test_label_verification_uses_single_issue_read_when_collection_filters_duplicates(
        self,
    ) -> None:
        live = FakeLive((_issue(78),))
        plan = self._label_plan(live.issues[78])

        def collection_without_duplicates() -> tuple[IssueRecord, ...]:
            return tuple(issue for issue in live.fetch() if "duplicate" not in issue.labels)

        with tempfile.TemporaryDirectory() as directory:
            journal_path = Path(directory) / "journal.json"
            result = advance_journal(
                plan,
                journal_path,
                writer="agent",
                fetch_records=collection_without_duplicates,
                add_label=live.add_label,
                fetch_label_record=lambda _repository, number: live.issues[number],
            )
            second_plan = plan_normalization(
                collection_without_duplicates(),
                _manifest(
                    {
                        "issue": 78,
                        "add_labels": ["duplicate"],
                        "expected_title": live.issues[78].title,
                        "expected_state": live.issues[78].state,
                    }
                ),
                repository="example/repo",
                fetch_label_record=lambda _repository, number: live.issues[number],
            )
            self.assertEqual(second_plan.changed_labels, ())
            second = advance_journal(
                second_plan,
                journal_path,
                writer="agent",
                fetch_records=collection_without_duplicates,
                add_label=live.add_label,
                fetch_label_record=lambda _repository, number: live.issues[number],
            )
        self.assertTrue(journal_complete(result))
        self.assertTrue(journal_complete(second))
        self.assertEqual(len(live.label_calls), 1)
        self.assertIn("duplicate", live.issues[78].labels)

    def test_same_login_different_actor_cannot_recover_bound_journal(self) -> None:
        live = FakeLive((_issue(1),))
        plan = self._plan()
        first = ClaimLease(
            actor="gordian-agent/harness/run-one",
            login="same-login",
            lease_id="a" * 32,
            claim_commit="b" * 40,
        )
        second = ClaimLease(
            actor="gordian-agent/harness/run-two",
            login="same-login",
            lease_id="c" * 32,
            claim_commit="d" * 40,
        )
        with tempfile.TemporaryDirectory() as directory:
            journal_path = Path(directory) / "journal.json"
            write_journal(initial_journal(plan, writer=first.actor, lease=first), journal_path)
            with self.assertRaisesRegex(NormalizationError, "owned by"):
                recover_journal(
                    journal_path,
                    writer=second.actor,
                    lease=second,
                    fetch_records=live.fetch,
                    patch_body=live.patch,
                )
        self.assertEqual(live.patch_calls, [])

    def test_stale_before_write_does_not_patch_and_persists_conflict(self) -> None:
        live = FakeLive((_issue(1),))
        plan = self._plan()
        live.issues[1] = replace(live.issues[1], body=_body(wording="external"))
        with tempfile.TemporaryDirectory() as directory:
            journal_path = Path(directory) / "journal.json"
            with self.assertRaises(NormalizationConflict):
                advance_journal(
                    plan,
                    journal_path,
                    writer="agent",
                    fetch_records=live.fetch,
                    patch_body=live.patch,
                )
            journal = read_journal(journal_path)
        self.assertEqual(live.patch_calls, [])
        self.assertEqual(journal["operations"][0]["state"], "conflict")
        self.assertEqual(journal["operations"][0]["conflict_reason"], "stale-before-write")

    def test_external_edit_after_write_is_not_overwritten_by_compensation(self) -> None:
        live = FakeLive((_issue(1),))
        plan = self._plan()

        def edit_after_write(_repository: str, _number: int, _body_text: str) -> None:
            live.issues[1] = replace(live.issues[1], body=_body(wording="someone else"))

        live.patch_hook = edit_after_write
        with tempfile.TemporaryDirectory() as directory:
            journal_path = Path(directory) / "journal.json"
            with self.assertRaises(NormalizationConflict):
                advance_journal(
                    plan,
                    journal_path,
                    writer="agent",
                    fetch_records=live.fetch,
                    patch_body=live.patch,
                )
            before = len(live.patch_calls)
            compensate_bodies(
                journal_path,
                writer="agent",
                fetch_records=live.fetch,
                patch_body=live.patch,
            )
            journal = read_journal(journal_path)
        self.assertEqual(len(live.patch_calls), before)
        self.assertEqual(journal["operations"][0]["state"], "conflict")
        self.assertTrue(
            any(
                item["reason"].startswith("compensation-")
                for item in journal["conflicts"]
            )
        )

    def test_interrupted_write_resumes_from_in_flight_state(self) -> None:
        live = FakeLive((_issue(1),))
        plan = self._plan()

        def interrupt_after_write(_repository: str, _number: int, _body_text: str) -> None:
            raise KeyboardInterrupt

        live.patch_hook = interrupt_after_write
        with tempfile.TemporaryDirectory() as directory:
            journal_path = Path(directory) / "journal.json"
            with self.assertRaises(KeyboardInterrupt):
                advance_journal(
                    plan,
                    journal_path,
                    writer="agent",
                    fetch_records=live.fetch,
                    patch_body=live.patch,
                )
            self.assertEqual(read_journal(journal_path)["operations"][0]["state"], "in_flight")
            live.patch_hook = None
            recovered = recover_journal(
                journal_path,
                writer="agent",
                fetch_records=live.fetch,
                patch_body=live.patch,
                manifest=_manifest({"issue": 1, "body": _body(wording="new")}),
            )
        self.assertTrue(journal_complete(recovered))
        self.assertEqual(len(live.patch_calls), 1)

    def test_reapplying_a_completed_normalization_is_idempotent(self) -> None:
        live = FakeLive((_issue(1),))
        plan = self._plan()
        manifest = _manifest({"issue": 1, "body": _body(wording="new")})
        with tempfile.TemporaryDirectory() as directory:
            journal_path = Path(directory) / "journal.json"
            first = advance_journal(
                plan,
                journal_path,
                writer="agent",
                fetch_records=live.fetch,
                patch_body=live.patch,
                manifest=manifest,
            )
            first["status"] = "completed"
            journal_path.write_text(json.dumps(first), encoding="utf-8")
            normalized_plan = self._plan(live.issues[1])
            second = advance_journal(
                normalized_plan,
                journal_path,
                writer="agent",
                fetch_records=live.fetch,
                patch_body=live.patch,
                manifest=manifest,
            )
        self.assertEqual(len(live.patch_calls), 1)
        self.assertEqual(second["status"], "completed")

    def test_guarded_compensation_restores_only_our_proposed_body(self) -> None:
        live = FakeLive((_issue(1),))
        plan = self._plan()
        with tempfile.TemporaryDirectory() as directory:
            journal_path = Path(directory) / "journal.json"
            advance_journal(
                plan,
                journal_path,
                writer="agent",
                fetch_records=live.fetch,
                patch_body=live.patch,
            )
            calls_before = len(live.patch_calls)
            journal = compensate_bodies(
                journal_path,
                writer="agent",
                fetch_records=live.fetch,
                patch_body=live.patch,
            )
        self.assertEqual(len(live.patch_calls), calls_before + 1)
        self.assertEqual(journal["operations"][0]["state"], "compensated")
        self.assertEqual(body_digest(live.issues[1].body), plan.bodies[0].old_digest)

    def test_ambiguous_edge_is_recorded_and_never_removed(self) -> None:
        live = FakeLive((_issue(1), _issue(2)))
        plan = self._plan(edge=True)

        def fail_edge(_repository: str, _issue: int, _blocker: int) -> None:
            raise RuntimeError("transport lost")

        live.edge_hook = fail_edge
        with tempfile.TemporaryDirectory() as directory:
            journal_path = Path(directory) / "journal.json"
            with self.assertRaises(RuntimeError):
                advance_journal(
                    plan,
                    journal_path,
                    writer="agent",
                    fetch_records=live.fetch,
                    patch_body=live.patch,
                    add_edge=live.add_edge,
                )
            journal = read_journal(journal_path)
        self.assertEqual(live.remove_calls, [])
        self.assertEqual(journal["operations"][-1]["kind"], "edge")
        self.assertEqual(journal["operations"][-1]["state"], "ambiguous")
        self.assertTrue(any(item["kind"] == "edge" for item in journal["conflicts"]))


if __name__ == "__main__":
    unittest.main()
