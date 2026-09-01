"""Deterministic claim-log tests; no test talks to GitHub."""

from __future__ import annotations

import io
import json
import os
import unittest
from contextlib import redirect_stderr
from dataclasses import replace
from datetime import UTC, datetime, timedelta
from pathlib import Path
from unittest.mock import patch

from gordian_orchestration import bootstrap_claims as claims
from gordian_orchestration.bootstrap_claims import (
    CLAIM_EVENT_SCHEMA,
    CLAIM_LOG_REF,
    CLAIM_MINUTES,
    MAX_ACTIVE_CLAIMS,
    ClaimLease,
    ClaimLogState,
    IssueClaim,
    _AppendResult,
    _CasConflict,
    _CommitSnapshot,
    _entry_object,
    _event_document,
    _event_object,
    _publish_ref,
    _read_claim_log,
    _RefSnapshot,
    _validate_event_document,
    adopt_legacy_claim,
    claim,
    main,
    parse_claim_comment,
    release,
    require_live_claim,
)
from gordian_orchestration.derive_status import Board, BoardItem, DerivedRow, IssueRecord
from gordian_orchestration.gh import GitHubApiResponse, PreflightReport

NOW = datetime(2026, 8, 31, 5, 0, tzinfo=UTC)
SERVER_DATE = "Mon, 31 Aug 2026 05:00:00 GMT"
ACTOR = "gordian-agent/codex/run-123"
OTHER_ACTOR = "gordian-agent/luna/run-456"
LOGIN = "agent-login"
OTHER_LOGIN = "luna-login"
HEAD = "a" * 40
MAIN = "b" * 40
TREE = "c" * 40
EVENT_ID = "d" * 32
LEASE_ID = "e" * 32
OTHER_LEASE_ID = "f" * 32

AUTH = PreflightReport(
    login=LOGIN,
    capabilities=(
        "authenticated_identity",
        "repository_write",
        "project_read",
        "project_write",
    ),
    credential_source="GH_TOKEN",
    repository="kmosoti/gordian",
    project_owner="kmosoti",
    project_number=9,
)


def _row(number: int, fan_out: int = 1) -> DerivedRow:
    return DerivedRow(
        number=number,
        title=f"Atom {number}",
        state="OPEN",
        wave=0,
        fan_in=0,
        fan_out=fan_out,
        status="Ready",
        blocked_by=(),
        unsatisfied_blockers=(),
    )


def _lease(
    atom: int = 2,
    *,
    actor: str = ACTOR,
    login: str = LOGIN,
    claimed_at: datetime | None = NOW,
    lease_id: str = LEASE_ID,
    claim_commit: str = HEAD,
    event_id: str = EVENT_ID,
) -> ClaimLease:
    return ClaimLease(
        actor=actor,
        claimed_at=claimed_at,
        expires_after_minutes=CLAIM_MINUTES,
        login=login,
        lease_id=lease_id,
        claim_commit=claim_commit,
        event_id=event_id,
        atom=atom,
    )


def _state(
    active: tuple[ClaimLease, ...] = (),
    *,
    head: str | None = HEAD,
    tree: str | None = TREE,
    event: dict[str, object] | None = None,
    server_now: datetime | None = NOW,
) -> ClaimLogState:
    return ClaimLogState(
        head_sha=head,
        tree_sha=tree,
        active_claims=active,
        event=event,
        server_now=server_now,
    )


def _claim_event(lease: ClaimLease | None = None) -> dict[str, object]:
    lease = lease or _lease()
    return _event_object(
        event_id=lease.event_id or EVENT_ID,
        event_type="claim",
        atom=lease.atom or 2,
        actor=lease.actor,
        login=lease.login,
        lease_id=lease.lease_id,
        reason="claim",
    )


def _append_result(lease: ClaimLease, state: ClaimLogState | None = None) -> _AppendResult:
    return _AppendResult(
        event_id=lease.event_id or EVENT_ID,
        commit_sha=state.head_sha if state and state.head_sha else HEAD,
        state=state or _state((lease,), event=_claim_event(lease)),
        lease=lease,
    )


class ClaimMessageTests(unittest.TestCase):
    def test_reason_redacts_the_credential(self) -> None:
        with patch.dict(
            os.environ,
            {"GH_TOKEN": "explicit-secret"},
            clear=True,
        ):
            event = _event_object(
                event_id=EVENT_ID,
                event_type="release",
                atom=2,
                actor=OTHER_ACTOR,
                login=OTHER_LOGIN,
                lease_id=LEASE_ID,
                reason="explicit-secret in the reason",
            )
            comment = claims._release_comment(
                ACTOR,
                event_id=EVENT_ID,
                lease_id=LEASE_ID,
                reason="explicit-secret in the reason",
            )
        self.assertEqual(event["reason"], "<redacted> in the reason")
        self.assertNotIn("explicit-secret", comment)
        self.assertNotIn("child-secret", comment)

    def test_canonical_message_contains_event_and_complete_state(self) -> None:
        lease = _lease(claim_commit="self")
        message = _event_document(
            _claim_event(lease),
            [
                _entry_object(
                    atom=2,
                    actor=ACTOR,
                    login=LOGIN,
                    lease_id=LEASE_ID,
                    claim_commit="self",
                )
            ],
        )
        event, active = _validate_event_document(message)
        self.assertEqual(event["type"], "claim")
        self.assertEqual(active[0]["claim_commit"], "self")
        self.assertEqual(
            json.dumps(json.loads(message), sort_keys=True, separators=(",", ":")), message
        )
        self.assertIn(CLAIM_EVENT_SCHEMA, message)

    def test_noncanonical_or_malformed_messages_are_rejected(self) -> None:
        lease = _lease(claim_commit="self")
        message = _event_document(_claim_event(lease), [_entry_object(
            atom=2,
            actor=ACTOR,
            login=LOGIN,
            lease_id=LEASE_ID,
            claim_commit="self",
        )])
        duplicate = message.replace(
            '"schema":"gordian-bootstrap-claim-event-v1"',
            '"schema":"gordian-bootstrap-claim-event-v1","schema":"gordian-bootstrap-claim-event-v1"',
        )
        for malformed in (message + "\n", message.replace(CLAIM_EVENT_SCHEMA, "wrong"), duplicate):
            with self.subTest(malformed=malformed[-30:]), self.assertRaises(RuntimeError):
                _validate_event_document(malformed)

    def test_active_state_rejects_duplicate_atom_and_cap_overflow(self) -> None:
        entries = [
            _entry_object(
                atom=index,
                actor=f"gordian-agent/harness/run-{index}",
                login=LOGIN,
                lease_id=f"{index:032x}",
                claim_commit="self",
            )
            for index in range(1, MAX_ACTIVE_CLAIMS + 1)
        ]
        event = _event_object(
            event_id=EVENT_ID,
            event_type="claim",
            atom=1,
            actor=entries[0]["actor"],
            login=LOGIN,
            lease_id=entries[0]["lease_id"],
            reason="claim",
        )
        valid = _event_document(event, entries)
        self.assertEqual(len(_validate_event_document(valid)[1]), MAX_ACTIVE_CLAIMS)
        overflow = dict(entries[-1])
        overflow["atom"] = MAX_ACTIVE_CLAIMS + 1
        overflow["actor"] = "gordian-agent/harness/run-overflow"
        overflow["lease_id"] = "1" * 32
        with self.assertRaisesRegex(RuntimeError, "active-claim cap"):
            _validate_event_document(_event_document(event, [*entries, overflow]))
        duplicate = [*entries]
        duplicate[-1] = dict(duplicate[-1], atom=1)
        with self.assertRaisesRegex(RuntimeError, "two active leases"):
            _validate_event_document(_event_document(event, duplicate))

    def test_projection_comment_carries_event_ref_and_lease(self) -> None:
        lease = _lease()
        body = claims._claim_comment(ACTOR, lease, EVENT_ID)
        parsed = parse_claim_comment(body)
        self.assertIsNotNone(parsed)
        assert parsed is not None
        self.assertEqual(parsed.lease_id, LEASE_ID)
        self.assertEqual(parsed.event_id, EVENT_ID)
        self.assertIn(CLAIM_LOG_REF, body)


