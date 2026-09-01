"""Shared validation for closure and attempt records.

The repository checker and readiness projection must agree about what a validating
closure is.  This module contains the small JSON-Schema subset used by the checked-in
schemas plus the cross-field/path, artifact-digest, and evidence-binding checks that are
intentionally not expressible in those schemas.

The module has no Mission Graph semantics.  It validates records and, when supplied a
reader, can validate bytes from an exact source revision without reading the mutable
working copy.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

_RFC3339_DATE_TIME = re.compile(
    r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$"
)
_SAFE_VERIFIER_ID = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")
_SHA256 = re.compile(r"^[0-9a-f]{64}$")

BytesReader = Callable[[str], bytes | None]
JJResolver = Callable[[str, str, str], "SourceBinding | None"]
_JSON_TYPES = frozenset(("object", "array", "string", "integer", "number", "boolean", "null"))
_JJ_COMMIT = re.compile(r"^[0-9a-f]{40}$")
_JJ_CHANGE = re.compile(r"^[a-z0-9]{32}$")
_ATOM_ID = re.compile(r"^[1-9][0-9]*$")
_LINE_BREAK = re.compile(r"[\r\n]")

# The first two lines of every verifier artifact.  A digest alone proves that a file
# exists; it does not say which state was verified or which command wrote the bytes.
# Atom #70 cited one 32,772-byte formal-verifier capture as the artifact of five different
# commands and every digest matched.  This header is the subject-to-predicate binding of
# an in-toto attestation, spelled in two lines so `head -2` can audit it.
EVIDENCE_SUBJECT_KEY = "subject_exact_state_id"
EVIDENCE_COMMAND_KEY = "command"

# A verifier's command is what a shell ran at the subject state, and the artifact header
# repeats it verbatim.  Atom #1 recorded "contract positive, injected-negative, and
# manifest-write-failure paths; committed manifest; ..." as a command: a description of
# a run, not a run, and the checker bound it because any non-empty string binds.  The
# first word after optional NAME=value assignments must therefore be a shell word that
# opens a compound command, an interpreter or a tool the repository pins
# (scripts/check-toolchain.sh, scripts/verify-local.sh), or a repository-relative path
# that exists at the subject state.  Deterministic on purpose: it never consults PATH,
# so a workspace and CI agree on what counts, and `true`, `:` and `echo` are absent
# because they verify nothing.
_COMMAND_ASSIGNMENT = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*=\S*$")
_SHELL_COMPOUND_WORDS = frozenset((
    "for", "while", "until", "if", "case", "(", "{", "!", "time", "cd", "set", "export",
    "eval", "exec", "test", "[", "[[", ".", "source",
))
_PINNED_TOOL_WORDS = frozenset((
    "bash", "sh", "env", "cargo", "cargo-deny", "rustc", "rustfmt", "lake", "lean",
    "leanchecker", "elan", "python", "python3", "python3.14", "ruff", "shellcheck", "jj",
    "gh",
))


@dataclass(frozen=True, slots=True)
class SourceBinding:
    """Resolved exact source subject for one closure record."""

    exact_state_id: str
    logical_change_id: str
    read: BytesReader
    in_trunk: bool
    before_bookkeeping: bool
    # Reads the subject state itself (where the verifiers ran); ``read`` serves the
    # bookkeeping state that carries the record.  ``None`` falls back to ``read``.
    read_subject: BytesReader | None = None


def jj_source_resolver(repository_root: Path, trunk_commit: str) -> JJResolver:
    """Resolve closure subjects from immutable JJ history, never the working copy."""
    def resolve(exact: str, logical: str, atom: str) -> SourceBinding | None:
        if _JJ_COMMIT.fullmatch(exact) is None or _JJ_CHANGE.fullmatch(logical) is None:
            return None
        relative = f"artifacts/atoms/{atom}/closure.json"
        try:
            resolved = subprocess.run(
                [
                    "jj", "log", "-r", exact, "-n", "2", "--no-graph",
                    "--ignore-working-copy", "-T", 'commit_id ++ "\\n" ++ change_id',
                ],
                cwd=repository_root, check=False, capture_output=True, text=True, timeout=30,
            )
            head = subprocess.run(
                [
                    "jj", "log", "-r", "@", "-n", "1", "--no-graph",
                    "--ignore-working-copy", "-T", "commit_id",
                ],
                cwd=repository_root, check=False, capture_output=True, text=True, timeout=30,
            )
            history = subprocess.run(
                [
                    "jj", "log", "-r",
                    f'files("{relative}") & {exact}::@ & ~{exact}', "-n", "2",
                    "--no-graph", "--ignore-working-copy", "-T", 'commit_id ++ "\\n"',
                ],
                cwd=repository_root, check=False, capture_output=True, text=True, timeout=30,
            )
            ancestry = subprocess.run(
                [
                    "jj", "log", "-r", f"{exact}::trunk()", "-n", "1", "--no-graph",
                    "--ignore-working-copy", "-T", 'commit_id ++ "\\n"',
                ],
                cwd=repository_root, check=False, capture_output=True, text=True, timeout=30,
            )
        except (OSError, subprocess.SubprocessError):
            return None
        lines = [line.strip() for line in resolved.stdout.splitlines() if line.strip()]
        if resolved.returncode != 0 or len(lines) != 2 or lines[0] != exact or lines[1] != logical:
            return None
        current_head = head.stdout.strip()
        if (
            head.returncode != 0
            or _JJ_COMMIT.fullmatch(current_head) is None
            or history.returncode != 0
        ):
            return None
        if exact != trunk_commit:
            if ancestry.returncode != 0 or ancestry.stdout.strip() != trunk_commit:
                return None
        elif current_head == exact:
            return None
        bookkeeping = [line.strip() for line in history.stdout.splitlines() if line.strip()]
        if len(bookkeeping) != 1:
            return None
        try:
            before = subprocess.run(
                [
                    "jj", "log", "-r", f"{exact}::{bookkeeping[0]} & ~{exact}", "-n", "1",
                    "--no-graph", "--ignore-working-copy", "-T", 'commit_id ++ "\\n"',
                ],
                cwd=repository_root, check=False, capture_output=True, text=True, timeout=30,
            )
        except (OSError, subprocess.SubprocessError):
            return None
        if before.returncode != 0 or before.stdout.strip() != bookkeeping[0]:
            return None
        def reader_for(revision: str) -> BytesReader:
            def read(path: str) -> bytes | None:
                try:
                    result = subprocess.run(
                        [
                            "jj", "file", "show", "-r", revision,
                            "--ignore-working-copy", "--", path,
                        ],
                        cwd=repository_root, check=False, capture_output=True, timeout=30,
                    )
                except (OSError, subprocess.SubprocessError):
                    return None
                return result.stdout if result.returncode == 0 else None
            return read
        return SourceBinding(
            exact, logical, reader_for(bookkeeping[0]), True, True, reader_for(exact)
        )
    return resolve


def evidence_header(exact_state: str, command: str) -> bytes:
    """Return the exact bytes a verifier artifact must open with to be evidence."""
    return (
        f"{EVIDENCE_SUBJECT_KEY}={exact_state}\n{EVIDENCE_COMMAND_KEY}={command}\n"
    ).encode()


def executable_command_problem(command: str, read_subject: BytesReader | None) -> str | None:
    """Say why ``command`` is not something a shell can run at the subject state.

    ``None`` means the first word after any ``NAME=value`` assignments is a shell word
    that opens a compound command, a pinned tool, or a normalized repository-relative
    path that ``read_subject`` can read.  Existence is not checked when no reader is
    given, so schema-only callers still reject prose without touching a repository.
    """
    words = command.split()
    while words and _COMMAND_ASSIGNMENT.fullmatch(words[0]) is not None:
        del words[0]
    if not words:
        return "command names no executable, only assignments"
    word = words[0]
    if word in _SHELL_COMPOUND_WORDS or word in _PINNED_TOOL_WORDS:
        return None
    if "/" not in word:
        return (
            f"command must start with an executable; {word!r} is not a pinned tool, a "
            "shell word, or a repository-relative path"
        )
    if word.startswith("/"):
        return f"command must start with a repository-relative path, not {word!r}"
    relative = word[2:] if word.startswith("./") else word
    if any(part in ("", ".", "..") for part in relative.split("/")):
        return f"command must start with a normalized repository-relative path, not {word!r}"
    if read_subject is not None and read_subject(relative) is None:
        return f"command starts with {word!r}, which does not exist at the subject state"
    return None


def evidence_binding_problems(artifact: bytes, exact_state: Any, command: str) -> list[str]:
    """Return why ``artifact`` is not bound to the state and command it is cited for.

    The artifact must begin with ``evidence_header(exact_state, command)``.  The two
    lines are checked separately so the failure names the line that lies.  A record
    whose ``exact_state_id`` is not a commit id cannot bind anything, and says so.
    """
    if not isinstance(exact_state, str) or _JJ_COMMIT.fullmatch(exact_state) is None:
        return ["artifact cannot be bound: closure.exact_state_id is not a commit id"]
    subject_line = f"{EVIDENCE_SUBJECT_KEY}={exact_state}\n".encode()
    if not artifact.startswith(subject_line):
        found = artifact.split(b"\n", 1)[0][:120].decode("utf-8", "replace")
        return [
            f"artifact line 1 must be {EVIDENCE_SUBJECT_KEY}={exact_state}; found {found!r}"
        ]
    command_line = f"{EVIDENCE_COMMAND_KEY}={command}\n".encode()
    if not artifact.startswith(subject_line + command_line):
        found = artifact[len(subject_line):].split(b"\n", 1)[0][:120].decode("utf-8", "replace")
        return [f"artifact line 2 must be {EVIDENCE_COMMAND_KEY}={command}; found {found!r}"]
    return []


def parse_rfc3339(value: Any) -> datetime | None:
    """Parse an RFC 3339 date-time and require a meaningful timezone.

    The schema's ``date-time`` format is checked here rather than by a shape-only
    regular expression.  The small syntax guard keeps Python's permissive
    ``fromisoformat`` variants (for example a space in place of ``T`` or a missing
    timezone) out, while ``datetime`` rejects impossible calendar and clock values.
    Returned values are timezone-aware UTC datetimes so callers can compare them
    semantically rather than lexicographically.
    """
    if not isinstance(value, str) or _RFC3339_DATE_TIME.fullmatch(value) is None:
        return None
    normalized = value[:-1] + "+00:00" if value.endswith("Z") else value
    try:
        parsed = datetime.fromisoformat(normalized)
    except ValueError:
        return None
    if parsed.tzinfo is None or parsed.utcoffset() is None:
        return None
    return parsed.astimezone(UTC)


def load_json(path: Path) -> Any:
    """Load one JSON file, letting the caller report the path-specific failure."""
    return json.loads(path.read_text(encoding="utf-8"))


def validate(value: Any, spec: Mapping[str, Any], path: str, problems: list[str]) -> None:
    """Validate the subset of JSON Schema used by the repository record schemas.

    This is deliberately the same contract the repository checker historically used:
    ``const``, ``enum``, basic types, string constraints, numeric minimums, object
    required/additional properties, and array item/minimum constraints.  Keeping it in
    one place prevents readiness from silently accepting a weaker record than CI does.
    """
    if not isinstance(spec, Mapping):
        problems.append(f"{path}: schema node is not an object")
        return
    if "const" in spec and (
        type(value) is not type(spec["const"]) or value != spec["const"]
    ):
        problems.append(f"{path}: expected const {spec['const']!r}, found {value!r}")
        return
    if "enum" in spec:
        allowed = spec["enum"]
        if not isinstance(allowed, list) or not any(
            type(value) is type(candidate) and value == candidate for candidate in allowed
        ):
            problems.append(f"{path}: {value!r} is not one of {allowed}")
            return

    expected = spec.get("type")
    if expected:
        kinds: dict[str, type | tuple[type, ...]] = {
            "object": dict,
            "array": list,
            "string": str,
            "integer": int,
            "number": (int, float),
            "boolean": bool,
        }
        names = expected if isinstance(expected, list) else [expected]
        if (
            not names
            or any(not isinstance(name, str) or name not in _JSON_TYPES for name in names)
        ):
            problems.append(f"{path}: invalid schema type {expected!r}")
            return
        if value is None:
            if "null" not in names:
                problems.append(f"{path}: expected {expected}, found null")
            return
        if (
            isinstance(value, bool)
            and "boolean" not in names
            and ("integer" in names or "number" in names)
        ):
            problems.append(f"{path}: expected {expected}, found boolean")
            return
        allowed_types = tuple(kinds[name] for name in names if name in kinds)
        if not allowed_types or not isinstance(value, allowed_types):
            problems.append(f"{path}: expected {expected}, found {type(value).__name__}")
            return

    if isinstance(value, str):
        if "pattern" in spec:
            pattern = spec["pattern"]
            try:
                matches = isinstance(pattern, str) and re.search(pattern, value)
            except re.error:
                matches = False
            if not matches:
                problems.append(f"{path}: {value!r} does not match {pattern!r}")
        minimum_length = spec.get("minLength")
        if minimum_length is not None:
            if not isinstance(minimum_length, int) or isinstance(minimum_length, bool):
                problems.append(f"{path}: schema minLength is not a non-negative integer")
            elif minimum_length < 0 or len(value) < minimum_length:
                problems.append(f"{path}: shorter than minLength {minimum_length}")
        maximum_length = spec.get("maxLength")
        if maximum_length is not None:
            if not isinstance(maximum_length, int) or isinstance(maximum_length, bool):
                problems.append(f"{path}: schema maxLength is not a non-negative integer")
            elif maximum_length < 0 or len(value) > maximum_length:
                problems.append(f"{path}: longer than maxLength {maximum_length}")
        if spec.get("format") == "date-time" and parse_rfc3339(value) is None:
            problems.append(f"{path}: {value!r} is not an RFC 3339 date-time")

    if isinstance(value, (int, float)) and not isinstance(value, bool):
        minimum = spec.get("minimum")
        if minimum is not None:
            if not isinstance(minimum, (int, float)) or isinstance(minimum, bool):
                problems.append(f"{path}: schema minimum is not a number")
            elif value < minimum:
                problems.append(f"{path}: {value} is below minimum {minimum}")

    if isinstance(value, dict):
        properties = spec.get("properties", {})
        if not isinstance(properties, Mapping):
            problems.append(f"{path}: schema properties is not an object")
            return
        required = spec.get("required", [])
        if not isinstance(required, list):
            problems.append(f"{path}: schema required is not an array")
            return
        for name in required:
            if not isinstance(name, str) or name not in value:
                problems.append(f"{path}: required property {name!r} is absent")
        if spec.get("additionalProperties") is False:
            for name in value:
                if name not in properties:
                    problems.append(f"{path}: unexpected property {name!r}")
        for name, child in value.items():
            if name in properties:
                validate(child, properties[name], f"{path}.{name}", problems)

    if isinstance(value, list):
        minimum = spec.get("minItems")
        if minimum is not None:
            if not isinstance(minimum, int) or isinstance(minimum, bool):
                problems.append(f"{path}: schema minItems is not a non-negative integer")
            elif minimum < 0 or len(value) < minimum:
                problems.append(f"{path}: {len(value)} items, minimum {minimum}")
        item_spec = spec.get("items")
        if item_spec:
            if not isinstance(item_spec, Mapping):
                problems.append(f"{path}: schema items is not an object")
            else:
                for position, item in enumerate(value):
                    validate(item, item_spec, f"{path}[{position}]", problems)


def schema_problems(payload: Any, schema: Mapping[str, Any] | None, label: str) -> list[str]:
    """Return schema failures for one already-decoded payload."""
    problems: list[str] = []
    if not isinstance(schema, Mapping) or not schema:
        problems.append(f"{label}: schema is missing or malformed")
    else:
        validate(payload, schema, label, problems)
    return problems


def closure_problems(
    payload: Any,
    schema: Mapping[str, Any] | None,
    *,
    label: str,
    expected_atom: str | None = None,
    record_path: str | None = None,
    read_artifact: BytesReader | None = None,
    resolve_source: JJResolver | None = None,
    source_binding_required: bool | None = None,
) -> list[str]:
    """Return all schema, atom-binding, and verifier artifact failures."""
    problems = schema_problems(payload, schema, label)
    if not isinstance(payload, dict):
        return problems

    expected_atom_is_safe = (
        isinstance(expected_atom, str) and _ATOM_ID.fullmatch(expected_atom) is not None
    )
    if expected_atom is not None and not expected_atom_is_safe:
        problems.append(f"{label}: expected Atom id must be a non-empty safe id")
    elif expected_atom is not None and payload.get("atom_id") != expected_atom:
        problems.append(
            f"{label}: atom_id {payload.get('atom_id')!r} does not match its directory "
            f"{expected_atom!r}"
        )

    exact_state = payload.get("exact_state_id")
    logical_change = payload.get("logical_change_id")
    binding = None
    binding_required = (
        resolve_source is not None
        if source_binding_required is None
        else source_binding_required
    )
    if binding_required and not isinstance(expected_atom, str):
        problems.append(f"{label}: source binding requires a canonical Atom id")
    if binding_required and isinstance(expected_atom, str):
        expected_record_path = f"artifacts/atoms/{expected_atom}/closure.json"
        if record_path is not None and record_path != expected_record_path:
            problems.append(
                f"{label}: record_path must be exactly {expected_record_path!r}"
            )
        if (
            isinstance(exact_state, str)
            and isinstance(logical_change, str)
            and _JJ_COMMIT.fullmatch(exact_state)
            and _JJ_CHANGE.fullmatch(logical_change)
        ):
            if resolve_source is None:
                problems.append(f"{label}: exact source resolver is unavailable")
            else:
                binding = resolve_source(exact_state, logical_change, expected_atom)
        if binding is None:
            problems.append(f"{label}: exact source state could not be resolved and validated")
        else:
            if binding.exact_state_id != exact_state:
                problems.append(f"{label}: resolver returned a different exact state")
            if binding.logical_change_id != logical_change:
                problems.append(f"{label}: exact state belongs to a different logical change")
            if not binding.in_trunk:
                problems.append(f"{label}: exact state is not an ancestor of trunk()")
            if not binding.before_bookkeeping:
                problems.append(f"{label}: exact state does not precede closure bookkeeping")

    artifact_reader = read_artifact
    subject_reader = read_artifact
    if binding is not None:
        artifact_reader = binding.read
        subject_reader = binding.read_subject or binding.read
        if record_path is None:
            problems.append(f"{label}: source binding requires the canonical record path")
        elif record_path != f"artifacts/atoms/{expected_atom}/closure.json":
            pass
        else:
            authoritative = binding.read(record_path)
            if authoritative is None:
                problems.append(f"{label}: closure record is absent from bookkeeping state")
            else:
                try:
                    authoritative_payload = json.loads(authoritative.decode("utf-8"))
                except (UnicodeDecodeError, json.JSONDecodeError):
                    problems.append(f"{label}: bookkeeping closure record is malformed")
                else:
                    if authoritative_payload != payload:
                        problems.append(
                            f"{label}: closure record differs from authoritative bookkeeping state"
                        )

    if binding_required:
        for position, verifier in enumerate(payload.get("verifiers", [])):
            if (
                isinstance(verifier, dict)
                and verifier.get("subject_exact_state_id") != exact_state
            ):
                problems.append(
                    f"{label}.verifiers[{position}]: subject_exact_state_id must equal "
                    "closure.exact_state_id"
                )

    verifiers = payload.get("verifiers")
    if not isinstance(verifiers, list):
        return problems

    seen_verifier_ids: set[str] = set()
    seen_digests: dict[str, int] = {}
    for position, verifier in enumerate(verifiers):
        if not isinstance(verifier, dict):
            continue

        verifier_id = verifier.get("verifier_id")
        verifier_id_is_safe = (
            isinstance(verifier_id, str) and _SAFE_VERIFIER_ID.fullmatch(verifier_id) is not None
        )
        duplicate_verifier_id = False
        if not verifier_id_is_safe:
            problems.append(
                f"{label}.verifiers[{position}]: verifier_id must be a non-empty safe id"
            )
        elif verifier_id in seen_verifier_ids:
            duplicate_verifier_id = True
            problems.append(
                f"{label}.verifiers[{position}]: duplicate verifier_id {verifier_id!r}"
            )
        else:
            seen_verifier_ids.add(verifier_id)

        command = verifier.get("command")
        command_is_bindable = isinstance(command, str) and bool(command)
        if not command_is_bindable:
            problems.append(
                f"{label}.verifiers[{position}]: command must be a non-empty string"
            )
        elif _LINE_BREAK.search(command) is not None:
            # The command is bound into one artifact header line; a line break inside
            # it would let a second command hide behind the first.
            command_is_bindable = False
            problems.append(
                f"{label}.verifiers[{position}]: command must not contain line breaks"
            )
        else:
            executable_problem = executable_command_problem(command, subject_reader)
            if executable_problem is not None:
                command_is_bindable = False
                problems.append(f"{label}.verifiers[{position}]: {executable_problem}")

        exit_code = verifier.get("exit_code")
        exit_code_is_zero = type(exit_code) is int and exit_code == 0
        if not exit_code_is_zero:
            problems.append(
                f"{label}.verifiers[{position}]: exit_code must be exactly 0, found {exit_code!r}"
            )

        artifact = verifier.get("artifact_path")
        digest = verifier.get("artifact_sha256")
        digest_is_valid = isinstance(digest, str) and _SHA256.fullmatch(digest) is not None
        if not digest_is_valid:
            problems.append(
                f"{label}.verifiers[{position}]: artifact_sha256 must be 64 lowercase "
                "hex characters"
            )
        elif digest in seen_digests:
            # Distinct verifiers are distinct runs; one capture cannot witness two commands.
            problems.append(
                f"{label}.verifiers[{position}]: artifact_sha256 duplicates "
                f"verifiers[{seen_digests[digest]}]; one artifact cannot witness two verifiers"
            )
        else:
            seen_digests[digest] = position

        if not expected_atom_is_safe or not verifier_id_is_safe:
            expected_path = None
        else:
            expected_path = (
                f"artifacts/atoms/{expected_atom}/verifiers/{verifier_id}.log"
            )
        if not isinstance(artifact, str) or not artifact:
            problems.append(
                f"{label}.verifiers[{position}]: artifact_path must be a non-empty "
                "repository-relative path"
            )
            continue
        if record_path is not None and artifact == record_path:
            problems.append(
                f"{label}.verifiers[{position}]: a record cannot record its own digest"
            )
            continue
        if expected_path is None or artifact != expected_path:
            expected_text = expected_path or "the canonical Atom verifier log path"
            problems.append(
                f"{label}.verifiers[{position}]: artifact_path {artifact!r} must be exactly "
                f"{expected_text!r}"
            )
            continue

        if (
            not verifier_id_is_safe
            or duplicate_verifier_id
            or not command_is_bindable
            or not exit_code_is_zero
            or not digest_is_valid
        ):
            continue

        if read_artifact is None:
            # Schema-only callers can still inspect structural records.  The repository
            # checker and readiness path always provide a reader, so no record is
            # considered satisfying evidence without this exact-artifact read.
            continue

        resolved = artifact_reader(expected_path) if artifact_reader is not None else None
        if resolved is None:
            problems.append(
                f"{label}.verifiers[{position}]: artifact_path {expected_path} does not exist"
            )
            continue
        actual = hashlib.sha256(resolved).hexdigest()
        if not isinstance(digest, str) or actual != digest:
            problems.append(
                f"{label}.verifiers[{position}]: {expected_path} hashes to {actual}, "
                f"record says {digest!r}"
            )
            continue
        # The bytes are the ones the record names.  Now: are they evidence for *this*
        # state and *this* command, or a file that merely exists?
        for problem in evidence_binding_problems(resolved, exact_state, command):
            problems.append(f"{label}.verifiers[{position}]: {problem}")

    return problems


def attempt_problems(
    payload: Any,
    schema: Mapping[str, Any] | None,
    *,
    label: str,
    attempts_path: str | None = None,
) -> list[str]:
    """Return schema and cross-field failures for one attempt record."""
    problems = schema_problems(payload, schema, label)
    if not isinstance(payload, dict):
        return problems

    if attempts_path is not None:
        if os.path.basename(os.path.dirname(attempts_path)) != "attempts":
            problems.append(
                f"{label}: attempt records live in artifacts/atoms/<atom_id>/attempts/"
            )
        else:
            expected_atom = os.path.basename(os.path.dirname(os.path.dirname(attempts_path)))
            if payload.get("atom_id") not in (None, expected_atom):
                problems.append(
                    f"{label}: atom_id {payload['atom_id']!r} does not match its directory "
                    f"{expected_atom}"
                )
            attempt_id = payload.get("attempt_id")
            expected_name = f"{attempt_id}.json"
            if attempt_id and os.path.basename(attempts_path) != expected_name:
                problems.append(
                    f"{label}: attempt_id {attempt_id!r} requires the file name {expected_name!r}"
                )

    attempt_id = payload.get("attempt_id")
    started = payload.get("started_at")
    started_dt = parse_rfc3339(started)
    if isinstance(attempt_id, str) and started_dt is not None:
        stamp = started_dt.strftime("%Y%m%dT%H%M%SZ")
        if not attempt_id.startswith(stamp):
            problems.append(
                f"{label}: attempt_id {attempt_id!r} does not open with started_at {stamp!r}"
            )

    finished = payload.get("finished_at")
    finished_dt = parse_rfc3339(finished)
    if started_dt is not None and finished_dt is not None and finished_dt < started_dt:
        problems.append(f"{label}: finished_at {finished} precedes started_at {started}")

    outcome = payload.get("outcome")
    defect_issue = payload.get("contract_defect_issue")
    if outcome == "contract_defect" and defect_issue is None:
        problems.append(
            f"{label}: outcome contract_defect requires contract_defect_issue; the runbook's "
            "section 7 forbids editing the Atom's acceptance bullets instead"
        )
    if outcome != "contract_defect" and defect_issue is not None:
        problems.append(f"{label}: contract_defect_issue is set but outcome is {outcome!r}")
    return problems


def local_bytes_reader(root: Path) -> BytesReader:
    """Build a safe repository-relative reader for the checker."""
    resolved_root = root.resolve()

    def read(relative: str) -> bytes | None:
        candidate = Path(relative)
        if candidate.is_absolute() or ".." in candidate.parts:
            return None
        path = (resolved_root / candidate).resolve()
        try:
            path.relative_to(resolved_root)
        except ValueError:
            return None
        try:
            return path.read_bytes()
        except (OSError, ValueError):
            return None

    return read


def repository_source_resolver(root: Path) -> JJResolver | None:
    """Build the production resolver from the accepted immutable trunk revision."""
    try:
        result = subprocess.run(
            [
                "jj", "log", "-r", "trunk()", "-n", "1", "--no-graph",
                "--ignore-working-copy", "-T", "commit_id",
            ],
            cwd=root, check=False, capture_output=True, text=True, timeout=30,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    trunk = result.stdout.strip()
    if result.returncode != 0 or _JJ_COMMIT.fullmatch(trunk) is None:
        return None
    return jj_source_resolver(root, trunk)


def validate_tree(
    root: Path, *, resolve_source: JJResolver | None = None
) -> tuple[list[str], int, int]:
    """Validate every closure and attempt record under ``root``.

    Returns ``(problems, closure_count, attempt_count)`` in deterministic path order.
    """
    closure_schema_path = root / "artifacts/schema/closure-record.schema.json"
    attempt_schema_path = root / "artifacts/schema/attempt-record.schema.json"
    problems: list[str] = []
    try:
        closure_schema = load_json(closure_schema_path)
    except (OSError, ValueError) as error:
        problems.append(f"{closure_schema_path}: unreadable or malformed JSON: {error}")
        closure_schema = None
    try:
        attempt_schema = load_json(attempt_schema_path)
    except (OSError, ValueError) as error:
        problems.append(f"{attempt_schema_path}: unreadable or malformed JSON: {error}")
        attempt_schema = None

    records = sorted((root / "artifacts/atoms").glob("*/closure.json"))
    attempts = sorted((root / "artifacts/atoms").glob("*/attempts/*.json"))
    reader = local_bytes_reader(root)
    resolver = resolve_source if resolve_source is not None else repository_source_resolver(root)
    for path in records:
        label = str(path)
        try:
            payload = load_json(path)
        except (OSError, ValueError) as error:
            problems.append(f"{label}: unreadable or malformed JSON: {error}")
            continue
        expected_atom = path.parent.name
        record_path = path.relative_to(root).as_posix()
        problems.extend(
            closure_problems(
                payload,
                closure_schema,
                label=label,
                expected_atom=expected_atom,
                record_path=record_path,
                read_artifact=reader,
                resolve_source=resolver,
                source_binding_required=True,
            )
        )
    for path in attempts:
        label = str(path)
        try:
            payload = load_json(path)
        except (OSError, ValueError) as error:
            problems.append(f"{label}: unreadable or malformed JSON: {error}")
            continue
        problems.extend(
            attempt_problems(
                payload,
                attempt_schema,
                label=label,
                attempts_path=path.relative_to(root).as_posix(),
            )
        )
    return problems, len(records), len(attempts)


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Validate Gordian closure and attempt records")
    parser.add_argument("root", type=Path)
    arguments = parser.parse_args(argv)
    root = arguments.root.resolve()
    problems, closure_count, attempt_count = validate_tree(root)
    if closure_count == 0 and attempt_count == 0 and not problems:
        print("SKIP: no closure or attempt records yet (both schemas present and parsed).")
        return 0
    if problems:
        for problem in problems:
            print(f"FAIL: {problem}")
        return 1
    print(
        f"OK: {closure_count} closure record(s) valid with every artifact digest matching "
        f"and bound to its subject state and command, and {attempt_count} attempt "
        "record(s) valid."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
