"""Thin experiment orchestration around the Rust Gordian substrate.

Submodules, none of which own Mission Graph semantics:

- `runner` runs external Gordian, Jujutsu, verifier, and agent processes.
- `gh` is the single entry point for GitHub CLI invocations.
- `provenance` stamps source and environment identity onto every report.
- `github_project` reconciles issue membership into the temporary Project 9 projection.
- `derive_status` projects GitHub's native blocked-by graph onto Project 9's derived
  Wave, Fan In, Fan Out and Status fields. It is deleted when #48 lands.
"""

from .provenance import Provenance
from .runner import CommandResult, run

__all__ = ["CommandResult", "Provenance", "run"]
