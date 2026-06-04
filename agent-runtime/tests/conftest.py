"""Shared test configuration and fixtures."""
import asyncio
import sys
from pathlib import Path

# Ensure project root is on sys.path for protocol imports
_PROJECT_ROOT = Path(__file__).resolve().parent.parent.parent
if str(_PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(_PROJECT_ROOT))


if sys.version_info < (3, 10):
    import pytest

    @pytest.fixture(autouse=True)
    def _reset_event_loop_policy():
        """Reset event-loop policy state after each test (Python 3.9 compat).

        In Python 3.9, ``asyncio.set_event_loop()`` (called by asyncio.run()
        and some pytest-asyncio versions) leaves ``_set_called=True`` in the
        thread-local event-loop policy.  This prevents subsequent code from
        creating new loops, causing ``RuntimeError: There is no current event
        loop`` in downstream tests that instantiate ``asyncio.Lock()`` etc.

        The fixture resets the policy state after every test so that later
        tests always start with a clean slate.
        """
        yield
        policy = asyncio.get_event_loop_policy()
        policy._local._set_called = False
        policy._local._loop = None
