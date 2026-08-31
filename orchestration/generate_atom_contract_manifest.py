"""One-off construction of the declarative Atom-contract manifest."""

# The deferred semantic inventory below intentionally keeps source Markdown
# wording intact; those long lines are not executable normalization payload.
# ruff: noqa: E501

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

ROOT = Path(".")
sys.path.insert(0, str(ROOT / "orchestration" / "src"))
_arguments = argparse.ArgumentParser(
    description="Generate the declarative Atom-contract normalization manifest."
)
_arguments.add_argument(
    "--issues",
    type=Path,
    required=True,
    help="Path to the issue snapshot used only as generation input.",
)
ISSUES = _arguments.parse_args().issues
rows = {int(row["number"]): row for row in json.loads(ISSUES.read_text(encoding="utf-8"))}


def cached_issues() -> tuple[Any, ...]:
    from gordian_orchestration.derive_status import IssueRecord

    issues = []
    for number, row in sorted(rows.items()):
        labels = tuple(sorted(item["name"] for item in row.get("labels", ())))
        native_blockers = row.get("blockedBy")
        if not isinstance(native_blockers, list) or any(
            isinstance(value, bool) or not isinstance(value, int) or value < 1
            for value in native_blockers
        ):
            raise RuntimeError(
                f"#{number}: issue snapshot must provide native blockedBy integers"
            )
        issues.append(
            IssueRecord(
                number=number,
                title=str(row["title"]),
                state=str(row["state"]),
                # The body is a human-readable mirror.  Never reconstruct the
                # authoritative graph from it while generating a normalization
                # plan; a stale body must be repaired to the native edge set.
                blocked_by=tuple(sorted(set(native_blockers))),
                body=str(row["body"]),
                labels=labels,
                milestone=None,
                url=str(row.get("url", "")),
            )
        )
    return tuple(issues)
crate_map = (ROOT / "docs/implementation/crate-map.md").read_text(encoding="utf-8")
BT = chr(96)


def md(text: str) -> str:
    return text.replace("@@", BT)


targets: dict[int, set[str]] = {}
for line in crate_map.splitlines():
    cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
    if len(cells) != 4 or not cells[0].startswith("`gordian-"):
        continue
    crate = cells[0].strip(BT)
    for owner in re.findall(r"#(\d+)(?:[ \t]*\([^()\n]*\))?", cells[3]):
        targets.setdefault(int(owner), set()).add(crate)
target_sets = {number: sorted(targets.get(number, set())) for number in range(1, 78)}


def section(number: int, heading: str, occurrence: int = 0) -> str:
    body = rows[number]["body"]
    matches = list(
        re.finditer(r"^## " + re.escape(heading) + r"[ \t]*$", body, re.MULTILINE)
    )
    if occurrence >= len(matches):
        raise RuntimeError(f"#{number}: missing {heading!r} occurrence {occurrence}")
    match = matches[occurrence]
    following = body[match.end() :]
    next_heading = re.search(r"^## [^\n]+$", following, re.MULTILINE)
    end = match.end() + (next_heading.start() if next_heading else len(following))
    return body[match.start() : end]


def section_replacement(
    number: int, heading: str, content: str, occurrence: int = 0
) -> dict[str, object]:
    return {
        "old": section(number, heading, occurrence),
        "new": f"## {heading}\n{md(content).rstrip()}\n\n",
    }


def paragraph_replacement(
    number: int, old: str, new: str, count: int = 1
) -> dict[str, object]:
    body = rows[number]["body"]
    found = body.count(old)
    if found != count:
        raise RuntimeError(
            f"#{number}: expected {count} occurrence(s), found {found} for {old!r}"
        )
    return {"old": old, "new": md(new), "count": count}


