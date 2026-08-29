"""Thin experiment orchestration around the Rust Gordian substrate."""

from .runner import CommandResult, run

__all__ = ["CommandResult", "run"]
