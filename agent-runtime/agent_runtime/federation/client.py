"""Backwards-compatible re-export — the canonical client now lives in __init__.py."""

from . import FederationClient  # noqa: F401

__all__ = ["FederationClient"]