class ClaimLogPrimitiveTests(unittest.TestCase):
    def _history_snapshots(
        self,
        *,
        malformed: str | None = None,
        release_at: datetime = NOW + timedelta(minutes=CLAIM_MINUTES),
        release_actor: str = OTHER_ACTOR,
        release_login: str = OTHER_LOGIN,
    ):
        root_sha = "1" * 40
        first_sha = "2" * 40
        second_sha = "3" * 40
        head_sha = "4" * 40
        first_lease = _lease(atom=2, claim_commit="self", event_id="1" * 32)
        second_lease = _lease(
            atom=3,
            actor=OTHER_ACTOR,
            login=OTHER_LOGIN,
            lease_id=OTHER_LEASE_ID,
            claim_commit="self",
            event_id="2" * 32,
        )
        first_entry = _entry_object(
            atom=2,
            actor=ACTOR,
            login=LOGIN,
            lease_id=LEASE_ID,
            claim_commit="self",
        )
        first_entry_after = _entry_object(
            atom=2,
            actor=ACTOR,
            login=LOGIN,
            lease_id=LEASE_ID,
            claim_commit=first_sha,
        )
        second_entry = _entry_object(
            atom=3,
            actor=OTHER_ACTOR,
            login=OTHER_LOGIN,
            lease_id=OTHER_LEASE_ID,
            claim_commit="self",
        )
        first_message = _event_document(_claim_event(first_lease), [first_entry])
        second_event = _event_object(
            event_id=second_lease.event_id,
            event_type="claim",
            atom=3,
            actor=OTHER_ACTOR,
            login=OTHER_LOGIN,
            lease_id=OTHER_LEASE_ID,
            reason="claim",
        )
        second_entries = [first_entry_after, second_entry]
        if malformed == "transition":
            second_entries = [
                _entry_object(
                    atom=2,
                    actor=ACTOR,
                    login=LOGIN,
                    lease_id=LEASE_ID,
                    claim_commit=root_sha,
                ),
                second_entry,
            ]
        second_message = _event_document(second_event, second_entries)
        head_message = _event_document(
            _event_object(
                event_id="3" * 32,
                event_type="release",
                atom=3,
                actor=release_actor,
                login=release_login,
                lease_id=OTHER_LEASE_ID,
                reason="release",
            ),
            [first_entry_after],
        )
        return {
            head_sha: _CommitSnapshot(
                head_sha,
                TREE,
                head_message,
                release_at,
                (second_sha,),
                NOW,
            ),
            second_sha: _CommitSnapshot(
                second_sha, TREE, second_message, NOW, (first_sha,), NOW
            ),
            first_sha: _CommitSnapshot(
                first_sha, TREE, first_message, NOW, (root_sha,), NOW
            ),
            root_sha: _CommitSnapshot(root_sha, TREE, "accepted main", NOW, (), NOW),
        }, head_sha, root_sha

    def test_server_date_is_required_and_local_now_is_not_authoritative(self) -> None:
        state = _state((
            _lease(),
        ), server_now=NOW)
        self.assertTrue(state.active_claims[0].is_live(NOW + timedelta(minutes=239)))
        self.assertFalse(state.active_claims[0].is_live(NOW + timedelta(minutes=240)))
        with self.assertRaisesRegex(RuntimeError, "no server Date"):
            claims._server_clock(_state(server_now=None))

    def test_foreign_release_is_rejected_regardless_of_advisory_expiry(self) -> None:
        entry = _entry_object(
            atom=2,
            actor=ACTOR,
            login=LOGIN,
            lease_id=LEASE_ID,
            claim_commit=HEAD,
        )
        event = _event_object(
            event_id=EVENT_ID,
            event_type="release",
            atom=2,
            actor=OTHER_ACTOR,
            login=OTHER_LOGIN,
            lease_id=LEASE_ID,
            reason="stale",
        )
        claim_commit = _CommitSnapshot(HEAD, TREE, "claim", NOW, (), NOW)
        before_expiry = replace(
            claim_commit, committed_at=NOW + timedelta(minutes=CLAIM_MINUTES - 1)
        )
        with self.assertRaisesRegex(RuntimeError, "different actor"):
            claims._validate_release_authority(
                parent_sha=HEAD,
                parent_entries=[entry],
                event=event,
                event_commit=before_expiry,
                commits={HEAD: claim_commit},
            )
        for offset in (CLAIM_MINUTES, CLAIM_MINUTES + 1):
            with self.assertRaisesRegex(RuntimeError, "different actor"):
                claims._validate_release_authority(
                    parent_sha=HEAD,
                    parent_entries=[entry],
                    event=event,
                    event_commit=replace(
                        claim_commit, committed_at=NOW + timedelta(minutes=offset)
                    ),
                    commits={HEAD: claim_commit},
                )

    def test_foreign_release_with_missing_canonical_time_fails_closed(self) -> None:
        entry = _entry_object(
            atom=2,
            actor=ACTOR,
            login=LOGIN,
            lease_id=LEASE_ID,
            claim_commit=HEAD,
        )
        event = _event_object(
            event_id=EVENT_ID,
            event_type="release",
            atom=2,
            actor=OTHER_ACTOR,
            login=OTHER_LOGIN,
            lease_id=LEASE_ID,
            reason="stale",
        )
        with self.assertRaisesRegex(RuntimeError, "different actor"):
            claims._validate_release_authority(
                parent_sha=HEAD,
                parent_entries=[entry],
                event=event,
                event_commit=_CommitSnapshot(HEAD, TREE, "release", NOW, (), NOW),
                commits={},
            )

    def test_transition_rejects_dropping_or_adding_unrelated_leases(self) -> None:
        first = _entry_object(
            atom=2,
            actor=ACTOR,
            login=LOGIN,
            lease_id=LEASE_ID,
            claim_commit="self",
        )
        second = _entry_object(
            atom=3,
            actor=OTHER_ACTOR,
            login=OTHER_LOGIN,
            lease_id=OTHER_LEASE_ID,
            claim_commit=MAIN,
        )
        release_event = _event_object(
            event_id=EVENT_ID,
            event_type="release",
            atom=2,
            actor=ACTOR,
            login=LOGIN,
            lease_id=LEASE_ID,
            reason="done",
        )
        with self.assertRaisesRegex(RuntimeError, "exactly one active lease"):
            claims._validate_transition(
                parent_sha=HEAD,
                parent_entries=[first, second],
                event=release_event,
                entries=[],
            )
        with self.assertRaisesRegex(RuntimeError, "exactly one active lease"):
            claims._validate_transition(
                parent_sha=HEAD,
                parent_entries=[first],
                event=release_event,
                entries=[second],
            )

    def test_reader_rejects_non_contiguous_claim_log_tree(self) -> None:
        lease = _lease(claim_commit="self")
        head = _CommitSnapshot(
            sha=HEAD,
            tree_sha=TREE,
            message=_event_document(_claim_event(lease), [_entry_object(
                atom=2,
                actor=ACTOR,
                login=LOGIN,
                lease_id=LEASE_ID,
                claim_commit="self",
            )]),
            committed_at=NOW,
            parents=(MAIN,),
            server_now=NOW,
        )
        parent = _CommitSnapshot(
            sha=MAIN,
            tree_sha="0" * 40,
            message="accepted main",
            committed_at=NOW,
            parents=(),
            server_now=NOW,
        )
        with (
            patch.object(claims, "_read_ref", return_value=_RefSnapshot(HEAD, NOW)),
            patch.object(claims, "_read_commit", side_effect=[head, parent]),
            self.assertRaisesRegex(RuntimeError, "changed the parent tree"),
        ):
            _read_claim_log("kmosoti/gordian")

    def test_reader_rejects_release_root_without_prior_claim_event(self) -> None:
        head = _CommitSnapshot(
            sha=HEAD,
            tree_sha=TREE,
            message=_event_document(
                _event_object(
                    event_id=EVENT_ID,
                    event_type="release",
                    atom=2,
                    actor=ACTOR,
                    login=LOGIN,
                    lease_id=LEASE_ID,
                    reason="done",
                ),
                [],
            ),
            committed_at=NOW,
            parents=(MAIN,),
            server_now=NOW,
        )
        parent = _CommitSnapshot(
            sha=MAIN,
            tree_sha=TREE,
            message="accepted main",
            committed_at=NOW,
            parents=(),
            server_now=NOW,
        )
        with (
            patch.object(claims, "_read_ref", return_value=_RefSnapshot(HEAD, NOW)),
            patch.object(claims, "_read_commit", side_effect=[head, parent]),
            self.assertRaisesRegex(RuntimeError, "root parent"),
        ):
            _read_claim_log("kmosoti/gordian")

    def test_reader_validates_every_ancestor_and_retains_history_for_recovery(self) -> None:
        snapshots, head_sha, root_sha = self._history_snapshots()
        with (
            patch.object(
                claims,
                "_read_ref",
                side_effect=[_RefSnapshot(head_sha, NOW), _RefSnapshot(root_sha, NOW)],
            ),
            patch.object(claims, "_read_commit", side_effect=lambda _, sha: snapshots[sha]),
        ):
            state = _read_claim_log("kmosoti/gordian")
        self.assertEqual(state.head_sha, head_sha)
        self.assertEqual(state.events[0]["type"], "release")
        self.assertEqual(len(state.events), 3)
        self.assertEqual(state.active_claims[0].claim_commit, "2" * 40)

    def test_reader_rejects_foreign_release_even_after_advisory_expiry(self) -> None:
        snapshots, head_sha, root_sha = self._history_snapshots(
            release_at=NOW + timedelta(minutes=CLAIM_MINUTES - 1),
            release_actor=ACTOR,
            release_login=LOGIN,
        )
        with (
            patch.object(
                claims,
                "_read_ref",
                side_effect=[_RefSnapshot(head_sha, NOW), _RefSnapshot(root_sha, NOW)],
            ),
            patch.object(claims, "_read_commit", side_effect=lambda _, sha: snapshots[sha]),
            self.assertRaisesRegex(RuntimeError, "different actor"),
        ):
            _read_claim_log("kmosoti/gordian")

    def test_reader_rejects_foreign_release_at_advisory_expiry(self) -> None:
        snapshots, head_sha, root_sha = self._history_snapshots(
            release_at=NOW + timedelta(minutes=CLAIM_MINUTES),
            release_actor=ACTOR,
            release_login=LOGIN,
        )
        with (
            patch.object(
                claims,
                "_read_ref",
                side_effect=[_RefSnapshot(head_sha, NOW), _RefSnapshot(root_sha, NOW)],
            ),
            patch.object(claims, "_read_commit", side_effect=lambda _, sha: snapshots[sha]),
            self.assertRaisesRegex(RuntimeError, "different actor"),
        ):
            _read_claim_log("kmosoti/gordian")

    def test_reader_rejects_malformed_historic_transition(self) -> None:
        snapshots, head_sha, root_sha = self._history_snapshots(malformed="transition")
        with (
            patch.object(
                claims,
                "_read_ref",
                side_effect=[_RefSnapshot(head_sha, NOW), _RefSnapshot(root_sha, NOW)],
            ),
            patch.object(claims, "_read_commit", side_effect=lambda _, sha: snapshots[sha]),
            self.assertRaisesRegex(RuntimeError, "changed an unrelated active lease"),
        ):
            _read_claim_log("kmosoti/gordian")

    def test_historic_root_acceptance_uses_compare_counts_not_head_commit(self) -> None:
        root_sha = "1" * 40
        main_sha = "2" * 40
        response = GitHubApiResponse(
            200,
            {"date": SERVER_DATE},
            json.dumps(
                {
                    "status": "ahead",
                    "ahead_by": 2,
                    "behind_by": 0,
                    "total_commits": 2,
                    "base_commit": {"sha": root_sha},
                    "merge_base_commit": {"sha": root_sha},
                }
            ),
        )
        with (
            patch.object(claims, "_read_ref", return_value=_RefSnapshot(main_sha, NOW)),
            patch.object(claims, "_api", return_value=response),
        ):
            claims._validate_initial_parent("kmosoti/gordian", root_sha)

    def test_historic_root_rejects_sibling_and_invalid_compare_counts(self) -> None:
        root_sha = "1" * 40
        main_sha = "2" * 40
        for payload in (
            {
                "status": "diverged",
                "ahead_by": 1,
                "behind_by": 1,
                "total_commits": 2,
                "base_commit": {"sha": root_sha},
                "merge_base_commit": {"sha": "3" * 40},
            },
            {
                "status": "ahead",
                "ahead_by": 2,
                "behind_by": 0,
                "total_commits": 1,
                "base_commit": {"sha": root_sha},
                "merge_base_commit": {"sha": root_sha},
            },
        ):
            with self.subTest(payload=payload), patch.object(
                claims, "_read_ref", return_value=_RefSnapshot(main_sha, NOW)
            ), patch.object(
                claims,
                "_api",
                return_value=GitHubApiResponse(
                    200, {"date": SERVER_DATE}, json.dumps(payload)
                ),
            ), self.assertRaisesRegex(RuntimeError, "accepted main ancestor|commit counts"):
                claims._validate_initial_parent("kmosoti/gordian", root_sha)

    def test_initial_append_parents_accepted_main_and_creates_ref(self) -> None:
        empty = _state((), head=None, tree=None)
        lease = _lease(claim_commit=MAIN)
        after = _state((lease,), event=_claim_event(lease))
        seen: dict[str, object] = {}
        with (
            patch.object(claims, "_read_claim_log", side_effect=[empty, after]),
            patch.object(claims, "_read_main_tree", return_value=(MAIN, TREE, NOW)),
            patch.object(claims, "_create_empty_commit", return_value=HEAD) as create,
            patch.object(
                claims,
                "_publish_ref",
                side_effect=lambda *args, **kwargs: seen.update(kwargs) or GitHubApiResponse(
                    201, {"date": SERVER_DATE}, ""
                ),
            ),
            patch.object(claims, "_new_id", return_value=LEASE_ID),
        ):
            result = claims._append_event(
                "kmosoti/gordian",
                event_type="claim",
                atom=2,
                actor=ACTOR,
                login=LOGIN,
                lease_id=LEASE_ID,
                reason="claim",
            )
        self.assertEqual(result.commit_sha, HEAD)
        self.assertIsNone(seen["observed_head"])
        self.assertEqual(create.call_args.kwargs["parent_sha"], MAIN)
        self.assertEqual(create.call_args.kwargs["tree_sha"], TREE)

    def test_initial_create_race_retries_from_the_new_global_head(self) -> None:
        empty = _state((), head=None, tree=None)
        foreign_lease = _lease(
            atom=3,
            actor=OTHER_ACTOR,
            login=OTHER_LOGIN,
            lease_id=OTHER_LEASE_ID,
        )
        foreign = _state(
            (foreign_lease,),
            head=MAIN,
            tree=TREE,
            event=_claim_event(foreign_lease),
        )
        own_lease = _lease(claim_commit="self")
        success = _state(
            (foreign_lease, own_lease),
            head=HEAD,
            tree=TREE,
            event=_claim_event(own_lease),
        )
        creates: list[dict[str, object]] = []
        with (
            patch.object(
                claims,
                "_read_claim_log",
                side_effect=[empty, foreign, foreign, success],
            ),
            patch.object(claims, "_read_main_tree", return_value=(MAIN, TREE, NOW)),
            patch.object(
                claims,
                "_create_empty_commit",
                side_effect=lambda *args, **kwargs: creates.append(kwargs) or HEAD,
            ),
            patch.object(
                claims,
                "_publish_ref",
                side_effect=[
                    _CasConflict("create race"),
                    GitHubApiResponse(200, {"date": SERVER_DATE}, ""),
                ],
            ),
            patch.object(claims, "_new_id", return_value=LEASE_ID),
        ):
            result = claims._append_event(
                "kmosoti/gordian",
                event_type="claim",
                atom=2,
                actor=ACTOR,
                login=LOGIN,
                lease_id=LEASE_ID,
                reason="claim",
            )
        self.assertTrue(result.lease is own_lease)
        self.assertEqual(len(creates), 2)
        self.assertEqual(creates[0]["parent_sha"], MAIN)
        self.assertEqual(creates[1]["parent_sha"], MAIN)

    def test_update_ref_is_heads_path_and_never_forced(self) -> None:
        with patch.object(
            claims,
            "_api",
            return_value=GitHubApiResponse(200, {"date": SERVER_DATE}, ""),
        ) as api:
            _publish_ref("kmosoti/gordian", commit_sha=HEAD, observed_head=MAIN)
        arguments = api.call_args.args[0]
        self.assertIn("git/refs/heads/gordian-claim-log", arguments[3])
        self.assertEqual(arguments[arguments.index("force=false") - 1], "-F")

    def test_reader_uses_event_server_timestamp_and_ref_date_for_expiry(self) -> None:
        lease = _lease(claim_commit="self")
        trusted_claimed_at = NOW - timedelta(minutes=10)
        message = _event_document(
            {
                **_claim_event(lease),
                "claimed_at": claims._timestamp(trusted_claimed_at),
            },
            [
                _entry_object(
                    atom=2,
                    actor=ACTOR,
                    login=LOGIN,
                    lease_id=LEASE_ID,
                    claim_commit="self",
                )
            ],
        )
        responses = [
            GitHubApiResponse(
                200,
                {"date": SERVER_DATE},
                json.dumps({"ref": CLAIM_LOG_REF, "object": {"type": "commit", "sha": HEAD}}),
            ),
            GitHubApiResponse(
                200,
                {"date": SERVER_DATE},
                json.dumps(
                    {
                        "sha": HEAD,
                        "message": message,
                        "tree": {"sha": TREE},
                        "committer": {"date": "1970-01-01T00:00:00Z"},
                        "parents": [{"sha": MAIN}],
                    }
                ),
            ),
            GitHubApiResponse(
                200,
                {"date": SERVER_DATE},
                json.dumps(
                    {
                        "sha": MAIN,
                        "message": "accepted main",
                        "tree": {"sha": TREE},
                        "committer": {"date": "2026-08-31T03:00:00Z"},
                        "parents": [],
                    }
                ),
            ),
            GitHubApiResponse(
                200,
                {"date": SERVER_DATE},
                json.dumps({"ref": "refs/heads/main", "object": {"type": "commit", "sha": MAIN}}),
            ),
        ]
        with patch.object(claims, "_api", side_effect=responses):
            state = _read_claim_log("kmosoti/gordian")
        self.assertEqual(state.server_now, NOW)
        self.assertEqual(state.active_claims[0].claimed_at, trusted_claimed_at)
        self.assertEqual(state.active_claims[0].claim_commit, HEAD)
        self.assertEqual(state.active_claims[0].event_id, EVENT_ID)

    def test_reader_fails_closed_without_server_date(self) -> None:
        with (
            patch.object(
                claims,
                "_api",
                return_value=GitHubApiResponse(404, {}, ""),
            ),
            self.assertRaisesRegex(RuntimeError, "missing Date"),
        ):
            _read_claim_log("kmosoti/gordian")

    def test_next_event_normalizes_prior_self_to_observed_parent(self) -> None:
        lease = _lease(claim_commit="self")
        before = _state((lease,), event=_claim_event(lease))
        after = _state(
            (),
            head=MAIN,
            event={
                "actor": ACTOR,
                "atom": 2,
                "event_id": EVENT_ID,
                "lease_id": LEASE_ID,
                "login": LOGIN,
                "reason": "done",
                "type": "release",
            },
        )
        captured: dict[str, object] = {}
        with (
            patch.object(claims, "_read_claim_log", side_effect=[before, after]),
            patch.object(
                claims,
                "_create_empty_commit",
                side_effect=lambda *a, **kw: captured.update(kw) or MAIN,
            ),
            patch.object(
                claims,
                "_publish_ref",
                return_value=GitHubApiResponse(200, {"date": SERVER_DATE}, ""),
            ),
            patch.object(claims, "_new_id", return_value=EVENT_ID),
        ):
            claims._append_event(
                "kmosoti/gordian",
                event_type="release",
                atom=2,
                actor=ACTOR,
                login=LOGIN,
                lease_id=LEASE_ID,
                reason="done",
            )
        document = json.loads(str(captured["message"]))
        self.assertEqual(document["active_claims"], [])
        self.assertEqual(captured["parent_sha"], HEAD)

    def test_forced_ref_update_is_refused(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "forced claim-log"):
            _publish_ref("kmosoti/gordian", commit_sha=HEAD, observed_head=MAIN, force=True)

    def test_conflict_rereads_and_loses_without_projection(self) -> None:
        empty = _state((), head=None, tree=None)
        foreign = _state((_lease(actor=OTHER_ACTOR, login=OTHER_LOGIN, lease_id=OTHER_LEASE_ID),))
        with (
            patch.object(claims, "_read_claim_log", side_effect=[empty, foreign]),
            patch.object(claims, "_read_main_tree", return_value=(MAIN, TREE, NOW)),
            patch.object(claims, "_create_empty_commit", return_value=HEAD),
            patch.object(claims, "_publish_ref", side_effect=_CasConflict("race")),
            patch.object(claims, "_new_id", return_value=LEASE_ID),
            self.assertRaisesRegex(RuntimeError, "lost claim-log CAS race"),
        ):
            claims._append_event(
                "kmosoti/gordian",
                event_type="claim",
                atom=2,
                actor=ACTOR,
                login=LOGIN,
                lease_id=LEASE_ID,
                reason="claim",
            )

    def test_stale_same_actor_claim_requires_explicit_release(self) -> None:
        stale = _lease(claimed_at=NOW - timedelta(minutes=CLAIM_MINUTES))
        state = _state((stale,), event=_claim_event(stale), server_now=NOW)
        with (
            patch.object(claims, "_read_claim_log", return_value=state),
            patch.object(claims, "_new_id", return_value=EVENT_ID),
            patch.object(claims, "_create_empty_commit") as create,
            self.assertRaisesRegex(RuntimeError, "stale claim.*release it explicitly"),
        ):
            claims._append_event(
                "kmosoti/gordian",
                event_type="claim",
                atom=2,
                actor=ACTOR,
                login=LOGIN,
                lease_id=OTHER_LEASE_ID,
                reason="claim",
            )
        create.assert_not_called()

    def test_reclaim_removes_expired_foreign_lease_with_server_clock(self) -> None:
        stale = _lease(
            actor=OTHER_ACTOR,
            login=OTHER_LOGIN,
            claimed_at=NOW - timedelta(minutes=CLAIM_MINUTES),
        )
        before = _state((stale,), event=_claim_event(stale), server_now=NOW)
        reclaim_event = _event_object(
            event_id=EVENT_ID,
            event_type="reclaim",
            atom=stale.atom or 2,
            actor=ACTOR,
            login=LOGIN,
            lease_id=stale.lease_id,
            reason="stale-owner recovery",
            target_actor=stale.actor,
            target_login=stale.login,
        )
        after = _state(head="a" * 39 + "1", event=reclaim_event, server_now=NOW)
        with (
            patch.object(claims, "_read_claim_log", side_effect=(before, after)),
            patch.object(claims, "_new_id", return_value=EVENT_ID),
            patch.object(claims, "_create_empty_commit", return_value=after.head_sha),
            patch.object(claims, "_publish_ref"),
        ):
            result = claims._append_event(
                "kmosoti/gordian",
                event_type="reclaim",
                atom=2,
                actor=ACTOR,
                login=LOGIN,
                lease_id=stale.lease_id,
                reason="stale-owner recovery",
                target_actor=stale.actor,
                target_login=stale.login,
                coordinator_authorized=True,
            )
        self.assertEqual(result.event_id, EVENT_ID)
        self.assertIsNone(result.lease)

    def test_reclaim_rejects_live_foreign_lease_without_writing(self) -> None:
        live = _lease(actor=OTHER_ACTOR, login=OTHER_LOGIN)
        with (
            patch.object(claims, "_read_claim_log", return_value=_state((live,))),
            patch.object(claims, "_create_empty_commit") as create,
            patch.object(claims, "_publish_ref") as publish,
            self.assertRaisesRegex(RuntimeError, "live foreign lease"),
        ):
            claims._append_event(
                "kmosoti/gordian",
                event_type="reclaim",
                atom=2,
                actor=ACTOR,
                login=LOGIN,
                lease_id=live.lease_id,
                reason="attempted theft",
                target_actor=live.actor,
                target_login=live.login,
                coordinator_authorized=True,
            )
        create.assert_not_called()
        publish.assert_not_called()

    def test_reclaim_cas_race_retries_from_fresh_server_state(self) -> None:
        stale = _lease(actor=OTHER_ACTOR, login=OTHER_LOGIN)
        stale = replace(stale, claimed_at=NOW - timedelta(minutes=CLAIM_MINUTES))
        before = _state((stale,), event=_claim_event(stale), server_now=NOW)
        after = _state(head="a" * 39 + "2", event=_event_object(
            event_id=EVENT_ID,
            event_type="reclaim",
            atom=2,
            actor=ACTOR,
            login=LOGIN,
            lease_id=stale.lease_id,
            reason="retry",
            target_actor=stale.actor,
            target_login=stale.login,
        ), server_now=NOW)
        with (
            patch.object(claims, "_read_claim_log", side_effect=(before, before, before, after)),
            patch.object(claims, "_new_id", return_value=EVENT_ID),
            patch.object(
                claims, "_create_empty_commit", side_effect=("a" * 39 + "3", after.head_sha)
            ),
            patch.object(claims, "_publish_ref", side_effect=(_CasConflict("race"), None)),
        ):
            result = claims._append_event(
                "kmosoti/gordian",
                event_type="reclaim",
                atom=2,
                actor=ACTOR,
                login=LOGIN,
                lease_id=stale.lease_id,
                reason="retry",
                target_actor=stale.actor,
                target_login=stale.login,
                coordinator_authorized=True,
            )
        self.assertEqual(result.event_id, EVENT_ID)

    def test_reclaim_requires_explicit_coordinator_authorization(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "authorized coordinator"):
            claims._append_event(
                "kmosoti/gordian",
                event_type="reclaim",
                atom=2,
                actor=ACTOR,
                login=LOGIN,
                lease_id=LEASE_ID,
                reason="missing authority",
                target_actor=OTHER_ACTOR,
                target_login=OTHER_LOGIN,
            )

    def test_append_release_rejects_foreign_target_before_remote_mutation(self) -> None:
        with (
            patch.object(claims, "_read_claim_log") as read,
            patch.object(claims, "_create_empty_commit") as create,
            patch.object(claims, "_publish_ref") as publish,
            self.assertRaisesRegex(RuntimeError, "target actor"),
        ):
            claims._append_event(
                "kmosoti/gordian",
                event_type="release",
                atom=2,
                actor=ACTOR,
                login=LOGIN,
                lease_id=LEASE_ID,
                reason="stale foreign release",
                target_actor=OTHER_ACTOR,
            )
        read.assert_not_called()
        create.assert_not_called()
        publish.assert_not_called()

    def test_append_abort_rejects_foreign_target_before_remote_mutation(self) -> None:
        with (
            patch.object(claims, "_read_claim_log") as read,
            patch.object(claims, "_create_empty_commit") as create,
            patch.object(claims, "_publish_ref") as publish,
            self.assertRaisesRegex(RuntimeError, "target actor"),
        ):
            claims._append_event(
                "kmosoti/gordian",
                event_type="abort",
                atom=2,
                actor=ACTOR,
                login=LOGIN,
                lease_id=LEASE_ID,
                reason="stale foreign abort",
                target_actor=OTHER_ACTOR,
            )
        read.assert_not_called()
        create.assert_not_called()
        publish.assert_not_called()

    def test_append_release_rejects_login_mismatch_before_remote_mutation(self) -> None:
        lease = _lease()
        state = _state((lease,), event=_claim_event(lease))
        with (
            patch.object(claims, "_read_claim_log", return_value=state),
            patch.object(claims, "_create_empty_commit") as create,
            patch.object(claims, "_publish_ref") as publish,
            self.assertRaisesRegex(RuntimeError, "login does not match"),
        ):
            claims._append_event(
                "kmosoti/gordian",
                event_type="release",
                atom=2,
                actor=ACTOR,
                login=OTHER_LOGIN,
                lease_id=LEASE_ID,
                reason="wrong login",
            )
        create.assert_not_called()
        publish.assert_not_called()

    def test_lost_release_response_is_found_in_validated_history(self) -> None:
        lease = _lease()
        before = _state((lease,), event=_claim_event(lease))
        release_event = _event_object(
            event_id=EVENT_ID,
            event_type="release",
            atom=2,
            actor=ACTOR,
            login=LOGIN,
            lease_id=LEASE_ID,
            reason="done",
        )
        later_event = _event_object(
            event_id=OTHER_LEASE_ID,
            event_type="claim",
            atom=3,
            actor=OTHER_ACTOR,
            login=OTHER_LOGIN,
            lease_id=OTHER_LEASE_ID,
            reason="claim",
        )
        after = _state(
            (),
            head=MAIN,
            event=later_event,
            server_now=NOW,
        )
        after = ClaimLogState(
            head_sha=after.head_sha,
            tree_sha=after.tree_sha,
            active_claims=after.active_claims,
            event=after.event,
            server_now=after.server_now,
            events=(release_event, later_event),
        )
        with (
            patch.object(claims, "_read_claim_log", side_effect=[before, after]),
            patch.object(claims, "_create_empty_commit", return_value=HEAD),
            patch.object(claims, "_publish_ref", side_effect=RuntimeError("response lost")),
            patch.object(claims, "_new_id", return_value=EVENT_ID),
        ):
            result = claims._append_event(
                "kmosoti/gordian",
                event_type="release",
                atom=2,
                actor=ACTOR,
                login=LOGIN,
                lease_id=LEASE_ID,
                reason="done",
            )
        self.assertTrue(result.already_present)
        self.assertEqual(result.event_id, EVENT_ID)

    def test_successful_release_followed_by_append_returns_requested_event_id(self) -> None:
        lease = _lease()
        before = _state((lease,), event=_claim_event(lease))
        release_event = _event_object(
            event_id=EVENT_ID,
            event_type="release",
            atom=2,
            actor=ACTOR,
            login=LOGIN,
            lease_id=LEASE_ID,
            reason="done",
        )
        later_event = _event_object(
            event_id=OTHER_LEASE_ID,
            event_type="claim",
            atom=3,
            actor=OTHER_ACTOR,
            login=OTHER_LOGIN,
            lease_id=OTHER_LEASE_ID,
            reason="claim",
        )
        after = ClaimLogState(
            head_sha=MAIN,
            tree_sha=TREE,
            active_claims=(),
            event=later_event,
            server_now=NOW,
            events=(release_event, later_event),
        )
        with (
            patch.object(claims, "_read_claim_log", side_effect=[before, after]),
            patch.object(claims, "_create_empty_commit", return_value=HEAD),
            patch.object(
                claims,
                "_publish_ref",
                return_value=GitHubApiResponse(200, {"date": SERVER_DATE}, ""),
            ),
            patch.object(claims, "_new_id", return_value=EVENT_ID),
        ):
            result = claims._append_event(
                "kmosoti/gordian",
                event_type="release",
                atom=2,
                actor=ACTOR,
                login=LOGIN,
                lease_id=LEASE_ID,
                reason="done",
            )
        self.assertEqual(result.event_id, EVENT_ID)
        self.assertNotEqual(result.event_id, OTHER_LEASE_ID)

    def test_ready_selection_ignores_assignee_projection(self) -> None:
        issue = IssueRecord(
            number=2,
            title="Assigned but canonically free",
            state="OPEN",
            blocked_by=(),
            assignees=("someone-else",),
        )
        with (
            patch.object(claims, "fetch_issues", return_value=(issue,)),
            patch.object(claims, "load_closure_schema", return_value={}),
            patch.object(claims, "bootstrap_satisfied", return_value=(frozenset(), ())),
        ):
            rows = claims._ready_rows(
                "kmosoti/gordian", Path("atoms"), Path("schema.json")
            )
        self.assertEqual([row.number for row in rows], [2])

    def test_same_actor_same_atom_login_converges_to_winner_lease(self) -> None:
        empty = _state((), head=None, tree=None)
        winner_lease = _lease(lease_id=OTHER_LEASE_ID, claim_commit=HEAD)
        winner = _state((winner_lease,), event=_claim_event(winner_lease))
        with (
            patch.object(claims, "_read_claim_log", side_effect=[empty, winner]),
            patch.object(claims, "_read_main_tree", return_value=(MAIN, TREE, NOW)),
            patch.object(claims, "_create_empty_commit", return_value=HEAD),
            patch.object(claims, "_publish_ref", side_effect=_CasConflict("race")),
            patch.object(claims, "_new_id", return_value=LEASE_ID),
        ):
            result = claims._append_event(
                "kmosoti/gordian",
                event_type="claim",
                atom=2,
                actor=ACTOR,
                login=LOGIN,
                lease_id=LEASE_ID,
                reason="claim",
            )
        self.assertTrue(result.already_present)
        self.assertEqual(result.lease, winner_lease)

    def test_lost_publish_response_converges_from_reread(self) -> None:
        empty = _state((), head=None, tree=None)
        lease = _lease(claim_commit=HEAD)
        success = _state((lease,), event=_claim_event(lease))
        with (
            patch.object(claims, "_read_claim_log", side_effect=[empty, success]),
            patch.object(claims, "_read_main_tree", return_value=(MAIN, TREE, NOW)),
            patch.object(claims, "_create_empty_commit", return_value=HEAD),
            patch.object(claims, "_publish_ref", side_effect=RuntimeError("response lost")),
            patch.object(claims, "_new_id", return_value=LEASE_ID),
        ):
            result = claims._append_event(
                "kmosoti/gordian",
                event_type="claim",
                atom=2,
                actor=ACTOR,
                login=LOGIN,
                lease_id=LEASE_ID,
                reason="claim",
            )
        self.assertTrue(result.already_present)
        self.assertEqual(result.lease, lease)

    def test_same_login_different_actor_and_global_cap_are_atomic_rejections(self) -> None:
        same_login = _state((_lease(actor=OTHER_ACTOR, login=LOGIN, lease_id=OTHER_LEASE_ID),))
        capped = _state(
            tuple(
                _lease(
                    atom=index,
                    actor=f"gordian-agent/harness/run-{index}",
                    lease_id=f"{index:032x}",
                )
                for index in range(1, MAX_ACTIVE_CLAIMS + 1)
            )
        )
        for state, expected, atom in (
            (same_login, "another actor", 2),
            (capped, "active cap", MAX_ACTIVE_CLAIMS + 1),
        ):
            with (
                self.subTest(expected=expected),
                patch.object(claims, "_read_claim_log", return_value=state),
                self.assertRaisesRegex(RuntimeError, expected),
            ):
                claims._append_event(
                    "kmosoti/gordian",
                    event_type="claim",
                    atom=atom,
                    actor=ACTOR,
                    login=LOGIN,
                    lease_id=LEASE_ID,
                    reason="claim",
                )


class ProjectionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.board = Board(
            project_id="PVT_1",
            fields={},
            items={2: BoardItem(item_id="ITEM_2", number=2, status="Ready")},
        )

    def test_projections_follow_successful_claim_cas_and_comment_has_ref_identity(self) -> None:
        lease = _lease()
        initial = _state(())
        state = _state((lease,), event=_claim_event(lease))
        issue = IssueClaim(2, "OPEN", (LOGIN,), lease, (lease,))
        appended = _append_result(lease, state)
        with (
            patch.dict(os.environ, {"GORDIAN_ACTOR": ACTOR}, clear=True),
            patch.object(claims, "preflight", return_value=AUTH),
            patch.object(claims, "_ready_rows", return_value=(_row(2),)),
            patch.object(claims, "_read_claim_log", side_effect=[initial, state]),
            patch.object(claims, "_append_event", return_value=appended) as append,
            patch.object(claims, "_issue_claim", return_value=issue),
            patch.object(claims, "fetch_board", return_value=self.board),
            patch.object(claims, "_set_status") as status,
            patch.object(claims, "run_gh", return_value="") as run_gh,
        ):
            result = claim(
                repository="kmosoti/gordian",
                project_owner="kmosoti",
                project_number=9,
                closure_root=Path("atoms"),
                closure_schema=Path("schema.json"),
                requested=2,
                now=NOW + timedelta(days=2),
            )
        append.assert_called_once()
        self.assertFalse(result["already_claimed"])
        self.assertEqual(status.call_count, 1)
        self.assertEqual(run_gh.call_count, 2)
        comment = run_gh.call_args_list[1].args[0][-1]
        self.assertIn(CLAIM_LOG_REF, comment)
        self.assertIn(LEASE_ID, comment)

    def test_closed_issue_after_cas_appends_abort_without_projection(self) -> None:
        lease = _lease()
        initial = _state(())
        state = _state((lease,), event=_claim_event(lease))
        appended = _append_result(lease, state)
        closed = IssueClaim(2, "CLOSED", (), lease, (lease,))
        with (
            patch.dict(os.environ, {"GORDIAN_ACTOR": ACTOR}, clear=True),
            patch.object(claims, "preflight", return_value=AUTH),
            patch.object(claims, "_ready_rows", return_value=(_row(2),)),
            patch.object(claims, "_read_claim_log", return_value=initial),
            patch.object(claims, "_append_event", return_value=appended) as append,
            patch.object(claims, "_issue_claim", return_value=closed),
            patch.object(claims, "_projection_abort", return_value=None) as abort,
            patch.object(claims, "fetch_board") as board,
            patch.object(claims, "run_gh") as run_gh,
            self.assertRaisesRegex(RuntimeError, "claim aborted"),
        ):
            claim(
                repository="kmosoti/gordian",
                project_owner="kmosoti",
                project_number=9,
                closure_root=Path("atoms"),
                closure_schema=Path("schema.json"),
                requested=2,
                now=NOW,
            )
        self.assertGreaterEqual(abort.call_count, 1)
        board.assert_not_called()
        run_gh.assert_not_called()
        self.assertEqual(append.call_args_list[0].kwargs["event_type"], "claim")

    def test_projection_failure_aborts_and_reconciles(self) -> None:
        lease = _lease()
        initial = _state(())
        state = _state((lease,), event=_claim_event(lease))
        appended = _append_result(lease, state)
        issue = IssueClaim(2, "OPEN", (LOGIN,), lease, (lease,))
        with (
            patch.dict(os.environ, {"GORDIAN_ACTOR": ACTOR}, clear=True),
            patch.object(claims, "preflight", return_value=AUTH),
            patch.object(claims, "_ready_rows", return_value=(_row(2),)),
            patch.object(claims, "_read_claim_log", return_value=initial),
            patch.object(claims, "_append_event", return_value=appended) as append,
            patch.object(claims, "_issue_claim", return_value=issue),
            patch.object(claims, "fetch_board", return_value=self.board),
            patch.object(claims, "_set_status", side_effect=RuntimeError("status failed")),
            patch.object(claims, "run_gh", return_value=""),
            patch.object(claims, "_projection_abort", return_value=None) as abort,
            patch.object(claims, "_reconcile_claim_projection", return_value=[]) as reconcile,
            self.assertRaisesRegex(RuntimeError, "status failed"),
        ):
            claim(
                repository="kmosoti/gordian",
                project_owner="kmosoti",
                project_number=9,
                closure_root=Path("atoms"),
                closure_schema=Path("schema.json"),
                requested=2,
                now=NOW,
            )
        self.assertGreaterEqual(abort.call_count, 1)
        reconcile.assert_called_once()
        self.assertEqual(append.call_args_list[0].kwargs["event_type"], "claim")

    def test_claim_compensation_does_not_remove_preexisting_assignee(self) -> None:
        board = self.board
        issue = IssueClaim(2, "OPEN", (LOGIN,), None, ())
        with (
            patch.object(claims, "_read_claim_log", return_value=_state()),
            patch.object(claims, "_issue_claim", return_value=issue),
            patch.object(claims, "run_gh") as run_gh,
            patch.object(claims, "_set_status") as status,
        ):
            failures = claims._reconcile_claim_projection(
                repository="kmosoti/gordian",
                number=2,
                login=LOGIN,
                board=board,
                previous_status="Ready",
                actor=ACTOR,
                assignee_added=False,
                status_attempted=False,
            )
        self.assertEqual(failures, [])
        run_gh.assert_not_called()
        status.assert_not_called()

    def test_claim_compensation_restores_same_login_winner_after_assignee_removal(self) -> None:
        board = self.board
        initial = _state(())
        winner_lease = _lease(
            actor=OTHER_ACTOR,
            login=LOGIN,
            lease_id=OTHER_LEASE_ID,
            claim_commit=HEAD,
            event_id=OTHER_LEASE_ID,
        )
        winner = _state((winner_lease,), event=_claim_event(winner_lease))
        old_issue = IssueClaim(2, "OPEN", (LOGIN,), None, ())
        restored = False

        def issue_for_state(repository, number, *, state=None):
            nonlocal restored
            if state is not None and state.for_atom(number):
                return IssueClaim(
                    2,
                    "OPEN",
                    (LOGIN,) if restored else (),
                    winner_lease,
                    (winner_lease,),
                )
            return old_issue

        def projected(command):
            nonlocal restored
            if "--remove-assignee" in command:
                restored = False
            elif "--add-assignee" in command:
                restored = True
            return ""

        with (
            patch.object(
                claims,
                "_read_claim_log",
                side_effect=[initial, initial, winner, winner, winner, winner, winner, winner],
            ),
            patch.object(claims, "_issue_claim", side_effect=issue_for_state),
            patch.object(claims, "run_gh", side_effect=projected) as run_gh,
            patch.object(claims, "_set_status") as status,
        ):
            failures = claims._reconcile_claim_projection(
                repository="kmosoti/gordian",
                number=2,
                login=LOGIN,
                board=board,
                previous_status="Ready",
                actor=ACTOR,
                assignee_added=True,
                status_attempted=True,
                status_changed=True,
            )
        self.assertEqual(failures, [])
        commands = [call.args[0] for call in run_gh.call_args_list]
        self.assertTrue(any("--remove-assignee" in command for command in commands))
        self.assertTrue(any("--add-assignee" in command for command in commands))
        status.assert_called_once()
        self.assertEqual(status.call_args.args[2], "In Progress")


    def test_claim_compensation_does_not_clobber_concurrent_project_status(self) -> None:
        board = Board(
            project_id="PVT_1",
            fields={},
            items={2: BoardItem(item_id="ITEM_2", number=2, status="In Review")},
        )
        issue = IssueClaim(2, "OPEN", (), None, ())
        with (
            patch.object(claims, "_read_claim_log", return_value=_state()),
            patch.object(claims, "_issue_claim", return_value=issue),
            patch.object(claims, "_set_status") as status,
        ):
            failures = claims._reconcile_claim_projection(
                repository="kmosoti/gordian",
                number=2,
                login=LOGIN,
                board=board,
                previous_status="Ready",
                actor=ACTOR,
                status_attempted=True,
                status_changed=True,
            )
        status.assert_not_called()
        self.assertEqual(board.items[2].status, "In Review")
        self.assertTrue(any("status changed concurrently" in failure for failure in failures))

    def test_release_closed_issue_still_releases_canonical_lease(self) -> None:
        lease = _lease()
        before = _state((lease,), event=_claim_event(lease))
        closed = IssueClaim(2, "CLOSED", (LOGIN,), lease, (lease,))
        released = _AppendResult(
            EVENT_ID,
            MAIN,
            _state((), head=MAIN, event={"type": "release"}),
            None,
        )
        with (
            patch.dict(os.environ, {"GORDIAN_ACTOR": ACTOR}, clear=True),
            patch.object(claims, "preflight", return_value=AUTH),
            patch.object(claims, "_read_claim_log", return_value=before),
            patch.object(claims, "_issue_claim", return_value=closed),
            patch.object(claims, "_append_event", return_value=released) as append,
            patch.object(claims, "fetch_board") as board,
            patch.object(claims, "run_gh") as run_gh,
        ):
            result = release(
                repository="kmosoti/gordian",
                project_owner="kmosoti",
                project_number=9,
                closure_root=Path("atoms"),
                closure_schema=Path("schema.json"),
                number=2,
                reason="closed cleanup",
            )
        self.assertTrue(result["released"])
        self.assertIn("closed", result["projection_drift"][0])
        self.assertEqual(append.call_args.kwargs["event_type"], "release")
        board.assert_not_called()
        run_gh.assert_not_called()

    def test_release_rejects_stale_foreign_lease_before_any_mutation(self) -> None:
        stale = _lease(
            actor=OTHER_ACTOR,
            login=OTHER_LOGIN,
            claimed_at=NOW - timedelta(minutes=CLAIM_MINUTES),
            lease_id=OTHER_LEASE_ID,
        )
        state = _state((stale,), event=_claim_event(stale), server_now=NOW)
        issue = IssueClaim(2, "OPEN", (OTHER_LOGIN,), stale, (stale,))
        with (
            patch.dict(os.environ, {"GORDIAN_ACTOR": ACTOR}, clear=True),
            patch.object(claims, "preflight", return_value=AUTH),
            patch.object(claims, "_read_claim_log", return_value=state),
            patch.object(claims, "_issue_claim", return_value=issue),
            patch.object(claims, "_append_event") as append,
            patch.object(claims, "fetch_board") as board,
            patch.object(claims, "run_gh") as run_gh,
            self.assertRaisesRegex(RuntimeError, "claim belongs"),
        ):
            release(
                repository="kmosoti/gordian",
                project_owner="kmosoti",
                project_number=9,
                closure_root=Path("atoms"),
                closure_schema=Path("schema.json"),
                number=2,
                reason="stale foreign release",
            )
        append.assert_not_called()
        board.assert_not_called()
        run_gh.assert_not_called()

    def test_release_rejects_stale_same_actor_with_wrong_authenticated_login(self) -> None:
        stale = _lease(
            claimed_at=NOW - timedelta(minutes=CLAIM_MINUTES),
            login=OTHER_LOGIN,
        )
        state = _state((stale,), event=_claim_event(stale), server_now=NOW)
        issue = IssueClaim(2, "OPEN", (OTHER_LOGIN,), stale, (stale,))
        with (
            patch.dict(os.environ, {"GORDIAN_ACTOR": ACTOR}, clear=True),
            patch.object(claims, "preflight", return_value=AUTH),
            patch.object(claims, "_read_claim_log", return_value=state),
            patch.object(claims, "_issue_claim", return_value=issue),
            patch.object(claims, "_append_event") as append,
            self.assertRaisesRegex(RuntimeError, "authenticated login"),
        ):
            release(
                repository="kmosoti/gordian",
                project_owner="kmosoti",
                project_number=9,
                closure_root=Path("atoms"),
                closure_schema=Path("schema.json"),
                number=2,
                reason="wrong login",
            )
        append.assert_not_called()

    def test_projection_abort_rejects_stale_foreign_lease_before_append(self) -> None:
        stale = _lease(
            actor=OTHER_ACTOR,
            login=OTHER_LOGIN,
            claimed_at=NOW - timedelta(minutes=CLAIM_MINUTES),
            lease_id=OTHER_LEASE_ID,
        )
        state = _state((stale,), event=_claim_event(stale), server_now=NOW)
        with (
            patch.object(claims, "_read_claim_log", return_value=state),
            patch.object(claims, "_append_event") as append,
        ):
            result = claims._projection_abort(
                "kmosoti/gordian",
                number=2,
                actor=ACTOR,
                lease=stale,
                reason="stale foreign abort",
            )
        self.assertIsNone(result)
        append.assert_not_called()

    def test_release_race_readds_shared_login_and_promotes_winner_status(self) -> None:
        lease = _lease()
        before = _state((lease,), event=_claim_event(lease))
        released = _state((), head=MAIN, event={"type": "release"})
        winner_lease = _lease(
            actor=OTHER_ACTOR,
            login=LOGIN,
            lease_id=OTHER_LEASE_ID,
            claim_commit=HEAD,
            event_id=OTHER_LEASE_ID,
        )
        winner = _state((winner_lease,), event=_claim_event(winner_lease))
        old_issue = IssueClaim(2, "OPEN", (LOGIN,), lease, (lease,))
        winner_issue = IssueClaim(2, "OPEN", (), winner_lease, (winner_lease,))
        released_append = _AppendResult(
            EVENT_ID,
            MAIN,
            released,
            None,
        )

        winner_assignee_added = False

        def issue_for_state(repository, number, *, state=None):
            if state is not None and state.for_atom(number):
                return replace(
                    winner_issue,
                    assignees=(LOGIN,) if winner_assignee_added else (),
                )
            return old_issue

        def projected(command):
            nonlocal winner_assignee_added
            if "--add-assignee" in command:
                winner_assignee_added = True
            return ""

        # The winner appears immediately after the stale release removes the
        # shared assignee.  Every subsequent guard sees that same winner.
        log_states = [before, released, released, released, winner] + [winner] * 8
        with (
            patch.dict(os.environ, {"GORDIAN_ACTOR": ACTOR}, clear=True),
            patch.object(claims, "preflight", return_value=AUTH),
            patch.object(claims, "_read_claim_log", side_effect=log_states),
            patch.object(claims, "_issue_claim", side_effect=issue_for_state),
            patch.object(claims, "_append_event", return_value=released_append),
            patch.object(claims, "_ready_rows", return_value=(_row(2),)),
            patch.object(claims, "fetch_board", return_value=self.board),
            patch.object(claims, "_set_status") as status,
            patch.object(claims, "run_gh", side_effect=projected) as run_gh,
        ):
            result = release(
                repository="kmosoti/gordian",
                project_owner="kmosoti",
                project_number=9,
                closure_root=Path("atoms"),
                closure_schema=Path("schema.json"),
                number=2,
                reason="done",
            )
        self.assertIn("reacquired", result["projection_drift"][0])
        commands = [call.args[0] for call in run_gh.call_args_list]
        self.assertTrue(any("--remove-assignee" in command for command in commands))
        self.assertTrue(any("--add-assignee" in command for command in commands))
        status.assert_called_once()
        self.assertEqual(status.call_args.args[2], "In Progress")

    def test_release_uses_canonical_lease_not_historical_comment(self) -> None:
        lease = _lease()
        before = _state((lease,), event=_claim_event(lease))
        after = _state(
            (),
            head=MAIN,
            event={
                "actor": ACTOR,
                "atom": 2,
                "event_id": EVENT_ID,
                "lease_id": LEASE_ID,
                "login": LOGIN,
                "reason": "done",
                "type": "release",
            },
        )
        issue = IssueClaim(2, "OPEN", (LOGIN,), lease, (lease,))
        released = _AppendResult(EVENT_ID, MAIN, after, None)
        with (
            patch.dict(os.environ, {"GORDIAN_ACTOR": ACTOR}, clear=True),
            patch.object(claims, "preflight", return_value=AUTH),
            # The release path brackets each external projection mutation with
            # a canonical reread.  Keep returning the post-release state for
            # each guard in this no-race fixture.
            patch.object(
                claims,
                "_read_claim_log",
                side_effect=[before, *([after] * 7)],
            ),
            patch.object(claims, "_issue_claim", return_value=issue),
            patch.object(claims, "_append_event", return_value=released) as append,
            patch.object(claims, "_ready_rows", return_value=(_row(2),)),
            patch.object(claims, "fetch_board", return_value=self.board),
            patch.object(claims, "_set_status"),
            patch.object(claims, "run_gh", return_value="") as run_gh,
        ):
            result = release(
                repository="kmosoti/gordian",
                project_owner="kmosoti",
                project_number=9,
                closure_root=Path("atoms"),
                closure_schema=Path("schema.json"),
                number=2,
                reason="done",
                now=NOW,
            )
        self.assertTrue(result["released"])
        self.assertEqual(append.call_args.kwargs["event_type"], "release")
        self.assertEqual(run_gh.call_count, 2)
        self.assertIn(CLAIM_LOG_REF, run_gh.call_args_list[-1].args[0][-1])

    def test_release_does_not_remove_a_winner_that_reacquires_before_projection(self) -> None:
        lease = _lease()
        before = _state((lease,), event=_claim_event(lease))
        winner_lease = _lease(
            actor=OTHER_ACTOR,
            login=LOGIN,
            lease_id=OTHER_LEASE_ID,
            claim_commit=HEAD,
        )
        winner = _state((winner_lease,), event=_claim_event(winner_lease))
        issue = IssueClaim(2, "OPEN", (LOGIN,), lease, (lease,))
        released = _AppendResult(EVENT_ID, MAIN, _state((), head=MAIN, event={}), None)
        with (
            patch.dict(os.environ, {"GORDIAN_ACTOR": ACTOR}, clear=True),
            patch.object(claims, "preflight", return_value=AUTH),
            patch.object(claims, "_read_claim_log", side_effect=[before, winner]),
            patch.object(claims, "_issue_claim", return_value=issue),
            patch.object(claims, "_append_event", return_value=released),
            patch.object(claims, "_ready_rows", return_value=(_row(2),)),
            patch.object(claims, "fetch_board", return_value=self.board),
            patch.object(claims, "_set_status") as status,
            patch.object(claims, "run_gh") as run_gh,
        ):
            result = release(
                repository="kmosoti/gordian",
                project_owner="kmosoti",
                project_number=9,
                closure_root=Path("atoms"),
                closure_schema=Path("schema.json"),
                number=2,
                reason="done",
                now=NOW,
            )
        self.assertIn("reacquired", result["projection_drift"][0])
        status.assert_not_called()
        run_gh.assert_not_called()