semantic: dict[int, list[dict[str, object]]] = {}
semantic[4] = [
    paragraph_replacement(
        4,
        "The differential test harness is `crates/gordian-reference/tests/differential.rs`.",
        "The differential test harness is `crates/gordian-core/tests/reference_differential.rs`.",
    )
]
semantic[76] = [
    paragraph_replacement(
        76,
        "- Implement `SourceAdapter` for Git worktrees in `crates/gordian-source/src/git/`, realizing every",
        "- Implement `SourceAdapter` for Git worktrees in `crates/gordian-git/src/`, realizing every",
    ),
    paragraph_replacement(
        76,
        "- The adapter is selected by a single configuration value drawn from the closed set `{jj, git}`; nothing\n  above `gordian-source` reads it.",
        "- Runtime selects the backend from the closed set `{jj, git}`; domain crates consume the\n  `SourceAdapter` trait and capability flags from `gordian-source` without reading backend selection.",
    ),
]
semantic[10] = [
    section_replacement(
        10,
        "Acceptance",
        """- Allowed decomposition kinds are enforced independently from dependency edges.
- Sibling creation/order never implies causal dependency.
- Hard-dependency depender and prerequisite kinds are both @@Atom@@, per @@docs/spec/data-model.md@@ "Global hard dependency target kinds"; validation rejects any other kind, rejects Quark targets, and rejects cycles over declared and derived edges.
- Plan validation rejects any Atom whose @@required_interfaces@@ or @@declared_inputs@@ are neither provided/produced by another Atom in the plan nor registered as @@ExternalProvision@@ (#58).
- Plan validation resolves exactly one @@ProviderBinding@@ per @@(consumer_atom, requirement)@@ and rejects an ambiguous plan in which two members provide the same interface without an explicit binding.
- Validation rejects any @@AtomSpec@@ precondition expression that references a work-object identifier.
- @@RankedDependencyGraph@@ carries @@WorkKind@@; @@edge_target_globally_dependable@@ (G-222) rejects every Quark dependency target structurally.
- Quark ownership is unique (G-232): every Quark belongs to exactly one Atom.
- Rank computation and cycle detection are deterministic and executable (G-231), and optional rank/topological certificates are checked edge-by-edge (G-233).
- Containment (@@contains@@) is distinct from @@dependsOn@@; containment never adds execution precedence, and dependency edges never arise from sibling creation order. The graph model records this separation (G-249).
- Generated sparse/dense/wide/deep DAG tests cover edge cases, and the reference path remains linear-time in graph size.""",
    )
]
semantic[15] = [
    section_replacement(
        15,
        "Required binding",
        """@@Evidence.binding@@ is required and non-null and has exactly these seven normative fields:

@@@@@@text
spec_revision
exact_state_id
input_digest
dependency_digest
environment_digest
verifier_digest
canonicalization_scheme
@@@@@@

The same seven fields remain separately queryable and are carried by both @@CandidateRef@@ and @@EvidenceRef@@; adding, removing, or re-meaning a field changes @@canonicalization_scheme@@ and invalidates prior evidence.""",
    ),
    section_replacement(
        15,
        "Acceptance",
        """- Canonicalization is deterministic and inspectable.
- @@Evidence.Compatible@@ is relevance-conditioned: it compares the verifier-declared relevant subset of the seven fields and fails closed on every relevant mismatch, while all seven stored fields remain available for provenance and freshness.
- @@Fresh@@ compares exactly the seven @@EvidenceBinding@@ fields, including @@dependency_digest@@ and @@canonicalization_scheme@@; a canonicalization-scheme change invalidates all prior evidence.
- @@CandidateRef@@ and @@EvidenceRef@@ are distinct types, each carrying exactly the seven normative fields, so a candidate cannot be passed where evidence is expected.
- Historical stale evidence remains addressable but cannot satisfy current acceptance.
- The dependency-boundary completeness assumption is documented and instrumentable.
- G-202 supplies executable @@Evidence.isCompatible@@; G-214 supplies evidence-compatibility content, G-238 supplies relevance-conditioned compatibility, G-241 supplies the distinct reference types, and G-310 is the binding/fingerprint integration obligation.""",
    ),
]
semantic[18] = [
    section_replacement(
        18,
        "Acceptance",
        """- Define positive capability grants scoped by actor, action, resource, and optional expiry/fencing context.
- @@issued_at_event@@, @@expires_at_event@@, and @@revoked_at_event@@ are dense @@EventSeq@@ values. Grant liveness is evaluated only from event order at the named evaluation event; wall-clock timestamps are provenance and never a predicate input.
- Delegation is explicit and bounded, and @@deploy_requires_explicit_grant@@ is enforced rather than inferred from a role name (G-211).
- Implement a minimal internal evaluator/reference policy and build equivalent representative Cedar policies.
- Compare expressiveness, analyzability, auditability, latency/throughput, memory, dependency/operational surface, and failure modes.
- Confirm abstract Worker/Coordinator/DeploymentAuthority separation through runtime enforcement tests.
- Adoption rule, pre-registered before the measurement runs: adopt Cedar iff all four thresholds are met — p99 policy-evaluation latency <= 50 microseconds; binary-size delta <= 3.0 MB; added transitive dependency count <= 40; and at least 3 required policy classes are expressible only in Cedar. If any threshold fails, keep the reference evaluator.
- Extend @@formal/Gordian/Authority.lean@@ to model grant and delegation so that "by default" in @@coordinator_cannot_deploy_by_default@@ has formal content.""",
    )
]
semantic[19] = [
    section_replacement(
        19,
        "Acceptance",
        """- Admission takes an @@IntegrationCandidate@@ built over the current frontier and a batch; it never admits a bare Candidate.
- The admission witness is exactly these ten ordered conjuncts from @@docs/spec/mission-graph.md@@ @@## Accepted frontier@@: @@CurrentFrontierReconciled@@, @@ParentsUnadmitted@@, @@NoUnresolvedConflict@@, @@VerifierManifestComplete@@, @@RequiredVerificationPasses@@, @@EvidenceBoundToExactCandidate@@, @@EvidenceFresh@@, @@EvidenceProvenanceValid@@, @@LeaseValidAtFreeze@@, and @@AuthorizedPromotion@@. Admission rejects a candidate failing any one.
- @@CurrentFrontierReconciled@@ checks source/frontier agreement, expected frontier ancestry, batch identity, and no unresolved conflict; unresolved structural or source-plane conflict fails closed.
- The required verifier manifest is complete and every required result freshly matches the exact candidate. Evidence provenance requires a preceding matching verification event.
- The frontier CAS is an expected-@@FrontierVersion@@ append to the canonical event log, followed by idempotent adapter-observed bookmark move/publish and one atomic completion transaction containing @@FrontierMoved@@, every @@AtomSatisfied@@, and @@CandidateClaimReleased@@; crash recovery re-drives unmatched intents and closes an unrecoverable intent with @@AdmissionAborted@@.
- The witness is evaluated at a named @@ProjectionVersion@@ and committed under a @@WitnessGuard@@ on that version as well as on @@FrontierVersion@@; @@CandidateAdmitted@@ records the witness itself, not only its digest.
- Admission requires a live exclusive @@LeaseSubject::Coordinator(project)@@ lease; completion is one conditional transactional append of @@FrontierMoved@@, every @@AtomSatisfied@@, and @@CandidateClaimReleased@@.
- @@SatisfactionRestored@@ is the only post-invalidation restoration path: under the same frontier/witness guards, the Atom Candidate must be in the current frontier's transitive parents and every required verifier must have a fresh pass on that current frontier.
- A candidate whose @@admission_attempts@@ (counting both @@AdmissionPreempted@@ and @@IntegrationConflictObserved@@) reaches @@MAX_ADMISSION_ATTEMPTS@@ (3) is admitted in an exclusive single-member batch, in FIFO order; after @@MAX_EXCLUSIVE_ATTEMPTS@@ it is closed with @@AdmissionRejected@@.
- Every false conjunct appends @@AdmissionRejected { subject, conjunct, detail }@@ and atomically releases the batch claims with @@CandidateClaimReleased@@; abort compensation uses @@AdmissionAborted@@ only after @@reset_frontier@@, and admission never fails silently.
- Define executable @@isAcceptable@@ and the conflict/evidence predicates in @@formal/Gordian/Acceptance.lean@@ rather than opaque @@Prop@@ fields (G-201, G-207, G-208, G-209, G-220, G-225, T020).""",
    ),
]
semantic[22] = [
    section_replacement(
        22,
        "Acceptance",
        """- Semantic resource identities are project-scoped and typed enough to represent crates/modules/types/APIs/schemas/config/services/artifacts.
- Atom specs can declare read, write, provide, and require claims; the claim expressions are executable data, not opaque predicates.
- @@DeclaredNonInterfering(A, B)@@ is the executable three-way disjointness contract: @@writes(A)@@ is disjoint from @@reads(B)@@, @@writes(A)@@ is disjoint from @@writes(B)@@, and @@writes(B)@@ is disjoint from @@reads(A)@@.
- Admission consumes @@DeclaredNonInterfering@@ as its conflict witness (G-246), and the claim evaluator is executable and testable (G-217).
- Execution observations can record file/module/interface/config/network/resource access without pretending instrumentation is complete.
- Undeclared writes produce a @@ScopeExpanded@@ event and force conflict/admission re-evaluation before candidate promotion.
- File/path overlap remains available as a cheap baseline predictor.
- @@ResourceClaim.confidence@@ is consumed only as a feature of the #52 conflict predictor; no scheduling, readiness, non-interference, lease, or admission predicate reads it.
- Extend @@formal/Gordian/Conflict.lean@@ to connect @@DeclaredNonInterfering@@ to the acceptance witness's @@conflictFree@@ field.""",
    )
]
semantic[23] = [
    section_replacement(
        23,
        "Acceptance",
        """- Leases are keyed on @@LeaseSubject@@, a sum of @@SemanticResource@@, @@LogicalChange@@, and @@Coordinator@@; at most one live @@write_exclusive@@ lease exists per subject.
- @@issued_at_event@@, @@expires_at_event@@, and @@revoked_at_event@@ are @@EventSeq@@ values. @@live(L, at_event)@@ is determined by event ordering, and expiry appends @@LeaseExpired@@; wall-clock expiry is provenance only and never protects a paused stale process.
- A holder self-fences: a @@CandidateFrozen@@ appended under a non-live @@LeaseSubject::LogicalChange@@ lease is rejected by the canonical event log.
- Shared write grants are permitted only under the @@commutative_operations@@ rule of @@docs/spec/data-model.md@@ @@## Lease@@; @@LogicalChange@@ subjects are never shared-write leased.
- Exclusive grants carry monotonically increasing fencing tokens where downstream enforcement is possible.
- Expiry, revocation, renewal, actor loss, and stale-holder behavior are explicit event transitions and lease facts are provenance-visible.
- Add @@formal/Gordian/Lease.lean@@ with the lease transition/exclusivity theorem G-227; @@docs/spec/invariants.md@@ @@## One normal-path writer per evolving change@@ gains its Lean counterpart.""",
    )
]
semantic[26] = [
    section_replacement(
        26,
        "Acceptance",
        """- The mandatory projection set is exactly this closed list: @@atom_state@@, @@mission_state@@, @@active_attempts@@, @@leases@@, @@fresh_evidence@@, @@ready_queue@@, @@accepted_frontier@@, @@frontier_chain@@, @@satisfaction_index@@, @@admission_queue@@, @@admission_claims@@, @@admission_attempts@@, @@observed_project_state@@, @@executor_registry@@, @@resource_pools@@, and @@capability_grants@@.
- All sixteen projections are rebuildable deterministically from canonical history and immutable records; @@satisfaction_index@@ writes are idempotent per @@(atom, frontier_seq)@@.
- Projection rows are never the sole canonical truth for facts derivable from history or immutable records.
- Rebuild can discard projections and reconstruct canonical fields deterministically.
- Projection updates and event persistence have an explicit crash-consistency protocol.
- Rebuild mismatch is a first-class invariant violation with evidence, not a warning log.
- Replaying a history across a @@PlanSelected@@ reproduces every historical @@Fingerprint(I, v)@@ byte for byte; no predicate or projector reads a wall clock.""",
    ),
    paragraph_replacement(
        26,
        "covering every one of the eleven mandatory projections named in Acceptance",
        "covering every one of the sixteen mandatory projections named in Acceptance",
    ),
]
semantic[31] = [
    section_replacement(
        31,
        "Acceptance",
        """- Candidate handoff records Atom/spec, producer attempt, exact base, @@plan_revision@@, @@base_frontier_seq@@, @@logical_change_id@@, @@exact_state_id@@, @@fencing_token@@, and @@frozen_at_event@@.
- Freeze requires a live exclusive @@LeaseSubject::LogicalChange(logical_change_id)@@ lease. At the freeze event the candidate records the highest fencing token held for that subject; a lower or stale token cannot freeze or hand off.
- Candidate is logically immutable for that verification attempt. A subsequent rewrite preserving @@logical_change_id@@ creates a new Candidate and invalidates evidence bound to the previous @@exact_state_id@@.
- Candidate metadata can be reconstructed after coordinator restart, and the worker cannot silently mutate the verification subject after handoff.
- The candidate-freeze transition and token ordering are the T011 / G-223 obligation.""",
    ),
]
semantic[32] = [
    section_replacement(
        32,
        "Acceptance",
        """- Independent candidates are not linearized merely by finish time.
- Integration produces an @@IntegrationCandidate@@ with its own identity, @@plan_revision@@-frozen @@integration_manifest@@, @@base_frontier@@, @@base_frontier_seq@@, parent candidates, exact state, and @@frozen_at_event@@; every parent candidate has @@parent.frozen_at_event < I.frozen_at_event@@.
- A conflicting member is removed from the batch without invalidating the rest, and removal produces a new @@IntegrationCandidate@@ rather than mutating the old one.
- Attribution is unconditional: every conflicted hunk in an integration candidate maps to at least one source candidate. The only exempt conflict kinds are the closed list (a) whole-file delete/modify pairs, demonstrated by the @@delete_modify@@ fixture; (b) binary blobs with no line structure, demonstrated by the @@binary_blob@@ fixture; and (c) generated-file regeneration collisions, demonstrated by the @@generated_artifact@@ fixture. No other exemption exists.
- Conflict resolution can be executed as a bounded Atom/attempt with explicit provenance.
- Integrated state receives independent verifier evidence, and unresolved conflict cannot pass candidate admission.""",
    )
]
semantic[41] = [
    section_replacement(
        41,
        "Acceptance",
        """- The canonical event log is the authority for lease, candidate, frontier, and admission state; materialized views, local bookmarks, and process state are projections only.
- Admission and accepted-frontier movement require a live exclusive coordinator lease on @@LeaseSubject::Coordinator(project)@@ and @@move_accepted_frontier@@; no worker or failover process can exercise that capability implicitly.
- @@FrontierVersion@@ is the only accepted-frontier CAS target, and the admission witness is guarded by its @@WitnessGuard@@/@@ProjectionVersion@@ scope.
- Source-plane move and publish are idempotent adapter effects that are observed and reconciled. Canonical events are @@CandidateAdmitted@@ intent and @@FrontierMoved@@ completion; a failed or permanently unmatched intent performs a compensating @@reset_frontier@@ to the expected frontier, then appends @@AdmissionAborted@@.
- Re-drive is idempotent and bounded by @@MAX_REDRIVE_ATTEMPTS@@; recovery reconstructs the coordinator lease, witness, and intent from the event log without reissuing nondeterministic work.
- Remote workers/coordinators cannot create two valid exclusive write leases for the same resource under the defined authority model, and stale fencing tokens are rejected where enforcement is available.
- Coordinator failover does not grant two independent writers to accepted reality; lease/frontier state recovers from canonical persistence/events and no correctness assumption depends on synchronized wall clocks alone.""",
    )
]
semantic[49] = [
    section_replacement(
        49,
        "Dependency rationale",
        """#48 supplies the native plan import and #38 supplies the local multi-worker coordinator; #33 supplies exact-revision verification and #28 supplies the crash/duplicate/recovery fault suite. Together these are the bounded prerequisite closure in @@docs/implementation/execution-order.md@@ section 15. Trace and metrics Atoms are outcomes of the runtime Mission, not imported dependencies or admission claims of this Atom; #34 and #57 consume the resulting evidence later.""",
    ),
    section_replacement(
        49,
        "Acceptance",
        """- Create a separate native runtime @@Mission@@ and selected @@PlanRevision@@ for self-hosting. Trace and metrics Atoms are outcomes linked to that Mission run; #48 neither imports nor decomposes #43, and this Atom makes no #43 satisfaction claim.
- Native Mission Graph derives ready work and dispatches isolated workers from exact bases through the Jujutsu source adapter (#29, #30, #31) behind the source-adapter trait. The Jujutsu-versus-Git comparison (#34) is scored against this run's substrate afterwards and is not a prerequisite of it.
- Workers coordinate through explicit shared semantic state, not unrestricted shared source mutation.
- Every candidate is frozen and verified against its exact @@exact_state_id@@.
- The integration candidate receives independent verification.
- The accepted frontier moves only through authorized admission (#19).
- Kill and restart the coordinator and reconstruct the run from canonical persistence without reissuing nondeterministic work. This is the real-workload repetition of the #38 fixture-worker restart test, and #28's crash/recovery fault suite is its fixture-level counterpart.
- Produce a complete provenance/evidence bundle and report expected versus observed Mission state as a typed delta set, with @@gordian-runtime::observability@@ trace/metrics evidence linked to the run rather than used as a satisfaction shortcut.

Self-hosting runs only the constrained-local-process sandbox backend; qualified sandbox backends (#62) and secret brokerage (#63) are evidence for #67 and #69, not preconditions of #49.""",
    ),
    paragraph_replacement(
        49,
        "the PlanRevision #48 imported",
        "the selected PlanRevision of the separate runtime Mission",
    ),
]
semantic[54] = [
    paragraph_replacement(
        54,
        "crates/gordian-mission/tests/fixtures/mutable_status.rs",
        "crates/gordian-core/tests/fixtures/mutable_status.rs",
    )
]
semantic[67] = [
    paragraph_replacement(
        67,
        "crates/gordian-security/tests/threat_fixtures.rs",
        "crates/gordian-runtime/tests/threat_fixtures.rs",
        count=10,
    )
]
semantic[69] = [
    section_replacement(
        69,
        "Out of first qualification",
        """No waiver removes #41. Distributed lease and accepted-frontier coordination is transitively included through #42 and remains part of release qualification; remote-worker robustness and the Mission Graph explorer are reached through the native blocked-by graph.""",
    )
]
semantic[70] = [
    section_replacement(
        70,
        "Acceptance",
        """- Preflight maps @@GORDIAN_GH_TOKEN -> GH_TOKEN@@ deterministically, checks identity and required repository/Project capabilities, and reports a configuration failure when the variable is unavailable. The loop never invokes interactive @@gh auth login@@ or @@gh auth refresh@@; those commands remain outside the loop because browser/configuration state is nondeterministic.
- Add every repository issue to user project 9 idempotently with @@gh project item-add 9 --owner kmosoti --url <issue-url>@@.
- Reconcile rather than blindly append: list project items, detect missing/duplicate/archive cases, and emit a machine-readable report.
- Preserve issue URLs/numbers as stable external identities.
- Do not infer Atom satisfaction from GitHub Project status.
- The Wave / Status / Fan In / Fan Out projection is the only readiness computation permitted in Python.
- The projection consumes GitHub's native @@blockedBy@@ node lists, never @@issueDependenciesSummary@@ and never the Markdown @@## Dependencies@@ prose.
- Reconciling newly created Atoms and recomputing all four derived fields after any issue closes is part of acceptance.
- This Atom is retired when #48 is accepted; the projection is deleted and the board becomes a projection of native Mission Graph state.
- Add an optional repository script or workflow for repeated reconciliation, but do not store a personal token in the repository.
- Verify all current Atoms appear in Project 9 and record the resulting item count/evidence.""",
    )
]
semantic[11] = [
    section_replacement(
        11,
        "Acceptance",
        """- @@ExecutionAttempt@@ records subject @@spec_revision@@, actor, exact base, timing/outcome, and the adapter-neutral source identities @@logical_change_id@@ and @@exact_state_id@@.
- Attempt failure cannot mutate the Atom specification.
- Candidate is immutable and references its exact producer attempt/base/source identity.
- Candidate fields include @@plan_revision@@, @@base_exact_state_id@@, @@base_frontier_seq@@, @@logical_change_id@@, @@exact_state_id@@, @@fencing_token@@, and @@frozen_at_event@@ (the @@EventSeq@@ of @@CandidateFrozen@@).
- Editing after candidate freeze creates a distinct Candidate and invalidates evidence bound to the previous exact state.
- Effect classes distinguish pure, hermetic, external read, idempotent write, compensatable write, irreversible, and judgment.
- Retry/replay policy is explicit per effect class, per @@docs/algorithms/reconciliation.md@@ section 8, and is realized as an exhaustive match with no @@_@@ wildcard.
- @@IntegrationCandidate@@ is a distinct record with its own identity, @@plan_revision@@, frozen integration manifest, @@base_frontier@@, @@base_frontier_seq@@, @@parent_candidates@@, @@exact_state_id@@, and @@frozen_at_event@@.
- Integration acyclicity is validated: every parent candidate's @@frozen_at_event@@ is strictly earlier than the IntegrationCandidate's @@frozen_at_event@@.""",
    )
]
semantic[12] = [
    section_replacement(
        12,
        "Acceptance",
        """- Event identity, type/schema identity, actor, subject, causal parents, timestamps, payload, and digest are represented, and every event carries a dense @@EventSeq@@ assigned by the canonical event log on append.
- The complete canonical event registry and projector contract is the normative registry in @@docs/algorithms/reconciliation.md@@ section 1; this Atom implements that full registry rather than restating a drift-prone closed list here.
- Rejection, invalidation, restoration, lease, capability, and observation events have explicit schemas and deterministic projector transitions; no event kind is silently ignored.
- Projector output is deterministic from the recorded history and has no hidden clock/network/model effects.
- Derived state can be rebuilt after discarding materialized projections, with duplicate-event semantics explicit and idempotent where the protocol permits.
- Unknown or invalid event forms reject deterministically rather than being silently ignored.
- G-218 owns the canonical event type and projector, G-224 owns duplicate-event idempotency, and G-261 owns substantive replay theorems beyond history congruence.""",
    )
]
semantic[13] = [
    section_replacement(
        13,
        "Acceptance",
        """- Acceptance predicates are explicit typed/versioned policy data with bounded evaluator interfaces.
- @@Blocked@@, @@Enabled@@, and @@Active@@ are derived deterministically.
- @@Satisfied(a)@@ requires an admitted frontier: @@a@@'s Candidate is a transitive parent of the admitted @@IntegrationCandidate@@ and every required verifier is discharged on that integration, or inherited under a @@compositional = true@@ manifest entry.
- @@satisfaction_index@@ is established only while applying @@AtomSatisfied@@ or a guarded @@SatisfactionRestored@@ event, and is cleared only by @@SatisfactionInvalidated@@; no cache or evidence-only inference can create satisfaction.
- @@SatisfactionRestored@@ uses the same @@FrontierVersion@@ and @@WitnessGuard@@ preconditions as admission, requires the Atom Candidate in the transitive parents of the current accepted frontier, and requires fresh passing evidence for every required verifier on that current frontier.
- @@SatisfactionInvalidated@@ has exactly five reasons, each with an explicit producer and trigger: @@spec_revision_superseded@@ (projector / @@PlanSelected@@ pinning a different revision), @@plan_superseded@@ (projector / @@PlanSelected@@ removing membership), @@evidence_retracted@@ (evidence store / @@EvidenceRetracted@@ naming counted evidence), @@verifier_manifest_changed@@ (projector / @@PlanSelected@@ adding an uncounted required verifier), and @@integration_regression@@ (coordinator / failing @@EvidenceRecorded@@ for an integration containing the Atom Candidate).
- The invalidation reason, producer, trigger, current accepted frontier, and witness are recorded in canonical history; restoration never creates a new Candidate.
- @@Satisfied@@ is produced only by evaluating the acceptance predicate over fresh exact-subject evidence obtained through #15's fingerprint interfaces; a missing evidence interface fails closed.
- The seven readiness sub-predicates are implemented exactly as defined in @@docs/spec/mission-graph.md@@ @@## Readiness predicate definitions@@, and differential-tested against Lean through #7.
- Initiative satisfaction is evaluated from its own acceptance contract rather than automatically inheriting child completion; no mutable @@status = done@@ field can bypass acceptance.
- G-206 defines @@dependenciesSatisfied@@ over the real dependency graph and @@Frontier.Satisfied@@, not an environment oracle field.""",
    )
]


