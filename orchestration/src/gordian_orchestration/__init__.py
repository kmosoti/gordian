"""Thin experiment orchestration around the Rust Gordian substrate.

Submodules, none of which own Mission Graph semantics:

- `runner` runs external Gordian, Jujutsu, verifier, and agent processes.
- `gh` is the single entry point for GitHub CLI invocations.
- `provenance` stamps source and environment identity onto every report.
- `github_project` reconciles issue membership into the temporary Project 9 projection.
- `atom_registry` audits and captures the temporary issue-contract registry.
- `bootstrap_claims` performs fail-closed credential preflight and expiring claims.
- `derive_status` projects GitHub's native blocked-by graph onto Project 9's derived
  Wave, Fan In, Fan Out and Status fields. It is deleted when #48 lands.
"""

from .provenance import Provenance
from .runner import CommandResult, run

_WORKLOAD_EXPORTS = {
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
    "validate_golden_manifest",
    "validate_workload",
    "workload_digest",
}


def __getattr__(name: str):
    """Load workload exports on demand, keeping ``python -m`` warning-free."""
    if name not in _WORKLOAD_EXPORTS:
        raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
    from importlib import import_module

    workloads = import_module(".workloads", __name__)
    value = getattr(workloads, name)
    globals()[name] = value
    return value


def __dir__() -> list[str]:
    return sorted(set(globals()) | _WORKLOAD_EXPORTS)


__all__ = [
    "CommandResult",
    "GENERATOR_NAME",
    "GENERATOR_VERSION",
    "Provenance",
    "SEED_MATRIX",
    "SOURCE_SCHEMA_VERSION",
    "WORKLOAD_SCHEMA_VERSION",
    "WorkloadError",
    "canonical_json",
    "extract_repository_source",
    "generate_repository",
    "generate_repository_derived",
    "generate_synthetic",
    "run",
    "validate_golden_manifest",
    "validate_workload",
    "workload_digest",
]