class LiveClaimTests(unittest.TestCase):
    def test_same_actor_retry_is_idempotent_and_server_date_wins(self) -> None:
        lease = _lease()
        state = _state((lease,), event=_claim_event(lease), server_now=NOW)
        issue = IssueClaim(2, "OPEN", (LOGIN,), lease, (lease,))
        with (
            patch.dict(os.environ, {"GORDIAN_ACTOR": ACTOR}, clear=True),
            patch.object(claims, "_read_claim_log", return_value=state),
            patch.object(claims, "_issue_claim", return_value=issue),
        ):
            result = require_live_claim(
                repository="kmosoti/gordian",
                number=2,
                login=LOGIN,
                now=NOW + timedelta(days=10),
            )
        self.assertEqual(result.lease_id, LEASE_ID)

    def test_expired_server_clock_rejects_even_when_local_now_would_pass(self) -> None:
        lease = _lease()
        state = _state(
            (lease,),
            event=_claim_event(lease),
            server_now=NOW + timedelta(minutes=CLAIM_MINUTES),
        )
        issue = IssueClaim(2, "OPEN", (LOGIN,), lease, (lease,))
        with (
            patch.dict(os.environ, {"GORDIAN_ACTOR": ACTOR}, clear=True),
            patch.object(claims, "_read_claim_log", return_value=state),
            patch.object(claims, "_issue_claim", return_value=issue),
            self.assertRaisesRegex(RuntimeError, "expired"),
        ):
            require_live_claim(repository="kmosoti/gordian", number=2, login=LOGIN, now=NOW)