def closure_intent(number: int) -> dict[str, object]:
    return {
        "kind": "canonical",
        "atom": "issue",
        "runbook_sections": [1, 2, "6.6"],
        "verifier_source": "verification-section",
        "atom_specific_verifier_ids": [f"atom-{number}-acceptance"],
        "record_path": "artifacts/atoms/<N>/closure.json",
        "verifier_log_path": "artifacts/atoms/<N>/verifiers/<verifier_id>.log",
    }


transforms: list[dict[str, object]] = []
for number in range(1, 78):
    transform: dict[str, object] = {
        "issue": number,
        "target_crates": target_sets[number],
        "closure_wording": closure_intent(number),
    }
    if number in semantic:
        transform["replacements"] = semantic[number]
    transforms.append(transform)

for number in (78, 79):
    transforms.append(
        {
            "issue": number,
            "add_labels": ["duplicate"],
            "expected_title": rows[number]["title"],
            "expected_state": rows[number]["state"],
        }
    )

manifest = {
    "format": "gordian-atom-contract-normalization-v1",
    "atom": 70,
    "repository": "kmosoti/gordian",
    "original_plan_sha256": "0" * 64,
    "description": (
        "Committed declarative source for the bounded #70 repair of live Atom contracts; "
        "the issue snapshot used to generate this file is not a runtime dependency."
    ),
    "transforms": transforms,
}
def finalize_manifest(payload: dict[str, object]) -> None:
    from gordian_orchestration.normalization_journal import (
        operation_plan_digest,
        parse_manifest,
        plan_normalization,
    )

    parsed = parse_manifest(payload, repository="kmosoti/gordian")
    payload["original_plan_sha256"] = operation_plan_digest(
        plan_normalization(cached_issues(), parsed, repository="kmosoti/gordian").as_json_object()
    )


finalize_manifest(manifest)
(ROOT / "docs/implementation/atom-contract-normalization.json").write_text(
    json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
    encoding="utf-8",
)
