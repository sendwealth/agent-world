"""Lightweight .env file loader for the agent runtime.

Loads environment variables from a ``.env`` file in the current working
directory (or an explicit path) into ``os.environ``.  Existing
environment variables are **not** overwritten.

Uses ``python-dotenv`` if available; otherwise falls back to a simple
line-based parser that handles the most common ``.env`` formats:

- ``KEY=VALUE``
- Quoted values: ``KEY="VALUE"`` or ``KEY='VALUE'``
- Comments (``#``) and blank lines are ignored.
"""

from __future__ import annotations

import logging
import os
import re
from pathlib import Path

logger = logging.getLogger(__name__)

# Regex for KEY=VALUE lines (with optional quotes)
_ENV_RE = re.compile(
    r"""^\s*"""
    r"""([A-Za-z_][A-Za-z0-9_]*)"""  # key
    r"""\s*=\s*"""
    r"""(?:"([^"]*)"|'([^']*)'|(.*?))"""  # value: double-quoted, single-quoted, or bare
    r"""\s*$"""
)


def load_dotenv(path: str | Path | None = None) -> int:
    """Load a ``.env`` file into ``os.environ``.

    Args:
        path: Path to the .env file.  Defaults to ``.env`` in the current
              working directory.

    Returns:
        Number of variables loaded (newly set).
    """
    if path is None:
        path = Path.cwd() / ".env"
    else:
        path = Path(path)

    if not path.exists():
        logger.debug("No .env file at %s", path)
        return 0

    # Try python-dotenv first (handles edge cases better)
    try:
        from dotenv import dotenv_values  # type: ignore[import-untyped]

        values = dotenv_values(path)
        count = 0
        for key, value in values.items():
            if value is not None and key not in os.environ:
                os.environ[key] = value
                count += 1
        logger.info("Loaded %d variables from %s (via python-dotenv)", count, path)
        return count
    except ImportError:
        pass

    # Fallback: manual parser
    return _parse_dotenv(path)


def _parse_dotenv(path: Path) -> int:
    """Parse a .env file manually and set variables in os.environ."""
    count = 0
    with open(path) as f:
        for line_num, line in enumerate(f, 1):
            line = line.strip()

            # Skip empty lines and comments
            if not line or line.startswith("#"):
                continue

            match = _ENV_RE.match(line)
            if match is None:
                logger.warning("Skipping invalid .env line %d: %s", line_num, line)
                continue

            key = match.group(1)
            # Value: try double-quoted, then single-quoted, then bare
            value = match.group(2)  # double-quoted
            if value is None:
                value = match.group(3)  # single-quoted
            if value is None:
                value = match.group(4)  # bare

            if value is not None and key not in os.environ:
                os.environ[key] = value
                count += 1

    logger.info("Loaded %d variables from %s (manual parser)", count, path)
    return count