class CliTests(unittest.TestCase):
    def test_preflight_configuration_failure_returns_78(self) -> None:
        stderr = io.StringIO()
        with patch.dict(os.environ, {}, clear=True), redirect_stderr(stderr):
            self.assertEqual(main(["preflight"]), 78)
        self.assertIn("GitHub authentication is unavailable", stderr.getvalue())


class LegacyClaimAdoptionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.board = Board(
            project_id="PVT_1",
            fields={},
            items={70: BoardItem(item_id="ITEM_70", number=70, status="In Progress")},
        )
        self.legacy = claims._LegacyClaimComment(
            comment_id=701,
            body=claims._claim_comment(ACTOR, NOW),
            lease=ClaimLease(
                actor=ACTOR,
                claimed_at=NOW,
                expires_after_minutes=CLAIM_MINUTES,
                comment_url="https://github.com/kmosoti/gordian/issues/70#issuecomment-701",
            ),
        )

    def _kwargs(self) -> dict[str, object]:
        return {
            "repository": "kmosoti/gordian",
            "project_owner": "kmosoti",
            "project_number": 9,
            "closure_root": Path("atoms"),
            "closure_schema": Path("schema.json"),
            "number": 70,
        }

    def test_adoption_appends_canonical_event_and_updates_existing_comment_only(self) -> None:
        lease = _lease(atom=70)
        initial = _state(())
        current = _state((lease,), event=_claim_event(lease))
        issue = IssueClaim(70, "OPEN", (LOGIN,), None, ())
        current_issue = IssueClaim(70, "OPEN", (LOGIN,), lease, (lease,))
        with (
            patch.dict(os.environ, {"GORDIAN_ACTOR": ACTOR}, clear=True),
            patch.object(claims, "preflight", return_value=AUTH),
            patch.object(claims, "_read_claim_log", side_effect=(initial, current)),
            patch.object(claims, "_issue_claim", side_effect=(issue, current_issue)),
            patch.object(claims, "fetch_board", return_value=self.board),
            patch.object(
                claims, "_read_legacy_claim_comment", return_value=(self.legacy, NOW)
            ) as read_legacy,
            patch.object(
                claims, "_append_event", return_value=_append_result(lease, current)
            ) as append,
            patch.object(claims, "_update_claim_comment") as update,
            patch.object(claims, "run_gh") as run_gh,
        ):
            result = adopt_legacy_claim(**self._kwargs())
        self.assertTrue(result["adopted_legacy"])
        append.assert_called_once()
        self.assertEqual(append.call_args.kwargs["atom"], 70)
        self.assertEqual(read_legacy.call_args.kwargs["expected_actor"], ACTOR)
        update.assert_called_once()
        self.assertEqual(update.call_args.kwargs["comment_id"], 701)
        body = update.call_args.kwargs["body"]
        self.assertIn(CLAIM_LOG_REF, body)
        self.assertIn(LEASE_ID, body)
        run_gh.assert_not_called()

    def test_adoption_rejects_every_invalid_legacy_precondition_before_cas(self) -> None:
        cases = (
            (
                "wrong actor",
                replace(self.legacy, lease=replace(self.legacy.lease, actor=OTHER_ACTOR)),
                "legacy claim comment belongs",
            ),
            (
                "stale comment",
                replace(
                    self.legacy,
                    lease=replace(
                        self.legacy.lease,
                        claimed_at=NOW - timedelta(minutes=CLAIM_MINUTES),
                    ),
                ),
                "stale",
            ),
            (
                "wrong login",
                IssueClaim(70, "OPEN", (OTHER_LOGIN,), None, ()),
                "exactly authenticated login",
            ),
            (
                "wrong status",
                Board("PVT_1", {}, {70: BoardItem("ITEM_70", 70, "Ready")}),
                "Project 9 Status",
            ),
            (
                "foreign lease",
                _state(
                    (
                        _lease(
                            atom=70,
                            actor=OTHER_ACTOR,
                            login=OTHER_LOGIN,
                            lease_id=OTHER_LEASE_ID,
                        ),
                    )
                ),
                "canonical lease",
            ),
            (
                "active cap",
                _state(
                    tuple(
                        _lease(
                            atom=index,
                            actor=f"gordian-agent/harness/run-{index}",
                            lease_id=f"{index:032x}",
                        )
                        for index in range(1, MAX_ACTIVE_CLAIMS + 1)
                    )
                ),
                "active cap",
            ),
        )
        for label, value, expected in cases:
            with self.subTest(label=label):
                issue = (
                    value
                    if isinstance(value, IssueClaim)
                    else IssueClaim(70, "OPEN", (LOGIN,), None, ())
                )
                state = value if isinstance(value, ClaimLogState) else _state(())
                board = value if isinstance(value, Board) else self.board
                comment = value if isinstance(value, claims._LegacyClaimComment) else self.legacy
                with (
                    patch.dict(os.environ, {"GORDIAN_ACTOR": ACTOR}, clear=True),
                    patch.object(claims, "preflight", return_value=AUTH),
                    patch.object(claims, "_read_claim_log", return_value=state),
                    patch.object(claims, "_issue_claim", return_value=issue),
                    patch.object(claims, "fetch_board", return_value=board),
                    patch.object(claims, "_read_legacy_claim_comment", return_value=(comment, NOW)),
                    patch.object(claims, "_append_event") as append,
                    self.assertRaisesRegex(RuntimeError, expected),
                ):
                    adopt_legacy_claim(**self._kwargs())
                append.assert_not_called()

    def test_adoption_is_explicitly_limited_to_atom_70(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "only supported for Atom 70"):
            adopt_legacy_claim(**{**self._kwargs(), "number": 2})

    def test_comment_reader_rejects_malformed_timestamp_and_ambiguous_comments(self) -> None:
        malformed = {
            "id": 701,
            "body": f"{ACTOR}\nclaimed_at: not-a-timestamp\nexpires_after_minutes: {CLAIM_MINUTES}",
        }
        response = GitHubApiResponse(200, {"date": SERVER_DATE}, json.dumps([malformed]))
        with (
            patch.object(
                claims,
                "run_gh_json_response",
                return_value=(json.loads(response.body), response),
            ),
            self.assertRaisesRegex(RuntimeError, "timestamp is malformed"),
        ):
            claims._read_legacy_claim_comment(
                "kmosoti/gordian", number=70, expected_actor=ACTOR
            )

        body = claims._claim_comment(ACTOR, NOW)
        comments = [{"id": 701, "body": body}, {"id": 702, "body": body}]
        response = GitHubApiResponse(200, {"date": SERVER_DATE}, json.dumps(comments))
        with (
            patch.object(claims, "run_gh_json_response", return_value=(comments, response)),
            self.assertRaisesRegex(RuntimeError, "ambiguous"),
        ):
            claims._read_legacy_claim_comment(
                "kmosoti/gordian", number=70, expected_actor=ACTOR
            )

    def test_comment_reader_selects_matching_actor_among_foreign_legacy_comments(self) -> None:
        foreign = {"id": 701, "body": claims._claim_comment(OTHER_ACTOR, NOW)}
        matching = {"id": 702, "body": claims._claim_comment(ACTOR, NOW)}
        comments = [foreign, matching]
        response = GitHubApiResponse(200, {"date": SERVER_DATE}, json.dumps(comments))
        with patch.object(
            claims, "run_gh_json_response", return_value=(comments, response)
        ):
            selected, observed_now = claims._read_legacy_claim_comment(
                "kmosoti/gordian", number=70, expected_actor=ACTOR
            )
        self.assertEqual(selected.comment_id, 702)
        self.assertEqual(selected.lease.actor, ACTOR)
        self.assertEqual(observed_now, NOW)

    def test_comment_reader_rejects_two_matching_legacy_comments(self) -> None:
        comments = [
            {"id": 701, "body": claims._claim_comment(ACTOR, NOW)},
            {"id": 702, "body": claims._claim_comment(ACTOR, NOW)},
        ]
        response = GitHubApiResponse(200, {"date": SERVER_DATE}, json.dumps(comments))
        with (
            patch.object(claims, "run_gh_json_response", return_value=(comments, response)),
            self.assertRaisesRegex(RuntimeError, "ambiguous"),
        ):
            claims._read_legacy_claim_comment(
                "kmosoti/gordian", number=70, expected_actor=ACTOR
            )

    def test_comment_reader_rejects_malformed_matching_legacy_comment(self) -> None:
        comment = {
            "id": 701,
            "body": f"{ACTOR}\nclaimed_at: not-a-timestamp\n"
            f"expires_after_minutes: {CLAIM_MINUTES}",
        }
        comments = [comment]
        response = GitHubApiResponse(200, {"date": SERVER_DATE}, json.dumps(comments))
        with (
            patch.object(claims, "run_gh_json_response", return_value=(comments, response)),
            self.assertRaisesRegex(RuntimeError, "timestamp is malformed"),
        ):
            claims._read_legacy_claim_comment(
                "kmosoti/gordian", number=70, expected_actor=ACTOR
            )

    def test_comment_reader_rejects_invalid_expected_actor(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "expected_actor.*valid Gordian actor"):
            claims._read_legacy_claim_comment(
                "kmosoti/gordian", number=70, expected_actor="not-an-actor"
            )


if __name__ == "__main__":
    unittest.main()
